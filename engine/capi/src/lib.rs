// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Stable C ABI over `fc4-core`. The contract is `include/fc4.h`; keep the two
//! in sync. This is the only crate of the engine that contains `unsafe` code,
//! and only to cross the FFI boundary.
//!
//! Panics: every entry point runs inside `catch_unwind` and returns a
//! documented fallback value; release builds additionally use
//! `panic = "abort"`, so unwinding can never reach C/C++ frames.

#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};

use fc4_core::host::{AgentDelivery, Broadcast, Clock, TimerSource};
use fc4_core::julian;
use fc4_core::save::{LoadError, RECORD_LAYOUT_V4, SAVE_VERSION};
use fc4_core::{AgentHandle, Message, Simulator, Tuning, Turbo};

pub const FC4_ABI_VERSION: u32 = 84;
pub const FC4_OK: i32 = 0;
pub const FC4_ERR_UNSUPPORTED_VERSION: i32 = -1;
pub const FC4_ERR_TRUNCATED: i32 = -2;
pub const FC4_ERR_INVALID_ARGUMENT: i32 = -3;
pub const FC4_ERR_PANIC: i32 = -100;

/// Opaque handle type. A `*mut fc4_sim` is really a `*const Simulator`.
#[repr(C)]
pub struct fc4_sim {
    _private: [u8; 0],
}

pub type fc4_agent = u64;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_message {
    pub type_: u32,
    pub reserved: u32,
    pub data1: i64,
    pub data2: i64,
    pub data3: i64,
}

impl From<&Message> for fc4_message {
    fn from(m: &Message) -> Self {
        fc4_message {
            type_: m.msg_type,
            reserved: 0,
            data1: m.data1,
            data2: m.data2,
            data3: m.data3,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    pub now_ms: Option<unsafe extern "C" fn(user: *mut c_void) -> u32>,
    pub deliver: Option<
        unsafe extern "C" fn(user: *mut c_void, sim: *mut fc4_sim, agent: fc4_agent, msg: *const fc4_message) -> bool,
    >,
    pub add_ref: Option<unsafe extern "C" fn(user: *mut c_void, agent: fc4_agent)>,
    pub release: Option<unsafe extern "C" fn(user: *mut c_void, agent: fc4_agent)>,
    pub post: Option<unsafe extern "C" fn(user: *mut c_void, msg: *const fc4_message)>,
}

#[repr(C)]
pub struct fc4_tuning {
    pub ms_per_day: *const u32,
    pub ms_per_day_count: usize,
    pub animation_dilation: *const f32,
    pub animation_dilation_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_date {
    pub julian: i32,
    pub year: i32,
    pub month: i32,
    pub day_of_month: i32,
    pub day_of_year: i32,
    pub weekday: i32,
}

impl From<julian::DateFields> for fc4_date {
    fn from(d: julian::DateFields) -> Self {
        fc4_date {
            julian: d.julian,
            year: d.year,
            month: d.month,
            day_of_month: d.day_of_month,
            day_of_year: d.day_of_year,
            weekday: d.weekday,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_state {
    pub struct_size: u32,
    pub initialized: u8,
    pub in_tick: u8,
    pub city_established: u8,
    pub speed_change_enabled: u8,
    pub pause_count: i32,
    pub hidden_pause_count: i32,
    pub emergency_pause_count: i32,
    pub sim_speed: i32,
    pub sim_time: i32,
    pub frame_budget_ms: u32,
    pub gate_level: u32,
    pub ms_per_day: u32,
    pub target_fps: i32,
    pub julian_day: i32,
    pub ms_per_day_table: [u32; 3],
    pub animation_dilation: [f32; 5],
    pub level_counters: [u32; 11],
    pub level_fired: [u8; 11],
    pub reserved: u8,
    pub agent_counts: [u32; 11],
}

const _: () = {
    assert!(std::mem::size_of::<fc4_message>() == 32);
    assert!(std::mem::size_of::<fc4_date>() == 24);
    assert!(std::mem::size_of::<fc4_state>() == 180);
    assert!(std::mem::size_of::<fc4_debug>() == 76);
    assert!(std::mem::size_of::<fc4_agent_info>() == 16);
    assert!(std::mem::size_of::<fc4_timer_source>() == if cfg!(target_pointer_width = "32") { 32 } else { 48 });
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_debug {
    pub struct_size: u32,
    pub interval_elapsed_ms: u32,
    pub slice_elapsed_ms: u32,
    pub ms_per_day: u32,
    pub ready: u8,
    pub one_shot_or_emergency_pending: u8,
    pub hours_complete: u8,
    pub day_level_done: u8,
    pub tomorrow_starts_week: u8,
    pub tomorrow_starts_month: u8,
    pub tomorrow_starts_year: u8,
    pub day_gate_open: u8,
    pub blocked_by_week: u8,
    pub blocked_by_month: u8,
    pub blocked_by_year: u8,
    pub reserved: u8,
    pub cursors: [i32; 11],
    pub idle_cursor: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_agent_info {
    pub agent: fc4_agent,
    pub flags: u32,
    pub enabled: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_timer_source {
    pub struct_size: u32,
    pub user: *mut c_void,
    pub qpc: Option<unsafe extern "C" fn(user: *mut c_void) -> u64>,
    pub tsc: Option<unsafe extern "C" fn(user: *mut c_void) -> u64>,
    pub publish: Option<unsafe extern "C" fn(user: *mut c_void, cycles_to_ms: f32, ms_to_cycles: f32)>,
    pub qpc_frequency: u64,
}

struct CTimers(fc4_timer_source);

impl TimerSource for CTimers {
    fn qpc(&self) -> u64 {
        // SAFETY: presence checked at installation; the creator promised validity.
        self.0.qpc.map_or(0, |f| unsafe { f(self.0.user) })
    }
    fn qpc_frequency(&self) -> u64 {
        self.0.qpc_frequency
    }
    fn tsc(&self) -> u64 {
        // SAFETY: as above.
        self.0.tsc.map_or(0, |f| unsafe { f(self.0.user) })
    }
    fn publish(&self, cycles_to_ms: f32, ms_to_cycles: f32) {
        if let Some(f) = self.0.publish {
            // SAFETY: as above.
            unsafe { f(self.0.user, cycles_to_ms, ms_to_cycles) }
        }
    }
}

pub type fc4_name_callback = Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char, name_len: usize)>;

// ---- host adapter -----------------------------------------------------------

struct CHost(fc4_host);

impl Clock for CHost {
    fn now_ms(&self) -> u32 {
        match self.0.now_ms {
            // SAFETY: the creator of the simulator promised valid callbacks.
            Some(f) => unsafe { f(self.0.user) },
            None => 0,
        }
    }
}

impl AgentDelivery for CHost {
    fn deliver(&self, sim: &Simulator, agent: AgentHandle, msg: &Message) -> bool {
        match self.0.deliver {
            Some(f) => {
                let m = fc4_message::from(msg);
                let handle = sim as *const Simulator as *mut fc4_sim;
                // SAFETY: valid callback; `m` outlives the call.
                unsafe { f(self.0.user, handle, agent, &m) }
            }
            None => true,
        }
    }
    fn add_ref(&self, agent: AgentHandle) {
        if let Some(f) = self.0.add_ref {
            // SAFETY: valid callback.
            unsafe { f(self.0.user, agent) }
        }
    }
    fn release(&self, agent: AgentHandle) {
        if let Some(f) = self.0.release {
            // SAFETY: valid callback.
            unsafe { f(self.0.user, agent) }
        }
    }
}

impl Broadcast for CHost {
    fn post(&self, msg: &Message) {
        if let Some(f) = self.0.post {
            let m = fc4_message::from(msg);
            // SAFETY: valid callback; `m` outlives the call.
            unsafe { f(self.0.user, &m) }
        }
    }
}

// ---- helpers ------------------------------------------------------------------

fn guard<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

/// SAFETY: `sim` must be NULL or a live pointer from `fc4_sim_create`.
unsafe fn sim_ref<'a>(sim: *const fc4_sim) -> Option<&'a Simulator> {
    (sim as *const Simulator).as_ref()
}

fn with_sim<R>(sim: *const fc4_sim, fallback: R, f: impl FnOnce(&Simulator) -> R) -> R {
    // SAFETY: documented precondition of every entry point.
    match unsafe { sim_ref(sim) } {
        Some(s) => guard(fallback, || f(s)),
        None => fallback,
    }
}

/// SAFETY: `ptr` must be NULL (with `len == 0`) or valid for `len` bytes.
unsafe fn str_arg(ptr: *const c_char, len: usize) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
    String::from_utf8_lossy(bytes).into_owned()
}

/// SAFETY: as `str_arg`, for typed slices.
unsafe fn slice_arg<'a, T>(ptr: *const T, len: usize) -> Option<&'a [T]> {
    if ptr.is_null() {
        None
    } else if len == 0 {
        Some(&[])
    } else {
        Some(std::slice::from_raw_parts(ptr, len))
    }
}

// ---- library --------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn fc4_abi_version() -> u32 {
    FC4_ABI_VERSION
}

/// `derive(S, K)` of foundations §6.3 (fc4.h: the session seed).
#[no_mangle]
pub extern "C" fn fc4_seed_derive(session_seed: u32, k: u32) -> u32 {
    fc4_core::rng::derive_seed(session_seed, k)
}

// ---- lifetime -------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_create(host: *const fc4_host) -> *mut fc4_sim {
    guard(std::ptr::null_mut(), || {
        let Some(h) = host.as_ref() else {
            return std::ptr::null_mut();
        };
        if (h.struct_size as usize) < std::mem::size_of::<fc4_host>() || h.now_ms.is_none() {
            return std::ptr::null_mut();
        }
        let sim = Box::new(Simulator::new(Box::new(CHost(*h))));
        Box::into_raw(sim) as *mut fc4_sim
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_destroy(sim: *mut fc4_sim) {
    if sim.is_null() {
        return;
    }
    guard((), || drop(Box::from_raw(sim as *mut Simulator)));
}

// ---- lifecycle ----------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_init(sim: *mut fc4_sim, tuning: *const fc4_tuning) -> bool {
    let t = match tuning.as_ref() {
        None => Tuning::default(),
        Some(t) => Tuning {
            ms_per_day: slice_arg(t.ms_per_day, t.ms_per_day_count).map(<[u32]>::to_vec),
            animation_dilation: slice_arg(t.animation_dilation, t.animation_dilation_count).map(<[f32]>::to_vec),
        },
    };
    with_sim(sim, false, |s| s.init(&t))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_shutdown(sim: *mut fc4_sim) -> bool {
    with_sim(sim, false, Simulator::shutdown)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_is_initialized(sim: *const fc4_sim) -> bool {
    with_sim(sim, false, Simulator::is_initialized)
}

// ---- per-frame ----------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_on_tick(sim: *mut fc4_sim) -> bool {
    with_sim(sim, true, Simulator::on_tick)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_simulate_time_passage(sim: *mut fc4_sim) {
    with_sim(sim, (), Simulator::simulate_time_passage)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_simulate_agents(sim: *mut fc4_sim) {
    with_sim(sim, (), Simulator::simulate_agents)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_simulate_idle_time(sim: *mut fc4_sim) {
    with_sim(sim, (), Simulator::simulate_idle_time)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_begin_new_interval(sim: *mut fc4_sim, level: u32) {
    with_sim(sim, (), |s| s.begin_new_interval(level))
}

// ---- pause and speed --------------------------------------------------------------

macro_rules! bool_entry {
    ($name:ident, $ptr:ty, $method:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(sim: $ptr) -> bool {
            with_sim(sim, false, Simulator::$method)
        }
    };
}

bool_entry!(fc4_sim_pause, *mut fc4_sim, pause);
bool_entry!(fc4_sim_resume, *mut fc4_sim, resume);
bool_entry!(fc4_sim_hidden_pause, *mut fc4_sim, hidden_pause);
bool_entry!(fc4_sim_hidden_resume, *mut fc4_sim, hidden_resume);
bool_entry!(fc4_sim_on_post_city_init, *mut fc4_sim, on_post_city_init);
bool_entry!(fc4_sim_emergency_pause, *mut fc4_sim, emergency_pause);
bool_entry!(fc4_sim_emergency_resume, *mut fc4_sim, emergency_resume);
bool_entry!(fc4_sim_is_paused, *const fc4_sim, is_paused);
bool_entry!(fc4_sim_is_hidden_paused, *const fc4_sim, is_hidden_paused);
bool_entry!(fc4_sim_is_emergency_paused, *const fc4_sim, is_emergency_paused);
bool_entry!(fc4_sim_is_any_paused, *const fc4_sim, is_any_paused);
bool_entry!(fc4_sim_remove_all_agents, *mut fc4_sim, remove_all_agents);
bool_entry!(fc4_sim_is_city_established, *const fc4_sim, is_city_established);

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_sim_speed(sim: *const fc4_sim) -> i32 {
    with_sim(sim, 0, Simulator::sim_speed)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_sim_speed(sim: *mut fc4_sim, speed: i32) -> bool {
    with_sim(sim, false, |s| s.set_sim_speed(speed))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_sim_time(sim: *const fc4_sim) -> i32 {
    with_sim(sim, 0, Simulator::sim_time)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_sim_time(sim: *mut fc4_sim, time: i32) -> bool {
    with_sim(sim, false, |s| s.set_sim_time(time))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_max_milliseconds_per_tick(sim: *mut fc4_sim, ms: u32) -> bool {
    with_sim(sim, false, |s| s.set_max_milliseconds_per_tick(ms))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_animation_time_dilation(sim: *const fc4_sim) -> f32 {
    with_sim(sim, 1.0, Simulator::animation_time_dilation)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_city_established(sim: *mut fc4_sim, established: bool) -> bool {
    with_sim(sim, false, |s| s.set_city_established(established))
}

// ---- agents -----------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_add_agent(
    sim: *mut fc4_sim,
    agent: fc4_agent,
    level: u32,
    name: *const c_char,
    name_len: usize,
    flags: u32,
) -> bool {
    let name = guard(String::new(), || str_arg(name, name_len));
    with_sim(sim, false, |s| s.add_agent(agent, level, &name, flags))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_remove_agent(sim: *mut fc4_sim, agent: fc4_agent, level: u32) -> bool {
    with_sim(sim, false, |s| s.remove_agent(agent, level))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_remove_agent_all_levels(sim: *mut fc4_sim, agent: fc4_agent) -> bool {
    with_sim(sim, false, |s| s.remove_agent_all_levels(agent))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_remove_all_agents_at(sim: *mut fc4_sim, level: u32) -> bool {
    with_sim(sim, false, |s| s.remove_all_agents_at(level))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_agent_enabled(sim: *const fc4_sim, name: *const c_char, name_len: usize) -> bool {
    let name = guard(String::new(), || str_arg(name, name_len));
    with_sim(sim, false, |s| s.get_agent_enabled(&name))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_agent_enabled(
    sim: *mut fc4_sim,
    name: *const c_char,
    name_len: usize,
    enabled: bool,
) -> bool {
    let name = guard(String::new(), || str_arg(name, name_len));
    with_sim(sim, false, |s| s.set_agent_enabled(&name, enabled))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_enumerate_agents(
    sim: *const fc4_sim,
    cb: fc4_name_callback,
    ctx: *mut c_void,
) -> usize {
    // Collect first so the callback may re-enter the simulator.
    let names = with_sim(sim, Vec::new(), Simulator::enumerate_agent_names);
    if let Some(f) = cb {
        for n in &names {
            f(ctx, n.as_ptr() as *const c_char, n.len());
        }
    }
    names.len()
}

// ---- date ----------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_julian_day(sim: *const fc4_sim) -> i32 {
    with_sim(sim, julian::JULIAN_2000_01_01, Simulator::julian_day)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_date(sim: *const fc4_sim, out: *mut fc4_date) -> bool {
    if out.is_null() {
        return false;
    }
    match with_sim(sim, None, |s| Some(s.date_fields())) {
        Some(d) => {
            *out = d.into();
            true
        }
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn fc4_julian_from_civil(year: i32, month: i32, day: i32) -> i32 {
    guard(0, || julian::julian_from_civil(year, month, day))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_date_from_julian(julian_day: i32, out: *mut fc4_date) {
    if let Some(o) = out.as_mut() {
        *o = guard(fc4_date::default(), || julian::date_fields(julian_day).into());
    }
}

// ---- persistence ---------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_save(sim: *const fc4_sim, buffer: *mut u8, capacity: usize) -> usize {
    let bytes = with_sim(sim, Vec::new(), Simulator::save);
    if !buffer.is_null() && capacity >= bytes.len() {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
    }
    bytes.len()
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_load(sim: *mut fc4_sim, buffer: *const u8, length: usize) -> i32 {
    if sim.is_null() {
        return FC4_ERR_INVALID_ARGUMENT;
    }
    let Some(bytes) = slice_arg(buffer, length) else {
        return FC4_ERR_INVALID_ARGUMENT;
    };
    with_sim(sim, FC4_ERR_PANIC, |s| match s.load(bytes) {
        Ok(()) => FC4_OK,
        Err(LoadError::UnsupportedVersion(_)) => FC4_ERR_UNSUPPORTED_VERSION,
        Err(LoadError::Truncated) => FC4_ERR_TRUNCATED,
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_record_layout(version: u16, widths: *mut *const u8) -> usize {
    if version != SAVE_VERSION {
        return 0;
    }
    if let Some(w) = widths.as_mut() {
        *w = RECORD_LAYOUT_V4.as_ptr();
    }
    RECORD_LAYOUT_V4.len()
}

// ---- cheats ----------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_handle_cheat(
    sim: *mut fc4_sim,
    cheat_id: u32,
    text: *const c_char,
    text_len: usize,
) -> bool {
    let text = guard(String::new(), || str_arg(text, text_len));
    with_sim(sim, false, |s| s.handle_cheat(cheat_id, &text))
}

// ---- inspection -------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_state(sim: *const fc4_sim, out: *mut fc4_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    if (o.struct_size as usize) < std::mem::size_of::<fc4_state>() {
        return false;
    }
    let Some(snap) = with_sim(sim, None, |s| Some(s.snapshot())) else {
        return false;
    };
    let mut st = fc4_state {
        struct_size: std::mem::size_of::<fc4_state>() as u32,
        ..Default::default()
    };
    st.initialized = snap.initialized.into();
    st.in_tick = snap.in_tick.into();
    st.city_established = snap.city_established.into();
    st.speed_change_enabled = snap.speed_change_enabled.into();
    st.pause_count = snap.pause_count;
    st.hidden_pause_count = snap.hidden_pause_count;
    st.emergency_pause_count = snap.emergency_pause_count;
    st.sim_speed = snap.sim_speed;
    st.sim_time = snap.sim_time;
    st.frame_budget_ms = snap.frame_budget_ms;
    st.gate_level = snap.gate_level;
    st.ms_per_day = snap.ms_per_day;
    st.target_fps = snap.target_fps;
    st.julian_day = snap.julian;
    st.ms_per_day_table = snap.ms_per_day_table;
    st.animation_dilation = snap.animation_dilation;
    st.level_counters = snap.level_counters;
    for l in 0..11 {
        st.level_fired[l] = snap.level_fired[l].into();
        st.agent_counts[l] = snap.agent_counts[l] as u32;
    }
    *o = st;
    true
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_timer_source(
    sim: *mut fc4_sim,
    source: *const fc4_timer_source,
    initial_ms_per_cycle: f32,
) -> bool {
    let Some(src) = source.as_ref() else { return false };
    if (src.struct_size as usize) < std::mem::size_of::<fc4_timer_source>() || src.qpc.is_none() || src.tsc.is_none() {
        return false;
    }
    let copy = *src;
    with_sim(sim, false, |s| {
        s.set_timer_source(Box::new(CTimers(copy)), initial_ms_per_cycle)
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_calibrate_tick(sim: *mut fc4_sim) {
    with_sim(sim, (), Simulator::calibrate_tick)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_calibration(
    sim: *const fc4_sim,
    cycles_to_ms: *mut f32,
    ms_to_cycles: *mut f32,
    samples: *mut u64,
) -> bool {
    let Some(cal) = with_sim(sim, None, Simulator::calibration) else {
        return false;
    };
    if let Some(p) = cycles_to_ms.as_mut() {
        *p = cal.cycles_to_ms();
    }
    if let Some(p) = ms_to_cycles.as_mut() {
        *p = cal.ms_to_cycles();
    }
    if let Some(p) = samples.as_mut() {
        *p = cal.samples();
    }
    true
}

// ---- turbo clock (tooling, not simulator.md; fc4_core::Turbo) ---------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_turbo(sim: *mut fc4_sim, on: bool, frame_ms: u32, until_julian: i32) -> bool {
    with_sim(sim, false, |s| {
        s.set_turbo(Turbo {
            on,
            frame_ms,
            until_julian,
        })
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_turbo(
    sim: *const fc4_sim,
    on: *mut bool,
    frame_ms: *mut u32,
    until_julian: *mut i32,
) -> bool {
    let Some(t) = with_sim(sim, None, |s| Some(s.turbo())) else {
        return false;
    };
    if let Some(p) = on.as_mut() {
        *p = t.on;
    }
    if let Some(p) = frame_ms.as_mut() {
        *p = t.frame_ms;
    }
    if let Some(p) = until_julian.as_mut() {
        *p = t.until_julian;
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_take_turbo_auto_off(sim: *mut fc4_sim) -> i32 {
    with_sim(sim, 0, |s| s.take_turbo_auto_off().unwrap_or(0))
}

// ---- pacing (simulator.md §5.7 [choice]; fc4_core::pacing) -------------------------

pub const FC4_PACING_ORIGINAL: u32 = 0;
pub const FC4_PACING_DETERMINISTIC: u32 = 1;

/// The simulator's deterministic-pacing hooks (fc4_core::PacingHooks).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_pacing_hooks {
    pub struct_size: u32,
    pub user: *mut c_void,
    pub is_paced: Option<unsafe extern "C" fn(user: *mut c_void, agent: fc4_agent) -> bool>,
    pub settle: Option<unsafe extern "C" fn(user: *mut c_void, julian: i32)>,
    pub idle_steps: Option<unsafe extern "C" fn(user: *mut c_void, agent: fc4_agent) -> u32>,
}

struct CPacingHooks(fc4_pacing_hooks);

impl fc4_core::PacingHooks for CPacingHooks {
    fn is_paced(&self, agent: AgentHandle) -> bool {
        match self.0.is_paced {
            // SAFETY: the installer promised valid callbacks while installed.
            Some(f) => unsafe { f(self.0.user, agent) },
            None => true,
        }
    }
    fn settle(&self, julian: i32) {
        if let Some(f) = self.0.settle {
            // SAFETY: as above.
            unsafe { f(self.0.user, julian) }
        }
    }
    fn idle_steps(&self, agent: AgentHandle) -> u32 {
        match self.0.idle_steps {
            // SAFETY: as above.
            Some(f) => unsafe { f(self.0.user, agent) },
            None => 1,
        }
    }
}

/// The process-wide pacing that cores capture when they are created
/// (`FC4_PACING_*`; default original). False for an unknown mode.
#[no_mangle]
pub extern "C" fn fc4_pacing_set_process(mode: u32) -> bool {
    match fc4_core::pacing::Pacing::from_u32(mode) {
        Some(p) => {
            fc4_core::pacing::set_process(p);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn fc4_pacing_process() -> u32 {
    fc4_core::pacing::process().as_u32()
}

/// Sets the simulator's pacing and hooks (NULL hooks: every idle agent is
/// paced, no settle callback). False for an unknown mode or a short struct.
#[no_mangle]
pub unsafe extern "C" fn fc4_sim_set_pacing(sim: *mut fc4_sim, mode: u32, hooks: *const fc4_pacing_hooks) -> bool {
    let Some(p) = fc4_core::pacing::Pacing::from_u32(mode) else {
        return false;
    };
    let h: Option<std::rc::Rc<dyn fc4_core::PacingHooks>> = match hooks.as_ref() {
        None => None,
        Some(k) => {
            if (k.struct_size as usize) < std::mem::size_of::<fc4_pacing_hooks>() {
                return false;
            }
            Some(std::rc::Rc::new(CPacingHooks(*k)))
        }
    };
    with_sim(sim, false, |s| s.set_pacing(p, h))
}

/// The simulator's pacing (`FC4_PACING_*`) and whether the current day has
/// had its settle point (always true under original pacing).
#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_pacing(sim: *const fc4_sim, mode: *mut u32, day_settled: *mut bool) -> bool {
    let Some((p, settled)) = with_sim(sim, None, |s| Some((s.pacing(), s.day_settled()))) else {
        return false;
    };
    if let Some(o) = mode.as_mut() {
        *o = p.as_u32();
    }
    if let Some(o) = day_settled.as_mut() {
        *o = settled;
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_debug(sim: *const fc4_sim, out: *mut fc4_debug) -> bool {
    let Some(o) = out.as_mut() else { return false };
    if (o.struct_size as usize) < std::mem::size_of::<fc4_debug>() {
        return false;
    }
    let Some(d) = with_sim(sim, None, |s| Some(s.debug_info())) else {
        return false;
    };
    let cur = |c: Option<usize>| c.map_or(-1, |i| i as i32);
    *o = fc4_debug {
        struct_size: std::mem::size_of::<fc4_debug>() as u32,
        interval_elapsed_ms: d.interval_elapsed_ms,
        slice_elapsed_ms: d.slice_elapsed_ms,
        ms_per_day: d.ms_per_day,
        ready: d.ready.into(),
        one_shot_or_emergency_pending: d.one_shot_or_emergency_pending.into(),
        hours_complete: d.hours_complete.into(),
        day_level_done: d.day_level_done.into(),
        tomorrow_starts_week: d.tomorrow_starts_week.into(),
        tomorrow_starts_month: d.tomorrow_starts_month.into(),
        tomorrow_starts_year: d.tomorrow_starts_year.into(),
        day_gate_open: d.day_gate_open.into(),
        blocked_by_week: d.blocked_by_week.into(),
        blocked_by_month: d.blocked_by_month.into(),
        blocked_by_year: d.blocked_by_year.into(),
        reserved: 0,
        cursors: d.cursors.map(cur),
        idle_cursor: cur(d.idle_cursor),
    };
    true
}

#[no_mangle]
pub unsafe extern "C" fn fc4_sim_get_agent_info(
    sim: *const fc4_sim,
    level: u32,
    index: usize,
    out: *mut fc4_agent_info,
    name: *mut c_char,
    name_capacity: usize,
) -> bool {
    let Some(info) = with_sim(sim, None, |s| s.agent_info(level, index)) else {
        return false;
    };
    if let Some(o) = out.as_mut() {
        *o = fc4_agent_info {
            agent: info.handle,
            flags: info.flags,
            enabled: info.enabled.into(),
            reserved: [0; 3],
        };
    }
    if !name.is_null() && name_capacity > 0 {
        let n = info.name.len().min(name_capacity - 1);
        std::ptr::copy_nonoverlapping(info.name.as_ptr(), name as *mut u8, n);
        *name.add(n) = 0;
    }
    true
}

pub mod advisors;
pub mod audio;
pub mod aura;
pub mod aura_shadow;
pub mod automata;
pub mod budget;
pub mod city_detail;
pub mod city_situation;
pub mod civic;
pub mod clock24;
pub mod commercial;
pub mod crime;
pub mod demand;
pub mod demolition;
pub mod development;
pub mod disasters;
pub mod dispatch;
pub mod effects;
pub mod flammability;
pub mod flammability_shadow;
pub mod flora;
pub mod flora_shadow;
pub mod growth;
pub mod history;
pub mod hostgrid;
pub mod landfill;
pub mod landfill_shadow;
pub mod landvalue;
pub mod landvalue_shadow;
pub mod lot_config;
pub mod lotdev;
pub mod neighbors;
pub mod neighbors_shadow;
pub mod network_geometry;
pub mod network_manager;
pub mod network_rules;
pub mod network_tools;
pub mod occupant_manager;
pub mod ordinance;
pub mod panic_log;
pub mod parks;
pub mod plumbing;
pub mod plumbing_shadow;
pub mod pollution;
pub mod pollution_shadow;
pub mod port_developer;
pub mod power;
pub mod power_shadow;
pub mod prop_developer;
pub mod prop_manager;
pub mod prop_occupant;
pub mod props;
pub mod protection;
pub mod residential;
pub mod residential_shadow;
pub mod sinks;
pub mod stream;
pub mod terrain;
pub mod tract_developer;
pub mod traffic;
pub mod traffic_shadow;
pub mod tutorial;
pub mod ui_windows;
pub mod weather;
pub mod zonedev;
pub mod zones;
// glue-formats4: DBPF packages and exemplars for the plug-in's formats shadow.
pub mod dbpf;
// the glue-span565-impl4 lane: the 565 span renderer bridge (span-render-565.md §1.1)
pub mod span565;
// glue-disasters-impl4: the layer-only disaster layer (`enable = disasters`).
pub mod disaster_layer;
// glue-span4444-impl4: the cSpanRender4444 bridge (span-render-4444.md §1.1-§7).
pub mod span_render4444;
// glue-span1555-impl4: the 1555 span renderer for the plug-in's native bridge.
pub mod span1555;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod aura_tests;

#[cfg(test)]
mod advisors_tests;

#[cfg(test)]
mod automata_tests;

#[cfg(test)]
mod budget_tests;

#[cfg(test)]
mod city_detail_tests;

#[cfg(test)]
mod city_situation_tests;

#[cfg(test)]
mod civic_tests;

#[cfg(test)]
mod clock24_tests;

#[cfg(test)]
mod commercial_tests;

#[cfg(test)]
mod crime_tests;

#[cfg(test)]
mod demand_tests;

#[cfg(test)]
mod demolition_tests;

#[cfg(test)]
mod development_tests;

#[cfg(test)]
mod disasters_tests;
#[cfg(test)]
mod effects_tests;

#[cfg(test)]
mod dispatch_tests;

#[cfg(test)]
mod flora_tests;

#[cfg(test)]
mod history_tests;

#[cfg(test)]
mod landfill_tests;

#[cfg(test)]
mod landvalue_tests;

#[cfg(test)]
mod landvalue_shadow_tests;

#[cfg(test)]
mod lot_config_tests;

#[cfg(test)]
mod lotdev_tests;

#[cfg(test)]
mod neighbors_tests;

#[cfg(test)]
mod network_geometry_tests;

#[cfg(test)]
mod network_manager_tests;
#[cfg(test)]
mod network_rules_tests;

#[cfg(test)]
mod network_tools_tests;

#[cfg(test)]
mod occupant_manager_tests;

#[cfg(test)]
mod flammability_shadow_tests;

#[cfg(test)]
mod ordinance_tests;

#[cfg(test)]
mod parks_tests;

#[cfg(test)]
mod plumbing_shadow_tests;

#[cfg(test)]
mod pollution_tests;

#[cfg(test)]
mod port_developer_tests;

#[cfg(test)]
mod power_tests;

#[cfg(test)]
mod power_shadow_tests;

#[cfg(test)]
mod prop_developer_tests;

#[cfg(test)]
mod prop_city_sweep_tests;

#[cfg(test)]
mod protection_tests;

#[cfg(test)]
mod residential_tests;

#[cfg(test)]
mod prop_manager_tests;

#[cfg(test)]
mod prop_occupant_tests;

#[cfg(test)]
mod save_sweep_tests;

#[cfg(test)]
mod terrain_tests;

#[cfg(test)]
mod test_streams;

#[cfg(test)]
mod tract_developer_tests;

#[cfg(test)]
mod traffic_tests;
#[cfg(test)]
mod tutorial_tests;

#[cfg(test)]
mod ui_windows_tests;

#[cfg(test)]
mod weather_tests;

#[cfg(test)]
mod zones_tests;

#[cfg(test)]
mod zonedev_tests;

#[cfg(test)]
mod audio_tests;

#[cfg(test)]
mod span565_tests;
#[cfg(test)]
mod disaster_layer_tests;
#[cfg(test)]
mod span1555_tests;
// glue-span8888-impl4: the cSpanRender8888 bridge (span-render-8888.md §1.1-§9).
pub mod span_render8888;

// Private native-grid verification transport (no public ABI entries).
mod simgrids_verify;
