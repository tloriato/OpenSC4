// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Conformance ops for `docs/specs/tutorial.md`: the tutorial system over a
//! scripted registry, window and 3D view. Every op name starts with `tut_`.
//! See README.md ("Tutorial ops").

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use fc4_core::stream::{ByteReader, ByteWriter, SaveObject};
use fc4_core::tutorial::{msg, FilterMsg, TutorialHost, TutorialSystem};

use crate::foundations::{hex_decode, hex_encode};
use crate::json::Json;
use crate::runner::{field, HostRef};
use crate::scalar::{self, arg, bool_of, check_expect, check_state, i64_of, ji, opt, str_of, u32_of, EventLog, R};

/// The window's token (the first and every later one).
const WINDOW: u64 = 0x40;
const LUA: u64 = 0x30;

#[derive(Default)]
struct World {
    events: EventLog,
    refs: RefCell<BTreeMap<u64, i64>>,
    /// GUID → the entry's byte +0x10.
    registry: RefCell<BTreeMap<u32, bool>>,
    /// GUID → the last `SetActive`.
    active: RefCell<BTreeMap<u32, bool>>,
    cheats: Cell<bool>,
    task: Cell<Option<u64>>,
    task_edge: Cell<bool>,
    registry_al: Cell<bool>,
    sys: RefCell<Option<Rc<TutorialSystem>>>,
}

impl World {
    fn new() -> Rc<World> {
        let w = Rc::new(World::default());
        w.task_edge.set(true);
        w
    }
    fn bump(&self, t: u64, d: i64) {
        *self.refs.borrow_mut().entry(t).or_insert(0) += d;
    }
    fn sys(&self) -> R<Rc<TutorialSystem>> {
        self.sys
            .borrow()
            .clone()
            .ok_or_else(|| "no tutorial system (tut_create)".into())
    }
}

fn ju(v: u32) -> Json {
    ji(i64::from(v))
}

impl TutorialHost for World {
    fn subscribe(&self, m: u32) {
        self.events.push("subscribe", vec![("type", ju(m))]);
    }
    fn unsubscribe(&self, m: u32) {
        self.events.push("unsubscribe", vec![("type", ju(m))]);
    }
    fn cheats_available(&self) -> bool {
        self.cheats.get()
    }
    fn has_cheat_manager(&self) -> bool {
        self.cheats.get()
    }
    fn register_cheat(&self) {
        self.events.push("register_cheat", vec![]);
    }
    fn unregister_cheat(&self) {
        self.events.push("unregister_cheat", vec![]);
    }
    fn add_ref(&self, t: u64) {
        self.bump(t, 1);
    }
    fn release(&self, t: u64) {
        self.bump(t, -1);
    }
    fn create_window(&self) -> u64 {
        self.events.push("create_window", vec![]);
        // TU9: the native constructor leaves count zero.
        WINDOW
    }
    fn init_window(&self, _w: u64) {
        self.events.push("init_window", vec![]);
    }
    fn close_window(&self, _w: u64) {
        self.events.push("close_window", vec![]);
    }
    fn window_current_task(&self, _w: u64) -> Option<u64> {
        self.task.get()
    }
    fn task_edge_scrolling_allowed(&self, _t: u64) -> bool {
        self.task_edge.get()
    }
    fn window_filter_value(&self, _w: u64, v: u32) {
        self.events.push("window_filter", vec![("value", ju(v))]);
    }
    fn window_show_hints(&self, _w: u64) {
        self.events.push("window_hints", vec![]);
    }
    fn window_catalog_menu(&self, _w: u64, menu: u64) {
        self.events.push("window_menu", vec![("menu", ji(menu as i64))]);
    }
    fn window_left_click(&self, _w: u64, _x: &mut i32, _z: &mut i32, _n: i32) -> bool {
        false
    }
    fn window_reset(&self, _w: u64, a: i32, b: i32) -> i32 {
        self.events
            .push("window_reset", vec![("a", ji(i64::from(a))), ("b", ji(i64::from(b)))]);
        b
    }
    fn window_event(&self, _w: u64, e: u32) {
        self.events.push("window_event", vec![("event", ju(e))]);
    }
    fn registry_set_active(&self, guid: u32, active: bool) -> bool {
        self.events
            .push("set_active", vec![("guid", ju(guid)), ("active", Json::Bool(active))]);
        self.active.borrow_mut().insert(guid, active);
        self.registry_al.get()
    }
    fn registry_entry_enabled(&self, guid: u32) -> Option<bool> {
        self.registry.borrow().get(&guid).copied()
    }
    fn view_restore_controls(&self) {
        self.events.push("view_restore", vec![]);
    }
    fn fetch_lua_context(&self) -> Option<u64> {
        self.bump(LUA, 1);
        Some(LUA)
    }
}

fn world(host: &HostRef) -> Rc<World> {
    scalar::world(host, "tutorial", World::new)
}

fn message_type(j: &Json) -> R<u32> {
    match j.as_str() {
        Some("CityInitComplete") => Ok(msg::CITY_INIT_COMPLETE),
        Some("EndTutorial") => Ok(msg::END_TUTORIAL),
        Some("PreSave") => Ok(msg::PRE_SAVE),
        _ => u32_of(j, "type"),
    }
}

fn call(w: &World, op: &Json) -> R<()> {
    let t = w.sys()?;
    let method = str_of(field(op, "method")?, "method")?;
    let a_i64 = |i| arg(op, i).and_then(|v| i64_of(v, "argument"));
    let got = match method {
        "init" => Json::Bool(t.init()),
        "shutdown" => Json::Bool(t.shutdown()),
        "start_tutorial" => Json::Bool(t.start_tutorial()),
        "end_tutorial" => Json::Bool(t.end_tutorial(bool_of(arg(op, 0)?, "argument")?)),
        "is_tutorial" => ju(t.is_tutorial()),
        "task_number" => ji(i64::from(t.task_number())),
        "guid" => ju(t.guid()),
        "set_task_number" => {
            t.set_task_number(a_i64(0)? as i32);
            Json::Null
        }
        "increment_task_number" => {
            t.increment_task_number();
            Json::Null
        }
        "is_edge_scrolling_allowed" => Json::Bool(t.is_edge_scrolling_allowed()),
        "current_task" => t.current_task().map_or(Json::Null, |v| ji(v as i64)),
        "scripting_context" => t.scripting_context().map_or(Json::Null, |v| ji(v as i64)),
        "filter_message" => Json::Bool(t.filter_message(&FilterMsg {
            kind: a_i64(0)? as u32,
            id: a_i64(1)? as u32,
            field2: a_i64(2)? as u32,
            field3: a_i64(3)? as u32,
        })),
        "show_ui_hints" => {
            t.show_ui_hints();
            Json::Null
        }
        "handle_catalog_menu" => Json::Bool(t.handle_catalog_menu(a_i64(0)? as u64)),
        "filter_left_mouse_click" => {
            let (mut x, mut z) = (a_i64(0)? as i32, a_i64(1)? as i32);
            Json::Bool(t.filter_left_mouse_click(&mut x, &mut z, a_i64(2)? as i32))
        }
        "reset_num_and_index" => Json::Bool(t.reset_num_and_index(a_i64(0)? as i32, a_i64(1)? as i32)),
        "on_tutorial_event" => Json::Bool(t.on_tutorial_event(a_i64(0)? as u32)),
        other => return Err(format!("unknown tutorial method '{other}'")),
    };
    check_expect(op, got)
}

fn expect(w: &World, op: &Json) -> R<()> {
    let s = w.sys()?.snapshot();
    let active: Vec<(String, Json)> = w
        .active
        .borrow()
        .iter()
        .map(|(g, a)| (format!("0x{g:08X}"), Json::Bool(*a)))
        .collect();
    let got = vec![
        ("is_tutorial".to_owned(), ju(u32::from(s.is_tutorial))),
        ("task_number".to_owned(), ji(i64::from(s.task_number))),
        ("guid".to_owned(), ju(s.guid)),
        ("field_28".to_owned(), ju(s.field_28)),
        ("end_pending".to_owned(), Json::Bool(s.end_pending)),
        ("start_tried".to_owned(), Json::Bool(s.start_tried)),
        ("window".to_owned(), Json::Bool(s.window.is_some())),
        (
            "window_refs".to_owned(),
            ji(w.refs.borrow().get(&WINDOW).copied().unwrap_or(0)),
        ),
        ("registry_active".to_owned(), Json::Object(active)),
    ];
    check_state(&got, field(op, "state")?)
}

pub(crate) fn exec(host: &HostRef, name: &str, op: &Json) -> Option<R<()>> {
    let rest = name.strip_prefix("tut_")?;
    let w = world(host);
    Some(match rest {
        "env" => (|| {
            if let Some(v) = opt(op, "cheats") {
                w.cheats.set(bool_of(v, "cheats")?);
            }
            if let Some(v) = opt(op, "registry_result") {
                w.registry_al.set(bool_of(v, "registry_result")?);
            }
            if let Some(r) = opt(op, "registry") {
                for (k, v) in r.as_object().ok_or("'registry' must be an object")? {
                    let g = Json::Str(k.clone()).as_i64().ok_or("bad GUID")? as u32;
                    w.registry.borrow_mut().insert(g, bool_of(v, "registry")?);
                }
            }
            if let Some(v) = op.get("task") {
                w.task.set(match v {
                    Json::Null => None,
                    v => Some(i64_of(v, "task")? as u64),
                });
            }
            if let Some(v) = opt(op, "task_edge") {
                w.task_edge.set(bool_of(v, "task_edge")?);
            }
            Ok(())
        })(),
        "create" => {
            *w.sys.borrow_mut() = Some(TutorialSystem::new(w.clone(), 0x1000));
            Ok(())
        }
        "read" => (|| {
            let bytes = hex_decode(str_of(field(op, "hex")?, "hex")?)?;
            let ok = w.sys()?.read(&mut ByteReader::new(&bytes));
            check_expect(op, Json::Bool(ok))
        })(),
        "record" => (|| {
            let mut out = ByteWriter::new();
            if !w.sys()?.write(&mut out) {
                return Err("Write returned false".into());
            }
            let got = out.into_bytes();
            let want = hex_decode(str_of(field(op, "hex")?, "hex")?)?;
            if got != want {
                return Err(format!("record {}, expected {}", hex_encode(&got), hex_encode(&want)));
            }
            Ok(())
        })(),
        "message" => (|| {
            let ty = message_type(field(op, "type")?)?;
            let data = match opt(op, "data") {
                Some(d) => u32_of(d, "data")?,
                None => 0,
            };
            check_expect(op, Json::Bool(w.sys()?.do_message(ty, data)))
        })(),
        "call" => call(&w, op),
        "expect" => expect(&w, op),
        "expect_events" => w.events.expect_events(op),
        "expect_event_count" => w.events.expect_count(op),
        "clear_events" => {
            w.events.clear();
            Ok(())
        }
        _ => Err(format!("unknown op '{name}'")),
    })
}
