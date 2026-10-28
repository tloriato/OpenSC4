// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The civic building simulator (`cSC4CivicBuildingSimulator`, class
//! `0x0A413481`), `docs/specs/civic-buildings.md`. Section references (`§n`)
//! point at that spec.
//!
//! # Structure
//!
//! - `simulator.rs`: the state of §2, `Init` / `Shutdown` (§4.1, §4.2), the
//!   messages of §3, the occupant handler (§4.3), `DoLotActivated` (§4.4),
//!   `DoBuildingRemoved` (§4.5), the monthly pass (§4.6) and the interface
//!   methods (§4.7).
//! - `record.rs`: the version-6 record of §7.
//! - `tests.rs`: a fake host and the §12 tests.
//!
//! Everything outside the class goes through [`CivicHost`]: agents and
//! subscriptions, the Lua table, the budget simulator's departments, items
//! and line items (budget.md), the occupants' properties and groups, the lot
//! manager and lots, the building development simulator's key lookup, the
//! automata controller manager, and save references. Objects are host
//! tokens. A building and its occupant are **one** token (the game's
//! `cISC4BuildingOccupant` and `cISC4Occupant` are two interfaces of one
//! object); the host answers "is this occupant a building occupant" with
//! [`CivicHost::is_building_occupant`].
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **Hash containers** (§2) are [`StlHashSet`] / [`StlHashMap`] built with
//!   the hint 100 (193 buckets), so iteration (and the record) follows the
//!   STLport order. `Read` inserts the file's entries in file order, each at
//!   the head of its chain (§7, QUESTIONS CB6 answered), so every chain of
//!   two or more keys is reversed by a write → read → write and restored by
//!   a second round trip. A repeated or already present exclusion ID is not
//!   inserted again; a repeated or already present status key overwrites
//!   the stored value (the last one in the file wins) in place.
//! - **`Release`** (slot 2, `0x0069D7A0`): the spec gives no behaviour. The
//!   sibling simulators' rule is used: above 1 it decrements; at 1 or less it
//!   runs `Shutdown`, destroys the object and returns 0 *[question]*.
//! - **Null list entries** (a record whose reference did not resolve, §7):
//!   the monthly pass skips them; an ape escape that draws one does nothing
//!   [choice].
//! - **CB5** (QUESTIONS): a business-deal income line that cannot be created
//!   is skipped [choice] (the original would call `AddToIncome` on null).
//! - **A department that cannot be created** in `Init` (budget returns null):
//!   its name and maximum are not set and nothing is held [choice].
//! - **A department funding** the host cannot read in the monthly pass: the
//!   entry is skipped [choice].
//! - **`DoBuildingRemoved` step 3** releases the Disease Research Center only
//!   when an entry was found and erased (the literal reading of "erase the
//!   first matching entry … and release it").
//! - **`UpdateConditionalBuildingStatus`** returns true (the spec's
//!   *[choice]*).
//! - **`Read`** (§7) is atomic [choice]: on a stream error or a wrong
//!   version nothing changes and the references read so far are released.
//!   As the spec says, the lists are replaced **without** releasing their
//!   old entries, and the old Landmarks department **is** released.
//! - **The temporary generator** (§5): `derive_seed(S, 0x0A413481 XOR n)`,
//!   `n` counting the temporaries this object made (not saved), behind the
//!   injectable [`CivicRandomFactory`].
//! - **Threading** (foundations §19.3): the state is a `RefCell`; the core is
//!   main-thread only. No borrow is held across a host call; host callbacks
//!   must not call back into the simulator.

pub mod archive;
mod record;
mod simulator;

#[cfg(test)]
mod tests;

pub use simulator::{query_interface, CivicSim, CivicState};

use std::rc::Rc;

use crate::budget::BudgetItem;
use crate::rng::{FixedSeeds, RzRandom};
use crate::stream::RefValue;

pub use crate::stl::{StlHashMap, StlHashSet};

/// Class ID (`GetGZCLSID`, §1).
pub const CLSID_CIVIC_BUILDING_SIMULATOR: u32 = 0x0A41_3481;

/// Interface IDs (§1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_CIVIC_BUILDING_SIMULATOR: u32 = 0x8A41_322F;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// The department reference is read with this IID (§7).
pub const IID_DEPARTMENT_BUDGET: u32 = 0x223E_2B67;
/// List entries are read with this IID (§7).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
/// `cISC4BuildingOccupant` (§4.3).
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;
/// `cISC4Lot` (§3: the lot-state message's IGZUnknown).
pub const IID_LOT: u32 = 0xE818_FCED;

/// City component and getter slot (§1).
pub const CITY_COMPONENT: u32 = 39;
pub const CITY_GETTER_SLOT: u32 = 73;

/// The one agent (§3).
pub const AGENT_NAME: &str = "cSC4CivicBuildingSimulator";
pub const AGENT_LEVEL: u32 = 6;
pub const AGENT_FLAGS: u32 = 2;

/// Messages (§3).
pub mod msg {
    pub const LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    /// The subscriptions of `Init` step 4, in order.
    pub const SUBSCRIPTIONS: [u32; 3] = [LOT_STATE_CHANGED, OCCUPANT_INSERTED, OCCUPANT_REMOVED];
}

/// The lot state that activates a building (§3: data2 = 3, occupied).
pub const LOT_STATE_OCCUPIED: u32 = 3;
/// The occupant type of a building (§4.3).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;

/// Building properties (§6).
pub mod prop {
    /// bool: conditional building (counted in its status entry).
    pub const CONDITIONAL_BUILDING: u32 = 0xEA32_09F8;
    /// u32: exclusion ID.
    pub const EXCLUSION_ID: u32 = 0xEA2E_078B;
}

/// Budget item purposes (§4.4).
pub const PURPOSE_MAINTENANCE: u32 = 0xAA59_670C;
pub const PURPOSE_INCOME: u32 = 0x4A5A_495E;
/// `line.SetType(1)`: an income line (§4.4 step 3).
pub const LINE_TYPE_INCOME: u32 = 1;

/// The Disease Research Center's occupant group (§4.4 step 4).
pub const GROUP_DISEASE_CENTER: u32 = 0x1917;

/// The departments of `Init` (§4.1): (ID, group, name instance). The name
/// group is [`DEPARTMENT_NAME_GROUP`] for all three.
pub mod department {
    pub const LANDMARKS: (u32, u32, u32) = (0x8A41_6368, 0x6A35_7B96, 0xCA49_1DD2);
    pub const BUILDING_MAINTENANCE: (u32, u32, u32) = (0xEA59_717A, 0xEA59_7195, 0x4A59_726D);
    pub const BUSINESS_DEALS: (u32, u32, u32) = (0x2A5A_723F, 0x0A5A_72D1, 0x8A5A_7330);
}
pub const DEPARTMENT_NAME_GROUP: u32 = 0x6A3F_F01C;
/// `SetMaxAllowedFundingPercentage(1.0)` (§4.1).
pub const MAX_FUNDING: f32 = 1.0;

/// The class IDs written for the references (§7.1).
pub const DEPARTMENT_CLASS: u32 = 0xE990_BFFC;
pub const OCCUPANT_CLASS: u32 = 0xA9BD_882D;

/// The ape-escape threshold (§4.6 step 1): the double constant at
/// `0x00A9C328`, the float 0.01 widened (0.009999999776482582). The draw
/// escapes when `≤`.
pub const APE_ESCAPE_CHANCE: f64 = 0.01f32 as f64;
/// The automata group of the ape escape.
pub const APE_ESCAPE_AUTOMATA: &str = "ape_escape";

/// The run-down threshold (§4.6 step 2): funding `< 0.5` runs down;
/// anything else (NaN included) recovers.
pub const FUNDING_THRESHOLD: f32 = 0.5;
/// Lot conditions (§4.6).
pub const CONDITION_NORMAL: u32 = 0;
pub const CONDITION_RUN_DOWN: u32 = 2;

/// The record version (§7).
pub const RECORD_VERSION: u16 = 6;
/// The shared list reader's limit (§7): a count of 10,000,000 or more reads
/// nothing further.
pub const MAX_LIST_COUNT: u32 = 10_000_000;

/// The hash containers' constructor hint (§2: 193 buckets).
pub const HASH_HINT: usize = 100;

/// `cISC4CivicBuildingSimulator::ConditionalBuildingStatus` (§2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConditionalBuildingStatus {
    /// Bits: [`status_bits::HIDDEN`], [`status_bits::AVAILABLE`],
    /// [`status_bits::ADVISOR_EVENT`].
    pub status: u8,
    pub building_count: u16,
    /// The tooltip text (`cRZString`, saved as a GZStr).
    pub tooltip: Vec<u8>,
}

/// The SDK's status bits (§2).
pub mod status_bits {
    pub const HIDDEN: u8 = 1;
    pub const AVAILABLE: u8 = 2;
    pub const ADVISOR_EVENT: u8 = 4;
}

/// A message as `DoMessage` sees it (§3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CivicMessage {
    pub msg_type: u32,
    pub data1: u32,
    /// The lot-state message's new state.
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message.
    pub void1: Option<u64>,
    /// `GetIGZUnknown()`: the lot of a lot-state message.
    pub unknown: Option<u64>,
}

impl CivicMessage {
    pub fn of_type(msg_type: u32) -> CivicMessage {
        CivicMessage {
            msg_type,
            ..CivicMessage::default()
        }
    }
}

/// The draws of the ape escape (§5) on a temporary generator.
pub trait CivicRandom {
    /// `RandomDoubleUniform()`.
    fn uniform_f64(&mut self) -> f64;
    /// `RandomUint32Uniform(n)`.
    fn uniform_u32(&mut self, n: u32) -> u32;
}

impl CivicRandom for RzRandom {
    fn uniform_f64(&mut self) -> f64 {
        RzRandom::uniform_f64(self)
    }
    fn uniform_u32(&mut self, n: u32) -> u32 {
        RzRandom::uniform_u32(self, n)
    }
}

/// Makes the temporary generator of one monthly pass from its seed.
pub type CivicRandomFactory = Box<dyn Fn(u32) -> Box<dyn CivicRandom>>;

/// The default factory: a `cRZRandom` seeded with the given seed.
pub fn default_random_factory() -> CivicRandomFactory {
    Box::new(|seed| {
        Box::new(RzRandom::new(
            seed,
            &FixedSeeds {
                wall_clock: seed,
                process_clock: seed,
            },
        ))
    })
}

/// The seed of the `n`-th temporary generator (§5 *[choice]*):
/// `derive(0x0A413481 XOR n)`.
pub fn temporary_seed(session: u32, n: u32) -> u32 {
    crate::seeding::seed(session, crate::seeding::keys::civic_monthly(n))
}

/// Everything the civic building simulator reads from, or does to, the
/// rest of the game. Objects are host tokens. Re-entrancy: callbacks must
/// not call back into the simulator.
pub trait CivicHost {
    // ---- agents, messages and Lua (§3, §4.1, §4.2) ----
    /// `AddAgent(+4, level, name, flags)` on the city's simulator.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(+4)` on the city's simulator.
    fn remove_agent(&self);
    /// Message server slots 5 / 6 with the message-target sub-object.
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// `Init` step 6: register the Lua table `civic` with
    /// `get_landmark_count()` (which returns `GetLandmarkCount()`).
    fn register_lua(&self);
    /// `Shutdown`: unregister the Lua functions.
    fn unregister_lua(&self);

    // ---- references ----
    /// `AddRef` / `Release` on a host object (occupants, departments, lots).
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- departments (§4.1; budget.md §3.2, §7) ----
    /// `budget.GetDepartmentBudget(id)`: `None` for a null department. Not
    /// held.
    fn department_budget(&self, id: u32) -> Option<u64>;
    /// `budget.CreateDepartmentBudget(id, group)`: `None` when it returns
    /// null. The simulator `AddRef`s the one it holds.
    fn create_department_budget(&self, id: u32, group: u32) -> Option<u64>;
    /// `dept.SetDepartmentName(group, instance)`.
    fn set_department_name(&self, dept: u64, group: u32, instance: u32);
    /// `dept.SetMaxAllowedFundingPercentage(&max)`.
    fn set_max_funding(&self, dept: u64, max: f32);
    /// `*dept.GetFundingPercentage()` (a fraction); `None` when the host
    /// cannot read it.
    fn department_funding(&self, dept: u64) -> Option<f32>;

    // ---- budget items and line items (§4.4, §4.5; budget.md §6, §8) ----
    /// `budget.GetBudgetItemForPurpose(holder, purpose, &item)`: `None` when
    /// the call fails.
    fn budget_item(&self, occupant: u64, purpose: u32) -> Option<BudgetItem>;
    /// `budget.ChangeBudgetItemLine(holder, index, line)`.
    fn change_budget_item_line(&self, occupant: u64, index: i32, line: u32);
    /// `budget.RemoveBudgetItemFromBudget(holder, index)`.
    fn remove_budget_item(&self, occupant: u64, index: i32);
    /// `dept.GetLineItem(line)`: `None` for a null line item.
    fn line_item(&self, dept: u64, line: u32) -> Option<u64>;
    /// `dept.CreateLineItemForBuildingType(line, isLocal)`.
    fn create_line_item(&self, dept: u64, line: u32, local: bool) -> Option<u64>;
    /// `dept.RemoveLineItem(line)`.
    fn remove_line_item(&self, dept: u64, line: u32);
    /// `line.AddToFullExpenses(amount)` (int64).
    fn add_to_full_expenses(&self, line: u64, amount: i64);
    /// `line.AddToIncome(amount)`.
    fn add_to_income(&self, line: u64, amount: i64);
    /// `line.GetIncome()`.
    fn line_income(&self, line: u64) -> i64;
    /// `line.SetIncome(v)` (clamps at 0, budget.md §8).
    fn set_line_income(&self, line: u64, income: i64);
    /// `line.SetType(t)`.
    fn set_line_type(&self, line: u64, line_type: u32);

    // ---- occupants and buildings (§4.3–§4.5) ----
    /// `occ.GetType()` (occupant slot 7).
    fn occupant_type(&self, occupant: u64) -> u32;
    /// A bool property of `occ.AsPropertyHolder()`; `None` when absent.
    fn occupant_bool_property(&self, occupant: u64, id: u32) -> Option<bool>;
    /// A u32 property of `occ.AsPropertyHolder()`; `None` when absent.
    fn occupant_u32_property(&self, occupant: u64, id: u32) -> Option<u32>;
    /// `occ.QueryInterface(0x87DDA3A9)` succeeds (the host releases the
    /// interface).
    fn is_building_occupant(&self, occupant: u64) -> bool;
    /// `b.GetBuildingType()`.
    fn building_type(&self, building: u64) -> u32;
    /// `BDS.GetBuildingKeyFromType(b.GetBuildingType())` (BDS slot 14): the
    /// key's instance, or `None` when the lookup fails.
    fn building_key_instance(&self, building: u64) -> Option<u32>;
    /// `occ.IsOccupantGroup(group)`.
    fn is_occupant_group(&self, occupant: u64, group: u32) -> bool;

    // ---- lots (§3, §4.4, §4.6) ----
    /// The lot-state message's IGZUnknown `QueryInterface(0xE818FCED)`: the
    /// lot, with a reference the simulator releases with
    /// [`release`](Self::release); `None` when it is not a lot.
    fn message_lot(&self, unknown: u64) -> Option<u64>;
    /// `lot.GetBuilding()` (lot slot 48), as the building's occupant token.
    fn lot_building(&self, lot: u64) -> Option<u64>;
    /// `lotManager.GetOccupantLot(occ)` (lot manager slot 14).
    fn occupant_lot(&self, occupant: u64) -> Option<u64>;
    /// `lot.GetCondition()` / `lot.SetCondition(c)`.
    fn lot_condition(&self, lot: u64) -> u32;
    fn set_lot_condition(&self, lot: u64, condition: u32);

    // ---- automata (§4.6 step 1) ----
    /// `city.GetAutomataControllerManager()`, if it exists:
    /// `AddControllersFromOccupantGroup(name, occ, 0)` (slot 10). Nothing
    /// without the manager.
    fn add_controllers(&self, name: &str, occupant: u64);

    // ---- save references (§7) ----
    /// How a save refers to a department or an occupant.
    fn object_ref(&self, token: u64) -> RefValue;
    /// The token of a reference read with `iid`; the reference the stream
    /// handed over is transferred to the simulator, and when this returns
    /// `None` the host drops it.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}

/// The host as the simulator holds it.
pub type CivicHostRef = Rc<dyn CivicHost>;
