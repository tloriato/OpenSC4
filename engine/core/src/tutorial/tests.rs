// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! A fake game and the §8 tests of `docs/specs/tutorial.md`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::stream::{ByteReader, ByteWriter, SaveObject};

use super::*;

const WINDOW: u64 = 0x40;
const TASK: u64 = 0x50;
const LUA: u64 = 0x30;

/// The fake game: a call log, reference counts and the registry.
#[derive(Default)]
pub(crate) struct Game {
    pub log: RefCell<Vec<String>>,
    pub refs: RefCell<BTreeMap<u64, i64>>,
    /// Registry entries: GUID → byte +0x10.
    pub registry: RefCell<BTreeMap<u32, bool>>,
    /// GUIDs `SetActive(…, true)`.
    pub active: RefCell<BTreeMap<u32, bool>>,
    pub cheats: Cell<bool>,
    pub task: Cell<Option<u64>>,
    pub task_edge: Cell<bool>,
    pub click: Cell<bool>,
    pub reset_answer: Cell<i32>,
    pub registry_al: Cell<bool>,
    pub on_window_init: RefCell<Option<Box<dyn Fn()>>>,
}

impl Game {
    pub fn new() -> Rc<Game> {
        Rc::new(Game::default())
    }

    fn ev(&self, s: String) {
        self.log.borrow_mut().push(s);
    }

    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    pub fn refs(&self, t: u64) -> i64 {
        self.refs.borrow().get(&t).copied().unwrap_or(0)
    }
}

impl TutorialHost for Game {
    fn subscribe(&self, m: u32) {
        self.ev(format!("subscribe {m:08X}"));
    }
    fn unsubscribe(&self, m: u32) {
        self.ev(format!("unsubscribe {m:08X}"));
    }
    fn cheats_available(&self) -> bool {
        self.cheats.get()
    }
    fn has_cheat_manager(&self) -> bool {
        self.cheats.get()
    }
    fn register_cheat(&self) {
        self.ev("register_cheat".into());
    }
    fn unregister_cheat(&self) {
        self.ev("unregister_cheat".into());
    }
    fn add_ref(&self, t: u64) {
        *self.refs.borrow_mut().entry(t).or_default() += 1;
    }
    fn release(&self, t: u64) {
        *self.refs.borrow_mut().entry(t).or_default() -= 1;
    }
    fn create_window(&self) -> u64 {
        self.ev("create_window".into());
        *self.refs.borrow_mut().entry(WINDOW).or_default() += 1; // the creation count
        WINDOW
    }
    fn init_window(&self, w: u64) {
        self.ev(format!("init_window {w:X}"));
        if let Some(callback) = self.on_window_init.borrow().as_ref() {
            callback();
        }
    }
    fn close_window(&self, w: u64) {
        self.ev(format!("close_window {w:X}"));
    }
    fn window_current_task(&self, _w: u64) -> Option<u64> {
        self.task.get()
    }
    fn task_edge_scrolling_allowed(&self, _t: u64) -> bool {
        self.task_edge.get()
    }
    fn window_filter_value(&self, _w: u64, v: u32) {
        self.ev(format!("filter {v}"));
    }
    fn window_show_hints(&self, _w: u64) {
        self.ev("hints".into());
    }
    fn window_catalog_menu(&self, _w: u64, menu: u64) {
        self.ev(format!("menu {menu:X}"));
    }
    fn window_left_click(&self, _w: u64, x: &mut i32, z: &mut i32, n: i32) -> bool {
        *x += 1;
        *z += n;
        self.click.get()
    }
    fn window_reset(&self, _w: u64, a: i32, b: i32) -> i32 {
        self.ev(format!("reset {a} {b}"));
        self.reset_answer.get()
    }
    fn window_event(&self, _w: u64, e: u32) {
        self.ev(format!("event {e}"));
    }
    fn registry_set_active(&self, guid: u32, active: bool) -> bool {
        self.ev(format!("set_active {guid:08X} {active}"));
        self.active.borrow_mut().insert(guid, active);
        self.registry_al.get()
    }
    fn registry_entry_enabled(&self, guid: u32) -> Option<bool> {
        self.registry.borrow().get(&guid).copied()
    }
    fn view_restore_controls(&self) {
        self.ev("view_restore".into());
    }
    fn fetch_lua_context(&self) -> Option<u64> {
        self.ev("fetch_lua".into());
        self.add_ref(LUA);
        Some(LUA)
    }
}

fn system(g: &Rc<Game>) -> Rc<TutorialSystem> {
    TutorialSystem::new(g.clone(), 0x1000)
}

fn record(t: &TutorialSystem) -> Vec<u8> {
    let mut w = ByteWriter::new();
    assert!(t.write(&mut w));
    w.into_bytes()
}

fn read(t: &TutorialSystem, bytes: &[u8]) -> bool {
    t.read(&mut ByteReader::new(bytes))
}

#[test]
fn identity_and_constructor() {
    assert!(query_interface(IID_TUTORIAL_SYSTEM));
    assert!(query_interface(IID_MESSAGE_TARGET2));
    assert!(query_interface(IID_CHEAT_HANDLER));
    assert!(query_interface(IID_SERIALIZABLE));
    assert!(query_interface(IID_UNKNOWN));
    assert!(!query_interface(0x1234_5678));
    let g = Game::new();
    let t = system(&g);
    assert_eq!(t.class_id(), CLSID_TUTORIAL_SYSTEM);
    let s = t.snapshot();
    assert_eq!((s.guid, s.field_28, s.is_tutorial, s.task_number), (NO_GUID, 3, 0, 0));
    assert_eq!(t.add_ref(), 1);
    assert_eq!(t.add_ref(), 2);
    assert_eq!(t.release(), 1);
    assert_eq!(t.release(), 0);
    assert!(t.snapshot().destroyed);
}

#[test]
fn window_init_can_synchronously_query_and_advance_its_owner() {
    let g = Game::new();
    let t = Rc::new(system(&g));
    g.task.set(Some(TASK));
    let weak = Rc::downgrade(&t);
    *g.on_window_init.borrow_mut() = Some(Box::new(move || {
        let t = weak.upgrade().unwrap();
        assert_eq!(t.current_task(), Some(TASK));
        t.increment_task_number();
        assert_eq!(t.task_number(), 1);
    }));
    t.start_tutorial();
    assert_eq!(t.task_number(), 1);
    assert_eq!(g.refs(WINDOW), 2);
    t.end_tutorial(true);
    assert_eq!(g.refs(WINDOW), 0);
}

#[test]
fn t1_plain_record() {
    let g = Game::new();
    let t = system(&g);
    let bytes = [0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(read(&t, &bytes));
    assert_eq!(t.is_tutorial(), 0);
    assert_eq!(t.task_number(), 0);
    assert_eq!(t.guid(), 0xFFFF_FFFF);
    assert_eq!(record(&t), bytes.to_vec());
    assert_eq!(bytes.len(), RECORD_SIZE);
}

#[test]
fn t2_tutorial_record() {
    let g = Game::new();
    let t = system(&g);
    assert!(read(
        &t,
        &[0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0xA3, 0xD3, 0x02, 0xCC]
    ));
    assert_eq!(t.is_tutorial(), 1);
    assert_eq!(t.guid(), 0xCC02_D3A3);
    // The consumers read IsTutorial: the fire's no-burn-down flag and the
    // flammability roll are covered by their own cores' tests.
}

#[test]
fn t3_guid_mapping() {
    let g = Game::new();
    for (saved, want) in [
        ([0x01, 0, 0, 0], 0xEA5D_6DC8),
        ([0x00, 0, 0, 0], 0x8A5B_7A6C),
        ([0x02, 0, 0, 0], 2),
    ] {
        let t = system(&g);
        let mut b = vec![0x02, 0x00, 0x01, 0, 0, 0, 0];
        b.extend_from_slice(&saved);
        assert!(read(&t, &b));
        assert_eq!(t.guid(), want);
    }
    // Quirk 1: saving again writes the mapped GUID.
    let t = system(&g);
    assert!(read(&t, &[0x02, 0x00, 0x01, 0, 0, 0, 0, 1, 0, 0, 0]));
    assert_eq!(record(&t), vec![0x02, 0x00, 0x01, 0, 0, 0, 0, 0xC8, 0x6D, 0x5D, 0xEA]);
}

#[test]
fn t4_version() {
    let g = Game::new();
    let t = system(&g);
    assert!(!read(&t, &[0x03, 0x00, 0x01, 0, 0, 0, 0, 1, 0, 0, 0]));
    let s = t.snapshot();
    assert_eq!((s.guid, s.is_tutorial, s.task_number), (NO_GUID, 0, 0));
    // A cut record reads what it can and reports the error.
    let t = system(&g);
    assert!(!read(&t, &[0x02, 0x00, 0x01]));
    assert_eq!(t.is_tutorial(), 1);
    assert_eq!(t.guid(), NO_GUID);
    assert!(!read(&system(&g), &[]));
}

#[test]
fn t5_end_without_registry_then_save() {
    let g = Game::new();
    let t = system(&g);
    assert!(read(&t, &[0x02, 0x00, 0x01, 0, 0, 0, 0, 0xA3, 0xD3, 0x02, 0xCC]));
    g.registry_al.set(true);
    assert!(t.start_tutorial());
    assert_eq!(g.refs(WINDOW), 2, "the two references startTutorial took (TU5)");
    assert_eq!(
        g.take_log(),
        vec!["create_window", "init_window 40", "set_active CC02D3A3 true"]
    );
    assert!(!t.end_tutorial(false));
    assert_eq!(t.is_tutorial(), 0);
    let s = t.snapshot();
    assert!(s.end_pending);
    assert_eq!(s.window, None);
    assert_eq!(g.refs(WINDOW), 0);
    assert_eq!(g.active.borrow().get(&0xCC02_D3A3), Some(&true));
    assert_eq!(g.take_log(), vec!["close_window 40", "view_restore"]);
    assert!(t.do_message(msg::PRE_SAVE, 0));
    assert_eq!(g.active.borrow().get(&0xCC02_D3A3), Some(&false));
    assert!(!t.snapshot().end_pending);
    assert_eq!(g.take_log(), vec!["set_active CC02D3A3 false", "view_restore"]);
    // Nothing pending: the next pre-save does nothing.
    assert!(t.do_message(msg::PRE_SAVE, 0));
    assert!(g.take_log().is_empty());
}

#[test]
fn t6_task_counter() {
    let g = Game::new();
    let t = system(&g);
    t.set_task_number(4);
    t.increment_task_number();
    t.increment_task_number();
    assert_eq!(t.task_number(), 6);
    assert_eq!(record(&t), vec![0x02, 0x00, 0x00, 6, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
}

#[test]
fn write_on_a_failed_stream_writes_nothing() {
    struct Failed(ByteWriter);
    impl crate::stream::OutStream for Failed {
        fn put_u8(&mut self, v: u8) -> bool {
            self.0.put_u8(v)
        }
        fn put_u16(&mut self, v: u16) -> bool {
            self.0.put_u16(v)
        }
        fn put_u32(&mut self, v: u32) -> bool {
            self.0.put_u32(v)
        }
        fn put_u64(&mut self, v: u64) -> bool {
            self.0.put_u64(v)
        }
        fn put_f32(&mut self, v: f32) -> bool {
            self.0.put_f32(v)
        }
        fn put_f64(&mut self, v: f64) -> bool {
            self.0.put_f64(v)
        }
        fn put_bytes(&mut self, v: &[u8]) -> bool {
            self.0.put_bytes(v)
        }
        fn put_str(&mut self, v: &[u8]) -> bool {
            self.0.put_str(v)
        }
        fn put_ref(&mut self, r: &crate::stream::RefValue) -> bool {
            self.0.put_ref(r)
        }
        fn error(&self) -> i32 {
            1
        }
    }
    let g = Game::new();
    let t = system(&g);
    let mut f = Failed(ByteWriter::new());
    assert!(!t.write(&mut f));
    assert!(f.0.into_bytes().is_empty());
}

#[test]
fn init_and_shutdown() {
    let g = Game::new();
    let t = system(&g);
    assert!(t.init());
    assert_eq!(
        g.take_log(),
        vec!["subscribe EA8AE29A", "subscribe AA3840E6", "subscribe 26C63343"]
    );
    g.cheats.set(true);
    assert!(t.init());
    assert_eq!(
        g.take_log(),
        vec![
            "subscribe EA8AE29A",
            "subscribe AA3840E6",
            "subscribe 26C63343",
            "register_cheat"
        ]
    );
    assert!(t.shutdown());
    // [TU5] The cheat goes first (no `cISC4App` slot 21 test), then the
    // window (none here), then the messages in the spec's order.
    assert_eq!(
        g.take_log(),
        vec![
            "unregister_cheat",
            "unsubscribe AA3840E6",
            "unsubscribe EA8AE29A",
            "unsubscribe 26C63343"
        ]
    );
}

/// §T 7: `Shutdown` with a running tutorial, a window and a cheat manager
/// (TU5).
#[test]
fn t7_shutdown() {
    let g = Game::new();
    let t = system(&g);
    g.registry_al.set(true);
    assert!(t.init());
    assert!(t.start_tutorial());
    assert_eq!(t.snapshot().window, Some(WINDOW));
    assert_eq!(g.refs(WINDOW), 2);
    g.take_log();
    // `Init` never registered a cheat here (`cheats_available` was false at
    // the time); `Shutdown` unregisters one anyway, without the slot 21
    // test, and drops both window references.
    g.cheats.set(true);
    assert!(t.shutdown());
    assert_eq!(
        g.take_log(),
        vec![
            "unregister_cheat",
            "close_window 40",
            "unsubscribe AA3840E6",
            "unsubscribe EA8AE29A",
            "unsubscribe 26C63343"
        ]
    );
    let s = t.snapshot();
    assert_eq!(s.window, None, "+0x40 is cleared");
    assert_eq!(g.refs(WINDOW), 0, "both references dropped");
    assert_eq!(s.is_tutorial, 1, "IsTutorial is left alone");
    // The spec's "a message 0x26C63343 posted afterwards does nothing" is
    // the unsubscribe above; the handler is also idle (end-pending was
    // cleared by startTutorial and Shutdown does not touch it).
    assert!(t.do_message(msg::PRE_SAVE, 0));
    assert_eq!(g.take_log(), Vec::<String>::new());
    assert_eq!(t.snapshot().is_tutorial, 1);
    assert_eq!(t.snapshot().window, None);
}

#[test]
fn city_init_complete_reopens_once() {
    let g = Game::new();
    // Not a tutorial: nothing, but the session flag is set.
    let t = system(&g);
    assert!(t.do_message(msg::CITY_INIT_COMPLETE, 0));
    assert!(t.snapshot().start_tried);
    assert!(g.take_log().is_empty());
    // A tutorial whose entry is enabled: the window opens, once.
    let t = system(&g);
    assert!(read(&t, &[0x02, 0x00, 0x01, 0, 0, 0, 0, 0xA3, 0xD3, 0x02, 0xCC]));
    g.registry.borrow_mut().insert(0xCC02_D3A3, true);
    t.do_message(msg::CITY_INIT_COMPLETE, 0);
    assert_eq!(t.snapshot().window, Some(WINDOW));
    assert_eq!(g.take_log().len(), 3);
    t.end_tutorial(true);
    t.with_state(|s| s.is_tutorial = 1);
    g.take_log();
    t.do_message(msg::CITY_INIT_COMPLETE, 0);
    assert!(g.take_log().is_empty());
    // Quirk 2: a disabled entry keeps is-tutorial without a window.
    let t = system(&g);
    assert!(read(&t, &[0x02, 0x00, 0x01, 0, 0, 0, 0, 0x1B, 0xB7, 0xFD, 0x0B]));
    g.registry.borrow_mut().insert(0x0BFD_B71B, false);
    t.do_message(msg::CITY_INIT_COMPLETE, 0);
    assert_eq!(t.snapshot().window, None);
    assert_eq!(t.is_tutorial(), 1);
    // An absent entry: the same.
    let t = system(&g);
    assert!(read(&t, &[0x02, 0x00, 0x01, 0, 0, 0, 0, 0x99, 0, 0, 0]));
    t.do_message(msg::CITY_INIT_COMPLETE, 0);
    assert_eq!(t.snapshot().window, None);
}

#[test]
fn end_tutorial_message() {
    let g = Game::new();
    let t = system(&g);
    // Without a window: nothing.
    t.with_state(|s| s.is_tutorial = 1);
    assert!(t.do_message(msg::END_TUTORIAL, 0));
    assert_eq!(t.is_tutorial(), 1);
    t.start_tutorial();
    g.take_log();
    // Data 0 ends the registry entry too.
    assert!(t.do_message(msg::END_TUTORIAL, 0));
    assert_eq!(t.is_tutorial(), 0);
    assert!(!t.snapshot().end_pending);
    assert_eq!(
        g.take_log(),
        vec!["close_window 40", "set_active FFFFFFFF false", "view_restore"]
    );
    // Non-zero data leaves it pending.
    t.start_tutorial();
    t.do_message(msg::END_TUTORIAL, 7);
    assert!(t.snapshot().end_pending);
    // Other messages are true and do nothing.
    g.take_log();
    assert!(t.do_message(0x1234, 0));
    assert!(g.take_log().is_empty());
}

#[test]
fn start_keeps_an_existing_window() {
    let g = Game::new();
    let t = system(&g);
    g.registry_al.set(false);
    assert!(!t.start_tutorial());
    g.registry_al.set(true);
    assert!(t.start_tutorial());
    assert_eq!(g.refs(WINDOW), 2, "the two references startTutorial took (TU5)");
    assert_eq!(
        g.take_log(),
        vec![
            "create_window",
            "init_window 40",
            "set_active FFFFFFFF true",
            "set_active FFFFFFFF true"
        ]
    );
}

#[test]
fn ui_slots_without_and_with_a_window() {
    let g = Game::new();
    let t = system(&g);
    let fm = |id, kind| FilterMsg {
        kind,
        id,
        field2: 22,
        field3: 33,
    };
    // Without a window.
    assert!(t.is_edge_scrolling_allowed());
    assert_eq!(t.current_task(), None);
    assert!(t.filter_message(&fm(0x2872_59F6, 3)));
    t.show_ui_hints();
    assert!(t.handle_catalog_menu(0x77));
    let (mut x, mut z) = (1, 2);
    assert!(t.filter_left_mouse_click(&mut x, &mut z, 5));
    assert_eq!((x, z), (1, 2));
    assert!(!t.reset_num_and_index(9, 1));
    assert!(t.on_tutorial_event(4));
    assert!(g.take_log().is_empty());
    assert_eq!(t.scripting_context(), Some(LUA));
    assert_eq!(t.scripting_context(), Some(LUA));
    assert_eq!(g.take_log(), vec!["fetch_lua"]);
    assert_eq!(g.refs(LUA), 1);
    // With a window.
    t.start_tutorial();
    g.take_log();
    assert!(t.is_edge_scrolling_allowed());
    g.task.set(Some(TASK));
    g.task_edge.set(false);
    assert!(!t.is_edge_scrolling_allowed());
    assert_eq!(t.current_task(), Some(TASK));
    assert!(t.filter_message(&fm(0x2872_59F6, 3)));
    assert!(t.filter_message(&fm(0x2872_59F7, 3)));
    assert!(t.filter_message(&fm(0x49D9_93D4, 3)));
    assert!(t.filter_message(&fm(0x49D9_93D4, 2)));
    assert!(t.filter_message(&fm(0x1111_1111, 3)));
    t.show_ui_hints();
    assert!(t.handle_catalog_menu(0x77));
    g.click.set(false);
    assert!(!t.filter_left_mouse_click(&mut x, &mut z, 5));
    assert_eq!((x, z), (2, 7));
    g.reset_answer.set(12);
    assert!(t.reset_num_and_index(-5, 1));
    assert_eq!((t.guid(), t.task_number()), (0xFFFF_FFFB, 12));
    assert!(t.on_tutorial_event(4));
    assert_eq!(
        g.take_log(),
        vec![
            "filter 22",
            "filter 22",
            "filter 33",
            "hints",
            "menu 77",
            "reset -5 1",
            "event 4"
        ]
    );
}

#[test]
fn guid_mapping_function() {
    assert_eq!(map_tutorial_guid(0), registry::GUID_0);
    assert_eq!(map_tutorial_guid(1), registry::GUID_1);
    assert_eq!(map_tutorial_guid(NO_GUID), NO_GUID);
}
