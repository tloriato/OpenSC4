// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The airport and seaport developers (`cSC4PortDeveloper` with its
//! subclasses `cSC4AirportDeveloper` `0xAA2049A1` and
//! `cSC4SeaportDeveloper` `0xEA230A28`), `docs/specs/port-developers.md`.
//! Section references (`§n`) point at that spec.
//!
//! One [`PortDeveloper`] is either kind ([`PortKind`]). It adopts phase-1
//! port lots (§4.1), keeps one budget department (§4.2), runs the month
//! (§4.3, §4.4), grows ports on the advisor's answer (§4.5), forgets
//! destroyed ports (§4.6), answers the queries of §4.7 and, for the
//! seaport, keeps the sea-access map (§4.8, [`water`]). The record (§7) is
//! the subclass and port versions, the tuning instance, the zone type and
//! the port lots.
//!
//! Everything outside the class goes through [`PortHost`]: exemplars, the
//! budget, lots and the lot manager, the lot developer, demand, traffic,
//! history, advisors and the terrain. Objects are host tokens.
//!
//! # Choices (`[choice]`)
//!
//! - `GetLeastEfficientPort` with two or more ports returns the minimum
//!   efficiency, first port on ties (§4.7; the original hangs).
//! - Every scan of the port types is bounded by the loaded count (§6); a
//!   type without a stage count is not loaded; missing per-stage entries
//!   read as 0.
//! - `Read` is atomic for the port list and rejects a type index out of
//!   range (§7). The tuning it loads (with the record's instance) stays
//!   loaded when the rest of the record fails, as the original's order
//!   gives (PD11).
//! - A port whose lot is null (a failed swap, §10 point 8; the original
//!   faults on it, PD7) is skipped by the month, the trips, the growth, the
//!   access recheck and every other walk over the ports, and is left out of
//!   `GetAllPortLots`.
//! - The seaport's terrain grid (§4.8, PD4, PD6) is reproduced in full
//!   except where the original reads or writes out of bounds: a grid index
//!   past the grid's values sets nothing, and a vertex index (the metre
//!   coordinates the original passes to `SetAltitude`) outside the
//!   terrain's `(cellsX + 1) × (cellsZ + 1)` vertices is skipped
//!   [question PD12, PD13]. It never runs with shipped data.
//! - `Shutdown` keeps `tuningLoaded`, `typeCount` and the type instance
//!   list (§3, the original's [quirk]; the spec allows clearing
//!   `tuningLoaded`, FreeCity4 does not).
//! - `GetCellSeaAccess` outside the map answers 0 (no bounds check in the
//!   original).
//! - Threading (foundations §19.3): main-thread only; no state is borrowed
//!   across a host call.

#[cfg(test)]
mod tests;
pub mod tuning;
pub mod water;

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::stream::{InStream, OutStream, SaveObject};
use crate::x87;

pub use tuning::{GrowthRule, PortType, TerrainGrid};
pub use water::WaterMap;

/// Interface IDs (§1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_PORT_DEVELOPER: u32 = 0x4A2C_4E69;
pub const IID_AIRPORT_DEVELOPER: u32 = 0xEA20_548D;
pub const IID_SEAPORT_DEVELOPER: u32 = 0xCA22_FE3E;
pub const IID_MESSAGE_TARGET: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;

/// Class IDs (§1).
pub const CLSID_AIRPORT: u32 = 0xAA20_49A1;
pub const CLSID_SEAPORT: u32 = 0xEA23_0A28;

/// The month agent (§3): one name for both developers.
pub const AGENT_NAME: &str = "cSC4PortDeveloper";
pub const AGENT_LEVEL: u32 = 6;
pub const AGENT_FLAGS: u32 = 2;

/// Messages (§3).
pub const MSG_MONTH: u32 = 0x6695_6816;
pub const MSG_OCCUPANT_INSERTED: u32 = 0x99EF_1142;
pub const MSG_LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;
pub const MSG_POST_CITY_INIT: u32 = 0x26D3_1EC1;
pub const MSG_TERRAIN_CHANGED: u32 = 0x6A3B_0AFD;
pub const MSG_HISTORY: u32 = 0x89EF_A536;

/// Airport growth events and messages (§4.5): by size class 1–3, stage 0
/// then later.
pub const AIRPORT_GROWTH: [[u32; 2]; 3] = [
    [0xEA39_4B74, 0xCB3B_E815],
    [0xEB3B_E415, 0x2B3B_E44A],
    [0x6B3B_E6D8, 0x0B3B_E81A],
];
/// The seaport growth event and message (§4.5).
pub const SEAPORT_GROWTH: u32 = 0x8A39_4DC4;
/// BAD_SEAPORT_LOCATION (§4.4).
pub const EVENT_BAD_SEAPORT: u32 = 0x4A49_0D33;

/// Budget (§4.2).
pub const DEPARTMENT_GROUP: u32 = 0xAA36_9059;
pub const DEPARTMENT_NAME_GROUP: u32 = 0x6A3F_F01C;
pub const MAX_FUNDING: f32 = 1.2;
pub const LINE_ITEM: u32 = 1;

/// Occupants (§3).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
pub const PREVIEW_MARKER: u32 = 0x0292_44DB;

/// Office developer types the airport counts (§4.4).
pub const OFFICE_MEDIUM: u32 = 0x3320;
pub const OFFICE_HIGH: u32 = 0x3330;
/// The freight trip type of the seaport (§4.4).
pub const FREIGHT_TRIPS: u32 = 3;
/// The prop property switched to state 1 without sea access (§4.4).
pub const PROP_SEAPORT_ACCESS: u32 = 0xC982_04B8;

/// Lot states and conditions (§4.3, §4.6, lot-developer.md §5).
pub const LOT_STATE_CONSTRUCTING: u32 = 2;
pub const LOT_STATE_DESTRUCTING: u32 = 5;
pub const CONDITION_NONE: u32 = 0;
pub const CONDITION_UNPOWERED: u32 = 1;
pub const CONDITION_STRESSED: u32 = 2;

/// Growth threshold and condition age (§6).
pub const GROWTH_THRESHOLD: f32 = 0.96;
pub const CONDITION_DAYS: u32 = 90;

/// Record versions (§7).
pub const SUBCLASS_VERSION: u16 = 2;
pub const PORT_VERSION: u16 = 1;

/// Which developer (§1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortKind {
    Airport,
    Seaport,
}

impl PortKind {
    pub fn class_id(self) -> u32 {
        match self {
            PortKind::Airport => CLSID_AIRPORT,
            PortKind::Seaport => CLSID_SEAPORT,
        }
    }
    pub fn iid(self) -> u32 {
        match self {
            PortKind::Airport => IID_AIRPORT_DEVELOPER,
            PortKind::Seaport => IID_SEAPORT_DEVELOPER,
        }
    }
    /// City component (§1).
    pub fn component(self) -> u32 {
        match self {
            PortKind::Airport => 13,
            PortKind::Seaport => 14,
        }
    }
    /// `GetSimulatorType` (§1).
    pub fn simulator_type(self) -> u32 {
        match self {
            PortKind::Airport => 0xEA39_55C4,
            PortKind::Seaport => 0xEA39_57AD,
        }
    }
    /// The initial tuning instance (§2).
    pub fn tuning_instance(self) -> u32 {
        match self {
            PortKind::Airport => tuning::AIRPORT_TUNING,
            PortKind::Seaport => tuning::SEAPORT_TUNING,
        }
    }
    /// Zone type (§2): 11 airport, 12 seaport.
    pub fn zone_type(self) -> u16 {
        match self {
            PortKind::Airport => 11,
            PortKind::Seaport => 12,
        }
    }
    /// History key (§2).
    pub fn history_key(self) -> u32 {
        match self {
            PortKind::Airport => 0x6A3E_9032,
            PortKind::Seaport => 0xEA3E_904A,
        }
    }
    /// Department ID (§2).
    pub fn department(self) -> u32 {
        match self {
            PortKind::Airport => 0x6A41_19F0,
            PortKind::Seaport => 0x4A41_55AB,
        }
    }
    /// The department name's instance (§4.2).
    pub fn department_name(self) -> u32 {
        match self {
            PortKind::Airport => 0xEA49_3564,
            PortKind::Seaport => 0xCA49_3AF3,
        }
    }
    /// Building occupant group (§2).
    pub fn occupant_group(self) -> u32 {
        match self {
            PortKind::Airport => 0x1508,
            PortKind::Seaport => 0x1509,
        }
    }
    /// Is `msg` one of this kind's growth messages (§3)?
    pub fn is_growth_message(self, msg: u32) -> bool {
        match self {
            PortKind::Airport => AIRPORT_GROWTH.iter().flatten().any(|&m| m == msg),
            PortKind::Seaport => msg == SEAPORT_GROWTH,
        }
    }
}

/// `QueryInterface` (§1): the sub-object offset an IID answers with.
pub fn query_interface(kind: PortKind, iid: u32) -> Option<u32> {
    match iid {
        _ if iid == kind.iid() => Some(0),
        IID_UNKNOWN | IID_PORT_DEVELOPER => Some(4),
        IID_MESSAGE_TARGET => Some(8),
        IID_SERIALIZABLE => Some(0xC),
        _ => None,
    }
}

/// A lot's bounding rectangle in cells (inclusive).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellRect {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
}

/// Everything the port developers read from, or do to, the rest of the
/// game. Objects are host tokens. Re-entrancy: callbacks must not call back
/// into the developer (messages the host posts are delivered later).
pub trait PortHost {
    /// The exemplar of type `0x6534284A`, group `0x67CD5FA1` and this
    /// instance (§6).
    fn exemplar(&self, instance: u32) -> Option<Rc<PropertyHolder>>;

    // ---- simulator (§3) ----
    /// `AddAgent(target, level, name, flags)`.
    fn add_agent(&self, name: &str, level: u32, flags: u32);
    /// Removes the agent from every level.
    fn remove_agent(&self, name: &str);

    // ---- budget (§4.2, §4.5) ----
    /// `GetDepartmentBudget(id)` (slot 28).
    fn department(&self, id: u32) -> Option<u64>;
    /// `CreateDepartmentBudget(id, group)`, `SetDepartmentName(nameGroup,
    /// nameInstance)`, `SetMaxAllowedFundingPercentage(max)`, line item
    /// `line` (not locally funded) with display flag 1 cleared.
    fn create_department(&self, id: u32, group: u32, name: (u32, u32), max_funding: f32, line: u32) -> Option<u64>;
    /// Line `line`'s (full, current) expenses; `None` without the line.
    fn line_expenses(&self, department: u64, line: u32) -> Option<(i64, i64)>;
    /// Line `line` `SetFullExpenses(v)`.
    fn set_full_expenses(&self, department: u64, line: u32, v: i64);
    /// `WithdrawFunds(amount)`: the result (ignored by the developer).
    fn withdraw_funds(&self, amount: i64) -> bool;

    // ---- occupants (§3) ----
    /// The occupant's property holder has the preview marker `0x029244DB`.
    fn occupant_is_preview(&self, occupant: u64) -> bool;
    fn occupant_type(&self, occupant: u64) -> u32;
    /// `IsOccupantGroup(group)` (occupant slot 22).
    fn occupant_in_group(&self, occupant: u64, group: u32) -> bool;
    /// `lotManager.GetOccupantLot(occupant)` (slot 14).
    fn occupant_lot(&self, occupant: u64) -> Option<u64>;

    // ---- lots ----
    /// `lot.GetLotConfiguration()`: `(GetID(), GetGrowthStage())`, `None`
    /// without a configuration.
    fn lot_config(&self, lot: u64) -> Option<(u32, u32)>;
    /// `lot.GetLocation()` (slot 21).
    fn lot_location(&self, lot: u64) -> (i16, i16);
    /// `lotManager.GetLot(x, z)`: resolves a loaded port lot (§2).
    fn lot_at(&self, x: i16, z: i16) -> Option<u64>;
    fn lot_state(&self, lot: u64) -> u32;
    fn lot_condition(&self, lot: u64) -> u32;
    fn set_lot_condition(&self, lot: u64, condition: u32);
    fn lot_days_since_state_change(&self, lot: u64) -> u32;
    fn lot_rect(&self, lot: u64) -> CellRect;
    fn lot_facing(&self, lot: u64) -> u32;
    /// The lot configuration is compatible with the zone type (cfg slot 15).
    fn lot_zone_compatible(&self, lot: u64, zone_type: u16) -> bool;

    // ---- growth (§4.5) ----
    /// `lotConfigurationManager.GetLotConfiguration(id)`; `None` (null)
    /// when the manager does not have it (PD5: the swap goes on).
    fn lot_configuration(&self, id: u32) -> Option<u64>;
    /// Lot developer `StartDemolishLot(lot)`.
    fn start_demolish_lot(&self, lot: u64);
    /// Lot developer `EndDemolishLot(lot)`.
    fn end_demolish_lot(&self, lot: u64);
    /// `lotManager.CreateLot(x, z, width, depth, facing)` (slot 23).
    fn create_lot(&self, x: i32, z: i32, width: i32, depth: i32, facing: u32) -> Option<u64>;
    /// `lot.SetLotConfiguration(cfg)` then `lot.UpdateZoneType()`; `cfg`
    /// may be null (`None`, PD5).
    fn set_lot_configuration(&self, lot: u64, config: Option<u64>);
    /// `ConstructDefaultLot(lot, cfg, 0, 1)`; `cfg` may be null (`None`,
    /// PD5: the lot developer then leaves an empty lot in state 2 with
    /// condition 1 and answers true, lot-developer.md §6.4).
    fn construct_default_lot(&self, lot: u64, config: Option<u64>) -> bool;
    /// `PlaceZone` with zone 0 over the rectangle, for free (§4.6).
    fn dezone(&self, rect: CellRect);

    // ---- demand, history, advisors ----
    /// `demand.UpdateOccupantEffects(lot.GetBuildingType(0), &old, &new)`.
    fn update_occupant_effects(&self, lot: u64, old: f32, new: f32);
    /// History record `0x89EFA536` (data1 = key, data2 = value).
    fn record_history(&self, key: u32, value: i32);
    /// Advisor system `PushEvent(event, null)`.
    fn push_event(&self, event: u32);

    // ---- traffic (§4.4) ----
    /// `GetOccupantCountForAllSubnetworks(type)`: one entry per subnetwork.
    fn office_counts(&self, developer_type: u32) -> Vec<u32>;
    /// `GetSubnetworksForLot(lot)`.
    fn lot_subnetworks(&self, lot: u64) -> Vec<u32>;
    /// `GetTrafficArrived(lot, type)`.
    fn traffic_arrived(&self, lot: u64, trip_type: u32) -> u32;
    /// `SetMaxTripCapacity(lot, type, capacity)`.
    fn set_max_trip_capacity(&self, lot: u64, trip_type: u32, capacity: u32);

    // ---- seaport (§4.4, §4.8) ----
    /// The terrain size in cells (terrain slots 7, 8).
    fn terrain_size(&self) -> (u32, u32);
    /// `terrain.CellHasWater(x, z)` (slot 34).
    fn cell_has_water(&self, x: u32, z: u32) -> bool;
    /// Switches the lot's props with property `0xC98204B8` to state 1;
    /// returns how many.
    fn set_access_props_state(&self, lot: u64, property: u32, state: u32) -> u32;
    /// `terrain.SetAltitude(x, z, altitude)` (`cISTETerrain` slot 51):
    /// `x`, `z` are vertex indices. The seaport passes the truncated metre
    /// coordinates of a cell centre (§4.8 step 4, PD6), as the original
    /// does; the developer has already skipped any outside the terrain.
    fn set_altitude(&self, x: i32, z: i32, altitude: f32);
}

/// A port lot (`cPortLot`, §2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortLot {
    pub stage: u32,
    /// Index into the loaded types.
    pub type_index: usize,
    pub needs_lookup: bool,
    pub x: i16,
    pub z: i16,
    pub efficiency: f32,
    pub swapping: bool,
    pub usage: f32,
    pub lot: Option<u64>,
    /// Airport only.
    pub trips: u32,
}

/// The fields of §2 (not the port lists).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortState {
    pub tuning_loaded: bool,
    pub initialized: bool,
    pub tuning_instance: u32,
    /// The length of the tuning exemplar's type list.
    pub type_count: u32,
    pub department: Option<u64>,
    pub zone_type: u16,
    pub ref_count: u32,
    pub destroyed: bool,
    // Seaport only.
    pub recheck_access: bool,
    pub rebuild_map: bool,
    pub city_has_sea: bool,
}

/// What the last month did (diagnostics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonthOutcome {
    pub ran: bool,
    /// The history value (when ran).
    pub history: i32,
    /// The advisor event pushed, 0 for none.
    pub event: u32,
    pub skipped: u32,
}

/// One port developer.
pub struct PortDeveloper {
    kind: PortKind,
    host: Rc<dyn PortHost>,
    token: Cell<u64>,
    s: Cell<PortState>,
    ports: RefCell<Vec<PortLot>>,
    types: RefCell<Vec<PortType>>,
    type_instances: RefCell<Vec<u32>>,
    map: RefCell<WaterMap>,
    last: Cell<MonthOutcome>,
}

fn f(v: u32) -> f32 {
    v as f32
}

impl PortDeveloper {
    pub fn new(kind: PortKind, host: Rc<dyn PortHost>, token: u64) -> Rc<PortDeveloper> {
        Rc::new(PortDeveloper {
            kind,
            host,
            token: Cell::new(token),
            s: Cell::new(PortState {
                tuning_loaded: false,
                initialized: false,
                tuning_instance: kind.tuning_instance(),
                type_count: 0,
                department: None,
                zone_type: kind.zone_type(),
                ref_count: 0,
                destroyed: false,
                recheck_access: false,
                rebuild_map: false,
                city_has_sea: false,
            }),
            ports: RefCell::new(Vec::new()),
            types: RefCell::new(Vec::new()),
            type_instances: RefCell::new(Vec::new()),
            map: RefCell::new(WaterMap::default()),
            last: Cell::new(MonthOutcome::default()),
        })
    }

    pub fn kind(&self) -> PortKind {
        self.kind
    }
    pub fn token(&self) -> u64 {
        self.token.get()
    }
    pub fn set_token(&self, t: u64) {
        self.token.set(t);
    }
    pub fn snapshot(&self) -> PortState {
        self.s.get()
    }
    pub fn ports(&self) -> Vec<PortLot> {
        self.ports.borrow().clone()
    }
    pub fn types(&self) -> Vec<PortType> {
        self.types.borrow().clone()
    }
    pub fn water_map(&self) -> WaterMap {
        self.map.borrow().clone()
    }
    pub fn last_month(&self) -> MonthOutcome {
        self.last.get()
    }

    fn edit(&self, f: impl FnOnce(&mut PortState)) {
        let mut s = self.s.get();
        f(&mut s);
        self.s.set(s);
    }

    fn port(&self, i: usize) -> PortLot {
        self.ports.borrow()[i]
    }

    fn set_port(&self, i: usize, p: PortLot) {
        self.ports.borrow_mut()[i] = p;
    }

    fn port_count(&self) -> usize {
        self.ports.borrow().len()
    }

    /// Replaces one port lot's fields (tests and tools).
    pub fn set_port_fields(&self, i: usize, p: PortLot) -> bool {
        if i < self.port_count() {
            self.set_port(i, p);
            true
        } else {
            false
        }
    }

    // ---- identity ----

    pub fn add_ref(&self) -> u32 {
        let mut n = 0;
        self.edit(|s| {
            s.ref_count = s.ref_count.wrapping_add(1);
            n = s.ref_count;
        });
        n
    }

    /// The last release calls `Shutdown` and destroys the object (here:
    /// marks it).
    pub fn release(&self) -> u32 {
        let s = self.s.get();
        if s.ref_count > 1 {
            self.edit(|s| s.ref_count -= 1);
            return s.ref_count - 1;
        }
        self.shutdown();
        self.edit(|s| {
            s.ref_count = 0;
            s.destroyed = true;
        });
        0
    }

    // ---- tuning (§6) ----

    /// Loads the tuning once, if the instance is non-zero.
    fn load_tuning(&self) {
        let s = self.s.get();
        if s.tuning_loaded || s.tuning_instance == 0 {
            return;
        }
        self.edit(|s| s.tuning_loaded = true);
        let list: Vec<u32> = self
            .host
            .exemplar(s.tuning_instance)
            .and_then(|h| {
                h.get(tuning::PROP_TYPE_LIST)
                    .map(|v| v.items().iter().map(|e| e.as_i64() as u32).collect())
            })
            .unwrap_or_default();
        let mut types = Vec::new();
        for &inst in &list {
            if let Some(t) = self.host.exemplar(inst).and_then(|h| PortType::from_exemplar(inst, &h)) {
                types.push(t);
            }
        }
        self.edit(|s| s.type_count = list.len() as u32);
        *self.type_instances.borrow_mut() = list;
        *self.types.borrow_mut() = types;
    }

    // ---- Init / Shutdown ----

    /// Slot 3 `Init` (§3, PD10): guarded by `initialized` (a second call
    /// returns true at once). The base: `initialized := 1`, the tuning
    /// (§6), the agent (added without removing anything first); then the
    /// subclass: the department (§4.2) and, for the seaport, the map
    /// (§4.8). Returns true.
    pub fn init(&self) -> bool {
        if self.s.get().initialized {
            return true;
        }
        self.edit(|s| s.initialized = true);
        self.load_tuning();
        let h = &*self.host;
        h.add_agent(AGENT_NAME, AGENT_LEVEL, AGENT_FLAGS);
        let id = self.kind.department();
        let dept = h.department(id).or_else(|| {
            h.create_department(
                id,
                DEPARTMENT_GROUP,
                (DEPARTMENT_NAME_GROUP, self.kind.department_name()),
                MAX_FUNDING,
                LINE_ITEM,
            )
        });
        self.edit(|s| s.department = dept);
        if self.kind == PortKind::Seaport {
            self.build_water_map();
        }
        true
    }

    /// Slot 4 `Shutdown` (§3, PD10): nothing without `Init`. Otherwise
    /// `initialized := 0`; every port lot is deleted (its lot resolved first
    /// when `needsLookup` is set) and the port list emptied; the port types
    /// are deleted and their vector emptied; the agent is removed and the
    /// department dropped. `tuningLoaded`, `typeCount` and the type
    /// instances are kept [quirk]: an `Init` or `Read` after a `Shutdown`
    /// finds no port types. Returns true.
    pub fn shutdown(&self) -> bool {
        if !self.s.get().initialized {
            return true;
        }
        self.edit(|s| s.initialized = false);
        for i in 0..self.port_count() {
            self.lot_of(i);
        }
        self.ports.borrow_mut().clear();
        self.types.borrow_mut().clear();
        self.host.remove_agent(AGENT_NAME);
        self.edit(|s| s.department = None);
        true
    }

    // ---- lots ----

    /// The port's lot, fetched again from the lot manager if `needsLookup`.
    fn lot_of(&self, i: usize) -> Option<u64> {
        let mut p = self.port(i);
        if p.needs_lookup {
            p.lot = self.host.lot_at(p.x, p.z);
            p.needs_lookup = false;
            self.set_port(i, p);
        }
        p.lot
    }

    fn rule(&self, p: &PortLot, stage: u32) -> Option<GrowthRule> {
        self.types
            .borrow()
            .get(p.type_index)
            .and_then(|t| t.rules.get(stage as usize).copied())
    }

    fn capacity(&self, p: &PortLot) -> u32 {
        self.rule(p, p.stage).map_or(0, |r| r.capacity)
    }

    fn threshold(&self, p: &PortLot) -> f32 {
        self.types.borrow().get(p.type_index).map_or(0.0, |t| t.threshold)
    }

    fn size_class(&self, p: &PortLot) -> u8 {
        self.types.borrow().get(p.type_index).map_or(0, |t| t.size_class)
    }

    fn has_next_stage(&self, p: &PortLot) -> bool {
        self.types
            .borrow()
            .get(p.type_index)
            .is_some_and(|t| p.stage.wrapping_add(1) < t.stage_count && (p.stage as usize + 1) < t.rules.len())
    }

    // ---- budget (§4.2) ----

    /// `RequiredBudget()`: Σ of the current stages' monthly costs.
    pub fn required_budget(&self) -> i64 {
        let ports = self.ports();
        ports
            .iter()
            .map(|p| self.rule(p, p.stage).map_or(0, |r| r.monthly_cost))
            .fold(0i64, i64::wrapping_add)
    }

    /// `AllocatedFunding()`: line 1's current expenses (0 without it).
    pub fn allocated_funding(&self) -> i64 {
        self.s
            .get()
            .department
            .and_then(|d| self.host.line_expenses(d, LINE_ITEM))
            .map_or(0, |(_, cur)| cur)
    }

    fn line(&self) -> Option<(i64, i64)> {
        self.s
            .get()
            .department
            .and_then(|d| self.host.line_expenses(d, LINE_ITEM))
    }

    /// The funding ratio `fr` (§4.2).
    pub fn funding_ratio(&self) -> f32 {
        match self.line() {
            Some((full, cur)) if full > 0 => (cur as f32) / (full as f32),
            _ => 1.0,
        }
    }

    /// `UpdateBudget()`.
    pub fn update_budget(&self) {
        if let Some(d) = self.s.get().department {
            let v = self.required_budget();
            self.host.set_full_expenses(d, LINE_ITEM, v);
        }
    }

    // ---- messages (§3) ----

    /// `0x99EF1142` occupant inserted.
    pub fn occupant_inserted(&self, occupant: u64) -> bool {
        let h = &*self.host;
        if !h.occupant_is_preview(occupant)
            && h.occupant_type(occupant) == OCCUPANT_TYPE_BUILDING
            && h.occupant_in_group(occupant, self.kind.occupant_group())
        {
            if let Some(lot) = h.occupant_lot(occupant) {
                self.adopt(lot);
            }
        }
        true
    }

    /// Adoption (§4.1). Returns whether a port was added.
    pub fn adopt(&self, lot: u64) -> bool {
        let h = &*self.host;
        let Some((cfg, growth_stage)) = h.lot_config(lot) else {
            return false;
        };
        if growth_stage != 1 {
            return false;
        }
        let (x, z) = h.lot_location(lot);
        let Some(type_index) = self
            .types
            .borrow()
            .iter()
            .position(|t| t.rules.first().is_some_and(|r| r.lot_config == cfg))
        else {
            return false;
        };
        let p = PortLot {
            stage: 0,
            type_index,
            needs_lookup: false,
            x,
            z,
            efficiency: 1.0,
            swapping: false,
            usage: 0.0,
            lot: Some(lot),
            trips: 0,
        };
        self.ports.borrow_mut().push(p);
        self.update_budget();
        self.on_new_lot_created(self.port_count() - 1);
        true
    }

    /// `0xE8BDA2E7` lot state changed (§4.6).
    pub fn lot_state_changed(&self, lot: u64, _old: u32, new: u32) -> bool {
        if new != LOT_STATE_DESTRUCTING {
            return true;
        }
        let h = &*self.host;
        if !h.lot_zone_compatible(lot, self.s.get().zone_type) {
            return true;
        }
        let (x, z) = h.lot_location(lot);
        let Some(i) = self.ports.borrow().iter().position(|p| p.x == x && p.z == z) else {
            return true;
        };
        let mut p = self.port(i);
        if p.swapping {
            p.swapping = false;
            self.set_port(i, p);
            return true;
        }
        h.update_occupant_effects(lot, p.usage, 0.0);
        p.usage = 0.0;
        self.set_port(i, p);
        h.dezone(h.lot_rect(lot));
        self.ports.borrow_mut().remove(i);
        self.update_budget();
        true
    }

    /// `0x26D31EC1` PostCityInit: resolves every port's lot.
    pub fn post_city_init(&self) -> bool {
        for i in 0..self.port_count() {
            self.lot_of(i);
        }
        true
    }

    /// A growth message (§3, §4.5): any of the kind's growth messages runs
    /// `GrowPortsToNextStage`. Returns true.
    pub fn growth_message(&self, msg: u32) -> bool {
        if self.kind.is_growth_message(msg) {
            self.grow_ports_to_next_stage();
        }
        true
    }

    /// `0x6A3B0AFD` (seaport): rebuild the map, recheck access.
    pub fn terrain_changed(&self) -> bool {
        if self.kind == PortKind::Seaport {
            self.build_water_map();
            self.edit(|s| s.recheck_access = true);
        }
        true
    }

    /// `DoMessage` for the subscriptions of §3 (`data` is the occupant or
    /// the lot; `old`/`new` the lot states). Returns true.
    pub fn do_message(&self, msg: u32, data: u64, old: u32, new: u32) -> bool {
        match msg {
            MSG_MONTH => self.month_tick(),
            MSG_OCCUPANT_INSERTED => self.occupant_inserted(data),
            MSG_LOT_STATE_CHANGED => self.lot_state_changed(data, old, new),
            MSG_POST_CITY_INIT => self.post_city_init(),
            MSG_TERRAIN_CHANGED => self.terrain_changed(),
            m => self.growth_message(m),
        }
    }

    // ---- the month (§4.3, §4.4) ----

    /// The agent (`0x66956816`): `DoMonthlyTick` then the shared month.
    pub fn month_tick(&self) -> bool {
        match self.kind {
            PortKind::Airport => self.airport_tick(),
            PortKind::Seaport => self.seaport_tick(),
        }
        self.month();
        true
    }

    /// `GetTripsForPort(p)` (§4.4).
    pub fn trips_for_port(&self, i: usize) -> u32 {
        match self.kind {
            PortKind::Airport => self.port(i).trips,
            PortKind::Seaport => match self.lot_of(i) {
                Some(lot) => self.host.traffic_arrived(lot, FREIGHT_TRIPS),
                None => 0,
            },
        }
    }

    /// `Efficiency(c, t)` (§4.3).
    pub fn efficiency(c: f32, t: u32) -> f32 {
        if t == 0 {
            1.0
        } else if c <= 0.0 || f(t) > c {
            0.0
        } else {
            1.0 - f(t) / c
        }
    }

    /// `GrowthTriggered(p)` (§4.3).
    fn growth_triggered(&self, i: usize) -> bool {
        let p = self.port(i);
        if !self.has_next_stage(&p) {
            return false;
        }
        let Some(lot) = self.lot_of(i) else {
            return false;
        };
        if self.host.lot_state(lot) == LOT_STATE_CONSTRUCTING {
            return false;
        }
        let trips = self.trips_for_port(i);
        f(trips) >= f(self.capacity(&p)) * GROWTH_THRESHOLD
    }

    /// `PortUsageValues` of port `i` (the internal `0x006E9770` the month
    /// calls, §4.3, PD8): `(min(trips, capacity), capacity)`, no funding
    /// scaling.
    fn port_usage_values(&self, i: usize) -> (u32, u32) {
        let cap = self.capacity(&self.port(i));
        let trips = self.trips_for_port(i);
        (trips.min(cap), cap)
    }

    /// The public query of port `i` (slot 12, §4.7): [`Self::port_usage_values`],
    /// then with line 1 and `full > 0` the funded cap
    /// `trunc(float(capacity) / full × float(current))` (each operation
    /// rounded to float, the int64 operands entering exactly) and the usage
    /// clamped to it again.
    fn usage_values_of(&self, i: usize) -> (u32, u32) {
        let (usage, capacity) = self.port_usage_values(i);
        match self.line() {
            Some((full, cur)) if full > 0 => {
                let q = (f64::from(capacity) / full as f64) as f32;
                let cap = x87::ftol32(f64::from(x87::mul_i64_f32(cur, q))) as u32;
                (usage.min(cap), cap)
            }
            _ => (usage, capacity),
        }
    }

    /// `AverageEfficiency()`: 1.0 without ports.
    pub fn average_efficiency(&self) -> f32 {
        let ports = self.ports.borrow();
        if ports.is_empty() {
            return 1.0;
        }
        let sum: f32 = ports.iter().map(|p| p.efficiency).sum();
        sum / ports.len() as f32
    }

    /// The shared month (§4.3).
    pub fn month(&self) {
        let mut out = MonthOutcome::default();
        if self.port_count() == 0 {
            self.last.set(out);
            return;
        }
        out.ran = true;
        let h = &*self.host;
        let funded = self.allocated_funding() as f32;
        let ratio = funded / self.required_budget() as f32;
        let mut offered = false;
        for i in 0..self.port_count() {
            let Some(lot) = self.lot_of(i) else {
                out.skipped += 1;
                continue;
            };
            let state = h.lot_state(lot);
            if h.lot_condition(lot) == CONDITION_UNPOWERED {
                out.skipped += 1;
                continue;
            }
            let trips = self.trips_for_port(i);
            let mut p = self.port(i);
            p.efficiency = Self::efficiency(f(self.capacity(&p)) * ratio, trips);
            self.set_port(i, p);
            // PD8: the plain capacity (the funded cap is the query's).
            let (usage, cap) = self.port_usage_values(i);
            let new = f(usage) / f(cap);
            h.update_occupant_effects(lot, p.usage, new);
            p.usage = new;
            self.set_port(i, p);
            let stressed = (p.efficiency <= self.threshold(&p) || funded <= 0.0)
                && h.lot_days_since_state_change(lot) > CONDITION_DAYS;
            h.set_lot_condition(lot, if stressed { CONDITION_STRESSED } else { CONDITION_NONE });
            if matches!(state, 1 | 3 | 4) && !offered && self.growth_triggered(i) {
                out.event = self.send_upgrade_advice(i);
                offered = true;
            }
        }
        out.history = x87::rhe(self.average_efficiency() * 100.0);
        h.record_history(self.kind.history_key(), out.history);
        self.last.set(out);
    }

    /// `SendAdvisorUpgradeMessage(p)` (§4.5): the pushed event.
    fn send_upgrade_advice(&self, i: usize) -> u32 {
        let p = self.port(i);
        let event = match self.kind {
            PortKind::Seaport => SEAPORT_GROWTH,
            PortKind::Airport => match self.size_class(&p) {
                s @ 1..=3 => AIRPORT_GROWTH[s as usize - 1][usize::from(p.stage != 0)],
                _ => 0,
            },
        };
        self.host.push_event(event);
        event
    }

    /// The airport's `DoMonthlyTick` (§4.4).
    fn airport_tick(&self) {
        for p in self.ports.borrow_mut().iter_mut() {
            p.trips = 0;
        }
        let h = &*self.host;
        let a = h.office_counts(OFFICE_MEDIUM);
        let b = h.office_counts(OFFICE_HIGH);
        let fr = self.funding_ratio();
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); a.len()];
        for i in 0..self.port_count() {
            let Some(lot) = self.lot_of(i) else {
                continue;
            };
            for s in h.lot_subnetworks(lot) {
                if let Some(bk) = buckets.get_mut(s as usize) {
                    bk.push(i);
                }
            }
        }
        let trunc = |v: f32| x87::ftol32(f64::from(v));
        for (s, bucket) in buckets.iter().enumerate() {
            if bucket.is_empty() {
                continue;
            }
            let sum: u32 = bucket
                .iter()
                .map(|&i| self.capacity(&self.port(i)))
                .fold(0u32, u32::wrapping_add);
            let d = trunc(f(sum) * fr);
            let jobs = a[s].wrapping_add(b.get(s).copied().unwrap_or(0));
            for &i in bucket {
                let mut p = self.port(i);
                let own = trunc(f(self.capacity(&p)) * fr);
                let share = trunc(own as f32 / d as f32 * f(jobs));
                p.trips = p.trips.wrapping_add(share as u32);
                self.set_port(i, p);
            }
        }
    }

    /// `UpdateTripCapacity(p)` (seaport, §4.4).
    fn update_trip_capacity(&self, i: usize) {
        let Some(lot) = self.lot_of(i) else {
            return;
        };
        let cap = x87::ftol32(f64::from(f(self.capacity(&self.port(i))) * self.funding_ratio()));
        self.host.set_max_trip_capacity(lot, FREIGHT_TRIPS, cap as u32);
    }

    /// The seaport's `DoMonthlyTick` (§4.4).
    fn seaport_tick(&self) {
        if self.s.get().rebuild_map {
            self.build_water_map();
        }
        for i in 0..self.port_count() {
            self.update_trip_capacity(i);
        }
        if self.s.get().recheck_access {
            self.edit(|s| s.recheck_access = false);
            // PD9: every listed port without access sets the event flag,
            // at any stage and whatever the prop switch finds.
            let mut any = false;
            for i in 0..self.port_count() {
                let Some(lot) = self.lot_of(i) else {
                    continue;
                };
                if !self.lot_has_access(lot) {
                    any = true;
                    self.host.set_access_props_state(lot, PROP_SEAPORT_ACCESS, 1);
                }
            }
            if any {
                self.host.push_event(EVENT_BAD_SEAPORT);
            }
        }
    }

    // ---- growth (§4.5) ----

    /// `GrowPortsToNextStage`: every triggered port. Returns how many grew.
    pub fn grow_ports_to_next_stage(&self) -> u32 {
        let mut grown = 0;
        for i in 0..self.port_count() {
            if !self.growth_triggered(i) {
                continue;
            }
            let p = self.port(i);
            let Some(rule) = self.rule(&p, p.stage + 1) else {
                continue;
            };
            if !self.swap(i, rule.lot_config) {
                continue;
            }
            let mut p = self.port(i);
            p.stage += 1;
            self.set_port(i, p);
            // The result is ignored [quirk].
            self.host.withdraw_funds(rule.upgrade_cost);
            if self.kind == PortKind::Seaport {
                self.update_trip_capacity(i);
            }
            self.update_budget();
            grown += 1;
        }
        grown
    }

    /// Swaps port `i`'s lot for one with configuration `config_id`. A
    /// missing configuration is not tested (PD5): the null configuration
    /// goes to `SetLotConfiguration` and `ConstructDefaultLot`, whose answer
    /// decides (true in the original: the port is left on an empty lot in
    /// state 2 that never grows again).
    fn swap(&self, i: usize, config_id: u32) -> bool {
        let h = &*self.host;
        let Some(old) = self.lot_of(i) else {
            return false;
        };
        let config = h.lot_configuration(config_id);
        let mut p = self.port(i);
        p.swapping = true;
        self.set_port(i, p);
        let facing = h.lot_facing(old);
        let r = h.lot_rect(old);
        h.start_demolish_lot(old);
        h.end_demolish_lot(old);
        p.lot = None;
        self.set_port(i, p);
        let Some(new) = h.create_lot(r.x0, r.z0, r.x1 - r.x0 + 1, r.z1 - r.z0 + 1, facing) else {
            return false;
        };
        p.lot = Some(new);
        self.set_port(i, p);
        h.set_lot_configuration(new, config);
        h.construct_default_lot(new, config)
    }

    // ---- seaport (§4.8) ----

    fn build_water_map(&self) {
        let h = &*self.host;
        let (w, d) = h.terrain_size();
        let (map, sea) = WaterMap::build(w, d, |x, z| h.cell_has_water(x, z));
        *self.map.borrow_mut() = map;
        self.edit(|s| {
            s.city_has_sea = sea;
            s.rebuild_map = false;
        });
    }

    fn lot_has_access(&self, lot: u64) -> bool {
        let r = self.host.lot_rect(lot);
        self.map.borrow().rect_has_access(r.x0, r.z0, r.x1, r.z1)
    }

    /// Does port `i` have sea access (§4.8)?
    pub fn port_has_access(&self, i: usize) -> bool {
        self.lot_of(i).is_some_and(|l| self.lot_has_access(l))
    }

    /// `OnNewLotCreated(p)` (seaport, §4.8): the trip capacity, a recheck,
    /// then the terrain grid if the port type has one.
    fn on_new_lot_created(&self, i: usize) {
        if self.kind != PortKind::Seaport {
            return;
        }
        self.update_trip_capacity(i);
        self.edit(|s| s.recheck_access = true);
        let p = self.port(i);
        let grid = self.types.borrow().get(p.type_index).and_then(|t| t.grid.clone());
        let (Some(grid), Some(lot)) = (grid, p.lot) else {
            return;
        };
        self.apply_terrain_grid(lot, &grid);
    }

    /// §4.8 steps 2–4 (PD4, PD6): the use test compares only the product
    /// `R × C` with `(x2 − x1)(z2 − z1)` of the inclusive rectangle; the walk
    /// covers `z1 ≤ z < z2` (outer) and `x1 ≤ x < x2` (inner), the last row
    /// and column of the lot excluded; the value is picked by the facing
    /// (facing 0: rows along x; 2: its 180° turn; 1 and 3: the transpose and
    /// its 180° turn; any other facing sets nothing); the write passes the
    /// truncated metre coordinates of the cell centre as vertex indices.
    fn apply_terrain_grid(&self, lot: u64, grid: &TerrainGrid) {
        let h = &*self.host;
        let r = h.lot_rect(lot);
        let walk = i64::from(r.x1 - r.x0) * i64::from(r.z1 - r.z0);
        if i64::from(grid.rows) * i64::from(grid.columns) != walk {
            return;
        }
        let facing = h.lot_facing(lot);
        if facing > 3 {
            return;
        }
        let (cells_x, cells_z) = h.terrain_size();
        let (vx, vz) = (i64::from(cells_x) + 1, i64::from(cells_z) + 1);
        let columns = i64::from(grid.columns);
        for z in r.z0..r.z1 {
            for x in r.x0..r.x1 {
                let (row, col) = match facing {
                    0 => (x - r.x0, z - r.z0),
                    1 => (r.z1 - 1 - z, r.x1 - 1 - x),
                    2 => (r.x1 - 1 - x, r.z1 - 1 - z),
                    _ => (z - r.z0, x - r.x0),
                };
                // No bounds check in the original: with matching products
                // and different sides the flat index runs on [quirk]; past
                // the values it would read beyond the grid [choice: skip].
                let idx = i64::from(row) * columns + i64::from(col);
                let Some(&alt) = usize::try_from(idx).ok().and_then(|k| grid.values.get(k)) else {
                    continue;
                };
                // `CellCenterToPosition` (`cISC4City` slot 97), truncated.
                let px = x87::ftol32((f64::from(x) + 0.5) * 16.0);
                let pz = x87::ftol32((f64::from(z) + 0.5) * 16.0);
                // Slot 51 takes vertex indices `(cellsX + 1) × z + x` with no
                // bounds check [quirk]; one outside the vertex array is
                // skipped [choice].
                let vertex = vx * i64::from(pz) + i64::from(px);
                if !(0..vx * vz).contains(&vertex) {
                    continue;
                }
                h.set_altitude(px, pz, alt);
            }
        }
    }

    // ---- queries (§4.7) ----

    /// `GetAvgEfficiency()`.
    pub fn avg_efficiency(&self) -> f32 {
        self.average_efficiency()
    }

    /// `GetLeastEfficientPort`: `(efficiency, x, z)`. [choice] The minimum,
    /// first port on ties.
    pub fn least_efficient_port(&self) -> Option<(f32, i16, i16)> {
        let ports = self.ports.borrow();
        let mut best: Option<&PortLot> = None;
        for p in ports.iter() {
            if best.map_or(true, |b| p.efficiency < b.efficiency) {
                best = Some(p);
            }
        }
        best.map(|p| (p.efficiency, p.x, p.z))
    }

    /// `GetAllPortLots`: every port's lot, resolved.
    pub fn all_port_lots(&self) -> Vec<u64> {
        (0..self.port_count()).filter_map(|i| self.lot_of(i)).collect()
    }

    /// `GetNumPorts(sizeFlags, stageFlags)`: size bit `1 << sizeClass`.
    pub fn num_ports(&self, size_flags: u32, stage_flags: u32) -> u32 {
        let ports = self.ports();
        ports
            .iter()
            .filter(|p| {
                let size = 1u32.checked_shl(u32::from(self.size_class(p))).unwrap_or(0);
                let stage = 1u32.checked_shl(p.stage).unwrap_or(0);
                size_flags & size != 0 && stage_flags & stage != 0
            })
            .count() as u32
    }

    /// `GetNumAirports(size)`.
    pub fn num_airports(&self, size: u32) -> u32 {
        if size == 0 {
            self.port_count() as u32
        } else {
            self.num_ports(1u32.checked_shl(size).unwrap_or(0), 0x1F)
        }
    }

    /// `GetNumSeaports()`.
    pub fn num_seaports(&self) -> u32 {
        self.port_count() as u32
    }

    fn port_of_lot(&self, lot: u64) -> Option<usize> {
        (0..self.port_count()).find(|&i| self.lot_of(i) == Some(lot))
    }

    /// `Get…UsageValues(lot)`: `(usage, cap)`, `None` if not a port.
    pub fn usage_values(&self, lot: u64) -> Option<(u32, u32)> {
        self.port_of_lot(lot).map(|i| self.usage_values_of(i))
    }

    /// `GetNum…Trips`: Σ `GetTripsForPort`.
    pub fn num_trips(&self) -> u32 {
        (0..self.port_count())
            .map(|i| self.trips_for_port(i))
            .fold(0u32, u32::wrapping_add)
    }

    /// `GetMaintenanceCost(lot)`: the stage's monthly cost × `fr`.
    pub fn maintenance_cost(&self, lot: u64) -> i64 {
        let Some(i) = self.port_of_lot(lot) else {
            return 0;
        };
        let p = self.port(i);
        let cost = self.rule(&p, p.stage).map_or(0, |r| r.monthly_cost);
        x87::ftol(f64::from(x87::mul_i64_f32(cost, self.funding_ratio())))
    }

    /// `CityHasSea()` (seaport).
    pub fn city_has_sea(&self) -> bool {
        self.s.get().city_has_sea
    }

    /// `GetCellSeaAccess(x, z)` (seaport): the map byte.
    pub fn cell_sea_access(&self, x: i32, z: i32) -> u8 {
        self.map.borrow().get(x, z)
    }

    // ---- record (§7) ----

    fn parse(&self, inp: &mut dyn InStream) -> Option<Vec<PortLot>> {
        if inp.get_u16()? != SUBCLASS_VERSION || inp.get_u16()? != PORT_VERSION {
            return None;
        }
        let instance = inp.get_u32()?;
        let zone_type = inp.get_u16()?;
        if !self.s.get().tuning_loaded {
            self.edit(|s| s.tuning_instance = instance);
            self.load_tuning();
        }
        let n = inp.get_u16()?;
        let types = self.types.borrow().len();
        let mut ports = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let stage = inp.get_u32()?;
            let index = inp.get_u16()? as usize;
            let x = inp.get_i16()?;
            let z = inp.get_i16()?;
            if index >= types {
                return None;
            }
            ports.push(PortLot {
                stage,
                type_index: index,
                needs_lookup: true,
                x,
                z,
                efficiency: 1.0,
                swapping: false,
                usage: 0.0,
                lot: None,
                trips: 0,
            });
        }
        if inp.error() != 0 {
            return None;
        }
        self.edit(|s| {
            s.tuning_instance = instance;
            s.zone_type = zone_type;
        });
        Some(ports)
    }
}

impl SaveObject for PortDeveloper {
    fn class_id(&self) -> u32 {
        self.kind.class_id()
    }

    /// §7. Always true (unless the stream fails).
    fn write(&self, out: &mut dyn OutStream) -> bool {
        let s = self.s.get();
        out.put_u16(SUBCLASS_VERSION);
        out.put_u16(PORT_VERSION);
        out.put_u32(s.tuning_instance);
        out.put_u16(s.zone_type);
        let ports = self.ports();
        out.put_u16(ports.len() as u16);
        for p in &ports {
            out.put_u32(p.stage);
            out.put_u16(p.type_index as u16);
            out.put_i16(p.x);
            out.put_i16(p.z);
        }
        out.error() == 0
    }

    /// §7: appends the saved ports. [choice] Atomic for the port list.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        match self.parse(inp) {
            Some(ports) => {
                self.ports.borrow_mut().extend(ports);
                true
            }
            None => false,
        }
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
