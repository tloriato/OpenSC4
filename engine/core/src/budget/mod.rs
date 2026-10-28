// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The budget subsystem (`docs/specs/budget.md`): the budget simulator, its
//! departments and their line items. Section references (`§n`) point at that
//! spec.
//!
//! The three classes are separate shared objects (`Rc`), like their GZCOM
//! counterparts: on load the class-object store creates and reads them
//! independently, in ascending class ID (line items, then the budget, then the
//! departments), and references link them. Departments hold their line items;
//! back-references (line → department, department → budget) are weak.
//!
//! The RiskyMoney generator is the budget file's static `cRZRandomSuper`,
//! seeded once per process (RN3): [`BudgetSim::process_wide`] (the C ABI's
//! create) shares the process-wide one of
//! [`crate::seeding::process_super`], whose stream continues across city
//! loads; [`BudgetSim::new`] builds a fresh one.
//!
//! Re-entrancy: no `RefCell` borrow is held across a call into another object
//! or the host, except the host's property and query callbacks, which must not
//! call back into the budget.

mod department;
mod items;
mod line_item;
mod simulator;
mod tax;
mod tuning;
pub mod archive;

pub use department::Department;
pub use department::DEPARTMENT_RECORD_VERSION;
pub use items::{
    budget_items, change_item_cost, change_item_line, change_item_local_funding, copy_item_properties,
    item_for_purpose, BudgetItem,
};
pub use items::{local_funding_fraction, local_funding_percent, PERCENT_TO_FRACTION};
pub use line_item::{LineItem, LINE_ITEM_RECORD_VERSION, TYPE_EXPENSE, TYPE_INCOME};
pub use simulator::{Bond, BudgetGroup, BudgetSim, BudgetSnapshot, LoanInfo, TaxGroup, BUDGET_RECORD_VERSION};
pub use tax::{difficulty_multiplier, scan_tax, tax_income_of, tax_type, TaxLot, TaxLotStatus, TaxScan};
pub use tuning::{BudgetTuning, NoProperties, PropertySource, TUNING_GROUP, TUNING_INSTANCE, TUNING_TYPE};

use std::rc::Rc;

use crate::stream::SaveObject;

/// Class IDs (§1).
pub const CLSID_BUDGET_SIMULATOR: u32 = 0xE990_BE01;
pub const CLSID_DEPARTMENT: u32 = 0xE990_BFFC;
pub const CLSID_LINE_ITEM: u32 = 0xAA31_3C9F;
/// Interface requested for local-object references (`cISCPropertyHolder`, §11.2).
pub const IID_PROPERTY_HOLDER: u32 = 0x2521_6283;

/// Messages (§4.3, §10).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const CITY_ESTABLISHED: u32 = 0x26D3_1EC4;
    pub const TOGGLE_BUDGET_DIALOG: u32 = 0x2922_2F02;
    pub const IDEAL_MONTHLY_FUNDING_CHANGED: u32 = 0x08DA_C3E8;
    pub const FUNDING_PERCENTAGE_CHANGED: u32 = 0x08DA_C3E7;
    pub const CHEAT_ISSUED: u32 = 0x230E_27AC;
    pub const FUNDS_CHANGED: u32 = 0x0772_FAD4;
    pub const BOND_ISSUED: u32 = 0xC772_FADA;
    pub const BOND_FINISHED: u32 = 0xCA3F_BDCE;
    pub const END_GAME_FOR_BUDGET_FAILURE: u32 = 0xAA5C_F1E3;
    pub const BUDGET_GROUP_CHANGED: u32 = 0x0A3E_7624;
    pub const BUDGET_FOR_OCCUPANT_CHANGED: u32 = 0x6A5F_D3FD;
    /// ToggleBudgetDialog data1 values (§4.3).
    pub const PANEL_1: i64 = 0x8922_30F2;
    pub const PANEL_0: i64 = 0xC922_30EE;
}

/// Cheats (§4.1, §4.3).
pub mod cheat {
    pub const RISKY_MONEY: u32 = 0xCBF3_1943;
    pub const WEAKNESS_PAYS: u32 = 0x4AB8_7766;
    pub const MOOLAH: u32 = 0x493F_4FE6;
    pub const RISKY_MONEY_DISASTER: u32 = 0x4296_3812;
}

/// Budget-item properties on buildings (§2).
pub mod prop {
    pub const DEPARTMENT: u32 = 0xEA54_D283;
    pub const LINE: u32 = 0xEA54_D284;
    pub const PURPOSE: u32 = 0xEA54_D285;
    pub const COST: u32 = 0xEA54_D286;
    pub const LOCAL_FUNDING: u32 = 0xEA54_D287;
    pub const BUILDING_VALUE: u32 = 0xE91A_0B5F;
}

/// History series (§10.2).
pub mod history {
    pub const TAX_RATE_BASE: u32 = 0xAABA_AA9F;
    pub const INCOME: u32 = 0x6A51_178B;
    pub const EXPENSE: u32 = 0xCA5B_A3FB;
    pub const FUNDS: u32 = 0x4A5B_A3D0;
}

/// LTEXT group of budget-group names (§2).
pub const GROUP_NAME_LTEXT: u32 = 0x6A3F_F01C;

/// Tax groups (§2).
pub const TAX_GROUPS: usize = 12;

/// A message posted by the budget (`MessagePost`, §10.1). Tokens are host
/// identities (0 = not set).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BudgetMessage {
    pub msg_type: u32,
    pub data1: i64,
    pub data2: i64,
    pub data3: i64,
    /// `SetVoid1(...)`.
    pub void1: u64,
    /// `SetIGZUnknown(...)`.
    pub unknown: u64,
}

/// One lot, as `ComputeTax` needs it (§5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LotInfo {
    pub has_building: bool,
    pub livable: bool,
    /// Building profile `PurposeType`.
    pub purpose: u32,
    /// Building profile wealth.
    pub building_wealth: u32,
    /// `cISC4Lot::GetOccupantWealth()`.
    pub lot_wealth: u32,
    /// Building property `0xE91A0B5F` (0 if absent).
    pub building_value: i64,
    /// Land value (0..255) at the lot's origin cell.
    pub land_value: u8,
    pub size_x: u32,
    pub size_z: u32,
}

/// What the budget needs from its environment. Property-holder and object
/// tokens are host identities (interface pointers in the GZCOM shim).
pub trait BudgetHost {
    // ---- messages and UI ----
    fn post(&self, msg: &BudgetMessage);
    /// `ShowBudgetWindow` with the selected panel (UI; §6 [choice]).
    fn show_budget_window(&self, _panel: u32) {}

    // ---- city ----
    /// `cISC4City::GetDifficultyLevel()` (0..2).
    fn difficulty(&self) -> u32;
    /// The simulator's current date (year, month 1..12).
    fn sim_date(&self) -> (i32, i32);
    fn city_established(&self) -> bool;
    /// City size along x and z (world units, for RiskyMoney).
    fn city_size(&self) -> (f32, f32);
    fn ordinance_monthly_income(&self) -> i64;
    fn ordinance_monthly_expense(&self) -> i64;
    fn neighbor_deal_income(&self) -> i64;
    fn neighbor_deal_expense(&self) -> i64;
    /// Every lot of the lot manager, in its master-list order (§5.2): the
    /// tax total is a float32 running sum, so it depends on that order (BQ3).
    fn for_each_lot(&self, f: &mut dyn FnMut(&LotInfo));
    fn record_history(&self, series: u32, value: i32);
    fn set_history_queue_length(&self, series: u32, length: u32);
    /// Starts disaster `id` at (x, z) if it exists; false otherwise.
    fn try_start_disaster(&self, id: u32, x: f32, z: f32) -> bool;
    /// The session seed `S` (foundations §6.3 *[choice]*). The budget's
    /// file-local `cRZRandomSuper` (§6.2: wall clock XOR `0x6C09C3D7`) is
    /// seeded with `derive(0x6C09C3D7)` of it, once per process for
    /// [`BudgetSim::process_wide`] (RN3).
    fn session_seed(&self) -> u32;

    // ---- strings ----
    fn ltext(&self, group: u32, instance: u32) -> Vec<u8>;
    /// Display name of a building type (from the building-development simulator).
    fn building_type_name(&self, building_type: u32) -> Vec<u8>;

    // ---- property holders ----
    fn get_u32s(&self, holder: u64, prop: u32) -> Option<Vec<u32>>;
    fn get_i64s(&self, holder: u64, prop: u32) -> Option<Vec<i64>>;
    fn get_f32s(&self, holder: u64, prop: u32) -> Option<Vec<f32>>;
    fn set_u32s(&self, holder: u64, prop: u32, values: &[u32]) -> bool;
    fn set_i64s(&self, holder: u64, prop: u32, values: &[i64]) -> bool;
    fn set_f32s(&self, holder: u64, prop: u32, values: &[f32]) -> bool;
    fn copy_property(&self, src: u64, dst: u64, prop: u32) -> bool;
    /// Whether a property is stored as an array (`Some(true)`) or a scalar (`Some(false)`); `None` when absent or
    /// unknown (data-formats §5.2, EX39). The budget item arrays ([`items`]) are read with the original's vector
    /// helpers, which give a scalar an empty vector; `None` keeps the converting reads' one value.
    fn property_is_array(&self, _holder: u64, _prop: u32) -> Option<bool> {
        None
    }
    /// The occupant behind a property holder, if it is one (§7.3).
    fn occupant_of(&self, holder: u64) -> Option<u64>;
    /// Class ID of a host object (for references written by the byte codec).
    fn class_of(&self, token: u64) -> u32;

    // ---- object lifetime ----
    /// Takes / drops one reference on a host object or on the GZCOM object
    /// fronting a core object.
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    /// Creates a new budget object of `class_id` (department or line item)
    /// through the class registry; the returned object carries one reference
    /// owned by the caller.
    fn create_object(&self, class_id: u32) -> Option<Rc<dyn SaveObject>>;
}

/// `roundHalfEven` (§5.2, §5.7).
pub(crate) fn round_half_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        r
    }
}
