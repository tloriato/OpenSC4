// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The pollution simulator's C ABI (fc4.h "pollution", docs/specs/pollution.md).
//!
//! The host implements [`fc4_pollution_host`] (every `PollutionHost` method of
//! the core, in the trait's order, plus the game's int16 grids through
//! [`fc4_grid_api`]); the plugin's `cISC4PollutionSimulator` forwards its
//! slots to the `fc4_pollution_*` functions.
//!
//! Tokens are opaque host identities (fc4.h): occupants, deals, visual
//! effects, the department. 0 is the null object.
//!
//! # Threads
//!
//! The game calls the interface's getters from a second thread while the main
//! thread is inside a day or a month. Every slot of §8.1 except the lifecycle,
//! the messages, `Write` / `Read` and slot 66 goes straight to the core object
//! (whose state is behind a `Mutex`, see `PollutionSim`): it never touches the
//! handle's call bookkeeping and never clones an `Rc`. The occupant slots
//! (47, 52-65) call the host's occupant callbacks on the caller's thread, as
//! the original does.
//!
//! Re-entrancy (main thread): a message delivered while a main-thread call is
//! running is queued and handled when the outermost call returns; other
//! re-entrant main-thread calls run and are counted
//! (`fc4_pollution_state::reentrant_calls`).

use std::ffi::{c_char, c_void};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use fc4_core::grid::{GridProvider, TractGrid};
use fc4_core::pollution::{
    BudgetItem, Deal, PollutionHost, PollutionSim, PollutionState, GRID_INSTANCES, L_INC, L_REC, L_WTE, L_WTP,
};
use fc4_core::property::PropertyHolder;
use fc4_core::stream::{RefValue, SaveObject};
use fc4_core::variant::Variant;
use fc4_core::x87::ftol;

use crate::budget::fc4_token_callback;
use crate::guard;
use crate::hostgrid::{
    collect_tokens, create_grid, fc4_grid_api, grid_token, native_variant, read_property, resolve_grid, take_props,
};
use crate::props::fc4_props;
use crate::stream::{fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

/// `GetBudgetItemForPurpose` (fc4.h `fc4_pollution_budget_item`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_pollution_budget_item {
    pub department: u32,
    pub line: u32,
    pub purpose: u32,
    pub index: u32,
    pub cost: i64,
    pub local_funding: f32,
    pub reserved: u32,
}

impl From<&BudgetItem> for fc4_pollution_budget_item {
    fn from(i: &BudgetItem) -> Self {
        fc4_pollution_budget_item {
            department: i.department_id,
            line: i.line_item,
            purpose: i.purpose,
            index: i.index,
            cost: i.cost,
            local_funding: i.local_funding,
            reserved: 0,
        }
    }
}

impl From<&fc4_pollution_budget_item> for BudgetItem {
    fn from(i: &fc4_pollution_budget_item) -> Self {
        BudgetItem {
            department_id: i.department,
            line_item: i.line,
            purpose: i.purpose,
            cost: i.cost,
            index: i.index,
            local_funding: i.local_funding,
        }
    }
}

/// One grid's statistics (fc4.h `fc4_pollution_grid_stats`): tracts, non-zero
/// tracts, sum, minimum and maximum of the raw int16 values.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_pollution_grid_stats {
    pub tracts: u32,
    pub nonzero: u32,
    pub sum: i64,
    pub min: i32,
    pub max: i32,
}

/// Diagnostics (fc4.h `fc4_pollution_state`). Grids in save order: air,
/// water, garbage, radiation sources, then the four value grids.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_pollution_state {
    pub struct_size: u32,
    pub initialized: u8,
    pub loaded: u8,
    pub garbage_uncollected: u8,
    pub effects_visible: u8,
    pub tracts_per_day: u32,
    pub cycle_resets: u32,
    pub reentrant_calls: u32,
    /// Water treatment, incinerators, waste-to-energy, recycling.
    pub lists: [u32; 4],
    pub avg_air: i32,
    pub avg_water: i32,
    pub avg_garbage: u32,
    pub total_uncollected: u32,
    pub garbage_supply: i32,
    pub garbage_capacity: i32,
    pub active_landfill_cells: u32,
    pub developed_tracts: u32,
    pub efficiency: f32,
    pub recycling_effect: f32,
    pub cell_x: u32,
    pub cell_z: u32,
    pub reserved: u32,
    /// Last month's counters: produced, imported, exported, recycled
    /// (truncated), incinerated, waste-to-energy, landfilled.
    pub last_month: [i64; 7],
    /// This month's, in the same order.
    pub month: [i64; 7],
    pub last_month_cost: i64,
    pub department: u64,
    pub grids: [fc4_pollution_grid_stats; 8],
}

/// Everything the pollution simulator asks of the game (fc4.h
/// `fc4_pollution_host`; one callback per `PollutionHost` method, in the
/// trait's order).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_pollution_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    pub grids: fc4_grid_api,
    // city
    pub cell_counts: Option<unsafe extern "C" fn(U, *mut i32, *mut i32)>,
    pub cell_widths: Option<unsafe extern "C" fn(U, *mut f32, *mut f32)>,
    pub position_to_cell: Option<unsafe extern "C" fn(U, f32, f32, *mut i32, *mut i32)>,
    /// A NEW fc4_props (the core destroys it), NULL if the exemplar is missing.
    pub tuning: Option<unsafe extern "C" fn(U) -> *mut fc4_props>,
    pub altitude: Option<unsafe extern "C" fn(U, f32, f32) -> f32>,
    // simulator, messages, history
    pub add_agent: Option<unsafe extern "C" fn(U, u32, *const c_char, usize, u32)>,
    pub remove_agent: Option<unsafe extern "C" fn(U)>,
    pub subscribe: Option<unsafe extern "C" fn(U, *const u32, usize)>,
    pub unsubscribe: Option<unsafe extern "C" fn(U, *const u32, usize)>,
    pub post: Option<unsafe extern "C" fn(U, u32, u32)>,
    pub record_history: Option<unsafe extern "C" fn(U, u32, i32)>,
    pub set_history_queue_length: Option<unsafe extern "C" fn(U, u32, u32)>,
    pub add_prop_requester: Option<unsafe extern "C" fn(U)>,
    pub remove_prop_requester: Option<unsafe extern "C" fn(U)>,
    // other simulators
    pub zone_type: Option<unsafe extern "C" fn(U, i32, i32) -> u32>,
    pub air_traffic: Option<unsafe extern "C" fn(U, i32, i32) -> u8>,
    pub cumulative_factor: Option<unsafe extern "C" fn(U, u32, u32) -> f32>,
    pub find_deals: Option<unsafe extern "C" fn(U, u32, fc4_token_callback, *mut c_void)>,
    pub deal_amount: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub deal_amount_used: Option<unsafe extern "C" fn(U, u64) -> i32>,
    pub set_deal_amount_used: Option<unsafe extern "C" fn(U, u64, i32)>,
    pub mark_deals_dirty: Option<unsafe extern "C" fn(U)>,
    pub is_occupant_powered: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub population: Option<unsafe extern "C" fn(U) -> i32>,
    pub landfill_usage: Option<unsafe extern "C" fn(U, *mut u32, *mut u32)>,
    pub landfill_tile_count: Option<unsafe extern "C" fn(U) -> u32>,
    pub set_lot_condition: Option<unsafe extern "C" fn(U, i32, i32, u32)>,
    // occupants
    pub occupant_property: Option<unsafe extern "C" fn(U, u64, u32, *mut fc4_props) -> bool>,
    pub set_occupant_property: Option<unsafe extern "C" fn(U, u64, u32, u16, bool, *const c_void, usize)>,
    pub remove_occupant_property: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub occupant_position: Option<unsafe extern "C" fn(U, u64, *mut f32)>,
    pub occupant_bounding_box: Option<unsafe extern "C" fn(U, u64, *mut f32, *mut f32)>,
    // host objects
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    pub resolve_ref: Option<unsafe extern "C" fn(U, u64, u32) -> u64>,
    // budget
    pub create_department: Option<unsafe extern "C" fn(U, u32, u32) -> u64>,
    pub set_department_name: Option<unsafe extern "C" fn(U, u64, u32, u32)>,
    pub set_department_max_funding: Option<unsafe extern "C" fn(U, u64, f32)>,
    pub create_line_item: Option<unsafe extern "C" fn(U, u64, u32, bool, u32, u32)>,
    pub department_funding: Option<unsafe extern "C" fn(U, u64) -> f32>,
    pub set_line_item_full_expenses: Option<unsafe extern "C" fn(U, u64, u32, i64)>,
    pub budget_item_for_purpose: Option<unsafe extern "C" fn(U, u64, u32, *mut fc4_pollution_budget_item) -> bool>,
    pub add_budget_item_to_budget: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub remove_budget_item_from_budget: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub change_budget_item_cost: Option<unsafe extern "C" fn(U, u64, u32, i64)>,
    pub add_to_budget_item_line_expenses: Option<unsafe extern "C" fn(U, *const fc4_pollution_budget_item, i64)>,
    // visual effects
    pub create_visual_effect: Option<unsafe extern "C" fn(U, u32, u32) -> u64>,
    pub set_effect_position: Option<unsafe extern "C" fn(U, u64, *const f32)>,
    pub set_effect_param: Option<unsafe extern "C" fn(U, u64, u32, f32)>,
    pub start_effect: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub stop_effect: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub is_effect_started: Option<unsafe extern "C" fn(U, u64) -> bool>,
}

const _: () = {
    assert!(std::mem::size_of::<fc4_pollution_budget_item>() == 32);
    assert!(std::mem::size_of::<fc4_pollution_grid_stats>() == 24);
    assert!(std::mem::size_of::<fc4_pollution_state>() == 408);
    assert!(std::mem::size_of::<fc4_pollution_host>() == if cfg!(target_pointer_width = "32") { 256 } else { 512 });
};

// ---- host adapter -----------------------------------------------------------------------------------

pub(crate) struct CHost(pub(crate) fc4_pollution_host);

macro_rules! cb {
    ($s:expr, $f:ident, $default:expr $(, $arg:expr)*) => {
        // SAFETY: whoever created the object promised valid callbacks.
        match $s.0.$f { Some(f) => unsafe { f($s.0.user $(, $arg)*) }, None => $default }
    };
}

fn opt(t: u64) -> Option<u64> {
    (t != 0).then_some(t)
}

impl CHost {
    /// `cISC4NeighborDeal` amounts of the deals `find_deals` lists.
    pub(crate) fn deals(&self, deal_type: u32) -> Vec<Deal> {
        collect_tokens(|sink, ctx| cb!(self, find_deals, (), deal_type, sink, ctx))
            .into_iter()
            .map(|token| Deal {
                token,
                amount: cb!(self, deal_amount, 0, token),
                amount_used: cb!(self, deal_amount_used, 0, token),
            })
            .collect()
    }
}

impl GridProvider<i16> for CHost {
    fn create_grid(&self, instance: u32, tract_size: i32) -> Option<Rc<dyn TractGrid<i16>>> {
        create_grid::<i16>(&self.0.grids, instance, tract_size, true)
    }
    fn resolve_grid(&self, r: &RefValue) -> Option<Rc<dyn TractGrid<i16>>> {
        resolve_grid::<i16>(&self.0.grids, r, true)
    }
}

impl PollutionHost for CHost {
    fn cell_counts(&self) -> (i32, i32) {
        let (mut x, mut z) = (0, 0);
        cb!(self, cell_counts, (), &mut x, &mut z);
        (x, z)
    }
    fn cell_widths(&self) -> (f32, f32) {
        let (mut x, mut z) = (16.0f32, 16.0f32);
        cb!(self, cell_widths, (), &mut x, &mut z);
        (x, z)
    }
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32) {
        let (mut cx, mut cz) = (0, 0);
        cb!(self, position_to_cell, (), x, z, &mut cx, &mut cz);
        (cx, cz)
    }
    fn tuning(&self) -> Option<Rc<PropertyHolder>> {
        let p = cb!(self, tuning, std::ptr::null_mut());
        // SAFETY: the host hands over a NEW fc4_props (or NULL).
        unsafe { take_props(p) }
    }
    fn altitude(&self, x: f32, z: f32) -> f32 {
        cb!(self, altitude, 0.0, x, z)
    }
    fn add_agent(&self, level: u32, name: &str, flags: u32) {
        cb!(
            self,
            add_agent,
            (),
            level,
            name.as_ptr() as *const c_char,
            name.len(),
            flags
        )
    }
    fn remove_agent(&self) {
        cb!(self, remove_agent, ())
    }
    fn subscribe(&self, msg_types: &[u32]) {
        cb!(self, subscribe, (), msg_types.as_ptr(), msg_types.len())
    }
    fn unsubscribe(&self, msg_types: &[u32]) {
        cb!(self, unsubscribe, (), msg_types.as_ptr(), msg_types.len())
    }
    fn post(&self, msg_type: u32, data1: u32) {
        cb!(self, post, (), msg_type, data1)
    }
    fn record_history(&self, series: u32, value: i32) {
        cb!(self, record_history, (), series, value)
    }
    fn set_history_queue_length(&self, series: u32, length: u32) {
        cb!(self, set_history_queue_length, (), series, length)
    }
    fn add_prop_requester(&self) {
        cb!(self, add_prop_requester, ())
    }
    fn remove_prop_requester(&self) {
        cb!(self, remove_prop_requester, ())
    }
    fn zone_type(&self, x: i32, z: i32) -> u32 {
        cb!(self, zone_type, 0, x, z)
    }
    fn air_traffic(&self, x: i32, z: i32) -> u8 {
        cb!(self, air_traffic, 0, x, z)
    }
    fn cumulative_factor(&self, id: u32, index: u32) -> f32 {
        cb!(self, cumulative_factor, 1.0, id, index)
    }
    fn find_deals(&self, deal_type: u32) -> Vec<Deal> {
        self.deals(deal_type)
    }
    fn set_deal_amount_used(&self, deal: u64, used: i32) {
        if deal != 0 {
            cb!(self, set_deal_amount_used, (), deal, used)
        }
    }
    fn mark_deals_dirty(&self) {
        cb!(self, mark_deals_dirty, ())
    }
    fn is_occupant_powered(&self, occ: u64) -> bool {
        cb!(self, is_occupant_powered, false, occ)
    }
    fn population(&self) -> i32 {
        cb!(self, population, 0)
    }
    fn landfill_usage(&self) -> (u32, u32) {
        let (mut usage, mut capacity) = (0u32, 0u32);
        cb!(self, landfill_usage, (), &mut usage, &mut capacity);
        (usage, capacity)
    }
    fn landfill_tile_count(&self) -> u32 {
        cb!(self, landfill_tile_count, 0)
    }
    fn set_lot_condition(&self, cell_x: i32, cell_z: i32, condition: u32) {
        cb!(self, set_lot_condition, (), cell_x, cell_z, condition)
    }
    fn occupant_property(&self, occ: u64, id: u32) -> Option<Variant> {
        let f = self.0.occupant_property?;
        // SAFETY: valid callback; the scratch holder outlives the call.
        read_property(id, |p| unsafe { f(self.0.user, occ, id, p) })
    }
    fn set_occupant_property(&self, occ: u64, id: u32, value: Variant) {
        let Some(n) = native_variant(&value) else { return };
        cb!(
            self,
            set_occupant_property,
            (),
            occ,
            id,
            n.mem_type,
            n.is_array,
            n.bytes.as_ptr() as *const c_void,
            n.count
        )
    }
    fn remove_occupant_property(&self, occ: u64, id: u32) {
        cb!(self, remove_occupant_property, (), occ, id)
    }
    fn occupant_position(&self, occ: u64) -> [f32; 3] {
        let mut p = [0.0f32; 3];
        cb!(self, occupant_position, (), occ, p.as_mut_ptr());
        p
    }
    fn occupant_bounding_box(&self, occ: u64) -> ([f32; 3], [f32; 3]) {
        let (mut a, mut b) = ([0.0f32; 3], [0.0f32; 3]);
        cb!(self, occupant_bounding_box, (), occ, a.as_mut_ptr(), b.as_mut_ptr());
        (a, b)
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
    fn create_department(&self, id: u32, group: u32) -> Option<u64> {
        opt(cb!(self, create_department, 0, id, group))
    }
    fn set_department_name(&self, dept: u64, group: u32, instance: u32) {
        cb!(self, set_department_name, (), dept, group, instance)
    }
    fn set_department_max_funding(&self, dept: u64, fraction: f32) {
        cb!(self, set_department_max_funding, (), dept, fraction)
    }
    fn create_line_item(&self, dept: u64, line: u32, locally_funded: bool, name_group: u32, name_instance: u32) {
        cb!(
            self,
            create_line_item,
            (),
            dept,
            line,
            locally_funded,
            name_group,
            name_instance
        )
    }
    fn department_funding(&self, dept: u64) -> f32 {
        cb!(self, department_funding, 1.0, dept)
    }
    fn set_line_item_full_expenses(&self, dept: u64, line: u32, value: i64) {
        cb!(self, set_line_item_full_expenses, (), dept, line, value)
    }
    fn budget_item_for_purpose(&self, occ: u64, purpose: u32) -> Option<BudgetItem> {
        let mut i = fc4_pollution_budget_item {
            purpose,
            ..Default::default()
        };
        cb!(self, budget_item_for_purpose, false, occ, purpose, &mut i).then(|| BudgetItem::from(&i))
    }
    fn add_budget_item_to_budget(&self, occ: u64, index: u32) {
        cb!(self, add_budget_item_to_budget, (), occ, index)
    }
    fn remove_budget_item_from_budget(&self, occ: u64, index: u32) {
        cb!(self, remove_budget_item_from_budget, (), occ, index)
    }
    fn change_budget_item_cost(&self, occ: u64, index: u32, cost: i64) {
        cb!(self, change_budget_item_cost, (), occ, index, cost)
    }
    fn add_to_budget_item_line_expenses(&self, item: &BudgetItem, delta: i64) {
        let i = fc4_pollution_budget_item::from(item);
        cb!(self, add_to_budget_item_line_expenses, (), &i, delta)
    }
    fn create_visual_effect(&self, id: u32, kind: u32) -> Option<u64> {
        opt(cb!(self, create_visual_effect, 0, id, kind))
    }
    fn set_effect_position(&self, effect: u64, position: [f32; 3]) {
        cb!(self, set_effect_position, (), effect, position.as_ptr())
    }
    fn set_effect_param(&self, effect: u64, param: u32, value: f32) {
        cb!(self, set_effect_param, (), effect, param, value)
    }
    fn start_effect(&self, effect: u64, t: u32) {
        cb!(self, start_effect, (), effect, t)
    }
    fn stop_effect(&self, effect: u64, t: u32) {
        cb!(self, stop_effect, (), effect, t)
    }
    fn is_effect_started(&self, effect: u64) -> bool {
        cb!(self, is_effect_started, false, effect)
    }
}

// ---- handles ----------------------------------------------------------------------------------------

/// Opaque handle (fc4.h `fc4_pollution`).
#[repr(C)]
pub struct fc4_pollution {
    _private: [u8; 0],
}

/// A message for the core: type, occupant (insert / remove), view level
/// (PreViewLevelChange).
#[derive(Clone, Copy, Debug)]
struct Msg {
    msg_type: u32,
    occupant: u64,
    view_level: u32,
}

/// The handle. `depth`, `reentrant` and `queue` belong to the main thread's
/// calls; a getter on another thread only reaches `sim` (through `peek`).
struct Handle {
    host: Rc<CHost>,
    sim: Rc<PollutionSim>,
    depth: AtomicU32,
    reentrant: AtomicU32,
    queue: Mutex<Vec<Msg>>,
}

/// Decrements the call depth when the call ends (also on a panic).
struct Depth<'a>(&'a AtomicU32);

impl Drop for Depth<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Handle {
    /// Runs `f` with the call depth counted (main thread); at the end of the
    /// outermost call, the queued messages are handled.
    fn call<R>(&self, f: impl FnOnce(&PollutionSim) -> R) -> R {
        if self.depth.load(Ordering::Relaxed) > 0 {
            self.reentrant.fetch_add(1, Ordering::Relaxed);
        }
        self.depth.fetch_add(1, Ordering::Relaxed);
        let _depth = Depth(&self.depth);
        let r = f(&self.sim);
        if self.depth.load(Ordering::Relaxed) == 1 {
            loop {
                let next = {
                    let mut q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                    if q.is_empty() {
                        None
                    } else {
                        Some(q.remove(0))
                    }
                };
                let Some(m) = next else { break };
                self.sim.do_message(m.msg_type, m.occupant, m.view_level);
            }
        }
        r
    }
}

fn handle<'a>(p: *const fc4_pollution) -> Option<&'a Handle> {
    // SAFETY: documented precondition: NULL or a live fc4_pollution*.
    unsafe { (p as *const Handle).as_ref() }
}

/// A main-thread call (lifecycle, messages, persistence, slot 66).
fn sim<R>(p: *const fc4_pollution, fallback: R, f: impl FnOnce(&PollutionSim) -> R) -> R {
    match handle(p) {
        Some(h) => guard(fallback, || h.call(f)),
        None => fallback,
    }
}

/// A slot that may run on any thread: the core object only.
fn peek<R>(p: *const fc4_pollution, fallback: R, f: impl FnOnce(&PollutionSim) -> R) -> R {
    match handle(p) {
        Some(h) => guard(fallback, || f(&h.sim)),
        None => fallback,
    }
}

/// Creates the core object (constructor values, §3). The host table is
/// copied. `token` is the host identity of the simulator itself;
/// `session_seed` is foundations §6.3's `S`: the visual-effect generator (the
/// original's file-local one, wall clock XOR `0x4C09C4D3`) takes
/// `derive(S, 0x4C09C4D3)`. It is process-wide (RN3): the first create seeds
/// it and later simulators continue its stream.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_create(
    host: *const fc4_pollution_host,
    token: u64,
    session_seed: u32,
) -> *mut fc4_pollution {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h) if (h.struct_size as usize) >= std::mem::size_of::<fc4_pollution_host>() && h.grids.complete() => {
            let host = Rc::new(CHost(*h));
            // The visual-effect generator is process-wide (RN3).
            let sim = PollutionSim::process_wide(host.clone(), token, session_seed);
            Box::into_raw(Box::new(Handle {
                host,
                sim,
                depth: AtomicU32::new(0),
                reentrant: AtomicU32::new(0),
                queue: Mutex::new(Vec::new()),
            })) as *mut fc4_pollution
        }
        _ => std::ptr::null_mut(),
    })
}

/// Destroys the core object. The references it still holds (a loaded object
/// that was never initialised: list occupants and the department) are
/// released; grids are shut down and released with their handles.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_destroy(p: *mut fc4_pollution) {
    if p.is_null() {
        return;
    }
    guard((), || {
        let b = Box::from_raw(p as *mut Handle);
        let held: Vec<u64> = b.sim.peek(|s| {
            if s.initialized {
                Vec::new()
            } else {
                s.lists.iter().flatten().copied().chain(s.department).collect()
            }
        });
        for t in held {
            b.host.release(t);
        }
        drop(b);
    });
}

/// `Init` (§4.1): always true for a live object.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_init(p: *mut fc4_pollution) -> bool {
    sim(p, false, PollutionSim::init)
}

/// `Shutdown` (§4.1): always true for a live object.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_shutdown(p: *mut fc4_pollution) -> bool {
    sim(p, false, PollutionSim::shutdown)
}

/// `DoMessage` (§5.3): always true. `occupant` is the message's occupant for
/// insert / remove (`0x99EF1142` / `0x99EF1143`), `view_level` the u32 that
/// void2 points at for PreViewLevelChange (`0xC6CDBF1A`). A message delivered
/// during another main-thread call is queued.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_do_message(
    p: *mut fc4_pollution,
    msg_type: u32,
    occupant: u64,
    view_level: u32,
) -> bool {
    let m = Msg {
        msg_type,
        occupant,
        view_level,
    };
    match handle(p) {
        Some(h) => guard(true, || {
            if h.depth.load(Ordering::Relaxed) > 0 {
                h.reentrant.fetch_add(1, Ordering::Relaxed);
                h.queue.lock().unwrap_or_else(|e| e.into_inner()).push(m);
                return true;
            }
            h.call(|s| s.do_message(m.msg_type, m.occupant, m.view_level))
        }),
        None => true,
    }
}

/// The argument-free slots that return a number (§8.1): 10, 11, 12, 20, 21,
/// 22, 23, 24, 31, 32, 34-41, 42-45, 67-70; booleans as 0 / 1. 0 for any
/// other slot. Slot 20 also stores its clamp. Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_slot_value(p: *const fc4_pollution, slot: u32) -> i64 {
    peek(p, 0, |s| match slot {
        10 => i64::from(s.average_air_value()),
        11 => i64::from(s.average_water_value()),
        12 => i64::from(s.average_garbage_value()),
        20 => i64::from(s.garbage_supply()),
        21 => i64::from(s.landfill_tile_capacity()),
        22 => i64::from(s.garbage_capacity()),
        23 => i64::from(s.is_garbage_uncollected()),
        24 => i64::from(s.total_garbage_uncollected()),
        31 => i64::from(s.monthly_incinerator_capacity()),
        32 => i64::from(s.monthly_wte_capacity()),
        34 => s.garbage_abducted_prev_month(),
        35 => s.last_month().produced,
        36 => s.last_month().imported,
        37 => s.last_month().exported,
        38 => s.garbage_recycled_last_month(),
        39 => s.last_month().incinerated,
        40 => s.last_month().wte,
        41 => s.last_month().landfilled,
        42 => i64::from(s.incinerator_count()),
        43 => i64::from(s.wte_building_count()),
        44 => i64::from(s.recycling_center_count()),
        45 => i64::from(s.water_treatment_plant_count()),
        67 => i64::from(s.max_level_for_ui(0)),
        68 => i64::from(s.max_level_for_ui(1)),
        69 => i64::from(s.max_level_for_ui(2)),
        70 => i64::from(s.max_level_for_ui(3)),
        _ => 0,
    })
}

/// The per-cell slots (§8.1): 7 / 8 / 9 (the value), 13-16 and 49 (0 / 1).
/// 0 for any other slot. Cell coordinates as the game passes them (uint32,
/// read as int32). Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_slot_cell(p: *const fc4_pollution, slot: u32, x: i32, z: i32) -> i32 {
    peek(p, 0, |s| match slot {
        7 => s.air_value(x, z),
        8 => s.water_value(x, z),
        9 => s.garbage_value(x, z),
        13 => i32::from(s.is_air_polluted(x, z)),
        14 => i32::from(s.is_water_polluted(x, z)),
        15 => i32::from(s.is_garbage_accumulated(x, z)),
        16 => i32::from(s.is_radioactive(x, z)),
        49 => i32::from(s.is_active_landfill(x, z)),
        _ => 0,
    })
}

/// Slot 5 `GetPollutionGrid(type)`: the game grid's token (borrowed, no
/// AddRef; 0 = none). Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_grid(p: *const fc4_pollution, kind: u32) -> u64 {
    peek(p, 0, |s| {
        s.with_pollution_grid(kind, |g| match g.map(|g| g.save_ref()) {
            Some(RefValue::Host { token, .. }) => token,
            _ => 0,
        })
    })
}

/// Slot 6 `GetPollutionValue(type, x, z)`. Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_value(p: *const fc4_pollution, kind: u32, x: i32, z: i32) -> u16 {
    peek(p, 0, |s| s.pollution_value(kind, x, z))
}

/// Slots 17-19 `GetWorst{Air,Water,Garbage}Tract` (kind 0 / 1 / 2). Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_worst_tract(
    p: *const fc4_pollution,
    kind: u32,
    x: *mut u32,
    z: *mut u32,
    v: *mut i32,
) -> bool {
    let w = peek(p, None, |s| match kind {
        0 => Some(s.worst_air_tract()),
        1 => Some(s.worst_water_tract()),
        2 => Some(s.worst_garbage_tract()),
        _ => None,
    });
    let Some(w) = w else { return false };
    if let Some(o) = x.as_mut() {
        *o = w.x;
    }
    if let Some(o) = z.as_mut() {
        *o = w.z;
    }
    if let Some(o) = v.as_mut() {
        *o = w.value;
    }
    true
}

/// The per-cell setters (§8.1): 25 `SetIsRadioactive` (v ≠ 0), 26 / 27 / 28
/// `Set{Air,Water,Garbage}Value` (v as int16), 50 `SetIsActiveLandfill`
/// (v ≠ 0). Any thread (they lock the state; slot 50 asks the zone manager).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_set_cell(p: *mut fc4_pollution, slot: u32, x: i32, z: i32, v: i32) -> bool {
    peek(p, false, |s| {
        match slot {
            25 => s.set_is_radioactive(x, z, v != 0),
            26 => s.set_air_value(x, z, v as i16),
            27 => s.set_water_value(x, z, v as i16),
            28 => s.set_garbage_value(x, z, v as i16),
            50 => s.set_is_active_landfill(x, z, v != 0),
            _ => return false,
        }
        true
    })
}

/// Slot 29 `AddToGarbageSupply(n)`: the applied delta, taken before the final
/// clamp at 0 (§8.1, PG11).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_add_to_garbage_supply(p: *mut fc4_pollution, n: i32) -> i32 {
    peek(p, 0, |s| s.add_to_garbage_supply(n))
}

/// Slot 30 `SetGarbageCapacity(n)`: the argument.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_set_garbage_capacity(p: *mut fc4_pollution, n: i32) -> i32 {
    peek(p, n, |s| s.set_garbage_capacity(n))
}

/// Slot 51 `GarbagePutInLandfill(n)`: the new total (§8.1).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_garbage_put_in_landfill(p: *mut fc4_pollution, n: i32) -> i64 {
    peek(p, 0, |s| s.garbage_put_in_landfill(n))
}

/// Slot 33: the value `GetRecyclingCenterEffect` points at (the plugin keeps
/// it in a member and returns that member's address).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_recycling_center_effect(p: *const fc4_pollution) -> f32 {
    peek(p, 0.0, PollutionSim::recycling_center_effect)
}

/// Slot 48 `GetGarbageScalingFactor`.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_garbage_scaling_factor(p: *const fc4_pollution) -> f32 {
    peek(p, 0.0, PollutionSim::garbage_scaling_factor)
}

/// Slot 46 `GetGarbageBuildings`: incinerators, then waste-to-energy, then
/// recycling, as **borrowed** tokens (the core's lists hold the references);
/// the caller AddRefs each one it appends. Returns the count. Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_garbage_buildings(
    p: *const fc4_pollution,
    cb: fc4_token_callback,
    ctx: *mut c_void,
) -> usize {
    let list: Vec<u64> = peek(p, Vec::new(), |s| {
        s.peek(|st| {
            [L_INC, L_WTE, L_REC]
                .iter()
                .flat_map(|&l| st.lists[l].iter().copied())
                .collect()
        })
    });
    if let Some(f) = cb {
        for &t in &list {
            f(ctx, t);
        }
    }
    list.len()
}

/// Slot 47 `GetBuildingGarbageCapacity(occ)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_building_garbage_capacity(p: *const fc4_pollution, occ: u64) -> u32 {
    peek(p, 0, |s| s.building_garbage_capacity(occ))
}

/// The occupant slots with a u32 result (§8.1): 52 `GetMaxCapacity(prop)`,
/// 53 `GetCurrentCapacity(prop)`, 58-64 (`prop` ignored). `*out` gets the
/// value the slot writes (0 when it fails); returns the slot's bool.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_occupant_u32(
    p: *const fc4_pollution,
    slot: u32,
    occ: u64,
    prop: u32,
    out: *mut u32,
) -> bool {
    let (ok, v) = peek(p, (false, 0), |s| match slot {
        52 => s.max_capacity(prop, occ),
        53 => s.current_capacity(prop, occ),
        58 => s.incinerator_used_capacity(occ),
        59 => s.wte_used_capacity(occ),
        60 => s.incinerator_current_capacity(occ),
        61 => s.wte_current_capacity(occ),
        62 => s.water_treatment_max_capacity(occ),
        63 => s.water_treatment_current_capacity(occ),
        64 => s.water_treatment_used_capacity(occ),
        _ => (false, 0),
    });
    if let Some(o) = out.as_mut() {
        *o = v;
    }
    ok
}

/// The occupant slots with an `SC4Percentage` result (§8.1): 54 max
/// recycling, 55 current recycling, 56 condition, 57 efficiency. `*out` is
/// written only when the slot writes a value: on success, and for slot 56
/// also on failure (1.0); 54, 55 and 57 leave it ("p := default") otherwise.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_occupant_percent(
    p: *const fc4_pollution,
    slot: u32,
    occ: u64,
    out: *mut f32,
) -> bool {
    let (ok, v) = peek(p, (false, 0.0), |s| match slot {
        54 => s.recycling_center_max_capacity(occ),
        55 => s.recycling_center_current_capacity(occ),
        56 => s.current_occupant_condition(occ),
        57 => s.current_occupant_efficiency(occ),
        _ => (false, 0.0),
    });
    if ok || slot == 56 {
        if let Some(o) = out.as_mut() {
            *o = v;
        }
    }
    ok
}

/// Slot 65 `CalcOccupantMaintenanceCost(occ, int64& c)`: `*out` is always
/// written (0, the base, or the cost).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_maintenance_cost(p: *const fc4_pollution, occ: u64, out: *mut i64) -> bool {
    let (ok, v) = peek(p, (false, 0), |s| s.calc_occupant_maintenance_cost(occ));
    if let Some(o) = out.as_mut() {
        *o = v;
    }
    ok
}

/// Slot 66 `AddRadiationToCity(position, strength, radius)` (main thread).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_add_radiation(
    p: *mut fc4_pollution,
    x: f32,
    y: f32,
    z: f32,
    strength: f32,
    radius: f32,
) {
    sim(p, (), |s| s.add_radiation_to_city([x, y, z], strength, radius))
}

/// `cISC4PropRequester` slot 3 `GetRequestedShare(rect)` (inclusive cells).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_requested_share(
    p: *const fc4_pollution,
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
) -> i32 {
    peek(p, 0, |s| s.requested_share(x0, z0, x1, z1))
}

/// `cISC4PropRequester` slots 4 (`GetRequesterSatisfactionId`), 5
/// (`UsesCustomPropSelector`, 0 / 1) and 6 (`SelectProp`); 0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_prop_requester_slot(p: *const fc4_pollution, slot: u32) -> u32 {
    peek(p, 0, |s| match slot {
        4 => s.requester_satisfaction_id(),
        5 => u32::from(s.uses_custom_prop_selector()),
        6 => s.select_prop(&[], 0, 0),
        _ => 0,
    })
}

fn grid_stats(g: &Option<Rc<dyn TractGrid<i16>>>) -> fc4_pollution_grid_stats {
    let Some(g) = g else {
        return fc4_pollution_grid_stats::default();
    };
    let geo = g.geometry();
    let (nx, nz) = (geo.count_x.clamp(0, 4096), geo.count_z.clamp(0, 4096));
    let mut st = fc4_pollution_grid_stats {
        tracts: (nx * nz) as u32,
        min: i32::MAX,
        max: i32::MIN,
        ..Default::default()
    };
    for x in 0..nx {
        for z in 0..nz {
            let v = i32::from(g.get(x, z));
            st.nonzero += u32::from(v != 0);
            st.sum += i64::from(v);
            st.min = st.min.min(v);
            st.max = st.max.max(v);
        }
    }
    if st.tracts == 0 {
        st.min = 0;
        st.max = 0;
    }
    st
}

fn counters(c: &fc4_core::pollution::Counters) -> [i64; 7] {
    [
        c.produced,
        c.imported,
        c.exported,
        ftol(c.recycled),
        c.incinerated,
        c.wte,
        c.landfilled,
    ]
}

/// Fills a diagnostics block from a state (also used by the shadow).
pub(crate) fn state_of(s: &PollutionState, cycle_resets: u32, reentrant: u32) -> fc4_pollution_state {
    let mut grids = [fc4_pollution_grid_stats::default(); 8];
    for (o, g) in grids.iter_mut().zip(s.grids.iter()) {
        *o = grid_stats(g);
    }
    fc4_pollution_state {
        struct_size: std::mem::size_of::<fc4_pollution_state>() as u32,
        initialized: u8::from(s.initialized),
        loaded: u8::from(s.loaded),
        garbage_uncollected: u8::from(s.garbage_uncollected),
        effects_visible: u8::from(s.effects_visible),
        tracts_per_day: s.tracts_per_day,
        cycle_resets,
        reentrant_calls: reentrant,
        lists: [
            s.lists[L_WTP].len() as u32,
            s.lists[L_INC].len() as u32,
            s.lists[L_WTE].len() as u32,
            s.lists[L_REC].len() as u32,
        ],
        avg_air: s.avg_air,
        avg_water: s.avg_water,
        avg_garbage: u32::from(s.avg_garbage),
        total_uncollected: s.total_uncollected,
        garbage_supply: s.garbage_supply,
        garbage_capacity: s.garbage_capacity,
        active_landfill_cells: s.active_landfill_cells,
        developed_tracts: s.developed_tracts,
        efficiency: s.efficiency,
        recycling_effect: s.recycling_effect,
        cell_x: s.cell_x,
        cell_z: s.cell_z,
        reserved: 0,
        last_month: counters(&s.last_month),
        month: counters(&s.month),
        last_month_cost: s.last_month_cost,
        department: s.department.unwrap_or(0),
        grids,
    }
}

/// Diagnostics (main thread: it walks the eight grids).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_get_state(p: *const fc4_pollution, out: *mut fc4_pollution_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    let Some(h) = handle(p) else { return false };
    guard(false, || {
        let s = h.sim.snapshot();
        *o = state_of(&s, h.sim.cycle_resets(), h.reentrant.load(Ordering::Relaxed));
        true
    })
}

/// The token of grid `index` in save order (0-3 sources, 4-7 values; 0 =
/// none), borrowed. Any thread.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_grid_by_index(p: *const fc4_pollution, index: u32) -> u64 {
    peek(p, 0, |s| {
        if index as usize >= GRID_INSTANCES.len() {
            return 0;
        }
        s.peek(|st| grid_token(&st.grids[index as usize]))
    })
}

/// `Write` (§6). Grid, occupant and department references go out as
/// `fc4_ref`s with the token (grids with their class ID).
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_write(p: *const fc4_pollution, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    sim(p, false, |s| s.write(&mut COut(o)))
}

fn no_core_objects(_: u32, _: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    None
}

/// `Read` (§6): versions 0-4; false (nothing changed) for a newer version or
/// a failed stream. Runs before `Init`: nothing is sized from the city.
#[no_mangle]
pub unsafe extern "C" fn fc4_pollution_read(p: *mut fc4_pollution, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    sim(p, false, |s| {
        s.read(&mut CIn {
            stream: i,
            decode: no_core_objects,
        })
    })
}
