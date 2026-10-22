// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The tutorial system through the C ABI (docs/specs/tutorial.md): a fake
//! game behind `fc4_tutorial_host`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::c_void;

use crate::test_streams::{istream, ostream, In, Out};
use crate::tutorial::*;

const WINDOW: u64 = 0x40;
const TASK: u64 = 0x50;
const LUA: u64 = 0x30;
const CITY_INIT_COMPLETE: u32 = 0xEA8A_E29A;
const END_TUTORIAL: u32 = 0xAA38_40E6;
const PRE_SAVE: u32 = 0x26C6_3343;

#[derive(Default)]
struct Game {
    log: RefCell<Vec<String>>,
    refs: RefCell<BTreeMap<u64, i32>>,
    registry: RefCell<BTreeMap<u32, bool>>,
    cheats: Cell<bool>,
    task: Cell<u64>,
    registry_al: Cell<bool>,
}

impl Game {
    fn new() -> Box<Game> {
        Box::new(Game::default())
    }
    fn user(&self) -> *mut c_void {
        self as *const Game as *mut c_void
    }
    fn log(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    fn refs(&self, t: u64) -> i32 {
        self.refs.borrow().get(&t).copied().unwrap_or(0)
    }
}

fn game(u: *mut c_void) -> &'static Game {
    unsafe { &*(u as *const Game) }
}

unsafe extern "C" fn subscribe(u: *mut c_void, m: u32) {
    game(u).log(format!("subscribe {m:08X}"));
}
unsafe extern "C" fn unsubscribe(u: *mut c_void, m: u32) {
    game(u).log(format!("unsubscribe {m:08X}"));
}
unsafe extern "C" fn cheats_available(u: *mut c_void) -> bool {
    game(u).cheats.get()
}
unsafe extern "C" fn has_cheat_manager(u: *mut c_void) -> bool {
    game(u).cheats.get()
}
unsafe extern "C" fn register_cheat(u: *mut c_void) {
    game(u).log("register cheat");
}
unsafe extern "C" fn unregister_cheat(u: *mut c_void) {
    game(u).log("unregister cheat");
}
unsafe extern "C" fn add_ref(u: *mut c_void, t: u64) {
    *game(u).refs.borrow_mut().entry(t).or_default() += 1;
}
unsafe extern "C" fn release(u: *mut c_void, t: u64) {
    *game(u).refs.borrow_mut().entry(t).or_default() -= 1;
}
unsafe extern "C" fn create_window(u: *mut c_void) -> u64 {
    game(u).log("create window");
    // TU9: the native constructor starts at zero; the owner takes both refs.
    WINDOW
}
unsafe extern "C" fn init_window(u: *mut c_void, w: u64) {
    game(u).log(format!("init window {w:X}"));
}
unsafe extern "C" fn close_window(u: *mut c_void, w: u64) {
    game(u).log(format!("close window {w:X}"));
}
unsafe extern "C" fn window_current_task(u: *mut c_void, _w: u64) -> u64 {
    game(u).task.get()
}
unsafe extern "C" fn task_edge_scrolling_allowed(_u: *mut c_void, t: u64) -> bool {
    t != TASK
}
unsafe extern "C" fn window_filter_value(u: *mut c_void, _w: u64, v: u32) {
    game(u).log(format!("filter {v:X}"));
}
unsafe extern "C" fn window_show_hints(u: *mut c_void, _w: u64) {
    game(u).log("hints");
}
unsafe extern "C" fn window_catalog_menu(u: *mut c_void, _w: u64, m: u64) {
    game(u).log(format!("menu {m:X}"));
}
unsafe extern "C" fn window_left_click(_u: *mut c_void, _w: u64, x: *mut i32, z: *mut i32, n: i32) -> bool {
    *x += n;
    *z -= n;
    false
}
unsafe extern "C" fn window_reset(u: *mut c_void, _w: u64, a: i32, b: i32) -> i32 {
    game(u).log(format!("reset {a} {b}"));
    b + 1
}
unsafe extern "C" fn window_event(u: *mut c_void, _w: u64, e: u32) {
    game(u).log(format!("event {e:X}"));
}
unsafe extern "C" fn registry_set_active(u: *mut c_void, guid: u32, active: bool) -> bool {
    game(u).log(format!("set active {guid:08X} {active}"));
    game(u).registry_al.get()
}
unsafe extern "C" fn registry_entry_enabled(u: *mut c_void, guid: u32, out: *mut bool) -> bool {
    match game(u).registry.borrow().get(&guid) {
        Some(&v) => {
            *out = v;
            true
        }
        None => false,
    }
}
unsafe extern "C" fn view_restore_controls(u: *mut c_void) {
    game(u).log("view restore");
}
unsafe extern "C" fn fetch_lua_context(u: *mut c_void) -> u64 {
    add_ref(u, LUA);
    LUA
}

fn host(g: &Game) -> fc4_tutorial_host {
    fc4_tutorial_host {
        struct_size: std::mem::size_of::<fc4_tutorial_host>() as u32,
        user: g.user(),
        subscribe: Some(subscribe),
        unsubscribe: Some(unsubscribe),
        cheats_available: Some(cheats_available),
        has_cheat_manager: Some(has_cheat_manager),
        register_cheat: Some(register_cheat),
        unregister_cheat: Some(unregister_cheat),
        add_ref: Some(add_ref),
        release: Some(release),
        create_window: Some(create_window),
        init_window: Some(init_window),
        close_window: Some(close_window),
        window_current_task: Some(window_current_task),
        task_edge_scrolling_allowed: Some(task_edge_scrolling_allowed),
        window_filter_value: Some(window_filter_value),
        window_show_hints: Some(window_show_hints),
        window_catalog_menu: Some(window_catalog_menu),
        window_left_click: Some(window_left_click),
        window_reset: Some(window_reset),
        window_event: Some(window_event),
        registry_set_active: Some(registry_set_active),
        registry_entry_enabled: Some(registry_entry_enabled),
        view_restore_controls: Some(view_restore_controls),
        fetch_lua_context: Some(fetch_lua_context),
    }
}

fn create(g: &Game) -> *mut fc4_tutorial {
    let t = unsafe { fc4_tutorial_create(&host(g), 0x1000) };
    assert!(!t.is_null());
    t
}

fn state(t: *const fc4_tutorial) -> fc4_tutorial_state {
    let mut s = fc4_tutorial_state::default();
    assert!(unsafe { fc4_tutorial_get_state(t, &mut s) });
    s
}

fn record(t: *const fc4_tutorial) -> Vec<u8> {
    let mut o = Out::default();
    let os = ostream(&mut o);
    assert!(unsafe { fc4_tutorial_write(t, &os) });
    o.bytes
}

fn read(t: *mut fc4_tutorial, bytes: &[u8]) -> bool {
    let mut i = In::new(bytes);
    let is = istream(&mut i);
    unsafe { fc4_tutorial_read(t, &is) }
}

#[test]
fn prepare_cheat_save_sets_all_fields_before_callbacks() {
    let g = Game::new();
    let t = create(&g);
    unsafe {
        fc4_tutorial_set_task_number(t, 27);
        fc4_tutorial_end(t, false);
        fc4_tutorial_prepare_city_save(t, 0xCC02_D3A3);
    }
    let s = state(t);
    assert_eq!((s.is_tutorial, s.guid, s.task_number, s.end_pending), (1, 0xCC02_D3A3, 0, 0));
    assert_eq!(s.window, 0);
    unsafe { fc4_tutorial_destroy(t) };
}

#[test]
fn create_rejects_a_short_table_and_starts_constructed() {
    let g = Game::new();
    let mut h = host(&g);
    h.struct_size -= 1;
    assert!(unsafe { fc4_tutorial_create(&h, 0) }.is_null());
    assert!(unsafe { fc4_tutorial_create(std::ptr::null(), 0) }.is_null());
    let t = create(&g);
    let s = state(t);
    assert_eq!(s.struct_size as usize, std::mem::size_of::<fc4_tutorial_state>());
    assert_eq!(
        (s.is_tutorial, s.task_number, s.guid, s.field_28),
        (0, 0, 0xFFFF_FFFF, 3)
    );
    assert_eq!((s.window, s.lua_context, s.ref_count, s.destroyed), (0, 0, 0, 0));
    assert!(fc4_tutorial_query_interface(0xAA38_36F6));
    assert!(fc4_tutorial_query_interface(1));
    assert!(!fc4_tutorial_query_interface(0x1234_5678));
    unsafe {
        assert_eq!(fc4_tutorial_add_ref(t), 1);
        assert_eq!(fc4_tutorial_add_ref(t), 2);
        assert_eq!(fc4_tutorial_release(t), 1);
        assert_eq!(fc4_tutorial_release(t), 0);
        assert_eq!(state(t).destroyed, 1);
        fc4_tutorial_destroy(t);
        // NULL handles read as the fallbacks.
        assert_eq!(fc4_tutorial_is_tutorial(std::ptr::null()), 0);
        assert!(fc4_tutorial_is_edge_scrolling_allowed(std::ptr::null()));
        fc4_tutorial_destroy(std::ptr::null_mut());
    }
}

#[test]
fn plain_record_round_trips() {
    let g = Game::new();
    let t = create(&g);
    // §8 test 1: a plain city.
    assert_eq!(record(t), [2, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    // §8 test 2: a tutorial city.
    assert!(read(t, &[2, 0, 1, 3, 0, 0, 0, 0x6C, 0x7A, 0x5B, 0x8A]));
    assert_eq!(unsafe { fc4_tutorial_is_tutorial(t) }, 1);
    assert_eq!(unsafe { fc4_tutorial_task_number(t) }, 3);
    assert_eq!(unsafe { fc4_tutorial_guid(t) }, 0x8A5B_7A6C);
    assert_eq!(record(t), [2, 0, 1, 3, 0, 0, 0, 0x6C, 0x7A, 0x5B, 0x8A]);
    unsafe { fc4_tutorial_destroy(t) };
}

#[test]
fn guid_mapping_and_version() {
    let g = Game::new();
    let t = create(&g);
    // §8 test 3: saved GUID 1 reads as 0xEA5D6DC8.
    assert!(read(t, &[2, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0]));
    assert_eq!(state(t).guid, 0xEA5D_6DC8);
    assert!(read(t, &[2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]));
    assert_eq!(state(t).guid, 0x8A5B_7A6C);
    // §8 test 4: version 1 fails and changes nothing.
    assert!(!read(t, &[1, 0, 0, 9, 0, 0, 0, 7, 0, 0, 0]));
    let s = state(t);
    assert_eq!((s.is_tutorial, s.task_number, s.guid), (1, 0, 0x8A5B_7A6C));
    // NULL streams are refused.
    unsafe {
        assert!(!fc4_tutorial_read(t, std::ptr::null()));
        assert!(!fc4_tutorial_write(t, std::ptr::null()));
        fc4_tutorial_destroy(t);
    }
}

#[test]
fn init_shutdown_and_messages() {
    let g = Game::new();
    g.cheats.set(true);
    g.registry.borrow_mut().insert(0x8A5B_7A6C, true);
    g.registry_al.set(true);
    let t = create(&g);
    unsafe {
        assert!(fc4_tutorial_init(t));
        assert_eq!(
            g.take(),
            [
                "subscribe EA8AE29A",
                "subscribe AA3840E6",
                "subscribe 26C63343",
                "register cheat"
            ]
        );
        assert!(read(t, &[2, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0]));
        // City init complete starts the tutorial once.
        assert!(fc4_tutorial_do_message(t, CITY_INIT_COMPLETE, 0));
        assert_eq!(
            g.take(),
            ["create window", "init window 40", "set active 8A5B7A6C true"]
        );
        assert_eq!(g.refs(WINDOW), 2, "the two references startTutorial took (TU5)");
        let s = state(t);
        assert_eq!((s.window, s.start_tried, s.end_pending), (WINDOW, 1, 0));
        assert!(fc4_tutorial_do_message(t, CITY_INIT_COMPLETE, 0));
        assert!(g.take().is_empty());
        // Tutorial_EndTutorial with data 1: ends without the registry.
        assert!(fc4_tutorial_do_message(t, END_TUTORIAL, 1));
        assert_eq!(g.take(), ["close window 40", "view restore"]);
        assert_eq!(g.refs(WINDOW), 0);
        let s = state(t);
        assert_eq!((s.window, s.is_tutorial, s.end_pending), (0, 0, 1));
        // §8 test 5: pre-save ends the registry entry.
        assert!(fc4_tutorial_do_message(t, PRE_SAVE, 0));
        assert_eq!(g.take(), ["set active 8A5B7A6C false", "view restore"]);
        assert_eq!(state(t).end_pending, 0);
        assert_eq!(record(t), [2, 0, 0, 2, 0, 0, 0, 0x6C, 0x7A, 0x5B, 0x8A]);
        assert!(fc4_tutorial_shutdown(t));
        // [TU5] The cheat first (no `cISC4App` slot 21 test), then the
        // window (already gone here), then the messages in the spec's order.
        assert_eq!(
            g.take(),
            [
                "unregister cheat",
                "unsubscribe AA3840E6",
                "unsubscribe EA8AE29A",
                "unsubscribe 26C63343"
            ]
        );
        fc4_tutorial_destroy(t);
    }
}

#[test]
fn start_end_and_window_slots() {
    let g = Game::new();
    let t = create(&g);
    unsafe {
        // Without a window: the slots are inert.
        assert!(fc4_tutorial_is_edge_scrolling_allowed(t));
        assert_eq!(fc4_tutorial_current_task(t), 0);
        assert!(!fc4_tutorial_reset_num_and_index(t, 5, 6));
        let (mut x, mut z) = (10, 20);
        assert!(fc4_tutorial_filter_left_mouse_click(t, &mut x, &mut z, 3));
        assert_eq!((x, z), (10, 20));
        // startTutorial: the registry's AL (false here).
        assert!(!fc4_tutorial_start(t));
        assert_eq!(fc4_tutorial_is_tutorial(t), 1);
        g.take();
        g.task.set(TASK);
        assert_eq!(fc4_tutorial_current_task(t), TASK);
        assert!(!fc4_tutorial_is_edge_scrolling_allowed(t));
        assert!(!fc4_tutorial_filter_left_mouse_click(t, &mut x, &mut z, 3));
        assert_eq!((x, z), (13, 17));
        assert!(fc4_tutorial_filter_message(t, 3, 0x2872_59F6, 0xA, 0xB));
        assert!(fc4_tutorial_filter_message(t, 3, 0x49D9_93D4, 0xA, 0xB));
        assert!(fc4_tutorial_filter_message(t, 2, 0x49D9_93D4, 0xA, 0xB));
        fc4_tutorial_show_ui_hints(t);
        assert!(fc4_tutorial_handle_catalog_menu(t, 0x77));
        assert!(fc4_tutorial_on_event(t, 0x99));
        assert!(fc4_tutorial_reset_num_and_index(t, 5, 6));
        assert_eq!(
            g.take(),
            ["filter A", "filter B", "hints", "menu 77", "event 99", "reset 5 6"]
        );
        assert_eq!((fc4_tutorial_guid(t), fc4_tutorial_task_number(t)), (5, 7));
        fc4_tutorial_set_task_number(t, 41);
        fc4_tutorial_increment_task_number(t);
        assert_eq!(fc4_tutorial_task_number(t), 42);
        // The Lua context is fetched once.
        assert_eq!(fc4_tutorial_scripting_context(t), LUA);
        assert_eq!(fc4_tutorial_scripting_context(t), LUA);
        assert_eq!(g.refs(LUA), 1);
        // endTutorial(true) returns its argument.
        assert!(fc4_tutorial_end(t, true));
        assert_eq!(
            g.take(),
            ["close window 40", "set active 00000005 false", "view restore"]
        );
        assert!(!fc4_tutorial_end(t, false));
        assert_eq!(state(t).end_pending, 1);
        fc4_tutorial_destroy(t);
    }
}
