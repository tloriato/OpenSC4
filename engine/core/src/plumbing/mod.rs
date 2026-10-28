// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The plumbing (water) simulator (`docs/specs/plumbing.md`,
//! `cISC4PlumbingSimulator`, class `0x0990C075`). Section references (`§n`)
//! point at that spec.
//!
//! # Structure
//!
//! - [`PlumbingSim`]: the state of §3, the lifecycle of §4, the messages and
//!   occupant changes of §5, the pass of §6 (`DoIdle`), the monthly pipe
//!   funding, production and ageing of §7, the save record of §9 and the
//!   portable part of the interface slots of §10.
//! - [`PlumbingState`]: every field of §3 as plain data (snapshots, tests),
//!   with the [`Producer`] records of §5.3.
//! - [`Tuning`]: the "Utilities" exemplar values of §8 with their code
//!   defaults and curve patterns; [`Tuning::shipped_exemplar`] holds the
//!   shipped values.
//! - [`PlumbingHost`]: everything the simulator reads from, or does to, the
//!   rest of the game: the city and terrain, occupants (buildings, pipes),
//!   lots, the neighbour deals and connections, the budget, the ordinance and
//!   pollution simulators, the pipe-burst disaster manager, the history
//!   warehouse and message posting. Grids come from its
//!   [`GridProvider<u8>`] supertrait (in game: the game's own
//!   `cSC4SimGrid<uint8>` objects, simgrid §6.4, created through GZCOM as
//!   §12.2 requires). Game objects are opaque `u64` tokens.
//! - Pure helpers: [`trunc_i64`], [`pipe_cost`], [`nearest_water_ring`].
//!
//! Host tokens only need to stay valid for the duration of the call that
//! receives them, except for the objects the simulator holds (producer
//! occupants, pipes, the department), which it `add_ref`s / `release`s.
//! Host callbacks should not call back into the simulator: a re-entrant call
//! sees an empty state and its changes are lost, but nothing panics.
//!
//! # Choices made where the spec is open (`[choice]`), and spec answers
//!
//! - Time slicing (§6): states 6 and 7 run whole in one call by default (the
//!   spec allows deterministic slicing). [`PlumbingSim::set_slice_cells`]
//!   limits the cells per call instead (a "time-out" after that many cells),
//!   so the time-out paths can be exercised; results equal those of an
//!   uninterrupted pass either way. `DoIdle`'s millisecond budget is not
//!   used.
//! - Arithmetic follows foundations §18 (24-bit x87): every float operation
//!   is one `f32` rounding of the exact result, with integer operands
//!   entering unrounded ([`crate::x87::r24`]). The `0.01` of §7.2 is a
//!   `float` constant (the spec's own test 5 shows it); the SC4Percentage
//!   getter of §8 multiplies by `0.01f` too, so the shipped max funding 120
//!   is 1.1999999 (`0x3F999999`, PL6). Truncations to integers
//!   ([`trunc_i64`]) are the C runtime's `_ftol` ([`crate::x87::ftol`],
//!   foundations §18.1): `0x8000000000000000` for NaN and out-of-range
//!   values, whose low 32 bits (where the spec says `low32`) are 0. These
//!   are [compat], not choices. The pipe cost (§5.4)
//!   rounds half to even at every magnitude (the original's `1.5 · 2^23`
//!   trick is only valid below 2^22).
//! - Cell counts read from the city are clamped to `0 ..= MAX_TRACTS`
//!   (4096); a saved count above that ends the read there (the rest of the
//!   record keeps its old values; `Read` still returns true, §9). The flag
//!   map always has `countX · countZ` bytes (a loaded map of another size is
//!   reallocated). Flag map writes outside the city are ignored, except the
//!   pipe-burst message, which writes the raw index `z · countX + x` when it
//!   lies inside the map (§5.1: no bounds check).
//! - The flood queues hold full `i32` coordinates (the original's `u8` pairs
//!   limit cities to 256 cells, which the original never exceeds).
//! - A missing grid (the host could not make one, or a loaded reference did
//!   not resolve) reads 0 and ignores writes. Without a working grid the
//!   anchor cell of §5.3 comes from the host's `PositionToCell`.
//! - Without a department the funding fraction is 1.0 (100 %), the pipe cost
//!   is not booked and §10 `FindWaterBuildings` finds no locally funded
//!   producer. Without an ordinance simulator the factor is 1.0.
//! - Without a random generator (a month before the first `Init`) no bursts
//!   are drawn (0, and no draws).
//! - `GetDepartmentBudget` or `GetLineItem`/`CreateLineItemForBuildingType`
//!   failing in the building budget step (§5.3 step 4) skips the line-item
//!   steps; the occupant's base cost property is still written.
//! - Producer maintenance (§6.7 steps 2–3) is skipped for a producer without
//!   the curve property `0x0911E2E7`. Missing producer properties read 0
//!   (water used, cost per unit, condition, base cost); ageing thresholds
//!   default to 99 and the rate to 0 (§7.3).
//! - Per-occupant curves (`0x8A1E07EE`, `0x0911E2E7`) and tuning curves:
//!   a numeric property gives a curve (possibly empty, evaluating to 0); a
//!   non-numeric or single-value property counts as absent (foundations
//!   §4.1).
//! - `NearestWater` (§7.2, PL1, [compat]): the box is computed in int32
//!   arithmetic (a reach ≥ 2^31 is negative), an anchor outside it gives −1
//!   untested, and the spiral tests the box cells one by one in the
//!   original's clockwise ring order; the ring counter is the Chebyshev
//!   distance. [choice] Terrain cell counts are clamped to
//!   `0 ..= MAX_TRACTS` so the work stays bounded.
//! - State 3 always seeds (§6.2, PL3): the current producer is always null
//!   there, so the original's dead "network in progress" branch is not
//!   reproduced.
//! - The scanning iterator (§6.6, PL4, [compat]) rests done at
//!   `(0, countZ)` after a full scan (states 2 and 7), and saves taken
//!   between passes hold that position. State 2 resets it and scans the
//!   working grid with it. The plumbing module sets the rest position itself
//!   after the shared iterator reports the end, so it does not depend on
//!   where [`crate::grid::ScanIterator`] leaves `z`.
//! - Re-`Init` (§4.2, PL2): a second `Init` without `Shutdown` changes
//!   nothing (step 2 creates the generator only when none is held).
//!   [choice] `Shutdown` also clears the grids-initialized flag (and deletes
//!   the iterator, as the original does), so an `Init` after `Shutdown`
//!   rebuilds the grids, flag map, bitmap and iterator as on a new city
//!   (state 0, recompute set) instead of leaving them null as the original
//!   does (which then crashes).
//! - `Read` (§9): fields are stored as they are read and a failing stream
//!   stops the read (`Read` still returns true once the version matched). A
//!   producer record whose occupant reference is null or does not resolve is
//!   skipped; pipe references likewise, and `numPipes` is set to the number
//!   of pipes actually loaded. An iterator state byte above 2 reads as
//!   "done". A loaded department replaces (and releases) any current one.
//! - Reference count: `Release` from a count ≤ 1 calls `Shutdown`, returns 0
//!   and marks the object destroyed; the plugin then drops it (§1).
//! - The producer without a lot (§5.3 step 5.2, a [quirk]) keeps the leaked
//!   reference forever: the core `add_ref`s the occupant and records it in
//!   [`PlumbingState::leaked`] without ever releasing it.
//! - The visited bitmap is the shared [`crate::grid::Bitmap`] (power §2.1),
//!   whose layout is the original's [compat] (§2, PL5): `side² / 32` words,
//!   word `(side · z + x) >> 5`, bit `x & 31`. When the cell count is not a
//!   multiple of 32 the last cells have no bit and cannot be marked. Flood
//!   pushes (§6.2, §6.3) mark first and then test the bit, and [choice] a
//!   cell whose bit does not stick is not pushed, so every flood ends (those
//!   cells are neither flooded nor watered). For sides that are not
//!   multiples of 32, cells of one word with the same `x` share a bit and
//!   the floods are aliased, as in the original. The side is always 64, 128
//!   or 256 for shipped city sizes, where no cell shares a bit.

mod simulator;
#[cfg(test)]
mod tests;
mod tuning;
pub mod archive;

pub use simulator::{
    nearest_water_ring, pipe_cost, pipe_funding, trunc_i64, PipeFunding, PlumbingSim, PlumbingState, Producer,
    ProducerInfo, Totals, PLUMBING_RECORD_VERSION,
};
pub use tuning::{id as tuning_id, Tuning, TUNING_KEY};

use std::rc::Rc;

use crate::grid::{GridProvider, TractGrid};
use crate::property::PropertyHolder;
use crate::stream::RefValue;
use crate::variant::Variant;

pub use crate::grid::CellRect;

/// The class ID and the interfaces `QueryInterface` answers (§1).
pub const CLSID_PLUMBING_SIMULATOR: u32 = 0x0990_C075;
pub const IID_PLUMBING_SIMULATOR: u32 = 0x4778_2CCC;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const IID_UNKNOWN: u32 = 1;

/// IIDs used when reading references (§9).
pub const IID_SIM_GRID: u32 = 0xC9BD_939A;
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
pub const IID_NETWORK_OCCUPANT: u32 = 0x49C0_5B9F;
pub const IID_DEPARTMENT_BUDGET: u32 = 0x223E_2B67;
/// `cISC4BuildingOccupant` (§5.3 step 3) and `cISC4NeighborDeal` (§5.1).
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;
pub const IID_NEIGHBOR_DEAL: u32 = 0x0773_FEF1;
/// `cISC4Occupant`, which a replacement should QI for instead of the
/// original's `+4` adjustment (§12.2 item 4).
pub const IID_OCCUPANT_INTERFACE: u32 = 0x8725_4E2F;

/// Whether `QueryInterface(iid)` succeeds (§1). For any other IID the
/// plugin returns false and leaves `*ppv` unchanged.
pub fn answers_iid(iid: u32) -> bool {
    matches!(
        iid,
        IID_PLUMBING_SIMULATOR | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE | IID_UNKNOWN
    )
}

/// Agent registration (§4.1 step 3): month, day, idle, in that order.
pub const AGENT_NAME: &str = "cSC4PlumbingSimulator";
pub const AGENT_LEVELS: [u32; 3] = [6, 4, 10];
pub const AGENT_FLAGS: u32 = 2;

/// The grid instance IDs (§2).
pub const WORKING_GRID_INSTANCE: u32 = 0x49D5_BB8C;
pub const PUBLISHED_GRID_INSTANCE: u32 = 0x49D5_BBA1;

/// Messages (§5, §6.7).
pub mod msg {
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_IDLE: u32 = 0x6695_681A;
    pub const CONNECTIONS_READY: u32 = 0x6AC2_84F3;
    pub const INSERT_OCCUPANT: u32 = 0x99EF_1142;
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    pub const TUNING_RELOAD: u32 = 0xCA1B_151C;
    /// A pipe burst at a cell (data1 = x, data2 = z).
    pub const PIPE_BURST: u32 = 0xEA1F_0DCD;
    /// Neighbour deal status (data1 = 0: deal changed; 8: the pass ended).
    pub const DEAL_STATUS: u32 = 0xEA41_8D8F;
    /// Posted at the end of a pass: void1 = IGZUnknown = the simulator.
    pub const WATER_MAP_CHANGED: u32 = 0x6946_DC34;
}

/// The notifications `Init` subscribes, in order (§4.1 step 3).
pub const NOTIFICATIONS: [u32; 6] = [
    msg::INSERT_OCCUPANT,
    msg::REMOVE_OCCUPANT,
    msg::TUNING_RELOAD,
    msg::PIPE_BURST,
    msg::DEAL_STATUS,
    msg::CONNECTIONS_READY,
];

/// Occupant types (§5.2).
pub mod occupant_type {
    pub const BUILDING: u32 = 0x2781_28A0;
    pub const NETWORK: u32 = 0xC772_BF98;
}

/// Flag map bits (§2).
pub mod flag {
    pub const WEST: u8 = 0x01;
    pub const NORTH: u8 = 0x02;
    pub const EAST: u8 = 0x04;
    pub const SOUTH: u8 = 0x08;
    pub const PIPE: u8 = 0x10;
    pub const PRODUCER: u8 = 0x20;
    pub const CONSUMER: u8 = 0x40;
    /// What a removal / burst keeps (`& 0xE0`).
    pub const KEEP_ON_CLEAR: u8 = 0xE0;
}

/// Published grid value bits (§2).
pub mod value {
    pub const WATERED: u8 = 0x01;
    pub const LOT: u8 = 0x02;
    pub const PRODUCER_ONLY: u8 = 0x04;
    pub const PIPE: u8 = 0x08;
}

/// Network occupant flags (§5.2, §5.4, §7.1).
pub mod network {
    /// Network type 4 (pipe) for `ConnectsToNeighbor`.
    pub const PIPE_TYPE: u32 = 4;
    pub const DISTRESSED: u32 = 0x10000;
    pub const BURST: u32 = 0x20000;
    /// A non-final / preview network piece: ignored.
    pub const PREVIEW: u32 = 0x8000_0000;
}

/// Neighbour deals and connections (§6).
pub mod deal {
    pub const IMPORT: u32 = 0x4773_FFE2;
    pub const EXPORT: u32 = 0x4773_FFE3;
    /// `SetFlags(0x08, true)`: no connection to the network.
    pub const FLAG_NO_CONNECTION: u32 = 0x08;
    /// `SetFlags(0x20, true)`: the export was not fully delivered.
    pub const FLAG_NOT_DELIVERED: u32 = 0x20;
    /// The water connection type.
    pub const WATER_CONNECTION: u32 = 5;
}

/// Budget IDs (§4.1, §5.3, §5.4).
pub mod budget {
    pub const DEPARTMENT: u32 = 0xAA40_14B4;
    pub const DEPARTMENT_GROUP: u32 = 0x4A35_7EAF;
    pub const NAME_GROUP: u32 = 0x6A3F_F01C;
    pub const DEPARTMENT_NAME: u32 = 0x2A49_37D3;
    /// The pipe maintenance line item and its name.
    pub const PIPE_LINE: u32 = 2;
    pub const PIPE_LINE_NAME: u32 = 0x8A75_F61E;
    /// The line item a loaded department loses at `Init`.
    pub const REMOVED_LINE: u32 = 1;
    pub const PURPOSE: u32 = 0xCA58_C9D5;
    /// `SetDisplayFlag(2, true)` on a created building line item.
    pub const DISPLAY_FLAG: u32 = 2;
}

/// Occupant properties (§8).
pub mod prop {
    pub const PREVIEW: u32 = 0x0292_44DB;
    pub const CONSUMPTION: u32 = 0xC8ED_2D84;
    pub const CAPACITY: u32 = 0x88ED_3303;
    pub const SOURCE_TYPE: u32 = 0x48F2_3A7E;
    pub const CONDITION: u32 = 0xC910_B4CF;
    pub const CONDITION_CURVE: u32 = 0x8A1E_07EE;
    pub const MAINTENANCE_CURVE: u32 = 0x0911_E2E7;
    pub const BASE_COST: u32 = 0x2A58_D350;
    pub const AGING_RATE: u32 = 0xEA1C_F220;
    pub const AGING_T1: u32 = 0xEA1C_F221;
    pub const AGING_T2: u32 = 0xEA1C_F222;
    pub const WATER_USED: u32 = 0x08D9_763D;
    pub const COST_PER_UNIT: u32 = 0xEA1E_3C53;
}

/// The ordinance factor of §6.1.
pub const ORDINANCE_FACTOR: u32 = 0xA8F4_EB0C;
/// The pipe-burst disaster manager (§7.1).
pub const PIPE_BURST_MANAGER: u32 = 0x4A13_32E8;

/// History series (§6.7 step 6), in recording order.
pub mod history {
    pub const PRODUCED: u32 = 0x0A14_EBB4;
    pub const CONSUMED: u32 = 0xCA14_EBD0;
    pub const IMPORTED: u32 = 0xCAB2_F27B;
    pub const EXPORTED: u32 = 0x8AB2_F280;
    pub const PIPES: u32 = 0xCA14_EBFE;
    pub const UNUSED: u32 = 0xCA14_EC19;
    pub const WATERED_BUILDINGS: u32 = 0xCA78_A955;
    pub const UNWATERED_BUILDINGS: u32 = 0x8A78_AB06;
}

/// How an interface slot returns its value (§10.1): what the plugin must
/// set in the registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ret {
    /// The full 32-bit EAX.
    Eax,
    /// Only AL is meaningful (bool).
    AlOnly,
    /// A bool returned in the full EAX (0/1).
    FullEax,
    /// The low 32 bits of a 64-bit value in EAX; EDX is not set (the SDK
    /// declares `int64`).
    EaxOnly,
    /// Nothing meaningful (the SDK declares `bool`; a replacement returns
    /// true).
    NoneMeaningful,
}

/// `cISC4PlumbingSimulator` slot return channels (§10.1), by slot.
pub const SLOT_RETURNS: [(u32, Ret); 29] = [
    (0, Ret::AlOnly),
    (1, Ret::Eax),
    (2, Ret::Eax),
    (3, Ret::AlOnly),
    (4, Ret::AlOnly),
    (5, Ret::AlOnly),
    (6, Ret::NoneMeaningful),
    (7, Ret::Eax),
    (8, Ret::FullEax),
    (9, Ret::FullEax),
    (10, Ret::Eax),
    (11, Ret::AlOnly),
    (12, Ret::Eax),
    (13, Ret::Eax),
    (14, Ret::EaxOnly),
    (15, Ret::EaxOnly),
    (16, Ret::EaxOnly),
    (17, Ret::EaxOnly),
    (18, Ret::Eax),
    (19, Ret::Eax),
    (20, Ret::Eax),
    (21, Ret::Eax),
    (22, Ret::Eax),
    (23, Ret::EaxOnly),
    (24, Ret::AlOnly),
    (25, Ret::AlOnly),
    (26, Ret::AlOnly),
    (27, Ret::NoneMeaningful),
    (28, Ret::NoneMeaningful),
];

/// The EAX value of a slot that returns a 64-bit total in EAX only
/// (§10.4): its low 32 bits.
pub fn eax_of(v: i64) -> u32 {
    v as u32
}

/// A budget item as `GetBudgetItemForPurpose` reports it (budget spec).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BudgetItem {
    pub department_id: u32,
    pub line_item: u32,
    pub cost: i64,
    pub index: u32,
}

/// A message as the simulator's `DoMessage` sees it (§5.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1()` as an occupant (Insert / RemoveOccupant).
    pub occupant: Option<u64>,
    /// `GetIGZUnknown()` `QueryInterface`d to `cISC4NeighborDeal`
    /// (`0x0773FEF1`; the plugin releases it after the call); `None` when it
    /// is null or not a deal.
    pub deal: Option<u64>,
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

/// A grid held by the simulator.
pub type GridRc = Rc<dyn TractGrid<u8>>;

/// Everything the plumbing simulator reads from, or does to, the rest of
/// the game (§11). Coordinates are city cells (= tracts, S = 1). Game
/// objects are host tokens.
pub trait PlumbingHost: GridProvider<u8> {
    // ---- city ----
    /// `cISC4City::CellCountX()` / `CellCountZ()` (slots 93, 94).
    fn cell_counts(&self) -> (i32, i32);
    /// `cISC4City::PositionToCell(x, z, &cx, &cz)` (slot 95).
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);
    /// The "Utilities" exemplar ([`TUNING_KEY`]), if it loads.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;
    /// The session seed `S` (foundations §6.3 *[choice]*). The original
    /// seeds its generator at `Init` from the wall clock
    /// (`cRZRandom::Seed(0xFFFFFFFF)`, §6.1); ours takes `derive(GZCLSID)`
    /// of `S` ([`crate::seeding::keys::PLUMBING`]).
    fn session_seed(&self) -> u32;

    // ---- simulator, messages, history ----
    /// `cISC4Simulator::AddAgent(self, level, name, flags)` (slot 19).
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `cISC4Simulator::RemoveAgent(self)` (slot 20).
    fn remove_agent(&self);
    /// `AddNotification` / `RemoveNotification` of the simulator's message
    /// target for each type (message server slots 5 / 6).
    fn subscribe(&self, msg_types: &[u32]);
    fn unsubscribe(&self, msg_types: &[u32]);
    /// Posts a standard message. For [`msg::WATER_MAP_CHANGED`] the plugin
    /// sets void1 = IGZUnknown = the simulator (`MessagePost(msg, false)`)
    /// and `data1` is unused; [`msg::DEAL_STATUS`] goes through
    /// `SC4PostMessage2` with data1.
    fn post(&self, msg_type: u32, data1: u32);
    /// `cISC4HistoryWarehouse::RecordHistoryData(id, value)` (slot 12).
    fn record_history(&self, id: u32, value: u32);
    /// `cISC4OrdinanceSimulator::GetCumulativeFactor(id, index)` (slot 25);
    /// `None` without an ordinance simulator.
    fn cumulative_factor(&self, id: u32, index: u32) -> Option<f32>;
    /// `cISC4PollutionSimulator::GetWaterValue(x, z, &v)` (slot 8) with `v`
    /// initialised to 0.
    fn water_value(&self, x: i32, z: i32) -> i32;

    // ---- occupants ----
    /// `cISC4Occupant::GetType()` (slot 7).
    fn occupant_type(&self, occ: u64) -> u32;
    /// A property of `occ.AsPropertyHolder()` (own or from its exemplar),
    /// `None` if absent (`HasProperty` is `is_some()`).
    fn occupant_property(&self, occ: u64, id: u32) -> Option<Variant>;
    /// Sets a property on the occupant's holder (the property helpers'
    /// setters).
    fn set_occupant_property(&self, occ: u64, id: u32, value: Variant);
    /// `GetPosition()` (x, y, z) (slot 8).
    fn occupant_position(&self, occ: u64) -> [f32; 3];
    /// `GetBoundingBox()`: (min corner, max corner) (slot 10).
    fn occupant_bounding_box(&self, occ: u64) -> ([f32; 3], [f32; 3]);
    /// `GetBoundingCityCells()` (inclusive, slot 11).
    fn occupant_bounding_cells(&self, occ: u64) -> CellRect;
    /// `QueryInterface(cISC4BuildingOccupant)` then `GetBuildingType()`;
    /// `None` when the QI fails.
    fn building_type(&self, occ: u64) -> Option<u32>;
    /// `GetOccupantsByStandardCityCells(list, &cell, &cell, 0x278128A0, −1)`
    /// (occupant manager compiled slot 41): the buildings in one cell.
    fn buildings_in_cell(&self, x: i32, z: i32) -> Vec<u64>;

    // ---- network occupants (pipes) ----
    /// `QueryInterface(cISC4NetworkOccupant)` succeeds.
    fn is_network_occupant(&self, occ: u64) -> bool;
    /// `HasNetworkFlag(flag)` (slot 8).
    fn has_network_flag(&self, occ: u64, flag: u32) -> bool;
    /// `GetNetworkFlag()` (slot 12).
    fn network_flags(&self, occ: u64) -> u32;
    /// `SetNetworkFlag(flag)` / `ClearNetworkFlag(flag)` (slots 10, 11).
    fn set_network_flag(&self, occ: u64, flag: u32);
    fn clear_network_flag(&self, occ: u64, flag: u32);
    /// `ConnectsToNeighbor(dir, networkType)` (slot 47).
    fn connects_to_neighbor(&self, occ: u64, dir: u32, network_type: u32) -> bool;
    /// `GetOccupiedCell(&x, &z)` (slot 59).
    fn occupied_cell(&self, occ: u64) -> (i32, i32);
    /// The direct call to `cSC4PipeOccupant::SetWateredState(bool)`
    /// (`0x00648540`, §12.2 item 3: a plug-in shim or nothing).
    fn set_pipe_watered(&self, occ: u64, watered: bool);

    // ---- lots ----
    /// `cISC4LotManager::GetLot(x, z, false)` (compiled slot 6).
    fn lot_at(&self, x: i32, z: i32) -> Option<u64>;
    /// `cISC4LotManager::GetOccupantLot(occ)` (slot 14).
    fn occupant_lot(&self, occ: u64) -> Option<u64>;
    /// `cISC4Lot::GetBoundingRect()` (inclusive cells, slot 14).
    fn lot_rect(&self, lot: u64) -> CellRect;
    /// `GetNeedsPower()` / `GetHasPower()` (slots 65, 66).
    fn lot_needs_power(&self, lot: u64) -> bool;
    fn lot_has_power(&self, lot: u64) -> bool;
    /// `SetCondition(c)` (slot 36).
    fn set_lot_condition(&self, lot: u64, condition: u8);

    // ---- terrain ----
    /// The terrain's cell counts (terrain slots 7, 8); `None` without a
    /// terrain.
    fn terrain_cell_counts(&self) -> Option<(i32, i32)>;
    /// `cISTETerrain::CellHasWater(x, z)` (slot 34).
    fn cell_has_water(&self, x: i32, z: i32) -> bool;
    /// `GetTerrain()->RedisplayTerrain(0, 0, &min, &max, 0)` (slot 69).
    fn redisplay_terrain(&self, min: (i32, i32), max: (i32, i32));

    // ---- disasters ----
    /// `GetDisasterLayer()->GetDisasterManager(0x4A1332E8)` and, if it
    /// exists, `AttemptDisasterStart(x, z)`. Returns whether the manager
    /// existed.
    fn attempt_pipe_burst(&self, x: i32, z: i32) -> bool;

    // ---- neighbours (deals and connections) ----
    /// `FindDeals(type, list)` (slot 30), in list order.
    fn find_deals(&self, deal_type: u32) -> Vec<u64>;
    /// `cISC4NeighborDeal::GetType()` (slot 8).
    fn deal_type(&self, deal: u64) -> u32;
    /// `GetCityOnOtherSideOfConnection()` (slot 7), as a token.
    fn deal_city(&self, deal: u64) -> u64;
    /// `GetAmount()` / `GetAmountUsed()` (slots 10, 20).
    fn deal_amount(&self, deal: u64) -> i32;
    fn deal_amount_used(&self, deal: u64) -> i32;
    /// `SetAmountUsed(v)` (slot 21).
    fn set_deal_amount_used(&self, deal: u64, used: i32);
    /// `SetFlags(flags, on)` (slot 37).
    fn set_deal_flags(&self, deal: u64, flags: u32, on: bool);
    /// `GetConnectionsOfType(type, list)` (slot 17).
    fn connections_of_type(&self, connection_type: u32) -> Vec<u64>;
    /// `GetConnectionsOfTypeToCity(type, city, list)` (slot 18).
    fn connections_of_type_to_city(&self, connection_type: u32, city: u64) -> Vec<u64>;
    /// `FindUnvisitedConnection(type, city)` (slot 15).
    fn find_unvisited_connection(&self, connection_type: u32, city: u64) -> Option<u64>;
    /// `FindConnectionOnOtherSide(conn)` (slot 13).
    fn connection_on_other_side(&self, conn: u64) -> Option<u64>;
    /// `GetConnectionCell(&x, &z)` (connection slot 6).
    fn connection_cell(&self, conn: u64) -> (i32, i32);
    /// `SetVisitedThisCycle(v)` (slot 12).
    fn set_connection_visited(&self, conn: u64, visited: bool);
    /// `GetIsActive()` / `SetIsActive(v)` (slots 13, 14).
    fn connection_is_active(&self, conn: u64) -> bool;
    fn set_connection_active(&self, conn: u64, active: bool);
    /// `MarkDealsDirty()` (slot 31).
    fn mark_deals_dirty(&self);

    // ---- budget ----
    /// `CreateDepartmentBudget(id, group)` (budget slot 25); a borrowed
    /// pointer the simulator `add_ref`s.
    fn create_department(&self, id: u32, group: u32) -> Option<u64>;
    /// `SetDepartmentName(group, instance)` (department slot 5).
    fn set_department_name(&self, dept: u64, group: u32, instance: u32);
    /// `SetMaxAllowedFundingPercentage(&p)` (slot 13).
    fn set_department_max_funding(&self, dept: u64, fraction: f32);
    /// `CreateLineItem(id, locallyFunded)` (slot 17).
    fn create_line_item(&self, dept: u64, line: u32, locally_funded: bool) -> Option<u64>;
    /// `cISC4LineItem::SetName(group, instance)` (line-item slot 7).
    fn set_line_item_name(&self, line_item: u64, group: u32, instance: u32);
    /// `RemoveLineItem(id)` (department slot 19).
    fn remove_line_item(&self, dept: u64, line: u32);
    /// `*GetFundingPercentage()` (slot 10): a fraction.
    fn department_funding(&self, dept: u64) -> f32;
    /// `GetDepartmentBudget(id)` (budget slot 28).
    fn department_by_id(&self, id: u32) -> Option<u64>;
    /// `GetLineItem(line)` (department slot 20).
    fn line_item(&self, dept: u64, line: u32) -> Option<u64>;
    /// `CreateLineItemForBuildingType(line, locallyFunded)` (slot 18).
    fn create_line_item_for_building_type(&self, dept: u64, line: u32, locally_funded: bool) -> Option<u64>;
    /// `SetDisplayFlag(flag, on)` (line-item slot 18).
    fn set_line_item_display_flag(&self, line_item: u64, flag: u32, on: bool);
    /// `AddToFullExpenses(delta)` / `SetFullExpenses(v)` (slots 13, 12).
    fn add_to_line_item_expenses(&self, line_item: u64, delta: i64);
    fn set_line_item_expenses(&self, line_item: u64, value: i64);
    /// `GetSecondaryInfoField()` / `SetSecondaryInfoField(v)` (slots 8, 9).
    fn line_item_secondary_info(&self, line_item: u64) -> i64;
    fn set_line_item_secondary_info(&self, line_item: u64, value: i64);
    /// `GetBudgetItemForPurpose(holder, purpose)` (budget slot 51).
    fn budget_item_for_purpose(&self, occ: u64, purpose: u32) -> Option<BudgetItem>;
    /// `ChangeBudgetItemLine(holder, index, line)` (slot 52).
    fn change_budget_item_line(&self, occ: u64, index: u32, line: u32);
    /// `ChangeBudgetItemCost(holder, index, cost)` (slot 53).
    fn change_budget_item_cost(&self, occ: u64, index: u32, cost: i64);
    /// `GetLineItemFromBudgetItem(item)` (slot 57).
    fn line_item_from_budget_item(&self, item: &BudgetItem) -> Option<u64>;
    /// `IsLocallyFundedObjectInDepartment(occ, &funding)` (department slot
    /// 28): the local funding when the department funds `occ` locally.
    fn local_funding(&self, dept: u64, occ: u64) -> Option<f32>;

    // ---- host objects ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    /// How a save refers to a host object (occupant, pipe, department).
    fn object_ref(&self, token: u64) -> RefValue;
    /// The token of a reference read from a save, if it answers `iid`
    /// ([`IID_OCCUPANT`], [`IID_NETWORK_OCCUPANT`], [`IID_DEPARTMENT_BUDGET`]).
    /// The stream's reference is transferred to the simulator; when this
    /// returns `None` the host drops that reference itself.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
    /// `Shutdown()` on a grid the simulator is about to release (§4.2).
    fn grid_shutdown(&self, grid: &GridRc);
}
