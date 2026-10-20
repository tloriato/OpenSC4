// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The budget subsystem (`docs/specs/budget.md`) over the C ABI. See the
//! "budget" section of `include/fc4.h`.
//!
//! Handles: `fc4_budget*`, `fc4_department*` and `fc4_line_item*` are boxed
//! `Rc`s owned by the GZCOM objects that front the core objects. Objects the
//! core hands back (a department, a line item, a property holder) cross the
//! ABI as **tokens**: the host identity the object was created with.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use fc4_core::budget::{
    difficulty_multiplier, scan_tax, tax_income_of, BudgetHost, BudgetItem, BudgetMessage, BudgetSim, BudgetTuning,
    Department, LineItem, LotInfo, PropertySource, TaxLotStatus, CLSID_BUDGET_SIMULATOR, CLSID_DEPARTMENT,
    CLSID_LINE_ITEM, TAX_GROUPS,
};
use fc4_core::stream::SaveObject;

use crate::stream::{fc4_bytes_sink, fc4_istream, fc4_ostream, istream_ok, ostream_ok, CIn, COut};
use crate::{guard, slice_arg, str_arg};

// ---- types ----------------------------------------------------------------------------------------

#[repr(C)]
pub struct fc4_budget {
    _private: [u8; 0],
}

#[repr(C)]
pub struct fc4_department {
    _private: [u8; 0],
}

#[repr(C)]
pub struct fc4_line_item {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_budget_message {
    pub type_: u32,
    pub reserved: u32,
    pub data1: i64,
    pub data2: i64,
    pub data3: i64,
    pub void1: u64,
    pub unknown: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_lot_info {
    pub has_building: u8,
    pub livable: u8,
    pub land_value: u8,
    pub reserved: u8,
    pub purpose: u32,
    pub building_wealth: u32,
    pub lot_wealth: u32,
    pub building_value: i64,
    pub size_x: u32,
    pub size_z: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_budget_item {
    pub department_id: u32,
    pub line_item: u32,
    pub purpose: u32,
    pub index: i32,
    pub cost: i64,
    pub local_funding: f32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_loan_info {
    pub months_remaining: u32,
    pub years_remaining: u32,
    pub total_loan_amount: i64,
    pub remaining_payment: i64,
    pub monthly_payment: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_bond {
    pub due_month: i32,
    pub due_year: i32,
    pub amount: i64,
    pub full_cost: i64,
    pub remaining: i64,
    pub paid: i64,
    pub monthly_payment: i64,
    pub principal_left: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct fc4_budget_group {
    pub id: u32,
    pub department_count: u32,
    pub total_expense: i64,
    pub total_income: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_budget_state {
    pub struct_size: u32,
    pub initialized: u8,
    pub loaded_from_save: u8,
    pub agriculture_rate_fix: u8,
    pub reserved0: u8,
    pub panel_flag: u32,
    pub bond_count: u32,
    pub funds: i64,
    pub shadow_funds: i64,
    pub total_borrowed: i64,
    pub last_tax_income: i64,
    pub ytd_income: i64,
    pub ytd_expense: i64,
    pub ytd_tax_income: i64,
    pub ytd_ordinance_income: i64,
    pub ytd_ordinance_expense: i64,
    pub ytd_bond_payments: i64,
    pub last_month_income: i64,
    pub last_month_expense: i64,
    pub yearly_income: i64,
    pub yearly_expense: i64,
    pub tax_rate: [f32; 12],
    pub max_tax_rate: [f32; 12],
    pub weighted_assessed: [i64; 12],
    pub monthly_tax: [i64; 12],
    pub department_count: u32,
    pub group_count: u32,
    pub risky_money_counter: u32,
    pub reserved1: u32,
}

/// One lot as the tax scan saw it (`fc4_budget_shadow_tax`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_tax_lot {
    /// `FC4_TAX_LOT_*`: 0 counted, 1 no building, 2 not livable, 3 untaxed.
    pub status: u32,
    /// Tax group 0..11, or -1 (none). For a not-livable lot: the group it would have had.
    pub group: i32,
    /// Assessed value (§5.2); for a not-livable lot, what it would have added.
    pub assessed: i64,
    /// `lotTax` (0 unless counted).
    pub tax: f32,
    pub reserved: u32,
}

/// The result of `fc4_budget_shadow_tax` (§5.2, §6 `GetTaxIncome(t)`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct fc4_tax_scan {
    /// Caller sets = sizeof(fc4_tax_scan).
    pub struct_size: u32,
    /// Lots visited.
    pub lots: u32,
    /// DifficultyTaxMultiplier[difficulty].
    pub multiplier: f32,
    /// `total` (float32 running sum).
    pub total: f32,
    pub weighted_assessed: [i64; 12],
    /// `monthlyTax[t] = trunc(groupTax[t])`.
    pub monthly_tax: [i64; 12],
    /// `GetTaxIncome(t)` with the given rates.
    pub tax_income: [i64; 12],
    pub group_tax: [f32; 12],
    /// Lots counted per group.
    pub counted: [u32; 12],
    /// `ComputeTax`'s result, `trunc(total)`.
    pub total_tax: i64,
}

pub type fc4_tax_lot_callback =
    Option<unsafe extern "C" fn(ctx: *mut c_void, index: u32, lot: *const fc4_lot_info, result: *const fc4_tax_lot)>;

pub type fc4_lot_visitor = Option<unsafe extern "C" fn(ctx: *mut c_void, lot: *const fc4_lot_info)>;
pub type fc4_token_callback = Option<unsafe extern "C" fn(ctx: *mut c_void, token: u64)>;
pub type fc4_budget_item_callback = Option<unsafe extern "C" fn(ctx: *mut c_void, item: *const fc4_budget_item)>;
pub type fc4_loan_callback = Option<unsafe extern "C" fn(ctx: *mut c_void, loan: *const fc4_loan_info)>;
pub type fc4_group_callback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        group: *const fc4_budget_group,
        name: *const u8,
        name_len: usize,
        departments: *const u32,
    ),
>;

type U = *mut c_void;

/// What the budget needs from the game (fc4.h: `fc4_budget_host`). Missing
/// callbacks behave as "nothing there" (0, false, absent).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct fc4_budget_host {
    pub struct_size: u32,
    pub user: *mut c_void,
    pub post: Option<unsafe extern "C" fn(U, *const fc4_budget_message)>,
    pub show_budget_window: Option<unsafe extern "C" fn(U, u32)>,
    pub difficulty: Option<unsafe extern "C" fn(U) -> u32>,
    pub sim_date: Option<unsafe extern "C" fn(U, *mut i32, *mut i32)>,
    pub city_established: Option<unsafe extern "C" fn(U) -> bool>,
    pub city_size: Option<unsafe extern "C" fn(U, *mut f32, *mut f32)>,
    pub ordinance_income: Option<unsafe extern "C" fn(U) -> i64>,
    pub ordinance_expense: Option<unsafe extern "C" fn(U) -> i64>,
    pub neighbor_income: Option<unsafe extern "C" fn(U) -> i64>,
    pub neighbor_expense: Option<unsafe extern "C" fn(U) -> i64>,
    pub for_each_lot: Option<unsafe extern "C" fn(U, fc4_lot_visitor, *mut c_void)>,
    pub record_history: Option<unsafe extern "C" fn(U, u32, i32)>,
    pub set_history_queue_length: Option<unsafe extern "C" fn(U, u32, u32)>,
    pub try_start_disaster: Option<unsafe extern "C" fn(U, u32, f32, f32) -> bool>,
    pub session_seed: Option<unsafe extern "C" fn(U) -> u32>,
    pub ltext: Option<unsafe extern "C" fn(U, u32, u32, fc4_bytes_sink, *mut c_void) -> bool>,
    pub building_type_name: Option<unsafe extern "C" fn(U, u32, fc4_bytes_sink, *mut c_void) -> bool>,
    pub get_u32s: Option<unsafe extern "C" fn(U, u64, u32, *mut u32, usize) -> i32>,
    pub get_i64s: Option<unsafe extern "C" fn(U, u64, u32, *mut i64, usize) -> i32>,
    pub get_f32s: Option<unsafe extern "C" fn(U, u64, u32, *mut f32, usize) -> i32>,
    pub set_u32s: Option<unsafe extern "C" fn(U, u64, u32, *const u32, usize) -> bool>,
    pub set_i64s: Option<unsafe extern "C" fn(U, u64, u32, *const i64, usize) -> bool>,
    pub set_f32s: Option<unsafe extern "C" fn(U, u64, u32, *const f32, usize) -> bool>,
    pub copy_property: Option<unsafe extern "C" fn(U, u64, u64, u32) -> bool>,
    pub occupant_of: Option<unsafe extern "C" fn(U, u64) -> u64>,
    pub class_of: Option<unsafe extern "C" fn(U, u64) -> u32>,
    pub add_ref: Option<unsafe extern "C" fn(U, u64)>,
    pub release: Option<unsafe extern "C" fn(U, u64)>,
    pub create_object: Option<unsafe extern "C" fn(U, u32) -> *mut c_void>,
    /// ABI 50 (EX39): 1 array, 0 scalar, -1 absent; NULL counts every property as an array.
    pub property_is_array: Option<unsafe extern "C" fn(U, u64, u32) -> i32>,
}

const _: () = {
    assert!(std::mem::size_of::<fc4_budget_message>() == 48);
    assert!(std::mem::size_of::<fc4_lot_info>() == 32);
    assert!(std::mem::size_of::<fc4_budget_item>() == 32);
    assert!(std::mem::size_of::<fc4_loan_info>() == 32);
    assert!(std::mem::size_of::<fc4_bond>() == 56);
    assert!(std::mem::size_of::<fc4_budget_group>() == 24);
    assert!(std::mem::size_of::<fc4_budget_state>() == 432);
    assert!(std::mem::size_of::<fc4_tax_lot>() == 24);
    assert!(std::mem::size_of::<fc4_tax_scan>() == 408);
    assert!(std::mem::size_of::<fc4_budget_host>() == if cfg!(target_pointer_width = "32") { 128 } else { 256 });
};

// ---- host adapter -----------------------------------------------------------------------------------

struct CBudgetHost(fc4_budget_host);

macro_rules! cb {
    ($s:expr, $f:ident, $default:expr $(, $arg:expr)*) => {
        // SAFETY: whoever created the object promised valid callbacks.
        match $s.0.$f { Some(f) => unsafe { f($s.0.user $(, $arg)*) }, None => $default }
    };
}

unsafe extern "C" fn collect(ctx: *mut c_void, data: *const u8, len: usize) {
    let v = &mut *(ctx as *mut Vec<u8>);
    if !data.is_null() && len > 0 {
        v.extend_from_slice(std::slice::from_raw_parts(data, len));
    }
}

unsafe extern "C" fn visit_lot(ctx: *mut c_void, lot: *const fc4_lot_info) {
    let f = &mut *(ctx as *mut &mut dyn FnMut(&LotInfo));
    if let Some(l) = lot.as_ref() {
        f(&LotInfo {
            has_building: l.has_building != 0,
            livable: l.livable != 0,
            purpose: l.purpose,
            building_wealth: l.building_wealth,
            lot_wealth: l.lot_wealth,
            building_value: l.building_value,
            land_value: l.land_value,
            size_x: l.size_x,
            size_z: l.size_z,
        });
    }
}

/// Reads an array property in at most two calls (first into a small buffer).
fn get_array<T: Copy + Default>(
    f: Option<unsafe extern "C" fn(U, u64, u32, *mut T, usize) -> i32>,
    user: U,
    holder: u64,
    prop: u32,
) -> Option<Vec<T>> {
    let f = f?;
    let mut buf = vec![T::default(); 16];
    // SAFETY: valid callback; `buf` has room for `buf.len()` values.
    let n = unsafe { f(user, holder, prop, buf.as_mut_ptr(), buf.len()) };
    if n < 0 {
        return None;
    }
    let n = n as usize;
    if n > buf.len() {
        buf = vec![T::default(); n];
        // SAFETY: as above.
        let m = unsafe { f(user, holder, prop, buf.as_mut_ptr(), buf.len()) };
        if m < 0 {
            return None;
        }
        buf.truncate((m as usize).min(n));
    } else {
        buf.truncate(n);
    }
    Some(buf)
}

impl CBudgetHost {
    fn string(
        &self,
        f: Option<unsafe extern "C" fn(U, u32, u32, fc4_bytes_sink, *mut c_void) -> bool>,
        a: u32,
        b: u32,
    ) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        if let Some(f) = f {
            // SAFETY: valid callback; the sink only appends to `v` during the call.
            unsafe { f(self.0.user, a, b, Some(collect), &mut v as *mut Vec<u8> as *mut c_void) };
        }
        v
    }
}

impl BudgetHost for CBudgetHost {
    fn post(&self, m: &BudgetMessage) {
        let msg = fc4_budget_message {
            type_: m.msg_type,
            reserved: 0,
            data1: m.data1,
            data2: m.data2,
            data3: m.data3,
            void1: m.void1,
            unknown: m.unknown,
        };
        cb!(self, post, (), &msg)
    }
    fn show_budget_window(&self, panel: u32) {
        cb!(self, show_budget_window, (), panel)
    }
    fn difficulty(&self) -> u32 {
        cb!(self, difficulty, 0)
    }
    fn sim_date(&self) -> (i32, i32) {
        let (mut y, mut m) = (2000, 1);
        cb!(self, sim_date, (), &mut y, &mut m);
        (y, m)
    }
    fn city_established(&self) -> bool {
        cb!(self, city_established, false)
    }
    fn city_size(&self) -> (f32, f32) {
        let (mut x, mut z) = (0.0f32, 0.0f32);
        cb!(self, city_size, (), &mut x, &mut z);
        (x, z)
    }
    fn ordinance_monthly_income(&self) -> i64 {
        cb!(self, ordinance_income, 0)
    }
    fn ordinance_monthly_expense(&self) -> i64 {
        cb!(self, ordinance_expense, 0)
    }
    fn neighbor_deal_income(&self) -> i64 {
        cb!(self, neighbor_income, 0)
    }
    fn neighbor_deal_expense(&self) -> i64 {
        cb!(self, neighbor_expense, 0)
    }
    fn for_each_lot(&self, f: &mut dyn FnMut(&LotInfo)) {
        let mut f: &mut dyn FnMut(&LotInfo) = f;
        let ctx = &mut f as *mut &mut dyn FnMut(&LotInfo) as *mut c_void;
        cb!(self, for_each_lot, (), Some(visit_lot), ctx)
    }
    fn record_history(&self, series: u32, value: i32) {
        cb!(self, record_history, (), series, value)
    }
    fn set_history_queue_length(&self, series: u32, length: u32) {
        cb!(self, set_history_queue_length, (), series, length)
    }
    fn try_start_disaster(&self, id: u32, x: f32, z: f32) -> bool {
        cb!(self, try_start_disaster, false, id, x, z)
    }
    fn session_seed(&self) -> u32 {
        cb!(self, session_seed, 0)
    }
    fn ltext(&self, group: u32, instance: u32) -> Vec<u8> {
        self.string(self.0.ltext, group, instance)
    }
    fn building_type_name(&self, building_type: u32) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        if let Some(f) = self.0.building_type_name {
            // SAFETY: valid callback; the sink only appends to `v` during the call.
            unsafe {
                f(
                    self.0.user,
                    building_type,
                    Some(collect),
                    &mut v as *mut Vec<u8> as *mut c_void,
                )
            };
        }
        v
    }
    fn get_u32s(&self, holder: u64, prop: u32) -> Option<Vec<u32>> {
        get_array(self.0.get_u32s, self.0.user, holder, prop)
    }
    fn get_i64s(&self, holder: u64, prop: u32) -> Option<Vec<i64>> {
        get_array(self.0.get_i64s, self.0.user, holder, prop)
    }
    fn get_f32s(&self, holder: u64, prop: u32) -> Option<Vec<f32>> {
        get_array(self.0.get_f32s, self.0.user, holder, prop)
    }
    fn set_u32s(&self, holder: u64, prop: u32, values: &[u32]) -> bool {
        cb!(self, set_u32s, false, holder, prop, values.as_ptr(), values.len())
    }
    fn set_i64s(&self, holder: u64, prop: u32, values: &[i64]) -> bool {
        cb!(self, set_i64s, false, holder, prop, values.as_ptr(), values.len())
    }
    fn set_f32s(&self, holder: u64, prop: u32, values: &[f32]) -> bool {
        cb!(self, set_f32s, false, holder, prop, values.as_ptr(), values.len())
    }
    fn copy_property(&self, src: u64, dst: u64, prop: u32) -> bool {
        cb!(self, copy_property, false, src, dst, prop)
    }
    fn property_is_array(&self, holder: u64, prop: u32) -> Option<bool> {
        match cb!(self, property_is_array, -1, holder, prop) {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn occupant_of(&self, holder: u64) -> Option<u64> {
        let t = cb!(self, occupant_of, 0, holder);
        (t != 0).then_some(t)
    }
    fn class_of(&self, token: u64) -> u32 {
        cb!(self, class_of, 0, token)
    }
    fn add_ref(&self, token: u64) {
        if token != 0 {
            cb!(self, add_ref, (), token)
        }
    }
    fn release(&self, token: u64) {
        if token != 0 {
            cb!(self, release, (), token)
        }
    }
    fn create_object(&self, class_id: u32) -> Option<Rc<dyn SaveObject>> {
        let handle = cb!(self, create_object, std::ptr::null_mut(), class_id);
        // SAFETY: the host returns the core handle of the object it created.
        unsafe { decode_handle(class_id, handle) }
    }
}

/// Tuning properties read from a host property holder (the tuning exemplar).
struct HolderProps<'a> {
    host: &'a dyn BudgetHost,
    holder: u64,
}

impl PropertySource for HolderProps<'_> {
    fn get_i64s(&self, prop: u32) -> Option<Vec<i64>> {
        self.host.get_i64s(self.holder, prop)
    }
    fn get_f32s(&self, prop: u32) -> Option<Vec<f32>> {
        self.host.get_f32s(self.holder, prop)
    }
}

// ---- handles ------------------------------------------------------------------------------------------

/// Turns a core handle (`fc4_budget*`, `fc4_department*`, `fc4_line_item*`)
/// into its object, by class. SAFETY: `handle` is NULL or a live handle of
/// that class.
pub(crate) unsafe fn decode_handle(class_id: u32, handle: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    if handle.is_null() {
        return None;
    }
    match class_id {
        CLSID_BUDGET_SIMULATOR => Some((*(handle as *const Rc<BudgetSim>)).clone() as Rc<dyn SaveObject>),
        CLSID_DEPARTMENT => Some((*(handle as *const Rc<Department>)).clone() as Rc<dyn SaveObject>),
        CLSID_LINE_ITEM => Some((*(handle as *const Rc<LineItem>)).clone() as Rc<dyn SaveObject>),
        _ => None,
    }
}

fn decoder(class_id: u32, handle: *mut c_void) -> Option<Rc<dyn SaveObject>> {
    // SAFETY: handles come from the host's reference reads, which only report
    // core handles of our own objects.
    unsafe { decode_handle(class_id, handle) }
}

unsafe fn budget_of<'a>(b: *const fc4_budget) -> Option<&'a Rc<BudgetSim>> {
    (b as *const Rc<BudgetSim>).as_ref()
}

unsafe fn dept_of<'a>(d: *const fc4_department) -> Option<&'a Rc<Department>> {
    (d as *const Rc<Department>).as_ref()
}

unsafe fn line_of<'a>(l: *const fc4_line_item) -> Option<&'a Rc<LineItem>> {
    (l as *const Rc<LineItem>).as_ref()
}

fn with_budget<R>(b: *const fc4_budget, fallback: R, f: impl FnOnce(&Rc<BudgetSim>) -> R) -> R {
    // SAFETY: documented precondition: NULL or a live fc4_budget*.
    match unsafe { budget_of(b) } {
        Some(x) => guard(fallback, || f(x)),
        None => fallback,
    }
}

fn with_dept<R>(d: *const fc4_department, fallback: R, f: impl FnOnce(&Rc<Department>) -> R) -> R {
    // SAFETY: as above.
    match unsafe { dept_of(d) } {
        Some(x) => guard(fallback, || f(x)),
        None => fallback,
    }
}

fn with_line<R>(l: *const fc4_line_item, fallback: R, f: impl FnOnce(&Rc<LineItem>) -> R) -> R {
    // SAFETY: as above.
    match unsafe { line_of(l) } {
        Some(x) => guard(fallback, || f(x)),
        None => fallback,
    }
}

fn host_of(h: *const fc4_budget_host) -> Option<Rc<dyn BudgetHost>> {
    // SAFETY: NULL or a valid struct (precondition).
    let h = unsafe { h.as_ref() }?;
    if (h.struct_size as usize) < std::mem::size_of::<fc4_budget_host>() {
        return None;
    }
    Some(Rc::new(CBudgetHost(*h)))
}

unsafe fn sink_bytes(bytes: &[u8], sink: fc4_bytes_sink, ctx: *mut c_void) {
    if let Some(f) = sink {
        f(ctx, bytes.as_ptr(), bytes.len());
    }
}

fn item_to_c(i: &BudgetItem) -> fc4_budget_item {
    fc4_budget_item {
        department_id: i.department_id,
        line_item: i.line_item,
        purpose: i.purpose,
        index: i.index,
        cost: i.cost,
        local_funding: i.local_funding,
        reserved: 0,
    }
}

fn item_from_c(i: &fc4_budget_item) -> BudgetItem {
    BudgetItem {
        department_id: i.department_id,
        line_item: i.line_item,
        purpose: i.purpose,
        cost: i.cost,
        index: i.index,
        local_funding: i.local_funding,
    }
}

unsafe fn emit_tokens(tokens: &[u64], cb: fc4_token_callback, ctx: *mut c_void) -> usize {
    if let Some(f) = cb {
        for &t in tokens {
            f(ctx, t);
        }
    }
    tokens.len()
}

// ---- lifetime -----------------------------------------------------------------------------------------

macro_rules! lifetime {
    ($create:ident, $destroy:ident, $handle:ty, $t:ty) => {
        lifetime!($create, $destroy, $handle, $t, new);
    };
    ($create:ident, $destroy:ident, $handle:ty, $t:ty, $ctor:ident) => {
        /// Creates the core object fronted by the GZCOM object `token`.
        #[no_mangle]
        pub unsafe extern "C" fn $create(host: *const fc4_budget_host, token: u64) -> *mut $handle {
            guard(std::ptr::null_mut(), || match host_of(host) {
                Some(h) => Box::into_raw(Box::new(<$t>::$ctor(h, token))) as *mut $handle,
                None => std::ptr::null_mut(),
            })
        }

        #[no_mangle]
        pub unsafe extern "C" fn $destroy(handle: *mut $handle) {
            if !handle.is_null() {
                guard((), || drop(Box::from_raw(handle as *mut Rc<$t>)));
            }
        }
    };
}

// The budget draws from the process-wide RiskyMoney generator (RN3).
lifetime!(
    fc4_budget_create,
    fc4_budget_destroy,
    fc4_budget,
    BudgetSim,
    process_wide
);
lifetime!(
    fc4_department_create,
    fc4_department_destroy,
    fc4_department,
    Department
);
lifetime!(fc4_line_item_create, fc4_line_item_destroy, fc4_line_item, LineItem);

// ---- persistence ------------------------------------------------------------------------------------------

macro_rules! persistence {
    ($write:ident, $read:ident, $handle:ty, $with:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $write(handle: *const $handle, out: *const fc4_ostream) -> bool {
            let Some(o) = out.as_ref() else { return false };
            if !ostream_ok(o) {
                return false;
            }
            $with(handle, false, |x| x.write(&mut COut(o)))
        }

        #[no_mangle]
        pub unsafe extern "C" fn $read(handle: *mut $handle, inp: *const fc4_istream) -> bool {
            let Some(i) = inp.as_ref() else { return false };
            if !istream_ok(i) {
                return false;
            }
            $with(handle, false, |x| {
                x.read(&mut CIn {
                    stream: i,
                    decode: decoder,
                })
            })
        }
    };
}

persistence!(fc4_budget_write, fc4_budget_read, fc4_budget, with_budget);
persistence!(fc4_department_write, fc4_department_read, fc4_department, with_dept);
persistence!(fc4_line_item_write, fc4_line_item_read, fc4_line_item, with_line);

// ---- budget simulator: lifecycle and messages --------------------------------------------------------------

/// `Init` (core part). `tuning_holder` is the tuning exemplar as a property
/// holder token (read through the host's property callbacks); 0 = code defaults.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_init(b: *mut fc4_budget, tuning_holder: u64, fix_cs3_weight: bool) -> bool {
    with_budget(b, false, |x| {
        let tuning = if tuning_holder == 0 {
            BudgetTuning::default()
        } else {
            BudgetTuning::from_properties(
                &HolderProps {
                    host: &**x.host(),
                    holder: tuning_holder,
                },
                fix_cs3_weight,
            )
        };
        x.init(tuning)
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_budget_handle_message(b: *mut fc4_budget, msg_type: u32, data1: i64) -> bool {
    with_budget(b, true, |x| x.handle_message(msg_type, data1))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_budget_handle_cheat(
    b: *mut fc4_budget,
    cheat_id: u32,
    text: *const c_char,
    text_len: usize,
) -> bool {
    let text = guard(String::new(), || str_arg(text, text_len));
    with_budget(b, false, |x| x.handle_cheat(cheat_id, &text))
}

macro_rules! budget_fn {
    ($name:ident, $ret:ty, $fallback:expr, $method:ident $(, $arg:ident : $t:ty)*) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(b: *mut fc4_budget $(, $arg: $t)*) -> $ret {
            with_budget(b, $fallback, |x| x.$method($($arg),*))
        }
    };
}

budget_fn!(fc4_budget_shutdown, bool, false, shutdown);
budget_fn!(fc4_budget_is_initialized, bool, false, is_initialized);
budget_fn!(fc4_budget_simulate_day, (), (), simulate_day);
budget_fn!(fc4_budget_settle_month, (), (), settle_month);
budget_fn!(fc4_budget_compute_tax, i64, 0, compute_tax);

// ---- budget shadow (diagnostics) -----------------------------------------------------------------------------

fn lot_to_c(l: &LotInfo) -> fc4_lot_info {
    fc4_lot_info {
        has_building: u8::from(l.has_building),
        livable: u8::from(l.livable),
        land_value: l.land_value,
        reserved: 0,
        purpose: l.purpose,
        building_wealth: l.building_wealth,
        lot_wealth: l.lot_wealth,
        building_value: l.building_value,
        size_x: l.size_x,
        size_z: l.size_z,
    }
}

/// `ComputeTax` (§5.2) without a budget object: the same scan the budget
/// simulator runs, over the host's `for_each_lot`, with the tuning read from
/// `tuning_holder` (0 = code defaults) and the given difficulty and 12 group
/// rates (percent). Only `for_each_lot`, `get_i64s` and `get_f32s` of `host`
/// are called; nothing is stored, posted or written. `cb` (may be NULL) sees
/// every lot in visit order. Returns false (and leaves `out` alone) on bad
/// arguments.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_shadow_tax(
    host: *const fc4_budget_host,
    tuning_holder: u64,
    fix_cs3_weight: bool,
    difficulty: u32,
    rates: *const f32,
    cb: fc4_tax_lot_callback,
    ctx: *mut c_void,
    out: *mut fc4_tax_scan,
) -> bool {
    let Some(out) = out.as_mut() else { return false };
    if (out.struct_size as usize) < std::mem::size_of::<fc4_tax_scan>() || rates.is_null() {
        return false;
    }
    let Some(h) = host_of(host) else { return false };
    let mut r = [0f32; TAX_GROUPS];
    r.copy_from_slice(std::slice::from_raw_parts(rates, TAX_GROUPS));
    guard(false, || {
        let tuning = if tuning_holder == 0 {
            BudgetTuning::default()
        } else {
            BudgetTuning::from_properties(
                &HolderProps {
                    host: &*h,
                    holder: tuning_holder,
                },
                fix_cs3_weight,
            )
        };
        let mult = difficulty_multiplier(&tuning, difficulty);
        let mut index = 0u32;
        let mut on_lot = |lot: &LotInfo, t: &fc4_core::budget::TaxLot| {
            if let Some(f) = cb {
                let c = fc4_tax_lot {
                    status: match t.status {
                        TaxLotStatus::Counted => 0,
                        TaxLotStatus::NoBuilding => 1,
                        TaxLotStatus::NotLivable => 2,
                        TaxLotStatus::Untaxed => 3,
                    },
                    group: t.group.map_or(-1, |g| g as i32),
                    assessed: t.assessed,
                    tax: t.tax,
                    reserved: 0,
                };
                let l = lot_to_c(lot);
                // SAFETY: valid callback (precondition); both pointers live for the call.
                unsafe { f(ctx, index, &l, &c) };
            }
            index += 1;
        };
        let scan = scan_tax(&tuning, mult, &r, |visit| h.for_each_lot(visit), &mut on_lot);
        *out = fc4_tax_scan {
            struct_size: out.struct_size,
            lots: scan.lots,
            multiplier: mult,
            total: scan.total,
            weighted_assessed: scan.weighted_assessed,
            monthly_tax: scan.monthly_tax(),
            tax_income: std::array::from_fn(|t| tax_income_of(scan.weighted_assessed[t], mult, r[t])),
            group_tax: scan.group_tax,
            counted: scan.counted,
            total_tax: scan.total_tax(),
        };
        true
    })
}

// ---- budget simulator: funds and totals ---------------------------------------------------------------------

budget_fn!(fc4_budget_get_total_funds, i64, 0, funds);
budget_fn!(fc4_budget_set_total_funds, bool, false, set_total_funds, v: i64);
budget_fn!(fc4_budget_deposit_funds, bool, false, deposit_funds, v: i64);
budget_fn!(fc4_budget_withdraw_funds, bool, false, withdraw_funds, v: i64);
budget_fn!(fc4_budget_get_min_allowable_funds, i64, 0, min_allowable_funds);
budget_fn!(fc4_budget_get_ytd_income, i64, 0, ytd_income);
budget_fn!(fc4_budget_get_est_income, i64, 0, est_income);
budget_fn!(fc4_budget_get_ytd_expenses, i64, 0, ytd_expenses);
budget_fn!(fc4_budget_get_est_expenses, i64, 0, est_expenses);
budget_fn!(fc4_budget_get_total_monthly_expense, i64, 0, total_monthly_expense);
budget_fn!(fc4_budget_get_total_yearly_expense, i64, 0, total_yearly_expense);
budget_fn!(fc4_budget_get_total_monthly_income, i64, 0, total_monthly_income);
budget_fn!(fc4_budget_get_total_yearly_income, i64, 0, total_yearly_income);
budget_fn!(fc4_budget_get_tax_income, i64, 0, tax_income);
budget_fn!(fc4_budget_get_tax_income_for, i64, 0, tax_income_for, t: i32);
budget_fn!(fc4_budget_set_total_monthly_expense, i64, 0, set_total_monthly_expense, v: i64);
budget_fn!(fc4_budget_set_total_yearly_expense, i64, 0, set_total_yearly_expense, v: i64);
budget_fn!(fc4_budget_set_total_monthly_income, i64, 0, set_total_monthly_income, v: i64);
budget_fn!(fc4_budget_set_total_yearly_income, i64, 0, set_total_yearly_income, v: i64);
budget_fn!(fc4_budget_show_budget_window, bool, false, show_budget_window);
budget_fn!(fc4_budget_get_funding, i64, 0, funding);

// ---- budget simulator: tax and bonds -------------------------------------------------------------------------

budget_fn!(fc4_budget_get_tax_rate, f32, 0.0, tax_rate, t: u32);
budget_fn!(fc4_budget_set_tax_rate, bool, false, set_tax_rate, t: u32, r: f32);
budget_fn!(fc4_budget_get_weighted_assessed_tax_value, i64, 0, weighted_assessed_tax_value, t: u32);
budget_fn!(fc4_budget_get_bond_increment, i64, 0, bond_increment);
budget_fn!(fc4_budget_issue_bond, bool, false, issue_bond, amount: u32);
budget_fn!(fc4_budget_get_total_borrowed, i64, 0, total_borrowed);
budget_fn!(fc4_budget_get_current_borrowing_limit, i64, 0, current_borrowing_limit);
budget_fn!(fc4_budget_get_current_bond_limit, i64, 0, current_bond_limit);
budget_fn!(
    fc4_budget_get_current_max_outstanding_bonds_limit,
    i64,
    0,
    current_max_outstanding_bonds_limit
);
budget_fn!(
    fc4_budget_get_total_monthly_bond_payments,
    i64,
    0,
    total_monthly_bond_payments
);
budget_fn!(fc4_budget_get_loan_time_in_months, i32, 0, loan_time_in_months);
budget_fn!(fc4_budget_get_monthly_payment_for_loan, i64, 0, monthly_payment_for_loan, loan: i64);
budget_fn!(fc4_budget_get_full_cost_of_loan, i64, 0, full_cost_of_loan, loan: i64);

/// `GetAllLoans`: calls `cb` once per bond, in issue order; returns the count.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_all_loans(
    b: *const fc4_budget,
    cb: fc4_loan_callback,
    ctx: *mut c_void,
) -> usize {
    let loans = with_budget(b, Vec::new(), |x| x.all_loans());
    if let Some(f) = cb {
        for l in &loans {
            let c = fc4_loan_info {
                months_remaining: l.months_remaining,
                years_remaining: l.years_remaining,
                total_loan_amount: l.total_loan_amount,
                remaining_payment: l.remaining_payment,
                monthly_payment: l.monthly_payment,
            };
            f(ctx, &c);
        }
    }
    loans.len()
}

/// The bond at `index` (issue order); false if there is none.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_bond(b: *const fc4_budget, index: usize, out: *mut fc4_bond) -> bool {
    let Some(bond) = with_budget(b, None, |x| x.bonds().get(index).copied()) else {
        return false;
    };
    if let Some(o) = out.as_mut() {
        *o = fc4_bond {
            due_month: bond.due_month,
            due_year: bond.due_year,
            amount: bond.amount,
            full_cost: bond.full_cost,
            remaining: bond.remaining,
            paid: bond.paid,
            monthly_payment: bond.monthly_payment,
            principal_left: bond.principal_left,
        };
    }
    true
}

// ---- budget simulator: departments and groups ------------------------------------------------------------------

/// `CreateDepartmentBudget`: the department's token (borrowed), 0 on failure.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_create_department(b: *mut fc4_budget, id: u32, group: u32) -> u64 {
    with_budget(b, 0, |x| x.create_department_budget(id, group).map_or(0, |d| d.token()))
}

budget_fn!(fc4_budget_remove_department, bool, false, remove_department_budget, id: u32);

/// `GetDepartmentBudget(id)`: token (borrowed) or 0.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_department(b: *const fc4_budget, id: u32) -> u64 {
    with_budget(b, 0, |x| x.department(id).map_or(0, |d| d.token()))
}

/// `GetDepartmentBudget(name)`: prefix, case-insensitive; token or 0.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_department_by_name(
    b: *const fc4_budget,
    name: *const u8,
    name_len: usize,
) -> u64 {
    let name = slice_arg(name, name_len).map(<[u8]>::to_vec).unwrap_or_default();
    with_budget(b, 0, |x| x.department_by_name(&name).map_or(0, |d| d.token()))
}

/// `GetAllGroups`: calls `cb` per group in ascending ID with the group's name
/// and its department IDs (ascending, `group.department_count` of them).
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_all_groups(
    b: *const fc4_budget,
    cb: fc4_group_callback,
    ctx: *mut c_void,
) -> usize {
    let groups = with_budget(b, Vec::new(), |x| x.all_groups());
    if let Some(f) = cb {
        for g in &groups {
            let ids: Vec<u32> = g.departments.iter().copied().collect();
            let c = fc4_budget_group {
                id: g.id,
                department_count: ids.len() as u32,
                total_expense: g.total_expense,
                total_income: g.total_income,
            };
            f(ctx, &c, g.name.as_ptr(), g.name.len(), ids.as_ptr());
        }
    }
    groups.len()
}

#[no_mangle]
pub unsafe extern "C" fn fc4_budget_set_group_name(
    b: *mut fc4_budget,
    id: u32,
    name: *const u8,
    name_len: usize,
) -> bool {
    let name = slice_arg(name, name_len).map(<[u8]>::to_vec).unwrap_or_default();
    with_budget(b, false, |x| x.set_group_name(id, &name))
}

/// `GetDepartmentBudgetsInGroup`: calls `cb` with each department's token
/// (ascending ID); returns the count. The caller takes its own references.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_departments_in_group(
    b: *const fc4_budget,
    id: u32,
    cb: fc4_token_callback,
    ctx: *mut c_void,
) -> usize {
    let tokens: Vec<u64> = with_budget(b, Vec::new(), |x| {
        x.departments_in_group(id).iter().map(|d| d.token()).collect()
    });
    emit_tokens(&tokens, cb, ctx)
}

/// `NeededFundingChanged(dept)`; `dept` NULL (not one of ours) is ignored.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_needed_funding_changed(b: *mut fc4_budget, dept: *const fc4_department) {
    if let Some(d) = dept_of(dept) {
        let d = d.clone();
        with_budget(b, (), |x| x.needed_funding_changed(&d))
    }
}

#[no_mangle]
pub unsafe extern "C" fn fc4_budget_funding_percentage_changed(b: *mut fc4_budget, dept: *const fc4_department) {
    if let Some(d) = dept_of(dept) {
        let d = d.clone();
        with_budget(b, (), |x| x.funding_percentage_changed(&d))
    }
}

// ---- budget simulator: budget items ------------------------------------------------------------------------------

/// `GetBudgetItemInfo`: false if the holder has no items; otherwise calls `cb`
/// once per item and returns true.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_budget_item_info(
    b: *const fc4_budget,
    holder: u64,
    cb: fc4_budget_item_callback,
    ctx: *mut c_void,
) -> bool {
    let Some(items) = with_budget(b, None, |x| x.budget_item_info(holder)) else {
        return false;
    };
    if let Some(f) = cb {
        for i in &items {
            f(ctx, &item_to_c(i));
        }
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_budget_item_for_purpose(
    b: *const fc4_budget,
    holder: u64,
    purpose: u32,
    out: *mut fc4_budget_item,
) -> bool {
    let Some(item) = with_budget(b, None, |x| x.budget_item_for_purpose(holder, purpose)) else {
        return false;
    };
    if let Some(o) = out.as_mut() {
        *o = item_to_c(&item);
    }
    true
}

budget_fn!(fc4_budget_change_budget_item_line, bool, false, change_budget_item_line, holder: u64, index: u32, line: u32);
budget_fn!(fc4_budget_change_budget_item_cost, bool, false, change_budget_item_cost, holder: u64, index: u32, cost: i64);
budget_fn!(
    fc4_budget_change_budget_item_local_funding,
    bool,
    false,
    change_budget_item_local_funding,
    holder: u64,
    index: u32,
    fraction: f32
);
budget_fn!(fc4_budget_add_budget_item_to_budget, bool, false, add_budget_item_to_budget, holder: u64, index: u32);
budget_fn!(
    fc4_budget_remove_budget_item_from_budget,
    bool,
    false,
    remove_budget_item_from_budget,
    holder: u64,
    index: u32
);
budget_fn!(fc4_budget_copy_budget_item_properties, bool, false, copy_budget_item_properties, src: u64, dst: u64);

/// `GetLineItemFromBudgetItem`: the line item's token (borrowed) or 0.
#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_line_item_from_budget_item(
    b: *const fc4_budget,
    item: *const fc4_budget_item,
) -> u64 {
    let Some(i) = item.as_ref() else { return 0 };
    let i = item_from_c(i);
    with_budget(b, 0, |x| x.line_item_from_budget_item(&i).map_or(0, |l| l.token()))
}

// ---- budget simulator: inspection ------------------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn fc4_budget_get_state(b: *const fc4_budget, out: *mut fc4_budget_state) -> bool {
    let Some(o) = out.as_mut() else { return false };
    if (o.struct_size as usize) < std::mem::size_of::<fc4_budget_state>() {
        return false;
    }
    let Some(s) = with_budget(b, None, |x| Some(x.snapshot())) else {
        return false;
    };
    let mut st = fc4_budget_state {
        struct_size: std::mem::size_of::<fc4_budget_state>() as u32,
        initialized: s.initialized.into(),
        loaded_from_save: s.loaded_from_save.into(),
        agriculture_rate_fix: s.agriculture_rate_fix.into(),
        panel_flag: s.panel_flag,
        bond_count: s.bond_count,
        funds: s.funds,
        shadow_funds: s.shadow_funds,
        total_borrowed: s.total_borrowed,
        last_tax_income: s.last_tax_income,
        ytd_income: s.ytd_income,
        ytd_expense: s.ytd_expense,
        ytd_tax_income: s.ytd_tax_income,
        ytd_ordinance_income: s.ytd_ordinance_income,
        ytd_ordinance_expense: s.ytd_ordinance_expense,
        ytd_bond_payments: s.ytd_bond_payments,
        last_month_income: s.last_month_income,
        last_month_expense: s.last_month_expense,
        yearly_income: s.yearly_income,
        yearly_expense: s.yearly_expense,
        department_count: s.department_ids.len() as u32,
        group_count: s.groups.len() as u32,
        risky_money_counter: s.risky_money_counter,
        ..Default::default()
    };
    for (i, g) in s.tax.iter().enumerate() {
        st.tax_rate[i] = g.rate;
        st.max_tax_rate[i] = g.max_rate;
        st.weighted_assessed[i] = g.weighted_assessed;
        st.monthly_tax[i] = g.monthly_tax;
    }
    *o = st;
    true
}

// ---- departments ----------------------------------------------------------------------------------------------------

macro_rules! dept_fn {
    ($name:ident, $ret:ty, $fallback:expr, $method:ident $(, $arg:ident : $t:ty)*) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(d: *mut fc4_department $(, $arg: $t)*) -> $ret {
            with_dept(d, $fallback, |x| x.$method($($arg),*))
        }
    };
}

dept_fn!(fc4_department_get_id, u32, 0, id);
dept_fn!(fc4_department_set_name, bool, false, set_department_name, group: u32, instance: u32);
dept_fn!(fc4_department_get_budget_group, u32, 0, budget_group);
dept_fn!(fc4_department_set_budget_group, bool, false, set_budget_group, group: u32);
dept_fn!(fc4_department_is_fixed_funding, bool, false, is_fixed_funding);
dept_fn!(fc4_department_set_fixed_funding, bool, false, set_fixed_funding, fixed: bool);
dept_fn!(fc4_department_get_funding_percentage, f32, 0.0, funding_percentage);
dept_fn!(fc4_department_set_funding_percentage, bool, false, set_funding_percentage, q: f32, arg: u32);
dept_fn!(
    fc4_department_get_max_funding_percentage,
    f32,
    0.0,
    max_funding_percentage
);
dept_fn!(fc4_department_set_max_funding_percentage, bool, false, set_max_funding_percentage, q: f32);
dept_fn!(fc4_department_get_ideal_monthly_funding, i64, 0, ideal_monthly_funding);
dept_fn!(fc4_department_get_total_expenses, i64, 0, total_expenses);
dept_fn!(fc4_department_get_total_income, i64, 0, total_income);
dept_fn!(fc4_department_remove_line_item, bool, false, remove_line_item, id: u32);
dept_fn!(fc4_department_add_locally_funded_object, bool, false, add_locally_funded_object, holder: u64, purpose: u32);
dept_fn!(
    fc4_department_remove_locally_funded_object,
    bool,
    false,
    remove_locally_funded_object,
    holder: u64,
    purpose: u32
);
dept_fn!(
    fc4_department_set_local_funding_percent,
    bool,
    false,
    set_local_funding_percent,
    holder: u64,
    q: f32,
    purpose: u32
);
dept_fn!(
    fc4_department_set_local_full_funding,
    bool,
    false,
    set_local_full_funding,
    holder: u64,
    cost: i64,
    purpose: u32
);
dept_fn!(fc4_department_get_local_funding_percent, f32, 0.0, local_funding_percent, holder: u64, purpose: u32);

/// `GetLocalFundingPercent` with the found/not-found distinction of the
/// budget erratum: true and `*out` = the fraction if registered and the item
/// exists; false (and `*out` untouched) otherwise.
#[no_mangle]
pub unsafe extern "C" fn fc4_department_find_local_funding_percent(
    d: *mut fc4_department,
    holder: u64,
    purpose: u32,
    out: *mut f32,
) -> bool {
    if out.is_null() {
        return false;
    }
    match with_dept(d, None, |x| x.find_local_funding_percent(holder, purpose)) {
        Some(q) => {
            // SAFETY: checked non-null; the caller passes a valid float.
            unsafe { *out = q };
            true
        }
        None => false,
    }
}
dept_fn!(fc4_department_get_local_full_funding, i64, 0, local_full_funding, holder: u64, purpose: u32);
dept_fn!(fc4_department_is_locally_funded_object, bool, false, is_locally_funded_object, holder: u64, purpose: u32);
dept_fn!(
    fc4_department_recalculate,
    bool,
    false,
    recalculate_all_line_item_current_expenses
);
dept_fn!(fc4_department_get_total_spending, i64, 0, total_spending);
dept_fn!(fc4_department_set_total_spending, bool, false, set_total_spending, v: i64);
dept_fn!(
    fc4_department_has_locally_funded_items_by_purpose,
    bool,
    false,
    has_locally_funded_items_by_purpose,
    purpose: u32
);

#[no_mangle]
pub unsafe extern "C" fn fc4_department_get_name(
    d: *const fc4_department,
    sink: fc4_bytes_sink,
    ctx: *mut c_void,
) -> bool {
    let Some(name) = with_dept(d, None, |x| Some(x.name())) else {
        return false;
    };
    sink_bytes(&name, sink, ctx);
    true
}

/// `CreateLineItem`: the new line's token (borrowed), 0 on failure.
#[no_mangle]
pub unsafe extern "C" fn fc4_department_create_line_item(d: *mut fc4_department, id: u32, is_local: bool) -> u64 {
    with_dept(d, 0, |x| x.create_line_item(id, is_local).map_or(0, |l| l.token()))
}

#[no_mangle]
pub unsafe extern "C" fn fc4_department_create_line_item_for_building_type(
    d: *mut fc4_department,
    building_type: u32,
    is_local: bool,
) -> u64 {
    with_dept(d, 0, |x| {
        x.create_line_item_for_building_type(building_type, is_local)
            .map_or(0, |l| l.token())
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_department_get_line_item(d: *const fc4_department, id: u32) -> u64 {
    with_dept(d, 0, |x| x.line_item(id).map_or(0, |l| l.token()))
}

/// `GetAllLineItems`: tokens in ascending line ID; returns the count.
#[no_mangle]
pub unsafe extern "C" fn fc4_department_get_all_line_items(
    d: *const fc4_department,
    cb: fc4_token_callback,
    ctx: *mut c_void,
) -> usize {
    let tokens: Vec<u64> = with_dept(d, Vec::new(), |x| {
        x.all_line_items().iter().map(|l| l.token()).collect()
    });
    emit_tokens(&tokens, cb, ctx)
}

/// `GetLocallyFundedItemsByPurpose`: holder tokens; returns the count.
#[no_mangle]
pub unsafe extern "C" fn fc4_department_get_locally_funded_items_by_purpose(
    d: *const fc4_department,
    purpose: u32,
    cb: fc4_token_callback,
    ctx: *mut c_void,
) -> usize {
    let tokens = with_dept(d, Vec::new(), |x| x.locally_funded_items_by_purpose(purpose));
    emit_tokens(&tokens, cb, ctx)
}

// ---- line items ----------------------------------------------------------------------------------------------------

macro_rules! line_fn {
    ($name:ident, $ret:ty, $fallback:expr, $method:ident $(, $arg:ident : $t:ty)*) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(l: *mut fc4_line_item $(, $arg: $t)*) -> $ret {
            with_line(l, $fallback, |x| x.$method($($arg),*))
        }
    };
}

line_fn!(fc4_line_item_get_id, u32, 0, id);
line_fn!(fc4_line_item_get_type, u32, 0, item_type);
line_fn!(fc4_line_item_set_type, bool, false, set_type, t: u32);
line_fn!(fc4_line_item_set_name, bool, false, set_name, group: u32, instance: u32);
line_fn!(fc4_line_item_get_secondary_info, i64, 0, secondary_info);
line_fn!(fc4_line_item_set_secondary_info, bool, false, set_secondary_info, v: i64);
line_fn!(fc4_line_item_get_full_expenses, i64, 0, full_expenses);
line_fn!(fc4_line_item_get_current_expenses, i64, 0, current_expenses);
line_fn!(fc4_line_item_set_full_expenses, bool, false, set_full_expenses, v: i64);
line_fn!(fc4_line_item_add_to_full_expenses, bool, false, add_to_full_expenses, v: i64);
line_fn!(fc4_line_item_get_income, i64, 0, income);
line_fn!(fc4_line_item_set_income, bool, false, set_income, v: i64);
line_fn!(fc4_line_item_add_to_income, bool, false, add_to_income, v: i64);
line_fn!(fc4_line_item_get_display_flags, u32, 0, display_flags);
line_fn!(fc4_line_item_set_display_flag, bool, false, set_display_flag, flag: u32, on: bool);
line_fn!(fc4_line_item_is_local, bool, false, is_local);

/// `GetName`: resolves a pending name first (§8), then delivers it.
#[no_mangle]
pub unsafe extern "C" fn fc4_line_item_get_name(
    l: *const fc4_line_item,
    sink: fc4_bytes_sink,
    ctx: *mut c_void,
) -> bool {
    let Some(name) = with_line(l, None, |x| Some(x.name())) else {
        return false;
    };
    sink_bytes(&name, sink, ctx);
    true
}

/// The owning department's token, or 0.
#[no_mangle]
pub unsafe extern "C" fn fc4_line_item_get_owner(l: *const fc4_line_item) -> u64 {
    with_line(l, 0, |x| x.owner().map_or(0, |d| d.token()))
}
