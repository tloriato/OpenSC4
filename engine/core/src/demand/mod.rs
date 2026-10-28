// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The demand subsystem (`docs/specs/demand.md`): the demand simulator
//! (`cISC4DemandSimulator`, class `0x2990BFF2`) and its demand nodes
//! (`cISC4Demand`, class `0x8990C12D`). Section references (`§n`) point at
//! that spec.
//!
//! # Structure
//!
//! - [`DemandNode`]: one node (§3). Nodes are shared objects (`Rc`) with
//!   interior mutability, like the budget's departments: the class-object
//!   store creates and reads them independently of the simulator, the
//!   simulator only holds references (§12.1). A node posts
//!   ActiveDemandChanged through a [`DemandPoster`].
//! - [`DemandSim`]: the graph (§4), census (§6), occupancy (§7), monthly
//!   modifiers and cap relief (§8), region (§9), scheduling (§10), history
//!   (§10.4), tuning (§11) and persistence (§12).
//! - [`DemandHost`]: everything the simulator reads from other subsystems,
//!   as plain-data callbacks. The GZCOM shim implements it over the game's
//!   interfaces; the conformance runner implements it with a scripted world.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - Foreign nodes (§1 replaceability note): the core only manages nodes it
//!   holds as [`DemandNode`]. A growth developer's source node is read through
//!   [`DemandHost::growth_developers`] (plain supply/demand values), so foreign
//!   nodes never enter the node map. A save reference that resolves to a host
//!   object (not a core node) is released and dropped on load.
//! - Embedded nodes in the save (§12.1, open question 1): written and read as
//!   object references (foundations §2.7), with each node in its own class
//!   record `0x8990C12D`.
//! - ActiveDemandChanged is posted on every `CalculateActiveDemand` and every
//!   `SimulationBegin` (no coalescing, §3.4).
//! - Caps links never make the propagation loop unstable (§4.4: only
//!   transfers and sums corrections keep it going; a cap is neither).
//! - A link whose target node does not exist is ignored (it neither transfers
//!   nor counts for Sums) and a warning is logged once per (source, target).
//! - `GetConnectionsOfType(T)` is the subset of the connection list with type
//!   T (§8.3), so the host provides one list.
//! - Census slicing (§6.2): the deadline is checked after every cell, and every
//!   `DoIdle` call processes at least one cell while scanning.
//! - The desirability weight is a float division (`value / 100.0`, §6.3).
//! - Regional sums (§9.1) and tax-history maxima are accumulated in `f32`.
//! - Missing nodes in the daily history (§10.4) record nothing.
//! - A growth developer without a source node is skipped by ExportDemand.

mod node;
mod shadow;
mod simulator;
mod tuning;
pub mod archive;

pub use node::{DemandNode, NodeData, NODE_RECORD_VERSION};
pub use shadow::{ShadowGraph, ShadowModifiers};
pub use simulator::{DemandSim, DemandSnapshot, Link, LinkKind, ModifierTerms, PropagationStats, SIM_RECORD_VERSION};
pub use tuning::{exemplar_prop, DemandExemplar, DemandTuning, NationEconomy, NATION_KEY, TUNING_KEY};

use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::stream::ResKey;

/// Class IDs (§1).
pub const CLSID_DEMAND_SIMULATOR: u32 = 0x2990_BFF2;
pub const CLSID_DEMAND_NODE: u32 = 0x8990_C12D;
/// Interface IDs (§1, inferred in the spec).
pub const IID_DEMAND_SIMULATOR: u32 = 0x4796_7E16;
pub const IID_DEMAND_NODE: u32 = 0x0797_A4E8;
/// `GetSimulatorType()` (§5).
pub const SIMULATOR_TYPE: u32 = 0x2796_81D7;
/// Agent name and flags (§10.1). Levels: SimBegin, Day, Month, Idle.
pub const AGENT_NAME: &str = "cSC4DemandSimulator";
pub const AGENT_FLAGS: u32 = 2;
pub const AGENT_LEVELS: [u32; 4] = [0, 4, 6, 10];

/// Demand exemplars (§4.1): group of the shipped data, ExemplarType value.
pub const DEMAND_EXEMPLAR_GROUP: u32 = 0xC7BB_4816;
pub const DEMAND_EXEMPLAR_TYPE: u32 = 3;

/// Demand indices (§2).
pub const INDEX_LOCAL: u32 = 0x0_0000;
pub const INDEX_REGIONAL: u32 = 0x1_0000;
pub const INDEX_TOTAL: u32 = 0x2_0000;

/// Messages (§3.4, §10.2).
pub mod msg {
    pub const ACTIVE_DEMAND_CHANGED: u32 = 0x4268_40A0;
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const SIM_IDLE: u32 = 0x6695_681A;
    pub const PRE_SAVE: u32 = 0x26C6_3343;
    pub const STATE_CHANGED: u32 = 0xE8BD_A2E7;
    pub const CONNECTION_CREATED: u32 = 0x8A0D_15DA;
    pub const CONNECTION_DELETED: u32 = 0xEA0D_163E;
    pub const INGRED_CHANGES_MADE: u32 = 0xCA1B_151C;
    /// The five notifications the simulator subscribes to (§13 step 8).
    pub const SUBSCRIPTIONS: [u32; 5] = [
        STATE_CHANGED,
        CONNECTION_CREATED,
        CONNECTION_DELETED,
        PRE_SAVE,
        INGRED_CHANGES_MADE,
    ];
}

/// Building exemplar properties (§7.1).
pub mod building_prop {
    pub const SATISFIED: u32 = 0x2781_2840;
    pub const CREATED: u32 = 0x2781_2841;
    pub const SATISFIED_AMOUNTS: u32 = 0x2781_2842;
    pub const CREATED_AMOUNTS: u32 = 0x2781_2843;
    pub const NO_DEMAND_EFFECTS: u32 = 0x2781_2844;
}

/// Neighbour connection types (SDK `cISC4NeighborsSimulator::ConnectionType`).
pub mod connection {
    pub const RAIL: u32 = 1;
    pub const ROAD: u32 = 2;
    pub const HIGHWAY: u32 = 3;
    pub const SUBWAY: u32 = 6;
    pub const LIGHT_RAIL: u32 = 7;
    pub const AVENUE: u32 = 8;
    pub const DIRT_ROAD: u32 = 10;
    pub const MONORAIL: u32 = 11;
    pub const GROUND_HIGHWAY: u32 = 12;
}

/// History series (§10.4).
pub mod history {
    /// Daily: trunc(active demand) of the 12 total nodes, in [`super::TOTAL_NODES`] order.
    pub const DAILY: [u32; 12] = [
        0x2A67_A399,
        0xAA67_A3A0,
        0x8A67_A3A4,
        0x0A67_A3A9,
        0xAA67_A3AD,
        0xCA67_A3B2,
        0xAA67_A3B7,
        0x2A67_A3BC,
        0xEA67_A3BF,
        0x0A67_A3C4,
        0xEA67_A3C8,
        0x8A67_A3CC,
    ];
    pub const RESIDENTIAL_TOTAL: u32 = 0x2A4E_2003;
    pub const RESIDENTS: [u32; 3] = [0x4A5E_9485, 0xEA5E_948A, 0x2A5E_948E];
    pub const C_JOBS: [u32; 5] = [0x8A5E_950E, 0x0A5E_9515, 0x8A5E_954A, 0x8A5E_9552, 0xEA5E_9557];
    pub const C_JOBS_TOTAL: u32 = 0x0A4E_2056;
    pub const I_JOBS: [u32; 4] = [0x2A5E_95A1, 0x6A5E_95A7, 0x0A5E_95AC, 0xAA5E_95B1];
    pub const I_JOBS_TOTAL: u32 = 0x4A4E_206B;
    pub const JOBS_DEMAND: u32 = 0x6A93_1184;
    pub const UNFILLED_JOBS: u32 = 0x8A93_1172;
    pub const AVERAGE_SALARY: u32 = 0x8A63_F30C;
    pub const EXPORT_R: u32 = 0xAA77_2F1D;
    pub const EXPORT_C: u32 = 0x6A77_4F64;
    pub const EXPORT_I: u32 = 0xCA77_4F69;
    /// Tax-rate series of the 12 types (§8.1), recorded by the budget.
    pub const TAX_RATE_BASE: u32 = 0xAABA_AA9F;
}

/// The 12 R/C/I total nodes with their demand IDs (§8.1), in history order.
pub const TOTAL_NODES: [(u32, u32); 12] = [
    (0x21010, 0x1010),
    (0x21020, 0x1020),
    (0x21030, 0x1030),
    (0x23110, 0x3110),
    (0x23120, 0x3120),
    (0x23130, 0x3130),
    (0x23320, 0x3320),
    (0x23330, 0x3330),
    (0x24100, 0x4100),
    (0x24200, 0x4200),
    (0x24300, 0x4300),
    (0x24400, 0x4400),
];

/// The 24 census keys (§6.1), ascending.
pub const CENSUS_KEYS: [u32; 24] = [
    0x1011, 0x1021, 0x1031, 0x2111, 0x2112, 0x2113, 0x2114, 0x2121, 0x2122, 0x2123, 0x2124, 0x2131, 0x2132, 0x2133,
    0x2134, 0x3111, 0x3121, 0x3131, 0x3321, 0x3331, 0x4101, 0x4201, 0x4301, 0x4401,
];

/// The nine commercial/industrial developer types (§2).
pub const BUSINESS_DEV_TYPES: [u32; 9] = [0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400];
/// The three residential developer types (§2).
pub const RESIDENTIAL_DEV_TYPES: [u32; 3] = [0x1010, 0x1020, 0x1030];

/// Posts ActiveDemandChanged (§3.4) through the message server (queued).
pub trait DemandPoster {
    /// `node` is the host token of the posting node (0 if none).
    fn active_demand_changed(&self, node: u64, key: u32, data2: i32);
}

/// The lot at a census cell (§6.3), when it has capacity and a building.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CensusLot {
    /// `GetBuildingType(true)` (non-zero).
    pub building_type: u32,
    /// `GetOccupantWealth()`.
    pub wealth: u32,
}

/// `GetBuildingProfile(bt)` (§6.3, §7.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildingProfile {
    /// `PurposeType` (1 Residence … 8 HighTech; 0 none).
    pub purpose: u32,
    /// The capacity list: a vector of maps developer type → capacity.
    pub capacities: Vec<Vec<(u32, u16)>>,
}

/// One neighbour connection (§8.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Connection {
    /// Host token of the connection.
    pub token: u64,
    /// `GetType()` (SDK `ConnectionType`).
    pub conn_type: u32,
    /// `GetCityOnOtherSideOfConnection()` (host token, 0 = none).
    pub city: u64,
}

/// One growth developer (§9.2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrowthDeveloper {
    /// `GetSubType()` (the developer type).
    pub sub_type: u32,
    /// `GetType()`: 1 R, 2 C, 3 I.
    pub kind: u32,
    /// `GetSourceDemand()`'s committed (supply, demand); `None` if it has no
    /// source node.
    pub source: Option<(f32, f32)>,
}

/// What the demand simulator reads from, and does to, the rest of the game.
/// Tokens (`u64`) are opaque host identities (interface pointers in the shim).
///
/// Re-entrancy: callbacks must not call back into the demand simulator.
pub trait DemandHost {
    // ---- resources ----
    /// Every exemplar resource (type `0x6534284A`, any group) in the resource
    /// manager's enumeration order (§4.2).
    fn exemplar_keys(&self) -> Vec<ResKey>;
    /// Loads one exemplar by key (as a property holder), or `None`.
    fn exemplar(&self, key: ResKey) -> Option<Rc<PropertyHolder>>;
    /// Logs a diagnostic (spec warnings: missing link targets, runaway
    /// propagation).
    fn warn(&self, _text: &str) {}

    // ---- node lifetime ----
    /// Creates a demand node through the class factory (`0x8990C12D`); the
    /// node carries one reference owned by the caller.
    fn create_node(&self) -> Option<Rc<DemandNode>>;
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- city and clock ----
    /// `CellCountX()`, `CellCountZ()`.
    fn cell_counts(&self) -> (i32, i32);
    /// The simulator's current date as a Julian day number.
    fn today(&self) -> u32;
    /// A millisecond wall clock (census time slicing, §6.2).
    fn now_ms(&self) -> u32;

    // ---- census (§6.3) ----
    /// `GetLot(x, z, true)`, if it exists, `HasCapacity()` and has a
    /// non-zero `GetBuildingType(true)`.
    fn census_lot(&self, x: i32, z: i32) -> Option<CensusLot>;
    /// `GetBuildingProfile(bt)`; `None` = no profile (purpose 0).
    fn building_profile(&self, building_type: u32) -> Option<BuildingProfile>;
    /// `GetBuildingProfile(bt).purpose` alone, as the census reads it (§6.3,
    /// DQ5: the original reads `purpose` at +0x00 and walks no capacity map);
    /// `None` = no profile (purpose 0). A host should answer without reading
    /// the capacity maps; the default goes through [`Self::building_profile`].
    /// DQ13 (answered: yes to both, with one condition): the default is
    /// faithful as long as an unknown type yields purpose 0, as the
    /// original's inserted zero profile does (`None` or `Some(0)` here: the
    /// census reads both as purpose 0); the host's visitor may be null.
    fn building_purpose(&self, building_type: u32) -> Option<u32> {
        self.building_profile(building_type).map(|p| p.purpose)
    }
    /// `GetDeveloperType(purpose, wealth)`.
    fn developer_type(&self, purpose: u32, wealth: u32) -> u32;
    /// `GetDeveloper(devType).GetDesirabilityMap().GetValue(x, z)`.
    fn desirability(&self, dev_type: u32, x: i32, z: i32) -> i16;
    /// `lot.GetCapacity(devType, false)` of the lot at (x, z).
    fn lot_capacity(&self, x: i32, z: i32, dev_type: u32) -> u16;
    /// `ResidentialSim.GetCellEQ(x, z)`.
    fn cell_eq(&self, x: i32, z: i32) -> f32;
    /// `ResidentialSim.GetCellWorkforcePercent(x, z)`.
    fn cell_workforce_percent(&self, x: i32, z: i32) -> f32;

    // ---- occupancy (§7) ----
    /// `lot.GetBuildingType(true)`.
    fn lot_building_type(&self, lot: u64) -> u32;
    /// Whether the lot's building has bool property `0x27812844` = true.
    fn lot_excluded(&self, lot: u64) -> bool;
    /// `GetAllBuildingTypes`.
    fn all_building_types(&self) -> Vec<u32>;
    /// The exemplar of a building type (`GetBuildingKeyFromType` + load).
    fn building_exemplar(&self, building_type: u32) -> Option<Rc<PropertyHolder>>;

    // ---- modifiers (§8.1) ----
    /// `GetMinMaxAverage(series, from, to).max`, `None` when the window holds
    /// no data.
    fn history_max(&self, series: u32, from_day: u32, to_day: u32) -> Option<f32>;
    /// `OrdinanceSim.GetCumulativeFactor(prop, index)`.
    fn ordinance_factor(&self, prop: u32, index: u32) -> f32;
    /// `Nation.GetEconomyModifier(demandID)` (§8.2; see [`NationEconomy`]).
    fn nation_economy_modifier(&self, demand_id: u32) -> f32;

    // ---- cap relief (§8.3) ----
    /// `NeighborsSim.GetAllConnections()`.
    fn connections(&self) -> Vec<Connection>;
    /// `TrafficSim.GetTrafficArrived(conn.AsISCPropertyHolder(), 3)`.
    fn traffic_arrived(&self, connection: u64) -> f32;
    /// `City.GetAirportDeveloper().GetNumAirportTrips()`.
    fn airport_trips(&self) -> f32;
    /// `City.GetSeaportDeveloper().GetNumSeaportTrips()`.
    fn seaport_trips(&self) -> f32;

    // ---- region (§9) ----
    /// `NeighborsSim.GetAllConnectedCities(list, false)`.
    fn all_connected_cities(&self) -> Vec<u64>;
    /// `NeighborsSim.GetConnectedCities(list, false)`.
    fn connected_cities(&self) -> Vec<u64>;
    /// `App.GetRegionalCity()` (this city's own region entry).
    fn own_regional_city(&self) -> u64;
    fn city_population(&self, city: u64, pop_type: u32) -> i32;
    fn city_extrapolated_population(&self, city: u64, pop_type: u32) -> i32;
    fn city_allowable_extrapolation(&self, city: u64, pop_type: u32) -> i32;
    fn city_tax_rate(&self, city: u64, tax_type: u32) -> f32;
    fn city_workforce_percentage(&self, city: u64) -> f32;
    fn city_extrapolate_growth(&self, city: u64, pop_type: u32, amount: f32);
    /// `BDS.GetDevelopers()` (map order).
    fn growth_developers(&self) -> Vec<GrowthDeveloper>;
    /// `BDS.GetDayOfLastSuccessfulBuild(devType)`.
    fn day_of_last_successful_build(&self, dev_type: u32) -> u32;

    // ---- history (§10.4) ----
    fn record_history(&self, series: u32, value: i32);
    fn set_history_queue_length(&self, series: u32, length: u32);
}
