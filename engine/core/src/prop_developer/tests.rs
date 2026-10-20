// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The conformance tests of `docs/specs/prop-developer.md` §12 and the
//! behaviours around them.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use super::*;
use crate::stream::{ByteReader, ByteWriter, RefResolver};
use crate::variant::{mem, Variant};

// ---- streams that write a reference as (token, class) -----------------------------------------

struct TokenWriter(ByteWriter);

impl OutStream for TokenWriter {
    fn put_u8(&mut self, v: u8) -> bool {
        self.0.put_u8(v)
    }
    fn put_u16(&mut self, v: u16) -> bool {
        self.0.put_u16(v)
    }
    fn put_u32(&mut self, v: u32) -> bool {
        self.0.put_u32(v)
    }
    fn put_u64(&mut self, v: u64) -> bool {
        self.0.put_u64(v)
    }
    fn put_f32(&mut self, v: f32) -> bool {
        self.0.put_f32(v)
    }
    fn put_f64(&mut self, v: f64) -> bool {
        self.0.put_f64(v)
    }
    fn put_bytes(&mut self, v: &[u8]) -> bool {
        self.0.put_bytes(v)
    }
    fn put_str(&mut self, v: &[u8]) -> bool {
        self.0.put_str(v)
    }
    fn put_ref(&mut self, r: &RefValue) -> bool {
        match r {
            RefValue::Null => self.0.put_u32(0),
            RefValue::Object(o) => self.0.put_u32(o.host_token() as u32) && self.0.put_u32(o.class_id()),
            RefValue::Host { token, class_id } => self.0.put_u32(*token as u32) && self.0.put_u32(*class_id),
        }
    }
    fn error(&self) -> i32 {
        OutStream::error(&self.0)
    }
}

struct TokenResolver;

impl RefResolver for TokenResolver {
    fn resolve(&mut self, class_id: u32, object_id: u32) -> Option<RefValue> {
        Some(RefValue::Host {
            token: u64::from(object_id),
            class_id,
        })
    }
}

fn record(d: &PropDeveloper) -> Vec<u8> {
    let mut w = TokenWriter(ByteWriter::new());
    assert!(d.write(&mut w));
    w.0.into_bytes()
}

fn read(d: &PropDeveloper, bytes: &[u8]) -> bool {
    let mut res = TokenResolver;
    d.read(&mut ByteReader::with_resolver(bytes, &mut res))
}

fn hex(s: &str) -> Vec<u8> {
    let c: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    (0..c.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&c[i..i + 2], 16).unwrap())
        .collect()
}

// ---- a scripted generator ---------------------------------------------------------------------

#[derive(Clone, Default)]
struct Script(Rc<RefCell<(VecDeque<u32>, Vec<String>)>>);

impl Script {
    fn push(&self, v: &[u32]) {
        self.0.borrow_mut().0.extend(v);
    }
    fn draws(&self) -> Vec<String> {
        std::mem::take(&mut self.0.borrow_mut().1)
    }
}

impl PropRandom for Script {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        let mut s = self.0.borrow_mut();
        s.1.push(format!("U({n})"));
        s.0.pop_front().unwrap_or(0)
    }
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        let mut s = self.0.borrow_mut();
        s.1.push(format!("RS({lo}, {hi})"));
        lo + s.0.pop_front().unwrap_or(0) as i32
    }
}

// ---- a fake city -------------------------------------------------------------------------------

#[derive(Clone)]
struct Lot {
    x: i32,
    z: i32,
    size: (i32, i32),
    livable: bool,
    building: u32,
    occupancy: f32,
    objs: Option<Rc<Vec<CfgObject>>>,
    wealth: u32,
    state: u32,
}

#[derive(Clone)]
struct Prop {
    lot: Option<u64>,
    cell: (i32, i32),
    tag: Option<u32>,
    ptype: u32,
    props: PropertyHolder,
    state: u32,
    pos: [f32; 3],
}

/// (network lot token, its configuration objects)
type NetworkLot = (u64, Rc<Vec<CfgObject>>);

#[derive(Default)]
struct Fake {
    cells: Cell<(i32, i32)>,
    lots: RefCell<BTreeMap<u64, Lot>>,
    network: RefCell<BTreeMap<(i32, i32), NetworkLot>>,
    zones: RefCell<BTreeMap<(i32, i32), u32>>,
    props: RefCell<BTreeMap<u64, Prop>>,
    /// id → (satisfaction, wealth)
    profiles: RefCell<BTreeMap<u32, (u32, u8)>>,
    families: RefCell<BTreeMap<u32, Vec<u32>>>,
    /// token → (share, satisfaction, custom)
    requesters: RefCell<BTreeMap<u64, (i32, u32, bool)>>,
    unpowered: RefCell<BTreeSet<(i32, i32)>>,
    clock: Cell<Option<(u32, u32)>>,
    mode: Cell<u32>,
    night: Cell<bool>,
    view: Cell<bool>,
    next: Cell<u64>,
    /// `CreateLotProp` answers null (PV8).
    fail_create: Cell<bool>,
    log: RefCell<Vec<String>>,
}

impl Fake {
    fn new() -> Rc<Fake> {
        let f = Fake::default();
        f.cells.set((64, 64));
        f.next.set(0x9000);
        f.view.set(true);
        Rc::new(f)
    }
    fn log(&self, s: String) {
        self.log.borrow_mut().push(s);
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    fn lot(&self, l: u64) -> Lot {
        self.lots.borrow()[&l].clone()
    }
    fn prop(&self, p: u64) -> Prop {
        self.props.borrow()[&p].clone()
    }
    fn add_lot(&self, token: u64, x: i32, z: i32, occupancy: f32, objs: Vec<CfgObject>) {
        self.lots.borrow_mut().insert(
            token,
            Lot {
                x,
                z,
                size: (1, 1),
                livable: true,
                building: 0x100,
                occupancy,
                objs: Some(Rc::new(objs)),
                wealth: 2,
                state: 3,
            },
        );
    }
    fn add_prop(&self, token: u64, lot: Option<u64>, tag: Option<u32>, ptype: u32, props: PropertyHolder) {
        self.props.borrow_mut().insert(
            token,
            Prop {
                lot,
                cell: (0, 0),
                tag,
                ptype,
                props,
                state: 0,
                pos: [8.0, 0.0, 8.0],
            },
        );
    }
    fn on_lot(&self, lot: u64) -> Vec<u64> {
        self.props
            .borrow()
            .iter()
            .filter(|(_, p)| p.lot == Some(lot))
            .map(|(&k, _)| k)
            .collect()
    }
}

fn opt_prop(tag: u32, ids: &[u32]) -> CfgObject {
    CfgObject {
        object_type: 1,
        flags: 1,
        tag,
        ids: ids.to_vec(),
        ..CfgObject::default()
    }
}

impl PropHost for Fake {
    fn city_cells(&self) -> (i32, i32) {
        self.cells.get()
    }
    fn cell_width(&self) -> (f32, f32) {
        (16.0, 16.0)
    }
    fn managers(&self) -> [PropRef; 3] {
        [
            PropRef::new(0x1331_0038, 0x498F_9B01),
            PropRef::new(0x1137_876C, 0xA990_BD10),
            PropRef::new(0x13B4_35FC, 0xE9A6_CD3B),
        ]
    }
    fn create_wealth_requester(&self) -> u64 {
        0x13B5_2FC0
    }
    fn add_agent(&self, level: u32, name: &str, flags: u32) {
        self.log(format!("add_agent {level} {name} {flags}"));
    }
    fn remove_agents(&self, name: &str) {
        self.log(format!("remove_agents {name}"));
    }
    fn subscribe(&self, _msg: u32) {}
    fn unsubscribe_all(&self) {
        self.log("unsubscribe_all".into());
    }
    fn register_animation_tick(&self, p: u32) {
        self.log(format!("animation {p:#x}"));
    }
    fn unregister_animation_tick(&self) {
        self.log("unanimation".into());
    }
    fn post_lot_state_props(&self, x: i32, z: i32, entered: bool) {
        self.log(format!("post {x} {z} {entered}"));
    }
    fn hold(&self, _t: u64) {}
    fn release(&self, _t: u64) {}
    fn requester_share(&self, r: u64, _rect: CellRect) -> i32 {
        self.requesters.borrow().get(&r).map_or(0, |v| v.0)
    }
    fn requester_satisfaction(&self, r: u64) -> u32 {
        self.requesters.borrow().get(&r).map_or(0, |v| v.1)
    }
    fn requester_custom(&self, r: u64) -> bool {
        self.requesters.borrow().get(&r).is_some_and(|v| v.2)
    }
    fn requester_select(&self, r: u64, ids: &[u32], _x: i32, _z: i32) -> u32 {
        self.log(format!("select {r:#x} {ids:x?}"));
        ids.first().copied().unwrap_or(0)
    }
    fn lot_at(&self, x: i32, z: i32, origin_only: bool) -> Option<u64> {
        self.lots
            .borrow()
            .iter()
            .find(|(_, l)| {
                if origin_only {
                    l.x == x && l.z == z
                } else {
                    (l.x..l.x + l.size.0).contains(&x) && (l.z..l.z + l.size.1).contains(&z)
                }
            })
            .map(|(&k, _)| k)
    }
    fn lot_livable(&self, lot: u64) -> bool {
        self.lot(lot).livable
    }
    fn lot_building_type(&self, lot: u64) -> u32 {
        self.lot(lot).building
    }
    fn lot_occupancy(&self, lot: u64) -> f32 {
        self.lot(lot).occupancy
    }
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>> {
        self.lot(lot).objs
    }
    fn lot_wealth(&self, lot: u64) -> u32 {
        self.lot(lot).wealth
    }
    fn lot_location(&self, lot: u64) -> (i32, i32) {
        let l = self.lot(lot);
        (l.x, l.z)
    }
    fn lot_size(&self, lot: u64) -> (i32, i32) {
        self.lot(lot).size
    }
    fn lot_state(&self, lot: u64) -> u32 {
        self.lot(lot).state
    }
    fn zone_type(&self, x: i32, z: i32) -> u32 {
        self.zones.borrow().get(&(x, z)).copied().unwrap_or(0)
    }
    fn network_lot_at(&self, x: i32, z: i32) -> Option<u64> {
        self.network.borrow().get(&(x, z)).map(|v| v.0)
    }
    fn network_lot_objects(&self, nl: u64) -> Rc<Vec<CfgObject>> {
        self.network
            .borrow()
            .values()
            .find(|v| v.0 == nl)
            .map(|v| v.1.clone())
            .unwrap_or_default()
    }
    fn network_wealth(&self, _x: i32, _z: i32) -> u32 {
        1
    }
    fn lot_props(&self, lot: u64) -> Vec<u64> {
        self.on_lot(lot)
    }
    fn street_props(&self, x: i32, z: i32) -> Vec<u64> {
        self.props
            .borrow()
            .iter()
            .filter(|(_, p)| p.lot.is_none() && p.cell == (x, z))
            .map(|(&k, _)| k)
            .collect()
    }
    fn street_props_in_rect(&self, r: CellRect) -> Vec<u64> {
        self.props
            .borrow()
            .iter()
            .filter(|(_, p)| p.lot.is_none() && (r.x1..=r.x2).contains(&p.cell.0) && (r.z1..=r.z2).contains(&p.cell.1))
            .map(|(&k, _)| k)
            .collect()
    }
    fn remove_prop(&self, prop: u64) {
        self.log(format!("remove {prop:#x}"));
        self.props.borrow_mut().remove(&prop);
    }
    fn prop_type_valid(&self, id: u32) -> bool {
        self.profiles.borrow().contains_key(&id)
    }
    fn props_by_family(&self, family: u32) -> Vec<u32> {
        self.families.borrow().get(&family).cloned().unwrap_or_default()
    }
    fn prop_satisfaction(&self, id: u32) -> u32 {
        self.profiles.borrow().get(&id).map_or(0, |p| p.0)
    }
    fn prop_wealth(&self, id: u32) -> u8 {
        self.profiles.borrow().get(&id).map_or(0, |p| p.1)
    }
    fn create_lot_prop(&self, lot: u64, index: usize, id: u32) -> Option<u64> {
        if self.fail_create.get() {
            self.log(format!("create {lot:#x} {index} {id:#x} -> null"));
            return None;
        }
        let t = self.next.get();
        self.next.set(t + 1);
        let tag = self.lot(lot).objs.and_then(|o| o.get(index).map(|o| o.tag));
        self.add_prop(t, Some(lot), tag, id, PropertyHolder::new());
        self.log(format!("create {lot:#x} {index} {id:#x} -> {t:#x}"));
        Some(t)
    }
    fn create_network_prop(&self, nl: u64, index: usize, id: u32) -> Option<u64> {
        self.log(format!("create_network {nl:#x} {index} {id:#x}"));
        None
    }
    fn prop_class(&self, _prop: u64) -> u32 {
        0x2977_AA47
    }
    fn prop_properties(&self, prop: u64) -> Option<Rc<PropertyHolder>> {
        self.props.borrow().get(&prop).map(|p| Rc::new(p.props.clone()))
    }
    fn prop_lot_tag(&self, prop: u64) -> Option<u32> {
        self.prop(prop).tag
    }
    fn set_prop_requester(&self, prop: u64, r: u32) {
        if let Some(p) = self.props.borrow_mut().get_mut(&prop) {
            p.props.add(PROP_REQUESTER_INDEX, Variant::Scalar(Elem::U32(r)));
        }
    }
    fn prop_type(&self, prop: u64) -> u32 {
        self.prop(prop).ptype
    }
    fn prop_state(&self, prop: u64) -> u32 {
        self.prop(prop).state
    }
    fn set_prop_state(&self, prop: u64, s: u32) {
        if let Some(p) = self.props.borrow_mut().get_mut(&prop) {
            p.state = s;
        }
        self.log(format!("state {prop:#x} {s}"));
    }
    fn set_prop_timer(&self, prop: u64, mask: u32, on: bool) {
        self.log(format!("timer {prop:#x} {mask} {on}"));
    }
    fn set_random_chance(&self, prop: u64, c: u8) {
        self.log(format!("chance {prop:#x} {c}"));
    }
    fn set_time_of_day_range(&self, prop: u64, s: f32, e: f32) {
        self.log(format!("tod_range {prop:#x} {s} {e}"));
        let mut h = self.prop(prop).props;
        h.add(0x7777, Variant::array(mem::FLOAT32, vec![Elem::F32(s), Elem::F32(e)]));
        self.props.borrow_mut().get_mut(&prop).unwrap().props = h;
    }
    fn time_of_day_range(&self, prop: u64) -> (f32, f32) {
        let v = self.prop(prop).props.get(0x7777).unwrap().items();
        (v[0].as_f64() as f32, v[1].as_f64() as f32)
    }
    fn set_date_range(&self, prop: u64, date: u32, duration: u32, flag: u8) {
        self.log(format!("date_range {prop:#x} {date} {duration} {flag}"));
    }
    fn tick_date(&self, prop: u64, date: u32) {
        self.log(format!("tick_date {prop:#x} {date}"));
    }
    fn prop_position(&self, prop: u64) -> [f32; 3] {
        self.prop(prop).pos
    }
    fn set_prop_position(&self, prop: u64, pos: [f32; 3]) {
        self.log(format!("position {prop:#x} {}", pos[1]));
    }
    fn set_prop_flag(&self, prop: u64, flag: u32, on: bool) {
        self.log(format!("flag {prop:#x} {flag} {on}"));
    }
    fn network_first_cell(&self, _o: u64) -> (i32, i32) {
        (0, 0)
    }
    fn position_to_cell(&self, x: f32, z: f32) -> (i32, i32) {
        ((x / 16.0) as i32, (z / 16.0) as i32)
    }
    fn is_powered(&self, x: i32, z: i32) -> bool {
        !self.unpowered.borrow().contains(&(x, z))
    }
    fn view_available(&self) -> bool {
        self.view.get()
    }
    fn view_trigger_state(&self, t: u32) -> bool {
        t & 1 != 0
    }
    fn sim_date(&self) -> u32 {
        777
    }
    fn clock_time(&self) -> Option<(u32, u32)> {
        self.clock.get()
    }
    fn visual_time_mode(&self) -> u32 {
        self.mode.get()
    }
    fn lighting_is_night(&self) -> bool {
        self.night.get()
    }
    fn terrain_heights(&self, _x: f32, _z: f32) -> (f32, f32) {
        (10.0, 12.0)
    }
}

struct Env {
    f: Rc<Fake>,
    d: Rc<PropDeveloper>,
    member: Script,
    wealth: Script,
}

fn env() -> Env {
    let f = Fake::new();
    let member = Script::default();
    let wealth = Script::default();
    let shared: SharedRandom = Rc::new(RefCell::new(wealth.clone()));
    let d = PropDeveloper::new(f.clone(), Box::new(member.clone()), shared, 0xD0);
    Env { f, d, member, wealth }
}

const CRIME: u64 = 0x13B4_2BB8;
const POLLUTION: u64 = 0x1134_7690;

/// Init plus the crime and pollution requesters (shares 0).
fn city(e: &Env) {
    e.d.init();
    for (t, class, sat, custom) in [
        (CRIME, 0x8990_C09A, 0x49A1_E996, true),
        (POLLUTION, 0x8990_C065, 0xE9A4_300B, false),
    ] {
        e.f.requesters.borrow_mut().insert(t, (0, sat, custom));
        e.d.add_prop_requester(Requester::Host(PropRef::new(t, class)));
    }
    e.f.take();
}

#[test]
fn identity() {
    assert_eq!(query_interface(IID_PROP_DEVELOPER), Some(0));
    assert_eq!(query_interface(1), Some(0));
    assert_eq!(query_interface(IID_MESSAGE_TARGET), Some(4));
    assert_eq!(query_interface(IID_SERIALIZABLE), Some(8));
    assert_eq!(query_interface(IID_PROP_REQUESTER), None);
    assert_eq!(wealth_query_interface(IID_PROP_REQUESTER), Some(0));
    assert_eq!(wealth_query_interface(IID_SERIALIZABLE), Some(4));
    assert_eq!(wealth_query_interface(IID_PROP_DEVELOPER), None);
    assert_eq!(
        member_seed(0x1234_5678),
        derive_seed(0x1234_5678, 0x89C4_8F47).wrapping_mul(28)
    );
}

#[test]
fn init_and_shutdown() {
    let e = env();
    e.f.cells.set((256, 256));
    assert!(e.d.init());
    assert_eq!(
        e.f.take(),
        [
            "add_agent 10 cSC4PropDeveloper 2",
            "add_agent 4 cSC4PropDeveloper 2",
            "animation 0xc0"
        ]
    );
    let s = e.d.snapshot();
    assert_eq!(s.scan_flag, 1);
    assert_eq!((s.scanner.x1, s.scanner.z1), (31, 31));
    assert!(s.wealth_added);
    assert_eq!(e.d.requesters().len(), 1);
    e.d.shutdown();
    assert_eq!(
        e.f.take(),
        ["remove_agents cSC4PropDeveloper", "unsubscribe_all", "unanimation"]
    );
    assert_eq!(e.d.snapshot().scan_flag, 0);
    assert!(!e.d.snapshot().wealth_added);
    assert!(e.d.requesters().is_empty());
    // A tiny city clips to (0, 0).
    let e = env();
    e.f.cells.set((4, 4));
    e.d.init();
    assert_eq!((e.d.snapshot().scanner.x1, e.d.snapshot().scanner.z1), (0, 0));
}

const EMPTY_V5: &str = "0500 01 00 00000000 00000000 01000000 00000000 00000000 0F000000 0F000000 01
    03000000 C02FB513 8699C489 B82BB413 9AC09089 90763411 65C09089
    38003113 019B8F49 6C873711 10BD90A9 FC35B413 3BCDA6E9
    00000000 00000000 00000000 00000000 00 00000000 00000000
    00000000 00000000 00000000 01 00000000";

// §12 test 1.
#[test]
fn t1_empty_record() {
    let e = env();
    e.f.cells.set((128, 128));
    city(&e);
    let r = record(&e.d);
    assert_eq!(r.len(), 127);
    assert_eq!(r, hex(EMPTY_V5));
}

const TEMPLATE_V4: &str = "0400 01 00 00000000 00000000 01000000 00000000 00000000 0F000000 0F000000 01
    03000000 C02FB513 8699C489 B82BB413 9AC09089 90763411 65C09089
    38003113 019B8F49 6C873711 10BD90A9 FC35B413 3BCDA6E9
    00000000 00000000 0DF0ADBA 00000000 00 00000000 00000000
    00000000 00000000 00000000";

// §12 test 2.
#[test]
fn t2_template_read() {
    let e = env();
    let t = hex(TEMPLATE_V4);
    assert_eq!(t.len(), 122);
    assert!(read(&e.d, &t));
    let s = e.d.snapshot();
    assert_eq!(s.night_batch, 0xBAAD_F00D);
    assert!(s.signs_visible);
    assert!(matches!(e.d.requesters()[0], Requester::Wealth(_)));
    let mut v5 = hex(EMPTY_V5);
    v5[0] = 5;
    // Batch kept.
    v5[2 + 1 + 29 + 1 + 4 + 24 + 24 + 8..][..4].copy_from_slice(&[0x0D, 0xF0, 0xAD, 0xBA]);
    assert_eq!(record(&e.d), v5);
    // Version 3 is rejected and changes nothing.
    let mut v3 = t.clone();
    v3[0] = 3;
    let fresh = env();
    assert!(!read(&fresh.d, &v3));
    assert!(fresh.d.requesters().is_empty());
    // Short records are rejected.
    assert!(!read(&fresh.d, &t[..100]));
}

const TUTORIAL: &str =
    "05 00 02 01 07 00 00 00 0E 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00 0F 00 00 00 0F 00 00 00 01
    03 00 00 00 C0 2F B5 13 86 99 C4 89 B8 2B B4 13 9A C0 90 89 90 76 34 11 65 C0 90 89
    38 00 31 13 01 9B 8F 49 6C 87 37 11 10 BD 90 A9 FC 35 B4 13 3B CD A6 E9
    00 00 00 00 02 00 00 00 00 00 00 00 B8 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00";

// §12 test 3 (the example of §7).
#[test]
fn t3_round_trip() {
    let e = env();
    let t = hex(TUTORIAL);
    assert_eq!(t.len(), 127);
    assert!(read(&e.d, &t));
    let s = e.d.snapshot();
    assert_eq!(
        (s.scan_flag, s.scanner.x, s.scanner.z, s.scanner.state),
        (2, 7, 14, ScanState::Running)
    );
    assert_eq!((s.night_mode, s.night_acc), (2, 184));
    assert_eq!(record(&e.d), t);
    // Lists with references round-trip too.
    let mut with_lists = t.clone();
    with_lists.truncate(t.len() - 25);
    with_lists.extend(hex("02000000 11000000 47AA7729 12000000 49AA7729"));
    with_lists.extend(hex("01000000 C6DFCBCA 01000000 12000000 49AA7729"));
    with_lists.extend(hex("01000000 13000000 47AA7729 00000000 01000000 11000000 47AA7729"));
    with_lists.extend(hex("00 01000000 14000000 47AA7729"));
    let d = env().d;
    assert!(read(&d, &with_lists));
    assert_eq!(d.night_list().len(), 2);
    assert_eq!(d.trigger_map(), [(0xCACB_DFC6, vec![PropRef::new(0x12, 0x2977_AA49)])]);
    assert!(!d.signs_visible());
    assert_eq!(record(&d), with_lists);
}

// §12 test 4.
#[test]
fn t4_block_order() {
    let e = env();
    city(&e);
    assert_eq!(e.d.block_step(), None);
    let mut got = Vec::new();
    for _ in 0..64 {
        got.push(e.d.block_step().unwrap());
    }
    let want: Vec<(i32, i32)> = (0..8).flat_map(|z| (0..8).map(move |x| (x, z))).collect();
    assert_eq!(got, want);
    assert_eq!(e.d.block_step(), Some((0, 0)));
    // Idle and Day both step.
    e.d.do_message(&PropMessage {
        msg_type: MSG_IDLE,
        ..Default::default()
    });
    e.d.do_message(&PropMessage {
        msg_type: MSG_DAY,
        ..Default::default()
    });
    assert_eq!(e.d.last_block().block, (2, 0));
}

/// A block with one lot at (0, 0): occupancy `occ`, four optional prop
/// objects 0xA0..0xA3 each listing wealth props 0x500 (wealth 2) and 0x501
/// (wealth 3).
fn example_lot(e: &Env, occ: f32) {
    e.f.add_lot(
        0x10,
        0,
        0,
        occ,
        (0..4).map(|i| opt_prop(0xA0 + i, &[0x500, 0x501])).collect(),
    );
    e.f.profiles.borrow_mut().insert(0x500, (WEALTH_SATISFACTION, 2));
    e.f.profiles.borrow_mut().insert(0x501, (WEALTH_SATISFACTION, 3));
}

// §12 test 5 (worked example A) and test 6 (example C).
#[test]
fn t5_t6_examples_a_b_c() {
    let e = env();
    city(&e);
    example_lot(&e, 0.5);
    e.member.push(&[2, 1]);
    e.d.update_block(0, 0);
    let o = e.d.last_block();
    assert_eq!((o.lots, o.slots, o.kept_all, o.target), (1, 4, 0, 2));
    assert_eq!(o.want, [2, 0, 0]);
    assert_eq!(o.created, 2);
    assert_eq!(e.member.draws(), ["RS(0, 4)", "RS(0, 3)"]);
    assert_eq!(e.wealth.draws(), ["U(1)", "U(1)"]);
    // RS(0, 4) = 2 picks object 2; then RS(0, 3) = 1 of (0, 1, 3) picks 1.
    assert_eq!(
        e.f.take(),
        ["create 0x10 2 0x500 -> 0x9000", "create 0x10 1 0x500 -> 0x9001"]
    );
    assert_eq!(
        e.f.prop(0x9000).props.get(PROP_REQUESTER_INDEX),
        Some(&Variant::Scalar(Elem::U32(0)))
    );
    // B: nothing changes; each kept prop costs one wealth draw.
    e.d.update_block(0, 0);
    let o = e.d.last_block();
    assert_eq!((o.slots, o.kept_all, o.target, o.created, o.removed), (4, 2, 2, 0, 0));
    assert!(e.member.draws().is_empty());
    assert_eq!(e.wealth.draws(), ["U(1)", "U(1)"]);
    // C: occupancy 0.25 → one removal with RS(0, 2).
    e.f.lots.borrow_mut().get_mut(&0x10).unwrap().occupancy = 0.25;
    e.member.push(&[1]);
    e.d.update_block(0, 0);
    let o = e.d.last_block();
    assert_eq!((o.target, o.removed, o.created, o.slots), (1, 1, 0, 5));
    assert_eq!(e.member.draws(), ["RS(0, 2)"]);
    assert_eq!(e.f.take(), ["remove 0x9001"]);
}

#[test]
fn example_e_wealth_change_removes_all() {
    let e = env();
    city(&e);
    example_lot(&e, 0.5);
    e.member.push(&[0, 0]);
    e.d.update_block(0, 0);
    e.f.take();
    e.member.draws();
    e.wealth.draws();
    e.f.lots.borrow_mut().get_mut(&0x10).unwrap().wealth = 3;
    e.d.update_block(0, 0);
    let log = e.f.take();
    assert_eq!(&log[..2], ["remove 0x9000", "remove 0x9001"]);
    assert_eq!(e.d.last_block().created, 2);
    assert!(log[2..].iter().all(|l| l.contains("0x501")));
}

// §12 test 7.
#[test]
fn t7_split() {
    let cases: [(&[i32], u32, &[u32]); 3] = [
        (&[5, 3, 2], 13, &[6, 4, 3]),
        (&[10, 0, 0], 2, &[2, 0, 0]),
        (&[0, 4, 0], 3, &[0, 3, 0]),
    ];
    for (shares, n, want) in cases {
        let e = env();
        e.d.edit_state(|s| s.wealth_added = true);
        e.d.init();
        for (i, &s) in shares.iter().enumerate() {
            let t = 0x100 + i as u64;
            e.f.requesters.borrow_mut().insert(t, (s, 0x77, false));
            e.d.add_prop_requester(Requester::Host(PropRef::new(t, 0x1)));
        }
        // N = ⌊1.0 × n⌋ with n optional slots.
        e.f.add_lot(0x10, 0, 0, 1.0, (0..n).map(|i| opt_prop(0xA0 + i, &[])).collect());
        e.d.update_block(0, 0);
        let o = e.d.last_block();
        assert_eq!(o.target, n);
        assert_eq!(o.want, want, "shares {shares:?}");
    }
}

// §12 test 8.
#[test]
fn t8_no_lots() {
    let e = env();
    city(&e);
    e.f.network
        .borrow_mut()
        .insert((1, 1), (0x50, Rc::new(vec![opt_prop(0xB0, &[0x500])])));
    e.f.profiles.borrow_mut().insert(0x500, (WEALTH_SATISFACTION, 1));
    e.d.update_block(0, 0);
    let o = e.d.last_block();
    assert!(o.stopped);
    assert_eq!((o.lots, o.slots, o.created), (0, 1, 0));
    assert!(e.member.draws().is_empty());
    assert!(e.wealth.draws().is_empty());
    assert!(e.f.take().is_empty());
}

// §12 test 9.
#[test]
fn t9_wealth_selection() {
    let e = env();
    city(&e);
    for (id, w) in [(1, 1), (2, 2), (3, 3), (4, 2)] {
        e.f.profiles.borrow_mut().insert(id, (WEALTH_SATISFACTION, w));
    }
    e.f.add_lot(0x10, 5, 5, 1.0, vec![]);
    e.wealth.push(&[1]);
    assert_eq!(e.d.wealth_select(&[1, 2, 3, 4], 5, 5), 4);
    assert_eq!(e.wealth.draws(), ["U(2)"]);
    // A seaport lot of wealth 3 counts as wealth 2.
    e.f.lots.borrow_mut().get_mut(&0x10).unwrap().wealth = 3;
    e.f.zones.borrow_mut().insert((5, 5), 12);
    assert_eq!(e.d.wealth_select(&[1, 2, 3, 4], 5, 5), 2);
    assert_eq!(e.wealth.draws(), ["U(2)"]);
    // No candidate: 0, no draw.
    assert_eq!(e.d.wealth_select(&[1, 3], 5, 5), 0);
    assert!(e.wealth.draws().is_empty());
    // Without a lot: the network wealth (1).
    assert_eq!(e.d.wealth_select(&[1, 2], 40, 40), 1);
}

fn night_prop(e: &Env, token: u64, x: f32) {
    let mut h = PropertyHolder::new();
    h.add(PROP_NIGHT_LIT, Variant::Scalar(Elem::U8(1)));
    e.f.add_prop(token, Some(0x10), None, 0x600, h);
    e.f.props.borrow_mut().get_mut(&token).unwrap().pos = [x, 0.0, 8.0];
    e.d.register_prop(token);
}

fn msg(t: u32) -> PropMessage {
    PropMessage {
        msg_type: t,
        ..Default::default()
    }
}

// §12 test 10.
#[test]
fn t10_night_sweep() {
    let e = env();
    city(&e);
    for i in 0..20 {
        night_prop(&e, 0x200 + i, 16.0 * i as f32 + 8.0);
    }
    e.f.unpowered.borrow_mut().insert((0, 0));
    assert_eq!(e.d.night_list().len(), 20);
    e.f.take();
    e.d.do_message(&msg(MSG_DUSK));
    let mut crt = CrtRand::default();
    for _ in 0..19 {
        crt.rand();
    }
    assert_eq!(e.d.crt(), crt);
    let s = e.d.snapshot();
    assert_eq!(
        (s.night_mode, s.night_batch, s.night_cursor, s.is_night),
        (1, 3, 0, true)
    );
    let tick = |us: u32| {
        e.d.do_message(&PropMessage {
            msg_type: MSG_ANIMATION_TICK,
            data2: us,
            ..Default::default()
        })
    };
    assert!(tick(150_000));
    assert!(e.f.take().is_empty());
    assert!(tick(50_000));
    assert!(e.f.take().is_empty());
    assert!(tick(1_000));
    assert_eq!(e.f.take().len(), 3);
    for _ in 0..6 {
        tick(201_000);
    }
    let s = e.d.snapshot();
    assert_eq!((s.night_mode, s.night_cursor), (0, 20));
    // The unpowered prop (cell (0, 0)) is off, the others on.
    assert_eq!(e.f.prop(0x200).state, 0);
    assert!((0x201..0x214).all(|p| e.f.prop(p).state == 1));
    // Mode 0: the tick answers false.
    assert!(!tick(500_000));
    // Dawn with an empty list keeps the mode [quirk].
    let e2 = env();
    city(&e2);
    e2.d.do_message(&msg(MSG_DAWN));
    e2.d.do_message(&PropMessage {
        msg_type: MSG_ANIMATION_TICK,
        data2: 300_000,
        ..Default::default()
    });
    assert_eq!(e2.d.snapshot().night_mode, 2);
}

#[test]
fn switch_all_and_day_night() {
    let e = env();
    city(&e);
    night_prop(&e, 0x200, 8.0);
    night_prop(&e, 0x201, 24.0);
    e.f.unpowered.borrow_mut().insert((1, 0));
    e.f.night.set(true);
    e.d.do_message(&msg(MSG_POST_CITY_INIT));
    assert!(e.d.snapshot().is_night);
    assert_eq!((e.f.prop(0x200).state, e.f.prop(0x201).state), (1, 0));
    // DayNightChanged in visual mode 1 while switching on: all off.
    e.d.do_message(&msg(MSG_DUSK));
    e.f.clock.set(Some((12, 0)));
    e.f.mode.set(1);
    e.d.do_message(&PropMessage {
        msg_type: MSG_DAY_NIGHT_CHANGED,
        data2: 1,
        ..Default::default()
    });
    assert_eq!(e.f.prop(0x200).state, 0);
    assert_eq!(e.d.snapshot().night_mode, 0);
}

fn tod_prop(e: &Env, token: u64, start: f32, end: f32) {
    let mut h = PropertyHolder::new();
    h.add(
        PROP_TIME_OF_DAY,
        Variant::array(mem::FLOAT32, vec![Elem::F32(start), Elem::F32(end)]),
    );
    e.f.add_prop(token, Some(0x10), None, 0x600, h);
    e.d.register_prop(token);
}

// §12 test 11.
#[test]
fn t11_time_of_day() {
    let check = |start: f32, end: f32, hour: u32, minute: u32, on: bool| {
        let e = env();
        city(&e);
        tod_prop(&e, 0x300, start, end);
        e.f.take();
        e.f.clock.set(Some((hour, minute)));
        e.d.do_message(&msg(MSG_TIME_TICK));
        assert_eq!(
            e.f.take(),
            [format!("timer 0x300 1 {on}")],
            "{start}-{end} at {hour}:{minute}"
        );
    };
    check(18.0, 8.0, 7, 30, true);
    check(18.0, 8.0, 12, 0, false);
    check(18.0, 8.0, 18, 0, true);
    check(8.0, 18.0, 8, 0, true);
    check(8.0, 18.0, 18, 0, true);
    check(8.0, 18.0, 18, 6, false);
    let e = env();
    city(&e);
    tod_prop(&e, 0x300, 18.0, 8.0);
    e.f.take();
    e.f.clock.set(Some((7, 0)));
    e.d.do_message(&msg(MSG_TIME_TICK));
    assert_eq!(e.f.take().len(), 1);
    e.f.clock.set(Some((7, 20)));
    e.d.do_message(&msg(MSG_TIME_TICK));
    assert!(e.f.take().is_empty());
    e.f.clock.set(Some((7, 30)));
    e.d.do_message(&msg(MSG_TIME_TICK));
    assert_eq!(e.f.take().len(), 1);
}

// §12 test 12.
#[test]
fn t12_registration() {
    let e = env();
    city(&e);
    let mut h = PropertyHolder::new();
    h.add(PROP_DATE, Variant::Scalar(Elem::U32(0x1234)));
    e.f.add_prop(0x400, Some(0x10), None, 0x600, h);
    e.d.register_prop(0x400);
    assert_eq!(
        e.f.take(),
        [
            "state 0x400 1",
            "chance 0x400 100",
            "timer 0x400 4 true",
            "timer 0x400 1 true",
            "date_range 0x400 4660 1 0",
            "timer 0x400 8 true"
        ]
    );
    assert_eq!(e.d.dated_list(), [PropRef::new(0x400, 0x2977_AA47)]);
    assert!(e.d.night_list().is_empty() && e.d.time_of_day_list().is_empty() && e.d.power_list().is_empty());
    // Day ticks the dated props.
    e.d.do_message(&msg(MSG_DAY));
    assert!(e.f.take().contains(&"tick_date 0x400 777".to_string()));
    // A trigger prop, a power prop, a preview prop.
    let mut h = PropertyHolder::new();
    h.add(PROP_TRIGGER, Variant::Scalar(Elem::U32(0xCACB_DFC7)));
    h.add(PROP_NEEDS_POWER, Variant::Scalar(Elem::Bool(true)));
    h.add(PROP_CHANCE, Variant::Scalar(Elem::U8(40)));
    e.f.add_prop(0x401, Some(0x10), None, 0x600, h);
    e.d.register_prop(0x401);
    assert_eq!(
        e.f.take(),
        [
            "state 0x401 1",
            "chance 0x401 40",
            "timer 0x401 4 true",
            "timer 0x401 1 true",
            "timer 0x401 2 true",
            "timer 0x401 8 true"
        ]
    );
    assert_eq!(e.d.trigger_map().len(), 1);
    assert_eq!(e.d.power_list().len(), 1);
    e.d.do_message(&PropMessage {
        msg_type: MSG_EFFECT_TRIGGER,
        data1: 0xCACB_DFC7,
        data2: 0,
        ..Default::default()
    });
    assert_eq!(e.f.take(), ["timer 0x401 4 false"]);
    e.f.unpowered.borrow_mut().insert((0, 0));
    e.d.do_message(&PropMessage {
        msg_type: MSG_POWER_CHANGED,
        data1: 0,
        data2: 0,
        data3: 3,
        data4: 3,
        ..Default::default()
    });
    assert_eq!(e.f.take(), ["timer 0x401 8 false"]);
    // Unregistering empties the lists and the trigger entry.
    e.d.do_message(&PropMessage {
        msg_type: MSG_OCCUPANT_REMOVED,
        object: 0x401,
        data3: OCCUPANT_TYPE_PROP,
        ..Default::default()
    });
    assert!(e.d.trigger_map().is_empty() && e.d.power_list().is_empty());
    let mut h = PropertyHolder::new();
    h.add(PROP_PREVIEW, Variant::Scalar(Elem::U8(1)));
    h.add(PROP_DATE, Variant::Scalar(Elem::U32(1)));
    e.f.add_prop(0x402, Some(0x10), None, 0x600, h);
    e.d.do_message(&PropMessage {
        msg_type: MSG_OCCUPANT_INSERTED,
        object: 0x402,
        data3: OCCUPANT_TYPE_PROP,
        ..Default::default()
    });
    assert!(e.f.take().is_empty());
}

// §12 test 13.
#[test]
fn t13_signs() {
    let e = env();
    city(&e);
    e.f.add_prop(0x500, None, None, PROP_TYPE_SIGNPOST, PropertyHolder::new());
    e.f.add_prop(0x501, None, None, PROP_TYPE_STREET_SIGN, PropertyHolder::new());
    e.d.register_prop(0x500);
    e.d.register_prop(0x501);
    assert_eq!(e.d.sign_list().len(), 2);
    e.d.show_signs(false);
    e.d.show_signs(false);
    assert_eq!(e.f.take(), ["flag 0x500 2 false", "flag 0x501 2 false"]);
    assert!(!e.d.signs_visible());
    e.d.toggle_signs();
    assert_eq!(e.f.take(), ["flag 0x500 2 true", "flag 0x501 2 true"]);
    assert!(e.d.signs_visible());
    // Terrain change: the signpost moves to max(10, 12).
    e.d.do_message(&msg(MSG_TERRAIN_CHANGED));
    assert_eq!(e.f.take(), ["position 0x500 12"]);
    // A removed network occupant removes the street-name sign on its cell.
    e.d.do_message(&PropMessage {
        msg_type: MSG_OCCUPANT_REMOVED,
        object: 0x77,
        data3: OCCUPANT_TYPE_NETWORK,
        ..Default::default()
    });
    assert_eq!(e.f.take(), ["remove 0x501"]);
}

// §12 test 14.
#[test]
fn t14_lot_state() {
    let e = env();
    city(&e);
    e.f.add_lot(0x10, 0, 0, 1.0, vec![opt_prop(0xA0, &[])]);
    let mut h = PropertyHolder::new();
    h.add(PROP_LOT_STATE, Variant::Scalar(Elem::U8(3)));
    e.f.add_prop(0x600, Some(0x10), None, 0x600, h);
    e.d.do_message(&PropMessage {
        msg_type: MSG_LOT_STATE_CHANGED,
        object: 0x10,
        data1: 2,
        data2: 3,
        ..Default::default()
    });
    assert_eq!(e.f.take(), ["post 0 0 true"]);
    e.d.do_message(&PropMessage {
        msg_type: MSG_LOT_STATE_PROPS,
        data1: 0,
        data2: 0,
        ..Default::default()
    });
    assert_eq!(e.f.prop(0x600).state, 0);
    e.f.lots.borrow_mut().get_mut(&0x10).unwrap().state = 4;
    e.d.do_message(&PropMessage {
        msg_type: MSG_LOT_STATE_PROPS,
        ..Default::default()
    });
    assert_eq!(e.f.prop(0x600).state, 1);
    // A lot that is not livable loses its optional props.
    e.f.add_prop(0x601, Some(0x10), Some(0xA0), 0x600, PropertyHolder::new());
    e.f.lots.borrow_mut().get_mut(&0x10).unwrap().livable = false;
    e.f.take();
    e.d.do_message(&PropMessage {
        msg_type: MSG_LOT_STATE_CHANGED,
        object: 0x10,
        data1: 3,
        data2: 4,
        ..Default::default()
    });
    assert_eq!(e.f.take(), ["state 0x600 1", "remove 0x601", "post 0 0 false"][1..]);
}

#[test]
fn cell_walk_quirks() {
    let e = env();
    city(&e);
    example_lot(&e, 1.0);
    // A tagged prop naming no object lowers the slot count [quirk]; a prop
    // with requester index 7 (≥ R) closes its slot but is not counted.
    e.f.add_prop(0x700, Some(0x10), Some(0xEE), 0x500, PropertyHolder::new());
    let mut h = PropertyHolder::new();
    h.add(PROP_REQUESTER_INDEX, Variant::Scalar(Elem::U32(7)));
    e.f.add_prop(0x701, Some(0x10), Some(0xA0), 0x500, h);
    // A pollution prop (no custom selector) is kept without a draw.
    let mut h = PropertyHolder::new();
    h.add(PROP_REQUESTER_INDEX, Variant::Scalar(Elem::U32(2)));
    e.f.add_prop(0x702, Some(0x10), Some(0xA1), 0x500, h);
    e.f.lots.borrow_mut().get_mut(&0x10).unwrap().objs =
        Some(Rc::new((0..4).map(|i| opt_prop(0xA0 + i, &[])).collect()));
    e.d.update_block(0, 0);
    let o = e.d.last_block();
    // opt 2 (A2, A3), k = −1 + 1 = 0 → slots 2, keptAll 0; the pollution
    // prop is over its want of 0, so it is removed and slots becomes 3.
    assert_eq!((o.slots, o.kept_all, o.kept.clone()), (3, 0, vec![0, 0, 1]));
    assert_eq!((o.target, o.removed), (2, 1));
    assert_eq!(e.member.draws()[0], "RS(0, 1)");
    assert!(e.wealth.draws().is_empty());
    // A crime prop is checked with the crime selector.
    let e = env();
    city(&e);
    example_lot(&e, 1.0);
    let mut h = PropertyHolder::new();
    h.add(PROP_REQUESTER_INDEX, Variant::Scalar(Elem::U32(1)));
    e.f.add_prop(0x703, Some(0x10), Some(0xA2), 0x555, h);
    e.d.update_block(0, 0);
    assert!(e.f.take().contains(&format!("select {CRIME:#x} [555]")));
}

#[test]
fn update_all_is_x_outer() {
    let e = env();
    e.f.cells.set((16, 24));
    city(&e);
    e.d.update_all_optional_props();
    assert_eq!(e.d.last_block().block, (1, 2));
}

#[test]
fn requesters_add_and_remove() {
    let e = env();
    city(&e);
    assert_eq!(e.d.requesters().len(), 3);
    e.d.add_prop_requester(Requester::Host(PropRef::new(CRIME, 0x8990_C09A)));
    assert_eq!(e.d.requesters().len(), 4);
    assert!(e.d.remove_prop_requester(CRIME));
    assert_eq!(e.d.requesters()[1].token(), POLLUTION);
    assert_eq!(e.d.requesters()[2].token(), CRIME);
    assert!(!e.d.remove_prop_requester(0x9999));
}

// PV8: a null created occupant returns false; the free entry is used up and
// `need` is unchanged, so every free slot is tried once.
#[test]
fn null_creation_returns_false() {
    let e = env();
    city(&e);
    example_lot(&e, 0.5);
    e.f.fail_create.set(true);
    e.member.push(&[0, 0, 0, 0]);
    e.d.update_block(0, 0);
    let o = e.d.last_block();
    assert_eq!((o.target, o.created), (2, 0));
    assert_eq!(e.member.draws(), ["RS(0, 4)", "RS(0, 3)", "RS(0, 2)", "RS(0, 1)"]);
    assert!(e.f.take().iter().all(|l| l.ends_with("-> null")));
    assert!(e.f.on_lot(0x10).is_empty());
}

// PV12: each list is searched only when the prop passes that list's test.
#[test]
fn unregister_tests_each_list() {
    let e = env();
    city(&e);
    let removed = |token: u64| PropMessage {
        msg_type: MSG_OCCUPANT_REMOVED,
        object: token,
        data3: OCCUPANT_TYPE_PROP,
        ..Default::default()
    };
    // A trigger prop joins while the 3D view exists.
    let mut h = PropertyHolder::new();
    h.add(PROP_TRIGGER, Variant::Scalar(Elem::U32(0xCACB_DFC7)));
    e.f.add_prop(0x500, Some(0x10), None, 0x600, h);
    e.d.register_prop(0x500);
    // A dated prop.
    let mut h = PropertyHolder::new();
    h.add(PROP_DATE, Variant::Scalar(Elem::U32(5)));
    e.f.add_prop(0x501, Some(0x10), None, 0x600, h);
    e.d.register_prop(0x501);
    e.f.take();
    // [quirk] The view has gone: the trigger-map prop stays, held.
    e.f.view.set(false);
    e.d.do_message(&removed(0x500));
    assert_eq!(e.d.trigger_map().len(), 1);
    e.f.view.set(true);
    e.d.do_message(&removed(0x500));
    assert!(e.d.trigger_map().is_empty());
    // The date property gone by the removal: the dated list is not searched.
    e.f.props.borrow_mut().get_mut(&0x501).unwrap().props = PropertyHolder::new();
    e.d.do_message(&removed(0x501));
    assert_eq!(e.d.dated_list().len(), 1);
    // A preview prop is not looked up at all.
    let mut h = PropertyHolder::new();
    h.add(PROP_PREVIEW, Variant::Scalar(Elem::U8(1)));
    h.add(PROP_DATE, Variant::Scalar(Elem::U32(5)));
    e.f.props.borrow_mut().get_mut(&0x501).unwrap().props = h;
    e.d.do_message(&removed(0x501));
    assert_eq!(e.d.dated_list().len(), 1);
}

// PV13: a scanner state byte above 2 is running and is written back as read
// until the scan finishes or resets.
#[test]
fn scanner_state_byte_above_two() {
    let e = env();
    let mut t = hex(TUTORIAL);
    t[3] = 5;
    assert!(read(&e.d, &t));
    let s = e.d.snapshot();
    assert_eq!((s.scanner.state, s.scan_byte, s.scanner.x, s.scanner.z), (ScanState::Running, 5, 7, 14));
    assert_eq!(record(&e.d), t);
    assert_eq!(e.d.block_step(), Some((8, 14)));
    assert_eq!(e.d.snapshot().scan_byte, 5);
    assert_eq!(record(&e.d)[3], 5);
    // From the clip's last block the scan finishes and restarts: byte 1.
    let e = env();
    t[3] = 0xFF;
    t[4] = 15;
    t[8] = 15;
    assert!(read(&e.d, &t));
    assert_eq!(e.d.snapshot().scan_byte, 0xFF);
    assert_eq!(e.d.block_step(), Some((0, 0)));
    assert_eq!(e.d.snapshot().scan_byte, 1);
    assert_eq!(record(&e.d)[3], 1);
    // 0 and 2 are fresh and done.
    for (b, st) in [(0u8, ScanState::Fresh), (2, ScanState::Done)] {
        let e = env();
        t[3] = b;
        assert!(read(&e.d, &t));
        assert_eq!(e.d.snapshot().scanner.state, st);
        assert_eq!(record(&e.d)[3], b);
    }
}
