// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The prop developer (`cSC4PropDeveloper`, class `0x89C48F47`) and its
//! wealth requester (`cSC4WealthRequester`, class `0x89C49986`),
//! `docs/specs/prop-developer.md`. Section references (`§n`) point at that
//! spec.
//!
//! The developer fills and empties the optional prop slots of one 8×8-cell
//! block per step (§4.2–§4.4) on behalf of its requesters (§4.1), keeps the
//! lists of props it drives (night lights §4.8, time of day §4.9, dates,
//! power §4.13, global triggers §4.10, signs §4.11) and handles the lot
//! state props (§4.12). The record (§7) holds the scanner, the requesters
//! and every list.
//!
//! Everything outside the class goes through [`PropHost`]: lots, network
//! lots, the prop manager, prop occupants, the requesters other than the
//! wealth requester (crime and pollution), the power grid, the clock, the
//! lighting manager, the 3D view and the terrain. Objects are host tokens;
//! the lists hold [`PropRef`]s (token and class, for the record).
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - The member generator is a [`RzRandomSuper`] seeded with
//!   `derive_seed(S, 0x89C48F47) × 28` (§5); the wealth requester's file
//!   generator is shared by the session ([`wealth_generator`]); the night
//!   shuffle uses a [`CrtRand`] owned by the developer, seeded with 1
//!   (foundations §12.6). Every generator is injectable ([`PropRandom`]).
//! - A requester index `≥ R` is treated as no index (§4.3 step 2.3.6).
//! - The null-lot creation path returns false (§4.4 step 5). A creation
//!   whose occupant is null returns false as the original does (§4.4 step
//!   6, PV8).
//! - `total = 0` (impossible with the wealth requester) ends the block
//!   update after the cell walk instead of dividing by zero (the original
//!   faults, PV10).
//! - `Shutdown` clears the power list (§3.2).
//! - The time-of-day walk stops at the end of the list (§4.9).
//! - A night batch decides on/off with the mode it started with: the batch
//!   that ends a dusk sweep still switches its props on (PV7).
//! - A timed prop whose global trigger is non-zero while the 3D view is
//!   missing leaves mask 4 unchanged (PV9).
//! - Unregistering searches each list only when the prop passes that
//!   list's test (§4.7, PV12).
//! - The manager references are written back as read until `Init`
//!   replaces them, and are never used.
//! - `Read` is atomic: on any failure nothing changes. A scanner state
//!   byte other than 0 and 2 is running (PV13): a byte above 2 is kept
//!   ([`PropState::scan_byte`]) and written back unchanged until the scan
//!   finishes or resets.
//! - Temporary references (the kept lists of §4.3) are not mirrored to the
//!   host; the held lists are (`hold` / `release`).
//! - Threading (foundations §19.3): main-thread only; no state is borrowed
//!   across a host call.

#[cfg(test)]
mod tests;

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::grid::{CellRect, ScanIterator, ScanState};
use crate::lot_developer::CfgObject;
use crate::property::PropertyHolder;
use crate::rng::{derive_seed, file_seed, CrtRand, FixedSeeds, RzRandomSuper};
use crate::stl::StlHashMap;
use crate::stream::{InStream, OutStream, RefValue, SaveObject};
use crate::variant::Elem;

/// Class and interface IDs (§1).
pub const CLSID_PROP_DEVELOPER: u32 = 0x89C4_8F47;
pub const IID_PROP_DEVELOPER: u32 = 0xE9A1_F4B6;
pub const CLSID_WEALTH_REQUESTER: u32 = 0x89C4_9986;
pub const IID_PROP_REQUESTER: u32 = 0x89C4_98BE;
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_MESSAGE_TARGET: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// Interfaces of the record's references (§7).
pub const IID_LOT_MANAGER: u32 = 0x6819_05BD;
pub const IID_PROP_MANAGER: u32 = 0x2832_6F3E;
pub const IID_NETWORK_LOT_MANAGER: u32 = 0x89A5_BA3F;
pub const IID_PROP_OCCUPANT: u32 = 0xE979_3A65;
/// City component (§1).
pub const CITY_COMPONENT: u32 = 10;

/// The agent (§3.1): levels 10 (Idle) and 4 (Day), flags 2.
pub const AGENT_NAME: &str = "cSC4PropDeveloper";
pub const LEVEL_IDLE: u32 = 10;
pub const LEVEL_DAY: u32 = 4;
pub const AGENT_FLAGS: u32 = 2;
/// The animation-tick priority (§3.1).
pub const ANIMATION_PRIORITY: u32 = 0xC0;

/// Messages (§3.3).
pub const MSG_IDLE: u32 = 0x6695_681A;
pub const MSG_DAY: u32 = 0x6695_6814;
pub const MSG_ANIMATION_TICK: u32 = 0x86AD_10EE;
pub const MSG_DUSK: u32 = 0xC9DA_96E9;
pub const MSG_DAWN: u32 = 0xC9DA_96E8;
pub const MSG_DAY_NIGHT_CHANGED: u32 = 0x4477_8234;
pub const MSG_POST_CITY_INIT: u32 = 0x26D3_1EC1;
pub const MSG_TIME_TICK: u32 = 0x4477_8231;
pub const MSG_OCCUPANT_INSERTED: u32 = 0x99EF_1142;
pub const MSG_OCCUPANT_REMOVED: u32 = 0x99EF_1143;
pub const MSG_EFFECT_TRIGGER: u32 = 0xAA37_5FE8;
pub const MSG_LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;
pub const MSG_LOT_STATE_PROPS: u32 = 0xCA5E_5027;
pub const MSG_POWER_CHANGED: u32 = 0x8A07_AE00;
pub const MSG_TERRAIN_CHANGED: u32 = 0x278E_6003;
pub const MSG_WATER_CHANGED: u32 = 0x678E_6E3A;
/// The subscriptions of §3.1 step 5, in order.
pub const SUBSCRIPTIONS: [u32; 13] = [
    MSG_OCCUPANT_INSERTED,
    MSG_OCCUPANT_REMOVED,
    MSG_DAWN,
    MSG_DUSK,
    MSG_TIME_TICK,
    MSG_DAY_NIGHT_CHANGED,
    MSG_POST_CITY_INIT,
    MSG_LOT_STATE_CHANGED,
    MSG_POWER_CHANGED,
    MSG_EFFECT_TRIGGER,
    MSG_LOT_STATE_PROPS,
    MSG_TERRAIN_CHANGED,
    MSG_WATER_CHANGED,
];

/// Occupant types (§3.3).
pub const OCCUPANT_TYPE_PROP: u32 = 0xA823_821E;
pub const OCCUPANT_TYPE_NETWORK: u32 = 0xC772_BF98;
/// Prop types (§4.11).
pub const PROP_TYPE_SIGNPOST: u32 = 0xAB72_FBB3;
pub const PROP_TYPE_STREET_SIGN: u32 = 0xCB79_919B;

/// Properties (§6).
pub const PROP_REQUESTER_INDEX: u32 = 0x89A1_C16C;
pub const PROP_PREVIEW: u32 = 0x0292_44DB;
pub const PROP_NIGHT_LIT: u32 = 0x49C9_C93C;
pub const PROP_TRIGGER: u32 = 0x6A3F_CC37;
pub const PROP_TIME_OF_DAY: u32 = 0x4A14_9631;
pub const PROP_DATE: u32 = 0x0A75_1675;
pub const PROP_DATE_DURATION: u32 = 0x4A76_4564;
pub const PROP_DATE_FLAG: u32 = 0xCA75_15CC;
pub const PROP_NEEDS_POWER: u32 = 0x6A95_9E71;
pub const PROP_CHANCE: u32 = 0x4A75_1AD5;
pub const PROP_LOT_STATE: u32 = 0x6A5E_0727;

/// Timer bits (§4.6).
pub const TIMER_TIME_OF_DAY: u32 = 1;
pub const TIMER_DATE: u32 = 2;
pub const TIMER_TRIGGER: u32 = 4;
pub const TIMER_POWER: u32 = 8;
/// Occupant flag 2: a visible sign (§4.11).
pub const FLAG_SIGN_VISIBLE: u32 = 2;

/// The wealth requester (§4.1).
pub const WEALTH_SHARE: i32 = 10;
pub const WEALTH_SATISFACTION: u32 = 0x49A1_E995;

/// Block size, night batches, thresholds (§6).
pub const BLOCK: i32 = 8;
pub const NIGHT_BATCHES: u32 = 8;
pub const NIGHT_INTERVAL_MS: u32 = 200;
pub const TIME_OF_DAY_THRESHOLD: f32 = 0.4;
pub const TIME_OF_DAY_INITIAL: f32 = -100.0;
pub const SIGNPOST_TOLERANCE: f32 = 0.01;

/// Record versions (§7).
pub const RECORD_VERSION: u16 = 5;
pub const MIN_RECORD_VERSION: u16 = 4;

/// `QueryInterface` of the developer (§1): the sub-object offset.
pub fn query_interface(iid: u32) -> Option<u32> {
    match iid {
        IID_UNKNOWN | IID_PROP_DEVELOPER => Some(0),
        IID_MESSAGE_TARGET => Some(4),
        IID_SERIALIZABLE => Some(8),
        _ => None,
    }
}

/// `QueryInterface` of the wealth requester (§1).
pub fn wealth_query_interface(iid: u32) -> Option<u32> {
    match iid {
        IID_UNKNOWN | IID_PROP_REQUESTER => Some(0),
        IID_SERIALIZABLE => Some(4),
        _ => None,
    }
}

/// The member generator's seed (§5): `derive_seed(S, 0x89C48F47) × 28`.
pub fn member_seed(session: u32) -> u32 {
    derive_seed(session, CLSID_PROP_DEVELOPER).wrapping_mul(28)
}

fn super_random(seed: u32) -> RzRandomSuper {
    RzRandomSuper::new(
        seed,
        &FixedSeeds {
            wall_clock: seed,
            process_clock: seed,
        },
    )
}

/// The member generator of a session (§5).
pub fn member_generator(session: u32) -> RzRandomSuper {
    super_random(member_seed(session))
}

/// The wealth requester's file generator of a session (§5, K `0x4C09C51F`).
pub fn wealth_generator(session: u32) -> RzRandomSuper {
    super_random(derive_seed(session, file_seed::WEALTH_REQUESTER))
}

/// The draws of §5.
pub trait PropRandom {
    /// `RandomUint32Uniform(n)`.
    fn uniform_u32(&mut self, n: u32) -> u32;
    /// `RandomSint32RangeUniform(lo, hi)`.
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32;
}

impl PropRandom for RzRandomSuper {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        RzRandomSuper::uniform_u32(self, n)
    }
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        RzRandomSuper::range_i32(self, lo, hi)
    }
}

/// The wealth requester's file generator, shared by every wealth requester
/// of the session.
pub type SharedRandom = Rc<RefCell<dyn PropRandom>>;

/// A held host object: its token (0 = null) and class (for the record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropRef {
    pub token: u64,
    pub class_id: u32,
}

impl PropRef {
    pub fn new(token: u64, class_id: u32) -> PropRef {
        PropRef { token, class_id }
    }
    pub fn is_null(&self) -> bool {
        self.token == 0
    }
    fn to_ref(self) -> RefValue {
        if self.token == 0 {
            RefValue::Null
        } else {
            RefValue::Host {
                token: self.token,
                class_id: self.class_id,
            }
        }
    }
    fn from_ref(r: &RefValue) -> PropRef {
        match r {
            RefValue::Null => PropRef::default(),
            RefValue::Host { token, class_id } => PropRef::new(*token, *class_id),
            RefValue::Object(o) => PropRef::new(o.host_token(), o.class_id()),
        }
    }
}

/// The wealth requester (§1, §4.1): no state; its record transfers nothing.
#[derive(Debug)]
pub struct WealthRequester {
    token: Cell<u64>,
}

impl WealthRequester {
    pub fn new(token: u64) -> Rc<WealthRequester> {
        Rc::new(WealthRequester {
            token: Cell::new(token),
        })
    }
    pub fn token(&self) -> u64 {
        self.token.get()
    }
}

impl SaveObject for WealthRequester {
    fn class_id(&self) -> u32 {
        CLSID_WEALTH_REQUESTER
    }
    fn write(&self, _out: &mut dyn OutStream) -> bool {
        true
    }
    fn read(&self, _inp: &mut dyn InStream) -> bool {
        true
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
    fn host_token(&self) -> u64 {
        self.token.get()
    }
}

/// One requester (§4.1).
#[derive(Clone, Debug)]
pub enum Requester {
    /// A null reference from a record [choice]: share 0, no selector.
    Null,
    Wealth(Rc<WealthRequester>),
    /// A requester implemented by the host (crime, pollution).
    Host(PropRef),
}

impl Requester {
    fn to_ref(&self) -> RefValue {
        match self {
            Requester::Null => RefValue::Null,
            Requester::Wealth(w) => RefValue::Object(w.clone()),
            Requester::Host(r) => r.to_ref(),
        }
    }
    fn from_ref(r: &RefValue) -> Requester {
        if let Some(w) = r.object::<WealthRequester>() {
            return Requester::Wealth(w);
        }
        match r {
            RefValue::Null => Requester::Null,
            RefValue::Host { token, class_id } if *class_id == CLSID_WEALTH_REQUESTER => {
                Requester::Wealth(WealthRequester::new(*token))
            }
            other => Requester::Host(PropRef::from_ref(other)),
        }
    }
    /// The host token (the wealth requester's own token; 0 for null).
    pub fn token(&self) -> u64 {
        match self {
            Requester::Null => 0,
            Requester::Wealth(w) => w.token(),
            Requester::Host(r) => r.token,
        }
    }
    pub fn class_id(&self) -> u32 {
        match self {
            Requester::Null => 0,
            Requester::Wealth(_) => CLSID_WEALTH_REQUESTER,
            Requester::Host(r) => r.class_id,
        }
    }
}

/// Everything the prop developer reads from, or does to, the rest of the
/// game. Objects are host tokens. Re-entrancy: callbacks must not call back
/// into the developer (messages the host posts are delivered later).
pub trait PropHost {
    // ---- city, simulator, messages ----
    /// The city size in cells (`cISC4City` slots 93, 94).
    fn city_cells(&self) -> (i32, i32);
    /// The cell widths in metres (`cISC4City` slots 91, 92).
    fn cell_width(&self) -> (f32, f32);
    /// The lot manager, prop manager and network lot manager (held at
    /// `Init`), for the record.
    fn managers(&self) -> [PropRef; 3];
    /// Creates the host object of a new wealth requester; its token.
    fn create_wealth_requester(&self) -> u64;
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// Removes the agent from every level.
    fn remove_agents(&self, name: &str);
    fn subscribe(&self, msg: u32);
    fn unsubscribe_all(&self);
    fn register_animation_tick(&self, priority: u32);
    fn unregister_animation_tick(&self);
    /// Posts `0xCA5E5027` (data1 x, data2 z, data3 `entered_3`).
    fn post_lot_state_props(&self, x: i32, z: i32, entered_3: bool);
    /// AddRef / Release of a held object (a requester or a prop).
    fn hold(&self, token: u64);
    fn release(&self, token: u64);

    // ---- host requesters (§4.1) ----
    fn requester_share(&self, requester: u64, rect: CellRect) -> i32;
    fn requester_satisfaction(&self, requester: u64) -> u32;
    fn requester_custom(&self, requester: u64) -> bool;
    fn requester_select(&self, requester: u64, ids: &[u32], x: i32, z: i32) -> u32;

    // ---- lots and network lots ----
    /// `lotManager.GetLot(x, z, originOnly)`.
    fn lot_at(&self, x: i32, z: i32, origin_only: bool) -> Option<u64>;
    fn lot_livable(&self, lot: u64) -> bool;
    fn lot_building_type(&self, lot: u64) -> u32;
    fn lot_occupancy(&self, lot: u64) -> f32;
    /// The lot configuration's objects; `None` without a configuration.
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>>;
    fn lot_wealth(&self, lot: u64) -> u32;
    fn lot_location(&self, lot: u64) -> (i32, i32);
    fn lot_size(&self, lot: u64) -> (i32, i32);
    fn lot_state(&self, lot: u64) -> u32;
    /// Zone manager slot 12.
    fn zone_type(&self, x: i32, z: i32) -> u32;
    fn network_lot_at(&self, x: i32, z: i32) -> Option<u64>;
    /// The network lot's configuration objects (its slot 5).
    fn network_lot_objects(&self, network_lot: u64) -> Rc<Vec<CfgObject>>;
    fn network_wealth(&self, x: i32, z: i32) -> u32;

    // ---- prop manager ----
    fn lot_props(&self, lot: u64) -> Vec<u64>;
    fn street_props(&self, x: i32, z: i32) -> Vec<u64>;
    fn street_props_in_rect(&self, rect: CellRect) -> Vec<u64>;
    /// `RemovePropA(prop)`.
    fn remove_prop(&self, prop: u64);
    fn prop_type_valid(&self, id: u32) -> bool;
    fn props_by_family(&self, family: u32) -> Vec<u32>;
    /// The prop profile's requester satisfaction (`0x49A1E05A`, default 0).
    fn prop_satisfaction(&self, id: u32) -> u32;
    /// The prop profile's wealth byte (`0xE9A316EB`, default 0).
    fn prop_wealth(&self, id: u32) -> u8;
    /// Lot developer slot 63 `CreateLotProp(lot, index, id)`.
    fn create_lot_prop(&self, lot: u64, index: usize, id: u32) -> Option<u64>;
    /// Network lot manager slot 8 `CreateProp(networkLot, index, id)`.
    fn create_network_prop(&self, network_lot: u64, index: usize, id: u32) -> Option<u64>;

    // ---- prop occupants ----
    /// The occupant's class (for the record).
    fn prop_class(&self, prop: u64) -> u32;
    /// The occupant's properties (exemplar and own), `None` if none.
    fn prop_properties(&self, prop: u64) -> Option<Rc<PropertyHolder>>;
    fn prop_lot_tag(&self, prop: u64) -> Option<u32>;
    /// Sets the u32 property `0x89A1C16C`.
    fn set_prop_requester(&self, prop: u64, requester: u32);
    fn prop_type(&self, prop: u64) -> u32;
    fn prop_state(&self, prop: u64) -> u32;
    fn set_prop_state(&self, prop: u64, state: u32);
    /// `SetPropTimerState(mask, on)`.
    fn set_prop_timer(&self, prop: u64, mask: u32, on: bool);
    fn set_random_chance(&self, prop: u64, chance: u8);
    fn set_time_of_day_range(&self, prop: u64, start: f32, end: f32);
    fn time_of_day_range(&self, prop: u64) -> (f32, f32);
    /// `SetSimulatorDateRange(date, duration, flag, 0)`.
    fn set_date_range(&self, prop: u64, date: u32, duration: u32, flag: u8);
    fn tick_date(&self, prop: u64, date: u32);
    fn prop_position(&self, prop: u64) -> [f32; 3];
    /// `SetPosition` and the occupant manager's notification (flag 1).
    fn set_prop_position(&self, prop: u64, position: [f32; 3]);
    /// `SetFlag` / `ClearFlag`.
    fn set_prop_flag(&self, prop: u64, flag: u32, on: bool);
    /// The first cell of a network occupant's bounding cells.
    fn network_first_cell(&self, occupant: u64) -> (i32, i32);
    /// `cISC4City::PositionToCell` (slot 95).
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32);

    // ---- others ----
    /// Power simulator slot 37 `IsPowered(x, z)`.
    fn is_powered(&self, x: i32, z: i32) -> bool;
    /// The 3D view is available.
    fn view_available(&self) -> bool;
    /// The 3D view's state of a global effect trigger (view slot 46).
    fn view_trigger_state(&self, trigger: u32) -> bool;
    fn sim_date(&self) -> u32;
    /// The 24-hour clock's (hour, minute); `None` without the clock.
    fn clock_time(&self) -> Option<(u32, u32)>;
    /// The 24-hour clock's visual time mode (0 normal, 1 day, 2 night).
    fn visual_time_mode(&self) -> u32;
    /// Lighting manager slot 9.
    fn lighting_is_night(&self) -> bool;
    /// (terrain altitude, water level) at a position.
    fn terrain_heights(&self, x: f32, z: f32) -> (f32, f32);
}

/// A message for [`PropDeveloper::do_message`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropMessage {
    pub msg_type: u32,
    /// The message's object (an occupant or a lot).
    pub object: u64,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
}

/// The fields of §2 (not the lists).
#[derive(Clone, Debug, PartialEq)]
pub struct PropState {
    pub initialized: bool,
    pub scan_flag: i8,
    pub scanner: ScanIterator,
    /// The scanner's state byte while it is running (§7, PV13): 1, or a
    /// byte above 2 read from a record, which `GetNext` never rewrites and
    /// which is written back as read until the scan finishes or resets.
    pub scan_byte: u8,
    pub wealth_added: bool,
    pub night_cursor: u32,
    pub night_mode: u32,
    pub night_batch: u32,
    pub night_acc: u32,
    pub is_night: bool,
    pub tod_threshold: f32,
    pub tod_last: f32,
    pub signs_visible: bool,
    pub ref_count: u32,
    pub destroyed: bool,
}

impl Default for PropState {
    fn default() -> PropState {
        PropState {
            initialized: false,
            scan_flag: 0,
            scanner: ScanIterator::default(),
            scan_byte: 1,
            wealth_added: false,
            night_cursor: 0,
            night_mode: 0,
            // [choice] 0 (the original leaves it uninitialised, §2).
            night_batch: 0,
            night_acc: 0,
            is_night: false,
            tod_threshold: TIME_OF_DAY_THRESHOLD,
            tod_last: TIME_OF_DAY_INITIAL,
            signs_visible: true,
            ref_count: 0,
            destroyed: false,
        }
    }
}

/// What the last block update did (diagnostics).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockOutcome {
    pub block: (i32, i32),
    pub lots: u32,
    pub occ_sum: f32,
    pub slots: i32,
    pub kept_all: i32,
    /// The target N (§4.3 step 4); 0 when stopped at step 3.
    pub target: u32,
    pub want: Vec<u32>,
    pub kept: Vec<u32>,
    pub removed: u32,
    pub created: u32,
    pub member_draws: u32,
    /// Stopped at step 3 (no lots).
    pub stopped: bool,
}

/// A slot of the free list or a kept prop (§4.3).
#[derive(Clone)]
struct Slot {
    x: i32,
    z: i32,
    objs: Rc<Vec<CfgObject>>,
    index: usize,
}

/// The prop developer.
pub struct PropDeveloper {
    host: Rc<dyn PropHost>,
    token: Cell<u64>,
    s: RefCell<PropState>,
    member: RefCell<Box<dyn PropRandom>>,
    wealth_rng: SharedRandom,
    crt: Cell<CrtRand>,
    requesters: RefCell<Vec<Requester>>,
    managers: Cell<[PropRef; 3]>,
    night: RefCell<Vec<PropRef>>,
    triggers: RefCell<StlHashMap<Vec<PropRef>>>,
    dated: RefCell<Vec<PropRef>>,
    tod: RefCell<Vec<PropRef>>,
    power: RefCell<Vec<PropRef>>,
    signs: RefCell<Vec<PropRef>>,
    last: RefCell<BlockOutcome>,
    member_draws: Cell<u32>,
}

fn trunc_i32(v: f32) -> i32 {
    crate::x87::ftol32(f64::from(v))
}

fn u8_prop(h: &PropertyHolder, id: u32) -> Option<u8> {
    h.get(id).and_then(|v| v.items().first().map(|e| e.as_i64() as u8))
}

fn u32_prop(h: &PropertyHolder, id: u32) -> Option<u32> {
    h.get(id).and_then(|v| v.items().first().map(|e| e.as_i64() as u32))
}

impl PropDeveloper {
    /// A constructed developer. `member` is the member generator (§5),
    /// `wealth_rng` the session's wealth file generator.
    pub fn new(
        host: Rc<dyn PropHost>,
        member: Box<dyn PropRandom>,
        wealth_rng: SharedRandom,
        token: u64,
    ) -> Rc<PropDeveloper> {
        Rc::new(PropDeveloper {
            host,
            token: Cell::new(token),
            s: RefCell::new(PropState::default()),
            member: RefCell::new(member),
            wealth_rng,
            crt: Cell::new(CrtRand::default()),
            requesters: RefCell::new(Vec::new()),
            managers: Cell::new([PropRef::default(); 3]),
            night: RefCell::new(Vec::new()),
            triggers: RefCell::new(StlHashMap::with_hint(100)),
            dated: RefCell::new(Vec::new()),
            tod: RefCell::new(Vec::new()),
            power: RefCell::new(Vec::new()),
            signs: RefCell::new(Vec::new()),
            last: RefCell::new(BlockOutcome::default()),
            member_draws: Cell::new(0),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }
    pub fn set_token(&self, t: u64) {
        self.token.set(t);
    }
    pub fn snapshot(&self) -> PropState {
        self.s.borrow().clone()
    }
    pub fn last_block(&self) -> BlockOutcome {
        self.last.borrow().clone()
    }
    pub fn requesters(&self) -> Vec<Requester> {
        self.requesters.borrow().clone()
    }
    pub fn night_list(&self) -> Vec<PropRef> {
        self.night.borrow().clone()
    }
    pub fn dated_list(&self) -> Vec<PropRef> {
        self.dated.borrow().clone()
    }
    pub fn time_of_day_list(&self) -> Vec<PropRef> {
        self.tod.borrow().clone()
    }
    pub fn power_list(&self) -> Vec<PropRef> {
        self.power.borrow().clone()
    }
    pub fn sign_list(&self) -> Vec<PropRef> {
        self.signs.borrow().clone()
    }
    /// The trigger map in iteration order.
    pub fn trigger_map(&self) -> Vec<(u32, Vec<PropRef>)> {
        self.triggers.borrow().iter().map(|(k, v)| (k, v.clone())).collect()
    }
    /// Replaces the member generator (tests, replays).
    pub fn set_member_random(&self, r: Box<dyn PropRandom>) {
        *self.member.borrow_mut() = r;
    }
    /// The C `rand()` state of the night shuffle.
    pub fn crt(&self) -> CrtRand {
        self.crt.get()
    }
    pub fn set_crt(&self, c: CrtRand) {
        self.crt.set(c);
    }
    /// Edits the state (tests and tools).
    pub fn edit_state(&self, f: impl FnOnce(&mut PropState)) {
        f(&mut self.s.borrow_mut());
    }

    fn edit(&self, f: impl FnOnce(&mut PropState)) {
        f(&mut self.s.borrow_mut());
    }

    fn draw_member_range(&self, lo: i32, hi: i32) -> i32 {
        self.member_draws.set(self.member_draws.get() + 1);
        self.member.borrow_mut().range_i32(lo, hi)
    }

    fn draw_member_uniform(&self, n: u32) -> u32 {
        self.member_draws.set(self.member_draws.get() + 1);
        self.member.borrow_mut().uniform_u32(n)
    }

    // ---- identity ----

    pub fn add_ref(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        s.ref_count = s.ref_count.wrapping_add(1);
        s.ref_count
    }

    /// The last release destroys the object (here: marks it) and returns 0.
    pub fn release(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        if s.ref_count > 1 {
            s.ref_count -= 1;
            return s.ref_count;
        }
        s.ref_count = 0;
        s.destroyed = true;
        0
    }

    // ---- Init / Shutdown (§3.1, §3.2) ----

    /// Slot 3 `Init`. Returns true.
    pub fn init(&self) -> bool {
        let h = &*self.host;
        // 1.
        self.managers.set(h.managers());
        // 2.
        if !self.s.borrow().wealth_added {
            let token = h.create_wealth_requester();
            self.add_prop_requester(Requester::Wealth(WealthRequester::new(token)));
            self.edit(|s| s.wealth_added = true);
        }
        // 3.
        if self.s.borrow().scan_flag == 0 {
            let (cx, cz) = h.city_cells();
            self.edit(|s| {
                s.scan_flag = 1;
                let mut it = ScanIterator::new(0, 0, (cx / BLOCK - 1).max(0), (cz / BLOCK - 1).max(0));
                it.step = s.scanner.step;
                it.reset(0, 0);
                s.scanner = it;
                s.scan_byte = 1;
            });
        }
        // 4.–6.
        h.add_agent(LEVEL_IDLE, AGENT_NAME, AGENT_FLAGS);
        h.add_agent(LEVEL_DAY, AGENT_NAME, AGENT_FLAGS);
        for m in SUBSCRIPTIONS {
            h.subscribe(m);
        }
        h.register_animation_tick(ANIMATION_PRIORITY);
        self.edit(|s| s.initialized = true);
        true
    }

    fn release_all(&self, list: Vec<PropRef>) {
        for p in list {
            if !p.is_null() {
                self.host.release(p.token);
            }
        }
    }

    /// Slot 4 `Shutdown`. Returns true.
    pub fn shutdown(&self) -> bool {
        let h = &*self.host;
        let map = std::mem::replace(&mut *self.triggers.borrow_mut(), StlHashMap::with_hint(100));
        for (_, v) in map.iter() {
            self.release_all(v.clone());
        }
        let tod = std::mem::take(&mut *self.tod.borrow_mut());
        self.release_all(tod);
        let dated = std::mem::take(&mut *self.dated.borrow_mut());
        self.release_all(dated);
        // [choice] cleared (the original keeps dangling entries).
        let power = std::mem::take(&mut *self.power.borrow_mut());
        self.release_all(power);
        let night = std::mem::take(&mut *self.night.borrow_mut());
        self.release_all(night);
        if self.s.borrow().initialized {
            h.remove_agents(AGENT_NAME);
            h.unsubscribe_all();
            h.unregister_animation_tick();
        }
        self.managers.set([PropRef::default(); 3]);
        self.edit(|s| {
            s.scan_flag = 0;
            s.initialized = false;
        });
        if self.s.borrow().wealth_added {
            let reqs = std::mem::take(&mut *self.requesters.borrow_mut());
            for r in reqs {
                if let Requester::Host(p) = r {
                    if !p.is_null() {
                        h.release(p.token);
                    }
                }
            }
            self.edit(|s| s.wealth_added = false);
        }
        true
    }

    // ---- requesters (§4.1) ----

    /// Slot 5 `AddPropRequester(r)`: appended, no duplicate check.
    pub fn add_prop_requester(&self, r: Requester) {
        if let Requester::Host(p) = &r {
            if !p.is_null() {
                self.host.hold(p.token);
            }
        }
        self.requesters.borrow_mut().push(r);
    }

    /// Slot 6 `RemovePropRequester(r)`: the first occurrence (by token).
    pub fn remove_prop_requester(&self, token: u64) -> bool {
        let pos = self.requesters.borrow().iter().position(|r| r.token() == token);
        let Some(i) = pos else {
            return false;
        };
        let r = self.requesters.borrow_mut().remove(i);
        if let Requester::Host(p) = r {
            if !p.is_null() {
                self.host.release(p.token);
            }
        }
        true
    }

    fn requester(&self, r: usize) -> Requester {
        self.requesters.borrow().get(r).cloned().unwrap_or(Requester::Null)
    }

    fn share(&self, r: usize, rect: CellRect) -> i32 {
        match self.requester(r) {
            Requester::Null => 0,
            Requester::Wealth(_) => WEALTH_SHARE,
            Requester::Host(p) => self.host.requester_share(p.token, rect),
        }
    }

    fn satisfaction(&self, r: usize) -> u32 {
        match self.requester(r) {
            Requester::Null => 0,
            Requester::Wealth(_) => WEALTH_SATISFACTION,
            Requester::Host(p) => self.host.requester_satisfaction(p.token),
        }
    }

    fn custom(&self, r: usize) -> bool {
        match self.requester(r) {
            Requester::Null => false,
            Requester::Wealth(_) => true,
            Requester::Host(p) => self.host.requester_custom(p.token),
        }
    }

    fn select(&self, r: usize, ids: &[u32], x: i32, z: i32) -> u32 {
        match self.requester(r) {
            Requester::Null => 0,
            Requester::Wealth(_) => self.wealth_select(ids, x, z),
            Requester::Host(p) => self.host.requester_select(p.token, ids, x, z),
        }
    }

    /// The wealth requester's `SelectProp(ids, x, z)` (§4.1).
    pub fn wealth_select(&self, ids: &[u32], x: i32, z: i32) -> u32 {
        let h = &*self.host;
        let w = match h.lot_at(x, z, false) {
            Some(lot) => {
                let w = h.lot_wealth(lot);
                if matches!(h.zone_type(x, z), 11 | 12 | 15) {
                    2
                } else {
                    w
                }
            }
            None => h.network_wealth(x, z),
        };
        let kept: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|&id| u32::from(h.prop_wealth(id)) == w)
            .collect();
        if kept.is_empty() {
            return 0;
        }
        let k = self.wealth_rng.borrow_mut().uniform_u32(kept.len() as u32) as usize;
        kept.get(k).copied().unwrap_or(0)
    }

    // ---- the block step (§4.2) ----

    /// One block step. Returns the block updated, if any.
    pub fn block_step(&self) -> Option<(i32, i32)> {
        let flag = self.s.borrow().scan_flag;
        match flag {
            1 => {
                self.edit(|s| s.scan_flag = 2);
                None
            }
            2 => {
                let next = {
                    let mut s = self.s.borrow_mut();
                    let was_running = s.scanner.state == ScanState::Running;
                    let mut reset = false;
                    let next = match s.scanner.next_cell() {
                        Some(b) => Some(b),
                        None => {
                            reset = true;
                            s.scanner.reset(0, 0);
                            s.scanner.next_cell()
                        }
                    };
                    // A kept byte above 2 lasts only while the scan runs on
                    // without finishing or resetting (PV13).
                    if reset || !was_running || s.scanner.state != ScanState::Running {
                        s.scan_byte = 1;
                    }
                    next
                };
                if let Some((bx, bz)) = next {
                    self.update_block(bx, bz);
                }
                next
            }
            _ => None,
        }
    }

    /// Slot 7 `UpdateAllOptionalProps`: every block of the clip, x outer.
    pub fn update_all_optional_props(&self) {
        let it = self.s.borrow().scanner.clone();
        for bx in it.x0..=it.x1 {
            for bz in it.z0..=it.z1 {
                self.update_block(bx, bz);
            }
        }
    }

    /// The block update (`UpdateTract`, §4.3).
    pub fn update_block(&self, bx: i32, bz: i32) {
        let h = &*self.host;
        self.member_draws.set(0);
        let n_req = self.requesters.borrow().len();
        let (x0, z0) = (bx.wrapping_mul(BLOCK), bz.wrapping_mul(BLOCK));
        let rect = CellRect::new(x0, z0, x0 + BLOCK - 1, z0 + BLOCK - 1);
        let mut out = BlockOutcome {
            block: (bx, bz),
            ..BlockOutcome::default()
        };
        // Step 1.
        let mut share: Vec<i32> = (0..n_req).map(|r| self.share(r, rect)).collect();
        let total: u32 = share.iter().fold(0u32, |a, &s| a.wrapping_add(s as u32));
        let custom: Vec<bool> = (0..n_req).map(|r| self.custom(r)).collect();
        // Step 2.
        let mut lots = 0u32;
        let mut occ_sum = 0f32;
        let mut slots = 0i32;
        let mut kept_all = 0i32;
        let mut kept = vec![0u32; n_req];
        let mut kept_lists: Vec<Vec<(Slot, u64)>> = vec![Vec::new(); n_req];
        let mut free: Vec<Slot> = Vec::new();
        for x in x0..x0 + BLOCK {
            for z in z0..z0 + BLOCK {
                let (objs, props) = match h.lot_at(x, z, true) {
                    Some(lot) => {
                        if !h.lot_livable(lot) || h.lot_building_type(lot) == 0 {
                            continue;
                        }
                        lots += 1;
                        occ_sum += h.lot_occupancy(lot);
                        let Some(objs) = h.lot_config_objects(lot) else {
                            continue;
                        };
                        (objs, h.lot_props(lot))
                    }
                    None => {
                        let Some(nl) = h.network_lot_at(x, z) else {
                            continue;
                        };
                        (h.network_lot_objects(nl), h.street_props(x, z))
                    }
                };
                let mut open: Vec<bool> = objs.iter().map(|o| o.object_type == 1 && o.flags & 1 != 0).collect();
                let mut opt = open.iter().filter(|&&b| b).count() as i32;
                let mut k = 0i32;
                for prop in props {
                    let Some(tag) = h.prop_lot_tag(prop) else {
                        continue;
                    };
                    let Some(i) = objs.iter().position(|o| o.tag == tag) else {
                        // [quirk]
                        k -= 1;
                        continue;
                    };
                    if !open[i] {
                        continue;
                    }
                    open[i] = false;
                    opt -= 1;
                    let r = h.prop_properties(prop).and_then(|p| u32_prop(&p, PROP_REQUESTER_INDEX));
                    // [choice] r ≥ R is no index.
                    let Some(r) = r.filter(|&r| (r as usize) < n_req) else {
                        continue;
                    };
                    let r = r as usize;
                    let keep = if custom[r] {
                        let t = h.prop_type(prop);
                        self.select(r, &[t], x, z) == t
                    } else {
                        true
                    };
                    if keep {
                        kept_lists[r].push((
                            Slot {
                                x,
                                z,
                                objs: objs.clone(),
                                index: i,
                            },
                            prop,
                        ));
                        kept[r] += 1;
                        k += 1;
                    } else {
                        h.remove_prop(prop);
                        open[i] = true;
                        opt += 1;
                    }
                }
                for (i, _) in open.iter().enumerate().filter(|(_, &o)| o) {
                    free.push(Slot {
                        x,
                        z,
                        objs: objs.clone(),
                        index: i,
                    });
                }
                slots = slots.wrapping_add(opt).wrapping_add(k);
                kept_all = kept_all.wrapping_add(k);
            }
        }
        out.lots = lots;
        out.occ_sum = occ_sum;
        out.slots = slots;
        out.kept_all = kept_all;
        out.kept = kept.clone();
        // Step 3.
        if lots == 0 || total == 0 {
            out.stopped = true;
            out.want = vec![0; n_req];
            out.member_draws = self.member_draws.get();
            *self.last.borrow_mut() = out;
            return;
        }
        // Step 4.
        let avg = occ_sum / lots as f32;
        let n = (avg * slots as f32).floor() as i32 as u32;
        out.target = n;
        let q = n / total;
        let m = n % total;
        let mut want: Vec<u32> = share.iter().map(|&s| (s as u32).wrapping_mul(q)).collect();
        let mut idx = 0usize;
        for _ in 0..m {
            loop {
                idx = if idx + 1 == n_req { 0 } else { idx + 1 };
                if share[idx] != 0 {
                    break;
                }
            }
            share[idx] -= 1;
            want[idx] = want[idx].wrapping_add(1);
        }
        out.want = want.clone();
        // Step 5.
        for r in 0..n_req {
            if kept[r] > want[r] {
                let j = self.draw_member_range(0, kept[r] as i32);
                let list = &mut kept_lists[r];
                if j >= 0 && (j as usize) < list.len() {
                    let (slot, prop) = list.remove(j as usize);
                    h.remove_prop(prop);
                    free.push(slot);
                    slots = slots.wrapping_add(1);
                    out.removed += 1;
                }
            }
        }
        // Step 6.
        let mut f = slots.wrapping_sub(kept_all);
        for r in 0..n_req {
            if want[r] <= kept[r] {
                continue;
            }
            let mut need = want[r] - kept[r];
            while need != 0 && f > 0 {
                let j = self.draw_member_range(0, f);
                if j >= 0 && (j as usize) < free.len() {
                    let slot = free.remove(j as usize);
                    if self.select_and_create(&slot, r) {
                        need -= 1;
                        out.created += 1;
                    }
                }
                f -= 1;
            }
        }
        out.slots = slots;
        out.member_draws = self.member_draws.get();
        *self.last.borrow_mut() = out;
    }

    /// Select and create (§4.4).
    fn select_and_create(&self, slot: &Slot, r: usize) -> bool {
        let h = &*self.host;
        let Some(obj) = slot.objs.get(slot.index) else {
            return false;
        };
        if obj.ids.is_empty() {
            return false;
        }
        let sat = self.satisfaction(r);
        let mut cands = Vec::new();
        for &id in &obj.ids {
            if h.prop_type_valid(id) {
                if h.prop_satisfaction(id) == sat {
                    cands.push(id);
                }
            } else {
                cands.extend(
                    h.props_by_family(id)
                        .into_iter()
                        .filter(|&m| h.prop_satisfaction(m) == sat),
                );
            }
        }
        if cands.is_empty() {
            return false;
        }
        let pick = if self.custom(r) {
            self.select(r, &cands, slot.x, slot.z)
        } else {
            let k = self.draw_member_uniform(cands.len() as u32) as usize;
            cands.get(k).copied().unwrap_or(0)
        };
        if pick == 0 {
            return false;
        }
        let occ = match h.lot_at(slot.x, slot.z, false) {
            Some(lot) => h.create_lot_prop(lot, slot.index, pick),
            None => match h.network_lot_at(slot.x, slot.z) {
                Some(nl) => h.create_network_prop(nl, slot.index, pick),
                // [choice] (the original's path is unreachable).
                None => None,
            },
        };
        // Step 6: the original tests the created occupant (`0x006F18C6`,
        // PV8): null → false, the free entry used up, `need` unchanged.
        let Some(occ) = occ else {
            return false;
        };
        h.set_prop_requester(occ, r as u32);
        true
    }

    // ---- messages (§3.3) ----

    /// `DoMessage`: false only for an animation tick in night mode 0.
    pub fn do_message(&self, m: &PropMessage) -> bool {
        match m.msg_type {
            MSG_IDLE => {
                self.block_step();
            }
            MSG_DAY => {
                self.block_step();
                let date = self.host.sim_date();
                for p in self.dated_list() {
                    if !p.is_null() {
                        self.host.tick_date(p.token, date);
                    }
                }
            }
            MSG_ANIMATION_TICK => {
                if self.s.borrow().night_mode == 0 {
                    return false;
                }
                self.night_batch(m.data2 / 1000);
            }
            MSG_DUSK => self.start_sweep(1, true),
            MSG_DAWN => self.start_sweep(2, false),
            MSG_DAY_NIGHT_CHANGED => {
                if self.host.clock_time().is_some() {
                    let mode = self.host.visual_time_mode();
                    if m.data2 == 0 && mode == 2 {
                        self.switch_all(true);
                    } else if m.data2 == 1 && mode == 1 {
                        self.switch_all(false);
                    }
                }
            }
            MSG_POST_CITY_INIT => {
                let night = self.host.lighting_is_night();
                self.edit(|s| s.is_night = night);
                self.switch_all(night);
            }
            MSG_TIME_TICK => self.time_tick(),
            MSG_OCCUPANT_INSERTED => {
                if m.data3 == OCCUPANT_TYPE_PROP {
                    self.register_prop(m.object);
                }
            }
            MSG_OCCUPANT_REMOVED => {
                if m.data3 == OCCUPANT_TYPE_PROP {
                    self.unregister_prop(m.object);
                } else if m.data3 == OCCUPANT_TYPE_NETWORK {
                    self.remove_street_signs(m.object);
                }
            }
            MSG_EFFECT_TRIGGER => self.effect_trigger(m.data1, m.data2 != 0),
            MSG_LOT_STATE_CHANGED => self.lot_state_changed(m.object, m.data1, m.data2),
            MSG_LOT_STATE_PROPS => self.lot_state_props(m.data1 as i32, m.data2 as i32),
            MSG_POWER_CHANGED => self.power_changed(CellRect::new(
                m.data1 as i32,
                m.data2 as i32,
                m.data3 as i32,
                m.data4 as i32,
            )),
            MSG_TERRAIN_CHANGED | MSG_WATER_CHANGED => self.reseat_signposts(),
            _ => {}
        }
        true
    }

    // ---- night lights (§4.8) ----

    /// The cell of a position for the night batch.
    fn batch_cell(&self, pos: [f32; 3]) -> (i32, i32) {
        let (wx, wz) = self.host.cell_width();
        let (tx, tz) = (trunc_i32(wx) as f32, trunc_i32(wz) as f32);
        (trunc_i32(pos[0] / tx), trunc_i32(pos[2] / tz))
    }

    fn start_sweep(&self, mode: u32, night: bool) {
        let mut list = std::mem::take(&mut *self.night.borrow_mut());
        let mut crt = self.crt.get();
        crt.random_shuffle(&mut list);
        self.crt.set(crt);
        let n = list.len() as u32;
        *self.night.borrow_mut() = list;
        self.edit(|s| {
            s.night_mode = mode;
            s.is_night = night;
            s.night_batch = n.div_ceil(NIGHT_BATCHES);
            s.night_cursor = 0;
        });
    }

    /// One batch (§4.8) after `ms` milliseconds.
    pub fn night_batch(&self, ms: u32) {
        let (mode, cursor, end) = {
            let mut s = self.s.borrow_mut();
            s.night_acc = s.night_acc.wrapping_add(ms);
            if s.night_acc <= NIGHT_INTERVAL_MS {
                return;
            }
            s.night_acc = 0;
            let n = self.night.borrow().len() as u32;
            let mode = s.night_mode;
            let mut end = s.night_cursor.wrapping_add(s.night_batch);
            if (end as i32) > (n as i32) {
                end = n;
                s.night_mode = 0;
            }
            (mode, s.night_cursor, end)
        };
        let mut c = cursor;
        while c < end {
            let p = self.night.borrow().get(c as usize).copied();
            if let Some(p) = p.filter(|p| !p.is_null()) {
                let on = if mode == 1 {
                    let (x, z) = self.batch_cell(self.host.prop_position(p.token));
                    u32::from(self.host.is_powered(x, z))
                } else {
                    0
                };
                self.host.set_prop_state(p.token, on);
            }
            c += 1;
        }
        self.edit(|s| s.night_cursor = c.max(s.night_cursor));
    }

    /// Switch all (§4.8).
    pub fn switch_all(&self, on: bool) {
        self.night.borrow_mut().retain(|p| !p.is_null());
        let (wx, wz) = self.host.cell_width();
        for p in self.night_list() {
            let pos = self.host.prop_position(p.token);
            let x = trunc_i32(pos[0] * (1.0 / wx));
            let z = trunc_i32(pos[2] * (1.0 / wz));
            let state = on && self.host.is_powered(x, z);
            self.host.set_prop_state(p.token, u32::from(state));
        }
        self.edit(|s| {
            s.night_cursor = 0;
            s.night_mode = 0;
        });
    }

    // ---- time of day (§4.9) ----

    fn time_tick(&self) {
        let Some((hour, minute)) = self.host.clock_time() else {
            return;
        };
        let t = hour as f32 + minute as f32 * (1.0f32 / 60.0);
        let (last, threshold) = {
            let s = self.s.borrow();
            (s.tod_last, s.tod_threshold)
        };
        // Also stops on NaN.
        let d = (last - t).abs();
        if d.is_nan() || d < threshold {
            return;
        }
        self.edit(|s| s.tod_last = t);
        let mut i = 0;
        loop {
            let p = self.tod.borrow().get(i).copied();
            let Some(p) = p else {
                break;
            };
            if p.is_null() {
                self.tod.borrow_mut().swap_remove(i);
                continue;
            }
            let (start, end) = self.host.time_of_day_range(p.token);
            let on = if end >= start {
                start <= t && t <= end
            } else {
                t >= start || t <= end
            };
            self.host.set_prop_timer(p.token, TIMER_TIME_OF_DAY, on);
            i += 1;
        }
    }

    // ---- registration (§4.6, §4.7) ----

    fn power_cell(pos: [f32; 3]) -> (i32, i32) {
        (trunc_i32(pos[0] * 0.0625), trunc_i32(pos[2] * 0.0625))
    }

    fn held(&self, list: &RefCell<Vec<PropRef>>, p: PropRef) {
        self.host.hold(p.token);
        list.borrow_mut().push(p);
    }

    /// `HandleInsertOccupant` for a prop (§4.6).
    pub fn register_prop(&self, prop: u64) {
        let h = &*self.host;
        let props = h.prop_properties(prop).unwrap_or_default();
        if props.has(PROP_PREVIEW) {
            return;
        }
        let p = PropRef::new(prop, h.prop_class(prop));
        // 1.
        if u8_prop(&props, PROP_NIGHT_LIT) == Some(1) {
            self.held(&self.night, p);
            let night = self.s.borrow().is_night;
            if h.prop_state(prop) != u32::from(night) {
                if !night {
                    h.set_prop_state(prop, 0);
                } else {
                    let pos = h.prop_position(prop);
                    let (x, z) = ((pos[0] / 16.0).floor() as i32, (pos[2] / 16.0).floor() as i32);
                    if h.is_powered(x, z) {
                        h.set_prop_state(prop, 1);
                    }
                }
            }
        }
        // 2.
        let timed = [PROP_TRIGGER, PROP_TIME_OF_DAY, PROP_DATE, PROP_NEEDS_POWER]
            .iter()
            .any(|&id| props.has(id));
        if timed {
            h.set_prop_state(prop, 1);
            h.set_random_chance(prop, u8_prop(&props, PROP_CHANCE).unwrap_or(100));
            // Mask 4.
            match u32_prop(&props, PROP_TRIGGER) {
                Some(t) if t != 0 => {
                    if h.view_available() {
                        self.host.hold(prop);
                        self.triggers.borrow_mut().entry(t, Vec::new).push(p);
                        h.set_prop_timer(prop, TIMER_TRIGGER, h.view_trigger_state(t));
                    }
                }
                _ => h.set_prop_timer(prop, TIMER_TRIGGER, true),
            }
            // Mask 1.
            match props.get(PROP_TIME_OF_DAY) {
                None => h.set_prop_timer(prop, TIMER_TIME_OF_DAY, true),
                Some(v) => {
                    let vals: Vec<f32> = v.items().iter().map(|e| e.as_f64() as f32).collect();
                    if vals.len() >= 2 && vals[0] != vals[1] {
                        h.set_time_of_day_range(prop, vals[0], vals[1]);
                        self.held(&self.tod, p);
                    }
                }
            }
            // Mask 2.
            match u32_prop(&props, PROP_DATE) {
                None => h.set_prop_timer(prop, TIMER_DATE, true),
                Some(date) => {
                    let duration = u32_prop(&props, PROP_DATE_DURATION).unwrap_or(1);
                    let flag = u8_prop(&props, PROP_DATE_FLAG).unwrap_or(0);
                    h.set_date_range(prop, date, duration, flag);
                    self.held(&self.dated, p);
                }
            }
            // Mask 8.
            let needs_power = props
                .get(PROP_NEEDS_POWER)
                .and_then(|v| v.items().first().map(Elem::as_i64))
                .is_some_and(|v| v != 0);
            if needs_power {
                let (x, z) = Self::power_cell(h.prop_position(prop));
                h.set_prop_timer(prop, TIMER_POWER, h.is_powered(x, z));
                self.held(&self.power, p);
            } else {
                h.set_prop_timer(prop, TIMER_POWER, true);
            }
        }
        // 3.
        let t = h.prop_type(prop);
        if t == PROP_TYPE_SIGNPOST || t == PROP_TYPE_STREET_SIGN {
            self.signs.borrow_mut().push(p);
        }
    }

    fn erase_from(&self, list: &RefCell<Vec<PropRef>>, prop: u64, swap: bool, release: bool) {
        let pos = list.borrow().iter().position(|p| p.token == prop);
        if let Some(i) = pos {
            if swap {
                list.borrow_mut().swap_remove(i);
            } else {
                list.borrow_mut().remove(i);
            }
            if release {
                self.host.release(prop);
            }
        }
    }

    /// `HandleRemoveOccupant` for a prop (§4.7, PV12): after the preview
    /// test, each list is searched only when the prop passes that list's
    /// test, which is looser than §4.6's join condition. A trigger-map prop
    /// whose 3D view has gone stays in the map, held [quirk].
    pub fn unregister_prop(&self, prop: u64) {
        let h = &*self.host;
        let props = h.prop_properties(prop).unwrap_or_default();
        if props.has(PROP_PREVIEW) {
            return;
        }
        // Night list: night-lit = 1 and a prop occupant (`0xE9793A65`; the
        // message's occupant is one, §3.3).
        if u8_prop(&props, PROP_NIGHT_LIT) == Some(1) {
            self.erase_from(&self.night, prop, false, true);
        }
        // Trigger map: the trigger present and non-zero, the 3D view there.
        if let Some(t) = u32_prop(&props, PROP_TRIGGER).filter(|&t| t != 0) {
            if h.view_available() {
                let found = {
                    let mut map = self.triggers.borrow_mut();
                    let hit = map.get_mut(t).and_then(|v| {
                        let i = v.iter().position(|p| p.token == prop)?;
                        v.remove(i);
                        Some(v.is_empty())
                    });
                    if hit == Some(true) {
                        map.remove(t);
                    }
                    hit.is_some()
                };
                if found {
                    h.release(prop);
                }
            }
        }
        // The time-of-day, dated and power lists: the property present (its
        // value not read).
        if props.has(PROP_TIME_OF_DAY) {
            self.erase_from(&self.tod, prop, true, true);
        }
        if props.has(PROP_DATE) {
            self.erase_from(&self.dated, prop, true, true);
        }
        if props.has(PROP_NEEDS_POWER) {
            self.erase_from(&self.power, prop, true, true);
        }
        // Signs: the two sign types (not held).
        let t = h.prop_type(prop);
        if t == PROP_TYPE_SIGNPOST || t == PROP_TYPE_STREET_SIGN {
            self.erase_from(&self.signs, prop, false, false);
        }
    }

    // ---- triggers (§4.10) ----

    fn effect_trigger(&self, trigger: u32, on: bool) {
        let list = {
            let mut map = self.triggers.borrow_mut();
            match map.get_mut(trigger) {
                Some(v) => {
                    v.retain(|p| !p.is_null());
                    v.clone()
                }
                None => return,
            }
        };
        for p in list {
            self.host.set_prop_timer(p.token, TIMER_TRIGGER, on);
        }
    }

    // ---- signs (§4.11) ----

    /// Slot 8 `ShowSignsAndLabels(show)`.
    pub fn show_signs(&self, show: bool) {
        if self.s.borrow().signs_visible == show {
            return;
        }
        self.edit(|s| s.signs_visible = show);
        for p in self.sign_list() {
            if !p.is_null() {
                self.host.set_prop_flag(p.token, FLAG_SIGN_VISIBLE, show);
            }
        }
    }

    /// Slot 9 `ToggleSignsAndLabels`.
    pub fn toggle_signs(&self) {
        let v = self.s.borrow().signs_visible;
        self.show_signs(!v);
    }

    /// Slot 10 `AreSignsAndLabelsVisible`.
    pub fn signs_visible(&self) -> bool {
        self.s.borrow().signs_visible
    }

    fn remove_street_signs(&self, network_occupant: u64) {
        let h = &*self.host;
        let cell = h.network_first_cell(network_occupant);
        for p in self.sign_list() {
            if p.is_null() || h.prop_type(p.token) != PROP_TYPE_STREET_SIGN {
                continue;
            }
            let pos = h.prop_position(p.token);
            if h.position_to_cell(pos[0], pos[2]) == cell {
                h.remove_prop(p.token);
            }
        }
    }

    fn reseat_signposts(&self) {
        let h = &*self.host;
        for p in self.sign_list() {
            if p.is_null() || h.prop_type(p.token) != PROP_TYPE_SIGNPOST {
                continue;
            }
            let pos = h.prop_position(p.token);
            let (alt, water) = h.terrain_heights(pos[0], pos[2]);
            let y = alt.max(water);
            if (pos[1] - y).abs() > SIGNPOST_TOLERANCE {
                h.set_prop_position(p.token, [pos[0], y, pos[2]]);
            }
        }
    }

    // ---- lot state (§4.12) ----

    fn lot_state_changed(&self, lot: u64, old: u32, new: u32) {
        let h = &*self.host;
        let (x, z) = h.lot_location(lot);
        let Some(l2) = h.lot_at(x, z, true) else {
            return;
        };
        if h.lot_size(l2) != h.lot_size(lot) {
            return;
        }
        if !h.lot_livable(lot) {
            if let Some(objs) = h.lot_config_objects(l2) {
                for prop in h.lot_props(l2) {
                    let Some(tag) = h.prop_lot_tag(prop) else {
                        continue;
                    };
                    if objs.iter().find(|o| o.tag == tag).is_some_and(|o| o.flags & 1 != 0) {
                        h.remove_prop(prop);
                    }
                }
            }
        }
        h.post_lot_state_props(x, z, old == 2 && new == 3);
    }

    fn lot_state_props(&self, x: i32, z: i32) {
        let h = &*self.host;
        let Some(lot) = h.lot_at(x, z, false) else {
            return;
        };
        for prop in h.lot_props(lot) {
            let Some(v) = h.prop_properties(prop).and_then(|p| u8_prop(&p, PROP_LOT_STATE)) else {
                continue;
            };
            let state = h.lot_state(lot);
            h.set_prop_state(prop, u32::from(state != u32::from(v)));
        }
    }

    // ---- power (§4.13) ----

    fn power_changed(&self, rect: CellRect) {
        let h = &*self.host;
        let night = self.s.borrow().is_night;
        for prop in h.street_props_in_rect(rect) {
            let lit = h.prop_properties(prop).and_then(|p| u8_prop(&p, PROP_NIGHT_LIT)) == Some(1);
            if !lit {
                continue;
            }
            let (x, z) = Self::power_cell(h.prop_position(prop));
            let powered = h.is_powered(x, z);
            let state = h.prop_state(prop);
            if !powered && state == 1 {
                h.set_prop_state(prop, 0);
            } else if powered && night && state == 0 {
                h.set_prop_state(prop, 1);
            }
        }
        let mut i = 0;
        loop {
            let p = self.power.borrow().get(i).copied();
            let Some(p) = p else {
                break;
            };
            if p.is_null() {
                self.power.borrow_mut().swap_remove(i);
                continue;
            }
            let (x, z) = Self::power_cell(h.prop_position(p.token));
            if (rect.x1..=rect.x2).contains(&x) && (rect.z1..=rect.z2).contains(&z) {
                h.set_prop_timer(p.token, TIMER_POWER, h.is_powered(x, z));
            }
            i += 1;
        }
    }

    // ---- record (§7) ----

    fn parse(inp: &mut dyn InStream) -> Option<Parsed> {
        let version = inp.get_u16()?;
        if version < MIN_RECORD_VERSION {
            return None;
        }
        let scan_flag = inp.get_i8()?;
        // Stored as read (PV13): 0 fresh, 2 done, any other byte running.
        let scan_byte = inp.get_u8()?;
        let state = match scan_byte {
            0 => ScanState::Fresh,
            2 => ScanState::Done,
            _ => ScanState::Running,
        };
        let mut v = [0i32; 7];
        for x in &mut v {
            *x = inp.get_i32()?;
        }
        let scanner = ScanIterator {
            x: v[0],
            z: v[1],
            step: v[2],
            x0: v[3],
            z0: v[4],
            x1: v[5],
            z1: v[6],
            state,
        };
        let wealth_added = inp.get_u8()? != 0;
        let n = inp.get_u32()?;
        let mut requesters = Vec::new();
        for _ in 0..n {
            requesters.push(Requester::from_ref(&inp.get_ref_as(IID_PROP_REQUESTER)?));
        }
        let mut managers = [PropRef::default(); 3];
        for (m, iid) in managers
            .iter_mut()
            .zip([IID_LOT_MANAGER, IID_PROP_MANAGER, IID_NETWORK_LOT_MANAGER])
        {
            *m = PropRef::from_ref(&inp.get_ref_as(iid)?);
        }
        let night_cursor = inp.get_u32()?;
        let night_mode = inp.get_u32()?;
        let night_batch = inp.get_u32()?;
        let night_acc = inp.get_u32()?;
        let is_night = inp.get_u8()? != 0;
        let refs = |inp: &mut dyn InStream| -> Option<Vec<PropRef>> {
            let n = inp.get_u32()?;
            (0..n)
                .map(|_| inp.get_ref_as(IID_PROP_OCCUPANT).map(|r| PropRef::from_ref(&r)))
                .collect()
        };
        let night = refs(inp)?;
        let entries = inp.get_u32()?;
        let mut triggers = Vec::new();
        for _ in 0..entries {
            let t = inp.get_u32()?;
            triggers.push((t, refs(inp)?));
        }
        let tod = refs(inp)?;
        let dated = refs(inp)?;
        let power = refs(inp)?;
        let (signs_visible, signs) = if version >= 5 {
            (Some(inp.get_u8()? != 0), refs(inp)?)
        } else {
            (None, Vec::new())
        };
        if inp.error() != 0 {
            return None;
        }
        Some(Parsed {
            scan_flag,
            scanner,
            scan_byte: if state == ScanState::Running { scan_byte } else { 1 },
            wealth_added,
            requesters,
            managers,
            night_cursor,
            night_mode,
            night_batch,
            night_acc,
            is_night,
            night,
            triggers,
            tod,
            dated,
            power,
            signs_visible,
            signs,
        })
    }
}

/// A parsed record (§7).
struct Parsed {
    scan_flag: i8,
    scanner: ScanIterator,
    scan_byte: u8,
    wealth_added: bool,
    requesters: Vec<Requester>,
    managers: [PropRef; 3],
    night_cursor: u32,
    night_mode: u32,
    night_batch: u32,
    night_acc: u32,
    is_night: bool,
    night: Vec<PropRef>,
    triggers: Vec<(u32, Vec<PropRef>)>,
    tod: Vec<PropRef>,
    dated: Vec<PropRef>,
    power: Vec<PropRef>,
    signs_visible: Option<bool>,
    signs: Vec<PropRef>,
}

fn put_refs(out: &mut dyn OutStream, list: &[PropRef]) {
    out.put_u32(list.len() as u32);
    for p in list {
        out.put_ref(&p.to_ref());
    }
}

impl SaveObject for PropDeveloper {
    fn class_id(&self) -> u32 {
        CLSID_PROP_DEVELOPER
    }

    /// §7: always version 5.
    fn write(&self, out: &mut dyn OutStream) -> bool {
        let s = self.snapshot();
        out.put_u16(RECORD_VERSION);
        out.put_i8(s.scan_flag);
        let it = &s.scanner;
        out.put_u8(match it.state {
            ScanState::Fresh => 0,
            ScanState::Running => s.scan_byte,
            ScanState::Done => 2,
        });
        for v in [it.x, it.z, it.step, it.x0, it.z0, it.x1, it.z1] {
            out.put_i32(v);
        }
        out.put_u8(u8::from(s.wealth_added));
        let reqs = self.requesters();
        out.put_u32(reqs.len() as u32);
        for r in &reqs {
            out.put_ref(&r.to_ref());
        }
        for m in self.managers.get() {
            out.put_ref(&m.to_ref());
        }
        out.put_u32(s.night_cursor);
        out.put_u32(s.night_mode);
        out.put_u32(s.night_batch);
        out.put_u32(s.night_acc);
        out.put_u8(u8::from(s.is_night));
        put_refs(out, &self.night_list());
        let map = self.trigger_map();
        out.put_u32(map.len() as u32);
        for (t, list) in &map {
            out.put_u32(*t);
            put_refs(out, list);
        }
        put_refs(out, &self.time_of_day_list());
        put_refs(out, &self.dated_list());
        put_refs(out, &self.power_list());
        out.put_u8(u8::from(s.signs_visible));
        put_refs(out, &self.sign_list());
        out.error() == 0
    }

    /// §7: false below version 4. [choice] Atomic.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        let Some(p) = Self::parse(inp) else {
            return false;
        };
        self.edit(|s| {
            s.scan_flag = p.scan_flag;
            s.scanner = p.scanner;
            s.scan_byte = p.scan_byte;
            s.wealth_added = p.wealth_added;
            s.night_cursor = p.night_cursor;
            s.night_mode = p.night_mode;
            s.night_batch = p.night_batch;
            s.night_acc = p.night_acc;
            s.is_night = p.is_night;
            if let Some(v) = p.signs_visible {
                s.signs_visible = v;
            }
        });
        *self.requesters.borrow_mut() = p.requesters;
        self.managers.set(p.managers);
        *self.night.borrow_mut() = p.night;
        {
            let mut map = self.triggers.borrow_mut();
            for (t, list) in p.triggers {
                map.entry(t, Vec::new).extend(list);
            }
        }
        self.tod.borrow_mut().extend(p.tod);
        self.dated.borrow_mut().extend(p.dated);
        self.power.borrow_mut().extend(p.power);
        self.signs.borrow_mut().extend(p.signs);
        true
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
