// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The ordinance subsystem (`docs/specs/ordinance.md`): the ordinance
//! simulator (`cISC4OrdinanceSimulator`, class `0xA990C08E`) and the standard
//! ordinance classes (`cISC4Ordinance`, §3.6). Section references (`§n`)
//! point at that spec.
//!
//! # Structure
//!
//! - [`Ordinance`]: the `cISC4Ordinance` interface as the simulator uses it.
//!   The simulator only talks to ordinances through this trait, so foreign
//!   ordinances (DLL plugins) work: the GZCOM shim wraps them.
//! - [`StandardOrdinance`]: `cSC4OrdinanceSimple` and its 20 subclasses (one
//!   type, the class ID selects the overrides of §3.6), with its save record.
//! - [`OrdinanceSim`]: the map, availability, enact/repeal accounting, the
//!   monthly pass, the cumulative effect queries and the save record.
//! - [`OrdinanceHost`]: everything the subsystem reads from, or does to, the
//!   rest of the game, as plain-data callbacks.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - Loaded map keys (§9.1, quirk 4): `Read` keys the entries by class ID as
//!   the original does (an ordinance read later in the same store load does
//!   not know its ID yet); `Init` after a load re-keys every entry by
//!   `GetID()` (the first entry wins on a clash, entries with ID 0 keep their
//!   class key), so a plugin ordinance whose class ID differs from its ID is
//!   found by ID after a load. This is a documented difference: the original
//!   never re-keys (ordinance.md §9.1).
//! - `GetCumulativeOffset` returns the 32-bit sum sign-extended to 64 bits
//!   (§6.1).
//! - A missing residential simulator reads as population 0; a missing
//!   department or line item as 0 expenses (JuniorSports' condition is then
//!   false); TrashPresort without a budget simulator returns its constant.
//! - `GetRecyclingCenterEffect`: the game's getter returns a pointer to an
//!   `f32` fraction (ordinance.md §3.6, pollution §8); the host trait passes
//!   the dereferenced `f32` (the plugin reads through the pointer).
//! - LegalizeGambling's side effects run on every `SetOn(false)`, also when
//!   the on flag was already clear (§3.6 "every"). `demolish_lots` is called
//!   even with an empty list; the host may skip the utility call then.
//! - References: the simulator's map holds one host reference per entry
//!   (`add_ref` on insert, `release` on erase/replace/drop). A store load
//!   leaves the creation reference with the store (as in the budget), so a
//!   loaded ordinance has two until the store lets go.
//! - Truncation `trunc_s64` of a NaN or out-of-range `f32` saturates (NaN →
//!   0), the Rust cast; a hardware conversion would typically give the
//!   "integer indefinite" `i64::MIN` instead (only reachable with corrupt
//!   data; not specified).
//! - A loaded ordinance whose parent exemplar no longer exists keeps its
//!   saved fields and is written back without a parent key.
//! - Names and descriptions are kept as bytes (round-trip exact); name
//!   lookup compares them ignoring ASCII case.

mod object;
pub mod record;
mod simulator;

pub use object::{OrdinanceData, OrdinanceKind, StandardOrdinance, ORDINANCE_RECORD_VERSION};
pub use simulator::{OrdinanceSim, OrdinanceSnapshot, SIM_RECORD_VERSION};

use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::stream::{RefValue, ResKey};
use crate::variant::Variant;

/// Class and interface IDs (§1).
pub const CLSID_ORDINANCE_SIMULATOR: u32 = 0xA990_C08E;
pub const IID_ORDINANCE_SIMULATOR: u32 = 0x4119_3C3A;
pub const IID_ORDINANCE: u32 = 0xAA5E_2247;
/// `cSC4OrdinanceSimple`: the base class, and the IID the loader requests.
pub const CLSID_ORDINANCE_SIMPLE: u32 = 0x8BD4_FC74;

/// Agent registration (§8.1): level 6 (Month), flags 2.
pub const AGENT_NAME: &str = "cSC4OrdinanceSimulator";
pub const AGENT_LEVEL: u32 = 6;
pub const AGENT_FLAGS: u32 = 2;

/// Ordinance exemplars have ExemplarType 14 (§8.1); the shipped ones are in
/// this group (§3.7).
pub const ORDINANCE_EXEMPLAR_TYPE: u32 = 14;
pub const ORDINANCE_EXEMPLAR_GROUP: u32 = 0xA9C2_C209;

/// Messages (§5, §7).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const ORDINANCE_UPDATED: u32 = 0xC2C0_9554;
    pub const AVAILABILITY_CHANGED: u32 = 0xA2D1_C5B9;
    pub const MONTHLY_DONE: u32 = 0xC8EA_5018;
}

/// Exemplar properties (§3.1, §3.2).
pub mod prop {
    pub const ID: u32 = 0x28EB_BC30;
    pub const NAME: u32 = 0x28EB_BC32;
    pub const DESCRIPTION: u32 = 0x28EB_BC34;
    pub const YEAR_FIRST_AVAILABLE: u32 = 0x28EB_BC35;
    pub const CHANCE_AVAILABILITY: u32 = 0x28EB_BC36;
    pub const ENACTMENT_INCOME: u32 = 0x28EB_BC37;
    pub const RETRACMENT_INCOME: u32 = 0x28EB_BC38;
    pub const MONTHLY_CONSTANT_INCOME: u32 = 0x28EB_BC39;
    pub const MONTHLY_INCOME_FACTOR: u32 = 0x28EB_BC3A;
    pub const ADVISOR_ID: u32 = 0x28EB_BC3B;
    pub const IS_INCOME_ORDINANCE: u32 = 0x28EB_BC3C;
    /// LTEXT keys (three `u32`) of the name and the description.
    pub const NAME_KEY: u32 = 0x6A41_6B21;
    pub const DESCRIPTION_KEY: u32 = 0xCA41_6B2A;
}

/// The casino building type demolished by LegalizeGambling (§3.6), and the
/// 3D-view input control it removes.
pub const CASINO_BUILDING_TYPE: u32 = 0x033A_0000;
pub const CASINO_INPUT_CONTROL: u32 = 0x88F1_54FB;

/// The `cISC4Ordinance` interface (§3.5) as the simulator and other readers
/// use it. Implemented by [`StandardOrdinance`] and, in the shim, by wrappers
/// of foreign ordinance objects.
pub trait Ordinance {
    /// Host identity (the `cISC4Ordinance*` in the shim), used for
    /// reference counting.
    fn token(&self) -> u64;
    fn init(&self) -> bool;
    fn shutdown(&self) -> bool;
    fn current_monthly_income(&self) -> i64;
    fn id(&self) -> u32;
    fn name(&self) -> Vec<u8>;
    fn description(&self) -> Vec<u8>;
    fn year_first_available(&self) -> u32;
    /// A fraction (`SC4Percentage`).
    fn chance_availability(&self) -> f32;
    fn enactment_income(&self) -> i64;
    fn retracment_income(&self) -> i64;
    fn monthly_constant_income(&self) -> i64;
    fn monthly_income_factor(&self) -> f32;
    fn advisor_id(&self) -> u32;
    /// One property of `GetMiscProperties()` (own, then the parent exemplar).
    fn misc_property(&self, id: u32) -> Option<Variant>;
    fn is_available(&self) -> bool;
    fn is_on(&self) -> bool;
    fn is_enabled(&self) -> bool;
    fn monthly_adjusted_income(&self) -> i64;
    fn check_conditions(&self) -> bool;
    fn is_income_ordinance(&self) -> bool;
    fn simulate(&self) -> bool;
    fn set_available(&self, available: bool) -> bool;
    fn set_on(&self, on: bool) -> bool;
    fn set_enabled(&self, enabled: bool) -> bool;
    fn force_available(&self, available: bool) -> bool;
    fn force_on(&self, on: bool) -> bool;
    fn force_monthly_adjusted_income(&self, v: i64) -> bool;
    /// How a save refers to this ordinance (§9.1): a core object, or a host
    /// object by token and class.
    fn save_ref(self: Rc<Self>) -> RefValue;
}

/// What the ordinance subsystem reads from, and does to, the rest of the
/// game. Re-entrancy: callbacks must not call back into the ordinance
/// subsystem.
pub trait OrdinanceHost {
    // ---- resources ----
    /// Every exemplar resource (type `0x6534284A`, any group), in the
    /// resource manager's enumeration order (§8.1).
    fn exemplar_keys(&self) -> Vec<ResKey>;
    /// Loads one exemplar (`cISCResExemplar`), or `None`.
    fn exemplar(&self, key: ResKey) -> Option<Rc<PropertyHolder>>;
    /// The LTEXT string with this key, or `None`.
    fn ltext(&self, key: ResKey) -> Option<Vec<u8>>;

    // ---- objects ----
    /// `GZCOM()->GetClassObject(classID, 0x8BD4FC74)`: a standard ordinance of
    /// that class, if the class is registered. It carries one reference
    /// owned by the caller. Use [`StandardOrdinance::new`].
    fn create_ordinance(&self, class_id: u32) -> Option<Rc<StandardOrdinance>>;
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    /// A host object read from a save (a reference the core cannot resolve
    /// itself), if it answers `cISC4Ordinance`. The reference read from the
    /// stream is handed over.
    fn foreign_ordinance(&self, token: u64) -> Option<Rc<dyn Ordinance>>;

    // ---- messages ----
    /// Posts a message with void1 = the ordinance simulator (§7.1).
    fn post(&self, msg_type: u32, data2: u32, data3: u32);

    // ---- city ----
    /// `cISC4Simulator::GetSimDate()` year; `None` if there is no simulator.
    fn sim_year(&self) -> Option<u32>;
    /// `cISC4ResidentialSimulator::GetPopulation()` (0 if missing).
    fn residential_population(&self) -> i32;
    fn has_budget_simulator(&self) -> bool;
    /// `GetDepartmentBudget(dept)->GetTotalExpenses()`; `None` if missing.
    fn department_total_expenses(&self, dept: u32) -> Option<i64>;
    /// `GetDepartmentBudget(dept)->GetLineItem(line)->GetFullExpenses()`.
    fn line_item_full_expenses(&self, dept: u32, line: u32) -> Option<i64>;
    /// The second output of `cISC4LandfillDeveloper::GetLandfillUsage`.
    fn landfill_capacity(&self) -> u32;
    /// `cISC4PowerSimulator::GetPowerPlantCount(type)`.
    fn power_plant_count(&self, power_type: i32) -> i32;
    /// `cISC4PoliceSimulator::GetPoliceStationCount()`.
    fn police_station_count(&self) -> u32;
    /// `cISC4PollutionSimulator::GetRecyclingCenterEffect()`.
    fn recycling_center_effect(&self) -> f32;

    // ---- LegalizeGambling (§3.6) ----
    /// Every lot whose `GetBuildingType(false)` is `building_type`.
    fn lots_with_building_type(&self, building_type: u32) -> Vec<u64>;
    /// Demolishes the lots in one call of the demolition utility.
    fn demolish_lots(&self, lots: &[u64]);
    /// Sets the civic building simulator's conditional-building status of
    /// `building_type` to Hidden (1), keeping its other fields.
    fn hide_conditional_building(&self, building_type: u32);
    /// Removes the 3D view's current input control if its ID is `control_id`.
    fn remove_view_input_control(&self, control_id: u32);
}

/// Adapter: the host as the property holders' exemplar source.
pub(crate) struct HostExemplars<'a>(pub(crate) &'a dyn OrdinanceHost);

impl crate::property::ExemplarSource for HostExemplars<'_> {
    fn exemplar(&self, key: ResKey) -> Option<Rc<PropertyHolder>> {
        self.0.exemplar(key)
    }
}
