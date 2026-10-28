// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The demolition utility (`cSC4Demolition`, class `0x0990C38A`),
//! `docs/specs/demolition.md`. Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - `utility.rs`: the state of §2, `Init` / `Shutdown` (§4.1), the
//!   demolition filter Φ (§4.2), `DemolishOccupant` (§4.3),
//!   `DemolishRegion` (§4.4), `DemolishLots` (§4.5), `UpdateNetworks`
//!   (§4.6), `ModifyTerrainHeight` (§4.7), the effects (§4.8), the messages
//!   (§3), the tuning (§6) and the record (§7).
//! - `tests.rs`: a fake city and the §12 tests.
//!
//! Everything outside the class goes through [`DemolitionHost`]: the city,
//! the occupant manager, the lot manager and lot developer, the zone
//! manager, the terrain, the network manager, the 3D view and its view
//! filter, the shared occupant filters, the simulators that price
//! occupants, the effects and the caller-owned containers.
//!
//! # Tokens and references
//!
//! Objects are opaque host tokens. Tokens the host hands out (occupants,
//! lots, pole connections, filters) are **borrowed** for the duration of the
//! call: the core never AddRefs or Releases them, except the view filter
//! (`acquire_view_filter`, AddRef'd by `GetCurrentViewFilterInfo`, §4.4
//! step 2) and the lot of a deferred lot demolition (§3), which the core
//! releases. The `occs` set of §4.4 holds `cRZAutoRefCount`s in the
//! original; here it is a set of borrowed tokens. The set order is the
//! token order (the plugin's tokens are the objects' addresses, the
//! original's set order).
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **No 3D view** (§4.4 step 2): the view filter is null and the view
//!   level is 1 (the spec's *[choice]*).
//! - **A null entry** in `DemolishLots`' list is skipped (the spec's
//!   *[choice]*).
//! - **A foundation without `cISC4FoundationOccupant`** (§4.2 step 5) is
//!   rejected: the spec's *[choice]* (DM5; the original crashes).
//! - **The 0.05 tolerance** of §4.7 is a `float` comparison: `|alt − h|` in
//!   `f32` against `0.05f32` (DM6). Per-vertex mode skips `|d| < 0.05f` as
//!   §4.7 states, so a vertex exactly 0.05f away is set there; the spec's
//!   "counted in neither" and §12 test 14 say otherwise [question DM15].
//! - **The area-mode redisplay rectangle** (§4.7 step 4) is the `SetAltitude`
//!   rectangle (DM7).
//! - **The group effect size** (§4.8, DM8): unsigned and strict, 2 when `n`
//!   exceeds the medium/large group threshold, else 1 when it exceeds the
//!   small/medium one, else 0; the centre cell is `((x0 + x1) / 2,
//!   (z0 + z1) / 2)` with truncating division.
//! - **`UpdateNetworks`' kinds** (§4.6, DM9): by occupant type alone, power
//!   pole `0x2890D4DE` and network `0xC772BF98` (no interface test).
//! - **The deferred lot demolition** (§3, `0xA9DED179`) releases the lot
//!   whenever it is non-null, initialised or not: the spec's *[choice]*
//!   (DM10; the original leaks an uninitialised lot's reference, which
//!   nothing can observe).
//! - **The 3D view's filter** (§4.4 step 2, DM12) is fetched on every
//!   `DemolishRegion`; `useView` gates only Φ. The empty-lot qualification
//!   and the effect gate use it directly, also with privilege 0 or `0x2000`.
//! - **Pole orphans** (§4.3, DM11): tokens are borrowed, so the remembered
//!   poles that keep connections need no release; an orphan is appended to
//!   the caller's list (`list_append`, the host takes its reference) or
//!   dropped when there is no list. The net reference balance is the
//!   original's (zero); the glue keeps the sets' own references.
//! - **The tuning** (§6) is per object here (the original's module globals);
//!   the glue keeps one utility per city, so nothing observable changes.
//! - **Threading** (foundations §19.3): the state is a `RefCell`; the core is
//!   main-thread only. No borrow is held across a host call; host callbacks
//!   must not call back into the utility.

mod utility;
pub mod archive;

#[cfg(test)]
mod tests;

pub use utility::{query_interface, DemolitionState, DemolitionUtility, Filter};

use std::rc::Rc;

use crate::grid::CellRect;
use crate::property::PropertyHolder;
pub use crate::zones::CellRegion;

/// Class ID and interface ID (§1).
pub const CLSID_DEMOLITION: u32 = 0x0990_C38A;
pub const IID_DEMOLITION: u32 = 0x26DD_E2EA;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// Orphan poles are appended as `cISC4Occupant` (§4.3).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;

/// City component (§1).
pub const CITY_COMPONENT: u32 = 2;

/// Messages (§3).
pub mod msg {
    /// Exemplar changed: `ReadProperties()` unconditionally.
    pub const EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;
    /// PostCityInit: subscribed, ignored.
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    /// Deferred lot demolition (no poster in the executable).
    pub const DEFERRED_LOT_DEMOLITION: u32 = 0xA9DE_D179;
    /// The subscriptions of `Init`, in order.
    pub const SUBSCRIPTIONS: [u32; 3] = [EXEMPLAR_CHANGED, POST_CITY_INIT, DEFERRED_LOT_DEMOLITION];
    /// Sent before a power pole is removed (the neighbours simulator answers).
    pub const PRE_DEMOLISH_NETWORK: u32 = 0xAA7A_014D;
}

/// Occupant types (§4.2 step 5, §4.3).
pub mod occ_type {
    pub const FOUNDATION: u32 = 0x68FD_0C69;
    pub const BUILDING: u32 = 0x2781_28A0;
    pub const POWER_POLE: u32 = 0x2890_D4DE;
    pub const CONSTRUCTION: u32 = 0x096E_763C;
    pub const PROP: u32 = 0xA823_821E;
    pub const NETWORK: u32 = 0xC772_BF98;
    pub const FLORA: u32 = 0x7475_8926;
}

/// Flag bits of the `flags` argument (§4.9).
pub mod flag {
    /// The "cause" byte: the effect family; 0 = no effects.
    pub const CAUSE: u32 = 0x0000_00FF;
    /// Skip occupants with occupant flag `0x40`.
    pub const SKIP_FLAG_40: u32 = 0x0000_0100;
    /// Explode the lots (unless water-compatible).
    pub const EXPLODE: u32 = 0x0000_0300;
    /// Do not re-parcel agriculture and industrial lots.
    pub const NO_REPARCEL: u32 = 0x0000_0800;
    /// With `EXPLODE`: `ExplodeLot(lot, false)`.
    pub const EXPLODE_FALSE: u32 = 0x0000_1000;
    /// `DemolishLots`: no view filter, no loose occupants of lot cells.
    pub const LOTS_ONLY: u32 = 0x0000_2000;
}

/// Occupant flags (`IsFlagSet`, occupant slot 32).
pub const OCCUPANT_FLAG_IN_MANAGER: u32 = 0x1;
pub const OCCUPANT_FLAG_SKIPPABLE: u32 = 0x40;

/// The network flag mask of Φ step 7.
pub const NETWORK_SURFACE_MASK: u32 = 0x1F4F;

/// Lot states (§4.4).
pub const LOT_STATE_EMPTY: u32 = 1;
pub const LOT_STATE_DESTROYED: u32 = 6;

/// Zone types (§4.4 step 7).
pub const ZONE_AIRPORT: u32 = 11;
pub const ZONE_SEAPORT: u32 = 12;
pub const ZONE_PLOPPED: u32 = 15;
pub const ZONES_REPARCELLED: [u32; 3] = [7, 8, 9];

/// Privileges (§4.9).
pub const PRIVILEGE_SYSTEM: u32 = 0;
pub const PRIVILEGE_PLAYER: u32 = 1;

/// The immovability property (§4.7): an occupant with it true is movable.
pub const PROP_MOVABLE: u32 = 0x29F0_13E6;
/// The effect group and the effect properties (§4.8).
pub const EFFECT_GROUP: u32 = 0x6A35_89A0;
pub const PROP_EFFECT_PROP: u32 = 0x8A5E_5DB8;
pub const PROP_SOUND: u32 = 0x0A93_036F;
pub const GROUP_PROP_FLORA_LIKE: u32 = 0x5004;
/// The tolerance of §4.7: the `float` constant 0.05f (`0x3D4CCCCD`, DM6).
pub const LEVEL_TOLERANCE: f32 = 0.05;
/// The immovable-overlap half side of §4.7 (per-vertex mode).
pub const VERTEX_HALF_SIDE: f32 = 14.5;
/// Metres per cell.
pub const CELL_METRES: f32 = 16.0;

/// The record version (§7).
pub const RECORD_VERSION: u16 = 1;

/// The tuning exemplars (§6): (type, group, instance).
pub const THRESHOLDS_KEY: (u32, u32, u32) = (0x6534_284A, 0x690F_693F, 0x2A8A_09CA);
pub const BUILDING_DEVELOPMENT_KEY: (u32, u32, u32) = (0x6534_284A, 0xE7E2_C2DB, 0xE8DA_7677);
/// Tuning properties (§6).
pub mod prop {
    pub const SMALL_MEDIUM_MODEL: u32 = 0x6A8A_09D2;
    pub const MEDIUM_LARGE_MODEL: u32 = 0x6A8A_09D3;
    pub const SMALL_MEDIUM_GROUP: u32 = 0x6A8A_09D4;
    pub const MEDIUM_LARGE_GROUP: u32 = 0x6A8A_09D5;
    pub const DESTROYED_TILE_COST: u32 = 0x2AB8_45E7;
}

/// The tuning of §6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DemolitionTuning {
    pub small_medium_model: u32,
    pub medium_large_model: u32,
    pub small_medium_group: u32,
    pub medium_large_group: u32,
    /// `kCostToDemolishDestroyedLotTile`.
    pub destroyed_tile_cost: u32,
}

impl DemolitionTuning {
    /// The code defaults (§6).
    pub const DEFAULT: DemolitionTuning = DemolitionTuning {
        small_medium_model: 64,
        medium_large_model: 128,
        small_medium_group: 20,
        medium_large_group: 100,
        destroyed_tile_cost: 3,
    };
    /// The shipped values (`SimCity_1.dat`, §6).
    pub const SHIPPED: DemolitionTuning = DemolitionTuning {
        small_medium_model: 20,
        medium_large_model: 64,
        small_medium_group: 20,
        medium_large_group: 100,
        destroyed_tile_cost: 2,
    };
}

impl Default for DemolitionTuning {
    fn default() -> Self {
        DemolitionTuning::DEFAULT
    }
}

/// The shipped "Demolition Thresholds" exemplar (§6), for tests and tools.
pub fn shipped_thresholds() -> PropertyHolder {
    use crate::variant::{Elem, Variant};
    let s = DemolitionTuning::SHIPPED;
    let mut h = PropertyHolder::new();
    h.add(
        prop::SMALL_MEDIUM_MODEL,
        Variant::Scalar(Elem::U32(s.small_medium_model)),
    );
    h.add(
        prop::MEDIUM_LARGE_MODEL,
        Variant::Scalar(Elem::U32(s.medium_large_model)),
    );
    h.add(
        prop::SMALL_MEDIUM_GROUP,
        Variant::Scalar(Elem::U32(s.small_medium_group)),
    );
    h.add(
        prop::MEDIUM_LARGE_GROUP,
        Variant::Scalar(Elem::U32(s.medium_large_group)),
    );
    h
}

/// The shipped "Building Development" exemplar's demolition property (§6).
pub fn shipped_building_development() -> PropertyHolder {
    use crate::variant::{Elem, Variant};
    let mut h = PropertyHolder::new();
    h.add(
        prop::DESTROYED_TILE_COST,
        Variant::Scalar(Elem::U32(DemolitionTuning::SHIPPED.destroyed_tile_cost)),
    );
    h
}

/// A message as `DoMessage` sees it (§3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DemolitionMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    /// `0xA9DED179`: the record's lot (it holds one reference).
    pub lot: Option<u64>,
    /// `0xA9DED179`: the record's `dezone` byte.
    pub dezone: bool,
}

/// Where `DemolishOccupant` puts orphaned poles (§4.3).
#[derive(Debug)]
pub enum Orphans<'a> {
    /// `orphans` is null: an orphan is dropped (released in the original).
    None,
    /// The caller's `list<cISC4Occupant*>` (host token): `list_append`.
    Host(u64),
    /// `DemolishRegion`'s own list (§4.4 step 5).
    Local(&'a mut Vec<u64>),
}

/// The result of a pricing / demolishing call: the bool (AL) and the value
/// written to `*cost` (`None`: the path that leaves it untouched).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub ok: bool,
    pub cost: Option<i64>,
}

/// Everything the demolition utility reads from, or does to, the rest of
/// the game. Tokens are borrowed (see the module documentation).
/// Re-entrancy: callbacks must not call back into the utility.
pub trait DemolitionHost {
    // ---- lifecycle (§4.1) ----
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// Hold the city (false: none). Called only while none is held.
    fn acquire_city(&self) -> bool;
    fn release_city(&self);
    /// City slots 93 and 94: the cell counts X and Z.
    fn city_cell_counts(&self) -> (i32, i32);
    fn acquire_occupant_manager(&self) -> bool;
    fn release_occupant_manager(&self);
    /// `buildingDevelopment.GetLotDeveloper()` (slot 36), held.
    fn acquire_lot_developer(&self) -> bool;
    fn release_lot_developer(&self);
    fn acquire_terrain(&self) -> bool;
    fn release_terrain(&self);

    // ---- tuning (§6) ----
    /// The "Demolition Thresholds" exemplar ([`THRESHOLDS_KEY`]), if it loads.
    fn thresholds_exemplar(&self) -> Option<Rc<PropertyHolder>>;
    /// The "Building Development" exemplar ([`BUILDING_DEVELOPMENT_KEY`]).
    fn building_development_exemplar(&self) -> Option<Rc<PropertyHolder>>;

    // ---- references ----
    fn release(&self, token: u64);

    // ---- the 3D view and the filters (§4.2, §4.4 step 2) ----
    /// `GetCurrentViewFilterInfo()` (view slot 18), AddRef'd; `None` when the
    /// view has no filter or there is no view.
    fn acquire_view_filter(&self) -> Option<u64>;
    /// `GetCurrentViewLevel()` (view slot 22); `None` without a view.
    fn view_level(&self) -> Option<u32>;
    /// `filter.IsOccupantIncluded(o)` (a caller or view filter).
    fn filter_includes(&self, filter: u64, occupant: u64) -> bool;
    /// `viewFilter` slot 4: includes the occupant type.
    fn filter_includes_type(&self, filter: u64, occupant_type: u32) -> bool;
    /// The shared default occupant filter's verdict.
    fn default_filter_includes(&self, occupant: u64) -> bool;
    /// The shared "realistic occupant" filter's verdict.
    fn realistic_filter_includes(&self, occupant: u64) -> bool;

    // ---- occupants ----
    fn occupant_type(&self, occupant: u64) -> u32;
    /// `GetPosition()`: (x, y, z).
    fn occupant_position(&self, occupant: u64) -> (f32, f32, f32);
    /// Occupant slot 14 `CanRemove(level)`.
    fn can_remove(&self, occupant: u64, level: u32) -> bool;
    /// Occupant slot 32 `IsFlagSet(flag)`.
    fn occupant_flag_set(&self, occupant: u64, flag: u32) -> bool;
    /// `GetFlags()`.
    fn occupant_flags(&self, occupant: u64) -> u32;
    /// `IsOccupantGroup(group)`.
    fn is_occupant_group(&self, occupant: u64, group: u32) -> bool;
    /// A bool property of the occupant: `None` when absent.
    fn occupant_bool_property(&self, occupant: u64, id: u32) -> Option<bool>;
    /// The bounding box in x and z: (min x, min z, max x, max z).
    fn occupant_bounds(&self, occupant: u64) -> (f32, f32, f32, f32);
    /// `cISC4FoundationOccupant` (`0xC8FCE101`) slot 9; `None` without the
    /// interface.
    fn foundation_slot9(&self, occupant: u64) -> Option<u32>;
    /// `cISC4NetworkOccupant` slot 9 `HasAnyNetworkFlag(mask)`; `None`
    /// without the interface.
    fn network_has_any_flag(&self, occupant: u64, mask: u32) -> Option<bool>;
    /// Network slot 22: the network has type `t`.
    fn network_has_type(&self, occupant: u64, t: u32) -> bool;
    /// Occupant slot 4 `Shutdown()`.
    fn occupant_shutdown(&self, occupant: u64);
    /// Occupant manager slot 21 `RemoveOccupant(o, b, 0)`.
    fn remove_occupant(&self, occupant: u64, b: bool) -> bool;
    /// Occupant manager slot 41 candidates whose bounding box meets the world
    /// box `(x0, z0)–(x1, z1)` (the core applies Φ, §4.4 step 4).
    fn occupants_in_box(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> Vec<u64>;
    /// Occupant manager slot 40 candidates in the cell rectangle (the core
    /// applies the immovability filter, §4.7).
    fn occupants_in_cell_rect(&self, rect: CellRect) -> Vec<u64>;

    // ---- prices and removals (§4.3) ----
    /// `buildingDevelopment.GetCostOfOccupantDemolition(b, 1)`; `None`
    /// without a building-development simulator.
    fn building_demolition_cost(&self, building: u64) -> Option<i64>;
    /// `flora.GetCostOfOccupantDemolition(o, 0)`; `None` without a flora
    /// simulator.
    fn flora_demolition_cost(&self, occupant: u64) -> Option<i64>;
    /// `QueryInterface(0xA821EF94)` (`cISC4NetworkOccupant`) succeeds.
    fn is_network_occupant(&self, occupant: u64) -> bool;
    /// `cISC4NetworkOccupant` slot 63.
    fn network_demolition_cost(&self, occupant: u64) -> i64;
    /// `QueryInterface(0xA8C86142)` succeeds.
    fn is_power_pole(&self, occupant: u64) -> bool;
    /// Pole slot 10.
    fn pole_demolition_cost(&self, pole: u64) -> i64;
    /// The connected poles (slot 4 count, slot 7 by index).
    fn pole_connections(&self, pole: u64) -> Vec<u64>;
    /// Pole slot 4: the connection count.
    fn pole_connection_count(&self, pole: u64) -> u32;
    /// Pole slot 6 `BreakAllConnectionsAtDestination(true)`.
    fn pole_break_all_connections(&self, pole: u64);
    /// `MessageSend` of `0xAA7A014D` with void1 = IGZUnknown = the occupant.
    fn send_pre_demolish(&self, occupant: u64);
    /// The city has a prop manager.
    fn has_prop_manager(&self) -> bool;
    /// `propManager.RemovePropA(o)` (slot 29).
    fn remove_prop(&self, occupant: u64) -> bool;

    // ---- caller-owned containers ----
    /// Appends to the caller's `list<cISC4Occupant*>` (the host takes a
    /// reference).
    fn list_append(&self, list: u64, occupant: u64);
    /// Inserts into the caller's `set<cRZAutoRefCount<cISC4Occupant>>`.
    fn set_insert(&self, set: u64, occupant: u64);

    // ---- networks (§4.4 steps 7-8, §4.6) ----
    /// Network manager slot 7 `QueueRemovals(b)`.
    fn queue_removals(&self, on: bool);
    /// `neighbors.DeleteOccupantConnection(o)` (slot 8).
    fn delete_occupant_connection(&self, occupant: u64);
    /// Network manager slot 6 `DestroyNetworkOccupants(list)`.
    fn destroy_network_occupants(&self, list: &[u64]);

    // ---- city geometry ----
    /// City slot 96 `CellCornerToPosition(x, z)`: (x, z) in metres.
    fn cell_corner_to_position(&self, x: i32, z: i32) -> (f32, f32);
    /// City slot 95 `PositionToCell(x, z)`; `None` outside the city.
    fn position_to_cell(&self, x: f32, z: f32) -> Option<(i32, i32)>;

    // ---- lots ----
    /// Lot manager slot 5 `GetLot(x, z)`.
    fn lot_at(&self, x: i32, z: i32) -> Option<u64>;
    /// Lot manager slot 7: the lots in the cell rectangle.
    fn lots_in_cell_rect(&self, rect: CellRect) -> Vec<u64>;
    /// Lot slot 48 `GetBuilding()`.
    fn lot_building(&self, lot: u64) -> Option<u64>;
    /// Lot slot 33 `GetState()`.
    fn lot_state(&self, lot: u64) -> u32;
    /// Lot slot 39 `IsInDemolishableState()`.
    fn lot_demolishable(&self, lot: u64) -> bool;
    /// Lot slot 49 `GetLotOccupants`.
    fn lot_occupants(&self, lot: u64) -> Vec<u64>;
    /// Lot slot 22 `GetSize` (bytes).
    fn lot_size(&self, lot: u64) -> (u8, u8);
    /// Lot slot 14 `GetBoundingRect` (inclusive).
    fn lot_bounding_rect(&self, lot: u64) -> CellRect;
    /// Lot slot 23 `GetFacing`.
    fn lot_facing(&self, lot: u64) -> u32;
    /// Lot slot 31 `GetZoneType`.
    fn lot_zone_type(&self, lot: u64) -> u32;
    /// The lot configuration (slot 9)'s `IsCompatibleWithWater()` (slot 14);
    /// `None` without a configuration.
    fn lot_water_compatible(&self, lot: u64) -> Option<bool>;
    /// Lot slot 6 `IsInitialized()`.
    fn lot_initialized(&self, lot: u64) -> bool;
    /// Lot slot 68 `GetVerticesAreLocked()`.
    fn lot_vertices_locked(&self, lot: u64) -> bool;
    /// Lot slot 35: the condition.
    fn lot_condition(&self, lot: u64) -> u32;
    /// Lot developer slot 14 `ExplodeLot(lot, b)`.
    fn explode_lot(&self, lot: u64, b: bool);
    /// Lot developer slots 12, 13.
    fn start_demolish_lot(&self, lot: u64);
    fn end_demolish_lot(&self, lot: u64);
    /// Lot manager slot 24 `DeleteLot(lot)`.
    fn delete_lot(&self, lot: u64);
    /// Lot manager slot 22 `CreateLotIfPossible(x, z, 1, 1, facing, &new, 0,
    /// 0, 0)`; the host releases the new lot.
    fn create_lot_if_possible(&self, x: i32, z: i32, facing: u32) -> bool;
    /// Zone manager slot 12: the zone of a cell.
    fn zone_at(&self, x: i32, z: i32) -> u32;
    /// Zone manager slot 11 `PlaceZone(region(rect), 0, 1, 1, 0, 1, 1, 0, 0,
    /// 0)`: dezone the rectangle.
    fn dezone(&self, rect: CellRect);

    // ---- terrain (§4.7) ----
    /// Terrain slot 14.
    fn altitude_at_vertex(&self, x: i32, z: i32) -> f32;
    /// Terrain slot 37.
    fn is_water_vertex(&self, x: i32, z: i32) -> bool;
    /// Terrain slot 49 `SetAltitude(rect, h, 0, reason)`.
    fn set_altitude_rect(&self, rect: CellRect, h: f32, reason: u32);
    /// Terrain slot 51 `SetAltitude(x, z, h)`.
    fn set_altitude_vertex(&self, x: i32, z: i32, h: f32);
    /// Terrain slot 68 `RedisplayTerrain(1, 1, rect, reason)`.
    fn redisplay_terrain(&self, rect: CellRect, reason: u32);

    // ---- effects (§4.8; presentation) ----
    /// The largest extent `s` of the occupant's bounding box, or `None` when
    /// `s` scaled to the screen is below the render-quality cutoff.
    fn effect_extent(&self, occupant: u64) -> Option<f32>;
    /// Plays the occupant's sound (u32 property `0x0A93036F`, if present).
    fn play_occupant_sound(&self, occupant: u64);
    /// Starts the first of `keys` found in effect group `0x6A3589A0` on the
    /// occupant (none found: nothing); `ray` = the clicked screen point;
    /// `grey` = tint it 50 % grey.
    fn start_occupant_effect(&self, occupant: u64, keys: &[u32], ray: Option<(i32, i32)>, grey: bool);
    /// Starts effect `0x6A3589A0` / `instance` at the cell `(x, z)`.
    fn start_group_effect(&self, instance: u32, x: i32, z: i32);
}

/// The key candidates of the occupant effect (§4.8): `k`, `k − 1`, … down to
/// the key whose low nibble is 0.
pub fn effect_key_candidates(k: u32) -> Vec<u32> {
    let low = k & 0xF;
    (0..=low).map(|i| k - i).collect()
}

/// The size class of the occupant effect (§4.8): `+2` above the
/// medium/large threshold, else `+1` above the small/medium one.
pub fn model_size_class(s: f32, t: &DemolitionTuning) -> u32 {
    if s > t.medium_large_model as f32 {
        2
    } else if s > t.small_medium_model as f32 {
        1
    } else {
        0
    }
}

/// The size of the group effect (§4.8): unsigned, strict compares (DM8;
/// `jbe` at `0x004675BB`), so a negative `n` is the large variant.
pub fn group_size_class(n: i32, t: &DemolitionTuning) -> u32 {
    let n = n as u32;
    if n > t.medium_large_group {
        2
    } else if n > t.small_medium_group {
        1
    } else {
        0
    }
}
