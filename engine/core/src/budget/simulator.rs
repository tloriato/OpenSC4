// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cISC4BudgetSimulator` (§3.1, §4, §5, §6, §11.1).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use crate::rng::RzRandomSuper;
use crate::seeding::SharedSuper;
use crate::stream::{InStream, OutStream, RefValue, SaveObject};

use super::items::{
    budget_items, change_item_cost, change_item_line, change_item_local_funding, copy_item_properties,
    item_for_purpose, BudgetItem,
};
use super::tax::{difficulty_multiplier, scan_tax, tax_income_of};
use super::tuning::BudgetTuning;
use super::{
    cheat, history, msg, round_half_even, BudgetHost, BudgetMessage, Department, LineItem, CLSID_BUDGET_SIMULATOR,
    CLSID_DEPARTMENT, GROUP_NAME_LTEXT, TAX_GROUPS,
};

/// Record version written (§11.1).
pub const BUDGET_RECORD_VERSION: u16 = 5;

/// `H(v)` (§5.1): values ≥ 2^32 are recorded as 0xFFFFFFFF; otherwise the low 32 bits.
fn history_value(v: i64) -> i32 {
    if v >= (1i64 << 32) {
        -1
    } else {
        v as u32 as i32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaxGroup {
    /// Percent.
    pub rate: f32,
    pub max_rate: f32,
    pub history: [i64; 12],
    pub reserved: i64,
    pub weighted_assessed: i64,
    pub monthly_tax: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bond {
    /// 1..12
    pub due_month: i32,
    pub due_year: i32,
    pub amount: i64,
    pub full_cost: i64,
    pub remaining: i64,
    pub paid: i64,
    pub monthly_payment: i64,
    pub principal_left: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BudgetGroup {
    pub id: u32,
    pub name: Vec<u8>,
    pub total_expense: i64,
    pub total_income: i64,
    pub departments: BTreeSet<u32>,
}

/// The SDK's `LoanInfo` (§6 `GetAllLoans`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoanInfo {
    pub months_remaining: u32,
    pub years_remaining: u32,
    pub total_loan_amount: i64,
    pub remaining_payment: i64,
    pub monthly_payment: i64,
}

#[derive(Clone, Default)]
struct BudgetData {
    initialized: bool,
    loaded_from_save: bool,
    agriculture_rate_fix: bool,
    panel_flag: u32,
    funds: i64,
    shadow_funds: i64,
    total_borrowed: i64,
    last_tax_income: i64,
    ytd_income: i64,
    ytd_expense: i64,
    ytd_tax_income: i64,
    ytd_reserved: [i64; 5],
    ytd_ordinance_income: i64,
    ytd_ordinance_expense: i64,
    ordinance_income_by_month: [i64; 12],
    ordinance_expense_by_month: [i64; 12],
    ytd_bond_payments: i64,
    last_month_income: i64,
    last_month_expense: i64,
    yearly_income: i64,
    yearly_expense: i64,
    tax: [TaxGroup; TAX_GROUPS],
    departments: BTreeMap<u32, Rc<Department>>,
    groups: BTreeMap<u32, BudgetGroup>,
    bonds: Vec<Bond>,
    bond_count: u32,
    risky_money_counter: u32,
    tuning: BudgetTuning,
}

/// A copy of the observable budget state (tests, diagnostics, the C ABI).
#[derive(Clone, Debug, PartialEq)]
pub struct BudgetSnapshot {
    pub initialized: bool,
    pub loaded_from_save: bool,
    pub agriculture_rate_fix: bool,
    pub panel_flag: u32,
    pub funds: i64,
    pub shadow_funds: i64,
    pub total_borrowed: i64,
    pub last_tax_income: i64,
    pub ytd_income: i64,
    pub ytd_expense: i64,
    pub ytd_tax_income: i64,
    pub ytd_reserved: [i64; 5],
    pub ytd_ordinance_income: i64,
    pub ytd_ordinance_expense: i64,
    pub ordinance_income_by_month: [i64; 12],
    pub ordinance_expense_by_month: [i64; 12],
    pub ytd_bond_payments: i64,
    pub last_month_income: i64,
    pub last_month_expense: i64,
    pub yearly_income: i64,
    pub yearly_expense: i64,
    pub tax: [TaxGroup; TAX_GROUPS],
    pub bonds: Vec<Bond>,
    pub bond_count: u32,
    pub department_ids: Vec<u32>,
    pub groups: Vec<BudgetGroup>,
    pub risky_money_counter: u32,
}

/// The budget simulator.
pub struct BudgetSim {
    host: Rc<dyn BudgetHost>,
    token: Cell<u64>,
    me: Weak<BudgetSim>,
    d: RefCell<BudgetData>,
    /// FundsChanged / BondIssued / BondFinished are only posted once `Init`
    /// has acquired the message server (§10.1).
    posting: Cell<bool>,
    /// The budget's file-local `cRZRandomSuper` (foundations §6.2), shared
    /// process-wide when built by [`BudgetSim::process_wide`] (RN3).
    rng: SharedSuper,
}

impl BudgetSim {
    /// A fresh budget simulator (as created by its class factory) with a
    /// generator of its own, seeded with `derive(S, 0x6C09C3D7)` (tests,
    /// conformance vectors, replays).
    pub fn new(host: Rc<dyn BudgetHost>, token: u64) -> Rc<BudgetSim> {
        let session = host.session_seed();
        let key = crate::seeding::keys::BUDGET;
        let (seed, seeds) = (crate::seeding::seed(session, key), crate::seeding::clocks(session, key));
        BudgetSim::with_generator(host, token, Rc::new(RefCell::new(RzRandomSuper::new(seed, &seeds))))
    }

    /// A fresh budget simulator drawing from the process-wide generator
    /// (RN3): seeded once per process with `derive(S, 0x6C09C3D7)` of the
    /// first session, its stream continuing across city loads.
    pub fn process_wide(host: Rc<dyn BudgetHost>, token: u64) -> Rc<BudgetSim> {
        let rng = crate::seeding::process_super(host.session_seed(), crate::seeding::keys::BUDGET);
        BudgetSim::with_generator(host, token, rng)
    }

    /// A fresh budget simulator drawing from `rng`.
    pub fn with_generator(host: Rc<dyn BudgetHost>, token: u64, rng: SharedSuper) -> Rc<BudgetSim> {
        Rc::new_cyclic(|me| BudgetSim {
            host,
            token: Cell::new(token),
            me: me.clone(),
            d: RefCell::new(BudgetData {
                tuning: BudgetTuning::default(),
                ..Default::default()
            }),
            posting: Cell::new(false),
            rng,
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    /// The RiskyMoney generator (shared with every budget drawing from the
    /// same one, RN3).
    pub fn generator(&self) -> &SharedSuper {
        &self.rng
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn host(&self) -> &Rc<dyn BudgetHost> {
        &self.host
    }

    fn me(&self) -> Option<Rc<BudgetSim>> {
        self.me.upgrade()
    }

    fn post_self(&self, msg_type: u32) {
        if self.posting.get() {
            let t = self.token();
            self.host.post(&BudgetMessage {
                msg_type,
                void1: t,
                unknown: t,
                ..Default::default()
            });
        }
    }

    fn departments_snapshot(&self) -> Vec<Rc<Department>> {
        self.d.borrow().departments.values().cloned().collect()
    }

    pub fn tuning(&self) -> BudgetTuning {
        self.d.borrow().tuning.clone()
    }

    fn difficulty(&self) -> u32 {
        self.host.difficulty()
    }

    // ---- lifecycle (§4) ----------------------------------------------------------

    /// `Init()` steps 1, 2, 7 and 8 (the GZCOM registrations of steps 3 to 6
    /// and 9 are the shim's). Does nothing if already initialized.
    pub fn init(&self, tuning: BudgetTuning) -> bool {
        if self.d.borrow().initialized {
            return true;
        }
        let diff = self.difficulty();
        let (new_city, fix) = {
            let mut d = self.d.borrow_mut();
            d.initialized = true;
            d.tuning = tuning;
            (!d.loaded_from_save, d.agriculture_rate_fix)
        };
        self.posting.set(true);
        if new_city {
            let (funds, rate, max_rate) = {
                let mut d = self.d.borrow_mut();
                let funds = BudgetTuning::by_difficulty(&d.tuning.initial_funds, diff);
                let rate = BudgetTuning::by_difficulty(&d.tuning.initial_tax_rate, diff);
                let max_rate = d.tuning.max_tax_rate;
                d.funds = funds;
                d.shadow_funds = funds;
                (funds, rate, max_rate)
            };
            self.post_self(msg::FUNDS_CHANGED);
            {
                let mut d = self.d.borrow_mut();
                for g in d.tax.iter_mut() {
                    *g = TaxGroup {
                        rate,
                        max_rate,
                        ..Default::default()
                    };
                }
            }
            for i in 0..TAX_GROUPS as u32 {
                self.host.set_history_queue_length(history::TAX_RATE_BASE + i, 365);
            }
            self.host.record_history(history::FUNDS, (funds / 1000) as i32);
        }
        if fix {
            let mut d = self.d.borrow_mut();
            d.tax[8].rate = BudgetTuning::by_difficulty(&d.tuning.initial_tax_rate, diff);
        }
        true
    }

    /// `Shutdown()` (core part of §4.2): shuts down and releases every
    /// department; funds, bonds, groups and tax data are kept.
    pub fn shutdown(&self) -> bool {
        if !self.d.borrow().initialized {
            return true;
        }
        let depts = {
            let mut d = self.d.borrow_mut();
            d.initialized = false;
            std::mem::take(&mut d.departments)
        };
        self.posting.set(false);
        for (_, dept) in depts {
            dept.shutdown();
            dept.set_budget(None);
            self.host.release(dept.token());
        }
        true
    }

    pub fn is_initialized(&self) -> bool {
        self.d.borrow().initialized
    }

    /// `DoMessage` for everything but cheats (§4.3). Returns the result.
    pub fn handle_message(&self, msg_type: u32, data1: i64) -> bool {
        match msg_type {
            msg::SIM_BEGIN => {
                self.post_self(msg::FUNDS_CHANGED);
                true
            }
            msg::SIM_NEW_DAY => {
                self.simulate_day();
                true
            }
            msg::SIM_NEW_MONTH => {
                self.settle_month();
                true
            }
            msg::CITY_ESTABLISHED => {
                let diff = self.difficulty();
                {
                    let mut d = self.d.borrow_mut();
                    let funds = BudgetTuning::by_difficulty(&d.tuning.initial_funds, diff);
                    let rate = BudgetTuning::by_difficulty(&d.tuning.initial_tax_rate, diff);
                    d.funds = funds;
                    d.shadow_funds = funds;
                    for g in d.tax.iter_mut() {
                        g.rate = rate;
                    }
                }
                self.post_self(msg::FUNDS_CHANGED);
                true
            }
            msg::TOGGLE_BUDGET_DIALOG => {
                let panel = match data1 {
                    msg::PANEL_1 => Some(1),
                    msg::PANEL_0 => Some(0),
                    _ => None,
                };
                if let Some(p) = panel {
                    self.d.borrow_mut().panel_flag = p;
                    self.show_budget_window();
                }
                true
            }
            _ => true,
        }
    }

    /// Cheat notification (§4.3): `text` is the full cheat line. Returns true
    /// for handled cheats; false for unknown IDs and `Moolah` without argument.
    pub fn handle_cheat(&self, cheat_id: u32, text: &str) -> bool {
        match cheat_id {
            cheat::WEAKNESS_PAYS => {
                let funds = self.funds();
                self.set_total_funds(funds.saturating_add(1000))
            }
            cheat::MOOLAH => {
                let mut toks = text.split_whitespace().peekable();
                // [choice] skip the leading cheat word if present.
                if toks.peek().is_some_and(|t| t.eq_ignore_ascii_case("moolah")) {
                    toks.next();
                }
                match toks.next().and_then(|t| t.parse::<i64>().ok()) {
                    Some(n) => self.set_total_funds(n),
                    None => false,
                }
            }
            cheat::RISKY_MONEY => {
                self.risky_money();
                true
            }
            _ => false,
        }
    }

    /// The RiskyMoney cheat (§4.3).
    fn risky_money(&self) {
        if !self.host.city_established() {
            return;
        }
        self.deposit_funds(10_000);
        let counter = {
            let mut d = self.d.borrow_mut();
            d.risky_money_counter += 1;
            d.risky_money_counter
        };
        let u = self.rng.borrow_mut().uniform_f64();
        if u < 1.0 - 1.0 / (0.2 * f64::from(counter) + 1.0) {
            let (w, dz) = self.host.city_size();
            let (r1, r2) = {
                let mut rng = self.rng.borrow_mut();
                (rng.uniform_f64(), rng.uniform_f64())
            };
            // §4.3: world coordinates, x := trunc(float(W*0.375 + r1*W*0.25))
            // (the host truncates).
            let (w, dz) = (f64::from(w), f64::from(dz));
            let x = (w * 0.375 + r1 * w * 0.25) as f32;
            let z = (dz * 0.375 + r2 * dz * 0.25) as f32;
            if self.host.try_start_disaster(cheat::RISKY_MONEY_DISASTER, x, z) {
                self.d.borrow_mut().risky_money_counter = 0;
            }
        }
    }

    // ---- money flow (§5) --------------------------------------------------------

    /// Daily simulation (§5.7).
    pub fn simulate_day(&self) {
        let rates: Vec<f32> = self.d.borrow().tax.iter().map(|g| g.rate).collect();
        for (i, r) in rates.into_iter().enumerate() {
            // §5.7: the product is single precision (0.35 -> 3.5 -> 4).
            self.host.record_history(
                history::TAX_RATE_BASE + i as u32,
                round_half_even(f64::from(r * 10.0)) as i32,
            );
        }
    }

    /// `ComputeTax` (§5.2). Single precision for the per-lot product and the
    /// running sums [compat]; the computation itself is [`scan_tax`].
    pub fn compute_tax(&self) -> i64 {
        let diff = self.difficulty();
        let (rates, tuning) = {
            let mut d = self.d.borrow_mut();
            for g in d.tax.iter_mut() {
                g.weighted_assessed = 0;
            }
            (d.tax.map(|g| g.rate), d.tuning.clone())
        };
        let mult = difficulty_multiplier(&tuning, diff);
        let host = &self.host;
        let scan = scan_tax(&tuning, mult, &rates, |visit| host.for_each_lot(visit), &mut |_, _| {});
        let monthly = scan.monthly_tax();
        let mut d = self.d.borrow_mut();
        for ((g, &wa), &tax) in d.tax.iter_mut().zip(&scan.weighted_assessed).zip(&monthly) {
            g.weighted_assessed = wa;
            g.monthly_tax = tax;
        }
        scan.total_tax()
    }

    /// The monthly settlement (§5.1), on SimNewMonth.
    pub fn settle_month(&self) {
        let (year, month) = self.host.sim_date();
        let m = if month == 1 {
            11
        } else {
            (month - 2).clamp(0, 11) as usize
        };
        {
            let mut d = self.d.borrow_mut();
            if d.funds != d.shadow_funds {
                d.funds = d.shadow_funds; // no message
            }
        }
        let tax = self.compute_tax();
        {
            let mut d = self.d.borrow_mut();
            d.last_tax_income = tax;
            d.ytd_tax_income = d.ytd_tax_income.wrapping_add(tax);
        }
        let ord_inc = self.host.ordinance_monthly_income();
        {
            let mut d = self.d.borrow_mut();
            d.ordinance_income_by_month[m] = ord_inc;
            d.ytd_ordinance_income = d.ytd_ordinance_income.wrapping_add(ord_inc);
        }
        let depts = self.departments_snapshot();
        let mut income = tax.wrapping_add(ord_inc).wrapping_add(self.host.neighbor_deal_income());
        for dept in &depts {
            income = income.wrapping_add(dept.total_income());
        }
        let ord_exp = self.host.ordinance_monthly_expense();
        {
            let mut d = self.d.borrow_mut();
            d.ordinance_expense_by_month[m] = ord_exp;
            d.ytd_ordinance_expense = d.ytd_ordinance_expense.wrapping_add(ord_exp);
        }
        let mut expense = ord_exp.wrapping_add(self.host.neighbor_deal_expense());
        for dept in &depts {
            dept.recalculate_all_line_item_current_expenses();
            let c = dept.total_expenses();
            dept.add_spending(c);
            expense = expense.wrapping_add(c);
        }
        expense = expense.wrapping_add(self.repay_bonds(year, month));
        let funds = self.funds();
        self.set_total_funds(funds.wrapping_add(income).wrapping_sub(expense));
        let (funds, min_allowable) = {
            let mut d = self.d.borrow_mut();
            d.ytd_income = d.ytd_income.wrapping_add(income);
            d.ytd_expense = d.ytd_expense.wrapping_add(expense);
            d.last_month_income = income;
            d.last_month_expense = expense;
            (d.funds, d.tuning.min_allowable_funds)
        };
        self.host.record_history(history::INCOME, history_value(income));
        self.host.record_history(history::EXPENSE, history_value(expense));
        self.host.record_history(history::FUNDS, (funds / 1000) as u32 as i32);
        self.d.borrow_mut().shadow_funds = funds;
        if m == 11 {
            {
                let mut d = self.d.borrow_mut();
                d.ytd_tax_income = 0;
                d.ytd_reserved = [0; 5];
                d.ytd_ordinance_income = 0;
                d.ytd_ordinance_expense = 0;
                d.ytd_income = 0;
                d.ytd_expense = 0;
                d.ytd_bond_payments = 0;
            }
            for dept in &depts {
                dept.set_total_spending(0);
            }
        }
        if funds <= min_allowable {
            self.host.post(&BudgetMessage {
                msg_type: msg::END_GAME_FOR_BUDGET_FAILURE,
                ..Default::default()
            });
        }
    }

    /// Monthly bond repayment (§5.4); returns the total paid.
    fn repay_bonds(&self, year: i32, month: i32) -> i64 {
        let rate = self.d.borrow().tuning.bond_interest_rate;
        let monthly_rate: f32 = rate / 12.0;
        let mut paid_total = 0i64;
        let mut finished = 0;
        {
            let mut d = self.d.borrow_mut();
            let bonds = std::mem::take(&mut d.bonds);
            let mut kept = Vec::with_capacity(bonds.len());
            for mut b in bonds {
                let mut dy = b.due_year - year;
                let mut dm = b.due_month - month;
                if dm < 0 {
                    dm += 12;
                    dy -= 1;
                }
                let p = b.monthly_payment;
                d.ytd_bond_payments = d.ytd_bond_payments.wrapping_add(p);
                b.remaining = b.remaining.wrapping_sub(p);
                b.paid = b.paid.wrapping_add(p);
                paid_total = paid_total.wrapping_add(p);
                if dy == 0 && dm == 0 {
                    d.total_borrowed = d.total_borrowed.wrapping_sub(b.amount);
                    d.bond_count = d.bond_count.saturating_sub(1);
                    finished += 1;
                } else {
                    let interest = (monthly_rate * b.principal_left as f32) as i64;
                    b.principal_left = b.principal_left.wrapping_sub(p - interest);
                    if dy == 0 && dm == 1 {
                        b.monthly_payment = b.principal_left;
                    }
                    kept.push(b);
                }
            }
            d.bonds = kept;
        }
        for _ in 0..finished {
            self.post_self(msg::BOND_FINISHED);
        }
        paid_total
    }

    // ---- funds (§6) ----------------------------------------------------------------

    pub fn funds(&self) -> i64 {
        self.d.borrow().funds
    }

    pub fn shadow_funds(&self) -> i64 {
        self.d.borrow().shadow_funds
    }

    /// For tests of the anti-tampering rule (§3.1): changes `funds` only.
    pub fn tamper_funds(&self, v: i64) {
        self.d.borrow_mut().funds = v;
    }

    /// `SetTotalFunds(v)`. [choice] returns true.
    pub fn set_total_funds(&self, v: i64) -> bool {
        let changed = {
            let mut d = self.d.borrow_mut();
            if d.funds != v {
                d.funds = v;
                d.shadow_funds = v;
                true
            } else {
                false
            }
        };
        if changed {
            self.post_self(msg::FUNDS_CHANGED);
        }
        true
    }

    pub fn deposit_funds(&self, x: i64) -> bool {
        if x < 0 {
            return self.withdraw_funds(x.checked_neg().unwrap_or(i64::MAX));
        }
        if x == 0 {
            return true;
        }
        {
            let mut d = self.d.borrow_mut();
            d.funds = d.funds.saturating_add(x);
            d.shadow_funds = d.funds;
        }
        self.post_self(msg::FUNDS_CHANGED);
        true
    }

    pub fn withdraw_funds(&self, x: i64) -> bool {
        if x < 0 {
            return self.deposit_funds(x.checked_neg().unwrap_or(i64::MAX));
        }
        if x == 0 {
            return true;
        }
        {
            let mut d = self.d.borrow_mut();
            if x > d.funds {
                return false;
            }
            d.funds -= x;
            d.shadow_funds = d.funds;
        }
        self.post_self(msg::FUNDS_CHANGED);
        true
    }

    pub fn min_allowable_funds(&self) -> i64 {
        self.d.borrow().tuning.min_allowable_funds
    }

    pub fn ytd_income(&self) -> i64 {
        self.d.borrow().ytd_income
    }

    pub fn ytd_expenses(&self) -> i64 {
        self.d.borrow().ytd_expense
    }

    fn months_left(&self) -> i64 {
        let (_, month) = self.host.sim_date();
        i64::from(13 - month)
    }

    pub fn est_income(&self) -> i64 {
        self.ytd_income()
            .wrapping_add(self.total_monthly_income().wrapping_mul(self.months_left()))
    }

    pub fn est_expenses(&self) -> i64 {
        self.ytd_expenses()
            .wrapping_add(self.total_monthly_expense().wrapping_mul(self.months_left()))
    }

    /// Live projection (§6), not the stored last-month value.
    pub fn total_monthly_expense(&self) -> i64 {
        let mut v = self
            .host
            .ordinance_monthly_expense()
            .wrapping_add(self.host.neighbor_deal_expense());
        for dept in self.departments_snapshot() {
            v = v.wrapping_add(dept.total_expenses());
        }
        v.wrapping_add(self.total_monthly_bond_payments())
    }

    pub fn total_yearly_expense(&self) -> i64 {
        self.d.borrow().yearly_expense
    }

    /// Live projection (§6).
    pub fn total_monthly_income(&self) -> i64 {
        let mut v: i64 = (0..TAX_GROUPS as i32).map(|t| self.tax_income_for(t)).sum();
        v = v
            .wrapping_add(self.host.ordinance_monthly_income())
            .wrapping_add(self.host.neighbor_deal_income());
        for dept in self.departments_snapshot() {
            v = v.wrapping_add(dept.total_income());
        }
        v
    }

    pub fn total_yearly_income(&self) -> i64 {
        self.d.borrow().yearly_income
    }

    /// `GetTaxIncome()`: the last `ComputeTax` result.
    pub fn tax_income(&self) -> i64 {
        self.d.borrow().last_tax_income
    }

    /// `GetTaxIncome(t)`: current rate with last month's assessed value.
    pub fn tax_income_for(&self, t: i32) -> i64 {
        let diff = self.difficulty();
        let d = self.d.borrow();
        let Some(g) = usize::try_from(t).ok().and_then(|t| d.tax.get(t)) else {
            return 0;
        };
        // §6 [compat]: float(assessed) × multiplier × rate × INV1200, left to right.
        tax_income_of(g.weighted_assessed, difficulty_multiplier(&d.tuning, diff), g.rate)
    }

    pub fn set_total_monthly_expense(&self, v: i64) -> i64 {
        self.d.borrow_mut().last_month_expense = v;
        v
    }

    pub fn set_total_yearly_expense(&self, v: i64) -> i64 {
        self.d.borrow_mut().yearly_expense = v;
        v
    }

    pub fn set_total_monthly_income(&self, v: i64) -> i64 {
        self.d.borrow_mut().last_month_income = v;
        v
    }

    pub fn set_total_yearly_income(&self, v: i64) -> i64 {
        self.d.borrow_mut().yearly_income = v;
        v
    }

    /// `ShowBudgetWindow` (UI). [choice] returns true.
    pub fn show_budget_window(&self) -> bool {
        let panel = self.d.borrow().panel_flag;
        self.host.show_budget_window(panel);
        true
    }

    // ---- departments and groups (§6, §6.1) ------------------------------------------------

    /// Find-or-create group `id` (name from LTEXT, totals 0).
    fn ensure_group(&self, id: u32) {
        if self.d.borrow().groups.contains_key(&id) {
            return;
        }
        let name = self.host.ltext(GROUP_NAME_LTEXT, id);
        self.d.borrow_mut().groups.insert(
            id,
            BudgetGroup {
                id,
                name,
                total_expense: 0,
                total_income: 0,
                departments: BTreeSet::new(),
            },
        );
    }

    /// Group refresh (§6.1).
    fn refresh_group(&self, g: u32) {
        let members: Vec<Rc<Department>> = {
            let d = self.d.borrow();
            let Some(group) = d.groups.get(&g) else { return };
            group
                .departments
                .iter()
                .filter_map(|id| d.departments.get(id).cloned())
                .collect()
        };
        let exp: i64 = members.iter().map(|m| m.total_expenses()).fold(0, i64::wrapping_add);
        let inc: i64 = members.iter().map(|m| m.total_income()).fold(0, i64::wrapping_add);
        let changed = {
            let mut d = self.d.borrow_mut();
            let Some(group) = d.groups.get_mut(&g) else { return };
            if group.total_expense != exp || group.total_income != inc {
                group.total_expense = exp;
                group.total_income = inc;
                true
            } else {
                false
            }
        };
        if changed {
            self.host.post(&BudgetMessage {
                msg_type: msg::BUDGET_GROUP_CHANGED,
                data1: i64::from(g),
                data2: i64::from(exp as i32),
                data3: i64::from(inc as i32),
                ..Default::default()
            });
        }
    }

    pub fn create_department_budget(&self, id: u32, group: u32) -> Option<Rc<Department>> {
        if let Some(d) = self.department(id) {
            return Some(d); // `group` ignored
        }
        let obj = self.host.create_object(CLSID_DEPARTMENT)?;
        let dept = obj.as_any_rc().downcast::<Department>().ok()?;
        dept.init();
        self.d.borrow_mut().departments.insert(id, dept.clone()); // holds the creation reference
        dept.set_budget(self.me().as_ref());
        dept.set_id(id);
        dept.set_funding_percentage(1.0, 0);
        self.ensure_group(group);
        if let Some(g) = self.d.borrow_mut().groups.get_mut(&group) {
            g.departments.insert(id);
        }
        dept.set_budget_group(group);
        self.refresh_group(group);
        Some(dept)
    }

    pub fn remove_department_budget(&self, id: u32) -> bool {
        let Some(dept) = self.department(id) else { return false };
        let g = dept.budget_group();
        if let Some(group) = self.d.borrow_mut().groups.get_mut(&g) {
            group.departments.remove(&id);
        }
        self.refresh_group(g);
        self.d.borrow_mut().departments.remove(&id);
        dept.shutdown();
        dept.set_budget(None);
        self.host.release(dept.token());
        true
    }

    pub fn department(&self, id: u32) -> Option<Rc<Department>> {
        self.d.borrow().departments.get(&id).cloned()
    }

    /// First department (ascending ID) whose name starts with `name`,
    /// case-insensitively; an empty name matches the first department.
    pub fn department_by_name(&self, name: &[u8]) -> Option<Rc<Department>> {
        self.departments_snapshot().into_iter().find(|d| {
            let n = d.name();
            n.len() >= name.len() && n[..name.len()].eq_ignore_ascii_case(name)
        })
    }

    pub fn department_ids(&self) -> Vec<u32> {
        self.d.borrow().departments.keys().copied().collect()
    }

    /// `GetAllGroups`: ascending ID.
    pub fn all_groups(&self) -> Vec<BudgetGroup> {
        self.d.borrow().groups.values().cloned().collect()
    }

    pub fn set_group_name(&self, id: u32, name: &[u8]) -> bool {
        self.ensure_group(id);
        if let Some(g) = self.d.borrow_mut().groups.get_mut(&id) {
            g.name = name.to_vec();
        }
        true
    }

    /// `GetDepartmentBudgetsInGroup` (ascending department ID; the caller takes references).
    pub fn departments_in_group(&self, id: u32) -> Vec<Rc<Department>> {
        let d = self.d.borrow();
        let Some(g) = d.groups.get(&id) else { return Vec::new() };
        g.departments
            .iter()
            .filter_map(|i| d.departments.get(i).cloned())
            .collect()
    }

    pub fn needed_funding_changed(&self, dept: &Department) {
        self.refresh_group(dept.budget_group());
    }

    pub fn funding_percentage_changed(&self, dept: &Department) {
        self.refresh_group(dept.budget_group());
    }

    /// `GetFunding(...)`: always 0 [compat].
    pub fn funding(&self) -> i64 {
        0
    }

    // ---- tax (§6) ------------------------------------------------------------------------

    pub fn tax_rate(&self, t: u32) -> f32 {
        self.d.borrow().tax.get(t as usize).map_or(0.0, |g| g.rate)
    }

    /// Stored unclamped; no message. [choice] true when stored.
    pub fn set_tax_rate(&self, t: u32, r: f32) -> bool {
        match self.d.borrow_mut().tax.get_mut(t as usize) {
            Some(g) => {
                g.rate = r;
                true
            }
            None => false,
        }
    }

    pub fn weighted_assessed_tax_value(&self, t: u32) -> i64 {
        self.d.borrow().tax.get(t as usize).map_or(0, |g| g.weighted_assessed)
    }

    pub fn tax_groups(&self) -> [TaxGroup; TAX_GROUPS] {
        self.d.borrow().tax
    }

    // ---- bonds (§5.4, §6) -------------------------------------------------------------------

    pub fn bond_increment(&self) -> i64 {
        self.d.borrow().tuning.bond_increment
    }

    /// `GetMonthlyPaymentForLoan(L)` (annuity; [choice] `L div n` when r == 0).
    pub fn monthly_payment_for_loan(&self, loan: i64) -> i64 {
        let (r, n) = {
            let d = self.d.borrow();
            (d.tuning.bond_interest_rate, d.tuning.bond_payback_months)
        };
        if n == 0 {
            return 0;
        }
        if r == 0.0 {
            return loan / i64::from(n);
        }
        // [compat] §5.4: the original's doubles run at 24-bit x87 precision
        // (foundations §18), so every step, the CRT pow included, rounds to
        // f32, in this order: q := r × 1/12; L × q; 1 + q; −(n + 0); pow;
        // 1 − that; divide. Each step is an f64 operation on f32 operands
        // rounded once to f32 (exact for +, −, ×, ÷); pow is the f64 pow
        // rounded to f32 ([inferred]: may differ by 1 near integer boundaries).
        let f = |x: f64| f64::from(x as f32);
        let q = f(f64::from(r) * (1.0 / 12.0));
        let num = f(loan as f64 * q);
        let base = f(1.0 + q);
        let exp = f(-(f64::from(n) + 0.0));
        let p = f(base.powf(exp));
        let den = f(1.0 - p);
        f(num / den) as i64
    }

    pub fn full_cost_of_loan(&self, loan: i64) -> i64 {
        let n = i64::from(self.d.borrow().tuning.bond_payback_months);
        self.monthly_payment_for_loan(loan).wrapping_mul(n)
    }

    /// `IssueBond(amount)` (§5.4).
    pub fn issue_bond(&self, amount: u32) -> bool {
        let t = self.tuning();
        let (count, borrowed) = {
            let d = self.d.borrow();
            (d.bond_count, d.total_borrowed)
        };
        if amount == 0 || i64::from(count) >= i64::from(t.max_bonds) || t.bond_increment <= 0 {
            return false;
        }
        // [compat] 32-bit truncation, sign-extended.
        let mut a = i64::from(((i64::from(amount) / t.bond_increment) * t.bond_increment) as i32);
        if a == 0 {
            a = t.bond_increment;
        }
        if a > t.max_bond_amount || borrowed + a > i64::from(t.max_bonds) * t.max_bond_amount {
            return false;
        }
        let pay = self.monthly_payment_for_loan(a);
        let full = i64::from(self.full_cost_of_loan(a) as i32);
        let (year, month) = self.host.sim_date();
        let n = t.bond_payback_months as i32;
        let mut due_month = month + n % 12;
        let mut due_year = year + n / 12;
        if due_month > 12 {
            due_month -= 12;
            due_year += 1;
        }
        {
            let mut d = self.d.borrow_mut();
            d.bond_count += 1;
            d.bonds.push(Bond {
                due_month,
                due_year,
                amount: a,
                full_cost: full,
                remaining: full,
                paid: 0,
                monthly_payment: pay,
                principal_left: a,
            });
        }
        self.post_self(msg::BOND_ISSUED);
        {
            let mut d = self.d.borrow_mut();
            d.total_borrowed = d.total_borrowed.wrapping_add(a);
        }
        self.deposit_funds(a);
        true
    }

    pub fn total_borrowed(&self) -> i64 {
        self.d.borrow().total_borrowed
    }

    pub fn current_borrowing_limit(&self) -> i64 {
        let d = self.d.borrow();
        i64::from(d.tuning.max_bonds) * d.tuning.max_bond_amount
    }

    pub fn current_bond_limit(&self) -> i64 {
        self.d.borrow().tuning.max_bond_amount
    }

    pub fn current_max_outstanding_bonds_limit(&self) -> i64 {
        i64::from(self.d.borrow().tuning.max_bonds)
    }

    pub fn total_monthly_bond_payments(&self) -> i64 {
        self.d
            .borrow()
            .bonds
            .iter()
            .map(|b| b.monthly_payment)
            .fold(0, i64::wrapping_add)
    }

    pub fn bonds(&self) -> Vec<Bond> {
        self.d.borrow().bonds.clone()
    }

    /// `GetAllLoans` (issue order).
    pub fn all_loans(&self) -> Vec<LoanInfo> {
        let (year, month) = self.host.sim_date();
        self.d
            .borrow()
            .bonds
            .iter()
            .map(|b| {
                let mut years = b.due_year - year;
                let mut months = b.due_month - month;
                if months < 0 {
                    months += 12;
                    years -= 1;
                }
                LoanInfo {
                    months_remaining: months as u32,
                    years_remaining: years as u32,
                    total_loan_amount: b.amount,
                    remaining_payment: b.principal_left,
                    monthly_payment: b.monthly_payment,
                }
            })
            .collect()
    }

    pub fn loan_time_in_months(&self) -> i32 {
        self.d.borrow().tuning.bond_payback_months as i32
    }

    // ---- budget items (§6) ---------------------------------------------------------------------

    pub fn budget_item_info(&self, holder: u64) -> Option<Vec<BudgetItem>> {
        budget_items(&*self.host, holder)
    }

    pub fn budget_item_for_purpose(&self, holder: u64, purpose: u32) -> Option<BudgetItem> {
        item_for_purpose(&*self.host, holder, purpose)
    }

    pub fn change_budget_item_line(&self, holder: u64, index: u32, line: u32) -> bool {
        change_item_line(&*self.host, holder, index, line);
        true
    }

    pub fn change_budget_item_cost(&self, holder: u64, index: u32, cost: i64) -> bool {
        change_item_cost(&*self.host, holder, index, cost);
        true
    }

    pub fn change_budget_item_local_funding(&self, holder: u64, index: u32, fraction: f32) -> bool {
        change_item_local_funding(&*self.host, holder, index, fraction)
    }

    fn item_line(&self, holder: u64, index: u32) -> Option<(Rc<LineItem>, i64)> {
        let items = budget_items(&*self.host, holder)?;
        let item = items.get(index as usize)?;
        let line = self.department(item.department_id)?.line_item(item.line_item)?;
        Some((line, item.cost))
    }

    /// `AddBudgetItemToBudget(holder, index)`. [choice] true if applied.
    pub fn add_budget_item_to_budget(&self, holder: u64, index: u32) -> bool {
        match self.item_line(holder, index) {
            Some((line, cost)) => line.add_to_full_expenses(cost),
            None => false,
        }
    }

    /// `RemoveBudgetItemFromBudget(holder, index)`. [choice] true if applied.
    pub fn remove_budget_item_from_budget(&self, holder: u64, index: u32) -> bool {
        match self.item_line(holder, index) {
            Some((line, cost)) => line.add_to_full_expenses(cost.wrapping_neg()),
            None => false,
        }
    }

    pub fn line_item_from_budget_item(&self, item: &BudgetItem) -> Option<Rc<LineItem>> {
        self.department(item.department_id)?.line_item(item.line_item)
    }

    pub fn copy_budget_item_properties(&self, src: u64, dst: u64) -> bool {
        copy_item_properties(&*self.host, src, dst)
    }

    // ---- inspection ----------------------------------------------------------------------------

    pub fn snapshot(&self) -> BudgetSnapshot {
        let d = self.d.borrow();
        BudgetSnapshot {
            initialized: d.initialized,
            loaded_from_save: d.loaded_from_save,
            agriculture_rate_fix: d.agriculture_rate_fix,
            panel_flag: d.panel_flag,
            funds: d.funds,
            shadow_funds: d.shadow_funds,
            total_borrowed: d.total_borrowed,
            last_tax_income: d.last_tax_income,
            ytd_income: d.ytd_income,
            ytd_expense: d.ytd_expense,
            ytd_tax_income: d.ytd_tax_income,
            ytd_reserved: d.ytd_reserved,
            ytd_ordinance_income: d.ytd_ordinance_income,
            ytd_ordinance_expense: d.ytd_ordinance_expense,
            ordinance_income_by_month: d.ordinance_income_by_month,
            ordinance_expense_by_month: d.ordinance_expense_by_month,
            ytd_bond_payments: d.ytd_bond_payments,
            last_month_income: d.last_month_income,
            last_month_expense: d.last_month_expense,
            yearly_income: d.yearly_income,
            yearly_expense: d.yearly_expense,
            tax: d.tax,
            bonds: d.bonds.clone(),
            bond_count: d.bond_count,
            department_ids: d.departments.keys().copied().collect(),
            groups: d.groups.values().cloned().collect(),
            risky_money_counter: d.risky_money_counter,
        }
    }
}

impl Drop for BudgetSim {
    fn drop(&mut self) {
        let d = self.d.get_mut();
        for (_, dept) in std::mem::take(&mut d.departments) {
            dept.set_budget(None);
            self.host.release(dept.token());
        }
    }
}

// ---- persistence (§11.1) --------------------------------------------------------------------------

impl SaveObject for BudgetSim {
    fn class_id(&self) -> u32 {
        CLSID_BUDGET_SIMULATOR
    }

    fn write(&self, out: &mut dyn OutStream) -> bool {
        let d = self.d.borrow().clone();
        out.put_u16(BUDGET_RECORD_VERSION);
        out.put_i64(d.funds);
        out.put_i64(d.shadow_funds);
        out.put_i64(d.total_borrowed);
        out.put_i64(d.last_tax_income);
        out.put_u32(d.departments.len() as u32);
        for (id, dept) in &d.departments {
            out.put_u32(*id);
            out.put_ref(&RefValue::Object(dept.clone() as Rc<dyn SaveObject>));
        }
        for g in &d.tax {
            out.put_f32(g.rate);
            out.put_f32(g.max_rate);
            for h in g.history {
                out.put_i64(h);
            }
            out.put_i64(g.reserved);
            out.put_i64(g.weighted_assessed);
            out.put_i64(g.monthly_tax);
        }
        out.put_i64(d.ytd_ordinance_expense);
        out.put_i64(d.ytd_ordinance_income);
        for v in d.ordinance_expense_by_month {
            out.put_i64(v);
        }
        for v in d.ordinance_income_by_month {
            out.put_i64(v);
        }
        out.put_i64(d.ytd_tax_income);
        for v in d.ytd_reserved {
            out.put_i64(v);
        }
        out.put_u32(d.bonds.len() as u32);
        for b in &d.bonds {
            out.put_i32(b.due_month);
            out.put_i32(b.due_year);
            out.put_i64(b.amount);
            out.put_i64(b.full_cost);
            out.put_i64(b.remaining);
            out.put_i64(b.paid);
            out.put_i64(b.monthly_payment);
            out.put_i64(b.principal_left);
        }
        out.put_i64(d.ytd_bond_payments);
        out.put_i64(d.last_month_income);
        out.put_i64(d.yearly_income);
        out.put_i64(d.last_month_expense);
        out.put_i64(d.yearly_expense);
        out.put_u32(d.groups.len() as u32);
        for (key, g) in &d.groups {
            out.put_u32(*key);
            out.put_u32(g.id);
            out.put_str(&g.name);
            out.put_i64(g.total_expense);
            out.put_i64(g.total_income);
            out.put_u32(g.departments.len() as u32);
            for id in &g.departments {
                out.put_u32(*id);
            }
        }
        out.error() == 0
    }

    fn read(&self, inp: &mut dyn InStream) -> bool {
        let mut depts: Vec<(u32, Rc<Department>)> = Vec::new();
        let parsed = (|| -> Option<(BudgetData, u16)> {
            let version = inp.get_u16()?;
            // §11.1: < 4 fails; [choice] > 5 fails too.
            if !(4..=BUDGET_RECORD_VERSION).contains(&version) {
                return None;
            }
            let mut d = BudgetData {
                funds: inp.get_i64()?,
                shadow_funds: inp.get_i64()?,
                total_borrowed: inp.get_i64()?,
                last_tax_income: inp.get_i64()?,
                ..Default::default()
            };
            for _ in 0..inp.get_u32()? {
                let id = inp.get_u32()?;
                match inp.get_ref()? {
                    r @ RefValue::Object(_) => {
                        if let Some(dept) = r.object::<Department>() {
                            depts.push((id, dept));
                        }
                    }
                    RefValue::Host { token, .. } => self.host.release(token),
                    RefValue::Null => {}
                }
            }
            for g in d.tax.iter_mut() {
                g.rate = inp.get_f32()?;
                g.max_rate = inp.get_f32()?;
                for h in g.history.iter_mut() {
                    *h = inp.get_i64()?;
                }
                g.reserved = inp.get_i64()?;
                g.weighted_assessed = inp.get_i64()?;
                g.monthly_tax = inp.get_i64()?;
            }
            d.ytd_ordinance_expense = inp.get_i64()?;
            d.ytd_ordinance_income = inp.get_i64()?;
            for v in d.ordinance_expense_by_month.iter_mut() {
                *v = inp.get_i64()?;
            }
            for v in d.ordinance_income_by_month.iter_mut() {
                *v = inp.get_i64()?;
            }
            d.ytd_tax_income = inp.get_i64()?;
            for v in d.ytd_reserved.iter_mut() {
                *v = inp.get_i64()?;
            }
            d.bond_count = inp.get_u32()?;
            for _ in 0..d.bond_count {
                d.bonds.push(Bond {
                    due_month: inp.get_i32()?,
                    due_year: inp.get_i32()?,
                    amount: inp.get_i64()?,
                    full_cost: inp.get_i64()?,
                    remaining: inp.get_i64()?,
                    paid: inp.get_i64()?,
                    monthly_payment: inp.get_i64()?,
                    principal_left: inp.get_i64()?,
                });
            }
            d.ytd_bond_payments = inp.get_i64()?;
            d.last_month_income = inp.get_i64()?;
            d.yearly_income = inp.get_i64()?;
            d.last_month_expense = inp.get_i64()?;
            d.yearly_expense = inp.get_i64()?;
            for _ in 0..inp.get_u32()? {
                let key = inp.get_u32()?;
                let id = inp.get_u32()?;
                let _stored_name = inp.get_str()?; // discarded (§11.1)
                let total_expense = inp.get_i64()?;
                let total_income = inp.get_i64()?;
                let mut departments = BTreeSet::new();
                for _ in 0..inp.get_u32()? {
                    departments.insert(inp.get_u32()?);
                }
                d.groups.insert(
                    key,
                    BudgetGroup {
                        id,
                        name: Vec::new(),
                        total_expense,
                        total_income,
                        departments,
                    },
                );
            }
            Some((d, version))
        })();
        let Some((mut d, version)) = parsed else { return false };
        if inp.error() != 0 {
            return false;
        }
        // Group names come from LTEXT, not from the file [compat].
        for g in d.groups.values_mut() {
            g.name = self.host.ltext(GROUP_NAME_LTEXT, g.id);
        }
        if version < 5 && d.tax[8].rate == 0.0 {
            d.agriculture_rate_fix = true; // §11.1 [compat]
        }
        d.loaded_from_save = true;
        // Keep the non-persisted state of this (normally fresh) instance.
        let old_departments = {
            let mut cur = self.d.borrow_mut();
            d.initialized = cur.initialized;
            d.panel_flag = cur.panel_flag;
            d.risky_money_counter = cur.risky_money_counter;
            d.tuning = cur.tuning.clone();
            let old = std::mem::take(&mut cur.departments);
            *cur = d;
            old
        };
        for (_, old) in old_departments {
            old.set_budget(None);
            self.host.release(old.token());
        }
        let me = self.me();
        for (id, dept) in depts {
            self.host.add_ref(dept.token()); // the map holds a reference
            dept.set_budget(me.as_ref());
            if let Some(old) = self.d.borrow_mut().departments.insert(id, dept) {
                old.set_budget(None);
                self.host.release(old.token());
            }
        }
        true
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
