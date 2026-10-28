// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The MySims: the agent simulator (`cSC4MySimAgentSimulator`, class
//! `0x29E37DEC`) and one MySim (`cSC4MySim`, class `0x4A1DBBBF`), the
//! simulation side of `docs/specs/mysims.md`. Section references (`§n`)
//! point at that spec.
//!
//! A MySim is an advisor of the advisor system (`crate::advisors`): its
//! class is registered with [`register`], and its behaviour is an
//! [`AdvisorExt`](crate::advisors::AdvisorExt) ([`MySim`]). The agent
//! ([`MySimAgent`]) forwards city messages to the MySims.
//!
//! Everything outside the two classes goes through [`MySimHost`]: the Lua
//! table, the city's buildings and lots, the simulators' grids and values,
//! the dispatch (automata, a stub per §10 *[choice]*), messages and save
//! references.
//!
//! # Choices (`[choice]`)
//!
//! - The My Street sim (`0x0BF606A5`) is out of scope; FreeCity4 runs it as a
//!   MySim that reports its own class ID [choice], so the agent finds it at
//!   index 5 (§1.3).
//! - MySim advisors beyond index 4 are ignored (§1.3, the spec's choice).
//! - Each MySim's generator is seeded with `derive_seed(session, 0x4A1DBBBF
//!   XOR creationIndex)` (foundations §6.3) [choice].
//! - `ResetAdvisorState` runs the plain advisor `Shutdown` and `Init` with
//!   the same table index [choice].
//! - The `AdviceCommand` message names its advisor by ID in `unknown` (the
//!   host resolves the advisor pointer) [choice].
//! - Records: version 17 is written; versions 11–17 are read except 12,
//!   whose inline dispatch record goes to the host
//!   ([`MySimHost::read_dispatch_inline`]) [choice].

mod agent;
pub mod archive;
pub mod dispatch;
pub mod pool;
pub mod scripted;
mod sim;
pub mod street;

#[cfg(test)]
mod dispatch_tests;
#[cfg(test)]
mod pool_tests;
#[cfg(test)]
mod tests;

pub use agent::{AgentState, MySimAgent};
pub use sim::{MySim, MySimState};

use std::cell::Cell;
use std::rc::Rc;

use crate::advisors::{clsid as adv_clsid, AdviceKind, AdviceSubject, AdvisorExt, AdvisorSystem};
use crate::stream::{InStream, RefValue};

/// Class IDs (§1.1).
pub mod clsid {
    pub const AGENT: u32 = 0x29E3_7DEC;
    pub const MYSIM: u32 = 0x4A1D_BBBF;
    pub const MY_STREET_SIM: u32 = 0x0BF6_06A5;
    /// `cSC4MySimDispatch` (§1.1, out of scope for the spec but named).
    pub const DISPATCH: u32 = 0xCBC1_4674;
}

/// `cSC4MySimDispatch`: class ID and read IID (§1.1).
pub const DISPATCH_CLSID: u32 = clsid::DISPATCH;
pub const DISPATCH_IID: u32 = iid::DISPATCH;

/// Interface IDs (§1.1).
pub mod iid {
    pub const AGENT: u32 = 0xA9E3_7EBB;
    pub const MYSIM: u32 = 0x0A1D_A42F;
    pub const BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;
    pub const DISPATCH: u32 = 0xEB8B_6F9A;
    pub const DISPATCH_TOOL: u32 = 0xC9B6_D7D6;
}

/// Messages (§3.2, §3.3, §5.1).
pub mod msg {
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_CHANGED: u32 = 0x99EF_1145;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const VIEW_CHANGED_1: u32 = 0xA6B7_9602;
    pub const VIEW_CHANGED_2: u32 = 0xA6B7_9612;
    pub const HOUR_CHANGED: u32 = 0x4477_8230;
    pub const ADVICE_COMMAND: u32 = 0x8A09_FA6C;
    pub const DISASTER_STARTED: u32 = 0xCA1B_1E64;
    pub const PIPE_BURST: u32 = 0xEA1F_0DCD;
    pub const TORNADO: u32 = 0x0A36_F1E9;
    pub const NEIGHBOR_CONNECTION_REMOVED: u32 = 0xEA0D_163E;
    /// The agent's subscriptions, in `Init` order (§3.2).
    pub const AGENT_SUBSCRIPTIONS: [u32; 11] = [
        OCCUPANT_INSERTED,
        OCCUPANT_CHANGED,
        OCCUPANT_REMOVED,
        VIEW_CHANGED_1,
        VIEW_CHANGED_2,
        HOUR_CHANGED,
        ADVICE_COMMAND,
        DISASTER_STARTED,
        PIPE_BURST,
        TORNADO,
        NEIGHBOR_CONNECTION_REMOVED,
    ];
    // The MySim's own (§3.3).
    pub const DEBUG_PRINT: u32 = 0x0B6F_3E27;
    pub const DEBUG_NEW_JOB: u32 = 0xAB6F_3E2D;
    pub const DEBUG_NEW_HOUSE: u32 = 0x4B6F_3EBF;
    pub const DEBUG_DISPATCH: u32 = 0x0BB0_4126;
    pub const MYSIM_UPDATE: u32 = 0x4BC4_2C48;
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const DISPATCH_NOTIFICATION: u32 = 0x2BA0_6B99;
    /// The MySim's subscriptions, in order.
    pub const MYSIM_SUBSCRIPTIONS: [u32; 6] = [
        DEBUG_PRINT,
        DEBUG_NEW_JOB,
        DEBUG_NEW_HOUSE,
        DEBUG_DISPATCH,
        MYSIM_UPDATE,
        POST_CITY_INIT,
    ];
    // Posted (§5.1).
    pub const MYSIMS_CHANGED: u32 = 0xAA25_827C;
    pub const MYSIM_MOVED_IN: u32 = 0xEA3A_DA33;
}

/// Dispatch notification sub-IDs (`0x2BA06B99` data1, §3.3).
pub mod dispatch_note {
    pub const TRIP_STATE_A: u32 = 0x4BB2_D020;
    pub const TRIP_STATE_B: u32 = 0x4BB2_CFC0;
    pub const JOB_FOUND: u32 = 0xEBA0_6BF4;
    pub const ARRIVAL: u32 = 0xEBA0_6C12;
    pub const STATE_1: u32 = 0xEBA0_6BD4;
    pub const STATE_2: u32 = 0x4BB7_1A7E;
}

/// Advisor events (§5.2).
pub mod event {
    pub const BUILDING_ADD: u32 = 0xCA4E_8986;
    pub const FIRE_STARTED: u32 = 0xEA5E_4E89;
    pub const PIPE_BURST: u32 = 0x6A5E_598C;
    pub const TORNADO: u32 = 0x0A5E_8243;
    pub const RIOT: u32 = 0x8AC1_1A9B;
    pub const VOLCANO: u32 = 0x2AC1_4492;
    pub const HOME_DESTROYED: u32 = 0xAA48_F5C1;
    pub const HOME_DESTROYED_BY_DISASTER: u32 = 0xAA4D_2D64;
    pub const JOB_DESTROYED: u32 = 0xCA48_F879;
    pub const JOB_DESTROYED_BY_DISASTER: u32 = 0xAA4D_2D65;
    pub const MOVE_IN: u32 = 0xCA3E_87A8;
    pub const NEW_JOB: u32 = 0xAA3E_DBB1;
    pub const COMMUTE_FAILED: u32 = 0xCA45_53B7;
    pub const CANT_FIND_HOME: u32 = 0x6A57_DD94;
    pub const APPROACHING_LOCATION: u32 = 0xEC10_7DF9;
}

/// Script commands (§4.8).
pub mod command {
    pub const MOVE_OUT: u32 = 0x6A47_E79B;
    pub const RETIRE: u32 = 0xAA49_34B4;
    pub const DIE: u32 = 0xEA4D_269C;
    pub const LEAVE_CITY: u32 = 0x0A57_DE1F;
    pub const SPAWN: u32 = 0x6A49_5F7F;
    pub const INCREASE_WEALTH: u32 = 0x6A4A_5E9E;
    pub const QUIT_JOB: u32 = 0x6A5E_6860;
}

/// Disaster types (§5.2, §4.6).
pub mod disaster {
    pub const FIRE: u32 = 0x621C_DA33;
    pub const RIOT: u32 = 0xCA38_0B06;
    pub const VOLCANO: u32 = 0xC4C9_0997;
}

pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
pub const OCCUPANT_TYPE_FIRE: u32 = 0x69D2_5C5F;
pub const GROUP_LANDMARK: u32 = 0x150A;
/// The landmark budget purpose (§4.6).
pub const PURPOSE_LANDMARK: u32 = 0xAA59_670C;
/// The My Street sim's advisor ID (§1.3).
pub const MY_STREET_ID: i32 = 30;
/// The agent's vector size (§1.3).
pub const SLOTS: usize = 6;
/// The record version (§6.2).
pub const RECORD_VERSION: u16 = 17;
pub const AGENT_RECORD_VERSION: u16 = 1;
/// `DistanceToClosest` without an answer (§4.9).
pub const NO_DISTANCE: u32 = 0xFFFF_FFFF;
/// The local building box (§4.9): home cell ± 20.
pub const LOCAL_BOX: i32 = 20;
/// The residential (wealth) developers of `desirability` (§4.5).
pub const DESIRABILITY_DEVELOPERS: [u32; 3] = [0x1010, 0x1020, 0x1030];
/// The automata trigger of the Die command (§4.8).
pub const DIED_TRIGGER: &str = "my_sim_died";

/// Tuning keys and code defaults (§4.11).
pub const TUNING: [(&str, f64); 10] = [
    ("MYSIM_BALLOON_FADE_DURATION", 300.0),
    ("MYSIM_BALLOON_SHOWN_DURATION", 1500.0),
    ("MYSIM_BALLOON_HIDDEN_DURATION", 300.0),
    ("MYSIM_TRIP_MIN_TIME_AT_LOCATION", 1.0),
    ("MYSIM_TRIP_MIN_TIME_AT_TRANSIT_SWITCH", 1.0),
    ("MYSIM_TRIP_MIN_TIME_IDLE", 1.0),
    ("MYSIM_TRIP_DESTINATION_PROXIMITY_MIN", 10.0),
    ("MYSIM_TRIP_LOCAL_AREA_SIZE", 10.0),
    ("MYSIM_TRIP_APPROACH_DISTANCE", 10.0),
    ("MYSIM_TRIP_DEFAULT_CAR_MODEL", 0.0),
];

/// A building as the MySim reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Building {
    /// `GetBuildingType`.
    pub building_type: u32,
    pub name: Vec<u8>,
    /// The exemplar's occupant groups.
    pub groups: Vec<u32>,
    /// `GetCompletionPercent`.
    pub completion: f32,
    /// The building profile's purpose (1 = residence).
    pub purpose: u32,
    /// `GetPosition` (metres).
    pub position: [f32; 3],
}

/// A lot as the MySim reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lot {
    pub building: Option<u64>,
    /// `GetLocation` (cell).
    pub cell: (i32, i32),
    /// The lot's cell rectangle, inclusive: x0, z0, x1, z1.
    pub rect: (i32, i32, i32, i32),
    pub condition: u8,
    pub road_access: bool,
    pub wealth: u8,
}

/// The grids the MySim reads (§4.5, §4.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Grid {
    Crime,
    /// Pollution grid 0–3 (air, water, garbage, radiation).
    Pollution(u8),
    /// The aura grid (int8).
    Aura,
    AverageEq,
    AverageHq,
    /// The traffic congestion map.
    Congestion,
    /// The desirability grid of developer 0x1010 / 0x1020 / 0x1030.
    Desirability(u32),
}

/// The dispatch (automata) as the trip variables read it (§4.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dispatch {
    /// 2 travelling, 3 at a location.
    pub state: i32,
    pub flags: u32,
    /// The current cell and position.
    pub cell: (i32, i32),
    pub position: [f32; 3],
    /// The travel type (0xFE / 0xFF: none).
    pub travel_type: u32,
    pub wait_minutes: u32,
    /// The occupant at the destination while travelling.
    pub destination: Option<u64>,
    /// The occupant at the current location.
    pub location: Option<u64>,
    /// At the default destination (home) / the commute destination.
    pub at_home: bool,
    pub at_work: bool,
    /// The destination is still "by commute" (arrival, §3.3).
    pub by_commute: bool,
}

/// Where a job search ended (§4.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobDestination {
    /// A city lot's building and the lot's cell.
    Lot { building: u64, cell: (i32, i32) },
    /// A neighbour city at a neighbour cell.
    Neighbor { cell: (i32, i32) },
}

/// A Lua table field value.
#[derive(Clone, Debug)]
pub enum Field {
    Num(f64),
    Str(Vec<u8>),
    /// A building subject (`SetTableBuilding`).
    Building(Option<u64>),
    Subject(Option<Rc<AdviceSubject>>),
}

impl PartialEq for Field {
    fn eq(&self, o: &Field) -> bool {
        match (self, o) {
            (Field::Num(a), Field::Num(b)) => a == b,
            (Field::Str(a), Field::Str(b)) => a == b,
            (Field::Building(a), Field::Building(b)) => a == b,
            (Field::Subject(a), Field::Subject(b)) => a.as_ref().map(|s| s.state()) == b.as_ref().map(|s| s.state()),
            _ => false,
        }
    }
}

/// A posted message (§5.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MySimPost {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub void1: Option<u64>,
    /// The MySim (advisor ID) for messages whose IGZUnknown is a MySim.
    pub sim: Option<i32>,
}

/// Everything a MySim and the agent read from, or do to, the rest of the
/// game. `sim` is the MySim's advisor ID. Callbacks must not call back into
/// the MySims.
pub trait MySimHost {
    // ---- Lua (§4.4) ----
    /// `CreateLuaTable`: true when the table exists afterwards.
    fn create_table(&self, sim: i32) -> bool;
    /// `mysim.<name> = v` in the MySim's table.
    fn set_field(&self, sim: i32, name: &str, v: Field);
    /// `ActivateLuaContext`: the global `mysim` := the table.
    fn activate_lua(&self, sim: i32) -> bool;
    /// `tuning_constants[key]` (§4.11).
    fn tuning(&self, key: &str) -> Option<f64>;
    /// UI service `0xA417445E` slot `+0xA8` (§4.2).
    fn ui_blocked(&self) -> bool;
    /// The agent's Lua functions `get_sim` / `get_count` (§1.3).
    fn register_agent_lua(&self, on: bool);

    // ---- city ----
    fn cell_counts(&self) -> (u32, u32);
    /// `CellCenterToPosition`.
    fn cell_center(&self, cx: i32, cz: i32) -> (f32, f32);
    /// The disaster layer's `IsADisasterOngoing`.
    fn disaster_ongoing(&self) -> bool;
    /// The disaster layer's registered locations: (manager type, cell).
    fn disaster_locations(&self) -> Vec<(u32, (i32, i32))>;

    // ---- buildings and lots ----
    /// A building occupant (`None`: not a building).
    fn building(&self, b: u64) -> Option<Building>;
    /// `GetOccupantLot`.
    fn lot(&self, b: u64) -> Option<Lot>;
    /// Any occupant's `GetPosition`.
    fn occupant_position(&self, o: u64) -> Option<[f32; 3]>;
    /// Buildings in a cell box (inclusive), unfiltered.
    fn buildings_in_box(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> Option<Vec<u64>>;
    /// Every building of the city, unfiltered.
    fn all_buildings(&self) -> Option<Vec<u64>>;
    /// `GetLotsInCellRect(x0, z0, x1, z1, false)`.
    fn lots_in_rect(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> Vec<Lot>;
    /// The building development simulator's special-building list.
    fn special_buildings(&self) -> Vec<u64>;
    /// The residential school (false) / hospital (true) buildings.
    fn education_buildings(&self, hospitals: bool) -> Vec<u64>;
    /// The query data's grade of a school / hospital.
    fn grade(&self, b: u64, hospital: bool) -> Option<f32>;
    /// The job title of a building.
    fn job_title(&self, b: u64) -> Vec<u8>;
    /// The landmark's funding for purpose `0xAA59670C` (§4.6).
    fn landmark_funding(&self, b: u64) -> Option<f32>;

    // ---- residential ----
    /// The average-age grid at a cell.
    fn average_age(&self, x: i32, z: i32) -> Option<i32>;
    fn eq_by_wealth(&self, x: i32, z: i32, wealth: u8) -> Option<f32>;
    fn hq_by_wealth(&self, x: i32, z: i32, wealth: u8) -> Option<f32>;
    fn life_expectancy(&self, x: i32, z: i32) -> Option<u8>;
    fn new_age_by_wealth(&self, wealth: u8) -> Option<u8>;

    // ---- simulators ----
    /// A grid's `GetCellValue`.
    fn grid(&self, g: Grid, x: i32, z: i32) -> Option<i32>;
    /// A grid's `PositionToTract` + `GetTractValue`.
    fn grid_at(&self, g: Grid, x: f32, z: f32) -> Option<i32>;
    /// Police (true) / fire coverage `GetAverageValueInCellRect`.
    fn coverage(&self, police: bool, x0: i32, z0: i32, w: i32, h: i32) -> Option<i32>;
    fn powered(&self, b: u64) -> Option<bool>;
    fn watered(&self, b: u64) -> Option<bool>;
    /// Police (true) / fire stations: (station, funding fraction).
    fn stations(&self, police: bool) -> Option<Vec<(u64, f32)>>;
    /// `GetTravelTimeRatio(cellX, cellZ, travelType)`.
    fn travel_time_ratio(&self, x: i32, z: i32, travel_type: u32) -> Option<f32>;
    /// The simulators of the trip block (traffic, pollution, crime) exist.
    fn trip_simulators(&self) -> bool;

    // ---- the dispatch (automata, §4.7) ----
    fn dispatch(&self, sim: i32) -> Option<Dispatch>;
    /// `FindNewJob`: the dispatch sets a commute destination.
    fn find_new_job(&self, sim: i32) -> bool;
    /// `ChangeJob(b)`.
    fn change_job(&self, sim: i32, b: u64) -> bool;
    /// The destination the dispatch reported (`0xEBA06BF4`).
    fn job_destination(&self, sim: i32) -> Option<JobDestination>;
    /// `CheckCommute`'s own pathfinder (§4.7, home lot rect → job lot rect
    /// or neighbour cell): the total time, `None` on failure. Its slot
    /// configuration is unpublished (question MS15); the host supplies it.
    fn commute_time(&self, sim: i32) -> Option<f32>;
    /// The name of the neighbour city at a cell.
    fn neighbor_city_name(&self, cell: (i32, i32)) -> Option<Vec<u8>>;
    /// Dispatch, UI and audio actions: `create`, `start_commute`,
    /// `shutdown`, `leave_for_work`, `return_home`, `night_sound`,
    /// `update_residence`, `move_in_control`, `ghost`. (`FailCommute` no
    /// longer sends `reset_commute`: it calls [`Self::find_new_job`].)
    fn dispatch_command(&self, sim: i32, what: &str);
    /// `dispatch:CheckFlags(mask)` (§4.1 step 3 tracking flag). Optional:
    /// the default false keeps the pre-dispatch-core stub behaviour.
    fn dispatch_check_flags(&self, _sim: i32, _mask: u32) -> bool {
        false
    }
    /// The inline dispatch record of version-12 MySim records.
    fn read_dispatch_inline(&self, _sim: i32, _inp: &mut dyn InStream) -> bool {
        false
    }

    // ---- messages and effects ----
    fn post(&self, m: &MySimPost);
    /// The automata controller manager's trigger.
    fn automata_trigger(&self, name: &str, position: [f32; 3]);
    /// The agent's subscriptions.
    fn subscribe(&self, msg_type: u32, on: bool);

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    fn object_ref(&self, token: u64) -> RefValue;
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}

/// Registers the MySim classes with the advisor system (the My Street sim
/// too [choice]) and the MySim advice classes (as plain advices).
pub fn register(sys: &Rc<AdvisorSystem>, host: Rc<dyn MySimHost>, session_seed: u32) {
    let counter = Rc::new(Cell::new(0u32));
    for class in [clsid::MYSIM, clsid::MY_STREET_SIM] {
        let host = host.clone();
        let counter = counter.clone();
        sys.register_advisor_class(
            class,
            Rc::new(move |_sys: &Rc<AdvisorSystem>| {
                let n = counter.get();
                counter.set(n + 1);
                MySim::new(host.clone(), class, session_seed, n) as Rc<dyn AdvisorExt>
            }),
        );
    }
    sys.register_advice_class(adv_clsid::MYSIM_ADVICE, AdviceKind::Plain);
    sys.register_advice_class(adv_clsid::MYSIM_BALLOON_ADVICE, AdviceKind::Plain);
}

/// `DistanceToOccupant` (§4.10): `trunc(sqrt(dx² + dz²))` in float32 steps,
/// `_ftol` to an unsigned value.
pub fn distance(home: [f32; 3], x: f32, z: f32) -> u32 {
    use crate::x87::{ftol, r24};
    let dx = r24(f64::from(home[0]) - f64::from(x));
    let dz = r24(f64::from(home[2]) - f64::from(z));
    let s = r24(r24(dx * dx) + r24(dz * dz));
    let d = r24(s.sqrt()).max(0.0);
    ftol(d) as u32
}
