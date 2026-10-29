// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Event(&'static str, Vec<u32>);
#[derive(Default)]
struct State {
    events: Vec<Event>,
    profile: usize,
    grid: usize,
    configuration: usize,
    building: usize,
    occupant: usize,
    system: usize,
    advisor: usize,
    factory: usize,
    subject: usize,
    replacement_table: usize,
    stages: [usize; 2],
    stage_calls: usize,
    stage: u32,
    bounds: [i32; 4],
    mutate_table: bool,
    mutate_slot: bool,
    location_slot: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(name: &'static str, words: &[u32]) {
    STATE.with(|s| s.borrow_mut().events.push(Event(name, words.to_vec())));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn kind(this: *mut u8, flag: u32) -> u32 {
    log("kind", &[this as u32, flag]);
    0x100
}
extern "thiscall" fn profile(this: *mut u8, key: *mut u32) -> *mut u8 {
    log("profile", &[this as u32, unsafe { read(key) }]);
    STATE.with(|s| s.borrow().profile as *mut u8)
}
extern "thiscall" fn bounds(this: *mut u8, out: *mut i32) {
    log("bounds", &[this as u32]);
    let bounds = STATE.with(|s| s.borrow().bounds);
    unsafe { core::ptr::copy_nonoverlapping(bounds.as_ptr(), out, 4) };
}
extern "thiscall" fn grid(this: *mut u8) -> *mut u8 {
    log("grid", &[this as u32]);
    STATE.with(|s| s.borrow().grid as *mut u8)
}
extern "thiscall" fn configuration(this: *mut u8) -> *mut u8 {
    log("configuration", &[this as u32]);
    STATE.with(|s| s.borrow().configuration as *mut u8)
}
extern "thiscall" fn building(this: *mut u8) -> *mut u8 {
    log("building", &[this as u32]);
    STATE.with(|s| s.borrow().building as *mut u8)
}
extern "C" fn developer_type(purpose: u32, wealth: u32) -> u32 {
    log("developer type", &[purpose, wealth]);
    0x1010
}
extern "thiscall" fn stage(this: *mut u8) -> u32 {
    log("stage", &[this as u32]);
    STATE.with(|s| s.borrow().stage)
}
extern "thiscall" fn highest(this: *mut u8, key: *mut u32) -> *mut u8 {
    log("highest", &[this as u32, unsafe { read(key) }]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.stages[usize::from(s.stage_calls != 0)];
        s.stage_calls += 1;
        p as *mut u8
    })
}
extern "thiscall" fn occupant(this: *mut u8) -> *mut u8 {
    log("occupant", &[this as u32]);
    STATE.with(|s| s.borrow().occupant as *mut u8)
}
extern "thiscall" fn position(this: *mut u8, out: *mut f32) {
    log("position", &[this as u32]);
    unsafe { core::ptr::copy_nonoverlapping([48.0f32, 100.0, 64.0].as_ptr(), out, 3) };
}
extern "thiscall" fn system(this: *mut u8) -> *mut u8 {
    log("system", &[this as u32]);
    STATE.with(|s| s.borrow().system as *mut u8)
}
extern "thiscall" fn advisor(this: *mut u8, id: u32) -> *mut u8 {
    log("advisor", &[this as u32, id]);
    STATE.with(|s| s.borrow().advisor as *mut u8)
}
extern "C" fn gzcom() -> *mut u8 {
    log("gzcom", &[]);
    STATE.with(|s| s.borrow().factory as *mut u8)
}
extern "thiscall" fn create(this: *mut u8, class: u32, iid: u32, out: *mut *mut u8) -> u32 {
    log("create", &[this as u32, class, iid]);
    assert!(unsafe { read(out) }.is_null());
    unsafe { write(out, STATE.with(|s| s.borrow().subject as *mut u8)) };
    0xFFFF_FF00
}
extern "thiscall" fn width_z(this: *mut u8) -> f64 {
    log("width z", &[this as u32]);
    STATE.with(|s| {
        let s = s.borrow();
        if s.mutate_table {
            unsafe { write(s.subject as *mut usize, s.replacement_table) };
        }
        if s.mutate_slot {
            unsafe { write(s.location_slot as *mut usize, replacement as *const () as usize) };
        }
    });
    32.000_001_907_348_64
}
extern "thiscall" fn width_x(this: *mut u8) -> f64 {
    log("width x", &[this as u32]);
    16.000_000_953_674_32
}
extern "thiscall" fn location(this: *mut u8, x: f32, z: f32) -> u32 {
    log("location", &[this as u32, x.to_bits(), z.to_bits()]);
    0
}
extern "thiscall" fn replacement(this: *mut u8, _x: f32, _z: f32) -> u32 {
    log("replacement location", &[this as u32]);
    0
}
extern "thiscall" fn set_object(this: *mut u8, lot: *mut u8) -> u32 {
    log("object", &[this as u32, lot as u32]);
    0
}
extern "thiscall" fn set_data(this: *mut u8, word: u32) -> u32 {
    log("data", &[this as u32, word]);
    0
}
extern "thiscall" fn push(this: *mut u8, id: u32, subject: *mut u8) -> u32 {
    log("push", &[this as u32, id, subject as u32]);
    0
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    log("release", &[this as u32]);
    0x1234_5678_AABB_CCDD
}

struct World {
    sim: Object,
    lot: *mut u8,
    objects: Vec<Object>,
    tables: Vec<Box<[usize]>>,
    cells: Vec<[u8; 4]>,
    rows: Vec<[usize; 3]>,
    data: Box<usize>,
    profile: Box<[u32; 2]>,
    stages: Box<[u8; 4]>,
    city: Box<*mut u8>,
    _guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                stage: 0xAABB_CC03,
                bounds: [0, 1, 1, 2],
                ..State::default()
            }
        });
        let mut w = Self {
            sim: Object::new(0xE4),
            lot: core::ptr::null_mut(),
            objects: vec![],
            tables: vec![],
            cells: vec![[0xA5; 4]; 3],
            rows: vec![[0; 3]; 3],
            data: Box::new(0),
            profile: Box::new([1, 1]),
            stages: Box::new([2, 0xA5, 0xA5, 0xA5]),
            city: Box::new(core::ptr::null_mut()),
            _guards: vec![],
        };
        for i in 0..3 {
            w.rows[i][0] = w.cells[i].as_mut_ptr() as usize;
            w.rows[i][1] = 0x1122_3344;
            w.rows[i][2] = 0x5566_7788;
        }
        *w.data = w.rows.as_mut_ptr() as usize;
        w.lot = w.interface(&[
            (0xB8, kind as *const ()),
            (0x38, bounds as *const ()),
            (0x24, configuration as *const ()),
            (0xC0, building as *const ()),
        ]);
        let map = w.interface(&[(0x60, grid as *const ())]);
        let configuration = w.interface(&[(0x2C, stage as *const ())]);
        let building = w.interface(&[(0x0C, occupant as *const ())]);
        let occupant = w.interface(&[(0x20, position as *const ())]);
        *w.city = w.interface(&[
            (0x144, system as *const ()),
            (0x170, width_z as *const ()),
            (0x16C, width_x as *const ()),
        ]);
        let system = w.interface(&[(0x1C, advisor as *const ())]);
        let advisor = w.interface(&[(0x48, push as *const ())]);
        let factory = w.interface(&[(0x0C, create as *const ())]);
        let subject = w.interface(&[
            (8, release as *const ()),
            (0x0C, location as *const ()),
            (0x1C, set_object as *const ()),
            (0x28, set_data as *const ()),
        ]);
        let replacement = w.interface(&[
            (8, release as *const ()),
            (0x0C, replacement as *const ()),
            (0x1C, set_object as *const ()),
            (0x28, set_data as *const ()),
        ]);
        unsafe { BuildingSim::from_ptr(w.sim.ptr()) }.set_building_map(map);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.profile = w.profile.as_mut_ptr() as usize;
            s.grid = &mut *w.data as *mut usize as usize;
            s.configuration = configuration as usize;
            s.building = building as usize;
            s.occupant = occupant as usize;
            s.system = system as usize;
            s.advisor = advisor as usize;
            s.factory = factory as usize;
            s.subject = subject as usize;
            s.replacement_table = unsafe { read(replacement.cast::<usize>()) };
            s.location_slot = unsafe { read(subject.cast::<usize>()) }.wrapping_add(0x0C);
            s.stages = [
                w.stages.as_mut_ptr() as usize,
                w.stages.as_mut_ptr().wrapping_add(1) as usize,
            ];
        });
        w._guards.extend([
            fake(0x0069_B720, profile as *const () as usize),
            fake(0x0069_5790, developer_type as *const () as usize),
            fake(0x006C_0F30, highest as *const () as usize),
            fake(0x0090_DDF1, gzcom as *const () as usize),
            fake_data(0x00B4_8584, w.profile.as_mut_ptr().cast()),
            fake_data(0x00B4_3CEC, (&mut *w.city as *mut *mut u8).cast()),
        ]);
        w
    }
    fn interface(&mut self, slots: &[(usize, *const ())]) -> *mut u8 {
        let mut table = vec![0usize; 100].into_boxed_slice();
        for &(slot, function) in slots {
            table[slot / 4] = function as usize;
        }
        let mut object = Object::new(4);
        let p = object.ptr();
        unsafe { write(p.cast::<*const usize>(), table.as_ptr()) };
        self.objects.push(object);
        self.tables.push(table);
        p
    }
    fn run(&mut self, wealth: u32, built: u32) -> u64 {
        unsafe { fc4p_0069ca80(self.sim.ptr(), self.lot, wealth, built) }
    }
}

#[test]
fn building_map_uses_wrapping_bytes_inclusive_noncontiguous_rows_and_no_advice_on_mismatch() {
    for (purpose, wealth, built, byte) in [(1, 2, 1, 7), (0x40, 0xFF, 1, 0), (1, 0x101, 0x100, 0), (0, 1, 0x80, 2)] {
        let mut w = World::new();
        w.profile[0] = purpose;
        w.profile[1] = 3;
        w.run(wealth, built);
        assert_eq!(w.cells, [[0xA5, byte, byte, 0xA5], [0xA5, byte, byte, 0xA5], [0xA5; 4]]);
        assert!(w.rows.iter().all(|row| row[1..] == [0x1122_3344, 0x5566_7788]));
        assert_eq!(
            events().iter().map(|e| e.0).collect::<Vec<_>>(),
            ["kind", "profile", "bounds", "grid"]
        );
    }
    for rect in [[2, 1, 1, 2], [0, 3, 1, 2]] {
        let mut w = World::new();
        STATE.with(|s| s.borrow_mut().bounds = rect);
        w.run(1, 0);
        assert_eq!(w.cells, [[0xA5; 4]; 3]);
        events();
    }
}

#[test]
fn unchanged_stage_and_missing_lot_objects_stop_before_subject_creation() {
    for missing in [0, 1, 2] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if missing == 1 {
                s.configuration = 0;
            }
            if missing == 2 {
                s.building = 0;
            }
            s.stage = 2;
        });
        w.run(1, 1);
        let calls = events();
        assert!(calls.iter().any(|e| e.0 == "configuration"));
        assert!(calls.iter().any(|e| e.0 == "building"));
        assert_eq!(
            calls.iter().filter(|e| e.0 == "highest").count(),
            usize::from(missing == 0)
        );
        assert!(!calls.iter().any(|e| e.0 == "gzcom"));
        assert_eq!(*w.stages, [2, 0xA5, 0xA5, 0xA5]);
    }
}

#[test]
fn new_stage_sets_second_lookup_subject_location_object_data_and_null_event_then_releases() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().mutate_table = true);
    let before = unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0xE4) }.to_vec();
    assert_eq!(w.run(1, 1), 0x1234_5678_AABB_CCDD);
    assert_eq!(*w.stages, [2, 3, 0xA5, 0xA5]);
    let calls = events();
    assert_eq!(
        calls.iter().map(|e| e.0).collect::<Vec<_>>(),
        [
            "kind",
            "profile",
            "bounds",
            "grid",
            "configuration",
            "building",
            "developer type",
            "stage",
            "highest",
            "highest",
            "occupant",
            "position",
            "system",
            "advisor",
            "gzcom",
            "create",
            "width z",
            "width x",
            "location",
            "object",
            "data",
            "push",
            "release"
        ]
    );
    let subject = STATE.with(|s| s.borrow().subject as u32);
    assert_eq!(calls[6].1, [1, 1]);
    assert_eq!(calls[8].1, [w.sim.ptr().wrapping_add(0x50) as u32, 0x1010]);
    assert_eq!(calls[18].1, [subject, 0x403F_FFFF, 0x3FFF_FFFF]);
    assert_eq!(calls[19].1, [subject, w.lot as u32]);
    assert_eq!(calls[20].1, [subject, 0x0300_1010]);
    assert_eq!(calls[21].1[1..], [0x4A64_A9CC, 0]);
    assert_eq!(calls[22].1, [subject]);
    assert_eq!(unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0xE4) }, before);
}

#[test]
fn location_slot_is_read_after_cell_width_calls_from_the_cached_table() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().mutate_slot = true);
    w.run(1, 1);
    let calls = events();
    assert!(calls.iter().any(|e| e.0 == "replacement location"));
    assert!(!calls.iter().any(|e| e.0 == "location"));
}
