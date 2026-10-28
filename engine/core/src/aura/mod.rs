// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The aura (mayor rating) subsystem (`docs/specs/aura.md`): the aura
//! simulator (`cISC4AuraSimulator`, class `0x0990BDD8`). Section references
//! (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`AuraSim`]: the state of §3, the lifecycle of §4, the messages of §5,
//!   the 18-state monthly pipeline of §6, the occupant cones of §7, the
//!   interface methods of §8, the registry callbacks of §4.3 and the save
//!   record of §11.
//! - [`AuraTuning`]: the tuning exemplar of §10 (code defaults, the
//!   "delete" curve pattern of foundations §4.1); [`shipped_tuning`] holds
//!   the shipped values.
//! - [`AuraHost`]: everything the simulator reads from, or does to, the rest
//!   of the game (city, lots, occupants, budget, ordinances, the grids of
//!   other simulators, history, disasters, strikes, messages, the CPU-cycle
//!   limit timer, agent and registry plumbing). Its supertraits
//!   [`GridProvider<i8>`] and [`GridProvider<i16>`] create the aura's own
//!   grids and resolve grid references read from a save (in game: the
//!   game's own `cSC4SimGrid` objects, simgrid §6.4).
//! - [`LotIterator`]: the lot manager's safe lot iterator (§6.4, §5.2).
//! - [`AuraRandom`]: the simulator's `cRZRandom` (§9), behind a trait so
//!   tests can script it ([`AuraRng`] is the real one).
//!
//! Grids are held as `Rc<dyn TractGrid<T>>`. Dropping a handle is the
//! owner's `Shutdown` + `Release` of that grid (the plugin's wrapper does
//! this in its `Drop`); the core drops its handles in `Shutdown` (§4.2) and
//! drops the extra landmark grid of a version-8 record right after reading
//! it (§11.3).
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - §6.6: a missing park curve advances to state 7 (the spec's own
//!   [choice]); §8.1: transient types ≥ 4 are ignored; §11.3: versions
//!   above 9 are rejected.
//! - §4.3: the registry node is published when the host can; a failure is
//!   not an error. The callbacks receive their parameters as variant
//!   elements and convert them with the element's numeric value (the
//!   plugin performs the game's `GetValFloat32` / `GetValUint32` reads);
//!   missing parameters read as 0.
//! - §6.2: a missing Parks and Recreation department (or budget) uses
//!   funding 1.0 (the spec's [choice]; the original dereferences null);
//!   §6.3 the same for a department that does not exist, as the spec states.
//! - Float → integer stores follow the CRT `_ftol` (§6 "Integer stores",
//!   foundations §18.1, [compat]): truncation to an `int64`, of which the
//!   int16 grid stores keep the low 16 bits (wrap, NaN → 0), the clamped
//!   int8 stores and every other `trunc` the low 32 bits (NaN → 0), and the
//!   transient decay (§6.17) the low 8 bits. Float arithmetic follows
//!   foundations §18: every operation is one `f32` rounding; an `int32`
//!   operand is used unrounded (computed in `f64`, rounded once to `f32`).
//! - §6.3: the department cursor is `Option<u32>` (`None` = −1), so a
//!   department with ID `0xFFFFFFFF` is processed like any other.
//! - §6.5: the time-slice `step` and miss counter (a process-wide static
//!   in the original) are kept per simulator (simulator §5.5 allows it);
//!   they are not reset by `Init`, `Shutdown` or a load. One `DoIdle` call
//!   processes at most [`MAX_LOTS_PER_CALL`] lots even if the host's timer
//!   never expires (the call then ends like a timer stop). A state-5 call
//!   without an iterator ends the walk (no adaptation).
//! - §6.5: a lot's bounding rectangle is clamped to the work grid; cells
//!   outside are skipped.
//! - §6.5.1, §6.15: 32-bit sums wrap (`sum`, `total`, `weighted`, the
//!   population total); a negative wrapped total counts as "≤ 0".
//! - §6.15: the population grids are read at the population tract that
//!   contains the aura tract's centre cell, `⌊(x·S + S/2) / P⌋` (the
//!   identity for the shipped P = S = 4; the original reads raw aura
//!   indices, garbage when P ≠ S).
//! - §6.7–§6.12, §6.15: a grid of another simulator the host cannot supply
//!   skips its stage (pollution, school, hospital, crime, trip, congestion)
//!   or counts as population 0 (residential population grids).
//! - §6.14: `d × m` is one `f32` rounding of the exact product
//!   (foundations §18) before `ceil` / `floor`; `avg + globalEffect` and `d`
//!   are computed in 64 bits.
//! - §6.16: hotspot arithmetic is done in 64 bits and stored as 32 bits.
//! - §7 (AU4): an effect property is used only as a Sint32 or Uint32 array
//!   (bitwise) of at least two values; anything else (a scalar, one value,
//!   another element type) is absent: no cone. The original reads the word
//!   at variant + 4 as an int32 array whatever the type and count (garbage
//!   past a one-value array, a crash for a scalar). `PositionToCell`'s
//!   result flag is ignored; "distance to the edge" is the absolute
//!   difference in `f32`.
//! - §6, §10: a curve property with fewer than two values (a scalar, a
//!   one-element or an empty array) or a non-numeric one is **absent**: the
//!   stage is skipped (state 6 still advances). The original builds a curve
//!   with no points and evaluates it (a crash, or a stale value after an
//!   in-place reload).
//! - §10 (AU2): a transient type's {magnitude, radius} is taken only from
//!   a Float32 array of at least two values; anything else keeps {0, 0}
//!   (the original copies raw words and reads `v[0]`, `v[1]` unchecked).
//! - §10 (AU11): the base-aura table is the aura's own storage (no other
//!   code reads it), so it is kept in the object; the host's
//!   `publish_base_aura` is told the tuned values and may ignore them.
//! - §6.5.1 (AU10): without a land value simulator the value/wealth term is
//!   skipped (the result is the lot average), as if the curve were absent
//!   (the original calls through a null pointer).
//! - §11.3: `Read` writes each field as it reads it. Grid references are
//!   null-first (foundations §2.7): a reference that fails, or that does not
//!   resolve to a grid of the expected element type, leaves its slot empty
//!   (`Init` recreates it), and so does every later reference once the
//!   stream is in error; the grid the slot held is released (the original
//!   leaks it; on the game's load path the slots are empty). A plain field
//!   whose read fails keeps its value. The department map is **merged**
//!   (AU1): each entry read in full sets `map[id] := grid`, IDs not in the
//!   record stay (a null reference adds or changes nothing, [choice]; a
//!   duplicate ID keeps the later grid; a replaced grid is released, the
//!   original leaks it). Hotspots are **appended** after the ones held.
//!   Counts are bounded by the stream: reading stops at the first stream
//!   error; a count that cannot be read is 0 ([choice]: the original loops
//!   over an uninitialised count).
//! - Unknown pipeline states (only reachable through debug setters) reset
//!   the state to 0.

mod pipeline;
mod simulator;
#[cfg(test)]
mod tests;
mod tuning;
pub mod archive;

pub use simulator::{AuraData, AuraSim, Hotspot, SliceStep, MAX_LOTS_PER_CALL};
pub use tuning::{shipped_tuning, AuraTuning};

use std::rc::Rc;

use crate::grid::{GridProvider, TractGrid};
use crate::property::PropertyHolder;
use crate::rng::{RzRandom, SeedSource, CLOCK_SEED};
use crate::stream::ResKey;

/// Class and interface IDs (§1).
pub const CLSID_AURA_SIMULATOR: u32 = 0x0990_BDD8;
pub const IID_AURA_SIMULATOR: u32 = 0xC773_E880;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const IID_UNKNOWN: u32 = 1;

/// The record version written (§11.2); 8 is also read (§11.3).
pub const AURA_RECORD_VERSION: u16 = 9;

/// Agent registration (§4.1 step 7): one target at these levels, in this
/// order, flags 2.
pub const AGENT_NAME: &str = "cSC4AuraSimulator";
pub const AGENT_LEVELS: [u32; 4] = [0, 10, 4, 6];
pub const AGENT_FLAGS: u32 = 2;
/// The time budget of the SimNewDay step (§5.1), in milliseconds.
pub const DAY_STEP_MS: u32 = 5;

/// The `"aura"` game-data registry node and its commands (§4.3).
pub const REGISTRY_NODE: &str = "aura";
pub const REGISTRY_ADD_TRANSIENT_EFFECT: &str = "add_transient_effect";
pub const REGISTRY_END_STRIKE: &str = "end_strike";

/// The tuning exemplar (§10).
pub const TUNING_KEY: ResKey = ResKey {
    type_id: 0x6534_284A,
    group: 0xE7E2_C2DB,
    instance: 0xAA02_3079,
};

/// Grid CLSIDs and instance IDs (§2).
pub mod grid_id {
    pub const CLSID_INT8: u32 = 0x49B9_E603;
    pub const CLSID_INT16: u32 = 0x49B9_E605;
    pub const AURA: u32 = 0x69D5_B600;
    pub const BASE: u32 = 0x69D5_B604;
    pub const TRANSIENT: u32 = 0x69D5_B605;
    pub const WORK: u32 = 0x69D5_B606;
    pub const BUILDING_EFFECT: u32 = 0x49D5_B607;
    pub const PARK_MAP: u32 = 0x49D5_B602;
    pub const RAW_PARK: u32 = 0x2A67_4CA5;
    /// The landmark map, and also the landmark work grid [quirk].
    pub const LANDMARK_MAP: u32 = 0x8A67_4CAF;
    /// Tract size of every grid but the work grid.
    pub const TRACT_SIZE: i32 = 4;
    /// Tract size of the per-cell work grid.
    pub const WORK_TRACT_SIZE: i32 = 1;
}

/// Messages handled and posted (§5, §11.1).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const SIM_IDLE: u32 = 0x6695_681A;
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const ORDINANCE_UPDATED: u32 = 0xC2C0_9554;
    /// Exemplar changed (foundations §5.2).
    pub const EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;
    /// The five subscriptions, in the order of §4.1 step 6.
    pub const SUBSCRIPTIONS: [u32; 5] = [
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        ORDINANCE_UPDATED,
        EXEMPLAR_CHANGED,
        POST_CITY_INIT,
    ];
    /// Posted at the end of every complete cycle (§11.1).
    pub const AURA_CYCLE_COMPLETED: u32 = 0x2A09_B21A;
}

/// Other IDs the simulator uses.
pub mod ids {
    /// Preview-occupant marker (foundations §5.2): presence test only.
    pub const PROP_PREVIEW_OCCUPANT: u32 = 0x0292_44DB;
    /// Occupant cone properties (§7).
    pub const PROP_PARK_EFFECT: u32 = 0x2781_2850;
    pub const PROP_LANDMARK_EFFECT: u32 = 0x2781_284F;
    pub const PROP_MAYOR_RATING_EFFECT: u32 = 0xCA5B_9305;
    /// The network occupant type (§7 step 1).
    pub const OCCUPANT_TYPE_NETWORK: u32 = 0xC772_BF98;
    /// Budget purpose of a landmark's department (§7.2).
    pub const PURPOSE_LANDMARK: u32 = 0xAA59_670C;
    /// Parks and Recreation (foundations §5.2).
    pub const DEPARTMENT_PARKS: u32 = 0x8A42_60A6;
    /// Ordinance offset property (§5.2, §6.14).
    pub const ORDINANCE_AURA_EFFECT: u32 = 0xAA5B_8407;
    /// History series of the mayor rating (§6.15).
    pub const HISTORY_MAYOR_RATING: u32 = 0x0A5C_BF37;
    /// The riot disaster manager (§6.16).
    pub const DISASTER_RIOT: u32 = 0xCA38_0B06;
    /// Residential developer types (§6.5.1): R$, R$$, R$$$.
    pub const DEVELOPER_RESIDENTIAL: [u32; 3] = [0x1010, 0x1020, 0x1030];
}

/// The `cISC4AuraSimulator` compiled slot table (§12.1): (slot, method,
/// callee-pop bytes). The two `AddTransientEffect` overloads are in MSVC
/// order (slot 6 = type, slot 7 = radius/magnitude).
pub const SLOT_TABLE: [(u32, &str, u32); 17] = [
    (0, "QueryInterface", 8),
    (1, "AddRef", 0),
    (2, "Release", 0),
    (3, "Init", 0),
    (4, "Shutdown", 0),
    (5, "GetAuraValue", 8),
    (6, "AddTransientEffect(x, z, type)", 12),
    (7, "AddTransientEffect(x, z, radius, magnitude)", 16),
    (8, "GetMayorRating", 0),
    (9, "GetMaxMayorRating", 4),
    (10, "GetMinMayorRating", 4),
    (11, "BustStrike", 4),
    (12, "GetAuraGrid", 0),
    (13, "GetTransientAuraGrid", 0),
    (14, "GetParkMap", 0),
    (15, "GetLandmarkMap", 0),
    (16, "destructor", 4),
];

/// Which sub-object `QueryInterface` returns (§1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuraInterface {
    /// `cISC4AuraSimulator` and `cIGZUnknown`: the primary pointer.
    Primary,
    MessageTarget,
    Serializable,
}

/// `QueryInterface` (§1): the sub-object for `iid`, or `None` (false).
pub fn query_interface(iid: u32) -> Option<AuraInterface> {
    match iid {
        IID_AURA_SIMULATOR | IID_UNKNOWN => Some(AuraInterface::Primary),
        IID_MESSAGE_TARGET2 => Some(AuraInterface::MessageTarget),
        IID_SERIALIZABLE => Some(AuraInterface::Serializable),
        _ => None,
    }
}

/// A strike `BustStrike` ends (§8.4), in call order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strike {
    /// `cISC4PoliceSimulator::EndStrike()`.
    Police,
    /// `cISC4FireProtectionSimulator::EndStrike()`.
    Fire,
    /// `cISC4ResidentialSimulator::EndHealthStrike()`.
    Health,
    /// `cISC4ResidentialSimulator::EndSchoolStrike()`.
    School,
}

/// A message as `DoMessage` sees it. `void1` is the occupant (as a host
/// token) for the occupant messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuraMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub void1: u64,
}

/// What the simulator reads from a lot (`cISC4Lot`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LotInfo {
    /// `GetBoundingRect`: inclusive cell rectangle.
    pub x1: i32,
    pub z1: i32,
    pub x2: i32,
    pub z2: i32,
    /// `GetZoneType()` (1..3 = residential).
    pub zone_type: u32,
}

/// A lot as the safe lot iterator delivers it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IteratedLot {
    pub info: LotInfo,
    /// The occupant of `GetBuilding()`, as a host token; `None` = no building.
    pub building: Option<u64>,
    /// `GetOccupantWealth()` (1..3).
    pub occupant_wealth: i32,
    /// `GetPopulations()`: (developer type, count) pairs, in order.
    pub populations: Vec<(u32, i32)>,
}

/// The lot manager's safe lot iterator (`GetSafeLotIterator()`): lots in
/// the manager's order, tolerating lots removed while it is held. Dropping
/// it releases it.
pub trait LotIterator {
    fn next_lot(&mut self) -> Option<IteratedLot>;
}

/// Everything the aura simulator reads from, or does to, the rest of the
/// game. Callbacks must not call back into the aura simulator. Occupants
/// are host tokens (`cISC4Occupant*` in the plugin).
pub trait AuraHost: GridProvider<i8> + GridProvider<i16> {
    // ---- city and simulator ----
    /// `cISC4City::CellWidthX()` / `CellWidthZ()` (metres).
    fn cell_width(&self) -> (f32, f32);
    /// `cISC4City::PositionToCell(x, z, cx, cz)` (compiled slot 95); the
    /// result flag is ignored.
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);
    /// `cISC4Simulator::GetSimDateNumber()` (compiled slot 8).
    fn sim_date_number(&self) -> u32;
    /// The "Aura Simulator" exemplar ([`TUNING_KEY`]), if it loads.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;
    /// The tuned base auras after every tuning read: R$, R$$, R$$$. The
    /// original copies them into a process-wide table (§10) that only the
    /// aura itself reads (AU11), so a host may ignore them.
    fn publish_base_aura(&self, base_aura: [i32; 3]);

    // ---- lots ----
    /// `cISC4LotManager::GetLotCount()`.
    fn lot_count(&self) -> u32;
    /// `cISC4LotManager::GetSafeLotIterator()`: a fresh iterator.
    fn lot_iterator(&self) -> Box<dyn LotIterator>;
    /// `cISC4LotManager::GetLot(x, z, false)` (cells).
    fn lot_at(&self, x: i32, z: i32) -> Option<LotInfo>;
    /// `cISC4LandValueSimulator::GetLandValueType(x, z)` (1..3); `None`
    /// when the city has no land value simulator (§6.5.1, AU10: the
    /// value/wealth term is then skipped).
    fn land_value_type(&self, x: i32, z: i32) -> Option<i32>;

    // ---- occupants ----
    /// `HasProperty(id)` on the occupant's property holder.
    fn occupant_has_property(&self, occupant: u64, prop: u32) -> bool;
    /// `GetProperty(id)->RefSint32()`: the Sint32 (or Uint32, bitwise)
    /// array, or `None`. The core treats fewer than two values as absent
    /// (§7, AU4).
    fn occupant_sint32s(&self, occupant: u64, prop: u32) -> Option<Vec<i32>>;
    /// `cISC4Occupant::GetType()`.
    fn occupant_type(&self, occupant: u64) -> u32;
    /// `cISC4Occupant::GetPosition()` (world x, y, z).
    fn occupant_position(&self, occupant: u64) -> [f32; 3];
    /// `budget.GetBudgetItemForPurpose(holder, purpose, &item)`: the item's
    /// department ID, or `None` when it returns false.
    fn budget_item_department(&self, occupant: u64, purpose: u32) -> Option<u32>;

    // ---- budget, ordinances ----
    /// `*GetDepartmentBudget(dept)->GetFundingPercentage()` (a fraction);
    /// `None` when the budget has no such department.
    fn department_funding(&self, dept: u32) -> Option<f32>;
    /// `cISC4BudgetSimulator::GetTaxRate(index)`.
    fn tax_rate(&self, index: i32) -> f32;
    /// `cISC4OrdinanceSimulator::GetCumulativeOffset(prop, index)`.
    fn cumulative_offset(&self, prop: u32, index: u32) -> i32;

    // ---- grids of other simulators (no reference added) ----
    /// `cISC4PollutionSimulator::GetPollutionGrid(kind)` (0 air, 1 water,
    /// 2 garbage, 3 radiation).
    fn pollution_grid(&self, kind: i32) -> Option<Rc<dyn TractGrid<i16>>>;
    /// Residential compiled slot 39: the school grade map.
    fn school_grade_map(&self) -> Option<Rc<dyn TractGrid<f32>>>;
    /// Residential compiled slot 40: the hospital grade map.
    fn hospital_grade_map(&self) -> Option<Rc<dyn TractGrid<f32>>>;
    /// `cISC4CrimeSimulator::GetCrimeGrid()`.
    fn crime_grid(&self) -> Option<Rc<dyn TractGrid<u8>>>;
    /// `cISC4TrafficSimulator::GetTripLengthMap()`.
    fn trip_length_map(&self) -> Option<Rc<dyn TractGrid<u8>>>;
    /// `cISC4TrafficSimulator::GetCongestionMap()`.
    fn congestion_map(&self) -> Option<Rc<dyn TractGrid<u8>>>;
    /// `cISC4ResidentialSimulator::GetPopulationGrids(&r1, &r2, &r3)`.
    fn population_grids(&self) -> Option<[Rc<dyn TractGrid<u16>>; 3]>;

    // ---- effects on the game ----
    /// `cISC4HistoryWarehouse::RecordHistoryData(series, value)`.
    fn record_history(&self, series: u32, value: i32);
    /// `city.GetDisasterLayer().GetDisasterManager(manager)` and, if it
    /// exists, `AttemptDisasterStart(x, z)` (result ignored).
    fn attempt_disaster_start(&self, manager: u32, x: i32, z: i32);
    /// Ends one service strike (§8.4).
    fn end_strike(&self, strike: Strike);
    /// `MessagePost` (queued) of `msg_type` with void1 = the IGZUnknown =
    /// the aura simulator (§11.1).
    fn post(&self, msg_type: u32);

    // ---- CPU-cycle limit timer (simulator §5.5) ----
    /// Starts the limit timer with a budget of `ms` milliseconds.
    fn limit_timer_start(&self, ms: u32);
    /// Whether the budget is used up.
    fn limit_timer_expired(&self) -> bool;

    // ---- framework plumbing (§4.1, §4.2) ----
    /// Subscribes the message target to `msgs` on the message server.
    fn subscribe(&self, msgs: &[u32]);
    /// Unsubscribes `msgs` and releases the message server.
    fn unsubscribe(&self, msgs: &[u32]);
    /// `cISC4Simulator::AddAgent(target, level, name, flags)`.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `cISC4Simulator::RemoveAgent(target)` (every level).
    fn remove_agent(&self);
    /// Publishes the `"aura"` registry node with its two commands (§4.3).
    fn registry_add(&self) -> bool;
    /// `RemoveChildEntry(aura simulator)` (§4.3).
    fn registry_remove(&self);
}

/// The draws the aura makes from its `cRZRandom` (§9).
pub trait AuraRandom {
    /// `RandomDoubleRangeUniform(a, b)`.
    fn range_f64(&mut self, a: f64, b: f64) -> f64;
    /// `RandomUint32Uniform(n)`.
    fn uniform_u32(&mut self, n: u32) -> u32;
}

/// The real generator: a `cRZRandom`.
pub struct AuraRng(pub RzRandom);

impl AuraRng {
    /// Seeded from the wall clock (`Seed(0xFFFFFFFF)`, §9).
    pub fn new(seeds: &dyn SeedSource) -> AuraRng {
        AuraRng(RzRandom::new(CLOCK_SEED, seeds))
    }

    pub fn with_seed(seed: u32, seeds: &dyn SeedSource) -> AuraRng {
        AuraRng(RzRandom::new(seed, seeds))
    }

    /// Seeded from the session seed (foundations §6.3 *[choice]*: a member
    /// seeded from the clock takes `derive(GZCLSID)`, `0x0990BDD8`).
    pub fn from_session(session: u32) -> AuraRng {
        let key = crate::seeding::keys::AURA;
        AuraRng::with_seed(
            crate::seeding::seed(session, key),
            &crate::seeding::clocks(session, key),
        )
    }
}

impl AuraRandom for AuraRng {
    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        self.0.range_f64(a, b)
    }
    fn uniform_u32(&mut self, n: u32) -> u32 {
        self.0.uniform_u32(n)
    }
}

/// `clamp` of §6: −127..127 (−128 is "no data").
pub(crate) fn clamp_aura(v: i64) -> i8 {
    v.clamp(-127, 127) as i8
}

/// `_ftol` kept to 32 bits (§6 "Integer stores", foundations §18.1):
/// truncation toward zero as an `int64`, low 32 bits (NaN, ±∞ and
/// |x| ≥ 2^63 give 0).
pub(crate) fn trunc32(v: f64) -> i32 {
    crate::x87::ftol32(v)
}

/// An int8 store with "clamp" (§6, rule A): the low 32 bits of `_ftol`,
/// clamped to −127..127 (NaN → 0).
pub(crate) fn clamp_store(v: f32) -> i8 {
    clamp_aura(i64::from(trunc32(f64::from(v))))
}

/// An int16 grid store (§6.2, §6.3): the low 16 bits of `_ftol` (wraps
/// modulo 65536, NaN → 0).
pub(crate) fn store_i16(v: f32) -> i16 {
    crate::x87::ftol(f64::from(v)) as i16
}

/// An int8 store without clamp (§6.17): the low 8 bits of `_ftol`.
pub(crate) fn store_i8(v: f32) -> i8 {
    crate::x87::ftol(f64::from(v)) as i8
}
