// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The neighbours subsystem (`docs/specs/neighbors.md`): the city's neighbour
//! connections (§4), its neighbour deals (§5) and their life cycle (§7), deal
//! pricing (§8), and the region interaction (§9, foundations §11.3).
//!
//! The core owns three kinds of objects:
//!
//! - [`NeighborConnection`]: the connection cells (§4). Plain data; the
//!   simulator owns the list (§6.1) and saves it (§13).
//! - [`NeighborDeal`]: a deal (§5). Deals are shared (`Rc`): the simulator's
//!   list holds them and the region keeps references through
//!   [`NeighborsHost::set_local_deals`] (§9.2). A deal reaches its simulator
//!   through a weak link set when the simulator creates or adopts it.
//! - [`NeighborsSimulator`]: the class `0xC990C0A4` (§6).
//!
//! Regional cities are **host** objects (`cISC4RegionalCity`), named by
//! [`CityId`] tokens; the host answers every region query of §2.2, §8.2 and
//! §9 through [`NeighborsHost`]. Deals the region holds (`GetLocalDeals`,
//! §14.1 step 7) reach the core as [`DealData`] snapshots, which the core
//! clones into its own deals (§5.5) *[choice]*: the original clones the
//! original object; the result is the same deal seen from the current city.
//!
//! Conventions: no GZCOM or OS types; `Rc`/`RefCell`/`Cell` are main-thread
//! only; no `RefCell` borrow is held across a host call.
//!
//! *[choice]* items, each marked where it is implemented:
//! - `CreateConnection` returns the new connection (or null), §6.2.
//! - `RequestCreateConnection` returns [`ConnectionRequest`]: the shim shows
//!   the dialog, §6.2.
//! - `GetAdjacentCities` orders the cities by region position (x, then z),
//!   §9.1 (NB14: the original's order is by heap address).
//! - Unknown deal types price at 0, §8.1.
//! - A missing `0xEA46934F` in `UpdateCommodityStats` makes the free-landfill
//!   term 0, §12 (NB13: the original reads an uninitialized float).
//! - The four dates start at [`DEFAULT_DATE`] = 0, §5.1 (NB1: the original's
//!   default is the wall-clock day).
//! - Paths where the original crashes on a null do nothing instead:
//!   `UpdateCost` without a simulator (NB5), a null reference in the
//!   connection list on read (NB7), `UpdateState`'s budget test without a
//!   current regional city (NB15, and the missing payer at renewal, NB16),
//!   `UpdateCommodityStats` without a current regional city (NB12).
//! - The proposal generator (`cRZRandom`, double path) is created on first use
//!   with `derive_seed(session, 0xC990C0A4)` (foundations §6.3, a temporary
//!   seeded from the clock, `n = 0`). It belongs to the simulator object, not
//!   to the process.
//! - The two-curve switch (§8.3), off by default.

mod connection;
pub mod archive;
mod deal;
mod pricing;
mod simulator;
#[cfg(test)]
mod tests;
mod tuning;

pub use connection::{ConnectionState, NeighborConnection, CONNECTION_RECORD_VERSION};
pub use deal::{DealCall, DealData, DealHost, NeighborDeal, DEAL_RECORD_VERSION, DEFAULT_DATE};
pub use pricing::{deal_cost, deal_max_amount, CommodityStats, PriceInputs};
pub use simulator::{answers_iid, ConnectionRequest, NeighborsSimulator, SimState, SIMULATOR_RECORD_VERSION};
pub use tuning::{prop, shipped_tuning, Tuning, DEAL_PRICE_CURVE, GARBAGE_COST_CURVE, TUNING_KEY};

use std::rc::Rc;

use crate::property::PropertyHolder;

/// Class IDs and interfaces (§1).
pub const CLSID_NEIGHBORS_SIMULATOR: u32 = 0xC990_C0A4;
pub const IID_NEIGHBORS_SIMULATOR: u32 = 0x0773_FEF0;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const CLSID_NEIGHBOR_CONNECTION: u32 = 0x0A13_4F24;
pub const IID_NEIGHBOR_CONNECTION: u32 = 0x0773_FEF2;
pub const CLSID_NEIGHBOR_DEAL: u32 = 0x2A13_4F42;
pub const IID_NEIGHBOR_DEAL: u32 = 0x0773_FEF1;

/// The proposal generator's constant `K` (foundations §6.3: the owner's
/// GZCLSID XOR the creation count 0).
pub const PROPOSAL_SEED_KEY: u32 = crate::seeding::keys::neighbors_proposals(0).k();

/// Agent registration (§14.1 step 2): month, day and idle levels, flags 2.
pub const AGENT_NAME: &str = "cSC4NeighborsSimulator";
pub const AGENT_LEVELS: [u32; 3] = [6, 4, 10];
pub const AGENT_FLAGS: u32 = 2;

/// A regional city (`cISC4RegionalCity`), as a host token.
pub type CityId = u64;

/// A city object (`cISC4City`), as a host token: the subject of
/// `UpdateCommodityStats` (§8.2, NB12), compared by identity with the current
/// city. A different object from any regional city, our own included.
pub type CityHandle = u64;

/// One region cell holds 64 city cells (§2.1).
pub const REGION_CELL: i32 = 64;

/// Deal types (§3).
pub mod deal_type {
    pub const BUY_POWER: u32 = 0x4773_FFE0;
    pub const SELL_POWER: u32 = 0x4773_FFE1;
    pub const BUY_WATER: u32 = 0x4773_FFE2;
    pub const SELL_WATER: u32 = 0x4773_FFE3;
    pub const BUY_GARBAGE: u32 = 0x4773_FFE4;
    pub const SELL_GARBAGE: u32 = 0x4773_FFE5;
    /// The six known types.
    pub const ALL: [u32; 6] = [BUY_POWER, SELL_POWER, BUY_WATER, SELL_WATER, BUY_GARBAGE, SELL_GARBAGE];

    /// One of the six types of §3.
    pub fn is_known(t: u32) -> bool {
        (BUY_POWER..=SELL_GARBAGE).contains(&t)
    }

    /// Even types: this city is the buyer and pays (§3).
    pub fn is_buy(t: u32) -> bool {
        is_known(t) && t & 1 == 0
    }

    /// Odd types: the neighbour pays.
    pub fn is_sell(t: u32) -> bool {
        is_known(t) && t & 1 == 1
    }

    /// Swapping the perspective toggles the low bit (§3).
    pub fn toggled(t: u32) -> u32 {
        t ^ 1
    }

    /// The commodity index (§3): 1 power, 2 water, 3 garbage.
    pub fn commodity(t: u32) -> Option<i32> {
        match t {
            BUY_POWER | SELL_POWER => Some(1),
            BUY_WATER | SELL_WATER => Some(2),
            BUY_GARBAGE | SELL_GARBAGE => Some(3),
            _ => None,
        }
    }

    /// The base-amount tuning property (§3, §12).
    pub fn base_property(t: u32) -> Option<u32> {
        match commodity(t)? {
            1 => Some(super::prop::BASE_POWER),
            2 => Some(super::prop::BASE_WATER),
            _ => Some(super::prop::BASE_GARBAGE),
        }
    }
}

/// The SDK's `ConnectionType` (§3).
pub mod conn_type {
    pub const NONE: u32 = 0;
    pub const RAIL: u32 = 1;
    pub const ROAD: u32 = 2;
    pub const HIGHWAY: u32 = 3;
    pub const POWER_POLE: u32 = 4;
    pub const PIPE: u32 = 5;
    pub const SUBWAY: u32 = 6;
    pub const LIGHT_RAIL: u32 = 7;
    pub const AVENUE: u32 = 8;
    pub const FERRY: u32 = 9;
    pub const DIRT_ROAD: u32 = 10;
    pub const MONORAIL: u32 = 11;
    pub const GROUND_HIGHWAY: u32 = 12;

    /// Transport types: all except 0, 4 and 5 (§3).
    pub const TRANSPORT_MASK: u32 = 0x1FCE;

    pub fn is_transport(t: u32) -> bool {
        t < 32 && TRANSPORT_MASK & (1 << t) != 0
    }

    /// The transport types a garbage deal searches, in order (§3, §6.6).
    pub const GARBAGE_SEARCH: [u32; 5] = [HIGHWAY, GROUND_HIGHWAY, AVENUE, ROAD, DIRT_ROAD];

    /// Elevated types: marker props sit 15.5 higher (§6.3, §11.1).
    pub fn is_elevated(t: u32) -> bool {
        matches!(t, HIGHWAY | LIGHT_RAIL | MONORAIL)
    }
}

/// The marker props' extra height for elevated types (§6.3).
pub const ELEVATED_MARKER_HEIGHT: f32 = 15.5;

/// Deal flags (§5.2).
pub mod flag {
    pub const PROPOSED: u32 = 0x001;
    pub const ACTIVE: u32 = 0x002;
    pub const CANCELLED: u32 = 0x004;
    pub const NO_CONNECTION: u32 = 0x008;
    pub const CANNOT_AFFORD: u32 = 0x010;
    pub const NO_LANDFILL: u32 = 0x020;
    pub const EXPIRED: u32 = 0x040;
    pub const SILENT: u32 = 0x080;
    pub const GRACE: u32 = 0x100;
    pub const PROBATION: u32 = 0x200;
    /// The failure flags (§5.2).
    pub const FAILURES: u32 = 0x38;
    /// Deals with any of these are ignored by totals and pricing (§5.2, §10.1).
    pub const TOTALS_EXCLUDED: u32 = 0x3D;
    /// `simulate_deals` step 3's test (§7.3).
    pub const BLOCKING: u32 = 0x3C;
    /// A new deal's flags (§5.1).
    pub const INITIAL: u32 = PROPOSED | SILENT;
}

/// Status report `data1` values (§5.3).
pub mod status {
    pub const CHANGED: u32 = 0;
    pub const ACTIVATED: u32 = 1;
    pub const CANCELLED: u32 = 2;
    pub const NO_CONNECTION: u32 = 3;
    pub const CANNOT_AFFORD: u32 = 4;
    pub const NO_LANDFILL: u32 = 5;
    pub const EXPIRED: u32 = 6;
    pub const RENEWED: u32 = 7;
    pub const RETRY: u32 = 8;
}

/// Messages (§11).
pub mod msg {
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_IDLE: u32 = 0x6695_681A;
    /// The deal status report (§5.3).
    pub const DEAL_STATUS: u32 = 0xEA41_8D8F;
    pub const CONNECTIONS_READY: u32 = 0x6AC2_84F3;
    pub const TERRAIN_HEIGHTS_CHANGED: u32 = 0x278E_6003;
    pub const PRE_DEMOLISH_NETWORK_OCCUPANT: u32 = 0xAA7A_014D;
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    pub const CITY_ESTABLISHED: u32 = 0x26D3_1EC4;
    pub const CITY_OBLITERATED: u32 = 0x26D3_1EC5;
    /// Posted (§6.3, §6.4).
    pub const CONNECTION_CREATED: u32 = 0x8A0D_15DA;
    pub const CONNECTION_DELETED: u32 = 0xEA0D_163E;
    /// The seven subscriptions of §14.1 step 3, in that order.
    pub const SUBSCRIPTIONS: [u32; 7] = [
        PRE_DEMOLISH_NETWORK_OCCUPANT,
        TERRAIN_HEIGHTS_CHANGED,
        DEAL_STATUS,
        CONNECTIONS_READY,
        REMOVE_OCCUPANT,
        CITY_OBLITERATED,
        CITY_ESTABLISHED,
    ];
}

/// Occupant types and props (§6.3, §6.5, §11.1).
pub mod occupant {
    /// A network occupant.
    pub const NETWORK: u32 = 0xC772_BF98;
    /// A power pole *[inferred]*.
    pub const POWER_POLE: u32 = 0x2890_D4DE;
    /// The marker prop.
    pub const MARKER_PROP: u32 = 0x29F1_0000;
    /// The bool property marking a connection marker.
    pub const MARKER_FLAG: u32 = 0xCAA4_6595;
    /// The extra bool properties of pipe and subway markers.
    pub const MARKER_UNDERGROUND: u32 = 0xCAA4_5670;
    pub const MARKER_SUBWAY: u32 = 0xCAA4_5671;
    pub const MARKER_PIPE: u32 = 0xCAA4_5672;
}

/// Budget IDs (§8.2).
pub mod budget_ids {
    pub const POWER_DEPARTMENT: u32 = 0x8910_BC8A;
    pub const WATER_DEPARTMENT: u32 = 0xAA40_14B4;
    pub const SANITATION_DEPARTMENT: u32 = 0x6A42_A8F5;
    /// The waste-to-energy line item of the power department.
    pub const WASTE_TO_ENERGY_LINE: u32 = 0x1F47_0000;
}

/// History series (§8.2).
pub mod history {
    pub const POWER_IMPORTED: u32 = 0xAA11_F837;
    pub const POWER_EXPORTED: u32 = 0xAA11_F83A;
    pub const WATER_IMPORTED: u32 = 0x0A14_EBB5;
    pub const WATER_EXPORTED: u32 = 0xCA14_EBC6;
    pub const GARBAGE_EXPORTED: u32 = 0xEA11_017B;
    pub const GARBAGE_IMPORTED: u32 = 0xEA11_017E;
    pub const GARBAGE_DEMANDED: u32 = 0xEB54_F32E;
    pub const GARBAGE_PRODUCED: u32 = 0x4B54_F333;
}

// ---- geometry (§2) --------------------------------------------------------------------------------

/// The side of a connection cell (§2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    West,
    North,
    East,
    South,
    Interior,
}

/// §2.1, in the spec's order: `x < 0` → West; else `z < 0` → North; else
/// `x < countX` → (`z ≥ countZ` → South, else interior); else East.
pub fn side_of(x: i32, z: i32, count_x: i32, count_z: i32) -> Side {
    if x < 0 {
        Side::West
    } else if z < 0 {
        Side::North
    } else if x < count_x {
        if z >= count_z {
            Side::South
        } else {
            Side::Interior
        }
    } else {
        Side::East
    }
}

/// The inner cell (§2.1): the city cell next to the connection cell.
pub fn inner_cell(x: i32, z: i32, count_x: i32, count_z: i32) -> (i32, i32) {
    (x.clamp(0, (count_x - 1).max(0)), z.clamp(0, (count_z - 1).max(0)))
}

/// `GetConnectionInnerCell` (§4.2): `None` when the cell is inside the city
/// or outside on both axes (a corner).
pub fn connection_inner_cell(x: i32, z: i32, count_x: i32, count_z: i32) -> Option<(i32, i32)> {
    let out_x = x < 0 || x >= count_x;
    let out_z = z < 0 || z >= count_z;
    if out_x == out_z {
        return None;
    }
    Some(inner_cell(x, z, count_x, count_z))
}

/// The neighbouring region cell of connection cell `(x, z)` (§2.2) for a
/// regional city at `pos` of size `size` (region cells). `/` truncates
/// toward zero.
pub fn neighbouring_region_cell(side: Side, x: i32, z: i32, pos: (i32, i32), size: (i32, i32)) -> (i32, i32) {
    let (px, pz) = pos;
    let (sx, sz) = size;
    match side {
        Side::West => (px - 1, pz + z / REGION_CELL),
        Side::East => (px + sx, pz + z / REGION_CELL),
        Side::North => (px + x / REGION_CELL, pz - 1),
        Side::South => (px + x / REGION_CELL, pz + sz),
        Side::Interior => (px, pz),
    }
}

/// The neighbour's connection cell (§2.2 table), with `(rx, rz)` the
/// neighbouring region cell and the neighbour at `npos` with size `nsize`.
/// `mod` is the C remainder. An interior cell gives `(0, 0)` (NB6,
/// `0x006CE920`: the outputs start zeroed and the side switch has no interior
/// case); the helper fails only when there is no city at `(rx, rz)`, which is
/// the caller's lookup.
pub fn neighbour_connection_cell(
    side: Side,
    x: i32,
    z: i32,
    region_cell: (i32, i32),
    npos: (i32, i32),
    nsize: (i32, i32),
) -> (i32, i32) {
    let (rx, rz) = region_cell;
    let (qx, qz) = npos;
    let (tx, tz) = nsize;
    match side {
        Side::East => (-1, z % REGION_CELL + (rz - qz) * REGION_CELL),
        Side::West => (tx * REGION_CELL, z % REGION_CELL + (rz - qz) * REGION_CELL),
        Side::North => (x % REGION_CELL + (rx - qx) * REGION_CELL, tz * REGION_CELL),
        Side::South => (x % REGION_CELL + (rx - qx) * REGION_CELL, -1),
        Side::Interior => (0, 0),
    }
}

/// A valid connection cell for a regional city of size `size` (§2.2):
/// exactly one of `x ∈ {−1, 64·sx}` and `z ∈ {−1, 64·sz}` holds.
pub fn is_valid_connection_cell(x: i32, z: i32, size: (i32, i32)) -> bool {
    let on_x = x == -1 || x == REGION_CELL * size.0;
    let on_z = z == -1 || z == REGION_CELL * size.1;
    on_x != on_z
}

// ---- host ----------------------------------------------------------------------------------------

/// The regional-city commodity statistics (§8.1, §8.2; `double` values,
/// slots 54–63 of `cISC4RegionalCity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stat {
    Exported,
    Imported,
    Produced,
    Demanded,
    CostPerUnit,
}

/// A connection held by another regional city (`FindConnectionOnOtherSide`,
/// §6.2): an original object in plug-in mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForeignConnection {
    pub token: u64,
    /// `GetIsActive()`.
    pub active: bool,
}

/// The city's utility and budget figures `UpdateCommodityStats` reads
/// (§8.2). `None` departments are missing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UtilityFigures {
    /// `budget.GetTotalFunds()`, `GetTotalMonthlyIncome()`.
    pub total_funds: f64,
    pub monthly_income: f64,
    /// Power: `GetMonthlyProduction()`, `GetMonthlyRequirement()`.
    pub power_production: f64,
    pub power_requirement: f64,
    /// Plumbing: `GetWaterProductionCapacity()`, `GetWaterDemand()`.
    pub water_capacity: f64,
    pub water_demand: f64,
    /// Pollution: recycled, waste-to-energy capacity, incinerator capacity,
    /// abducted, produced last month.
    pub garbage_recycled: f64,
    pub waste_to_energy_capacity: f64,
    pub incinerator_capacity: f64,
    pub garbage_abducted: f64,
    pub garbage_produced: f64,
    /// `GetTotalExpenses()` of the power, water and sanitation departments.
    pub power_department_expenses: Option<f64>,
    pub water_department_expenses: Option<f64>,
    pub sanitation_department_expenses: Option<f64>,
    /// Full expenses of the power department's waste-to-energy line item
    /// `0x1F470000` (0 when missing).
    pub waste_to_energy_expenses: f64,
}

/// What the game provides to the neighbours simulator.
pub trait NeighborsHost {
    // ---- lifecycle ----
    /// The tuning exemplar "Neighbor Deals" (§12), if present.
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(this)`: every agent of the simulator.
    fn remove_agents(&self);
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);

    // ---- city ----
    /// `CellCountX/Z`.
    fn cell_counts(&self) -> (i32, i32);
    /// The simulator date as a Julian day ("today").
    fn today(&self) -> u32;
    /// The figures `UpdateCommodityStats` reads (§8.2).
    fn utility_figures(&self) -> UtilityFigures;
    /// `GetLandfillUsage(usage, capacity)` (landfill spec): `(usage, capacity)`.
    fn landfill_usage(&self) -> (u32, u32);
    /// A history record (§8.2), the value truncated to `int`.
    fn send_history(&self, series: u32, value: i32);

    // ---- region ----
    /// The current city's regional record.
    fn current_city(&self) -> Option<CityId>;
    /// The current city object (the game's global `cISC4City` pointer), or
    /// none (§8.2, NB12).
    fn current_city_object(&self) -> Option<CityHandle>;
    /// `region.GetCity(rx, rz)`.
    fn city_at(&self, rx: i32, rz: i32) -> Option<CityId>;
    /// `GetPosition()` and `GetCitySize()` in region cells.
    fn city_position(&self, city: CityId) -> (i32, i32);
    fn city_size(&self, city: CityId) -> (i32, i32);
    fn city_established(&self, city: CityId) -> bool;
    fn city_stat(&self, city: CityId, stat: Stat, commodity: i32) -> f64;
    fn set_city_stat(&self, city: CityId, stat: Stat, commodity: i32, value: f64);
    fn city_budget(&self, city: CityId) -> f64;
    fn set_city_budget(&self, city: CityId, value: f64);
    fn set_city_income(&self, city: CityId, value: f64);
    /// `city.FindConnection(type, x, z)` on another regional city (§6.2).
    fn city_find_connection(&self, city: CityId, conn_type: u32, x: i32, z: i32) -> Option<ForeignConnection>;
    /// `cISC4RegionalCity::GetAllConnections` as (type, other-side region
    /// cell) pairs (`GetAllConnectedCities`, §6.2).
    fn city_connections(&self, city: CityId) -> Vec<(u32, (i32, i32))>;
    /// `ChangeSymmetricConnection(conn, add)` (§6.3, §6.4).
    fn change_symmetric_connection(&self, city: CityId, conn: &Rc<NeighborConnection>, add: bool);
    /// `region.ReloadCity(city)` (§14.1 step 5).
    fn reload_city(&self, city: CityId);
    /// `UpdateLocalDeals()` (foundations §11.3).
    fn update_local_deals(&self, city: CityId);
    /// `GetLocalDeals()`: snapshots of the region's deals (§14.1 step 7).
    fn local_deals(&self, city: CityId) -> Vec<DealData>;
    /// `SetLocalDeals(list)` (foundations §11.3): the region keeps references
    /// to the deals and **removes** from `deals` every deal with a missing
    /// region city *[quirk]*.
    fn set_local_deals(&self, city: CityId, deals: &mut Vec<Rc<NeighborDeal>>);
    /// `UpdateImportExport()` (foundations §11.3).
    fn update_import_export(&self, city: CityId);

    // ---- messages ----
    /// Posts a deal status report `0xEA418D8F` (§5.3); `deal` is null for the
    /// retry report (status 8).
    fn post_status(&self, deal: Option<&Rc<NeighborDeal>>, status: u32);
    /// Sends a status report synchronously (`SetAmount`, §5.3): the host
    /// delivers it to every subscriber now, the simulator included.
    fn send_status(&self, deal: &Rc<NeighborDeal>, status: u32);
    /// Posts `0x8A0D15DA` / `0xEA0D163E` (§6.3, §6.4).
    fn post_connection_message(&self, msg_type: u32, conn_type: u32, x: i32, z: i32);

    // ---- markers (§6.3, §6.4, §11.1) ----
    /// `CellCenterToPosition(x, z)`.
    fn cell_center(&self, x: i32, z: i32) -> (f32, f32);
    /// `terrain.GetAltitude(px, pz)` at a world position: §6.3 asks it at
    /// the inner cell's centre, the point the marker is placed at (NB3).
    fn marker_altitude(&self, px: f32, pz: f32) -> f32;
    /// `terrain.GetMaxSurfaceLevel(x, z)` for the inner cell (§11.1).
    fn max_surface_level(&self, x: i32, z: i32) -> f32;
    /// Inserts the marker prop `0x29F10000` with the given bool properties
    /// set true; returns its token.
    fn add_marker_prop(&self, position: [f32; 3], orientation: i32, bool_props: &[u32]) -> Option<u64>;
    /// The props with `0xCAA46595 = true` in a cell.
    fn marker_props(&self, x: i32, z: i32) -> Vec<u64>;
    /// Removes a prop.
    fn remove_prop(&self, prop: u64);
    /// Moves a prop and updates it in the occupant manager.
    fn move_prop(&self, prop: u64, position: [f32; 3]);

    // ---- occupants (§6.5, §11.1) ----
    fn occupant_type(&self, occupant: u64) -> u32;
    /// The network type numbers a network occupant carries.
    fn occupant_networks(&self, occupant: u64) -> Vec<u32>;
    /// The occupant's cell.
    fn occupant_cell(&self, occupant: u64) -> (i32, i32);
    /// `cISC4PowerPoleOccupant`'s 14th method (compiled slot 13) for a power
    /// pole (§11.1).
    fn power_pole_flag(&self, occupant: u64) -> bool;

    // ---- connection objects (§15.2) ----
    /// A new connection of ours (§6.3) needs a host object (the GZCOM
    /// connection the region and the traffic simulator are given): the host
    /// makes one and returns its token, which the core stores. The
    /// simulator's list holds one reference to it. 0 = none (tests).
    fn connection_object(&self, _conn: &Rc<NeighborConnection>) -> u64 {
        0
    }
    /// The live state of a foreign connection (an original object accepted
    /// on load, §15.2), read through `cISC4NeighborConnection`; `None` if
    /// it cannot be read.
    fn foreign_connection(&self, _token: u64) -> Option<ConnectionState> {
        None
    }
    /// `SetVisitedThisCycle` / `SetIsActive` on a foreign connection.
    fn set_foreign_connection_flags(&self, _token: u64, _visited: Option<bool>, _active: Option<bool>) {}
    /// `Repair(city)` on a foreign connection (§14.1 step 4).
    fn repair_foreign_connection(&self, _token: u64, _city: CityId) {}
    /// The simulator's list let go of a connection (ours or foreign): the
    /// reference the list held is released.
    fn release_connection(&self, _token: u64) {}

    // ---- foreign deals (§15.4 NG4) ----
    /// The live state of a foreign deal (a caller's object `AddDeal` kept),
    /// read through `cISC4NeighborDeal`; `None` if it cannot be read.
    fn foreign_deal(&self, _token: u64) -> Option<DealData> {
        None
    }
    /// A call on a foreign deal through `cISC4NeighborDeal`: its own code
    /// runs. Returns the result of `Activate` / `SetFlags`, true for the
    /// others.
    fn foreign_deal_call(&self, _token: u64, _call: DealCall) -> bool {
        false
    }
    /// The core let go of a foreign deal (no list or report holds it any
    /// more): the reference `AddDeal` took is released.
    fn release_deal(&self, _token: u64) {}
}

/// A message as `DoMessage` sees it (§11.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NeighborsMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    /// `GetVoid1`: the occupant of `0xAA7A014D` / `0x99EF1143`.
    pub occupant: Option<u64>,
}

impl NeighborsMessage {
    pub fn of_type(msg_type: u32) -> NeighborsMessage {
        NeighborsMessage {
            msg_type,
            ..NeighborsMessage::default()
        }
    }
}
