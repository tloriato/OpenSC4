// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The port developers through the C ABI (docs/specs/port-developers.md):
//! a fake game behind `fc4_port_host`.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{c_char, c_void, CStr};

use fc4_core::port_developer::tuning::shipped_exemplars;
use fc4_core::property::PropertyHolder;

use crate::hostgrid::fc4_cell_rect;
use crate::port_developer::*;
use crate::props::fc4_props;
use crate::sinks::fc4_u32s_sink;
use crate::test_streams::{istream, ostream, In, Out};

#[derive(Default)]
struct Game {
    exemplars: BTreeMap<u32, PropertyHolder>,
    /// lot → (config, growth stage, x, z)
    lots: RefCell<BTreeMap<u64, (u32, u32, i16, i16)>>,
    line: Cell<(i64, i64)>,
    offices: Cell<(u32, u32)>,
    water: BTreeSet<(u32, u32)>,
    log: RefCell<Vec<String>>,
}

impl Game {
    fn new() -> Box<Game> {
        let mut g = Game::default();
        for airport in [true, false] {
            g.exemplars.extend(shipped_exemplars(airport));
        }
        for c in [(0, 0), (1, 0), (1, 1), (3, 2)] {
            g.water.insert(c);
        }
        Box::new(g)
    }
    fn user(&self) -> *mut c_void {
        self as *const Game as *mut c_void
    }
    fn log(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
}

fn game(u: *mut c_void) -> &'static Game {
    unsafe { &*(u as *const Game) }
}

unsafe extern "C" fn exemplar(u: *mut c_void, instance: u32) -> *mut fc4_props {
    match game(u).exemplars.get(&instance) {
        Some(h) => Box::into_raw(Box::new(h.clone())) as *mut fc4_props,
        None => std::ptr::null_mut(),
    }
}
unsafe extern "C" fn add_agent(u: *mut c_void, level: u32, name: *const c_char, flags: u32) {
    let n = CStr::from_ptr(name).to_string_lossy();
    game(u).log(format!("add_agent {n} {level} {flags}"));
}
unsafe extern "C" fn create_department(
    u: *mut c_void,
    id: u32,
    _g: u32,
    _ng: u32,
    _ni: u32,
    _m: f32,
    line: u32,
) -> u64 {
    game(u).log(format!("create_department {id:#x} {line}"));
    0xD
}
unsafe extern "C" fn line_expenses(u: *mut c_void, _d: u64, _l: u32, full: *mut i64, cur: *mut i64) -> bool {
    let (f, c) = game(u).line.get();
    *full = f;
    *cur = c;
    true
}
unsafe extern "C" fn set_full_expenses(u: *mut c_void, _d: u64, _l: u32, v: i64) {
    game(u).line.set((v, v));
    game(u).log(format!("full {v}"));
}
unsafe extern "C" fn occupant_type(_u: *mut c_void, _o: u64) -> u32 {
    0x2781_28A0
}
unsafe extern "C" fn occupant_in_group(_u: *mut c_void, _o: u64, g: u32) -> bool {
    g == 0x1508
}
unsafe extern "C" fn occupant_lot(_u: *mut c_void, o: u64) -> u64 {
    o >> 8
}
unsafe extern "C" fn lot_config(u: *mut c_void, lot: u64, id: *mut u32, stage: *mut u32) -> bool {
    match game(u).lots.borrow().get(&lot) {
        Some(l) => {
            *id = l.0;
            *stage = l.1;
            true
        }
        None => false,
    }
}
unsafe extern "C" fn lot_location(u: *mut c_void, lot: u64, x: *mut i16, z: *mut i16) {
    let l = game(u).lots.borrow()[&lot];
    *x = l.2;
    *z = l.3;
}
unsafe extern "C" fn lot_at(u: *mut c_void, x: i16, z: i16) -> u64 {
    game(u)
        .lots
        .borrow()
        .iter()
        .find(|(_, l)| l.2 == x && l.3 == z)
        .map_or(0, |(&k, _)| k)
}
unsafe extern "C" fn lot_state(_u: *mut c_void, _lot: u64) -> u32 {
    3
}
unsafe extern "C" fn set_lot_condition(u: *mut c_void, lot: u64, c: u32) {
    game(u).log(format!("condition {lot:#x} {c}"));
}
unsafe extern "C" fn lot_rect(u: *mut c_void, lot: u64, r: *mut fc4_cell_rect) {
    let l = game(u).lots.borrow()[&lot];
    *r = fc4_cell_rect {
        x1: i32::from(l.2),
        z1: i32::from(l.3),
        x2: i32::from(l.2),
        z2: i32::from(l.3),
    };
}
unsafe extern "C" fn update_effects(u: *mut c_void, lot: u64, old: f32, new: f32) {
    game(u).log(format!("effects {lot:#x} {old} {new}"));
}
unsafe extern "C" fn record_history(u: *mut c_void, key: u32, v: i32) {
    game(u).log(format!("history {key:#x} {v}"));
}
unsafe extern "C" fn office_counts(u: *mut c_void, t: u32, sink: fc4_u32s_sink, ctx: *mut c_void) {
    let (a, b) = game(u).offices.get();
    let v = [if t == 0x3320 { a } else { b }];
    sink.unwrap()(ctx, v.as_ptr(), 1);
}
unsafe extern "C" fn lot_subnetworks(_u: *mut c_void, _lot: u64, sink: fc4_u32s_sink, ctx: *mut c_void) {
    let v = [0u32];
    sink.unwrap()(ctx, v.as_ptr(), 1);
}
unsafe extern "C" fn terrain_size(_u: *mut c_void, w: *mut u32, d: *mut u32) {
    *w = 4;
    *d = 3;
}
unsafe extern "C" fn cell_has_water(u: *mut c_void, x: u32, z: u32) -> bool {
    game(u).water.contains(&(x, z))
}

fn host(g: &Game) -> fc4_port_host {
    // SAFETY: every field is a nullable pointer or an integer.
    let mut h: fc4_port_host = unsafe { std::mem::zeroed() };
    h.struct_size = std::mem::size_of::<fc4_port_host>() as u32;
    h.user = g.user();
    h.exemplar = Some(exemplar);
    h.add_agent = Some(add_agent);
    h.create_department = Some(create_department);
    h.line_expenses = Some(line_expenses);
    h.set_full_expenses = Some(set_full_expenses);
    h.occupant_type = Some(occupant_type);
    h.occupant_in_group = Some(occupant_in_group);
    h.occupant_lot = Some(occupant_lot);
    h.lot_config = Some(lot_config);
    h.lot_location = Some(lot_location);
    h.lot_at = Some(lot_at);
    h.lot_state = Some(lot_state);
    h.set_lot_condition = Some(set_lot_condition);
    h.lot_rect = Some(lot_rect);
    h.update_occupant_effects = Some(update_effects);
    h.record_history = Some(record_history);
    h.office_counts = Some(office_counts);
    h.lot_subnetworks = Some(lot_subnetworks);
    h.terrain_size = Some(terrain_size);
    h.cell_has_water = Some(cell_has_water);
    h
}

fn write(p: *const fc4_port_developer) -> Vec<u8> {
    let mut o = Out::default();
    let os = ostream(&mut o);
    assert!(unsafe { fc4_port_developer_write(p, &os) });
    o.bytes
}

fn read(p: *mut fc4_port_developer, bytes: &[u8]) -> bool {
    let mut i = In::new(bytes);
    let is = istream(&mut i);
    unsafe { fc4_port_developer_read(p, &is) }
}

#[test]
fn create_and_identity() {
    let g = Game::new();
    unsafe {
        assert!(fc4_port_developer_create(std::ptr::null(), 0, 0).is_null());
        assert!(fc4_port_developer_create(&host(&g), 2, 0).is_null());
        let mut h = host(&g);
        h.struct_size -= 1;
        assert!(fc4_port_developer_create(&h, 0, 0).is_null());
        let mut off = 99;
        assert!(fc4_port_developer_query_interface(0, 0xEA20_548D, &mut off));
        assert_eq!(off, 0);
        assert!(fc4_port_developer_query_interface(1, 1, &mut off));
        assert_eq!(off, 4);
        assert!(fc4_port_developer_query_interface(1, 0xE4FD_A3D4, &mut off));
        assert_eq!(off, 0xC);
        assert!(!fc4_port_developer_query_interface(1, 0xEA20_548D, &mut off));
        assert!(!fc4_port_developer_query_interface(5, 1, &mut off));
        assert_eq!(fc4_port_developer_num_ports(std::ptr::null(), 0x1F, 0x1F), 0);
    }
}

/// §12 tests 1–4 through the ABI.
#[test]
fn airport_adoption_month_and_record() {
    let g = Game::new();
    unsafe {
        let p = fc4_port_developer_create(&host(&g), 0, 0x77);
        assert_eq!(write(p), [0x02, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0, 0]);
        assert!(fc4_port_developer_init(p));
        assert_eq!(
            g.take(),
            // §3 (PD10): the base's agent, then the subclass's department.
            ["add_agent cSC4PortDeveloper 6 2", "create_department 0x6a4119f0 1"]
        );
        g.lots.borrow_mut().insert(0x10, (0xCA24_36C0, 1, 3, 4));
        assert!(fc4_port_developer_do_message(p, 0x99EF_1142, 0x1000, 0, 0));
        assert_eq!(g.take(), ["full 700"]);
        g.offices.set((4000, 1200));
        assert!(fc4_port_developer_do_message(p, 0x6695_6816, 0, 0, 0));
        assert_eq!(
            g.take(),
            ["effects 0x10 0 0.52", "condition 0x10 0", "history 0x6a3e9032 48"]
        );
        let mut l = fc4_port_lot::default();
        assert!(fc4_port_developer_get_port(p, 0, &mut l));
        assert_eq!((l.trips, l.lot, l.x, l.z), (5200, 0x10, 3, 4));
        assert!(!fc4_port_developer_get_port(p, 1, &mut l));
        let (mut u, mut c) = (0, 0);
        assert!(fc4_port_developer_usage_values(p, 0x10, &mut u, &mut c));
        assert_eq!((u, c), (5200, 10_000));
        assert!(!fc4_port_developer_usage_values(p, 0x11, &mut u, &mut c));
        assert_eq!(fc4_port_developer_num_trips(p), 5200);
        assert_eq!(fc4_port_developer_maintenance_cost(p, 0x10), 700);
        assert_eq!(fc4_port_developer_num_airports(p, 0), 1);
        assert_eq!(fc4_port_developer_num_ports(p, 2, 1), 1);
        assert!((fc4_port_developer_avg_efficiency(p) - 0.48).abs() < 1e-6);
        let (mut e, mut x, mut z) = (0.0, 0, 0);
        assert!(fc4_port_developer_least_efficient(p, &mut e, &mut x, &mut z));
        assert_eq!((x, z), (3, 4));
        let mut lots = [0u64; 4];
        assert_eq!(fc4_port_developer_all_port_lots(p, lots.as_mut_ptr(), 4), 1);
        assert_eq!(lots[0], 0x10);
        let rec = write(p);
        assert_eq!(
            rec,
            [0x02, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0x01, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0]
        );
        let mut s = fc4_port_state::default();
        assert!(fc4_port_developer_get_state(p, &mut s));
        assert_eq!(s.struct_size, 64);
        assert_eq!((s.kind, s.port_count, s.types_loaded, s.department), (0, 1, 3, 0xD));
        assert_eq!((s.month_ran, s.month_history), (1, 48));

        // A fresh developer reads the record; the lot resolves by location.
        let q = fc4_port_developer_create(&host(&g), 0, 0x78);
        assert!(read(q, &rec));
        assert!(!read(q, &rec[..5]));
        assert!(fc4_port_developer_do_message(q, 0x26D3_1EC1, 0, 0, 0));
        assert!(fc4_port_developer_get_port(q, 0, &mut l));
        assert_eq!((l.lot, l.needs_lookup), (0x10, 0));
        assert_eq!(fc4_port_developer_release(q), 0);
        fc4_port_developer_destroy(q);
        fc4_port_developer_destroy(p);
    }
}

/// §12 test 9 through the ABI.
#[test]
fn seaport_map() {
    let g = Game::new();
    unsafe {
        let p = fc4_port_developer_create(&host(&g), 1, 0x79);
        assert_eq!(write(p), [0x02, 0, 0x01, 0, 0x92, 0xAD, 0x29, 0xEA, 0x0C, 0, 0, 0]);
        assert!(fc4_port_developer_init(p));
        assert!(fc4_port_developer_city_has_sea(p));
        let got: Vec<u8> = (0..3)
            .flat_map(|z| (0..4).map(move |x| (x, z)))
            .map(|(x, z)| fc4_port_developer_cell_sea_access(p, x, z))
            .collect();
        assert_eq!(got, [0x83, 0x83, 0, 0, 0, 0x83, 0, 0, 0, 0, 0, 0x8C]);
        assert_eq!(fc4_port_developer_num_seaports(p), 0);
        fc4_port_developer_destroy(p);
    }
}
