// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The advisor system and the MySims over the C ABI (fc4.h "advisor
//! system", docs/specs/advisors.md and docs/specs/mysims.md).
//!
//! One [`fc4_advisors`] handle holds the advisor system and, when a
//! [`fc4_mysim_host`] is given, the MySim classes and the MySim agent. The
//! host implements [`fc4_advisor_host`] (Lua, city, messages, the
//! variables' sources) and [`fc4_mysim_host`].
//!
//! Core objects (the system, advisors, advices, subjects) cross the ABI as
//! [`fc4_adv_obj`] handles: the class store creates them with
//! `fc4_advisors_create_object`, writes and reads them with
//! `fc4_adv_obj_write` / `_read`, and hands them back in `fc4_ref::
//! core_handle` on reference reads. The host's `object_created` callback is
//! told about every object the core creates (with a new handle the host
//! owns) and returns its token, which the core writes for references.
//!
//! Data the host returns through pointers (entry strings, effect lists,
//! building names) only has to stay valid until the callback returns.
//!
//! Threads: the core is single-threaded (`RefCell`). The plugin serialises
//! every call on one handle.

use std::cell::RefCell;
use std::ffi::{c_char, c_void, CStr, CString};
use std::rc::Rc;

use fc4_core::advisors::{
    self as adv, Advice, AdviceEntry, AdviceSubject, Advisor, AdvisorEntry, AdvisorHost, AdvisorMessage, AdvisorSystem,
    ConditionalStatus, Item, ListSrc, LuaResult, LuaValue, MsgObj, OutMessage, SimDate, Src, SubjectInit, SubjectState,
    Target, Val,
};
use fc4_core::mysims::{
    self, Building, Dispatch, Field, Grid, JobDestination, Lot, MySim, MySimAgent, MySimHost, MySimPost,
};
use fc4_core::stream::{InStream, RefValue, SaveObject};

use crate::development::fc4_res_key;
use crate::guard;
use crate::stream::{fc4_bytes_sink, fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

// ---- object handles -----------------------------------------------------------------------------

#[repr(C)]
pub struct fc4_advisors {
    _private: [u8; 0],
}

#[repr(C)]
pub struct fc4_adv_obj {
    _private: [u8; 0],
}

pub const FC4_ADV_OBJ_SYSTEM: u32 = 1;
pub const FC4_ADV_OBJ_ADVISOR: u32 = 2;
pub const FC4_ADV_OBJ_ADVICE: u32 = 3;
pub const FC4_ADV_OBJ_SUBJECT: u32 = 4;

/// What an `fc4_adv_obj*` points at.
#[derive(Clone)]
enum Obj {
    System(Rc<AdvisorSystem>),
    Advisor(Rc<Advisor>),
    Advice(Rc<Advice>),
    Subject(Rc<AdviceSubject>),
}

impl Obj {
    fn from_save(o: Rc<dyn SaveObject>) -> Option<Obj> {
        let any = o.as_any_rc();
        let any = match any.downcast::<AdvisorSystem>() {
            Ok(s) => return Some(Obj::System(s)),
            Err(a) => a,
        };
        let any = match any.downcast::<Advisor>() {
            Ok(a) => return Some(Obj::Advisor(a)),
            Err(a) => a,
        };
        let any = match any.downcast::<Advice>() {
            Ok(a) => return Some(Obj::Advice(a)),
            Err(a) => a,
        };
        any.downcast::<AdviceSubject>().ok().map(Obj::Subject)
    }
    fn save(&self) -> Rc<dyn SaveObject> {
        match self {
            Obj::System(s) => s.clone(),
            Obj::Advisor(a) => a.clone(),
            Obj::Advice(a) => a.clone(),
            Obj::Subject(s) => s.clone(),
        }
    }
    fn kind(&self) -> u32 {
        match self {
            Obj::System(_) => FC4_ADV_OBJ_SYSTEM,
            Obj::Advisor(_) => FC4_ADV_OBJ_ADVISOR,
            Obj::Advice(_) => FC4_ADV_OBJ_ADVICE,
            Obj::Subject(_) => FC4_ADV_OBJ_SUBJECT,
        }
    }
    fn into_raw(self) -> *mut fc4_adv_obj {
        Box::into_raw(Box::new(self)) as *mut fc4_adv_obj
    }
}

/// SAFETY: NULL or a live `fc4_adv_obj*`.
unsafe fn obj_ref<'a>(o: *const fc4_adv_obj) -> Option<&'a Obj> {
    (o as *const Obj).as_ref()
}

fn decoder(_class_id: u32, handle: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    // SAFETY: core handles on reference reads are this module's handles
    // (fc4.h: the host reports them only for objects it got from us).
    unsafe { obj_ref(handle as *const fc4_adv_obj).map(Obj::save) }
}

// ---- data crossing the ABI -------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_adv_advisor_entry {
    pub class_id: u32,
    pub id: i32,
    pub advice_type: i32,
    pub reserved: u32,
    pub caption: *const u8,
    pub caption_len: usize,
    pub name: *const u8,
    pub name_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_adv_advice_entry {
    pub guid: u32,
    pub class_id: u32,
    pub advice_type: i32,
    pub priority: i32,
    pub mood: i32,
    pub frequency: i32,
    pub timeout: i32,
    pub event: u32,
    pub command: u32,
    pub nd_type: i32,
    pub once: u8,
    pub news_only: u8,
    pub persist: u8,
    pub no_timeout: u8,
    pub has_trigger: u8,
    pub reserved: [u8; 3],
    pub trigger: *const u8,
    pub trigger_len: usize,
    pub effects: *const u32,
    pub effect_count: usize,
}

pub const FC4_ADV_LUA_OTHER: u32 = 0;
pub const FC4_ADV_LUA_BOOL: u32 = 1;
pub const FC4_ADV_LUA_NUMBER: u32 = 2;
pub const FC4_ADV_LUA_STRING: u32 = 3;
pub const FC4_ADV_LUA_ERROR: u32 = 4;

/// A protected Lua call's result.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_adv_lua_result {
    pub kind: u32,
    pub boolean: u32,
    pub number: f64,
    pub str_: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_adv_date {
    pub day_number: i32,
    pub year: i32,
    pub month: i32,
    pub day: i32,
}

/// An outgoing message; `obj_kind` 0 = none, else `FC4_ADV_OBJ_*`, or 5 = a
/// host object (`obj_token`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_adv_out_message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub obj_kind: u32,
    pub obj_token: u64,
    /// The advisor ID, or the advice GUID.
    pub obj_id: u32,
    pub obj_serial: u32,
}

pub const FC4_ADV_OBJ_HOST: u32 = 5;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_adv_src {
    pub kind: u32,
    pub a: u32,
    pub b: u32,
    pub reserved: u32,
    pub object: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_adv_val {
    pub i: i64,
    pub f: f64,
    pub object: u64,
    pub object2: u64,
    pub n: u64,
    pub m: u64,
    pub cell_x: f32,
    pub cell_z: f32,
    pub has_cell: u8,
    pub reserved: [u8; 7],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_adv_item {
    pub object: u64,
    pub f0: f32,
    pub f1: f32,
    pub x: f32,
    pub z: f32,
    pub flags: u32,
    pub kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_adv_conditional_status {
    pub status: u8,
    pub reserved: u8,
    pub building_count: u16,
    pub reserved2: u32,
    pub tooltip: *const u8,
    pub tooltip_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_adv_subject_state {
    pub x: f32,
    pub z: f32,
    pub has_location: u8,
    pub reserved: [u8; 3],
    pub data1: u32,
    pub data2: u32,
    pub reserved2: u32,
    /// The object's token (0 = none).
    pub object: u64,
}

impl From<&SubjectState> for fc4_adv_subject_state {
    fn from(s: &SubjectState) -> Self {
        let (x, z) = s.location.unwrap_or((0.0, 0.0));
        fc4_adv_subject_state {
            x,
            z,
            has_location: s.location.is_some().into(),
            data1: s.data1,
            data2: s.data2,
            object: s.object.unwrap_or(0),
            ..Default::default()
        }
    }
}

/// An incoming message (`DoMessage`); 0 = no void1 / IGZUnknown.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_adv_message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    pub reserved: u32,
    pub void1: u64,
    pub unknown: u64,
}

fn msg_in(m: &fc4_adv_message) -> AdvisorMessage {
    AdvisorMessage {
        msg_type: m.msg_type,
        data1: m.data1,
        data2: m.data2,
        data3: m.data3,
        data4: m.data4,
        void1: opt(m.void1),
        unknown: opt(m.unknown),
    }
}

pub const FC4_ADV_VALUE_NIL: u32 = 0;
pub const FC4_ADV_VALUE_BOOL: u32 = 1;
pub const FC4_ADV_VALUE_NUMBER: u32 = 2;
pub const FC4_ADV_VALUE_STRING: u32 = 3;
pub const FC4_ADV_VALUE_SUBJECT: u32 = 4;

/// A Lua value of `fc4_advisors_call_lua`. A subject is an `fc4_adv_obj*`
/// (NULL = a null userdata); a result subject is a new handle the caller
/// destroys; a result string stays valid until the next call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_adv_lua_value {
    pub kind: u32,
    pub boolean: u32,
    pub number: f64,
    pub str_: *const u8,
    pub len: usize,
    pub subject: *mut fc4_adv_obj,
}

/// The system's diagnostics.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_advisors_state {
    pub struct_size: u32,
    pub advisor_count: u32,
    pub last_day: i32,
    pub idle_start: u32,
    pub has_idle_start: u8,
    pub loaded_from_save: u8,
    pub initialized: u8,
    pub reload_pending: u8,
    pub dolly_llama: u8,
    pub has_mysims: u8,
    pub reserved: [u8; 2],
    pub thresholds: [i32; 3],
    pub tag_subject: i32,
    pub tag_string: i32,
    pub context_depth: u32,
    pub subscribed_events: u32,
    pub reserved2: u32,
}

/// One advisor's diagnostics.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_advisor_state {
    pub struct_size: u32,
    pub class_id: u32,
    pub id: i32,
    pub table_index: i32,
    pub advice_type: i32,
    pub month_counter: i32,
    pub current_day: i32,
    pub cursor: i32,
    pub advice_count: u32,
    pub active_count: u32,
    pub queue_length: u32,
    pub pending: u32,
    pub dirty: u8,
    pub base_subscribed: u8,
    pub own_subscribed: u8,
    pub debug_subscribed: u8,
    pub fire_active: u8,
    pub riot_active: u8,
    pub user_disaster: u8,
    pub is_mysim: u8,
    pub building_count: i32,
    pub park_count: i32,
    pub recreation_count: i32,
    pub landmark_count: u32,
    pub zot_count: u32,
    pub serial: u32,
    pub token: u64,
}

/// One advice's diagnostics (the advisor's vector entry `index`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_advice_state {
    pub struct_size: u32,
    pub guid: u32,
    pub class_id: u32,
    pub flags: u32,
    pub advisor_id: i32,
    pub table_index: i32,
    pub priority: i32,
    pub frequency: i32,
    pub timeout: i32,
    pub mood: i32,
    pub event_id: u32,
    pub command_id: u32,
    pub trigger: i32,
    pub nd_type: i32,
    pub serial: u32,
    pub effect_count: u32,
    pub loaded: u8,
    pub active: u8,
    pub has_subject: u8,
    pub has_trigger_text: u8,
    pub next_day: i32,
    pub expiry: i32,
    pub last_month: i32,
    pub reserved: u32,
    pub reserved2: u32,
    pub token: u64,
}

// ---- the advisor host ---------------------------------------------------------------------------

/// What the advisor system needs from the game (fc4.h
/// `fc4_advisor_host`). A missing callback reads as 0 / false / none.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_advisor_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    // Lua: script data
    pub advisor_count: Option<unsafe extern "C" fn(U, *mut u32) -> bool>,
    pub advisor_entry: Option<unsafe extern "C" fn(U, u32, *mut fc4_adv_advisor_entry) -> bool>,
    pub advice_count: Option<unsafe extern "C" fn(U) -> u32>,
    pub advice_entry: Option<unsafe extern "C" fn(U, u32, *mut fc4_adv_advice_entry) -> bool>,
    pub compile_trigger: Option<unsafe extern "C" fn(U, *const u8, usize, *mut i32) -> bool>,
    pub release_trigger: Option<unsafe extern "C" fn(U, i32)>,
    pub call_trigger: Option<unsafe extern "C" fn(U, i32, *mut fc4_adv_lua_result)>,
    pub call_condition: Option<unsafe extern "C" fn(U, i32, *mut fc4_adv_lua_result)>,
    pub detokenize: Option<unsafe extern "C" fn(U, *const u8, usize, fc4_bytes_sink, *mut c_void) -> bool>,
    pub reload_scripts: Option<unsafe extern "C" fn(U)>,
    pub register_api: Option<unsafe extern "C" fn(U, bool)>,
    pub set_tutorial_lua: Option<unsafe extern "C" fn(U, bool)>,
    pub create_tags: Option<unsafe extern "C" fn(U, *mut i32, *mut i32)>,
    pub register_detokenizer: Option<unsafe extern "C" fn(U, bool)>,
    pub popup_priority: Option<unsafe extern "C" fn(U, *const c_char, *mut f64) -> bool>,
    pub collect_garbage: Option<unsafe extern "C" fn(U)>,
    pub close_lua: Option<unsafe extern "C" fn(U)>,
    pub set_global: Option<unsafe extern "C" fn(U, *const c_char, f64)>,
    pub set_local: Option<unsafe extern "C" fn(U, *const c_char, f64, *const fc4_adv_subject_state, u64)>,
    pub set_global_string: Option<unsafe extern "C" fn(U, *const c_char, *const u8, usize)>,
    pub debug_enabled: Option<unsafe extern "C" fn(U) -> bool>,
    pub register_cheat: Option<unsafe extern "C" fn(U, u32, *const c_char)>,
    pub unregister_cheat: Option<unsafe extern "C" fn(U, u32)>,
    pub advisor_window: Option<unsafe extern "C" fn(U, bool)>,
    // city and simulator
    pub has_city: Option<unsafe extern "C" fn(U) -> bool>,
    pub city_established: Option<unsafe extern "C" fn(U) -> bool>,
    pub difficulty: Option<unsafe extern "C" fn(U, *mut i32) -> bool>,
    pub cell_counts: Option<unsafe extern "C" fn(U, *mut u32, *mut u32)>,
    pub sim_date: Option<unsafe extern "C" fn(U, *mut fc4_adv_date)>,
    pub start_year: Option<unsafe extern "C" fn(U) -> i32>,
    pub is_any_paused: Option<unsafe extern "C" fn(U) -> bool>,
    pub now_ms: Option<unsafe extern "C" fn(U) -> u32>,
    // messages
    pub subscribe: Option<unsafe extern "C" fn(U, u32, i32, u64, u32)>,
    pub unsubscribe: Option<unsafe extern "C" fn(U, u32, i32, u64, u32)>,
    pub post: Option<unsafe extern "C" fn(U, *const fc4_adv_out_message)>,
    pub send: Option<unsafe extern "C" fn(U, *const fc4_adv_out_message)>,
    pub post_update_self: Option<unsafe extern "C" fn(U, i32, u64, u32, u32)>,
    pub post_ticker: Option<unsafe extern "C" fn(U)>,
    pub global_effect: Option<unsafe extern "C" fn(U, u32, bool)>,
    pub rebuild_text: Option<unsafe extern "C" fn(U, u32, u64)>,
    // the variables' sources
    pub value: Option<unsafe extern "C" fn(U, *const fc4_adv_src, *mut fc4_adv_val) -> bool>,
    pub list: Option<unsafe extern "C" fn(U, u32, u32, *const u32, usize, *mut fc4_adv_item, usize) -> i64>,
    pub record_history: Option<unsafe extern "C" fn(U, u32, i32)>,
    // occupants
    pub occupant_type: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub occupant_bool_property: Option<unsafe extern "C" fn(U, u64, u32) -> i32>,
    pub building_key_instance: Option<unsafe extern "C" fn(U, u64, *mut u32) -> bool>,
    pub is_occupant_group: Option<unsafe extern "C" fn(U, u64, u32) -> bool>,
    // civic and rewards
    pub conditional_status: Option<unsafe extern "C" fn(U, u32, *mut fc4_adv_conditional_status) -> bool>,
    pub update_conditional_status: Option<unsafe extern "C" fn(U, u32, *const fc4_adv_conditional_status)>,
    pub reward_subject: Option<unsafe extern "C" fn(U, u32, *mut fc4_adv_subject_state) -> bool>,
    // neighbour deals
    pub deal_type: Option<unsafe extern "C" fn(U, u64, *mut u32) -> bool>,
    pub cancel_deal: Option<unsafe extern "C" fn(U, u64)>,
    // history, ordinances, region
    pub history_value: Option<unsafe extern "C" fn(U, u32, i32, *mut f64) -> bool>,
    pub history_trend: Option<unsafe extern "C" fn(U, u32, i32, i32, *mut f32) -> bool>,
    pub history_count: Option<unsafe extern "C" fn(U, u32, i32, i32, *mut f64) -> bool>,
    pub ordinance_flag: Option<unsafe extern "C" fn(U, u32, bool) -> i32>,
    pub set_ordinance_on: Option<unsafe extern "C" fn(U, u32, bool)>,
    pub ordinance_amount: Option<unsafe extern "C" fn(U, u32, u32, *mut i64) -> bool>,
    pub ordinance_text: Option<unsafe extern "C" fn(U, u32, u32, fc4_bytes_sink, *mut c_void) -> bool>,
    pub tutorial_guid: Option<unsafe extern "C" fn(U, *mut u32) -> bool>,
    // references
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    pub resolve_ref: Option<unsafe extern "C" fn(U, u64, u32) -> u64>,
    pub object_created: Option<unsafe extern "C" fn(U, u32, u32, *mut fc4_adv_obj) -> u64>,
    pub object_dropped: Option<unsafe extern "C" fn(U, u64)>,
}

/// What the MySims need from the game (fc4.h `fc4_mysim_host`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_mysim_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    // Lua
    pub create_table: Option<unsafe extern "C" fn(U, i32) -> bool>,
    pub set_field: Option<unsafe extern "C" fn(U, i32, *const c_char, *const fc4_mysim_field)>,
    pub activate_lua: Option<unsafe extern "C" fn(U, i32) -> bool>,
    pub tuning: Option<unsafe extern "C" fn(U, *const c_char, *mut f64) -> bool>,
    pub ui_blocked: Option<unsafe extern "C" fn(U) -> bool>,
    pub register_agent_lua: Option<unsafe extern "C" fn(U, bool)>,
    // city
    pub cell_counts: Option<unsafe extern "C" fn(U, *mut u32, *mut u32)>,
    pub cell_center: Option<unsafe extern "C" fn(U, i32, i32, *mut f32, *mut f32)>,
    pub disaster_ongoing: Option<unsafe extern "C" fn(U) -> bool>,
    pub disaster_locations: Option<unsafe extern "C" fn(U, *mut fc4_mysim_disaster_location, usize) -> usize>,
    // buildings and lots
    pub building: Option<unsafe extern "C" fn(U, u64, *mut fc4_mysim_building) -> bool>,
    pub lot: Option<unsafe extern "C" fn(U, u64, *mut fc4_mysim_lot) -> bool>,
    pub occupant_position: Option<unsafe extern "C" fn(U, u64, *mut f32) -> bool>,
    pub buildings_in_box: Option<unsafe extern "C" fn(U, i32, i32, i32, i32, *mut u64, usize) -> i64>,
    pub all_buildings: Option<unsafe extern "C" fn(U, *mut u64, usize) -> i64>,
    pub lots_in_rect: Option<unsafe extern "C" fn(U, i32, i32, i32, i32, *mut fc4_mysim_lot, usize) -> usize>,
    pub special_buildings: Option<unsafe extern "C" fn(U, *mut u64, usize) -> usize>,
    pub education_buildings: Option<unsafe extern "C" fn(U, bool, *mut u64, usize) -> usize>,
    pub grade: Option<unsafe extern "C" fn(U, u64, bool, *mut f32) -> bool>,
    pub job_title: Option<unsafe extern "C" fn(U, u64, fc4_bytes_sink, *mut c_void)>,
    pub landmark_funding: Option<unsafe extern "C" fn(U, u64, *mut f32) -> bool>,
    // residential
    pub average_age: Option<unsafe extern "C" fn(U, i32, i32, *mut i32) -> bool>,
    pub eq_by_wealth: Option<unsafe extern "C" fn(U, i32, i32, u8, *mut f32) -> bool>,
    pub hq_by_wealth: Option<unsafe extern "C" fn(U, i32, i32, u8, *mut f32) -> bool>,
    pub life_expectancy: Option<unsafe extern "C" fn(U, i32, i32, *mut u8) -> bool>,
    pub new_age_by_wealth: Option<unsafe extern "C" fn(U, u8, *mut u8) -> bool>,
    // simulators
    pub grid: Option<unsafe extern "C" fn(U, u32, u32, i32, i32, *mut i32) -> bool>,
    pub grid_at: Option<unsafe extern "C" fn(U, u32, u32, f32, f32, *mut i32) -> bool>,
    pub coverage: Option<unsafe extern "C" fn(U, bool, i32, i32, i32, i32, *mut i32) -> bool>,
    pub powered: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub watered: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub stations: Option<unsafe extern "C" fn(U, bool, *mut fc4_mysim_station, usize) -> i64>,
    pub travel_time_ratio: Option<unsafe extern "C" fn(U, i32, i32, u32, *mut f32) -> bool>,
    pub trip_simulators: Option<unsafe extern "C" fn(U) -> bool>,
    // the dispatch
    pub dispatch: Option<unsafe extern "C" fn(U, i32, *mut fc4_mysim_dispatch) -> bool>,
    pub find_new_job: Option<unsafe extern "C" fn(U, i32) -> bool>,
    pub change_job: Option<unsafe extern "C" fn(U, i32, u64) -> bool>,
    pub job_destination: Option<unsafe extern "C" fn(U, i32, *mut fc4_mysim_job_destination) -> bool>,
    pub commute_time: Option<unsafe extern "C" fn(U, i32, *mut f32) -> bool>,
    pub neighbor_city_name: Option<unsafe extern "C" fn(U, i32, i32, fc4_bytes_sink, *mut c_void) -> bool>,
    pub dispatch_command: Option<unsafe extern "C" fn(U, i32, *const c_char)>,
    // messages and effects
    pub post: Option<unsafe extern "C" fn(U, *const fc4_mysim_post)>,
    pub automata_trigger: Option<unsafe extern "C" fn(U, *const c_char, *const f32)>,
    pub subscribe: Option<unsafe extern "C" fn(U, u32, bool)>,
    // references
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    pub resolve_ref: Option<unsafe extern "C" fn(U, u64, u32) -> u64>,
}

pub const FC4_MYSIM_FIELD_NUMBER: u32 = 0;
pub const FC4_MYSIM_FIELD_STRING: u32 = 1;
pub const FC4_MYSIM_FIELD_BUILDING: u32 = 2;
pub const FC4_MYSIM_FIELD_SUBJECT: u32 = 3;

/// A Lua field value: a number, a string, a building subject (`object`, 0
/// = nil) or a subject (`subject`, NULL = nil, its token in `object`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_mysim_field {
    pub kind: u32,
    pub reserved: u32,
    pub number: f64,
    pub object: u64,
    pub str_: *const u8,
    pub len: usize,
    pub subject: *const fc4_adv_subject_state,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_mysim_disaster_location {
    pub disaster_type: u32,
    pub x: i32,
    pub z: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_mysim_building {
    pub building_type: u32,
    pub purpose: u32,
    pub completion: f32,
    pub position: [f32; 3],
    pub name: *const u8,
    pub name_len: usize,
    pub groups: *const u32,
    pub group_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_mysim_lot {
    /// 0 = no building.
    pub building: u64,
    pub cell_x: i32,
    pub cell_z: i32,
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
    pub condition: u8,
    pub road_access: u8,
    pub wealth: u8,
    pub reserved: u8,
    pub reserved2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_mysim_station {
    pub station: u64,
    pub funding: f32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_mysim_dispatch {
    pub state: i32,
    pub flags: u32,
    pub cell_x: i32,
    pub cell_z: i32,
    pub position: [f32; 3],
    pub travel_type: u32,
    pub wait_minutes: u32,
    pub at_home: u8,
    pub at_work: u8,
    pub by_commute: u8,
    pub reserved: u8,
    pub destination: u64,
    pub location: u64,
}

pub const FC4_MYSIM_JOB_LOT: u32 = 1;
pub const FC4_MYSIM_JOB_NEIGHBOR: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_mysim_job_destination {
    pub kind: u32,
    pub cell_x: i32,
    pub cell_z: i32,
    pub reserved: u32,
    pub building: u64,
}

/// A posted message; `sim` = the MySim's advisor ID (0 = none).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_mysim_post {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub sim: i32,
    pub void1: u64,
}

pub const FC4_MYSIM_GRID_CRIME: u32 = 1;
pub const FC4_MYSIM_GRID_POLLUTION: u32 = 2;
pub const FC4_MYSIM_GRID_AURA: u32 = 3;
pub const FC4_MYSIM_GRID_AVERAGE_EQ: u32 = 4;
pub const FC4_MYSIM_GRID_AVERAGE_HQ: u32 = 5;
pub const FC4_MYSIM_GRID_CONGESTION: u32 = 6;
pub const FC4_MYSIM_GRID_DESIRABILITY: u32 = 7;

fn grid_code(g: Grid) -> (u32, u32) {
    match g {
        Grid::Crime => (FC4_MYSIM_GRID_CRIME, 0),
        Grid::Pollution(i) => (FC4_MYSIM_GRID_POLLUTION, u32::from(i)),
        Grid::Aura => (FC4_MYSIM_GRID_AURA, 0),
        Grid::AverageEq => (FC4_MYSIM_GRID_AVERAGE_EQ, 0),
        Grid::AverageHq => (FC4_MYSIM_GRID_AVERAGE_HQ, 0),
        Grid::Congestion => (FC4_MYSIM_GRID_CONGESTION, 0),
        Grid::Desirability(d) => (FC4_MYSIM_GRID_DESIRABILITY, d),
    }
}

/// The agent's diagnostics.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_mysims_agent_state {
    pub struct_size: u32,
    pub initialised: u8,
    pub reserved: [u8; 3],
    /// The slot vector as advisor IDs (0 = empty).
    pub slots: [i32; 6],
    pub last_disaster_type: u32,
    pub active_ui_index: i32,
}

/// One MySim's fields (mysims.md §2.2).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_mysim_state {
    pub struct_size: u32,
    pub state: u8,
    pub slot: u8,
    pub wealth: u8,
    pub sign: u8,
    pub is_male: u8,
    pub age: u8,
    pub birth_month: u8,
    pub retired: u8,
    pub neighbor_job: u8,
    pub commute_check: u8,
    pub lua_table: u8,
    pub unnamed_flag: u8,
    pub eq_index: u8,
    pub hq_index: u8,
    pub generation: u16,
    pub home_cell_x: u32,
    pub home_cell_z: u32,
    pub neighbor_cell: [i32; 2],
    pub commute_cell: [i32; 2],
    pub jobless_days: u32,
    pub bitmap_id: u32,
    pub last_commute_time: u32,
    pub last_local_disaster: u32,
    pub home_position: [f32; 3],
    pub eq: f32,
    pub hq: f32,
    pub eq_delta: f32,
    pub hq_delta: f32,
    pub eq_ring: [f32; 6],
    pub hq_ring: [f32; 6],
    pub vehicle_key: fc4_res_key,
    pub car_key: fc4_res_key,
    pub pedestrian_key: fc4_res_key,
    pub unnamed_pair: [i32; 2],
    pub last_player_trip_day: i32,
    pub last_trip_update_day: i32,
    pub local_area_size: i32,
    pub pylon_units: u8,
    pub pylon_running: u8,
    pub reserved: [u8; 2],
    pub local_group_count: u32,
    pub pylon_elapsed: u64,
    pub residence: u64,
    pub job: u64,
    pub dispatch: u64,
    pub dispatch_tool: u64,
}

const _: () = {
    let p = std::mem::size_of::<usize>();
    assert!(std::mem::size_of::<fc4_adv_advisor_entry>() == 16 + 4 * p);
    assert!(std::mem::size_of::<fc4_adv_advice_entry>() == 48 + 4 * p);
    assert!(std::mem::size_of::<fc4_adv_lua_result>() == 16 + 2 * p);
    assert!(std::mem::size_of::<fc4_adv_date>() == 16);
    assert!(std::mem::size_of::<fc4_adv_out_message>() == 32);
    assert!(std::mem::size_of::<fc4_adv_src>() == 24);
    assert!(std::mem::size_of::<fc4_adv_val>() == 64);
    assert!(std::mem::size_of::<fc4_adv_item>() == 32);
    assert!(std::mem::size_of::<fc4_adv_conditional_status>() == 8 + 2 * p);
    assert!(std::mem::size_of::<fc4_adv_subject_state>() == 32);
    assert!(std::mem::size_of::<fc4_adv_message>() == 40);
    assert!(std::mem::size_of::<fc4_adv_lua_value>() == if p == 4 { 32 } else { 40 });
    assert!(std::mem::size_of::<fc4_advisors_state>() == 56);
    assert!(std::mem::size_of::<fc4_advisor_state>() == 88);
    assert!(std::mem::size_of::<fc4_advice_state>() == 96);
    assert!(std::mem::size_of::<fc4_mysim_field>() == if p == 4 { 40 } else { 48 });
    assert!(std::mem::size_of::<fc4_mysim_disaster_location>() == 12);
    assert!(std::mem::size_of::<fc4_mysim_building>() == 24 + 4 * p);
    assert!(std::mem::size_of::<fc4_mysim_lot>() == 40);
    assert!(std::mem::size_of::<fc4_mysim_station>() == 16);
    assert!(std::mem::size_of::<fc4_mysim_dispatch>() == 56);
    assert!(std::mem::size_of::<fc4_mysim_job_destination>() == 24);
    assert!(std::mem::size_of::<fc4_mysim_post>() == 24);
    assert!(std::mem::size_of::<fc4_mysims_agent_state>() == 40);
    assert!(std::mem::size_of::<fc4_mysim_state>() == 240);
    assert!(std::mem::size_of::<fc4_advisor_host>() == 2 * p + 65 * p);
    assert!(std::mem::size_of::<fc4_mysim_host>() == 2 * p + 47 * p);
};

// ---- helpers --------------------------------------------------------------------------------------

fn opt(t: u64) -> Option<u64> {
    (t != 0).then_some(t)
}

fn c_name(name: &str) -> CString {
    CString::new(name).unwrap_or_default()
}

/// SAFETY: `p` is NULL or valid for `len` bytes.
unsafe fn bytes<'a>(p: *const u8, len: usize) -> &'a [u8] {
    if p.is_null() || len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p, len)
    }
}

unsafe extern "C" fn sink_bytes(ctx: *mut c_void, data: *const u8, len: usize) {
    let v = &mut *(ctx as *mut Vec<u8>);
    v.extend_from_slice(bytes(data, len));
}

macro_rules! cb {
    ($s:expr, $f:ident, $default:expr $(, $arg:expr)*) => {
        // SAFETY: whoever created the object promised valid callbacks.
        match $s.h.$f { Some(f) => unsafe { f($s.h.user $(, $arg)*) }, None => $default }
    };
}

fn lua_result(r: &fc4_adv_lua_result) -> LuaResult {
    match r.kind {
        FC4_ADV_LUA_BOOL => LuaResult::Bool(r.boolean != 0),
        FC4_ADV_LUA_NUMBER => LuaResult::Number(r.number),
        // SAFETY: the host's string is valid until the callback returned.
        FC4_ADV_LUA_STRING => LuaResult::Str(unsafe { bytes(r.str_, r.len) }.to_vec()),
        FC4_ADV_LUA_ERROR => LuaResult::Error,
        _ => LuaResult::Other,
    }
}

fn lua_result_default() -> fc4_adv_lua_result {
    fc4_adv_lua_result {
        kind: FC4_ADV_LUA_OTHER,
        boolean: 0,
        number: 0.0,
        str_: std::ptr::null(),
        len: 0,
    }
}

fn out_message(m: &OutMessage) -> fc4_adv_out_message {
    let (obj_kind, obj_token, obj_id, obj_serial) = match &m.unknown {
        MsgObj::None => (0, 0, 0, 0),
        MsgObj::Advice(a) => (FC4_ADV_OBJ_ADVICE, a.host_token(), a.guid(), a.serial()),
        MsgObj::Advisor(a) => (FC4_ADV_OBJ_ADVISOR, a.host_token(), a.id() as u32, a.serial()),
        MsgObj::Host(t) => (FC4_ADV_OBJ_HOST, *t, 0, 0),
    };
    fc4_adv_out_message {
        msg_type: m.msg_type,
        data1: m.data1,
        data2: m.data2,
        obj_kind,
        obj_token,
        obj_id,
        obj_serial,
    }
}

fn target_args(t: &Target) -> (u32, i32, u64) {
    match t {
        Target::System => (FC4_ADV_OBJ_SYSTEM, 0, 0),
        Target::Advisor(a) => (FC4_ADV_OBJ_ADVISOR, a.id(), a.host_token()),
    }
}

fn status_in(s: &fc4_adv_conditional_status) -> ConditionalStatus {
    ConditionalStatus {
        status: s.status,
        building_count: s.building_count,
        // SAFETY: valid until the callback returned.
        tooltip: unsafe { bytes(s.tooltip, s.tooltip_len) }.to_vec(),
    }
}

// ---- the advisor host adapter ---------------------------------------------------------------------

struct CHost {
    h: fc4_advisor_host,
}

impl AdvisorHost for CHost {
    fn advisor_count(&self) -> Option<u32> {
        let mut n = 0u32;
        cb!(self, advisor_count, false, &mut n).then_some(n)
    }
    fn advisor_entry(&self, i: u32) -> Option<AdvisorEntry> {
        let mut e = fc4_adv_advisor_entry {
            class_id: 0,
            id: 0,
            advice_type: 0,
            reserved: 0,
            caption: std::ptr::null(),
            caption_len: 0,
            name: std::ptr::null(),
            name_len: 0,
        };
        if !cb!(self, advisor_entry, false, i, &mut e) {
            return None;
        }
        // SAFETY: valid until the callback returned (we are still in it).
        let (caption, name) = unsafe { (bytes(e.caption, e.caption_len), bytes(e.name, e.name_len)) };
        Some(AdvisorEntry {
            class_id: e.class_id,
            id: e.id,
            caption: caption.to_vec(),
            name: name.to_vec(),
            advice_type: e.advice_type,
        })
    }
    fn advice_count(&self) -> u32 {
        cb!(self, advice_count, 0)
    }
    fn advice_entry(&self, i: u32) -> Option<AdviceEntry> {
        let mut e = fc4_adv_advice_entry {
            guid: 0,
            class_id: 0,
            advice_type: 0,
            priority: 0,
            mood: 0,
            frequency: 0,
            timeout: 0,
            event: 0,
            command: 0,
            nd_type: 0,
            once: 0,
            news_only: 0,
            persist: 0,
            no_timeout: 0,
            has_trigger: 0,
            reserved: [0; 3],
            trigger: std::ptr::null(),
            trigger_len: 0,
            effects: std::ptr::null(),
            effect_count: 0,
        };
        if !cb!(self, advice_entry, false, i, &mut e) {
            return None;
        }
        // SAFETY: valid until the callback returned.
        let trigger = (e.has_trigger != 0).then(|| unsafe { bytes(e.trigger, e.trigger_len) }.to_vec());
        let effects = if e.effects.is_null() || e.effect_count == 0 {
            Vec::new()
        } else {
            // SAFETY: as above.
            unsafe { std::slice::from_raw_parts(e.effects, e.effect_count) }.to_vec()
        };
        Some(AdviceEntry {
            guid: e.guid,
            class_id: e.class_id,
            advice_type: e.advice_type,
            priority: e.priority,
            mood: e.mood,
            frequency: e.frequency,
            timeout: e.timeout,
            trigger,
            once: e.once != 0,
            news_only: e.news_only != 0,
            event: e.event,
            command: e.command,
            persist: e.persist != 0,
            no_timeout: e.no_timeout != 0,
            effects,
            nd_type: e.nd_type,
        })
    }
    fn compile_trigger(&self, text: &[u8]) -> Option<i32> {
        let mut r = 0i32;
        cb!(self, compile_trigger, false, text.as_ptr(), text.len(), &mut r).then_some(r)
    }
    fn release_trigger(&self, r: i32) {
        cb!(self, release_trigger, (), r)
    }
    fn call_trigger(&self, r: i32) -> LuaResult {
        let mut out = lua_result_default();
        cb!(self, call_trigger, (), r, &mut out);
        lua_result(&out)
    }
    fn call_condition(&self, table_index: i32) -> LuaResult {
        let mut out = lua_result_default();
        cb!(self, call_condition, (), table_index, &mut out);
        lua_result(&out)
    }
    fn detokenize(&self, text: &[u8]) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        let ctx = &mut v as *mut Vec<u8> as *mut c_void;
        if cb!(
            self,
            detokenize,
            false,
            text.as_ptr(),
            text.len(),
            Some(sink_bytes),
            ctx
        ) {
            v
        } else {
            text.to_vec()
        }
    }
    fn reload_scripts(&self) {
        cb!(self, reload_scripts, ())
    }
    fn register_api(&self, global: bool) {
        cb!(self, register_api, (), global)
    }
    fn set_tutorial_lua(&self, on: bool) {
        cb!(self, set_tutorial_lua, (), on)
    }
    fn create_tags(&self) -> (i32, i32) {
        let (mut a, mut b) = (0i32, 0i32);
        cb!(self, create_tags, (), &mut a, &mut b);
        (a, b)
    }
    fn register_detokenizer(&self, on: bool) {
        cb!(self, register_detokenizer, (), on)
    }
    fn popup_priority(&self, key: &str) -> Option<f64> {
        let k = c_name(key);
        let mut v = 0.0f64;
        cb!(self, popup_priority, false, k.as_ptr(), &mut v).then_some(v)
    }
    fn collect_garbage(&self) {
        cb!(self, collect_garbage, ())
    }
    fn close_lua(&self) {
        cb!(self, close_lua, ())
    }
    fn set_global(&self, name: &str, value: f64) {
        let n = c_name(name);
        cb!(self, set_global, (), n.as_ptr(), value)
    }
    fn set_local(&self, name: &str, value: f64, subject: Option<&Rc<AdviceSubject>>) {
        let n = c_name(name);
        let st = subject.map(|s| fc4_adv_subject_state::from(&s.state()));
        let token = subject.map_or(0, |s| s.host_token());
        let p = st
            .as_ref()
            .map_or(std::ptr::null(), |s| s as *const fc4_adv_subject_state);
        cb!(self, set_local, (), n.as_ptr(), value, p, token)
    }
    fn set_global_string(&self, name: &str, value: &[u8]) {
        let n = c_name(name);
        cb!(self, set_global_string, (), n.as_ptr(), value.as_ptr(), value.len())
    }
    fn debug_enabled(&self) -> bool {
        cb!(self, debug_enabled, false)
    }
    fn register_cheat(&self, id: u32, name: &str) {
        let n = c_name(name);
        cb!(self, register_cheat, (), id, n.as_ptr())
    }
    fn unregister_cheat(&self, id: u32) {
        cb!(self, unregister_cheat, (), id)
    }
    fn advisor_window(&self, start: bool) {
        cb!(self, advisor_window, (), start)
    }
    fn has_city(&self) -> bool {
        cb!(self, has_city, false)
    }
    fn city_established(&self) -> bool {
        cb!(self, city_established, false)
    }
    fn difficulty(&self) -> Option<i32> {
        let mut v = 0i32;
        cb!(self, difficulty, false, &mut v).then_some(v)
    }
    fn cell_counts(&self) -> (u32, u32) {
        let (mut x, mut z) = (0u32, 0u32);
        cb!(self, cell_counts, (), &mut x, &mut z);
        (x, z)
    }
    fn sim_date(&self) -> SimDate {
        let mut d = fc4_adv_date::default();
        cb!(self, sim_date, (), &mut d);
        SimDate {
            day_number: d.day_number,
            year: d.year,
            month: d.month,
            day: d.day,
        }
    }
    fn start_year(&self) -> i32 {
        cb!(self, start_year, 0)
    }
    fn is_any_paused(&self) -> bool {
        cb!(self, is_any_paused, false)
    }
    fn now_ms(&self) -> u32 {
        cb!(self, now_ms, 0)
    }
    fn subscribe(&self, target: &Target, msg_type: u32) {
        let (k, id, t) = target_args(target);
        cb!(self, subscribe, (), k, id, t, msg_type)
    }
    fn unsubscribe(&self, target: &Target, msg_type: u32) {
        let (k, id, t) = target_args(target);
        cb!(self, unsubscribe, (), k, id, t, msg_type)
    }
    fn post(&self, m: &OutMessage) {
        let o = out_message(m);
        cb!(self, post, (), &o)
    }
    fn send(&self, m: &OutMessage) {
        let o = out_message(m);
        cb!(self, send, (), &o)
    }
    fn post_update_self(&self, advisor: &Rc<Advisor>, data1: u32, data2: u32) {
        cb!(
            self,
            post_update_self,
            (),
            advisor.id(),
            advisor.host_token(),
            data1,
            data2
        )
    }
    fn post_ticker(&self) {
        cb!(self, post_ticker, ())
    }
    fn global_effect(&self, id: u32, assert: bool) {
        cb!(self, global_effect, (), id, assert)
    }
    fn rebuild_text(&self, advice: &Rc<Advice>) {
        cb!(self, rebuild_text, (), advice.guid(), advice.host_token())
    }
    fn value(&self, src: &Src) -> Option<Val> {
        let s = fc4_adv_src {
            kind: src.kind.code(),
            a: src.a,
            b: src.b,
            reserved: 0,
            object: src.object,
        };
        let mut v = fc4_adv_val::default();
        if !cb!(self, value, false, &s, &mut v) {
            return None;
        }
        Some(Val {
            i: v.i,
            f: v.f,
            cell: (v.has_cell != 0).then_some((v.cell_x, v.cell_z)),
            object: v.object,
            object2: v.object2,
            n: v.n,
            m: v.m,
        })
    }
    fn list(&self, src: &ListSrc) -> Option<Vec<Item>> {
        let kind = src.kind.map_or(0, |k| k as u32);
        let g = src.groups.as_ptr();
        let n = cb!(
            self,
            list,
            -1,
            kind,
            src.a,
            g,
            src.groups.len(),
            std::ptr::null_mut(),
            0
        );
        if n < 0 {
            return None;
        }
        let mut v = vec![fc4_adv_item::default(); n as usize];
        let m = cb!(
            self,
            list,
            -1,
            kind,
            src.a,
            g,
            src.groups.len(),
            v.as_mut_ptr(),
            v.len()
        );
        if m < 0 {
            return None;
        }
        v.truncate((m as usize).min(n as usize));
        Some(
            v.iter()
                .map(|i| Item {
                    object: i.object,
                    f0: i.f0,
                    f1: i.f1,
                    x: i.x,
                    z: i.z,
                    flags: i.flags,
                    kind: i.kind,
                })
                .collect(),
        )
    }
    fn record_history(&self, history_type: u32, value: i32) {
        cb!(self, record_history, (), history_type, value)
    }
    fn occupant_type(&self, occupant: u64) -> u32 {
        cb!(self, occupant_type, 0, occupant)
    }
    fn occupant_bool_property(&self, occupant: u64, id: u32) -> Option<bool> {
        match cb!(self, occupant_bool_property, -1, occupant, id) {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn building_key_instance(&self, occupant: u64) -> Option<u32> {
        let mut v = 0u32;
        cb!(self, building_key_instance, false, occupant, &mut v).then_some(v)
    }
    fn is_occupant_group(&self, occupant: u64, group: u32) -> bool {
        cb!(self, is_occupant_group, false, occupant, group)
    }
    fn conditional_status(&self, guid: u32) -> Option<ConditionalStatus> {
        let mut s = fc4_adv_conditional_status {
            status: 0,
            reserved: 0,
            building_count: 0,
            reserved2: 0,
            tooltip: std::ptr::null(),
            tooltip_len: 0,
        };
        cb!(self, conditional_status, false, guid, &mut s).then(|| status_in(&s))
    }
    fn update_conditional_status(&self, guid: u32, status: &ConditionalStatus) {
        let s = fc4_adv_conditional_status {
            status: status.status,
            reserved: 0,
            building_count: status.building_count,
            reserved2: 0,
            tooltip: status.tooltip.as_ptr(),
            tooltip_len: status.tooltip.len(),
        };
        cb!(self, update_conditional_status, (), guid, &s)
    }
    fn reward_subject(&self, guid: u32) -> Option<SubjectInit> {
        let mut s = fc4_adv_subject_state::default();
        cb!(self, reward_subject, false, guid, &mut s).then(|| SubjectInit {
            location: (s.has_location != 0).then_some((s.x, s.z)),
            object: s.object,
            data1: s.data1,
            data2: s.data2,
        })
    }
    fn deal_type(&self, object: u64) -> Option<u32> {
        let mut v = 0u32;
        cb!(self, deal_type, false, object, &mut v).then_some(v)
    }
    fn cancel_deal(&self, object: u64) {
        cb!(self, cancel_deal, (), object)
    }
    fn history_value(&self, history_type: u32, day: i32) -> Option<f64> {
        let mut v = 0.0f64;
        cb!(self, history_value, false, history_type, day, &mut v).then_some(v)
    }
    fn history_trend(&self, history_type: u32, from: i32, to: i32) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, history_trend, false, history_type, from, to, &mut v).then_some(v)
    }
    fn history_count(&self, history_type: u32, from: i32, to: i32) -> Option<f64> {
        let mut v = 0.0f64;
        cb!(self, history_count, false, history_type, from, to, &mut v).then_some(v)
    }
    fn ordinance_flag(&self, id: u32, on: bool) -> Option<bool> {
        match cb!(self, ordinance_flag, -1, id, on) {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn set_ordinance_on(&self, id: u32, on: bool) {
        cb!(self, set_ordinance_on, (), id, on)
    }
    fn ordinance_amount(&self, id: u32, which: u32) -> Option<i64> {
        let mut v = 0i64;
        cb!(self, ordinance_amount, false, id, which, &mut v).then_some(v)
    }
    fn ordinance_text(&self, id: u32, which: u32) -> Option<Vec<u8>> {
        let mut v: Vec<u8> = Vec::new();
        let ctx = &mut v as *mut Vec<u8> as *mut c_void;
        cb!(self, ordinance_text, false, id, which, Some(sink_bytes), ctx).then_some(v)
    }
    fn tutorial_guid(&self) -> Option<u32> {
        let mut v = 0u32;
        cb!(self, tutorial_guid, false, &mut v).then_some(v)
    }
    fn add_ref(&self, token: u64) {
        if token != 0 {
            cb!(self, add_ref, (), token)
        }
    }
    fn release(&self, token: u64) {
        if token != 0 {
            cb!(self, release, (), token)
        }
    }
    fn object_ref(&self, token: u64) -> RefValue {
        if token == 0 {
            RefValue::Null
        } else {
            RefValue::Host { token, class_id: 0 }
        }
    }
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64> {
        match *r {
            RefValue::Host { token, .. } if token != 0 => opt(cb!(self, resolve_ref, 0, token, iid)),
            _ => None,
        }
    }
    fn object_created(&self, obj: &RefValue) -> u64 {
        let (Some(f), RefValue::Object(o)) = (self.h.object_created, obj) else {
            return 0;
        };
        let Some(h) = Obj::from_save(o.clone()) else { return 0 };
        let (kind, class) = (h.kind(), o.class_id());
        // SAFETY: valid callback; the handle is the host's from here on.
        unsafe { f(self.h.user, kind, class, h.into_raw()) }
    }
    fn object_dropped(&self, token: u64) {
        if token != 0 {
            cb!(self, object_dropped, (), token)
        }
    }
}

// ---- the MySim host adapter -------------------------------------------------------------------------

struct CMySimHost {
    h: fc4_mysim_host,
}

fn list_u64(f: impl Fn(*mut u64, usize) -> i64) -> Option<Vec<u64>> {
    let n = f(std::ptr::null_mut(), 0);
    if n < 0 {
        return None;
    }
    let mut v = vec![0u64; n as usize];
    let m = f(v.as_mut_ptr(), v.len());
    if m < 0 {
        return None;
    }
    v.truncate((m as usize).min(n as usize));
    Some(v)
}

fn lot_in(l: &fc4_mysim_lot) -> Lot {
    Lot {
        building: opt(l.building),
        cell: (l.cell_x, l.cell_z),
        rect: (l.x0, l.z0, l.x1, l.z1),
        condition: l.condition,
        road_access: l.road_access != 0,
        wealth: l.wealth,
    }
}

impl MySimHost for CMySimHost {
    fn create_table(&self, sim: i32) -> bool {
        cb!(self, create_table, false, sim)
    }
    fn set_field(&self, sim: i32, name: &str, v: Field) {
        let n = c_name(name);
        let mut f = fc4_mysim_field {
            kind: FC4_MYSIM_FIELD_NUMBER,
            reserved: 0,
            number: 0.0,
            object: 0,
            str_: std::ptr::null(),
            len: 0,
            subject: std::ptr::null(),
        };
        let st;
        match &v {
            Field::Num(x) => f.number = *x,
            Field::Str(s) => {
                f.kind = FC4_MYSIM_FIELD_STRING;
                f.str_ = s.as_ptr();
                f.len = s.len();
            }
            Field::Building(b) => {
                f.kind = FC4_MYSIM_FIELD_BUILDING;
                f.object = b.unwrap_or(0);
            }
            Field::Subject(s) => {
                f.kind = FC4_MYSIM_FIELD_SUBJECT;
                if let Some(s) = s {
                    st = fc4_adv_subject_state::from(&s.state());
                    f.subject = &st;
                    f.object = s.host_token();
                }
            }
        }
        cb!(self, set_field, (), sim, n.as_ptr(), &f)
    }
    fn activate_lua(&self, sim: i32) -> bool {
        cb!(self, activate_lua, false, sim)
    }
    fn tuning(&self, key: &str) -> Option<f64> {
        let k = c_name(key);
        let mut v = 0.0f64;
        cb!(self, tuning, false, k.as_ptr(), &mut v).then_some(v)
    }
    fn ui_blocked(&self) -> bool {
        cb!(self, ui_blocked, false)
    }
    fn register_agent_lua(&self, on: bool) {
        cb!(self, register_agent_lua, (), on)
    }
    fn cell_counts(&self) -> (u32, u32) {
        let (mut x, mut z) = (0u32, 0u32);
        cb!(self, cell_counts, (), &mut x, &mut z);
        (x, z)
    }
    fn cell_center(&self, cx: i32, cz: i32) -> (f32, f32) {
        let (mut x, mut z) = (0.0f32, 0.0f32);
        cb!(self, cell_center, (), cx, cz, &mut x, &mut z);
        (x, z)
    }
    fn disaster_ongoing(&self) -> bool {
        cb!(self, disaster_ongoing, false)
    }
    fn disaster_locations(&self) -> Vec<(u32, (i32, i32))> {
        let n = cb!(self, disaster_locations, 0, std::ptr::null_mut(), 0);
        let mut v = vec![fc4_mysim_disaster_location::default(); n];
        let m = cb!(self, disaster_locations, 0, v.as_mut_ptr(), v.len());
        v.truncate(m.min(n));
        v.iter().map(|d| (d.disaster_type, (d.x, d.z))).collect()
    }
    fn building(&self, b: u64) -> Option<Building> {
        let mut o = fc4_mysim_building {
            building_type: 0,
            purpose: 0,
            completion: 0.0,
            position: [0.0; 3],
            name: std::ptr::null(),
            name_len: 0,
            groups: std::ptr::null(),
            group_count: 0,
        };
        if !cb!(self, building, false, b, &mut o) {
            return None;
        }
        let groups = if o.groups.is_null() || o.group_count == 0 {
            Vec::new()
        } else {
            // SAFETY: valid until the callback returned.
            unsafe { std::slice::from_raw_parts(o.groups, o.group_count) }.to_vec()
        };
        Some(Building {
            building_type: o.building_type,
            // SAFETY: as above.
            name: unsafe { bytes(o.name, o.name_len) }.to_vec(),
            groups,
            completion: o.completion,
            purpose: o.purpose,
            position: o.position,
        })
    }
    fn lot(&self, b: u64) -> Option<Lot> {
        let mut l = fc4_mysim_lot::default();
        cb!(self, lot, false, b, &mut l).then(|| lot_in(&l))
    }
    fn occupant_position(&self, o: u64) -> Option<[f32; 3]> {
        let mut p = [0.0f32; 3];
        cb!(self, occupant_position, false, o, p.as_mut_ptr()).then_some(p)
    }
    fn buildings_in_box(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> Option<Vec<u64>> {
        list_u64(|p, n| cb!(self, buildings_in_box, -1, x0, z0, x1, z1, p, n))
    }
    fn all_buildings(&self) -> Option<Vec<u64>> {
        list_u64(|p, n| cb!(self, all_buildings, -1, p, n))
    }
    fn lots_in_rect(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> Vec<Lot> {
        let n = cb!(self, lots_in_rect, 0, x0, z0, x1, z1, std::ptr::null_mut(), 0);
        let mut v = vec![fc4_mysim_lot::default(); n];
        let m = cb!(self, lots_in_rect, 0, x0, z0, x1, z1, v.as_mut_ptr(), v.len());
        v.truncate(m.min(n));
        v.iter().map(lot_in).collect()
    }
    fn special_buildings(&self) -> Vec<u64> {
        list_u64(|p, n| cb!(self, special_buildings, 0, p, n) as i64).unwrap_or_default()
    }
    fn education_buildings(&self, hospitals: bool) -> Vec<u64> {
        list_u64(|p, n| cb!(self, education_buildings, 0, hospitals, p, n) as i64).unwrap_or_default()
    }
    fn grade(&self, b: u64, hospital: bool) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, grade, false, b, hospital, &mut v).then_some(v)
    }
    fn job_title(&self, b: u64) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        let ctx = &mut v as *mut Vec<u8> as *mut c_void;
        cb!(self, job_title, (), b, Some(sink_bytes), ctx);
        v
    }
    fn landmark_funding(&self, b: u64) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, landmark_funding, false, b, &mut v).then_some(v)
    }
    fn average_age(&self, x: i32, z: i32) -> Option<i32> {
        let mut v = 0i32;
        cb!(self, average_age, false, x, z, &mut v).then_some(v)
    }
    fn eq_by_wealth(&self, x: i32, z: i32, wealth: u8) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, eq_by_wealth, false, x, z, wealth, &mut v).then_some(v)
    }
    fn hq_by_wealth(&self, x: i32, z: i32, wealth: u8) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, hq_by_wealth, false, x, z, wealth, &mut v).then_some(v)
    }
    fn life_expectancy(&self, x: i32, z: i32) -> Option<u8> {
        let mut v = 0u8;
        cb!(self, life_expectancy, false, x, z, &mut v).then_some(v)
    }
    fn new_age_by_wealth(&self, wealth: u8) -> Option<u8> {
        let mut v = 0u8;
        cb!(self, new_age_by_wealth, false, wealth, &mut v).then_some(v)
    }
    fn grid(&self, g: Grid, x: i32, z: i32) -> Option<i32> {
        let (k, a) = grid_code(g);
        let mut v = 0i32;
        cb!(self, grid, false, k, a, x, z, &mut v).then_some(v)
    }
    fn grid_at(&self, g: Grid, x: f32, z: f32) -> Option<i32> {
        let (k, a) = grid_code(g);
        let mut v = 0i32;
        cb!(self, grid_at, false, k, a, x, z, &mut v).then_some(v)
    }
    fn coverage(&self, police: bool, x0: i32, z0: i32, w: i32, h: i32) -> Option<i32> {
        let mut v = 0i32;
        cb!(self, coverage, false, police, x0, z0, w, h, &mut v).then_some(v)
    }
    fn powered(&self, b: u64) -> Option<bool> {
        match cb!(self, powered, -1, b) {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn watered(&self, b: u64) -> Option<bool> {
        match cb!(self, watered, -1, b) {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn stations(&self, police: bool) -> Option<Vec<(u64, f32)>> {
        let n = cb!(self, stations, -1, police, std::ptr::null_mut(), 0);
        if n < 0 {
            return None;
        }
        let mut v = vec![fc4_mysim_station::default(); n as usize];
        let m = cb!(self, stations, -1, police, v.as_mut_ptr(), v.len());
        if m < 0 {
            return None;
        }
        v.truncate((m as usize).min(n as usize));
        Some(v.iter().map(|s| (s.station, s.funding)).collect())
    }
    fn travel_time_ratio(&self, x: i32, z: i32, travel_type: u32) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, travel_time_ratio, false, x, z, travel_type, &mut v).then_some(v)
    }
    fn trip_simulators(&self) -> bool {
        cb!(self, trip_simulators, false)
    }
    fn dispatch(&self, sim: i32) -> Option<Dispatch> {
        let mut d = fc4_mysim_dispatch::default();
        cb!(self, dispatch, false, sim, &mut d).then(|| Dispatch {
            state: d.state,
            flags: d.flags,
            cell: (d.cell_x, d.cell_z),
            position: d.position,
            travel_type: d.travel_type,
            wait_minutes: d.wait_minutes,
            destination: opt(d.destination),
            location: opt(d.location),
            at_home: d.at_home != 0,
            at_work: d.at_work != 0,
            by_commute: d.by_commute != 0,
        })
    }
    fn find_new_job(&self, sim: i32) -> bool {
        cb!(self, find_new_job, false, sim)
    }
    fn change_job(&self, sim: i32, b: u64) -> bool {
        cb!(self, change_job, false, sim, b)
    }
    fn job_destination(&self, sim: i32) -> Option<JobDestination> {
        let mut d = fc4_mysim_job_destination::default();
        if !cb!(self, job_destination, false, sim, &mut d) {
            return None;
        }
        match d.kind {
            FC4_MYSIM_JOB_LOT if d.building != 0 => Some(JobDestination::Lot {
                building: d.building,
                cell: (d.cell_x, d.cell_z),
            }),
            FC4_MYSIM_JOB_NEIGHBOR => Some(JobDestination::Neighbor {
                cell: (d.cell_x, d.cell_z),
            }),
            _ => None,
        }
    }
    fn commute_time(&self, sim: i32) -> Option<f32> {
        let mut v = 0.0f32;
        cb!(self, commute_time, false, sim, &mut v).then_some(v)
    }
    fn neighbor_city_name(&self, cell: (i32, i32)) -> Option<Vec<u8>> {
        let mut v: Vec<u8> = Vec::new();
        let ctx = &mut v as *mut Vec<u8> as *mut c_void;
        cb!(self, neighbor_city_name, false, cell.0, cell.1, Some(sink_bytes), ctx).then_some(v)
    }
    fn dispatch_command(&self, sim: i32, what: &str) {
        let w = c_name(what);
        cb!(self, dispatch_command, (), sim, w.as_ptr())
    }
    fn read_dispatch_inline(&self, _sim: i32, _inp: &mut dyn InStream) -> bool {
        // Version-12 records are not supported over the ABI (fc4.h).
        false
    }
    fn post(&self, m: &MySimPost) {
        let p = fc4_mysim_post {
            msg_type: m.msg_type,
            data1: m.data1,
            data2: m.data2,
            sim: m.sim.unwrap_or(0),
            void1: m.void1.unwrap_or(0),
        };
        cb!(self, post, (), &p)
    }
    fn automata_trigger(&self, name: &str, position: [f32; 3]) {
        let n = c_name(name);
        cb!(self, automata_trigger, (), n.as_ptr(), position.as_ptr())
    }
    fn subscribe(&self, msg_type: u32, on: bool) {
        cb!(self, subscribe, (), msg_type, on)
    }
    fn add_ref(&self, token: u64) {
        if token != 0 {
            cb!(self, add_ref, (), token)
        }
    }
    fn release(&self, token: u64) {
        if token != 0 {
            cb!(self, release, (), token)
        }
    }
    fn object_ref(&self, token: u64) -> RefValue {
        if token == 0 {
            RefValue::Null
        } else {
            RefValue::Host { token, class_id: 0 }
        }
    }
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64> {
        match *r {
            RefValue::Host { token, .. } if token != 0 => opt(cb!(self, resolve_ref, 0, token, iid)),
            _ => None,
        }
    }
}

// ---- the handle ----------------------------------------------------------------------------------

struct Handle {
    sys: Rc<AdvisorSystem>,
    agent: Option<Rc<MySimAgent>>,
    /// The strings of the last `call_lua` results.
    strings: RefCell<Vec<Vec<u8>>>,
}

fn with_h<R>(p: *const fc4_advisors, fallback: R, f: impl FnOnce(&Handle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_advisors*.
    match unsafe { (p as *const Handle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

fn with_adv<R>(p: *const fc4_advisors, id: i32, fallback: R, f: impl FnOnce(&Handle, &Rc<Advisor>) -> R) -> R {
    with_h(p, None, |h| h.sys.get_advisor(id).map(|a| f(h, &a))).unwrap_or(fallback)
}

fn with_sim<R>(p: *const fc4_advisors, id: i32, fallback: R, f: impl FnOnce(&MySim) -> R) -> R {
    with_h(p, None, |h| h.sys.get_advisor(id).and_then(|a| MySim::with(&a, f))).unwrap_or(fallback)
}

fn with_agent<R>(p: *const fc4_advisors, fallback: R, f: impl FnOnce(&MySimAgent, &Handle) -> R) -> R {
    with_h(p, None, |h| h.agent.as_ref().map(|a| f(a, h))).unwrap_or(fallback)
}

unsafe fn copy_out<T: Copy>(v: &[T], out: *mut T, capacity: usize) -> usize {
    if !out.is_null() {
        for (i, x) in v.iter().take(capacity).enumerate() {
            *out.add(i) = *x;
        }
    }
    v.len()
}

/// Copies `v` NUL-terminated (truncated to `capacity - 1`); returns the
/// full length.
unsafe fn copy_text(v: &[u8], out: *mut c_char, capacity: usize) -> usize {
    if !out.is_null() && capacity > 0 {
        let n = v.len().min(capacity - 1);
        std::ptr::copy_nonoverlapping(v.as_ptr(), out as *mut u8, n);
        *out.add(n) = 0;
    }
    v.len()
}

// ---- system: lifetime and lifecycle -------------------------------------------------------------------

/// Creates the advisor system (constructed, not initialised). Both host
/// tables are copied; `mysim_host` NULL = no MySim classes (MySim advisors
/// of the script data are then skipped as unknown classes). `session_seed`
/// seeds the system's generator and each MySim's (mysims.md §8).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_create(
    host: *const fc4_advisor_host,
    mysim_host: *const fc4_mysim_host,
    session_seed: u32,
) -> *mut fc4_advisors {
    guard(std::ptr::null_mut(), || {
        let Some(h) = host.as_ref() else {
            return std::ptr::null_mut();
        };
        if (h.struct_size as usize) < std::mem::size_of::<fc4_advisor_host>() {
            return std::ptr::null_mut();
        }
        let m = match mysim_host.as_ref() {
            None => None,
            Some(m) if (m.struct_size as usize) >= std::mem::size_of::<fc4_mysim_host>() => {
                Some(Rc::new(CMySimHost { h: *m }))
            }
            Some(_) => return std::ptr::null_mut(),
        };
        let sys = AdvisorSystem::new(Rc::new(CHost { h: *h }), session_seed);
        let agent = m.map(|m| {
            mysims::register(&sys, m.clone(), session_seed);
            MySimAgent::new(m)
        });
        Box::into_raw(Box::new(Handle {
            sys,
            agent,
            strings: RefCell::new(Vec::new()),
        })) as *mut fc4_advisors
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_destroy(p: *mut fc4_advisors) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut Handle)));
    }
}

/// The system's `QueryInterface` answers (§1.1).
#[no_mangle]
pub extern "C" fn fc4_advisors_query_interface(iid: u32) -> bool {
    adv::system_query_interface(iid)
}

/// `Init` (§4.1).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_init(p: *mut fc4_advisors) -> bool {
    with_h(p, false, |h| h.sys.init())
}

/// `Shutdown` (§4.6).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_shutdown(p: *mut fc4_advisors) -> bool {
    with_h(p, false, |h| h.sys.shutdown())
}

/// `reload_data` (§4.2).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_reload_data(p: *mut fc4_advisors) -> bool {
    with_h(p, false, |h| h.sys.reload_data())
}

/// The system's `DoMessage` (§5): the ticker, PostCityInit, city
/// established. Always true for a live handle.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_do_message(p: *mut fc4_advisors, m: *const fc4_adv_message) -> bool {
    let Some(m) = m.as_ref() else { return false };
    let m = msg_in(m);
    with_h(p, false, |h| h.sys.do_message(&m))
}

/// A cheat (§5.2): `text` is the cheat's whole text (NUL-terminated).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_cheat(p: *mut fc4_advisors, id: u32, text: *const c_char) -> bool {
    let t = if text.is_null() {
        String::new()
    } else {
        CStr::from_ptr(text).to_string_lossy().into_owned()
    };
    with_h(p, false, |h| {
        h.sys.cheat(id, &t);
        true
    })
}

/// The system's `PushEvent` to the subscribed advisors (`subject` NULL = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_push_event(
    p: *mut fc4_advisors,
    event_id: u32,
    subject: *const fc4_adv_obj,
) -> bool {
    let s = match obj_ref(subject) {
        Some(Obj::Subject(s)) => Some(s.clone()),
        _ => None,
    };
    with_h(p, false, |h| h.sys.push_event(event_id, s))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_get_state(p: *const fc4_advisors, out: *mut fc4_advisors_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_h(p, false, |h| {
        let s = h.sys.state();
        *o = fc4_advisors_state {
            struct_size: std::mem::size_of::<fc4_advisors_state>() as u32,
            advisor_count: s.advisors.len() as u32,
            last_day: s.last_day,
            idle_start: s.idle_start.unwrap_or(0),
            has_idle_start: s.idle_start.is_some().into(),
            loaded_from_save: s.loaded_from_save.into(),
            initialized: s.initialized.into(),
            reload_pending: s.reload_pending.into(),
            dolly_llama: s.dolly_llama.into(),
            has_mysims: h.agent.is_some().into(),
            reserved: [0; 2],
            thresholds: s.thresholds,
            tag_subject: s.tags.0,
            tag_string: s.tags.1,
            context_depth: s.context.len() as u32,
            subscribed_events: s.subscriptions.len() as u32,
            reserved2: 0,
        };
        true
    })
}

/// The advisor IDs in map order. Returns the count; copies at most
/// `capacity` (`out` may be NULL).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_ids(p: *const fc4_advisors, out: *mut i32, capacity: usize) -> usize {
    with_h(p, 0, |h| {
        let ids: Vec<i32> = h.sys.advisors().into_iter().map(|(id, _)| id).collect();
        copy_out(&ids, out, capacity)
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_state(
    p: *const fc4_advisors,
    id: i32,
    out: *mut fc4_advisor_state,
) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_adv(p, id, false, |_, a| {
        let s = a.state();
        *o = fc4_advisor_state {
            struct_size: std::mem::size_of::<fc4_advisor_state>() as u32,
            class_id: a.class_id(),
            id: s.id,
            table_index: s.table_index,
            advice_type: s.advice_type,
            month_counter: s.month_counter,
            current_day: s.current_day,
            cursor: s.cursor,
            advice_count: s.infos.len() as u32,
            active_count: s.active.len() as u32,
            queue_length: s.queue.len() as u32,
            pending: s.pending,
            dirty: s.dirty.into(),
            base_subscribed: s.base_subscribed.into(),
            own_subscribed: s.own_subscribed.into(),
            debug_subscribed: s.debug_subscribed.into(),
            fire_active: s.fire_active.into(),
            riot_active: s.riot_active.into(),
            user_disaster: s.user_disaster.into(),
            is_mysim: a.ext().is_some_and(|e| e.is_mysim()).into(),
            building_count: s.building_count,
            park_count: s.park_count,
            recreation_count: s.recreation_count,
            landmark_count: s.landmarks.len() as u32,
            zot_count: s.zots.len() as u32,
            serial: a.serial(),
            token: a.host_token(),
        };
        true
    })
}

/// The advisor's caption (`which` 0) or personal name (1), NUL-terminated
/// (truncated to `capacity - 1`). Returns the full length.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_text(
    p: *const fc4_advisors,
    id: i32,
    which: u32,
    out: *mut c_char,
    capacity: usize,
) -> usize {
    with_adv(p, id, 0, |_, a| {
        let t = if which == 0 { a.caption() } else { a.personal_name() };
        copy_text(&t, out, capacity)
    })
}

/// The advisor's `Simulate(kind)` (§5.3): 0 daily, 1 monthly.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_simulate(p: *mut fc4_advisors, id: i32, kind: i32) -> bool {
    with_adv(p, id, false, |_, a| a.simulate(kind))
}

/// The advisor's `DoMessage` (§5.4, mysims.md §3.3).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_do_message(
    p: *mut fc4_advisors,
    id: i32,
    m: *const fc4_adv_message,
) -> bool {
    let Some(m) = m.as_ref() else { return false };
    let m = msg_in(m);
    with_adv(p, id, false, |_, a| a.do_message(&m))
}

/// Slot 19 `CommandWouldTrigger`.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_command_would_trigger(
    p: *mut fc4_advisors,
    id: i32,
    command: u32,
) -> bool {
    with_adv(p, id, false, |_, a| a.command_would_trigger(command))
}

/// The advisor's `PushEvent` (slot 18; `subject` NULL = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_push_event(
    p: *mut fc4_advisors,
    id: i32,
    event_id: u32,
    subject: *const fc4_adv_obj,
) -> bool {
    let s = match obj_ref(subject) {
        Some(Obj::Subject(s)) => Some(s.clone()),
        _ => None,
    };
    with_adv(p, id, false, |_, a| a.push_event(event_id, s))
}

/// The advice at `index` of the advisor's vector.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advice_state(
    p: *const fc4_advisors,
    advisor_id: i32,
    index: u32,
    out: *mut fc4_advice_state,
) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_adv(p, advisor_id, false, |_, a| {
        let Some(info) = a.info(index as usize) else {
            return false;
        };
        let s = info.advice.state();
        *o = fc4_advice_state {
            struct_size: std::mem::size_of::<fc4_advice_state>() as u32,
            guid: s.guid,
            class_id: info.advice.class_id(),
            flags: s.flags,
            advisor_id: s.advisor_id,
            table_index: s.table_index,
            priority: s.priority,
            frequency: s.frequency,
            timeout: s.timeout,
            mood: s.mood,
            event_id: s.event_id,
            command_id: s.command_id,
            trigger: s.trigger,
            nd_type: s.nd_type,
            serial: info.advice.serial(),
            effect_count: s.effects.len() as u32,
            loaded: s.loaded.into(),
            active: info.advice.is_active().into(),
            has_subject: s.subject.is_some().into(),
            has_trigger_text: s.trigger_text.is_some().into(),
            next_day: info.next_day,
            expiry: info.expiry,
            last_month: info.last_month,
            reserved: 0,
            reserved2: 0,
            token: info.advice.host_token(),
        };
        true
    })
}

/// Pushes the advice at `index` of the advisor as the context advice
/// (§6.7).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_push_context_advice(p: *mut fc4_advisors, advisor_id: i32, index: u32) -> bool {
    with_adv(p, advisor_id, false, |h, a| match a.info(index as usize) {
        Some(i) => {
            h.sys.push_context_advice(i.advice);
            true
        }
        None => false,
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_pop_context_advice(p: *mut fc4_advisors) -> bool {
    with_h(p, false, |h| h.sys.pop_context_advice().is_some())
}

/// Calls the `game.*` function or global `name` (§8.3). Returns the number
/// of results (copies at most `capacity`), or −1 when `name` is not one of
/// the system's functions.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_call_lua(
    p: *mut fc4_advisors,
    name: *const c_char,
    args: *const fc4_adv_lua_value,
    nargs: usize,
    out: *mut fc4_adv_lua_value,
    capacity: usize,
) -> i32 {
    if name.is_null() {
        return -1;
    }
    let n = CStr::from_ptr(name).to_string_lossy().into_owned();
    let mut a = Vec::new();
    if !args.is_null() {
        for v in std::slice::from_raw_parts(args, nargs) {
            a.push(match v.kind {
                FC4_ADV_VALUE_BOOL => LuaValue::Bool(v.boolean != 0),
                FC4_ADV_VALUE_NUMBER => LuaValue::Number(v.number),
                FC4_ADV_VALUE_STRING => LuaValue::Str(bytes(v.str_, v.len).to_vec()),
                FC4_ADV_VALUE_SUBJECT => LuaValue::Subject(match obj_ref(v.subject) {
                    Some(Obj::Subject(s)) => Some(s.clone()),
                    _ => None,
                }),
                _ => LuaValue::Nil,
            });
        }
    }
    with_h(p, -1, |h| {
        let Some(r) = h.sys.call_lua(&n, &a) else { return -1 };
        let mut strings = h.strings.borrow_mut();
        strings.clear();
        let mut vals = Vec::new();
        for v in r.iter() {
            let mut c = fc4_adv_lua_value {
                kind: FC4_ADV_VALUE_NIL,
                boolean: 0,
                number: 0.0,
                str_: std::ptr::null(),
                len: 0,
                subject: std::ptr::null_mut(),
            };
            match v {
                LuaValue::Nil => {}
                LuaValue::Bool(b) => {
                    c.kind = FC4_ADV_VALUE_BOOL;
                    c.boolean = u32::from(*b);
                }
                LuaValue::Number(x) => {
                    c.kind = FC4_ADV_VALUE_NUMBER;
                    c.number = *x;
                }
                LuaValue::Str(s) => {
                    c.kind = FC4_ADV_VALUE_STRING;
                    strings.push(s.clone());
                    let s = strings.last().expect("pushed");
                    c.str_ = s.as_ptr();
                    c.len = s.len();
                }
                LuaValue::Subject(s) => {
                    c.kind = FC4_ADV_VALUE_SUBJECT;
                    if out.is_null() || vals.len() >= capacity {
                        // Not copied out: no handle.
                    } else if let Some(s) = s {
                        c.subject = Obj::Subject(s.clone()).into_raw();
                    }
                }
            }
            vals.push(c);
        }
        copy_out(&vals, out, capacity) as i32
    })
}

// ---- objects and persistence --------------------------------------------------------------------------

/// A new handle on the system object.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_system_object(p: *const fc4_advisors) -> *mut fc4_adv_obj {
    with_h(p, std::ptr::null_mut(), |h| Obj::System(h.sys.clone()).into_raw())
}

/// A new handle on advisor `id` (NULL if none).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advisor_object(p: *const fc4_advisors, id: i32) -> *mut fc4_adv_obj {
    with_adv(p, id, std::ptr::null_mut(), |_, a| Obj::Advisor(a.clone()).into_raw())
}

/// A new handle on the advice at `index` of advisor `advisor_id`.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_advice_object(
    p: *const fc4_advisors,
    advisor_id: i32,
    index: u32,
) -> *mut fc4_adv_obj {
    with_adv(p, advisor_id, std::ptr::null_mut(), |_, a| {
        a.info(index as usize)
            .map_or(std::ptr::null_mut(), |i| Obj::Advice(i.advice).into_raw())
    })
}

/// The class store's factory (§11): a new object of a saved class (the
/// system itself for its class), NULL for an unknown class.
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_create_object(p: *mut fc4_advisors, class_id: u32) -> *mut fc4_adv_obj {
    with_h(p, std::ptr::null_mut(), |h| match h.sys.create_object(class_id) {
        Some(RefValue::Object(o)) => Obj::from_save(o).map_or(std::ptr::null_mut(), Obj::into_raw),
        _ => std::ptr::null_mut(),
    })
}

/// A new, empty subject (`CreateInstance(0x6A26D049)`).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_new_subject(p: *mut fc4_advisors) -> *mut fc4_adv_obj {
    with_h(p, std::ptr::null_mut(), |h| {
        Obj::Subject(h.sys.new_subject()).into_raw()
    })
}

/// Puts a loaded advisor into the map (`AddAdvisor`, §4.3).
#[no_mangle]
pub unsafe extern "C" fn fc4_advisors_add_advisor(p: *mut fc4_advisors, obj: *const fc4_adv_obj) -> bool {
    let a = match obj_ref(obj) {
        Some(Obj::Advisor(a)) => a.clone(),
        _ => return false,
    };
    with_h(p, false, |h| h.sys.add_advisor(Some(a)))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_destroy(o: *mut fc4_adv_obj) {
    if !o.is_null() {
        guard((), || drop(Box::from_raw(o as *mut Obj)));
    }
}

/// `FC4_ADV_OBJ_*`, 0 for NULL.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_kind(o: *const fc4_adv_obj) -> u32 {
    obj_ref(o).map_or(0, Obj::kind)
}

/// `GetGZCLSID`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_class(o: *const fc4_adv_obj) -> u32 {
    obj_ref(o).map_or(0, |x| guard(0, || x.save().class_id()))
}

/// Whether two handles name the same object.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_same(a: *const fc4_adv_obj, b: *const fc4_adv_obj) -> bool {
    match (obj_ref(a), obj_ref(b)) {
        (Some(x), Some(y)) => Rc::ptr_eq(&x.save(), &y.save()),
        _ => false,
    }
}

/// The token the core writes for references to the object.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_token(o: *const fc4_adv_obj) -> u64 {
    obj_ref(o).map_or(0, |x| guard(0, || x.save().host_token()))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_set_token(o: *const fc4_adv_obj, token: u64) -> bool {
    let Some(x) = obj_ref(o) else { return false };
    guard(false, || {
        match x {
            Obj::System(s) => s.set_token(token),
            Obj::Advisor(a) => a.set_token(token),
            Obj::Advice(a) => a.set_token(token),
            Obj::Subject(s) => s.set_token(token),
        }
        true
    })
}

/// `Write` of the object's record (§11).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_write(o: *const fc4_adv_obj, out: *const fc4_ostream) -> bool {
    let (Some(x), Some(s)) = (obj_ref(o), out.as_ref()) else {
        return false;
    };
    if !ostream_ok(s) {
        return false;
    }
    guard(false, || x.save().write(&mut COut(s)))
}

/// `Read` of the object's record (§11). Core objects come back through
/// `fc4_ref::core_handle` (an `fc4_adv_obj*`).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_read(o: *const fc4_adv_obj, inp: *const fc4_istream) -> bool {
    let (Some(x), Some(i)) = (obj_ref(o), inp.as_ref()) else {
        return false;
    };
    if !istream_ok(i) {
        return false;
    }
    guard(false, || {
        x.save().read(&mut CIn {
            stream: i,
            decode: decoder,
        })
    })
}

/// A subject's fields.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_subject_get(o: *const fc4_adv_obj, out: *mut fc4_adv_subject_state) -> bool {
    match (obj_ref(o), out.as_mut()) {
        (Some(Obj::Subject(s)), Some(out)) => guard(false, || {
            *out = fc4_adv_subject_state::from(&s.state());
            true
        }),
        _ => false,
    }
}

/// Sets a subject's fields (`SetLocation` with its clamp when
/// `has_location`, `SetObject`, data1, data2).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_subject_set(o: *const fc4_adv_obj, v: *const fc4_adv_subject_state) -> bool {
    match (obj_ref(o), v.as_ref()) {
        (Some(Obj::Subject(s)), Some(v)) => guard(false, || {
            if v.has_location != 0 {
                s.set_location(v.x, v.z);
            }
            s.set_object(opt(v.object));
            s.set_data1(v.data1);
            s.set_data2(v.data2);
            true
        }),
        _ => false,
    }
}

/// Private production transport for subject slot 13. The caller owns the
/// returned handle; the front is announced through the existing object_created
/// callback, exactly as for advice clones. NULL/non-subject handles fail closed.
#[no_mangle]
pub unsafe extern "C" fn fc4_private_adv_subject_clone(o: *const fc4_adv_obj) -> *mut fc4_adv_obj {
    match obj_ref(o) {
        Some(Obj::Subject(s)) => guard(std::ptr::null_mut(), || Obj::Subject(s.clone_subject()).into_raw()),
        _ => std::ptr::null_mut(),
    }
}

// Private slot transports preserve each Rust method's return and ownership
// effects instead of replaying the aggregate state setter for a single field.
#[no_mangle]
pub unsafe extern "C" fn fc4_private_adv_subject_set_location(o: *const fc4_adv_obj, x: f32, z: f32) -> bool {
    matches!(obj_ref(o), Some(Obj::Subject(s)) if guard(false, || s.set_location(x, z)))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_private_adv_subject_set_object(o: *const fc4_adv_obj, token: u64) -> bool {
    matches!(obj_ref(o), Some(Obj::Subject(s)) if guard(false, || s.set_object(opt(token))))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_private_adv_subject_set_data1(o: *const fc4_adv_obj, value: u32) -> bool {
    matches!(obj_ref(o), Some(Obj::Subject(s)) if guard(false, || s.set_data1(value)))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_private_adv_subject_set_data2(o: *const fc4_adv_obj, value: u32) -> bool {
    matches!(obj_ref(o), Some(Obj::Subject(s)) if guard(false, || s.set_data2(value)))
}

// ---- the MySim agent ------------------------------------------------------------------------------------

/// The agent's `Init` (mysims.md §1.3): false without MySims.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_agent_init(p: *mut fc4_advisors) -> bool {
    with_agent(p, false, |a, h| a.init(&h.sys))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_agent_shutdown(p: *mut fc4_advisors) -> bool {
    with_agent(p, false, |a, _| a.shutdown())
}

/// The agent's `DoMessage` (§3.2): AdviceCommand names the advisor by ID
/// in `unknown`.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_agent_do_message(p: *mut fc4_advisors, m: *const fc4_adv_message) -> bool {
    let Some(m) = m.as_ref() else { return false };
    let m = msg_in(m);
    with_agent(p, false, |a, _| a.do_message(&m))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_agent_get_state(p: *const fc4_advisors, out: *mut fc4_mysims_agent_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_agent(p, false, |a, _| {
        let s = a.state();
        let mut slots = [0i32; 6];
        for (i, v) in s.slots.iter().enumerate() {
            slots[i] = v.unwrap_or(0);
        }
        *o = fc4_mysims_agent_state {
            struct_size: std::mem::size_of::<fc4_mysims_agent_state>() as u32,
            initialised: s.initialised.into(),
            reserved: [0; 3],
            slots,
            last_disaster_type: s.last_disaster_type,
            active_ui_index: s.active_ui_index,
        };
        true
    })
}

/// `GetActiveMySims` (slot 5): the active MySims' advisor IDs.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_active(p: *const fc4_advisors, out: *mut i32, capacity: usize) -> usize {
    with_agent(p, 0, |a, _| {
        let ids: Vec<i32> = a.get_active_mysims().iter().map(|s| s.id()).collect();
        copy_out(&ids, out, capacity)
    })
}

/// The agent's `MoveIn` (slot 6, §4.1): the advisor ID of the MySim that
/// moves in, 0 if all five are active.
#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_move_in(
    p: *mut fc4_advisors,
    name: *const u8,
    name_len: usize,
    is_male: bool,
    sign: u8,
    bio: *const u8,
    bio_len: usize,
    bitmap_id: u32,
    path: *const u8,
    path_len: usize,
    slot: u8,
) -> i32 {
    let (n, b, pa) = (bytes(name, name_len), bytes(bio, bio_len), bytes(path, path_len));
    with_agent(p, 0, |a, _| {
        a.move_in(n, is_male, sign, b, bitmap_id, pa, slot)
            .map_or(0, |s| s.id())
    })
}

/// `MySimLivesHere(b)` (slot 9): the advisor ID, 0 if none.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_lives_here(p: *const fc4_advisors, building: u64) -> i32 {
    with_agent(p, 0, |a, _| a.my_sim_lives_here(building).map_or(0, |s| s.id()))
}

/// `SetActiveUIPanelSim(slot)` (slot 11).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_set_active_ui_panel_sim(p: *mut fc4_advisors, slot: i32) -> bool {
    with_agent(p, false, |a, _| {
        a.set_active_ui_panel_sim(slot);
        true
    })
}

/// `GetActiveUIPanelSim(index&)` (slot 12): the advisor ID (0 = none);
/// `index` gets the vector index, unchanged without one.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_get_active_ui_panel_sim(p: *const fc4_advisors, index: *mut i32) -> i32 {
    with_agent(p, 0, |a, _| {
        let mut i = index.as_ref().copied().unwrap_or(-1);
        let r = a.get_active_ui_panel_sim(&mut i).map_or(0, |s| s.id());
        if let Some(x) = index.as_mut() {
            *x = i;
        }
        r
    })
}

/// The agent's `Write` (`u16 1`, §6.1).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_agent_write(p: *const fc4_advisors, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_agent(p, false, |a, _| a.write(&mut COut(o)))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_mysims_agent_read(p: *mut fc4_advisors, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    with_agent(p, false, |a, _| {
        a.read(&mut CIn {
            stream: i,
            decode: decoder,
        })
    })
}

// ---- one MySim (by advisor ID) -------------------------------------------------------------------------

fn key_out(k: fc4_core::stream::ResKey) -> fc4_res_key {
    fc4_res_key {
        type_id: k.type_id,
        group: k.group,
        instance: k.instance,
    }
}

#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_get_state(p: *const fc4_advisors, id: i32, out: *mut fc4_mysim_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_sim(p, id, false, |m| {
        let s = m.state();
        *o = fc4_mysim_state {
            struct_size: std::mem::size_of::<fc4_mysim_state>() as u32,
            state: s.state,
            slot: s.slot,
            wealth: s.wealth,
            sign: s.sign,
            is_male: s.is_male.into(),
            age: s.age,
            birth_month: s.birth_month,
            retired: s.retired.into(),
            neighbor_job: s.neighbor_job.into(),
            commute_check: s.commute_check.into(),
            lua_table: s.lua_table.into(),
            unnamed_flag: s.unnamed_flag.into(),
            eq_index: s.eq_index,
            hq_index: s.hq_index,
            generation: s.generation,
            home_cell_x: s.home_cell.0,
            home_cell_z: s.home_cell.1,
            neighbor_cell: [s.neighbor_cell.0, s.neighbor_cell.1],
            commute_cell: [s.commute_cell.0, s.commute_cell.1],
            jobless_days: s.jobless_days,
            bitmap_id: s.bitmap_id,
            last_commute_time: s.last_commute_time,
            last_local_disaster: s.last_local_disaster,
            home_position: s.home_position,
            eq: s.eq,
            hq: s.hq,
            eq_delta: s.eq_delta,
            hq_delta: s.hq_delta,
            eq_ring: s.eq_ring,
            hq_ring: s.hq_ring,
            vehicle_key: key_out(s.vehicle_key),
            car_key: key_out(s.car_key),
            pedestrian_key: key_out(s.pedestrian_key),
            unnamed_pair: [s.unnamed_pair.0, s.unnamed_pair.1],
            last_player_trip_day: s.last_player_trip_day,
            last_trip_update_day: s.last_trip_update_day,
            local_area_size: s.local_area_size,
            pylon_units: s.pylon.0,
            pylon_running: s.pylon.2,
            reserved: [0; 2],
            local_group_count: s.local.len() as u32,
            pylon_elapsed: s.pylon.1,
            residence: s.residence.unwrap_or(0),
            job: s.job.unwrap_or(0),
            dispatch: s.dispatch.unwrap_or(0),
            dispatch_tool: s.dispatch_tool.unwrap_or(0),
        };
        true
    })
}

/// The MySim's bio (`which` 0) or portrait path (1), NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_text(
    p: *const fc4_advisors,
    id: i32,
    which: u32,
    out: *mut c_char,
    capacity: usize,
) -> usize {
    with_sim(p, id, 0, |m| {
        let s = m.state();
        copy_text(if which == 0 { &s.bio } else { &s.bitmap_path }, out, capacity)
    })
}

/// Slot 13 `SetResidence(b)` (0 = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_set_residence(p: *mut fc4_advisors, id: i32, building: u64) -> bool {
    with_sim(p, id, false, |m| m.set_residence(opt(building)))
}

/// Slot 6 `ChangeResidence(b)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_change_residence(p: *mut fc4_advisors, id: i32, building: u64) -> bool {
    with_sim(p, id, false, |m| m.change_residence(building))
}

/// Slot 5 `FindNewResidence` (always false, §4.7).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_find_new_residence(p: *mut fc4_advisors, id: i32) -> bool {
    with_sim(p, id, false, |m| m.find_new_residence())
}

/// Slot 7 `TestForResidence(b, filter, matchWealth)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_test_for_residence(
    p: *const fc4_advisors,
    id: i32,
    building: u64,
    filter: bool,
    match_wealth: bool,
) -> bool {
    with_h(p, None, |h| {
        let lives = h.agent.as_ref().and_then(|a| a.my_sim_lives_here(building)).is_some();
        let a = h.sys.get_advisor(id)?;
        MySim::with(&a, |m| m.test_for_residence(building, filter, match_wealth, lives))
    })
    .unwrap_or(false)
}

/// Slot 8 `ChangeJob(b)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_change_job(p: *mut fc4_advisors, id: i32, building: u64) -> bool {
    with_sim(p, id, false, |m| m.change_job(building))
}

/// Slot 9 `DeleteFromGame` (true).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_delete_from_game(p: *mut fc4_advisors, id: i32) -> bool {
    with_sim(p, id, false, |m| m.delete_from_game())
}

/// Slot 40 `AnnounceMoveIn`.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_announce_move_in(p: *mut fc4_advisors, id: i32) -> bool {
    with_sim(p, id, false, |m| {
        m.announce_move_in();
        true
    })
}

/// Slot 41 `DoScriptCommand(cmd)` (§4.8).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_do_script_command(p: *mut fc4_advisors, id: i32, command: u32) -> bool {
    with_sim(p, id, false, |m| {
        m.do_script_command(command);
        true
    })
}

/// `UpdateVariables` (§4.5).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_update_variables(p: *mut fc4_advisors, id: i32) -> bool {
    with_sim(p, id, false, |m| {
        m.update_variables();
        true
    })
}

/// Slot 50 `DistanceToClosest(group)` (`0xFFFFFFFF` = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_distance_to_closest(p: *const fc4_advisors, id: i32, group: u32) -> u32 {
    with_sim(p, id, mysims::NO_DISTANCE, |m| m.distance_to_closest(group))
}

/// Slot 51 `DistanceToClosestSpecial(type)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_distance_to_closest_special(
    p: *const fc4_advisors,
    id: i32,
    building_type: u32,
) -> u32 {
    with_sim(p, id, mysims::NO_DISTANCE, |m| {
        m.distance_to_closest_special(building_type)
    })
}

/// Slots 52 / 53 (`hospital`): the grade as a fraction (the SDK's result
/// pointer, §7.4).
#[no_mangle]
pub unsafe extern "C" fn fc4_mysim_local_grade(p: *const fc4_advisors, id: i32, group: u32, hospital: bool) -> f32 {
    with_sim(p, id, 0.0, |m| m.local_grade(group, hospital))
}

// ---- weak handles and per-object calls (glue-advisors-activate4) ----------------------------------------
//
// A host wrapper keeps a strong `fc4_adv_obj` only while the game holds a
// GZCOM reference on it and an `fc4_adv_weak` otherwise; `object_dropped`
// tells it when the core object is gone (advisors.md §12.2-§12.4 fronts).

#[repr(C)]
pub struct fc4_adv_weak {
    _private: [u8; 0],
}

enum WeakObj {
    System(std::rc::Weak<AdvisorSystem>),
    Advisor(std::rc::Weak<Advisor>),
    Advice(std::rc::Weak<Advice>),
    Subject(std::rc::Weak<AdviceSubject>),
}

/// A new weak handle on the object (NULL for NULL); does not keep it alive.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_obj_downgrade(o: *const fc4_adv_obj) -> *mut fc4_adv_weak {
    let Some(x) = obj_ref(o) else { return std::ptr::null_mut() };
    let w = match x {
        Obj::System(s) => WeakObj::System(Rc::downgrade(s)),
        Obj::Advisor(a) => WeakObj::Advisor(Rc::downgrade(a)),
        Obj::Advice(a) => WeakObj::Advice(Rc::downgrade(a)),
        Obj::Subject(s) => WeakObj::Subject(Rc::downgrade(s)),
    };
    Box::into_raw(Box::new(w)) as *mut fc4_adv_weak
}

/// A new strong handle on the object, NULL once it was dropped.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_weak_upgrade(w: *const fc4_adv_weak) -> *mut fc4_adv_obj {
    let Some(w) = (w as *const WeakObj).as_ref() else { return std::ptr::null_mut() };
    let o = match w {
        WeakObj::System(s) => s.upgrade().map(Obj::System),
        WeakObj::Advisor(a) => a.upgrade().map(Obj::Advisor),
        WeakObj::Advice(a) => a.upgrade().map(Obj::Advice),
        WeakObj::Subject(s) => s.upgrade().map(Obj::Subject),
    };
    o.map_or(std::ptr::null_mut(), Obj::into_raw)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_adv_weak_destroy(w: *mut fc4_adv_weak) {
    if !w.is_null() {
        guard((), || drop(Box::from_raw(w as *mut WeakObj)));
    }
}

unsafe fn advice_of<'a>(o: *const fc4_adv_obj) -> Option<&'a Rc<Advice>> {
    match obj_ref(o) {
        Some(Obj::Advice(a)) => Some(a),
        _ => None,
    }
}

unsafe fn advisor_of<'a>(o: *const fc4_adv_obj) -> Option<&'a Rc<Advisor>> {
    match obj_ref(o) {
        Some(Obj::Advisor(a)) => Some(a),
        _ => None,
    }
}

unsafe fn subject_of(o: *const fc4_adv_obj) -> Option<Rc<AdviceSubject>> {
    match obj_ref(o) {
        Some(Obj::Subject(s)) => Some(s.clone()),
        _ => None,
    }
}

/// An advice's state (next_day, expiry and last_month are the owning
/// advisor's and read 0 here).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_get(o: *const fc4_adv_obj, out: *mut fc4_advice_state) -> bool {
    let (Some(a), Some(o)) = (advice_of(o), out.as_mut()) else { return false };
    guard(false, || {
        let s = a.state();
        *o = fc4_advice_state {
            struct_size: std::mem::size_of::<fc4_advice_state>() as u32,
            guid: s.guid,
            class_id: a.class_id(),
            flags: s.flags,
            advisor_id: s.advisor_id,
            table_index: s.table_index,
            priority: s.priority,
            frequency: s.frequency,
            timeout: s.timeout,
            mood: s.mood,
            event_id: s.event_id,
            command_id: s.command_id,
            trigger: s.trigger,
            nd_type: s.nd_type,
            serial: a.serial(),
            effect_count: s.effects.len() as u32,
            loaded: s.loaded.into(),
            active: a.is_active().into(),
            has_subject: s.subject.is_some().into(),
            has_trigger_text: s.trigger_text.is_some().into(),
            next_day: 0,
            expiry: 0,
            last_month: 0,
            reserved: 0,
            reserved2: 0,
            token: a.host_token(),
        };
        true
    })
}

/// Slot 3 `Init(tableIndex)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_init(o: *const fc4_adv_obj, table_index: i32) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || a.init(table_index)))
}

/// Slot 4 `Shutdown`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_shutdown(o: *const fc4_adv_obj) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || a.shutdown()))
}

/// Slot 5 `Clone`: a new handle on the copy (its wrapper came through
/// `object_created`).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_clone(o: *const fc4_adv_obj) -> *mut fc4_adv_obj {
    advice_of(o).map_or(std::ptr::null_mut(), |a| {
        guard(std::ptr::null_mut(), || Obj::Advice(a.clone_advice()).into_raw())
    })
}

/// Slot 8 `GetTrigger`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_get_trigger(o: *const fc4_adv_obj) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || a.get_trigger()))
}

/// Slot 12 `SetAdvisorID`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_set_advisor_id(o: *const fc4_adv_obj, id: i32) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || {
        a.set_advisor_id(id);
        true
    }))
}

/// Slot 14 `SetFlags(mask, on)` (§6.9).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_set_flags(o: *const fc4_adv_obj, mask: u32, on: bool) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || a.set_flags(mask, on)))
}

/// Slot 23 `NotifyGlobalEffectTriggerChanged`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_notify_global_effects(o: *const fc4_adv_obj) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || {
        a.notify_global_effects();
        true
    }))
}

/// Slot 24 `SetEventSubject` (`subject` NULL = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_set_event_subject(o: *const fc4_adv_obj, subject: *const fc4_adv_obj) -> bool {
    let s = subject_of(subject);
    advice_of(o).is_some_and(|a| guard(false, || a.set_event_subject(s)))
}

/// Slot 25 `GetEventSubject`: a new handle, NULL for none.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_event_subject(o: *const fc4_adv_obj) -> *mut fc4_adv_obj {
    advice_of(o).map_or(std::ptr::null_mut(), |a| {
        guard(std::ptr::null_mut(), || {
            a.event_subject().map_or(std::ptr::null_mut(), |s| Obj::Subject(s).into_raw())
        })
    })
}

/// Slot 28 `UpdateText` (the text rebuild goes to the host's `rebuild_text`).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_update_text(o: *const fc4_adv_obj) -> bool {
    advice_of(o).is_some_and(|a| guard(false, || a.update_text()))
}

/// The trigger text (slot 9), NUL-terminated; the full length, 0 for none.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advice_trigger_text(o: *const fc4_adv_obj, out: *mut c_char, capacity: usize) -> usize {
    advice_of(o).map_or(0, |a| guard(0, || copy_text(&a.trigger_text().unwrap_or_default(), out, capacity)))
}

/// An advisor's state (as `fc4_advisors_advisor_state`).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_id(o: *const fc4_adv_obj) -> i32 {
    advisor_of(o).map_or(0, |a| guard(0, || a.id()))
}

/// Slot 3 `Init(tableIndex)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_init(o: *const fc4_adv_obj, table_index: i32) -> bool {
    advisor_of(o).is_some_and(|a| guard(false, || a.init(table_index)))
}

/// Slot 4 `Shutdown`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_shutdown(o: *const fc4_adv_obj) -> bool {
    advisor_of(o).is_some_and(|a| guard(false, || a.shutdown()))
}

/// The caption (`which` 0, slot 6) or personal name (1, slot 8),
/// NUL-terminated; the full length.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_text(o: *const fc4_adv_obj, which: u32, out: *mut c_char, capacity: usize) -> usize {
    advisor_of(o).map_or(0, |a| {
        guard(0, || {
            let t = if which == 0 { a.caption() } else { a.personal_name() };
            copy_text(&t, out, capacity)
        })
    })
}

/// Slots 7 / 9 `SetName` / `SetPersonalName`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_set_text(o: *const fc4_adv_obj, which: u32, text: *const u8, len: usize) -> bool {
    let Some(a) = advisor_of(o) else { return false };
    let v: &[u8] = if text.is_null() || len == 0 { &[] } else { std::slice::from_raw_parts(text, len) };
    guard(false, || {
        if which == 0 {
            a.set_caption(v);
        } else {
            a.set_personal_name(v);
        }
        true
    })
}

/// Slots 10/11: the active map's entries in key order are the host's (it
/// orders by its own advice pointers); this returns entry `i` of the core's
/// list as a new handle and its value (the advice's index), NULL past the end.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_current_advice(o: *const fc4_adv_obj, i: u32, value: *mut i32) -> *mut fc4_adv_obj {
    advisor_of(o).map_or(std::ptr::null_mut(), |a| {
        guard(std::ptr::null_mut(), || match a.current_advices().into_iter().nth(i as usize) {
            Some((adv, idx)) => {
                if let Some(v) = value.as_mut() {
                    *v = idx as i32;
                }
                Obj::Advice(adv).into_raw()
            }
            None => std::ptr::null_mut(),
        })
    })
}

/// Slot 12 `GetAdviceByGUID`: a new handle, NULL for none.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_advice_by_guid(o: *const fc4_adv_obj, guid: u32) -> *mut fc4_adv_obj {
    advisor_of(o).map_or(std::ptr::null_mut(), |a| {
        guard(std::ptr::null_mut(), || a.advice_by_guid(guid).map_or(std::ptr::null_mut(), |x| Obj::Advice(x).into_raw()))
    })
}

/// Slot 13 `GetAdviceBySessionID`: a new handle, NULL for none.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_advice_by_session(o: *const fc4_adv_obj, session: i32) -> *mut fc4_adv_obj {
    advisor_of(o).map_or(std::ptr::null_mut(), |a| {
        guard(std::ptr::null_mut(), || {
            a.advice_by_session(session).map_or(std::ptr::null_mut(), |x| Obj::Advice(x).into_raw())
        })
    })
}

/// Slot 14 `Simulate(kind)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_simulate(o: *const fc4_adv_obj, kind: i32) -> bool {
    advisor_of(o).is_some_and(|a| guard(false, || a.simulate(kind)))
}

/// Slot 18 `PushEvent` (`subject` NULL = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_push_event(o: *const fc4_adv_obj, event_id: u32, subject: *const fc4_adv_obj) -> bool {
    let s = subject_of(subject);
    advisor_of(o).is_some_and(|a| guard(false, || a.push_event(event_id, s)))
}

/// Slot 19 `CommandWouldTrigger`.
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_command_would_trigger(o: *const fc4_adv_obj, command: u32) -> bool {
    advisor_of(o).is_some_and(|a| guard(false, || a.command_would_trigger(command)))
}

/// Slot 20 `RetireAdvice` (`advice` NULL = none).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_retire_advice(o: *const fc4_adv_obj, advice: *const fc4_adv_obj) -> bool {
    let x = advice_of(advice).cloned();
    advisor_of(o).is_some_and(|a| guard(false, || a.retire_advice(x.as_ref())))
}

/// The advisor's `DoMessage` (§5.4).
#[no_mangle]
pub unsafe extern "C" fn fc4_adv_advisor_do_message(o: *const fc4_adv_obj, m: *const fc4_adv_message) -> bool {
    let (Some(a), Some(m)) = (advisor_of(o), m.as_ref()) else { return false };
    let m = msg_in(m);
    guard(false, || a.do_message(&m))
}
