// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The budget window family (`docs/specs/budget-window.md`): the sub-budget
//! dialog `cSC4SubBudgetWindow` and the parts of the budget panel
//! `cSC4BudgetUIWinProc` that open it and coalesce its refreshes. Section
//! references (`§n`) point at that spec.
//!
//! The core is host-independent: every window, simulator, budget, ordinance,
//! neighbour and occupant call goes through [`BudgetWindowHost`] with typed
//! arguments, in the order the spec gives. The per-row layout routines
//! (`initNeighborBudget`, `updateNeighborBudget`, `initOrdinanceBudget`, the
//! `add_*` helpers) are specified by contract only (§8, BW1); they are host
//! callbacks here that receive the data the dialog owns and return what the
//! dialog keeps (slider IDs, ordinance states, restore records).
//!
//! What is not modelled because the spec leaves it open is listed in
//! `docs/lanes/core-budgetwindow4.md` and filed as questions BW6 onwards.

pub mod dialog;
pub mod ids;
pub mod panel;

#[cfg(test)]
mod tests;

pub use dialog::{
    end_dialog, IndividualRestoreInfo, OrdinanceRow, ScrollList, ScrollView, SliderInfo, SubBudgetMode, SubBudgetMsg,
    SubBudgetWindow,
};
pub use panel::{panel_command, BudgetPanel, CommandOutcome, PanelCommand};

/// An opaque handle to a host-side `cIGZWin` (or a window generator's window).
pub type WinRef = u64;
/// An opaque handle to a `cISC4DepartmentBudget`.
pub type DeptRef = u64;
/// An opaque handle to an ordinance row's ordinance.
pub type OrdRef = u64;
/// An opaque handle to a neighbour deal (`cISC4NeighborDeal`-like object).
pub type DealRef = u64;
/// An opaque handle to an occupant (individual-budget mode, `+0x24`).
pub type OccRef = u64;

/// Everything the budget window family calls outside itself. Method order in
/// the spec is preserved by the callers; implementations only perform the
/// call they are named after.
pub trait BudgetWindowHost {
    // ---- §3 window generator and window (cIGZWin) ----
    /// Create the window generator (class `0x4386D516`, interface
    /// `0x5386D516`) and return its `cIGZWin`, or `None` on failure. The
    /// returned reference is the one the dialog holds at `+0x0C`.
    fn create_window_generator(&mut self) -> Option<WinRef>;
    /// `AddRef` on a window (the `*out` reference of `CreateSubBudgetWindow`).
    fn add_ref_win(&mut self, win: WinRef);
    /// Release a window reference held by the core (the inner child at `+0x14`).
    fn release_win(&mut self, win: WinRef);
    /// Set a generator flag (generator interface `0x5386D516`).
    fn set_generator_flag(&mut self, win: WinRef, flag: u32, on: bool);
    /// Set the generator's window procedure (generator `+0x1C`) to the dialog.
    fn set_generator_win_proc(&mut self, win: WinRef);
    /// Whether the window answers interface `0x5386D516` (§2 step 4).
    fn query_generator(&mut self, win: WinRef) -> bool;
    /// `cIGZWin::SetFlag`.
    fn set_win_flag(&mut self, win: WinRef, flag: u32, on: bool);
    /// Read a `cIGZWin` flag (`end_dialog` tests `0x1000`).
    fn win_flag(&mut self, win: WinRef, flag: u32) -> bool;
    /// `cIGZWin::SetID` (slot 64).
    fn set_win_id(&mut self, win: WinRef, id: u32);
    /// `ShowWindow` on the dialog window (§2 step 4).
    fn show_win(&mut self, win: WinRef);
    /// `PullToFront` (§2 step 4).
    fn pull_to_front(&mut self, win: WinRef);
    /// `end_dialog`, modal branch: ask the window's parent to end the modal
    /// loop (parent `+0x24`, code `0xF`, the window twice).
    fn end_modal_via_parent(&mut self, win: WinRef);
    /// `end_dialog`, modeless branch: remove the window from its parent (`+0x38`).
    fn remove_from_parent(&mut self, win: WinRef);
    /// Remove the inner child held at `+0x14` from the dialog window.
    fn remove_child(&mut self, win: WinRef, child: WinRef);
    /// Show or hide one of the scroll arrow buttons `0x451`..`0x454`.
    fn set_arrow_visible(&mut self, win: WinRef, button: u32, visible: bool);

    // ---- §2 the panel's modal run ----
    /// `WinLocationSaver.RestoreWindowLocation(win, 1)`.
    fn restore_window_location(&mut self, win: WinRef);
    /// `WinLocationSaver.SaveWindowLocation(win)`.
    fn save_window_location(&mut self, win: WinRef);
    /// `DoModalWin` (window manager `+0xA4`): run the modal loop, delivering
    /// the dialog's messages to `dialog.do_win_msg`, until it is ended.
    fn do_modal_win(&mut self, win: WinRef, dialog: &mut SubBudgetWindow);
    /// Give focus back to the 3D view window (manager `+0x94`).
    fn focus_view3d(&mut self);
    /// The panel's `UpdateBalanceSheetInfo` (`0x00767990`).
    fn update_balance_sheet(&mut self);
    /// `nsSC4TutorialSystem::FilterUICommandMessage`: true = swallow.
    fn filter_ui_command(&mut self, command: u32) -> bool;

    // ---- §4 simulator ----
    /// `cISC4Simulator::HiddenPause` (slot 10).
    fn hidden_pause(&mut self);
    /// `cISC4Simulator::HiddenResume` (slot 13).
    fn hidden_resume(&mut self);
    /// Load the mode's background image (type, group, instance).
    fn load_background(&mut self, win: WinRef, type_id: u32, group: u32, instance: u32);

    // ---- §4.1 ordinances ----
    /// `initOrdinanceBudget` contract: build one row per ordinance and return
    /// each row's ordinance and its current on/off state.
    fn init_ordinance_budget(&mut self, win: WinRef) -> Vec<(OrdRef, bool)>;
    /// `updateOrdinanceBudget`: redraw the rows from the dialog's state.
    fn update_ordinance_budget(&mut self, win: WinRef, rows: &[OrdinanceRow], view: ScrollView);
    /// `OrdinanceSimulator.SetOrdinanceOn` (slot 13).
    fn set_ordinance_on(&mut self, ordinance: OrdRef, on: bool);

    // ---- §4.2 neighbour deals ----
    /// `NeighborsSimulator.SetUIDisableFlag`.
    fn set_neighbor_ui_disable(&mut self, on: bool);
    /// `initNeighborBudget` contract after the UI-disable call:
    /// `ProposeDeals(list, true)` and the rows; returns the held deals
    /// (vector `+0x68`).
    fn init_neighbor_budget(&mut self, win: WinRef) -> Vec<DealRef>;
    /// `updateNeighborBudget`.
    fn update_neighbor_budget(&mut self, win: WinRef, deals: &[DealRef], view: ScrollView);
    /// Combo box changed in neighbour mode: rewrite the selected deal rows
    /// (vector `+0x50`, 24-byte entries; content is §8 open work).
    fn rewrite_selected_deal_rows(&mut self, win: WinRef, combo: u32);
    fn add_ref_deal(&mut self, deal: DealRef);
    fn release_deal(&mut self, deal: DealRef);
    /// Deal slot 36: flags.
    fn deal_flags(&mut self, deal: DealRef) -> u32;
    /// Deal slot 34 (called when flag bit 0 is set at close).
    fn deal_slot34(&mut self, deal: DealRef);
    /// `NeighborsSimulator.MarkDealsDirty`.
    fn mark_deals_dirty(&mut self);

    // ---- §4 group mode ----
    /// `budget.GetDepartmentBudgetsInGroup(group, list)` (budget slot 31).
    fn department_budgets_in_group(&mut self, group: u32) -> Vec<DeptRef>;
    /// The department's current funding fraction (saved at init).
    fn funding_percentage(&mut self, dept: DeptRef) -> f32;
    /// Department slot 11: `SetFundingPercentage(fraction, arg)`.
    fn set_funding_percentage(&mut self, dept: DeptRef, fraction: f32, arg: u32);
    /// Build one department's slider row; returns its slider window ID.
    fn add_department_row(&mut self, win: WinRef, group: u32, dept: DeptRef) -> u32;
    /// `UpdateValues` (`0x007898A0`): refresh every row's text.
    fn update_values(&mut self, win: WinRef);

    // ---- §4 individual (occupant) mode ----
    /// `initIndividualBudget` contract: build the rows and return the
    /// restore records (vector `+0x44`).
    fn init_individual_budget(&mut self, win: WinRef, occupant: OccRef) -> Vec<IndividualRestoreInfo>;
    /// `get_window_indiv_item`: the occupant a slider belongs to.
    fn indiv_item_for_slider(&mut self, slider: u32) -> Option<OccRef>;
    /// `SetOccupantFundingLevel(occupant, which, level)` (`0x0077BC90`).
    fn set_occupant_funding_level(&mut self, occupant: OccRef, which: u32, level: i32);
    /// `updateIndividualBudget`.
    fn update_individual_budget(&mut self, win: WinRef);
    /// Buttons `0x0ABCDE00`..`0x0ABCDEFF` in occupant mode (BW3, open).
    fn occupant_control(&mut self, win: WinRef, button: u32);
    /// `DoIndividualBudgetWindow` (`0x00786530`) for an occupant row button
    /// `0x0ABCE600`..`0x0ABCE6FF` (the nested dialog; its own steps are not
    /// specified beyond "a second `cSC4SubBudgetWindow` through
    /// `CreateSubBudgetWindow`").
    fn do_individual_budget_window(&mut self, win: WinRef, button: u32);
}
