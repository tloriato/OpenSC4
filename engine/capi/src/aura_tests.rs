// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The aura C ABI (fc4.h "aura") against a fake game: the aura's int8 /
//! int16 grids and the other simulators' input grids through `fc4_grid_api`,
//! a lot iterator, occupants and their properties, persistence, getters from
//! a second thread during a main-thread call, and re-entrant calls.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::mpsc;
use std::time::Duration;

use fc4_core::aura::{grid_id, ids, msg, shipped_tuning, AGENT_LEVELS, AGENT_NAME};
use fc4_core::variant::mem;

use crate::aura::*;
use crate::hostgrid::fc4_grid_api;
use crate::landvalue::fc4_grid_geometry;
use crate::props::{fc4_props, fc4_props_set_typed};
use crate::test_streams::{istream, ostream, In, Out};

pub(crate) const CELLS: i32 = 32;
const UINT8_GRID: u32 = 0x49B9_E602;
const UINT16_GRID: u32 = 0x49B9_E604;
pub(crate) const LOT_BUILDING: u64 = 0x55;
pub(crate) const PARK: u64 = 0x66;
const ITERATOR: u64 = 0x900;
pub(crate) const CRIME: u64 = 0x800;
pub(crate) const POP: [u64; 3] = [0x810, 0x820, 0x830];

pub(crate) struct Grid {
    pub(crate) data: Vec<f64>,
    pub(crate) clsid: u32,
    pub(crate) tract_size: i32,
    pub(crate) refs: i32,
    pub(crate) shut: bool,
    pub(crate) instance: u32,
}

impl Grid {
    fn count(&self) -> i32 {
        CELLS / self.tract_size
    }
}

/// (element type, array flag, bytes, count).
type Prop = (u16, bool, Vec<u8>, usize);

#[derive(Default)]
pub(crate) struct Game {
    pub(crate) grids: RefCell<BTreeMap<u64, Grid>>,
    next_grid: Cell<u64>,
    pub(crate) agents: RefCell<Vec<(u32, String, u32)>>,
    pub(crate) subs: RefCell<Vec<u32>>,
    pub(crate) unsubs: RefCell<Vec<u32>>,
    pub(crate) posts: RefCell<Vec<u32>>,
    pub(crate) history: RefCell<Vec<(u32, i32)>>,
    pub(crate) strikes: RefCell<Vec<u32>>,
    pub(crate) disasters: RefCell<Vec<(u32, i32, i32)>>,
    pub(crate) base_aura: Cell<[i32; 3]>,
    pub(crate) props: RefCell<BTreeMap<(u64, u32), Prop>>,
    /// Lot iterators handed out and not yet released.
    pub(crate) iterators: Cell<i32>,
    /// Lots handed out per live iterator (the shadow's runs beside the
    /// original's).
    pub(crate) iterated: RefCell<BTreeMap<u64, u32>>,
    next_iterator: Cell<u64>,
    pub(crate) no_lots: Cell<bool>,
    pub(crate) funding: Cell<f32>,
    pub(crate) inputs_asked: RefCell<Vec<u32>>,
    /// Second-thread test: the handle to read from another thread.
    pub(crate) probe: Cell<usize>,
    pub(crate) probes: Cell<u32>,
    /// Re-entrancy test: the handle to call back into.
    pub(crate) reenter: Cell<usize>,
}

pub(crate) fn game(u: *mut c_void) -> &'static Game {
    unsafe { &*(u as *const Game) }
}

impl Game {
    pub(crate) fn new() -> Box<Game> {
        let g = Box::<Game>::default();
        g.funding.set(0.5);
        // The other simulators' grids: crime 0 everywhere, R$ population 100 in tract (0, 0).
        g.input(CRIME, UINT8_GRID, 2, 0.0);
        for (i, t) in POP.iter().enumerate() {
            g.input(*t, UINT16_GRID, 4, 0.0);
            if i == 0 {
                g.grids.borrow_mut().get_mut(t).unwrap().data[0] = 100.0;
            }
        }
        // The park: effect {20, 8} (magnitude, radius in cells).
        g.set(PARK, ids::PROP_PARK_EFFECT, mem::SINT32, true, &[20i32, 8]);
        g
    }

    fn input(&self, t: u64, clsid: u32, tract_size: i32, v: f64) {
        let n = (CELLS / tract_size) as usize;
        self.grids.borrow_mut().insert(
            t,
            Grid {
                data: vec![v; n * n],
                clsid,
                tract_size,
                refs: 1,
                shut: false,
                instance: 0,
            },
        );
    }

    pub(crate) fn set<T: Copy>(&self, occ: u64, id: u32, t: u16, array: bool, v: &[T]) {
        let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }.to_vec();
        self.props.borrow_mut().insert((occ, id), (t, array, bytes, v.len()));
    }

    pub(crate) fn user(&self) -> *mut c_void {
        self as *const Game as *mut c_void
    }

    pub(crate) fn value(&self, t: u64, x: i32, z: i32) -> f64 {
        let grids = self.grids.borrow();
        let g = &grids[&t];
        g.data[(x * g.count() + z) as usize]
    }

    /// The token of the aura's grid with this instance (the first one).
    pub(crate) fn grid_by_instance(&self, instance: u32, tract_size: i32) -> u64 {
        *self
            .grids
            .borrow()
            .iter()
            .find(|(_, g)| g.instance == instance && g.tract_size == tract_size)
            .expect("grid")
            .0
    }

    /// Reads the handle from another thread while this (main) thread is
    /// inside a host call; a getter blocked on the state would time out.
    fn probe_now(&self) {
        let p = self.probe.get();
        if p == 0 {
            return;
        }
        self.probes.set(self.probes.get() + 1);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let p = p as *const fc4_aura;
            let mut r = fc4_aura_ratings::default();
            let ok = unsafe { fc4_aura_get_ratings(p, &mut r) };
            let g = unsafe { fc4_aura_grid(p, FC4_AURA_GRID_AURA) };
            let _ = tx.send((ok, r.aura_grid, g));
        });
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok((ok, a, g)) => assert!(ok && a != 0 && a == g, "the getters answer from the snapshot"),
            Err(_) => panic!("a getter on another thread blocked during a main-thread host call"),
        }
    }
}

// ---- grids --------------------------------------------------------------------------------------------

unsafe extern "C" fn g_create(u: *mut c_void, clsid: u32, instance: u32, ts: i32) -> u64 {
    assert!(clsid == grid_id::CLSID_INT8 || clsid == grid_id::CLSID_INT16);
    assert!(ts == 1 || ts == 4);
    let g = game(u);
    let t = g.next_grid.get() + 0x7000;
    g.next_grid.set(g.next_grid.get() + 0x10);
    let n = (CELLS / ts) as usize;
    g.grids.borrow_mut().insert(
        t,
        Grid {
            data: vec![0.0; n * n],
            clsid,
            tract_size: ts,
            refs: 1,
            shut: false,
            instance,
        },
    );
    t
}
pub(crate) unsafe extern "C" fn g_class(u: *mut c_void, t: u64) -> u32 {
    game(u).grids.borrow().get(&t).map_or(0, |g| g.clsid)
}
pub(crate) unsafe extern "C" fn g_geometry(u: *mut c_void, t: u64, out: *mut fc4_grid_geometry) {
    let ts = game(u).grids.borrow().get(&t).map_or(1, |g| g.tract_size);
    let w = 16.0 * ts as f32;
    *out = fc4_grid_geometry {
        tract_size: ts,
        shift: ts.trailing_zeros() as i32,
        count_x: CELLS / ts,
        count_z: CELLS / ts,
        width_x: w,
        width_z: w,
        inv_width_x: 1.0 / w,
        inv_width_z: 1.0 / w,
    };
}
pub(crate) unsafe extern "C" fn g_get(u: *mut c_void, t: u64, kind: u32, x: i32, z: i32) -> f64 {
    let grids = game(u).grids.borrow();
    let g = &grids[&t];
    assert_eq!(kind, g.clsid, "read with the grid's own element type");
    let n = g.count();
    if x < 0 || z < 0 || x >= n || z >= n {
        return 0.0;
    }
    g.data[(x * n + z) as usize]
}
unsafe extern "C" fn g_set(u: *mut c_void, t: u64, kind: u32, x: i32, z: i32, v: f64) {
    let mut grids = game(u).grids.borrow_mut();
    let g = grids.get_mut(&t).unwrap();
    assert_eq!(kind, g.clsid);
    assert!(g.instance != 0, "an input grid is never written");
    let n = g.count();
    if x >= 0 && z >= 0 && x < n && z < n {
        g.data[(x * n + z) as usize] = v;
    }
}
unsafe extern "C" fn g_fill(u: *mut c_void, t: u64, _kind: u32, v: f64) {
    game(u).grids.borrow_mut().get_mut(&t).unwrap().data.fill(v);
}
unsafe extern "C" fn g_shutdown(u: *mut c_void, t: u64) {
    game(u).grids.borrow_mut().get_mut(&t).unwrap().shut = true;
}
unsafe extern "C" fn g_release(u: *mut c_void, t: u64) {
    game(u).grids.borrow_mut().get_mut(&t).unwrap().refs -= 1;
}

// ---- city, lots, occupants --------------------------------------------------------------------------------

unsafe extern "C" fn cell_width(_: *mut c_void, x: *mut f32, z: *mut f32) {
    *x = 16.0;
    *z = 16.0;
}
unsafe extern "C" fn position_to_cell(_: *mut c_void, x: f32, z: f32, cx: *mut i32, cz: *mut i32) {
    *cx = (x / 16.0).floor() as i32;
    *cz = (z / 16.0).floor() as i32;
}
unsafe extern "C" fn sim_date_number(_: *mut c_void) -> u32 {
    100
}
pub(crate) unsafe extern "C" fn tuning(_: *mut c_void) -> *mut fc4_props {
    Box::into_raw(Box::new(shipped_tuning())) as *mut fc4_props
}
unsafe extern "C" fn publish_base_aura(u: *mut c_void, v: *const i32) {
    game(u).base_aura.set([*v, *v.add(1), *v.add(2)]);
}
pub(crate) unsafe extern "C" fn lot_count(u: *mut c_void) -> u32 {
    u32::from(!game(u).no_lots.get())
}
pub(crate) unsafe extern "C" fn lot_iterator(u: *mut c_void) -> u64 {
    let g = game(u);
    g.iterators.set(g.iterators.get() + 1);
    let t = ITERATOR + g.next_iterator.get();
    g.next_iterator.set(g.next_iterator.get() + 1);
    g.iterated.borrow_mut().insert(t, 0);
    t
}
/// One lot: cells (0..3, 0..3), R$, wealth 1, ten R$ residents and two
/// commercial workers.
pub(crate) unsafe extern "C" fn lot_next(
    u: *mut c_void,
    it: u64,
    out: *mut fc4_aura_lot,
    sink: fc4_aura_population_sink,
    ctx: *mut c_void,
) -> bool {
    let g = game(u);
    let mut iterated = g.iterated.borrow_mut();
    let n = iterated.get_mut(&it).expect("a live lot iterator");
    if g.no_lots.get() || *n > 0 {
        return false;
    }
    *n = 1;
    drop(iterated);
    *out = fc4_aura_lot {
        x1: 0,
        z1: 0,
        x2: 3,
        z2: 3,
        zone_type: 1,
        occupant_wealth: 1,
        building: LOT_BUILDING,
    };
    if let Some(f) = sink {
        f(ctx, 0x1010, 10);
    }
    true
}
pub(crate) unsafe extern "C" fn lot_iterator_release(u: *mut c_void, it: u64) {
    let g = game(u);
    assert!(g.iterated.borrow_mut().remove(&it).is_some(), "a live lot iterator");
    g.iterators.set(g.iterators.get() - 1);
}
pub(crate) unsafe extern "C" fn lot_at(u: *mut c_void, x: i32, z: i32, out: *mut fc4_aura_lot) -> bool {
    if game(u).no_lots.get() || !(0..4).contains(&x) || !(0..4).contains(&z) {
        return false;
    }
    *out = fc4_aura_lot {
        x1: 0,
        z1: 0,
        x2: 3,
        z2: 3,
        zone_type: 1,
        occupant_wealth: 1,
        building: 0,
    };
    true
}
pub(crate) unsafe extern "C" fn land_value_type(_: *mut c_void, _x: i32, _z: i32) -> i32 {
    3
}
pub(crate) unsafe extern "C" fn occupant_has_property(u: *mut c_void, occ: u64, id: u32) -> bool {
    game(u).props.borrow().contains_key(&(occ, id))
}
pub(crate) unsafe extern "C" fn occupant_property(u: *mut c_void, occ: u64, id: u32, p: *mut fc4_props) -> bool {
    match game(u).props.borrow().get(&(occ, id)) {
        Some((t, a, b, n)) => fc4_props_set_typed(p, id, *t, *a, b.as_ptr() as *const c_void, *n),
        None => false,
    }
}
pub(crate) unsafe extern "C" fn occupant_type(_: *mut c_void, _occ: u64) -> u32 {
    0x278128A0
}
pub(crate) unsafe extern "C" fn occupant_position(_: *mut c_void, _occ: u64, out: *mut f32) {
    *out = 8.0;
    *out.add(1) = 0.0;
    *out.add(2) = 8.0;
}
pub(crate) unsafe extern "C" fn department_funding(u: *mut c_void, dept: u32, out: *mut f32) -> bool {
    if dept != ids::DEPARTMENT_PARKS {
        return false;
    }
    *out = game(u).funding.get();
    true
}
/// The landmark occupants of the state-3 test (0x77, 0x78) belong to
/// department 0x1234 (a department grid with a non-zero instance).
pub(crate) unsafe extern "C" fn budget_item_department(_: *mut c_void, occ: u64, _purpose: u32, out: *mut u32) -> bool {
    if occ != 0x77 && occ != 0x78 {
        return false;
    }
    *out = 0x1234;
    true
}
pub(crate) unsafe extern "C" fn tax_rate(_: *mut c_void, _index: i32) -> f32 {
    9.0
}
pub(crate) unsafe extern "C" fn cumulative_offset(_: *mut c_void, _prop: u32, _index: u32) -> i32 {
    0
}
pub(crate) unsafe extern "C" fn input_grid(u: *mut c_void, which: u32) -> u64 {
    game(u).inputs_asked.borrow_mut().push(which);
    match which {
        FC4_AURA_INPUT_CRIME => CRIME,
        FC4_AURA_INPUT_POP1 => POP[0],
        FC4_AURA_INPUT_POP2 => POP[1],
        FC4_AURA_INPUT_POP3 => POP[2],
        _ => 0,
    }
}

// ---- effects and plumbing ---------------------------------------------------------------------------

unsafe extern "C" fn record_history(u: *mut c_void, series: u32, value: i32) {
    let g = game(u);
    g.history.borrow_mut().push((series, value));
    g.probe_now();
    let p = g.reenter.replace(0);
    if p != 0 {
        // A transient effect from inside the month's call: queued.
        fc4_aura_add_transient_effect(p as *mut fc4_aura, 8.0, 8.0, 64.0, 30.0);
    }
}
unsafe extern "C" fn attempt_disaster_start(u: *mut c_void, manager: u32, x: i32, z: i32) {
    game(u).disasters.borrow_mut().push((manager, x, z));
}
unsafe extern "C" fn end_strike(u: *mut c_void, k: u32) {
    game(u).strikes.borrow_mut().push(k);
}
unsafe extern "C" fn post(u: *mut c_void, t: u32) {
    let g = game(u);
    g.posts.borrow_mut().push(t);
    g.probe_now();
}
unsafe extern "C" fn subscribe(u: *mut c_void, ids: *const u32, n: u32) {
    game(u)
        .subs
        .borrow_mut()
        .extend_from_slice(std::slice::from_raw_parts(ids, n as usize));
}
unsafe extern "C" fn unsubscribe(u: *mut c_void, ids: *const u32, n: u32) {
    game(u)
        .unsubs
        .borrow_mut()
        .extend_from_slice(std::slice::from_raw_parts(ids, n as usize));
}
unsafe extern "C" fn add_agent(u: *mut c_void, level: u32, name: *const c_char, flags: u32) {
    let name = CStr::from_ptr(name).to_string_lossy().into_owned();
    game(u).agents.borrow_mut().push((level, name, flags));
}
unsafe extern "C" fn remove_agent(u: *mut c_void) {
    game(u).agents.borrow_mut().clear();
}

pub(crate) fn grid_api(g: &Game) -> fc4_grid_api {
    fc4_grid_api {
        struct_size: std::mem::size_of::<fc4_grid_api>() as u32,
        user: g.user(),
        create: Some(g_create),
        class_of: Some(g_class),
        geometry: Some(g_geometry),
        get: Some(g_get),
        set: Some(g_set),
        fill: Some(g_fill),
        shutdown: Some(g_shutdown),
        release: Some(g_release),
    }
}

pub(crate) fn host(g: &Game) -> fc4_aura_host {
    fc4_aura_host {
        struct_size: std::mem::size_of::<fc4_aura_host>() as u32,
        user: g.user(),
        grids: grid_api(g),
        cell_width: Some(cell_width),
        position_to_cell: Some(position_to_cell),
        sim_date_number: Some(sim_date_number),
        tuning: Some(tuning),
        publish_base_aura: Some(publish_base_aura),
        lot_count: Some(lot_count),
        lot_iterator: Some(lot_iterator),
        lot_next: Some(lot_next),
        lot_iterator_release: Some(lot_iterator_release),
        lot_at: Some(lot_at),
        land_value_type: Some(land_value_type),
        occupant_has_property: Some(occupant_has_property),
        occupant_property: Some(occupant_property),
        occupant_type: Some(occupant_type),
        occupant_position: Some(occupant_position),
        budget_item_department: Some(budget_item_department),
        department_funding: Some(department_funding),
        tax_rate: Some(tax_rate),
        cumulative_offset: Some(cumulative_offset),
        input_grid: Some(input_grid),
        record_history: Some(record_history),
        attempt_disaster_start: Some(attempt_disaster_start),
        end_strike: Some(end_strike),
        post: Some(post),
        limit_timer_start: None,
        limit_timer_expired: None,
        subscribe: Some(subscribe),
        unsubscribe: Some(unsubscribe),
        add_agent: Some(add_agent),
        remove_agent: Some(remove_agent),
        registry_add: None,
        registry_remove: None,
    }
}

fn message(p: *mut fc4_aura, msg_type: u32, data3: u32, void1: u64) -> bool {
    unsafe { fc4_aura_do_message(p, msg_type, 0, 0, data3, void1) }
}

fn state_of(p: *const fc4_aura) -> fc4_aura_state {
    let mut s = fc4_aura_state::default();
    assert!(unsafe { fc4_aura_get_state(p, &mut s) });
    s
}

fn ratings_of(p: *const fc4_aura) -> fc4_aura_ratings {
    let mut r = fc4_aura_ratings::default();
    assert!(unsafe { fc4_aura_get_ratings(p, &mut r) });
    r
}

fn started(g: &Game) -> *mut fc4_aura {
    let h = host(g);
    let p = unsafe { fc4_aura_create(&h, 0xA0, 1) };
    assert!(!p.is_null());
    assert!(unsafe { fc4_aura_init(p) });
    p
}

/// Idle calls until the cycle ends (at most 100).
fn run_cycle(p: *mut fc4_aura) -> u32 {
    let mut calls = 0;
    while state_of(p).state != 0 && calls < 100 {
        message(p, msg::SIM_IDLE, 5, 0);
        calls += 1;
    }
    calls
}

fn stop(p: *mut fc4_aura) {
    assert!(unsafe { fc4_aura_shutdown(p) });
    unsafe { fc4_aura_destroy(p) };
}

#[test]
fn create_rejects_short_tables_and_null_handles_answer_fallbacks() {
    let g = Game::new();
    let mut h = host(&g);
    unsafe {
        assert!(fc4_aura_create(std::ptr::null(), 1, 1).is_null());
        h.struct_size -= 4;
        assert!(fc4_aura_create(&h, 1, 1).is_null());
        h.struct_size += 4;
        h.grids.struct_size = 8;
        assert!(fc4_aura_create(&h, 1, 1).is_null());

        let p = std::ptr::null_mut();
        assert!(!fc4_aura_init(p));
        assert!(!fc4_aura_shutdown(p));
        assert!(fc4_aura_do_message(p, msg::SIM_NEW_DAY, 0, 0, 0, 0));
        fc4_aura_add_transient_effect(p, 0.0, 0.0, 1.0, 1.0);
        fc4_aura_add_transient_effect_type(p, 0.0, 0.0, 0);
        fc4_aura_bust_strike(p, 0);
        let mut r = fc4_aura_ratings::default();
        assert!(!fc4_aura_get_ratings(p, &mut r));
        assert_eq!(fc4_aura_grid(p, 0), 0);
        let mut s = fc4_aura_state::default();
        assert!(!fc4_aura_get_state(p, &mut s));
        fc4_aura_destroy(p);
    }
}

#[test]
fn init_registers_and_creates_the_grids() {
    let g = Game::new();
    let p = started(&g);
    let agents: Vec<u32> = g.agents.borrow().iter().map(|a| a.0).collect();
    assert_eq!(agents, AGENT_LEVELS.to_vec());
    assert!(g.agents.borrow().iter().all(|a| a.1 == AGENT_NAME && a.2 == 2));
    assert_eq!(g.subs.borrow().len(), 5);
    // §2: aura, base, transient, work, building effect, park map, raw park,
    // landmark map, landmark work.
    let created = g.grids.borrow().values().filter(|gr| gr.instance != 0).count();
    assert_eq!(created, 9);
    let work = g.grid_by_instance(grid_id::WORK, 1);
    assert_eq!(g.grids.borrow()[&work].clsid, grid_id::CLSID_INT8);
    let base = g.grid_by_instance(grid_id::BASE, 4);
    assert_eq!(g.value(base, 0, 0), -128.0);
    let st = state_of(p);
    assert_eq!((st.initialized, st.state), (1, 1));
    assert_eq!(st.long_decay, 0.9);
    // The getters' snapshot names the four grids.
    let r = ratings_of(p);
    assert_eq!(r.aura_grid, g.grid_by_instance(grid_id::AURA, 4));
    assert_eq!(r.park_map, g.grid_by_instance(grid_id::PARK_MAP, 4));
    assert_eq!(unsafe { fc4_aura_grid(p, FC4_AURA_GRID_TRANSIENT) }, r.transient_grid);
    assert_eq!(unsafe { fc4_aura_grid(p, FC4_AURA_GRID_LANDMARK) }, r.landmark_map);
    assert_eq!(unsafe { fc4_aura_grid(p, 9) }, 0);
    stop(p);
    // Shutdown: every grid shut down and released, the agent and the
    // subscriptions removed.
    assert!(g
        .grids
        .borrow()
        .values()
        .filter(|gr| gr.instance != 0)
        .all(|gr| gr.shut && gr.refs == 0));
    assert!(g.agents.borrow().is_empty());
    assert_eq!(g.unsubs.borrow().len(), 5);
}

/// A cycle on the fake city: the lot's base aura (shipped tuning: base 0,
/// value curve at land value 3 − wealth 1 = −5), crime 0 (+10), the base,
/// aura and mayor rating, the history record and the end-of-cycle post.
#[test]
fn a_cycle_rates_the_lot() {
    let g = Game::new();
    let p = started(&g);
    let calls = run_cycle(p);
    assert!(calls >= 17, "one step per idle call: {calls}");
    let base = g.grid_by_instance(grid_id::BASE, 4);
    let aura = g.grid_by_instance(grid_id::AURA, 4);
    assert_eq!(g.value(base, 0, 0), 5.0);
    assert_eq!(g.value(base, 1, 0), -128.0);
    assert_eq!(g.value(aura, 0, 0), 5.0);
    assert_eq!(g.value(aura, 1, 1), 0.0);
    assert_eq!(*g.history.borrow(), vec![(ids::HISTORY_MAYOR_RATING, 5)]);
    assert_eq!(*g.posts.borrow(), vec![msg::AURA_CYCLE_COMPLETED]);
    assert_eq!(g.iterators.get(), 0, "the lot iterator is released");
    let r = ratings_of(p);
    assert_eq!((r.mayor_rating, r.max_rating, r.min_rating), (5, 5, 5));
    assert_eq!((r.max_x, r.max_z, r.min_x, r.min_z), (2, 2, 2, 2));
    let st = state_of(p);
    assert_eq!((st.aura_tracts, st.aura_sum, st.aura_min, st.aura_max), (64, 5, 0, 5));
    assert_eq!(st.base_tracts, 1);
    assert_eq!(st.effective_base_aura, [0, 0, 0]);
    // Only the inputs the cycle reads: pollution 0..3, school, hospital,
    // crime, trip, congestion, then the population grids.
    let asked = g.inputs_asked.borrow().clone();
    assert_eq!(asked, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    // The next month starts a new cycle; SimNewDay with the state after its step.
    assert!(message(p, msg::SIM_NEW_MONTH, 0, 0));
    assert_eq!(state_of(p).state, 1);
    assert!(message(p, msg::SIM_NEW_DAY, 0, 0));
    stop(p);
}

#[test]
fn a_city_without_lots_ends_the_cycle_silently() {
    let g = Game::new();
    g.no_lots.set(true);
    let p = started(&g);
    run_cycle(p);
    assert!(g.history.borrow().is_empty());
    assert!(g.posts.borrow().is_empty());
    // State 4 took an iterator before it asked the count; it is kept until
    // the next prepare or Shutdown.
    assert_eq!(g.iterators.get(), 1);
    stop(p);
    assert_eq!(g.iterators.get(), 0);
}

/// §7 and §6.2: a park's cone in the raw park grid, scaled by the Parks
/// funding into the park map at the next cycle.
#[test]
fn occupants_stamp_parks_and_the_cycle_scales_them() {
    let g = Game::new();
    let p = started(&g);
    assert!(message(p, msg::OCCUPANT_INSERTED, 0, PARK));
    let raw = g.grid_by_instance(grid_id::RAW_PARK, 4);
    let centre = g.value(raw, 0, 0);
    assert!(centre > 0.0, "the cone's centre");
    run_cycle(p);
    let park = g.grid_by_instance(grid_id::PARK_MAP, 4);
    assert_eq!(g.value(park, 0, 0), (centre * 0.5).trunc());
    // Removing it takes the cone back out.
    assert!(message(p, msg::OCCUPANT_REMOVED, 0, PARK));
    assert_eq!(g.value(raw, 0, 0), 0.0);
    // A preview occupant is ignored.
    g.set(PARK, ids::PROP_PREVIEW_OCCUPANT, mem::UINT32, false, &[1u32]);
    assert!(message(p, msg::OCCUPANT_INSERTED, 0, PARK));
    assert_eq!(g.value(raw, 0, 0), 0.0);
    stop(p);
}

#[test]
fn strikes_and_transient_effects() {
    let g = Game::new();
    let p = started(&g);
    unsafe {
        fc4_aura_bust_strike(p, 0);
        fc4_aura_bust_strike(p, 3);
        fc4_aura_bust_strike(p, 9);
    }
    assert_eq!(*g.strikes.borrow(), vec![0, 1, 2, 3, 2]);
    let transient = g.grid_by_instance(grid_id::TRANSIENT, 4);
    // Type 2 (civic plop, shipped {25, 128 m}) at the city's first tract.
    unsafe { fc4_aura_add_transient_effect_type(p, 8.0, 8.0, 2) };
    assert_eq!(g.value(transient, 0, 0), 25.0);
    // Type 3 is {0, 0}: nothing; type 4 is out of range: nothing.
    unsafe {
        fc4_aura_add_transient_effect_type(p, 200.0, 200.0, 3);
        fc4_aura_add_transient_effect_type(p, 200.0, 200.0, 4);
    }
    let sum: f64 = g.grids.borrow()[&transient].data.iter().sum();
    unsafe { fc4_aura_add_transient_effect(p, 400.0, 400.0, 64.0, -10.0) };
    let after: f64 = g.grids.borrow()[&transient].data.iter().sum();
    assert!(after < sum);
    stop(p);
}

/// Requirement: the game reads the getters from a second thread during
/// OnTick. The history record and the post of a cycle read the handle from
/// another thread and wait.
#[test]
fn getters_answer_from_another_thread_during_a_main_thread_call() {
    let g = Game::new();
    let p = started(&g);
    g.probe.set(p as usize);
    run_cycle(p);
    g.probe.set(0);
    assert_eq!(g.probes.get(), 2);
    stop(p);
}

/// A transient effect delivered from inside a call (a host callback) is
/// queued and applied when the call returns.
#[test]
fn calls_during_a_call_are_queued() {
    let g = Game::new();
    let p = started(&g);
    g.reenter.set(p as usize);
    run_cycle(p);
    assert_eq!(g.reenter.get(), 0, "the cycle recorded history");
    let st = state_of(p);
    assert_eq!(st.reentrant_calls, 1);
    let transient = g.grid_by_instance(grid_id::TRANSIENT, 4);
    // Applied after the history call (state 15), then decayed at state 17:
    // trunc(30 × 0.75).
    assert_eq!(g.value(transient, 0, 0), 22.0);
    // The ratings were published after the queued call.
    assert_eq!(ratings_of(p).mayor_rating, 5);
    stop(p);
}

#[test]
fn sint32s_reads_integer_arrays_of_two_or_more() {
    // §7 "How the values are read", [choice] AU4: a Sint32 or Uint32 array
    // (bitwise) of at least two values; anything else is absent (no cone).
    use fc4_core::variant::{Elem, Variant};
    let a = |elem: u16, items: Vec<Elem>| Variant::array(elem, items);
    let pair = a(mem::SINT32, vec![Elem::I32(50), Elem::I32(22)]);
    assert_eq!(sint32s(&pair), Some(vec![50, 22]));
    let bits = vec![Elem::U32(0xFFFF_FF9C), Elem::U32(32), Elem::U32(7)];
    assert_eq!(sint32s(&a(mem::UINT32, bits)), Some(vec![-100, 32, 7]));
    assert_eq!(sint32s(&a(mem::SINT32, vec![Elem::I32(50)])), None);
    assert_eq!(sint32s(&a(mem::SINT32, vec![])), None);
    assert_eq!(sint32s(&Variant::Scalar(Elem::I32(50))), None);
    assert_eq!(sint32s(&a(mem::FLOAT32, vec![Elem::F32(50.0), Elem::F32(22.0)])), None);
    assert_eq!(sint32s(&a(mem::SINT16, vec![Elem::I16(50), Elem::I16(22)])), None);
}

#[test]
fn record_round_trip_through_host_streams() {
    let g = Game::new();
    let p = started(&g);
    run_cycle(p);
    let before = state_of(p);
    let mut out = Out::default();
    let os = ostream(&mut out);
    assert!(unsafe { fc4_aura_write(p, &os) });
    let aura = g.grid_by_instance(grid_id::AURA, 4);
    assert!(out.refs.contains(&(aura, grid_id::CLSID_INT8)));
    assert_eq!(out.refs.len(), 7, "seven grids, no department grid");

    // Read runs before Init: the reader takes the references the stream hands over.
    let h = host(&g);
    let q = unsafe { fc4_aura_create(&h, 0xA1, 1) };
    let gp: *const Game = &*g;
    let take = move |r: &mut crate::stream::fc4_ref| unsafe {
        if let Some(gr) = (*gp).grids.borrow_mut().get_mut(&r.token) {
            gr.refs += 1;
        }
    };
    let mut inp = In::new(&out.bytes);
    inp.resolve = Some(&take);
    let is = istream(&mut inp);
    assert!(unsafe { fc4_aura_read(q, &is) });
    let after = state_of(q);
    assert_eq!(after.mayor_rating, before.mayor_rating);
    assert_eq!(
        (after.max_rating, after.max_x, after.max_z),
        (before.max_rating, before.max_x, before.max_z)
    );
    assert_eq!(after.initialized, 0);
    assert_eq!(ratings_of(q).aura_grid, aura, "the loaded grids are published");
    unsafe { fc4_aura_destroy(q) };
    assert_eq!(
        g.grids.borrow()[&aura].refs,
        1,
        "the reader's references are given back"
    );

    // A version above 9 fails.
    let bad = 10u16.to_le_bytes();
    let q = unsafe { fc4_aura_create(&h, 0xA2, 1) };
    let mut inp = In::new(&bad);
    let is = istream(&mut inp);
    assert!(!unsafe { fc4_aura_read(q, &is) });
    unsafe { fc4_aura_destroy(q) };
    stop(p);
}

// ---- the shadow ---------------------------------------------------------------------------------------

use crate::aura_shadow::*;
use crate::stream::{fc4_ostream, fc4_ref};

/// What the plugin's capture stream produces: the original's `Write`, with
/// references as `u32 index + 1, u32 class` and each one queried for the
/// grid interface.
struct Capture {
    game: *const Game,
    bytes: Vec<u8>,
    refs: Vec<fc4_aura_capture_ref>,
}

impl Capture {
    fn view(&self) -> fc4_aura_capture {
        fc4_aura_capture {
            struct_size: std::mem::size_of::<fc4_aura_capture>() as u32,
            ok: 1,
            bytes: self.bytes.as_ptr(),
            len: self.bytes.len(),
            refs: self.refs.as_ptr(),
            ref_count: self.refs.len(),
        }
    }
}

fn cap(u: *mut c_void) -> &'static mut Capture {
    unsafe { &mut *(u as *mut Capture) }
}
macro_rules! cput {
    ($name:ident, $t:ty) => {
        unsafe extern "C" fn $name(u: *mut c_void, v: $t) -> bool {
            cap(u).bytes.extend_from_slice(&v.to_le_bytes());
            true
        }
    };
}
cput!(c_u8, u8);
cput!(c_u16, u16);
cput!(c_u32, u32);
cput!(c_u64, u64);
cput!(c_f32, f32);
cput!(c_f64, f64);
unsafe extern "C" fn c_bytes(u: *mut c_void, p: *const u8, n: usize) -> bool {
    if n > 0 {
        cap(u).bytes.extend_from_slice(std::slice::from_raw_parts(p, n));
    }
    true
}
unsafe extern "C" fn c_str(u: *mut c_void, p: *const u8, n: usize) -> bool {
    c_u32(u, n as u32);
    c_bytes(u, p, n)
}
unsafe extern "C" fn c_ref(u: *mut c_void, r: *const fc4_ref) -> bool {
    let c = cap(u);
    let r = &*r;
    if r.token == 0 {
        return c_u32(u, 0);
    }
    let is_grid = (*c.game).grids.borrow().contains_key(&r.token);
    c.refs.push(fc4_aura_capture_ref {
        grid: if is_grid { r.token } else { 0 },
        class_id: r.class_id,
        reserved: 0,
    });
    let id = c.refs.len() as u32;
    c_u32(u, id);
    c_u32(u, r.class_id)
}
unsafe extern "C" fn c_no_error(_: *mut c_void) -> i32 {
    0
}

fn capture(p: *const fc4_aura, g: &Game) -> Box<Capture> {
    let mut c = Box::new(Capture {
        game: g,
        bytes: Vec::new(),
        refs: Vec::new(),
    });
    let os = fc4_ostream {
        struct_size: std::mem::size_of::<fc4_ostream>() as u32,
        user: &mut *c as *mut Capture as *mut c_void,
        put_u8: Some(c_u8),
        put_u16: Some(c_u16),
        put_u32: Some(c_u32),
        put_u64: Some(c_u64),
        put_f32: Some(c_f32),
        put_f64: Some(c_f64),
        put_bytes: Some(c_bytes),
        put_str: Some(c_str),
        put_ref: Some(c_ref),
        error: Some(c_no_error),
    };
    assert!(unsafe { fc4_aura_write(p, &os) });
    c
}

unsafe extern "C" fn lines_sink(ctx: *mut c_void, data: *const u8, len: usize) {
    let s = String::from_utf8(std::slice::from_raw_parts(data, len).to_vec()).unwrap();
    (*(ctx as *mut Vec<String>)).push(s);
}

/// The plugin's read-only table: reads only, grids by geometry and get.
fn read_only_host(g: &Game) -> fc4_aura_host {
    let mut h = host(g);
    h.grids.create = None;
    h.grids.set = None;
    h.grids.fill = None;
    h.grids.shutdown = None;
    h.grids.release = None;
    h.publish_base_aura = None;
    h.record_history = None;
    h.attempt_disaster_start = None;
    h.end_strike = None;
    h.post = None;
    h.subscribe = None;
    h.unsubscribe = None;
    h.add_agent = None;
    h.remove_agent = None;
    h
}

fn shadow(g: &Game) -> *mut fc4_aura_shadow {
    let h = read_only_host(g);
    let s = unsafe { fc4_aura_shadow_create(&h, CELLS, CELLS, 3) };
    assert!(!s.is_null());
    s
}

fn baseline(s: *mut fc4_aura_shadow, a: *const fc4_aura, g: &Game) -> (bool, Vec<String>) {
    let c = capture(a, g);
    let mut lines: Vec<String> = Vec::new();
    let r = unsafe { fc4_aura_shadow_baseline(s, &c.view(), Some(lines_sink), &mut lines as *mut _ as *mut c_void) };
    (r, lines)
}

fn shadow_cycle(s: *mut fc4_aura_shadow, a: *const fc4_aura, g: &Game, julian: i32) -> (i32, Vec<String>) {
    let c = capture(a, g);
    let mut lines: Vec<String> = Vec::new();
    let r = unsafe {
        fc4_aura_shadow_cycle(
            s,
            &c.view(),
            julian,
            Some(lines_sink),
            &mut lines as *mut _ as *mut c_void,
        )
    };
    (r, lines)
}

fn shadow_stats(s: *mut fc4_aura_shadow) -> fc4_aura_shadow_stats {
    let mut st = fc4_aura_shadow_stats::default();
    assert!(unsafe { fc4_aura_shadow_get_stats(s, &mut st) });
    st
}

/// Everything the game holds that a shadow could change.
fn fingerprint(g: &Game) -> String {
    let grids: Vec<(u64, Vec<f64>, i32, bool)> = g
        .grids
        .borrow()
        .iter()
        .map(|(t, gr)| (*t, gr.data.clone(), gr.refs, gr.shut))
        .collect();
    format!(
        "{grids:?} {:?} {:?} {:?} {:?} {:?} {:?} {}",
        g.agents.borrow(),
        g.subs.borrow(),
        g.posts.borrow(),
        g.history.borrow(),
        g.strikes.borrow(),
        g.disasters.borrow(),
        g.iterators.get()
    )
}

const FEB_2000: i32 = 2_451_576; // 2000-02-01

/// The plugin's part (FC4AuraShadow.cpp): one delivery to the original with
/// its state read before and after; on a move, the shadow's step with the
/// original's work and transient grids; at a return to 0, its record and
/// the cycle comparison.
struct Follower {
    s: *mut fc4_aura_shadow,
    a: *mut fc4_aura,
    julian: i32,
    /// fc4_aura_shadow_step's results.
    calls: Vec<i32>,
    /// The cycles that ended: (result, lines).
    cycles: Vec<(i32, Vec<String>)>,
    /// Transient grids copied (fc4_aura_shadow_before_step).
    copies: u32,
}

impl Follower {
    fn new(s: *mut fc4_aura_shadow, a: *mut fc4_aura) -> Follower {
        Follower {
            s,
            a,
            julian: FEB_2000,
            calls: Vec::new(),
            cycles: Vec::new(),
            copies: 0,
        }
    }

    fn deliver(&mut self, g: &Game, msg_type: u32) {
        let from = state_of(self.a).state;
        let transient = unsafe { fc4_aura_grid(self.a, FC4_AURA_GRID_TRANSIENT) };
        let copied = unsafe { fc4_aura_shadow_before_step(self.s, from, transient) };
        if copied {
            self.copies += 1;
        }
        assert!(message(self.a, msg_type, 5, 0));
        let to = state_of(self.a).state;
        if from == 3 && to == 3 {
            // §6.3: the original took one department (the plugin sees its
            // cursor move); ours takes one too.
            let step = fc4_aura_shadow_step_in {
                struct_size: std::mem::size_of::<fc4_aura_shadow_step_in>() as u32,
                from,
                to,
                julian: self.julian,
                work_grid: 0,
            };
            self.calls.push(unsafe { fc4_aura_shadow_step(self.s, &step) });
            return;
        }
        if from == to {
            return;
        }
        let step = fc4_aura_shadow_step_in {
            struct_size: std::mem::size_of::<fc4_aura_shadow_step_in>() as u32,
            from,
            to,
            julian: self.julian,
            work_grid: g.grid_by_instance(grid_id::WORK, grid_id::WORK_TRACT_SIZE),
        };
        self.calls.push(unsafe { fc4_aura_shadow_step(self.s, &step) });
        if to == 0 {
            self.cycles.push(shadow_cycle(self.s, self.a, g, self.julian));
        }
    }

    /// SimIdle until the original is in `state`.
    fn until(&mut self, g: &Game, state: i32) {
        for _ in 0..100 {
            if state_of(self.a).state == state {
                return;
            }
            self.deliver(g, msg::SIM_IDLE);
        }
        panic!("the original never reached state {state}");
    }

    /// SimIdle until a cycle ends: its result and line.
    fn cycle(&mut self, g: &Game) -> (i32, String) {
        let n = self.cycles.len();
        for _ in 0..100 {
            self.deliver(g, msg::SIM_IDLE);
            if self.cycles.len() > n {
                let (r, lines) = self.cycles.last().unwrap().clone();
                assert_eq!(lines.len(), 1, "{lines:?}");
                return (r, lines[0].clone());
            }
        }
        panic!("no cycle ended");
    }

    /// The next month (SimNewMonth: 0 → 1).
    fn month(&mut self, g: &Game) {
        self.julian += 29;
        self.deliver(g, msg::SIM_NEW_MONTH);
    }
}

const CLEAN: &str = "tracts=64; base mismatches=0 max|d|=0; aura mismatches=0 max|d|=0; park map mismatches=0 \
                     max|d|=0; landmark map mismatches=0 max|d|=0; transient mismatches=0 max|d|=0 (copied before \
                     states 15 and 17: 2 times); work after states 5-13: 0,0,0,0,0,0,0,0,0; mayor rating orig/ours=5/5; \
                     max orig/ours=5@(2,2)/5@(2,2); min orig/ours=5@(2,2)/5@(2,2); hotspots orig/ours=0/0 (same)";

#[test]
fn shadow_create_checks_its_arguments() {
    let g = Game::new();
    let h = read_only_host(&g);
    unsafe {
        assert!(fc4_aura_shadow_create(std::ptr::null(), CELLS, CELLS, 0).is_null());
        assert!(fc4_aura_shadow_create(&h, 0, CELLS, 0).is_null());
        assert!(fc4_aura_shadow_create(&h, CELLS, 5000, 0).is_null());
        let mut short = h;
        short.struct_size = 8;
        assert!(fc4_aura_shadow_create(&short, CELLS, CELLS, 0).is_null());
        // NULL handles answer the fallbacks.
        let n = std::ptr::null_mut();
        assert_eq!(
            fc4_aura_shadow_cycle(n, std::ptr::null(), 0, None, std::ptr::null_mut()),
            -1
        );
        assert!(!fc4_aura_shadow_baseline(
            n,
            std::ptr::null(),
            None,
            std::ptr::null_mut()
        ));
        let step = fc4_aura_shadow_step_in {
            struct_size: std::mem::size_of::<fc4_aura_shadow_step_in>() as u32,
            from: 1,
            to: 2,
            ..Default::default()
        };
        assert_eq!(fc4_aura_shadow_step(n, &step), -1);
        assert!(!fc4_aura_shadow_occupant(n, PARK, true));
        assert!(!fc4_aura_shadow_before_step(n, 15, 1));
        let mut st = fc4_aura_shadow_stats::default();
        assert!(!fc4_aura_shadow_get_stats(n, &mut st));
        fc4_aura_shadow_destroy(n);

        // A live shadow: no step without a simulator of ours, a NULL or
        // short step block, a state outside 0..=17 or no move.
        let s = shadow(&g);
        assert_eq!(fc4_aura_shadow_step(s, &step), -1);
        assert!(!fc4_aura_shadow_occupant(s, PARK, true));
        let a = started(&g);
        assert!(baseline(s, a, &g).0);
        assert_eq!(fc4_aura_shadow_step(s, std::ptr::null()), -1);
        let mut bad = step;
        bad.struct_size = 16;
        assert_eq!(fc4_aura_shadow_step(s, &bad), -1);
        for (from, to) in [(1, 18), (-1, 2), (4, 4)] {
            bad = step;
            bad.from = from;
            bad.to = to;
            assert_eq!(fc4_aura_shadow_step(s, &bad), -1);
        }
        // 3 -> 3 is the department step (§6.3): with ours not in state 3 it
        // runs nothing (0, a desync).
        bad = step;
        bad.from = 3;
        bad.to = 3;
        assert_eq!(fc4_aura_shadow_step(s, &bad), 0);
        fc4_aura_shadow_destroy(s);
        stop(a);
    }
}

/// Our own simulator as the "original": followed step by step from the
/// city load, every cycle matches, and the shadow writes nothing to the
/// game (a twin game without a shadow ends the same).
#[test]
fn shadow_follows_an_identical_original_step_by_step() {
    let twin = Game::new();
    let b = started(&twin);
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    let (ok, lines) = baseline(s, a, &g);
    assert!(ok, "{lines:?}");
    assert!(
        lines[0].contains(
            "aura shadow: city load: ours starts from the original's record (Read, then Init: state 1): base 0 of \
             64 tracts with data, transient 0 nonzero, 0 departments, 0 hotspots"
        ),
        "{lines:?}"
    );
    let mut f = Follower::new(s, a);
    for month in 0..3u32 {
        if month > 0 {
            f.month(&g);
            assert!(message(b, msg::SIM_NEW_MONTH, 0, 0));
        }
        let (r, l) = f.cycle(&g);
        run_cycle(b);
        assert_eq!(r, 0, "{l}");
        // The original starts in state 1 after the load; later cycles begin
        // with the month's 0 → 1.
        let moves = if month == 0 { 17 } else { 18 };
        assert!(
            l.contains(&format!(
                "cycle {} (followed states 1-17 over 0 days: {moves} moves, 17 calls, 0 occupant messages): {CLEAN}",
                month + 1
            )),
            "{l}"
        );
        assert!(
            l.contains("; tax baseline: the first cycle after the load (as the original's); riot roll not compared"),
            "{l}"
        );
        assert!(
            l.contains("inputs not available (neutral): air pollution, water pollution"),
            "{l}"
        );
    }
    assert!(f.cycles[0].1[0].starts_with("aura shadow 2000-02-01 cycle 1 "));
    assert_eq!(f.calls.iter().filter(|&&c| c == 1).count(), 3 * 17);
    assert_eq!(f.calls.iter().filter(|&&c| c == 0).count(), 2, "the months' 0 → 1");
    assert_eq!(f.copies, 3 * 2, "before states 15 and 17");
    let st = shadow_stats(s);
    assert_eq!(
        (st.cycles, st.compared, st.clean, st.incomplete, st.desynced),
        (3, 3, 3, 0, 0)
    );
    assert_eq!(st.tracts, 3 * 64);
    assert_eq!(st.last_mismatches, 0);
    unsafe { fc4_aura_shadow_destroy(s) };
    assert_eq!(fingerprint(&g), fingerprint(&twin), "the shadow wrote to the game");
    stop(a);
    stop(b);
}

/// §5.3 / §14.3: each of our states reads its inputs right after the
/// original's state read them, so inputs that change later in the cycle do
/// not show (a shadow run at the end message would read the new values).
#[test]
fn shadow_reads_each_input_when_the_original_does() {
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    assert!(baseline(s, a, &g).0);
    let mut f = Follower::new(s, a);
    // A park, so that state 2 reads the Parks funding.
    assert!(message(a, msg::OCCUPANT_INSERTED, 0, PARK));
    assert!(unsafe { fc4_aura_shadow_occupant(s, PARK, true) });
    // Parks funding after state 2, the crime grid after state 10, the R$
    // population after state 15.
    f.until(&g, 3);
    g.funding.set(1.0);
    f.until(&g, 11);
    g.grids.borrow_mut().get_mut(&CRIME).unwrap().data.fill(250.0);
    f.until(&g, 16);
    g.grids.borrow_mut().get_mut(&POP[0]).unwrap().data[0] = 0.0;
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(l.contains("park map mismatches=0 max|d|=0;"), "{l}");
    assert!(l.contains("work after states 5-13: 0,0,0,0,0,0,0,0,0;"), "{l}");
    let park = unsafe { fc4_aura_grid(a, FC4_AURA_GRID_PARK) };
    assert_eq!(g.value(park, 0, 0), 10.0, "20 at the 50 % read in state 2");
    assert!(ratings_of(a).mayor_rating > 0, "the population read in state 15");
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}

/// §5.3 / §6.3: occupant messages reach ours when the original receives
/// them; the park map follows at the next state 2 on both sides.
#[test]
fn shadow_follows_occupant_messages() {
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    assert!(baseline(s, a, &g).0);
    let mut f = Follower::new(s, a);
    // Inserted before state 2 (the original is in state 1), to both.
    assert!(message(a, msg::OCCUPANT_INSERTED, 0, PARK));
    assert!(unsafe { fc4_aura_shadow_occupant(s, PARK, true) });
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(l.contains("17 moves, 17 calls, 1 occupant messages)"), "{l}");
    let park = |p: *const fc4_aura| {
        let t = unsafe { fc4_aura_grid(p, FC4_AURA_GRID_PARK) };
        g.value(t, 0, 0)
    };
    assert_eq!(park(a), 10.0, "20 at 50 % funding");
    // Removed after state 2: both park maps keep the cone this cycle.
    f.month(&g);
    f.until(&g, 3);
    assert!(message(a, msg::OCCUPANT_REMOVED, 0, PARK));
    assert!(unsafe { fc4_aura_shadow_occupant(s, PARK, false) });
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert_eq!(park(a), 10.0, "the map lags a cycle");
    // Inserted again but NOT given to ours: the park map and the work grid
    // from state 6 on differ.
    f.month(&g);
    assert!(message(a, msg::OCCUPANT_INSERTED, 0, PARK));
    let (r, l) = f.cycle(&g);
    assert!(r > 0, "{l}");
    assert!(l.contains("park map mismatches="), "{l}");
    assert!(!l.contains("park map mismatches=0 "), "{l}");
    assert!(l.contains("work after states 5-13: 0,"), "{l}");
    assert!(l.contains("park map (0,0) 10/0"), "{l}");
    let st = shadow_stats(s);
    assert_eq!((st.compared, st.clean), (3, 2));
    // A message between a cycle end and the next SimNewMonth counts for the
    // next cycle.
    assert!(message(a, msg::OCCUPANT_REMOVED, 0, PARK));
    assert!(unsafe { fc4_aura_shadow_occupant(s, PARK, false) });
    f.month(&g);
    let (_, l) = f.cycle(&g);
    assert!(l.contains("18 moves, 17 calls, 1 occupant messages)"), "{l}");
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}

/// §6.3 / §14.3: state 3 takes one department per call. A landmark inserted
/// between the original's department call and its copy call reaches the
/// landmark map one cycle later on both sides, because ours takes its
/// department call at the original's moment (verify-regress4: in game a
/// landfill tile's cone, 208 tracts / 188, differed when ours summed every
/// department at the 3 -> 4 move).
#[test]
fn shadow_follows_state_3_one_department_per_call() {
    const LANDMARK: u64 = 0x77;
    const LANDMARK2: u64 = 0x78;
    let g = Game::new();
    g.set(LANDMARK, ids::PROP_LANDMARK_EFFECT, mem::SINT32, true, &[-100i32, 8]);
    g.set(LANDMARK2, ids::PROP_LANDMARK_EFFECT, mem::SINT32, true, &[-100i32, 8]);
    let a = started(&g);
    let s = shadow(&g);
    assert!(baseline(s, a, &g).0);
    let mut f = Follower::new(s, a);
    // A department grid on both sides before the cycle.
    assert!(message(a, msg::OCCUPANT_INSERTED, 0, LANDMARK));
    assert!(unsafe { fc4_aura_shadow_occupant(s, LANDMARK, true) });
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    let landmark = |p: *const fc4_aura| {
        let t = unsafe { fc4_aura_grid(p, FC4_AURA_GRID_LANDMARK) };
        g.value(t, 0, 0)
    };
    let before = landmark(a);
    assert!(before < 0.0, "the first landmark's cone: {before}");
    // Next cycle: the original takes its one department, then the second
    // landmark arrives (to both), then the original copies and moves to 4.
    f.month(&g);
    f.until(&g, 3);
    let n = f.calls.len();
    f.deliver(&g, msg::SIM_IDLE);
    assert_eq!(state_of(a).state, 3, "one department: still in state 3");
    assert_eq!(f.calls[n..], [1], "ours took its department call");
    assert!(message(a, msg::OCCUPANT_INSERTED, 0, LANDMARK2));
    assert!(unsafe { fc4_aura_shadow_occupant(s, LANDMARK2, true) });
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(l.contains("landmark map mismatches=0 max|d|=0;"), "{l}");
    assert_eq!(landmark(a), before, "the second landmark is a cycle late");
    // The cycle after: both maps take it.
    f.month(&g);
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(landmark(a) < before, "both landmarks now");
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}

/// The transient grid takes effects the shadow cannot see: it is copied
/// before our states 15 and 17, so the aura matches.
#[test]
fn shadow_copies_the_original_transient_grid() {
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    assert!(baseline(s, a, &g).0);
    let mut f = Follower::new(s, a);
    f.until(&g, 10);
    unsafe { fc4_aura_add_transient_effect(a, 8.0, 8.0, 8.0, 20.0) };
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(
        l.contains("transient mismatches=0 max|d|=0 (copied before states 15 and 17: 2 times)"),
        "{l}"
    );
    assert!(l.contains("mayor rating orig/ours=25/25"), "{l}");
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}

/// Differences: a work cell after state 6 (the per-step diagnostic) and a
/// base tract after state 14.
#[test]
fn shadow_reports_differences() {
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    assert!(baseline(s, a, &g).0);
    let mut f = Follower::new(s, a);
    let work = g.grid_by_instance(grid_id::WORK, grid_id::WORK_TRACT_SIZE);
    f.until(&g, 7);
    let w = g.value(work, 1, 1);
    g.grids.borrow_mut().get_mut(&work).unwrap().data[CELLS as usize + 1] = w + 3.0;
    f.until(&g, 15);
    let base = g.grid_by_instance(grid_id::BASE, 4);
    g.grids.borrow_mut().get_mut(&base).unwrap().data[5 * 8 + 5] = 50.0;
    let (r, l) = f.cycle(&g);
    assert!(l.contains("work after states 5-13: 0,0,1,1,1,1,1,1,1;"), "{l}");
    assert!(
        l.contains("base mismatches=1 max|d|=178; aura mismatches=1 max|d|=50;"),
        "{l}"
    );
    assert!(
        l.contains("mayor rating orig/ours=5/5; max orig/ours=50@(22,22)/5@(2,2)"),
        "{l}"
    );
    assert!(
        l.contains(&format!(
            "first differing (x,z) orig/ours: base (5,5) 50/-128; aura (5,5) 50/0; work after state 7 (1,1) {}/{}",
            w + 3.0,
            w
        )),
        "{l}"
    );
    assert_eq!(r, 1 + 1 + 7 + 1, "base, aura, seven work grids, best");
    let st = shadow_stats(s);
    assert_eq!((st.compared, st.clean, st.rating_mismatches), (1, 0, 0));
    assert_eq!((st.base_mismatches, st.aura_mismatches, st.aura_transient), (1, 1, 0));
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}

/// A cycle joined in its middle is not compared (ours missed its first
/// states); a delivery the shadow did not see is a desync.
#[test]
fn shadow_joined_mid_cycle_or_out_of_step() {
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    assert!(baseline(s, a, &g).0);
    // Deliveries the shadow did not see: the original runs to state 7.
    while state_of(a).state != 7 {
        message(a, msg::SIM_IDLE, 5, 0);
    }
    let mut f = Follower::new(s, a);
    let (r, l) = f.cycle(&g);
    assert_eq!(r, -1);
    assert!(
        l.contains(
            "(followed states 7-17 over 0 days: 11 moves, 11 calls, 0 occupant messages): not compared (joined the \
             cycle in state 7); ours restarts from this record"
        ),
        "{l}"
    );
    // The next cycle is compared. Ours ran no state 4 in the first cycle,
    // so its tax baseline is taken in cycle 2.
    f.month(&g);
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(
        l.contains(
            "tax baseline: taken in cycle 2, after the original's (the first cycle after the load): tax-change \
             effects can differ"
        ),
        "{l}"
    );
    // One delivery not seen in the middle: ours is moved to the original's
    // state (a desync), and the cycle is compared but not clean.
    f.month(&g);
    f.until(&g, 8);
    message(a, msg::SIM_IDLE, 5, 0);
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(l.contains("; desync: 9->10 with ours in 8;"), "{l}");
    assert!(l.contains("17 moves, 16 calls"), "{l}");
    let st = shadow_stats(s);
    assert_eq!(
        (st.cycles, st.compared, st.clean, st.incomplete, st.desynced),
        (3, 2, 1, 1, 1)
    );
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}

#[test]
fn shadow_without_a_record_to_start_from_compares_nothing() {
    let g = Game::new();
    let a = started(&g);
    let s = shadow(&g);
    let mut f = Follower::new(s, a);
    // No baseline: no step runs; the cycle end starts ours from its record.
    let (r, l) = f.cycle(&g);
    assert_eq!(r, -1);
    assert!(f.calls.iter().all(|&c| c == -1));
    assert!(
        l.contains(
            "aura shadow 2000-02-01 cycle 1: not compared (no record to start from: no baseline at city load); \
             ours starts from this record"
        ),
        "{l}"
    );
    f.month(&g);
    let (r, l) = f.cycle(&g);
    assert_eq!(r, 0, "{l}");
    assert!(l.contains("tax baseline: taken in cycle 2"), "{l}");
    // No capture: INCOMPLETE, and ours is dropped until the next record.
    let mut lines: Vec<String> = Vec::new();
    let r = unsafe {
        fc4_aura_shadow_cycle(
            s,
            std::ptr::null(),
            FEB_2000,
            Some(lines_sink),
            &mut lines as *mut _ as *mut c_void,
        )
    };
    assert_eq!(r, -1);
    assert!(
        lines[0].contains("cycle 3: INCOMPLETE (no capture); ours restarts from the next record"),
        "{lines:?}"
    );
    f.month(&g);
    let (r, l) = f.cycle(&g);
    assert_eq!(r, -1);
    assert!(
        l.contains("cycle 4: not compared (no record to start from: the last record could not be read)"),
        "{l}"
    );
    // A record of the version alone does not parse.
    let bytes = 9u16.to_le_bytes();
    let c = fc4_aura_capture {
        struct_size: std::mem::size_of::<fc4_aura_capture>() as u32,
        ok: 1,
        bytes: bytes.as_ptr(),
        len: 2,
        refs: std::ptr::null(),
        ref_count: 0,
    };
    let mut lines: Vec<String> = Vec::new();
    let r = unsafe { fc4_aura_shadow_cycle(s, &c, FEB_2000, Some(lines_sink), &mut lines as *mut _ as *mut c_void) };
    assert_eq!(r, -1);
    assert!(
        lines[0].contains("INCOMPLETE (the original's record did not parse)"),
        "{lines:?}"
    );
    let st = shadow_stats(s);
    assert_eq!(
        (st.cycles, st.compared, st.incomplete, st.last_mismatches),
        (5, 1, 4, -1)
    );
    unsafe { fc4_aura_shadow_destroy(s) };
    stop(a);
}
