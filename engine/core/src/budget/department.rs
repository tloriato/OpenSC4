// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cISC4DepartmentBudget` (§3.2, §7, §11.2).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

use crate::stream::{InStream, OutStream, RefValue, SaveObject};

use super::items::{budget_items, change_item_cost, change_item_local_funding, item_for_purpose};
use super::line_item::{TYPE_EXPENSE, TYPE_INCOME};
use super::{
    msg, BudgetHost, BudgetMessage, BudgetSim, LineItem, CLSID_DEPARTMENT, CLSID_LINE_ITEM, IID_PROPERTY_HOLDER,
};

/// Record version (§11.2).
pub const DEPARTMENT_RECORD_VERSION: u16 = 10;

/// Safety net for the redistribution loop (§7.1 [choice]: it must terminate).
const MAX_DISTRIBUTION_PASSES: usize = 100_000;

#[derive(Clone)]
struct DeptData {
    initialized: bool,
    id: u32,
    name: Vec<u8>,
    name_group: u32,
    name_instance: u32,
    budget_group: u32,
    p: f32,
    p_max: f32,
    total_spending: i64,
    fixed: bool,
    lines: BTreeMap<u32, Rc<LineItem>>,
    /// Line items read from a save but not yet keyed: their records may be
    /// read after this department's (references are created before they are
    /// loaded), so the key - the line's own ID (§11.2) - is taken on first use.
    /// Each holds one host reference.
    pending_lines: Vec<Rc<LineItem>>,
    /// Locally funded objects (property holder token, purpose), [choice] in
    /// insertion order (§7.1). Each holds one host reference.
    locals: Vec<(u64, u32)>,
}

impl Default for DeptData {
    fn default() -> Self {
        DeptData {
            initialized: false,
            id: 0,
            name: Vec::new(),
            name_group: 0,
            name_instance: 0,
            budget_group: 0,
            p: 1.0,
            p_max: 1.0,
            total_spending: 0,
            fixed: false,
            lines: BTreeMap::new(),
            pending_lines: Vec::new(),
            locals: Vec::new(),
        }
    }
}

/// A department budget. Shared (`Rc`); the budget simulator is a weak
/// back-reference, set while the department is in its map.
pub struct Department {
    host: Rc<dyn BudgetHost>,
    token: Cell<u64>,
    me: Weak<Department>,
    d: RefCell<DeptData>,
    budget: RefCell<Weak<BudgetSim>>,
}

/// `floor(x + 0.5)` on doubles, as an integer.
fn round_half_up(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

impl Department {
    /// A fresh department (as created by its class factory).
    pub fn new(host: Rc<dyn BudgetHost>, token: u64) -> Rc<Department> {
        Rc::new_cyclic(|me| Department {
            host,
            token: Cell::new(token),
            me: me.clone(),
            d: RefCell::new(DeptData::default()),
            budget: RefCell::new(Weak::new()),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    fn me(&self) -> Option<Rc<Department>> {
        self.me.upgrade()
    }

    pub fn budget(&self) -> Option<Rc<BudgetSim>> {
        self.budget.borrow().upgrade()
    }

    pub(crate) fn set_budget(&self, b: Option<&Rc<BudgetSim>>) {
        *self.budget.borrow_mut() = b.map(Rc::downgrade).unwrap_or_default();
    }

    pub(crate) fn init(&self) {
        self.d.borrow_mut().initialized = true;
    }

    pub(crate) fn shutdown(&self) {
        self.d.borrow_mut().initialized = false;
    }

    pub fn is_initialized(&self) -> bool {
        self.d.borrow().initialized
    }

    pub(crate) fn set_id(&self, id: u32) {
        self.d.borrow_mut().id = id;
    }

    /// Keys the line items read from a save by their IDs (see `pending_lines`).
    /// A later line with the same ID replaces an earlier one, as at read time.
    fn settle_lines(&self) {
        let pending = std::mem::take(&mut self.d.borrow_mut().pending_lines);
        for l in pending {
            let old = self.d.borrow_mut().lines.insert(l.id(), l);
            if let Some(old) = old {
                self.host.release(old.token());
            }
        }
    }

    fn lines_snapshot(&self) -> Vec<Rc<LineItem>> {
        self.settle_lines();
        self.d.borrow().lines.values().cloned().collect()
    }

    fn locals_snapshot(&self) -> Vec<(u64, u32)> {
        self.d.borrow().locals.clone()
    }

    fn is_registered(&self, holder: u64, purpose: u32) -> bool {
        self.d.borrow().locals.contains(&(holder, purpose))
    }

    /// The department change message (§7 "Notify").
    pub(crate) fn notify(&self, msg_type: u32, data3: i64) {
        if let Some(b) = self.budget() {
            if msg_type == msg::FUNDING_PERCENTAGE_CHANGED {
                b.funding_percentage_changed(self);
            } else if msg_type == msg::IDEAL_MONTHLY_FUNDING_CHANGED {
                b.needed_funding_changed(self);
            }
        }
        let token = self.token();
        self.host.post(&BudgetMessage {
            msg_type,
            data2: i64::from(self.id()),
            data3,
            void1: token,
            unknown: token,
            ..Default::default()
        });
    }

    // ---- cISC4DepartmentBudget (SDK order) ----

    pub fn id(&self) -> u32 {
        self.d.borrow().id
    }

    pub fn name(&self) -> Vec<u8> {
        self.d.borrow().name.clone()
    }

    pub fn set_department_name(&self, ltext_group: u32, ltext_instance: u32) -> bool {
        let name = self.host.ltext(ltext_group, ltext_instance);
        let mut d = self.d.borrow_mut();
        d.name_group = ltext_group;
        d.name_instance = ltext_instance;
        d.name = name;
        true
    }

    pub fn name_ids(&self) -> (u32, u32) {
        let d = self.d.borrow();
        (d.name_group, d.name_instance)
    }

    pub fn budget_group(&self) -> u32 {
        self.d.borrow().budget_group
    }

    /// [compat] Does not move the department between the budget's group sets.
    pub fn set_budget_group(&self, g: u32) -> bool {
        self.d.borrow_mut().budget_group = g;
        true
    }

    pub fn is_fixed_funding(&self) -> bool {
        self.d.borrow().fixed
    }

    pub fn set_fixed_funding(&self, b: bool) -> bool {
        self.d.borrow_mut().fixed = b;
        true
    }

    pub fn funding_percentage(&self) -> f32 {
        self.d.borrow().p
    }

    /// `SetFundingPercentage(q, arg)` = SetPct(q, arg, redistribute = true).
    pub fn set_funding_percentage(&self, q: f32, arg: u32) -> bool {
        self.set_pct(q, i64::from(arg), true);
        true
    }

    pub fn max_funding_percentage(&self) -> f32 {
        self.d.borrow().p_max
    }

    /// Stores `pMax`; `p` is not re-clamped.
    pub fn set_max_funding_percentage(&self, q: f32) -> bool {
        self.d.borrow_mut().p_max = q;
        true
    }

    pub fn ideal_monthly_funding(&self) -> i64 {
        self.lines_snapshot()
            .iter()
            .map(|l| l.full_expenses())
            .fold(0i64, i64::wrapping_add)
    }

    pub fn total_expenses(&self) -> i64 {
        self.lines_snapshot()
            .iter()
            .map(|l| l.current_expenses())
            .fold(0i64, i64::wrapping_add)
    }

    pub fn total_income(&self) -> i64 {
        self.lines_snapshot()
            .iter()
            .map(|l| l.income())
            .fold(0i64, i64::wrapping_add)
    }

    /// `CreateLineItem(id, isLocal)`: always a new line item, stored under
    /// `id`, replacing any existing entry [compat].
    pub fn create_line_item(&self, id: u32, is_local: bool) -> Option<Rc<LineItem>> {
        let obj = self.host.create_object(CLSID_LINE_ITEM)?;
        let line = match obj.as_any_rc().downcast::<LineItem>() {
            Ok(l) => l,
            Err(_) => return None,
        };
        line.init(id, is_local);
        line.set_owner(self.me().as_ref());
        self.settle_lines();
        let old = self.d.borrow_mut().lines.insert(id, line.clone());
        if let Some(old) = old {
            old.set_owner(None);
            self.host.release(old.token());
        }
        Some(line)
    }

    /// `CreateLineItemForBuildingType(iid, isLocal)`.
    pub fn create_line_item_for_building_type(&self, building_type: u32, is_local: bool) -> Option<Rc<LineItem>> {
        let line = self.create_line_item(building_type, is_local)?;
        line.set_name(0, building_type);
        Some(line)
    }

    pub fn remove_line_item(&self, id: u32) -> bool {
        self.settle_lines();
        let removed = self.d.borrow_mut().lines.remove(&id);
        match removed {
            Some(line) => {
                line.set_owner(None);
                self.host.release(line.token());
                true
            }
            None => false,
        }
    }

    pub fn line_item(&self, id: u32) -> Option<Rc<LineItem>> {
        self.settle_lines();
        self.d.borrow().lines.get(&id).cloned()
    }

    /// Every line item, ascending ID (the caller takes its own references).
    pub fn all_line_items(&self) -> Vec<Rc<LineItem>> {
        self.lines_snapshot()
    }

    /// `AddLocallyFundedObject(holder, purpose)` (§7). [choice] Returns false
    /// (and changes nothing) unless the holder has an item for `purpose` in
    /// this department whose line exists; adding a registered pair again is a
    /// no-op returning true.
    pub fn add_locally_funded_object(&self, holder: u64, purpose: u32) -> bool {
        let Some(item) = item_for_purpose(&*self.host, holder, purpose) else {
            return false;
        };
        if item.department_id != self.id() {
            return false;
        }
        let Some(line) = self.line_item(item.line_item) else {
            return false;
        };
        if self.is_registered(holder, purpose) {
            return true;
        }
        self.host.add_ref(holder);
        self.d.borrow_mut().locals.push((holder, purpose));
        line.add_to_full_expenses(item.cost);
        let p = self.funding_percentage();
        let add = (p * item.cost as f32) as i64;
        line.set_current(line.current_expenses().wrapping_add(add).max(0));
        change_item_local_funding(&*self.host, holder, item.index as u32, p);
        if let Some(b) = self.budget() {
            b.funding_percentage_changed(self);
        }
        true
    }

    /// `RemoveLocallyFundedObject(holder, purpose)` (§7).
    pub fn remove_locally_funded_object(&self, holder: u64, purpose: u32) -> bool {
        if !self.is_registered(holder, purpose) {
            return false;
        }
        // [compat] this sets the building's cost property to 0.
        self.set_local_full_funding(holder, 0, purpose);
        {
            let mut d = self.d.borrow_mut();
            d.locals.retain(|&e| e != (holder, purpose));
        }
        self.host.release(holder);
        if self.d.borrow().locals.is_empty() && self.ideal_monthly_funding() == 0 {
            self.set_funding_percentage(1.0, 0);
        }
        true
    }

    /// `SetLocalFundingPercent(holder, q, purpose)` (§7.3).
    pub fn set_local_funding_percent(&self, holder: u64, q: f32, purpose: u32) -> bool {
        let (p, p_max) = {
            let d = self.d.borrow();
            (d.p, d.p_max)
        };
        if !(q >= 0.0 && q <= p_max) || !self.is_registered(holder, purpose) {
            return false;
        }
        let Some(item) = item_for_purpose(&*self.host, holder, purpose) else {
            return false;
        };
        let cost = item.cost as f64;
        let new_c = round_half_up(f64::from(q) * 100.0 * cost);
        let old_c = round_half_up(cost * f64::from(item.local_funding) * 100.0);
        let mut q2 = p;
        if new_c != old_c {
            let f = self.ideal_monthly_funding();
            q2 = if f == 0 {
                q
            } else {
                // Foundations 18.3: percentage scaling uses the float32 constant.
                (((new_c - old_c) as f64 + (f64::from(p) * 100.0 * f as f64 + 0.5).floor()) / f as f64
                    * f64::from(0.01f32)) as f32
            };
            if let Some(occupant) = self.host.occupant_of(holder) {
                self.host.post(&BudgetMessage {
                    msg_type: msg::BUDGET_FOR_OCCUPANT_CHANGED,
                    unknown: occupant,
                    ..Default::default()
                });
            }
        }
        change_item_local_funding(&*self.host, holder, item.index as u32, q);
        self.recompute_line(item.line_item);
        if q2 != p {
            self.set_pct(q2, 0, false);
        }
        true
    }

    /// `SetLocalFullFunding(holder, cost', purpose)` (§7.4).
    pub fn set_local_full_funding(&self, holder: u64, cost: i64, purpose: u32) -> bool {
        if !self.is_registered(holder, purpose) {
            return false;
        }
        let Some(item) = item_for_purpose(&*self.host, holder, purpose) else {
            return false;
        };
        change_item_cost(&*self.host, holder, item.index as u32, cost);
        let delta = cost.wrapping_sub(item.cost);
        let delta_c = round_half_up(f64::from(item.local_funding) * 100.0 * delta as f64);
        let f = self.ideal_monthly_funding(); // before the change
        let p = self.funding_percentage();
        let q2 = if f.wrapping_add(delta) == 0 {
            1.0
        } else {
            (((f as f64 * f64::from(p) * 100.0 + 0.5).floor() + delta_c as f64) / (f + delta) as f64
                * f64::from(0.01f32)) as f32
        };
        if let Some(line) = self.line_item(item.line_item) {
            line.add_to_full_expenses(delta);
            line.set_current(line.current_expenses().wrapping_add(delta_c / 100).max(0));
        }
        // [compat] subscribers also get the line's IdealMonthlyFundingChanged.
        self.notify(msg::IDEAL_MONTHLY_FUNDING_CHANGED, 0);
        self.set_pct(q2, 0, false);
        true
    }

    pub fn local_funding_percent(&self, holder: u64, purpose: u32) -> f32 {
        self.find_local_funding_percent(holder, purpose).unwrap_or(0.0)
    }

    /// `GetLocalFundingPercent`'s "found" case (budget erratum): the item's
    /// local funding fraction if the object is registered and its budget item
    /// exists, else `None`.
    pub fn find_local_funding_percent(&self, holder: u64, purpose: u32) -> Option<f32> {
        if !self.is_registered(holder, purpose) {
            return None;
        }
        item_for_purpose(&*self.host, holder, purpose).map(|i| i.local_funding)
    }

    pub fn local_full_funding(&self, holder: u64, purpose: u32) -> i64 {
        if !self.is_registered(holder, purpose) {
            return 0;
        }
        item_for_purpose(&*self.host, holder, purpose).map_or(0, |i| i.cost)
    }

    pub fn is_locally_funded_object(&self, holder: u64, purpose: u32) -> bool {
        self.is_registered(holder, purpose)
    }

    /// `RecalculateAllLineItemCurrentExpenses` (§7.2).
    pub fn recalculate_all_line_item_current_expenses(&self) -> bool {
        let lines = self.lines_snapshot();
        let id = self.id();
        let mut sums: BTreeMap<u32, i64> = lines
            .iter()
            .filter(|l| l.is_local() && l.item_type() == TYPE_EXPENSE)
            .map(|l| (l.id(), 0i64))
            .collect();
        let mut n: i64 = lines.iter().filter(|l| !l.is_local()).count() as i64;
        for (holder, _) in self.locals_snapshot() {
            // All of the holder's items, not only the registered purpose
            // ([compat]: a holder registered twice is counted twice).
            for item in budget_items(&*self.host, holder).unwrap_or_default() {
                if item.department_id != id {
                    continue;
                }
                if let Some(s) = sums.get_mut(&item.line_item) {
                    *s = (item.cost as f32 * item.local_funding * 100.0 + *s as f32) as i64;
                    n += 1;
                }
            }
        }
        for l in &lines {
            if let Some(s) = sums.get(&l.id()) {
                l.set_current((s / 100).max(0));
            }
        }
        let c: i64 = lines.iter().map(|l| l.current_expenses()).sum();
        let f: i64 = lines.iter().map(|l| l.full_expenses()).sum();
        if f > 0 {
            let (p, p_max) = {
                let d = self.d.borrow();
                (d.p, d.p_max)
            };
            let a = c as f32 * 100.0 / f as f32;
            if (a - 100.0 * p).abs() > 0.5 && (c as f32 - p * f as f32).abs() > (n + 2) as f32 {
                let clamped = a.max(0.0).min(100.0 * p_max);
                self.set_pct(clamped / 100.0, 0, true);
            }
        }
        true
    }

    /// Single-line variant of §7.2 (used by §7.3): recomputes one local
    /// Expense line, without the re-synchronisation step.
    fn recompute_line(&self, line_id: u32) {
        let Some(line) = self.line_item(line_id) else { return };
        if !line.is_local() || line.item_type() != TYPE_EXPENSE {
            return;
        }
        let id = self.id();
        let mut sum: i64 = 0;
        for (holder, _) in self.locals_snapshot() {
            for item in budget_items(&*self.host, holder).unwrap_or_default() {
                if item.department_id == id && item.line_item == line_id {
                    sum = (item.cost as f32 * item.local_funding * 100.0 + sum as f32) as i64;
                }
            }
        }
        line.set_current((sum / 100).max(0));
    }

    pub fn total_spending(&self) -> i64 {
        self.d.borrow().total_spending
    }

    pub fn set_total_spending(&self, v: i64) -> bool {
        self.d.borrow_mut().total_spending = v;
        true
    }

    pub(crate) fn add_spending(&self, v: i64) {
        let mut d = self.d.borrow_mut();
        d.total_spending = d.total_spending.wrapping_add(v);
    }

    /// Holders registered with `purpose` (the caller takes its own references).
    pub fn locally_funded_items_by_purpose(&self, purpose: u32) -> Vec<u64> {
        self.d
            .borrow()
            .locals
            .iter()
            .filter(|(_, p)| *p == purpose)
            .map(|(h, _)| *h)
            .collect()
    }

    pub fn has_locally_funded_items_by_purpose(&self, purpose: u32) -> bool {
        self.d.borrow().locals.iter().any(|(_, p)| *p == purpose)
    }

    // ---- §7.1 ----

    /// SetPct(q, arg, redistribute).
    pub(crate) fn set_pct(&self, q: f32, arg: i64, redistribute: bool) {
        {
            let mut d = self.d.borrow_mut();
            let q = if q < 0.0 {
                0.0
            } else if q > d.p_max {
                d.p_max
            } else {
                q
            };
            if d.fixed || q == d.p {
                return;
            }
            d.p = q;
        }
        let q = self.funding_percentage();
        for l in self.lines_snapshot() {
            if !l.is_local() && l.item_type() != TYPE_INCOME {
                l.set_current(((l.full_expenses() as f32 * q + 0.5) as i64).max(0));
            }
        }
        if redistribute {
            self.distribute_to_buildings();
        }
        self.notify(msg::FUNDING_PERCENTAGE_CHANGED, arg);
    }

    /// DistributeToBuildings (§7.1): spreads the department percentage over
    /// the locally funded buildings in whole simoleons.
    fn distribute_to_buildings(&self) {
        let locals = self.locals_snapshot();
        if locals.is_empty() {
            return;
        }
        let (p, p_max) = {
            let d = self.d.borrow();
            (f64::from(d.p), f64::from(d.p_max))
        };
        struct Obj {
            holder: u64,
            index: u32,
            line: u32,
            c: i64,
            cur: i64,
            hi: i64,
            active: bool,
        }
        let mut objs: Vec<Obj> = Vec::new();
        for (holder, purpose) in locals {
            if let Some(item) = item_for_purpose(&*self.host, holder, purpose) {
                let c = i64::from(item.cost as i32); // low 32 bits
                objs.push(Obj {
                    holder,
                    index: item.index as u32,
                    line: item.line_item,
                    c,
                    cur: round_half_up(c as f64 * f64::from(item.local_funding)),
                    hi: round_half_up(c as f64 * p_max),
                    // [choice] zero-cost objects take no part in the loop.
                    active: c != 0,
                });
            }
        }
        let total_c: i64 = objs.iter().map(|o| o.c).sum();
        let mut r: i64 = round_half_up(total_c as f64 * p) - objs.iter().map(|o| o.cur).sum::<i64>();
        let mut active_cost: i64 = objs.iter().filter(|o| o.active).map(|o| o.c).sum();
        let mut passes = 0;
        while r != 0 && active_cost != 0 && objs.iter().any(|o| o.active) && passes < MAX_DISTRIBUTION_PASSES {
            passes += 1;
            let k = r as f64 / active_cost as f64;
            for o in objs.iter_mut().filter(|o| o.active) {
                let mut done = false;
                let mut d;
                if r < 0 {
                    d = ((o.c as f64 * k).floor() as i64).max(r);
                    if o.cur + d < 0 {
                        d = -o.cur;
                        done = true;
                    }
                } else {
                    d = ((o.c as f64 * k).ceil() as i64).min(r);
                    if o.cur + d > o.hi {
                        d = o.hi - o.cur;
                        done = true;
                    }
                }
                if done {
                    o.active = false;
                    active_cost -= o.c;
                }
                o.cur += d;
                r -= d;
                if r == 0 {
                    break;
                }
            }
        }
        for l in self.lines_snapshot() {
            if l.is_local() {
                l.set_current(0);
            }
        }
        for o in &objs {
            if o.c != 0 {
                // [choice] zero-cost objects keep their local funding.
                change_item_local_funding(&*self.host, o.holder, o.index, (o.cur as f64 / o.c as f64) as f32);
            }
            if let Some(l) = self.line_item(o.line) {
                l.set_current(l.current_expenses().wrapping_add(o.cur).max(0));
            }
        }
    }

    /// Releases every line item and locally funded object (budget shutdown).
    pub(crate) fn release_all(&self) {
        self.settle_lines();
        let (lines, locals) = {
            let mut d = self.d.borrow_mut();
            (std::mem::take(&mut d.lines), std::mem::take(&mut d.locals))
        };
        for (_, l) in lines {
            l.set_owner(None);
            self.host.release(l.token());
        }
        for (h, _) in locals {
            self.host.release(h);
        }
    }
}

impl Drop for Department {
    fn drop(&mut self) {
        let d = self.d.get_mut();
        for (_, l) in std::mem::take(&mut d.lines) {
            self.host.release(l.token());
        }
        for l in std::mem::take(&mut d.pending_lines) {
            self.host.release(l.token());
        }
        for (h, _) in std::mem::take(&mut d.locals) {
            self.host.release(h);
        }
    }
}

impl SaveObject for Department {
    fn class_id(&self) -> u32 {
        CLSID_DEPARTMENT
    }

    fn write(&self, out: &mut dyn OutStream) -> bool {
        self.settle_lines();
        let d = self.d.borrow().clone();
        out.put_u16(DEPARTMENT_RECORD_VERSION);
        out.put_bool(d.initialized);
        out.put_u32(d.id);
        out.put_str(&d.name);
        out.put_u32(d.budget_group);
        out.put_f32(d.p);
        out.put_f32(d.p_max);
        out.put_i64(d.total_spending);
        out.put_bool(d.fixed);
        out.put_u32(d.lines.len() as u32);
        for line in d.lines.values() {
            out.put_ref(&RefValue::Object(line.clone() as Rc<dyn SaveObject>));
        }
        out.put_u32(d.locals.len() as u32);
        // Section 7.1: the original registry orders holders by address. Its
        // section 11.2 references must follow that order, independently of
        // the insertion-order choice used for funding remainder distribution.
        let mut locals = d.locals;
        locals.sort_unstable();
        for (holder, purpose) in &locals {
            out.put_ref(&RefValue::Host {
                token: *holder,
                class_id: self.host.class_of(*holder),
            });
            out.put_u32(*purpose);
        }
        out.put_u32(d.name_instance);
        out.put_u32(d.name_group);
        out.error() == 0
    }

    fn read(&self, inp: &mut dyn InStream) -> bool {
        let host = self.host.clone();
        let mut lines: Vec<Rc<LineItem>> = Vec::new();
        let mut locals: Vec<(u64, u32)> = Vec::new();
        // Any reference taken before a failure is dropped again.
        let release_partial = |lines: &mut Vec<Rc<LineItem>>, locals: &mut Vec<(u64, u32)>| {
            lines.clear();
            for (h, _) in locals.drain(..) {
                host.release(h);
            }
        };
        let parsed = (|| -> Option<DeptData> {
            let version = inp.get_u16()?;
            if version > DEPARTMENT_RECORD_VERSION {
                return None; // §11.2: version >= 11 fails
            }
            let mut d = DeptData {
                initialized: inp.get_bool()?,
                id: inp.get_u32()?,
                ..Default::default()
            };
            d.name = inp.get_str()?;
            d.budget_group = inp.get_u32()?;
            d.p = inp.get_f32()?;
            d.p_max = inp.get_f32()?;
            if version < 10 {
                // Older records store the percentages in percent.
                d.p *= 0.01;
                d.p_max *= 0.01;
            }
            d.total_spending = inp.get_i64()?;
            d.fixed = inp.get_bool()?;
            for _ in 0..inp.get_u32()? {
                match inp.get_ref()? {
                    r @ RefValue::Object(_) => {
                        if let Some(l) = r.object::<LineItem>() {
                            lines.push(l);
                        }
                    }
                    RefValue::Host { token, .. } => self.host.release(token),
                    RefValue::Null => {}
                }
            }
            for _ in 0..inp.get_u32()? {
                let holder = inp.get_ref_as(IID_PROPERTY_HOLDER)?;
                let purpose = inp.get_u32()?;
                if let RefValue::Host { token, .. } = holder {
                    locals.push((token, purpose));
                }
            }
            d.name_instance = inp.get_u32()?;
            d.name_group = inp.get_u32()?;
            Some(d)
        })();
        let Some(mut d) = parsed else {
            release_partial(&mut lines, &mut locals);
            return false;
        };
        if inp.error() != 0 {
            release_partial(&mut lines, &mut locals);
            return false;
        }
        if d.name_group != 0 && d.name_instance != 0 {
            d.name = self.host.ltext(d.name_group, d.name_instance);
        }
        // Drop whatever this object held before (normally nothing: fresh instance).
        self.release_all();
        let me = self.me();
        for l in lines {
            // Keyed by the line item's own ID (§11.2) on first use; the
            // department holds a reference from now on.
            self.host.add_ref(l.token());
            l.set_owner(me.as_ref());
            d.pending_lines.push(l);
        }
        d.locals = locals;
        *self.d.borrow_mut() = d;
        true
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
