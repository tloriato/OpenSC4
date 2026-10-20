// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! A fake city and the §8 tests of `docs/specs/city-detail.md`.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use super::*;
use crate::stream::{ByteReader, ByteWriter};

#[derive(Clone, Default)]
struct Occ {
    occ_type: u32,
    tag: Option<u32>,
    lot: u64,
}

#[derive(Default)]
struct City {
    setting: Cell<u32>,
    lots: RefCell<Vec<u64>>,
    configs: RefCell<BTreeMap<u64, Rc<Vec<CfgObject>>>>,
    occupants: RefCell<BTreeMap<u64, Occ>>,
    ids: RefCell<BTreeMap<(u64, usize), Vec<u32>>>,
    network: RefCell<Vec<(i32, i32, u64)>>,
    log: RefCell<Vec<String>>,
    next: Cell<u64>,
}

impl City {
    fn new() -> Rc<City> {
        let c = Rc::new(City::default());
        c.setting.set(DETAIL_HIGH);
        c.next.set(0x9000);
        c
    }
    fn log(&self, s: String) {
        self.log.borrow_mut().push(s);
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    fn lot(&self, lot: u64, objs: Vec<CfgObject>) {
        self.lots.borrow_mut().push(lot);
        self.configs.borrow_mut().insert(lot, Rc::new(objs));
    }
    fn occ(&self, occ: u64, lot: u64, occ_type: u32, tag: Option<u32>) {
        self.occupants.borrow_mut().insert(occ, Occ { occ_type, tag, lot });
    }
    fn on_lot(&self, lot: u64) -> Vec<u64> {
        let mut v: Vec<u64> = self
            .occupants
            .borrow()
            .iter()
            .filter(|(_, o)| o.lot == lot)
            .map(|(k, _)| *k)
            .collect();
        v.sort();
        v
    }
}

fn obj(object_type: u32, flags: u8, tag: u32) -> CfgObject {
    CfgObject {
        object_type,
        flags,
        tag,
        ids: vec![tag],
        ..CfgObject::default()
    }
}

impl CityDetailHost for City {
    fn detail_setting(&self) -> u32 {
        self.setting.get()
    }
    fn lots(&self) -> Vec<u64> {
        self.lots.borrow().clone()
    }
    fn lot_config_objects(&self, lot: u64) -> Option<Rc<Vec<CfgObject>>> {
        self.configs.borrow().get(&lot).cloned()
    }
    fn lot_occupants(&self, lot: u64) -> Vec<u64> {
        self.on_lot(lot)
    }
    fn occupants_in_lot_rect(&self, lot: u64) -> Vec<u64> {
        self.on_lot(lot)
    }
    fn occupant_type(&self, occupant: u64) -> u32 {
        self.occupants.borrow().get(&occupant).map_or(0, |o| o.occ_type)
    }
    fn occupant_lot_tag(&self, occupant: u64) -> Option<u32> {
        self.occupants.borrow().get(&occupant).and_then(|o| o.tag)
    }
    fn demolish_flora(&self, occupant: u64) {
        self.log(format!("demolish_flora {occupant:#x}"));
        self.occupants.borrow_mut().remove(&occupant);
    }
    fn remove_prop(&self, occupant: u64) {
        self.log(format!("remove_prop {occupant:#x}"));
        self.occupants.borrow_mut().remove(&occupant);
    }
    fn object_placement(&self, lot: u64, index: usize) -> ObjectPlacement {
        self.log(format!("placement {lot:#x} {index}"));
        ObjectPlacement {
            x: index as f32 * 16.0,
            y: 0.0,
            z: lot as f32,
            orientation: index as u32,
        }
    }
    fn object_id_array(&self, lot: u64, index: usize) -> Vec<u32> {
        if let Some(v) = self.ids.borrow().get(&(lot, index)) {
            return v.clone();
        }
        self.configs.borrow()[&lot][index].ids.clone()
    }
    fn add_lot_prop(&self, id: u32, p: ObjectPlacement) -> Option<u64> {
        let t = self.next.get();
        self.next.set(t + 1);
        self.log(format!("add_prop {id:#x} at {} {}", p.x, p.orientation));
        self.occupants.borrow_mut().insert(
            t,
            Occ {
                occ_type: OCCUPANT_TYPE_PROP,
                tag: None,
                lot: p.z as u64,
            },
        );
        Some(t)
    }
    fn add_flora(&self, id: u32, p: ObjectPlacement) -> Option<u64> {
        let t = self.next.get();
        self.next.set(t + 1);
        self.log(format!("add_flora {id:#x} at {}", p.x));
        self.occupants.borrow_mut().insert(
            t,
            Occ {
                occ_type: OCCUPANT_TYPE_FLORA,
                tag: None,
                lot: p.z as u64,
            },
        );
        Some(t)
    }
    fn set_lot_tag(&self, occupant: u64, tag: u32) {
        self.log(format!("tag {occupant:#x} {tag:#x}"));
        if let Some(o) = self.occupants.borrow_mut().get_mut(&occupant) {
            o.tag = Some(tag);
        }
    }
    fn network_lots(&self) -> Vec<(i32, i32, u64)> {
        self.network.borrow().clone()
    }
    fn redevelop_network_lot(&self, x: i32, z: i32, network_lot: u64) {
        self.log(format!("redevelop {x} {z} {network_lot:#x}"));
    }
}

/// A queued generator; every draw is logged.
struct Queue(Rc<City>, VecDeque<u32>);

impl DetailRandom for Queue {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        self.0.log(format!("draw {n}"));
        self.1.pop_front().unwrap_or(0)
    }
}

fn manager(c: &Rc<City>) -> Rc<CityDetailManager> {
    CityDetailManager::new(c.clone(), DetailSession::new(0x1234_5678), 0x77)
}

fn record(m: &CityDetailManager) -> Vec<u8> {
    let mut w = ByteWriter::new();
    assert!(m.write(&mut w));
    w.into_bytes()
}

#[test]
fn identity() {
    assert!(query_interface(1));
    assert!(query_interface(0x6AD4_B9F1));
    assert!(query_interface(0xE4FD_A3D4));
    assert!(!query_interface(0x4522_94AA));
    let c = City::new();
    let m = manager(&c);
    assert_eq!(m.class_id(), 0xEAD4_BA36);
    assert_eq!(m.city_detail(), LEVEL_NONE);
    assert!(m.shutdown());
}

/// §8 test 1: a new city writes the current preference.
#[test]
fn t1_new_city_record() {
    let c = City::new();
    c.network.borrow_mut().push((1, 2, 0x50));
    let m = manager(&c);
    assert!(m.init());
    assert_eq!(record(&m), [0, 0, 0, 0, 3, 0, 0, 0]);
    // A new city redevelops nothing (test 6).
    assert!(c.take().is_empty());
    assert_eq!(m.last_outcome(), InitOutcome::default());
    let c = City::new();
    c.setting.set(DETAIL_LOW);
    let m = manager(&c);
    m.init();
    assert_eq!(record(&m), [0, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(m.city_detail(), 1);
}

/// §8 test 2 and the atomic read.
#[test]
fn t2_record_versions() {
    let c = City::new();
    let m = manager(&c);
    assert!(m.read(&mut ByteReader::new(&[0, 0, 0, 0, 2, 0, 0, 0])));
    assert_eq!(m.city_detail(), 2);
    assert!(!m.read(&mut ByteReader::new(&[1, 0, 0, 0, 3, 0, 0, 0])));
    assert_eq!(m.city_detail(), 2);
    assert!(!m.read(&mut ByteReader::new(&[0, 0, 0, 0, 3, 0])));
    assert_eq!(m.city_detail(), 2);
    // Round trip.
    let bytes = record(&m);
    let m2 = manager(&c);
    assert!(m2.read(&mut ByteReader::new(&bytes)));
    assert_eq!(record(&m2), bytes);
}

/// §8 test 3: the same level changes nothing.
#[test]
fn t3_same_level() {
    let c = City::new();
    c.lot(0x10, vec![obj(1, 0, 0xA)]);
    c.occ(0x11, 0x10, OCCUPANT_TYPE_PROP, Some(0xB));
    c.network.borrow_mut().push((1, 2, 0x50));
    let m = manager(&c);
    m.set_level(3);
    m.init();
    assert!(c.take().is_empty());
    assert_eq!(c.occupants.borrow().len(), 1);
}

/// §8 test 4: reduce.
#[test]
fn t4_reduce() {
    let c = City::new();
    // Lot 0x10 lists prop object 0xA (optional) and flora object 0xF.
    c.lot(0x10, vec![obj(1, 1, 0xA), obj(4, 0, 0xF), obj(2, 0, 0xC)]);
    c.occ(0x11, 0x10, OCCUPANT_TYPE_PROP, Some(0xA)); // listed (optional): stays
    c.occ(0x12, 0x10, OCCUPANT_TYPE_PROP, Some(0xF)); // tagged with the flora object: stays
    c.occ(0x13, 0x10, OCCUPANT_TYPE_PROP, Some(0x7)); // T: removed
    c.occ(0x14, 0x10, OCCUPANT_TYPE_FLORA, Some(0x7)); // T: demolished
    c.occ(0x15, 0x10, OCCUPANT_TYPE_PROP, None); // untagged: stays
    c.occ(0x16, 0x10, 0x2781_28A0, Some(0x7)); // a building: untouched
    c.occ(0x17, 0x10, OCCUPANT_TYPE_PROP, Some(0xC)); // a texture object's tag: not kept
                                                      // A lot without a configuration.
    c.lots.borrow_mut().push(0x20);
    c.occ(0x21, 0x20, OCCUPANT_TYPE_PROP, Some(0x7));
    c.network.borrow_mut().extend([(3, 4, 0x50), (5, 6, 0x51)]);
    let m = manager(&c);
    m.set_level(3);
    c.setting.set(1);
    m.init();
    assert_eq!(
        c.take(),
        [
            "remove_prop 0x13",
            "demolish_flora 0x14",
            "remove_prop 0x17",
            "redevelop 3 4 0x50",
            "redevelop 5 6 0x51"
        ]
    );
    assert_eq!(m.city_detail(), 1);
    assert_eq!(
        m.last_outcome(),
        InitOutcome {
            pass: 1,
            removed: 3,
            network_lots: 2,
            ..Default::default()
        }
    );
}

/// §8 test 5: increase.
#[test]
fn t5_increase() {
    let c = City::new();
    // Required props A, B; optional prop C; required flora D.
    c.lot(
        0x10,
        vec![obj(1, 0, 0xA), obj(1, 0, 0xB), obj(1, 1, 0xC), obj(4, 0, 0xD)],
    );
    c.occ(0x11, 0x10, OCCUPANT_TYPE_PROP, Some(0xA));
    c.ids.borrow_mut().insert((0x10, 3), vec![0xD0, 0xD1, 0xD2]);
    c.network.borrow_mut().push((1, 1, 0x50));
    let m = manager(&c);
    m.set_level(1);
    m.set_next_random(Box::new(Queue(c.clone(), VecDeque::from([0, 2]))));
    m.init();
    assert_eq!(
        c.take(),
        [
            "placement 0x10 1",
            "draw 1",
            "add_prop 0xb at 16 1",
            "tag 0x9000 0xb",
            "placement 0x10 3",
            "draw 3",
            "add_flora 0xd2 at 48",
            "tag 0x9001 0xd",
            "redevelop 1 1 0x50"
        ]
    );
    assert_eq!(m.city_detail(), 3);
    assert_eq!(m.last_outcome().draws, 2);
    assert_eq!(m.last_outcome().created, 2);
}

/// §3.3: an empty ID list draws nothing, an ID 0 creates nothing, a
/// repeated required ID is created twice [quirk], and a missing occupant
/// leaves the tag unset.
#[test]
fn increase_edge_cases() {
    let c = City::new();
    c.lot(
        0x10,
        vec![obj(1, 0, 0xA), obj(1, 0, 0xA), obj(1, 0, 0xB), obj(4, 0, 0xE)],
    );
    c.ids.borrow_mut().insert((0x10, 2), vec![]);
    c.ids.borrow_mut().insert((0x10, 3), vec![0]);
    let m = manager(&c);
    m.set_level(2);
    m.set_next_random(Box::new(Queue(c.clone(), VecDeque::new())));
    m.init();
    assert_eq!(
        c.take(),
        [
            "placement 0x10 0",
            "draw 1",
            "add_prop 0xa at 0 0",
            "tag 0x9000 0xa",
            "placement 0x10 1",
            "draw 1",
            "add_prop 0xa at 16 1",
            "tag 0x9001 0xa",
            "placement 0x10 2",
            "placement 0x10 3",
            "draw 1",
        ]
    );
}

/// §3.5: each increase pass gets its own generator,
/// `derive_seed(S, 0xEAD4BA36 ^ n)`, `n` counted per manager object from 0
/// (CD5).
#[test]
fn session_generators() {
    let s = DetailSession::new(0x1234_5678);
    assert_eq!(s.pass_seed(0), derive_seed(0x1234_5678, 0xEAD4_BA36));
    assert_eq!(s.pass_seed(1), derive_seed(0x1234_5678, 0xEAD4_BA37));
    let mut a = s.generator(0);
    let b = s.generator(1);
    let mut a2 = RzRandom::new(
        s.pass_seed(0),
        &FixedSeeds {
            wall_clock: 0,
            process_clock: 0,
        },
    );
    assert_eq!(a.lcg_state(), s.pass_seed(0));
    assert_eq!(b.lcg_state(), s.pass_seed(1));
    assert_eq!(DetailRandom::uniform_u32(&mut a, 1000), a2.uniform_u32(1000));
    // A reduce pass does not count.
    let c = City::new();
    let m = CityDetailManager::new(c.clone(), s.clone(), 1);
    assert_eq!(m.passes(), 0);
    m.set_level(1);
    c.setting.set(3);
    m.init();
    assert_eq!(m.passes(), 1);
    c.setting.set(2);
    m.init();
    assert_eq!(m.passes(), 1);
    c.setting.set(3);
    m.init();
    assert_eq!(m.passes(), 2);
    // A new manager (the next city load) starts again at n = 0, even with
    // the same session.
    let m2 = CityDetailManager::new(c.clone(), s.clone(), 2);
    assert_eq!(m2.passes(), 0);
    m2.set_level(1);
    m2.init();
    assert_eq!((m.passes(), m2.passes()), (2, 1));
}

/// CD5: the n-th pass of a new manager draws as the n-th pass of the first.
#[test]
fn new_manager_repeats_the_draws() {
    let run = |c: &Rc<City>, m: &CityDetailManager| {
        m.set_level(1);
        c.setting.set(3);
        m.init();
        c.take()
    };
    let lot = |c: &Rc<City>| {
        c.lot(0x10, vec![obj(1, 0, 0xA)]);
        c.ids
            .borrow_mut()
            .insert((0x10, 0), (0..97).map(|i| 0x100 + i).collect());
    };
    let s = DetailSession::new(0xCAFE_F00D);
    let c1 = City::new();
    lot(&c1);
    let first = run(&c1, &CityDetailManager::new(c1.clone(), s.clone(), 1));
    let c2 = City::new();
    lot(&c2);
    let second = run(&c2, &CityDetailManager::new(c2.clone(), s.clone(), 2));
    assert_eq!(first, second);
    let mut r = s.generator(0);
    let k = DetailRandom::uniform_u32(&mut r, 97);
    assert_eq!(first[1], format!("add_prop {:#x} at 0 0", 0x100 + k));
}

/// CD4: the presence test of step 3 counts flora and prop occupants by tag
/// alone; any other occupant is ignored. Step 4 draws for any object type
/// whose ID is needed; only props and flora create.
#[test]
fn cd4_presence_and_object_types() {
    let c = City::new();
    // Required prop A, required flora F, a required type-2 object sharing
    // prop B's ID, required prop B.
    c.lot(
        0x10,
        vec![obj(1, 0, 0xA), obj(4, 0, 0xF), obj(2, 0, 0xB), obj(1, 0, 0xB)],
    );
    // A building tagged A does not satisfy A.
    c.occ(0x11, 0x10, 0x2781_28A0, Some(0xA));
    // A prop tagged with the flora object's ID satisfies F.
    c.occ(0x12, 0x10, OCCUPANT_TYPE_PROP, Some(0xF));
    let m = manager(&c);
    m.set_level(1);
    m.set_next_random(Box::new(Queue(c.clone(), VecDeque::new())));
    m.init();
    assert_eq!(
        c.take(),
        [
            "placement 0x10 0",
            "draw 1",
            "add_prop 0xa at 0 0",
            "tag 0x9000 0xa",
            "placement 0x10 2",
            "draw 1",
            "placement 0x10 3",
            "draw 1",
            "add_prop 0xb at 48 3",
            "tag 0x9001 0xb",
        ]
    );
    assert_eq!((m.last_outcome().draws, m.last_outcome().created), (3, 2));
}

#[test]
fn reference_count() {
    let c = City::new();
    let m = manager(&c);
    assert_eq!(m.add_ref(), 1);
    assert_eq!(m.add_ref(), 2);
    assert_eq!(m.release(), 1);
    assert!(!m.snapshot().destroyed);
    assert_eq!(m.release(), 0);
    assert!(m.snapshot().destroyed);
}
