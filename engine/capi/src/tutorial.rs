// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The tutorial system over the C ABI (fc4.h "tutorial system",
//! docs/specs/tutorial.md).
//!
//! The host implements [`fc4_tutorial_host`]; the plugin's
//! `cISC4TutorialSystem` (with its message-target, cheat-handler and
//! serializable sub-objects) forwards its slots here. The tutorial window,
//! its tasks, the registry, the 3D view, the cheat manager and the Lua
//! context stay host objects, named by tokens (0 = none).
//!
//! Threads: the core is single-threaded (`RefCell`). The plugin serialises
//! every call on one object (fc4.h).

use std::ffi::c_void;
use std::rc::Rc;

use fc4_core::stream::SaveObject;
use fc4_core::tutorial::{query_interface, FilterMsg, TutorialHost, TutorialSystem};

use crate::guard;
use crate::stream::{fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};

type U = *mut c_void;

#[repr(C)]
pub struct fc4_tutorial {
    _private: [u8; 0],
}

/// What the tutorial system needs from the game (fc4.h
/// `fc4_tutorial_host`). A missing callback reads as 0 / false / none.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_tutorial_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    // lifecycle (§4.3)
    pub subscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub unsubscribe: Option<unsafe extern "C" fn(U, u32)>,
    pub cheats_available: Option<unsafe extern "C" fn(U) -> bool>,
    pub has_cheat_manager: Option<unsafe extern "C" fn(U) -> bool>,
    pub register_cheat: Option<unsafe extern "C" fn(U)>,
    pub unregister_cheat: Option<unsafe extern "C" fn(U)>,
    // references
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    // the window (§4.2, §4.4)
    pub create_window: Option<unsafe extern "C" fn(U) -> u64>,
    pub init_window: Option<unsafe extern "C" fn(U, u64)>,
    pub close_window: Option<unsafe extern "C" fn(U, u64)>,
    pub window_current_task: Option<unsafe extern "C" fn(U, u64) -> u64>,
    pub task_edge_scrolling_allowed: Option<unsafe extern "C" fn(U, u64) -> bool>,
    pub window_filter_value: Option<unsafe extern "C" fn(U, u64, u32)>,
    pub window_show_hints: Option<unsafe extern "C" fn(U, u64)>,
    pub window_catalog_menu: Option<unsafe extern "C" fn(U, u64, u64)>,
    pub window_left_click: Option<unsafe extern "C" fn(U, u64, *mut i32, *mut i32, i32) -> bool>,
    pub window_reset: Option<unsafe extern "C" fn(U, u64, i32, i32) -> i32>,
    pub window_event: Option<unsafe extern "C" fn(U, u64, u32)>,
    // the registry (§4.2, §4.3)
    pub registry_set_active: Option<unsafe extern "C" fn(U, u32, bool) -> bool>,
    pub registry_entry_enabled: Option<unsafe extern "C" fn(U, u32, *mut bool) -> bool>,
    // the rest
    pub view_restore_controls: Option<unsafe extern "C" fn(U)>,
    pub fetch_lua_context: Option<unsafe extern "C" fn(U) -> u64>,
}

/// Every field of §2 (`fc4_tutorial_get_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_tutorial_state {
    pub struct_size: u32,
    /// +0x1F (saved): the byte as read.
    pub is_tutorial: u8,
    pub destroyed: u8,
    /// +0x2C.
    pub end_pending: u8,
    /// +0x2D.
    pub start_tried: u8,
    pub ref_count: u32,
    /// +0x20 (saved).
    pub task_number: i32,
    /// +0x24 (saved; 0xFFFFFFFF = none).
    pub guid: u32,
    /// +0x28 (3).
    pub field_28: u32,
    /// +0x30 (0 = none; one reference held).
    pub lua_context: u64,
    /// +0x40 (0 = none; one reference held).
    pub window: u64,
}

const _: () = {
    assert!(std::mem::size_of::<fc4_tutorial_state>() == 40);
    assert!(std::mem::size_of::<fc4_tutorial_host>() == if cfg!(target_pointer_width = "32") { 100 } else { 200 });
};

// ---- host adapter -----------------------------------------------------------------------------------

struct CHost {
    h: fc4_tutorial_host,
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

impl TutorialHost for CHost {
    fn subscribe(&self, msg_type: u32) {
        cb!(self, subscribe, (), msg_type)
    }
    fn unsubscribe(&self, msg_type: u32) {
        cb!(self, unsubscribe, (), msg_type)
    }
    fn cheats_available(&self) -> bool {
        cb!(self, cheats_available, false)
    }
    fn has_cheat_manager(&self) -> bool {
        cb!(self, has_cheat_manager, false)
    }
    fn register_cheat(&self) {
        cb!(self, register_cheat, ())
    }
    fn unregister_cheat(&self) {
        cb!(self, unregister_cheat, ())
    }
    fn add_ref(&self, token: u64) {
        cb!(self, add_ref, (), token)
    }
    fn release(&self, token: u64) {
        cb!(self, release, (), token)
    }
    fn create_window(&self) -> u64 {
        cb!(self, create_window, 0)
    }
    fn init_window(&self, window: u64) {
        cb!(self, init_window, (), window)
    }
    fn close_window(&self, window: u64) {
        cb!(self, close_window, (), window)
    }
    fn window_current_task(&self, window: u64) -> Option<u64> {
        opt(cb!(self, window_current_task, 0, window))
    }
    fn task_edge_scrolling_allowed(&self, task: u64) -> bool {
        cb!(self, task_edge_scrolling_allowed, false, task)
    }
    fn window_filter_value(&self, window: u64, value: u32) {
        cb!(self, window_filter_value, (), window, value)
    }
    fn window_show_hints(&self, window: u64) {
        cb!(self, window_show_hints, (), window)
    }
    fn window_catalog_menu(&self, window: u64, menu: u64) {
        cb!(self, window_catalog_menu, (), window, menu)
    }
    fn window_left_click(&self, window: u64, x: &mut i32, z: &mut i32, n: i32) -> bool {
        cb!(self, window_left_click, false, window, x as *mut i32, z as *mut i32, n)
    }
    fn window_reset(&self, window: u64, a: i32, b: i32) -> i32 {
        cb!(self, window_reset, 0, window, a, b)
    }
    fn window_event(&self, window: u64, event: u32) {
        cb!(self, window_event, (), window, event)
    }
    fn registry_set_active(&self, guid: u32, active: bool) -> bool {
        cb!(self, registry_set_active, false, guid, active)
    }
    fn registry_entry_enabled(&self, guid: u32) -> Option<bool> {
        let mut v = false;
        cb!(self, registry_entry_enabled, false, guid, &mut v).then_some(v)
    }
    fn view_restore_controls(&self) {
        cb!(self, view_restore_controls, ())
    }
    fn fetch_lua_context(&self) -> Option<u64> {
        opt(cb!(self, fetch_lua_context, 0))
    }
}

// ---- handles ----------------------------------------------------------------------------------------

struct Handle {
    sys: Rc<TutorialSystem>,
}

fn with_t<R>(t: *const fc4_tutorial, fallback: R, f: impl FnOnce(&Handle) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_tutorial*.
    match unsafe { (t as *const Handle).as_ref() } {
        Some(h) => guard(fallback, || f(h)),
        None => fallback,
    }
}

/// Creates the core object (constructed, §2: GUID none, reference count 0).
/// The host table is copied. `token` is the object's own host identity.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_create(host: *const fc4_tutorial_host, token: u64) -> *mut fc4_tutorial {
    guard(std::ptr::null_mut(), || match host.as_ref() {
        Some(h) if (h.struct_size as usize) >= std::mem::size_of::<fc4_tutorial_host>() => {
            let sys = TutorialSystem::new(Rc::new(CHost { h: *h }), token);
            Box::into_raw(Box::new(Handle { sys })) as *mut fc4_tutorial
        }
        _ => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_destroy(t: *mut fc4_tutorial) {
    if !t.is_null() {
        guard((), || drop(Box::from_raw(t as *mut Handle)));
    }
}

/// `QueryInterface` (§1): the four named IIDs and `1`.
#[no_mangle]
pub extern "C" fn fc4_tutorial_query_interface(iid: u32) -> bool {
    query_interface(iid)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_add_ref(t: *mut fc4_tutorial) -> u32 {
    with_t(t, 0, |h| h.sys.add_ref())
}

/// `Release`: 0 when the last reference went (the object is marked
/// destroyed, TU3); the handle stays valid until [`fc4_tutorial_destroy`].
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_release(t: *mut fc4_tutorial) -> u32 {
    with_t(t, 0, |h| h.sys.release())
}

/// `Init` (§4.3): subscribes, registers the cheat when cheats are available.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_init(t: *mut fc4_tutorial) -> bool {
    with_t(t, false, |h| h.sys.init())
}

/// `Shutdown` (§4.3, TU4).
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_shutdown(t: *mut fc4_tutorial) -> bool {
    with_t(t, false, |h| h.sys.shutdown())
}

/// `DoMessage` (§4.3): city init complete, `Tutorial_EndTutorial` (`data` =
/// the message's data field; 0 ends the registry entry) and pre-save.
/// Always true.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_do_message(t: *mut fc4_tutorial, msg_type: u32, data: u32) -> bool {
    with_t(t, true, |h| h.sys.do_message(msg_type, data))
}

/// Slot 5 `startTutorial` (§4.2): the registry call's result.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_start(t: *mut fc4_tutorial) -> bool {
    with_t(t, false, |h| h.sys.start_tutorial())
}

/// Slot 6 `endTutorial(end_registry)` (§4.2): returns `end_registry`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_end(t: *mut fc4_tutorial, end_registry: bool) -> bool {
    with_t(t, false, |h| h.sys.end_tutorial(end_registry))
}

/// Slot 7 `GetScriptingContext`: the Lua context token (fetched once and
/// kept with a reference), 0 when unavailable.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_scripting_context(t: *mut fc4_tutorial) -> u64 {
    with_t(t, 0, |h| h.sys.scripting_context().unwrap_or(0))
}

/// Slot 8 `IsTutorial` (§4.1): 1 or 0.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_is_tutorial(t: *const fc4_tutorial) -> u32 {
    with_t(t, 0, |h| h.sys.is_tutorial())
}

/// Slot 9 `IsEdgeScrollingAllowed`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_is_edge_scrolling_allowed(t: *const fc4_tutorial) -> bool {
    with_t(t, true, |h| h.sys.is_edge_scrolling_allowed())
}

/// Slot 10 `GetCurrentTask`: the task token, 0 for none.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_current_task(t: *const fc4_tutorial) -> u64 {
    with_t(t, 0, |h| h.sys.current_task().unwrap_or(0))
}

/// Slot 11 `GetTutorialTaskNumber`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_task_number(t: *const fc4_tutorial) -> i32 {
    with_t(t, 0, |h| h.sys.task_number())
}

/// Slot 12 `GetTutorialGUID`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_guid(t: *const fc4_tutorial) -> u32 {
    with_t(t, 0xFFFF_FFFF, |h| h.sys.guid())
}

/// Slot 13 `FilterMessage`: the message's kind (`+0x10`), ID and data fields
/// 2 and 3. Always true.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_filter_message(
    t: *mut fc4_tutorial,
    kind: u32,
    id: u32,
    field2: u32,
    field3: u32,
) -> bool {
    let m = FilterMsg {
        kind,
        id,
        field2,
        field3,
    };
    with_t(t, true, |h| h.sys.filter_message(&m))
}

/// Slot 14 `ShowUIHintsForUsers`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_show_ui_hints(t: *mut fc4_tutorial) {
    with_t(t, (), |h| h.sys.show_ui_hints())
}

/// Slot 15 `HandleCatalogMenu(menu)`. Always true.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_handle_catalog_menu(t: *mut fc4_tutorial, menu: u64) -> bool {
    with_t(t, true, |h| h.sys.handle_catalog_menu(menu))
}

/// Slot 16 `FilterLeftMouseClick(x, z, n)`: `x` and `z` are in/out (NULL
/// reads as 0 and is not written back). True without a window.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_filter_left_mouse_click(
    t: *mut fc4_tutorial,
    x: *mut i32,
    z: *mut i32,
    n: i32,
) -> bool {
    let mut xv = x.as_ref().copied().unwrap_or(0);
    let mut zv = z.as_ref().copied().unwrap_or(0);
    let r = with_t(t, true, |h| h.sys.filter_left_mouse_click(&mut xv, &mut zv, n));
    if let Some(p) = x.as_mut() {
        *p = xv;
    }
    if let Some(p) = z.as_mut() {
        *p = zv;
    }
    r
}

/// Slot 17 `ResetTutorialNumAndIndex(a, b)`: false without a window.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_reset_num_and_index(t: *mut fc4_tutorial, a: i32, b: i32) -> bool {
    with_t(t, false, |h| h.sys.reset_num_and_index(a, b))
}

/// Slot 18 `OnTutorialEvent(event)`. Always true.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_on_event(t: *mut fc4_tutorial, event: u32) -> bool {
    with_t(t, true, |h| h.sys.on_tutorial_event(event))
}

/// Slot 19 `SetTutorialTaskNumber(n)`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_set_task_number(t: *mut fc4_tutorial, n: i32) {
    with_t(t, (), |h| h.sys.set_task_number(n))
}

/// Slot 20 `IncrementTutorialTaskNumber`.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_increment_task_number(t: *mut fc4_tutorial) {
    with_t(t, (), |h| h.sys.increment_task_number())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_get_state(t: *const fc4_tutorial, out: *mut fc4_tutorial_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    with_t(t, false, |h| {
        let s = h.sys.snapshot();
        *o = fc4_tutorial_state {
            struct_size: std::mem::size_of::<fc4_tutorial_state>() as u32,
            is_tutorial: s.is_tutorial,
            destroyed: s.destroyed.into(),
            end_pending: s.end_pending.into(),
            start_tried: s.start_tried.into(),
            ref_count: s.ref_count,
            task_number: s.task_number,
            guid: s.guid,
            field_28: s.field_28,
            lua_context: s.lua_context.unwrap_or(0),
            window: s.window.unwrap_or(0),
        };
        true
    })
}

/// `Write` (§3): the 11-byte component 48 record. False (nothing written)
/// when the stream already reports an error.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_write(t: *const fc4_tutorial, out: *const fc4_ostream) -> bool {
    let Some(o) = out.as_ref() else { return false };
    if !ostream_ok(o) {
        return false;
    }
    with_t(t, false, |h| h.sys.write(&mut COut(o)))
}

fn no_core_objects(_: u32, _: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    None
}

/// TU7 `Tutorial save n`: publish all four fields before metadata/save callbacks.
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_prepare_city_save(t: *mut fc4_tutorial, guid: u32) {
    with_t(t, (), |h| {
        h.sys.with_state(|s| {
            s.is_tutorial = 1;
            s.guid = guid;
            s.task_number = 0;
            s.end_pending = false;
        });
    });
}

/// `Read` (§3): false unless the version is 2; the GUID goes through the
/// registry's mapping (0 → 0x8A5B7A6C, 1 → 0xEA5D6DC8).
#[no_mangle]
pub unsafe extern "C" fn fc4_tutorial_read(t: *mut fc4_tutorial, inp: *const fc4_istream) -> bool {
    let Some(i) = inp.as_ref() else { return false };
    if !istream_ok(i) {
        return false;
    }
    with_t(t, false, |h| {
        h.sys.read(&mut CIn {
            stream: i,
            decode: no_core_objects,
        })
    })
}
