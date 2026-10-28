// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The commercial simulator (`cSC4CommercialSimulator`, class `0x8990C080`)
//! and the industrial simulator stub (`cSC4IndustrialSimulator`, class
//! `0xA990C022`), `docs/specs/commercial-industrial.md`. Section references
//! (`§n`) point at that spec.
//!
//! # Structure
//!
//! - `simulator.rs`: the commercial simulator: the state of §2.1, `AddRef` /
//!   `Release` (§2.3), the agents and messages of §3, `Init` / `Shutdown`
//!   (§4.1, §4.2), the four-state `DoIdle` machine (§4.3), the map rebuild
//!   (§4.4, the residential seed-and-converge transform run directly on the
//!   proximity grid), `GetProximityMap` (§4.6) and the record of §8.1.
//! - `industrial.rs`: the industrial stub (§2.2, §8.2).
//! - `tests.rs`: a fake host and the §13 tests.
//!
//! Everything outside the class goes through [`CommercialHost`]: the agents
//! and the message server (§3), the city, the building development
//! simulator and its building map (§4.4), the demand simulator's two nodes
//! (§2.1), the occupants' building profiles (§3) and the grids themselves
//! (the host creates the game's own `cSC4SimGrid<uint8>`, simgrid §6.3,
//! through [`GridProvider`]).
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **The seed** (§4.4 *[compat]*, RG23) is [`crate::residential::seed_proximity`]
//!   and the passes [`crate::residential::distance_to_convergence`]: the
//!   shared helper of residential.md §5.3. So only the z = 0 edge is seeded
//!   and the maps are 1 on that edge (0 under an office of that wealth) and 0
//!   elsewhere on a fresh grid. FreeCity4 does not correct RG23 (CM2: a
//!   correction would be a separate option, off by default; not offered).
//! - **No building map** (BDS slot 41 returns null) *[compat]*: helper
//!   `0x005E5130` returns at once, with no seed and **no passes** (§4.4,
//!   QUESTIONS CM5), so the grid is left exactly as it was: a fresh grid
//!   stays all 0 and a loaded one keeps its values.
//! - **A grid that is absent** at a rebuild (an `Init` that failed before
//!   step 6, or a record whose reference did not resolve): [choice] that
//!   rebuild does nothing; the state machine still advances.
//! - **`Init` after a failed `Init`** (§4.1 step 5) returns true at once
//!   (`initialized` is set in step 1) without creating the grids: the
//!   literal reading of the spec *[question]*.
//! - **Message server**: unsubscribing and releasing happen only when one is
//!   held ([choice]; the original holds one in every shipped case).
//! - **`Read`** (§8.1) is atomic [choice]: a stream error leaves the object
//!   unchanged (the original sets `dirty` before reading the references).
//!   Old grids are released before they are replaced (the spec's FreeCity4
//!   *[choice]* for the §8.1 quirk).
//! - **The write-only statics** `0x00B48614` (commercial) and `0x00B490A4`
//!   (industrial) are kept as the `static_flag` state fields; nothing reads
//!   them.
//! - **Threading** (foundations §19.3): the state is a `RefCell`; the core is
//!   main-thread only, like every subsystem core. Host callbacks must not
//!   call back into the simulator.

mod industrial;
mod record;
mod simulator;

#[cfg(test)]
mod tests;

pub use industrial::{IndustrialSim, IndustrialState};
pub use record::CommercialRecord;
pub use simulator::{query_interface, CommercialSim, CommercialState};

use std::rc::Rc;

use crate::grid::{GridProvider, TractGrid};

/// Class IDs (`GetGZCLSID`, §1).
pub const CLSID_COMMERCIAL_SIMULATOR: u32 = 0x8990_C080;
pub const CLSID_INDUSTRIAL_SIMULATOR: u32 = 0xA990_C022;

/// Interface IDs (§1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_COMMERCIAL_SIMULATOR: u32 = 0x077A_BEB2;
pub const IID_INDUSTRIAL_SIMULATOR: u32 = 0x477A_9E7D;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// Grid references are read with this IID (§8.1).
pub const IID_SIM_GRID: u32 = 0xC9BD_939A;
/// `cISC4BuildingOccupant` (§3).
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;

/// City components (foundations §3.6) and the city's getter slots (§1).
pub const CITY_COMPONENT_COMMERCIAL: u32 = 22;
pub const CITY_GETTER_SLOT_COMMERCIAL: u32 = 54;
pub const CITY_COMPONENT_INDUSTRIAL: u32 = 28;
pub const CITY_GETTER_SLOT_INDUSTRIAL: u32 = 60;

/// Agent registration (§3): four agents, all named
/// `"cSC4CommercialSimulator"`, flags 2, at levels 0, 10, 4, 6 in that order.
pub const AGENT_NAME: &str = "cSC4CommercialSimulator";
pub const AGENT_FLAGS: u32 = 2;
pub const AGENT_LEVELS: [u32; 4] = [0, 10, 4, 6];

/// Messages (§3).
pub mod msg {
    /// One-shot agent: ignored.
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    /// Day agent: one `DoIdle` step.
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    /// Month agent: ignored.
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    /// Idle agent: one `DoIdle` step.
    pub const SIM_IDLE: u32 = 0x6695_681A;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    /// The subscriptions of `Init` step 3, in order.
    pub const SUBSCRIPTIONS: [u32; 2] = [OCCUPANT_INSERTED, OCCUPANT_REMOVED];
}

/// The occupant type (data3) of a building insert / remove (§3).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
/// The building profile purpose that marks the maps dirty: commercial
/// office (§3).
pub const PURPOSE_OFFICE: u32 = 3;
/// The two wealths rebuilt (§4.4): CO$$ and CO$$$.
pub const WEALTH_CO2: u32 = 2;
pub const WEALTH_CO3: u32 = 3;

/// Grid instance IDs (§2.1).
pub mod instance {
    pub const CO2: u32 = 0x89D5_B788;
    pub const CO3: u32 = 0x89D5_B6EF;
}

/// The grids' class (`uint8`, simgrid §2) and tract size (§2.1, §7).
pub const GRID_CLASS_U8: u32 = 0x49B9_E602;
pub const TRACT_SIZE: i32 = 1;

/// The developer types `GetProximityMap` answers (§4.6).
pub mod developer {
    pub const CO2: u32 = 0x3320;
    pub const CO3: u32 = 0x3330;
}

/// The demand nodes held but never read (§2.1): `GetDemand(id, 0)`, in the
/// order `Init` asks for them.
pub const DEMAND_NODES: [u32; 2] = [0x3000, 0x3800];

/// The `DoIdle` states (§4.3).
pub mod state {
    pub const START: i32 = 0;
    pub const REBUILD_CO2: i32 = 1;
    pub const REBUILD_CO3: i32 = 2;
    pub const IDLE: i32 = 3;
}

/// The record versions (§8.1, §8.2).
pub const RECORD_VERSION: u16 = 1;
pub const INDUSTRIAL_RECORD_VERSION: u16 = 1;

/// A message as `DoMessage` sees it (§3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommercialMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    /// The occupant type of an insert / remove message.
    pub data3: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message.
    pub void1: Option<u64>,
}

impl CommercialMessage {
    pub fn of_type(msg_type: u32) -> CommercialMessage {
        CommercialMessage {
            msg_type,
            ..CommercialMessage::default()
        }
    }
}

/// Everything the commercial simulator reads from, or does to, the rest of
/// the game. Objects are host tokens. Re-entrancy: callbacks must not call
/// back into the simulator.
pub trait CommercialHost: GridProvider<u8> {
    // ---- agents and messages (§3, §4.1, §4.2) ----
    /// `AddAgent(messageTarget, level, name, flags)` on the city's simulator.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// `RemoveAgent(messageTarget)` on the city's simulator.
    fn remove_agent(&self);
    /// `Init` step 3: hold the cached message server; false when there is
    /// none. Called only when the simulator holds none.
    fn acquire_message_server(&self) -> bool;
    /// Message server slots 5 / 6 with the message-target sub-object.
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// `Shutdown`: release the held message server.
    fn release_message_server(&self);

    // ---- services (§4.1 step 4) ----
    /// Hold the cached city; false when there is none. Called only when the
    /// simulator holds none.
    fn acquire_city(&self) -> bool;
    fn release_city(&self);
    /// Hold the cached building development simulator; false when there is
    /// none. Called only when the simulator holds none.
    fn acquire_bds(&self) -> bool;
    fn release_bds(&self);

    // ---- the demand nodes (§2.1, §4.1 step 5) ----
    /// `city.GetDemandSimulator()` (city slot 56) is not null.
    fn has_demand_simulator(&self) -> bool;
    /// `GetDemand(demand_id, 0)` on the demand simulator, `AddRef`ed and
    /// held by the simulator; `None` for a null node.
    fn demand_node(&self, demand_id: u32) -> Option<u64>;
    /// Releases a held demand node (`Shutdown`).
    fn release_demand_node(&self, node: u64);

    // ---- grids (§4.1 step 6, §4.2) ----
    /// `Shutdown` on a grid before the simulator releases it.
    fn shutdown_grid(&self, grid: &Rc<dyn TractGrid<u8>>);

    // ---- the rebuild's inputs (§4.4) ----
    /// BDS slot 41 `GetBuildingMap()`.
    fn building_map(&self) -> Option<Rc<dyn TractGrid<u8>>>;
    /// BDS slot 42 `GetBuildingMapKey(purpose, wealth)`; only the low byte
    /// matters.
    fn building_map_key(&self, purpose: u32, wealth: u32) -> u8;

    // ---- occupants (§3) ----
    /// `void1.QueryInterface(0x87DDA3A9)` then the building's slot 11
    /// `GetBuildingProfile()`: its first u32 (the purpose). `None` when the
    /// occupant is not a building occupant (or has no profile). The host
    /// releases the building interface.
    fn building_purpose(&self, occupant: u64) -> Option<u32>;
}
