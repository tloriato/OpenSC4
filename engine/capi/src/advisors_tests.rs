// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The advisor system and the MySims through the C ABI
//! (docs/specs/advisors.md, docs/specs/mysims.md): a fake game behind
//! `fc4_advisor_host` and `fc4_mysim_host`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CStr};

use fc4_core::advisors::{clsid, SrcKind};
use fc4_core::mysims::{command, event, msg as ms_msg, NO_DISTANCE};

use crate::advisors::*;
use crate::stream::{fc4_bytes_sink, fc4_ref};
use crate::test_streams::{istream, ostream, In, Out};

const HOME: u64 = 0x900;
const OTHER: u64 = 0x901;
const BUILDING_TYPE: u32 = 0x2781_28A0;
const INSERTED: u32 = 0x99EF_1142;

struct Entry {
    class_id: u32,
    id: i32,
    caption: Vec<u8>,
    name: Vec<u8>,
}

/// A field: kind, number, string, object.
type FieldValue = (u32, f64, Vec<u8>, u64);
/// A building: position, groups, name.
type BuildingData = ([f32; 3], Vec<u32>, Vec<u8>);

#[derive(Default)]
struct Game {
    advisors: Vec<Entry>,
    day: Cell<i32>,
    created: RefCell<Vec<(u32, u32, usize)>>,
    next_token: Cell<u64>,
    refs: RefCell<BTreeMap<u64, i32>>,
    reference_events: RefCell<Vec<(bool, u64)>>,
    posts: RefCell<Vec<fc4_adv_out_message>>,
    subs: RefCell<Vec<(u32, i32, u32)>>,
    history: Cell<f64>,
    // MySims
    fields: RefCell<BTreeMap<(i32, String), FieldValue>>,
    activated: RefCell<Vec<i32>>,
    ms_posts: RefCell<Vec<fc4_mysim_post>>,
    ms_subs: RefCell<Vec<(u32, bool)>>,
    dispatch: RefCell<Vec<(i32, String)>>,
    buildings: RefCell<BTreeMap<u64, BuildingData>>,
    lots: RefCell<BTreeMap<u64, fc4_mysim_lot>>,
}

impl Game {
    fn new() -> Box<Game> {
        let mut g = Box::new(Game::default());
        let mut push = |class_id: u32, id: i32| {
            g.advisors.push(Entry {
                class_id,
                id,
                caption: b"Caption|tail".to_vec(),
                name: format!("Sim{id}").into_bytes(),
            })
        };
        push(clsid::ADVISOR, 23);
        for id in 1..=5 {
            push(clsid::MYSIM, id);
        }
        push(clsid::MY_STREET_SIM, 30);
        g.day.set(2_000_000);
        g.next_token.set(0x1_0000);
        g.history.set(42.0);
        g.buildings
            .borrow_mut()
            .insert(HOME, ([100.0, 0.0, 100.0], vec![0x1000], b"House".to_vec()));
        g.buildings
            .borrow_mut()
            .insert(OTHER, ([130.0, 0.0, 140.0], vec![0x1000], b"Shop".to_vec()));
        g.lots.borrow_mut().insert(
            HOME,
            fc4_mysim_lot {
                building: HOME,
                cell_x: 6,
                cell_z: 6,
                x0: 6,
                z0: 6,
                x1: 6,
                z1: 6,
                condition: 1,
                road_access: 1,
                wealth: 2,
                ..Default::default()
            },
        );
        g
    }
    fn user(&self) -> *mut c_void {
        self as *const Game as *mut c_void
    }
    fn refs(&self, t: u64) -> i32 {
        self.refs.borrow().get(&t).copied().unwrap_or(0)
    }
    fn num(&self, sim: i32, name: &str) -> Option<f64> {
        self.fields
            .borrow()
            .get(&(sim, name.to_string()))
            .filter(|f| f.0 == FC4_MYSIM_FIELD_NUMBER)
            .map(|f| f.1)
    }
}

fn g<'a>(u: *mut c_void) -> &'a Game {
    unsafe { &*(u as *const Game) }
}

// ---- advisor host callbacks ----

unsafe extern "C" fn advisor_count(u: *mut c_void, n: *mut u32) -> bool {
    *n = g(u).advisors.len() as u32;
    true
}
unsafe extern "C" fn advisor_entry(u: *mut c_void, i: u32, out: *mut fc4_adv_advisor_entry) -> bool {
    let Some(e) = g(u).advisors.get(i as usize - 1) else {
        return false;
    };
    let o = &mut *out;
    o.class_id = e.class_id;
    o.id = e.id;
    o.advice_type = 1;
    o.caption = e.caption.as_ptr();
    o.caption_len = e.caption.len();
    o.name = e.name.as_ptr();
    o.name_len = e.name.len();
    true
}
unsafe extern "C" fn has_city(_: *mut c_void) -> bool {
    true
}
unsafe extern "C" fn cell_counts(_: *mut c_void, x: *mut u32, z: *mut u32) {
    *x = 64;
    *z = 64;
}
unsafe extern "C" fn sim_date(u: *mut c_void, d: *mut fc4_adv_date) {
    (*d).day_number = g(u).day.get();
}
unsafe extern "C" fn subscribe(u: *mut c_void, kind: u32, id: i32, _token: u64, m: u32) {
    g(u).subs.borrow_mut().push((kind, id, m));
}
unsafe extern "C" fn post(u: *mut c_void, m: *const fc4_adv_out_message) {
    g(u).posts.borrow_mut().push(*m);
}
unsafe extern "C" fn history_value(u: *mut c_void, _t: u32, _day: i32, out: *mut f64) -> bool {
    *out = g(u).history.get();
    true
}
unsafe extern "C" fn ordinance_text(
    _: *mut c_void,
    id: u32,
    which: u32,
    sink: fc4_bytes_sink,
    ctx: *mut c_void,
) -> bool {
    if id != 7 {
        return false;
    }
    let t: &[u8] = if which == 0 { b"Tax" } else { b"A tax" };
    sink.unwrap()(ctx, t.as_ptr(), t.len());
    true
}
unsafe extern "C" fn add_ref(u: *mut c_void, t: u64) {
    g(u).reference_events.borrow_mut().push((true, t));
    *g(u).refs.borrow_mut().entry(t).or_insert(0) += 1;
}
unsafe extern "C" fn release(u: *mut c_void, t: u64) {
    g(u).reference_events.borrow_mut().push((false, t));
    *g(u).refs.borrow_mut().entry(t).or_insert(0) -= 1;
}
unsafe extern "C" fn resolve_ref(_: *mut c_void, t: u64, _iid: u32) -> u64 {
    t
}
unsafe extern "C" fn object_created(u: *mut c_void, kind: u32, class: u32, h: *mut fc4_adv_obj) -> u64 {
    let game = g(u);
    let t = game.next_token.get() + 1;
    game.next_token.set(t);
    game.created.borrow_mut().push((kind, class, h as usize));
    t
}

// ---- MySim host callbacks ----

unsafe extern "C" fn create_table(_: *mut c_void, _sim: i32) -> bool {
    true
}
unsafe extern "C" fn set_field(u: *mut c_void, sim: i32, name: *const c_char, f: *const fc4_mysim_field) {
    let f = &*f;
    let n = CStr::from_ptr(name).to_string_lossy().into_owned();
    let s = if f.str_.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(f.str_, f.len).to_vec()
    };
    g(u).fields
        .borrow_mut()
        .insert((sim, n), (f.kind, f.number, s, f.object));
}
unsafe extern "C" fn activate_lua(u: *mut c_void, sim: i32) -> bool {
    g(u).activated.borrow_mut().push(sim);
    true
}
unsafe extern "C" fn ms_cell_counts(_: *mut c_void, x: *mut u32, z: *mut u32) {
    *x = 64;
    *z = 64;
}
unsafe extern "C" fn building(u: *mut c_void, b: u64, out: *mut fc4_mysim_building) -> bool {
    let bs = g(u).buildings.borrow();
    let Some((pos, groups, name)) = bs.get(&b) else {
        return false;
    };
    let o = &mut *out;
    o.building_type = 0x10;
    o.purpose = 1;
    o.completion = 1.0;
    o.position = *pos;
    // The game keeps these alive for the call (the map is not touched).
    o.name = name.as_ptr();
    o.name_len = name.len();
    o.groups = groups.as_ptr();
    o.group_count = groups.len();
    true
}
unsafe extern "C" fn lot(u: *mut c_void, b: u64, out: *mut fc4_mysim_lot) -> bool {
    match g(u).lots.borrow().get(&b) {
        Some(l) => {
            *out = *l;
            true
        }
        None => false,
    }
}
unsafe extern "C" fn occupant_position(u: *mut c_void, b: u64, out: *mut f32) -> bool {
    match g(u).buildings.borrow().get(&b) {
        Some((p, ..)) => {
            std::ptr::copy_nonoverlapping(p.as_ptr(), out, 3);
            true
        }
        None => false,
    }
}
unsafe extern "C" fn all_buildings(u: *mut c_void, out: *mut u64, cap: usize) -> i64 {
    let v: Vec<u64> = g(u).buildings.borrow().keys().copied().collect();
    if !out.is_null() {
        for (i, b) in v.iter().take(cap).enumerate() {
            *out.add(i) = *b;
        }
    }
    v.len() as i64
}
unsafe extern "C" fn average_age(_: *mut c_void, x: i32, z: i32, out: *mut i32) -> bool {
    *out = if (x, z) == (6, 6) { 12 } else { 0 };
    true
}
unsafe extern "C" fn eq_by_wealth(_: *mut c_void, _x: i32, _z: i32, w: u8, out: *mut f32) -> bool {
    *out = 40.0 + f32::from(w);
    true
}
unsafe extern "C" fn dispatch_command(u: *mut c_void, sim: i32, what: *const c_char) {
    let w = CStr::from_ptr(what).to_string_lossy().into_owned();
    g(u).dispatch.borrow_mut().push((sim, w));
}
unsafe extern "C" fn ms_post(u: *mut c_void, m: *const fc4_mysim_post) {
    g(u).ms_posts.borrow_mut().push(*m);
}
unsafe extern "C" fn ms_subscribe(u: *mut c_void, m: u32, on: bool) {
    g(u).ms_subs.borrow_mut().push((m, on));
}

fn advisor_host(game: &Game) -> fc4_advisor_host {
    // SAFETY: an all-zero table is valid (no callbacks, NULL user).
    let mut h: fc4_advisor_host = unsafe { std::mem::zeroed() };
    h.struct_size = std::mem::size_of::<fc4_advisor_host>() as u32;
    h.user = game.user();
    h.advisor_count = Some(advisor_count);
    h.advisor_entry = Some(advisor_entry);
    h.has_city = Some(has_city);
    h.city_established = Some(has_city);
    h.cell_counts = Some(cell_counts);
    h.sim_date = Some(sim_date);
    h.subscribe = Some(subscribe);
    h.post = Some(post);
    h.history_value = Some(history_value);
    h.ordinance_text = Some(ordinance_text);
    h.add_ref = Some(add_ref);
    h.release = Some(release);
    h.resolve_ref = Some(resolve_ref);
    h.object_created = Some(object_created);
    h
}

fn mysim_host(game: &Game) -> fc4_mysim_host {
    // SAFETY: as above.
    let mut h: fc4_mysim_host = unsafe { std::mem::zeroed() };
    h.struct_size = std::mem::size_of::<fc4_mysim_host>() as u32;
    h.user = game.user();
    h.create_table = Some(create_table);
    h.set_field = Some(set_field);
    h.activate_lua = Some(activate_lua);
    h.cell_counts = Some(ms_cell_counts);
    h.building = Some(building);
    h.lot = Some(lot);
    h.occupant_position = Some(occupant_position);
    h.all_buildings = Some(all_buildings);
    h.average_age = Some(average_age);
    h.eq_by_wealth = Some(eq_by_wealth);
    h.dispatch_command = Some(dispatch_command);
    h.post = Some(ms_post);
    h.subscribe = Some(ms_subscribe);
    h.add_ref = Some(add_ref);
    h.release = Some(release);
    h.resolve_ref = Some(resolve_ref);
    h
}

struct Fixture {
    game: Box<Game>,
    p: *mut fc4_advisors,
}

impl Fixture {
    fn new() -> Fixture {
        let game = Game::new();
        let (h, m) = (advisor_host(&game), mysim_host(&game));
        let p = unsafe { fc4_advisors_create(&h, &m, 0x1234) };
        assert!(!p.is_null());
        assert!(unsafe { fc4_advisors_init(p) });
        Fixture { game, p }
    }
    fn advisor(&self, id: i32) -> fc4_advisor_state {
        let mut s = fc4_advisor_state::default();
        assert!(unsafe { fc4_advisors_advisor_state(self.p, id, &mut s) });
        s
    }
    fn sim(&self, id: i32) -> fc4_mysim_state {
        let mut s = fc4_mysim_state::default();
        assert!(unsafe { fc4_mysim_get_state(self.p, id, &mut s) });
        s
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe {
            fc4_advisors_destroy(self.p);
            for (_, _, h) in self.game.created.borrow_mut().drain(..) {
                fc4_adv_obj_destroy(h as *mut fc4_adv_obj);
            }
        }
    }
}

#[test]
fn create_checks_its_tables() {
    let game = Game::new();
    let mut h = advisor_host(&game);
    let m = mysim_host(&game);
    unsafe {
        assert!(fc4_advisors_create(std::ptr::null(), &m, 0).is_null());
        let mut short = m;
        short.struct_size -= 4;
        assert!(fc4_advisors_create(&h, &short, 0).is_null());
        h.struct_size -= 4;
        assert!(fc4_advisors_create(&h, &m, 0).is_null());
        // NULL handles: fallbacks.
        assert!(!fc4_advisors_init(std::ptr::null_mut()));
        assert_eq!(fc4_advisors_advisor_ids(std::ptr::null(), std::ptr::null_mut(), 0), 0);
        assert_eq!(fc4_adv_obj_kind(std::ptr::null()), 0);
        fc4_advisors_destroy(std::ptr::null_mut());
        fc4_adv_obj_destroy(std::ptr::null_mut());
    }
    assert!(fc4_advisors_query_interface(0x4A0D_D617));
    assert!(!fc4_advisors_query_interface(0x1234_5678));
}

#[test]
fn init_loads_the_advisors_and_wraps_them() {
    let f = Fixture::new();
    let mut st = fc4_advisors_state::default();
    assert!(unsafe { fc4_advisors_get_state(f.p, &mut st) });
    assert_eq!(st.struct_size as usize, std::mem::size_of::<fc4_advisors_state>());
    assert_eq!((st.advisor_count, st.initialized, st.has_mysims), (7, 1, 1));
    let mut ids = [0i32; 8];
    let n = unsafe { fc4_advisors_advisor_ids(f.p, ids.as_mut_ptr(), ids.len()) };
    assert_eq!(&ids[..n], &[1, 2, 3, 4, 5, 23, 30]);
    let a = f.advisor(1);
    assert_eq!((a.class_id, a.is_mysim, a.id), (clsid::MYSIM, 1, 1));
    assert_ne!(a.token, 0, "object_created gave a token");
    assert_eq!(f.advisor(23).is_mysim, 0);
    // Every advisor was announced once, with a handle on the same object.
    let created = f.game.created.borrow();
    let advisors: Vec<_> = created.iter().filter(|c| c.0 == FC4_ADV_OBJ_ADVISOR).collect();
    assert_eq!(advisors.len(), 7);
    let mine = unsafe { fc4_advisors_advisor_object(f.p, 1) };
    assert!(advisors
        .iter()
        .any(|c| unsafe { fc4_adv_obj_same(c.2 as *const fc4_adv_obj, mine) }));
    assert_eq!(unsafe { fc4_adv_obj_token(mine) }, a.token);
    assert_eq!(unsafe { fc4_adv_obj_class(mine) }, clsid::MYSIM);
    unsafe { fc4_adv_obj_destroy(mine) };
    drop(created);
    // Captions are cut at '|'; names come from the table.
    let mut buf = [0 as c_char; 32];
    let n = unsafe { fc4_advisors_advisor_text(f.p, 23, 0, buf.as_mut_ptr(), buf.len()) };
    assert_eq!(n, 7);
    assert_eq!(unsafe { CStr::from_ptr(buf.as_ptr()) }.to_bytes(), b"Caption");
    let n = unsafe { fc4_advisors_advisor_text(f.p, 23, 1, buf.as_mut_ptr(), 3) };
    assert_eq!(n, 5);
    assert_eq!(unsafe { CStr::from_ptr(buf.as_ptr()) }.to_bytes(), b"Si");
    // Unknown advisor: false.
    let mut s = fc4_advisor_state::default();
    assert!(!unsafe { fc4_advisors_advisor_state(f.p, 99, &mut s) });
}

#[test]
fn mysims_through_the_abi() {
    let f = Fixture::new();
    unsafe {
        assert!(fc4_mysims_agent_init(f.p));
        let mut a = fc4_mysims_agent_state::default();
        assert!(fc4_mysims_agent_get_state(f.p, &mut a));
        assert_eq!(a.slots, [1, 2, 3, 4, 5, 30]);
        assert_eq!((a.initialised, a.active_ui_index), (1, -1));
        assert_eq!(f.game.ms_subs.borrow().iter().filter(|s| s.1).count(), 11);
        // Move in: MySim 1, slot 3.
        let id = fc4_mysims_move_in(
            f.p,
            b"Ann".as_ptr(),
            3,
            false,
            4,
            b"bio".as_ptr(),
            3,
            9,
            std::ptr::null(),
            0,
            3,
        );
        assert_eq!(id, 1);
        assert!(fc4_mysim_set_residence(f.p, 1, HOME));
        let s = f.sim(1);
        assert_eq!((s.state, s.age, s.wealth, s.slot, s.generation), (1, 18, 2, 3, 1));
        assert_eq!((s.eq, s.residence), (42.0, HOME));
        assert_eq!(f.game.refs(HOME), 1);
        assert_eq!(f.game.num(1, "age"), Some(18.0));
        assert_eq!(f.game.num(1, "eq"), Some(42.0));
        let name = f.game.fields.borrow()[&(1, "name".to_string())].2.clone();
        assert_eq!(name, b"Ann".to_vec());
        let mut buf = [0 as c_char; 8];
        assert_eq!(fc4_mysim_text(f.p, 1, 0, buf.as_mut_ptr(), buf.len()), 3);
        assert_eq!(CStr::from_ptr(buf.as_ptr()).to_bytes(), b"bio");
        assert!(f
            .game
            .ms_posts
            .borrow()
            .iter()
            .any(|p| p.msg_type == ms_msg::MYSIMS_CHANGED && p.sim == 1));
        let mut active = [0i32; 6];
        assert_eq!(fc4_mysims_active(f.p, active.as_mut_ptr(), 6), 1);
        assert_eq!(fc4_mysims_lives_here(f.p, HOME), 1);
        assert!(!fc4_mysim_test_for_residence(f.p, 2, HOME, false, false));
        assert!(fc4_mysim_test_for_residence(f.p, 2, OTHER, false, false));
        // A building added: an advisor event on MySim 1 (50 m away).
        let m = fc4_adv_message {
            msg_type: INSERTED,
            data3: BUILDING_TYPE,
            void1: OTHER,
            ..Default::default()
        };
        assert!(fc4_mysims_agent_do_message(f.p, &m));
        assert_eq!(f.advisor(1).queue_length, 1);
        // The UI panel sim.
        assert!(fc4_mysims_set_active_ui_panel_sim(f.p, 3));
        let mut idx = -5;
        assert_eq!(fc4_mysims_get_active_ui_panel_sim(f.p, &mut idx), 1);
        assert_eq!(idx, 0);
        assert!(fc4_mysims_set_active_ui_panel_sim(f.p, 9));
        let mut idx = -5;
        assert_eq!(fc4_mysims_get_active_ui_panel_sim(f.p, &mut idx), 0);
        assert_eq!(idx, -5);
        // Distances and grades without data.
        assert_eq!(fc4_mysim_distance_to_closest(f.p, 1, 0x4444), NO_DISTANCE);
        assert_eq!(fc4_mysim_distance_to_closest_special(f.p, 1, 0x10), NO_DISTANCE);
        assert_eq!(fc4_mysim_local_grade(f.p, 1, 0x4444, false), 0.0);
        // CommandWouldTrigger activates the MySim's Lua table.
        f.game.activated.borrow_mut().clear();
        assert!(!fc4_advisors_advisor_command_would_trigger(f.p, 1, command::MOVE_OUT));
        assert_eq!(*f.game.activated.borrow(), vec![1]);
        // Leave the city.
        assert!(fc4_mysim_do_script_command(f.p, 1, command::LEAVE_CITY));
        assert_eq!(f.sim(1).state, 0);
        assert_eq!(f.game.refs(HOME), 0);
        assert!(f.game.dispatch.borrow().contains(&(1, "shutdown".to_string())));
        // Not a MySim: false / fallbacks.
        let mut s = fc4_mysim_state::default();
        assert!(!fc4_mysim_get_state(f.p, 23, &mut s));
        assert_eq!(fc4_mysim_distance_to_closest(f.p, 23, 1), NO_DISTANCE);
        // The agent record.
        let mut out = Out::default();
        assert!(fc4_mysims_agent_write(f.p, &ostream(&mut out)));
        assert_eq!(out.bytes, vec![1, 0]);
        let mut inp = In::new(&out.bytes);
        assert!(fc4_mysims_agent_read(f.p, &istream(&mut inp)));
        assert!(fc4_mysims_agent_shutdown(f.p));
        assert_eq!(f.game.ms_subs.borrow().iter().filter(|s| !s.1).count(), 11);
    }
}

#[test]
fn script_command_and_events_through_messages() {
    let f = Fixture::new();
    unsafe {
        assert!(fc4_mysims_agent_init(f.p));
        fc4_mysims_move_in(
            f.p,
            b"A".as_ptr(),
            1,
            true,
            0,
            std::ptr::null(),
            0,
            0,
            std::ptr::null(),
            0,
            0,
        );
        assert!(fc4_mysim_set_residence(f.p, 1, HOME));
        // AdviceCommand Die for advisor 1.
        let m = fc4_adv_message {
            msg_type: 0x8A09_FA6C,
            data1: command::DIE,
            unknown: 1,
            ..Default::default()
        };
        assert!(fc4_mysims_agent_do_message(f.p, &m));
        assert_eq!(f.sim(1).state, 0);
        // The MySim's own dispatch notification (job found without a
        // destination: nothing).
        let m = fc4_adv_message {
            msg_type: 0x2BA0_6B99,
            data1: 0xEBA0_6BF4,
            ..Default::default()
        };
        assert!(fc4_advisors_advisor_do_message(f.p, 2, &m));
        // A disaster subject pushed by the host.
        let s = fc4_advisors_new_subject(f.p);
        assert!(fc4_advisors_advisor_push_event(f.p, 2, event::RIOT, s));
        assert_eq!(f.advisor(2).queue_length, 1);
        fc4_adv_obj_destroy(s);
    }
}

#[test]
fn call_lua() {
    let f = Fixture::new();
    let num = |x: f64| fc4_adv_lua_value {
        kind: FC4_ADV_VALUE_NUMBER,
        boolean: 0,
        number: x,
        str_: std::ptr::null(),
        len: 0,
        subject: std::ptr::null_mut(),
    };
    let mut out = [num(0.0); 2];
    unsafe {
        let args = [num(5.0), num(3.0)];
        let n = fc4_advisors_call_lua(
            f.p,
            b"_combine_guids\0".as_ptr() as *const c_char,
            args.as_ptr(),
            2,
            out.as_mut_ptr(),
            2,
        );
        assert_eq!(n, 1);
        assert_eq!((out[0].kind, out[0].number), (FC4_ADV_VALUE_NUMBER, 6.0));
        let n = fc4_advisors_call_lua(
            f.p,
            b"no_such\0".as_ptr() as *const c_char,
            std::ptr::null(),
            0,
            out.as_mut_ptr(),
            2,
        );
        assert_eq!(n, -1);
        let n = fc4_advisors_call_lua(
            f.p,
            b"trend_value\0".as_ptr() as *const c_char,
            args.as_ptr(),
            2,
            out.as_mut_ptr(),
            2,
        );
        assert_eq!((n, out[0].number), (1, 42.0));
        let args = [num(7.0)];
        let n = fc4_advisors_call_lua(
            f.p,
            b"ordinance_name\0".as_ptr() as *const c_char,
            args.as_ptr(),
            1,
            out.as_mut_ptr(),
            2,
        );
        assert_eq!((n, out[0].kind), (1, FC4_ADV_VALUE_STRING));
        assert_eq!(std::slice::from_raw_parts(out[0].str_, out[0].len), b"Tax");
        // Missing ordinance: nil.
        let args = [num(8.0)];
        let n = fc4_advisors_call_lua(
            f.p,
            b"ordinance_name\0".as_ptr() as *const c_char,
            args.as_ptr(),
            1,
            out.as_mut_ptr(),
            2,
        );
        assert_eq!((n, out[0].kind), (1, FC4_ADV_VALUE_NIL));
        // The count without copying.
        let n = fc4_advisors_call_lua(
            f.p,
            b"print\0".as_ptr() as *const c_char,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(n, 0);
    }
}

#[test]
fn mysim_record_round_trip() {
    let f = Fixture::new();
    unsafe {
        assert!(fc4_mysims_agent_init(f.p));
        fc4_mysims_move_in(
            f.p,
            b"Ann".as_ptr(),
            3,
            false,
            4,
            b"b".as_ptr(),
            1,
            9,
            b"p".as_ptr(),
            1,
            2,
        );
        assert!(fc4_mysim_set_residence(f.p, 1, HOME));
        let o = fc4_advisors_advisor_object(f.p, 1);
        let mut out = Out::default();
        assert!(fc4_adv_obj_write(o, &ostream(&mut out)));
        // Version 17, state 1, slot 2, then the residence (host token).
        assert_eq!(&out.bytes[..4], &[17, 0, 1, 2]);
        assert_eq!(out.refs[0], (HOME, 0));
        // Read into a new MySim; written again: the same bytes.
        let n = fc4_advisors_create_object(f.p, clsid::MYSIM);
        assert_eq!(fc4_adv_obj_kind(n), FC4_ADV_OBJ_ADVISOR);
        let mut inp = In::new(&out.bytes);
        assert!(fc4_adv_obj_read(n, &istream(&mut inp)));
        assert_eq!(f.game.refs(HOME), 2, "the read residence is held");
        let mut again = Out::default();
        assert!(fc4_adv_obj_write(n, &ostream(&mut again)));
        assert_eq!(again.bytes, out.bytes);
        // Into the map: then visible by ID.
        assert!(fc4_advisors_add_advisor(f.p, n));
        fc4_adv_obj_destroy(o);
        fc4_adv_obj_destroy(n);
        // A bad version is rejected.
        let mut bad = out.bytes.clone();
        bad[0] = 18;
        let x = fc4_advisors_create_object(f.p, clsid::MYSIM);
        let mut inp = In::new(&bad);
        assert!(!fc4_adv_obj_read(x, &istream(&mut inp)));
        fc4_adv_obj_destroy(x);
        // Unknown classes: NULL.
        assert!(fc4_advisors_create_object(f.p, 0x1234_5678).is_null());
    }
}

#[test]
fn subject_round_trip_and_core_handles() {
    let f = Fixture::new();
    unsafe {
        let s = fc4_advisors_new_subject(f.p);
        assert_eq!(fc4_adv_obj_kind(s), FC4_ADV_OBJ_SUBJECT);
        let v = fc4_adv_subject_state {
            x: 3.0,
            z: 99.0,
            has_location: 1,
            data1: 5,
            data2: 6,
            object: 0x77,
            ..Default::default()
        };
        assert!(fc4_adv_subject_set(s, &v));
        assert_eq!(f.game.refs(0x77), 1);
        let mut got = fc4_adv_subject_state::default();
        assert!(fc4_adv_subject_get(s, &mut got));
        assert_eq!(
            (got.x, got.z, got.data1, got.object),
            (3.0, 64.0, 5, 0x77),
            "clamped to the city"
        );
        let mut out = Out::default();
        assert!(fc4_adv_obj_write(s, &ostream(&mut out)));
        let t = fc4_advisors_create_object(f.p, clsid::SUBJECT);
        let mut inp = In::new(&out.bytes);
        assert!(fc4_adv_obj_read(t, &istream(&mut inp)));
        let mut back = fc4_adv_subject_state::default();
        assert!(fc4_adv_subject_get(t, &mut back));
        assert_eq!(back, got);
        assert!(fc4_adv_obj_set_token(t, 0xABC));
        assert_eq!(fc4_adv_obj_token(t), 0xABC);
        fc4_adv_obj_destroy(s);
        fc4_adv_obj_destroy(t);
        // The system record refers to the advisors by their tokens; read
        // into a second system, the references come back as core handles.
        let sys = fc4_advisors_system_object(f.p);
        assert_eq!(fc4_adv_obj_kind(sys), FC4_ADV_OBJ_SYSTEM);
        assert_eq!(fc4_adv_obj_class(sys), clsid::SYSTEM);
        let mut out = Out::default();
        assert!(fc4_adv_obj_write(sys, &ostream(&mut out)));
        assert_eq!(out.refs.len(), 7);
        assert_eq!(out.refs[0], (f.advisor(1).token, clsid::MYSIM));
        let handles: BTreeMap<u64, usize> = f
            .game
            .created
            .borrow()
            .iter()
            .map(|c| (fc4_adv_obj_token(c.2 as *const fc4_adv_obj), c.2))
            .collect();
        let resolve = |r: &mut fc4_ref| {
            if let Some(h) = handles.get(&r.token) {
                r.core_handle = *h as *mut c_void;
            }
        };
        let game2 = Game::new();
        let (h2, m2) = (advisor_host(&game2), mysim_host(&game2));
        let p2 = fc4_advisors_create(&h2, &m2, 1);
        let sys2 = fc4_advisors_system_object(p2);
        let mut inp = In::new(&out.bytes);
        inp.resolve = Some(&resolve);
        assert!(fc4_adv_obj_read(sys2, &istream(&mut inp)));
        let mut ids = [0i32; 8];
        let n = fc4_advisors_advisor_ids(p2, ids.as_mut_ptr(), ids.len());
        assert_eq!(&ids[..n], &[1, 2, 3, 4, 5, 23, 30]);
        let mut a = fc4_advisor_state::default();
        assert!(fc4_advisors_advisor_state(p2, 30, &mut a));
        assert_eq!(a.token, f.advisor(30).token, "the same object");
        let mut st = fc4_advisors_state::default();
        assert!(fc4_advisors_get_state(p2, &mut st));
        assert_eq!(st.loaded_from_save, 1);
        fc4_adv_obj_destroy(sys);
        fc4_adv_obj_destroy(sys2);
        fc4_advisors_destroy(p2);
    }
}

/// The header's `FC4_ADV_SRC_*` codes are the core's.
#[test]
fn header_source_codes() {
    let header = include_str!("../include/fc4.h");
    let mut seen = 0;
    for line in header.lines() {
        let Some(rest) = line.strip_prefix("#define FC4_ADV_SRC_") else {
            continue;
        };
        let mut it = rest.split_whitespace();
        let (name, code) = (it.next().unwrap(), it.next().unwrap());
        let code: u32 = code.trim_end_matches('u').parse().unwrap();
        let kind = SrcKind::from_code(code).unwrap_or_else(|| panic!("{name} = {code}"));
        let snake: String = kind
            .name()
            .chars()
            .enumerate()
            .flat_map(|(i, c)| {
                let sep = (i > 0 && c.is_ascii_uppercase()).then_some('_');
                sep.into_iter().chain(std::iter::once(c.to_ascii_uppercase()))
            })
            .collect();
        assert_eq!(name, snake, "code {code}");
        seen += 1;
    }
    assert_eq!(seen, SrcKind::ALL.len());
}

thread_local! {
    static DROPPED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C" fn object_dropped(_: *mut c_void, t: u64) {
    DROPPED.with(|d| d.borrow_mut().push(t));
}

#[test]
fn private_subject_clone_owns_distinct_state_and_one_more_object_reference() {
    let game = Game::new();
    DROPPED.with(|d| d.borrow_mut().clear());
    unsafe {
        let mut host = advisor_host(&game);
        host.object_dropped = Some(object_dropped);
        let p = fc4_advisors_create(&host, std::ptr::null(), 1);
        assert!(!p.is_null());
        assert!(fc4_private_adv_subject_clone(std::ptr::null()).is_null());
        let system = fc4_advisors_system_object(p);
        assert!(fc4_private_adv_subject_clone(system).is_null());
        for invalid in [std::ptr::null(), system as *const fc4_adv_obj] {
            assert!(!fc4_private_adv_subject_set_location(invalid, 1.0, 2.0));
            assert!(!fc4_private_adv_subject_set_object(invalid, HOME));
            assert!(!fc4_private_adv_subject_set_data1(invalid, 0));
            assert!(!fc4_private_adv_subject_set_data2(invalid, u32::MAX));
        }
        fc4_adv_obj_destroy(system);
        let source = fc4_advisors_new_subject(p);
        let initial = fc4_adv_subject_state {
            x: 3.25,
            z: 7.5,
            has_location: 1,
            object: HOME,
            data1: 0x1234_5600,
            data2: u32::MAX,
            ..Default::default()
        };
        assert!(fc4_adv_subject_set(source, &initial));
        assert_eq!(game.refs(HOME), 1);
        game.reference_events.borrow_mut().clear();
        let copy = fc4_private_adv_subject_clone(source);
        assert!(!copy.is_null());
        assert_eq!(fc4_adv_obj_kind(copy), FC4_ADV_OBJ_SUBJECT);
        assert!(!fc4_adv_obj_same(source, copy));
        assert_ne!(fc4_adv_obj_token(source), fc4_adv_obj_token(copy));
        assert_eq!(game.refs(HOME), 2, "clone holds the same foreign object again");
        assert_eq!(*game.reference_events.borrow(), vec![(true, HOME)]);
        let mut state = fc4_adv_subject_state::default();
        assert!(fc4_adv_subject_get(copy, &mut state));
        assert_eq!(state, initial);
        let changed = fc4_adv_subject_state {
            object: OTHER,
            data1: 0x8000_0000,
            x: 17.0,
            ..initial
        };
        game.reference_events.borrow_mut().clear();
        assert!(fc4_private_adv_subject_set_location(copy, changed.x, changed.z));
        for word in [0, 0x1234_5600, 0x8000_0000, u32::MAX] {
            assert!(fc4_private_adv_subject_set_data1(copy, word));
            assert!(fc4_private_adv_subject_set_data2(copy, word));
            assert!(fc4_adv_subject_get(copy, &mut state));
            assert_eq!((state.data1, state.data2), (word, word));
        }
        assert!(fc4_private_adv_subject_set_data1(copy, changed.data1));
        assert!(
            game.reference_events.borrow().is_empty(),
            "scalar setters do not re-hold object"
        );
        assert!(fc4_private_adv_subject_set_object(copy, OTHER));
        assert_eq!(*game.reference_events.borrow(), vec![(true, OTHER), (false, HOME)]);
        assert!(fc4_adv_subject_get(source, &mut state));
        assert_eq!(state, initial, "copy mutation leaves the source intact");
        assert_eq!((game.refs(HOME), game.refs(OTHER)), (1, 1));
        // Remove only the host-owned callback handles, leaving the returned
        // owned handles responsible for final destruction (no invented owner).
        for (_, _, h) in game.created.borrow_mut().drain(..) {
            fc4_adv_obj_destroy(h as *mut fc4_adv_obj);
        }
        let copy_token = fc4_adv_obj_token(copy);
        let source_token = fc4_adv_obj_token(source);
        fc4_adv_obj_destroy(source);
        assert_eq!((game.refs(HOME), game.refs(OTHER)), (0, 1));
        assert!(fc4_adv_subject_get(copy, &mut state));
        assert_eq!(state, changed, "copy survives destruction of source");
        fc4_adv_obj_destroy(copy);
        assert_eq!((game.refs(HOME), game.refs(OTHER)), (0, 0));
        assert_eq!(DROPPED.with(|d| d.borrow().clone()), vec![source_token, copy_token]);
        fc4_advisors_destroy(p);
    }
}

#[test]
fn weak_handles_and_object_dropped() {
    let f = Fixture::new();
    unsafe {
        // The fixture's host table predates the callback: set it on a copy.
        let mut h = advisor_host(&f.game);
        h.object_dropped = Some(object_dropped);
        let p = fc4_advisors_create(&h, std::ptr::null(), 0x1234);
        assert!(!p.is_null());
        let s = fc4_advisors_new_subject(p);
        let token = fc4_adv_obj_token(s);
        assert_ne!(token, 0);
        // The host's own handle from object_created: keep only a weak one.
        let host = f.game.created.borrow_mut().pop().unwrap().2 as *mut fc4_adv_obj;
        assert!(fc4_adv_obj_same(host, s));
        let w = fc4_adv_obj_downgrade(host);
        fc4_adv_obj_destroy(host);
        let up = fc4_adv_weak_upgrade(w);
        assert!(fc4_adv_obj_same(up, s), "alive while a strong handle exists");
        fc4_adv_obj_destroy(up);
        assert!(DROPPED.with(|d| d.borrow().is_empty()));
        fc4_adv_obj_destroy(s);
        assert_eq!(DROPPED.with(|d| d.borrow().clone()), vec![token]);
        assert!(fc4_adv_weak_upgrade(w).is_null(), "gone after the last strong handle");
        fc4_adv_weak_destroy(w);
        // Advisor objects: per-object calls agree with the id-based ones.
        let a = fc4_advisors_advisor_object(f.p, 23);
        assert_eq!(fc4_adv_advisor_id(a), 23);
        let mut buf = [0 as c_char; 32];
        assert_eq!(fc4_adv_advisor_text(a, 0, buf.as_mut_ptr(), buf.len()), 7);
        assert!(fc4_adv_advisor_set_text(a, 1, b"Bob".as_ptr(), 3));
        assert_eq!(fc4_advisors_advisor_text(f.p, 23, 1, buf.as_mut_ptr(), buf.len()), 3);
        assert!(fc4_adv_advisor_advice_by_guid(a, 0xDEAD_BEEF).is_null());
        assert!(fc4_adv_advisor_current_advice(a, 0, std::ptr::null_mut()).is_null());
        fc4_adv_obj_destroy(a);
        fc4_advisors_destroy(p);
    }
}
