// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Identities of the budget window family (budget-window.md §1-§7).

/// §1: window-generator class and interface.
pub const GENERATOR_CLSID: u32 = 0x4386_D516;
pub const GENERATOR_IID: u32 = 0x5386_D516;
/// §1: the dialog window's ID.
pub const DIALOG_WIN_ID: u32 = 0x0423_278F;
/// §1: `cSC4SubBudgetWindow` allocation size.
pub const SUB_BUDGET_WINDOW_SIZE: usize = 0x140;
/// §1: the budget panel's two tab windows.
pub const PANEL_TAB_0: u32 = 0xAA3A_C000;
pub const PANEL_TAB_1: u32 = 0xAA3A_C001;

/// §4 messages.
pub const MSG_INIT: u32 = 0xA2BF_8AD0;
pub const MSG_SHUTDOWN: u32 = 0xA2BF_8AD1;
pub const MSG_BUTTON: u32 = 0x2872_59F6;
pub const MSG_SLIDER: u32 = 0x8871_13A4;
pub const MSG_COMBO: u32 = 0xE871_1255;
pub const KEY_ESC: u32 = 0x1B;
pub const KEY_ENTER: u32 = 0x0D;
pub const KEY_0X73: u32 = 0x73;
/// §4: modifier bit that enables the Enter / `0x73` close.
pub const KEY_CLOSE_MODIFIER: u32 = 0x4;

/// §4 selectors.
pub const SELECTOR_ORDINANCES: u32 = 1;
pub const SELECTOR_NEIGHBORS: u32 = 2;

/// §4 background images (type, group, instance).
pub const BG_TYPE: u32 = 0x856D_DBAC;
pub const BG_GROUP: u32 = 0x46A0_06B0;
pub const BG_ORDINANCES: u32 = 0x1401_55F2;
pub const BG_NEIGHBORS: u32 = 0x1401_55D2;

/// §4.4: slider IDs below this set occupant funding level 0, else level 1.
pub const OCCUPANT_LEVEL1_SLIDER_MIN: u32 = 0x0ABC_E500;

/// §5 buttons.
pub const BTN_OK: u32 = 0x67;
pub const BTN_OK_ALT: u32 = 0x42B7_C353;
pub const BTN_CANCEL: u32 = 0x6D;
pub const BTN_CLOSE_CHILD: u32 = 0x68;
pub const BTN_CLOSE_CHILD_ALT: u32 = 0x384;
pub const BTN_END: u32 = 0xCC;
pub const BTN_END_ALT: u32 = 0x385;
pub const BTN_ORDINANCE_APPLY: u32 = 0xCD;
pub const BTN_CE: u32 = 0xCE;
pub const BTN_CF: u32 = 0xCF;
pub const BTN_LIST_A_UP: u32 = 0x451;
pub const BTN_LIST_A_DOWN: u32 = 0x452;
pub const BTN_LIST_B_UP: u32 = 0x453;
pub const BTN_LIST_B_DOWN: u32 = 0x454;
pub const BTN_OCCUPANT_ROW_FIRST: u32 = 0x0ABC_E600;
pub const BTN_OCCUPANT_ROW_LAST: u32 = 0x0ABC_E6FF;
pub const BTN_OCCUPANT_CONTROL_FIRST: u32 = 0x0ABC_DE00;
pub const BTN_OCCUPANT_CONTROL_LAST: u32 = 0x0ABC_DEFF;
/// §5: scroll step and page, in rows.
pub const SCROLL_STEP: u32 = 2;
pub const SCROLL_PAGE: u32 = 9;

/// §3 generator flags in `CreateSubBudgetWindow` order.
pub const CREATE_GENERATOR_FLAGS: [(u32, bool); 8] = [
    (0x1, true),
    (0x2, false),
    (0x1_0000, false),
    (0x20, false),
    (0x40, false),
    (0x1000, true),
    (0x2000, false),
    (0x4000, false),
];
/// §3 window flags in `CreateSubBudgetWindow` order.
pub const CREATE_WIN_FLAGS: [(u32, bool); 4] = [(0x100, true), (0x1_0000, false), (0x4, true), (0x800, false)];
/// §6: the window flag `end_dialog` tests (modal).
pub const WIN_FLAG_MODAL: u32 = 0x1000;

/// §7 panel messages.
pub const PANEL_MSG_MONEY_A: u32 = 0x08DA_C3E7;
pub const PANEL_MSG_MONEY_B: u32 = 0x08DA_C3E8;
pub const PANEL_MSG_TICK: u32 = 0x86AD_10EE;

/// §2.1 department-group selectors.
pub const GROUP_CIVIC: u32 = 0x0A5A_72D1;
pub const GROUP_TRANSPORTATION: u32 = 0xAA36_9059;
pub const GROUP_PROTECTION: u32 = 0x4A35_7B40;
pub const GROUP_EDUCATION_HEALTH: u32 = 0x6A35_7B7F;
pub const GROUP_UTILITIES: u32 = 0x4A35_7EAF;
pub const GROUP_PARKS_LANDMARKS: u32 = 0x6A35_7B96;
pub const GROUP_CIVIC2: u32 = 0xEA59_7195;
