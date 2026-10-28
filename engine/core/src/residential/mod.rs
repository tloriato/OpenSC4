// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The residential simulator (`cSC4ResidentialSimulator`, class
//! `0x4990C013`), `docs/specs/residential.md`. Section references (`§n`)
//! point at that spec unless they are prefixed `REH`, which cites
//! `docs/specs/residential-education-health.md` (the education and health
//! half of the same class).
//!
//! # Structure
//!
//! - `geometry.rs`: the proximity seed (z = 0 edge only, *[compat]* RG23)
//!   and the four-sweep Manhattan distance passes of §5.3
//!   ([`seed_proximity`], [`distance_pass`], [`distance_to_convergence`],
//!   [`copy_proximity`]).
//! - `tuning.rs`: the population tuning of §7.1 ([`PopulationTuning`]), the
//!   simulator tuning of §7.2 ([`SimulatorTuning`], whose `read` reproduces
//!   the §9.3 strike-reset quirk), the two per-call curves of §7.3
//!   ([`le_curve`], [`workforce_curve`]), [`new_ages_by_wealth`] (§6.5),
//!   [`shipped_tuning`] and the one `0.01` constant ([`PERCENT`], [`pct`]).
//! - `simulator.rs`: the state of §3, `AddRef` / `Release` (§1), the
//!   lifecycle of §4, `DoMessage` (§4.3), the daily state machine
//!   (§5.1–§5.2), `DoPopulationChange` (§5.5), `PostPopulationChange` (§5.6),
//!   the history of §5.7, the queries of §6 and the slot table of §10.
//! - `education_health.rs`: the education and health half (REH):
//!   recognition and budget registration (REH §4.1), the health-affecting
//!   pending list (§5.8), funding and efficiency (REH §4.2), the ordinance
//!   effects (REH §4.3), coverage (REH §4.4), grading and the EQ/HQ boost
//!   (REH §4.5, §4.6), the monthly strike check and the strike chance (REH
//!   §4.9), the auto-budget and ideal funding (REH §4.10) and the query
//!   slots (REH §4.11).
//! - `record.rs`: the save record of §9.
//! - `tests.rs`: the fake host and the §15 / REH §11 tests.
//!
//! # The education and health half
//!
//! REH is one class with residential.md, so it is implemented on the same
//! [`ResidentialSim`] and state. Everything it reaches outside the class —
//! the budget simulator's items, departments and line items, the occupants'
//! property holders, types and lots, the ordinance simulator and the
//! automata controller manager — goes through [`EducationHealthHost`], a
//! supertrait of [`ResidentialHost`]. Its literal readings (`[question]`)
//! and choices are listed in `education_health.rs`.
//!
//! # Choices (`[choice]`), limits and inferences (`[inferred]`)
//!
//! - **Seeding** (§3, foundations §6.3): the class's one `cRZRandom` is a
//!   *member* generator seeded from the wall clock at construction, so its
//!   constant is the class's own GZCLSID: [`ResidentialRng::from_session`]
//!   uses `derive_seed(session, 0x4990C013)`. [`ResidentialRng::new`] keeps
//!   the original's clock seed for hosts that supply one. The stream is
//!   shared by the buzzards rolls (§5.5) and the monthly strike rolls
//!   (REH §4.9), which draw from it in the original's order (one draw per
//!   eligible non-striking building in state 0). No RNG state is saved
//!   (foundations §6.2).
//! - **Time slicer** (§5.2): the 5 ms CPU-cycle budget is replaced by a
//!   deterministic work budget, [`ResidentialSim::work_budget`] (default
//!   [`DEFAULT_WORK_BUDGET`] = `u32::MAX`, effectively unbounded). One work
//!   unit is one distance pass (states 1 and 2) or one tract (state 4); a
//!   budget of 0 never progresses. Each call still completes at most one
//!   state, and an interrupted state resumes where it stopped (the iterator
//!   keeps its position; states 1–2 re-run their passes, which is
//!   idempotent), so the results equal those of an uninterrupted pass.
//! - **Float arithmetic** follows foundations §18 (24-bit x87): every add,
//!   subtract, multiply and divide is one `f32` rounding, in the order the
//!   spec gives. An integer operand (a population, a histogram count, a
//!   pollution byte) enters that one operation unrounded
//!   ([`crate::x87::mul_i64_f32`]; an unsigned load is
//!   [`crate::x87::u32_to_float`]). Every `trunc` is the CRT `_ftol`
//!   ([`crate::x87::ftol32`]). The per-mille history uses the 1.5·2²³ idiom
//!   ([`crate::x87::rhe`], §5.7).
//! - **The `0.01` constant** (RS1, answered): every "× 0.01" of this class
//!   goes through [`pct`], which multiplies by [`PERCENT`], the float32
//!   `0.01f`, and rounds the product once to float32 (so the shipped 80 %
//!   reads as 0.79999995 and `1200 × 0.01f` is 12.0 before a truncation).
//! - **`min(cap, v)`** (§5.5 step 4's `min(200, …)`) is the original's
//!   `a < b ? a : b` with the cap first, so a NaN operand is stored rather
//!   than capped (REH §4.5 *[inferred]*).
//! - **Uninitialised tuning** (§7.1, §7.2): the newcomer HQ bonuses have no
//!   code default and several simulator values are set only when the exemplar
//!   exists. FreeCity4 uses `0.0` for the HQ bonuses (residential.md §7.1
//!   *[choice]*, which is normative over REH §4.13's "use the shipped
//!   values") and the "this branch" defaults with empty chunk lists for the
//!   rest (§7.2 *[choice]*).
//! - **Curve patterns** (§7.1, §7.3): the three pollution curves are
//!   rebuilt from their code default only when the exemplar exists, then
//!   loaded by key ("default" with the exemplar, "keep" without: empty on the
//!   first read, and an empty curve evaluates to 0.0). The two per-call
//!   curves (life expectancy, workforce) are fresh per call with the
//!   "default" pattern, and a missing exemplar makes slot 29/34 return 0 and
//!   slot 30/31 write 0.0.
//! - **Pollution average** (§5.5 step 1): the `S × S` cell values (each the
//!   pollution slot's 16-bit result) are summed as a wrapping `u32` and
//!   divided by `S · S` as an unsigned 32-bit division; only the low byte is
//!   kept *[quirk]*. With `S ≤ 0` the average is 0 ([choice]: no cells, no
//!   division); [choice] at most [`MAX_TRACTS`] cells per axis are read, so a
//!   hostile saved tract size cannot make the loop unbounded.
//! - **Newcomers** (§5.5 step 4, REH §4.7): `nc_w` is `int16(new − old)`
//!   (REH's `delta`), in the blend and in the age numerator alike; the age
//!   formula is `int32` wrapping arithmetic.
//! - **Missing grids**: a grid whose reference did not resolve reads 0 and
//!   ignores writes ([`TractGrid`]'s out-of-range rule); `Init` creates every
//!   absent grid of §2, and for the two coverage vectors it creates all
//!   entries when the first is null (§4.1 step 8).
//! - **`GetGlobalEQ` / `GetGlobalHQ`** (REH §4.8 *[compat]*, RS4) accumulate
//!   over the R$ EQ (HQ) grid's tract counts, x outer, z inner, through
//!   `GetTractValue`, storing the sum as `f32` after every tract.
//! - **Slots with undefined returns** (§10.1): 19, 20, 28, 47, 49 return
//!   true; 21/22 return the grid and `(min, max)`; 23–25 return the three
//!   grids (the `AddRef` is the `Rc` clone the caller holds) and EAX 1; the
//!   AL-only slots 29, 34, 42 zero-extend into EAX.
//! - **`GetCellEQByWealth` / `GetCellHQByWealth`** (slots 37/38) take the
//!   wealth byte decremented as `u8`, so wealth 0 or > 3 reads out of bounds
//!   in the original *[quirk]*; FreeCity4 returns 0.0 ([choice]).
//! - **Occupant lists** hold `Option` tokens, but `Read` drops null
//!   references (RS12 *[choice]*), so no list entry is ever `None`; the
//!   passes still skip one rather than dereference it.
//! - **The other list** (§5.8, RS8) is the pending health-affecting list:
//!   `DoBuildingChanged` appends a health-affecting building to it, and state
//!   0 appends every entry to the health-affecting list and clears it.
//! - **`Read`** (§9.2) accepts every version. A stream error or a damaged
//!   record (vector counts other than 7 / 3, a state above 15, P, E or the
//!   loaded R$ population grid's tract size outside 1…256 as signed) returns
//!   false and leaves the simulator as it was (RS9, RS11, RS14, RS15, EH14
//!   *[choice]*; see `record.rs`). A list count of 0 or ≥
//!   [`MAX_LIST_COUNT`] reads no references.
//! - **Histograms** (§9.3 *[choice]*): zeroed at construction, so a save
//!   taken during state 4 gives that month's age-group history zeros rather
//!   than uninitialised memory. The loaded running population is kept, so
//!   the tracts already visited are counted twice in that month's population
//!   *[quirk]*.
//! - **Left out**: the dirty flag has no reader (§13 question 5) but is
//!   saved and set as specified; the two unused floats are carried through
//!   the record untouched (§3); `ToggleTractTracking`, `TrackTracts` and
//!   `OutputTractInfo` are no-ops (§10.1 slots 59, 62, 63); the grade totals
//!   (REH §2, RS13) are kept and accumulated but never read or saved.
//! - **Day-level order** (§4.3, foundations §13.5): registration order is
//!   the *[choice]* — the original's order is heap-address order and is not
//!   stable between sessions, so there is nothing to match. Note the
//!   same-pass couplings this leaves open: state 13 reads the BDS building
//!   map, and state 4 reads the growth developers' population maps and the
//!   pollution **source** grids, each as they stand when the residential day
//!   step runs. If a differential ever shows a value depending on residential
//!   running before or after one of those writers, that is a *derived*
//!   requirement for foundations §13.4's rank table, not something to fix by
//!   reordering registrations.
//! - **Threading** (foundations §19.3 rule 1): this simulator's state is a
//!   `RefCell`, as every other subsystem core's is. §19 requires `Sync` only
//!   for state the *clock* exposes to the diagnostic threads, which is
//!   `Simulator`'s own (a `Mutex` since 6f59cb2); no subsystem core's state
//!   is published, and §19.3 rule 2 prefers that it stay that way. So
//!   residential is **main-thread-only**: the host must not read its state
//!   from the watchdog or autotest thread. If a future heartbeat needs a
//!   residential value (for example the population), publish it as an
//!   atomic from the main thread rather than making the state `Sync`.
//!
//! Host callbacks must not call back into the residential simulator.

mod education_health;
pub mod archive;
mod geometry;
mod record;
mod simulator;
mod tuning;

#[cfg(test)]
mod tests;

pub use education_health::{
    add_radius_effect_with_curves, decay_product, flat_curve, ordinance_modifiers, tracts_in_radius, BudgetItem,
    OrdinanceEffect, RadiusEffect, Service, FLAT_CURVE, HOSPITAL, LINE_ITEM_DISPLAY_FLAG, NOT_GRADED,
    PROP_HEALTH_AFFECTING, PROP_STRIKE_DAY, SCHOOL, STRIKING_EDUCATION, STRIKING_HEALTH,
};
pub use geometry::{copy_proximity, distance_pass, distance_to_convergence, seed_proximity, FAR, MAX_PASSES};
pub use record::RECORD_VERSION;
pub use simulator::{
    age_bucket, query_interface, Grids, ResidentialRng, ResidentialSim, ResidentialState, SlotReturn, SlotRow,
    GRID_TRIPLE_RESULT, SLOTS,
};
pub use tuning::{
    id, le_curve, new_ages_by_wealth, pct, shipped_tuning, workforce_curve, PopulationTuning, SimulatorTuning,
    AGE_GROUPS, PERCENT, TUNING_KEY,
};

pub use crate::grid::{GridProvider, ScanIterator, TractGrid, MAX_TRACTS};
pub use crate::rng::RzRandom;

use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::stream::RefValue;
use crate::variant::Variant;

/// Class ID (`GetGZCLSID`, §1).
pub const CLSID_RESIDENTIAL_SIMULATOR: u32 = 0x4990_C013;
/// Interface IDs answered by `QueryInterface` (§1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_RESIDENTIAL_SIMULATOR: u32 = 0x077A_C1EE;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// IIDs used when reading references (§9.1, foundations §2.7).
pub const IID_SIM_GRID: u32 = 0xC9BD_939A;
pub const IID_DEPARTMENT_BUDGET: u32 = 0x223E_2B67;
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
/// `cISC4BuildingOccupant` (§4.3).
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;

/// City component slot (§1, foundations §3.6) and the city's getter slot.
pub const CITY_COMPONENT: u32 = 36;
pub const CITY_GETTER_SLOT: u32 = 68;

/// Agent registration (§4.3): levels 0, 10, 6, 4 in that order, flags 2.
pub const AGENT_NAME: &str = "cSC4ResidentialSimulator";
pub const AGENT_FLAGS: u32 = 2;
pub const AGENT_LEVELS: [u32; 4] = [0, 10, 6, 4];

/// The daily state machine's budget in milliseconds (§4.3); replaced by
/// [`DEFAULT_WORK_BUDGET`] work units ([choice], §5.2).
pub const DAILY_TIME_BUDGET_MS: u32 = 5;

/// [choice] The default work budget that replaces the 5 ms CPU-cycle slicer
/// (§5.2): effectively unbounded, so one call runs its state to the end.
pub const DEFAULT_WORK_BUDGET: u32 = u32::MAX;

/// A list count at or above this reads no references (§9.2).
pub const MAX_LIST_COUNT: u32 = 10_000_000;

/// The `GetMaxEQ` / `GetMaxHQ` value (slots 50, 51) and the EQ/HQ blend cap.
pub const MAX_QUOTIENT: i32 = 200;

/// Messages (§4.3).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const SIM_IDLE: u32 = 0x6695_681A;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    /// The two subscriptions of `Init` step 2, in order.
    pub const SUBSCRIPTIONS: [u32; 2] = [OCCUPANT_INSERTED, OCCUPANT_REMOVED];
    /// Sent (§5.7, §8): the history warehouse's class ID.
    pub const RECORD_HISTORY: u32 = 0x89EF_A536;
    /// Posted by the education/health half (REH §5).
    pub const SCHOOL_STRIKE: u32 = 0x077A_CCD0;
    pub const HEALTH_STRIKE: u32 = 0x077A_CCD2;
}

/// The occupant type of an insert / remove message (§4.3).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;

/// History series (§8).
pub mod history {
    /// The population-weighted average life expectancy (§5.6).
    pub const AVERAGE_LIFE_EXPECTANCY: u32 = 0x2A5E_7009;
    /// Age-group population, `+ g` for g = 0..8 (§5.6).
    pub const AGE_GROUP_POPULATION: u32 = 0xCA54_5CAB;
    /// Age-group average EQ, `+ g` for g = 0..8 (§5.6).
    pub const AGE_GROUP_EQ: u32 = 0xCA54_5CCA;
    pub const GLOBAL_EQ: u32 = 0xAA1A_2864;
    pub const GLOBAL_HQ: u32 = 0xAA1A_2866;
    pub const POPULATION: u32 = 0xAA1A_2CCA;
    /// School-covered per mille of population (§5.7).
    pub const SCHOOL_COVERAGE: u32 = 0x4A8A_3FE5;
    /// Hospital-covered per mille (§5.7).
    pub const HOSPITAL_COVERAGE: u32 = 0x4A8A_3FE2;
    /// The nine age groups (§5.6).
    pub const AGE_GROUPS: usize = 9;
    /// The queue length set for every series at `Init` step 5.
    pub const QUEUE_LENGTH: u32 = 2;
}

/// The daily state machine's states (§5.1).
pub mod state {
    pub const STRIKES_AND_LISTS: u32 = 0;
    pub const PROXIMITY_LOW: u32 = 1;
    pub const PROXIMITY_MEDIUM: u32 = 2;
    pub const PROXIMITY_HIGH: u32 = 3;
    pub const POPULATION: u32 = 4;
    pub const SCHOOL_COVERAGE: u32 = 5;
    pub const SCHOOL_GRADING: u32 = 6;
    pub const HOSPITAL_COVERAGE: u32 = 7;
    pub const HOSPITAL_GRADING: u32 = 8;
    pub const HISTORY: u32 = 9;
    pub const IDLE: u32 = 10;
    pub const HISTOGRAM_RESET: u32 = 11;
    pub const POST_POPULATION: u32 = 12;
    pub const SEED_LOW: u32 = 13;
    pub const SEED_MEDIUM: u32 = 14;
    pub const SEED_HIGH: u32 = 15;
    /// The cycle order of §5.1.
    pub const ORDER: [u32; 16] = [0, 13, 1, 14, 2, 15, 3, 11, 4, 12, 5, 6, 7, 8, 9, 10];
}

/// Wealth levels (§2): R$ = 1, R$$ = 2, R$$$ = 3, array indices 0..2.
pub mod wealth {
    pub const LOW: usize = 0;
    pub const MEDIUM: usize = 1;
    pub const HIGH: usize = 2;
    pub const COUNT: usize = 3;
    /// The wealth numbers the interface uses.
    pub const NUMBERS: [u32; COUNT] = [1, 2, 3];
}

/// Grid instance IDs (§2).
pub mod instance {
    pub const PROXIMITY: [u32; 3] = [0x49D5_BC96, 0x49D5_BCA6, 0x49D5_BCB7];
    pub const POPULATION: [u32; 3] = [0x69D5_C2A0, 0x69D5_C2A1, 0x69D5_C2A2];
    pub const AGE: u32 = 0x69D5_C303;
    pub const SCHOOL_COVERAGE: [u32; 7] = [
        0xAA8E_1DC1,
        0x8A8D_F4B6,
        0xCA8E_449E,
        0xAA8D_F4BD,
        0x8A8D_F4C4,
        0xAA8D_F4D4,
        0x69D5_C30D,
    ];
    pub const HOSPITAL_COVERAGE: [u32; 3] = [0xAA8E_1DE4, 0x8A8D_F6FC, 0x69D5_C317];
    pub const SCHOOL_GRADE: u32 = 0x69D5_C324;
    pub const HOSPITAL_GRADE: u32 = 0x69D5_C330;
    pub const EQ: [u32; 3] = [0x69D5_C2D0, 0x69D5_C2D1, 0x69D5_C2D2];
    pub const HQ: [u32; 3] = [0x69D5_C2E0, 0x69D5_C2E1, 0x69D5_C2E2];
    /// `GetAverageEQGrid` / `GetAverageHQGrid` (§6.2).
    pub const AVERAGE_EQ: u32 = 0x69D5_C351;
    pub const AVERAGE_HQ: u32 = 0x69D5_C360;
}

/// The class IDs of the grids of §2 (§11.3: a replacement must hand out game
/// grids of these element types).
pub mod grid_class {
    pub const U8: u32 = 0x49B9_E602;
    pub const U16: u32 = 0x49B9_E604;
    /// The growth developers' population maps (REH §4.7, growth-developer §1).
    pub const I16: u32 = 0x49B9_E605;
    pub const F32: u32 = 0x49B9_E60A;
    /// The temporary proximity grid is constructed with this ID (§2); it is
    /// never saved.
    pub const TEMPORARY: u32 = 0x49B9_E600;
}

/// The proximity grid's tract size (§3): one cell.
pub const PROXIMITY_TRACT_SIZE: i32 = 1;
/// The constructed population and EQ tract sizes (§3), replaced at
/// new-city `Init` by the R$ developer's population-map tract size.
pub const DEFAULT_TRACT_SIZE: i32 = 4;

/// Growth developer types (§4.1 step 1), in wealth order.
pub const DEVELOPERS: [u32; 3] = [0x1010, 0x1020, 0x1030];
/// The demand node held but never read (§4.1 step 1): `GetDemand(0x1000, 0)`.
pub const DEMAND_NODE: (u32, u32) = (0x1000, 0);

/// Budget departments (§4.1 step 9, REH §4.2).
pub mod department {
    pub const GROUP: u32 = 0x6A35_7B7F;
    pub const HEALTH: u32 = 0x0918_8F42;
    pub const HEALTH_SECONDARY: u32 = 0xAA53_8CB3;
    pub const EDUCATION: u32 = 0x0918_8F4C;
    pub const EDUCATION_SECONDARY: u32 = 0x4A53_8CC6;
    pub const NAME_GROUP: u32 = 0x6A3F_F01C;
    pub const HEALTH_NAME: u32 = 0xCA49_3A56;
    pub const HEALTH_SECONDARY_NAME: u32 = 0xCA53_8D98;
    pub const EDUCATION_NAME: u32 = 0x6A49_3A66;
    pub const EDUCATION_SECONDARY_NAME: u32 = 0x8A53_8DAC;
}

/// Pollution type indices passed to `GetPollutionValue` (§5.5 step 1).
pub mod pollution_kind {
    pub const AIR: u32 = 0;
    pub const WATER: u32 = 1;
    pub const GARBAGE: u32 = 2;
}

/// The building map key of a residential wealth level (§5.3 step 1):
/// `GetBuildingMapKey(purpose 1, w)` = `(purpose << 2) + w + 1`.
pub const BUILDING_PURPOSE_RESIDENTIAL: u32 = 1;

/// The death's-door visual effect's name (§5.5 step 2).
pub const BUZZARDS_EFFECT: &str = "buzzards";

/// What [`ResidentialHost::create_visual_effect`] gave (§5.5 step 2, REH §4.7
/// step 3, RG1 answered: the bracket "not created" differed, the game creates
/// the buzzards through the city's effects manager, slot 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatedEffect {
    /// The created effect (one reference): positioned, started, released;
    /// both draws, the tract's flag set.
    Effect(u64),
    /// The host cannot show effects (standalone, tests, a read-only shadow,
    /// a switched-off glue). *[choice]* (REH §4.7 step 3): treated as
    /// created, as the original with shipped data (whose effect directory
    /// defines `buzzards`): both draws and the flag, nothing positioned or
    /// started.
    NotShown,
    /// The create returned no effect (an unknown name): only the first draw
    /// was made, the flag stays clear and the next wealth level may draw
    /// again.
    Failed,
}

/// The occupant auto-budget bool property (§6.6, REH §2).
pub const PROP_AUTO_BUDGET: u32 = 0x6C05_FC4D;

/// The number of age histogram buckets (§3, §5.4).
pub const AGE_BUCKETS: usize = 20;

/// A message as `DoMessage` sees it (§4.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResidentialMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message.
    pub void1: Option<u64>,
}

impl ResidentialMessage {
    pub fn of_type(msg_type: u32) -> ResidentialMessage {
        ResidentialMessage {
            msg_type,
            ..ResidentialMessage::default()
        }
    }
}

/// The shared random stream (§3): the buzzards rolls (§5.5) and the monthly
/// strike rolls (REH §4.9) draw from one `cRZRandom`. The generator itself is
/// [`ResidentialRng`] (re-exported from `simulator`).
pub trait ResidentialRandom {
    /// `RandomDoubleRangeUniform(a, b)` (foundations §6.1).
    fn range_f64(&mut self, a: f64, b: f64) -> f64;
}

/// One education or health department's record: the saved scalars of §9.1
/// plus the two departments it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepartmentBlock {
    /// `0xE918946F` / `0xC91894A8`; loaded but never read (§7.2).
    pub f0: f32,
    pub max_funding: f32,
    pub strike_efficiency: f32,
    /// Compared in **days** (REH §4.9 *[quirk]*).
    pub strike_duration: i32,
    /// Compared with the staff ratio (REH §4.9 *[quirk]*).
    pub min_grade: f32,
    pub strike_flag: u8,
    pub strike_start: i32,
    pub init_funding: f32,
    /// The "a 100.0 field" of §9.1.
    pub hundred: f32,
    /// The strike message type (§7.2): `0x077ACCD2` health, `0x077ACCD0`
    /// education.
    pub id: u32,
    /// The primary and secondary department budgets (host tokens).
    pub department1: Option<u64>,
    pub department2: Option<u64>,
}

impl DepartmentBlock {
    /// The code defaults of §7.2 for one service.
    pub fn code_defaults(id: u32) -> DepartmentBlock {
        DepartmentBlock {
            f0: 0.6,
            max_funding: 1.2,
            strike_efficiency: 0.2,
            strike_duration: 10,
            min_grade: 0.9,
            strike_flag: 0,
            strike_start: 0,
            init_funding: 0.25,
            hundred: 100.0,
            id,
            department1: None,
            department2: None,
        }
    }

    /// `BeginStrike(dept, occ)` (REH §4.9): false if the flag is set;
    /// otherwise flag := 1, start := `today` and true (the caller then posts
    /// the strike message with data2 = 1).
    pub fn begin_strike(&mut self, today: u32) -> bool {
        if self.strike_flag != 0 {
            return false;
        }
        self.strike_flag = 1;
        self.strike_start = today as i32;
        true
    }

    /// `EndStrike(dept, today − start)` (REH §4.9): `None` if the flag is
    /// clear; otherwise flag := 0, start := 0 and the strike's length in days
    /// (`today − start`, 32-bit wrapping), which the caller posts as data3
    /// of the end-of-strike message (data2 = 0).
    pub fn end_strike(&mut self, today: u32) -> Option<u32> {
        if self.strike_flag == 0 {
            return None;
        }
        let length = today.wrapping_sub(self.strike_start as u32);
        self.strike_flag = 0;
        self.strike_start = 0;
        Some(length)
    }
}

/// The occupant groups of the state-0 distribution (REH §4.1), in
/// coverage-grid order: a pending building goes to the **first** group it
/// belongs to.
pub mod group {
    /// K-6 school, high school, `0x1514`, college, `0x1505`, museum,
    /// `0x151B`.
    pub const SCHOOL: [u32; 7] = [0x150F, 0x1510, 0x1514, 0x1504, 0x1505, 0x1506, 0x151B];
    pub const HOSPITAL: [u32; 3] = [0x1512, 0x1513, 0x151C];
}

/// Everything the residential simulator reads from, or does to, the rest of
/// the game (§11.1), with the education and health half's needs in
/// [`EducationHealthHost`]. Objects are host tokens. Re-entrancy: callbacks
/// must not call back into the residential simulator.
pub trait ResidentialHost: GridProvider<u8> + GridProvider<u16> + GridProvider<f32> + EducationHealthHost {
    /// Live arithmetic context, not saved state (foundations §18.4).
    fn invocation_precision(&self) -> crate::x87::Precision {
        crate::x87::Precision::Bits24
    }

    // ---- lifecycle (§4.1, §4.2) ----
    /// `Init` step 1: take and `AddRef` the city, the building development
    /// simulator and the occupant manager, each only if not already held.
    /// `Init` does not test the result (§4.1: only the demand node is tested).
    fn acquire_services(&self);
    /// `Init` step 1: the three growth developers through
    /// `BDS.GetDeveloper(0x1010 / 0x1020 / 0x1030)`, each only if not held.
    fn acquire_developers(&self);
    /// `Init` step 1: the demand node `GetDemand(0x1000, 0)` from the demand
    /// simulator, if not held. **`Init` fails without it**; the node is only
    /// held, never read.
    fn acquire_demand_node(&self) -> bool;
    /// `Init` step 2: take the message server. False when there is none. The
    /// simulator calls this only when it does not hold one (it tracks that
    /// itself, [`ResidentialState::message_server_held`]), and subscribes the
    /// two occupant messages right after a successful take.
    fn acquire_message_server(&self) -> bool;
    /// `Init` step 3: take the budget simulator if not held.
    fn acquire_budget_simulator(&self) -> bool;
    /// `Init` step 4: the simulator is there; false when there is none
    /// (`Init` then fails). RG18 (answered): the original borrows it (no
    /// `AddRef`, no member; `Shutdown` asks the city again for
    /// `RemoveAgent`), so the host need not hold it.
    fn has_simulator(&self) -> bool;
    /// `Init` step 4: `AddAgent(messageTarget, level, "cSC4ResidentialSimulator", 2)`
    /// for each level of [`AGENT_LEVELS`], in that order.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `Shutdown`: `RemoveAgent(target)` on the simulator.
    fn remove_agent(&self);
    /// Message server slots 5 / 6 with the message-target sub-object.
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// `Shutdown`, after the two unsubscriptions: release the message server
    /// and the budget simulator (§4.2). The departments follow.
    fn release_message_server_and_budget(&self);
    /// `Shutdown`, after the grids: release the demand node, the city, the
    /// BDS, the occupant manager and the developers (§4.2). The occupant
    /// lists follow.
    fn release_services(&self);
    /// `Init` step 5: `SetHistoryDataQueueLength(id, n)`.
    fn set_history_queue_length(&self, series: u32, length: u32);
    /// `Init` step 9: `CreateDepartmentBudget(id, 0x6A357B7F)`, `AddRef`,
    /// `SetDepartmentName(0x6A3FF01C, name)`,
    /// `SetMaxAllowedFundingPercentage(max)` and
    /// `SetFundingPercentage(initial, 0)`. The secondary departments also
    /// create line item 1 (local) and clear its display flag 1 (REH §4.2).
    fn create_department(&self, id: u32, name: u32, max_funding: f32, init_funding: f32) -> Option<u64>;
    /// `Shutdown` / a department replaced by `Read`: `Release`.
    fn release_department(&self, department: u64);
    /// `Shutdown` on a grid before the simulator forgets it (§4.2). The
    /// temporary grid is only released (dropped), not shut down.
    fn shutdown_grid_u8(&self, grid: &Rc<dyn TractGrid<u8>>);
    fn shutdown_grid_u16(&self, grid: &Rc<dyn TractGrid<u16>>);
    fn shutdown_grid_f32(&self, grid: &Rc<dyn TractGrid<f32>>);
    /// `AddRef` / `Release` on a held occupant (§4.1, REH §4.1).
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- resources and tuning (§7) ----
    /// The residential tuning exemplar (type `0x6534284A`, group
    /// `0xE7E2C2DB`, instance `0x69189C84`), if it loads. Fetched on every
    /// call: §7.1 reads it at `Init` and at the first state-4 call of each
    /// cycle, and §7.3 fetches it afresh for every life-expectancy and
    /// workforce query.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;

    // ---- the monthly pass's inputs (§5.3, §5.5) ----
    /// BDS slot 41 `GetBuildingMap()`: the `uint8` grid of building keys.
    fn building_map(&self) -> Option<Rc<dyn TractGrid<u8>>>;
    /// BDS slot 42 `GetBuildingMapKey(purpose, wealth)` (AL).
    fn building_map_key(&self, purpose: u32, wealth: u32) -> u8;
    /// Growth developer slot 15 `GetPopulationMap()` of developer `dev_type`
    /// (`0x1010` / `0x1020` / `0x1030`): a `cISC4SimGrid<int16>` whose 16
    /// bits this simulator reads as unsigned (§5.5 step 3).
    fn developer_population_map(&self, dev_type: u32) -> Option<Rc<dyn TractGrid<i16>>>;
    /// Pollution slot 6 `GetPollutionValue(type, x, z)` (**AX**, the source
    /// grids) at a cell.
    fn pollution_value(&self, kind: u32, cell_x: i32, cell_z: i32) -> u16;
    /// Simulator slot 6 `GetSimDate(year*, month*, day*, dayOfYear*,
    /// weekDay*)`: only the month is used (§5.5 step 5).
    fn sim_month(&self) -> u32;
    /// "today": `city.GetSimulator().GetSimDate().DayNumber()` (REH §4.9).
    fn sim_day_number(&self) -> u32;
    /// `cISTETerrain` slot 13 `GetAltitude(x, z)` (ST0) at a world position.
    fn altitude(&self, x: f32, z: f32) -> f32;

    // ---- visual effects (§5.5 step 2) ----
    /// Effects manager slot 7 `CreateVisualEffect(name, effect*&)` (the
    /// city's, city slot 85; RG1): the created effect, [`CreatedEffect::Failed`]
    /// when it returns no effect (the out value is what the original tests),
    /// or [`CreatedEffect::NotShown`] when the host cannot show effects.
    fn create_visual_effect(&self, name: &str) -> CreatedEffect;
    /// `cISC4VisualEffect` slot 8 `SetSourceTransform` with an identity
    /// rotation, this translation and scale 1.
    fn set_effect_transform(&self, effect: u64, translation: [f32; 3]);
    /// Slot 3 `Start(0)`, then slot 2 `Release`.
    fn start_and_release_effect(&self, effect: u64);

    // ---- messages (§5.7, §8, REH §5) ----
    /// `RecordHistoryData(id, value)`: create a `cRZMessage2Standard` of type
    /// [`msg::RECORD_HISTORY`] with data1 = `series` and data2 = `value`,
    /// **send** it synchronously and release it. Nothing happens without a
    /// message server.
    fn record_history(&self, series: u32, value: i32);
    /// `MessagePost(m, 0)` of a strike message (REH §5): type `msg_type`
    /// ([`msg::SCHOOL_STRIKE`] / [`msg::HEALTH_STRIKE`]), void1 and the
    /// IGZUnknown = `occupant` (a strike start), data2 = 1 (start) or 0
    /// (end), data3 = the strike's length in days (end).
    fn post_strike_message(&self, msg_type: u32, occupant: Option<u64>, data2: u32, data3: u32);

    // ---- occupants (§6.6, REH §4.1) ----
    /// `QueryInterface(void1, 0x87DDA3A9)` for `cISC4BuildingOccupant`
    /// (§4.3): the building occupant, if it is one.
    fn occupant_as_building(&self, occupant: u64) -> Option<u64>;
    /// `cISC4Occupant` slot 22 `IsOccupantGroup(group)` (REH §4.1 state-0
    /// distribution).
    fn is_occupant_group(&self, occupant: u64, group: u32) -> bool;
    /// A bool property of the occupant's property holder (occupant slot 6);
    /// `None` when absent.
    fn occupant_bool_property(&self, occupant: u64, id: u32) -> Option<bool>;
    /// Sets a bool property through the occupant's property holder; the
    /// property helper's result (slot 57's EAX).
    fn set_occupant_bool_property(&self, occupant: u64, id: u32, value: bool) -> bool;
    /// How a save refers to an occupant or a department (§9.1).
    fn object_ref(&self, token: u64) -> RefValue;
    /// The token of a reference read with `iid`; the reference the stream
    /// handed over is transferred to the simulator, and when this returns
    /// `None` the host drops it.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}

/// What the education and health half (REH, `education_health.rs`) reads
/// from, or does to, the rest of the game: the budget simulator's items,
/// departments and line items (budget.md), the occupants' property holders
/// and lots, the ordinance simulator and the automata controller manager.
/// Objects are host tokens; a department is the token
/// [`EducationHealthHost::department_budget`] returns. Re-entrancy: callbacks
/// must not call back into the residential simulator.
pub trait EducationHealthHost {
    // ---- budget (REH §4.1, §4.2; budget.md) ----
    /// `budget.GetBudgetItemForPurpose(holder, purpose)`: the occupant's item
    /// for a purpose, or `None` when the call fails.
    fn budget_item(&self, occupant: u64, purpose: u32) -> Option<BudgetItem>;
    /// `budget.ChangeBudgetItemLine(holder, index, line)` (REH §4.1: a line of
    /// 0 becomes the building type).
    fn change_budget_item_line(&self, occupant: u64, index: u32, line: u32);
    /// `budget.GetDepartmentBudget(id)`: `None` for a null department.
    fn department_budget(&self, department_id: u32) -> Option<u64>;
    /// `*dept.GetFundingPercentage()` (a fraction), or `None` when the
    /// department's funding cannot be read. REH §4.2 (RG10, answered: the
    /// bracket "1.0" differed): the original never substitutes 1.0 for a
    /// missing department, it leaves the output alone, so a host that cannot
    /// read the value reports "missing" and the core leaves the output as
    /// the caller set it.
    fn department_funding(&self, department: u64) -> Option<f32>;
    /// `dept.IsLocallyFundedObjectInDepartment(holder, purpose)`.
    fn is_locally_funded(&self, department: u64, occupant: u64, purpose: u32) -> bool;
    /// `dept.AddLocallyFundedObject(holder, purpose)` /
    /// `RemoveLocallyFundedObject(holder, purpose)`.
    fn add_locally_funded(&self, department: u64, occupant: u64, purpose: u32);
    fn remove_locally_funded(&self, department: u64, occupant: u64, purpose: u32);
    /// `dept.SetLocalFundingPercent(holder, fraction, purpose)` (slot 19).
    fn set_local_funding(&self, department: u64, occupant: u64, fraction: f32, purpose: u32);
    /// `dept.GetLineItem(line)`: `None` for a null line item.
    fn line_item(&self, department: u64, line: u32) -> Option<u64>;
    /// `dept.CreateLineItemForBuildingType(line, local)`.
    fn create_line_item_for_building_type(&self, department: u64, line: u32, local: bool) -> Option<u64>;
    /// `dept.RemoveLineItem(line)`.
    fn remove_line_item(&self, department: u64, line: u32);
    /// The line item's 64-bit count (line item slots 8 / 9).
    fn line_item_count(&self, line_item: u64) -> i64;
    fn set_line_item_count(&self, line_item: u64, count: i64);
    /// `line.SetDisplayFlag(flag, on)`.
    fn set_line_item_display_flag(&self, line_item: u64, flag: u32, on: bool);

    // ---- occupants (REH §2, §4.1, §4.4–§4.6) ----
    /// A property of the occupant's property holder (`AsPropertyHolder`), as
    /// stored; `None` when absent. The getters read its first element (u32 /
    /// f32) or the whole value (u32 arrays, curves).
    fn occupant_property(&self, occupant: u64, id: u32) -> Option<Variant>;
    /// Creates or sets a u32 / f32 property of the occupant (§2's
    /// per-building state and the strike day).
    fn set_occupant_u32(&self, occupant: u64, id: u32, value: u32);
    fn set_occupant_f32(&self, occupant: u64, id: u32, value: f32);
    /// `b.GetBuildingType()`.
    fn building_type(&self, building: u64) -> u32;
    /// The building's slot-11 value, first u32 (REH §4.1 step 4).
    fn building_slot11(&self, building: u64) -> u32;
    /// `lotManager.GetOccupantLot(occ)` → `lot.GetCenter()` as (x, z); `None`
    /// when the occupant has no lot.
    fn occupant_lot_center(&self, occupant: u64) -> Option<(f32, f32)>;
    /// `occ.GetPosition()` as (x, z).
    fn occupant_position(&self, occupant: u64) -> (f32, f32);

    // ---- ordinances (REH §4.3) ----
    /// `GetOrdinanceOnCount` / `GetOrdinanceOnArray`: the IDs of the
    /// ordinances that are on, in ascending ID order.
    fn ordinances_on(&self) -> Vec<u32>;
    /// `GetOrdinanceByID(id)` → `GetMiscProperties()`; `None` skips the
    /// ordinance.
    fn ordinance_properties(&self, id: u32) -> Option<Rc<PropertyHolder>>;

    // ---- automata (REH §4.9) ----
    /// `city.GetAutomataControllerManager()`:
    /// `AddControllersFromOccupantGroup(name, occ, 0)` on a strike start and
    /// `RemoveControllers(name, occ)` on its end.
    fn add_controllers(&self, name: &str, occupant: u64);
    fn remove_controllers(&self, name: &str, occupant: u64);
}

/// The occupant lists of §3 / §9.1, in save order. All are host tokens; the
/// simulator holds one reference per entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OccupantLists {
    /// Buildings added since the last state 0 (REH §4.1).
    pub pending_school: Vec<Option<u64>>,
    pub pending_hospital: Vec<Option<u64>>,
    /// The "other list" of §9.1.
    pub other: Vec<Option<u64>>,
    /// The seven school groups, in coverage-grid order (REH §4.1).
    pub school_groups: [Vec<Option<u64>>; 7],
    /// The three hospital groups (REH §4.1).
    pub hospital_groups: [Vec<Option<u64>>; 3],
    /// Health-affecting buildings (REH §4.1 step 3).
    pub health_affecting: Vec<Option<u64>>,
}

impl OccupantLists {
    /// Every list, in the record's order (§9.1).
    pub fn in_save_order(&mut self) -> Vec<&mut Vec<Option<u64>>> {
        let mut v: Vec<&mut Vec<Option<u64>>> = Vec::with_capacity(14);
        v.push(&mut self.pending_school);
        v.push(&mut self.pending_hospital);
        v.push(&mut self.other);
        for l in self.school_groups.iter_mut() {
            v.push(l);
        }
        for l in self.hospital_groups.iter_mut() {
            v.push(l);
        }
        v.push(&mut self.health_affecting);
        v
    }

    /// The same, immutably.
    pub fn save_order(&self) -> Vec<&Vec<Option<u64>>> {
        let mut v: Vec<&Vec<Option<u64>>> = Vec::with_capacity(14);
        v.push(&self.pending_school);
        v.push(&self.pending_hospital);
        v.push(&self.other);
        for l in &self.school_groups {
            v.push(l);
        }
        for l in &self.hospital_groups {
            v.push(l);
        }
        v.push(&self.health_affecting);
        v
    }
}

/// The query-tool record of REH §4.11 (slots 26 / 27), the fields the spec
/// gives. The leading `cRZString` is not filled by those methods.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QueryData {
    pub capacity: u32,
    pub effective_capacity: u32,
    /// `capacity / divisor` (floor).
    pub capacity_per_staff: u32,
    /// `ceil(effCap / divisor)` for schools, `floor` for hospitals *[quirk]*.
    pub effective_per_staff: u32,
    /// Served population; `0xFFFFFFFF` reads as 0.
    pub served: u32,
    pub grade: f32,
    pub staff_funding: f32,
    pub staff_cost: i64,
    pub vehicle_funding: f32,
    pub vehicle_cost: i64,
}
