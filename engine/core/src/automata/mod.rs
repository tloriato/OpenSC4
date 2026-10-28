// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The automata layer, short form (`docs/specs/automata.md`). Section
//! references (`§n`) point at that spec.
//!
//! The layer is mostly animation; this core keeps what the simulation and
//! the save file depend on (§0): the records of the six saved components and
//! of every object they reference, the calls other specs make, and the
//! riot, fire and U-Drive-It hooks. Movement, path following, animation and
//! sound are not modelled (stubs).
//!
//! # Structure
//!
//! - `codec.rs`: the encodings of §1, reference identity and the IID check,
//!   the `cSC4Occupant` base block (lot-records §2.3).
//! - `script.rs`: component 40, the script system (§2): templates and
//!   occupant groups, the record.
//! - `controller.rs`: controllers (§3.5): the base record, generator,
//!   attractor, flag 8.
//! - `controller_manager.rs`: component 41 (§3, §7.1).
//! - `automaton.rs`: automata (§6): base, states, traffic automaton,
//!   pedestrian, vehicle, emergency vehicle, patrol car, trailer; the slots
//!   of §7.4.
//! - `manager.rs`: components 43–46 (§4, §5, §7.3).
//! - `hooks.rs`: §8 and §9 as host-facing operations.
//! - `tests.rs`: the §14 tests and more.
//!
//! # Choices (`[choice]`) and literal readings (questions AM1–AM12 in the
//! lane report)
//!
//! - **Versions on re-save.** Each record is written with the version it was
//!   read with, raised to the current version only when it holds data the
//!   older layout cannot carry (a traffic manager's incidents, a base
//!   manager's group counts, the automaton base's last `u32`, a state's
//!   last `u32`, an aircraft or watercraft manager's lists). So every record
//!   of the corpus re-encodes byte-identically (§14 test 4 compares the bytes
//!   of a version-2 record), while a new object writes the current versions.
//!   The original's writers always write the current version (AM1).
//! - **Fields Read ignores or discards** are kept as read and written back:
//!   the controller base version, the attractor's `u16`, the controller
//!   manager's version, the traffic manager's `u32` and cursor, the vehicle's
//!   path entries, its `u32` 0 and its constant vector (§12 item 5).
//! - **Versions the spec gives no read check for** (pedestrian, vehicle,
//!   patrol car, trailer, the incident list's `u16`) are accepted
//!   and kept (AM2).
//!   Emergency and the derived record codecs instead follow the version and
//!   failure contracts in §§6.4.1–6.4.4.
//! - **The occupant base's exemplar parent** is kept by key even when no
//!   exemplar resolves it (`codec::KeepParentKeys`).
//! - **Emergency vehicles** are never written by the managers (§12 quirk 3);
//!   class `0x896E714A` loads as a plain vehicle.
//! - **Reads are atomic** except for the concrete record codecs whose
//!   partial updates and inherited results are specified in §§6.4.1–6.4.4. The manager base
//!   Read reports a short stream (the original does not, §12 item 4) so a
//!   truncated record is detected; the controller manager gives back the
//!   host references it read when it fails or drops a duplicate (§12 item 2).
//! - **Flag 8** on a host controller is asked of the controller manager's
//!   host; automata and states written outside a manager treat a host
//!   controller as saved.
//! - **`RemoveControllersFromOccupantGroup`** removes the occupant's
//!   controllers whose template is one of the group's (AM7).
//! - **`SetState`** makes the state stack one new state of that type; its
//!   six arguments are kept, not interpreted (Q5).
//! - **`CreateAutomaton`** is a stub (pedestrian or vehicle record at the
//!   position, listed, null at the hard limit).
//!
//! # Threading
//!
//! Main-thread only (foundations §19.3): objects use `RefCell` / `Cell`.

mod automaton;
mod codec;
mod controller;
mod controller_manager;
mod derived;
mod hooks;
mod manager;
mod script;
pub mod archive;

pub use derived::{
    AmbulanceRecord, DerivedRecordRefs, FireTruckRecord, MassTransitTrainRecord, TankRecord, VehicleWrapperRecord,
    CLSID_AMBULANCE, CLSID_CARJACKED, CLSID_FIRE_TRUCK, CLSID_LIMO, CLSID_MASS_TRANSIT_TRAIN, CLSID_TANK,
};

mod aircraft;
pub use crate::helicopter::Helicopter;
pub use aircraft::{
    aircraft_altitude, airplane_effect_names, clip_aircraft_to_world, AircraftBounds, AircraftGoalOutcome,
    AircraftPlacementRecord, AircraftRecord, AircraftTargets, AircraftUnsupported, AircraftWorld, Airplane,
    AirplaneDriveParameters, ChaseTarget, DriveParameterSource, FlightPhase, FlightPhaseChange,
    HelicopterChaseGeometry, CLSID_AIRCRAFT, CLSID_AIRPLANE, CLSID_HELICOPTER,
};
pub use aircraft::{initialize_fire_plane_station_units, AirStationProperties};

pub(crate) mod aircraft_actions;
pub use aircraft_actions::{
    AircraftWeaponEffects, EngineNotification, PlayerWeaponRecord, SoundRequest, WeaponOutcome,
};

pub mod airplane;

#[cfg(test)]
mod tests;

pub use automaton::{
    state_type, Automaton, AutomatonBase, AutomatonRecord, EmergencyRecord, PatrolCarRecord, PedestrianRecord,
    SetStateArgs, State, StateBase, StateExtra, TrafficRecord, TrailerRecord, VehicleRecord, AUTOMATON_BASE_VERSION,
    AUTOMATON_OCCUPANT_TYPE, EMERGENCY_VERSION, PATROL_CAR_VERSION, PEDESTRIAN_VERSION, STATE_BASE_VERSION,
    TRAFFIC_VERSION, TRAILER_VERSION, VEHICLE_CONSTANT, VEHICLE_VERSION,
};
pub use codec::{
    get_ref_checked, ref_answers, ref_id, refs_equal, same_ref, ControllerTime, KeepParentKeys, OccupantBase, RefId,
    Vec3, OCCUPANT_BASE_VERSION, OCCUPANT_FLAG_VISIBLE,
};
pub use controller::{
    ref_is_transient, saved_controller_ref, Controller, ControllerBase, ControllerKind, ControllerRecord,
    ATTRACTOR_TAG, CONTROLLER_BASE_VERSION, DEFAULT_FLAGS, FLAG_TRANSIENT, GENERATOR_VERSION,
};
pub use controller_manager::{
    missing_template_message, AutomatonSet, ControllerManager, ControllerManagerHost, ControllerManagerRecord,
    NoControllerHost, OccupantEntry, CONTROLLER_MANAGER_VERSION,
};
pub use hooks::{
    pause_all, resume_all, riot_leader_template, FireHook, FireHost, RiotHost, RiotLeader, RiotLeaders,
    FIRE_OCCUPANT_GROUP, RIOT_CREATE_FLAGS, RIOT_CREATE_PRIORITY, RIOT_END_STATE_ARGS, RIOT_LEADER_STATE_ARGS,
    RIOT_STAGES,
};
pub use manager::{
    AutomataManager, CreateArgs, FerryEntry, Incident, ManagerBase, ManagerKind, ManagerPart, ManagerRecord,
    RouteElement, TrafficPart, AIRCRAFT_MANAGER_VERSION, INCIDENT_LIST_TAG, MANAGER_BASE_VERSION, MAX_COUNT_LIMIT,
    TRAFFIC_MANAGER_VERSION, WATERCRAFT_MANAGER_VERSION,
};
pub use script::{ControllerTemplate, OccupantGroup, ScriptSource, ScriptSystem, TemplateKind, SCRIPT_SYSTEM_VERSION};

// ---- class IDs (§0, §3.5, §6.4) -----------------------------------------------------------------------------

/// Component 40, the automata script system.
pub const CLSID_SCRIPT_SYSTEM: u32 = 0x4A1B_664E;
/// Component 41, the automata controller manager.
pub const CLSID_CONTROLLER_MANAGER: u32 = 0xCA14_CA03;
/// Component 43, the vehicle manager.
pub const CLSID_VEHICLE_MANAGER: u32 = 0xA98F_9D3B;
/// Component 44, the pedestrian manager.
pub const CLSID_PEDESTRIAN_MANAGER: u32 = 0xC98F_9D17;
/// Component 45, the aircraft manager.
pub const CLSID_AIRCRAFT_MANAGER: u32 = 0xC98F_9D4A;
/// Component 46, the watercraft manager.
pub const CLSID_WATERCRAFT_MANAGER: u32 = 0xA98F_9D28;
/// The generator controller.
pub const CLSID_GENERATOR: u32 = 0x4A16_28C5;
/// The attractor controller.
pub const CLSID_ATTRACTOR: u32 = 0x6A16_28BA;
/// The pedestrian.
pub const CLSID_PEDESTRIAN: u32 = 0x896E_75AF;
/// The vehicle (and the emergency vehicle, §12 quirk 3).
pub const CLSID_VEHICLE: u32 = 0x896E_714A;
/// The patrol car.
pub const CLSID_PATROL_CAR: u32 = 0x4A22_2B98;
/// The trailer vehicle.
pub const CLSID_TRAILER: u32 = 0x4A5B_69ED;
/// The 24-hour clock the controller manager references (§3.1).
pub const CLSID_24_HOUR_CLOCK: u32 = 0xA7E7_F929;

/// Every class whose record this module reads and writes.
pub const RECORD_CLASSES: [u32; 12] = [
    CLSID_SCRIPT_SYSTEM,
    CLSID_CONTROLLER_MANAGER,
    CLSID_VEHICLE_MANAGER,
    CLSID_PEDESTRIAN_MANAGER,
    CLSID_AIRCRAFT_MANAGER,
    CLSID_WATERCRAFT_MANAGER,
    CLSID_GENERATOR,
    CLSID_ATTRACTOR,
    CLSID_PEDESTRIAN,
    CLSID_VEHICLE,
    CLSID_PATROL_CAR,
    CLSID_TRAILER,
];

// ---- interface IDs (§0, §1) ---------------------------------------------------------------------------------

pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const IID_SCRIPT_SYSTEM: u32 = 0x8A1B_6647;
pub const IID_CONTROLLER_MANAGER: u32 = 0xCA14_C9FC;
pub const IID_AUTOMATA_MANAGER: u32 = 0x098F_9BF4;
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
pub const IID_AUTOMATON: u32 = 0xA9B4_0F05;
pub const IID_AUTOMATA_CONTROLLER: u32 = 0x6A18_57F8;
pub const IID_LOT: u32 = 0xE818_FCED;
pub const IID_NETWORK_OCCUPANT: u32 = 0xA821_EF94;
pub const IID_24_HOUR_CLOCK: u32 = 0x681B_D53A;
pub const IID_MODEL: u32 = 0xA818_EDD8;
/// The callback target interface (§6.1 field 13).
pub const IID_MESSAGE_TARGET: u32 = 0x4522_94AA;

// ---- other numbers ------------------------------------------------------------------------------------------

/// City slots (§0 table).
pub const CITY_SLOT_SCRIPT_SYSTEM: u32 = 48;
pub const CITY_SLOT_CONTROLLER_MANAGER: u32 = 47;
pub const CITY_SLOT_VEHICLE_MANAGER: u32 = 43;
pub const CITY_SLOT_PEDESTRIAN_MANAGER: u32 = 44;
pub const CITY_SLOT_AIRCRAFT_MANAGER: u32 = 45;
pub const CITY_SLOT_WATERCRAFT_MANAGER: u32 = 46;
/// The disaster layer (§8.1).
pub const CITY_SLOT_DISASTER_LAYER: u32 = 72;

/// The U-Drive-It view (§7.3 slot 13).
pub const U_DRIVE_IT_VIEW: u32 = 0x2B70_9676;
/// The riot leaders' automata group (§7.2, §8.1).
pub const RIOT_AUTOMATA_GROUP: u32 = 0x4107;
/// A building's occupant type (§8.2).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;

/// The messages of §10.
pub mod msg {
    pub const VIEW_UPDATE: u32 = 0xA6B7_9602;
    pub const ANIMATION_TICK: u32 = 0x86AD_10EE;
    pub const AUTOMATON_NOTIFICATION: u32 = 0x09D9_2FC3;
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const PRE_CITY_SHUTDOWN: u32 = 0x26D3_1EC2;
    pub const HOUR_CHANGED: u32 = 0x4477_8230;
    pub const INSERT_OCCUPANT: u32 = 0x99EF_1142;
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    pub const CHEAT: u32 = 0x230E_27AC;
    pub const CHEAT_DEBUG_TOGGLE: u32 = 0xABA2_B5BB;
    pub const EMERGENCY_PAUSE: u32 = 0x4A7F_B807;
    pub const BUILDING_COMPLETION_CHANGED: u32 = 0x2977_AE2F;
    pub const CROWD_REACTION: u32 = 0xA9E3_B6E2;
    pub const FUNDS_CHANGED: u32 = 0x0772_FAD4;
    pub const DAY_NIGHT_CHANGED: u32 = 0x4477_8234;
    pub const EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;
    pub const DAWN: u32 = 0xC9DA_96E8;
    pub const DUSK: u32 = 0xC9DA_96E9;
    /// The names of §13 Q6 (answered).
    pub const NETWORK_OCCUPANT_USABLE_CHANGE: u32 = 0x89D3_E9F3;
    pub const PRE_VIEW_FILTER_CHANGE: u32 = 0x06CD_B65A;
    pub const MOUSE_SCROLL_BEGIN: u32 = 0xCAD6_1E00;
    pub const MOUSE_SCROLL_END: u32 = 0xEAD6_1E16;
    pub const REMOVE_AUTOMATA_CONTROLLER: u32 = 0xEA29_8D50;
    pub const OCCUPANT_GROUP_CHANGED: u32 = 0xEBFF_18A3;
    pub const VIEW_T_CHANGE_2: u32 = 0xA6B7_9611;
    pub const FERRY_PASS_FINISHED: u32 = 0x2C06_BA82;

    /// The controller manager's messages (§10 row 1).
    pub const CONTROLLER_MANAGER: [u32; 8] = [
        VIEW_UPDATE,
        ANIMATION_TICK,
        AUTOMATON_NOTIFICATION,
        POST_CITY_INIT,
        HOUR_CHANGED,
        NETWORK_OCCUPANT_USABLE_CHANGE,
        INSERT_OCCUPANT,
        REMOVE_OCCUPANT,
    ];
    /// Every manager's (the base, §10 row 3).
    pub const MANAGER_BASE: [u32; 16] = [
        REMOVE_OCCUPANT,
        EMERGENCY_PAUSE,
        PRE_CITY_SHUTDOWN,
        PRE_VIEW_FILTER_CHANGE,
        POST_CITY_INIT,
        BUILDING_COMPLETION_CHANGED,
        ANIMATION_TICK,
        NETWORK_OCCUPANT_USABLE_CHANGE,
        INSERT_OCCUPANT,
        MOUSE_SCROLL_BEGIN,
        VIEW_UPDATE,
        0xA6B7_9612,
        0xC9F7_75BB,
        REMOVE_AUTOMATA_CONTROLLER,
        MOUSE_SCROLL_END,
        OCCUPANT_GROUP_CHANGED,
    ];
    /// The pedestrian manager's own additions (§10 row 4).
    pub const PEDESTRIAN_EXTRA: [u32; 2] = [CHEAT, CROWD_REACTION];
    /// The vehicle manager's own additions (§10 row 5, corrected by §13 Q6:
    /// dawn `0xC9DA96E8`, not `0xC9DA96E7`).
    pub const VEHICLE_EXTRA: [u32; 7] = [
        VIEW_T_CHANGE_2,
        FUNDS_CHANGED,
        DAY_NIGHT_CHANGED,
        0xA6B7_9601,
        EXEMPLAR_CHANGED,
        DAWN,
        DUSK,
    ];
    /// The watercraft manager's own addition (§10 row 7).
    pub const WATERCRAFT_EXTRA: [u32; 1] = [FERRY_PASS_FINISHED];
}
