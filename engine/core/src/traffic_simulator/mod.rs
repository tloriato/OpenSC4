// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The traffic simulator `cSC4TrafficSimulator`
//! (`docs/specs/traffic-simulator.md`). Section references (`§n`) point at
//! that spec unless they name another one.
//!
//! The simulator turns lots and neighbour connections into commute and
//! freight trips, routes them with the pathfinder ([`crate::pathfinder`])
//! over the network model ([`crate::traffic_network`]), adds the traffic to
//! the volume grids, edge densities, switches and destinations, and publishes
//! the congestion, air-polluting, commercial and trip-length maps. It also
//! keeps the monthly incomes, the damage and pothole sets, the strike, the
//! accident check, the history and the save record.
//!
//! # Structure
//!
//! - [`TrafficSimulator`] (this file): the object, its 44 interface slots
//!   (§7) and the messages (§3.1).
//! - `state.rs`: the data model of §2 ([`State`]).
//! - `tuning.rs`: `ReadSimulatorTunableValues` (§4.14) and the travel
//!   strategies (§4.1.2).
//! - `cycle.rs`: Init, Shutdown, `DoMessage`, `DoDailySimulation`,
//!   `WorkOnSimCycle` and the small phases (§1, §3, §4.8, §4.1.3).
//! - `trips.rs`: trip generation and the three trip phases (§4.1, §4.3, §4.4,
//!   §4.6).
//! - `route.rs`: the compressed route reader, `ApplyRoute`, `ConfirmRoute`
//!   and the destination capacities (§4.2, §4.5).
//! - `maps.rs`: the map phases (§4.7, §4.9–§4.11).
//! - `monthly.rs`: `FinalCalculations`, `StartRecalc`, `DoMonthlySimulation`,
//!   the accidents (§4.12, §4.13).
//! - `record.rs`: the save record (save-records.md §7.1.1) and the trip
//!   record (§6.3).
//! - `adapters.rs`: the host the network model sees and the host the
//!   pathfinder sees, both built on [`TrafficSimHost`] and the simulator's
//!   own state.
//!
//! # Collaborators
//!
//! The network model and the pathfinder are used directly: the simulator owns
//! a [`NetworkModel`] (whose host is an adapter that answers the three
//! simulator questions itself: trip capacity, the pothole table and the
//! neighbour pools) and two [`Pathfinder`]s. Everything else in the game
//! (lots, neighbour connections, departments, history, dates, the view,
//! accidents, other simulators) comes through [`TrafficSimHost`]. Game
//! objects are opaque `u64` tokens; 0 is null.
//!
//! # Re-entrancy (foundations §19)
//!
//! The state lives in a `RefCell<Option<Box<State>>>`. Every operation takes
//! it out for its duration (and the network state out of the model), so no
//! borrow is held across a host callback; a re-entrant call finds no state
//! and does nothing (it returns the type's default).
//!
//! # Arithmetic and draws
//!
//! Foundations §18: every float operation is one `f32` rounding
//! ([`crate::x87::r24`]); `round` is [`crate::x87::rhe`] and `ftol` is
//! [`crate::x87::ftol`]. The draws are those of §4.15, on one
//! [`crate::rng::RzRandomSuper`] seeded with `derive_seed(session,
//! 0x6990C1AA)` and one [`crate::rng::CrtRand`] (seed 1) for the accident
//! iterator.
//!
//! # Choices and open points
//!
//! Every place where the spec is silent is marked in the code with
//! *[choice]* or *[question]* and listed in the lane report.

mod adapters;
pub mod archive;
mod cycle;
mod maps;
mod monthly;
mod pools;
mod record;
mod route;
pub mod shadow;
mod state;
mod trips;
pub mod tuning;

#[cfg(test)]
mod tests;

pub use adapters::{
    capacity_element, capacity_values, set_capacity_element, HolderStore, Holders, NetAdapter, CAPACITY_LEN,
};
pub use maps::{air_polluting_byte, background_value, congestion_byte};
pub use monthly::{damage_target, kind as damage_kind, STRIKE_KEEP, STRIKE_MAX_MONTHS, STRIKE_SCALE, STRIKE_START};
pub use pools::{
    is_pool_token, map_pool_token, pool_of_token, pool_token, Pools, POOL_CITY_X, POOL_CITY_Z, POOL_KEY0_TOKEN,
    POOL_TOKEN_BASE,
};
pub use record::{legacy_density, read_mask_and_flags, read_trip, write_trip, TRIP_RECORD_VERSION};
pub use route::{decode_route, read_route, RouteCell, RouteStep};
pub use state::{
    AccidentEntry, DamageBlock, DamageState, SaveExtras, SourceIter, StatRow, State, StatsSet, SwitchStat,
    PC_REROUTE_BUCKET, PC_SOURCE_COUNTER,
};
pub use trips::{
    ceil_idiom, find_highway_return_tile, floor_idiom, industrial_trips, merge_routed, merge_search,
    reconcile_connection, reconcile_lot, residential_commuters, residential_split, trip_length_byte, COMMUTER_START,
    COMMUTER_TYPES, DIVIDED_ROAD_MASK, FREIGHT_TYPES,
};
pub use tuning::{invert_congestion_curve, SimTuning};

use std::cell::RefCell;
use std::rc::Rc;

use crate::grid::{CellRect, GridProvider, TractGrid};
use crate::pathfinder::Pathfinder;
use crate::property::PropertyHolder;
use crate::stream::RefValue;
use crate::traffic_network::{FerryRouteInfo, NetworkHost, NetworkModel, TrafficEdgeDensity, EDGE_DENSITY_ZERO};
use crate::variant::Variant;

pub use crate::traffic_network::{CLSID_TRAFFIC_SIMULATOR, IID_TRAFFIC_SIMULATOR};

/// `cIGZUnknown`.
pub const IID_UNKNOWN: u32 = 1;
/// `cIGZMessageTarget2` (QueryInterface answers it, §1).
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
/// `cIGZSerializable`.
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// `GetSimulatorType` (slot 5).
pub const SIMULATOR_TYPE: u32 = 0x8802_7F0D;
/// The generator constant `K` of foundations §6.3 for this simulator
/// [choice]: its class ID.
pub const RNG_K: u32 = 0x6990_C1AA;
/// The agent name (§1 Init step 2).
pub const AGENT_NAME: &str = "cSC4TrafficSimulator";
/// The agent levels (§1 Init step 2): one-shot, month, day; flags 2.
pub const AGENT_LEVELS: [u32; 3] = [0, 6, 4];
pub const AGENT_FLAGS: u32 = 2;

/// The record version written (§6.1, save-records.md §7.1.1).
pub const RECORD_VERSION: u16 = 0x29;
/// The oldest record version Read accepts.
pub const MIN_RECORD_VERSION: u16 = 0x13;

/// Messages handled (§3.1) and posted (§5).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const DAY: u32 = 0x6695_6814;
    pub const MONTH: u32 = 0x6695_6816;
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const BUDGET_CHANGED: u32 = 0x08DA_C3E7;
    pub const NEIGHBOUR_ADDED: u32 = 0x8A0D_15DA;
    pub const NEIGHBOUR_REMOVED: u32 = 0xEA0D_163E;
    pub const TUNING_RELOAD: u32 = 0xC53D_10AA;
    /// The tuning-reload message's data1 and data2 (§3.1).
    pub const TUNING_GROUP: u32 = 0xE7E2_C2DB;
    pub const TUNING_INSTANCE: u32 = 0xC913_3286;
    pub const ORDINANCE_UPDATED: u32 = 0xC2C0_9554;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;
    /// The messages Init subscribes to, in order (§1 Init step 3).
    pub const SUBSCRIPTIONS: [u32; 9] = [
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        POST_CITY_INIT,
        LOT_STATE_CHANGED,
        TUNING_RELOAD,
        NEIGHBOUR_ADDED,
        NEIGHBOUR_REMOVED,
        BUDGET_CHANGED,
        ORDINANCE_UPDATED,
    ];
    /// Posted at the end of CreateFinalMap (§4.10, §5).
    pub const TRAFFIC_MAP_CHANGED: u32 = 0x6924_7DC7;
    /// Posted by the accident check (§4.13, §5).
    pub const TRAFFIC_ACCIDENT: u32 = 0x0C05_E4C3;
    /// Strike transitions (§4.12 step 2).
    pub const STRIKE_OVER_A: u32 = 0x6924_7DC3;
    pub const STRIKE_ON: u32 = 0x6924_7DC4;
    pub const STRIKE_OVER_B: u32 = 0x6924_7DC5;
    pub const STRIKE_STARTS: u32 = 0x6924_7DC6;
    /// A cell damaged or repaired (§4.12, ApplyPotholeChanges).
    pub const CELL_DAMAGE: u32 = 0x6924_7DC8;
}

/// The grids of §1 Init step 4 (instance IDs).
pub mod grid_id {
    pub const AIR_POLLUTING: u32 = 0x89D5_C3A2;
    pub const PEAK_VOLUME: u32 = 0x69D5_C3AC;
    pub const COMMERCIAL: u32 = 0xCABD_2925;
    pub const COMMERCIAL_WORK: u32 = 0xCABD_2926;
    pub const UNUSED: u32 = 0x69D5_C3B5;
    pub const TRIP_LENGTH: u32 = 0x69D5_C3BF;
    /// Volume grid of transit network `n` is `VOLUME + n`, n = 0…5.
    pub const VOLUME: u32 = 0x69D5_C3E0;
    pub const FLOAT: u32 = 0x69D5_C3FA;
    pub const CONGESTION: u32 = 0x69D5_C402;
    pub const PEAK_RATIO: u32 = 0x69D5_C40E;
}

/// The departments (§1 Init step 5).
pub mod dept {
    pub const ROADS: u32 = 0xC921_CEEA;
    pub const MASS_TRANSIT: u32 = 0xC921_CEEB;
    /// The line items the day-1 income goes to (§3.3).
    pub const TRANSIT_INCOME_ITEM: u32 = 2;
    pub const TOLL_INCOME_ITEM: u32 = 5;
    /// The budget group `CreateDepartmentBudget` is given (§9.1 TG7).
    pub const GROUP: u32 = 0xAA36_9059;
    /// The LTEXT group of the department and line-item names (§9.1 TG7).
    pub const NAME_GROUP: u32 = 0x6A3F_F01C;
    /// The departments' names (`SetDepartmentName(NAME_GROUP, name)`).
    pub const ROADS_NAME: u32 = 0x4A49_3B73;
    pub const MASS_TRANSIT_NAME: u32 = 0xEA49_3B83;

    /// A line item of a traffic department: `CreateLineItem(line, false)`,
    /// `SetName(NAME_GROUP, name)`, and `SetType(Income)` when `income`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct LineItem {
        pub line: u32,
        pub name: u32,
        pub income: bool,
    }

    const fn item(line: u32, name: u32, income: bool) -> LineItem {
        LineItem { line, name, income }
    }

    /// Created with the roads department, when the member is still null
    /// after `Read` (§9.1 TG7).
    pub const ROADS_LINES: [LineItem; 3] = [
        item(1, 0xEA84_46BE, false),
        item(2, 0x2A84_46DA, false),
        item(3, 0x2A84_46DF, false),
    ];
    /// Created with the mass-transit department; line 2 is Income.
    pub const MASS_TRANSIT_LINES: [LineItem; 3] = [
        item(1, 0xEA5F_96E7, false),
        item(2, 0x8A5F_978C, true),
        item(3, 0x6A8B_0CFB, false),
    ];
    /// On every `Init`, loaded departments included, each created only when
    /// `GetLineItem(line)` is null, in this order (`true` = mass transit;
    /// §9.1 TG7). Roads line 5 (tolls) is Income.
    pub const INIT_LINES: [(bool, LineItem); 6] = [
        (true, item(4, 0x2AB7_EA93, false)),
        (true, item(5, 0x8BCE_33E0, false)),
        (false, item(4, 0xCBCE_33D5, false)),
        (true, item(6, 0x8BCE_33E1, false)),
        (false, item(5, 0x0C3B_F3C4, true)),
        (false, item(6, 0xEA5F_96E7, false)),
    ];
}

/// Interface IDs used when reading references (save-records.md §7.1.1).
pub mod ref_iid {
    pub const GRID: u32 = 0xC9BD_939A;
    pub const BUILDING: u32 = 0x26BB_767E;
    pub const DEPARTMENT: u32 = 0x223E_2B67;
}

/// Properties the simulator reads or writes on lots, buildings, neighbour
/// connections and pools.
pub mod prop {
    /// The strategy re-draw value (u8), never read (§4.1.1).
    pub const STRATEGY_DRAW: u32 = 0x4A61_1773;
    /// The strategy bucket of a lot (§4.1.3 step 1).
    pub const STRATEGY_BUCKET: u32 = 0x691C_863C;
    /// The trip capacity array (12 × u16, promoted to u32, §4.2).
    pub const TRIP_CAPACITY: u32 = 0x8A0B_4100;
    /// The neighbour trip targets per trip type `w`: `0xAA9EA81D + w`
    /// (§4.1.4).
    pub const NEIGHBOUR_TARGETS: u32 = 0xAA9E_A81D;
    /// The neighbour pool arrivals per travel type: `0xAA9EA81A + type`
    /// (§4.2).
    pub const POOL_ARRIVALS: u32 = 0xAA9E_A81A;
    /// The average trip length of a neighbour connection (§4.6).
    pub const CONNECTION_TRIP_LENGTH: u32 = 0xCA94_7077;
    /// The ordinance travel-strategy offsets (s32[9], §4.1.2).
    pub const ORDINANCE_STRATEGY: u32 = 0x8A61_2FEE;
    /// The budget-item properties of an activated building (§4.1.3 step 3).
    pub const BUDGET_ITEM: u32 = 0xCA55_0301;
    pub const BUDGET_ITEM_ALT: u32 = 0x0C3B_F549;
}

/// History series (§4.12 step 4).
pub mod history {
    pub const CONGESTION: u32 = 0xEA5B_A01D;
    pub const COMMUTE: u32 = 0x8A5D_4116;
    /// `TRAVEL_TYPES + i` for travel type i = 0…8.
    pub const TRAVEL_TYPES: u32 = 0xABB8_FAAC;
    pub const FERRY: u32 = 0xEC0B_DBF8;
}

/// The Carpool ordinance (§4.5).
pub const CARPOOL_ORDINANCE: u32 = crate::pathfinder::CARPOOL_ORDINANCE;

/// Work-cycle states (§3.2).
pub mod cycle_state {
    pub const START_RECALC: u8 = 0;
    pub const LOT_SWEEP: u8 = 1;
    pub const CREATE_MAPS: u8 = 2;
    pub const REDUCE_SWITCHES: u8 = 3;
    pub const CREATE_FINAL_MAP: u8 = 4;
    pub const CLEAR_DESTINATIONS: u8 = 5;
    pub const REDUCE_EDGES: u8 = 6;
    pub const DUMP_EDGES: u8 = 7;
    pub const FINAL_CALCULATIONS: u8 = 8;
    pub const REFRESH_COMMUTE: u8 = 10;
    pub const UPDATE_NEIGHBOURS: u8 = 11;
    pub const UPDATE_SUBNETWORKS: u8 = 12;
    pub const SUBNETWORK_INFO: u8 = 13;
    pub const FINISH_TRIP_MAP: u8 = 15;
    pub const CONFIRM_TRIPS: u8 = 16;
    pub const REROUTE_TRIPS: u8 = 17;
    pub const FULL_TRIPS: u8 = 18;
    pub const FERRY_ROUTES: u8 = 19;
    /// The next-state table (`0x00AB3480`).
    pub const NEXT: [u8; 20] = [16, 14, 4, 7, 5, 11, 10, 6, 0, 0, 12, 3, 13, 19, 15, 2, 17, 18, 15, 8];
}

/// Trip status values (§2).
pub mod status {
    pub const SEARCH: u8 = 0;
    pub const REROUTE: u8 = 1;
    pub const ROUTED: u8 = 3;
    pub const NEW: u8 = 4;
    pub const FAILED: u8 = 5;
}

/// Trip flag bits (§2).
pub mod trip_flag {
    pub const REVERSIBLE: u8 = 1;
    pub const PROBE: u8 = 2;
    pub const FORCED_START: u8 = 4;
    pub const ROUTED_LAST_PASS: u8 = 8;
}

/// The freight trip type (§2).
pub const FREIGHT: u8 = 3;

/// `cSC4TripData` (§2): one batch of trips in a traffic source's list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TripData {
    /// The compressed outbound route (pathfinder.md §4.15).
    pub route: Vec<u8>,
    /// The compressed return route (empty for reversible and freight trips).
    pub return_route: Vec<u8>,
    pub count: u32,
    /// 0, 1, 2 = commuters of wealth $, $$, $$$; 3 = freight.
    pub trip_type: u8,
    /// Lots: 0 mass transit, 1 car, 2 neutral. Neighbour connections: a
    /// travel type 0…8 (§4.1.4).
    pub strategy: u8,
    /// [`trip_flag`] bits.
    pub flags: u8,
    /// [`status`] value.
    pub status: u8,
    pub dest_x: i16,
    pub dest_z: i16,
    pub trip_length: f32,
    /// `1 << t` of the travel types on the route(s).
    pub travel_types_used: u32,
}

impl TripData {
    /// A new trip as the reconcile steps append it (§4.1.1, §4.1.4): the
    /// fields the original leaves uninitialised are 0 *[choice]*.
    pub fn new(count: u32, trip_type: u8, strategy: u8, flags: u8, status: u8) -> TripData {
        TripData {
            count,
            trip_type,
            strategy,
            flags,
            status,
            ..TripData::default()
        }
    }

    pub fn is_probe(&self) -> bool {
        self.flags & trip_flag::PROBE != 0
    }

    pub fn is_freight(&self) -> bool {
        self.trip_type == FREIGHT
    }
}

/// A traffic source (§4.1): a lot or a neighbour connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Lot(u64),
    Connection(u64),
}

/// A message the simulator posts (§5). The glue fills void1 and the
/// `IGZUnknown` field with the simulator where §5 says so.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
}

impl SimMessage {
    pub fn of_type(msg_type: u32) -> SimMessage {
        SimMessage {
            msg_type,
            ..SimMessage::default()
        }
    }
}

/// A message `DoMessage` receives (§3.1); `object` is void1 / the
/// `IGZUnknown` (an occupant or a lot copy), 0 = none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub object: u64,
}

/// An established neighbouring city reached by a transport connection, as
/// the pool capacities need it (traffic-network.md §2.7.4 steps 2–4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NeighbourCity {
    /// `GetPosition()` (regional-city slot 5): the region cell of the
    /// city's corner.
    pub position: (i32, i32),
    /// `GetExtrapolatedPopulation(0x2010 + 0x10·w)` (regional-city slot 17),
    /// w = 0…2.
    pub population: [i32; 3],
}

/// The five fields of `GetTransitSwitchQueryData`'s `out` (§4.16), named
/// after the query window's readers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwitchQueryData {
    /// `+0x00`: the available capacity (service quality numerator).
    pub available: u32,
    /// `+0x04`: the maximum capacity.
    pub max_capacity: u32,
    /// `+0x08`: the load fraction.
    pub load_fraction: f32,
    /// `+0x0C`: the actual usage.
    pub usage: u32,
    /// `+0x10`: the monthly maintenance cost.
    pub cost: u32,
}

/// A `u8` grid.
pub type GridU8 = Rc<dyn TractGrid<u8>>;
/// An `f32` grid.
pub type GridF32 = Rc<dyn TractGrid<f32>>;

/// Everything the traffic simulator reads from, or does to, the rest of the
/// game (§8) that the network model and the pathfinder do not cover.
///
/// The [`NetworkHost`] supertrait is what the network model needs; the
/// simulator hands the model an adapter ([`NetAdapter`]) that forwards to it,
/// except for three questions the simulator answers itself
/// (`max_trip_capacity`, `is_road_damaged`, `neighbor_pool_holder`): a host
/// may implement those three as it likes, the simulator never calls them.
/// The neighbour pools are the simulator's ([`Pools`]); their holder tokens
/// are reserved ([`POOL_TOKEN_BASE`]).
///
/// Host callbacks must not call back into the simulator (a re-entrant call
/// finds no state and does nothing).
pub trait TrafficSimHost: NetworkHost + GridProvider<f32> {
    // ---- framework (§1, §5) ----
    /// Registers the message target as a simulator agent (simulator slot
    /// `+0x4C`, simulator.md §4).
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// Subscribes the message target to a message type.
    fn subscribe(&self, msg_type: u32);
    /// Posts a message, queued (`MessagePost(msg, false)`, §5).
    fn post_message(&self, msg: SimMessage);
    /// The reference to save for a host object (foundations §2.7).
    fn object_ref(&self, token: u64) -> RefValue;
    /// Resolves a reference read from a save, asking `iid`.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;

    // ---- city, tuning, dates ----
    /// `cISC4City` `+0x16C`: the cell width (§4.14 speed table).
    fn cell_width(&self) -> f32;
    /// The tuning exemplar `0x6534284A / 0xE7E2C2DB / 0xC9133286` (§4.14),
    /// `None` when the resource manager has none.
    fn tuning_exemplar(&self) -> Option<Rc<PropertyHolder>>;
    /// `GetSimDate` day of month (§3.3).
    fn sim_day_of_month(&self) -> u32;
    /// `GetSimDateNumber` (§3.3, §4.12).
    fn sim_date_number(&self) -> u32;
    /// The simulation clock in milliseconds of game time (§4.13
    /// *[choice]*: the accident timer).
    fn sim_clock_ms(&self) -> u64;

    // ---- departments (budget.md §7–§8) ----
    /// `CreateDepartmentBudget(id, dept::GROUP)` (an existing department is
    /// returned unchanged), one reference for the core,
    /// `SetDepartmentName(dept::NAME_GROUP, name of id)` and
    /// `SetMaxAllowedFundingPercentage(max_funding)` (§1 Init step 5, §9.1
    /// TG7); `max_funding` is the tuning value (1.2 = 120 %). The line items
    /// come through `create_line_item`.
    fn create_department(&self, id: u32, max_funding: f32) -> Option<u64>;
    /// `CreateLineItem(item.line, false)` (replacing any line of that ID),
    /// `SetName(dept::NAME_GROUP, item.name)` and, for an income line,
    /// `SetType(Income)` (§9.1 TG7). With `only_if_missing`, nothing when
    /// `GetLineItem(item.line)` is not null.
    fn create_line_item(&self, department: u64, item: dept::LineItem, only_if_missing: bool);
    /// Releases a department (Shutdown).
    fn release_department(&self, department: u64);
    /// The department's funding fraction (department slot 10).
    fn department_funding(&self, department: u64) -> f32;
    /// Line item `item`'s `SetIncome(int64)` (§3.3).
    fn set_line_item_income(&self, department: u64, item: u32, income: i64);
    /// `UpdateRoadsBudget` / `UpdateMassTransitBudget` (§4.9; Q1, not
    /// traced): the department's monthly costs from the cell counts per
    /// network type and the per-tile costs `0x6A84493E`.
    fn update_network_costs(&self, department: u64, mass_transit: bool, cells: &[u32; 13], tile_costs: &[f32; 13]);
    /// §4.1.3 step 3: the budget items of an activated building (budget
    /// simulator slots 51, 58, 55; `lot` is the real lot when found). Returns
    /// the int64 to add to the total `E` (non-zero only on the `0xCA550301`
    /// path).
    fn activate_budget_items(&self, building: u64, lot: Option<u64>) -> i64;
    /// §4.1.3 DoLotDeactivated step 3 (TS23): with the lot's own holder,
    /// slot 51 for `0xCA550301` and, if found, slot 56; then, independently,
    /// the same for `0x0C3BF549`. Returns the `0xCA550301` item's cost (the
    /// int64 at `+0x10`) to subtract from `E`, 0 when there is none.
    fn deactivate_budget_items(&self, holder: u64) -> i64;
    /// Budget simulator slot 51 `GetBudgetItemForPurpose(holder, purpose,
    /// item)` on the building's property holder (§4.16, TS11): the
    /// building's first budget item with that purpose, as its department
    /// ID and its int64 monthly cost at 100 %; `None` when there is none.
    fn budget_item_for_purpose(&self, building: u64, purpose: u32) -> Option<(u32, i64)>;
    /// Budget simulator slot 28 `GetDepartmentBudget(id)` (§4.16): the
    /// department, `None` when there is none.
    fn department_budget(&self, department_id: u32) -> Option<u64>;

    // ---- history (§4.12) ----
    fn record_history(&self, series: u32, value: u32);

    // ---- lots (§4.1, §4.7) ----
    /// All lots in the lot manager's list order (slot 15).
    fn lots(&self) -> Vec<u64>;
    /// [`lots`](Self::lots), shared. A host that knows its list unchanged
    /// may hand out the same list again (FreeCity4's host does within one
    /// outermost call, where the plugin keeps one lot walk), which lets the
    /// source iterator's step cost O(1) instead of O(lots); the walk is
    /// the same either way. Default: a fresh copy each time.
    fn lots_shared(&self) -> Rc<[u64]> {
        self.lots().into()
    }
    /// `QueryInterface(0xE818FCED)` on a message object: the lot copy.
    fn as_lot(&self, object: u64) -> Option<u64>;
    /// Lot slot 31: the zone type.
    fn lot_zone(&self, lot: u64) -> u32;
    /// Lot slot 37: in a livable state.
    fn lot_is_livable(&self, lot: u64) -> bool;
    /// Lot slot 28: has road access.
    fn lot_has_road_access(&self, lot: u64) -> bool;
    /// Lot slot 59: the linked lot, `None` for 0.
    fn lot_link(&self, lot: u64) -> Option<u64>;
    /// Lot slot 56 `GetTravelPopulation(developerType)`.
    fn lot_travel_population(&self, lot: u64, developer_type: u32) -> u32;
    /// The lot's own strategy byte (§4.1.1).
    fn lot_strategy_byte(&self, lot: u64) -> u8;
    /// Lot slots 12/13: clears a lot flag (§4.7).
    fn clear_lot_flag(&self, lot: u64, flag: u32);

    /// `cISC4TrafficSource::UpdateTrafficVolume` (slot 4) run by the source
    /// itself (§4.1: the simulator calls the source's own method; a lot's is
    /// §4.1.1, a connection's §4.1.4). True when the host ran it; false (the
    /// default) lets the simulator compute it from [`TrafficSimHost::lot_zone`]
    /// and the other lot and connection facts below.
    fn native_update_traffic_volume(&self, source: Source) -> bool {
        let _ = source;
        false
    }

    // ---- trip lists (cISC4TrafficSource slot 3) ----
    /// A copy of the source's trip list.
    fn trip_list(&self, source: Source) -> Vec<TripData>;
    /// Replaces the source's trip list.
    fn set_trip_list(&self, source: Source, trips: Vec<TripData>);

    // ---- neighbour connections (§4.1.4, §4.6; traffic-network.md §2.7) ----
    /// The connection's own property holder (connection slot 3,
    /// traffic-network.md §2.7.5).
    fn connection_holder(&self, connection: u64) -> u64;
    /// Reads and clears the connection's "volume dirty" flag (§4.1.4).
    fn take_connection_volume_dirty(&self, connection: u64) -> bool;
    /// Neighbour simulator slot 11 `FindConnection(type, x, z)`.
    fn find_connection(&self, connection_type: u32, x: i32, z: i32) -> Option<u64>;
    /// The connection's neighbour-connection type (the type DoPostCityInit
    /// passes to Add, traffic-network.md §2.7.2).
    fn connection_type(&self, connection: u64) -> u32;
    /// Neighbour simulator slot 13 `FindConnectionOnOtherSide(conn)`
    /// (`0x006CDFA0`, pop 4; TS38: §4.1.4 and traffic-network.md §2.7.2 name
    /// it right): the neighbouring city's end of the connection, from the
    /// neighbour's regional record (slot 14), whose properties the caller
    /// reads (§2.7.2 step 3).
    fn connection_on_other_side(&self, connection: u64) -> Option<u64>;
    /// §2.7.2 step 5: the city on the other side (connection slot 9) when
    /// there is one and it is established (regional-city slot 43), as its
    /// `GetPosition()` (slot 5); `None` otherwise.
    fn established_city_position(&self, connection: u64) -> Option<(i32, i32)>;
    /// Neighbour simulator slot 21 `GetConnectedCities(list, false)`, in
    /// list order, with each city's position and extrapolated populations
    /// (§2.7.4 steps 2–4).
    fn connected_cities(&self) -> Vec<NeighbourCity>;
    /// The regional jobs of wealth `w` (§2.7.4 step 1): the demand
    /// simulator's `GetDemand(0x2010 + 0x10·w, 0x10000)` and its
    /// `QueryDemandValue()`; `None` when the node is missing.
    fn regional_demand(&self, w: u32) -> Option<f32>;

    // ---- other simulators ----
    /// Residential simulator slot 30 `GetCellWorkforcePercent(x, z)`.
    fn workforce_percent(&self, x: i32, z: i32) -> f32;
    /// The BDS developer's population grid value (developer slot 15, s16).
    fn developer_population(&self, developer_type: u32, x: i32, z: i32) -> i16;
    /// Ordinance simulator slot 8.
    fn is_ordinance_on(&self, id: u32) -> bool;
    /// The IDs of the ordinances that are on (slots 14/15).
    fn ordinances_on(&self) -> Vec<u32>;
    /// An ordinance's misc property (ordinance slot 16).
    fn ordinance_property(&self, id: u32, prop: u32) -> Option<Variant>;

    // ---- occupants ----
    /// `QueryInterface(0xA821EF94)` on a message object (§3.1).
    fn as_network_occupant(&self, object: u64) -> Option<u64>;
    /// `cISC4NetworkOccupant` slot 59: the network occupant's own cell
    /// (DoRailChanged, §3.6 step 3).
    fn network_occupant_cell(&self, occ: u64) -> (i32, i32);
    /// Occupant manager slot 40 over a cell rectangle (pathfinder filter 2).
    fn occupants_in_cells(&self, rect: CellRect) -> Vec<u64>;
    /// `cISC4OccupantFilter::IsOccupantIncluded`.
    fn occupant_filter_includes(&self, filter: u64, occupant: u64) -> bool;
    /// The accident check's occupant test (§4.13, grid slot 14 of `+0x1C`).
    fn cell_has_network_occupant(&self, x: i32, z: i32) -> bool;

    // ---- language (§4.4) ----
    /// The language manager's utility for the current language (slot 38
    /// `GetLanguageUtility(0)`, then its slot 25 `GetLanguageRoadDrivingSide`)
    /// = 1: false for UK English (2) and Japanese (15), true for every other
    /// language and when there is no utility (TS17).
    fn drives_on_right(&self) -> bool;

    // ---- accidents (§4.13) ----
    /// The visible cell rectangle from the view, `None` without a view.
    fn view_cell_rect(&self) -> Option<CellRect>;
    /// `cSC4TrafficAutomataManager::CreateAccident` (result not checked).
    fn create_accident(&self, x: i32, z: i32, duration_ms: i64);
}

/// The traffic simulator (§1).
pub struct TrafficSimulator {
    pub(crate) host: Rc<dyn TrafficSimHost>,
    /// The network model (traffic-network.md), whose host is a
    /// [`NetAdapter`].
    pub(crate) net: Rc<NetworkModel>,
    /// The damage sets, shared with the network adapter (`IsRoadDamaged`).
    pub(crate) damage: Rc<RefCell<DamageState>>,
    /// The neighbour pools, shared with the network adapter
    /// (traffic-network.md §2.7.3).
    pub(crate) pools: Rc<RefCell<Pools>>,
    pub(crate) st: RefCell<Option<Box<State>>>,
}

impl TrafficSimulator {
    /// The constructor (§1): cycle state 0, last day −1, strike state 2,
    /// transit multiplier 1.0, the tuning code defaults; the generator is
    /// seeded with `derive_seed(session_seed, 0x6990C1AA)` (§4.15
    /// *[choice]*).
    pub fn new(host: Rc<dyn TrafficSimHost>, session_seed: u32) -> Rc<TrafficSimulator> {
        let damage = Rc::new(RefCell::new(DamageState::default()));
        let pools = Rc::new(RefCell::new(Pools::default()));
        let adapter: Rc<dyn NetworkHost> = Rc::new(NetAdapter {
            host: host.clone(),
            damage: damage.clone(),
            pools: pools.clone(),
        });
        let net = NetworkModel::new(adapter);
        let seed = crate::seeding::seed(session_seed, crate::seeding::keys::TRAFFIC_SIMULATOR);
        Rc::new(TrafficSimulator {
            host,
            net,
            damage,
            pools,
            st: RefCell::new(Some(Box::new(State::new(seed)))),
        })
    }

    /// The host.
    pub fn host(&self) -> &Rc<dyn TrafficSimHost> {
        &self.host
    }

    /// The network model.
    pub fn network(&self) -> &Rc<NetworkModel> {
        &self.net
    }

    /// The damage sets.
    pub fn damage(&self) -> std::cell::Ref<'_, DamageState> {
        self.damage.borrow()
    }

    /// Changes the damage sets directly (hosts restoring state, tests).
    pub fn update_damage<R>(&self, f: impl FnOnce(&mut DamageState) -> R) -> R {
        f(&mut self.damage.borrow_mut())
    }

    /// The neighbour pools.
    pub fn pools(&self) -> std::cell::Ref<'_, Pools> {
        self.pools.borrow()
    }

    /// Changes the pools directly (hosts restoring state, tests).
    pub fn update_pools<R>(&self, f: impl FnOnce(&mut Pools) -> R) -> R {
        f(&mut self.pools.borrow_mut())
    }

    /// The simulator's view of the property holders: pool tokens are its
    /// pools, any other token the host's.
    pub fn holders(&self) -> Holders<'_> {
        Holders {
            h: &*self.host,
            pools: &self.pools,
        }
    }

    /// Runs `f` with the state taken out (see the module documentation);
    /// `R::default()` when the state is already out (re-entrant call).
    pub(crate) fn with_state<R: Default>(&self, f: impl FnOnce(&dyn TrafficSimHost, &mut State) -> R) -> R {
        let Some(mut st) = self.st.borrow_mut().take() else {
            return R::default();
        };
        let r = f(&*self.host, &mut st);
        *self.st.borrow_mut() = Some(st);
        r
    }

    /// Runs `f` with the simulator state and the network state taken out:
    /// the context of every operation that touches both.
    pub(crate) fn run<R: Default>(&self, f: impl FnOnce(&mut cycle::Run<'_, '_>) -> R) -> R {
        let Some(mut st) = self.st.borrow_mut().take() else {
            return R::default();
        };
        let r = self.net.run(|cx| {
            let mut run = cycle::Run {
                h: &*self.host,
                st: &mut st,
                cx,
                dmg: &self.damage,
                pools: &self.pools,
            };
            f(&mut run)
        });
        *self.st.borrow_mut() = Some(st);
        r
    }

    /// A copy of the state (inspection, tests). `None` during an operation.
    pub fn snapshot(&self) -> Option<State> {
        self.st.borrow().as_ref().map(|s| (**s).clone())
    }

    /// Changes the state directly (hosts restoring state, tests).
    pub fn update_state<R: Default>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        match self.st.borrow_mut().as_mut() {
            Some(s) => f(s),
            None => R::default(),
        }
    }

    /// Reads a value of the state.
    pub fn read_state<R: Default>(&self, f: impl FnOnce(&State) -> R) -> R {
        match self.st.borrow().as_ref() {
            Some(s) => f(s),
            None => R::default(),
        }
    }

    // ---- slots 0–2, 5 (§1) ----

    /// `QueryInterface` (slot 0): 1, `0x68027F37`, `0x452294AA`, `0xE4FDA3D4`
    /// (the three faces of the object), with an `AddRef`.
    pub fn query_interface(&self, iid: u32) -> bool {
        let ok = matches!(
            iid,
            IID_UNKNOWN | IID_TRAFFIC_SIMULATOR | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE
        );
        if ok {
            self.add_ref();
        }
        ok
    }

    /// `AddRef` (slot 1).
    pub fn add_ref(&self) -> u32 {
        self.update_state(|s| {
            s.ref_count = s.ref_count.wrapping_add(1);
            s.ref_count
        })
    }

    /// `Release` (slot 2): above 1 it decrements and returns the new count;
    /// at 0 or 1 it calls `Shutdown` and returns 0 (the owner then drops the
    /// object).
    pub fn release(&self) -> u32 {
        let count = self.read_state(|s| s.ref_count);
        if count > 1 {
            return self.update_state(|s| {
                s.ref_count -= 1;
                s.ref_count
            });
        }
        self.shutdown();
        0
    }

    /// `GetSimulatorType` (slot 5).
    pub fn simulator_type(&self) -> u32 {
        SIMULATOR_TYPE
    }

    // ---- slots 6, 7: pathfinders ----

    /// `CreatePathFinder` (slot 6): a new initialised pathfinder sharing the
    /// work-check statics (pathfinder.md §1). The caller runs it with
    /// [`TrafficSimulator::find_path`].
    pub fn create_path_finder(&self) -> Pathfinder {
        self.run(|r| Some(r.create_path_finder()))
            .unwrap_or_else(|| Pathfinder::new(crate::pathfinder::shared_work_check()))
    }

    /// Runs an outside caller's pathfinder over the simulator's network and
    /// state (the context the original reaches through the pathfinder's
    /// owner pointer).
    pub fn find_path(&self, pf: &mut Pathfinder) -> u32 {
        self.run(|r| r.find_path_with(pf))
    }

    /// `Init` (pathfinder slot 3) of an outside caller's pathfinder, again
    /// after its `Shutdown` (pathfinder.md §1).
    pub fn init_path_finder(&self, pf: &mut Pathfinder) -> bool {
        self.run(|r| r.init_path_finder(pf))
    }

    /// `ApplyTraffic(style)` (pathfinder slot 29, pathfinder.md §4.17) of an
    /// outside caller's pathfinder.
    pub fn apply_path_finder_traffic(&self, pf: &mut Pathfinder, style: u32) {
        self.run(|r| r.apply_path_finder_traffic(pf, style))
    }

    /// `GetTripLengthScaledForDisplay` (pathfinder slot 26, pathfinder.md
    /// §4.14): `r24(GetTripScaleForDisplay() × tripLength)`.
    pub fn path_finder_display_length(&self, pf: &Pathfinder) -> f32 {
        let scale = self.trip_scale_for_display();
        crate::x87::r24(f64::from(scale) * f64::from(pf.get_trip_length())) as f32
    }

    /// `GetCompressedRoute(out, mask)` (pathfinder slot 32, pathfinder.md
    /// §4.15) of a pathfinder over the simulator's network; returns the mask.
    pub fn path_finder_compressed_route(&self, pf: &Pathfinder, out: &mut Vec<u8>) -> u32 {
        pf.get_compressed_route(&self.net.state(), out)
    }

    /// `WasTripReversible` (pathfinder slot 30, pathfinder.md §4.16).
    pub fn path_finder_was_reversible(&self, pf: &Pathfinder) -> bool {
        pf.was_trip_reversible(&self.net.state())
    }

    /// `SetupPathFinderForLot(pf, lot)` (slot 7, `0x00711610`, §4.4, TS10):
    /// configures a caller's pathfinder from the lot's trip list. False for
    /// lot 0 (the null pointer) or a lot with no trips.
    pub fn setup_path_finder_for_lot(&self, pf: &mut Pathfinder, lot: u64) -> bool {
        self.run(|r| r.setup_path_finder_for_lot(pf, lot))
    }

    // ---- slots 8–10, 13: maps ----

    /// `GetAirPollutingTrafficMap` (slot 8).
    pub fn air_polluting_traffic_map(&self) -> Option<GridU8> {
        self.read_state(|s| s.air.clone())
    }

    /// `GetCommercialTrafficMap` (slot 9), tract size 4.
    pub fn commercial_traffic_map(&self) -> Option<GridU8> {
        self.read_state(|s| s.commercial.clone())
    }

    /// `GetCongestionMap` (slot 10).
    pub fn congestion_map(&self) -> Option<GridU8> {
        self.read_state(|s| s.congestion.clone())
    }

    /// `GetTripLengthMap` (slot 13).
    pub fn trip_length_map(&self) -> Option<GridU8> {
        self.read_state(|s| s.trip_length.clone())
    }

    /// The volume grid of transit network `n` (§4.5).
    pub fn volume_grid(&self, n: usize) -> Option<GridF32> {
        self.read_state(|s| s.volume.get(n).cloned().flatten())
    }

    /// The peak-volume grid (§4.9).
    pub fn peak_volume_grid(&self) -> Option<GridF32> {
        self.read_state(|s| s.peak_volume.clone())
    }

    /// The peak volume/capacity ratio grid (§4.9).
    pub fn peak_ratio_grid(&self) -> Option<GridF32> {
        self.read_state(|s| s.peak_ratio.clone())
    }

    /// The commercial work grid (§4.9).
    pub fn commercial_work_grid(&self) -> Option<GridU8> {
        self.read_state(|s| s.commercial_work.clone())
    }

    // ---- slots 11, 12, 14–18 ----

    /// `GetBackgroundTraffic(x, z)` (slot 11, §4.9).
    pub fn background_traffic(&self, x: i32, z: i32) -> f32 {
        self.with_state(|h, s| maps::background_traffic(h, &s.tuning, x, z))
    }

    /// `GetTrafficEdgeDensity(x, z, t, evening)` (slot 12, §4.8): the record,
    /// or the shared all-zero record. The original takes `u8` coordinates.
    pub fn traffic_edge_density(&self, x: u8, z: u8, travel_type: u32, evening: bool) -> TrafficEdgeDensity {
        let s = self.net.state();
        let key = crate::traffic_network::NetworkState::edge_key(i32::from(x), i32::from(z), travel_type, evening);
        s.edge_density.get(key).copied().unwrap_or(EDGE_DENSITY_ZERO)
    }

    /// `GetTripScale` (slot 14): `commuteMaxTime`.
    pub fn trip_scale(&self) -> f32 {
        self.read_state(|s| s.tuning.commute_max_time)
    }

    /// `GetTripScaleForDisplay` (slot 15).
    pub fn trip_scale_for_display(&self) -> f32 {
        self.read_state(|s| s.tuning.trip_display_multiplier)
    }

    /// `IsRoadDamaged(x, z)` (slot 16, §4.13).
    pub fn is_road_damaged(&self, x: i32, z: i32) -> bool {
        self.damage.borrow().is_road_damaged(x, z)
    }

    /// `CheckRailAccident(x, z)` (slot 17, §4.13): one draw for a damaged
    /// rail cell.
    pub fn check_rail_accident(&self, x: i32, z: i32) -> bool {
        let damaged = self.damage.borrow().rail.damaged.contains(u32::from(cell_key16(x, z)));
        if !damaged {
            return false;
        }
        self.with_state(|_, s| {
            let u = s.rng.uniform_f64();
            f64::from(s.tuning.rail_damage_accident_factor) > u
        })
    }

    /// `GetLastMonthlyIncome` (slot 18).
    pub fn last_monthly_income(&self) -> f32 {
        self.read_state(|s| s.last_monthly_income)
    }

    // ---- slots 19–21: capacities (§4.2) ----

    /// `SetMaxTripCapacity(holder, k, value)` (slot 19), through the
    /// element store (§4.2, TS18). A pool token reaches the pool.
    pub fn set_max_trip_capacity(&self, holder: u64, k: u32, value: u32) {
        adapters::set_capacity_element(&self.holders(), holder, k as usize, value);
    }

    /// `GetMaxTripCapacity(holder, k)` (slot 20).
    pub fn max_trip_capacity(&self, holder: u64, k: u32) -> u32 {
        adapters::capacity_element(&self.holders(), holder, k as usize)
    }

    /// `GetTrafficArrived(holder, k)` (slot 21): arrived **last** cycle.
    pub fn traffic_arrived(&self, holder: u64, k: u32) -> u32 {
        adapters::capacity_element(&self.holders(), holder, 8 + k as usize)
    }

    /// The trips arrived this cycle (`0x0070DFF0`).
    pub fn traffic_arrived_this_cycle(&self, holder: u64, k: u32) -> u32 {
        adapters::capacity_element(&self.holders(), holder, 4 + k as usize)
    }

    // ---- slots 22–27 ----

    /// `GetDesiredLotInsertionPoint` (slot 22, traffic-network.md §4.14):
    /// whether the safe lot iterator would be returned, i.e. the cycle is in
    /// state 1 (never entered after construction).
    pub fn desired_lot_insertion_point(&self) -> bool {
        self.read_state(|s| s.cycle_state == cycle_state::LOT_SWEEP)
    }

    /// `GetCapacity(n, x, z)` (slot 23).
    pub fn capacity(&self, n: u32, x: i32, z: i32) -> f32 {
        self.net.get_capacity(n, x, z)
    }

    /// `GetTravelTimeRatio(x, z, t)` (slot 24, §4.8).
    pub fn travel_time_ratio(&self, x: i32, z: i32, travel_type: u32) -> f32 {
        let affected = self.read_state(|s| {
            s.tuning
                .affected_by_traffic
                .get(travel_type as usize)
                .copied()
                .unwrap_or(false)
        });
        let generates = self.read_state(|s| s.tuning.generates_traffic);
        maps::travel_time_ratio(&self.net.state(), affected, &generates, x, z, travel_type)
    }

    /// `GetConnectionCount(n, x, z)` (slot 25).
    pub fn connection_count(&self, n: u32, x: i32, z: i32) -> u32 {
        self.net.get_connection_count(n, x, z)
    }

    /// `GetTravelStrategyPercentages(w)` (slot 26, §4.1.2).
    pub fn travel_strategy_percentages(&self, wealth: u32) -> [f32; 3] {
        self.read_state(|s| s.tuning.strategy_percentages(wealth))
    }

    /// `GetFreightScalingFactor` (slot 27).
    pub fn freight_scaling_factor(&self) -> f32 {
        self.read_state(|s| s.tuning.freight_scaling)
    }

    // ---- slots 28–33, 35–42: the network queries ----

    /// `GetTransitSwitches` (slot 28): the switch buildings in table order.
    pub fn transit_switches(&self) -> Vec<u64> {
        self.net.get_transit_switches()
    }

    /// `GetFerryRouteBetweenTiles` (slot 29).
    pub fn ferry_route_between_tiles(&self, x1: i32, z1: i32, x2: i32, z2: i32) -> Option<Vec<u8>> {
        self.net.ferry_route_between_tiles(x1, z1, x2, z2)
    }

    /// `GetAllFerryRoutes` (slot 30).
    pub fn all_ferry_routes(&self) -> Vec<Vec<u8>> {
        self.net.all_ferry_routes()
    }

    /// `GetFerryRoutesInUse` (slot 31).
    pub fn ferry_routes_in_use(&self) -> Vec<FerryRouteInfo> {
        self.net.ferry_routes_in_use()
    }

    /// `GetFerryTerminalCount(type)` (slot 32).
    pub fn ferry_terminal_count(&self, ferry_type: u32) -> u32 {
        self.net.ferry_terminal_count(ferry_type)
    }

    /// `GetWaterRoute` (slot 33).
    pub fn water_route(&self, x1: i32, z1: i32, x2: i32, z2: i32, route: &mut Vec<u8>) -> bool {
        self.net.get_water_route(x1, z1, x2, z2, route)
    }

    /// `GetTrafficStats` (slot 34): the published ("last") statistics block.
    pub fn traffic_stats(&self) -> StatsSet {
        self.read_state(|s| s.stats_last.clone())
    }

    /// `GetTransitSwitchQueryData(x, z, out)` (slot 35, `0x0071CB80`, §4.16,
    /// TS11): `out`, or `None` for false (`out` untouched).
    pub fn transit_switch_query_data(&self, x: i32, z: i32) -> Option<SwitchQueryData> {
        self.run(|r| r.transit_switch_query_data(x, z))
    }

    /// `AreLotsConnected` (slot 36).
    pub fn are_lots_connected(&self, a: u64, b: u64) -> bool {
        self.net.are_lots_connected(a, b)
    }

    /// `GetConnectedOccupantCount` (slot 37).
    pub fn connected_occupant_count(&self, lot: u64, developer_type: u32) -> u32 {
        self.net.get_connected_occupant_count(lot, developer_type)
    }

    /// `GetConnectedDestinationCount` (slot 38).
    pub fn connected_destination_count(&self, lot: u64, k: i32) -> u32 {
        self.net.get_connected_destination_count(lot, k)
    }

    /// `GetSubnetworksForLot` (slot 39).
    pub fn subnetworks_for_lot(&self, lot: u64) -> (Vec<u32>, bool) {
        self.net.get_subnetworks_for_lot(lot)
    }

    /// `GetSubnetworksInRectangle` (slot 40).
    pub fn subnetworks_in_rectangle(&self, rect: CellRect) -> Vec<u32> {
        self.net.get_subnetworks_in_rectangle(rect)
    }

    /// `GetSubnetworksInRegion` (slot 41).
    pub fn subnetworks_in_region(&self, region: &crate::zones::CellRegion) -> Vec<u32> {
        self.net.get_subnetworks_in_region(region)
    }

    /// `GetOccupantCountForAllSubnetworks` (slot 42).
    pub fn occupant_count_for_all_subnetworks(&self, developer_type: u32) -> Vec<u32> {
        self.net.get_occupant_count_for_all_subnetworks(developer_type)
    }

    // ---- inspection helpers ----

    /// The current work-cycle state (§3.2).
    pub fn cycle_state(&self) -> u8 {
        self.read_state(|s| s.cycle_state)
    }

    /// The destination byte at (x, z), x in −1…W, z in −1…H.
    pub fn destination_byte(&self, x: i32, z: i32) -> u8 {
        self.read_state(|s| s.dest_byte(x, z))
    }

    /// The free destination counts.
    pub fn free_destinations(&self) -> [u32; 4] {
        self.read_state(|s| std::array::from_fn(|k| s.free_dest[k].get()))
    }

    /// Sets `trafficUnitsPerSlice` (§3.4 *[choice]*; at least 1).
    pub fn set_traffic_units_per_slice(&self, n: u32) {
        self.update_state(|s| s.tuning.traffic_units_per_slice = n.max(1));
    }

    // ---- single steps, for hosts restoring state and for tests ----

    /// Sets the work-cycle state and runs its entry action (§3.2).
    pub fn enter_cycle_state(&self, s: u8) {
        self.run(|r| {
            r.st.cycle_state = s;
            r.enter_state(s);
        });
    }

    /// `UpdateTrafficVolume(source)` (§4.1).
    pub fn update_traffic_volume(&self, source: Source) {
        self.run(|r| r.update_traffic_volume(source));
    }

    /// `ConfirmTrips(source)` (§4.3), without `UpdateTrafficVolume`.
    pub fn confirm_source(&self, source: Source) {
        self.run(|r| r.confirm_source(source));
    }

    /// `CompleteTripSourceCalculations(source)` (§4.6) on the source's trip
    /// list.
    pub fn complete_trip_source_calculations(&self, source: Source) {
        self.run(|r| {
            let mut trips = r.h.trip_list(source);
            let rect = r.source_rect(source);
            r.complete_trip_source_calculations(source, rect, &mut trips);
            r.h.set_trip_list(source, trips);
        });
    }

    /// `SetupTrip(trip)` (§4.4) on a pair of pathfinders, for a trip of
    /// `source`.
    pub fn setup_trip(&self, pf: &mut Pathfinder, pf2: &mut Pathfinder, trip: &TripData, source: Source) {
        self.run(|r| {
            let rect = r.source_rect(source);
            r.setup_trip(pf, pf2, trip, rect);
        });
    }

    /// The return leg's start rect (`None`: pf2 keeps its own) and end rect
    /// from pf's end cell, its result node's travel type and its start rect
    /// (§4.4, TS17).
    pub fn return_leg_rects(&self, end: (i32, i32), end_type: u8, pf_start: CellRect) -> (Option<CellRect>, CellRect) {
        self.run(|r| r.return_leg_rects(end, end_type, pf_start))
    }

    /// `CapacityStillAvailable(trip)` (§4.2).
    pub fn capacity_still_available(&self, trip: &TripData) -> bool {
        self.run(|r| r.capacity_still_available(trip))
    }

    /// `ApplyTrafficToDestination(count, x, z, lastTravelType, type)` (§4.2).
    pub fn apply_traffic_to_destination(&self, count: u32, x: i32, z: i32, last_travel_type: u8, trip_type: u8) {
        self.run(|r| r.apply_traffic_to_destination(count, x, z, last_travel_type, trip_type));
    }

    // ---- the traffic shadow's steps (tooling, [`shadow`]) ----

    /// Step 2 of `DoDailySimulation` (§3.3) on its own: on a new day of
    /// month, day 1's incomes and the strategy re-draw.
    pub fn shadow_day_start(&self) {
        self.run(|r| r.day_start());
    }

    /// One `WorkOnSimCycle` (§3.2) with the phase transitions of §3.3 step 3
    /// (the pacing resets on entering 16, 17 and 18), without the share
    /// test: the caller decides where a day ends. With
    /// `trafficUnitsPerSlice` 1 a call in states 16–18 is one unit (§3.4).
    pub fn shadow_slice(&self) {
        self.run(|r| {
            let finished = r.work_on_sim_cycle();
            let _ = r.after_slice(finished, false);
        });
    }

    /// In ReRouteTrips or FullTrips: runs on, examining no trip, to where
    /// the original stopped after its last unit: the source at `target` (the
    /// original's saved source cell) held, or the next trip to examine,
    /// whichever comes first (`trip_run_on`). True when the phase finished
    /// on the way (the state is left as it is); false in any other state.
    pub fn shadow_advance(&self, target: [i16; 2]) -> bool {
        self.run(|r| match r.st.cycle_state {
            cycle_state::REROUTE_TRIPS => r.trip_run_on(false, target),
            cycle_state::FULL_TRIPS => r.trip_run_on(true, target),
            _ => false,
        })
    }

    /// In ReRouteTrips or FullTrips: the held source with no trip left to
    /// examine whose list end (merge or `CompleteTripSourceCalculations`) is
    /// not taken yet (tooling: the traffic shadow decides it from the
    /// original's record and trip list).
    pub fn shadow_pending_list_end(&self) -> Option<Source> {
        self.run(|r| match r.st.cycle_state {
            cycle_state::REROUTE_TRIPS => r.pending_list_end(false),
            cycle_state::FULL_TRIPS => r.pending_list_end(true),
            _ => None,
        })
    }

    /// Takes [`Self::shadow_pending_list_end`]'s list end (the next source is
    /// not taken). False when there is none.
    pub fn shadow_end_list(&self) -> bool {
        self.run(|r| match r.st.cycle_state {
            cycle_state::REROUTE_TRIPS => r.end_pending_list(false),
            cycle_state::FULL_TRIPS => r.end_pending_list(true),
            _ => false,
        })
    }

    /// The load resume of §3.5 step 11 for the trip phases, after a `Read`
    /// of a record taken from a running simulator: a phase that has taken no
    /// unit yet (its progress counter 0) restarts its source iterator with no
    /// source held; otherwise the resume of a load (the source at the saved
    /// cell held). Other states are left as the record has them.
    pub fn shadow_resume(&self) {
        self.run(|r| {
            use cycle_state as cs;
            if !matches!(r.st.cycle_state, cs::CONFIRM_TRIPS | cs::REROUTE_TRIPS | cs::FULL_TRIPS) {
                return;
            }
            if shadow::progress(r.st) == 0 {
                r.reset_sources();
                r.st.current = None;
                r.st.source_pos = 0;
                r.st.trip_pos = 0;
                r.st.continue_search = false;
            } else {
                r.resume_loaded_phase();
            }
        });
    }

    /// The damage sets' extra step costs on the network info, as
    /// `ApplyPotholeChanges` leaves them (§4.12): `damagedRoadExtraCost` on
    /// every damaged road (transit network 1) and rail (2) cell with network
    /// info, 0 on every intact one. After a `Read` that replaced the sets.
    pub fn shadow_apply_damage_costs(&self) {
        let d = self.damage.borrow().clone();
        self.net.update_state(|n| {
            let cost = n.tuning.damaged_road_cost;
            for (kind, block) in [(damage_kind::ROAD, &d.road), (damage_kind::RAIL, &d.rail)] {
                for (keys, extra) in [(&block.intact, 0.0), (&block.damaged, cost)] {
                    for key in keys.iter_order() {
                        let (x, z) = ((key >> 8) as i32 & 0xFF, (key & 0xFF) as i32);
                        if let Some(info) = n.network_info_mut(kind, x, z) {
                            info.extra_cost = extra;
                        }
                    }
                }
            }
        });
    }
}

/// The 16-bit cell key `(x << 8 | z)` truncated to 16 bits (§4.12, §4.13).
pub fn cell_key16(x: i32, z: i32) -> u16 {
    (((x as u32) << 8) | (z as u32 & 0xFF)) as u16
}
