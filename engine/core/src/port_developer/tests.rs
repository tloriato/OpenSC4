// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The conformance tests of `docs/specs/port-developers.md` §12 and the
//! behaviours around them.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::*;
use crate::stream::{ByteReader, ByteWriter};
use crate::variant::{Elem, Variant};

#[derive(Clone, Debug)]
struct Lot {
    config: Option<(u32, u32)>,
    x: i16,
    z: i16,
    state: u32,
    condition: u32,
    days: u32,
    rect: CellRect,
    facing: u32,
    compatible: bool,
    subnetworks: Vec<u32>,
    arrived: u32,
}

impl Lot {
    fn new(cfg: u32, stage: u32, x: i16, z: i16) -> Lot {
        Lot {
            config: Some((cfg, stage)),
            x,
            z,
            state: 3,
            condition: 0,
            days: 0,
            rect: CellRect {
                x0: i32::from(x),
                z0: i32::from(z),
                x1: i32::from(x) + 4,
                z1: i32::from(z) + 2,
            },
            facing: 0,
            compatible: true,
            subnetworks: vec![0],
            arrived: 0,
        }
    }
}

#[derive(Default)]
struct Fake {
    exemplars: BTreeMap<u32, Rc<PropertyHolder>>,
    lots: RefCell<BTreeMap<u64, Lot>>,
    occupants: RefCell<BTreeMap<u64, (bool, u32, u32, u64)>>,
    department: Cell<Option<u64>>,
    line: Cell<Option<(i64, i64)>>,
    /// Current expenses follow the full expenses (fully funded).
    follow: Cell<bool>,
    a: RefCell<Vec<u32>>,
    b: RefCell<Vec<u32>>,
    size: Cell<(u32, u32)>,
    water: RefCell<BTreeSet<(u32, u32)>>,
    next_lot: Cell<u64>,
    create_fails: Cell<bool>,
    /// Lot configuration IDs the manager does not have (PD5).
    missing_configs: RefCell<BTreeSet<u32>>,
    /// What `set_access_props_state` answers (props switched).
    access_props: Cell<u32>,
    log: RefCell<Vec<String>>,
}

impl Fake {
    fn new() -> Rc<Fake> {
        let f = Fake {
            follow: Cell::new(true),
            next_lot: Cell::new(0x9000),
            access_props: Cell::new(1),
            ..Default::default()
        };
        let mut f = f;
        for airport in [true, false] {
            for (k, h) in tuning::shipped_exemplars(airport) {
                f.exemplars.insert(k, Rc::new(h));
            }
        }
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
    fn edit(&self, l: u64, f: impl FnOnce(&mut Lot)) {
        f(self.lots.borrow_mut().get_mut(&l).unwrap());
    }
}

impl PortHost for Fake {
    fn exemplar(&self, instance: u32) -> Option<Rc<PropertyHolder>> {
        self.exemplars.get(&instance).cloned()
    }
    fn add_agent(&self, name: &str, level: u32, flags: u32) {
        self.log(format!("add_agent {name} {level} {flags}"));
    }
    fn remove_agent(&self, name: &str) {
        self.log(format!("remove_agent {name}"));
    }
    fn department(&self, _id: u32) -> Option<u64> {
        self.department.get()
    }
    fn create_department(&self, id: u32, group: u32, name: (u32, u32), max: f32, line: u32) -> Option<u64> {
        self.log(format!(
            "create_department {id:#x} {group:#x} {:#x} {:#x} {max} {line}",
            name.0, name.1
        ));
        self.department.set(Some(0xD));
        self.line.set(Some((0, 0)));
        Some(0xD)
    }
    fn line_expenses(&self, _d: u64, _line: u32) -> Option<(i64, i64)> {
        self.line.get()
    }
    fn set_full_expenses(&self, _d: u64, _line: u32, v: i64) {
        let cur = if self.follow.get() {
            v
        } else {
            self.line.get().map_or(0, |l| l.1)
        };
        self.line.set(Some((v, cur)));
        self.log(format!("full {v}"));
    }
    fn withdraw_funds(&self, amount: i64) -> bool {
        self.log(format!("withdraw {amount}"));
        false
    }
    fn occupant_is_preview(&self, o: u64) -> bool {
        self.occupants.borrow()[&o].0
    }
    fn occupant_type(&self, o: u64) -> u32 {
        self.occupants.borrow()[&o].1
    }
    fn occupant_in_group(&self, o: u64, group: u32) -> bool {
        self.occupants.borrow()[&o].2 == group
    }
    fn occupant_lot(&self, o: u64) -> Option<u64> {
        Some(self.occupants.borrow()[&o].3)
    }
    fn lot_config(&self, lot: u64) -> Option<(u32, u32)> {
        self.lot(lot).config
    }
    fn lot_location(&self, lot: u64) -> (i16, i16) {
        let l = self.lot(lot);
        (l.x, l.z)
    }
    fn lot_at(&self, x: i16, z: i16) -> Option<u64> {
        self.lots
            .borrow()
            .iter()
            .find(|(_, l)| l.x == x && l.z == z)
            .map(|(&k, _)| k)
    }
    fn lot_state(&self, lot: u64) -> u32 {
        self.lot(lot).state
    }
    fn lot_condition(&self, lot: u64) -> u32 {
        self.lot(lot).condition
    }
    fn set_lot_condition(&self, lot: u64, c: u32) {
        self.edit(lot, |l| l.condition = c);
        self.log(format!("condition {lot:#x} {c}"));
    }
    fn lot_days_since_state_change(&self, lot: u64) -> u32 {
        self.lot(lot).days
    }
    fn lot_rect(&self, lot: u64) -> CellRect {
        self.lot(lot).rect
    }
    fn lot_facing(&self, lot: u64) -> u32 {
        self.lot(lot).facing
    }
    fn lot_zone_compatible(&self, lot: u64, _zone: u16) -> bool {
        self.lot(lot).compatible
    }
    fn lot_configuration(&self, id: u32) -> Option<u64> {
        (!self.missing_configs.borrow().contains(&id)).then_some(u64::from(id))
    }
    fn start_demolish_lot(&self, lot: u64) {
        self.log(format!("start_demolish {lot:#x}"));
    }
    fn end_demolish_lot(&self, lot: u64) {
        self.log(format!("end_demolish {lot:#x}"));
    }
    fn create_lot(&self, x: i32, z: i32, w: i32, d: i32, facing: u32) -> Option<u64> {
        self.log(format!("create_lot {x} {z} {w} {d} {facing}"));
        if self.create_fails.get() {
            return None;
        }
        let t = self.next_lot.get();
        self.next_lot.set(t + 1);
        let mut l = Lot::new(0, 0, x as i16, z as i16);
        l.config = None;
        l.state = 2;
        l.rect = CellRect {
            x0: x,
            z0: z,
            x1: x + w - 1,
            z1: z + d - 1,
        };
        self.lots.borrow_mut().insert(t, l);
        Some(t)
    }
    fn set_lot_configuration(&self, lot: u64, config: Option<u64>) {
        self.edit(lot, |l| l.config = config.map(|c| (c as u32, 2)));
        self.log(format!("set_config {lot:#x} {:#x}", config.unwrap_or(0)));
    }
    fn construct_default_lot(&self, lot: u64, config: Option<u64>) -> bool {
        self.log(format!("construct {lot:#x} {:#x}", config.unwrap_or(0)));
        if config.is_none() {
            // The lot developer with a null configuration (lot-developer.md
            // §6.4): state 2, condition 1, true.
            self.edit(lot, |l| {
                l.state = 2;
                l.condition = CONDITION_UNPOWERED;
            });
        }
        true
    }
    fn dezone(&self, r: CellRect) {
        self.log(format!("dezone {} {} {} {}", r.x0, r.z0, r.x1, r.z1));
    }
    fn update_occupant_effects(&self, lot: u64, old: f32, new: f32) {
        self.log(format!("effects {lot:#x} {old} {new}"));
    }
    fn record_history(&self, key: u32, value: i32) {
        self.log(format!("history {key:#x} {value}"));
    }
    fn push_event(&self, event: u32) {
        self.log(format!("event {event:#x}"));
    }
    fn office_counts(&self, t: u32) -> Vec<u32> {
        if t == OFFICE_MEDIUM {
            self.a.borrow().clone()
        } else {
            self.b.borrow().clone()
        }
    }
    fn lot_subnetworks(&self, lot: u64) -> Vec<u32> {
        self.lot(lot).subnetworks
    }
    fn traffic_arrived(&self, lot: u64, _t: u32) -> u32 {
        self.lot(lot).arrived
    }
    fn set_max_trip_capacity(&self, lot: u64, t: u32, cap: u32) {
        self.log(format!("trip_capacity {lot:#x} {t} {cap}"));
    }
    fn terrain_size(&self) -> (u32, u32) {
        self.size.get()
    }
    fn cell_has_water(&self, x: u32, z: u32) -> bool {
        self.water.borrow().contains(&(x, z))
    }
    fn set_access_props_state(&self, lot: u64, prop: u32, state: u32) -> u32 {
        self.log(format!("props_state {lot:#x} {prop:#x} {state}"));
        self.access_props.get()
    }
    fn set_altitude(&self, x: i32, z: i32, a: f32) {
        self.log(format!("altitude {x} {z} {a}"));
    }
}

const SMALL0: u32 = 0xCA24_36C0;
const MEDIUM0: u32 = 0x6A51_26D0;

fn airport() -> (Rc<Fake>, Rc<PortDeveloper>) {
    let f = Fake::new();
    let d = PortDeveloper::new(PortKind::Airport, f.clone(), 1);
    d.init();
    (f, d)
}

fn add_lot(f: &Fake, token: u64, cfg: u32, x: i16, z: i16) {
    f.lots.borrow_mut().insert(token, Lot::new(cfg, 1, x, z));
}

fn record(d: &PortDeveloper) -> Vec<u8> {
    let mut out = ByteWriter::new();
    assert!(d.write(&mut out));
    out.into_bytes()
}

fn read(d: &PortDeveloper, bytes: &[u8]) -> bool {
    let mut inp = ByteReader::new(bytes);
    d.read(&mut inp)
}

#[test]
fn identity() {
    use PortKind::*;
    assert_eq!(query_interface(Airport, IID_AIRPORT_DEVELOPER), Some(0));
    assert_eq!(query_interface(Airport, IID_SEAPORT_DEVELOPER), None);
    assert_eq!(query_interface(Seaport, IID_SEAPORT_DEVELOPER), Some(0));
    assert_eq!(query_interface(Seaport, IID_UNKNOWN), Some(4));
    assert_eq!(query_interface(Airport, IID_PORT_DEVELOPER), Some(4));
    assert_eq!(query_interface(Airport, IID_MESSAGE_TARGET), Some(8));
    assert_eq!(query_interface(Seaport, IID_SERIALIZABLE), Some(0xC));
    assert_eq!(Airport.component(), 13);
    assert_eq!(Seaport.simulator_type(), 0xEA39_57AD);
    assert!(Airport.is_growth_message(0x0B3B_E81A));
    assert!(!Airport.is_growth_message(SEAPORT_GROWTH));
    assert!(Seaport.is_growth_message(SEAPORT_GROWTH));
}

#[test]
fn tuning_loads_shipped_types() {
    let (_f, d) = airport();
    let t = d.types();
    assert_eq!(t.len(), 3);
    assert_eq!(d.snapshot().type_count, 3);
    assert_eq!(t[0].rules[1].lot_config, 0xCA24_36C1);
    assert_eq!(t[1].rules[2].capacity, 164_200);
    assert_eq!(t[2].rules[2].upgrade_cost, 81_300);
    assert_eq!(t[0].threshold, 0.25);
    assert_eq!(t[2].size_class, 3);
    assert!(t[0].grid.is_none());
    let f = Fake::new();
    let s = PortDeveloper::new(PortKind::Seaport, f, 2);
    s.init();
    let t = s.types();
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].rules[0].capacity, 650);
    // 72 values for an 11 × 5 grid: rejected.
    assert!(t[0].grid.is_none());
}

#[test]
fn init_creates_the_department_and_agent() {
    let (f, d) = airport();
    assert_eq!(
        f.take(),
        [
            "add_agent cSC4PortDeveloper 6 2",
            "create_department 0x6a4119f0 0xaa369059 0x6a3ff01c 0xea493564 1.2 1"
        ]
    );
    assert_eq!(d.snapshot().department, Some(0xD));
    d.shutdown();
    assert_eq!(f.take(), ["remove_agent cSC4PortDeveloper"]);
    // A department that exists is reused.
    d.init();
    assert_eq!(f.take(), ["add_agent cSC4PortDeveloper 6 2"]);
}

// §12 test 13 (PD10): Init is guarded, Shutdown deletes the ports and the
// types and keeps `tuningLoaded`.
#[test]
fn t13_init_guard_and_shutdown() {
    let (f, d) = airport();
    f.take();
    assert!(d.init());
    assert!(f.take().is_empty());
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    assert!(read(&d, &EXAMPLE));
    assert_eq!(d.ports().len(), 2);
    f.take();
    assert!(d.shutdown());
    // The read port is resolved before it is deleted (no lot at (40, 52)
    // here: nothing to log), then the agent goes.
    assert_eq!(f.take(), ["remove_agent cSC4PortDeveloper"]);
    assert!(d.ports().is_empty());
    assert!(d.types().is_empty());
    let s = d.snapshot();
    assert!(s.tuning_loaded && !s.initialized);
    assert_eq!(s.type_count, 3);
    assert_eq!(s.department, None);
    // A second Shutdown does nothing.
    assert!(d.shutdown());
    assert!(f.take().is_empty());
    // [quirk] A re-Init does not reload the tuning: no port types, so a
    // record with a port is rejected and nothing is adopted.
    assert!(d.init());
    assert!(d.types().is_empty());
    assert!(!read(&d, &EXAMPLE));
    add_lot(&f, 0x11, SMALL0, 9, 9);
    f.occupants
        .borrow_mut()
        .insert(0x101, (false, OCCUPANT_TYPE_BUILDING, 0x1508, 0x11));
    d.occupant_inserted(0x101);
    assert!(d.ports().is_empty());
    // Shutdown without Init does nothing.
    let f2 = Fake::new();
    let d2 = PortDeveloper::new(PortKind::Seaport, f2.clone(), 9);
    assert!(d2.shutdown());
    assert!(f2.take().is_empty());
}

// §12 test 1.
#[test]
fn t1_empty_records() {
    let (_f, d) = airport();
    assert_eq!(record(&d), [0x02, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0, 0]);
    let s = PortDeveloper::new(PortKind::Seaport, Fake::new(), 2);
    assert_eq!(record(&s), [0x02, 0, 0x01, 0, 0x92, 0xAD, 0x29, 0xEA, 0x0C, 0, 0, 0]);
    let fresh = PortDeveloper::new(PortKind::Airport, Fake::new(), 3);
    assert!(!read(
        &fresh,
        &[0x01, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0, 0]
    ));
    assert!(!read(
        &fresh,
        &[0x03, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0, 0]
    ));
    assert!(!read(
        &fresh,
        &[0x02, 0, 0x02, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0, 0]
    ));
    assert!(read(&fresh, &[0x02, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0, 0]));
}

const EXAMPLE: [u8; 22] = [
    0x02, 0, 0x01, 0, 0x9D, 0x0E, 0x27, 0xCA, 0x0B, 0, 0x01, 0, 0x01, 0, 0, 0, 0, 0, 0x28, 0, 0x34, 0,
];

// §12 test 2.
#[test]
fn t2_round_trip() {
    let f = Fake::new();
    let d = PortDeveloper::new(PortKind::Airport, f.clone(), 1);
    assert!(read(&d, &EXAMPLE));
    assert!(d.snapshot().tuning_loaded);
    let p = d.ports()[0];
    assert_eq!((p.stage, p.type_index, p.x, p.z), (1, 0, 40, 52));
    assert!(p.needs_lookup);
    assert_eq!(p.efficiency, 1.0);
    assert_eq!(record(&d), EXAMPLE);
    assert_eq!(d.num_airports(0), 1);
    assert_eq!(d.num_ports(0x1F, 2), 1);
    assert_eq!(d.num_ports(2, 0x1F), 1);
    assert_eq!(d.num_ports(1, 0x1F), 0);
    assert_eq!(d.num_airports(1), 1);
    assert_eq!(d.num_airports(2), 0);
    // PostCityInit resolves the lot by location.
    add_lot(&f, 0x40, SMALL0, 40, 52);
    d.post_city_init();
    assert_eq!(d.ports()[0].lot, Some(0x40));
    // A type index out of range rejects the record [choice]; Read appends.
    let mut bad = EXAMPLE;
    bad[16] = 3;
    assert!(!read(&d, &bad));
    assert_eq!(d.ports().len(), 1);
    assert!(read(&d, &EXAMPLE));
    assert_eq!(d.ports().len(), 2);
}

fn adopt(f: &Fake, d: &PortDeveloper, occ: u64, lot: u64, cfg: u32, x: i16, z: i16) {
    add_lot(f, lot, cfg, x, z);
    f.occupants
        .borrow_mut()
        .insert(occ, (false, OCCUPANT_TYPE_BUILDING, 0x1508, lot));
    d.occupant_inserted(occ);
}

// §12 test 3.
#[test]
fn t3_adoption() {
    let (f, d) = airport();
    f.take();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    assert_eq!(d.ports().len(), 1);
    assert_eq!(d.ports()[0].stage, 0);
    assert_eq!(f.take(), ["full 700"]);
    // A phase-2 lot adds nothing.
    f.lots.borrow_mut().insert(0x11, Lot::new(SMALL0, 2, 9, 9));
    f.occupants
        .borrow_mut()
        .insert(0x101, (false, OCCUPANT_TYPE_BUILDING, 0x1508, 0x11));
    d.occupant_inserted(0x101);
    // A preview occupant adds nothing.
    add_lot(&f, 0x12, SMALL0, 20, 20);
    f.occupants
        .borrow_mut()
        .insert(0x102, (true, OCCUPANT_TYPE_BUILDING, 0x1508, 0x12));
    d.occupant_inserted(0x102);
    // Another group, or an unknown configuration, adds nothing.
    f.occupants
        .borrow_mut()
        .insert(0x103, (false, OCCUPANT_TYPE_BUILDING, 0x1509, 0x12));
    d.occupant_inserted(0x103);
    add_lot(&f, 0x13, 0x1234, 30, 30);
    f.occupants
        .borrow_mut()
        .insert(0x104, (false, OCCUPANT_TYPE_BUILDING, 0x1508, 0x13));
    d.occupant_inserted(0x104);
    assert_eq!(d.ports().len(), 1);
    assert!(f.take().is_empty());
    // A lot adopted twice is listed twice [quirk].
    d.occupant_inserted(0x100);
    assert_eq!(d.ports().len(), 2);
    assert_eq!(f.take(), ["full 1400"]);
}

// §12 test 4.
#[test]
fn t4_month_one_airport() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    *f.a.borrow_mut() = vec![4000];
    *f.b.borrow_mut() = vec![1200];
    f.take();
    d.month_tick();
    let p = d.ports()[0];
    assert_eq!(p.trips, 5200);
    assert_eq!(p.efficiency, 1.0 - 5200.0f32 / 10_000.0);
    assert_eq!(p.usage, 0.52);
    assert_eq!(
        f.take(),
        ["effects 0x10 0 0.52", "condition 0x10 0", "history 0x6a3e9032 48"]
    );
    assert_eq!(d.num_trips(), 5200);
    assert_eq!(d.usage_values(0x10), Some((5200, 10_000)));
    assert_eq!(d.usage_values(0x11), None);
    assert_eq!(d.maintenance_cost(0x10), 700);
}

// §12 test 5.
#[test]
fn t5_two_airports_share_the_jobs() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    adopt(&f, &d, 0x101, 0x11, MEDIUM0, 30, 4);
    *f.a.borrow_mut() = vec![31_800];
    *f.b.borrow_mut() = vec![0];
    d.month_tick();
    let p = d.ports();
    assert_eq!((p[0].trips, p[1].trips), (5000, 26_800));
    // A lot in two subnetworks is counted in both; a subnetwork past the
    // count lists is ignored.
    f.edit(0x10, |l| l.subnetworks = vec![0, 1, 5]);
    *f.a.borrow_mut() = vec![31_800, 100];
    *f.b.borrow_mut() = vec![0];
    d.month_tick();
    assert_eq!(d.ports()[0].trips, 5100);
}

#[test]
fn unfunded_trips_are_zero() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    f.follow.set(false);
    f.line.set(Some((700, 0)));
    *f.a.borrow_mut() = vec![4000];
    *f.b.borrow_mut() = vec![0];
    f.take();
    d.month_tick();
    // fr = 0: 0/0 is NaN, truncated to 0; no trips means efficiency 1.
    let p = d.ports()[0];
    assert_eq!(p.trips, 0);
    assert_eq!(p.efficiency, 1.0);
    // PD8: the month's usage uses the plain capacity: 0 / 10000.
    assert_eq!(p.usage, 0.0);
    // funded 0: stressed once older than 90 days.
    f.edit(0x10, |l| l.days = 91);
    d.month_tick();
    assert_eq!(f.lot(0x10).condition, 2);
}

// §12 tests 6 and 7.
#[test]
fn t6_t7_advice_and_growth() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    *f.a.borrow_mut() = vec![9700];
    *f.b.borrow_mut() = vec![0];
    f.edit(0x10, |l| l.days = 91);
    f.take();
    d.month_tick();
    assert_eq!(d.ports()[0].efficiency, 1.0 - 9700.0f32 / 10_000.0);
    let log = f.take();
    assert!(log.contains(&"condition 0x10 2".to_string()));
    assert_eq!(log.iter().filter(|l| l.starts_with("event")).count(), 1);
    assert!(log.contains(&"event 0xea394b74".to_string()));
    assert_eq!(d.last_month().event, 0xEA39_4B74);
    // Not at 90 days.
    f.edit(0x10, |l| l.days = 90);
    d.month_tick();
    assert!(f.take().contains(&"condition 0x10 0".to_string()));

    // §12 test 7: the answer.
    f.follow.set(false);
    f.line.set(Some((700, 700)));
    d.growth_message(0xEA39_4B74);
    let r = f.lot(0x10).rect;
    assert_eq!(
        f.take(),
        [
            "start_demolish 0x10".to_string(),
            "end_demolish 0x10".to_string(),
            format!("create_lot {} {} 5 3 0", r.x0, r.z0),
            "set_config 0x9000 0xca2436c1".to_string(),
            "construct 0x9000 0xca2436c1".to_string(),
            "withdraw 5700".to_string(),
            "full 800".to_string(),
        ]
    );
    let p = d.ports()[0];
    assert_eq!((p.stage, p.lot, p.swapping), (1, Some(0x9000), true));
    // The old lot's state-5 message only clears `swapping`.
    d.lot_state_changed(0x10, 3, LOT_STATE_DESTRUCTING);
    assert_eq!(d.ports().len(), 1);
    assert!(!d.ports()[0].swapping);
    assert!(f.take().is_empty());
}

#[test]
fn growth_needs_a_trigger_and_skips_construction() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    *f.a.borrow_mut() = vec![9599];
    *f.b.borrow_mut() = vec![0];
    d.month_tick();
    f.take();
    assert_eq!(d.grow_ports_to_next_stage(), 0);
    *f.a.borrow_mut() = vec![9600];
    d.month_tick();
    f.edit(0x10, |l| l.state = LOT_STATE_CONSTRUCTING);
    assert_eq!(d.grow_ports_to_next_stage(), 0);
    f.edit(0x10, |l| l.state = 1);
    // A failed rebuild loses the lot; the port stays at its stage.
    f.create_fails.set(true);
    assert_eq!(d.grow_ports_to_next_stage(), 0);
    let p = d.ports()[0];
    assert_eq!((p.stage, p.lot), (0, None));
    // The last stage cannot grow.
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    let mut p = d.ports()[0];
    p.stage = 2;
    p.trips = 40_000;
    d.set_port_fields(0, p);
    assert_eq!(d.grow_ports_to_next_stage(), 0);
}

// §12 test 8.
#[test]
fn t8_removal() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    adopt(&f, &d, 0x101, 0x11, MEDIUM0, 30, 4);
    let mut p = d.ports()[0];
    p.usage = 0.5;
    d.set_port_fields(0, p);
    f.take();
    // Not destructing, or not compatible: nothing.
    d.lot_state_changed(0x10, 3, 4);
    f.edit(0x10, |l| l.compatible = false);
    d.lot_state_changed(0x10, 3, 5);
    assert_eq!(d.ports().len(), 2);
    f.edit(0x10, |l| l.compatible = true);
    d.lot_state_changed(0x10, 3, 5);
    assert_eq!(d.ports().len(), 1);
    assert_eq!(f.take(), ["effects 0x10 0.5 0", "dezone 3 4 7 6", "full 1200"]);
}

// §12 test 9.
#[test]
fn t9_seaport_map() {
    let f = Fake::new();
    f.size.set((4, 3));
    for c in [(0, 0), (1, 0), (1, 1), (3, 2)] {
        f.water.borrow_mut().insert(c);
    }
    let d = PortDeveloper::new(PortKind::Seaport, f.clone(), 2);
    d.init();
    let m = d.water_map();
    assert_eq!(m.cells, [0x83, 0x83, 0, 0, 0, 0x83, 0, 0, 0, 0, 0, 0x8C]);
    assert!(d.city_has_sea());
    assert_eq!(d.cell_sea_access(3, 2), 0x8C);
    assert_eq!(d.cell_sea_access(9, 9), 0);
    // A seaport whose lot covers only (2, 1) has no access.
    let mut l = Lot::new(0x4A52_917D, 1, 2, 1);
    l.rect = CellRect {
        x0: 2,
        z0: 1,
        x1: 2,
        z1: 1,
    };
    f.lots.borrow_mut().insert(0x20, l);
    f.take();
    assert!(d.adopt(0x20));
    assert!(!d.port_has_access(0));
    assert_eq!(f.take(), ["full 500", "trip_capacity 0x20 3 650"]);
    assert!(d.snapshot().recheck_access);
    f.edit(0x20, |l| l.arrived = 300);
    d.month_tick();
    assert_eq!(
        f.take(),
        [
            "trip_capacity 0x20 3 650",
            "props_state 0x20 0xc98204b8 1",
            "event 0x4a490d33",
            "effects 0x20 0 0.46153846",
            "condition 0x20 0",
            "history 0xea3e904a 54",
        ]
    );
    assert_eq!(d.num_trips(), 300);
    // The recheck happens once.
    d.month_tick();
    assert!(!f.take().iter().any(|l| l.starts_with("props_state")));
    // A terrain change rebuilds the map and rechecks.
    f.water.borrow_mut().insert((2, 1));
    d.terrain_changed();
    assert!(d.port_has_access(0));
    d.month_tick();
    assert!(!f.take().iter().any(|l| l.starts_with("props_state")));
}

#[test]
fn seaport_advice_and_no_sea() {
    let f = Fake::new();
    f.size.set((3, 3));
    f.water.borrow_mut().insert((1, 1));
    let d = PortDeveloper::new(PortKind::Seaport, f.clone(), 2);
    d.init();
    assert!(!d.city_has_sea());
    assert_eq!(d.cell_sea_access(1, 1), 0x80);
    add_lot(&f, 0x20, 0x4A52_917D, 0, 0);
    assert!(d.adopt(0x20));
    f.edit(0x20, |l| l.arrived = 640);
    f.take();
    d.month_tick();
    assert!(f.take().contains(&"event 0x8a394dc4".to_string()));
}

#[test]
fn flood_fill_stack_limit_drops_pushes() {
    // A long comb: each tooth is one seed; the fill still reaches cells
    // through the scanline.
    let (m, sea) = WaterMap::build(5, 5, |_, _| true);
    assert!(sea);
    assert!(m.cells.iter().all(|&c| c == 0x8F));
}

// §12 test 10.
#[test]
fn t10_no_ports() {
    let (f, d) = airport();
    f.take();
    d.month_tick();
    assert!(f.take().is_empty());
    assert_eq!(d.avg_efficiency(), 1.0);
    assert_eq!(d.least_efficient_port(), None);
    assert_eq!(d.num_trips(), 0);
    assert_eq!(d.maintenance_cost(0x10), 0);
}

#[test]
fn least_efficient_and_skipped_ports() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    assert_eq!(d.least_efficient_port(), Some((1.0, 3, 4)));
    adopt(&f, &d, 0x101, 0x11, MEDIUM0, 30, 4);
    adopt(&f, &d, 0x102, 0x12, MEDIUM0, 60, 4);
    for (i, e) in [(0, 0.5), (1, 0.25), (2, 0.25)] {
        let mut p = d.ports()[i];
        p.efficiency = e;
        d.set_port_fields(i, p);
    }
    // [choice] The minimum, first on ties.
    assert_eq!(d.least_efficient_port(), Some((0.25, 30, 4)));
    assert_eq!(d.all_port_lots(), [0x10, 0x11, 0x12]);
    assert_eq!(d.num_airports(2), 2);
    // A port in condition 1 keeps its values.
    f.edit(0x10, |l| l.condition = CONDITION_UNPOWERED);
    *f.a.borrow_mut() = vec![1000];
    *f.b.borrow_mut() = vec![0];
    f.take();
    d.month_tick();
    assert_eq!(d.ports()[0].efficiency, 0.5);
    assert_eq!(d.last_month().skipped, 1);
    assert!(!f.take().iter().any(|l| l.contains("0x10")));
}

#[test]
fn funding_ratio_scales_usage_and_cost() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    f.follow.set(false);
    f.line.set(Some((700, 350)));
    assert_eq!(d.funding_ratio(), 0.5);
    assert_eq!(d.maintenance_cost(0x10), 350);
    let mut p = d.ports()[0];
    p.trips = 8000;
    d.set_port_fields(0, p);
    assert_eq!(d.usage_values(0x10), Some((5000, 5000)));
    // Above 100 %.
    f.line.set(Some((700, 840)));
    assert_eq!(d.usage_values(0x10), Some((8000, 12_000)));
}

#[test]
fn reference_count() {
    let (f, d) = airport();
    f.take();
    assert_eq!(d.add_ref(), 1);
    assert_eq!(d.add_ref(), 2);
    assert_eq!(d.release(), 1);
    assert_eq!(d.release(), 0);
    assert!(d.snapshot().destroyed);
    assert_eq!(f.take(), ["remove_agent cSC4PortDeveloper"]);
}

#[test]
fn do_message_routes() {
    let (f, d) = airport();
    add_lot(&f, 0x10, SMALL0, 3, 4);
    f.occupants
        .borrow_mut()
        .insert(0x100, (false, OCCUPANT_TYPE_BUILDING, 0x1508, 0x10));
    assert!(d.do_message(MSG_OCCUPANT_INSERTED, 0x100, 0, 0));
    assert_eq!(d.ports().len(), 1);
    assert!(d.do_message(MSG_MONTH, 0, 0, 0));
    assert!(d.last_month().ran);
    assert!(d.do_message(MSG_LOT_STATE_CHANGED, 0x10, 3, 5));
    assert!(d.ports().is_empty());
    assert!(d.do_message(0x1234, 0, 0, 0));
}

/// A seaport developer whose type has a `rows × columns` grid of the
/// values 1, 2, …, on a terrain of `size` cells.
fn grid_seaport(rows: u8, columns: u8, size: (u32, u32)) -> (Rc<Fake>, Rc<PortDeveloper>) {
    let mut f = Fake::new();
    let mut h = (*f.exemplars[&0xCA2D_8CE6]).clone();
    h.add(tuning::PROP_GRID_ROWS, Variant::Scalar(Elem::U8(rows)));
    h.add(tuning::PROP_GRID_COLUMNS, Variant::Scalar(Elem::U8(columns)));
    let n = u32::from(rows) * u32::from(columns);
    h.add(
        tuning::PROP_GRID,
        Variant::array(
            crate::variant::mem::FLOAT32,
            (1..=n).map(|v| Elem::F32(v as f32)).collect(),
        ),
    );
    Rc::get_mut(&mut f).unwrap().exemplars.insert(0xCA2D_8CE6, Rc::new(h));
    f.size.set(size);
    let d = PortDeveloper::new(PortKind::Seaport, f.clone(), 2);
    d.init();
    (f, d)
}

/// Adopts a seaport lot with this rectangle and facing; the altitude lines.
fn grid_writes(f: &Fake, d: &PortDeveloper, token: u64, rect: (i32, i32, i32, i32), facing: u32) -> Vec<String> {
    let mut l = Lot::new(0x4A52_917D, 1, rect.0 as i16, rect.1 as i16);
    l.rect = CellRect {
        x0: rect.0,
        z0: rect.1,
        x1: rect.2,
        z1: rect.3,
    };
    l.facing = facing;
    f.lots.borrow_mut().insert(token, l);
    f.take();
    assert!(d.adopt(token));
    f.take().into_iter().filter(|l| l.starts_with("altitude")).collect()
}

// §4.8 steps 2–4 (PD4, PD6): the product use test, the walk without the last
// row and column, the facings, metre coordinates as vertex indices.
#[test]
fn seaport_terrain_grid() {
    // 3 rows × 2 columns; a 4 × 3-cell lot walks 3 × 2 cells.
    let (f, d) = grid_seaport(3, 2, (64, 64));
    assert_eq!(d.types()[0].grid.as_ref().map(|g| g.values.len()), Some(6));
    let at = |v: &[(i32, i32, i32)]| -> Vec<String> {
        v.iter().map(|(x, z, a)| format!("altitude {x} {z} {a}")).collect()
    };
    // Facing 0: rows along x (row = x − x1, column = z − z1).
    assert_eq!(
        grid_writes(&f, &d, 0x20, (0, 0, 3, 2), 0),
        at(&[(8, 8, 1), (24, 8, 3), (40, 8, 5), (8, 24, 2), (24, 24, 4), (40, 24, 6)])
    );
    // Facing 2: the 180° turn.
    assert_eq!(
        grid_writes(&f, &d, 0x21, (0, 0, 3, 2), 2),
        at(&[(8, 8, 6), (24, 8, 4), (40, 8, 2), (8, 24, 5), (24, 24, 3), (40, 24, 1)])
    );
    // Facing 1: row = z2 − 1 − z, column = x2 − 1 − x; the flat index runs
    // over the 3 × 2 grid as if it were 2 × 3 [quirk].
    assert_eq!(
        grid_writes(&f, &d, 0x22, (0, 0, 3, 2), 1),
        at(&[(8, 8, 5), (24, 8, 4), (40, 8, 3), (8, 24, 3), (24, 24, 2), (40, 24, 1)])
    );
    // Facing 3: row = z − z1, column = x − x1.
    assert_eq!(
        grid_writes(&f, &d, 0x23, (0, 0, 3, 2), 3),
        at(&[(8, 8, 1), (24, 8, 2), (40, 8, 3), (8, 24, 3), (24, 24, 4), (40, 24, 5)])
    );
    // Any other facing sets nothing.
    assert!(grid_writes(&f, &d, 0x24, (0, 0, 3, 2), 4).is_empty());
    // Another walk product is left alone (a 3 × 2-cell rectangle walks 2).
    assert!(grid_writes(&f, &d, 0x25, (30, 20, 32, 21), 0).is_empty());
    // An offset lot: cells (10..12, 20) and (10..12, 21), on a terrain
    // large enough for its metre coordinates.
    let (f, d) = grid_seaport(3, 2, (512, 512));
    assert_eq!(
        grid_writes(&f, &d, 0x26, (10, 20, 13, 22), 0),
        at(&[
            (168, 328, 1),
            (184, 328, 3),
            (200, 328, 5),
            (168, 344, 2),
            (184, 344, 4),
            (200, 344, 6)
        ])
    );
}

#[test]
fn seaport_terrain_grid_bounds() {
    // 1 × 6 on a 3 × 2 walk: the products match, the sides do not; at facing
    // 0 the index x · 6 + z runs past the six values for x ≥ 1 [choice: set
    // nothing there].
    let (f, d) = grid_seaport(1, 6, (64, 64));
    assert_eq!(
        grid_writes(&f, &d, 0x20, (0, 0, 3, 2), 0),
        ["altitude 8 8 1", "altitude 8 24 2"]
    );
    // A vertex index outside the terrain's (cellsX + 1)(cellsZ + 1) vertices
    // is skipped [choice]; one inside it is written even when x > cellsX,
    // as the original's index would be.
    // 21 × 21 vertices: the z = 0 row (pz 8) fits, x = 40 included
    // (21 · 8 + 40 < 441); the z = 1 row (pz 24) does not.
    let (f, d) = grid_seaport(3, 2, (20, 20));
    assert_eq!(
        grid_writes(&f, &d, 0x20, (0, 0, 3, 2), 0),
        ["altitude 8 8 1", "altitude 24 8 3", "altitude 40 8 5"]
    );
    let (f, d) = grid_seaport(3, 2, (4, 4));
    assert!(grid_writes(&f, &d, 0x20, (0, 0, 3, 2), 0).is_empty());
}

// PD4 corrected (PD6): the shipped 11 × 5 declaration with 55 values serves a
// 12 × 6-cell lot (walk 11 × 5); 72 values are rejected at load.
#[test]
fn seaport_terrain_grid_shipped_size() {
    let (f, d) = grid_seaport(11, 5, (256, 256));
    assert_eq!(grid_writes(&f, &d, 0x20, (0, 0, 11, 5), 0).len(), 55);
    assert!(grid_writes(&f, &d, 0x21, (0, 10, 12, 16), 0).is_empty());
    let s = PortDeveloper::new(PortKind::Seaport, Fake::new(), 2);
    s.init();
    assert!(s.types()[0].grid.is_none());
}

// §12 test 11 (PD8): funding scales only the public query.
#[test]
fn t11_funding_only_in_the_query() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    f.follow.set(false);
    f.line.set(Some((700, 350)));
    *f.b.borrow_mut() = vec![0];
    for (jobs, ratio, query) in [(4000, 0.4f32, (4000, 5000)), (6000, 0.6, (5000, 5000))] {
        *f.a.borrow_mut() = vec![jobs];
        d.month_tick();
        assert_eq!(d.ports()[0].trips, jobs);
        assert_eq!(d.ports()[0].usage, ratio);
        assert_eq!(d.usage_values(0x10), Some(query));
    }
}

// §12 test 12 (PD5): a missing next-stage configuration is not tested.
#[test]
fn t12_missing_configuration() {
    let (f, d) = airport();
    adopt(&f, &d, 0x100, 0x10, SMALL0, 3, 4);
    *f.a.borrow_mut() = vec![9700];
    *f.b.borrow_mut() = vec![0];
    d.month_tick();
    f.missing_configs.borrow_mut().insert(0xCA24_36C1);
    f.follow.set(false);
    f.line.set(Some((700, 700)));
    f.take();
    assert_eq!(d.grow_ports_to_next_stage(), 1);
    let r = f.lot(0x10).rect;
    assert_eq!(
        f.take(),
        [
            "start_demolish 0x10".to_string(),
            "end_demolish 0x10".to_string(),
            format!("create_lot {} {} 5 3 0", r.x0, r.z0),
            "set_config 0x9000 0x0".to_string(),
            "construct 0x9000 0x0".to_string(),
            "withdraw 5700".to_string(),
            "full 800".to_string(),
        ]
    );
    let p = d.ports()[0];
    assert_eq!((p.stage, p.lot), (1, Some(0x9000)));
    let l = f.lot(0x9000);
    assert_eq!((l.config, l.state, l.condition), (None, 2, CONDITION_UNPOWERED));
    // Later months skip the port (condition 1); it never grows again.
    d.month_tick();
    assert_eq!(d.last_month().skipped, 1);
    let mut p = d.ports()[0];
    p.trips = 40_000;
    d.set_port_fields(0, p);
    assert_eq!(d.grow_ports_to_next_stage(), 0);
}

// PD9: every port without access raises the event, whatever its props.
#[test]
fn seaport_event_without_access_props() {
    let f = Fake::new();
    f.size.set((4, 3));
    f.access_props.set(0);
    let d = PortDeveloper::new(PortKind::Seaport, f.clone(), 2);
    d.init();
    add_lot(&f, 0x20, 0x4A52_917D, 2, 1);
    assert!(d.adopt(0x20));
    f.take();
    d.month_tick();
    let log = f.take();
    assert!(log.contains(&"props_state 0x20 0xc98204b8 1".to_string()));
    assert!(log.contains(&"event 0x4a490d33".to_string()));
}
