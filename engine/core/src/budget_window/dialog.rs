// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cSC4SubBudgetWindow` (budget-window.md §3-§6).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::ids::*;
use super::{BudgetWindowHost, DealRef, DeptRef, OccRef, OrdRef, WinRef};

/// One group-mode slider row (`SliderInfo`, vector `+0x2C`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderInfo {
    pub slider: u32,
    pub dept: DeptRef,
}

/// One ordinance row: the original and the edited on/off state (the byte
/// pairs at `+0xA8`, count `+0xA0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrdinanceRow {
    pub ordinance: OrdRef,
    pub original: bool,
    pub edited: bool,
}

/// `IndividualRestoreInfo` (12 bytes: occupant, level0, level1; vector `+0x44`).
/// The level type is not given by the spec; it is carried as the slider
/// position type `SetOccupantFundingLevel` receives in §4.4 (BW8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndividualRestoreInfo {
    pub occupant: OccRef,
    pub level0: i32,
    pub level1: i32,
}

/// A scrolled row list (§5 `0x451`..`0x454`): position and length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollList {
    pub pos: u32,
    pub len: u32,
}

/// The two list positions handed to the mode refresh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollView {
    pub a: u32,
    pub b: u32,
}

/// The dialog mode, in the order `on_init` tests it (§4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubBudgetMode {
    Ordinances,
    Neighbors,
    Individual(OccRef),
    Group(u32),
}

/// The messages `DoWinMsg` routes (§4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubBudgetMsg {
    Init,
    Shutdown,
    Button(u32),
    Slider { slider: u32, position: i32 },
    Combo(u32),
    KeyDown { key: u32, modifiers: u32 },
    Other(u32),
}

/// `end_dialog` (`0x00779850`, §6): null → false; a modal window (flag
/// `0x1000`) asks its parent to end the modal loop, any other is removed from
/// its parent. Returns true.
pub fn end_dialog<H: BudgetWindowHost + ?Sized>(host: &mut H, win: Option<WinRef>) -> bool {
    let Some(win) = win else { return false };
    if host.win_flag(win, WIN_FLAG_MODAL) {
        host.end_modal_via_parent(win);
    } else {
        host.remove_from_parent(win);
    }
    true
}

/// `cSC4SubBudgetWindow`.
#[derive(Debug)]
pub struct SubBudgetWindow {
    /// `+0x0C`: the dialog window.
    window: Option<WinRef>,
    /// `+0x14`: the inner child popup.
    inner_child: Option<WinRef>,
    /// `+0x20`.
    selector: u32,
    /// `+0x24`: the occupant (individual mode), null from the panel.
    occupant: Option<OccRef>,
    /// `+0x28`: the panel's `+0x198` value (meaning not specified).
    context: u32,
    sliders: Vec<SliderInfo>,
    saved_fractions: Vec<(f32, DeptRef)>,
    restore: Vec<IndividualRestoreInfo>,
    ordinances: Vec<OrdinanceRow>,
    deals: Vec<DealRef>,
    list_a: ScrollList,
    list_b: ScrollList,
    /// The process-wide slider re-entry byte (`0x00B4DFC8`); share one
    /// guard between every dialog to get the process-wide behaviour.
    slider_guard: Arc<AtomicBool>,
}

impl Default for SubBudgetWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SubBudgetWindow {
    /// Construct (`0x007862B0`) with a guard of its own.
    pub fn new() -> Self {
        Self::with_slider_guard(Arc::new(AtomicBool::new(false)))
    }

    pub fn with_slider_guard(slider_guard: Arc<AtomicBool>) -> Self {
        Self {
            window: None,
            inner_child: None,
            selector: 0,
            occupant: None,
            context: 0,
            sliders: Vec::new(),
            saved_fractions: Vec::new(),
            restore: Vec::new(),
            ordinances: Vec::new(),
            deals: Vec::new(),
            list_a: ScrollList::default(),
            list_b: ScrollList::default(),
            slider_guard,
        }
    }

    pub fn window(&self) -> Option<WinRef> {
        self.window
    }
    pub fn selector(&self) -> u32 {
        self.selector
    }
    pub fn occupant(&self) -> Option<OccRef> {
        self.occupant
    }
    pub fn context(&self) -> u32 {
        self.context
    }
    pub fn inner_child(&self) -> Option<WinRef> {
        self.inner_child
    }
    pub fn sliders(&self) -> &[SliderInfo] {
        &self.sliders
    }
    pub fn saved_fractions(&self) -> &[(f32, DeptRef)] {
        &self.saved_fractions
    }
    pub fn ordinances(&self) -> &[OrdinanceRow] {
        &self.ordinances
    }
    pub fn deals(&self) -> &[DealRef] {
        &self.deals
    }
    pub fn list_a(&self) -> ScrollList {
        self.list_a
    }
    pub fn list_b(&self) -> ScrollList {
        self.list_b
    }

    /// The mode in `on_init`'s test order (§4): selector 1, selector 2,
    /// occupant non-null, otherwise a department group.
    pub fn mode(&self) -> SubBudgetMode {
        match (self.selector, self.occupant) {
            (SELECTOR_ORDINANCES, _) => SubBudgetMode::Ordinances,
            (SELECTOR_NEIGHBORS, _) => SubBudgetMode::Neighbors,
            (_, Some(o)) => SubBudgetMode::Individual(o),
            (g, None) => SubBudgetMode::Group(g),
        }
    }

    /// Host input: the inner popup child held at `+0x14`. Which code opens
    /// it is not specified (BW9); the reference given here is the one the
    /// dialog releases when it closes the child.
    pub fn set_inner_child(&mut self, child: Option<WinRef>) {
        self.inner_child = child;
    }

    /// Host input: an ordinance row's check toggled. The check-button IDs
    /// are not specified (BW9), so the edit arrives as typed input.
    pub fn set_ordinance_edited(&mut self, row: usize, on: bool) -> bool {
        match self.ordinances.get_mut(row) {
            Some(r) => {
                r.edited = on;
                true
            }
            None => false,
        }
    }

    /// Host input: the length of scroll list A or B, set by the row-layout
    /// routines (§8 open work). The position is clamped to the new range.
    pub fn set_list_len(&mut self, list_b: bool, len: u32) {
        let l = if list_b { &mut self.list_b } else { &mut self.list_a };
        l.len = len;
        l.pos = l.pos.min(Self::max_pos(len));
    }

    /// The largest first row: `len - page`, 0 when the list fits. The spec
    /// says "page = 9 rows" only; this bound is its literal reading (BW7).
    fn max_pos(len: u32) -> u32 {
        len.saturating_sub(SCROLL_PAGE)
    }

    /// `CreateSubBudgetWindow` (`0x00779E50`, §3). Returns the window (the
    /// `*out` reference, AddRef'd) or `None` on failure.
    pub fn create<H: BudgetWindowHost + ?Sized>(
        &mut self,
        host: &mut H,
        selector: u32,
        context: u32,
        occupant: Option<OccRef>,
    ) -> Option<WinRef> {
        let win = host.create_window_generator()?;
        host.add_ref_win(win);
        self.window = Some(win);
        for (flag, on) in CREATE_GENERATOR_FLAGS {
            host.set_generator_flag(win, flag, on);
        }
        host.set_generator_win_proc(win);
        for (flag, on) in CREATE_WIN_FLAGS {
            host.set_win_flag(win, flag, on);
        }
        host.set_win_id(win, DIALOG_WIN_ID);
        self.selector = selector;
        self.context = context;
        self.occupant = occupant;
        Some(win)
    }

    /// `DoWinMsg` (`0x0078BCA0`, §4). `Some(result)` where the spec gives
    /// the return value, `None` where it does not (init, shutdown, slider,
    /// combo, an ignored key, other messages).
    pub fn do_win_msg<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H, msg: SubBudgetMsg) -> Option<bool> {
        match msg {
            SubBudgetMsg::Init => {
                self.on_init(host);
                None
            }
            // on_shutdown (0x0077E440) is named but not specified (BW9).
            SubBudgetMsg::Shutdown => None,
            SubBudgetMsg::Button(id) => Some(self.on_button_activated(host, id)),
            SubBudgetMsg::Slider { slider, position } => {
                self.on_slider(host, slider, position);
                None
            }
            SubBudgetMsg::Combo(id) => {
                if self.mode() == SubBudgetMode::Neighbors {
                    if let Some(w) = self.window {
                        host.rewrite_selected_deal_rows(w, id);
                    }
                    self.refresh_mode(host);
                }
                None
            }
            SubBudgetMsg::KeyDown { key, modifiers } => {
                let close =
                    key == KEY_ESC || ((key == KEY_ENTER || key == KEY_0X73) && modifiers & KEY_CLOSE_MODIFIER != 0);
                if !close {
                    return None;
                }
                self.close_inner_child(host);
                // The spec writes `end_dialog(win, 0x42B7C353)`; end_dialog
                // (§6) takes the window only, so the code is not passed (BW9).
                end_dialog(host, self.window);
                Some(true)
            }
            SubBudgetMsg::Other(_) => None,
        }
    }

    /// `on_init` (`0x007876B0`, §4): the hidden pause, then the mode's rows.
    fn on_init<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H) {
        host.hidden_pause();
        let Some(win) = self.window else { return };
        match self.mode() {
            SubBudgetMode::Ordinances => {
                host.load_background(win, BG_TYPE, BG_GROUP, BG_ORDINANCES);
                self.ordinances = host
                    .init_ordinance_budget(win)
                    .into_iter()
                    .map(|(ordinance, on)| OrdinanceRow {
                        ordinance,
                        original: on,
                        edited: on,
                    })
                    .collect();
            }
            SubBudgetMode::Neighbors => {
                host.load_background(win, BG_TYPE, BG_GROUP, BG_NEIGHBORS);
                host.set_neighbor_ui_disable(true);
                self.deals = host.init_neighbor_budget(win);
            }
            SubBudgetMode::Individual(occ) => {
                // When the restore records are filled is not stated; they
                // are taken from initIndividualBudget's contract (BW8).
                self.restore = host.init_individual_budget(win, occ);
            }
            SubBudgetMode::Group(group) => {
                for dept in host.department_budgets_in_group(group) {
                    let slider = host.add_department_row(win, group, dept);
                    self.sliders.push(SliderInfo { slider, dept });
                    let f = host.funding_percentage(dept);
                    self.saved_fractions.push((f, dept));
                }
            }
        }
    }

    /// §4.4: slider moved.
    fn on_slider<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H, slider: u32, position: i32) {
        if self.slider_guard.swap(true, Ordering::AcqRel) {
            return;
        }
        match self.occupant {
            None => {
                // "position x 0.01" (BW8: the precision of the product is
                // not given; computed in double, stored as float).
                let fraction = (f64::from(position) * 0.01) as f32;
                for s in self.sliders.clone() {
                    if s.slider == slider {
                        host.set_funding_percentage(s.dept, fraction, 0);
                    }
                }
                if let Some(w) = self.window {
                    host.update_values(w);
                }
            }
            Some(_) => {
                if let Some(occ) = host.indiv_item_for_slider(slider) {
                    let which = u32::from(slider >= OCCUPANT_LEVEL1_SLIDER_MIN);
                    host.set_occupant_funding_level(occ, which, position);
                    if let Some(w) = self.window {
                        host.update_individual_budget(w);
                    }
                }
            }
        }
        self.slider_guard.store(false, Ordering::Release);
    }

    /// Remove and drop the inner child at `+0x14`, if any.
    fn close_inner_child<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H) {
        if let Some(child) = self.inner_child.take() {
            if let Some(w) = self.window {
                host.remove_child(w, child);
            }
            host.release_win(child);
        }
    }

    /// `RestoreStartSettings` (`0x0077C5E0`, §5.1).
    pub fn restore_start_settings<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H) {
        match self.occupant {
            None => {
                for &(f, dept) in &self.saved_fractions {
                    host.set_funding_percentage(dept, f, 0);
                }
            }
            Some(_) => {
                for r in &self.restore {
                    host.set_occupant_funding_level(r.occupant, 0, r.level0);
                    host.set_occupant_funding_level(r.occupant, 1, r.level1);
                }
            }
        }
    }

    fn scroll<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H, list_b: bool, down: bool) {
        let l = if list_b { &mut self.list_b } else { &mut self.list_a };
        let max = Self::max_pos(l.len);
        l.pos = if down {
            (l.pos + SCROLL_STEP).min(max)
        } else {
            l.pos.saturating_sub(SCROLL_STEP)
        };
        let (pos, up_id, down_id) = if list_b {
            (l.pos, BTN_LIST_B_UP, BTN_LIST_B_DOWN)
        } else {
            (l.pos, BTN_LIST_A_UP, BTN_LIST_A_DOWN)
        };
        if let Some(w) = self.window {
            host.set_arrow_visible(w, up_id, pos > 0);
            host.set_arrow_visible(w, down_id, pos < max);
        }
    }

    /// The mode's row refresh (ordinances, neighbours, occupant).
    fn refresh_mode<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H) {
        let Some(w) = self.window else { return };
        let view = ScrollView {
            a: self.list_a.pos,
            b: self.list_b.pos,
        };
        match self.mode() {
            SubBudgetMode::Ordinances => host.update_ordinance_budget(w, &self.ordinances, view),
            SubBudgetMode::Neighbors => host.update_neighbor_budget(w, &self.deals, view),
            SubBudgetMode::Individual(_) => host.update_individual_budget(w),
            SubBudgetMode::Group(_) => {}
        }
    }

    /// `on_button_activated` (`0x0078B120`, §5).
    pub fn on_button_activated<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H, id: u32) -> bool {
        match id {
            BTN_OK | BTN_OK_ALT | BTN_END | BTN_END_ALT => {
                end_dialog(host, self.window);
                return true;
            }
            BTN_CANCEL => {
                self.restore_start_settings(host);
                end_dialog(host, self.window);
                return true;
            }
            BTN_CLOSE_CHILD | BTN_CLOSE_CHILD_ALT => self.close_inner_child(host),
            BTN_ORDINANCE_APPLY => {
                for r in &self.ordinances {
                    if r.edited != r.original {
                        host.set_ordinance_on(r.ordinance, r.edited);
                    }
                }
                self.close_inner_child(host);
                end_dialog(host, self.window);
            }
            BTN_CE => {
                self.close_inner_child(host);
                end_dialog(host, self.window);
            }
            BTN_CF => {
                // FUN_0077AB40 runs first; what it does is BW2 (open).
                self.close_inner_child(host);
                end_dialog(host, self.window);
            }
            BTN_LIST_A_UP => self.scroll(host, false, false),
            BTN_LIST_A_DOWN => self.scroll(host, false, true),
            BTN_LIST_B_UP => self.scroll(host, true, false),
            BTN_LIST_B_DOWN => self.scroll(host, true, true),
            BTN_OCCUPANT_ROW_FIRST..=BTN_OCCUPANT_ROW_LAST => {
                if let Some(w) = self.window {
                    host.do_individual_budget_window(w, id);
                }
            }
            BTN_OCCUPANT_CONTROL_FIRST..=BTN_OCCUPANT_CONTROL_LAST if self.occupant.is_some() => {
                if let Some(w) = self.window {
                    host.occupant_control(w, id);
                }
            }
            // Literal reading of "after any other handled button": every
            // button not returning 1 above, unlisted IDs included, takes the
            // common tail (BW7).
            _ => {}
        }
        self.refresh_mode(host);
        if let Some(w) = self.window {
            host.update_values(w);
        }
        false
    }

    /// `DestroySubBudgetWindow` (`0x0077A1A0`, §6). The pause ends before
    /// the window is ended.
    pub fn destroy<H: BudgetWindowHost + ?Sized>(&mut self, host: &mut H) {
        if self.selector == SELECTOR_NEIGHBORS {
            for &deal in &self.deals {
                host.add_ref_deal(deal);
                if host.deal_flags(deal) & 1 != 0 {
                    host.deal_slot34(deal);
                }
                host.release_deal(deal);
            }
            host.set_neighbor_ui_disable(false);
            host.mark_deals_dirty();
        }
        // "In every mode": also when CreateSubBudgetWindow failed and no
        // init (so no HiddenPause) ran; literal reading (BW6).
        host.hidden_resume();
        end_dialog(host, self.window);
    }
}
