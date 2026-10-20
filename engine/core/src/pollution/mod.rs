// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The pollution and garbage subsystem (`docs/specs/pollution.md`): the
//! pollution simulator (`cISC4PollutionSimulator`, class `0x8990C065`).
//! Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`PollutionSim`]: the state of §3, the lifecycle of §4.1, building
//!   changes (§4.2), disaster radiation (§4.3), the daily tract visit (§4.4,
//!   §4.5), the garbage flow (§4.6), efficiency (§4.7), the end of cycle
//!   (§4.8), the monthly simulation (§4.9), the visual effects and worst
//!   tracts (§4.10), prop requests (§4.11), messages (§5), the save record
//!   (§6) and the interface slots of §8.1 / §8.4.
//! - [`PollutionState`]: every field of §3 as plain data (snapshots, tests).
//! - [`Tuning`]: the exemplar of §7.1 with its code defaults and the curve
//!   patterns (foundations §4.1); [`shipped_tuning`] holds the shipped values.
//! - [`PollutionHost`]: everything the simulator reads from, or does to, the
//!   rest of the game. Grids come from its [`GridProvider<i16>`] supertrait
//!   (in game: the game's own `cSC4SimGrid<int16>` objects, simgrid §6.4).
//!   Occupants, deals, effects and the department are host objects named by
//!   opaque `u64` tokens.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - Tuning is held per simulator, not process-wide (§3.3: only matters with
//!   two cities at once). The two decay curves `0x0911E2E8` / `0x0911E2E9`
//!   are rebuilt as the constant 1 before each read ("default" pattern);
//!   every other tuning curve is emptied first ("empty" pattern); `costPerUnit`
//!   keeps its value when absent ("keep"). The per-occupant curves
//!   `0x8A1E07EE` / `0x0911E2E7` are read fresh per call and skipped when
//!   absent; the efficiency curve `0x6911E2DF` is read fresh at each end of
//!   cycle (absent or empty → efficiency 0). Curve properties are read with
//!   [`crate::props::curve`] (foundations §4.1, §4.3): a single value (a
//!   scalar or a one-element array) counts as absent, so the caller keeps its
//!   default (the decay curves stay constant 1); a present property without
//!   points is an empty curve, which evaluates to 0.
//! - Float arithmetic follows foundations §18 (24-bit x87): every float
//!   add, subtract, multiply and divide is one `f32` rounding; an integer
//!   operand (a counter, a capacity, a deal amount) enters that operation
//!   unrounded (computed in `f64`, rounded once to `f32`), and so does the
//!   `f64` `recycled` counter, whose sums are rounded to `f32` before being
//!   stored back. The effect strength (§4.10) and the prop share (§4.11)
//!   multiply the average by the `float32` constant 1/32767 (one rounding;
//!   not a division). `0.1f` (deals, §4.4.2) and `0.01f` (efficiency §4.7,
//!   slot 55) are `float32` literals *[compat]*, so funding 90 % gives
//!   efficiency `0x3F4CCCCC` and a base capacity of 1000 gives 799. The deal
//!   share is rounded with the 1.5·2²³ idiom (crime.md §6 `rhe`, exact only
//!   up to 2²²). Every `trunc` is the CRT `_ftol` (foundations §18.1,
//!   [`crate::x87::ftol`]): an `int64` truncation whose low bits are kept
//!   (NaN, ±∞ and |x| ≥ 2⁶³ give 0), wrapped to the field's width where the
//!   spec says "held as int16". Every other `roundHalfEven` site (§4.12:
//!   garbage produced, water treated, valve share, landfill limit and amount,
//!   prop share, slot 53) is `rhe` too, so the landfill limit follows the
//!   formula above 2²² (839 shipped tiles); the two `Dispose` floors are
//!   `rhe(g − 0.5f)` plus the exact `float(n + 1) = g` correction.
//! - Arrays shorter than four elements (centre, outer, radii) read missing
//!   elements as 0 (§4.2.2 states this one).
//! - Zone types ≥ 16 use the multipliers 1.0 (the day reads 16 entries).
//! - `totalUncollected ≥ cur` (§4.4.4 step 6) compares unsigned, as C does
//!   for `uint32 ≥ int32`.
//! - `Dispose` (§4.6.1): the landfill room `capacity − usage` and the
//!   decomposition room `decompositionAmount − decomposed` are signed 64-bit
//!   differences (a usage above capacity gives no room).
//! - The waste-to-energy capacity added on insert / subtracted on removal
//!   (§4.2.3) is the base property `0x48EBD0B1`.
//! - The recycling re-order drops (and releases) a member whose capacity
//!   query fails.
//! - The water "stop at 1" bounds use the water value grid's tract counts.
//! - Visits per day are capped at the source tract count (a hostile save may
//!   store any `tractsPerDay`); with no source tracts nothing is visited.
//! - `GetPollutionGrid(type ≥ 4)` returns no grid (PG13: the original reads
//!   past its grid table, which is never a grid, and no caller passes such a
//!   type); `GetPollutionValue` with types 4–7 reads the value grids (the
//!   quirk), types ≥ 8 return 0.
//! - Missing grids (a save whose grid reference does not resolve) read 0 and
//!   ignore writes. `Init` creates missing grids only for a new city (§4.1).
//! - Without a department, the funding fraction reads 1.0 (100 %) and the
//!   monthly cost is still computed and remembered but booked nowhere.
//! - `GetRequestedShare` with an empty or inverted rectangle returns 0; the
//!   cell count is the 32-bit product `w × h` (0 → 0).
//! - The worst-tract reset (§4.10 step 1) resets the values only; the stored
//!   coordinates stay until a block beats 0.
//! - Water effect positions: `8·bx·cw + U(−2cw, 2cw)` is summed in `f64`
//!   and rounded once to `f32`.
//! - The effect arrays are `(CellCountX/8) × (CellCountZ/8)` with both
//!   counts clamped to `0 ..= 1024`; cell counts read from the host are
//!   clamped to `0 ..= 8192` for loops over cells.
//! - `Read` parses the whole record first and changes nothing when the
//!   version is rejected or the stream fails (references already taken are
//!   released); `loaded` is set only by a successful read (§6 *[choice]*,
//!   spec test 25). A null grid reference reads as no grid (foundations
//!   §2.7). Null or unresolvable list references are skipped, where the
//!   original list reader appends a null entry (foundations §2.7): a list
//!   holding null crashes the original's first day, and the lists here hold
//!   host tokens only. The department reference is only replaced (and the
//!   old one released) by a successful read.
//! - The department created at `Init` is `AddRef`ed (the budget returns a
//!   borrowed pointer) and released at `Shutdown`; a loaded one comes with
//!   the reference the stream handed over.
//! - The four `effectsVisible` / worst / effect-iterator fields are not
//!   saved (§6); the effect iterator lives in the simulator and its one
//!   `rand()` per refresh comes from the simulator's own [`CrtRand`]
//!   (seeded 1, foundations §12.6), the same generator the tract iterator
//!   uses.
//! - The visual-effect generator is a `cRZRandomSuper` seeded by the host
//!   ([`PollutionSim::new`]); foundations §6.2 seeds it from the wall clock.
//!   It is a file-local static of the original, seeded once per process
//!   (RN3): [`PollutionSim::process_wide`] (the C ABI's create) shares the
//!   process-wide one of [`crate::seeding::process_super`], whose stream
//!   continues across city loads.
//!
//! [`CrtRand`]: crate::rng::CrtRand

mod daily;
mod effects;
mod record;
mod simulator;
#[cfg(test)]
mod tests;
mod tuning;

pub use simulator::{
    Counters, Grids, PollutionSim, PollutionState, Remaining, WorstTract, EFFECT_KINDS, GRID_INSTANCES,
    GRID_TRACT_SIZES, G_AIR_SRC, G_AIR_VAL, G_GARBAGE_SRC, G_GARBAGE_VAL, G_RAD_SRC, G_RAD_VAL, G_WATER_SRC,
    G_WATER_VAL, L_INC, L_REC, L_WTE, L_WTP,
};
pub use tuning::{id as tuning_id, shipped_tuning, Tuning, TUNING_KEY};

pub use crate::grid::{CellIterator, GridProvider, TractGrid};

/// Class and interface IDs (§1).
pub const CLSID_POLLUTION_SIMULATOR: u32 = 0x8990_C065;
pub const IID_POLLUTION_SIMULATOR: u32 = 0x8771_81EC;
pub const IID_PROP_REQUESTER: u32 = 0x89C4_98BE;
/// IIDs used when reading references (§6).
pub const IID_SIM_GRID: u32 = 0xC9BD_939A;
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
pub const IID_DEPARTMENT_BUDGET: u32 = 0x223E_2B67;

/// The record version written (§6); versions above it are rejected.
pub const RECORD_VERSION: u16 = 4;

/// Agent registration (§4.1 step 3.2): month first, then day, flags 2.
pub const AGENT_NAME: &str = "cSC4PollutionSimulator";
pub const AGENT_LEVELS: [u32; 2] = [6, 4];
pub const AGENT_FLAGS: u32 = 2;

/// The garbage value element's landfill flag (§2).
pub const LANDFILL_FLAG: u16 = 0x8000;
/// The landfill zone type (§4.4.4).
pub const ZONE_LANDFILL: u32 = 14;
/// The surface view level (§5.3).
pub const VIEW_SURFACE: u32 = 1;

/// Pollution type indices (§2).
pub mod kind {
    pub const AIR: usize = 0;
    pub const WATER: usize = 1;
    pub const GARBAGE: usize = 2;
    pub const RADIATION: usize = 3;
}

/// Messages (§5).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const TUNING_CHANGED: u32 = 0xCA1B_151C;
    pub const PRE_VIEW_LEVEL_CHANGE: u32 = 0xC6CD_BF1A;
    /// The five subscriptions, in the order of §4.1 step 3.3.
    pub const SUBSCRIPTIONS: [u32; 5] = [
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        POST_CITY_INIT,
        TUNING_CHANGED,
        PRE_VIEW_LEVEL_CHANGE,
    ];
    /// Posted (§5.2).
    pub const RADIATION_CHANGED: u32 = 0xDA6D_D35D;
    pub const AIR_CHANGED: u32 = 0xFA6D_D0BB;
    pub const WATER_CHANGED: u32 = 0x5A6D_D1AA;
    pub const GARBAGE_CHANGED: u32 = 0xDA6D_D2CB;
    pub const DEAL_STATUS: u32 = 0xEA41_8D8F;
}

/// Building properties (§7.2).
pub mod prop {
    pub const PREVIEW: u32 = 0x0292_44DB;
    pub const CENTRE: u32 = 0x2781_2851;
    pub const OUTER: u32 = 0xAA58_32F3;
    pub const RADII: u32 = 0x68EE_9764;
    pub const WATER_TREATMENT: u32 = 0xC8EB_D131;
    pub const INCINERATOR: u32 = 0xA8EB_CF0C;
    pub const WASTE_TO_ENERGY: u32 = 0x48EB_D0B1;
    pub const RECYCLING: u32 = 0x88ED_1059;
    pub const WTP_USED: u32 = 0x28EC_F510;
    pub const INC_USED: u32 = 0xE8EC_F641;
    pub const WTE_USED: u32 = 0x88EC_F76C;
    pub const WTP_USED_LAST: u32 = 0x08FC_DB69;
    pub const INC_USED_LAST: u32 = 0xA8FC_DBA8;
    pub const WTE_USED_LAST: u32 = 0x28FC_DBB0;
    pub const WTE_OUTPUT: u32 = 0x2781_2852;
    pub const WTE_MAX_OUTPUT: u32 = 0x08FD_1613;
    pub const POPULATION_SERVED: u32 = 0x68ED_1736;
    pub const CONDITION: u32 = 0xC910_B4CF;
    pub const AGING_RATE: u32 = 0xEA1C_F220;
    pub const AGING_T1: u32 = 0xEA1C_F221;
    pub const AGING_T2: u32 = 0xEA1C_F222;
    pub const CONDITION_CURVE: u32 = 0x8A1E_07EE;
    pub const MAINTENANCE_CURVE: u32 = 0x0911_E2E7;
    pub const BASE_COST: u32 = 0x2A58_D350;
    pub const GARBAGE_CAPACITY: u32 = 0x48EE_7A44;
}

/// Ordinance cumulative factors (§4.4.3): `(id, index)`.
pub mod factor {
    pub const TRAFFIC: u32 = 0x8A67_E378;
    pub const AIR: u32 = 0x08F7_9B8E;
    pub const WATER: u32 = 0xE8F7_9C8B;
    pub const GARBAGE: u32 = 0xE8F7_9C90;
    pub const ZONE_AIR: u32 = 0x8A67_E373;
    pub const ZONE_WATER: u32 = 0x8A67_E374;
    pub const ZONE_GARBAGE: u32 = 0x8A67_E376;
}

/// Neighbour deal types (§4.4.2).
pub mod deal {
    /// "Sell garbage disposal": this city accepts trash (import).
    pub const IMPORT: u32 = 0x4773_FFE5;
    /// "Buy garbage disposal": this city exports trash.
    pub const EXPORT: u32 = 0x4773_FFE4;
}

/// Budget IDs (§4.1, §4.2.4, §7.2).
pub mod budget {
    pub const DEPARTMENT: u32 = 0x6A42_A8F5;
    pub const DEPARTMENT_GROUP: u32 = 0x4A35_7EAF;
    pub const NAME_GROUP: u32 = 0x6A3F_F01C;
    pub const DEPARTMENT_NAME: u32 = 0xCA49_38F8;
    pub const MAX_FUNDING: f32 = 1.2;
    /// Line items (id, name instance), all not locally funded.
    pub const LINE_ITEMS: [(u32, u32); 3] = [(10, 0xCA57_FE96), (20, 0x4A57_FFF0), (30, 0x0A58_0005)];
    pub const LINE_MAINTENANCE: u32 = 10;
    pub const LINE_DISPOSAL: u32 = 20;
    pub const PURPOSE: u32 = 0xCA58_EDC7;
}

/// History series (§4.8, §4.9.2).
pub mod history {
    pub const AIR: u32 = 0x0A11_00E8;
    pub const WATER: u32 = 0xEA11_0130;
    pub const GARBAGE: u32 = 0xEA11_0134;
    pub const RADIATION: u32 = 0xEA11_0137;
    pub const AIR_PLUS_RADIATION: u32 = 0xEA11_0138;
    pub const PRODUCED: u32 = 0xEA11_0158;
    pub const IMPORTED: u32 = 0x8AB2_F258;
    pub const EXPORTED: u32 = 0x6AB2_F260;
    pub const RECYCLED: u32 = 0xEA11_016F;
    pub const INCINERATED: u32 = 0xEA11_0197;
    pub const WTE: u32 = 0xEA11_01A1;
    pub const LANDFILLED: u32 = 0xEA11_01AA;
}

/// Visual effects (§4.10).
pub mod effect {
    pub const ID: u32 = 0x0A78_747A;
    /// Array / kind order: water 1, air 2, radiation 3, garbage 4.
    pub const WATER: usize = 0;
    pub const AIR: usize = 1;
    pub const RADIATION: usize = 2;
    pub const GARBAGE: usize = 3;
    /// Shown when the strength is above this.
    pub const THRESHOLD: f32 = 0.05;
}

/// The prop requester (§4.11, §8.4).
pub const PROP_SATISFACTION_ID: u32 = 0xE9A4_300B;

/// A budget item as `GetBudgetItemForPurpose` reports it (budget spec).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BudgetItem {
    pub department_id: u32,
    pub line_item: u32,
    pub purpose: u32,
    pub cost: i64,
    pub index: u32,
    pub local_funding: f32,
}

/// One neighbour deal as `FindDeals` returns it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Deal {
    /// Host identity of the `cISC4NeighborDeal`.
    pub token: u64,
    /// `GetAmount()`.
    pub amount: i32,
    /// `GetAmountUsed()`.
    pub amount_used: i32,
}

/// Everything the pollution simulator reads from, or does to, the rest of
/// the game. Occupants, deals, visual effects and the department are host
/// objects named by tokens. Re-entrancy: callbacks must not call back into
/// the pollution simulator (a re-entrant call works on a copy of the state
/// that the outer call overwrites when it returns).
pub trait PollutionHost: GridProvider<i16> {
    // ---- city (§9) ----
    /// `cISC4City::CellCountX()` / `CellCountZ()`.
    fn cell_counts(&self) -> (i32, i32);
    /// `cISC4City::CellWidthX()` / `CellWidthZ()`.
    fn cell_widths(&self) -> (f32, f32);
    /// `cISC4City::PositionToCell(x, z)`.
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);
    /// The tuning exemplar (type `0x6534284A`, group `0xE7E2C2DB`, instance
    /// `0xC911E35B`), if loaded.
    fn tuning(&self) -> Option<std::rc::Rc<crate::property::PropertyHolder>>;
    /// Terrain altitude at a world position (`cISTETerrain` slot 13).
    fn altitude(&self, x: f32, z: f32) -> f32;

    // ---- simulator, messages, history ----
    /// `cISC4Simulator::AddAgent(self, level, name, flags)`.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `cISC4Simulator::RemoveAgent(self)` (all levels).
    fn remove_agent(&self);
    /// Subscribe / unsubscribe the simulator's message target.
    fn subscribe(&self, msg_types: &[u32]);
    fn unsubscribe(&self, msg_types: &[u32]);
    /// Posts a standard message (data1..4 = 0 except `data1`).
    fn post(&self, msg_type: u32, data1: u32);
    /// `cISC4HistoryWarehouse::RecordHistoryData(type, value)`.
    fn record_history(&self, series: u32, value: i32);
    /// `cISC4HistoryWarehouse::SetHistoryDataQueueLength(type, n)`.
    fn set_history_queue_length(&self, series: u32, length: u32);
    /// `cISC4PropDeveloper::AddPropRequester` / `RemovePropRequester` with
    /// the simulator's `cISC4PropRequester`. Added for a new city only (§4.1,
    /// PG5): the prop developer's own save restores it after a load.
    fn add_prop_requester(&self);
    fn remove_prop_requester(&self);

    // ---- other simulators ----
    /// `cISC4ZoneManager::GetZoneType(x, z)`.
    fn zone_type(&self, x: i32, z: i32) -> u32;
    /// The traffic simulator's air-polluting traffic map value at a cell
    /// (`GetAirPollutingTrafficMap()->GetCellValue(x, z)`).
    fn air_traffic(&self, x: i32, z: i32) -> u8;
    /// `cISC4OrdinanceSimulator::GetCumulativeFactor(id, index)`.
    fn cumulative_factor(&self, id: u32, index: u32) -> f32;
    /// `cISC4NeighborsSimulator::FindDeals(type)`, in list order.
    fn find_deals(&self, deal_type: u32) -> Vec<Deal>;
    /// `cISC4NeighborDeal::SetAmountUsed(v)`.
    fn set_deal_amount_used(&self, deal: u64, used: i32);
    /// `cISC4NeighborsSimulator::MarkDealsDirty()`.
    fn mark_deals_dirty(&self);
    /// `cISC4PowerSimulator::IsOccupantPowered(occ)`.
    fn is_occupant_powered(&self, occ: u64) -> bool;
    /// `cISC4ResidentialSimulator::GetPopulation()`.
    fn population(&self) -> i32;
    /// `cISC4LandfillDeveloper::GetLandfillUsage(&usage, &capacity)`.
    fn landfill_usage(&self) -> (u32, u32);
    /// `cISC4LandfillDeveloper::GetLandfillTileCount()`.
    fn landfill_tile_count(&self) -> u32;
    /// `GetLot(cellX, cellZ, false)->SetCondition(c)`; nothing without a lot.
    fn set_lot_condition(&self, cell_x: i32, cell_z: i32, condition: u32);

    // ---- occupants ----
    /// A property of the occupant (own or inherited from its exemplar).
    fn occupant_property(&self, occ: u64, id: u32) -> Option<crate::variant::Variant>;
    /// `AddProperty(id, value)` on the occupant's property holder.
    fn set_occupant_property(&self, occ: u64, id: u32, value: crate::variant::Variant);
    /// `RemoveProperty(id)`.
    fn remove_occupant_property(&self, occ: u64, id: u32);
    /// `cISC4Occupant::GetPosition()` (x, y, z).
    fn occupant_position(&self, occ: u64) -> [f32; 3];
    /// `cISC4Occupant::GetBoundingBox()`: (min corner, max corner).
    fn occupant_bounding_box(&self, occ: u64) -> ([f32; 3], [f32; 3]);

    // ---- host objects ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    /// How a save refers to a host object (occupant or department).
    fn object_ref(&self, token: u64) -> crate::stream::RefValue;
    /// The token of a reference read from a save, if it answers `iid`
    /// (occupant `0x26BB767E`, department `0x223E2B67`). The reference the
    /// stream handed over is transferred to the simulator; when it returns
    /// `None` the host drops that reference itself.
    fn resolve_ref(&self, r: &crate::stream::RefValue, iid: u32) -> Option<u64>;

    // ---- budget ----
    /// `CreateDepartmentBudget(id, group)` (borrowed); `None` on failure.
    fn create_department(&self, id: u32, group: u32) -> Option<u64>;
    /// `SetDepartmentName(group, instance)`.
    fn set_department_name(&self, dept: u64, group: u32, instance: u32);
    /// `SetMaxAllowedFundingPercentage(p)`.
    fn set_department_max_funding(&self, dept: u64, fraction: f32);
    /// `CreateLineItem(id, locallyFunded)` then `SetName(group, instance)`.
    fn create_line_item(&self, dept: u64, line: u32, locally_funded: bool, name_group: u32, name_instance: u32);
    /// `*GetFundingPercentage()` (a fraction).
    fn department_funding(&self, dept: u64) -> f32;
    /// `GetLineItem(line)->SetFullExpenses(v)` (nothing without the line).
    fn set_line_item_full_expenses(&self, dept: u64, line: u32, value: i64);
    /// `GetBudgetItemForPurpose(holder, purpose)`.
    fn budget_item_for_purpose(&self, occ: u64, purpose: u32) -> Option<BudgetItem>;
    /// `AddBudgetItemToBudget(holder, index)`.
    fn add_budget_item_to_budget(&self, occ: u64, index: u32);
    /// `RemoveBudgetItemFromBudget(holder, index)`.
    fn remove_budget_item_from_budget(&self, occ: u64, index: u32);
    /// `ChangeBudgetItemCost(holder, index, cost)`.
    fn change_budget_item_cost(&self, occ: u64, index: u32, cost: i64);
    /// `GetLineItemFromBudgetItem(item)->AddToFullExpenses(delta)`.
    fn add_to_budget_item_line_expenses(&self, item: &BudgetItem, delta: i64);

    // ---- visual effects (§4.10) ----
    /// `cISC4EffectsManager::CreateVisualEffect(id, kind)`.
    fn create_visual_effect(&self, id: u32, kind: u32) -> Option<u64>;
    /// `SetSourceTransform` with the identity rotation and this translation.
    fn set_effect_position(&self, effect: u64, position: [f32; 3]);
    /// `SetFloatParams(param, &value, 1)`.
    fn set_effect_param(&self, effect: u64, param: u32, value: f32);
    fn start_effect(&self, effect: u64, t: u32);
    fn stop_effect(&self, effect: u64, t: u32);
    fn is_effect_started(&self, effect: u64) -> bool;
}
