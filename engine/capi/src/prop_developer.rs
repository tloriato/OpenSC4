// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The prop developer and its wealth requester over the C ABI (fc4.h "prop
//! developer", docs/specs/prop-developer.md).
//!
//! The host implements [`fc4_prop_host`]; the plugin's `cISC4PropDeveloper`
//! (with the message-target and serializable sub-objects) forwards its
//! slots here, and its `cSC4WealthRequester` objects forward `SelectProp`
//! to [`fc4_prop_developer_wealth_select`] (the other requester slots are
//! constants: share 10, satisfaction `0x49A1E995`, custom selector). The
//! crime and pollution requesters stay in the host and are reached through
//! the `requester_*` callbacks. Tokens are opaque host identities: lots,
//! network lots, prop occupants, requesters, managers.
//!
//! Generators (§5): the member generator is seeded from the session seed
//! `S` (`derive_seed(S, 0x89C48F47) × 28`); the wealth requester's file
//! generator (`derive_seed(S, 0x4C09C51F)`) is shared by every developer
//! created on this thread with the same session seed.
//!
//! Threads: the core is single-threaded. The plugin serialises every call
//! on one object (fc4.h).

use std::cell::RefCell;
use std::ffi::{c_char, c_void, CString};
use std::rc::Rc;

use fc4_core::grid::{CellRect, ScanState};
use fc4_core::lot_developer::CfgObject;
use fc4_core::prop_developer::{
    member_generator, query_interface, wealth_generator, wealth_query_interface, PropDeveloper, PropHost, PropMessage,
    PropRef, Requester, SharedRandom, WealthRequester, CLSID_WEALTH_REQUESTER,
};
use fc4_core::property::PropertyHolder;
use fc4_core::stream::SaveObject;

use crate::guard;
use crate::hostgrid::{fc4_cell_rect, take_props};
use crate::props::fc4_props;
use crate::sinks::{cfg_objects, fc4_lot_cfg_objects_sink, fc4_u32s_sink, fc4_u64s_sink, tokens, u32s};
use crate::stream::{fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

#[repr(C)]
pub struct fc4_prop_developer {
    _private: [u8; 0],
}

/// A held host object: its token (0 = null) and class (for the record).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_prop_ref {
    pub token: u64,
    pub class_id: u32,
    pub reserved: u32,
}

/// What the prop developer needs from the game (fc4.h `fc4_prop_host`). A
/// missing callback reads as 0 / false / none / empty.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_prop_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    // ---- city, simulator, messages ----
    pub city_cells: Option<unsafe extern "C" fn(U, *mut i32, *mut i32)>,
    pub cell_width: Option<unsafe extern "C" fn(U, *mut f32, *mut f32)>,
    /// Fills three refs: lot manager, prop manager, network lot manager.
    pub managers: Option<unsafe extern "C" fn(U, *mut fc4_prop_ref)>,
    /// Creates the host object of a new wealth requester; its token.
    pub create_wealth_requester: Option<unsafe extern "C" fn(U) -> u64>,
    pub add_agent: Option<unsafe extern "C" fn(U, u32, *const c_char, u32)>,
    pub remove_agents: Option<unsafe extern "C" fn(U, *const c_char)>,
    pub subscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub unsubscribe_all: Option<unsafe extern "C" fn(U)>,
    pub register_animation_tick: Option<unsafe extern "C" fn(U, u32)>,
    pub unregister_animation_tick: Option<unsafe extern "C" fn(U)>,
    /// Posts `0xCA5E5027` (data1 x, data2 z, data3 entered state 3).
    pub post_lot_state_props: Option<unsafe extern "C" fn(U, i32, i32, bool)>,
    /// AddRef / Release of a held requester or prop.
    pub hold: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    // ---- host requesters (crime, pollution) ----
    pub requester_share: Option<unsafe extern "C" fn(U, u64, *const fc4_cell_rect) -> i32>,
    pub requester_satisfaction: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub requester_custom: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub requester_select: Option<unsafe extern "C" fn(U, u64, *const u32, usize, i32, i32) -> u32>,
    // ---- lots and network lots ----
    /// `lotManager.GetLot(x, z, originOnly)`; 0 = none.
    pub lot_at: Option<unsafe extern "C" fn(U, i32, i32, bool) -> u64>,
    pub lot_livable: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub lot_building_type: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub lot_occupancy: Option<unsafe extern "C" fn(U, u64) -> f32>,
    /// The configuration's objects through the sink; false without one.
    pub lot_config_objects: Option<unsafe extern "C" fn(U, u64, fc4_lot_cfg_objects_sink, U) -> bool>,
    pub lot_wealth: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub lot_location: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32)>,
    pub lot_size: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32)>,
    pub lot_state: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub zone_type: Option<unsafe extern "C" fn(U, i32, i32) -> u32>,
    pub network_lot_at: Option<unsafe extern "C" fn(U, i32, i32) -> u64>,
    pub network_lot_objects: Option<unsafe extern "C" fn(U, u64, fc4_lot_cfg_objects_sink, U) -> bool>,
    pub network_wealth: Option<unsafe extern "C" fn(U, i32, i32) -> u32>,
    // ---- prop manager ----
    pub lot_props: Option<unsafe extern "C" fn(U, u64, fc4_u64s_sink, U)>,
    pub street_props: Option<unsafe extern "C" fn(U, i32, i32, fc4_u64s_sink, U)>,
    pub street_props_in_rect: Option<unsafe extern "C" fn(U, *const fc4_cell_rect, fc4_u64s_sink, U)>,
    pub remove_prop: Option<unsafe extern "C" fn(U, u64)>,
    pub prop_type_valid: Option<unsafe extern "C" fn(U, u32) -> bool>,
    pub props_by_family: Option<unsafe extern "C" fn(U, u32, fc4_u32s_sink, U)>,
    pub prop_satisfaction: Option<unsafe extern "C" fn(U, u32) -> u32>,
    pub prop_wealth: Option<unsafe extern "C" fn(U, u32) -> u8>,
    /// Lot developer `CreateLotProp(lot, index, id)`; the occupant, 0 = none.
    pub create_lot_prop: Option<unsafe extern "C" fn(U, u64, u32, u32) -> u64>,
    pub create_network_prop: Option<unsafe extern "C" fn(U, u64, u32, u32) -> u64>,
    // ---- prop occupants ----
    pub prop_class: Option<unsafe extern "C" fn(U, u64) -> u32>,
    /// A NEW fc4_props with the occupant's properties, NULL if none.
    pub prop_properties: Option<unsafe extern "C" fn(U, u64) -> *mut fc4_props>,
    /// The lot tag; false without one.
    pub prop_lot_tag: Option<unsafe extern "C" fn(U, u64, *mut u32) -> bool>,
    pub set_prop_requester: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub prop_type: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub prop_state: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub set_prop_state: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub set_prop_timer: Option<unsafe extern "C" fn(U, u64, u32, bool)>,
    pub set_random_chance: Option<unsafe extern "C" fn(U, u64, u8)>,
    pub set_time_of_day_range: Option<unsafe extern "C" fn(U, u64, f32, f32)>,
    pub time_of_day_range: Option<unsafe extern "C" fn(U, u64, *mut f32, *mut f32)>,
    pub set_date_range: Option<unsafe extern "C" fn(U, u64, u32, u32, u8)>,
    pub tick_date: Option<unsafe extern "C" fn(U, u64, u32)>,
    /// Three floats (x, y, z).
    pub prop_position: Option<unsafe extern "C" fn(U, u64, *mut f32)>,
    pub set_prop_position: Option<unsafe extern "C" fn(U, u64, *const f32)>,
    pub set_prop_flag: Option<unsafe extern "C" fn(U, u64, u32, bool)>,
    pub network_first_cell: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32)>,
    pub position_to_cell: Option<unsafe extern "C" fn(U, f32, f32, *mut i32, *mut i32)>,
    // ---- others ----
    pub is_powered: Option<unsafe extern "C" fn(U, i32, i32) -> bool>,
    pub view_available: Option<unsafe extern "C" fn(U) -> bool>,
    pub view_trigger_state: Option<unsafe extern "C" fn(U, u32) -> bool>,
    pub sim_date: Option<unsafe extern "C" fn(U) -> u32>,
    /// The 24-hour clock's hour and minute; false without the clock.
    pub clock_time: Option<unsafe extern "C" fn(U, *mut u32, *mut u32) -> bool>,
    pub visual_time_mode: Option<unsafe extern "C" fn(U) -> u32>,
    pub lighting_is_night: Option<unsafe extern "C" fn(U) -> bool>,
    /// Terrain altitude and water level at a position.
    pub terrain_heights: Option<unsafe extern "C" fn(U, f32, f32, *mut f32, *mut f32)>,
}

/// A message for [`fc4_prop_developer_do_message`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_prop_message {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    pub reserved: u32,
    /// The message's object (an occupant or a lot), 0 = none.
    pub object: u64,
}

/// Diagnostics (`fc4_prop_developer_get_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_prop_state {
    pub struct_size: u32,
    pub scan_flag: i32,
    /// 0 fresh, 1 running, 2 done.
    pub scanner_state: u32,
    pub scanner_x: i32,
    pub scanner_z: i32,
    pub scanner_step: i32,
    pub clip_x0: i32,
    pub clip_z0: i32,
    pub clip_x1: i32,
    pub clip_z1: i32,
    pub night_cursor: u32,
    pub night_mode: u32,
    pub night_batch: u32,
    pub night_acc: u32,
    pub tod_threshold: f32,
    pub tod_last: f32,
    pub ref_count: u32,
    pub requester_count: u32,
    pub night_count: u32,
    pub trigger_count: u32,
    pub time_of_day_count: u32,
    pub dated_count: u32,
    pub power_count: u32,
    pub sign_count: u32,
    pub wealth_added: u8,
    pub is_night: u8,
    pub signs_visible: u8,
    pub initialized: u8,
    pub destroyed: u8,
    pub reserved: [u8; 3],
}

/// What the last block update did (`fc4_prop_developer_last_block`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_prop_block {
    pub struct_size: u32,
    pub bx: i32,
    pub bz: i32,
    pub lots: u32,
    pub occ_sum: f32,
    pub slots: i32,
    pub kept_all: i32,
    pub target: u32,
    pub removed: u32,
    pub created: u32,
    pub member_draws: u32,
    pub requester_count: u32,
    pub stopped: u8,
    pub reserved: [u8; 3],
}

/// The lists of `fc4_prop_developer_list`.
pub const FC4_PROP_LIST_REQUESTERS: u32 = 0;
pub const FC4_PROP_LIST_NIGHT: u32 = 1;
pub const FC4_PROP_LIST_TIME_OF_DAY: u32 = 2;
pub const FC4_PROP_LIST_DATED: u32 = 3;
pub const FC4_PROP_LIST_POWER: u32 = 4;
pub const FC4_PROP_LIST_SIGNS: u32 = 5;

const _: () = {
    assert!(std::mem::size_of::<fc4_prop_ref>() == 16);
    assert!(std::mem::size_of::<fc4_prop_message>() == 32);
    assert!(std::mem::size_of::<fc4_prop_state>() == 104);
    assert!(std::mem::size_of::<fc4_prop_block>() == 52);
    assert!(std::mem::size_of::<fc4_prop_host>() == if cfg!(target_pointer_width = "32") { 272 } else { 544 });
};

// ---- host adapter -----------------------------------------------------------------------------------

struct CHost {
    h: fc4_prop_host,
}

macro_rules! cb {
    ($s:expr, $f:ident, $default:expr $(, $arg:expr)*) => {
        // SAFETY: whoever created the object promised valid callbacks.
        match $s.h.$f { Some(f) => unsafe { f($s.h.user $(, $arg)*) }, None => $default }
    };
}

fn opt(t: u64) -> Option<u64> {
    (t != 0).then_some(t)
}

fn rect_c(r: CellRect) -> fc4_cell_rect {
    fc4_cell_rect {
        x1: r.x1,
        z1: r.z1,
        x2: r.x2,
        z2: r.z2,
    }
}

fn ref_c(r: PropRef) -> fc4_prop_ref {
    fc4_prop_ref {
        token: r.token,
        class_id: r.class_id,
        reserved: 0,
    }
}

impl PropHost for CHost {
    fn city_cells(&self) -> (i32, i32) {
        let (mut x, mut z) = (0, 0);
        cb!(self, city_cells, (), &mut x, &mut z);
        (x, z)
    }
    fn cell_width(&self) -> (f32, f32) {
        let (mut x, mut z) = (0.0, 0.0);
        cb!(self, cell_width, (), &mut x, &mut z);
        (x, z)
    }
    fn managers(&self) -> [PropRef; 3] {
        let mut m = [fc4_prop_ref::default(); 3];
        cb!(self, managers, (), m.as_mut_ptr());
        m.map(|r| PropRef::new(r.token, r.class_id))
    }
    fn create_wealth_requester(&self) -> u64 {
        cb!(self, create_wealth_requester, 0)
    }
    fn add_agent(&self, level: u32, name: &str, flags: u32) {
        let n = CString::new(name).unwrap_or_default();
        cb!(self, add_agent, (), level, n.as_ptr(), flags)
    }
    fn remove_agents(&self, name: &str) {
        let n = CString::new(name).unwrap_or_default();
        cb!(self, remove_agents, (), n.as_ptr())
    }
    fn subscribe(&self, msg: u32) {
        cb!(self, subscribe, (), msg)
    }
    fn unsubscribe_all(&self) {
        cb!(self, unsubscribe_all, ())
    }
    fn register_animation_tick(&self, priority: u32) {
        cb!(self, register_animation_tick, (), priority)
    }
    fn unregister_animation_tick(&self) {
        cb!(self, unregister_animation_tick, ())
    }
    fn post_lot_state_props(&self, x: i32, z: i32, entered_3: bool) {
        cb!(self, post_lot_state_props, (), x, z, entered_3)
    }
    fn hold(&self, token: u64) {
        cb!(self, hold, (), token)
    }
    fn release(&self, token: u64) {
        cb!(self, release, (), token)
    }
    fn requester_share(&self, requester: u64, rect: CellRect) -> i32 {
        let r = rect_c(rect);
        cb!(self, requester_share, 0, requester, &r)
    }
    fn requester_satisfaction(&self, requester: u64) -> u32 {
        cb!(self, requester_satisfaction, 0, requester)
    }
    fn requester_custom(&self, requester: u64) -> bool {
        cb!(self, requester_custom, false, requester)
    }
    fn requester_select(&self, requester: u64, ids: &[u32], x: i32, z: i32) -> u32 {
        cb!(self, requester_select, 0, requester, ids.as_ptr(), ids.len(), x, z)
    }
    fn lot_at(&self, x: i32, z: i32, origin_only: bool) -> Option<u64> {
        opt(cb!(self, lot_at, 0, x, z, origin_only))
    }
    fn lot_livable(&self, lot: u64) -> bool {
        cb!(self, lot_livable, false, lot)
    }
    fn lot_building_type(&self, lot: u64) -> u32 {
        cb!(self, lot_building_type, 0, lot)
    }
    fn lot_occupancy(&self, lot: u64) -> f32 {
        cb!(self, lot_occupancy, 0.0, lot)
    }
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>> {
        cfg_objects(|s, c| cb!(self, lot_config_objects, false, lot, s, c))
    }
    fn lot_wealth(&self, lot: u64) -> u32 {
        cb!(self, lot_wealth, 0, lot)
    }
    fn lot_location(&self, lot: u64) -> (i32, i32) {
        let (mut x, mut z) = (0, 0);
        cb!(self, lot_location, (), lot, &mut x, &mut z);
        (x, z)
    }
    fn lot_size(&self, lot: u64) -> (i32, i32) {
        let (mut x, mut z) = (0, 0);
        cb!(self, lot_size, (), lot, &mut x, &mut z);
        (x, z)
    }
    fn lot_state(&self, lot: u64) -> u32 {
        cb!(self, lot_state, 0, lot)
    }
    fn zone_type(&self, x: i32, z: i32) -> u32 {
        cb!(self, zone_type, 0, x, z)
    }
    fn network_lot_at(&self, x: i32, z: i32) -> Option<u64> {
        opt(cb!(self, network_lot_at, 0, x, z))
    }
    fn network_lot_objects(&self, network_lot: u64) -> Rc<Vec<CfgObject>> {
        cfg_objects(|s, c| cb!(self, network_lot_objects, false, network_lot, s, c)).unwrap_or_default()
    }
    fn network_wealth(&self, x: i32, z: i32) -> u32 {
        cb!(self, network_wealth, 0, x, z)
    }
    fn lot_props(&self, lot: u64) -> Vec<u64> {
        tokens(|s, c| cb!(self, lot_props, (), lot, s, c))
    }
    fn street_props(&self, x: i32, z: i32) -> Vec<u64> {
        tokens(|s, c| cb!(self, street_props, (), x, z, s, c))
    }
    fn street_props_in_rect(&self, rect: CellRect) -> Vec<u64> {
        let r = rect_c(rect);
        tokens(|s, c| cb!(self, street_props_in_rect, (), &r, s, c))
    }
    fn remove_prop(&self, prop: u64) {
        cb!(self, remove_prop, (), prop)
    }
    fn prop_type_valid(&self, id: u32) -> bool {
        cb!(self, prop_type_valid, false, id)
    }
    fn props_by_family(&self, family: u32) -> Vec<u32> {
        u32s(|s, c| cb!(self, props_by_family, (), family, s, c))
    }
    fn prop_satisfaction(&self, id: u32) -> u32 {
        cb!(self, prop_satisfaction, 0, id)
    }
    fn prop_wealth(&self, id: u32) -> u8 {
        cb!(self, prop_wealth, 0, id)
    }
    fn create_lot_prop(&self, lot: u64, index: usize, id: u32) -> Option<u64> {
        opt(cb!(self, create_lot_prop, 0, lot, index as u32, id))
    }
    fn create_network_prop(&self, network_lot: u64, index: usize, id: u32) -> Option<u64> {
        opt(cb!(self, create_network_prop, 0, network_lot, index as u32, id))
    }
    fn prop_class(&self, prop: u64) -> u32 {
        cb!(self, prop_class, 0, prop)
    }
    fn prop_properties(&self, prop: u64) -> Option<Rc<PropertyHolder>> {
        let p = cb!(self, prop_properties, std::ptr::null_mut(), prop);
        // SAFETY: the host hands over a NEW fc4_props (or NULL).
        unsafe { take_props(p) }
    }
    fn prop_lot_tag(&self, prop: u64) -> Option<u32> {
        let mut t = 0u32;
        cb!(self, prop_lot_tag, false, prop, &mut t).then_some(t)
    }
    fn set_prop_requester(&self, prop: u64, requester: u32) {
        cb!(self, set_prop_requester, (), prop, requester)
    }
    fn prop_type(&self, prop: u64) -> u32 {
        cb!(self, prop_type, 0, prop)
    }
    fn prop_state(&self, prop: u64) -> u32 {
        cb!(self, prop_state, 0, prop)
    }
    fn set_prop_state(&self, prop: u64, state: u32) {
        cb!(self, set_prop_state, (), prop, state)
    }
    fn set_prop_timer(&self, prop: u64, mask: u32, on: bool) {
        cb!(self, set_prop_timer, (), prop, mask, on)
    }
    fn set_random_chance(&self, prop: u64, chance: u8) {
        cb!(self, set_random_chance, (), prop, chance)
    }
    fn set_time_of_day_range(&self, prop: u64, start: f32, end: f32) {
        cb!(self, set_time_of_day_range, (), prop, start, end)
    }
    fn time_of_day_range(&self, prop: u64) -> (f32, f32) {
        let (mut s, mut e) = (0.0, 0.0);
        cb!(self, time_of_day_range, (), prop, &mut s, &mut e);
        (s, e)
    }
    fn set_date_range(&self, prop: u64, date: u32, duration: u32, flag: u8) {
        cb!(self, set_date_range, (), prop, date, duration, flag)
    }
    fn tick_date(&self, prop: u64, date: u32) {
        cb!(self, tick_date, (), prop, date)
    }
    fn prop_position(&self, prop: u64) -> [f32; 3] {
        let mut p = [0f32; 3];
        cb!(self, prop_position, (), prop, p.as_mut_ptr());
        p
    }
    fn set_prop_position(&self, prop: u64, position: [f32; 3]) {
        cb!(self, set_prop_position, (), prop, position.as_ptr())
    }
    fn set_prop_flag(&self, prop: u64, flag: u32, on: bool) {
        cb!(self, set_prop_flag, (), prop, flag, on)
    }
    fn network_first_cell(&self, occupant: u64) -> (i32, i32) {
        let (mut x, mut z) = (0, 0);
        cb!(self, network_first_cell, (), occupant, &mut x, &mut z);
        (x, z)
    }
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32) {
        let (mut cx, mut cz) = (0, 0);
        cb!(self, position_to_cell, (), x, z, &mut cx, &mut cz);
        (cx, cz)
    }
    fn is_powered(&self, x: i32, z: i32) -> bool {
        cb!(self, is_powered, false, x, z)
    }
    fn view_available(&self) -> bool {
        cb!(self, view_available, false)
    }
    fn view_trigger_state(&self, trigger: u32) -> bool {
        cb!(self, view_trigger_state, false, trigger)
    }
    fn sim_date(&self) -> u32 {
        cb!(self, sim_date, 0)
    }
    fn clock_time(&self) -> Option<(u32, u32)> {
        let (mut h, mut m) = (0u32, 0u32);
        cb!(self, clock_time, false, &mut h, &mut m).then_some((h, m))
    }
    fn visual_time_mode(&self) -> u32 {
        cb!(self, visual_time_mode, 0)
    }
    fn lighting_is_night(&self) -> bool {
        cb!(self, lighting_is_night, false)
    }
    fn terrain_heights(&self, x: f32, z: f32) -> (f32, f32) {
        let (mut t, mut w) = (0.0, 0.0);
        cb!(self, terrain_heights, (), x, z, &mut t, &mut w);
        (t, w)
    }
}

// ---- the session's wealth generator (§5) --------------------------------------------------------------

thread_local! {
    /// The session seed and its wealth file generator.
    static WEALTH: RefCell<Option<(u32, SharedRandom)>> = const { RefCell::new(None) };
}

/// The wealth file generator for `seed` (a new one when the seed changes).
fn wealth_rng(seed: u32) -> SharedRandom {
    WEALTH.with(|w| {
        let mut w = w.borrow_mut();
        match &*w {
            Some((s, r)) if *s == seed => r.clone(),
            _ => {
                let r: SharedRandom = Rc::new(RefCell::new(wealth_generator(seed)));
                *w = Some((seed, r.clone()));
                r
            }
        }
    })
}

// ---- handles ----------------------------------------------------------------------------------------

struct Handle {
    sim: Rc<PropDeveloper>,
}

fn with_p<R>(p: *const fc4_prop_developer, fallback: R, f: impl FnOnce(&Handle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_prop_developer*.
    match unsafe { (p as *const Handle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

unsafe fn ids<'a>(p: *const u32, n: usize) -> &'a [u32] {
    if p.is_null() || n == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p, n)
    }
}

fn put_offset(iid: Option<u32>, offset: *mut u32) -> bool {
    match iid {
        Some(o) => {
            // SAFETY: NULL or a valid pointer (documented).
            if let Some(out) = unsafe { offset.as_mut() } {
                *out = o;
            }
            true
        }
        None => false,
    }
}

/// Creates a developer (reference count 0). The host table is copied.
/// `session_seed` is the session seed `S` of §5; `token` is the developer's
/// own host identity.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_create(
    host: *const fc4_prop_host,
    session_seed: u32,
    token: u64,
) -> *mut fc4_prop_developer {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h) if (h.struct_size as usize) >= std::mem::size_of::<fc4_prop_host>() => {
            let c = Rc::new(CHost { h: *h });
            let sim = PropDeveloper::new(
                c,
                Box::new(member_generator(session_seed)),
                wealth_rng(session_seed),
                token,
            );
            Box::into_raw(Box::new(Handle { sim })) as *mut fc4_prop_developer
        }
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_destroy(p: *mut fc4_prop_developer) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut Handle)));
    }
}

/// The developer's `QueryInterface` (§1): the sub-object offset (0, 4 or 8)
/// in `*offset`; false for another IID.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_query_interface(iid: u32, offset: *mut u32) -> bool {
    put_offset(query_interface(iid), offset)
}

/// The wealth requester's `QueryInterface` (§1): 0 or 4.
#[no_mangle]
pub unsafe extern "C" fn fc4_wealth_requester_query_interface(iid: u32, offset: *mut u32) -> bool {
    put_offset(wealth_query_interface(iid), offset)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_add_ref(p: *mut fc4_prop_developer) -> u32 {
    with_p(p, 0, |h| h.sim.add_ref())
}

/// `Release`: the last release marks the object destroyed and returns 0;
/// the handle stays valid until [`fc4_prop_developer_destroy`].
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_release(p: *mut fc4_prop_developer) -> u32 {
    with_p(p, 0, |h| h.sim.release())
}

/// `Init` (§3.1): the managers, the wealth requester (new city), the
/// scanner, the agents, the subscriptions, the animation tick.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_init(p: *mut fc4_prop_developer) -> bool {
    with_p(p, false, |h| h.sim.init())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_shutdown(p: *mut fc4_prop_developer) -> bool {
    with_p(p, false, |h| h.sim.shutdown())
}

/// `DoMessage` (§3.3): false only for an animation tick in night mode 0
/// (and for a NULL message).
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_do_message(
    p: *mut fc4_prop_developer,
    msg: *const fc4_prop_message,
) -> bool {
    let Some(m) = msg.as_ref() else { return false };
    with_p(p, false, |h| {
        h.sim.do_message(&PropMessage {
            msg_type: m.msg_type,
            object: m.object,
            data1: m.data1,
            data2: m.data2,
            data3: m.data3,
            data4: m.data4,
        })
    })
}

/// `AddPropRequester` (slot 5): appended with no duplicate check. A class
/// of `0x89C49986` is a wealth requester (the core answers for it); any
/// other is a host requester (held).
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_add_prop_requester(p: *mut fc4_prop_developer, token: u64, class_id: u32) {
    with_p(p, (), |h| {
        let r = if token == 0 {
            Requester::Null
        } else if class_id == CLSID_WEALTH_REQUESTER {
            Requester::Wealth(WealthRequester::new(token))
        } else {
            Requester::Host(PropRef::new(token, class_id))
        };
        h.sim.add_prop_requester(r)
    })
}

/// `RemovePropRequester` (slot 6): the first requester with this token;
/// false if absent.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_remove_prop_requester(p: *mut fc4_prop_developer, token: u64) -> bool {
    with_p(p, false, |h| h.sim.remove_prop_requester(token))
}

/// `UpdateAllOptionalProps` (slot 7): every block, x outer.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_update_all(p: *mut fc4_prop_developer) {
    with_p(p, (), |h| h.sim.update_all_optional_props())
}

/// `ShowSignsAndLabels` (slot 8).
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_show_signs(p: *mut fc4_prop_developer, show: bool) {
    with_p(p, (), |h| h.sim.show_signs(show))
}

/// `ToggleSignsAndLabels` (slot 9).
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_toggle_signs(p: *mut fc4_prop_developer) {
    with_p(p, (), |h| h.sim.toggle_signs())
}

/// `AreSignsAndLabelsVisible` (slot 10).
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_signs_visible(p: *const fc4_prop_developer) -> bool {
    with_p(p, false, |h| h.sim.signs_visible())
}

/// The wealth requester's `SelectProp(ids, x, z)` (§4.1), drawing from the
/// session's wealth file generator; 0 when no ID matches.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_wealth_select(
    p: *mut fc4_prop_developer,
    id_list: *const u32,
    count: usize,
    x: i32,
    z: i32,
) -> u32 {
    with_p(p, 0, |h| h.sim.wealth_select(ids(id_list, count), x, z))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_get_state(p: *const fc4_prop_developer, out: *mut fc4_prop_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_p(p, false, |h| {
        let d = &h.sim;
        let s = d.snapshot();
        let it = &s.scanner;
        *o = fc4_prop_state {
            struct_size: std::mem::size_of::<fc4_prop_state>() as u32,
            scan_flag: i32::from(s.scan_flag),
            // The stored byte (PV13): any value other than 0 and 2 runs.
            scanner_state: match it.state {
                ScanState::Fresh => 0,
                ScanState::Running => u32::from(s.scan_byte),
                ScanState::Done => 2,
            },
            scanner_x: it.x,
            scanner_z: it.z,
            scanner_step: it.step,
            clip_x0: it.x0,
            clip_z0: it.z0,
            clip_x1: it.x1,
            clip_z1: it.z1,
            night_cursor: s.night_cursor,
            night_mode: s.night_mode,
            night_batch: s.night_batch,
            night_acc: s.night_acc,
            tod_threshold: s.tod_threshold,
            tod_last: s.tod_last,
            ref_count: s.ref_count,
            requester_count: d.requesters().len() as u32,
            night_count: d.night_list().len() as u32,
            trigger_count: d.trigger_map().len() as u32,
            time_of_day_count: d.time_of_day_list().len() as u32,
            dated_count: d.dated_list().len() as u32,
            power_count: d.power_list().len() as u32,
            sign_count: d.sign_list().len() as u32,
            wealth_added: s.wealth_added.into(),
            is_night: s.is_night.into(),
            signs_visible: s.signs_visible.into(),
            initialized: s.initialized.into(),
            destroyed: s.destroyed.into(),
            reserved: [0; 3],
        };
        true
    })
}

/// The last block update; `want` and `kept` (each `cap` entries, either may
/// be NULL) receive the per-requester targets and kept counts.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_last_block(
    p: *const fc4_prop_developer,
    out: *mut fc4_prop_block,
    want: *mut u32,
    kept: *mut u32,
    cap: usize,
) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_p(p, false, |h| {
        let b = h.sim.last_block();
        *o = fc4_prop_block {
            struct_size: std::mem::size_of::<fc4_prop_block>() as u32,
            bx: b.block.0,
            bz: b.block.1,
            lots: b.lots,
            occ_sum: b.occ_sum,
            slots: b.slots,
            kept_all: b.kept_all,
            target: b.target,
            removed: b.removed,
            created: b.created,
            member_draws: b.member_draws,
            requester_count: b.want.len().max(b.kept.len()) as u32,
            stopped: b.stopped.into(),
            reserved: [0; 3],
        };
        for (dst, src) in [(want, &b.want), (kept, &b.kept)] {
            if !dst.is_null() {
                for (i, &v) in src.iter().take(cap).enumerate() {
                    *dst.add(i) = v;
                }
            }
        }
        true
    })
}

/// One of the lists (`FC4_PROP_LIST_*`): up to `cap` refs into `out`;
/// returns the length (call with `cap` 0 to size), 0 for an unknown list.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_list(
    p: *const fc4_prop_developer,
    which: u32,
    out: *mut fc4_prop_ref,
    cap: usize,
) -> usize {
    with_p(p, 0, |h| {
        let d = &h.sim;
        let list: Vec<PropRef> = match which {
            FC4_PROP_LIST_REQUESTERS => d
                .requesters()
                .iter()
                .map(|r| PropRef::new(r.token(), r.class_id()))
                .collect(),
            FC4_PROP_LIST_NIGHT => d.night_list(),
            FC4_PROP_LIST_TIME_OF_DAY => d.time_of_day_list(),
            FC4_PROP_LIST_DATED => d.dated_list(),
            FC4_PROP_LIST_POWER => d.power_list(),
            FC4_PROP_LIST_SIGNS => d.sign_list(),
            _ => Vec::new(),
        };
        if !out.is_null() {
            for (i, &r) in list.iter().take(cap).enumerate() {
                *out.add(i) = ref_c(r);
            }
        }
        list.len()
    })
}

/// Trigger map entry `index` (iteration order): its trigger in `*trigger`
/// and up to `cap` props into `out`; returns the entry's length, or
/// `SIZE_MAX` past the end.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_trigger_entry(
    p: *const fc4_prop_developer,
    index: u32,
    trigger: *mut u32,
    out: *mut fc4_prop_ref,
    cap: usize,
) -> usize {
    with_p(p, usize::MAX, |h| match h.sim.trigger_map().get(index as usize) {
        Some((t, list)) => {
            if let Some(o) = trigger.as_mut() {
                *o = *t;
            }
            if !out.is_null() {
                for (i, &r) in list.iter().take(cap).enumerate() {
                    *out.add(i) = ref_c(r);
                }
            }
            list.len()
        }
        None => usize::MAX,
    })
}

/// `Write` (§7): always version 5.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_write(p: *const fc4_prop_developer, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_p(p, false, |h| h.sim.write(&mut COut(o)))
}

fn no_core_objects(_: u32, _: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    None
}

/// `Read` (§7): false below version 4 or for a short record (nothing
/// changes). A requester reference of class `0x89C49986` becomes a wealth
/// requester with that token.
#[no_mangle]
pub unsafe extern "C" fn fc4_prop_developer_read(p: *mut fc4_prop_developer, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    with_p(p, false, |h| {
        h.sim.read(&mut CIn {
            stream: i,
            decode: no_core_objects,
        })
    })
}
