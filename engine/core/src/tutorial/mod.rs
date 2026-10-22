// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The tutorial system (`cSC4TutorialSystem`, class `0xAA3836FB`, city
//! component 48), `docs/specs/tutorial.md`. Section references (`§n`) point
//! at that spec.
//!
//! # Structure
//!
//! - `system.rs`: the component: the saved record (§3), `IsTutorial` (§4.1),
//!   starting and ending (§4.2), the three messages and `Init` / `Shutdown`
//!   (§4.3), and the UI slots that forward to the tutorial window (§4.4).
//! - `tests.rs`: a fake game and the §8 tests.
//!
//! The tutorial window, its tasks, the tutorial registry, the 3D view, the
//! cheat manager and the Lua context are **host objects**: the core decides
//! when each is called and with what, the host does the call. The simulation
//! reads one thing, [`TutorialSystem::is_tutorial`]: the fire simulator's
//! no-burn-down flag (`DisasterHost::tutorial_no_burn_down`) and the
//! flammability fire roll (`FlammabilityHost::is_tutorial`) take it from
//! there.
//!
//! # Choices (`[choice]`) and literal readings
//!
//! - **`QueryInterface`** (§1): the four named IIDs, and `1` for the base
//!   `cRZUnknown` lookup [inferred].
//! - **`Release`** (§1, "the shared ones of the advisor system"): the last
//!   reference marks the object destroyed; it does not run `Shutdown` or
//!   close the window [inferred]. Question TU3.
//! - **`Shutdown`** (§4.3, TU5): the cheat (when a cheat manager exists —
//!   the `cISC4App` slot 21 test `Init` applied is **not** repeated), then
//!   the window (close, release, clear +0x40, release again), then the
//!   messages in the order `0xAA3840E6`, `0xEA8AE29A`, `0x26C63343`.
//!   Questions TU4, TU5.
//! - **`startTutorial`** (§4.2): a new window comes from
//!   [`TutorialHost::create_window`] with its creation reference; the core
//!   then takes a second one ([`TutorialHost::add_ref`]) and initialises it,
//!   so the system holds the two references `Shutdown` drops (TU5). The
//!   result is the registry call's (quirk 3).
//! - **Messages** carry their data as `u32` values; `FilterMessage`'s message
//!   is [`FilterMsg`].
//! - **Threading**: the state is a `RefCell`; main thread only.

mod system;

#[cfg(test)]
pub(crate) mod tests;

pub use system::{query_interface, TutorialState, TutorialSystem};

/// Class ID (§1).
pub const CLSID_TUTORIAL_SYSTEM: u32 = 0xAA38_36FB;
/// `cISC4TutorialSystem` (§1).
pub const IID_TUTORIAL_SYSTEM: u32 = 0xAA38_36F6;
/// `cIGZMessageTarget2`.
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
/// The cheat handler sub-object.
pub const IID_CHEAT_HANDLER: u32 = 0xE98B_2F57;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// `cIGZUnknown` (the base lookup) [inferred].
pub const IID_UNKNOWN: u32 = 0x0000_0001;

/// City component (§1).
pub const CITY_COMPONENT: u32 = 48;

/// The record version (§3).
pub const RECORD_VERSION: u16 = 2;
/// The record's size in bytes (§3).
pub const RECORD_SIZE: usize = 11;

/// No tutorial GUID (§2).
pub const NO_GUID: u32 = 0xFFFF_FFFF;
/// The constructor's +0x28 (§2), not read by this class.
pub const FIELD_28_DEFAULT: u32 = 3;

/// The tutorial registry's ID mapping (§3, `0x004B5C20`).
pub mod registry {
    /// Saved GUID 0 reads as this.
    pub const GUID_0: u32 = 0x8A5B_7A6C;
    /// Saved GUID 1 reads as this.
    pub const GUID_1: u32 = 0xEA5D_6DC8;
}

/// Messages (§4.3).
pub mod msg {
    /// City init complete.
    pub const CITY_INIT_COMPLETE: u32 = 0xEA8A_E29A;
    /// `Tutorial_EndTutorial`.
    pub const END_TUTORIAL: u32 = 0xAA38_40E6;
    /// Pre-save.
    pub const PRE_SAVE: u32 = 0x26C6_3343;
    /// `Init`'s subscriptions, in the table's order [inferred].
    pub const SUBSCRIPTIONS: [u32; 3] = [CITY_INIT_COMPLETE, END_TUTORIAL, PRE_SAVE];
    /// `Shutdown`'s unsubscriptions, in the spec's order (§4.3, TU5):
    /// `0xAA3840E6`, `0xEA8AE29A`, `0x26C63343`. Not the reverse of
    /// [`SUBSCRIPTIONS`]: PRE_SAVE stays last.
    pub const UNSUBSCRIPTIONS: [u32; 3] = [END_TUTORIAL, CITY_INIT_COMPLETE, PRE_SAVE];
    /// The cheat message the handler answers (UI, not modelled).
    pub const CHEAT: u32 = 0x230E_27AC;
}

/// The debug cheat (§4.3).
pub const CHEAT_ID: u32 = 0x6A38_2BAF;
pub const CHEAT_NAME: &str = "Tutorial";

/// `FilterMessage` (§4.4): the message kind and IDs the window sees.
pub mod filter {
    /// The message kind that is looked at.
    pub const KIND: u32 = 3;
    /// IDs that pass data field 2.
    pub const FIELD2_IDS: [u32; 2] = [0x2872_59F6, 0x2872_59F7];
    /// The ID that passes data field 3.
    pub const FIELD3_ID: u32 = 0x49D9_93D4;
}

/// The message `FilterMessage` receives (§4.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FilterMsg {
    pub kind: u32,
    pub id: u32,
    pub field2: u32,
    pub field3: u32,
}

/// The registry's ID mapping (§3): `0 → 0x8A5B7A6C`, `1 → 0xEA5D6DC8`, any
/// other value unchanged (quirk 1).
pub fn map_tutorial_guid(guid: u32) -> u32 {
    match guid {
        0 => registry::GUID_0,
        1 => registry::GUID_1,
        g => g,
    }
}

/// Everything the tutorial system reads from, or does to, the rest of the
/// game. Objects are host tokens (never 0). Re-entrancy: callbacks must not
/// call back into the system.
pub trait TutorialHost {
    // ---- lifecycle (§4.3) ----
    /// Message server slot 5 / its removal.
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// The cheat manager exists and `cISC4App` slot 21 is true (`Init`).
    fn cheats_available(&self) -> bool;
    /// The cheat manager exists (`Shutdown`). [TU5] The `cISC4App` slot 21
    /// test that `Init` applied is **not** repeated on the way out.
    fn has_cheat_manager(&self) -> bool;
    /// Registers the cheat handler and the cheat `0x6A382BAF` "Tutorial".
    fn register_cheat(&self);
    /// Undoes [`TutorialHost::register_cheat`]: cheat manager slot 6 with
    /// (the cheat handler +8, 0), which removes the handler's notification,
    /// then slot 4 with `0x6A382BAF`, which unregisters the cheat (TU5).
    fn unregister_cheat(&self);

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- the window (§4.2, §4.4) ----
    /// A new tutorial window (`cSC4TutorialUIWinProc`) with count **zero**
    /// (TU9). `startTutorial` takes both owner references explicitly.
    fn create_window(&self) -> u64;
    /// Initialises a new window.
    fn init_window(&self, window: u64);
    /// Closes the window (before its release).
    fn close_window(&self, window: u64);
    /// Window slot 9: the current task (no reference).
    fn window_current_task(&self, window: u64) -> Option<u64>;
    /// Task slot 25: whether edge scrolling is allowed.
    fn task_edge_scrolling_allowed(&self, task: u64) -> bool;
    /// Window slot 6 with a message's data field.
    fn window_filter_value(&self, window: u64, value: u32);
    /// Window slot 7 (`ShowUIHintsForUsers`).
    fn window_show_hints(&self, window: u64);
    /// Window slot 8 (`HandleCatalogMenu`).
    fn window_catalog_menu(&self, window: u64, menu: u64);
    /// Window slot 10 (`FilterLeftMouseClick`).
    fn window_left_click(&self, window: u64, x: &mut i32, z: &mut i32, n: i32) -> bool;
    /// Window slot 11 `(a, b)`: the new task number.
    fn window_reset(&self, window: u64, a: i32, b: i32) -> i32;
    /// `OnTutorialEvent`: passes the event to the window.
    fn window_event(&self, window: u64, event: u32);

    // ---- the registry (§4.2, §4.3) ----
    /// `SetActive(guid, active)`; the result is what the call left in AL.
    fn registry_set_active(&self, guid: u32, active: bool) -> bool;
    /// The registry entry of `guid` (`0x004B5D10`): `None` when absent, else
    /// whether its byte +0x10 is set.
    fn registry_entry_enabled(&self, guid: u32) -> Option<bool>;

    // ---- the rest ----
    /// The 3D view's slots 60 and 59, when there is a 3D view.
    fn view_restore_controls(&self);
    /// The city's Lua context (`cISC4City` slot 81, then its slot 10), with
    /// a reference; `None` when unavailable.
    fn fetch_lua_context(&self) -> Option<u64>;
}
