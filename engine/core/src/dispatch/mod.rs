// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The dispatch manager (`cSC4DispatchManager`, class `0x4990BD93`),
//! `docs/specs/dispatch.md`. Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - `manager.rs`: the state of §2, `Init` / `Shutdown` (§4.1),
//!   `Dispatch` (§4.2), `UpdateDispatchState` (§4.3), ending and revoking
//!   (§4.4), the dispatch tool slots (§4.5), the queries (§4.6), the
//!   messages (§3) and the record of §7.
//! - `tests.rs`: a fake city and the §12 tests.
//!
//! Everything outside the class goes through [`DispatchHost`]: the message
//! server, the city, the vehicle view, the dispatch sources (protection.md
//! §8.2), the vehicle automata, the pylon occupants, the terrain and save
//! references. The pylon layout along a station's lot and the vehicle
//! hand-off are presentation and automata work: the host does them
//! (`pylon_place_at_unit`, `pylon_return_to_station`, `begin_vehicle`).
//!
//! # References
//!
//! The core holds one reference on every record's station, automaton and
//! pylon, on the active pylon, on every available pylon and on every
//! registered source (the original's `cRZAutoRefCount`s and held
//! pointers). Tokens the host returns from queries (`pylon_source`, the
//! nearest station) are borrowed; the core AddRefs what it keeps. A created
//! pylon (`create_pylon`) and a created vehicle (`begin_vehicle` without an
//! automaton) come with one reference for the core.
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **"Clear the available pylons"** (§4.1, §4.5) shuts every pylon down
//!   and releases it (DP4).
//! - **"Clear the active pylon"** (§3, §4.4) is `SetActivePylon(null)`: the
//!   old pylon gets UI state 1 and is released (DP5). `Shutdown` shuts the
//!   active pylon down and releases it with no UI state; `EndDispatchTool`
//!   leaves it alone.
//! - **Returning a pylon without a source occupant** (§4.4) shuts it down
//!   and does not release it: `EndDispatch` releases the record's reference
//!   right after. The original releases twice (an over-release); the single
//!   release is the spec's *[choice]* (DP6).
//! - **A station removed** (§3, DP7): each of its records' vehicles is
//!   untracked, despawned and released; its pylon cleared from active, shut
//!   down and released; then its available pylons are shut down, released,
//!   cleared from active and erased.
//! - **`RevokeDispatchByPylon`** always calls `ReturnPylonToStation` with
//!   the source of the pylon's dispatch type, which does nothing when there
//!   is no such source (DP8).
//! - **`BeginDispatch`** (§4.2 step 5, DP9) tracks only a new vehicle (kind
//!   0 in occupant group `0x4202` or `0x4303`, else kind 1); a re-targeted
//!   vehicle keeps the record's handle and is not tracked again.
//! - **`Shutdown`** runs only when initialised and clears `initialized`
//!   [choice]; `Release` at a count of 1 or less runs `Shutdown` and the
//!   destructor, which releases the records' references and the sources.
//! - **`Read`** (§7) sets `loaded` first; on a version other than 2 it fails;
//!   otherwise it is atomic [choice]: on a stream error nothing is appended
//!   and the references read so far are released. The active pylon is
//!   overwritten without releasing the old one.
//! - **Threading** (foundations §19.3): the state is a `RefCell`; the core is
//!   main-thread only. No borrow is held across a host call; host callbacks
//!   must not call back into the manager.

pub mod archive;
mod manager;

#[cfg(test)]
mod tests;

pub use manager::{query_interface, DispatchManager, DispatchState};

use crate::stream::RefValue;

/// Class ID and interface ID (§1).
pub const CLSID_DISPATCH_MANAGER: u32 = 0x4990_BD93;
pub const IID_DISPATCH_MANAGER: u32 = 0x6940_28EC;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// Save references (§7).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
pub const IID_AUTOMATON: u32 = 0xA9B4_0F05;
pub const IID_PYLON: u32 = 0xC9B6_D7D6;

/// City component and getter slot (§1).
pub const CITY_COMPONENT: u32 = 3;
pub const CITY_GETTER_SLOT: u32 = 39;

/// Dispatch types (§2, *[inferred]*).
pub mod dispatch_type {
    pub const POLICE_CAR: u32 = 0;
    pub const FIRE_ENGINE: u32 = 1;
    pub const FIRE_PLANE: u32 = 5;
    pub const POLICE_HELICOPTER: u32 = 6;
    /// "No tool" (`toolType`).
    pub const NONE: u32 = 8;
}

/// Dispatch states (§4.3, *[inferred names]*).
pub mod state {
    pub const IDLE: u32 = 0;
    pub const DISPATCHED: u32 = 3;
    pub const WORKING: u32 = 5;
}

/// Pylon UI states (§4.4, §4.5).
pub const UI_STATE_IDLE: u32 = 1;
pub const UI_STATE_ACTIVE: u32 = 2;

/// Vehicle-view kinds (§4.1): types 0 and 6 as kind 1, types 1 and 5 as
/// kind 0, other types untracked.
/// The occupant groups whose new vehicles `BeginDispatch` tracks as kind 0
/// (§4.2 step 5); any other new vehicle is kind 1.
pub const TRACK_KIND0_GROUPS: [u32; 2] = [0x4202, 0x4303];

pub fn vehicle_view_kind(t: u32) -> Option<u32> {
    match t {
        dispatch_type::POLICE_CAR | dispatch_type::POLICE_HELICOPTER => Some(1),
        dispatch_type::FIRE_ENGINE | dispatch_type::FIRE_PLANE => Some(0),
        _ => None,
    }
}

/// Messages (§3).
pub mod msg {
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    pub const DISASTER_PERIOD_ENDED: u32 = 0xAA1B_2002;
    pub const VIEW_CHANGED: u32 = 0xA6B7_9602;
    /// The subscriptions of `Init`, in order.
    pub const SUBSCRIPTIONS: [u32; 3] = [REMOVE_OCCUPANT, DISASTER_PERIOD_ENDED, VIEW_CHANGED];
    /// A vehicle automaton's event (delivered directly).
    pub const AUTOMATON_EVENT: u32 = 0x09D9_2FC3;
    /// Event codes of `0x09D92FC3` (data2).
    pub const EVENT_GONE: u32 = 3;
    pub const EVENT_STATE: u32 = 7;
    /// Posted: data1 = dispatch type, data2 = dispatch ID.
    pub const BEGIN: u32 = 0xE946_71EB;
    pub const ACTIVE: u32 = 0xE946_71EC;
    pub const INACTIVE: u32 = 0xE946_71ED;
    pub const END: u32 = 0x8946_71F6;
}

/// The record version (§7).
pub const RECORD_VERSION: u16 = 2;

/// `DispatchInfo` (docs/abi/layouts.md, 0x2C bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DispatchInfo {
    /// `+0x00` (saved as `s32`).
    pub dispatch_type: u32,
    /// `+0x04`.
    pub id: u32,
    /// `+0x08`: the station's unit number (only the low 16 bits are used).
    pub call_id: u32,
    /// `+0x0C`: the vehicle-view handle (0 = not tracked; not saved).
    pub view_handle: u32,
    /// `+0x10`: set while the dispatch is in state 5.
    pub active: bool,
    /// `+0x14` … `+0x1C`: the target (y = the terrain altitude).
    pub target: (f32, f32, f32),
    /// `+0x20`: the station occupant (held).
    pub station: Option<u64>,
    /// `+0x24`: the vehicle automaton (held).
    pub automaton: Option<u64>,
    /// `+0x28`: the pylon (held).
    pub pylon: Option<u64>,
}

/// One station of a source (`GetStations`) with its unit state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StationUnits {
    pub station: u64,
    /// Bit `i` set: unit `i` is free.
    pub mask: u32,
    /// The call number of unit 0.
    pub base: u32,
    /// The number of units.
    pub count: u32,
}

impl StationUnits {
    /// The free units below `count`.
    pub fn free_units(&self) -> u32 {
        let below = if self.count >= 32 {
            u32::MAX
        } else {
            (1u32 << self.count) - 1
        };
        (self.mask & below).count_ones()
    }
}

/// A message as `DoMessage` sees it (§3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DispatchMessage {
    pub msg_type: u32,
    /// `0x09D92FC3`: the event code.
    pub data2: u32,
    /// `0x09D92FC3` code 7: the new dispatch state.
    pub data3: u32,
    /// `0x99EF1143`: the removed occupant; `0x09D92FC3`: the automaton.
    pub object: Option<u64>,
}

/// Everything the dispatch manager reads from, or does to, the rest of the
/// game. Re-entrancy: callbacks must not call back into the manager.
pub trait DispatchHost {
    // ---- messages (§3, §4.1) ----
    /// Hold the message server; false = none.
    fn acquire_message_server(&self) -> bool;
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    fn release_message_server(&self);
    /// `MessagePost` (queued, priority 0): IGZUnknown = the manager, data1 =
    /// the dispatch type, data2 = the dispatch ID.
    fn post(&self, msg_type: u32, dispatch_type: u32, id: u32);

    // ---- services (§4.1) ----
    fn acquire_city(&self) -> bool;
    fn release_city(&self);
    /// Creates the vehicle view and registers it with the renderer (layer 5,
    /// priority 1000); the core holds the returned token.
    fn create_vehicle_view(&self) -> Option<u64>;
    /// Unregisters and releases the vehicle view.
    fn destroy_vehicle_view(&self, view: u64);
    /// The vehicle view refreshes (`0xA6B79602`).
    fn refresh_vehicle_view(&self, view: Option<u64>);
    /// Tracks the vehicle in the vehicle view as `kind`: the handle.
    fn track_vehicle(&self, automaton: u64, kind: u32) -> u32;
    fn untrack_vehicle(&self, handle: u32);

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- the terrain ----
    /// The terrain altitude at (x, z).
    fn altitude(&self, x: f32, z: f32) -> f32;

    // ---- dispatch sources (protection.md §8.2) ----
    fn source_can_handle(&self, source: u64, dispatch_type: u32) -> bool;
    /// `CanDispatch(x, z, &allBusy)`: (result, allBusy).
    fn source_can_dispatch(&self, source: u64, x: f32, z: f32) -> (bool, bool);
    /// `DispatchFromNearestStation(x, z, &callID)`: the station (borrowed)
    /// and the call ID, or `None`.
    fn source_dispatch_from_nearest_station(&self, source: u64, x: f32, z: f32) -> Option<(u64, u32)>;
    /// `DispatchFromStation(station, &callID)`.
    fn source_dispatch_from_station(&self, source: u64, station: Option<u64>, call_id: &mut u32) -> bool;
    /// `RevokeDispatchFromStation(station, callID)`: the unit returns.
    fn source_revoke_dispatch_from_station(&self, source: u64, station: Option<u64>, call_id: u32);
    /// `GetStations` with each station's unit state.
    fn source_stations(&self, source: u64) -> Vec<StationUnits>;

    // ---- vehicles ----
    /// `BeginDispatch`'s automata work (§4.2 step 5): find the automata
    /// manager of the source's vehicle group, create the vehicle (or
    /// re-target `info.automaton` when set), set its dispatch ID, call ID
    /// and state 3, and make the manager its event target. Returns the
    /// vehicle (a created one with one reference for the core), or `None`.
    fn begin_vehicle(&self, source: u64, info: &DispatchInfo) -> Option<u64>;
    /// Automaton slot 13 with 0 (despawn).
    fn automaton_despawn(&self, automaton: u64);
    /// Automaton slot 29 (go home).
    fn automaton_go_home(&self, automaton: u64);

    // ---- pylons ----
    /// A new pylon occupant, initialised (one reference for the core).
    fn create_pylon(&self) -> Option<u64>;
    /// The pylon's source occupant (borrowed).
    fn pylon_source(&self, pylon: u64) -> Option<u64>;
    fn pylon_set_source(&self, pylon: u64, station: u64);
    fn pylon_call_number(&self, pylon: u64) -> u32;
    fn pylon_set_call_number(&self, pylon: u64, n: u32);
    fn pylon_dispatch_type(&self, pylon: u64) -> u32;
    fn pylon_set_dispatch_type(&self, pylon: u64, t: u32);
    fn pylon_set_dispatch_state(&self, pylon: u64, s: u32);
    fn pylon_set_ui_state(&self, pylon: u64, s: u32);
    /// Moves the pylon to the target.
    fn pylon_move_to(&self, pylon: u64, x: f32, y: f32, z: f32);
    /// Places the pylon of unit `index` of `count` along the station's lot
    /// diagonal (§4.5; presentation).
    fn pylon_place_at_unit(&self, pylon: u64, station: u64, index: u32, count: u32);
    /// `ReturnPylonToStation`'s placement (§4.4): `source` slot 7 for the
    /// station's unit state; when it succeeds, the pylon is placed back at
    /// its parking spot next to the station (index `callNumber − base + 1`)
    /// and the call returns true (the core then sets dispatch state 0 and UI
    /// state 1); false = the unit state was not available, nothing placed.
    fn pylon_return_to_station(&self, source: u64, pylon: u64, station: u64) -> bool;
    fn pylon_shutdown(&self, pylon: u64);

    // ---- save references (§7) ----
    fn object_ref(&self, token: u64) -> RefValue;
    /// Resolves a reference read from a record (one reference for the core).
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;

    // ---- vehicles, continued ----
    /// The vehicle's occupant is in occupant group `group` (§4.2 step 5: a
    /// new vehicle is tracked as kind 0 in group `0x4202` or `0x4303`).
    fn vehicle_in_group(&self, automaton: u64, group: u32) -> bool;
}
