// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The tract developer (`cSC4TractDeveloper`, class `0x2990C142`),
//! `docs/specs/tract-developer.md`. Section references (`§n`) point at that
//! spec.
//!
//! # Structure
//!
//! - [`TractDeveloper`]: one tract developer (city component 17). It holds
//!   the style rotation of §2.1 (the only saved state, §6), the tuning of
//!   §4.21, the configuration-filter cache of §4.6 / §7.4, the debug
//!   histogram of §4.1 and the per-attempt working state of §2.2. Its
//!   methods are the interface slots of §7.1: [`TractDeveloper::grow`]
//!   (§4.3 and every step it calls, §4.4–§4.17),
//!   [`TractDeveloper::decline`] (§4.19),
//!   [`TractDeveloper::update_population`] (§4.18), the style accessors and
//!   `NewYear` (§4.20), `Init` / `Shutdown` (§1.3, §1.4), `DoMessage`
//!   (§3) and the save record (§6).
//! - [`TractDeveloperStatics`]: the class-wide statics shared by every tract
//!   developer (§9.1): the file-local `cRZRandom` (§4.22), the C `rand()`
//!   state ([`CrtRand`], foundations §12.6) and the two class-wide random
//!   cell iterators of §4.5 and §4.19. Hand the same
//!   [`SharedStatics`] to every instance.
//! - [`TractDeveloperHost`]: everything outside the tract developer — the
//!   growth developer (§8 `cISC4GrowthDeveloper`), the BDS (catalogue,
//!   construction resources, cheats, `GetDeveloper`), the lot developer, the
//!   lot manager, lots, lot configurations and their manager, the zone
//!   manager, land value, power, plumbing, fire, aura, the traffic network,
//!   demolition, terrain, the simulator and the message server. Objects are
//!   host tokens (`u64`). The other two M4 cores are reached through it.
//! - The STLport `std::sort` (§4.14) and `hash_set<uint32>` (§4.16) are
//!   the shared [`crate::stl::stl_sort`] and [`crate::stl::StlHashSet`].
//! - `tuning.rs`: [`Tuning`] (§4.21).
//!
//! # Choices (`[choice]`), limits (`[compat-limit]`) and inferences (`[inferred]`)
//!
//! - **Pointer-ordered sets** *[compat-limit]* (the lot set, the
//!   configuration set and `PickBuilding`'s `S_cfg`, §2.2, §4.16) are
//!   ordered by the host token (`u64`). The original orders by object
//!   address, which a standalone engine cannot reproduce (§9.5 item 1): the
//!   configuration pick (draw B) only matches the specified distribution
//!   (uniform over the compatible configurations), not the original's exact
//!   choice for a given seed. A plugin running in the game process may hand
//!   out the object addresses as tokens to get the original order.
//! - The **file-local generator** is one [`RzRandom`] shared by every tract
//!   developer, not saved. [`TractDeveloperStatics::from_session`] seeds it
//!   from the session seed per foundations §6.3 *[choice]*
//!   (`derive_seed(S, RNG_SEED_XOR)`: the original's local wall clock XOR
//!   [`RNG_SEED_XOR`], §4.22, with `S` in place of the clock);
//!   [`TractDeveloperStatics::new`] takes an exact seed. The **C
//!   `rand()`** used by the iterator resets R1 / R8 is the statics' own
//!   [`CrtRand`] (seed 1 by default, foundations §12.6).
//! - **UpdatePopulation's seed** (§4.18 [choice]): tract (0, 0), whose seed
//!   is 0, uses the fixed substitute `0x00000001`. Any other seed (including
//!   one that wraps to 0) is used as computed; the temporary generator's
//!   clock reseed then reads a fixed clock value of 1.
//! - **R9** (§4.15.3, §4.22): `CreateLotIfPossible` with facing −1 draws on
//!   the zone developer's generator inside the lot manager, i.e. inside the
//!   host call. The core does not record it; a host may record it in the
//!   shared trace ([`TractDeveloperStatics::record`], [`DrawKind::R9`]).
//! - **Histogram** (§4.1): kept behind a debug switch
//!   ([`TractDeveloper::set_histogram_enabled`], off by default). The debug
//!   lists of the original are not kept.
//! - **Reference counts** are the plugin's: the core never AddRefs or
//!   releases. Every token the host hands out stays valid for the duration
//!   of the call that uses it; the core keeps no host list across calls
//!   (the lot, configuration and candidate sets are rebuilt each pass, §2.2).
//!   Lists the original receives in caller-made containers come back as
//!   owned `Vec`s; where the original appends to one list across calls
//!   (`BuildFarm`'s neighbour list, §4.15.2) the core appends.
//! - **Configuration filters** (§4.6, §7.4): the host allocates and
//!   registers a filter for `(wealth, purpose)`
//!   ([`TractDeveloperHost::register_configuration_filter`]) and frees it at
//!   `Shutdown` ([`TractDeveloperHost::unregister_configuration_filter`]);
//!   the filter's `IsLotConfigurationIncluded` is
//!   [`configuration_filter_includes`].
//! - **Zone conversion** in `TakeOverAbandoned` (§4.4 "the lot's zone must
//!   convert to the developer's target zone type") *[inferred]*: zone types
//!   1–3 convert to 1 (residential), 4–6 to 2 (commercial), 7–9 to 3
//!   (industrial), others to none ([`zone_family`]).
//! - **Type-1 candidates** take their anchor `(x, z)` from the lot's
//!   location when made, so the cap checks of §4.4 read the lot's cell
//!   *[inferred]*; their facing stays −1 (§2.3).
//! - **Missing building developer** in `TakeOverAbandoned` (BDS
//!   `GetDeveloper` returns null): the power and water cap checks pass (the
//!   original would dereference null).
//! - **Missing source demand** reads as 0, so `desired` is 1 (the original
//!   would dereference null).
//! - **Facings** outside 0–3 are taken modulo 4 in the geometry of §4.10.1,
//!   §4.10.3 and §4.15.4.
//! - **`IsAggregationLegal`** visits its rectangle row by row (`z` outer,
//!   `x` inner) *[inferred]*; the order only affects which failure is
//!   counted first.
//! - **Slope checks**: the existing-lot path (§4.9.1) fails unless
//!   `min ≤ slope ≤ max` (any NaN fails); aggregation, subdivision and
//!   `IsLotConfigurationCompatible` fail only on `slope < min` or
//!   `slope > max` (NaN passes).
//! - **`PickBuilding`'s anchor rectangle** is the candidate's cached
//!   rectangle as it stands (§4.16 step 2 passes `&c.rect`); for a
//!   subdivision it is `(−1, −1, −1, −1)` because `FindNetworks` uses the
//!   lot's own rectangle (§4.10.2) *[inferred]*. The anchor rule itself is
//!   the lot developer's (`lot-developer.md` §5.7, `NeedsAnchorBuilding`).
//! - **`AbandonBuilding`** is a synchronous `SetState(4, 0, reason)`
//!   (`lot-developer.md` §4.3), so `Decline` counts a multi-cell lot once
//!   (§4.19); the core relies on the host doing so.
//! - **`BuildFarm`**: an absent field-ID property, or an empty one (the
//!   original reads element 0, quirk 14), gives field type 0. The neighbour
//!   list stops growing at [`MAX_FARM_NEIGHBOURS`] entries.
//! - **Subdivision** with `n = 0` (not reachable from §4.9.3) leaves
//!   `lo` undivided.
//! - **Out-of-range style index** (`currentStyleIndex ≥ |activeStyles|`,
//!   reachable only between `Read` and `Init`) reads style 0;
//!   `SetActiveStyles` on an empty list treats the current style as none
//!   (§4.20 [choice]).
//! - **Save read** (§6): a failed count read uses 0 (§6 [choice]); the
//!   style list stops at the first failed element and is then zero-padded
//!   to the count, at most [`MAX_SAVED_STYLES`] entries.
//! - **Tuning pairs** shorter than two floats keep the default for the
//!   missing elements (§4.21 quirk 9).
//! - **`std::sort`** and the "unguarded" loops never index outside the
//!   candidate list ([`crate::stl::stl_sort`]).
//! - **Return values** (§7.1 "what a replacement returns"): `grow` returns
//!   `built`, `decline` the sum, `update_population` nothing (the plugin
//!   returns 0); slots 10, 12 and 14 are the plugin's (nonzero, `this`,
//!   `this`).
//!
//! Host callbacks must not call back into the tract developer.

mod simulator;
pub mod archive;
#[cfg(test)]
mod tests;
mod tuning;

pub use simulator::{
    candidate_rect, capacity_window, configuration_filter_includes, corner_ok, desired_of, has_required_roads,
    roads_check, score_candidate, sort_candidates, style_group, style_points, zone_family, Candidate, Draw, DrawKind,
    SharedStatics, TractDeveloper, TractDeveloperStatics, HIST_COLUMNS, HIST_ROWS, MAX_FARM_NEIGHBOURS,
    MAX_SAVED_STYLES, RECORD_VERSION,
};
pub use tuning::{prop, Tuning, TUNING_KEY};

use std::rc::Rc;

pub use crate::development::{BuildingProfile, ConstructionProfile, GrowthProfile, Outcome};
pub use crate::grid::CellRect;
use crate::property::PropertyHolder;
pub use crate::rng::{CrtRand, RzRandom};

/// Class ID (`GetGZCLSID`, §1.1).
pub const CLSID_TRACT_DEVELOPER: u32 = 0x2990_C142;
/// Interface IDs answered by `QueryInterface` (§1.1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_TRACT_DEVELOPER: u32 = 0xA818_ED1B;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;

/// Whether `QueryInterface` answers `iid` (§1.1; unknown IIDs → false).
pub fn answers_iid(iid: u32) -> bool {
    matches!(
        iid,
        IID_UNKNOWN | IID_TRACT_DEVELOPER | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE
    )
}

/// Agent registration (§1.3): name, flags, then levels with their
/// messages.
pub const AGENT_NAME: &str = "cSC4TractDeveloper";
pub const AGENT_FLAGS: u32 = 2;
pub const AGENT_LEVELS: [(u32, u32); 2] = [(6, MSG_MONTH), (7, MSG_YEAR)];
/// Messages handled (§3).
pub const MSG_MONTH: u32 = 0x6695_6816;
pub const MSG_YEAR: u32 = 0x6695_6817;

/// The XOR constant mixed into the file-local generator's seed (§4.22).
pub const RNG_SEED_XOR: u32 = crate::rng::file_seed::TRACT_DEVELOPER;

/// Bool property of a building exemplar: monopoly piece (§4.17).
pub const PROP_MONOPOLY: u32 = 0x2781_283F;
/// uint32 array property of a farm's exemplar: the field types (§4.15.2).
pub const PROP_FIELD_IDS: u32 = 0x29B5_5F73;
/// The anchor occupant group (§4.16 step 2).
pub const ANCHOR_GROUP: u32 = 0x3000;
/// The `FindNetworkTile` mask and flag (§4.10.2).
pub const NETWORK_TILE_MASK: u32 = 0xC4B;
/// Demolition `DemolishLots` flags (§4.15.1): `(true, [lot], 0, 0x800,
/// false, 0, 0, 0)`.
pub const DEMOLISH_FLAGS: u32 = 0x800;
/// Lot states (§4.8, §4.19).
pub mod lot_state {
    pub const CONSTRUCTING: u32 = 2;
    pub const OCCUPIED: u32 = 3;
    pub const ABANDONED: u32 = 4;
}
/// `AbandonBuilding` reasons: kick-out (§4.4) and decline (§4.19).
pub const ABANDON_KICK_OUT: u32 = 5;
pub const ABANDON_DECLINE: u32 = 6;
/// Network types (§4.10.2).
pub mod network {
    pub const ROAD: u32 = 0;
    pub const RAIL: u32 = 1;
    pub const STREET: u32 = 3;
    pub const AVENUE: u32 = 6;
    pub const ONE_WAY_ROAD: u32 = 10;
    pub const DIRT_ROAD: u32 = 11;
}

/// The four developer caps (growth developer slots 19–22, §4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    Power,
    Water,
    Fire,
    Park,
}

/// The BDS cheats read (slots 54–57, §4.2, §4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cheat {
    IgnorePlumbing,
    IgnorePower,
    IgnoreCaps,
    DebugGrowth,
}

/// Everything the tract developer reads from, or does to, the rest of the
/// game (§8). Objects are host tokens. The contracts are those the spec
/// states; the owning specs are named per group.
pub trait TractDeveloperHost {
    // ---- lifecycle (§1.3, §1.4) ----
    /// `Init` step 3: take the city services (city, BDS, land value, lot
    /// developer, lot configuration manager, lot manager, plumbing, power,
    /// traffic, zone manager). False if any is missing.
    fn acquire_services(&self) -> bool;
    /// Simulator slot 19 `AddAgent(target, level, "cSC4TractDeveloper", 2)`
    /// for `message`. False when there is no simulator.
    fn add_agent(&self, level: u32, message: u32) -> bool;
    /// `Init` step 5: take the message server. False when missing.
    fn acquire_message_server(&self) -> bool;
    fn release_message_server(&self);
    /// Simulator slot 20 `RemoveAgent`.
    fn remove_agent(&self);
    /// `Shutdown` steps 3–5: release the services taken at `Init`.
    fn release_services(&self);
    /// The "Building Development" tuning exemplar (§4.21), if loaded.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;

    // ---- growth developer (growth-developer.md) ----
    /// Slot 5 `GetType`: the target zone type (1 R, 2 C, 3 I).
    fn dev_target_zone(&self, dev: u64) -> i32;
    /// Slot 6 `GetSubType`: the developer type.
    fn dev_subtype(&self, dev: u64) -> u32;
    fn dev_purpose(&self, dev: u64) -> u32;
    fn dev_wealth(&self, dev: u64) -> u32;
    /// Slot 9 `GetConstructionInfo` (growth-developer §6.2).
    fn dev_construction_info(&self, dev: u64) -> ConstructionProfile;
    /// The desirability map's (slot 13) tract size and shift.
    fn dev_desirability_tract_size(&self, dev: u64) -> i32;
    fn dev_desirability_shift(&self, dev: u64) -> u32;
    /// The desirability map's tract value (`int16`).
    fn dev_desirability_value(&self, dev: u64, tx: i32, tz: i32) -> i16;
    /// The capacity map's (slot 14) tract value; the low 16 bits are used.
    fn dev_capacity_value(&self, dev: u64, tx: i32, tz: i32) -> u32;
    /// `GetSourceDemand()` (slot 18) then `cISC4Demand` slot 12; `None`
    /// without a demand node.
    fn dev_source_demand(&self, dev: u64) -> Option<f32>;
    /// Slots 19–22: the power / water / fire / park caps (`uint8`).
    fn dev_cap(&self, dev: u64, cap: Cap) -> u8;
    /// Slot 25 `GetMonopolyUse`.
    fn dev_monopoly_use(&self, dev: u64) -> i32;
    /// Slot 26 `GetStylePercentages`: `(occupant group, fraction)` pairs;
    /// the core walks them in ascending key order.
    fn dev_style_percentages(&self, dev: u64) -> Vec<(u32, f32)>;

    // ---- BDS (building-development.md) ----
    /// Slot 39 `GetBuildingProfile` (an unknown type reads as zeros).
    fn building_profile(&self, building: u32) -> BuildingProfile;
    /// Slot 19 `GetBuildingTypesByFamily`: the types in the order the BDS
    /// inserts them into the caller's `hash_set` (the core models the set).
    fn building_types_by_family(&self, family: u32) -> Vec<u32>;
    /// `IsOccupantGroup(GetBuildingKeyFromType(b), group)` (slot 14 and
    /// `0x005E8F70`).
    fn building_in_occupant_group(&self, building: u32, group: u32) -> bool;
    /// Bool property [`PROP_MONOPOLY`] of the building's exemplar (absent →
    /// false).
    fn building_is_monopoly(&self, building: u32) -> bool;
    /// uint32 array property [`PROP_FIELD_IDS`] of the building's exemplar.
    fn building_field_ids(&self, building: u32) -> Option<Vec<u32>>;
    /// Slot 33 `GetDeveloper(purpose, wealth)` (no AddRef).
    fn developer_for(&self, purpose: u32, wealth: u32) -> Option<u64>;
    /// Slots 50 / 51.
    fn claim_construction_resources(&self, amount: u16) -> bool;
    fn release_construction_resources(&self, amount: u16);
    /// Slots 54–57.
    fn cheat(&self, cheat: Cheat) -> bool;

    // ---- lot developer (lot-developer.md) ----
    /// `BDS.GetLotDeveloper()` (slot 36) then lot developer slot 36
    /// `NeedsAnchorBuilding(&rect, purpose)`.
    fn needs_anchor_building(&self, rect: CellRect, purpose: u32) -> bool;
    /// Slot 6 `StartConstruction(lot, cfg, buildingType, flag)`.
    fn start_construction(&self, lot: u64, cfg: u64, building: u32, flag: bool) -> bool;
    /// Slot 8 `AbandonBuilding(lot, reason)`.
    fn abandon_building(&self, lot: u64, reason: u32) -> bool;
    /// Slot 9 `ReoccupyBuilding(lot, devType)`.
    fn reoccupy_building(&self, lot: u64, dev_type: u32) -> bool;
    /// Slot 29 `GetAreaSlope(&rect, 0)`.
    fn area_slope(&self, rect: CellRect) -> f32;

    // ---- lot manager (zones.md) ----
    /// Slot 6 `GetLot(x, z, false)`.
    fn get_lot(&self, x: i32, z: i32) -> Option<u64>;
    /// Slot 7 `GetLotsInCellRect(list, x1, z1, x2, z2, true)`, list order,
    /// handed to `f` lot by lot (devperf2: no list is built for
    /// `UpdatePopulation`, which asks once per desirability tract).
    fn lots_in_cell_rect(&self, rect: CellRect, f: &mut dyn FnMut(u64));
    /// Slot 9 `GetLotsSurroundingLot(lot, list, flags)`: the lots it
    /// appends.
    fn lots_surrounding_lot(&self, lot: u64, flags: u32) -> Vec<u64>;
    /// Slot 10 `GetLotsSurroundingRect(rect, list, flags)`: the lots it
    /// appends.
    fn lots_surrounding_rect(&self, rect: CellRect, flags: u32) -> Vec<u64>;
    /// Slot 20 `CanCreateLot(x, z, w, h, 0, 0, 0)` → code.
    fn can_create_lot(&self, x: i32, z: i32, w: i32, h: i32) -> i32;
    /// Slot 22 `CreateLotIfPossible(x, z, w, h, facing, &lot, 0, 0, 0)`.
    fn create_lot_if_possible(&self, x: i32, z: i32, w: i32, h: i32, facing: i32) -> Option<u64>;
    /// Slot 24 `DeleteLot(lot)`.
    fn delete_lot(&self, lot: u64) -> bool;

    // ---- lots (cISC4Lot) ----
    fn lot_bounding_rect(&self, lot: u64) -> CellRect;
    fn lot_slope(&self, lot: u64) -> f32;
    fn lot_location(&self, lot: u64) -> (i32, i32);
    fn lot_size(&self, lot: u64) -> (u32, u32);
    fn lot_facing(&self, lot: u64) -> i32;
    fn lot_set_facing(&self, lot: u64, facing: i32);
    fn lot_has_road_access(&self, lot: u64) -> bool;
    fn lot_has_organic_road_access(&self, lot: u64, purpose: u32) -> bool;
    fn lot_zone_type(&self, lot: u64) -> i32;
    fn lot_state(&self, lot: u64) -> u32;
    fn lot_set_state(&self, lot: u64, state: u32, wealth: u32, reason: u32);
    fn lot_is_livable(&self, lot: u64) -> bool;
    fn lot_has_capacity(&self, lot: u64) -> bool;
    fn lot_days_since_state_change(&self, lot: u64) -> u32;
    fn lot_occupant_wealth(&self, lot: u64) -> u32;
    fn lot_is_historical(&self, lot: u64) -> bool;
    /// Slot 46 `GetBuildingType(false)`.
    fn lot_building_type(&self, lot: u64) -> u32;
    /// Slot 50 `GetCapacity(DeveloperType, flag)`.
    fn lot_capacity(&self, lot: u64, dev_type: u32, flag: bool) -> u16;
    /// Slot 52 `GetAvailableCapacity(DeveloperType, flag)`.
    fn lot_available_capacity(&self, lot: u64, dev_type: u32, flag: bool) -> u16;
    /// Slot 55 `SetPopulation(DeveloperType, n)`.
    fn lot_set_population(&self, lot: u64, dev_type: u32, population: u16);
    /// The growth stage of the lot's configuration (`None`: no
    /// configuration).
    fn lot_configuration_stage(&self, lot: u64) -> Option<u8>;

    // ---- lot configurations (cISC4LotConfiguration) ----
    fn cfg_size(&self, cfg: u64) -> (u32, u32);
    fn cfg_min_slope(&self, cfg: u64) -> f32;
    fn cfg_max_slope(&self, cfg: u64) -> f32;
    fn cfg_growth_stage(&self, cfg: u64) -> u8;
    fn cfg_min_capacity(&self, cfg: u64) -> u16;
    fn cfg_max_capacity(&self, cfg: u64) -> u16;
    fn cfg_compatible_zone(&self, cfg: u64, zone: i32) -> bool;
    fn cfg_compatible_wealth(&self, cfg: u64, wealth: u32) -> bool;
    fn cfg_compatible_purpose(&self, cfg: u64, purpose: u32) -> bool;
    fn cfg_compatible_style(&self, cfg: u64, style: u32) -> bool;
    /// Slot 21 `GetRequiredRoads`: up to four sides, configuration frame.
    fn cfg_required_roads(&self, cfg: u64) -> Vec<bool>;
    /// Slot 21 into `out` (at most `out.len()` bits); the number written. A
    /// host may answer without allocating.
    fn cfg_required_roads_into(&self, cfg: u64, out: &mut [bool]) -> usize {
        let v = self.cfg_required_roads(cfg);
        let n = v.len().min(out.len());
        out[..n].copy_from_slice(&v[..n]);
        n
    }
    /// Slot 24 `GetPrimaryBuildingConfigurationObject`: its object-ID
    /// list, `None` when there is no object.
    fn cfg_primary_building_ids(&self, cfg: u64) -> Option<Vec<u32>>;
    /// Whether slot 24 gives an object (§4.7 check 5), without its list (a
    /// host may answer without allocating).
    fn cfg_has_primary_building(&self, cfg: u64) -> bool {
        self.cfg_primary_building_ids(cfg).is_some()
    }

    // ---- lot configuration manager ----
    /// Slot 8 `GetLotConfiguration(id)`.
    fn get_lot_configuration(&self, id: u32) -> Option<u64>;
    /// Allocate a filter `{wealth, purpose}` and register it (slot 18).
    fn register_configuration_filter(&self, wealth: u32, purpose: u32) -> u64;
    /// Unregister the filter (slot 19) and free it.
    fn unregister_configuration_filter(&self, filter: u64);
    /// Slot 9 `GetLotConfigurationsByFilter`: the configurations included.
    fn configurations_by_filter(&self, filter: u64) -> Vec<u64>;

    // ---- city grids and services ----
    /// Zone manager slot 12: zone type at `(x, z)`.
    fn zone_at(&self, x: i32, z: i32) -> i32;
    /// Land value slot 6.
    fn land_value(&self, x: i32, z: i32) -> u8;
    /// Power slot 35 `GetPoweredGrid()` → `GetCellValue(x, z)`.
    fn powered_value(&self, x: i32, z: i32) -> i32;
    /// Plumbing slot 11 `IsCellWatered`.
    fn is_cell_watered(&self, x: i32, z: i32) -> bool;
    /// Fire protection slot 5 grid (`uint8`).
    fn fire_coverage(&self, x: i32, z: i32) -> u8;
    /// Aura slot 14 `GetParkMap()` (`int16`).
    fn park_value(&self, x: i32, z: i32) -> i16;
    /// The traffic simulator's road-access value at `(x, z)` (§4.8 row 7
    /// *[inferred]*).
    fn road_access_value(&self, x: i32, z: i32) -> i32;
    /// City slots 93 / 94.
    fn city_cell_counts(&self) -> (i32, i32);
    /// Traffic network map slot 8 `FindNetworkTile(x, z, 0xC4B, 1)`.
    fn find_network_tile(&self, x: i32, z: i32) -> Option<u64>;
    /// Network occupant slot 22 `IsOfType(type)`.
    fn network_is_of_type(&self, tile: u64, network_type: u32) -> bool;
    /// Demolition slot 7 `DemolishLots(true, [lot], 0, 0x800, false, 0, 0, 0)`.
    fn demolish_lot(&self, lot: u64) -> bool;
    /// Terrain slot 68 `RedisplayTerrain(0, 0, &rect, 0)`.
    fn redisplay_terrain(&self, rect: CellRect);
}
