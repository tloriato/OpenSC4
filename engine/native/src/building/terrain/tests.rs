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
    allocations: Vec<Object>,
    lots: Vec<usize>,
    height: f64,
    slope: f64,
    cost: i64,
    funds: u64,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(name: &'static str, words: &[u32]) {
    STATE.with(|s| s.borrow_mut().events.push(Event(name, words.to_vec())));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
fn memory(size: usize) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut object = Object::new(size);
        let p = object.ptr();
        unsafe { core::ptr::write_bytes(p, 0xA5, size) };
        s.allocations.push(object);
        p
    })
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log("allocate", &[size]);
    memory(size as usize)
}
extern "C" fn free(header: *mut u8) -> u64 {
    log("free", &[]);
    unsafe {
        assert_eq!(read(header.add(8).cast::<u32>()), 0xA5A5_A5A5);
    }
    0x9876_5432_AABB_CCDD
}
extern "thiscall" fn list_new(out: *mut *mut u8, _cookie: *mut u8) -> u64 {
    log("list new", &[]);
    let header = memory(12);
    unsafe {
        write(header.cast::<*mut u8>(), header);
        write(header.add(4).cast::<*mut u8>(), header);
        write(out, header);
    }
    0
}
extern "thiscall" fn list_clear(out: *mut *mut u8) -> u64 {
    let mut ids = vec![];
    unsafe {
        let header = read(out);
        let mut p = read(header.cast::<*mut u8>());
        while p != header {
            ids.push(read(p.add(8).cast::<u32>()));
            p = read(p.cast());
        }
        write(header.cast::<*mut u8>(), header);
        write(header.add(4).cast::<*mut u8>(), header);
    }
    log("clear", &ids);
    0
}
extern "thiscall" fn get_lots(_this: *mut u8, out: *mut *mut u8, x0: i32, z0: i32, x1: i32, z1: i32, flag: u32) -> u32 {
    log("get lots", &[x0 as u32, z0 as u32, x1 as u32, z1 as u32, flag]);
    let lots = STATE.with(|s| s.borrow().lots.clone());
    unsafe {
        let header = read(out);
        let mut previous = header;
        for lot in lots {
            let node = memory(12);
            write(node.add(8).cast::<usize>(), lot);
            write(node.add(4).cast::<*mut u8>(), previous);
            write(previous.cast::<*mut u8>(), node);
            previous = node;
        }
        write(previous.cast::<*mut u8>(), header);
        write(header.add(4).cast::<*mut u8>(), previous);
    }
    0
}
extern "thiscall" fn region_new(this: *mut u8, rect: *mut i32, convert: u32) -> u32 {
    let mut args = unsafe { core::slice::from_raw_parts(rect.cast::<u32>(), 4) }.to_vec();
    args.push(convert);
    log("region", &args);
    unsafe { core::ptr::write_bytes(this, 0x5A, 36) };
    0
}
extern "thiscall" fn region_end(this: *mut u8) -> u32 {
    assert_eq!(unsafe { read(this.cast::<u32>()) }, 0x00A8_6364);
    log("region end", &[]);
    0
}
#[repr(C)]
struct Lot {
    table: *const usize,
    center: [f32; 3],
    slope: f64,
    demolishable: u32,
}
extern "thiscall" fn center(this: *mut Lot, out: *mut f32) {
    log("center", &[this as u32]);
    unsafe { core::ptr::copy_nonoverlapping((*this).center.as_ptr(), out, 3) };
}
extern "thiscall" fn bounds(this: *mut Lot, out: *mut i32) {
    log("bounds", &[this as u32]);
    unsafe { core::ptr::copy_nonoverlapping([2, 3, 4, 5].as_ptr(), out, 4) };
}
extern "thiscall" fn old_slope(this: *mut Lot) -> f64 {
    log("old slope", &[this as u32]);
    unsafe { (*this).slope }
}
extern "thiscall" fn demolishable(this: *mut Lot) -> u32 {
    log("state", &[this as u32]);
    unsafe { (*this).demolishable }
}
extern "thiscall" fn addref(this: *mut u8) -> u32 {
    log("addref", &[this as u32]);
    0
}
extern "thiscall" fn release(this: *mut u8) -> u32 {
    log("release", &[this as u32]);
    0
}
extern "thiscall" fn height(_this: *mut u8, x: f32, z: f32) -> f64 {
    log("height", &[x.to_bits(), z.to_bits()]);
    STATE.with(|s| s.borrow().height)
}
extern "thiscall" fn new_slope(_this: *mut u8, lot: *mut u8, flag: u32) -> f64 {
    log("new slope", &[lot as u32, flag]);
    STATE.with(|s| s.borrow().slope)
}
extern "thiscall" fn width(_this: *mut u8) -> f64 {
    log("width", &[]);
    16.0
}
extern "thiscall" fn update(_this: *mut u8, lot: *mut u8, mode: u32) -> u32 {
    log("update", &[lot as u32, mode]);
    0
}
extern "thiscall" fn zone(
    _this: *mut u8,
    _region: *mut u8,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    f: u32,
    g: u32,
    h: u32,
    i: u32,
) -> u32 {
    log("zone", &[a, b, c, d, e, f, g, h, i]);
    0
}
extern "thiscall" fn demolish(
    _this: *mut u8,
    a: u32,
    list: *mut *mut u8,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    cost: *mut i64,
    f: u32,
) -> u32 {
    let mut args = vec![a, b, c, d, e, f];
    unsafe {
        assert_eq!(read(cost), 0);
        write(cost, STATE.with(|s| s.borrow().cost));
        let header = read(list);
        let mut node = read(header.cast::<*mut u8>());
        while node != header {
            args.push(read(node.add(8).cast::<u32>()));
            node = read(node.cast());
        }
    }
    log("demolish", &args);
    0
}
extern "thiscall" fn funds(_this: *mut u8) -> u64 {
    log("funds", &[]);
    STATE.with(|s| s.borrow().funds)
}
extern "thiscall" fn set_funds(_this: *mut u8, low: u32, high: u32) -> u32 {
    log("set funds", &[low, high]);
    0
}

struct World {
    sim: Object,
    _objects: Vec<Object>,
    _tables: Vec<Box<[usize]>>,
    lots: Vec<Lot>,
    _guards: Vec<Fake>,
    constants: Box<[f32; 4]>,
    globals: Box<[*mut u8; 2]>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                height: 100.0,
                ..State::default()
            }
        });
        let mut w = Self {
            sim: Object::new(0xE4),
            _objects: vec![],
            _tables: vec![],
            lots: vec![],
            _guards: vec![],
            constants: Box::new([-1.0, 2.0, 1.0, f32::from_bits(0x3DB2_B8C3)]),
            globals: Box::new([core::ptr::null_mut(); 2]),
        };
        let lot_manager = w.interface(&[(0x1C, get_lots as *const ())]);
        let lot_developer = w.interface(&[(0x94, update as *const ()), (0x78, new_slope as *const ())]);
        let terrain = w.interface(&[(0x34, height as *const ())]);
        let city = w.interface(&[(0x16C, width as *const ())]);
        let zones = w.interface(&[(0x2C, zone as *const ())]);
        w.globals[0] = w.interface(&[(0x1C, demolish as *const ())]);
        w.globals[1] = w.interface(&[(0x18, funds as *const ()), (0x14, set_funds as *const ())]);
        let sim = unsafe { BuildingSim::from_ptr(w.sim.ptr()) };
        sim.set_lots(lot_manager);
        sim.set_lot_developer(lot_developer);
        sim.set_terrain(terrain);
        sim.set_city(city);
        sim.set_zones(zones);
        w._guards.extend([
            fake(0x0090_CF54, allocate as *const () as usize),
            fake(0x0090_CF63, free as *const () as usize),
            fake(0x004F_5A00, list_new as *const () as usize),
            fake(0x0047_3EC0, list_clear as *const () as usize),
            fake(0x0044_2640, region_new as *const () as usize),
            fake(0x0044_1A60, region_end as *const () as usize),
            fake_data(0x00B4_3D00, w.globals.as_mut_ptr().cast()),
            fake_data(0x00B4_3D64, w.globals.as_mut_ptr().wrapping_add(1).cast()),
        ]);
        for (i, addr) in [0x00A8_FE78, 0x00A8_825C, 0x00A8_1228, 0x00AB_0ABC]
            .into_iter()
            .enumerate()
        {
            w._guards
                .push(fake_data(addr, w.constants.as_mut_ptr().wrapping_add(i).cast()));
        }
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
        self._objects.push(object);
        self._tables.push(table);
        p
    }
    fn lots(&mut self, count: usize, state: u32) {
        let pointer = self.interface(&[
            (4, addref as *const ()),
            (8, release as *const ()),
            (0x38, bounds as *const ()),
            (0x3C, center as *const ()),
            (0x4C, old_slope as *const ()),
            (0x9C, demolishable as *const ()),
        ]);
        let table = unsafe { read(pointer.cast::<*const usize>()) };
        self.lots = (0..count)
            .map(|_| Lot {
                table,
                center: [48.0, 100.0, 64.0],
                slope: 0.0,
                demolishable: state,
            })
            .collect();
        let pointers = self.lots.iter_mut().map(|lot| lot as *mut Lot as usize).collect();
        STATE.with(|s| s.borrow_mut().lots = pointers);
    }
    fn run(&mut self, kind: u32) -> u64 {
        unsafe {
            terrain_changed(
                self.sim.ptr(),
                0xDEAD_BEEF,
                [1, 2, 3, 4].as_mut_ptr(),
                kind,
                0xAABB_CCDD,
            )
        }
    }
}

#[test]
fn empty_rectangles_skip_all_work_and_ordinary_kinds_keep_the_full_word() {
    let mut w = World::new();
    for rect in [[1, 9, 3, 4], [5, 2, 3, 4]] {
        let args = [rect.as_ptr() as u32, 1];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_00698610 as *const () as usize,
            w.sim.ptr() as u32,
            &args,
        );
        call.edx = 0xDEAD_BEEF;
        unsafe { call_raw(&mut call) };
        assert_eq!((call.eax, call.edx), (rect[1] as u32, 0xDEAD_BEEF));
        assert!(events().is_empty());
    }
    w.lots(2, 0);
    for kind in [0, 4, 5, 0x1005, u32::MAX] {
        assert_eq!(w.run(kind), 0x9876_5432_AABB_CCDD);
        let calls = events();
        assert_eq!(
            calls.iter().map(|e| e.0).collect::<Vec<_>>(),
            ["list new", "get lots", "update", "update", "clear", "free"]
        );
        assert_eq!(calls[2].1, [&mut w.lots[0] as *mut Lot as u32, kind]);
        assert_eq!(calls[3].1, [&mut w.lots[1] as *mut Lot as u32, kind]);
    }
}

#[test]
fn terraforming_demolishes_even_empty_lists_and_charges_only_positive_cost_in_modes_one_and_two() {
    for mode in [1, 2, 3] {
        for cost in [0i64, -1, 0x1_0000_0001] {
            let mut w = World::new();
            STATE.with(|s| {
                s.borrow_mut().cost = cost;
                s.borrow_mut().funds = 0;
            });
            assert_eq!(w.run(mode), 0x9876_5432_AABB_CCDD);
            let calls = events();
            let mut expected = vec!["allocate", "allocate", "get lots", "region", "demolish"];
            if mode != 3 && cost > 0 {
                expected.extend(["funds", "set funds"]);
            }
            expected.extend(["region end", "clear", "free", "clear", "free"]);
            assert_eq!(calls.iter().map(|e| e.0).collect::<Vec<_>>(), expected);
            assert_eq!(calls[4].1, [1, 0, 1, 1, 0, 0]);
            if mode != 3 && cost > 0 {
                assert_eq!(
                    calls.iter().find(|e| e.0 == "set funds").unwrap().1,
                    [u32::MAX, 0xFFFF_FFFE]
                );
            }
        }
    }
}

#[test]
fn altitude_and_slope_boundaries_choose_updates_or_refcounted_demolition() {
    let _fpu = GameFpu::new();
    for (height, old, new, update) in [
        (99.0, 0.0, 0.0, true),
        (102.0, 0.0, 0.0, true),
        (98.99, 0.0, 0.0, false),
        (102.01, 0.0, 0.0, false),
        (f64::NAN, 0.0, 0.0, true),
        (f64::INFINITY, 0.0, 0.0, false),
        (100.0, 0.0, 1.399, true),
        (100.0, 0.0, 1.4, false),
        (100.0, 2.0, 0.0, true),
        (100.0, 0.0, f64::NAN, true),
    ] {
        let mut w = World::new();
        w.lots(1, 0x1001);
        w.lots[0].slope = old;
        STATE.with(|s| {
            s.borrow_mut().height = height;
            s.borrow_mut().slope = new;
        });
        let before = unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0xE4) }.to_vec();
        w.run(2);
        let calls = events();
        assert_eq!(calls.iter().any(|e| e.0 == "update"), update);
        assert_eq!(
            calls.iter().filter(|e| e.0 == "addref").count(),
            if update { 0 } else { 2 }
        );
        assert_eq!(calls.iter().filter(|e| e.0 == "release").count(), usize::from(!update));
        assert_eq!(
            calls.iter().find(|e| e.0 == "demolish").unwrap().1.len(),
            if update { 6 } else { 7 }
        );
        assert_eq!(unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0xE4) }, before);
    }
}

#[test]
fn non_demolishable_lots_dezone_with_raw_flag_word_and_destroy_the_region() {
    let _fpu = GameFpu::new();
    for mode in [1, 2, 3] {
        let mut w = World::new();
        w.lots(1, 0x100);
        STATE.with(|s| s.borrow_mut().height = 110.0);
        w.run(mode);
        let calls = events();
        assert!(!calls.iter().any(|e| e.0 == "addref"));
        assert_eq!(calls.iter().filter(|e| e.0 == "bounds").count(), 2);
        assert_eq!(
            calls.iter().find(|e| e.0 == "zone").unwrap().1,
            [0, 1, 0xAABB_CC00 | u32::from(mode != 3), 0, 1, 1, 0, 0, 0]
        );
        assert_eq!(calls.iter().filter(|e| e.0 == "region end").count(), 2);
    }
}

#[test]
fn terrain_export_preserves_the_raw_zoning_flag_padding() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.lots(1, 0);
    STATE.with(|s| s.borrow_mut().height = 110.0);
    let mut rect = [1i32, 2, 3, 4];
    // SAFETY: the entry pops the two pushed words; the seed is the original local flag word.
    unsafe {
        core::arch::asm!(
            "push 2", "push {rect}", "mov dword ptr [esp - 140], 0x11223344", "call {entry}",
            rect = in(reg) rect.as_mut_ptr(), entry = sym fc4p_00698610,
            in("ecx") w.sim.ptr(), clobber_abi("C"),
        );
    }
    assert_eq!(events().iter().find(|e| e.0 == "zone").unwrap().1[2], 0x1122_3301);
}
