// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The tutorial system component (§1–§4).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::stream::{InStream, OutStream, SaveObject};

use super::{
    filter, map_tutorial_guid, msg, FilterMsg, TutorialHost, CLSID_TUTORIAL_SYSTEM, FIELD_28_DEFAULT,
    IID_CHEAT_HANDLER, IID_MESSAGE_TARGET2, IID_SERIALIZABLE, IID_TUTORIAL_SYSTEM, IID_UNKNOWN, NO_GUID,
    RECORD_VERSION,
};

/// `QueryInterface` (§1): `0xAA3836F6`, `cIGZMessageTarget2`, the cheat
/// handler, `cIGZSerializable`, and `1` through the base lookup
/// [inferred].
pub fn query_interface(iid: u32) -> bool {
    matches!(
        iid,
        IID_TUTORIAL_SYSTEM | IID_MESSAGE_TARGET2 | IID_CHEAT_HANDLER | IID_SERIALIZABLE | IID_UNKNOWN
    )
}

/// The window teardown of `endTutorial` and `Shutdown` (§4.2, TU5): close
/// it, release it, then clear +0x40 and release it again — the two
/// references `startTutorial` took.
fn drop_window(s: &RefCell<TutorialState>, host: &dyn TutorialHost) {
    let Some(w) = s.borrow_mut().window.take() else {
        return;
    };
    host.close_window(w);
    host.release(w);
    host.release(w);
}

/// Every field of §2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TutorialState {
    /// +0x14.
    pub ref_count: u32,
    /// The last `Release` destroyed the object (TU3).
    pub destroyed: bool,
    /// +0x1F (saved): the byte as read; any non-zero value is "in a
    /// tutorial".
    pub is_tutorial: u8,
    /// +0x20 (saved).
    pub task_number: i32,
    /// +0x24 (saved; [`NO_GUID`] = none).
    pub guid: u32,
    /// +0x28: 3, not read by this class.
    pub field_28: u32,
    /// +0x2C: "registry end pending".
    pub end_pending: bool,
    /// +0x2D: "start already tried this session".
    pub start_tried: bool,
    /// +0x30: the shared Lua context (one reference held).
    pub lua_context: Option<u64>,
    /// +0x40: the tutorial window (one reference held).
    pub window: Option<u64>,
}

impl Default for TutorialState {
    /// The constructor (`0x0043F5A0`): zero except the GUID and +0x28.
    fn default() -> Self {
        TutorialState {
            ref_count: 0,
            destroyed: false,
            is_tutorial: 0,
            task_number: 0,
            guid: NO_GUID,
            field_28: FIELD_28_DEFAULT,
            end_pending: false,
            start_tried: false,
            lua_context: None,
            window: None,
        }
    }
}

/// The tutorial system.
pub struct TutorialSystem {
    host: Rc<dyn TutorialHost>,
    token: Cell<u64>,
    s: RefCell<TutorialState>,
}

impl TutorialSystem {
    /// A constructed system (§1). `token` is its own host identity.
    pub fn new(host: Rc<dyn TutorialHost>, token: u64) -> Rc<TutorialSystem> {
        Rc::new(TutorialSystem {
            host,
            token: Cell::new(token),
            s: RefCell::new(TutorialState::default()),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn snapshot(&self) -> TutorialState {
        self.s.borrow().clone()
    }

    pub fn with_state<R>(&self, f: impl FnOnce(&mut TutorialState) -> R) -> R {
        f(&mut self.s.borrow_mut())
    }

    // ---- identity (§1) ----

    pub fn add_ref(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        s.ref_count = s.ref_count.wrapping_add(1);
        s.ref_count
    }

    /// The advisor system's shared `Release`: above 1 it decrements; the
    /// last one destroys the object (here: marks it) and returns 0
    /// [inferred] (TU3).
    pub fn release(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        if s.ref_count > 1 {
            s.ref_count -= 1;
            return s.ref_count;
        }
        s.ref_count = 0;
        s.destroyed = true;
        0
    }

    // ---- lifecycle (§4.3) ----

    /// `Init` (slot 3): subscribes to the three messages and, when cheats
    /// are available, registers the cheat handler and the "Tutorial" cheat.
    /// No guard (quirk 4). True.
    pub fn init(&self) -> bool {
        for m in msg::SUBSCRIPTIONS {
            self.host.subscribe(m);
        }
        if self.host.cheats_available() {
            self.host.register_cheat();
        }
        true
    }

    /// `Shutdown` (slot 4): the teardown of §4.3, in the spec's order (TU5).
    /// True.
    ///
    /// 1. The cheat manager, **whether or not `Init` registered anything**
    ///    (the `cISC4App` slot 21 test is not repeated): [`TutorialHost::unregister_cheat`].
    /// 2. The window, when there is one: close it, release it, clear +0x40
    ///    and release it again — the two references `startTutorial` took.
    /// 3. Unsubscribe `0xAA3840E6`, `0xEA8AE29A` and `0x26C63343`, in that
    ///    order.
    ///
    /// Is-tutorial, end-pending, the task number, the GUID and the registry
    /// are not touched.
    pub fn shutdown(&self) -> bool {
        // [TU5] Step 1: the cheat manager test only — `Init` gated its
        // registration on `cISC4App` slot 21 as well, and that test is not
        // repeated here.
        if self.host.has_cheat_manager() {
            self.host.unregister_cheat();
        }
        // [TU5] Step 2: the same window teardown as `endTutorial`, without
        // its registry, 3D view and flag steps. Both references
        // `startTutorial` took are dropped.
        drop_window(&self.s, &*self.host);
        // [TU5] Step 3: the unsubscribe order is not `SUBSCRIPTIONS`'s.
        for m in msg::UNSUBSCRIPTIONS {
            self.host.unsubscribe(m);
        }
        true
    }

    // ---- starting and ending (§4.2) ----

    /// `startTutorial` (slot 5): opens the window when there is none, sets
    /// is-tutorial, clears end-pending and activates the registry entry.
    /// The result is the registry call's (quirk 3).
    pub fn start_tutorial(&self) -> bool {
        if self.s.borrow().window.is_none() {
            let w = self.host.create_window();
            // §4.2.2: Init calls the owner synchronously. Publish the window
            // before opening it, with no state borrow spanning those calls.
            self.s.borrow_mut().window = Some(w);
            self.host.add_ref(w);
            self.host.init_window(w);
        }
        let guid = {
            let mut s = self.s.borrow_mut();
            s.is_tutorial = 1;
            s.end_pending = false;
            s.guid
        };
        self.host.registry_set_active(guid, true)
    }

    /// `endTutorial(endRegistry)` (slot 6): closes and drops the window's
    /// two references, deactivates the registry entry when `end_registry`,
    /// restores the 3D view's controls, clears is-tutorial and sets
    /// end-pending to `!end_registry`. Returns `end_registry`.
    pub fn end_tutorial(&self, end_registry: bool) -> bool {
        // [TU5] The same window teardown as `Shutdown`, without its cheat
        // and unsubscribe steps.
        drop_window(&self.s, &*self.host);
        if end_registry {
            let guid = self.s.borrow().guid;
            self.host.registry_set_active(guid, false);
        }
        self.host.view_restore_controls();
        let mut s = self.s.borrow_mut();
        s.is_tutorial = 0;
        s.end_pending = !end_registry;
        end_registry
    }

    // ---- messages (§4.3) ----

    /// `DoMessage` (sub-object +4). `data` is the message's data field
    /// (message slot 9; only `Tutorial_EndTutorial` reads it). True.
    pub fn do_message(&self, msg_type: u32, data: u32) -> bool {
        match msg_type {
            msg::CITY_INIT_COMPLETE => {
                let (tried, tutorial, guid) = {
                    let s = self.s.borrow();
                    (s.start_tried, s.is_tutorial, s.guid)
                };
                if !tried && tutorial != 0 && self.host.registry_entry_enabled(guid) == Some(true) {
                    self.start_tutorial();
                }
                self.s.borrow_mut().start_tried = true;
            }
            msg::END_TUTORIAL if self.window().is_some() => {
                self.end_tutorial(data == 0);
            }
            msg::PRE_SAVE if self.s.borrow().end_pending => {
                self.end_tutorial(true);
            }
            _ => {}
        }
        true
    }

    // ---- the simulation's flag (§4.1) ----

    /// `IsTutorial` (slot 8): 1 when is-tutorial is set, else 0.
    pub fn is_tutorial(&self) -> u32 {
        u32::from(self.s.borrow().is_tutorial != 0)
    }

    // ---- the other slots (§4.4) ----

    /// Slot 7 `GetScriptingContext`: fetched once, kept with a reference.
    pub fn scripting_context(&self) -> Option<u64> {
        if let Some(c) = self.s.borrow().lua_context {
            return Some(c);
        }
        let c = self.host.fetch_lua_context();
        self.s.borrow_mut().lua_context = c;
        c
    }

    fn window(&self) -> Option<u64> {
        self.s.borrow().window
    }

    /// Slot 9 `IsEdgeScrollingAllowed`.
    pub fn is_edge_scrolling_allowed(&self) -> bool {
        match self.window() {
            None => true,
            Some(w) => match self.host.window_current_task(w) {
                None => true,
                Some(t) => self.host.task_edge_scrolling_allowed(t),
            },
        }
    }

    /// Slot 10 `GetCurrentTask`.
    pub fn current_task(&self) -> Option<u64> {
        self.window().and_then(|w| self.host.window_current_task(w))
    }

    /// Slot 11 `GetTutorialTaskNumber`.
    pub fn task_number(&self) -> i32 {
        self.s.borrow().task_number
    }

    /// Slot 12 `GetTutorialGUID`.
    pub fn guid(&self) -> u32 {
        self.s.borrow().guid
    }

    /// Slot 13 `FilterMessage`: a kind-3 message with one of the three IDs
    /// passes its data field to the window. True.
    pub fn filter_message(&self, m: &FilterMsg) -> bool {
        if let Some(w) = self.window() {
            if m.kind == filter::KIND {
                if filter::FIELD2_IDS.contains(&m.id) {
                    self.host.window_filter_value(w, m.field2);
                } else if m.id == filter::FIELD3_ID {
                    self.host.window_filter_value(w, m.field3);
                }
            }
        }
        true
    }

    /// Slot 14 `ShowUIHintsForUsers`.
    pub fn show_ui_hints(&self) {
        if let Some(w) = self.window() {
            self.host.window_show_hints(w);
        }
    }

    /// Slot 15 `HandleCatalogMenu`. True.
    pub fn handle_catalog_menu(&self, menu: u64) -> bool {
        if let Some(w) = self.window() {
            self.host.window_catalog_menu(w, menu);
        }
        true
    }

    /// Slot 16 `FilterLeftMouseClick`.
    pub fn filter_left_mouse_click(&self, x: &mut i32, z: &mut i32, n: i32) -> bool {
        match self.window() {
            None => true,
            Some(w) => self.host.window_left_click(w, x, z, n),
        }
    }

    /// Slot 17 `ResetTutorialNumAndIndex(a, b)`: with a window, GUID := `a`
    /// and the task number := the window's answer; true. False without.
    pub fn reset_num_and_index(&self, a: i32, b: i32) -> bool {
        let Some(w) = self.window() else {
            return false;
        };
        self.s.borrow_mut().guid = a as u32;
        let n = self.host.window_reset(w, a, b);
        self.s.borrow_mut().task_number = n;
        true
    }

    /// Slot 18 `OnTutorialEvent`. True.
    pub fn on_tutorial_event(&self, event: u32) -> bool {
        if let Some(w) = self.window() {
            self.host.window_event(w, event);
        }
        true
    }

    /// Slot 19 `SetTutorialTaskNumber`.
    pub fn set_task_number(&self, n: i32) {
        self.s.borrow_mut().task_number = n;
    }

    /// Slot 20 `IncrementTutorialTaskNumber`.
    pub fn increment_task_number(&self) {
        let mut s = self.s.borrow_mut();
        s.task_number = s.task_number.wrapping_add(1);
    }
}

impl SaveObject for TutorialSystem {
    fn class_id(&self) -> u32 {
        CLSID_TUTORIAL_SYSTEM
    }

    /// §3 `Write`: nothing on a stream that already has an error; else
    /// version 2, is-tutorial, the task number and the GUID. True when the
    /// stream has no error afterwards.
    fn write(&self, out: &mut dyn OutStream) -> bool {
        if out.error() != 0 {
            return false;
        }
        let s = self.s.borrow();
        out.put_u16(RECORD_VERSION);
        out.put_u8(s.is_tutorial);
        out.put_u32(s.task_number as u32);
        out.put_u32(s.guid);
        out.error() == 0
    }

    /// §3 `Read`: a version other than 2 is false with nothing else read;
    /// when the stream is then error-free the three fields are read
    /// (each one that reads replaces its field) and the GUID goes through
    /// the registry mapping. True when the stream has no error.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        match inp.get_u16() {
            Some(RECORD_VERSION) => {}
            _ => return false,
        }
        if inp.error() == 0 {
            let flag = inp.get_u8();
            let task = inp.get_u32();
            let guid = inp.get_u32();
            let mut s = self.s.borrow_mut();
            if let Some(f) = flag {
                s.is_tutorial = f;
            }
            if let Some(t) = task {
                s.task_number = t as i32;
            }
            if let Some(g) = guid {
                s.guid = g;
            }
            s.guid = map_tutorial_guid(s.guid);
        }
        inp.error() == 0
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
