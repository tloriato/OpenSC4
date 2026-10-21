// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! A fake occupant world and the §18 tests (one test per item, same
//! numbers), plus the §14 lock regions.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::*;
use crate::stream::{ByteReader, ByteWriter, RefResolver, RefValue, RegisteredObject, SaveObject, SaveRegistry};

/// The class written for occupants in these tests.
const OCC_CLASS: u32 = 0x1234_5678;

/// One fake occupant: the base `Init` / `Shutdown` / `PostOccupantMessage`
/// of §5.6.
#[derive(Clone, Debug, Default)]
struct Occ {
    bbox: BBox,
    holder: bool,
    occ_type: u32,
    flags: u32,
    /// The stored manager box (`None` = the 0xFF "no box" mark).
    om: Option<CellBox>,
    initialized: bool,
    /// Bit 1 of `+0x2E` (set by the occupant's own `Read`, §13): the base
    /// `Init` calls slot 17 instead of posting Create (§5.6).
    loaded: bool,
    /// `+0x30` is set: Move is sent synchronously.
    sync: bool,
}

/// A message as the queue holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Posted {
    msg: u32,
    occ: u64,
    data2: bool,
    data3: u32,
    data4: u32,
    sync: bool,
}

#[derive(Default)]
struct World {
    occs: RefCell<BTreeMap<u64, Occ>>,
    refs: RefCell<BTreeMap<u64, i64>>,
    queue: RefCell<Vec<Posted>>,
    /// Filters: token → occupants excluded.
    filters: RefCell<BTreeMap<u64, BTreeSet<u64>>>,
    depth: Cell<i32>,
    /// Occupants whose base `Init` called slot 17 (loaded ones).
    slot17: RefCell<Vec<u64>>,
    /// "lock" / "unlock" when tracing.
    trace: RefCell<Vec<String>>,
    tracing: Cell<bool>,
}

impl World {
    fn new() -> Rc<World> {
        Rc::new(World::default())
    }

    fn add(&self, occ: u64, occ_type: u32, min: Vec3, max: Vec3) {
        self.occs.borrow_mut().insert(
            occ,
            Occ {
                bbox: BBox { min, max },
                holder: true,
                occ_type,
                ..Occ::default()
            },
        );
    }

    /// A flat occupant `x0..x1`, `z0..z1`, y 0..10, type 0x100.
    fn flat(&self, occ: u64, x0: f32, z0: f32, x1: f32, z1: f32) {
        self.add(occ, 0x100, [x0, 0.0, z0], [x1, 10.0, z1]);
    }

    fn with(&self, occ: u64, f: impl FnOnce(&mut Occ)) {
        f(self.occs.borrow_mut().get_mut(&occ).expect("occupant"));
    }

    fn get(&self, occ: u64) -> Occ {
        self.occs.borrow().get(&occ).cloned().expect("occupant")
    }

    fn refs(&self, occ: u64) -> i64 {
        self.refs.borrow().get(&occ).copied().unwrap_or(0)
    }

    fn take(&self) -> Vec<Posted> {
        std::mem::take(&mut *self.queue.borrow_mut())
    }

    fn post(&self, occ: u64, msg: u32, data4: u32, sync: bool) {
        let o = self.get(occ);
        self.queue.borrow_mut().push(Posted {
            msg,
            occ,
            data2: o.flags & 4 != 0,
            data3: o.occ_type,
            data4,
            sync,
        });
    }

    fn mark(&self, s: &str) {
        if self.tracing.get() {
            self.trace.borrow_mut().push(s.to_owned());
        }
    }
}

impl OccupantHost for World {
    fn lock(&self) {
        self.depth.set(self.depth.get() + 1);
        self.mark("lock");
    }
    fn unlock(&self) {
        self.depth.set(self.depth.get() - 1);
        self.mark("unlock");
    }
    fn add_ref(&self, occ: u64) {
        *self.refs.borrow_mut().entry(occ).or_insert(0) += 1;
    }
    fn release(&self, occ: u64) {
        *self.refs.borrow_mut().entry(occ).or_insert(0) -= 1;
    }
    fn init(&self, occ: u64) {
        if self.get(occ).initialized {
            return;
        }
        self.with(occ, |o| o.initialized = true);
        if self.get(occ).loaded {
            self.slot17.borrow_mut().push(occ);
        } else {
            self.post(occ, msg::CREATE, 0, false);
        }
    }
    fn shutdown(&self, occ: u64) {
        if self.get(occ).initialized {
            self.with(occ, |o| o.initialized = false);
            self.post(occ, msg::DESTROY, 0, false);
        }
    }
    fn has_property_holder(&self, occ: u64) -> bool {
        self.get(occ).holder
    }
    fn occupant_type(&self, occ: u64) -> u32 {
        self.get(occ).occ_type
    }
    fn bounding_box(&self, occ: u64) -> BBox {
        self.get(occ).bbox
    }
    fn post_message(&self, occ: u64, msg_type: u32, data4: u32) {
        let sync = msg_type == msg::MOVE && self.get(occ).sync;
        self.post(occ, msg_type, data4, sync);
    }
    fn manager_bbox(&self, occ: u64) -> Option<CellBox> {
        self.get(occ).om
    }
    fn set_manager_bbox(&self, occ: u64, b: CellBox) {
        self.with(occ, |o| o.om = Some(b));
    }
    fn set_flag(&self, occ: u64, flag: u32) {
        self.with(occ, |o| o.flags |= flag);
    }
    fn clear_flag(&self, occ: u64, flag: u32) {
        self.with(occ, |o| o.flags &= !flag);
    }
    fn is_flag_set(&self, occ: u64, flag: u32) -> bool {
        self.get(occ).flags & flag != 0
    }
    fn filter_includes(&self, filter: u64, occ: u64) -> bool {
        !self.filters.borrow().get(&filter).is_some_and(|x| x.contains(&occ))
    }
    fn object_ref(&self, occ: u64) -> RefValue {
        RefValue::Host {
            token: occ,
            class_id: OCC_CLASS,
        }
    }
    fn resolve_ref(&self, r: &RefValue, _iid: u32) -> Option<u64> {
        match *r {
            RefValue::Host { token, .. } if self.occs.borrow().contains_key(&token) => Some(token),
            RefValue::Host { token, .. } => {
                self.release(token);
                None
            }
            _ => None,
        }
    }
}

/// A 1024 m city, initialised.
fn city() -> (Rc<World>, OccupantManager) {
    let w = World::new();
    let m = OccupantManager::new(w.clone());
    assert!(m.set_city_size(1024.0, 1024.0));
    assert!(m.init());
    (w, m)
}

fn occupants(m: &OccupantManager, x: i32, z: i32) -> Vec<u64> {
    m.cell(x, z).unwrap_or_default().iter().map(|n| n.occupant).collect()
}

fn ids(p: &[Posted]) -> Vec<(u32, u64)> {
    p.iter().map(|p| (p.msg, p.occ)).collect()
}

fn posted(msg: u32, occ: u64, data3: u32, data4: u32) -> Posted {
    Posted {
        msg,
        occ,
        data2: false,
        data3,
        data4,
        sync: false,
    }
}

// ================================================================ §18.1

#[test]
fn om18_01_counts() {
    let w = World::new();
    let m = OccupantManager::new(w.clone());
    assert!(m.set_city_size(1024.0, 2048.0));
    assert_eq!(m.snapshot().counts, [16, 32, 64, 128]);
    assert_eq!(m.cell_bounds(), (64, 64, 79, 95));
    assert_eq!(m.cell_count(), (16, 32));
    assert_eq!(m.standard_cell_count(), (64, 128));
    assert_eq!(m.cell_sizes(), (64.0, 64.0));
    assert_eq!(m.world_cell_count(), (192, 192));
    assert!(m.init());
    assert!(!m.set_city_size(4096.0, 4096.0));
    assert_eq!(m.snapshot().counts, [16, 32, 64, 128]);
    assert_eq!(m.city_size(), (1024.0, 2048.0));
    // The three shipped sizes (§2.1).
    for (size, om, std) in [(1024.0, 16, 64), (2048.0, 32, 128), (4096.0, 64, 256)] {
        let m = OccupantManager::new(w.clone());
        assert!(m.set_city_size(size, size));
        assert_eq!(m.snapshot().counts, [om, om, std, std]);
    }
}

// ================================================================ §18.2

#[test]
fn om18_02_position_to_world_cell() {
    let table: [(f32, i32); 12] = [
        (0.0, 64),
        (63.9, 64),
        (63.9365, 65),
        (63.95, 65),
        (64.0, 65),
        (-0.064, 64),
        (-0.065, 63),
        (-0.5, 63),
        (1023.9, 79),
        (1024.0, 80),
        (-4096.0, 0),
        (-4200.0, -2),
    ];
    for (f, want) in table {
        assert_eq!(position_to_world_cell(f, f), (want, want), "slot 15({f})");
    }
    for (f, want) in [(0.0, 0), (63.9, 0), (63.95, 1), (64.0, 1), (-0.5, -1)] {
        assert_eq!(position_to_occupant_manager_cell(f, f), (want, want), "slot 17({f})");
    }
}

// ================================================================ §18.3

#[test]
fn om18_03_standard_cell_floor() {
    for (f, want) in [
        (0.0, 0),
        (15.99, 0),
        (16.0, 1),
        (31.99, 1),
        (32.0, 2),
        (-0.1, -1),
        (-16.0, -1),
    ] {
        assert_eq!(position_to_standard_city_cell(f, f), (want, want), "slot 19({f})");
    }
    // An exact floor at and just below every boundary of a 4096 m city.
    for c in 1..=256 {
        let edge = c as f32 * 16.0;
        let below = f32::from_bits(edge.to_bits() - 1);
        assert_eq!(position_to_standard_city_cell(edge, below), (c, c - 1), "edge {edge}");
    }
}

// ================================================================ §18.4

#[test]
fn om18_04_slot12_bug() {
    assert_eq!(world_cell_to_standard_city_cell(70, 66, 5), (8, 20));
    assert_eq!(standard_city_cell_to_world_cell(-1, 5), (64, 65));
    assert_eq!(world_cell_to_occupant_manager_cell(70, 64), (6, 0));
    assert_eq!(occupant_manager_cell_to_world_cell(6, 0), (70, 64));
    assert_eq!(world_cell_to_position(65, 63), (64.0, -64.0));
    assert_eq!(occupant_manager_cell_to_position(2, 3), (128.0, 192.0));
    assert_eq!(standard_city_cell_to_position(2, 3), (32.0, 48.0));
}

// ================================================================ §18.5

#[test]
fn om18_05_insert_cells() {
    let (w, m) = city();
    w.flat(1, 100.0, 0.0, 164.0, 64.0);
    assert!(m.insert(1, 0));
    assert_eq!(w.get(1).om, Some(CellBox::new(65, 64, 66, 64)));
    for x in [65, 66] {
        assert_eq!(
            m.cell(x, 64),
            Some(vec![Node {
                occupant: 1,
                single: false
            }])
        );
    }
    w.flat(2, 128.0, 128.0, 128.5, 128.5);
    assert!(m.insert(2, 0));
    assert_eq!(w.get(2).om, Some(CellBox::new(66, 66, 66, 66)));
    assert_eq!(
        m.cell(66, 66),
        Some(vec![Node {
            occupant: 2,
            single: true
        }])
    );
    w.flat(3, 63.9, 10.0, 64.5, 10.5);
    assert!(m.insert(3, 0));
    assert_eq!(w.get(3).om, Some(CellBox::new(64, 64, 65, 64)));
    // Exactly 1.0 m is not shrunk; z is shrunk separately.
    let b = cell_box_of(&BBox {
        min: [63.5, 0.0, 63.95],
        max: [64.5, 0.0, 128.0],
    });
    assert_eq!(b, CellBox::new(64, 65, 65, 65), "z 64.05 … 127.9");
}

// ================================================================ §18.6

#[test]
fn om18_06_insert_messages() {
    let (w, m) = city();
    w.add(1, 0x0BEE, [10.0, 0.0, 10.0], [20.0, 5.0, 20.0]);
    assert!(m.insert(1, 7));
    assert_eq!(
        w.take(),
        vec![posted(msg::CREATE, 1, 0x0BEE, 0), posted(msg::INSERT, 1, 0x0BEE, 7)]
    );
    assert!(w.get(1).flags & FLAG_INSERTED != 0);
    assert_eq!(w.get(1).om, Some(CellBox::new(64, 64, 64, 64)));
    assert_eq!(w.refs(1), 1);
    assert!(!m.insert(1, 7));
    assert!(w.take().is_empty());
    assert_eq!(w.refs(1), 1);
    // An initialised occupant posts no Create; data2 = IsFlagSet(4).
    w.flat(2, 10.0, 10.0, 20.0, 20.0);
    w.with(2, |o| {
        o.flags = 4;
        o.initialized = true;
    });
    assert!(m.insert(2, 3));
    let mut want = posted(msg::INSERT, 2, 0x100, 3);
    want.data2 = true;
    assert_eq!(w.take(), vec![want]);
    assert_eq!(occupants(&m, 64, 64), vec![2, 1]);
}

// ================================================================ §18.7

#[test]
fn om18_07_insert_failure() {
    let (w, m) = city();
    w.flat(1, 10.0, 10.0, 20.0, 20.0);
    w.with(1, |o| o.holder = false);
    assert!(!m.insert(1, 0));
    assert_eq!(ids(&w.take()), vec![(msg::CREATE, 1)]);
    assert_eq!(w.refs(1), 0);
    assert_eq!(w.get(1).flags & FLAG_INSERTED, 0);
    assert_eq!(w.get(1).om, None);
    assert!(m.is_cell_empty(64, 64));
    assert!(!m.insert(0, 0));
}

// ================================================================ §18.8

#[test]
fn om18_08_newest_first() {
    let (w, m) = city();
    for o in [1, 2, 3] {
        w.flat(o, 130.0, 130.0, 140.0, 140.0);
        assert!(m.insert(o, 0));
    }
    assert_eq!(m.first_in_cell(66, 66, Filter::All), Some(3));
    assert_eq!(w.refs(3), 2, "the caller owns one reference");
    w.release(3);
    let mut l = Vec::new();
    assert!(m.occupants_by_cells(&mut l, Some((66, 66)), Some((66, 66)), Filter::All, -1));
    assert_eq!(l, vec![3, 2, 1]);
    m.release_occupant_list(&mut l);
    assert!(l.is_empty());
    assert!(m.remove(3, false, 0));
    assert_eq!(m.first_in_cell(66, 66, Filter::All), Some(2));
    assert_eq!([w.refs(1), w.refs(2), w.refs(3)], [1, 2, 0]);
}

// ================================================================ §18.9

#[test]
fn om18_09_dedup_and_order() {
    let (w, m) = city();
    w.flat(4, 100.0, 0.0, 164.0, 64.0); // D: x 65…66, z 64
    w.flat(5, 130.0, 10.0, 140.0, 20.0); // E: (66, 64)
    assert!(m.insert(4, 0));
    assert!(m.insert(5, 0));
    let q = |l: &mut Vec<u64>, f: Filter, n: i32| m.occupants_by_cells(l, Some((65, 64)), Some((66, 64)), f, n);
    let mut l = Vec::new();
    assert!(q(&mut l, Filter::All, -1));
    assert_eq!(l, vec![4, 5]);
    let mut l = Vec::new();
    assert!(q(&mut l, Filter::All, 1));
    assert_eq!(l, vec![4]);
    let mut l = vec![99];
    assert!(!q(&mut l, Filter::All, 0));
    assert_eq!(l, vec![99]);
    w.filters.borrow_mut().insert(0xF1, BTreeSet::from([4]));
    let mut l = Vec::new();
    assert!(q(&mut l, Filter::Host(0xF1), -1));
    assert_eq!(l, vec![5]);
    let mut l = Vec::new();
    assert!(m.occupants_by_cells(&mut l, Some((100, 100)), Some((101, 101)), Filter::All, -1));
    assert!(l.is_empty());
    // Each appended element holds one reference (not released here).
    assert_eq!([w.refs(4), w.refs(5)], [3, 3]);
}

// ================================================================ §18.10

#[test]
fn om18_10_strict_box() {
    let (w, m) = city();
    w.add(6, 1, [0.0, -1.0, 0.0], [16.0, 5.0, 16.0]); // F
    w.add(7, 1, [16.0, -1.0, 0.0], [32.0, 5.0, 16.0]); // G
    assert!(m.insert(6, 0) && m.insert(7, 0));
    let mut l = Vec::new();
    assert!(m.occupants_by_standard_cells(&mut l, Some((0, 0)), Some((0, 0)), Filter::All, -1));
    assert_eq!(l, vec![6]);
    let p = [8.0, 0.0, 8.0];
    let mut l = Vec::new();
    assert!(m.occupants_by_bbox(&mut l, Some(p), Some(p), Filter::All, -1));
    assert_eq!(l, vec![6], "F's y range −1…5 strictly contains 0");
    w.with(6, |o| o.bbox.min[1] = 0.0);
    let mut l = Vec::new();
    assert!(m.occupants_by_bbox(&mut l, Some(p), Some(p), Filter::All, -1));
    assert!(l.is_empty(), "F's y range 0…5 only touches 0");
    // Slot 32: one standard cell.
    assert_eq!(m.first_by_standard_cell(1, 0, Filter::All), Some(7));
    // NaN passes the overlap test.
    let nan = BBox {
        min: [f32::NAN; 3],
        max: [f32::NAN; 3],
    };
    assert!(overlaps(&nan, &[0.0; 3], &[1.0; 3], &[0, 1, 2]));
    // Slot 28: min ≤ f < max on the real box.
    assert_eq!(m.first_by_position(16.0, 8.0, Filter::All), Some(7));
    assert_eq!(m.first_by_position(15.99, 8.0, Filter::All), Some(6));
    assert_eq!(m.first_by_position(32.0, 8.0, Filter::All), None);
}

// ================================================================ §18.11

#[test]
fn om18_11_null_max_quirk() {
    let (w, m) = city();
    w.flat(8, 1030.0, 10.0, 1035.0, 20.0);
    assert!(m.insert(8, 0));
    assert_eq!(w.get(8).om, Some(CellBox::new(80, 64, 80, 64)));
    let mut l = Vec::new();
    assert!(m.occupants_by_standard_cells(&mut l, None, None, Filter::All, -1));
    assert_eq!(l, vec![8], "x up to 65·16 − 0.1 = 1039.9");
    let mut l = Vec::new();
    assert!(m.occupants_by_standard_cells(&mut l, None, Some((100, 100)), Filter::All, -1));
    assert!(l.is_empty(), "an explicit max clamps to 63");
    let mut n = 0;
    assert!(m.iterate_by_standard_cells(
        &mut |_| {
            n += 1;
            true
        },
        None,
        None,
        Filter::All
    ));
    assert_eq!(n, 1);
}

// ================================================================ §18.12

#[test]
fn om18_12_typed_type_zero() {
    let (w, m) = city();
    w.add(1, 0xA, [10.0, 0.0, 10.0], [20.0, 1.0, 20.0]);
    w.add(2, 0xB, [10.0, 0.0, 10.0], [20.0, 1.0, 20.0]);
    assert!(m.insert(1, 0) && m.insert(2, 0));
    let mut l = Vec::new();
    assert!(m.occupants_by_standard_cells_typed(&mut l, None, None, 0, -1));
    assert_eq!(l, vec![2, 1]);
    assert_eq!(m.shared_filter_type(), Some(0));
    assert_eq!(m.first_in_cell_typed(64, 64, 0), None);
    assert!(m.remove_occupants_of_type(0, false, 0));
    assert_eq!(occupants(&m, 64, 64), vec![2, 1]);
    let mut l = Vec::new();
    assert!(m.occupants_by_standard_cells_typed(&mut l, None, None, 0xA, -1));
    assert_eq!(l, vec![1]);
    assert_eq!(m.shared_filter_type(), Some(0xA));
    assert_eq!(m.first_in_cell_typed(64, 64, 0xB), Some(2));
    assert_eq!(m.shared_filter_type(), Some(0xB));
    // An occupant of type 0 is found by the literal typed thunks.
    w.add(3, 0, [10.0, 0.0, 10.0], [20.0, 1.0, 20.0]);
    assert!(m.insert(3, 0));
    assert_eq!(m.first_in_cell_typed(64, 64, 0), Some(3));
    assert!(m.remove_occupants_of_type(0, false, 0));
    assert_eq!(occupants(&m, 64, 64), vec![2, 1]);
}

// ================================================================ §18.13

#[test]
fn om18_13_remove_order() {
    let (w, m) = city();
    w.flat(1, 100.0, 0.0, 164.0, 64.0);
    assert!(m.insert(1, 0));
    w.take();
    assert_eq!(w.refs(1), 1);
    assert!(m.remove(1, true, 3));
    let q = w.take();
    assert_eq!(ids(&q), vec![(msg::REMOVE, 1), (msg::DESTROY, 1)]);
    assert_eq!((q[0].data4, q[1].data4), (3, 0));
    assert_eq!(w.get(1).flags & FLAG_INSERTED, 0);
    assert_eq!(w.refs(1), 0);
    assert!(m.is_cell_empty(65, 64) && m.is_cell_empty(66, 64));
    assert_eq!(m.cell(65, 64), None, "the holder is freed");
    // Not inserted: false, but Shutdown still runs (initialised: Destroy).
    w.with(1, |o| o.initialized = true);
    assert!(!m.remove(1, true, 3));
    assert_eq!(ids(&w.take()), vec![(msg::DESTROY, 1)]);
    assert_eq!(w.refs(1), 0);
    // Inserted without a stored box: false, flag stays, nothing posted.
    assert!(m.insert(1, 0));
    w.with(1, |o| o.om = None);
    w.take();
    assert!(!m.remove(1, false, 3));
    assert!(w.take().is_empty());
    assert!(w.get(1).flags & FLAG_INSERTED != 0);
    assert_eq!(w.refs(1), 1);
    // A null occupant does nothing.
    assert!(!m.remove(0, true, 0));
}

// ================================================================ §18.14

#[test]
fn om18_14_remove_occupants_order() {
    let (w, m) = city();
    w.flat(10, 10.0, 10.0, 20.0, 20.0); // H: (64, 64)
    w.flat(11, 10.0, 70.0, 100.0, 80.0); // J: (64, 65)…(65, 65)
    assert!(m.insert(10, 0) && m.insert(11, 0));
    assert_eq!(w.get(11).om, Some(CellBox::new(64, 65, 65, 65)));
    w.take();
    assert!(m.remove_occupants(Filter::All, true, 0));
    assert_eq!(
        ids(&w.take()),
        vec![
            (msg::REMOVE, 10),
            (msg::DESTROY, 10),
            (msg::REMOVE, 11),
            (msg::DESTROY, 11)
        ]
    );
    assert_eq!(m.snapshot().holders, 0);
    assert_eq!([w.refs(10), w.refs(11)], [0, 0]);
    assert_eq!(w.get(11).flags & FLAG_INSERTED, 0);
}

// ================================================================ §18.15

#[test]
fn om18_15_move() {
    let (w, m) = city();
    w.flat(1, 100.0, 0.0, 164.0, 64.0); // (65, 64)…(66, 64)
    w.flat(2, 140.0, 10.0, 150.0, 20.0); // (66, 64)
    w.flat(3, 200.0, 10.0, 210.0, 20.0); // (67, 64)
    for o in [1, 2, 3] {
        assert!(m.insert(o, 0));
    }
    assert_eq!(occupants(&m, 66, 64), vec![2, 1]);
    w.take();
    w.with(1, |o| {
        o.bbox.min[0] = 150.0;
        o.bbox.max[0] = 230.0;
        o.sync = true;
    });
    assert!(m.move_occupant(1, true));
    assert_eq!(w.get(1).om, Some(CellBox::new(66, 64, 67, 64)));
    assert_eq!(m.cell(65, 64), None);
    assert_eq!(occupants(&m, 66, 64), vec![2, 1], "keeps its place");
    assert_eq!(occupants(&m, 67, 64), vec![1, 3], "at the front");
    let mut want = posted(msg::MOVE, 1, 0x100, 0);
    want.sync = true;
    assert_eq!(w.take(), vec![want]);
    assert!(m.move_occupant(1, true));
    assert_eq!(ids(&w.take()), vec![(msg::MOVE, 1)], "no box change: still posted");
    assert!(m.move_occupant(1, false));
    assert!(w.take().is_empty());
    // To a single cell: the bit is rewritten in the common cell.
    w.with(1, |o| o.bbox.max[0] = 160.0);
    assert!(m.move_occupant(1, false));
    assert_eq!(
        m.cell(66, 64).unwrap()[1],
        Node {
            occupant: 1,
            single: true
        }
    );
    assert_eq!(occupants(&m, 67, 64), vec![3]);
    // Not inserted: false; no reference taken.
    w.flat(4, 0.0, 0.0, 1.0, 1.0);
    assert!(!m.move_occupant(4, true));
    assert_eq!(w.refs(1), 1);
}

// ================================================================ §18.16

#[test]
fn om18_16_find_occupant() {
    let (w, m) = city();
    w.flat(1, 384.0, 384.0, 512.0, 448.0);
    assert!(m.insert(1, 0));
    assert_eq!(w.get(1).om, Some(CellBox::new(70, 70, 71, 70)));
    let (mut x, mut z) = (-1, -1);
    assert!(m.find_occupant(1, false, &mut x, &mut z));
    assert_eq!((x, z), (-1, -1));
    w.with(1, |o| o.om = Some(CellBox::new(0, 0, 0, 0)));
    assert!(!m.find_occupant(1, false, &mut x, &mut z));
    assert!(m.find_occupant(1, true, &mut x, &mut z));
    assert_eq!((x, z), (70, 70));
    // No stored box: the guess comes from the real box (x shrink only).
    w.with(1, |o| o.om = None);
    let (mut x, mut z) = (-1, -1);
    assert!(m.find_occupant(1, false, &mut x, &mut z));
    assert_eq!((x, z), (-1, -1));
}

// ================================================================ §18.17

#[test]
fn om18_17_bounding_cells() {
    assert_eq!(
        bounding_cells(5, 5),
        vec![(4, 4), (4, 5), (4, 6), (5, 4), (5, 6), (6, 4), (6, 5), (6, 6)]
    );
    assert_eq!(bounding_cells(0, 0), vec![(0, 1), (1, 0), (1, 1)]);
    assert_eq!(
        bounding_cells(191, 5),
        vec![(190, 4), (190, 5), (190, 6), (191, 4), (191, 6)]
    );
    assert!(bounding_cells(-5, 7).is_empty());
}

// ================================================================ §18.18

#[test]
fn om18_18_iteration() {
    let (w, m) = city();
    w.flat(1, 10.0, 10.0, 20.0, 20.0); // (64, 64)
    w.flat(2, 70.0, 10.0, 80.0, 20.0); // (65, 64)
    w.flat(3, 10.0, 70.0, 20.0, 80.0); // (64, 65)
    for o in [1, 2, 3] {
        assert!(m.insert(o, 0));
    }
    let mut seen = Vec::new();
    assert!(m.iterate(
        &mut |o| {
            seen.push(o);
            true
        },
        Some([64, 999, 64]),
        Some([64, -5, 64]),
        Filter::All
    ));
    assert_eq!(seen, vec![1]);
    let mut seen = Vec::new();
    assert!(m.iterate(
        &mut |o| {
            seen.push(o);
            true
        },
        None,
        None,
        Filter::All
    ));
    assert_eq!(seen, vec![1, 3, 2], "x-major");
    // Slot 42 stopping on the first of three in a cell.
    for o in [4, 5] {
        w.flat(o, 10.0, 10.0, 20.0, 20.0);
        assert!(m.insert(o, 0));
    }
    let before: Vec<i64> = [1, 4, 5].iter().map(|&o| w.refs(o)).collect();
    let mut calls = Vec::new();
    assert!(m.iterate_by_bbox(
        &mut |o| {
            calls.push(o);
            false
        },
        [0.0, -100.0, 0.0],
        [30.0, 100.0, 30.0],
        Filter::All
    ));
    assert_eq!(calls, vec![5]);
    let after: Vec<i64> = [1, 4, 5].iter().map(|&o| w.refs(o)).collect();
    assert_eq!(before, after);
    // Before Init: slots 42 and 44 true; 43, 45 and 47 false.
    let m2 = OccupantManager::new(w.clone());
    assert!(m2.iterate_by_bbox(&mut |_| true, [0.0; 3], [1.0; 3], Filter::All));
    assert!(!m2.iterate_by_bbox_typed(&mut |_| true, [0.0; 3], [1.0; 3], 0));
    assert!(!m2.iterate_by_standard_cells(&mut |_| true, None, None, Filter::All));
    assert!(!m2.iterate_by_standard_cells_typed(&mut |_| true, None, None, 0));
    assert!(m2.iterate(&mut |_| true, None, None, Filter::All));
    assert!(!m2.iterate_typed(&mut |_| true, None, None, 0));
}

// ================================================================ §18.19

/// Resolves registered objects back to their tokens, handing one reference.
struct Resolver(Vec<RegisteredObject>, Rc<World>);

impl RefResolver for Resolver {
    fn resolve(&mut self, class_id: u32, object_id: u32) -> Option<RefValue> {
        let e = self.0.iter().find(|e| e.0 == class_id && e.1 == object_id)?;
        self.1.add_ref(e.3);
        Some(RefValue::Host { token: e.3, class_id })
    }
}

fn save(m: &OccupantManager) -> (Vec<u8>, Vec<RegisteredObject>) {
    let reg = Rc::new(RefCell::new(SaveRegistry::new()));
    let mut out = ByteWriter::with_registry(reg.clone());
    assert!(m.write(&mut out));
    let e = reg.borrow().entries.clone();
    (out.into_bytes(), e)
}

#[test]
fn om18_19_save_round_trip() {
    let (w, m) = city();
    w.flat(1, 10.0, 10.0, 20.0, 20.0); // K: (64, 64)
    w.flat(2, 10.0, 10.0, 100.0, 20.0); // L: (64, 64)…(65, 64)
    assert!(m.insert(1, 0) && m.insert(2, 0));
    let (bytes, entries) = save(&m);
    assert_eq!(bytes.len(), 148_254 + 3 * 8);
    let mut head = vec![0x01, 0x00];
    head.extend_from_slice(&1024.0f32.to_le_bytes());
    head.extend_from_slice(&1024.0f32.to_le_bytes());
    for c in [16u32, 16, 64, 64, 192, 192] {
        head.extend_from_slice(&c.to_le_bytes());
    }
    assert_eq!(&bytes[..head.len()], head.as_slice());
    let m2 = OccupantManager::new(w.clone());
    let mut res = Resolver(entries.clone(), w.clone());
    assert!(m2.read(&mut ByteReader::with_resolver(&bytes, &mut res)));
    for x in 0..192 {
        for z in 0..192 {
            assert_eq!(occupants(&m2, x, z), occupants(&m, x, z));
        }
    }
    assert_eq!(occupants(&m2, 64, 64), vec![2, 1]);
    assert_eq!([w.refs(1), w.refs(2)], [2, 2], "one reference per manager");
    let s = m2.snapshot();
    assert!(s.was_read && !s.initialized);
    assert_eq!(s.counts, [16, 16, 64, 64]);
    // Bit 0 comes from the stored box when it is readable.
    assert_eq!(
        m2.cell(64, 64).unwrap()[1],
        Node {
            occupant: 1,
            single: true
        }
    );
    // Version 2: false, nothing read, the flag set.
    let mut bad = bytes.clone();
    bad[0] = 2;
    let m3 = OccupantManager::new(w.clone());
    let mut res = Resolver(entries.clone(), w.clone());
    assert!(!m3.read(&mut ByteReader::with_resolver(&bad, &mut res)));
    assert_eq!(m3.snapshot().rows, 0);
    assert!(m3.snapshot().was_read);
    // Truncated: false, the references taken are given back.
    let m4 = OccupantManager::new(w.clone());
    let mut res = Resolver(entries, w.clone());
    assert!(!m4.read(&mut ByteReader::with_resolver(&bytes[..bytes.len() - 4], &mut res)));
    assert_eq!([w.refs(1), w.refs(2)], [2, 2]);
    // Init after the load initialises the loaded occupants once each. Their
    // own Read set bit 1 of +0x2E, so the base Init calls slot 17 and posts
    // no Create (§2.2, §5.6, §13 as corrected by re-specs14b).
    for o in [1, 2] {
        w.with(o, |o| {
            o.initialized = false;
            o.loaded = true;
        });
    }
    w.take();
    assert!(m2.init());
    assert!(w.take().is_empty());
    assert_eq!(*w.slot17.borrow(), vec![2, 1]);
}

// ================================================================ §18.20

#[test]
fn om18_20_shutdown() {
    let (w, m) = city();
    w.flat(1, 10.0, 10.0, 20.0, 20.0);
    w.flat(2, 10.0, 10.0, 100.0, 20.0);
    assert!(m.insert(1, 0) && m.insert(2, 0));
    w.take();
    assert!(m.shutdown());
    assert_eq!(
        ids(&w.take()),
        vec![(msg::REMOVE, 2), (msg::DESTROY, 2), (msg::REMOVE, 1), (msg::DESTROY, 1)]
    );
    let s = m.snapshot();
    assert!(!s.initialized);
    assert_eq!(s.filter_type, None);
    assert_eq!(s.rows, 192, "the rows are kept");
    assert_eq!(s.holders, 0);
    assert!(m.shutdown());
    assert!(w.take().is_empty());
    // Slot 23 with the filter null: nothing (the original crashes).
    assert!(m.remove_occupants_of_type(5, true, 0));
    // Release of the last reference shuts down and destroys.
    let (_w2, m2) = city();
    assert_eq!(m2.add_ref(), 1);
    assert_eq!(m2.add_ref(), 2);
    assert_eq!(m2.release(), 1);
    assert_eq!(m2.release(), 0);
    let s = m2.snapshot();
    assert!(s.destroyed && !s.initialized);
}

// ================================================================ §18.21

#[test]
fn om18_21_slot34_clamp() {
    let (w, m) = city();
    w.flat(1, 1000.0, 0.0, 1020.0, 16.0); // M
    w.flat(2, 1030.0, 0.0, 1035.0, 16.0); // N, one cell past the city edge
    assert!(m.insert(1, 0) && m.insert(2, 0));
    // max (70, 0) is clamped to (63, 0): x 1008.1…1023.9 finds M, not N.
    assert_eq!(
        m.first_by_standard_cells(Some((63, 0)), Some((70, 0)), Filter::All),
        Some(1)
    );
    w.release(1);
    assert!(m.remove(1, false, 0));
    assert_eq!(
        m.first_by_standard_cells(Some((63, 0)), Some((70, 0)), Filter::All),
        None
    );
    // A null max is (count, count) = (64, 64): x up to 65·16 − 0.1 = 1039.9.
    assert_eq!(m.first_by_standard_cells(Some((63, 0)), None, Filter::All), Some(2));
    w.release(2);
    // A given min is clamped too: (−5, −5) is (0, 0), (99, 0) is (63, 0).
    assert_eq!(
        m.first_by_standard_cells(Some((99, 0)), Some((99, 0)), Filter::All),
        None
    );
    w.flat(3, 1010.0, 0.0, 1012.0, 16.0);
    assert!(m.insert(3, 0));
    assert_eq!(
        m.first_by_standard_cells(Some((99, -5)), Some((99, -5)), Filter::All),
        Some(3)
    );
    w.release(3);
    assert_eq!(m.first_by_standard_cells_typed(Some((99, 0)), None, 0x100), Some(3));
    w.release(3);
    assert_eq!([w.refs(2), w.refs(3)], [1, 1]);
}

// ================================================================ §18.22

#[test]
fn om18_22_wrapped_box() {
    let (w, m) = city();
    w.flat(1, -4100.0, 100.0, -4000.0, 120.0);
    w.take();
    assert!(m.insert(1, 0));
    // −4099.9 → cell −1, stored as 255; −4000.1 → cell 1.
    assert_eq!(w.get(1).om, Some(CellBox::new(255, 65, 1, 65)));
    assert_eq!(m.snapshot().nodes, 0, "no cell holds it");
    assert!(w.get(1).flags & FLAG_INSERTED != 0);
    assert_eq!(
        w.take(),
        vec![posted(msg::CREATE, 1, 0x100, 0), posted(msg::INSERT, 1, 0x100, 0)]
    );
    assert!(m.remove(1, false, 4));
    assert_eq!(w.take(), vec![posted(msg::REMOVE, 1, 0x100, 4)]);
    assert_eq!(w.refs(1), 0);
    assert!(w.get(1).flags & FLAG_INSERTED == 0);
}

// ================================================================ §18.23

#[test]
fn om18_23_slot44_batches() {
    let (w, m) = city();
    for o in [1, 2, 3] {
        w.flat(o, 10.0, 10.0, 20.0, 20.0); // (64, 64)
        assert!(m.insert(o, 0));
    }
    w.flat(4, 10.0, 70.0, 20.0, 80.0); // (64, 65)
    assert!(m.insert(4, 0));
    let mut seen = Vec::new();
    let mr = &m;
    assert!(m.iterate(
        &mut |o| {
            seen.push(o);
            if o != 4 {
                assert!(mr.remove(o, false, 0), "a callback removes the occupant it was given");
            }
            true
        },
        None,
        None,
        Filter::All
    ));
    assert_eq!(seen, vec![3, 2, 1, 4], "called three times, then the next cell");
    assert_eq!(m.cell(64, 64), None);
    assert_eq!([w.refs(1), w.refs(2), w.refs(3), w.refs(4)], [0, 0, 0, 1]);
    // A single-cell walk has no duplicate check; a multi-cell one skips it
    // only for bit-0 nodes: an occupant spanning two cells is called once.
    w.flat(5, 10.0, 10.0, 20.0, 80.0); // (64, 64)…(64, 65)
    assert!(m.insert(5, 0));
    let mut seen = Vec::new();
    assert!(m.iterate(
        &mut |o| {
            seen.push(o);
            true
        },
        Some([64, 0, 64]),
        Some([64, 0, 65]),
        Filter::All
    ));
    assert_eq!(seen, vec![5, 4]);
    let mut seen = Vec::new();
    assert!(m.iterate(
        &mut |o| {
            seen.push(o);
            true
        },
        Some([64, 0, 65]),
        Some([64, 0, 65]),
        Filter::All
    ));
    assert_eq!(seen, vec![5, 4], "single-cell walk: the cell's list in order");
    // Before Init: true, no cell walked.
    let m2 = OccupantManager::new(w.clone());
    let mut n = 0;
    assert!(m2.iterate(
        &mut |_| {
            n += 1;
            true
        },
        None,
        None,
        Filter::All
    ));
    assert_eq!(n, 0);
}

// ================================================================ §18.24

#[test]
fn om18_24_shrink_precision() {
    let (min, max) = shrink(100.0, 164.0);
    assert_eq!(min.to_bits(), 0x42C8_3333, "float(100.0 + 0.1f) = 100.09999847…");
    assert_eq!(max.to_bits(), 0x4323_E666, "float(164.0 − 0.1f) = 163.89999389…");
    assert_eq!(f64::from(min), 100.099_998_474_121_1);
    assert_eq!(f64::from(max), 163.899_993_896_484_38);
    assert_eq!(super::cells::SHRINK.to_bits(), 0x3DCC_CCCD);
    assert_eq!(super::cells::SHRINK_MIN_WIDTH.to_bits(), 0x3F80_0000);
    // The width is compared in float: 1.0 exactly is not shrunk.
    assert_eq!(shrink(10.0, 11.0), (10.0, 11.0));
    assert_eq!(shrink(10.0, 12.0), (10.1, 11.9));
}

// ================================================================ §14

#[test]
fn om14_lock_regions() {
    let (w, m) = city();
    w.flat(1, 10.0, 10.0, 20.0, 20.0);
    assert!(m.insert(1, 0));
    assert_eq!(w.depth.get(), 0);
    let w2 = w.clone();
    let mut depth = Vec::new();
    m.iterate_by_bbox(
        &mut |_| {
            depth.push(w2.depth.get());
            true
        },
        [0.0, -1.0, 0.0],
        [30.0, 11.0, 30.0],
        Filter::All,
    );
    assert_eq!(depth, vec![0], "slot 42 callbacks run unlocked");
    let mut depth = Vec::new();
    m.iterate(
        &mut |_| {
            depth.push(w2.depth.get());
            true
        },
        None,
        None,
        Filter::All,
    );
    assert_eq!(
        depth,
        vec![1],
        "slot 44: the walk's lock; the cell's inner hold is dropped before the callbacks (OM7)"
    );
    assert_eq!(w.depth.get(), 0);
    // Insert: the flag check and the messages are outside the lock.
    w.tracing.set(true);
    w.flat(2, 10.0, 10.0, 20.0, 20.0);
    assert!(m.insert(2, 0));
    assert_eq!(*w.trace.borrow(), vec!["lock".to_owned(), "unlock".to_owned()]);
    assert_eq!(slot_locking(42), Locking::PerCell);
    assert_eq!(slot_locking(44), Locking::WholeWalk);
    assert_eq!(slot_locking(46), Locking::Delegated);
    assert_eq!(slot_locking(25), Locking::None);
    assert_eq!(slot_locking(20), Locking::WholeCall);
}

#[test]
fn om14_callbacks_may_change_the_grid() {
    let (w, m) = city();
    for o in [1, 2, 3] {
        w.flat(o, 10.0, 10.0, 20.0, 20.0);
        assert!(m.insert(o, 0));
    }
    // Slot 44: a callback removing the visited node and a later one. The
    // cell's batch holds references, so the removed later one is still
    // called (OM7), then released.
    let mut seen = Vec::new();
    let mr = &m;
    assert!(m.iterate(
        &mut |o| {
            seen.push(o);
            if o == 3 {
                assert!(mr.remove(3, false, 0));
                assert!(mr.remove(1, false, 0));
            }
            true
        },
        None,
        None,
        Filter::All
    ));
    assert_eq!(seen, vec![3, 2, 1]);
    assert_eq!(occupants(&m, 64, 64), vec![2]);
    assert_eq!([w.refs(1), w.refs(3)], [0, 0]);
    // Slot 42: an occupant removed after it was collected is still called.
    w.flat(4, 10.0, 10.0, 20.0, 20.0);
    assert!(m.insert(4, 0));
    let mut seen = Vec::new();
    m.iterate_by_bbox(
        &mut |o| {
            seen.push(o);
            if o == 4 {
                assert!(mr.remove(2, false, 0));
            }
            true
        },
        [0.0, -1.0, 0.0],
        [30.0, 11.0, 30.0],
        Filter::All,
    );
    assert_eq!(seen, vec![4, 2]);
    assert_eq!(w.refs(2), 0);
}

#[test]
fn identity_and_edges() {
    assert_eq!(query_interface(1), Some(Interface::OccupantManager));
    assert_eq!(query_interface(IID_OCCUPANT_MANAGER), Some(Interface::OccupantManager));
    assert_eq!(query_interface(IID_SERIALIZABLE), Some(Interface::Serializable));
    assert_eq!(query_interface(IID_MESSAGE_TARGET2), Some(Interface::MessageTarget2));
    assert_eq!(query_interface(IID_OCCUPANT_FILTER), None);
    let w = World::new();
    let m = OccupantManager::new(w.clone());
    assert_eq!(m.class_id(), CLSID_OCCUPANT_MANAGER);
    // Queries before Init.
    let mut l = Vec::new();
    assert!(!m.occupants_by_bbox(&mut l, None, None, Filter::All, -1));
    assert!(!m.occupants_by_cells_typed(&mut l, None, None, 0, -1));
    assert_eq!(m.first_in_cell(64, 64, Filter::All), None);
    assert!(m.is_cell_empty(64, 64));
    assert!(m.remove_occupants(Filter::All, true, 0));
    // Insert before Init: no grid, no node, but it succeeds [choice].
    w.flat(1, 10.0, 10.0, 20.0, 20.0);
    assert!(m.insert(1, 0));
    assert_eq!(m.snapshot().nodes, 0);
    // Insert beyond the grid: bytes, no clamp; cells past 191 are skipped.
    let (w, m) = city();
    w.flat(2, 8000.0, 10.0, 8300.0, 20.0);
    assert!(m.insert(2, 0));
    assert_eq!(w.get(2).om, Some(CellBox::new(189, 64, 193, 64)));
    assert_eq!(m.snapshot().nodes, 3);
    // Move clamps its new cells: one node per new cell, all in 191.
    w.with(2, |o| {
        o.bbox.min[0] = 8100.0;
        o.bbox.max[0] = 8400.0;
    });
    assert!(m.move_occupant(2, false));
    assert_eq!(w.get(2).om, Some(CellBox::new(190, 64, 195, 64)));
    assert_eq!(occupants(&m, 191, 64), vec![2, 2, 2]);
    assert!(m.cell(189, 64).is_none());
}

/// glue25fix3 g5: a base texture over standard cell (3, 32) is found by slot 33.
#[test]
fn glue25fix3_first_by_standard_cell_typed_game_box() {
    let (w, m) = city();
    w.add(1, 0x497F_6D9D, [0.1, 0.0, 512.1], [63.9, 0.0, 575.9]);
    assert!(m.insert(1, 0));
    let mut l = Vec::new();
    assert!(m.occupants_by_standard_cells(&mut l, Some((3, 32)), Some((3, 32)), Filter::All, -1));
    assert_eq!(l, vec![1]);
    assert_eq!(m.first_by_standard_cell_typed(3, 32, 0x497F_6D9D), Some(1));
}
