// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The network manager (`cSC4NetworkManager`, class `0xC990BD46`, city
//! component 53) and the network lot manager (`cSC4NetworkLotManager`, class
//! `0xE9A6CD3B`, component 11), `docs/specs/network-manager.md`. Section
//! references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - `flags.rs`: the network flags of §5, the usable message, the pipe model
//!   choice and the removal counters of §10.
//! - `manager.rs`: [`NetworkManager`]: the state of §2.1, `Init` /
//!   `Shutdown` (§3.1), `DoMessage` for the 11 subscribed messages (§6.1,
//!   the globally managed list with swap-and-pop), the tool cache (§7.1),
//!   `DestroyNetworkOccupants` / `QueueRemovals` / `RemoveOrReplaceOccupant`
//!   (§7.2–§7.4), the small slots (§7.5), the terrain rule (§7.6), the
//!   retrofit list (§7.10), the construction crews (§7.12) and the record of
//!   §4.1.
//! - `config.rs`: the network lot configuration (§2.4), `IsLotCompatible`
//!   (§8.4), the zone and wealth choice (§8.2, §8.3), the road style tables
//!   (§8.7), the object position formula (§8.6) and the pothole offsets
//!   (§8.8).
//! - `lots.rs`: [`NetworkLotManager`]: the state of §2.2, `Init` / `Shutdown`
//!   (§3.2), `DoMessage` (§6.2), choosing, keeping and removing network lots
//!   (§8.5), the lot objects and `RedevelopLot` (§8.6), road and avenue style
//!   (§8.7), potholes (§8.8), the random draws (§17) and the record of §4.2.
//! - `riot.rs`: the riot's removal lock (§10, the riot manager's slot 9
//!   contract).
//! - `tests.rs`: fake hosts and the §16 tests 1–23.
//!
//! Everything outside the two classes goes through [`NetworkManagerHost`] and
//! [`NetworkLotHost`]: the occupants (network occupants, effect props, lots),
//! the terrain, the tools, the zone grid, the lot developer's slope, the prop
//! manager, the flora and the traffic network map. Objects are host tokens.
//! The traffic graph built from the occupants is traffic-network.md's
//! (`crate::traffic_network`); this module only reads the map's
//! `FindNetworkTile` / `UpdateIntersection` through the host and reuses its
//! network type numbers and side offsets.
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **Ordered maps** [choice]: the original's hash maps (`lots`, `potholes`,
//!   `configs`) iterate in bucket order. FreeCity4 keeps insertion order (an
//!   overwritten key keeps its place), so `Read` followed by `Write` gives the
//!   same bytes (§4.2). `ProcessUpdateList` uses the original's SGI hash-set
//!   order (§8.5, NM13).
//! - **QueueRemovals order**: the host's per-occupant creation number
//!   ([`NetworkManagerHost::creation_number`]) instead of the pointer value
//!   (the spec's *[choice]*, §7.3).
//! - **Null tools** (§7.2, §7.6, §7.4): a group whose cached tool is null
//!   (after PreCityShutdown or Shutdown) is skipped [choice] (the original
//!   would call through null).
//! - **Read of component 53** (§4.1): a null reference is appended as a null
//!   entry and counts as read [question NM6]; a reference that does not
//!   resolve fails the read at once, keeping the entries appended so far.
//! - **Read of component 11** (§4.2): a reference that does not resolve is
//!   stored as null and the loop goes on; a stream error stops the loop
//!   [choice] (every later read would fail; the result is false either way).
//! - **Shutdown of the lot manager** (§3.2): after releasing the lots and the
//!   pothole props the two maps are cleared [choice] (the spec infers the
//!   destructor clears them).
//! - **The retrofit** (§7.10): FreeCity4 loads only 1.1.641 saves [choice];
//!   the host's [`NetworkManagerHost::retrofit`] does whatever model work it
//!   wants for each entry still asking for it; the list is then emptied and
//!   each entry released [question NM7].
//! - **Construction crews** (§7.12): the PreCityShutdown loop runs at most
//!   the starting count of times (the spec's *[choice]*).
//! - **`0xA9B6C2D6`** (§6.2, never posted): `void1` is read as a network
//!   lot's cell key [choice].
//! - **Zone and wealth values outside the tables** (§8.2, §8.4, §8.7): a
//!   neighbour zone outside 0..15 never wins; a configuration test with a
//!   zone outside 0..15 or a wealth outside 0..3 fails; the style tables give
//!   a zone outside 0..15 the "every other zone" rows [choice] [question NM8].
//! - **Random draws** (§17): one [`NetworkLotRandom`] behind the generator
//!   `0x00B44870`, an `RzRandomSuper` seeded with `derive_seed(S,
//!   0x8C09C379)` (foundations §6.3).
//! - **Leaks reproduced** *[quirk]*: `DestroyNetworkOccupants` keeps one
//!   reference per destroyed element (§7.2); the lot manager's `Init` drops
//!   lots with a missing configuration without releasing them (§3.2); an
//!   overwritten lot or pothole entry is not released (§4.2, §8.5).
//! - **Threading** (foundations §19.3): the state is a `RefCell`; both cores
//!   are main-thread only. No borrow is held across a host call, so a crew's
//!   slot 5 may call back into `unregister_construction_crew`.

mod config;
mod flags;
mod lots;
mod manager;
mod ordered;
mod riot;

#[cfg(test)]
mod tests;

pub use config::{
    corner_mismatch, is_lot_compatible_pattern, object_position, pothole_offset, prop, road_texture, road_variation,
    slope_fits, terrain_rect, wealth_choice, zone_choice, zone_priority, NetworkLotConfig, NetworkLotObject,
    PotholeOffset, NEIGHBOUR_ORDER, OBJECT_FLORA, OBJECT_PROP,
};
pub use flags::{
    clear_network_flag, flag, has_all, has_any, pipe_model, set_network_flag, FlagChange, PipeModel, RemovalCounters,
};
pub use lots::{
    LotEntry, NetworkLotManager, NetworkLotState, AVENUE_EDGE_KIND, FLIPPED_PROP_FLAG, POTHOLE_NAME, POTHOLE_RAIL_NAME,
    RAIL_TEXTURE,
};
pub use manager::{NetworkManager, NetworkManagerState, DESTROY_GROUPS};
pub use ordered::OrderedMap;
pub use riot::{RiotHost, RiotLock, PLAYER_REMOVAL};

use crate::grid::CellRect;
use crate::stream::RefValue;

/// Class IDs and interface IDs (§1).
pub const CLSID_NETWORK_MANAGER: u32 = 0xC990_BD46;
pub const IID_NETWORK_MANAGER: u32 = 0xA976_4C8C;
pub const CLSID_NETWORK_LOT_MANAGER: u32 = 0xE9A6_CD3B;
pub const IID_NETWORK_LOT_MANAGER: u32 = 0x89A5_BA3F;
/// `cISC4NetworkLot` (§2.3).
pub const IID_NETWORK_LOT: u32 = 0x89A5_BA66;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// `cISC4NetworkOccupant`: managed list and lot entries are read with it
/// (§4.1, §4.2).
pub const IID_NETWORK_OCCUPANT: u32 = 0xA821_EF94;
/// The pothole effect prop (§4.2, §8.8).
pub const IID_EFFECT_PROP: u32 = 0x0982_0792;
pub const CLSID_EFFECT_PROP: u32 = 0x2977_AA49;
/// `cISC4Lot` (§6.2).
pub const IID_LOT: u32 = 0xE818_FCED;
/// The occupant type of network occupants (§6.1).
pub const OCCUPANT_TYPE_NETWORK: u32 = 0xC772_BF98;

/// City components and city getter slots (§1, foundations §3.6).
pub const COMPONENT_NETWORK_MANAGER: u32 = 53;
pub const COMPONENT_NETWORK_LOT_MANAGER: u32 = 11;
pub const CITY_SLOT_NETWORK_MANAGER: u32 = 38;
pub const CITY_SLOT_NETWORK_LOT_MANAGER: u32 = 42;

/// Record versions (§4).
pub const MANAGER_RECORD_VERSION: u16 = 1;
pub const LOT_RECORD_VERSION: u16 = 2;

/// The number of network types and tools (§3.1 step 5).
pub const TOOL_COUNT: usize = 13;

/// The English network names of slot 22 (§7.5, table `0x00AA845C`); entry 13
/// is "road".
pub const ENGLISH_NETWORK_NAMES: [&str; 14] = [
    "Road",
    "Rail",
    "Highway",
    "Street",
    "Pipe",
    "PowerLine",
    "Avenue",
    "Subway",
    "LightRail",
    "Monorail",
    "OneWayRoad",
    "DirtRoad",
    "GroundHighway",
    "road",
];

/// Slot 22 `GetEnglishNetworkName(type)` (§7.5). The original has no bounds
/// check *[quirk]*; past entry 13 FreeCity4 answers `None` [choice].
pub fn english_network_name(network_type: u32) -> Option<&'static str> {
    ENGLISH_NETWORK_NAMES.get(network_type as usize).copied()
}

/// Messages (§3, §6).
pub mod msg {
    pub const VIEW_CHANGED: u32 = 0xA6B7_9602;
    pub const TERRAIN_CHANGED: u32 = 0x278E_6003;
    pub const LONG_TERRAIN_BEGIN: u32 = 0xEA3B_0AF1;
    pub const LONG_TERRAIN_END: u32 = 0x6A3B_0AFD;
    pub const WATER_DISPLAY_CHANGED: u32 = 0x6AB9_7795;
    pub const PRE_CITY_SHUTDOWN: u32 = 0x26D3_1EC2;
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const DAWN: u32 = 0xC9DA_96E8;
    pub const DUSK: u32 = 0xC9DA_96E9;
    /// The network manager's subscriptions (§3.1 step 3), in order.
    pub const MANAGER_SUBSCRIPTIONS: [u32; 11] = [
        VIEW_CHANGED,
        TERRAIN_CHANGED,
        LONG_TERRAIN_BEGIN,
        LONG_TERRAIN_END,
        WATER_DISPLAY_CHANGED,
        PRE_CITY_SHUTDOWN,
        POST_CITY_INIT,
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        DAWN,
        DUSK,
    ];
    /// `kSC4MessageNetworkOccupantUsableChange` (§5).
    pub const USABLE_CHANGED: u32 = 0x89D3_E9F3;
    /// Subscribed but never posted (§6.2) *[quirk]*.
    pub const CREATE_LOT_OBJECTS: u32 = 0xA9B6_C2D6;
    pub const ZONING_CHANGED: u32 = 0x0769_9455;
    pub const LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;
    /// traffic-simulator.md §3.6.
    pub const ROAD_DAMAGE_CHANGED: u32 = 0x6924_7DC8;
    /// The network lot manager's subscriptions (§3.2 step 2), in order.
    pub const LOT_SUBSCRIPTIONS: [u32; 6] = [
        USABLE_CHANGED,
        OCCUPANT_REMOVED,
        CREATE_LOT_OBJECTS,
        ZONING_CHANGED,
        LOT_STATE_CHANGED,
        ROAD_DAMAGE_CHANGED,
    ];
    /// The presentation refresh posted for an occupant (§6.1, §7.10).
    pub const OCCUPANT_REFRESH: u32 = 0x99EF_1145;
}

/// Terrain change kinds (`data4` of `0x278E6003`, §7.6; NM3).
pub mod terrain_kind {
    /// The only kind that also destroys pipes and subways.
    pub const UNDERGROUND: u32 = 3;
    /// No network is destroyed.
    pub const KEEP_ALL: u32 = 4;
    /// The levelling reason (landfill.md): no query at all.
    pub const LEVELLING: u32 = 5;
    /// Pieces with network flag `0x80000` survive.
    pub const SKIP_80000: u32 = 6;
}

/// A 3D position (`cS3DVector3`): x, y, z.
pub type Position = [f32; 3];

/// The cell key `(x & 0xFF) << 8 | z` (u16, §2.2). `z` is not masked: its
/// high bits are cut by the 16-bit width only.
pub fn lot_key(x: i32, z: i32) -> u16 {
    ((((x & 0xFF) as u32) << 8) | z as u32) as u16
}

/// The cell of a key: `(key >> 8, key & 0xFF)`.
pub fn key_cell(key: u16) -> (i32, i32) {
    (i32::from(key >> 8), i32::from(key & 0xFF))
}

/// The empty `dirty` rectangle (§2.1): `0x7FFFFFFF, 0x7FFFFFFF, −1, −1`.
pub const EMPTY_RECT: CellRect = CellRect {
    x1: 0x7FFF_FFFF,
    z1: 0x7FFF_FFFF,
    x2: -1,
    z2: -1,
};

/// Whether a rectangle is empty (`x1 > x2` or `z1 > z2`).
pub fn rect_is_empty(r: &CellRect) -> bool {
    r.x1 > r.x2 || r.z1 > r.z2
}

/// A message as the network manager's `DoMessage` sees it (§6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message.
    pub void1: Option<u64>,
    /// The two values `*(void1 + 4)` and `*(void2 + 4)` of the view message
    /// (`0xA6B79602`), read by the host.
    pub view_old: u32,
    pub view_new: u32,
}

impl NetworkMessage {
    pub fn of_type(msg_type: u32) -> NetworkMessage {
        NetworkMessage {
            msg_type,
            ..NetworkMessage::default()
        }
    }
}

/// A message as the network lot manager's `DoMessage` sees it (§6.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkLotMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1()` (`0x99EF1143`: the occupant; `0xA9B6C2D6`: the lot).
    pub void1: Option<u64>,
    /// `GetIGZUnknown()` (`0x89D3E9F3`: the network occupant;
    /// `0xE8BDA2E7`: the lot).
    pub unknown: Option<u64>,
    /// `*data2` of `0x07699455` (a cell rectangle), read by the host.
    pub rect: Option<CellRect>,
}

impl NetworkLotMessage {
    pub fn of_type(msg_type: u32) -> NetworkLotMessage {
        NetworkLotMessage {
            msg_type,
            ..NetworkLotMessage::default()
        }
    }
}

/// `RandomUint32Uniform(n)` on the network lot manager's generator
/// `0x00B44870` (§17).
pub trait NetworkLotRandom {
    fn uniform_u32(&mut self, n: u32) -> u32;
    fn raw_state(&self) -> Option<([u32; 624], usize)> {
        None
    }
}

impl NetworkLotRandom for crate::rng::RzRandomSuper {
    fn raw_state(&self) -> Option<([u32; 624], usize)> {
        Some(crate::rng::RzRandomSuper::raw_state(self))
    }
    fn uniform_u32(&mut self, n: u32) -> u32 {
        crate::rng::RzRandomSuper::uniform_u32(self, n)
    }
}

/// Everything the network manager reads from, or does to, the rest of the
/// game (§12). Objects are host tokens. Re-entrancy: only
/// [`NetworkManagerHost::crew_shutdown`] may call back into the manager (to
/// unregister the crew).
pub trait NetworkManagerHost {
    // ---- messages and services (§3.1) ----
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// `Init` step 2: (language manager slot 38, then slot 25) `== 0`.
    fn uk_paths(&self) -> bool;
    /// The city's cell counts `X`, `Z` (city slots 93 and 94).
    fn city_size(&self) -> (i32, i32);
    /// The 24-hour clock's `IsNight` (clock slot 28).
    fn is_night(&self) -> bool;
    /// `DoViewRotationChange` (presentation, §6.1).
    fn view_rotation_changed(&self);

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- tools (§3.1 step 5, §7) ----
    /// A new tool of `network_type` with the caller's reference (`None`:
    /// allocation failed). The classes are those of §3.1 step 5.
    fn create_tool(&self, network_type: u32) -> Option<u64>;
    fn tool_init(&self, tool: u64);
    fn tool_shutdown(&self, tool: u64);
    /// Tool slot 50 `DestroyOccupantsAndResolve(list)` (§7.2).
    fn tool_destroy_occupants(&self, tool: u64, occupants: &[u64]);
    /// `UpdateUndergroundOccupantTerrainHeights(cells, list, type)` (§7.6).
    fn tool_update_underground_heights(&self, tool: u64, cells: &[(i32, i32)], occupants: &[u64], network_type: u32);
    /// `tool.GetCellInfo((z << 16) + x)` (§7.4 step 2.2): an opaque info.
    fn tool_cell_info(&self, tool: u64, packed: u32) -> u64;
    /// §7.4 step 2.3: set the tool's byte `+0x50`, then
    /// `InsertNetworkOccupant(info, {0, 0, 0}, 0)` (tool slot 72).
    fn tool_reinsert_piece(&self, tool: u64, info: u64);

    // ---- occupants ----
    /// `QueryInterface(0xA821EF94)`: the network occupant, with a reference
    /// for the caller.
    fn query_network_occupant(&self, occupant: u64) -> Option<u64>;
    /// The occupant is a network occupant (§7.4 step 1; no reference kept).
    fn is_network_occupant(&self, occupant: u64) -> bool;
    /// `GetNetworkFlag()` (slot 12); 0 for an occupant that is not a network
    /// occupant.
    fn network_flags(&self, n: u64) -> u32;
    /// `IsOfType(type)` (slot 22).
    fn is_of_type(&self, n: u64, network_type: u32) -> bool;
    /// `cISC4Occupant::GetFlags()` (bit 1: in the city).
    fn occupant_flags(&self, occupant: u64) -> u32;
    /// `cISC4Occupant` slot 32 (`IsFlagSet`-style test of an occupant flag).
    fn occupant_flag_set(&self, occupant: u64, flag: u32) -> bool;
    /// `GetOccupiedCell()` (slot 59).
    fn occupied_cell(&self, n: u64) -> (i32, i32);
    /// `GetAltitude(x, z)` (slot 56, float).
    fn occupant_altitude(&self, n: u64, x: f32, z: f32) -> f32;
    /// Slot 5 `IsManagedGlobally`.
    fn is_managed_globally(&self, n: u64) -> bool;
    /// Slot 62 `CanBeLit` / slot 61 `SetLit`.
    fn can_be_lit(&self, n: u64) -> bool;
    fn set_lit(&self, n: u64, lit: bool);
    /// Slot 6 `CheckForRetrofit`.
    fn check_for_retrofit(&self, n: u64) -> bool;
    /// §7.10 steps 1–4 for one entry (models and paths; presentation).
    fn retrofit(&self, n: u64);
    /// A stable per-occupant creation number: the `QueueRemovals` sort key
    /// (§7.3 *[choice]*).
    fn creation_number(&self, occupant: u64) -> u64;

    // ---- occupant manager (§12) ----
    /// Slot 21 `RemoveOccupant(occ, false, 0)`.
    fn remove_occupant(&self, occupant: u64) -> bool;
    /// Slot 40: the occupants in `rect` whose network flags meet `mask` (an
    /// OR filter), in query order (borrowed).
    fn occupants_in_rect(&self, rect: CellRect, mask: u32) -> Vec<u64>;
    /// Slot 45: every network occupant (type `0xC772BF98`), borrowed.
    fn network_occupants(&self) -> Vec<u64>;
    /// Posts `0x99EF1145` for an occupant (`0x00610BB0`).
    fn post_refresh(&self, n: u64);

    // ---- terrain ----
    /// Terrain slot 13 `GetAltitude(x, z)` (float).
    fn terrain_altitude(&self, x: f32, z: f32) -> f32;

    // ---- construction crews (§7.12) ----
    /// Crew slot 5 (the crew is expected to unregister itself).
    fn crew_shutdown(&self, crew: u64);

    // ---- save references (§4) ----
    fn object_ref(&self, token: u64) -> RefValue;
    /// `SC4ReadGZSerializable(stream, iid, &ptr, 0)`: the object with one
    /// reference for the reader, or `None` (the read failed).
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}

/// Everything the network lot manager reads from, or does to, the rest of
/// the game (§12). Objects are host tokens. Re-entrancy: callbacks must not
/// call back into the manager.
pub trait NetworkLotHost {
    // ---- messages (§3.2) ----
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// `Init` step 4: every network lot configuration (exemplar type `0x21`)
    /// that reads, in the resource manager's key-list order.
    fn load_configurations(&self) -> Vec<NetworkLotConfig>;

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- network occupants (§11.3) ----
    /// `QueryInterface(0xA821EF94)`: the network occupant, with a reference
    /// for the caller.
    fn query_network_occupant(&self, unknown: u64) -> Option<u64>;
    /// `GetNetworkFlag()`.
    fn network_flags(&self, n: u64) -> u32;
    /// Slot 23 `PieceId`.
    fn piece_id(&self, n: u64) -> u32;
    /// Slot 24 `GetRotation` / slot 25 `GetFlip`.
    fn rotation(&self, n: u64) -> i32;
    fn flip(&self, n: u64) -> i32;
    /// `city.PositionToCell(n.AsOccupant().GetPosition())`.
    fn position_cell(&self, n: u64) -> (i32, i32);
    /// Slot 59 `GetOccupiedCell`.
    fn occupied_cell(&self, n: u64) -> (i32, i32);
    /// Slot 56 `GetAltitude(x, z)` (float).
    fn occupant_altitude(&self, n: u64, x: f32, z: f32) -> f32;
    /// Slot 55 `IsIntersection`.
    fn is_intersection(&self, n: u64) -> bool;
    /// Slot 38 `GetEdgeTypesFlag(kind)`.
    fn edge_types_flag(&self, n: u64, kind: u32) -> u32;
    /// Slot 40 `GetActiveEdges`.
    fn active_edges(&self, n: u64) -> u32;
    /// Slot 10 `SetNetworkFlag(bits)` (the occupant posts the usable message
    /// itself, §5).
    fn set_network_flag(&self, n: u64, bits: u32);
    /// Slot 29 `SetUnderTexture` / slot 27 `SetVariation`.
    fn set_under_texture(&self, n: u64, texture: u32);
    fn set_variation(&self, n: u64, variation: u32);

    // ---- traffic network map (traffic-network.md) ----
    /// The traffic network map exists (city slot 40).
    fn traffic_map_exists(&self) -> bool;
    /// Slot 8 `FindNetworkTile(x, z, mask, true)` (borrowed).
    fn find_network_tile(&self, x: i32, z: i32, mask: u32) -> Option<u64>;
    /// Slot 36 `UpdateIntersection(occupant)`.
    fn update_intersection(&self, n: u64);

    // ---- city, terrain, zones ----
    fn city_size(&self) -> (i32, i32);
    /// City slot 97 `CellCenterToPosition(x, z)`: (x, z) in metres.
    fn cell_center(&self, x: i32, z: i32) -> (f32, f32);
    /// Terrain slot 13 `GetAltitude(x, z)`.
    fn terrain_altitude(&self, x: f32, z: f32) -> f32;
    /// The zone manager's zone grid at a cell (a signed byte).
    fn zone_type(&self, x: i32, z: i32) -> i32;

    // ---- lots (§8.3) ----
    /// Lot manager slot 6 `GetLot(x, z, false)` (borrowed).
    fn lot_at(&self, x: i32, z: i32) -> Option<u64>;
    /// Lot slot 41 `GetOccupantWealth`.
    fn lot_wealth(&self, lot: u64) -> i32;
    /// Lot slot 37 `IsInLivableState`.
    fn lot_is_livable(&self, lot: u64) -> bool;
    /// Lot slot 46 `GetBuildingType(false)`.
    fn lot_building_type(&self, lot: u64) -> u32;
    /// The building development simulator's profile for a building type
    /// (BDS slot 40): its wealth, `None` without a profile.
    fn building_profile_wealth(&self, building_type: u32) -> Option<i32>;
    /// `QueryInterface(0xE818FCED)`: the lot, with a reference.
    fn query_lot(&self, unknown: u64) -> Option<u64>;
    /// Lot slot 14 `GetBoundingRect` (inclusive cells).
    fn lot_bounding_rect(&self, lot: u64) -> CellRect;
    /// `lotDeveloper.GetAreaSlope({x, z, x, z}, true)` (lot developer slot
    /// 29, through BDS slot 36).
    fn area_slope(&self, x: i32, z: i32) -> f32;

    // ---- props and flora (§8.5, §8.6) ----
    /// Prop manager slot 33 `RemoveStreetPropsFromTile(x, z)` (already masked
    /// to 8 bits).
    fn remove_street_props(&self, x: i32, z: i32);
    /// Prop manager slot 32 `AddStreetProp(id, pos, orientation)` (borrowed).
    fn add_street_prop(&self, id: u32, pos: Position, orientation: u32) -> Option<u64>;
    /// Street prop slot 28 (the object's field `+0x24`) and slot 29 (`0x80`
    /// on a flipped piece).
    fn street_prop_slot28(&self, prop: u64, value: u32);
    fn street_prop_slot29(&self, prop: u64, value: u32);
    /// Flora slot 20 `DemolishFloraInCellArea({x, z, x, z}, 0)`.
    fn demolish_flora(&self, x: i32, z: i32);
    /// Flora slot 17 `AddNewFloraOccupant(id, pos, true)` (borrowed).
    fn add_flora(&self, id: u32, pos: Position) -> Option<u64>;
    /// Flora occupant slot 28 `(index)`.
    fn flora_slot28(&self, flora: u64, index: u32);
    /// The lot configuration manager's `GetObjectIDArray(object)` (slot 27)
    /// for object `index` of the configuration `config` (its host token).
    fn object_ids(&self, config: u64, index: usize) -> Vec<u32>;

    // ---- potholes (§8.8) ----
    /// A new effect prop (class `0x2977AA49`, IID `0x09820792`) with the
    /// caller's reference.
    fn create_effect_prop(&self) -> Option<u64>;
    /// Effect prop slot 6 `(name)` / 7 `(1)` start / 8 `(1)` stop (NM5).
    fn effect_set_name(&self, prop: u64, name: &str);
    fn effect_start(&self, prop: u64);
    fn effect_stop(&self, prop: u64);
    /// `prop.AsOccupant().SetPosition(pos)`.
    fn set_position(&self, prop: u64, pos: Position);
    /// Occupant manager slot 20 `InsertOccupant(occ, 0)` / slot 21
    /// `RemoveOccupant(occ, true, 0)`.
    fn insert_occupant(&self, occupant: u64);
    fn remove_occupant(&self, occupant: u64);
    /// Occupant manager slot 32: the first occupant at the cell whose network
    /// flags meet `mask` (borrowed).
    fn first_network_occupant_at(&self, x: i32, z: i32, mask: u32) -> Option<u64>;

    // ---- save references (§4.2) ----
    fn object_ref(&self, token: u64) -> RefValue;
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}

pub mod archive;

/// `QueryInterface` of the network manager (§1).
pub fn manager_query_interface(iid: u32) -> bool {
    matches!(
        iid,
        IID_UNKNOWN | IID_NETWORK_MANAGER | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE
    )
}

/// `QueryInterface` of the network lot manager (§1).
pub fn lot_manager_query_interface(iid: u32) -> bool {
    matches!(
        iid,
        IID_UNKNOWN | IID_NETWORK_LOT_MANAGER | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE
    )
}
