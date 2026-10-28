// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The flora simulator (`docs/specs/flora.md`, `cISC4FloraSimulator`, class
//! `0xC990C034`). Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`FloraSim`]: the state of §3.1, the lifecycle of §4, the messages and
//!   the cheat of §5, the algorithms of §6, the tuning of §7, the posted
//!   messages of §9, the save record and the obliteration records of §10
//!   and the portable part of the interface slots of §11.1.
//! - [`FloraRegistry`] / [`FloraRecord`]: the process-wide flora type
//!   registry of §3.2 and the static flora records of §3.3 (built from
//!   flora exemplars), with the cluster chains and seed types of §6.11. The
//!   registry is shared ([`SharedRegistry`]): the host keeps one per process
//!   and hands it to every simulator it creates.
//! - [`FloraHost`]: everything the simulator reads from, or does to, the
//!   game: the city, the simulator clock, the framework (agents,
//!   notifications, the command and cheat servers), the terrain, the lot
//!   manager, the weather's moisture, the occupant manager and the flora
//!   occupants, object references in saves, and the obliteration segment.
//! - [`FloraRandom`]: the simulator's own saved `cRZRandom` (§8), behind a
//!   trait so tests can script it; [`FloraRng`] is the real generator with
//!   the 24-bit double path of foundations §6.1. The block iterator's
//!   `random_shuffle` draws from the simulator's own [`CrtRand`]
//!   (foundations §12.6).
//! - Pure helpers: [`elevation_index`], [`score_from_table`],
//!   [`table_byte`], [`blocks_per_month`], [`scale_wild_seeds`],
//!   [`blocks_flora`], [`boxes_intersect`], and the occupant manager's
//!   64 m index as flora's queries see it (§6.4.2, §6.10.4; for hosts that
//!   model the occupant manager and for the census shadow):
//!   [`manager_cell`], [`manager_listing`], [`manager_query_cells`],
//!   [`manager_query_returns`], [`cell_query_box`].
//!
//! The simulator keeps **no grid** (§2): flora lives in the occupant
//! manager (occupant type `0x74758926`, the type the flammability simulator
//! reads, `crate::flammability::occupant_type::FLORA`).
//!
//! Host handles ([`OccupantId`]) are opaque tokens (a `cISC4Occupant*` in
//! the plugin); 0 is never used for a live occupant. Host callbacks must not
//! call back into the simulator.
//!
//! # Arithmetic
//!
//! Foundations §18 (24-bit x87): every float add, subtract, multiply and
//! divide is one `f32` rounding; integer operands (an age, a day number, a
//! table byte, a seed count) enter their operation unrounded (computed in
//! `f64`, rounded once to `f32`). `trunc` is `_ftol` (truncation toward zero
//! to 64 bits, of which the low bits are kept; NaN and out-of-range give
//! `0x8000000000000000`). `floor` (§6.1, §6.8) and `roundHalfEven` (§6.7.2)
//! are exact on the rounded value.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - The key-0 default record (§3.2) gets the flags byte [`DEFAULT_RECORD_FLAGS`]
//!   = `0x01` (the original leaves it uninitialised): bit 0, so it is never
//!   picked automatically (the quirk of §14.1 then never fires).
//!   [`FloraRegistry::set_default_record_flags`] changes it.
//! - The registry keeps records as `Rc<FloraRecord>`; the seed type is a
//!   `Cell` because registering a type rewrites other records' seed types
//!   (§6.11). A reload (§5 exemplar changed) drops the registry's records;
//!   occupants that hold one keep it alive (the original leaks them; the
//!   plugin must keep game-side records alive, §13.2).
//! - Exemplar parsing (§3.3): fewer than 14 values in `0x083DC35B` leave the
//!   remaining fields at their code defaults and extra values are ignored
//!   (the original reads past the property). A propagation table property
//!   with fewer than 256 values keeps the default bytes after them.
//! - `AdvanceClusterType` (§6.11): a walk stops at type 0 (the key-0 record's
//!   own next type is 0, which would otherwise loop) and after as many steps
//!   as there are registered types plus one (a cycle, §14 quirk, ends there).
//! - `GetBestFloraTypeForLocation` (§6.3) asks the weather for the moisture
//!   type once and the terrain for each elevation index at most once per
//!   water flag.
//! - A moisture type ≥ 16 (the original indexes past the row) reads table
//!   byte 0, so the score is 0.
//! - The blocking filter (§6.4.1) is the pure [`blocks_flora`]; the host
//!   applies it when [`FloraHost::occupants_in_bbox`] is called with
//!   [`OccupantFilter::Blocking`]. `BoundingBoxesIntersect` (null outputs) is
//!   [`boxes_intersect`]: strict on every axis, touching faces do not
//!   intersect, an unordered (NaN) compare overlaps (§6.4 [compat]).
//! - Planting (§6.5): the cluster-merging loop runs at most
//!   [`MAX_MERGE_ROUNDS`] rounds. A host that cannot create a flora occupant
//!   makes planting return null after the location test (no draw).
//! - Monthly (§6.7.1): a second iterator wrap without a visit ends the
//!   month (an empty iterator would loop forever); at most
//!   [`MAX_BLOCKS_PER_MONTH`] blocks per month. The table-mode reset refills
//!   the table in natural order before every shuffle (the dirty flag is never
//!   cleared, weather §6.7), done by emptying the shared iterator's table
//!   before its reset. Blocks beyond 256 per axis are not visited (the shared
//!   iterator's table covers 256 × 256).
//! - Seeding (§6.7.2): the seed count is `roundHalfEven(seedsPerCycle)`; NaN
//!   or negative counts give no seed and counts above
//!   [`MAX_SEEDS_PER_CYCLE`] are capped.
//! - Wild seeding (§6.7.3) runs only when the type array holds at least one
//!   type with a record and flag bit 0 clear (the original hangs or reads out
//!   of bounds otherwise); a type without a record is skipped like a bit-0
//!   type; at most [`MAX_WILD_SEEDS`] seeds per month.
//! - `period = 0` gives `blocksPerMonth = 0` (the original faults).
//! - The terrain test of §6.10.2 compares `|h − y|` with the `double` 0.001.
//!   A non-flora occupant in the listing is skipped; one whose type has no
//!   record counts as a land type.
//! - The cheat line is split at ASCII whitespace; `blast` parses each number
//!   as its leading decimal digits (none → 0, saturating at `u32::MAX`), a
//!   missing word reads 0; the blast runs at most [`MAX_BLAST_CENTRES`]
//!   centres and [`MAX_BLAST_SEEDS`] seeds in total.
//! - PlaceFlora with fewer than three parameters is ignored.
//! - The exemplar-changed handler re-reads the tuning when `inst` is the
//!   tuning instance whether or not `inst` was a flora type.
//! - Without a city (`Init` step 6), sizes, counts and `blocksPerMonth`
//!   keep their values; the city reads "not established" (step 10).
//! - `Shutdown` releases the burnt list's references one by one
//!   (`ReleaseOccupantList`) and empties it. `Read` releases the references
//!   of the list it clears; null or unresolvable references are not kept; a
//!   failed iterator or generator read keeps the previous one.
//! - Obliteration (§10.3): at most [`MAX_OBLITERATED`] occupant records are
//!   opened (the original has no bound; AQ8). *[compat]* An occupant whose
//!   record does not open keeps flag 1 (it is cleared and set again only
//!   around its `Write`, AQ4); a restored occupant's record is closed after
//!   its `Read`, `Init`, `InsertOccupant` and `Release` (AQ5); a count record
//!   whose `GetUint32` fails leaves the count at 0 (AQ6).
//! - Reference count: `Release` from a count ≤ 1 calls `Shutdown`, returns 0
//!   and marks the object destroyed; the plugin then drops it (§1).
//! - Interface returns: "AL only" slots return a clean bool (§11.1's own
//!   [choice]).

mod registry;
mod simulator;
#[cfg(test)]
mod tests;

pub use registry::{
    advance_cluster_type, FloraRecord, FloraRegistry, SharedRegistry, DEFAULT_RECORD_FLAGS, DEFAULT_TABLE_BYTE,
};
pub use simulator::{predict_visit, FloraData, FloraSim, VisitOutcome, FLORA_RECORD_VERSION};
pub mod occupant;
pub mod archive;

use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::rng::{RzRandom, SeedSource, CLOCK_SEED};
use crate::stream::{RefValue, ResKey};

pub use crate::grid::CellIterator;
pub use crate::rng::CrtRand;
pub use crate::weather::RngState;

/// Class and interface IDs (§1, §3.4, §6.4.1, §10.1).
pub const CLSID_FLORA_SIMULATOR: u32 = 0xC990_C034;
pub const IID_FLORA_SIMULATOR: u32 = 0x06C6_6177;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const IID_UNKNOWN: u32 = 1;
/// `cISC4Occupant`: the IID burnt-list references are resolved with.
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
/// `cISC4FloraOccupant` and its class.
pub const IID_FLORA_OCCUPANT: u32 = 0xC83D_D057;
pub const CLSID_FLORA_OCCUPANT: u32 = 0xA9C0_5C85;
/// `cISC4NetworkOccupant`.
pub const IID_NETWORK_OCCUPANT: u32 = 0xA821_EF94;

/// Which sub-object `QueryInterface` returns (§1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloraInterface {
    /// `cISC4FloraSimulator` and `cIGZUnknown`: the primary pointer.
    Primary,
    MessageTarget,
    Serializable,
}

/// `QueryInterface` (§1): the sub-object for `iid`, or `None` (false, the
/// out pointer untouched).
pub fn query_interface(iid: u32) -> Option<FloraInterface> {
    match iid {
        IID_FLORA_SIMULATOR | IID_UNKNOWN => Some(FloraInterface::Primary),
        IID_MESSAGE_TARGET2 => Some(FloraInterface::MessageTarget),
        IID_SERIALIZABLE => Some(FloraInterface::Serializable),
        _ => None,
    }
}

/// Agent registration (§4.1 step 4, §5): one target at the day and month
/// levels, in that order, flags 3.
pub const AGENT_NAME: &str = "cSC4FloraSimulator";
pub const AGENT_LEVELS: [u32; 2] = [4, 6];
pub const AGENT_FLAGS: u32 = 3;

/// The PlaceFlora command (§4.1 step 3, §5).
pub const PLACE_FLORA_COMMAND: u32 = 0xAB99_A64B;
pub const PLACE_FLORA_NAME: &str = "PlaceFlora";
/// The `Flora` cheat (§4.1 step 11, §5.2).
pub const FLORA_CHEAT: u32 = 0x095B_DC9C;
pub const FLORA_CHEAT_NAME: &str = "Flora";

/// The tuning exemplar (§7).
pub const TUNING_KEY: ResKey = ResKey {
    type_id: 0x6534_284A,
    group: 0x7A4A_8458,
    instance: TUNING_INSTANCE,
};
/// The tuning exemplar's instance: the exemplar-changed message re-reads
/// the tuning for it (§5).
pub const TUNING_INSTANCE: u32 = 0x1A2F_DB6A;
/// The flora exemplar resource type and its ExemplarType value (§3.3).
pub const EXEMPLAR_TYPE: u32 = 0x6534_284A;
pub const FLORA_EXEMPLAR_TYPE: u32 = 0x0F;

/// Messages (§4, §5, §9).
pub mod msg {
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const INSERT_OCCUPANT: u32 = 0x99EF_1142;
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    /// Posted through the occupant's `PostOccupantMessage` (§9).
    pub const CHANGE_OCCUPANT: u32 = 0x99EF_1145;
    pub const TERRAIN_HEIGHTS_CHANGED: u32 = 0x278E_6003;
    /// Foundations §5.2.
    pub const EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;
    pub const CITY_ESTABLISHED: u32 = 0x26D3_1EC4;
    pub const CITY_OBLITERATED: u32 = 0x26D3_1EC5;
    pub const PLACE_FLORA: u32 = super::PLACE_FLORA_COMMAND;
    /// The cheat notification (data1 = the cheat ID).
    pub const CHEAT: u32 = 0x230E_27AC;
    /// Posted after every (re)load of the flora types (§9): data2 = the
    /// number of map entries, void1 = the simulator.
    pub const FLORA_TYPES_UPDATED: u32 = 0x87A6_277F;
    /// The subscriptions of `Init` step 3, in order.
    pub const SUBSCRIPTIONS: [u32; 7] = [
        TERRAIN_HEIGHTS_CHANGED,
        INSERT_OCCUPANT,
        REMOVE_OCCUPANT,
        EXEMPLAR_CHANGED,
        CITY_ESTABLISHED,
        CITY_OBLITERATED,
        PLACE_FLORA,
    ];
    /// The unsubscriptions of `Shutdown` (§4.2), in order.
    pub const UNSUBSCRIPTIONS: [u32; 7] = [
        CITY_OBLITERATED,
        CITY_ESTABLISHED,
        EXEMPLAR_CHANGED,
        TERRAIN_HEIGHTS_CHANGED,
        INSERT_OCCUPANT,
        REMOVE_OCCUPANT,
        PLACE_FLORA,
    ];
}

/// Occupant types (§3.4, §6.4.1).
pub mod occupant_type {
    /// Flora (the same ID the flammability simulator reads).
    pub const FLORA: u32 = 0x7475_8926;
    pub const BUILDING: u32 = 0x2781_28A0;
    pub const LANDFILL: u32 = 0x6A11_BD2B;
    pub const PROP: u32 = 0xA823_821E;
    pub const NETWORK: u32 = 0xC772_BF98;
}

/// Network flags that block flora (§6.4.1).
pub const BLOCKING_NETWORK_FLAGS: u32 = 0x1F4F;

/// Properties (§3.3, §5, §7).
pub mod prop {
    /// The preview-occupant marker (foundations §5.2): tested for presence.
    pub const PREVIEW_OCCUPANT: u32 = 0x0292_44DB;
    /// ExemplarType.
    pub const EXEMPLAR_TYPE: u32 = 0x10;
    pub const TYPE_ID: u32 = 0x21;
    pub const NEXT_CLUSTER_TYPE: u32 = 0x2A03_48BA;
    pub const CLUSTER_RADIUS: u32 = 0x2A03_48BB;
    pub const CLUSTER_HEIGHT: u32 = 0x2A03_48BC;
    pub const WATER: u32 = 0xEA34_6888;
    /// 14 floats (§3.3 fields 1–14).
    pub const FLOATS: u32 = 0x083D_C35B;
    /// 256 floats: the propagation table.
    pub const TABLE: u32 = 0x083D_C487;
    pub const UNNAMED: u32 = 0x083D_E16F;
    /// Sets flag bit 0.
    pub const NEVER_AUTO: u32 = 0xE9D1_17C3;
    /// true clears flag bit 2, false clears bit 1.
    pub const FLAG_SWITCH: u32 = 0x6A37_EBB6;
    pub const FAMILIES: u32 = 0xA8F1_49C5;
    pub const PLOP_COST: u32 = 0x49CA_C341;
    // ---- tuning (§7) ----
    pub const BURN_RATE: u32 = 0xC9DB_DD00;
    pub const BURNT_REMOVAL_STEP: u32 = 0xC9DB_DD01;
    pub const ELEVATION_SCALE: u32 = 0xC9DB_DD02;
    pub const PERIOD: u32 = 0x8761_ABD0;
    pub const WILD_SEEDS: u32 = 0x0854_89C7;
}

/// The Burnt Tree type (§6.9).
pub const BURNT_TREE: u32 = 0x29CF_F3CB;

/// Flora occupant states (§3.4).
pub mod state {
    pub const YOUNG: i8 = 0;
    pub const MATURING: i8 = 1;
    pub const MATURE: i8 = 2;
    pub const BURNT: i8 = 3;
    pub const DYING: i8 = 4;
}

/// Record flag bits (§3.3).
pub mod flag {
    /// Never chosen automatically (best type, wild seeds).
    pub const NEVER_AUTO: u8 = 1;
    /// Skips the score test while propagation is off.
    pub const SKIP_SCORE: u8 = 2;
    pub const BIT2: u8 = 4;
}

/// The obliteration records (§10.3).
pub const OBLITERATED_COUNT_KEY: ResKey = ResKey {
    type_id: 0xC9BB_3FC6,
    group: 0xC9BB_3FC7,
    instance: 1,
};
pub const OBLITERATED_TYPE: u32 = 0xC9BB_3FC6;
pub const OBLITERATED_OCCUPANT_GROUP: u32 = 0x4A66_4F83;

/// The key of obliterated occupant `i`.
pub fn obliterated_occupant_key(i: u32) -> ResKey {
    ResKey {
        type_id: OBLITERATED_TYPE,
        group: OBLITERATED_OCCUPANT_GROUP,
        instance: i,
    }
}

/// The burnt-list count limit of `Read` (§10.1).
pub const MAX_BURNT_COUNT: u32 = 10_000_000;

/// [choice] Loop bounds (see the module documentation).
pub const MAX_MERGE_ROUNDS: u32 = 1024;
pub const MAX_BLOCKS_PER_MONTH: u32 = 1 << 24;
pub const MAX_SEEDS_PER_CYCLE: i64 = 65_536;
pub const MAX_WILD_SEEDS: u32 = 65_536;
pub const MAX_BLAST_CENTRES: u32 = 1 << 20;
pub const MAX_BLAST_SEEDS: u64 = 1 << 20;
pub const MAX_OBLITERATED: u32 = 1 << 24;

/// An occupant handle (a `cISC4Occupant*` in the plugin).
pub type OccupantId = u64;
/// An obliteration segment handle (a `cIGZPersistDBSegment*`).
pub type SegmentId = u64;
/// An open record stream of the obliteration segment (host handle, §10.3).
pub type StreamId = u64;
/// An inclusive cell rectangle `(x0, z0, x1, z1)` (`SC4Rect<long>`).
pub type CellRect = (i32, i32, i32, i32);
/// A box corner (x, y, z), world metres.
pub type Vec3 = [f32; 3];

/// The occupant filter of a bounding-box query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccupantFilter {
    /// Occupants of one type (`GetOccupantsByBBox(…, type, −1)`, slot 37).
    Type(u32),
    /// The flora blocking filter (§6.4.1, slot 36); see [`blocks_flora`].
    Blocking,
}

/// The `cISC4FloraOccupant` values the simulator reads (§3.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloraOccupant {
    pub flora_type: u32,
    pub birth_date: u32,
    pub last_seeding_date: u32,
    /// Signed byte (`GetState` sign-extends).
    pub state: i8,
    pub orientation: u8,
}

/// One command parameter (`cIGZVariant`) as the PlaceFlora handler reads
/// it (§5): its value as `u32` and as `f32`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CommandParam {
    pub as_u32: u32,
    pub as_f32: f32,
}

/// A message as `DoMessage` sees it (§5).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1()` as an occupant (Insert / RemoveOccupant).
    pub occupant: Option<OccupantId>,
    /// The cheat's command line (cheat messages).
    pub text: Option<String>,
    /// The command parameter set (PlaceFlora).
    pub params: Vec<CommandParam>,
}

impl Message {
    /// A message with only a type.
    pub fn of_type(msg_type: u32) -> Message {
        Message {
            msg_type,
            ..Message::default()
        }
    }
}

/// Everything the flora simulator reads from, or does to, the rest of the
/// game (§4, §12). Positions are world metres.
pub trait FloraHost {
    // ---- city, clock, resources ----
    /// Whether the city exists (`Init` step 6).
    fn city_exists(&self) -> bool;
    /// `cISC4City::SizeX()` / `SizeZ()` (slots 89 / 90).
    fn city_size(&self) -> (f32, f32);
    /// The city's cell counts (slots 93 / 94).
    fn city_cell_counts(&self) -> (i32, i32);
    /// `cISC4City::GetEstablished()` (slot 22); false without a city.
    fn city_established(&self) -> bool;
    /// `cISC4Simulator::GetSimDateNumber()` (slot 8).
    fn sim_date_number(&self) -> u32;
    /// The "Flora Tuning Parameters" exemplar ([`TUNING_KEY`]), if it loads.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;
    /// The available resources of type `0x6534284A`, in the resource
    /// manager's enumeration order, each with its exemplar (§4.1 step 2;
    /// the simulator keeps those with ExemplarType `0x0F`).
    fn flora_exemplars(&self) -> Vec<(ResKey, Rc<PropertyHolder>)>;
    /// `GetNewCitySpecification()->obliteratedCityDBSegment` (§4.1 step 12).
    fn obliterated_segment(&self) -> Option<SegmentId>;

    // ---- framework ----
    /// `cISC4App::RegisterShutdownCallbackFunction(cb, 0)` (first `Init` of
    /// the process; the callback is [`FloraRegistry::app_shutdown`]).
    fn register_app_shutdown_callback(&self);
    /// `AddNotification` on the message server, in order.
    fn subscribe(&self, msg_types: &[u32]);
    /// `RemoveNotification`, in order.
    fn unsubscribe(&self, msg_types: &[u32]);
    /// `cIGZCommandServer::RegisterCommand(id, name, null)`.
    fn register_command(&self, id: u32, name: &str);
    /// `UnregisterCommand(id)`.
    fn unregister_command(&self, id: u32);
    /// `cISC4Simulator::AddAgent(target, level, name, flags)`.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(target)`, then the simulator's release.
    fn remove_agent(&self);
    /// `Init` step 5: hold the occupant manager, the weather (may be null),
    /// the terrain and the lot manager; create the blocking filter.
    fn acquire_services(&self);
    /// `Shutdown`: release the filter, lot manager, terrain, occupant
    /// manager and weather.
    fn release_services(&self);
    /// `RegisterCheatCode(id, name)` and `AddNotification2(target, id)`.
    fn register_cheat(&self, id: u32, name: &str);
    /// `RemoveNotification2(target, id)` and `UnregisterCheatCode(id)`.
    fn unregister_cheat(&self, id: u32);
    /// `MessagePost(msg, false)` of a standard message with void1 =
    /// IGZUnknown = the simulator (§9).
    fn post(&self, msg_type: u32, data2: u32);

    // ---- terrain, lots, weather ----
    /// `cISTETerrain` slot 5 (initialized).
    fn terrain_initialized(&self) -> bool;
    /// `GetAltitude(x, z)`.
    fn altitude(&self, x: f32, z: f32) -> f32;
    /// `GetSeaLevel()`.
    fn sea_level(&self) -> f32;
    /// `GetFaceNormal(x, z, &n)`.
    fn face_normal(&self, x: f32, z: f32) -> Vec3;
    /// `cISC4LotManager::GetLotSavvyTerrainAltitude(x, z, &onLot)`.
    fn lot_savvy_altitude(&self, x: f32, z: f32) -> (f32, bool);
    /// `cISC4WeatherSimulator::GetMoistureType(x, z)` (all of EAX, §6.2, FQ5).
    fn moisture_type(&self, x: f32, z: f32) -> u32;

    // ---- occupants ----
    /// `cISC4Occupant::GetType()`.
    fn occupant_type(&self, occ: OccupantId) -> u32;
    /// `AsPropertyHolder()->HasProperty(id)`.
    fn occupant_has_property(&self, occ: OccupantId, id: u32) -> bool;
    /// `GetPosition()`.
    fn occupant_position(&self, occ: OccupantId) -> Vec3;
    /// `SetPosition(pos)`.
    fn set_occupant_position(&self, occ: OccupantId, pos: Vec3);
    /// `GetBoundingBox(&min, &max)`.
    fn occupant_bounding_box(&self, occ: OccupantId) -> (Vec3, Vec3);
    /// `IsFlagSet(1)` (inserted in the occupant manager).
    fn occupant_in_manager(&self, occ: OccupantId) -> bool;
    /// `SetFlag(1)` / `ClearFlag(1)`.
    fn set_occupant_in_manager(&self, occ: OccupantId, on: bool);
    /// QI `cISC4NetworkOccupant` and `GetNetworkFlag()`; `None` when the QI
    /// fails.
    fn network_flags(&self, occ: OccupantId) -> Option<u32>;
    /// QI `cISC4FloraOccupant` and its values; `None` when the QI fails.
    fn flora_occupant(&self, occ: OccupantId) -> Option<FloraOccupant>;
    fn set_flora_type(&self, occ: OccupantId, flora_type: u32);
    fn set_birth_date(&self, occ: OccupantId, day: u32);
    fn set_last_seeding_date(&self, occ: OccupantId, day: u32);
    fn set_flora_state(&self, occ: OccupantId, state: i8);
    fn set_flora_orientation(&self, occ: OccupantId, orientation: u32);
    /// `SetStaticFloraData(record)` (the occupant holds it).
    fn set_flora_record(&self, occ: OccupantId, record: Option<Rc<FloraRecord>>);
    /// `PostOccupantMessage(0x99EF1145, 0)`.
    fn post_occupant_change(&self, occ: OccupantId);
    /// A new `cSC4FloraOccupant` with one reference for the caller.
    fn create_flora_occupant(&self) -> Option<OccupantId>;
    /// `cISC4Occupant::Init()`.
    fn init_occupant(&self, occ: OccupantId);
    fn add_ref(&self, occ: OccupantId);
    fn release(&self, occ: OccupantId);
    /// How a save refers to an occupant.
    fn object_ref(&self, occ: OccupantId) -> RefValue;
    /// The occupant of a reference read from a save, if it answers `iid`.
    /// The reference the stream handed over passes to the simulator; on
    /// `None` the host drops it.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<OccupantId>;

    // ---- occupant manager ----
    /// Whether the simulator holds an occupant manager.
    fn has_occupant_manager(&self) -> bool;
    /// `InsertOccupant(occ, 0)`.
    fn insert_occupant(&self, occ: OccupantId) -> bool;
    /// `RemoveOccupant(occ, true, 0)`.
    fn remove_occupant(&self, occ: OccupantId) -> bool;
    /// `MoveOccupant(occ, true)`.
    fn move_occupant(&self, occ: OccupantId);
    /// `GetOccupantsByBBox(list, min, max, filter, −1)` (slots 36 / 37).
    fn occupants_in_bbox(&self, min: Vec3, max: Vec3, filter: OccupantFilter) -> Vec<OccupantId>;
    /// `GetOccupantsByStandardCityCells(list, &min, &max, type, −1)`;
    /// `None` = null corners = the whole city.
    fn occupants_in_cells(&self, rect: Option<CellRect>, occ_type: u32) -> Vec<OccupantId>;
    /// `GetOccupantsByOccupantManagerCells(list, null, null, type, −1)`.
    fn occupants_in_manager_cells(&self, occ_type: u32) -> Vec<OccupantId>;

    // ---- obliteration segment (§10.3) ----
    /// Opens record `key` of the segment: `QueryInterface` for
    /// `cISC4DBSegment` (`0x499899DA`), then `OpenOStream(key, &s, true)`
    /// (slot 8) when `write`, else `OpenIStream(key, &s)` (slot 5). `None`:
    /// not open (the segment does not answer, or the key does not open). The
    /// simulator puts or gets the record's `u16` version itself.
    fn segment_open(&self, seg: SegmentId, key: ResKey, write: bool) -> Option<StreamId>;
    /// `CloseOStream` / `CloseIStream` (slot 9 / 6), then both references
    /// (the stream's and the segment interface's) released.
    fn segment_close(&self, s: StreamId);
    /// `PutUint16` (stream slot 7).
    fn stream_put_u16(&self, s: StreamId, v: u16);
    /// `PutUint32` (stream slot 9).
    fn stream_put_u32(&self, s: StreamId, v: u32);
    /// `GetUint16` (stream slot 7); `None` when the call fails.
    fn stream_get_u16(&self, s: StreamId) -> Option<u16>;
    /// `GetUint32` (stream slot 9); `None` when the call fails.
    fn stream_get_u32(&self, s: StreamId) -> Option<u32>;
    /// `GetError()` (stream slot 18) is non-zero (sticky).
    fn stream_error(&self, s: StreamId) -> bool;
    /// The occupant's own `cIGZSerializable::Write` on the stream.
    fn write_occupant(&self, occ: OccupantId, s: StreamId) -> bool;
    /// The occupant's own `cIGZSerializable::Read` from the stream.
    fn read_occupant(&self, occ: OccupantId, s: StreamId) -> bool;
}

/// The draws the flora simulator makes from its `cRZRandom` (§8).
pub trait FloraRandom {
    /// `RandomSint32RangeUniform(lo, hi)`.
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32;
    /// `RandomUint32Uniform(n)`.
    fn uniform_u32(&mut self, n: u32) -> u32;
    /// `RandomDoubleUniform()`.
    fn uniform_f64(&mut self) -> f64;
    /// `RandomDoubleRangeUniform(a, b)`.
    fn range_f64(&mut self, a: f64, b: f64) -> f64;
    /// `RandomDoubleGaussian()`.
    fn gaussian(&mut self) -> f64;
    /// The saved state (§10.1 field 16).
    fn state(&self) -> RngState;
    fn set_state(&mut self, s: RngState);
}

/// The real generator: a `cRZRandom` (its double methods are 24-bit,
/// foundations §6.1; at most [`crate::rng::MAX_GAUSSIAN_TRIES`] polar
/// pairs, then 0 [choice]), plus the saved Gaussian cache word.
pub struct FloraRng {
    rz: RzRandom,
    /// The cached word as the original keeps it (not cleared on use).
    last_cached: f64,
}

impl FloraRng {
    /// Seeded from the wall clock (`Seed(0xFFFFFFFF)`, §3.1).
    pub fn new(seeds: &dyn SeedSource) -> FloraRng {
        FloraRng::with_seed(CLOCK_SEED, seeds)
    }

    pub fn with_seed(seed: u32, seeds: &dyn SeedSource) -> FloraRng {
        FloraRng {
            rz: RzRandom::new(seed, seeds),
            last_cached: 0.0,
        }
    }

    /// Seeded from the session seed (foundations §6.3 *[choice]*: a member
    /// seeded from the clock takes `derive(GZCLSID)`, `0xC990C034`).
    pub fn from_session(session: u32) -> FloraRng {
        let key = crate::seeding::keys::FLORA;
        FloraRng::with_seed(
            crate::seeding::seed(session, key),
            &crate::seeding::clocks(session, key),
        )
    }
}

impl FloraRandom for FloraRng {
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        self.rz.range_i32(lo, hi)
    }

    fn uniform_u32(&mut self, n: u32) -> u32 {
        self.rz.uniform_u32(n)
    }

    fn uniform_f64(&mut self) -> f64 {
        self.rz.uniform_f64()
    }

    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        self.rz.range_f64(a, b)
    }

    fn gaussian(&mut self) -> f64 {
        let g = self.rz.gaussian();
        if let Some(c) = self.rz.cached_gaussian() {
            self.last_cached = c;
        }
        g
    }

    fn state(&self) -> RngState {
        let cached = self.rz.cached_gaussian();
        RngState {
            lcg: self.rz.lcg_state(),
            mul: self.rz.mul_state(),
            has_cached_gaussian: i32::from(cached.is_some()),
            cached_gaussian: cached.unwrap_or(self.last_cached),
        }
    }

    fn set_state(&mut self, s: RngState) {
        let cached = (s.has_cached_gaussian != 0).then_some(s.cached_gaussian);
        self.rz.set_state(s.lcg, s.mul, cached);
        self.last_cached = s.cached_gaussian;
    }
}

/// One 24-bit rounding (foundations §18), as an `f32`.
fn r24(x: f64) -> f32 {
    x as f32
}

// ---- pure helpers -----------------------------------------------------------------------------------

/// `_ftol` to 64 bits (foundations §18.1): truncation toward zero; NaN and
/// magnitudes ≥ 2^63 give `0x8000000000000000`.
pub fn ftol(v: f64) -> i64 {
    const TWO_63: f64 = 9_223_372_036_854_775_808.0;
    if v.is_nan() || !(-TWO_63..TWO_63).contains(&v) {
        i64::MIN
    } else {
        v as i64
    }
}

/// The elevation index of §6.1: `clamp(floor(d × scale), 0, 15)` with `d =
/// altitude − sea` (water types: `sea − altitude`), each step one `f32`
/// operation. NaN gives 0 (the floor trick's integer indefinite is
/// negative).
pub fn elevation_index(altitude: f32, sea: f32, water: bool, scale: f32) -> usize {
    let d = if water { sea - altitude } else { altitude - sea };
    let f = (d * scale).floor();
    if f >= 15.0 {
        15
    } else if f >= 0.0 {
        f as usize
    } else {
        0
    }
}

/// `table[e][m] × (1/255 as f32)` (§6.2), the byte entering unrounded.
pub fn score_from_table(table: &[u8; 256], e: usize, m: u32) -> f32 {
    let b = if e < 16 && m < 16 {
        table[e * 16 + m as usize]
    } else {
        0
    };
    r24(f64::from(b) * f64::from(1.0f32 / 255.0))
}

/// One propagation-table byte from its exemplar float (§3.3): the low 8 bits
/// of `trunc(v × 255)`.
pub fn table_byte(v: f32) -> u8 {
    ftol(f64::from(v * 255.0)) as u8
}

/// `((cellsX × cellsZ) div period) >> 2`, unsigned (§4.1 step 6, §7);
/// [choice] period 0 divides by 1 (a full pass every month; the original
/// faults).
pub fn blocks_per_month(cells_x: i32, cells_z: i32, period: u32) -> u32 {
    let cells = (cells_x as u32).wrapping_mul(cells_z as u32);
    (cells / period.max(1)) >> 2
}

/// The wild-seed scaling of §7:
/// `u32(trunc(float(u32(trunc(float(w) × sizeX))) × sizeZ)) >> 20`.
pub fn scale_wild_seeds(w: u32, size_x: f32, size_z: f32) -> u32 {
    let a = ftol(f64::from(r24(f64::from(w) * f64::from(size_x)))) as u32;
    let b = ftol(f64::from(r24(f64::from(a) * f64::from(size_z)))) as u32;
    b >> 20
}

/// The flora blocking filter (§6.4.1): flora, building, landfill and prop
/// occupants, and network occupants with a road-like flag. `network_flags`
/// is the network occupant's `GetNetworkFlag()` (`None` when the QI fails).
pub fn blocks_flora(occ_type: u32, network_flags: Option<u32>) -> bool {
    match occ_type {
        occupant_type::FLORA | occupant_type::BUILDING | occupant_type::LANDFILL | occupant_type::PROP => true,
        occupant_type::NETWORK => network_flags.is_some_and(|f| f & BLOCKING_NETWORK_FLAGS != 0),
        _ => false,
    }
}

/// `SC4GeometryUtilities::BoundingBoxesIntersect(aMin, aMax, bMin, bMax,
/// null, null)` (§6.4 [compat]): strict on every axis, `aMin < bMax` and
/// `bMin < aMax`, so boxes that only touch do not intersect; an unordered
/// compare (NaN) counts as overlapping on that axis.
pub fn boxes_intersect(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    // "Not separated": `x >= y` is false for NaN, so NaN overlaps.
    let separated = |lo: f32, hi: f32| lo >= hi;
    (0..3).all(|i| !separated(a.0[i], b.1[i]) && !separated(b.0[i], a.1[i]))
}

// ---- the occupant manager's 64 m index (§6.4.2, FQ6, FQ15) ----------------------------------------

/// The side of the occupant manager's index in cells (§6.4.2): 192 × 192
/// cells of 64 m; city (0, 0) is cell (64, 64).
pub const MANAGER_GRID: i32 = 192;

/// `cell(p)` of the occupant manager's index (§6.4.2, slot 15):
/// `rhe(f32(f32(p + 4096) × (1/64)) − 0.499f)`, every step a `float32`
/// operation and `rhe` the original's idiom ([`crate::x87::rhe`]). That is
/// `floor((p + 4096) / 64)`, except that a coordinate less than about
/// 0.064 m below a 64 m line maps to the cell above the line.
pub fn manager_cell(p: f32) -> i32 {
    let s = (p + 4096.0f32) * 0.015625f32;
    crate::x87::rhe(s - 0.499f32)
}

/// The cells `(x0, z0, x1, z1)` an occupant is listed in (§6.4.2,
/// `InsertOccupant` / `MoveOccupant`), from its box at that moment: on x and
/// on z separately, a box wider than 1 m (`max − min > 1.0`, `float32`) has
/// both ends moved inwards by `0.1f` first. y plays no part.
///
/// *[choice]* (§6.4.2, AQ7, recommended): each cell is clamped to
/// `0..=191`, like the query's. The original keeps each cell's low byte with
/// no test against 192, so an edge more than a city width outside the city
/// writes past its index or is listed nowhere; inside the index nothing
/// changes.
pub fn manager_listing(min: Vec3, max: Vec3) -> CellRect {
    let inset = |lo: f32, hi: f32| {
        if hi - lo > 1.0 {
            (lo + 0.1f32, hi - 0.1f32)
        } else {
            (lo, hi)
        }
    };
    let (x0, x1) = inset(min[0], max[0]);
    let (z0, z1) = inset(min[2], max[2]);
    let c = |p: f32| manager_cell(p).clamp(0, MANAGER_GRID - 1);
    (c(x0), c(z0), c(x1), c(z1))
}

/// The cells a box query reads (§6.4.2): both corners mapped to cells and
/// clamped to `0..=191`.
pub fn manager_query_cells(min: Vec3, max: Vec3) -> CellRect {
    let c = |p: f32| manager_cell(p).clamp(0, MANAGER_GRID - 1);
    (c(min[0]), c(min[2]), c(max[0]), c(max[2]))
}

/// Whether a box query `q` (occupant manager slot 36, §6.4.2) returns an
/// occupant listed in `listed` ([`manager_listing`]) whose current box is
/// `occ`: listed in one of the query's cells ([`manager_query_cells`]) and
/// overlapping it strictly on x, y and z ([`boxes_intersect`]). The type
/// filter, if any, is the caller's.
pub fn manager_query_returns(listed: CellRect, occ: (Vec3, Vec3), q: (Vec3, Vec3)) -> bool {
    let (qx0, qz0, qx1, qz1) = manager_query_cells(q.0, q.1);
    let (lx0, lz0, lx1, lz1) = listed;
    let meets = |a0: i32, a1: i32, b0: i32, b1: i32| a0 <= a1 && b0 <= b1 && a0 <= b1 && b0 <= a1;
    meets(qx0, qx1, lx0, lx1) && meets(qz0, qz1, lz0, lz1) && boxes_intersect(occ, q)
}

/// The world box of a standard-city-cell query (§6.10.4, slot 40) for the
/// cells `(x0, z0)..(x1, z1)` once the corners are settled:
/// `(16·x0 + 0.1f, −FLT_MAX, 16·z0 + 0.1f) … (16·(x1 + 1) − 0.1f, +FLT_MAX,
/// 16·(z1 + 1) − 0.1f)`, each a `float32` add or subtract. A monthly
/// block `(bx, bz)` is the cells `(2bx, 2bz)..(2bx + 1, 2bz + 1)` (§6.7.1).
pub fn cell_query_box(cells: CellRect) -> (Vec3, Vec3) {
    let (x0, z0, x1, z1) = cells;
    let lo = |c: i32| 16.0 * c as f32 + 0.1f32;
    let hi = |c: i32| 16.0 * c.wrapping_add(1) as f32 - 0.1f32;
    ([lo(x0), -f32::MAX, lo(z0)], [hi(x1), f32::MAX, hi(z1)])
}

/// The corners of `GetOccupantsByStandardCityCells` (§6.10.4) for a city of
/// `cells` standard cells: null corners (`None`) are cell (0, 0) and
/// `(nx, nz)` (not `n − 1`); given corners are clamped to `[0, n − 1]` per
/// axis.
pub fn standard_cell_corners(rect: Option<CellRect>, cells: (i32, i32)) -> CellRect {
    let (nx, nz) = cells;
    match rect {
        None => (0, 0, nx, nz),
        Some((x0, z0, x1, z1)) => {
            let cx = |v: i32| v.min(nx - 1).max(0);
            let cz = |v: i32| v.min(nz - 1).max(0);
            (cx(x0), cz(z0), cx(x1), cz(z1))
        }
    }
}
