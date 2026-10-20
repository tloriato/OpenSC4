// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The budget through the C ABI only, with a host that plays the GZCOM shim:
//! it creates departments and line items through `create_object`, counts
//! references on tokens, stores holder properties and implements the streams.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;

use crate::budget::*;
use crate::stream::{fc4_bytes_sink, fc4_istream, fc4_ostream, fc4_ref};

const CLSID_BUDGET: u32 = 0xE990_BE01;
const CLSID_DEPT: u32 = 0xE990_BFFC;
const CLSID_LINE: u32 = 0xAA31_3C9F;
const HOLDER_CLASS: u32 = 0x0F4C_0002;

#[derive(Clone, Copy)]
enum Obj {
    Budget(*mut fc4_budget),
    Dept(*mut fc4_department),
    Line(*mut fc4_line_item),
    Holder,
}

/// The fake game. Tokens start at 0x100 and step by 0x10.
struct Game {
    host: RefCell<Option<fc4_budget_host>>,
    next: Cell<u64>,
    objects: RefCell<HashMap<u64, (Obj, i64)>>,
    posts: RefCell<Vec<fc4_budget_message>>,
    history: RefCell<Vec<(u32, i32)>>,
    props: RefCell<HashMap<(u64, u32), Vec<u64>>>,
    /// Properties stored as scalars (one value in `props`; EX39).
    scalars: RefCell<HashSet<(u64, u32)>>,
    lots: RefCell<Vec<fc4_lot_info>>,
    ordinance_income: Cell<i64>,
    month: Cell<i32>,
}

impl Game {
    fn new() -> Box<Game> {
        let g = Box::new(Game {
            host: RefCell::new(None),
            next: Cell::new(0x100),
            objects: RefCell::new(HashMap::new()),
            posts: RefCell::new(Vec::new()),
            history: RefCell::new(Vec::new()),
            props: RefCell::new(HashMap::new()),
            scalars: RefCell::new(HashSet::new()),
            lots: RefCell::new(Vec::new()),
            ordinance_income: Cell::new(0),
            month: Cell::new(1),
        });
        let user = &*g as *const Game as *mut c_void;
        *g.host.borrow_mut() = Some(fc4_budget_host {
            struct_size: std::mem::size_of::<fc4_budget_host>() as u32,
            user,
            post: Some(post),
            show_budget_window: None,
            difficulty: Some(difficulty),
            sim_date: Some(sim_date),
            city_established: Some(established),
            city_size: None,
            ordinance_income: Some(ordinance_income),
            ordinance_expense: None,
            neighbor_income: None,
            neighbor_expense: None,
            for_each_lot: Some(for_each_lot),
            record_history: Some(record_history),
            set_history_queue_length: None,
            try_start_disaster: None,
            session_seed: None,
            ltext: Some(ltext),
            building_type_name: None,
            get_u32s: Some(get_u32s),
            get_i64s: Some(get_i64s),
            get_f32s: Some(get_f32s),
            set_u32s: Some(set_u32s),
            set_i64s: Some(set_i64s),
            set_f32s: Some(set_f32s),
            copy_property: None,
            occupant_of: None,
            class_of: Some(class_of),
            add_ref: Some(add_ref),
            release: Some(release),
            create_object: Some(create_object),
            property_is_array: Some(property_is_array),
        });
        g
    }

    fn host(&self) -> fc4_budget_host {
        self.host.borrow().expect("host")
    }

    fn token(&self) -> u64 {
        let t = self.next.get();
        self.next.set(t + 0x10);
        t
    }

    /// A GZCOM object with one reference (the creator's).
    fn create(&self, class: u32) -> (u64, *mut c_void) {
        let t = self.token();
        let h = self.host();
        let (obj, handle) = unsafe {
            match class {
                CLSID_BUDGET => {
                    let p = fc4_budget_create(&h, t);
                    (Obj::Budget(p), p as *mut c_void)
                }
                CLSID_DEPT => {
                    let p = fc4_department_create(&h, t);
                    (Obj::Dept(p), p as *mut c_void)
                }
                CLSID_LINE => {
                    let p = fc4_line_item_create(&h, t);
                    (Obj::Line(p), p as *mut c_void)
                }
                _ => (Obj::Holder, std::ptr::null_mut()),
            }
        };
        self.objects.borrow_mut().insert(t, (obj, 1));
        (t, handle)
    }

    fn refs(&self, t: u64) -> i64 {
        self.objects.borrow().get(&t).map_or(0, |o| o.1)
    }

    fn dept(&self, t: u64) -> *mut fc4_department {
        match self.objects.borrow()[&t].0 {
            Obj::Dept(p) => p,
            _ => panic!("not a department"),
        }
    }

    fn line(&self, t: u64) -> *mut fc4_line_item {
        match self.objects.borrow()[&t].0 {
            Obj::Line(p) => p,
            _ => panic!("not a line item"),
        }
    }

    fn handle_of(&self, t: u64) -> (u32, *mut c_void) {
        match self.objects.borrow()[&t].0 {
            Obj::Budget(p) => (CLSID_BUDGET, p as *mut c_void),
            Obj::Dept(p) => (CLSID_DEPT, p as *mut c_void),
            Obj::Line(p) => (CLSID_LINE, p as *mut c_void),
            Obj::Holder => (HOLDER_CLASS, std::ptr::null_mut()),
        }
    }
}

fn game(u: *mut c_void) -> &'static Game {
    unsafe { &*(u as *const Game) }
}

unsafe extern "C" fn post(u: *mut c_void, m: *const fc4_budget_message) {
    game(u).posts.borrow_mut().push(*m);
}
unsafe extern "C" fn difficulty(_: *mut c_void) -> u32 {
    2
}
unsafe extern "C" fn sim_date(u: *mut c_void, y: *mut i32, m: *mut i32) {
    *y = 2000;
    *m = game(u).month.get();
}
unsafe extern "C" fn established(_: *mut c_void) -> bool {
    true
}
unsafe extern "C" fn ordinance_income(u: *mut c_void) -> i64 {
    game(u).ordinance_income.get()
}
unsafe extern "C" fn for_each_lot(u: *mut c_void, visit: fc4_lot_visitor, ctx: *mut c_void) {
    let lots = game(u).lots.borrow().clone();
    if let Some(v) = visit {
        for l in &lots {
            v(ctx, l);
        }
    }
}
unsafe extern "C" fn record_history(u: *mut c_void, series: u32, value: i32) {
    game(u).history.borrow_mut().push((series, value));
}
unsafe extern "C" fn ltext(_: *mut c_void, g: u32, i: u32, sink: fc4_bytes_sink, ctx: *mut c_void) -> bool {
    let s = format!("{g:x}/{i:x}");
    if let Some(f) = sink {
        f(ctx, s.as_ptr(), s.len());
    }
    true
}
unsafe fn get_vals<T>(u: *mut c_void, h: u64, p: u32, out: *mut T, cap: usize, conv: impl Fn(u64) -> T) -> i32 {
    match game(u).props.borrow().get(&(h, p)) {
        None => -1,
        Some(v) => {
            for (i, &x) in v.iter().take(cap).enumerate() {
                *out.add(i) = conv(x);
            }
            v.len() as i32
        }
    }
}
unsafe extern "C" fn get_u32s(u: *mut c_void, h: u64, p: u32, out: *mut u32, cap: usize) -> i32 {
    get_vals(u, h, p, out, cap, |x| x as u32)
}
unsafe extern "C" fn get_i64s(u: *mut c_void, h: u64, p: u32, out: *mut i64, cap: usize) -> i32 {
    get_vals(u, h, p, out, cap, |x| x as i64)
}
unsafe extern "C" fn get_f32s(u: *mut c_void, h: u64, p: u32, out: *mut f32, cap: usize) -> i32 {
    get_vals(u, h, p, out, cap, |x| f32::from_bits(x as u32))
}
unsafe extern "C" fn set_u32s(u: *mut c_void, h: u64, p: u32, v: *const u32, n: usize) -> bool {
    let vals = std::slice::from_raw_parts(v, n).iter().map(|&x| u64::from(x)).collect();
    game(u).props.borrow_mut().insert((h, p), vals);
    true
}
unsafe extern "C" fn set_i64s(u: *mut c_void, h: u64, p: u32, v: *const i64, n: usize) -> bool {
    let vals = std::slice::from_raw_parts(v, n).iter().map(|&x| x as u64).collect();
    game(u).props.borrow_mut().insert((h, p), vals);
    true
}
unsafe extern "C" fn set_f32s(u: *mut c_void, h: u64, p: u32, v: *const f32, n: usize) -> bool {
    let vals = std::slice::from_raw_parts(v, n)
        .iter()
        .map(|&x| u64::from(x.to_bits()))
        .collect();
    game(u).props.borrow_mut().insert((h, p), vals);
    true
}
unsafe extern "C" fn property_is_array(u: *mut c_void, h: u64, p: u32) -> i32 {
    let g = game(u);
    if !g.props.borrow().contains_key(&(h, p)) {
        -1
    } else if g.scalars.borrow().contains(&(h, p)) {
        0
    } else {
        1
    }
}
unsafe extern "C" fn class_of(u: *mut c_void, t: u64) -> u32 {
    game(u).handle_of(t).0
}
unsafe extern "C" fn add_ref(u: *mut c_void, t: u64) {
    if let Some(o) = game(u).objects.borrow_mut().get_mut(&t) {
        o.1 += 1;
    }
}
unsafe extern "C" fn release(u: *mut c_void, t: u64) {
    let g = game(u);
    let dead = {
        let mut objs = g.objects.borrow_mut();
        let Some(o) = objs.get_mut(&t) else { return };
        o.1 -= 1;
        assert!(o.1 >= 0, "over-release of {t:#x}");
        (o.1 == 0).then_some(o.0)
    };
    // The GZCOM object dies: it destroys its core handle (outside any borrow).
    match dead {
        Some(Obj::Dept(p)) => fc4_department_destroy(p),
        Some(Obj::Line(p)) => fc4_line_item_destroy(p),
        Some(Obj::Budget(p)) => fc4_budget_destroy(p),
        _ => {}
    }
}
unsafe extern "C" fn create_object(u: *mut c_void, class: u32) -> *mut c_void {
    game(u).create(class).1
}

// ---- streams: bytes plus references by save ID ---------------------------------------------------------

#[derive(Default)]
struct Out {
    bytes: Vec<u8>,
    ids: HashMap<u64, u32>,
    order: Vec<u64>,
}

fn out_of(u: *mut c_void) -> &'static mut Out {
    unsafe { &mut *(u as *mut Out) }
}
unsafe extern "C" fn put_u8(u: *mut c_void, v: u8) -> bool {
    out_of(u).bytes.push(v);
    true
}
unsafe extern "C" fn put_u16(u: *mut c_void, v: u16) -> bool {
    out_of(u).bytes.extend_from_slice(&v.to_le_bytes());
    true
}
unsafe extern "C" fn put_u32(u: *mut c_void, v: u32) -> bool {
    out_of(u).bytes.extend_from_slice(&v.to_le_bytes());
    true
}
unsafe extern "C" fn put_u64(u: *mut c_void, v: u64) -> bool {
    out_of(u).bytes.extend_from_slice(&v.to_le_bytes());
    true
}
unsafe extern "C" fn put_f32(u: *mut c_void, v: f32) -> bool {
    out_of(u).bytes.extend_from_slice(&v.to_le_bytes());
    true
}
unsafe extern "C" fn put_f64(u: *mut c_void, v: f64) -> bool {
    out_of(u).bytes.extend_from_slice(&v.to_le_bytes());
    true
}
unsafe extern "C" fn put_bytes(u: *mut c_void, d: *const u8, n: usize) -> bool {
    out_of(u).bytes.extend_from_slice(std::slice::from_raw_parts(d, n));
    true
}
unsafe extern "C" fn put_str(u: *mut c_void, d: *const u8, n: usize) -> bool {
    let o = out_of(u);
    o.bytes.extend_from_slice(&(n as u32).to_le_bytes());
    o.bytes.extend_from_slice(std::slice::from_raw_parts(d, n));
    true
}
unsafe extern "C" fn put_ref(u: *mut c_void, r: *const fc4_ref) -> bool {
    let o = out_of(u);
    let r = &*r;
    let id = if r.token == 0 {
        0
    } else {
        let n = o.ids.len() as u32 + 1;
        *o.ids.entry(r.token).or_insert_with(|| {
            o.order.push(r.token);
            n
        })
    };
    o.bytes.extend_from_slice(&id.to_le_bytes());
    o.bytes.extend_from_slice(&r.class_id.to_le_bytes());
    true
}
unsafe extern "C" fn no_error(_: *mut c_void) -> i32 {
    0
}

fn ostream(o: &mut Out) -> fc4_ostream {
    fc4_ostream {
        struct_size: std::mem::size_of::<fc4_ostream>() as u32,
        user: o as *mut Out as *mut c_void,
        put_u8: Some(put_u8),
        put_u16: Some(put_u16),
        put_u32: Some(put_u32),
        put_u64: Some(put_u64),
        put_f32: Some(put_f32),
        put_f64: Some(put_f64),
        put_bytes: Some(put_bytes),
        put_str: Some(put_str),
        put_ref: Some(put_ref),
        error: Some(no_error),
    }
}

struct In<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// Save ID → token of the object that stands for it after loading.
    objects: &'a HashMap<u32, u64>,
    game: &'a Game,
}

fn in_of(u: *mut c_void) -> &'static mut In<'static> {
    unsafe { &mut *(u as *mut In<'static>) }
}
fn take<const N: usize>(u: *mut c_void) -> Option<[u8; N]> {
    let i = in_of(u);
    let b: [u8; N] = i.bytes.get(i.pos..i.pos + N)?.try_into().ok()?;
    i.pos += N;
    Some(b)
}
unsafe extern "C" fn get_u8(u: *mut c_void, v: *mut u8) -> bool {
    take::<1>(u).map(|b| *v = b[0]).is_some()
}
unsafe extern "C" fn get_u16(u: *mut c_void, v: *mut u16) -> bool {
    take::<2>(u).map(|b| *v = u16::from_le_bytes(b)).is_some()
}
unsafe extern "C" fn get_u32(u: *mut c_void, v: *mut u32) -> bool {
    take::<4>(u).map(|b| *v = u32::from_le_bytes(b)).is_some()
}
unsafe extern "C" fn get_u64(u: *mut c_void, v: *mut u64) -> bool {
    take::<8>(u).map(|b| *v = u64::from_le_bytes(b)).is_some()
}
unsafe extern "C" fn get_f32(u: *mut c_void, v: *mut f32) -> bool {
    take::<4>(u).map(|b| *v = f32::from_le_bytes(b)).is_some()
}
unsafe extern "C" fn get_f64(u: *mut c_void, v: *mut f64) -> bool {
    take::<8>(u).map(|b| *v = f64::from_le_bytes(b)).is_some()
}
unsafe extern "C" fn get_bytes(u: *mut c_void, out: *mut u8, n: usize) -> bool {
    let i = in_of(u);
    let Some(src) = i.bytes.get(i.pos..i.pos + n) else {
        return false;
    };
    std::ptr::copy_nonoverlapping(src.as_ptr(), out, n);
    i.pos += n;
    true
}
unsafe extern "C" fn get_str(u: *mut c_void, sink: fc4_bytes_sink, ctx: *mut c_void) -> bool {
    let Some(n) = take::<4>(u).map(u32::from_le_bytes) else {
        return false;
    };
    let i = in_of(u);
    let Some(src) = i.bytes.get(i.pos..i.pos + n as usize) else {
        return false;
    };
    i.pos += n as usize;
    if let Some(f) = sink {
        f(ctx, src.as_ptr(), src.len());
    }
    true
}
unsafe extern "C" fn get_ref(u: *mut c_void, _iid: u32, out: *mut fc4_ref) -> bool {
    let (Some(id), Some(_class)) = (
        take::<4>(u).map(u32::from_le_bytes),
        take::<4>(u).map(u32::from_le_bytes),
    ) else {
        return false;
    };
    *out = fc4_ref::default();
    if id == 0 {
        return true;
    }
    let i = in_of(u);
    let token = i.objects[&id];
    let (class, handle) = i.game.handle_of(token);
    (*out).token = token;
    (*out).class_id = class;
    if handle.is_null() {
        // A host object: the reader owns one reference.
        add_ref(i.game as *const Game as *mut c_void, token);
    } else {
        (*out).core_handle = handle;
    }
    true
}
unsafe extern "C" fn skip(u: *mut c_void, n: usize) -> bool {
    let i = in_of(u);
    if i.pos + n > i.bytes.len() {
        return false;
    }
    i.pos += n;
    true
}

fn istream(i: &mut In) -> fc4_istream {
    fc4_istream {
        struct_size: std::mem::size_of::<fc4_istream>() as u32,
        user: i as *mut In as *mut c_void,
        get_u8: Some(get_u8),
        get_u16: Some(get_u16),
        get_u32: Some(get_u32),
        get_u64: Some(get_u64),
        get_f32: Some(get_f32),
        get_f64: Some(get_f64),
        get_bytes: Some(get_bytes),
        get_str: Some(get_str),
        get_ref: Some(get_ref),
        skip: Some(skip),
        error: Some(no_error),
    }
}

/// Writes the object behind `token` with the shared ID map.
unsafe fn write_obj(g: &Game, token: u64, out: &mut Out) -> Vec<u8> {
    out.bytes.clear();
    let s = ostream(out);
    let obj = g.objects.borrow()[&token].0;
    let ok = match obj {
        Obj::Budget(p) => fc4_budget_write(p, &s),
        Obj::Dept(p) => fc4_department_write(p, &s),
        Obj::Line(p) => fc4_line_item_write(p, &s),
        Obj::Holder => false,
    };
    assert!(ok);
    std::mem::take(&mut out.bytes)
}

// ---- tests -------------------------------------------------------------------------------------------------------

unsafe extern "C" fn push_token(ctx: *mut c_void, t: u64) {
    (*(ctx as *mut Vec<u64>)).push(t);
}

#[test]
fn budget_through_the_abi() {
    let g = Game::new();
    unsafe {
        let (bt, bh) = g.create(CLSID_BUDGET);
        let b = bh as *mut fc4_budget;
        assert!(fc4_budget_init(b, 0, false));
        assert!(fc4_budget_is_initialized(b));
        assert_eq!(fc4_budget_get_total_funds(b), 100_000);
        let p = g.posts.borrow()[0];
        assert_eq!((p.type_, p.void1, p.unknown), (0x0772_FAD4, bt, bt));

        // Department and lines, created through create_object; the budget owns them.
        let dt = fc4_budget_create_department(b, 4096, 8192);
        assert_ne!(dt, 0);
        assert_eq!(fc4_budget_get_department(b, 4096), dt);
        assert_eq!(g.refs(dt), 1, "only the budget's reference");
        let d = g.dept(dt);
        assert_eq!(fc4_department_get_id(d), 4096);
        let lt = fc4_department_create_line_item(d, 1, false);
        let l = g.line(lt);
        assert_eq!(fc4_line_item_get_owner(l), dt);
        assert!(fc4_line_item_set_full_expenses(l, 1000));
        assert!(fc4_department_set_funding_percentage(d, 0.5, 3));
        assert_eq!(fc4_line_item_get_current_expenses(l), 500);
        let fpc = g
            .posts
            .borrow()
            .iter()
            .rev()
            .find(|m| m.type_ == 0x08DA_C3E7)
            .copied()
            .unwrap();
        assert_eq!((fpc.data2, fpc.data3, fpc.void1), (4096, 3, dt));

        let mut tokens = Vec::new();
        let ctx = &mut tokens as *mut Vec<u64> as *mut c_void;
        assert_eq!(fc4_department_get_all_line_items(d, Some(push_token), ctx), 1);
        assert_eq!(fc4_budget_get_departments_in_group(b, 8192, Some(push_token), ctx), 1);
        assert_eq!(tokens, vec![lt, dt]);

        // A locally funded holder with a budget item (department 4096, line 2).
        let lt2 = fc4_department_create_line_item(d, 2, true);
        let (ht, _) = g.create(HOLDER_CLASS);
        set_u32s(g.host().user, ht, 0xEA54_D283, [4096u32].as_ptr(), 1);
        set_u32s(g.host().user, ht, 0xEA54_D284, [2u32].as_ptr(), 1);
        set_u32s(g.host().user, ht, 0xEA54_D285, [7u32].as_ptr(), 1);
        set_i64s(g.host().user, ht, 0xEA54_D286, [200i64].as_ptr(), 1);
        let mut item = fc4_budget_item::default();
        assert!(fc4_budget_get_budget_item_for_purpose(b, ht, 7, &mut item));
        assert_eq!(
            (item.department_id, item.line_item, item.cost, item.local_funding),
            (4096, 2, 200, 1.0)
        );
        assert!(fc4_department_add_locally_funded_object(d, ht, 7));
        assert_eq!(g.refs(ht), 2);
        assert_eq!(
            fc4_line_item_get_current_expenses(g.line(lt2)),
            100,
            "starts at the department's 50 %"
        );
        // GetLocalFundingPercent's found / not-found cases (budget erratum).
        let mut q = -1.0f32;
        assert!(fc4_department_find_local_funding_percent(d, ht, 7, &mut q));
        assert_eq!(q, fc4_department_get_local_funding_percent(d, ht, 7));
        assert_eq!(q, 0.5, "AddLocallyFundedObject wrote the department's 50 %");
        let mut untouched = -1.0f32;
        assert!(!fc4_department_find_local_funding_percent(d, ht, 8, &mut untouched));
        assert_eq!(untouched, -1.0);
        assert!(!fc4_department_find_local_funding_percent(d, ht, 7, std::ptr::null_mut()));
        // RG13 (budget.md §6): ChangeBudgetItemLocalFunding stores float32(q ×
        // 100.0f) and GetBudgetItemForPurpose reads back float32(pct × 0.01f),
        // so q = 0.43 comes back as 0.42999998 (0x3EDC28F5), not 0.43.
        assert!(fc4_budget_change_budget_item_local_funding(b, ht, 0, 0.43));
        let mut stored = [0.0f32; 1];
        assert_eq!(get_f32s(g.host().user, ht, 0xEA54_D287, stored.as_mut_ptr(), 1), 1);
        assert_eq!(stored[0], 43.0);
        let mut item = fc4_budget_item::default();
        assert!(fc4_budget_get_budget_item_for_purpose(b, ht, 7, &mut item));
        assert_eq!(item.local_funding.to_bits(), 0x3EDC_28F5);
        assert_eq!(item.local_funding, 0.42999998);
        assert!(fc4_department_find_local_funding_percent(d, ht, 7, &mut q));
        assert_eq!(q, 0.42999998);
        assert!(fc4_budget_change_budget_item_local_funding(b, ht, 0, 0.5));

        // A month: tax from one lot plus ordinance income; a bond.
        g.lots.borrow_mut().push(fc4_lot_info {
            has_building: 1,
            livable: 1,
            purpose: 1,
            building_wealth: 1,
            lot_wealth: 1,
            building_value: 10_000,
            size_x: 1,
            size_z: 1,
            ..Default::default()
        });
        g.ordinance_income.set(25);
        assert!(fc4_budget_issue_bond(b, 10_000));
        g.month.set(2);
        assert!(fc4_budget_handle_message(b, 0x6695_6816, 0));
        let mut st = fc4_budget_state {
            struct_size: std::mem::size_of::<fc4_budget_state>() as u32,
            ..Default::default()
        };
        assert!(fc4_budget_get_state(b, &mut st));
        // Code-default tuning: difficulty multiplier 100000 [compat] -> 9 % x 10,000 x 1e5 / 1200.
        assert_eq!(st.monthly_tax[0], st.last_tax_income);
        assert_eq!(
            st.last_month_expense,
            500 + 100 + fc4_budget_get_monthly_payment_for_loan(b, 10_000)
        );
        assert_eq!(st.last_month_income, st.last_tax_income + 25);
        assert_eq!(st.funds, 110_000 + st.last_month_income - st.last_month_expense);
        assert_eq!((st.bond_count, st.department_count, st.group_count), (1, 1, 1));
        let mut bond = fc4_bond::default();
        assert!(fc4_budget_get_bond(b, 0, &mut bond));
        assert_eq!((bond.due_month, bond.due_year, bond.amount), (1, 2010, 10_000));
        assert!(!fc4_budget_get_bond(b, 1, &mut bond));

        // Save the three records, load them into fresh objects, save again: identical.
        let mut out = Out::default();
        let budget_rec = write_obj(&g, bt, &mut out);
        let order = out.order.clone(); // dept first (ID 1)
        let mut recs = vec![(bt, budget_rec)];
        let mut k = 0;
        while k < out.order.len() {
            let t = out.order[k];
            if !matches!(g.objects.borrow()[&t].0, Obj::Holder) {
                let r = write_obj(&g, t, &mut out);
                recs.push((t, r));
            }
            k += 1;
        }
        let saved_ids = out.ids.clone();
        assert_eq!(order[0], dt);

        // Load: fresh objects for every saved ID, then each reads its record.
        let mut new_of: HashMap<u64, u64> = HashMap::new();
        let mut objects: HashMap<u32, u64> = HashMap::new();
        for (&t, &id) in &saved_ids {
            let (class, _) = g.handle_of(t);
            let nt = if class == HOLDER_CLASS { t } else { g.create(class).0 };
            new_of.insert(t, nt);
            objects.insert(id, nt);
        }
        let (nbt, _) = g.create(CLSID_BUDGET);
        new_of.insert(bt, nbt);
        // Records in DESCENDING class ID: departments before the line items they refer to.
        // A department must not depend on its lines having been read (their IDs are taken on
        // first use); the conformance vectors cover the ascending order.
        let mut load_order: Vec<&(u64, Vec<u8>)> = recs.iter().collect();
        load_order.sort_by_key(|(t, _)| std::cmp::Reverse(g.handle_of(*t).0));
        for (t, rec) in load_order {
            let nt = new_of[t];
            let mut i = In {
                bytes: rec,
                pos: 0,
                objects: &objects,
                game: &g,
            };
            let s = istream(&mut i);
            let obj = g.objects.borrow()[&nt].0;
            let ok = match obj {
                Obj::Budget(p) => fc4_budget_read(p, &s),
                Obj::Dept(p) => fc4_department_read(p, &s),
                Obj::Line(p) => fc4_line_item_read(p, &s),
                Obj::Holder => false,
            };
            assert!(ok, "record of {t:#x} rejected");
            assert_eq!(i.pos, rec.len(), "record of {t:#x} not fully read");
        }
        // The loaded objects hold their references; the store's creation
        // reference goes away (the plugin's IStreamAdapter releases it).
        for (t, nt) in &new_of {
            if *t != ht && *t != bt {
                release(g.host().user, *nt);
            }
        }
        assert_eq!(g.refs(ht), 3, "original department + loaded department");
        let nb = match g.objects.borrow()[&nbt].0 {
            Obj::Budget(p) => p,
            _ => unreachable!(),
        };
        let mut out2 = Out::default();
        let again = write_obj(&g, nbt, &mut out2);
        assert_eq!(again, recs[0].1, "budget record");
        for (t, rec) in recs.iter().skip(1) {
            assert_eq!(&write_obj(&g, new_of[t], &mut out2), rec, "record of {t:#x}");
        }
        let nd = fc4_budget_get_department(nb, 4096);
        assert_eq!(nd, new_of[&dt]);
        assert_eq!(fc4_department_get_funding_percentage(g.dept(nd)), 0.5);

        // Removing the original department releases its lines and holder.
        assert!(fc4_budget_remove_department(b, 4096));
        assert_eq!(g.refs(dt), 0);
        assert_eq!(g.refs(lt), 0);
        assert!(fc4_budget_init(nb, 0, false), "a loaded budget is Init-ed before use");
        assert_eq!(
            fc4_budget_get_total_funds(nb),
            st.funds,
            "no new-city setup after a load"
        );
        assert!(fc4_budget_shutdown(nb));
        assert!(fc4_budget_shutdown(nb));
        assert_eq!(g.refs(new_of[&dt]), 0);
        assert_eq!(g.refs(ht), 1);
        release(g.host().user, bt);
        release(g.host().user, nbt);
    }
}

#[test]
fn null_handles_are_tolerated() {
    unsafe {
        assert!(fc4_budget_create(std::ptr::null(), 1).is_null());
        assert_eq!(fc4_budget_get_total_funds(std::ptr::null_mut()), 0);
        assert!(!fc4_budget_init(std::ptr::null_mut(), 0, false));
        assert_eq!(fc4_department_get_id(std::ptr::null_mut()), 0);
        assert!(!fc4_line_item_is_local(std::ptr::null_mut()));
        fc4_budget_destroy(std::ptr::null_mut());
        let bad = fc4_budget_host {
            struct_size: 8,
            ..std::mem::zeroed()
        };
        assert!(fc4_department_create(&bad, 1).is_null());
    }
}

unsafe extern "C" fn collect_tax_lot(ctx: *mut c_void, index: u32, lot: *const fc4_lot_info, r: *const fc4_tax_lot) {
    let v = &mut *(ctx as *mut Vec<(u32, fc4_lot_info, fc4_tax_lot)>);
    v.push((index, *lot, *r));
}

/// The budget shadow's scan (`fc4_budget_shadow_tax`) gives exactly what the
/// budget's own `ComputeTax` stored, and writes nothing through the host.
#[test]
fn shadow_tax_matches_compute_tax_and_writes_nothing() {
    let g = Game::new();
    unsafe {
        // The shipped tuning values that matter for tax (§9), on holder 0x999.
        let f = |v: &[f32]| v.iter().map(|x| u64::from(x.to_bits())).collect::<Vec<u64>>();
        g.props.borrow_mut().insert((0x999, 0xEBFF_69E7), f(&[1.2, 1.1, 1.0]));
        g.props.borrow_mut().insert((0x999, 0x8A48_0DF7), f(&[1.0, 0.75, 0.5]));
        g.props.borrow_mut().insert((0x999, 0x4AC5_19B8), f(&[0.0, 0.0, 255.0, 0.0]));
        let (_, bh) = g.create(CLSID_BUDGET);
        let b = bh as *mut fc4_budget;
        assert!(fc4_budget_init(b, 0x999, false));
        assert!(fc4_budget_set_tax_rate(b, 3, 7.5));
        let lot = |purpose: u32, bw: u32, lw: u32, value: i64, livable: u8| fc4_lot_info {
            has_building: 1,
            livable,
            purpose,
            building_wealth: bw,
            lot_wealth: lw,
            building_value: value,
            size_x: 2,
            size_z: 1,
            ..Default::default()
        };
        g.lots.borrow_mut().extend([
            lot(1, 1, 1, 10_000, 1),
            lot(2, 1, 1, 3_333, 1), // CS$ at 7.5 %
            lot(2, 2, 1, 4_001, 1), // CS$, waning 0.75
            lot(4, 1, 1, 777, 0),   // tourism CS$, not livable
            lot(3, 1, 1, 5_000, 1), // office $: untaxed
            lot(6, 1, 1, 998_857, 1),
            fc4_lot_info::default(), // no building
        ]);
        g.month.set(2);
        assert!(fc4_budget_handle_message(b, 0x6695_6816, 0));
        let mut st = fc4_budget_state {
            struct_size: std::mem::size_of::<fc4_budget_state>() as u32,
            ..Default::default()
        };
        assert!(fc4_budget_get_state(b, &mut st));

        let writes_before = (g.posts.borrow().len(), g.history.borrow().len(), g.props.borrow().clone());
        let mut rates = [0f32; 12];
        for (t, r) in rates.iter_mut().enumerate() {
            *r = fc4_budget_get_tax_rate(b, t as u32);
        }
        let mut seen: Vec<(u32, fc4_lot_info, fc4_tax_lot)> = Vec::new();
        let mut scan = fc4_tax_scan {
            struct_size: std::mem::size_of::<fc4_tax_scan>() as u32,
            ..Default::default()
        };
        let h = g.host();
        assert!(fc4_budget_shadow_tax(
            &h,
            0x999,
            false,
            2,
            rates.as_ptr(),
            Some(collect_tax_lot),
            &mut seen as *mut _ as *mut c_void,
            &mut scan
        ));
        assert_eq!(scan.weighted_assessed, st.weighted_assessed);
        assert_eq!(scan.monthly_tax, st.monthly_tax);
        assert_eq!(scan.total_tax, st.last_tax_income);
        for t in 0..12 {
            assert_eq!(scan.tax_income[t], fc4_budget_get_tax_income_for(b, t as i32), "type {t}");
        }
        assert_eq!(scan.multiplier, 1.0);
        assert_eq!(scan.lots, 7);
        assert_eq!(scan.counted[3], 2);
        assert_eq!(scan.weighted_assessed[3], 3_333 + 3_000);
        // Every lot, in order, with its status; the not-livable one says what it would have added.
        let status: Vec<u32> = seen.iter().map(|s| s.2.status).collect();
        assert_eq!(status, [0, 0, 0, 2, 3, 0, 1]);
        assert_eq!(seen.iter().map(|s| s.0).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!((seen[3].2.group, seen[3].2.assessed), (3, 777));
        assert_eq!(seen[3].1.building_value, 777);
        // Nothing posted, recorded or written.
        let writes_after = (g.posts.borrow().len(), g.history.borrow().len(), g.props.borrow().clone());
        assert_eq!(writes_before, writes_after);

        // Bad arguments.
        let mut small = fc4_tax_scan {
            struct_size: 8,
            ..Default::default()
        };
        let none = std::ptr::null_mut();
        assert!(!fc4_budget_shadow_tax(&h, 0, false, 2, rates.as_ptr(), None, none, &mut small));
        assert!(!fc4_budget_shadow_tax(&h, 0, false, 2, std::ptr::null(), None, none, &mut scan));
        assert!(!fc4_budget_shadow_tax(std::ptr::null(), 0, false, 2, rates.as_ptr(), None, none, &mut scan));
    }
}

unsafe extern "C" fn push_item(ctx: *mut c_void, item: *const fc4_budget_item) {
    (*(ctx as *mut Vec<fc4_budget_item>)).push(*item);
}

/// data-formats §5.2 (EX39): the budget item arrays are read with the original's vector helpers, so a cost stored
/// as a scalar (two shipped landfill tiles hold `0xEA54D286` as a Sint64 scalar) gives no cost entry: the item's
/// cost is 0 and ChangeBudgetItemCost finds no entry to change. The host's `get_i64s` still converts the scalar to one value
/// (the converting getter); `property_is_array` tells them apart. Without that callback the old one-value reading
/// stays.
#[test]
fn ex39_a_scalar_item_cost_reads_as_empty() {
    let g = Game::new();
    unsafe {
        let (_, bh) = g.create(CLSID_BUDGET);
        let b = bh as *mut fc4_budget;
        assert!(fc4_budget_init(b, 0, false));
        let (ht, _) = g.create(HOLDER_CLASS);
        let u = g.host().user;
        set_u32s(u, ht, 0xEA54_D283, [4096u32].as_ptr(), 1);
        set_i64s(u, ht, 0xEA54_D286, [10i64].as_ptr(), 1);
        g.scalars.borrow_mut().insert((ht, 0xEA54_D286));
        let mut v = [0i64; 1];
        // The converting read keeps 10.
        assert_eq!(get_i64s(u, ht, 0xEA54_D286, v.as_mut_ptr(), 1), 1);
        assert_eq!(v[0], 10);
        let mut items: Vec<fc4_budget_item> = Vec::new();
        let ctx = &mut items as *mut Vec<fc4_budget_item> as *mut c_void;
        assert!(fc4_budget_get_budget_item_info(b, ht, Some(push_item), ctx));
        assert_eq!(items.len(), 1);
        // vec<s64> of a scalar is empty: no cost entry.
        assert_eq!((items[0].department_id, items[0].cost), (4096, 0));
        // ChangeBudgetItemCost finds no entry 0 in the empty array: nothing is written (it still reports true).
        fc4_budget_change_budget_item_cost(b, ht, 0, 5);
        assert_eq!(get_i64s(u, ht, 0xEA54_D286, v.as_mut_ptr(), 1), 1);
        assert_eq!(v[0], 10, "the scalar is left alone");
        // The same value stored as a one-element array is an entry.
        g.scalars.borrow_mut().clear();
        items.clear();
        let ctx = &mut items as *mut Vec<fc4_budget_item> as *mut c_void;
        assert!(fc4_budget_get_budget_item_info(b, ht, Some(push_item), ctx));
        assert_eq!(items[0].cost, 10);
        // A department scalar is an empty department array: no items [literal reading].
        g.scalars.borrow_mut().insert((ht, 0xEA54_D283));
        assert!(!fc4_budget_get_budget_item_info(b, ht, None, std::ptr::null_mut()));
    }
}

thread_local! {
    static SEED: Cell<u32> = const { Cell::new(0) };
}

unsafe extern "C" fn test_session_seed(_: *mut c_void) -> u32 {
    SEED.with(|s| s.get())
}

/// RN3: the RiskyMoney generator is process-wide. The first budget seeds it
/// with derive(S, 0x6C09C3D7); a budget created later (the next city load,
/// even with another session seed) draws from the same stream.
#[test]
fn risky_money_generator_is_process_wide() {
    use fc4_core::rng::RzRandomSuper;
    use fc4_core::seeding::{clocks, keys, seed};
    let g = Game::new();
    let mut h = g.host();
    h.session_seed = Some(test_session_seed);
    unsafe {
        SEED.with(|s| s.set(0x0BAD_5EED));
        let a = fc4_budget_create(&h, 0x10);
        let ga = (*(a as *const std::rc::Rc<fc4_core::budget::BudgetSim>))
            .generator()
            .clone();
        let first = ga.borrow_mut().next_u32();
        fc4_budget_destroy(a);
        SEED.with(|s| s.set(0x1234_0000));
        let b = fc4_budget_create(&h, 0x20);
        let gb = (*(b as *const std::rc::Rc<fc4_core::budget::BudgetSim>))
            .generator()
            .clone();
        assert!(std::rc::Rc::ptr_eq(&ga, &gb));
        let mut fresh = RzRandomSuper::new(seed(0x0BAD_5EED, keys::BUDGET), &clocks(0x0BAD_5EED, keys::BUDGET));
        assert_eq!(fresh.next_u32(), first);
        assert_eq!(gb.borrow_mut().next_u32(), fresh.next_u32());
        fc4_budget_destroy(b);
    }
}
