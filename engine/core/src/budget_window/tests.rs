// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Tests from budget-window.md §9 plus the routing edge cases of §2-§7.

use std::collections::HashMap;

use super::ids::*;
use super::*;

const WIN: WinRef = 77;

/// A recording host. `script` is the message sequence `do_modal_win` delivers.
#[derive(Default)]
struct Rec {
    log: Vec<String>,
    create_fails: bool,
    modal_flag: bool,
    is_generator: bool,
    script: Vec<SubBudgetMsg>,
    funding: HashMap<DeptRef, f32>,
    group: Vec<DeptRef>,
    ordinances: Vec<(OrdRef, bool)>,
    deals: Vec<(DealRef, u32)>,
    restore: Vec<IndividualRestoreInfo>,
    indiv: HashMap<u32, OccRef>,
    paused: i32,
    ui_disabled: bool,
    filter: bool,
    results: Vec<Option<bool>>,
}

impl Rec {
    fn l(&mut self, s: String) {
        self.log.push(s);
    }
    fn count(&self, prefix: &str) -> usize {
        self.log.iter().filter(|s| s.starts_with(prefix)).count()
    }
    fn pos(&self, s: &str) -> usize {
        self.log
            .iter()
            .position(|x| x == s)
            .unwrap_or_else(|| panic!("{s} not in {:?}", self.log))
    }
}

impl BudgetWindowHost for Rec {
    fn create_window_generator(&mut self) -> Option<WinRef> {
        self.l("create".into());
        (!self.create_fails).then_some(WIN)
    }
    fn add_ref_win(&mut self, w: WinRef) {
        self.l(format!("addref {w}"));
    }
    fn release_win(&mut self, w: WinRef) {
        self.l(format!("release {w}"));
    }
    fn set_generator_flag(&mut self, _: WinRef, f: u32, on: bool) {
        self.l(format!("gflag {f:#x} {on}"));
    }
    fn set_generator_win_proc(&mut self, _: WinRef) {
        self.l("winproc".into());
    }
    fn query_generator(&mut self, _: WinRef) -> bool {
        self.is_generator
    }
    fn set_win_flag(&mut self, _: WinRef, f: u32, on: bool) {
        self.l(format!("wflag {f:#x} {on}"));
    }
    fn win_flag(&mut self, _: WinRef, f: u32) -> bool {
        f == WIN_FLAG_MODAL && self.modal_flag
    }
    fn set_win_id(&mut self, _: WinRef, id: u32) {
        self.l(format!("id {id:#x}"));
    }
    fn show_win(&mut self, _: WinRef) {
        self.l("show".into());
    }
    fn pull_to_front(&mut self, _: WinRef) {
        self.l("front".into());
    }
    fn end_modal_via_parent(&mut self, _: WinRef) {
        self.l("end_modal".into());
    }
    fn remove_from_parent(&mut self, _: WinRef) {
        self.l("remove_from_parent".into());
    }
    fn remove_child(&mut self, _: WinRef, c: WinRef) {
        self.l(format!("remove_child {c}"));
    }
    fn set_arrow_visible(&mut self, _: WinRef, b: u32, v: bool) {
        self.l(format!("arrow {b:#x} {v}"));
    }
    fn restore_window_location(&mut self, _: WinRef) {
        self.l("restore_loc".into());
    }
    fn save_window_location(&mut self, _: WinRef) {
        self.l("save_loc".into());
    }
    fn do_modal_win(&mut self, _: WinRef, dialog: &mut SubBudgetWindow) {
        self.l("modal".into());
        let script = std::mem::take(&mut self.script);
        for m in script {
            let r = dialog.do_win_msg(self, m);
            self.results.push(r);
        }
    }
    fn focus_view3d(&mut self) {
        self.l("focus3d".into());
    }
    fn update_balance_sheet(&mut self) {
        self.l("balance".into());
    }
    fn filter_ui_command(&mut self, _: u32) -> bool {
        self.filter
    }
    fn hidden_pause(&mut self) {
        self.paused += 1;
        self.l("pause".into());
    }
    fn hidden_resume(&mut self) {
        self.paused -= 1;
        self.l("resume".into());
    }
    fn load_background(&mut self, _: WinRef, t: u32, g: u32, i: u32) {
        self.l(format!("bg {t:#x} {g:#x} {i:#x}"));
    }
    fn init_ordinance_budget(&mut self, _: WinRef) -> Vec<(OrdRef, bool)> {
        self.ordinances.clone()
    }
    fn update_ordinance_budget(&mut self, _: WinRef, rows: &[OrdinanceRow], v: ScrollView) {
        self.l(format!("upd_ord {} {}", rows.len(), v.a));
    }
    fn set_ordinance_on(&mut self, o: OrdRef, on: bool) {
        self.l(format!("ord_on {o} {on}"));
    }
    fn set_neighbor_ui_disable(&mut self, on: bool) {
        self.ui_disabled = on;
        self.l(format!("ui_disable {on}"));
    }
    fn init_neighbor_budget(&mut self, _: WinRef) -> Vec<DealRef> {
        self.deals.iter().map(|d| d.0).collect()
    }
    fn update_neighbor_budget(&mut self, _: WinRef, d: &[DealRef], v: ScrollView) {
        self.l(format!("upd_nb {} {} {}", d.len(), v.a, v.b));
    }
    fn rewrite_selected_deal_rows(&mut self, _: WinRef, c: u32) {
        self.l(format!("rewrite {c:#x}"));
    }
    fn add_ref_deal(&mut self, d: DealRef) {
        self.l(format!("deal_addref {d}"));
    }
    fn release_deal(&mut self, d: DealRef) {
        self.l(format!("deal_release {d}"));
    }
    fn deal_flags(&mut self, d: DealRef) -> u32 {
        self.deals.iter().find(|x| x.0 == d).map_or(0, |x| x.1)
    }
    fn deal_slot34(&mut self, d: DealRef) {
        self.l(format!("deal34 {d}"));
    }
    fn mark_deals_dirty(&mut self) {
        self.l("deals_dirty".into());
    }
    fn department_budgets_in_group(&mut self, g: u32) -> Vec<DeptRef> {
        self.l(format!("group {g:#x}"));
        self.group.clone()
    }
    fn funding_percentage(&mut self, d: DeptRef) -> f32 {
        self.funding[&d]
    }
    fn set_funding_percentage(&mut self, d: DeptRef, f: f32, a: u32) {
        self.funding.insert(d, f);
        self.l(format!("fund {d} {f} {a}"));
    }
    fn add_department_row(&mut self, _: WinRef, _: u32, d: DeptRef) -> u32 {
        1000 + d as u32
    }
    fn update_values(&mut self, _: WinRef) {
        self.l("values".into());
    }
    fn init_individual_budget(&mut self, _: WinRef, o: OccRef) -> Vec<IndividualRestoreInfo> {
        self.l(format!("init_indiv {o}"));
        self.restore.clone()
    }
    fn indiv_item_for_slider(&mut self, s: u32) -> Option<OccRef> {
        self.indiv.get(&s).copied()
    }
    fn set_occupant_funding_level(&mut self, o: OccRef, w: u32, lv: i32) {
        self.l(format!("occ {o} {w} {lv}"));
    }
    fn update_individual_budget(&mut self, _: WinRef) {
        self.l("upd_indiv".into());
    }
    fn occupant_control(&mut self, _: WinRef, b: u32) {
        self.l(format!("occ_ctrl {b:#x}"));
    }
    fn do_individual_budget_window(&mut self, _: WinRef, b: u32) {
        self.l(format!("nested {b:#x}"));
    }
}

fn group_host() -> Rec {
    let mut h = Rec {
        group: vec![1, 2, 3],
        ..Default::default()
    };
    h.funding.extend([(1, 0.5f32), (2, 0.75), (3, 1.0)]);
    h
}

/// A dialog created and initialised outside the panel.
fn open(h: &mut Rec, selector: u32, occ: Option<OccRef>) -> SubBudgetWindow {
    let mut d = SubBudgetWindow::new();
    assert_eq!(d.create(h, selector, 5, occ), Some(WIN));
    d.do_win_msg(h, SubBudgetMsg::Init);
    d
}

#[test]
fn panel_command_selector_table() {
    use PanelCommand::*;
    let cases = [
        (0xAA3AC401, SubBudget(1)),
        (0xAA3AC509, SubBudget(1)),
        (0xAA3AC40A, SubBudget(2)),
        (0xAA3AC502, SubBudget(2)),
        (0xAA3AC403, SubBudget(0x0A5A72D1)),
        (0xAA3AC405, SubBudget(0xAA369059)),
        (0xAA3AC504, SubBudget(0xAA369059)),
        (0xAA3AC406, SubBudget(0x4A357B40)),
        (0xAA3AC507, SubBudget(0x6A357B7F)),
        (0xAA3AC408, SubBudget(0x4A357EAF)),
        (0xAA3AC50B, SubBudget(0x6A357B96)),
        (0xAA3AC40C, SubBudget(0xEA597195)),
        (0xAA3AC500, DisplayTaxesWindow),
        (0xAA3AC40D, DisplayLoanWindow),
        (0xAA3AC50E, SetFlag18cThenSlot6(true)),
        (0x4A8879DB, SetFlag18cThenSlot6(false)),
        (0x0A4C3354, RemoveLoanWindow),
        (0xAA4C339C, ScrollLoanList(0xAA4C339C)),
        (0xAA4C33E6, TakeLoan),
    ];
    for (id, want) in cases {
        assert_eq!(panel_command(id), Some(want), "{id:#x}");
    }
    assert_eq!(panel_command(0xAA3AC410), None);
    assert_eq!(panel_command(0xAA3AC600), None);
}

#[test]
fn create_sequence_and_failure() {
    let mut h = Rec::default();
    let mut d = SubBudgetWindow::new();
    assert_eq!(d.create(&mut h, 2, 9, Some(4)), Some(WIN));
    let want = [
        "create",
        "addref 77",
        "gflag 0x1 true",
        "gflag 0x2 false",
        "gflag 0x10000 false",
        "gflag 0x20 false",
        "gflag 0x40 false",
        "gflag 0x1000 true",
        "gflag 0x2000 false",
        "gflag 0x4000 false",
        "winproc",
        "wflag 0x100 true",
        "wflag 0x10000 false",
        "wflag 0x4 true",
        "wflag 0x800 false",
        "id 0x423278f",
    ];
    assert_eq!(h.log, want);
    assert_eq!(
        (d.selector(), d.context(), d.occupant(), d.window()),
        (2, 9, Some(4), Some(WIN))
    );

    let mut h = Rec {
        create_fails: true,
        ..Default::default()
    };
    let mut d = SubBudgetWindow::new();
    assert_eq!(d.create(&mut h, 2, 9, None), None);
    assert_eq!(h.log, ["create"]);
    assert_eq!((d.selector(), d.window()), (0, None));
}

// §9.1
#[test]
fn opening_while_held_does_nothing() {
    let mut h = group_host();
    let mut p = BudgetPanel::new(3);
    p.held = Some(SubBudgetWindow::new());
    p.do_sub_budget_window(&mut h, 0xAA369059);
    assert!(h.log.is_empty());
    assert!(p.is_holding_sub_budget());
}

// §9.2 and the §2 / §6 order.
#[test]
fn modal_run_pauses_and_refreshes_once() {
    let mut h = Rec {
        is_generator: true,
        modal_flag: true,
        script: vec![SubBudgetMsg::Init],
        ..group_host()
    };
    let mut p = BudgetPanel::new(3);
    assert_eq!(p.on_command(&mut h, 0xAA3AC406), CommandOutcome::SubBudgetRun);
    // While the modal loop ran the pause was held; after close it is not.
    assert_eq!(h.paused, 0);
    assert_eq!(h.count("pause"), 1);
    assert_eq!(h.count("balance"), 1);
    assert!(!p.is_holding_sub_budget());
    let order = [
        "show",
        "front",
        "restore_loc",
        "modal",
        "pause",
        "focus3d",
        "save_loc",
        "resume",
        "end_modal",
        "balance",
    ];
    let idx: Vec<usize> = order.iter().map(|s| h.pos(s)).collect();
    assert!(idx.windows(2).all(|w| w[0] < w[1]), "{:?}", h.log);
    // Step 4 flags: 0x800 on, front, 0x800 off, 0x100 on, generator 0x1000/0x2000/0x4000.
    let f = h.pos("front");
    assert_eq!(h.log[f - 1], "wflag 0x800 true");
    assert_eq!(
        &h.log[f + 1..f + 6],
        [
            "wflag 0x800 false",
            "wflag 0x100 true",
            "gflag 0x1000 true",
            "gflag 0x2000 true",
            "gflag 0x4000 true"
        ]
    );
}

#[test]
fn modal_run_create_failure_still_destroys_and_refreshes() {
    let mut h = Rec {
        create_fails: true,
        ..Default::default()
    };
    let mut p = BudgetPanel::new(0);
    p.do_sub_budget_window(&mut h, 1);
    assert_eq!(h.log, ["create", "resume", "balance"]);
}

#[test]
fn tutorial_filter_swallows() {
    let mut h = Rec {
        filter: true,
        ..Default::default()
    };
    let mut p = BudgetPanel::new(0);
    assert_eq!(p.on_command(&mut h, 0xAA3AC401), CommandOutcome::Swallowed);
    assert_eq!(p.on_command(&mut h, 0xAA3AC400), CommandOutcome::Swallowed);
    assert!(h.log.is_empty());
    h.filter = false;
    assert_eq!(
        p.on_command(&mut h, 0xAA3AC40D),
        CommandOutcome::Panel(PanelCommand::DisplayLoanWindow)
    );
    assert_eq!(p.on_command(&mut h, 0x1234), CommandOutcome::Unknown);
}

// §9.3
#[test]
fn group_slider_then_cancel_restores_start_fractions() {
    let mut h = group_host();
    let mut d = open(&mut h, GROUP_PROTECTION, None);
    assert_eq!(d.sliders().len(), 3);
    assert_eq!(d.saved_fractions(), [(0.5, 1), (0.75, 2), (1.0, 3)]);
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 1002,
            position: 30,
        },
    );
    assert_eq!(h.funding[&2], 0.3f32);
    assert_eq!(h.log.last().unwrap(), "values");
    // An unknown slider still refreshes the values.
    h.log.clear();
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 5,
            position: 10,
        },
    );
    assert_eq!(h.log, ["values"]);
    // A department moved elsewhere by redistribution is restored too.
    h.funding.insert(3, 0.1);
    h.log.clear();
    assert_eq!(d.do_win_msg(&mut h, SubBudgetMsg::Button(BTN_CANCEL)), Some(true));
    assert_eq!(
        h.log,
        ["fund 1 0.5 0", "fund 2 0.75 0", "fund 3 1 0", "remove_from_parent"]
    );
}

#[test]
fn slider_reentry_guard_is_shared() {
    let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut h = group_host();
    let mut d = SubBudgetWindow::with_slider_guard(guard.clone());
    d.create(&mut h, GROUP_UTILITIES, 0, None);
    d.do_win_msg(&mut h, SubBudgetMsg::Init);
    h.log.clear();
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 1001,
            position: 30,
        },
    );
    assert!(h.log.is_empty());
    guard.store(false, std::sync::atomic::Ordering::SeqCst);
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 1001,
            position: 30,
        },
    );
    assert_eq!(h.log, ["fund 1 0.3 0", "values"]);
    assert!(!guard.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn ok_keeps_edits() {
    let mut h = group_host();
    let mut d = open(&mut h, GROUP_UTILITIES, None);
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 1001,
            position: 0,
        },
    );
    for ok in [BTN_OK, BTN_OK_ALT, BTN_END, BTN_END_ALT] {
        h.log.clear();
        assert!(d.on_button_activated(&mut h, ok));
        assert_eq!(h.log, ["remove_from_parent"]);
    }
    assert_eq!(h.funding[&1], 0.0);
}

// §9.4
#[test]
fn ordinance_apply_calls_only_changed_rows() {
    let mut h = Rec {
        ordinances: vec![(10, false), (11, true), (12, false)],
        ..Default::default()
    };
    let mut d = open(&mut h, 1, None);
    assert!(h.log.contains(&"bg 0x856ddbac 0x46a006b0 0x140155f2".to_string()));
    assert!(d.set_ordinance_edited(0, true));
    assert!(d.set_ordinance_edited(1, false));
    assert!(d.set_ordinance_edited(2, false)); // unchanged
    assert!(!d.set_ordinance_edited(3, true));
    h.log.clear();
    assert_eq!(
        d.do_win_msg(&mut h, SubBudgetMsg::Button(BTN_ORDINANCE_APPLY)),
        Some(false)
    );
    assert_eq!(h.count("ord_on"), 2);
    assert_eq!(&h.log[..3], ["ord_on 10 true", "ord_on 11 false", "remove_from_parent"]);

    let mut h = Rec {
        ordinances: vec![(10, false), (11, true)],
        ..Default::default()
    };
    let mut d = open(&mut h, 1, None);
    d.set_ordinance_edited(0, true);
    d.set_ordinance_edited(1, false);
    d.on_button_activated(&mut h, BTN_CANCEL);
    assert_eq!(h.count("ord_on"), 0);
}

// §9.5
#[test]
fn neighbour_mode_disables_ui_and_marks_dirty() {
    let mut h = Rec {
        deals: vec![(30, 1), (31, 0), (32, 3)],
        ..Default::default()
    };
    let mut p = BudgetPanel::new(0);
    h.script = vec![SubBudgetMsg::Init, SubBudgetMsg::Button(BTN_OK)];
    p.do_sub_budget_window(&mut h, 2);
    assert!(!h.ui_disabled);
    let on = h.pos("ui_disable true");
    let off = h.pos("ui_disable false");
    assert!(h.pos("modal") < on && on < off);
    let tail = &h.log[h.pos("deal_addref 30")..];
    assert_eq!(
        &tail[..12],
        [
            "deal_addref 30",
            "deal34 30",
            "deal_release 30",
            "deal_addref 31",
            "deal_release 31",
            "deal_addref 32",
            "deal34 32",
            "deal_release 32",
            "ui_disable false",
            "deals_dirty",
            "resume",
            "remove_from_parent"
        ]
    );
}

#[test]
fn neighbour_combo_and_other_mode_combo() {
    let mut h = Rec {
        deals: vec![(30, 0)],
        ..Default::default()
    };
    let mut d = open(&mut h, 2, None);
    h.log.clear();
    assert_eq!(d.do_win_msg(&mut h, SubBudgetMsg::Combo(0x55)), None);
    assert_eq!(h.log, ["rewrite 0x55", "upd_nb 1 0 0"]);
    let mut h = group_host();
    let mut d = open(&mut h, GROUP_UTILITIES, None);
    h.log.clear();
    d.do_win_msg(&mut h, SubBudgetMsg::Combo(0x55));
    assert!(h.log.is_empty());
}

// §9.6 and BW5's literal reading.
#[test]
fn keys_close_only_as_specified() {
    let mut h = group_host();
    let mut d = open(&mut h, GROUP_UTILITIES, None);
    d.set_inner_child(Some(88));
    h.log.clear();
    assert_eq!(
        d.do_win_msg(
            &mut h,
            SubBudgetMsg::KeyDown {
                key: KEY_ENTER,
                modifiers: 0
            }
        ),
        None
    );
    assert_eq!(
        d.do_win_msg(
            &mut h,
            SubBudgetMsg::KeyDown {
                key: KEY_0X73,
                modifiers: 3
            }
        ),
        None
    );
    assert_eq!(
        d.do_win_msg(
            &mut h,
            SubBudgetMsg::KeyDown {
                key: 0x41,
                modifiers: 4
            }
        ),
        None
    );
    assert!(h.log.is_empty());
    assert_eq!(
        d.do_win_msg(
            &mut h,
            SubBudgetMsg::KeyDown {
                key: KEY_ESC,
                modifiers: 0
            }
        ),
        Some(true)
    );
    assert_eq!(h.log, ["remove_child 88", "release 88", "remove_from_parent"]);
    assert_eq!(d.inner_child(), None);
    for key in [KEY_ENTER, KEY_0X73] {
        h.log.clear();
        assert_eq!(
            d.do_win_msg(&mut h, SubBudgetMsg::KeyDown { key, modifiers: 4 }),
            Some(true)
        );
        assert_eq!(h.log, ["remove_from_parent"]);
    }
}

#[test]
fn end_dialog_branches() {
    let mut h = Rec::default();
    assert!(!end_dialog(&mut h, None));
    assert!(end_dialog(&mut h, Some(1)));
    h.modal_flag = true;
    assert!(end_dialog(&mut h, Some(1)));
    assert_eq!(h.log, ["remove_from_parent", "end_modal"]);
}

#[test]
fn close_child_buttons_keep_dialog_and_refresh() {
    let mut h = Rec {
        ordinances: vec![(1, true)],
        ..Default::default()
    };
    let mut d = open(&mut h, 1, None);
    d.set_inner_child(Some(9));
    h.log.clear();
    assert!(!d.on_button_activated(&mut h, BTN_CLOSE_CHILD));
    assert_eq!(h.log, ["remove_child 9", "release 9", "upd_ord 1 0", "values"]);
    h.log.clear();
    assert!(!d.on_button_activated(&mut h, BTN_CLOSE_CHILD_ALT));
    assert_eq!(h.log, ["upd_ord 1 0", "values"]);
    for b in [BTN_CE, BTN_CF] {
        h.log.clear();
        assert!(!d.on_button_activated(&mut h, b));
        assert_eq!(h.log, ["remove_from_parent", "upd_ord 1 0", "values"]);
    }
}

#[test]
fn scroll_by_two_page_nine_with_arrows() {
    let mut h = Rec {
        ordinances: vec![(1, true)],
        ..Default::default()
    };
    let mut d = open(&mut h, 1, None);
    d.set_list_len(false, 14); // max first row 5
    let mut seen = vec![];
    for _ in 0..4 {
        d.on_button_activated(&mut h, BTN_LIST_A_DOWN);
        seen.push(d.list_a().pos);
    }
    assert_eq!(seen, [2, 4, 5, 5]);
    h.log.clear();
    d.on_button_activated(&mut h, BTN_LIST_A_UP);
    assert_eq!(h.log, ["arrow 0x451 true", "arrow 0x452 true", "upd_ord 1 3", "values"]);
    d.on_button_activated(&mut h, BTN_LIST_A_UP);
    h.log.clear();
    d.on_button_activated(&mut h, BTN_LIST_A_UP);
    assert_eq!(d.list_a().pos, 0);
    assert_eq!(&h.log[..2], ["arrow 0x451 false", "arrow 0x452 true"]);
    // List B is independent; a list that fits never scrolls.
    d.set_list_len(true, 9);
    h.log.clear();
    d.on_button_activated(&mut h, BTN_LIST_B_DOWN);
    assert_eq!(d.list_b().pos, 0);
    assert_eq!(&h.log[..2], ["arrow 0x453 false", "arrow 0x454 false"]);
    d.set_list_len(false, 3);
    assert_eq!(d.list_a().pos, 0);
}

#[test]
fn occupant_mode_sliders_restore_and_buttons() {
    let restore = vec![IndividualRestoreInfo {
        occupant: 40,
        level0: 7,
        level1: 8,
    }];
    let mut h = Rec {
        restore,
        ..Default::default()
    };
    h.indiv.extend([(0x0ABCE4FF, 40), (0x0ABCE500, 40)]);
    let mut d = open(&mut h, GROUP_UTILITIES, Some(40));
    assert_eq!(d.mode(), SubBudgetMode::Individual(40));
    assert!(h.log.contains(&"init_indiv 40".to_string()));
    h.log.clear();
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 0x0ABCE4FF,
            position: 55,
        },
    );
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 0x0ABCE500,
            position: 66,
        },
    );
    d.do_win_msg(
        &mut h,
        SubBudgetMsg::Slider {
            slider: 0x0ABCE501,
            position: 1,
        },
    );
    assert_eq!(h.log, ["occ 40 0 55", "upd_indiv", "occ 40 1 66", "upd_indiv"]);
    h.log.clear();
    d.on_button_activated(&mut h, 0x0ABCDE05);
    d.on_button_activated(&mut h, 0x0ABCE6AA);
    assert_eq!(
        h.log,
        [
            "occ_ctrl 0xabcde05",
            "upd_indiv",
            "values",
            "nested 0xabce6aa",
            "upd_indiv",
            "values"
        ]
    );
    h.log.clear();
    d.on_button_activated(&mut h, BTN_CANCEL);
    assert_eq!(h.log, ["occ 40 0 7", "occ 40 1 8", "remove_from_parent"]);
}

#[test]
fn occupant_controls_ignored_outside_occupant_mode() {
    let mut h = group_host();
    let mut d = open(&mut h, GROUP_UTILITIES, None);
    h.log.clear();
    assert!(!d.on_button_activated(&mut h, 0x0ABCDE05));
    assert_eq!(h.log, ["values"]);
}

#[test]
fn init_modes_and_pause_first() {
    let mut h = Rec {
        deals: vec![(1, 0)],
        ..Default::default()
    };
    let d = open(&mut h, 2, Some(9));
    assert_eq!(d.mode(), SubBudgetMode::Neighbors);
    assert_eq!(
        h.log[h.log.len() - 3..],
        ["pause", "bg 0x856ddbac 0x46a006b0 0x140155d2", "ui_disable true"]
    );
    let mut h = group_host();
    let d = open(&mut h, GROUP_PARKS_LANDMARKS, None);
    assert_eq!(d.mode(), SubBudgetMode::Group(GROUP_PARKS_LANDMARKS));
    assert_eq!(h.log[h.log.len() - 2..], ["pause", "group 0x6a357b96"]);
    assert_eq!(d.sliders()[0], SliderInfo { slider: 1001, dept: 1 });
    // Shutdown and other messages: no action, return unspecified.
    let mut d = d;
    h.log.clear();
    assert_eq!(d.do_win_msg(&mut h, SubBudgetMsg::Shutdown), None);
    assert_eq!(d.do_win_msg(&mut h, SubBudgetMsg::Other(3)), None);
    assert!(h.log.is_empty());
}

// §7
#[test]
fn panel_refreshes_at_most_once_per_tick() {
    let mut h = Rec::default();
    let mut p = BudgetPanel::new(0);
    p.do_message(&mut h, PANEL_MSG_TICK);
    assert_eq!(h.count("balance"), 0);
    for _ in 0..5 {
        p.do_message(&mut h, PANEL_MSG_MONEY_A);
        p.do_message(&mut h, PANEL_MSG_MONEY_B);
    }
    assert!(p.is_dirty());
    p.do_message(&mut h, PANEL_MSG_TICK);
    p.do_message(&mut h, PANEL_MSG_TICK);
    p.do_message(&mut h, 0x1234);
    assert_eq!(h.count("balance"), 1);
    assert!(!p.is_dirty());
}

#[test]
fn slider_fraction_rounding() {
    let mut h = group_host();
    let mut d = open(&mut h, GROUP_UTILITIES, None);
    for pos in [0, 1, 7, 29, 30, 99, 100, 120] {
        d.do_win_msg(
            &mut h,
            SubBudgetMsg::Slider {
                slider: 1001,
                position: pos,
            },
        );
        assert_eq!(h.funding[&1], (f64::from(pos) * 0.01) as f32);
    }
}
