// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The lot developer (`cSC4LotDeveloper`, class `0xA990BFE0`),
//! `docs/specs/lot-developer.md`. Section references (`§n`) point at that
//! spec.
//!
//! # Structure
//!
//! - [`LotDeveloper`]: the city's one lot developer (component slot 16).
//!   It holds the saved state of §2.1 (terrain extents, the night deque and
//!   its cursor, the night mode, the lit flag), the unsaved state of §2.2
//!   (daily cycle, latches and snapshots, zot gate, force-empty flag, night
//!   accumulator and batch size) and the tuning of §3.5. Its methods are the
//!   interface slots of §11.1 whose behaviour the spec states:
//!   `Init` / `Shutdown` (§1.3, §1.4), `DoMessage` (§3.2), the daily pass
//!   and `UpdateLot` (§4) with every test of §5, construction (§6),
//!   demolition and explosion with rubble (§7), the night lights (§8), the
//!   power-changed handler (§9), the terrain and cost slots of §11.4, the
//!   geometry of §17 (altitudes, levelling, foundations, object positions,
//!   boundary textures) and the save record (§13).
//! - [`LotDeveloperStatics`]: the file-local `cRZRandom` (`0x00B49AF0`,
//!   §10.1) that draws R2–R14, the C `rand()` state ([`CrtRand`], R15,
//!   foundations §12.6) and the draw trace ([`Draw`]). Hand the same
//!   [`SharedStatics`] to the instance (one object per process, like the
//!   original's static).
//! - [`LotDeveloperHost`]: everything outside the lot developer — the city,
//!   the simulator, the message server, the lot manager and its safe
//!   iterator, lots, lot configurations and their manager, buildings and
//!   other occupants, the occupant and prop managers, the BDS, growth
//!   developers, power, plumbing, traffic, demand, flora, terrain,
//!   demolition (including `ModifyTerrainHeight`, the only terrain writer,
//!   §17.3), business units, the 24-hour clock, the animation-tick manager,
//!   the zone manager, the retaining-wall helpers and the resource manager.
//!   Objects are host tokens (`u64`); every occupant token is the occupant
//!   itself (buildings, props, foundations, base-texture occupants).
//! - [`TerrainView`]: the slots that read no lot-developer state but the
//!   tuning, the stored extents and cell counts and the boundary map —
//!   15–32 and the §17 geometry (39, 40, 42, 43, 48–61) — so a host can
//!   answer them while the lot developer is busy. `terrain.rs` (15–18, 22,
//!   25–32), `levelling.rs` (19–21), `positions.rs` (39, 40), `boundary.rs`
//!   (42, 43), `foundation.rs` (48–61).
//! - `tuning.rs`: [`Tuning`] (§3.5). `geometry.rs`: the slope and cost
//!   formulas of §11.4 and the rubble count of §7.3.
//!
//! # Choices (`[choice]`), limits and inferences (`[inferred]`)
//!
//! - **Seeding** (§10.1): the file-local generator is seeded by the host,
//!   [`LotDeveloperStatics::new`] (any seed, for tests) or
//!   [`LotDeveloperStatics::from_session`] (`derive_seed(session,
//!   0x6C09C0F4)`, foundations §6.3). R1 is a temporary generator whose
//!   LCG state is the location seed `(x << 16) + z` (wrapping); only its
//!   integer path is drawn, so no clock is read. R15 draws from the statics'
//!   own [`CrtRand`] (state 1 by default; [`LotDeveloperStatics::crt`] may be
//!   overwritten by the host).
//! - **Time slicer** (§4.1): the 5 ms CPU-cycle slicer is replaced by a
//!   deterministic work budget: at most [`LotDeveloper::daily_work_budget`]
//!   lots are updated per delivery of the day message (default
//!   [`DEFAULT_DAILY_WORK_BUDGET`] = `u32::MAX`, effectively unbounded). As
//!   in the original the budget is checked before the end-of-list test, and
//!   it only changes how many calls a day takes, never which lots are
//!   visited or in what order.
//! - **Reference counts** are the plugin's except where the spec states a
//!   lasting one: the night deque's reference per building is taken and
//!   dropped with [`LotDeveloperHost::building_add_ref`] /
//!   [`LotDeveloperHost::building_release`]. Transient AddRef / Release
//!   pairs inside one call (the lot in `StartConstruction`, list elements,
//!   rubble props) are not modelled. Lists the original receives in
//!   caller-made containers come back as owned `Vec`s; where the lot manager
//!   **appends** (`GetLotsSurroundingLot` / `Rect`, §5.7) the core passes
//!   one `&mut Vec` through the four side flags, as the original passes one
//!   list.
//! - **The safe lot iterator** (§4.1, §15.1) is a host object
//!   ([`LotDeveloperHost::safe_lot_iterator`]): the lot manager keeps it
//!   valid across deletions; the core only rewinds, advances and frees it.
//! - **`UpdateLot` purpose 0** (§4.2): "stop (no travel test)" ends the
//!   purpose-0 block; the business-unit step 5 still runs *[inferred]*.
//! - **`PickLotBaseTextureForLot`** wealth (§6.6) *[inferred]*: the
//!   occupant wealth when non-zero, else the building profile's wealth when
//!   the lot has a building type and that wealth is non-zero, else 1. The
//!   decay of a condition above 6 is 2.
//! - **Neighbour building types** in `PickUnitToJoin` and
//!   `NeedsAnchorBuilding` (§5.7) are read with `GetBuildingType(false)`;
//!   the lot's own (step 1) with `GetBuildingType(true)` as stated.
//! - **Business-unit dissolution** (§5.7, answers Q10): when member `m`
//!   joins `u'`, every lot of the copy, `m` included, is removed from the
//!   unit it is in (if any) and added to `u'`, in copy order.
//! - **`UpdateTravelDesignates`** with an anchor (§5.7 table, answers Q9):
//!   a member designate with road access is kept; others are cleared (a null
//!   designate is not); then every lot but the anchor gets the anchor.
//! - **Zot property reads** (§5.6): reading the current zot never creates
//!   the property (absent reads as 0); `AddBuildingZotProp` creates it, and
//!   so do the counters of §5.1–§5.4 (created as 0 on their read).
//!   `RemoveLotProp` removes only props that **have** `0x48E95539` equal to
//!   the zot (a prop without the property never matches).
//!   `StartDemolishLot` writes the zot property 0 unconditionally.
//! - **`StartConstruction`** tests slot 21's result as a bool: the low byte
//!   of its count (§17.4 "AL is the count's low byte") *[inferred]*.
//! - **`GetPositionForBuilding`** with no configuration or primary object
//!   writes nothing (§17.6); the callers' position is then `(0, 0, 0)` and
//!   orientation 0 *[choice]*.
//! - **Light** (§8.4, answers Q8): the building's cells find the lot; the
//!   props are looked up in the **lot's** bounding rectangle (lot slot 14).
//! - **Day / night message** (§8.2): `data2 = 0` is night, any other value
//!   day.
//! - **`0xE99B3D4D`** (§6.5) looks the lot up with `GetLot(x, z, true)`,
//!   like `0xE8BDA2E7`.
//! - **Null pointers** the original would dereference: a night-deque slot
//!   left null by a failed save read is skipped by `Light`; a missing
//!   business unit in `RemoveLotFromItsBusinessUnit` returns false; a
//!   missing demand node reads as no commute check; a missing
//!   `0x60000C40` configuration stops the coffee bar; a failed
//!   `AddLotProp` skips the follow-up calls.
//! - **`ClearLotBuildings`** stops when `GetFirstBuilding` returns the same
//!   building twice in a row (the host failed to remove it) or after
//!   [`MAX_CLEAR_BUILDINGS`] buildings, instead of looping forever.
//! - **Save read** (§13): at most [`MAX_SAVED_BUILDINGS`] deque slots are
//!   read; reading stops at the first failed reference (the record is then
//!   rejected); a cursor index past the end is clamped to the end.
//! - **Undefined returns** (§11.1): slots 13, 14, 34, 35 return true, slot
//!   44 nothing (the plugin returns the lot), `Release` is the plugin's.
//! - **Left out**: the dead parts (static byte `0x00B49AE8`, the scanning
//!   iterator, `RotateLotToFaceRoad`, which the never-set byte
//!   `0x00B49AE9` disables, §4.2 / §12.1 quirk 2), the `0x278E6003`
//!   handler (ignored, quirk 15), the blocking filter object (the host's
//!   `occupants_in_bbox` applies it), `GetJobs` / `GetBoundingRect` calls
//!   whose results are unused, and the slots §17 does not cover (41, 45,
//!   62–64, the retaining-wall helpers, `RemoveNetworkConnectionNodes`:
//!   host calls, no draws). `UpdateLotObjectAlitudes` and the parameters of
//!   `AddNetworkConnectionNodes` are the core's (§6.5, LD17). The boundary
//!   map is the constant table of §17.7 ([`BOUNDARY_RULES`]); `Init` /
//!   `Shutdown` only switch it on and off.
//!
//! Host callbacks must not call back into the lot developer.

pub mod archive;
mod boundary;
mod foundation;
mod geometry;
mod levelling;
mod positions;
mod simulator;
mod terrain;
#[cfg(test)]
mod tests;
mod tuning;

pub use boundary::{empty_lot_extra, zone_has_no_empty_texture, BoundaryRule, BOUNDARY_RULES};
pub use foundation::{
    kind as foundation_kind, IID_FOUNDATION_OCCUPANT, LOT_FOUNDATION_FLAGS, OCCUPANT_TYPE_FOUNDATION,
    PROP_BUILDING_FOUNDATION_TYPE,
};
pub use levelling::NO_SLOPE_TEST_FROM;

pub use geometry::{area_slope_of, area_slope_with_precision, development_cost, rubble_count, RubbleShape};
pub use simulator::{
    is_empty_lot_suitable_for_zot, Draw, DrawKind, LotDeveloper, LotDeveloperStatics, SharedStatics,
    DEFAULT_DAILY_WORK_BUDGET, MAX_CLEAR_BUILDINGS, MAX_SAVED_BUILDINGS, NIGHT_THRESHOLD_MS, RECORD_VERSION,
    TICK_PRIORITY,
};
pub use terrain::{TerrainView, EDGE_STEP, FLT_MAX, MAX_EDGE_SAMPLES};
pub use tuning::{prop, Tuning, DEFAULT_FOUNDATION_TYPE, TUNING_KEY};

use std::rc::Rc;

pub use crate::development::BuildingProfile;
pub use crate::grid::CellRect;
use crate::property::PropertyHolder;
pub use crate::rng::{CrtRand, RzRandom};
use crate::stream::RefValue;

/// A position or bounding-box corner in metres.
pub type Vec3 = [f32; 3];

/// Class ID (`GetGZCLSID`, §1.1).
pub const CLSID_LOT_DEVELOPER: u32 = 0xA990_BFE0;
/// `GetSimulatorType` (slot 5, §1.1).
pub const SIMULATOR_TYPE: u32 = 0x6818_E5C9;
/// Interface IDs answered by `QueryInterface` (§1.1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_LOT_DEVELOPER: u32 = 0x4818_E5D2;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// `cISC4BuildingOccupant` (night deque elements, §8.1, §13).
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;

/// Whether `QueryInterface` answers `iid` (§1.1; unknown IIDs → false).
pub fn answers_iid(iid: u32) -> bool {
    matches!(
        iid,
        IID_UNKNOWN | IID_LOT_DEVELOPER | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE
    )
}

/// Agent registration (§3.1).
pub const AGENT_NAME: &str = "cSC4LotDeveloper";
pub const AGENT_FLAGS: u32 = 2;
pub const AGENT_LEVELS: [(u32, u32); 3] = [(0, MSG_ONE_SHOT), (4, MSG_SIM_NEW_DAY), (6, MSG_SIM_NEW_MONTH)];

/// Messages (§3.2).
pub mod msg {
    pub const ONE_SHOT: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const DAY_NIGHT_CHANGED: u32 = 0x4477_8234;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;
    pub const LOT_CONDITION_CHANGED: u32 = 0xE99B_3D4D;
    pub const TERRAIN_CHANGED: u32 = 0x278E_6003;
    pub const POWER_CHANGED: u32 = 0x8A07_AE00;
    pub const CITY_ESTABLISHED: u32 = 0x26D3_1EC4;
    pub const TRAFFIC_MAP_CHANGED: u32 = 0x6924_7DC7;
    pub const ANIMATION_TICK: u32 = 0x86AD_10EE;
    /// Posted (§3.3).
    pub const LOT_DEVELOPER_NOTIFICATION: u32 = 0x4C04_DE69;
    pub const NIGHT_LIGHTS_UPDATED: u32 = 0xA6B7_9621;
    /// Occupant messages (§3.3).
    pub const OCCUPANT_LIGHT: u32 = 0x09D8_E0B6;
    pub const OCCUPANT_CHANGED: u32 = 0x99EF_1145;
}
use msg::{ONE_SHOT as MSG_ONE_SHOT, SIM_NEW_DAY as MSG_SIM_NEW_DAY, SIM_NEW_MONTH as MSG_SIM_NEW_MONTH};

/// The nine subscribed messages, in `Init` order (§1.3 step 9).
pub const SUBSCRIBED: [u32; 9] = [
    msg::DAY_NIGHT_CHANGED,
    msg::OCCUPANT_INSERTED,
    msg::OCCUPANT_REMOVED,
    msg::LOT_STATE_CHANGED,
    msg::LOT_CONDITION_CHANGED,
    msg::TERRAIN_CHANGED,
    msg::POWER_CHANGED,
    msg::CITY_ESTABLISHED,
    msg::TRAFFIC_MAP_CHANGED,
];

/// Occupant types (§3.2, §6.2).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
pub const OCCUPANT_TYPE_NETWORK: u32 = 0xC772_BF98;
pub const OCCUPANT_TYPE_FLORA: u32 = 0x7475_8926;
/// The prop occupant type of `GetOccupantsByStandardCityCells` in `Light`
/// (§8.4).
pub const OCCUPANT_TYPE_PROP: u32 = 0xA823_821E;
/// The base-texture occupant type (§6.6, §17.7).
pub const OCCUPANT_TYPE_BASE_TEXTURE: u32 = 0x497F_6D9D;
/// The five bytes after object byte 3 of a normal base-texture record
/// (§6.6: they default to `0xFF`).
pub const BASE_TEXTURE_EXTRA_DEFAULT: [u8; 5] = [0xFF; 5];

/// Lot properties (counters and the zot, §0 table).
pub mod lot_prop {
    pub const LOW_DESIRABILITY: u32 = 0xC999_E206;
    pub const NO_POWER: u32 = 0xE999_B11F;
    pub const NO_WATER: u32 = 0x8999_B20B;
    pub const TRAVEL: u32 = 0x8999_B10B;
    pub const ZOT: u32 = 0xC999_C45E;
}
/// The zot number on a zot prop (§5.6).
pub const PROP_ZOT_NUMBER: u32 = 0x48E9_5539;
/// The night-light flag of a prop (§8.4).
pub const PROP_NIGHT_LIGHT: u32 = 0x4A9F_188B;

/// Zots (§5.6) and their prop types.
pub mod zot {
    pub const NONE: u32 = 0;
    pub const POWER: u32 = 4;
    pub const WATER: u32 = 5;
    pub const ROAD: u32 = 6;
    pub const COMMUTE: u32 = 7;
    pub const POWER_PROP: u32 = 0x0FD1_0000;
    pub const WATER_PROP: u32 = 0x1C43_0000;
    pub const ROAD_PROP: u32 = 0x107A_0000;
    pub const COMMUTE_PROP: u32 = 0x1C44_0000;
}

/// Lot states (`cISC4Lot::HabitationState`).
pub mod lot_state {
    pub const CREATED: u32 = 0;
    pub const READY: u32 = 1;
    pub const CONSTRUCTING: u32 = 2;
    pub const OCCUPIED: u32 = 3;
    pub const ABANDONED: u32 = 4;
    pub const DESTRUCTING: u32 = 5;
    pub const DESTROYED: u32 = 6;
}

/// Vacate reasons (`cISC4Lot::VacateReason`, §4.3).
pub mod reason {
    pub const LOW_DESIRABILITY: u32 = 1;
    pub const NO_POWER: u32 = 2;
    pub const NO_WATER: u32 = 3;
    pub const COMMUTE: u32 = 4;
}

/// Configuration object types (§6.5, §6.6).
pub mod object_type {
    pub const PROP: u32 = 1;
    pub const TEXTURE: u32 = 2;
    pub const FLORA: u32 = 4;
    pub const WATER: u32 = 7;
    /// Type 7 read as the network objects of `AddNetworkConnectionNodes`
    /// (§6.5, LD17).
    pub const NETWORK: u32 = 7;
}

/// The coffee-bar cheat (§5.8).
pub const COFFEE_BAR_CFG: u32 = 0x6000_0C40;
pub const COFFEE_BAR_BUILDING: u32 = 0x05B8_0000;
/// `DemolishLots` flags of the coffee bar (§5.8).
pub const DEMOLISH_FLAGS: u32 = 0x800;
/// The anchor occupant group (§5.7).
pub const ANCHOR_GROUP: u32 = 0x3000;
/// Base-texture resources (§6.6): type and group.
pub const TEXTURE_RESOURCE: (u32, u32) = (0x7AB5_0E44, 0x0986_135E);
/// Rubble decal key (§7.2): type, group, instance (burnt / not).
pub const RUBBLE_DECAL: (u32, u32, u32, u32) = (0x7AB5_0E44, 0x1ABE_787D, 0x49C8_0EC7, 0x2A2E_F55D);
/// The demand node of the commute check (§3.2): `GetDemand(0x2000, 0x20000)`.
pub const JOBS_DEMAND: (u32, u32) = (0x2000, 0x2_0000);

/// The BDS cheats read (slots 54, 55, 58).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cheat {
    IgnorePlumbing,
    IgnorePower,
    CoffeeBar,
}

/// A message as `DoMessage` sees it (§3.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LotDevMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message.
    pub void1: Option<u64>,
    /// `GetIGZUnknown()`: the lot of a state / condition message.
    pub lot: Option<u64>,
}

impl LotDevMessage {
    pub fn of_type(msg_type: u32) -> LotDevMessage {
        LotDevMessage {
            msg_type,
            ..LotDevMessage::default()
        }
    }
}

/// One lot configuration object: the 0x34-byte record of
/// `GetLotConfigurationObjectArray` (cfg slot 23), slots 24 and 26 (§17.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CfgObject {
    /// `+0x00`: 1 prop, 2 base texture, 4 flora, 7 water, … .
    pub object_type: u32,
    /// `+0x01`, the flag byte; bit 0 set keeps a prop object out of
    /// `AddLotProps` (§6.5).
    pub flags: u8,
    /// `+0x02`: the orientation (slot 39, base-texture records).
    pub orientation: u8,
    /// `+0x03`: the mirror flag, object byte 3 of base-texture records.
    pub byte3: u8,
    /// `+0x04`, `+0x08`, `+0x0C`: the lot-space offsets x, y, z.
    pub offset: [f32; 3],
    /// `+0x10`, `+0x14`: the footprint's max x, max z.
    pub footprint_max: [f32; 2],
    /// `+0x18`, `+0x1C`: the footprint's min x, min z.
    pub footprint_min: [f32; 2],
    /// `+0x20`: the foundation type (slots 54, 58).
    pub foundation: u32,
    /// `+0x24`: the lot tag given to the occupant made from it.
    pub tag: u32,
    /// `+0x28`: the object's ID list (families not expanded).
    pub ids: Vec<u32>,
}

/// One base-texture record handed to the base-texture occupant (§6.6,
/// §17.7): 14 bytes on disk, 16 in memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureRecord {
    pub texture: u32,
    pub x: i32,
    pub z: i32,
    /// The orientation (normal textures) or rotation (empty-lot textures).
    pub orientation: u8,
    /// Object byte 3 (normal textures) or the flip (empty-lot textures).
    pub byte3: u8,
    /// The five bytes after it: `0xFF` for normal textures
    /// ([`BASE_TEXTURE_EXTRA_DEFAULT`]), the zone's colour, alpha and 3
    /// for empty-lot textures ([`empty_lot_extra`]).
    pub extra: [u8; 5],
    /// The frame byte (`0xFF`: the occupant draws one).
    pub frame: u8,
}

/// One network connection node of `AddNetworkConnectionNodes` (§6.5,
/// LD17): the arguments of network manager slot 20
/// `CreateConnectionNode(&n, node_type, &cell, b, c, d, orientation)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionNode {
    /// `ids[0]`.
    pub node_type: u32,
    /// The object's cell (`floor(pos / 16)`, the rounding of city slot 95).
    pub x: i32,
    pub z: i32,
    /// `rotl32(ids[1], 8·orientation)`, `rotl32(ids[2], 8·orientation)`.
    pub b: u32,
    pub c: u32,
    /// `ids[3]` with at least 4 IDs, else 0.
    pub d: u32,
    /// Slot 39's orientation (passed as a byte).
    pub orientation: u8,
}

/// The burnt-building prop made from a building by `ExplodeLot` (§7.2
/// step 2): its occupant and bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurntBuilding {
    pub occupant: u64,
    pub min: Vec3,
    pub max: Vec3,
}

/// Everything the lot developer reads from, or does to, the rest of the
/// game (§14). Objects are host tokens. The contracts are those the specs
/// state; the owning spec is named per group.
pub trait LotDeveloperHost {
    /// Live arithmetic context, not saved state (foundations §18.4).
    fn invocation_precision(&self) -> crate::x87::Precision {
        crate::x87::Precision::Bits24
    }

    // ---- lifecycle (§1.3, §1.4) ----
    /// `Init` step 3: take the services still missing (city, lot manager,
    /// occupant manager, prop manager, BDS, power, plumbing, flora, terrain,
    /// traffic; demolition without a check). False if any is missing.
    fn acquire_services(&self) -> bool;
    /// `Shutdown`: release the services, the BDS and the blocking filter.
    fn release_services(&self);
    /// The "Building Development" tuning exemplar (§3.5), if loaded.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;
    /// Whether the simulator global is set (`Init` step 8).
    fn has_simulator(&self) -> bool;
    /// Simulator slot 19 `AddAgent(this+4, level, "cSC4LotDeveloper", 2)`
    /// for `message`.
    fn add_agent(&self, level: u32, message: u32);
    /// Simulator slot 20 `RemoveAgent(this+4)`.
    fn remove_agent(&self);
    /// `Init` step 9: take the message server. False when missing.
    fn acquire_message_server(&self) -> bool;
    fn release_message_server(&self);
    /// Message server slots 5 / 6.
    fn subscribe(&self, message: u32);
    fn unsubscribe(&self, message: u32);
    /// The animation-tick manager's slots 5 / 6 with (this+4, `0xC0`).
    fn register_animation_tick(&self);
    fn unregister_animation_tick(&self);
    /// `MessagePost` of a new standard message (queued, §3.3): `lot` is the
    /// IGZUnknown (0x4C04DE69), data2–4 are 0.
    fn post_message(&self, msg_type: u32, data1: u32, lot: Option<u64>);

    // ---- city, simulator, terrain ----
    /// City slots 93 / 94.
    fn city_cell_counts(&self) -> (i32, i32);
    /// City slots 91 / 92 (`CellWidthX`, `CellWidthZ`).
    fn city_cell_widths(&self) -> (f32, f32);
    /// City slot 95 `PositionToCell(x, z)`: the cell `(cx, cz)`. It returns
    /// nothing (§17.2, LD18): AL is left from the x87 status word, so no
    /// result is read.
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);
    /// City slot 96 `CellCornerToPosition(x, z)` (§17.6).
    fn cell_corner_to_position(&self, x: i32, z: i32) -> (f32, f32);
    /// City slot 97 `CellCenterToPosition(x, z)` (§17.5.4).
    fn cell_center_to_position(&self, x: i32, z: i32) -> (f32, f32);
    /// Simulator slot 8 `GetSimDateNumber`.
    fn sim_date(&self) -> u32;
    /// Terrain slots 7 / 8 (cell counts) and 6 (`CellWidth`).
    fn terrain_cell_counts(&self) -> (i32, i32);
    fn terrain_cell_width(&self) -> f32;
    /// Terrain slot 12 `GetAltitude(index)` / slot 31 `GetWaterLevel(index)`
    /// at vertex index `x + z · vertsX`.
    fn terrain_altitude(&self, index: i32) -> f32;
    fn terrain_water_level(&self, index: i32) -> f32;
    /// Terrain slot 13 `GetAltitude(x, z)` / slot 30 `GetWaterLevel(x, z)`
    /// at a position in metres (§17.2, §17.6).
    fn terrain_altitude_at(&self, x: f32, z: f32) -> f32;
    fn terrain_water_level_at(&self, x: f32, z: f32) -> f32;
    /// Terrain slot 37 `IsWaterVertex(x, z)` (slot 19, §17.4).
    fn terrain_is_water_vertex(&self, x: i32, z: i32) -> bool;
    /// Terrain slot 34 `CellHasWater(x, z)` (§17.5.4).
    fn terrain_cell_has_water(&self, x: i32, z: i32) -> bool;
    /// Zone manager slot 15 `GetZoneDragColor(zone)` and slot 16
    /// `GetZoneDisplayAlpha()` (§17.7).
    fn zone_drag_color(&self, zone: i32) -> u32;
    fn zone_display_alpha(&self) -> u8;
    /// The object at `0x00B43DD8`, field +0x10: the zot orientation (§5.6).
    fn view_rotation(&self) -> i32;
    /// 24-hour clock slot 6 `GetVisualTimeMode` (§8.2).
    fn visual_time_mode(&self) -> u32;
    /// Resource manager slot 14: whether `{type, group, instance}` exists.
    fn resource_exists(&self, type_id: u32, group: u32, instance: u32) -> bool;

    // ---- lot manager (zones.md §8) ----
    /// Slot 16 `GetLotCount`.
    fn lot_count(&self) -> i32;
    /// Slot 15 `GetSafeLotIterator`: a new registered iterator at the front.
    fn safe_lot_iterator(&self) -> u64;
    /// `iterator.cur := first node` (§4.1).
    fn safe_lot_iterator_rewind(&self, it: u64);
    /// The current lot, advancing the iterator; `None` at the end.
    fn safe_lot_iterator_next(&self, it: u64) -> Option<u64>;
    /// Unregister and free the iterator (C free).
    fn safe_lot_iterator_free(&self, it: u64);
    /// Slot 6 `GetLot(x, z, originOnly)`.
    fn get_lot(&self, x: i32, z: i32, origin_only: bool) -> Option<u64>;
    /// Slot 7 `GetLotsInCellRect(list, x1, z1, x2, z2, originOnly)` on an
    /// empty list.
    fn lots_in_cell_rect(&self, rect: CellRect, origin_only: bool) -> Vec<u64>;
    /// Slot 9 `GetLotsSurroundingLot(lot, list, flags)`: appends to `list`
    /// (lots already in it are not appended again).
    fn lots_surrounding_lot(&self, lot: u64, list: &mut Vec<u64>, flags: u32);
    /// Slot 10 `GetLotsSurroundingRect(rect, list, flags)`: appends, as
    /// slot 9.
    fn lots_surrounding_rect(&self, rect: CellRect, list: &mut Vec<u64>, flags: u32);
    /// Slot 24 `DeleteLot`.
    fn delete_lot(&self, lot: u64) -> bool;

    // ---- lots (cISC4Lot) ----
    fn lot_is_initialized(&self, lot: u64) -> bool;
    /// Slot 21 `GetLocation`.
    fn lot_location(&self, lot: u64) -> (i32, i32);
    /// Slot 22 `GetSize`.
    fn lot_size(&self, lot: u64) -> (u32, u32);
    fn lot_bounding_rect(&self, lot: u64) -> CellRect;
    /// Slot 15 `GetCenter`.
    fn lot_center(&self, lot: u64) -> Vec3;
    fn lot_zone_type(&self, lot: u64) -> i32;
    /// Slots 16 / 17 `GetAltitude` / `SetAltitude` (§17.5, §17.6).
    fn lot_altitude(&self, lot: u64) -> f32;
    fn lot_set_altitude(&self, lot: u64, altitude: f32);
    /// Slot 23 `GetFacing` (0–3; §17.6, §17.7).
    fn lot_facing(&self, lot: u64) -> i32;
    fn lot_flags(&self, lot: u64) -> u32;
    fn lot_set_flags(&self, lot: u64, flags: u32);
    fn lot_set_vertices_locked(&self, lot: u64, locked: bool);
    /// Slot 20 `UpdateSlope`.
    fn lot_update_slope(&self, lot: u64);
    fn lot_state(&self, lot: u64) -> u32;
    /// Slot 34 `SetState(state, wealth, reason)`; the lot posts
    /// `0xE8BDA2E7` (queued) when the state changes.
    fn lot_set_state(&self, lot: u64, state: u32, wealth: u32, reason: u32) -> bool;
    fn lot_condition(&self, lot: u64) -> u32;
    /// Slot 36 `SetCondition`; posts `0xE99B3D4D` (queued) when it changes.
    fn lot_set_condition(&self, lot: u64, condition: u32);
    /// Slot 37 `IsInLivableState`.
    fn lot_is_livable(&self, lot: u64) -> bool;
    fn lot_days_since_state_change(&self, lot: u64) -> u32;
    /// Slot 46 `GetBuildingType(flag)`.
    fn lot_building_type(&self, lot: u64, flag: bool) -> u32;
    fn lot_set_building_type(&self, lot: u64, building: u32);
    /// Slot 48 `GetBuilding`.
    fn lot_building(&self, lot: u64) -> Option<u64>;
    /// Slots 9 / 10.
    fn lot_configuration(&self, lot: u64) -> Option<u64>;
    fn lot_set_configuration(&self, lot: u64, cfg: Option<u64>);
    fn lot_occupant_wealth(&self, lot: u64) -> u32;
    /// Slot 61 `GetOccupancy(float&)`.
    fn lot_occupancy(&self, lot: u64) -> f32;
    /// Slots 65–67.
    fn lot_needs_power(&self, lot: u64) -> bool;
    fn lot_has_power(&self, lot: u64) -> bool;
    fn lot_set_has_power(&self, lot: u64, powered: bool);
    /// Slots 27–30.
    fn lot_has_job_access(&self, lot: u64) -> bool;
    fn lot_has_road_access(&self, lot: u64) -> bool;
    fn lot_has_organic_road_access(&self, lot: u64, purpose: u32) -> bool;
    fn lot_update_road_access(&self, lot: u64);
    /// Slot 63 `GetTravelJobs(wealth)`.
    fn lot_travel_jobs(&self, lot: u64, wealth: u32) -> f32;
    /// The travel designate (slot 59) and slot 60 `SetTravelDesignate`.
    fn lot_travel_designate(&self, lot: u64) -> Option<u64>;
    fn lot_set_travel_designate(&self, lot: u64, designate: Option<u64>);
    /// Slot 25 `GetUnit`.
    fn lot_unit(&self, lot: u64) -> Option<u64>;
    /// Slots 57 / 58: `lot.AddTravelPopulation(from)`.
    fn lot_reset_travel_population(&self, lot: u64);
    fn lot_add_travel_population(&self, lot: u64, from: u64);
    /// Slots 43 / 44.
    fn lot_is_historical(&self, lot: u64) -> bool;
    fn lot_set_historical(&self, lot: u64, historical: bool);
    /// Slots 52 / 55.
    fn lot_available_capacity(&self, lot: u64, dev_type: u32, flag: bool) -> u16;
    fn lot_set_population(&self, lot: u64, dev_type: u32, population: u16) -> bool;
    /// A lot property (variant slot 73); `None` when absent.
    fn lot_property(&self, lot: u64, id: u32) -> Option<u32>;
    /// Writes a lot property (variant slot 26), creating it (holder slot
    /// 13) when absent.
    fn lot_set_property(&self, lot: u64, id: u32, value: u32);

    // ---- lot configurations and their manager ----
    fn cfg_id(&self, cfg: u64) -> u32;
    fn cfg_size(&self, cfg: u64) -> (u32, u32);
    /// Slot 10 `GetMaximumSlopeBeforeFoundation`.
    fn cfg_max_slope_before_foundation(&self, cfg: u64) -> f32;
    /// Slot 11 `GetGrowthStage`.
    fn cfg_growth_stage(&self, cfg: u64) -> u8;
    /// Slot 14 `IsCompatibleWithWater`.
    fn cfg_is_compatible_with_water(&self, cfg: u64) -> bool;
    /// Slot 23 `GetLotConfigurationObjectArray`, in array order.
    fn cfg_objects(&self, cfg: u64) -> Vec<CfgObject>;
    /// Slot 24 `GetPrimaryBuildingConfigurationObject` (§17.1), `None`
    /// without one.
    fn cfg_primary_building_object(&self, cfg: u64) -> Option<CfgObject>;
    /// Slot 26 `GetPrimaryFoundationConfigurationObject` (§17.1), `None`
    /// without one.
    fn cfg_primary_foundation_object(&self, cfg: u64) -> Option<CfgObject>;
    /// Slot 27 `HasObjectsOfType`.
    fn cfg_has_objects_of_type(&self, cfg: u64, object_type: u32) -> bool;
    /// Manager slot 8 `GetLotConfiguration(id)`.
    fn get_lot_configuration(&self, id: u32) -> Option<u64>;
    /// Manager slot 27 `GetObjectIDArray(obj, ids)` for object `index` of
    /// `cfg`'s array: the IDs with families expanded in hash order.
    fn object_id_array(&self, cfg: u64, index: usize) -> Vec<u32>;

    // ---- slots and helpers §17 does not cover ----
    /// Slot 38 `GetFirstBuilding(lot)` (no reference).
    fn first_building(&self, lot: u64) -> Option<u64>;
    /// Slot 62 `UpdateLotRetainingWalls(lot)` (`0x005E37F0`).
    fn update_lot_retaining_walls(&self, lot: u64) -> bool;
    /// Lot slot 49 `GetLotOccupants` (§6.5 `UpdateLotObjectAlitudes`,
    /// LD17): the lot's occupants in list order (references kept for the
    /// call).
    fn lot_occupants(&self, lot: u64) -> Vec<u64>;
    /// Lot slot 11 `GetLotConfigurationObject(occ)` (§6.5, LD17): the
    /// configuration object the occupant was made from, `None` without one.
    fn lot_configuration_object(&self, lot: u64, occ: u64) -> Option<CfgObject>;
    /// Slot 45 `UpdateLotBaseTextureOccupant(lot, hadFoundation)`.
    fn update_lot_base_texture_occupant(&self, lot: u64, had_foundation: bool) -> bool;
    /// Slot 41 `GetLotBaseTextureOccupant(lot, occ, create)`: the
    /// base-texture occupant (as an occupant), `None` when the slot returns
    /// false.
    fn base_texture_occupant(&self, lot: u64, create: bool) -> Option<u64>;
    /// The base-texture occupant's slot 8 `SetTextureSpecification(records,
    /// n, 0)` (also with no record).
    fn base_texture_set(&self, occ: u64, records: &[TextureRecord]);
    /// `cSC4LotRetainingWallOccupant::AddRetainingWallOccupant(lot, flag)`
    /// (`0x005E3070`): flag 0 from `ClearLot` (§7.2), 1 from slot 55
    /// (§17.5.4).
    fn add_retaining_wall_occupant(&self, lot: u64, flag: u32);
    /// `cSC4LotRetainingWallOccupant::UpdateSurroundingRetainingWalls(lot)`
    /// (`0x005E3960`).
    fn update_surrounding_retaining_walls(&self, lot: u64);
    /// `AddNetworkConnectionNodes` steps 2-3 (§6.5, LD17): network manager
    /// slot 20 `CreateConnectionNode` for each node in order, every result
    /// appended to one list (also a failed one), then slot 21
    /// `AutoconnectLotNodes(lot, &list)` (also with no node), then the list's
    /// entries are released.
    fn add_network_connection_nodes(&self, lot: u64, nodes: &[ConnectionNode]);
    /// `RemoveNetworkConnectionNodes(lot)` (§6.5, LD17): the lot's network
    /// occupants with flag `0x200000` go to network manager slot 6.
    fn remove_network_connection_nodes(&self, lot: u64);
    /// §6.3: create a construction occupant, `SetLot(lot)`, insert it.
    fn start_construction_occupant(&self, lot: u64);

    // ---- buildings and other occupants ----
    /// QueryInterface of an occupant for `cISC4BuildingOccupant`.
    fn occupant_as_building(&self, occ: u64) -> Option<u64>;
    fn building_add_ref(&self, building: u64);
    fn building_release(&self, building: u64);
    /// Building slot 17 `SetLit`.
    fn building_set_lit(&self, building: u64, lit: bool);
    /// Occupant slot 7 `GetType`.
    fn occupant_type(&self, occ: u64) -> u32;
    /// Occupant slot 8 `GetPosition`.
    fn occupant_position(&self, occ: u64) -> Vec3;
    /// Occupant slot 10 `GetBoundingBox` (min, max).
    fn occupant_bounding_box(&self, occ: u64) -> (Vec3, Vec3);
    /// Occupant slot 11 `GetBoundingCityCells`.
    fn occupant_city_cells(&self, occ: u64) -> CellRect;
    /// Occupant slot 22 `IsOccupantGroup`.
    fn occupant_in_group(&self, occ: u64, group: u32) -> bool;
    /// Occupant slot 15 `PostOccupantMessage(msg, 0)`.
    fn occupant_post_message(&self, occ: u64, message: u32);
    /// Occupant slot 12 `SetRemovalFlags`.
    fn occupant_set_removal_flags(&self, occ: u64, flags: u32);
    /// Occupant slot 28 `SetLotTag`.
    fn occupant_set_lot_tag(&self, occ: u64, tag: u32);
    /// A uint32 property of an occupant (`None` when absent).
    fn occupant_property(&self, occ: u64, id: u32) -> Option<u32>;
    fn occupant_set_property(&self, occ: u64, id: u32, value: u32);
    /// Occupant slots 3 (`Init`), 29 (`SetFlag`), orientation and position.
    fn occupant_init(&self, occ: u64);
    fn occupant_set_flag(&self, occ: u64, flag: u32);
    /// Occupant slot 32 `IsFlagSet(flag)` (flag 1: inserted).
    fn occupant_is_flag_set(&self, occ: u64, flag: u32) -> bool;
    /// Occupant slot 4 `Shutdown` (§17.7 `RemoveLotBaseTextures`).
    fn occupant_shutdown(&self, occ: u64);
    fn occupant_set_orientation(&self, occ: u64, orientation: u32);
    fn occupant_set_position(&self, occ: u64, pos: Vec3);
    /// Occupant manager slot 20 `InsertOccupant(occ, 0)`.
    fn occupant_insert(&self, occ: u64);
    /// Occupant manager slot 21 `RemoveOccupant(occ, a, flags)`.
    fn occupant_remove(&self, occ: u64, a: u32, flags: u32);
    /// Occupant manager slot 24: occupant changed `(occ, 0)`.
    fn occupant_changed(&self, occ: u64);
    /// Occupant manager slot 24 with 1: the occupant moved
    /// (`UpdateLotObjectAlitudes`, §6.5, LD17).
    fn occupant_moved(&self, occ: u64);
    /// Occupant manager slot 36 `GetOccupantsByBBox` with the blocking
    /// filter (flora `0x74758926` only).
    fn occupants_in_bbox(&self, min: Vec3, max: Vec3) -> Vec<u64>;
    /// Occupant manager slot 41 `GetOccupantsByStandardCityCells(list,
    /// min, max, type, −1)`.
    fn occupants_in_cells(&self, rect: CellRect, occupant_type: u32) -> Vec<u64>;
    /// Occupant manager slot 37 (the box query with an occupant-type
    /// filter, flags −1) (§17.5.1), in list order.
    fn occupants_in_box(&self, min: Vec3, max: Vec3, occupant_type: u32) -> Vec<u64>;
    /// Building slot 14 `GetOrientation` (§17.5.5).
    fn building_orientation(&self, building: u64) -> i32;

    // ---- foundations (cISC4FoundationOccupant, §17.5) ----
    /// `QueryInterface(0xC8FCE101)` then slot 9 `GetKind()`; `None` when the
    /// occupant is not a foundation.
    fn foundation_kind(&self, occ: u64) -> Option<i32>;
    /// Slot 4 `GetFoundationType()`.
    fn foundation_type(&self, occ: u64) -> u32;
    /// BDS slot 23 `AddNewFoundationOccupant(type, kind, pos, dx, dz,
    /// orient, bottom)` (§17.5.3): AL; it may replace `pos[1]` (a negative
    /// top becomes the highest vertex under the foundation).
    #[allow(clippy::too_many_arguments)]
    fn add_new_foundation_occupant(
        &self,
        foundation_type: u32,
        kind: i32,
        pos: &mut Vec3,
        dx: f32,
        dz: f32,
        orient: i32,
        bottom: f32,
    ) -> bool;

    // ---- prop manager ----
    /// Slot 15 `CreateProp(type)`.
    fn prop_create(&self, prop_type: u32) -> Option<u64>;
    /// Slot 24 `GetLotProps(list, lot)`.
    fn lot_props(&self, lot: u64) -> Vec<u64>;
    /// Slot 25 `AddLotProp(lot, occ)`.
    fn prop_add_to_lot(&self, lot: u64, occ: u64) -> bool;
    /// Slot 26 `AddLotProp(type, pos, orientation)` → occupant.
    fn prop_add(&self, prop_type: u32, pos: Vec3, orientation: i32) -> Option<u64>;
    /// Slot 28 `RemoveLotProps(lot)`.
    fn remove_lot_props(&self, lot: u64);
    /// Slot 29: remove one prop.
    fn prop_remove(&self, occ: u64) -> bool;
    /// Slot 38 `RemoveBuildingProps(building)`.
    fn remove_building_props(&self, building: u64);
    /// §7.2 step 2: a `cSC4PropOccupantBurntBuilding` made from `building`.
    fn make_burnt_building(&self, building: u64) -> Option<BurntBuilding>;
    /// §7.2 step 4: the terrain-decal prop `{type, group, instance}` of
    /// size `size`, alpha 0.7, at the building position, added to the lot.
    fn add_rubble_decal(&self, lot: u64, burnt: &BurntBuilding, instance: u32, size: f32);

    // ---- BDS (building-development.md) ----
    /// Slot 39 `GetBuildingProfile` (inserts a zero profile when unknown).
    fn building_profile(&self, building: u32) -> BuildingProfile;
    /// Slot 32 `GetDeveloperType(purpose, wealth)`.
    fn developer_type(&self, purpose: u32, wealth: u32) -> u32;
    /// Slot 30 `GetDeveloper(devType)`.
    fn developer_of_type(&self, dev_type: u32) -> Option<u64>;
    /// Slot 31 `GetWealth(devType)`.
    fn wealth_of_type(&self, dev_type: u32) -> u32;
    /// Slot 33 `GetDeveloper(purpose, wealth)`.
    fn developer_for(&self, purpose: u32, wealth: u32) -> Option<u64>;
    /// Slot 15: whether `id` is a valid building type.
    fn is_building_type(&self, id: u32) -> bool;
    /// Slot 19 `GetBuildingTypesByFamily`: the types in the order the BDS
    /// inserts them into the caller's `hash_set` (the core models the set).
    fn building_types_by_family(&self, family: u32) -> Vec<u32>;
    /// Slot 7 `AddNewBuildingOccupant(type, pos, orientation)`.
    fn add_new_building_occupant(&self, building: u32, pos: Vec3, orientation: i32) -> Option<u64>;
    /// Slot 50 `ClaimConstructionResources`.
    fn claim_construction_resources(&self, amount: u16) -> bool;
    /// Slots 54, 55, 58.
    fn cheat(&self, cheat: Cheat) -> bool;

    // ---- growth developers (growth-developer.md) ----
    /// Slot 7 `GetPurpose`.
    fn dev_purpose(&self, dev: u64) -> u32;
    /// Slot 13 `GetDesirabilityMap()` → grid slot 7 `GetCellValue(x, z)`.
    fn dev_desirability(&self, dev: u64, x: i32, z: i32) -> i16;
    /// Slot 20 `GetWaterSupplyCap`.
    fn dev_water_supply_cap(&self, dev: u64) -> u8;

    // ---- power, plumbing, traffic, demand ----
    /// Power slot 37 `IsPowered`.
    fn is_powered(&self, x: i32, z: i32) -> bool;
    /// Plumbing slot 11 `IsCellWatered`.
    fn is_cell_watered(&self, x: i32, z: i32) -> bool;
    /// Traffic slot 13 `GetTripLengthMap()` → `GetCellValue(x, z)`.
    fn trip_length(&self, x: i32, z: i32) -> u8;
    /// Demand simulator slot 6 `GetDemand(0x2000, 0x20000)` then demand
    /// slot 9 `QueryDemandValue`; `None` without the node.
    fn jobs_demand(&self) -> Option<f32>;

    // ---- flora ----
    /// Slot 11 `IsFloraTypeValid`.
    fn flora_type_valid(&self, id: u32) -> bool;
    /// Slot 14 `GetFloraTypesByFamily(set, id)`: `None` when it returns 0;
    /// otherwise the types in insertion order (the core models the set).
    fn flora_types_by_family(&self, id: u32) -> Option<Vec<u32>>;
    /// Slot 17 `AddNewFloraOccupant(type, pos, false)`.
    fn flora_add(&self, flora_type: u32, pos: Vec3) -> Option<u64>;
    /// Slot 20 `DemolishFloraInCellArea(rect, 0)`.
    fn flora_demolish_in_cell_area(&self, rect: CellRect);

    // ---- demolition ----
    /// Slot 8 `DemolishOccupant(1, occ, 0, 1, 0, 0, 0)`.
    fn demolish_occupant(&self, occ: u64) -> bool;
    /// Slot 7 `DemolishLots(1, [lot], 0, 0x800, 0, 0, 0, 0)`.
    fn demolish_lot(&self, lot: u64) -> bool;
    /// Slot 9 `ModifyTerrainHeight(1, perVertex, x0, z0, x1, z1, h, exempt,
    /// 5, null)` (§17.3): EAX, the number of vertices counted. The only
    /// terrain writer the lot developer reaches.
    #[allow(clippy::too_many_arguments)]
    fn modify_terrain_height(
        &self,
        per_vertex: bool,
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
        h: f32,
        exempt: Option<u64>,
    ) -> i32;

    // ---- business units (cISC4BusinessUnit) ----
    /// Slot 5 `GetMembers`, in member order.
    fn unit_members(&self, unit: u64) -> Vec<u64>;
    /// A new `cSC4BusinessUnit` (no members).
    fn unit_create(&self) -> u64;
    /// Slots 3 / 4 `Add` / `Remove` (they set or clear the lot's unit).
    fn unit_add(&self, unit: u64, lot: u64) -> bool;
    fn unit_remove(&self, unit: u64, lot: u64) -> bool;

    // ---- persistence (§13) ----
    /// The save reference of a night-deque building.
    fn building_ref(&self, building: u64) -> RefValue;
    /// The building of a reference read with [`IID_BUILDING_OCCUPANT`].
    fn resolve_building(&self, r: &RefValue) -> Option<u64>;
}
