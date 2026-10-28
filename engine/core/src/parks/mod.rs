// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The park manager (`cSC4ParkManager`, class `0xAA3FE6BC`),
//! `docs/specs/parks.md`. Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - `manager.rs`: the state of §2, `Init` / `Shutdown` (§4.1), the
//!   occupant handlers (§4.2, §4.3), the monthly pass (§4.4), the zoo escape
//!   (§4.5) and the record of §7.
//! - `tests.rs`: a fake host and the §12 tests.
//!
//! Everything outside the class goes through [`ParkHost`]: agents and the
//! message server, the city, the budget simulator (departments, items and
//! line items), the lot manager, the tuning exemplar, the occupants, the
//! message posted on an escape, the automata controller manager and save
//! references. Objects are host tokens; a building and its occupant are one
//! token (see `civic`).
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **The tuning** (§6) is read once per object, at the first `Init`:
//!   the threshold keeps 0.25 when the property is missing; the curve
//!   follows the "default" pattern with the exemplar (code default (0, 50),
//!   (100, 1) when the property is missing) and stays empty without it; an
//!   empty curve evaluates to 0 (the spec's *[choice]*), so no zoo escapes.
//! - **A missing department** in §4.2 step 4: steps 5–7 are skipped (the
//!   spec's *[choice]*). A line item that cannot be created is skipped too
//!   [choice].
//! - **The monthly pass without a held department** (it could not be found
//!   or created) or without a readable funding: the pass stops after step 1
//!   [choice] (the original would dereference null).
//! - **Null list entries** (a record whose reference did not resolve):
//!   skipped by the run-down loop; an escape that draws one stops (§4.5
//!   step 1: "if it is null, return").
//! - **The zoo removal** (§4.3 step 2) releases the occupant once even when
//!   it was not listed: the literal reading of "remove every entry … and
//!   release it once", as for parks.
//! - **`Read`** (§7) is atomic [choice]: on a stream error or a wrong version
//!   nothing changes and the references read so far are released. The lists
//!   are replaced **without** releasing their old entries (the shared list
//!   reader of civic-buildings.md §7).
//! - **The generator** (§5): `RzRandomSuper` seeded with `derive_seed(S,
//!   0xAA3FE6BC)` at construction (the spec's *[choice]*), behind the
//!   injectable [`ParkRandom`].
//! - **Threading** (foundations §19.3): the state is a `RefCell`; the core is
//!   main-thread only. No borrow is held across a host call; host callbacks
//!   must not call back into the manager.

pub mod archive;
mod manager;

#[cfg(test)]
mod tests;

pub use manager::{query_interface, ParkManager, ParkState};

use std::rc::Rc;

use crate::budget::BudgetItem;
use crate::curve::ResponseCurve;
use crate::property::PropertyHolder;
use crate::rng::RzRandomSuper;
use crate::stream::RefValue;

/// Class ID and interface ID (§1: the same value).
pub const CLSID_PARK_MANAGER: u32 = 0xAA3F_E6BC;
pub const IID_PARK_MANAGER: u32 = 0xAA3F_E6BC;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// List entries are read with this IID (§7).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
/// `cISC4BuildingOccupant` (§4.2 step 1.3).
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;
/// The tuning exemplar is read with this IID (§6).
pub const IID_TUNING: u32 = 0xA521_60F5;

/// City component and getter slot (§1).
pub const CITY_COMPONENT: u32 = 15;
pub const CITY_GETTER_SLOT: u32 = 74;

/// The one agent (§3).
pub const AGENT_NAME: &str = "cSC4ParkManager";
pub const AGENT_LEVEL: u32 = 6;
pub const AGENT_FLAGS: u32 = 2;

/// Messages (§3, §4.5).
pub mod msg {
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    /// The subscriptions of `Init` step 5, in order.
    pub const SUBSCRIPTIONS: [u32; 2] = [OCCUPANT_INSERTED, OCCUPANT_REMOVED];
    /// Posted on an escape (§4.5).
    pub const ZOO_ESCAPE: u32 = 0xAA66_7CE2;
}

/// The preview-occupant property (§3; presence, not value).
pub const PROP_PREVIEW_OCCUPANT: u32 = 0x0292_44DB;
/// The occupant type of a building.
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
/// Occupant groups (§6).
pub const GROUP_PARK: u32 = 0x1006;
pub const GROUP_ZOO: u32 = 0x1702;
/// The park maintenance budget item purpose (§4.2).
pub const PURPOSE_PARK: u32 = 0xCA63_9989;
/// `li.SetDisplayFlag(2, true)`: show the secondary field (§4.2 step 6).
pub const DISPLAY_FLAG_SECONDARY: u32 = 2;

/// The Parks and Recreation department (§4.1 step 3).
pub const DEPARTMENT_ID: u32 = 0x8A42_60A6;
pub const DEPARTMENT_GROUP: u32 = 0x6A35_7B96;
pub const DEPARTMENT_NAME: (u32, u32) = (0x6A3F_F01C, 0xCA49_375E);
pub const MAX_FUNDING: f32 = 1.0;

/// The tuning exemplar "Parks Manager" (§6): (type, group, instance).
pub const TUNING_KEY: (u32, u32, u32) = (0x6534_284A, 0x67CD_5FA1, 0x8A48_05BC);
/// Tuning properties (§6).
pub mod prop {
    /// Efficiency Distress Threshold (float).
    pub const DISTRESS_THRESHOLD: u32 = 0xCA48_0830;
    /// Zoo Escape Chance Curve.
    pub const ESCAPE_CURVE: u32 = 0x4A66_76AD;
}
/// The constructor's threshold (`0x00B0D3D8`).
pub const DEFAULT_THRESHOLD: f32 = 0.25;
/// The code default of the escape curve ("default" pattern, §6).
pub const DEFAULT_ESCAPE_CURVE: [(f32, f32); 2] = [(0.0, 50.0), (100.0, 1.0)];
/// The shipped exemplar's values (§4.6, §6).
pub const SHIPPED_THRESHOLD: f32 = 25.0;
pub const SHIPPED_ESCAPE_CURVE: [(f32, f32); 3] = [(0.0, 40.0), (25.0, 10.0), (100.0, 1.0)];

/// Lot conditions (§4.4).
pub const CONDITION_NORMAL: u32 = 0;
pub const CONDITION_RUN_DOWN: u32 = 2;

/// The automata group of a zoo escape (§4.5).
pub const ZOO_ESCAPE_AUTOMATA: &str = "zoomanji";

/// The record version (§7) and the shared list reader's limit.
pub const RECORD_VERSION: u16 = 2;
pub const MAX_LIST_COUNT: u32 = 10_000_000;
/// The class written for the list entries (§7).
pub const OCCUPANT_CLASS: u32 = 0xA9BD_882D;

/// The escape probability of §4.4 step 5: `curve(f × 100) × 0.01` with
/// `float` constants and single-precision products, widened to `double`.
pub fn escape_probability(curve: &ResponseCurve, funding: f32) -> f64 {
    let x = funding * 100.0f32;
    f64::from(curve.eval(x) * 0.01f32)
}

/// The tuning of §6 as read: (threshold, curve).
pub fn tuning_from(exemplar: Option<&PropertyHolder>) -> (f32, ResponseCurve) {
    match exemplar {
        None => (DEFAULT_THRESHOLD, ResponseCurve::default()),
        Some(h) => (
            crate::props::f32_of(h, prop::DISTRESS_THRESHOLD)
                .map(|percent| percent * 0.01f32)
                .unwrap_or(DEFAULT_THRESHOLD),
            crate::props::curve(h, prop::ESCAPE_CURVE)
                .unwrap_or_else(|| ResponseCurve::from_points(&DEFAULT_ESCAPE_CURVE)),
        ),
    }
}

/// The shipped "Parks Manager" exemplar (§6), for tests and tools.
pub fn shipped_tuning() -> PropertyHolder {
    use crate::variant::{mem, Elem, Variant};
    let mut h = PropertyHolder::new();
    h.add(prop::DISTRESS_THRESHOLD, Variant::Scalar(Elem::F32(SHIPPED_THRESHOLD)));
    h.add(
        prop::ESCAPE_CURVE,
        Variant::Array {
            elem: mem::FLOAT32,
            items: SHIPPED_ESCAPE_CURVE
                .iter()
                .flat_map(|&(x, y)| [Elem::F32(x), Elem::F32(y)])
                .collect(),
        },
    );
    h.add(0xCA48_0831, Variant::Scalar(Elem::I64(0)));
    h
}

/// A message as `DoMessage` sees it (§3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParkMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message.
    pub void1: Option<u64>,
}

impl ParkMessage {
    pub fn of_type(msg_type: u32) -> ParkMessage {
        ParkMessage {
            msg_type,
            ..ParkMessage::default()
        }
    }
}

/// The draws of §5 on the member generator.
pub trait ParkRandom {
    /// `RandomDoubleUniform()`.
    fn uniform_f64(&mut self) -> f64;
    /// `RandomUint32Uniform(n)`.
    fn uniform_u32(&mut self, n: u32) -> u32;
}

impl ParkRandom for RzRandomSuper {
    fn uniform_f64(&mut self) -> f64 {
        RzRandomSuper::uniform_f64(self)
    }
    fn uniform_u32(&mut self, n: u32) -> u32 {
        RzRandomSuper::uniform_u32(self, n)
    }
}

/// Everything the park manager reads from, or does to, the rest of the
/// game. Objects are host tokens. Re-entrancy: callbacks must not call back
/// into the manager.
pub trait ParkHost {
    // ---- agents and messages (§3, §4.1) ----
    /// `AddAgent(+4, level, name, flags)` on the city's simulator.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(+4)` on the city's simulator.
    fn remove_agent(&self);
    /// `Init` step 5: hold the message server; false when there is none.
    /// Called only when the manager holds none.
    fn acquire_message_server(&self) -> bool;
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    fn release_message_server(&self);

    // ---- services (§4.1 steps 2 and 4) ----
    /// Hold the city / the budget simulator / the lot manager; false when
    /// there is none.
    fn acquire_city(&self) -> bool;
    fn release_city(&self);
    fn acquire_budget(&self) -> bool;
    fn release_budget(&self);
    fn acquire_lot_manager(&self) -> bool;
    fn release_lot_manager(&self);

    // ---- tuning (§6) ----
    /// The "Parks Manager" exemplar ([`TUNING_KEY`], read with
    /// [`IID_TUNING`]), if it loads.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- the department (§4.1 step 3, §4.4) ----
    /// `budget.GetDepartmentBudget(id)`: `None` for a null department.
    fn department_budget(&self, id: u32) -> Option<u64>;
    /// `budget.CreateDepartmentBudget(id, group)`.
    fn create_department_budget(&self, id: u32, group: u32) -> Option<u64>;
    fn set_department_name(&self, dept: u64, group: u32, instance: u32);
    fn set_max_funding(&self, dept: u64, max: f32);
    /// `*dept.GetFundingPercentage()` (a fraction); `None` when unreadable.
    fn department_funding(&self, dept: u64) -> Option<f32>;

    // ---- budget items and line items (§4.2, §4.3) ----
    /// `budget.GetBudgetItemForPurpose(holder, purpose, &item)`.
    fn budget_item(&self, occupant: u64, purpose: u32) -> Option<BudgetItem>;
    /// `budget.ChangeBudgetItemLine(holder, index, line)`.
    fn change_budget_item_line(&self, occupant: u64, index: i32, line: u32);
    /// `budget.GetLineItemFromBudgetItem(item)`.
    fn line_item_from_budget_item(&self, item: &BudgetItem) -> Option<u64>;
    /// `dept.GetLineItem(line)`.
    fn line_item(&self, dept: u64, line: u32) -> Option<u64>;
    /// `dept.CreateLineItemForBuildingType(line, isLocal)`.
    fn create_line_item(&self, dept: u64, line: u32, local: bool) -> Option<u64>;
    /// `li.SetDisplayFlag(flag, on)`.
    fn set_line_display_flag(&self, line: u64, flag: u32, on: bool);
    /// `li.AddToFullExpenses(amount)`.
    fn add_to_full_expenses(&self, line: u64, amount: i64);
    /// `li.GetSecondaryInfoField()` / `SetSecondaryInfoField(v)` (64-bit).
    fn line_secondary(&self, line: u64) -> i64;
    fn set_line_secondary(&self, line: u64, value: i64);

    // ---- occupants (§3, §4.2, §4.3) ----
    /// `occ.GetType()`.
    fn occupant_type(&self, occupant: u64) -> u32;
    /// `occ.AsPropertyHolder()` has the property `id` (any value).
    fn occupant_has_property(&self, occupant: u64, id: u32) -> bool;
    /// `occ.IsOccupantGroup(group)`.
    fn is_occupant_group(&self, occupant: u64, group: u32) -> bool;
    /// `occ.QueryInterface(0x87DDA3A9)` succeeds (the host releases the
    /// interface: §4.2 step 8).
    fn is_building_occupant(&self, occupant: u64) -> bool;
    /// `building.GetBuildingType()`.
    fn building_type(&self, building: u64) -> u32;

    // ---- lots (§4.4 step 4) ----
    /// The lot manager's `GetLot(cellX, cellZ, false)` at the cell
    /// `city.PositionToCell` gives for the occupant's position.
    fn occupant_position_lot(&self, occupant: u64) -> Option<u64>;
    /// `lot.SetCondition(c)`.
    fn set_lot_condition(&self, lot: u64, condition: u32);

    // ---- the escape (§4.5) ----
    /// Posts `0xAA667CE2`: a new standard message with void1 = the manager
    /// (`manager`, its host token) and IGZUnknown = the zoo,
    /// `MessagePost(msg, false)`.
    fn post_zoo_escape(&self, manager: u64, zoo: u64);
    /// `city.GetAutomataControllerManager()`, if it exists:
    /// `AddControllersFromOccupantGroup(name, occ, 0)`.
    fn add_controllers(&self, name: &str, occupant: u64);

    // ---- save references (§7) ----
    fn object_ref(&self, token: u64) -> RefValue;
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}
