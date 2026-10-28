// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The growth developer (`cSC4GrowthDeveloper`, class `0x0990C138`),
//! `docs/specs/growth-developer.md`. Section references (`§n`) point at that
//! spec.
//!
//! # Structure
//!
//! - [`GrowthDeveloper`]: one developer object (one per developer exemplar):
//!   creation, `Init`, `Shutdown` and references (§1), the state of §2, the
//!   `DoIdle` state machine and messages (§3), the desirability layers,
//!   special buildings and `EstimateDesirability` (§4), the growth and
//!   decline lists, population pass, phantom demand, sort and stages (§5),
//!   the capacity bookkeeping (§6), the save record (§9) and the interface
//!   slots of §10 as plain methods.
//! - [`DeveloperTuning`] (§4.8): the exemplar read, the 19 curves (§4.2,
//!   "delete" pattern) and stage curves; [`shipped_exemplar`] gives the
//!   twelve shipped exemplars of §4.9, every property as in `SimCity_1.dat`.
//! - Pure arithmetic in `algo.rs` ([`population_of`], [`add_curve`],
//!   [`commute_add`], [`phantom_term`], [`assign_stages`], [`city_size`],
//!   [`decline_key`], the capacity helpers); the game's `std::sort` is the shared
//!   [`crate::stl::stl_sort`].
//! - [`GrowthStatics`]: the process-wide state shared by every growth
//!   developer: the file-local `cRZRandom` of §7 and the time-slice step of
//!   §5.2. The host creates one and hands the same `Rc` to every developer.
//!   [`GrowthStatics::from_session`] seeds it from the session seed per
//!   foundations §6.3 *[choice]* (`derive_seed(S, 0xAC09C0E8)`: the
//!   original's `cRZTime() XOR 0xAC09C0E8` with `S` in place of the clock);
//!   [`GrowthStatics::new`] and [`GrowthStatics::reseed`] take an exact seed
//!   (tests and replays).
//! - [`GrowthDeveloperHost`]: everything the developer reads from, or does
//!   to, the rest of the game: the services of `Init`, the message server
//!   and the simulator, the source maps (§4.3), the BDS (`GetMaxCapacity`,
//!   `GetBuildingProfile`, `GetSpecialBuildings`,
//!   `GetStagePopulationMultiplier`, `GetTractDeveloper`), the tract
//!   developer (`UpdatePopulation`), the zone manager, lots, occupants, the
//!   demand nodes and the neighbours. Its supertraits
//!   [`GridProvider<u8>`], [`GridProvider<i16>`] and [`GridProvider<u16>`]
//!   create the six grids and resolve grid references read from a save (in
//!   game: the game's own `cSC4SimGrid` objects, simgrid §6.4). The other
//!   M4 cores (BDS, tract developer) are reached only through this trait.
//!
//! Other subsystems read the grids through [`GrowthDeveloper::desirability_map`]
//! (slot 13, D), [`GrowthDeveloper::capacity_map`] (14),
//! [`GrowthDeveloper::population_map`] (15) and
//! [`GrowthDeveloper::max_capacity_map`] (16), and the lists through
//! [`GrowthDeveloper::growth_list`] / [`GrowthDeveloper::decline_list`]
//! (read in place, never modified by callers *[inferred]*).
//!
//! Host handles are opaque tokens (`u64`). Host callbacks must not call
//! back into the developer; no `RefCell` borrow is held across a callback.
//!
//! # Replacement unit (§12, `building-development.md` §1.5)
//!
//! The developer is glued and replaced together with the BDS's developer
//! creation. Each developer operation of the BDS core's host trait
//! (`fc4_core::development_simulator::DevelopmentHost`) maps to one method:
//!
//! | BDS host method | `GrowthDeveloper` |
//! |---|---|
//! | `create_developer` | [`GrowthDeveloper::create`] (constructor + `AddRef`, count 1) |
//! | `create_developer` + `developer_init` | [`GrowthDeveloper::create_from_exemplar`] |
//! | `developer_init(dev, exemplar)` | [`GrowthDeveloper::init`]`(Some(exemplar))` |
//! | `developer_shutdown` / `developer_release` | [`GrowthDeveloper::shutdown`] / [`GrowthDeveloper::release`] |
//! | `developer_sub_type` / `developer_purpose` | [`GrowthDeveloper::sub_type`] / [`GrowthDeveloper::purpose`] |
//! | `developer_lists_valid` | [`GrowthDeveloper::lists_valid`] |
//! | `developer_growth_list` / `developer_decline_list` | [`GrowthDeveloper::growth_items`] / [`GrowthDeveloper::decline_items`] (owned copies; [`GrowthDeveloper::growth_list`] / [`GrowthDeveloper::decline_list`] borrow in place) |
//! | `developer_active_demand` | [`GrowthDeveloper::source_active_demand`] (`GetSourceDemand` is [`GrowthDeveloper::source_demand`]) |
//! | `developer_start_map_update` / `developer_map_update_finished` | [`GrowthDeveloper::start_map_update`] / [`GrowthDeveloper::is_map_update_finished`] |
//! | `developer_ref` / `resolve_developer` | [`GrowthDeveloper::save_ref`] / [`GrowthDeveloper::from_save_ref`] |
//!
//! A developer read from a save is made by the class store's factory for
//! [`CLSID_GROWTH_DEVELOPER`] with [`GrowthDeveloper::new`] (count 0; the
//! reference the stream hands over is the BDS's).
//!
//! # Contracts with the zone manager and lots (`zones.md`, `lot-developer.md`)
//!
//! - `GetZoneType(x, z)` (zone manager slot 12, `0x004DC740`) returns the
//!   cell's `int8` sign-extended to all of EAX (zones.md §2, §11.1). The host
//!   returns the `int8` ([`GrowthDeveloperHost::zone_type`]); `UpdateMaxCapacity`
//!   sign-extends it before `BuildingFitsInZone` and `GetMaxCapacity`.
//! - ZoningChange `0x07699455` is posted (queued) by `PlaceZone`; its void2 is
//!   an `SC4CellRegion<long>` whose first 16 bytes are the request rectangle
//!   `{x1, z1, x2, z2}` (inclusive, zones.md §5.3, §11.4). The host passes
//!   those four words as [`GrowthMessage::rect`]; the region is the
//!   message's and is neither kept nor freed.
//! - StateChanged `0xE8BDA2E7` is posted (queued) by `cSC4Lot::SetState`;
//!   lot states 2 (constructing) and 3 (occupied) count as occupied, 4 is
//!   vacant (abandoned) (lot-developer.md notation, §3.2, §6.4). Lot slots
//!   14, 21, 22, 46, 50 as in §11.
//! - The lot developer reads slots 7 (`GetPurpose`), 13
//!   (`GetDesirabilityMap`, then the grid's `GetCellValue`) and 20
//!   (`GetWaterSupplyCap`, unsigned byte); it never calls slot 24
//!   (`GetAbandonmentThreshold`), whose caller is still open (§13.3 Q4).
//!
//! # Compatibility limit (`[compat-limit]`)
//!
//! Which industrial developer gets which salt (§7) depends in the original on
//! the address order of the agents. FreeCity4 matches the specified
//! distribution — every industrial (purpose 5..8) update draws exactly one
//! `NextRaw()` from the one shared file-local generator in state 20, before
//! its decline sort; R, C and other purposes draw nothing — but a seeded run
//! need not reproduce the assignment of salts to developers. The vectors pin
//! the draw count per update, the shared generator's state, the key
//! derivation and the sort; with several industrial developers they check
//! only that the salts handed out are the drawn ones (`gdev_expect_salts`).
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - `Init(null)` returns false (§1.2 step 3). An exemplar's property `0x21`
//!   is read in step 3 and again by the tuning read.
//! - Services (§1.2 step 2) are host calls [`GrowthDeveloperHost::acquire_service`]
//!   in the original's order; the developer remembers which it holds, so a
//!   later `Init` resumes. `Shutdown` releases the held ones in the order of
//!   §1.3 and nulls the six grids after `Shutdown` + `Release` (a re-`Init`
//!   creates new ones) *[inferred]*.
//! - The iterator rectangle is set after every tuning read from W's counts,
//!   also when the tract size is missing or unchanged (§4.8, §13.1 quirk 10).
//! - Curves: an empty curve (a present property with fewer than two values)
//!   counts as absent (§4.2). A stage curve whose property is present but
//!   empty is the constant (0, 0) like a missing one.
//! - Out-of-bounds reads of the original (`0x4A5F7315` triples, `0x87CD6380`
//!   with fewer than 3 values, an odd `0x87CD63A0`, `0x87CD6333` with fewer
//!   than 2 bytes) treat the missing values as absent (§13.1 quirk 9): the
//!   occupant is skipped, the threshold keeps 0.2, the last group is dropped,
//!   the range is kept.
//! - "Raw" tuning reads (`0x87CD6333`, `0x87CD6380`) take the property's
//!   elements as little-endian bytes in memory order (an `f32` array gives its
//!   floats, other types are reinterpreted).
//! - A source map the host cannot supply (null) skips its layer (and its
//!   `EstimateDesirability` term); the original would dereference null. A
//!   missing tract developer skips the `UpdatePopulation` calls of state 18
//!   (the population map is still written); a missing source demand node
//!   skips `AddToDemandValue` (the phantom sum is still stored); a demand node
//!   missing in §5.5 supplies 0.
//! - `UpdatePopulation` gets stage 0 (§5.2, quirk 12).
//! - Float source walks (§4.3): the W-finer conversion `float(ftol(v))`
//!   keeps the low 32 bits of `_ftol` (NaN → 0); the W-coarser accumulator
//!   visits the covered source tracts x outer, z inner, like
//!   [`crate::grid::process_two_grids`] (used for the integer sources).
//! - The special-building copy (§4.4) `AddRef`s the new elements before it
//!   releases the old ones; `Shutdown` keeps the list (§1.3), the final
//!   `Release` releases it.
//! - `UpdateMaxCapacity` (§6.1) starts from a zeroed profile (the zone code
//!   of an industrial developer with a purpose outside 5..8 is 0 instead of
//!   stack garbage, quirk 15) and clamps the lower tract bounds to 0 (the
//!   original writes outside the grids; FreeCity4's grids ignore such writes,
//!   so only the zone-type queries of cells outside the city are dropped).
//! - `UpdatePopulationCount` (§6.3) skips a lot of area 0 entirely (quirk 7)
//!   and visits only the cells of the lot rectangle that lie inside the
//!   capacity grid (writes outside are ignored anyway).
//! - A StateChanged message whose `IGZUnknown` is not a lot (no token) is
//!   ignored.
//! - The city size of §5.5 is computed (with its host queries) on every
//!   state 20, also when there are no stage curves or no growth items; the R
//!   and C cases query both C nodes (`0x3001`, `0x3301`) every time.
//! - `EstimateDesirability` returns the `int16` result sign-extended (§4.6).
//! - The time-slice step and miss counter are process-wide like the
//!   original's static (kept in [`GrowthStatics`]).
//! - The salt drawn in state 20 is kept per developer ([`GrowthData::last_salt`])
//!   and the shared generator counts its draws, for conformance checks.
//! - [`GrowthDeveloper::create`] and [`GrowthDeveloper::create_from_exemplar`]
//!   take the BDS's reference (count 1), like `CreateDeveloper`'s
//!   constructor + `AddRef`; [`GrowthDeveloper::source_active_demand`] is
//!   `None` without a source node (the BDS core counts that as 0).
//! - `Release` from a count ≤ 1 calls `Shutdown`, releases the
//!   special-building copy, returns 0 and marks the object destroyed (the
//!   plugin then drops it).
//! - The decline-list sort (§5.4 step 3) uses the same `std::sort` as the
//!   growth list *[inferred]*; keys are recomputed in every comparison, as
//!   the original's comparator does.

mod algo;
pub mod archive;
mod shipped;
mod simulator;
#[cfg(test)]
mod tests;
mod tuning;

pub use algo::{
    add_curve, add_to_capacity, assign_stages, city_size, commute_add, commute_minimum, decline_key, decline_key_seed,
    desirability_fraction, low_byte, phantom_term, population_of, round_capacity, split_capacity, CitySizeInputs,
};
pub use simulator::{GrowthData, GrowthDeveloper, RECORD_VERSION};
pub use tuning::{shipped_exemplar, shipped_types, style_fraction, DeveloperTuning, ShippedValue, TuningRead};

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::development::GrowthProfile;
use crate::grid::{GridProvider, TractGrid};
use crate::rng::{RzRandom, SeedSource};
use crate::variant::Variant;

/// Class ID (§1).
pub const CLSID_GROWTH_DEVELOPER: u32 = 0x0990_C138;
/// Interface IDs (§1).
pub const IID_UNKNOWN: u32 = 1;
pub const IID_GROWTH_DEVELOPER: u32 = 0xE7CD_D662;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;

/// Agent registration (§1.2 step 9): level 10 (idle), then 4 (day), flags 2.
pub const AGENT_NAME: &str = "cSC4GrowthDeveloper";
pub const AGENT_LEVELS: [u32; 2] = [10, 4];
pub const AGENT_FLAGS: u32 = 2;

/// The developer exemplar (§1, §4.8).
pub const TUNING_TYPE: u32 = 0x6534_284A;
pub const TUNING_GROUP: u32 = 0x67CD_5FA1;

/// Messages (§3.1).
pub mod msg {
    pub const SIM_IDLE: u32 = 0x6695_681A;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const ZONING_CHANGE: u32 = 0x0769_9455;
    pub const STATE_CHANGED: u32 = 0xE8BD_A2E7;
    /// `Init`'s two subscriptions, in order (§1.2 step 8).
    pub const SUBSCRIPTIONS: [u32; 2] = [STATE_CHANGED, ZONING_CHANGE];
}

/// Tuning and occupant properties (§4.4, §4.8).
pub mod prop {
    pub const DEVELOPER_TYPE: u32 = 0x0000_0021;
    pub const TARGET_ZONE: u32 = 0x87CD_6300;
    pub const SOURCE_DEMAND: u32 = 0x87CD_6301;
    pub const TRACT_SIZE: u32 = 0x87CD_6310;
    pub const WEALTH: u32 = 0x87CD_6330;
    pub const UNREAD_6331: u32 = 0x87CD_6331;
    pub const PURPOSE: u32 = 0x87CD_6332;
    pub const LAND_VALUE_RANGE: u32 = 0x87CD_6333;
    pub const USE_INTRINSIC: u32 = 0x87CD_633F;
    pub const BASELINE: u32 = 0x87CD_6357;
    pub const GROWTH_THRESHOLD: u32 = 0x87CD_6355;
    pub const DECLINE_THRESHOLD: u32 = 0x87CD_6354;
    pub const PHANTOM_DEMAND: u32 = 0x87CD_6356;
    pub const STAGE_COUNT: u32 = 0x87CD_6360;
    /// Stage curve i (0-based) is `STAGE_CURVE_0 + i`.
    pub const STAGE_CURVE_0: u32 = 0x87CD_6361;
    pub const POWER_CAP: u32 = 0x87CD_6373;
    pub const WATER_CAP: u32 = 0x87CD_6370;
    pub const FIRE_CAP: u32 = 0x87CD_6371;
    pub const PARK_CAP: u32 = 0x87CD_6372;
    pub const ABANDONMENT: u32 = 0x87CD_6380;
    pub const MONOPOLY: u32 = 0x87CD_6381;
    pub const STYLES: u32 = 0x87CD_63A0;
    pub const SUBJECTIVE_MIN: u32 = 0xCAB1_CE3A;
    pub const SUBJECTIVE_MAX: u32 = 0xCAB1_CE3C;
    /// Special-building occupant properties (§4.4): developer types (u32
    /// array) and (radius in cells, centre, outer) triples (f32 array).
    pub const SPECIAL_TYPES: u32 = 0x4A5F_7C4B;
    pub const SPECIAL_EFFECTS: u32 = 0x4A5F_7315;
}

/// The source demand node index (§4.8: the city-total node).
pub const SOURCE_DEMAND_INDEX: u32 = 0x20000;

/// The 19 desirability curves (§4.2, §4.3), in `EstimateDesirability`'s
/// order with the commute curve between crime and traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Curve {
    LandValue = 0,
    Slope,
    ProximityR1,
    ProximityR2,
    ProximityR3,
    ProximityCO2,
    ProximityCO3,
    Park,
    Air,
    Water,
    Garbage,
    Radiation,
    School,
    Hospital,
    Crime,
    Commute,
    Traffic,
    Landmark,
    Transient,
}

pub const CURVE_COUNT: usize = 19;

impl Curve {
    pub const ALL: [Curve; CURVE_COUNT] = [
        Curve::LandValue,
        Curve::Slope,
        Curve::ProximityR1,
        Curve::ProximityR2,
        Curve::ProximityR3,
        Curve::ProximityCO2,
        Curve::ProximityCO3,
        Curve::Park,
        Curve::Air,
        Curve::Water,
        Curve::Garbage,
        Curve::Radiation,
        Curve::School,
        Curve::Hospital,
        Curve::Crime,
        Curve::Commute,
        Curve::Traffic,
        Curve::Landmark,
        Curve::Transient,
    ];

    /// The tuning property of the curve.
    pub fn property(self) -> u32 {
        CURVE_PROPS[self as usize]
    }
}

/// The curve properties by [`Curve`] (§4.3).
pub const CURVE_PROPS: [u32; CURVE_COUNT] = [
    0x87CD_6340,
    0x87CD_6398,
    0x87CD_6343,
    0x87CD_6344,
    0x87CD_6345,
    0x87CD_6391,
    0x87CD_6392,
    0x87CD_6341,
    0x87CD_634B,
    0x87CD_634C,
    0x87CD_634D,
    0x87CD_634E,
    0x87CD_6348,
    0x87CD_6349,
    0x87CD_634A,
    0x87CD_6347,
    0x87CD_6346,
    0x87CD_6399,
    0x87CD_6397,
];

/// The developer's six grids (§1.2 step 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DevGrid {
    /// `uint8`, saved.
    ZonedCount,
    /// W, `int16`, not saved.
    Working,
    /// D, `int16`, saved (slot 13).
    Desirability,
    /// `int16`, saved (slot 15).
    Population,
    /// Built capacity, `uint16`, saved (slot 14).
    Capacity,
    /// `uint16`, saved (slot 16).
    MaxCapacity,
}

impl DevGrid {
    /// Creation (and `Shutdown`) order.
    pub const ALL: [DevGrid; 6] = [
        DevGrid::ZonedCount,
        DevGrid::Working,
        DevGrid::Desirability,
        DevGrid::Population,
        DevGrid::Capacity,
        DevGrid::MaxCapacity,
    ];

    /// The instance ID offset added to the developer type (§1.2).
    pub fn instance_offset(self) -> u32 {
        match self {
            DevGrid::ZonedCount => 0x0100_0000,
            DevGrid::Working => 0x0210_0000,
            DevGrid::Desirability => 0x0200_0000,
            DevGrid::Population => 0x0300_0000,
            DevGrid::Capacity => 0x0500_0000,
            DevGrid::MaxCapacity => 0x0600_0000,
        }
    }
}

/// A grid of the developer as handed to the host (one of three element
/// types).
#[derive(Clone, Copy)]
pub enum GridHandle<'a> {
    U8(&'a Rc<dyn TractGrid<u8>>),
    I16(&'a Rc<dyn TractGrid<i16>>),
    U16(&'a Rc<dyn TractGrid<u16>>),
}

/// The services `Init` references in order (§1.2 step 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Service {
    City,
    Bds,
    OccupantManager,
    ZoneManager,
    LandValue,
    Traffic,
}

impl Service {
    /// The acquisition order (§1.2 step 2).
    pub const ACQUIRE: [Service; 6] = [
        Service::City,
        Service::Bds,
        Service::OccupantManager,
        Service::ZoneManager,
        Service::LandValue,
        Service::Traffic,
    ];
    /// The release order of `Shutdown` (§1.3).
    pub const RELEASE: [Service; 6] = [
        Service::Bds,
        Service::LandValue,
        Service::Traffic,
        Service::ZoneManager,
        Service::OccupantManager,
        Service::City,
    ];
}

/// The source maps of the layers (§4.3) and of `EstimateDesirability`
/// (§4.6), with their element types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceMap {
    /// Land value slot 7 (`uint8`).
    IntrinsicLandValue,
    /// Land value slot 8 (`uint8`).
    TotalLandValue,
    /// Land value slot 9 (`float`).
    Slope,
    /// Residential slot 5 `GetProximityMap(wealth 1..3)` (`uint8`).
    ResidentialProximity(u32),
    /// Commercial slot 5 `GetProximityMap(0x3320 / 0x3330)` (`uint8`).
    CommercialProximity(u32),
    /// Aura slot 14 (`int16`).
    Park,
    /// Pollution slot 5 `GetPollutionGrid(0..3)` (`int16`).
    Pollution(u32),
    /// Residential slots 39, 40 (`float`).
    SchoolGrade,
    HospitalGrade,
    /// Crime slot 5 (`uint8`).
    Crime,
    /// Traffic slot 13 (`uint8`, cell resolution).
    TripLength,
    /// Traffic slot 9 (`uint8`).
    CommercialTraffic,
    /// Aura slot 15 (`int16`).
    Landmark,
    /// Aura slot 13 (`int8`).
    TransientAura,
}

/// A message as `DoMessage` sees it (§3.1, §6.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GrowthMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    /// ZoningChange `void2`: `{x0, z0, x1, z1}` in cells.
    pub rect: [i32; 4],
    /// StateChanged `IGZUnknown`: the lot (host token).
    pub unknown: Option<u64>,
}

impl GrowthMessage {
    pub fn of_type(msg_type: u32) -> GrowthMessage {
        GrowthMessage {
            msg_type,
            ..GrowthMessage::default()
        }
    }
}

/// Everything the growth developer reads from, or does to, the rest of the
/// game (§11). Objects are host tokens.
pub trait GrowthDeveloperHost: GridProvider<u8> + GridProvider<i16> + GridProvider<u16> {
    // ---- lifecycle (§1.2, §1.3) ----
    /// Takes a reference to a service; false when it is null.
    fn acquire_service(&self, s: Service) -> bool;
    fn release_service(&self, s: Service);
    /// Takes the message server; false when it is null.
    fn acquire_message_server(&self) -> bool;
    fn release_message_server(&self);
    /// `AddNotification` / `RemoveNotification` for each type, in order.
    fn subscribe(&self, msg_types: &[u32]);
    fn unsubscribe(&self, msg_types: &[u32]);
    /// Whether the simulator exists (§1.2 step 9).
    fn simulator_present(&self) -> bool;
    /// `AddAgent(target, level, name, flags)`.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(target)` on every level.
    fn remove_agent(&self);
    /// `cISC4Simulator::GetSimDateNumber()`.
    fn sim_date_number(&self) -> u32;
    /// `SetTractSize(S)` on one of the developer's grids (§4.8).
    fn grid_set_tract_size(&self, which: DevGrid, grid: GridHandle<'_>, tract_size: i32);
    /// `Shutdown()` then `Release()` on a grid (§1.3).
    fn grid_shutdown(&self, which: DevGrid, grid: GridHandle<'_>);
    /// `AddRef` / `Release` on a held object (special buildings, the source
    /// demand node).
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- demand and neighbours (§4.8, §5.3, §5.5) ----
    /// `cISC4DemandSimulator::GetDemand(id, index)` (no `AddRef`).
    fn get_demand(&self, id: u32, index: u32) -> Option<u64>;
    /// `cISC4Demand::QuerySupplyValue()`.
    fn query_supply(&self, node: u64) -> f32;
    /// `cISC4Demand::AddToDemandValue(v)`.
    fn add_to_demand_value(&self, node: u64, v: f32);
    /// `cISC4Demand::QueryActiveDemandValue()` (slot 12). The developer never calls
    /// it; [`GrowthDeveloper::source_active_demand`] does for the BDS glue.
    fn active_demand(&self, node: u64) -> f32;
    /// The counts of `GetConnectedCities(list, 0)` and
    /// `GetAllConnectedCities(list, 0)` (temporary lists, §5.5).
    fn connected_cities(&self) -> u32;
    fn all_connected_cities(&self) -> u32;

    // ---- the BDS (building-development.md) ----
    /// `GetStagePopulationMultiplier()` (slot 53).
    fn stage_population_multiplier(&self) -> f32;
    /// `GetTractDeveloper()` (slot 35).
    fn tract_developer(&self) -> Option<u64>;
    /// `GetSpecialBuildings()` (slot 44): the occupants in list order.
    fn special_buildings(&self) -> Vec<u64>;
    /// `GetMaxCapacity(zoneType, purpose, wealth)` (slot 45, ST0).
    fn max_capacity(&self, zone_type: i32, purpose: u32, wealth: u32) -> f32;
    /// `GetBuildingProfile(type).purpose` (slot 39; inserts a default
    /// profile for an unknown type).
    fn building_purpose(&self, building_type: u32) -> u32;

    // ---- the tract developer ----
    /// `cISC4TractDeveloper::UpdatePopulation(profile)` (slot 7).
    fn update_population(&self, tract_developer: u64, profile: &GrowthProfile);

    // ---- zones, lots, occupants, city ----
    /// `cISC4ZoneManager::GetZoneType(x, z)` (slot 12, `0x004DC740`): the
    /// cell's `int8` zone type; the original returns it sign-extended to all
    /// of EAX (zones.md §2, §11.1), which the developer does on its side.
    fn zone_type(&self, cell_x: i32, cell_z: i32) -> i8;
    /// `cISC4Lot::GetBuildingType(flag)` (slot 46).
    fn lot_building_type(&self, lot: u64, flag: bool) -> u32;
    /// `GetCapacity(devType, flag)` (slot 50).
    fn lot_capacity(&self, lot: u64, developer_type: u32, flag: bool) -> u16;
    /// `GetSize()` (slot 22): width, depth in cells.
    fn lot_size(&self, lot: u64) -> (u8, u8);
    /// `GetBoundingRect()` (slot 14): `{x0, z0, x1, z1}` in cells, inclusive.
    fn lot_bounding_rect(&self, lot: u64) -> [i32; 4];
    /// `GetLocation()` (slot 21), cells.
    fn lot_location(&self, lot: u64) -> (i32, i32);
    /// A property of an occupant's property holder (occupant slot 6, holder
    /// slot 9); `None` when absent.
    fn occupant_property(&self, occupant: u64, id: u32) -> Option<Variant>;
    /// `GetPosition()` (occupant slot 8): x, y, z.
    fn occupant_position(&self, occupant: u64) -> [f32; 3];
    /// `cISC4City::PositionToCell(x, z)` (slot 95).
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);

    // ---- source maps (§4.3; `None` = null) ----
    fn source_u8(&self, map: SourceMap) -> Option<Rc<dyn TractGrid<u8>>>;
    fn source_i8(&self, map: SourceMap) -> Option<Rc<dyn TractGrid<i8>>>;
    fn source_i16(&self, map: SourceMap) -> Option<Rc<dyn TractGrid<i16>>>;
    fn source_f32(&self, map: SourceMap) -> Option<Rc<dyn TractGrid<f32>>>;

    // ---- time slicing and seeds ----
    /// Starts the CPU-cycle limit timer with `ms` milliseconds (simulator
    /// §5.5).
    fn limit_timer_start(&self, ms: u32);
    /// Whether the deadline has passed.
    fn limit_timer_expired(&self) -> bool;
    /// The session seed `S` (foundations §6.3). Read only when a decline
    /// key's coordinate seed is `0xFFFFFFFF` (§7 quirk: the original's
    /// `Seed` then reads the clock; ours uses `derive(0x0990C138 XOR n)`).
    fn session_seed(&self) -> u32;
}

/// The adaptive step of the time-slice helper (simulator §5.5, aura §6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliceStep {
    /// Added per iteration; the clock is read when the counter is a
    /// multiple of 1024 (1..=1024).
    pub step: u32,
    pub misses: i32,
}

impl Default for SliceStep {
    fn default() -> Self {
        SliceStep { step: 1024, misses: 0 }
    }
}

impl SliceStep {
    /// The adaptation at the end of a call with `n = count >> 10`.
    pub fn adapt(&mut self, n: u32) {
        if n > 20 {
            self.misses = self.misses.saturating_add(1);
            if self.misses >= 16 {
                self.misses = 0;
                if self.step > 1 {
                    self.step /= 2;
                }
            }
        } else if (1..5).contains(&n) {
            self.misses = self.misses.saturating_sub(1);
            if self.misses <= -4 {
                self.misses = 0;
                if self.step < 1024 {
                    self.step *= 2;
                }
            }
        }
    }
}

/// The process-wide state of the growth developer code (§5.2, §7): the
/// file-local `cRZRandom` shared by all developers and the time-slice
/// step. One per process (or per test world); every developer holds the same
/// `Rc`.
pub struct GrowthStatics {
    rng: RefCell<RzRandom>,
    draws: Cell<u64>,
    slice: Cell<SliceStep>,
}

impl GrowthStatics {
    /// The generator seeded with `seed` exactly (tests and replays).
    /// `0xFFFFFFFF` takes `seeds`' wall clock (foundations §6.1).
    pub fn new(seed: u32, seeds: &dyn SeedSource) -> Rc<GrowthStatics> {
        Rc::new(GrowthStatics {
            rng: RefCell::new(RzRandom::new(seed, seeds)),
            draws: Cell::new(0),
            slice: Cell::new(SliceStep::default()),
        })
    }

    /// The generator seeded from the session seed (foundations §6.3
    /// *[choice]*): `derive_seed(session, 0xAC09C0E8)`, the original's
    /// `cRZTime() XOR 0xAC09C0E8` with the session seed in place of the
    /// clock.
    pub fn from_session(session: u32, seeds: &dyn SeedSource) -> Rc<GrowthStatics> {
        GrowthStatics::new(
            crate::seeding::seed(session, crate::seeding::keys::GROWTH_DEVELOPER),
            seeds,
        )
    }

    /// One salt (§7 row 1): `NextRaw()`.
    pub fn draw_salt(&self) -> u32 {
        self.draws.set(self.draws.get().wrapping_add(1));
        self.rng.borrow_mut().next_raw()
    }

    /// The generator's LCG state.
    pub fn rng_state(&self) -> u32 {
        self.rng.borrow().lcg_state()
    }

    /// Reseeds the generator (tests and replays).
    pub fn reseed(&self, seed: u32, seeds: &dyn SeedSource) {
        self.rng.borrow_mut().seed(seed, seeds);
    }

    /// Salts drawn so far.
    pub fn draws(&self) -> u64 {
        self.draws.get()
    }

    pub fn slice_step(&self) -> SliceStep {
        self.slice.get()
    }

    pub fn set_slice_step(&self, s: SliceStep) {
        self.slice.set(s);
    }
}

/// Which sub-object `QueryInterface` returns (§1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthInterface {
    /// `cISC4GrowthDeveloper` (also `cIGZUnknown`).
    Primary,
    MessageTarget,
    Serializable,
}

/// `QueryInterface(iid)` (§1); `None` = false.
pub fn query_interface(iid: u32) -> Option<GrowthInterface> {
    match iid {
        IID_UNKNOWN | IID_GROWTH_DEVELOPER => Some(GrowthInterface::Primary),
        IID_MESSAGE_TARGET2 => Some(GrowthInterface::MessageTarget),
        IID_SERIALIZABLE => Some(GrowthInterface::Serializable),
        _ => None,
    }
}

/// How a compiled slot returns its result (§10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotReturn {
    Al,
    Eax,
    /// Only AX is meaningful (slot 29).
    Ax,
    St0,
    None,
}

/// One row of the compiled slot table: (slot, method, callee-pop bytes,
/// return channel).
pub type SlotRow = (u32, &'static str, u32, SlotReturn);

/// `cISC4GrowthDeveloper` (§10.1), with the class slot 31.
pub const SLOTS: [SlotRow; 32] = {
    use SlotReturn::*;
    [
        (0, "QueryInterface", 8, Al),
        (1, "AddRef", 0, Eax),
        (2, "Release", 0, Eax),
        (3, "Init", 4, Al),
        (4, "Shutdown", 0, Al),
        (5, "GetType", 0, Eax),
        (6, "GetSubType", 0, Eax),
        (7, "GetPurpose", 0, Eax),
        (8, "GetWealth", 0, Eax),
        (9, "GetConstructionInfo", 4, None),
        (10, "GrowthAndDeclineListsAreValid", 0, Eax),
        (11, "GetGrowthList", 0, Eax),
        (12, "GetDeclineList", 0, Eax),
        (13, "GetDesirabilityMap", 0, Eax),
        (14, "GetCapacityMap", 0, Eax),
        (15, "GetPopulationMap", 0, Eax),
        (16, "GetMaxCapacityMap", 0, Eax),
        (17, "GetTotalCapacity", 0, Eax),
        (18, "GetSourceDemand", 0, Eax),
        (19, "GetPowerSupplyCap", 0, Al),
        (20, "GetWaterSupplyCap", 0, Al),
        (21, "GetFireCoverageCap", 0, Al),
        (22, "GetParkCap", 0, Al),
        (23, "GetDesirabilityThreshold", 0, Eax),
        (24, "GetAbandonmentThreshold", 4, St0),
        (25, "GetMonopolyUse", 0, Eax),
        (26, "GetStylePercentages", 0, Eax),
        (27, "StartMapUpdate", 0, None),
        (28, "IsMapUpdateFinished", 0, Eax),
        (29, "EstimateDesirability", 8, Ax),
        (30, "GetSubjectiveThresholds", 12, None),
        (31, "~cSC4GrowthDeveloper (scalar deleting)", 4, Eax),
    ]
};
