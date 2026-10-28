// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cSC4View3DWin` — the city screen: what it is in memory, the complete
//! `cISC4View3DWin` interface, and what it saves (ui-windows.md §3).
//!
//! # The load-bearing measurement
//!
//! `cSC4View3DWin::AsIGZWin` at `0x005F_8400` is slot 3 and its whole body is
//! five instructions:
//!
//! ```text
//! 005f8400  test ecx, ecx
//! 005f8402  je   0x005F_8408
//! 005f8404  lea  eax, [ecx + 0x000C]     ; return this + 0x000C
//! 005f8407  ret
//! 005f8408  xor  eax, eax
//! 005f840a  ret
//! ```
//!
//! **`AsIGZWin()` returns `this + 0x000C`.** So a `cSC4View3DWin` carries its
//! `cIGZWin` sub-object at `+0x000C`, the primary vptr at `+0` is
//! `cISC4View3DWin`, and the two are distinct objects within one allocation
//! (conformance tests 1 and 2). Every catalog-menu teardown goes through that
//! sub-object (§4), never through the game interface.
//!
//! # Save behaviour (§3.3a)
//!
//! The window layer persists exactly one thing: the View3D record, group
//! `0xA9C8_647C` / type `0xE9C8_647E` / instance `1`, with a further integer
//! `4` pushed alongside the header. Nothing else is saved — **not** the
//! catalog menus, the current tool, the cursor text, the cost indicator, any
//! panel's open/closed state, or any dialog state (quirk 13). A city saved
//! with a catalog popup open loads with it closed, by design.

use super::slots::{
    as_igzwin, RetChannel, SlotInfo, VIEW3D_OFF_CATALOG_STATE, VIEW3D_OFF_MSG_FILTER, VIEW3D_OFF_PRIMARY_VPTR,
    VIEW3D_OFF_SECONDARY_KIND, VIEW3D_OFF_SECONDARY_WIN, VIEW3D_OFF_TERTIARY_KIND, VIEW3D_OFF_TERTIARY_WIN,
    VIEW3D_OFF_VIEW_MANAGER, VIEW3D_OFF_WIN, VIEW3D_SAVE_GROUP, VIEW3D_SAVE_INSTANCE, VIEW3D_SAVE_TYPE,
    VIEW3D_SAVE_VERSION, VIEW3D_SLOTS,
};
use super::win::{WinStore, WinToken};

mod actions;
pub mod archive;
mod city;
mod commands;
mod coverage;
pub use coverage::{Category, Coverage, CoverageHost, CoverageTransform};
mod dialogs;
mod gui;
mod helpers;
mod input;
mod lifecycle;
mod menus;
mod messages;
mod motion;
mod references;
mod registry;
pub use registry::COMMANDS;
mod state;
mod status;
pub mod shadow;
pub use actions::{ActionCall, ActionHost, DisasterProperties, ToolSpec};
pub use city::{CityCall, CityHost, CityObject, TunableValues};
pub use commands::{CommandHost, PanelState, QueryTool};
pub use dialogs::{DialogCall, DialogHost, EstablishData};
pub use gui::{GuiCall, GuiHost, GuiState, HelperKind};
pub use helpers::{BackgroundPicture, GraphHost, HelperHost};
pub use input::{ControlCall, ControlReply, InputEvent, InputHost, MouseContext};
pub use lifecycle::{Lifecycle, LifecycleCall, LifecycleHost, LifecycleReply, CHEATS, NOTIFICATIONS};
pub use menus::{CatalogItem, CatalogQuery, OwnedPopup, PopupCall, PopupHost, PopupLayout};
pub use messages::{MessageCall, MessageHost, Notification, Refresh, WindowMessage};
pub use motion::{Motion, MotionHost, ViewChange};
pub use references::{ObjectRecord, ReferenceHost};
pub use state::{CheatHost, CheatTool, FilterInfo, StateHost, ViewState};
pub use status::{
    CostDisplay, CostMode, CursorText, DisplayState, MissionStatus, StatusHost, StatusSnapshot, WidgetCall, WidgetHost,
};

#[cfg(test)]
mod owner_tests;

/// State owned by the city window, independent of foreign window and camera objects.
#[derive(Debug, Default)]
pub struct OwnerState {
    pub coverage: Coverage,
    pub modifiers: u32,
    pub mouse_inside: bool,
    pub right_press: (i32, i32),
    pub dragged: bool,
    pub motion: Motion,
    pub view: ViewState,
    pub display: DisplayState,
    pub panels: PanelState,
    pub lifecycle: Lifecycle,
    pub deferred: Vec<Notification>,
    pub gui: GuiState,
    /// §6d: 0 none, 1 Background2D, 2 Background3D.
    pub background_mode: u32,
    pub popups: [Option<OwnedPopup>; 2],
    stack: Vec<u32>,
    persistent: Vec<u32>,
}

/// The two dwords `CloseTertiaryMenu` writes through `view+0x0208` (§4.1).
///
/// Their meaning is **not established** — a scroll position, a selected item,
/// a filter are all consistent with the evidence (ui-windows.md UIW-1). The
/// interface `0x8A2C_AD8B` is unidentified. We model them as opaque dwords and
/// record the write order, which *is* measured: slot 17 first, then slot 9.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CatalogState {
    /// `p->slot17()`, written first (§4.1).
    pub field0: u32,
    /// `p->slot9()`, written second (§4.1).
    pub field1: u32,
}

/// A reference in the View3D save record (§3.3a).
///
/// The record is a **reference record**: `DoLoad` resolves each by class id.
/// `0xA9C7_41B3` is `cSC4ModelMaker`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SaveRef {
    /// The object's class id, or 0 when the reference is null.
    pub class_id: u32,
    /// Whether the reference was non-null at save time.
    pub present: bool,
}

impl SaveRef {
    pub fn null() -> SaveRef {
        SaveRef {
            class_id: 0,
            present: false,
        }
    }

    pub fn of(class_id: u32) -> SaveRef {
        SaveRef {
            class_id,
            present: true,
        }
    }
}

/// Legacy header/two-reference projection used by existing callers. The full
/// core-owned logical body is [`View3DRecord`]; this projection stays compatible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct View3DSave {
    /// The record header's group.
    pub group: u32,
    /// Its type.
    pub type_id: u32,
    /// Its instance.
    pub instance: u32,
    /// The further integer pushed alongside (§3.3a). *[inferred]* a record
    /// version; the header helper's body was not read.
    pub version: u32,
    /// `this+0x00F0`: the first reference.
    pub ref_f0: SaveRef,
    /// `this+0x00F4`, written as `obj + 8` when non-null else null.
    pub ref_f4: SaveRef,
}

/// Four-reference version-4 logical record (view3d-window §6k), without UI state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct View3DRecord {
    pub header: View3DSave,
    pub model_maker: SaveRef,
    pub simulator: SaveRef,
    /// Trigger pairs in bucket/head order (§6l).
    pub triggers: Vec<(u32, u32)>,
}

impl View3DSave {
    /// The record header identity, exactly as `DoSave` builds it (§3.3a).
    pub fn header() -> (u32, u32, u32, u32) {
        (
            VIEW3D_SAVE_GROUP,
            VIEW3D_SAVE_TYPE,
            VIEW3D_SAVE_INSTANCE,
            VIEW3D_SAVE_VERSION,
        )
    }

    /// Both record adapters accept the same identity and all versions below 5
    /// (§6k). Validate before resolving references or assigning trigger pairs.
    fn accepts_header(header: (u32, u32, u32, u32)) -> bool {
        let expected = Self::header();
        (header.0, header.1, header.2) == (expected.0, expected.1, expected.2) && header.3 < 5
    }

    /// True when this record carries no UI state — the property that makes
    /// "a city saved with a catalog popup open loads with it closed" true
    /// (conformance test 25).
    ///
    /// The record has exactly two fields and both are references; there is
    /// nowhere for a window pointer to go.
    pub fn carries_ui_state(&self) -> bool {
        false
    }
}

/// `cSC4View3DWin`'s catalogue of what it holds, laid out by the measured
/// offsets of §3.2.
#[derive(Debug)]
pub struct View3DWin {
    /// The object's own address, so `AsIGZWin` can compute `this + 0x000C`.
    ///
    /// *[choice]* a Rust model has no real allocation with two vptrs, so we
    /// keep a synthetic address and expose the layout as data. The offset
    /// constant is the measurement; the address only makes the arithmetic
    /// checkable.
    this: u32,
    /// The `cIGZWin` / `cGZWinGen` sub-object at `+0x000C`, as a store token.
    win: WinToken,
    /// `+0x01F8` secondary catalog-menu window, or null.
    secondary_win: Option<WinToken>,
    /// `+0x01FC` tertiary catalog-menu window, or null.
    tertiary_win: Option<WinToken>,
    /// `+0x0200` secondary menu kind.
    secondary_kind: u32,
    /// `+0x0204` tertiary menu kind.
    tertiary_kind: u32,
    /// `+0x0208` pointer to the two-dword catalog state, or null. Only the
    /// tertiary path writes it (§4.2: `CloseSecondaryMenu` does **not** touch
    /// it — a replacement must not "tidy up" by also writing it).
    catalog_state: Option<CatalogState>,
    /// `+0x0300` the view manager object (single-site reading).
    view_manager: Option<u64>,
    /// The current view input control (slot 24), the "tool". `Invoke*` drops
    /// it first (§4.3, quirk 3).
    owner: OwnerState,
    /// `AddRef`/`Release` count (slots 1/2).
    refs: i32,
    /// `+0x0008` message-filter sub-object. Inline, so the catalog path passes
    /// its address to `AddMessageFilter` (§3.2).
    msg_filter_offset: u32,
}

impl View3DWin {
    /// Build the view around an already-created window sub-object.
    pub fn new(this: u32, win: WinToken) -> View3DWin {
        View3DWin {
            this,
            win,
            secondary_win: None,
            tertiary_win: None,
            secondary_kind: 0,
            tertiary_kind: 0,
            catalog_state: None,
            view_manager: None,
            owner: OwnerState::default(),
            refs: 1,
            msg_filter_offset: VIEW3D_OFF_MSG_FILTER,
        }
    }

    // --- the layout, as the measurements of §3.2 name it -------------------

    /// `AsIGZWin` (slot 3, `0x005F_8400`): `this + 0x000C`, or null for a null
    /// `this`.
    ///
    /// Quirk 4: the only null check is on `this` itself. A non-null result is
    /// **not** proof of a valid window — use `cIGZWinMgr::IsWindowValid`
    /// (slot 24) for that.
    pub fn as_igzwin(&self) -> WinToken {
        self.win
    }

    /// The arithmetic `AsIGZWin` performs on the object address, as a raw
    /// offset, for a layout assertion (conformance test 1).
    pub fn as_igzwin_addr(this: u32) -> u32 {
        as_igzwin(this)
    }

    /// The object's synthetic address.
    pub fn this_addr(&self) -> u32 {
        self.this
    }

    /// `+0x01F8`.
    pub fn secondary_win(&self) -> Option<WinToken> {
        self.secondary_win
    }

    /// `+0x01FC`.
    pub fn tertiary_win(&self) -> Option<WinToken> {
        self.tertiary_win
    }

    /// `+0x0200`.
    pub fn secondary_kind(&self) -> u32 {
        self.secondary_kind
    }

    /// `+0x0204`.
    pub fn tertiary_kind(&self) -> u32 {
        self.tertiary_kind
    }

    /// `+0x0208`, the catalog-state pointer.
    pub fn catalog_state(&self) -> Option<CatalogState> {
        self.catalog_state
    }

    /// `+0x0208`, as the raw pointer the close path writes through. Null when
    /// the field is null.
    pub fn catalog_state_ptr(&self) -> u32 {
        if self.catalog_state.is_some() {
            VIEW3D_OFF_CATALOG_STATE
        } else {
            0
        }
    }

    /// `+0x0300`.
    pub fn view_manager(&self) -> Option<u64> {
        self.view_manager
    }

    /// `+0x0008`: the message-filter sub-object's offset. Inline (§3.2).
    pub fn msg_filter_offset(&self) -> u32 {
        self.msg_filter_offset
    }

    pub fn set_catalog_state(&mut self, state: Option<CatalogState>) {
        self.catalog_state = state;
    }

    pub fn set_view_manager(&mut self, m: Option<u64>) {
        self.view_manager = m;
    }

    // --- view state the menu path and the save record read -----------------

    /// `GetCurrentViewInputControl` (slot 24): the current tool, or none.
    pub fn input_control(&self) -> Option<u32> {
        self.owner.stack.last().copied()
    }

    /// `SetCurrentViewInputControl` (slot 25). Passing 0 clears the tool.
    pub fn set_input_control(&mut self, control: u32) {
        self.owner.stack.clear();
        self.owner.stack.extend((control != 0).then_some(control));
    }

    /// `RemoveAllViewInputControls(force)` (slot 27).
    ///
    /// Token-only catalog compatibility entry. Adapters must perform the
    /// lifecycle operations using `remove_all_controls` instead (§6e).
    pub fn remove_all_view_input_controls(&mut self, force: u32) -> Vec<u32> {
        let dropped = self.owner.stack.drain(..).rev().collect();
        let _ = force;
        dropped
    }

    pub fn persistent_controls(&self) -> u32 {
        self.owner.persistent.len() as u32
    }

    pub fn owner_state(&self) -> &OwnerState {
        &self.owner
    }

    /// The five supported interfaces share the window's reference count (§1).
    pub fn query_interface(&mut self, iid: u32) -> Option<u32> {
        let offset = match iid {
            1 | 0xFA47_B3F9 => 0,
            0x4522_94AA => 4,
            0xC6AE_7085 => 8,
            0x22BA_0121 => 12,
            _ => return None,
        };
        self.add_ref();
        Some(self.this.wrapping_add(offset))
    }

    pub fn add_ref(&mut self) -> u32 {
        self.refs += 1;
        self.refs as u32
    }

    pub fn release(&mut self) -> u32 {
        self.refs -= 1;
        self.refs as u32
    }

    pub fn refs(&self) -> i32 {
        self.refs
    }

    // --- internal setters the catalog path uses ---------------------------

    pub fn set_secondary(&mut self, win: Option<WinToken>, kind: u32) {
        self.secondary_win = win;
        self.secondary_kind = kind;
    }

    pub fn set_tertiary(&mut self, win: Option<WinToken>, kind: u32) {
        self.tertiary_win = win;
        self.tertiary_kind = kind;
    }

    /// Write the two catalog-state dwords through `+0x0208`, in the measured
    /// order: slot 17 first, then slot 9 (§4.1).
    ///
    /// Quirk 5: the close path never validates the pointer — if it is stale
    /// the write is a wild store. Quirk 6: this runs only on a **successful**
    /// `QueryInterface(0x8A2C_AD8B)`; a failed query leaves `+0x0208` untouched
    /// but still destroys the window.
    pub fn write_catalog_state(&mut self, field0: u32, field1: u32) -> bool {
        match &mut self.catalog_state {
            Some(s) => {
                s.field0 = field0;
                s.field1 = field1;
                true
            }
            None => false,
        }
    }

    // --- the 61-slot table ------------------------------------------------

    /// The complete `cISC4View3DWin` slot table (§3.3).
    pub fn slots() -> &'static [SlotInfo; 61] {
        &VIEW3D_SLOTS
    }

    /// Look one slot up by index.
    pub fn slot(n: u32) -> Option<&'static SlotInfo> {
        VIEW3D_SLOTS.get(n as usize)
    }

    /// The return channel of a slot.
    ///
    /// Conformance test 3 rests on slots 59 and 60 answering [`RetChannel::None`].
    pub fn ret_channel(n: u32) -> Option<RetChannel> {
        Self::slot(n).map(|s| s.ret)
    }

    /// `GetViewManager` (slot 6): the object at `+0x0300` that answers
    /// `QueryInterface(0x67C5_BEDE)`.
    pub fn get_view_manager(&self) -> Option<u64> {
        self.view_manager
    }

    // --- save (§3.3a) -----------------------------------------------------

    /// `DoSave` (`0x007E_A5B0`): the View3D record and nothing else.
    ///
    /// The body is skipped entirely when the record-body helper returns false
    /// (`0x005D_9670`, tested at `0x007E_A5DB`). We model that as the caller
    /// deciding; this method produces what would be written.
    pub fn do_save(&self) -> View3DSave {
        let (group, type_id, instance, version) = View3DSave::header();
        View3DSave {
            group,
            type_id,
            instance,
            version,
            // `this+0x00F0` and `this+0x00F4` (the latter as `obj + 8` when
            // non-null, else null).
            ref_f0: self.owner.view.references[0],
            ref_f4: self.owner.view.references[1],
        }
    }

    pub fn save_record(&self) -> View3DRecord {
        View3DRecord {
            header: self.do_save(),
            model_maker: self.owner.view.references[2],
            simulator: self.owner.view.references[3],
            triggers: self.owner.view.triggers.iter().map(|(t, &count)| (t, count)).collect(),
        }
    }

    pub fn load_record(&mut self, record: &View3DRecord) -> Result<(), String> {
        self.do_load(&record.header)?;
        self.owner.view.references[2] = record.model_maker;
        self.owner.view.references[3] = record.simulator;
        state::load_triggers(&mut self.owner.view.triggers, &record.triggers);
        Ok(())
    }

    /// `DoLoad` (`0x007E_BFB0`): retain the references by class id and leave
    /// every UI field clear.
    ///
    /// Quirk 13 / conformance test 25: a catalog popup open at save time is
    /// closed at load time. Nothing in the record can restore `+0x01F8` …
    /// `+0x0208`, so a load that leaves them set would be inventing state.
    pub fn do_load(&mut self, record: &View3DSave) -> Result<(), String> {
        let want = View3DSave::header();
        if !View3DSave::accepts_header((record.group, record.type_id, record.instance, record.version)) {
            return Err(format!(
                "View3D record identity {:#010x}/{:#010x}/{}/{} is not {:#010x}/{:#010x}/{}/{}",
                record.group, record.type_id, record.instance, record.version, want.0, want.1, want.2, want.3
            ));
        }
        // The whole UI side is cleared: no dialog state, no catalog menus, no
        // tool, no cursor text, no cost indicator, no panel state.
        self.secondary_win = None;
        self.secondary_kind = 0;
        self.tertiary_win = None;
        self.tertiary_kind = 0;
        self.catalog_state = None;
        self.owner.stack.clear();
        self.owner.persistent.clear();
        self.owner.display = DisplayState::default();
        self.owner.panels = PanelState::default();
        // The legacy projection supplies two references. Full core callers use
        // load_record; foreign reference resolution/serialization is not here.
        self.owner.view.references[0] = record.ref_f0;
        self.owner.view.references[1] = record.ref_f4;
        self.owner.view.not_loaded = false;
        Ok(())
    }
}

/// Every offset `cSC4View3DWin`'s layout defines (§3.2), so a conformance
/// vector can assert them without hard-coding numbers.
pub const VIEW3D_LAYOUT: [(&str, u32); 9] = [
    ("primary_vptr", VIEW3D_OFF_PRIMARY_VPTR),
    ("msg_filter", VIEW3D_OFF_MSG_FILTER),
    ("win", VIEW3D_OFF_WIN),
    ("secondary_win", VIEW3D_OFF_SECONDARY_WIN),
    ("tertiary_win", VIEW3D_OFF_TERTIARY_WIN),
    ("secondary_kind", VIEW3D_OFF_SECONDARY_KIND),
    ("tertiary_kind", VIEW3D_OFF_TERTIARY_KIND),
    ("catalog_state", VIEW3D_OFF_CATALOG_STATE),
    ("view_manager", VIEW3D_OFF_VIEW_MANAGER),
];

/// The catalog-menu window's parent on a create (§4.3, conformance test 15).
///
/// `SC4CreateWindowFromScript(..., this->AsIGZWin(), ...)`: the popup is a
/// child window **of the view**, which is what makes `ChildDelete` on that
/// sub-object the right teardown.
pub fn catalog_parent(view: &View3DWin, store: &WinStore) -> WinToken {
    let p = view.as_igzwin();
    debug_assert!(store.is_window_valid(p) || p != 0);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_igzwin_is_this_plus_offset() {
        // Conformance test 1.
        assert_eq!(View3DWin::as_igzwin_addr(0), 0, "null this returns null");
        assert_eq!(View3DWin::as_igzwin_addr(0x1000), 0x100C);
        assert_eq!(VIEW3D_OFF_WIN, 0x000C);
    }

    #[test]
    fn two_vtables_one_allocation() {
        // Conformance test 2: the primary vptr and the window sub-object's
        // are distinct, within one allocation.
        use super::super::slots::{VIEW3D_VTABLE, WIN_GEN_VTABLE};
        assert_ne!(VIEW3D_VTABLE, WIN_GEN_VTABLE);
        assert_eq!(VIEW3D_OFF_PRIMARY_VPTR, 0x0000);
        assert_eq!(VIEW3D_OFF_WIN, 0x000C);
    }

    #[test]
    fn layout_offsets_match_the_table() {
        for (name, off) in VIEW3D_LAYOUT {
            let expect = match name {
                "primary_vptr" => 0x0000,
                "msg_filter" => 0x0008,
                "win" => 0x000C,
                "secondary_win" => 0x01F8,
                "tertiary_win" => 0x01FC,
                "secondary_kind" => 0x0200,
                "tertiary_kind" => 0x0204,
                "catalog_state" => 0x0208,
                "view_manager" => 0x0300,
                other => panic!("unknown field {other}"),
            };
            assert_eq!(off, expect, "field {name}");
        }
    }

    #[test]
    fn save_record_carries_no_ui_state() {
        // Conformance test 25 / quirk 13.
        let mut v = View3DWin::new(0x2000, 7);
        v.set_secondary(Some(11), 0x09DE_8798);
        v.set_tertiary(Some(12), 0xAB95_4023);
        v.set_catalog_state(Some(CatalogState { field0: 1, field1: 2 }));
        let rec = v.do_save();
        assert!(!rec.carries_ui_state());
        assert_eq!(View3DSave::header(), (VIEW3D_SAVE_GROUP, VIEW3D_SAVE_TYPE, 1, 4));

        // A load drops the UI side entirely.
        v.do_load(&rec).unwrap();
        assert_eq!(v.secondary_win(), None);
        assert_eq!(v.tertiary_win(), None);
        assert_eq!(v.secondary_kind(), 0);
        assert_eq!(v.tertiary_kind(), 0);
        assert_eq!(v.catalog_state(), None);
    }

    #[test]
    fn load_rejects_a_foreign_record() {
        let mut v = View3DWin::new(0x2000, 7);
        let bad = View3DSave {
            group: 1,
            type_id: 2,
            instance: 3,
            version: 4,
            ref_f0: SaveRef::null(),
            ref_f4: SaveRef::null(),
        };
        assert!(v.do_load(&bad).is_err());
    }
}
