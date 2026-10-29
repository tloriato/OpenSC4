// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Parcel geometry and depth-rule vectors moved from the core's zone tests.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Allocate(usize),
    Attempt([i32; 5]),
    Remnants([i32; 5]),
    Street([i32; 4], u32),
    Width,
    Height,
    Level([i32; 4]),
    Lock(usize, u32),
    Release(usize),
    Free(usize),
}

#[derive(Default)]
struct State {
    dev: usize,
    city: usize,
    level: usize,
    lot_table: usize,
    real: bool,
    calls: Vec<Call>,
    street_words: Vec<u32>,
    failed: Vec<[i32; 5]>,
    nodes: Vec<Box<[u32]>>,
    lots: Vec<Box<[usize]>>,
    width: i32,
    height: i32,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "C" fn allocate(size: u32) -> *mut u8 {
    assert_eq!(size, 12);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = s.nodes.len();
        s.calls.push(Call::Allocate(id));
        s.nodes.push(vec![0xA5A5_A5A5; 3].into_boxed_slice());
        s.nodes[id].as_mut_ptr().cast()
    })
}

extern "C" fn free(p: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = s.nodes.iter().position(|n| n.as_ptr().cast::<u8>() == p).unwrap();
        if id == 0 {
            assert_eq!(&s.nodes[0][..2], &[p as u32; 2]);
            assert_eq!(s.nodes[0][2], 0xA5A5_A5A5);
        }
        s.calls.push(Call::Free(id));
    });
}

unsafe extern "thiscall" fn attempt(
    this: *mut u8,
    x: i32,
    z: i32,
    w: i32,
    h: i32,
    f: i32,
    out: *mut *mut u8,
    zone: u32,
    water: u32,
    cfg: u32,
) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev + 4);
        assert_eq!((zone, water, cfg), (0, 0, 0));
        assert!(unsafe { *out }.is_null());
        let args = [x, z, w, h, f];
        s.calls.push(Call::Attempt(args));
        if s.failed.contains(&args) {
            return 0xAABB_CC00;
        }
        if s.real {
            let id = s.lots.len();
            let table = s.lot_table;
            s.lots.push(vec![table, id, 0].into_boxed_slice());
            unsafe {
                *out = s.lots[id].as_mut_ptr().cast();
            }
        }
        0xAABB_CC01
    })
}

extern "thiscall" fn remnants(this: *mut u8, x: i32, z: i32, w: i32, h: i32, f: i32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Remnants([x, z, w, h, f]));
    });
}

extern "thiscall" fn street(this: *mut u8, x: i32, z: i32, ex: i32, ez: i32, per_cell: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Street([x, z, ex, ez], per_cell & 0xFF));
        s.street_words.push(per_cell);
    });
}

extern "thiscall" fn width(this: *mut u8) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.city);
        s.calls.push(Call::Width);
        s.width
    })
}

extern "thiscall" fn height(this: *mut u8) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.city);
        s.calls.push(Call::Height);
        s.height
    })
}

unsafe extern "thiscall" fn level(this: *mut u8, rect: *const [i32; 4], zero: u32, a: u32, b: u32, max: u32, lot: u32) {
    assert_eq!((zero, a, b, max, lot), (0, 1, 1, 0x7F7F_FFFF, 0));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.level);
        s.calls.push(Call::Level(unsafe { *rect }));
    });
}

unsafe extern "thiscall" fn lock(this: *mut usize, value: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = unsafe { *this.add(1) };
        assert_eq!(this, s.lots[id].as_mut_ptr());
        assert_ne!(unsafe { *this.add(2) }, value as usize);
        unsafe {
            *this.add(2) = value as usize;
        }
        s.calls.push(Call::Lock(id, value));
    });
}

unsafe extern "thiscall" fn release(this: *mut usize) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = unsafe { *this.add(1) };
        assert_eq!(unsafe { *this.add(2) }, 0);
        s.calls.push(Call::Release(id));
    });
}

struct World {
    dev: Object,
    bounds: [i32; 4],
}

#[unsafe(naked)]
unsafe extern "C" fn seeded_entry() {
    core::arch::naked_asm!("mov dword ptr [esp - 0x20], 0xDDEEFF55",
        "mov dword ptr [esp - 0x10], 0xAABBCC66", "jmp {port}", port = sym fc4p_0072edd0);
}

impl World {
    fn new(bounds: [i32; 4], q: u32, d: u32, conditions: [i32; 4], streets: bool) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                real: true,
                width: 64,
                height: 64,
                ..State::default()
            }
        });
        let mut dev = Object::new(0x100);
        unsafe {
            let view = ZoneDeveloper::from_ptr(dev.ptr());
            view.set_orientation_q(q);
            view.set_orientation_d(d);
            view.set_place_streets(u8::from(streets));
            view.set_real_mode(1);
            let settings = ParcelSettings::from_ptr(dev.ptr());
            settings.set_width(2);
            settings.set_depth(2);
            settings.set_minimum(1);
            settings.set_maximum(3);
            settings.set_block(6);
            for (i, c) in conditions.into_iter().enumerate() {
                *dev.ptr().add(0xBC + i * 4).cast::<i32>() = c;
            }
        }
        Self { dev, bounds }
    }

    fn run(&mut self) -> Vec<Call> {
        let mut manager_vt = [0usize; 23];
        manager_vt[0x58 / 4] = attempt as *const () as usize;
        let mut city_vt = [0usize; 95];
        city_vt[0x174 / 4] = width as *const () as usize;
        city_vt[0x178 / 4] = height as *const () as usize;
        let mut city = [city_vt.as_ptr() as usize];
        let mut level_vt = [0usize; 22];
        level_vt[0x54 / 4] = level as *const () as usize;
        let mut leveller = [level_vt.as_ptr() as usize];
        let mut lot_vt = [0usize; 70];
        lot_vt[0x114 / 4] = lock as *const () as usize;
        lot_vt[2] = release as *const () as usize;
        unsafe {
            let dev = ZoneDeveloper::from_ptr(self.dev.ptr());
            dev.set_restriction(self.bounds.as_mut_ptr().cast());
            dev.set_lot_manager_vtable(manager_vt.as_ptr() as u32);
            dev.set_city(city.as_mut_ptr().cast());
            dev.set_lot_configuration(leveller.as_mut_ptr().cast());
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.dev = self.dev.ptr() as usize;
                s.city = city.as_ptr() as usize;
                s.level = leveller.as_ptr() as usize;
                s.lot_table = lot_vt.as_ptr() as usize;
                s.real = dev.real_mode() != 0;
            });
        }
        let _alloc = fake(0x0090_CF54, allocate as *const () as usize);
        let _free = fake(0x0090_CF63, free as *const () as usize);
        let _remnants = fake(0x0072_D440, remnants as *const () as usize);
        let _street = fake(0x0072_A2A0, street as *const () as usize);
        let before = unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0x100).to_vec() };
        unsafe {
            call_thiscall_with_eax(seeded_entry as *const () as usize, self.dev.ptr(), &[], 0);
        }
        assert_eq!(before, unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0x100) });
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
}

fn geometry(calls: &[Call]) -> Vec<Call> {
    calls
        .iter()
        .filter(|c| matches!(c, Call::Attempt(..) | Call::Street(..) | Call::Remnants(..)))
        .cloned()
        .collect()
}

#[test]
fn residential_road_example_levels_then_locks_and_releases_in_order() {
    let mut w = World::new([10, 10, 15, 13], 0, 3, [1, 0, 0, 0], true);
    let calls = w.run();
    let mut expected = vec![Call::Allocate(0)];
    let lots = [
        [10, 10, 2, 2, 1],
        [13, 10, 3, 2, 3],
        [10, 12, 2, 2, 1],
        [13, 12, 3, 2, 3],
    ];
    for (id, [x, z, width, height, facing]) in lots.into_iter().enumerate() {
        expected.push(Call::Attempt([x, z, width, height, facing]));
        if id == 0 {
            expected.push(Call::Street([12, 10, 12, 13], 0));
        }
        expected.extend([
            Call::Width,
            Call::Height,
            Call::Level([x, z, x + width - 1, z + height - 1]),
            Call::Lock(id, 1),
            Call::Allocate(id + 1),
        ]);
    }
    for id in 0..4 {
        expected.extend([Call::Lock(id, 0), Call::Release(id)]);
    }
    for id in 1..=4 {
        expected.push(Call::Free(id));
    }
    expected.push(Call::Free(0));
    assert_eq!(calls, expected);
}

#[test]
fn commercial_forces_a_middle_cross_street_after_the_first_row() {
    let mut w = World::new([10, 10, 15, 13], 3, 0, [1, 0, 0, 0], true);
    unsafe {
        ParcelSettings::from_ptr(w.dev.ptr()).set_middle_street(1);
    }
    STATE.with(|s| s.borrow_mut().failed = vec![[15, 10, 2, 2, 0], [15, 12, 2, 1, 2]]);
    assert_eq!(
        geometry(&w.run()),
        [
            Call::Attempt([10, 10, 2, 2, 0]),
            Call::Attempt([10, 12, 2, 1, 2]),
            Call::Street([10, 13, 15, 13], 1),
            Call::Street([12, 10, 12, 13], 0),
            Call::Attempt([13, 10, 2, 2, 0]),
            Call::Attempt([13, 12, 2, 1, 2]),
            Call::Attempt([15, 10, 2, 2, 0]),
            Call::Remnants([15, 10, 2, 2, 0]),
            Call::Attempt([15, 12, 2, 1, 2]),
            Call::Remnants([15, 12, 2, 1, 2]),
        ]
    );
    STATE.with(|s| assert_eq!(s.borrow().street_words, [0xAABB_CC01, 0xDDEE_FF00]));
}

#[test]
fn no_streets_and_both_reverse_orientations_keep_their_cell_order() {
    let mut w = World::new([10, 10, 15, 13], 3, 0, [1; 4], false);
    assert_eq!(
        geometry(&w.run()),
        [
            Call::Attempt([10, 10, 2, 2, 0]),
            Call::Attempt([10, 12, 2, 2, 2]),
            Call::Attempt([12, 10, 2, 2, 0]),
            Call::Attempt([12, 12, 2, 2, 2]),
            Call::Attempt([14, 10, 2, 2, 0]),
            Call::Attempt([14, 12, 2, 2, 2]),
        ]
    );
    let mut w = World::new([10, 10, 11, 13], 2, 1, [0, 1, 0, 0], true);
    assert_eq!(
        geometry(&w.run()),
        [Call::Attempt([10, 12, 2, 2, 1]), Call::Attempt([10, 10, 2, 2, 1])]
    );
    let mut w = World::new([10, 10, 13, 11], 1, 2, [0, 0, 1, 0], true);
    assert_eq!(
        geometry(&w.run()),
        [Call::Attempt([12, 10, 2, 2, 2]), Call::Attempt([10, 10, 2, 2, 2])]
    );
}

#[test]
fn large_block_spacing_and_l_shaped_failures() {
    let mut w = World::new([10, 10, 21, 19], 0, 3, [1, 0, 0, 0], true);
    let mut expected = vec![];
    for row in [10, 12, 14, 17, 19] {
        for (x, depth, facing) in [(10, 2, 1), (13, 2, 3), (15, 2, 1), (18, 2, 3), (20, 1, 1)] {
            expected.push(Call::Attempt([x, row, depth, 2, facing]));
            if row == 10 {
                match x {
                    10 => expected.push(Call::Street([12, 10, 12, 19], 0)),
                    15 => expected.push(Call::Street([17, 10, 17, 19], 0)),
                    20 => expected.push(Call::Street([21, 10, 21, 19], 1)),
                    _ => {}
                }
            }
        }
        if row == 14 {
            expected.push(Call::Street([10, 16, 21, 16], 0));
        }
    }
    assert_eq!(geometry(&w.run()), expected);
    let mut w = World::new([10, 10, 15, 15], 0, 3, [1, 0, 0, 0], true);
    STATE.with(|s| s.borrow_mut().failed = vec![[13, 12, 3, 2, 3], [13, 14, 3, 2, 3]]);
    let calls = geometry(&w.run());
    assert!(calls.contains(&Call::Remnants([13, 12, 3, 2, 3])));
    assert!(calls.contains(&Call::Remnants([13, 14, 3, 2, 3])));
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Street(..))).count(), 1);
}

#[test]
fn map_edge_start_clamp_and_repeated_city_queries_for_clipping() {
    let mut w = World::new([0, 5, 1, 5], 2, 1, [1; 4], false);
    unsafe {
        let p = ParcelSettings::from_ptr(w.dev.ptr());
        p.set_width(1);
        p.set_depth(3);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.width = 2;
        s.height = 5;
        s.failed = vec![[1, 5, 2, 1, 1], [-1, 5, 2, 1, 3]];
    });
    let calls = w.run();
    assert_eq!(
        geometry(&calls),
        [
            Call::Attempt([1, 5, 2, 1, 1]),
            Call::Remnants([1, 5, 2, 1, 1]),
            Call::Attempt([-1, 5, 2, 1, 3]),
            Call::Remnants([-1, 5, 2, 1, 3])
        ]
    );
    assert_eq!(
        calls,
        [
            Call::Allocate(0),
            Call::Attempt([1, 5, 2, 1, 1]),
            Call::Remnants([1, 5, 2, 1, 1]),
            Call::Width,
            Call::Width,
            Call::Height,
            Call::Height,
            Call::Level([1, 5, 1, 4]),
            Call::Attempt([-1, 5, 2, 1, 3]),
            Call::Remnants([-1, 5, 2, 1, 3]),
            Call::Width,
            Call::Height,
            Call::Height,
            Call::Level([0, 5, 0, 4]),
            Call::Free(0)
        ]
    );
}

#[test]
fn highlight_empty_bounds_invalid_orientation_and_oversized_row() {
    let mut w = World::new([10, 10, 15, 13], 3, 0, [1; 4], false);
    unsafe {
        ZoneDeveloper::from_ptr(w.dev.ptr()).set_real_mode(0);
    }
    let calls = w.run();
    assert_eq!(calls.len(), 8);
    assert!(!calls
        .iter()
        .any(|c| matches!(c, Call::Level(..) | Call::Lock(..) | Call::Release(..))));
    let mut w = World::new([10, 10, 9, 9], 0, 3, [1; 4], true);
    assert_eq!(w.run(), [Call::Allocate(0), Call::Free(0)]);
    let mut w = World::new([10, 10, 15, 13], u32::MAX, 3, [1; 4], true);
    assert!(w.run().is_empty());
}

#[test]
fn all_depth_rule_cases_match_the_core_vectors() {
    let rows = [
        (
            2,
            1,
            3,
            1,
            [1, 2, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2],
            [1, 2, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2],
            [2, 1, 2, 3, 2, 2, 2, 2, 2, 2, 2, 2],
        ),
        (
            3,
            2,
            4,
            1,
            [1, 2, 3, 4, 3, 3, 2, 3, 3, 3, 3, 3],
            [1, 2, 3, 4, 2, 3, 3, 3, 3, 3, 3, 3],
            [3, 1, 2, 3, 4, 3, 3, 3, 3, 3, 3, 3],
        ),
        (
            4,
            3,
            5,
            1,
            [1, 2, 3, 4, 5, 3, 4, 4, 5, 3, 4, 4],
            [1, 2, 3, 4, 5, 6, 3, 4, 4, 4, 4, 4],
            [4, 1, 2, 3, 4, 5, 3, 4, 4, 4, 4, 4],
        ),
        (
            4,
            3,
            5,
            0,
            [1, 2, 3, 4, 5, 3, 4, 4, 3, 3, 4, 4],
            [1, 2, 3, 4, 5, 3, 3, 4, 4, 4, 4, 4],
            [4, 1, 2, 3, 4, 5, 3, 4, 4, 4, 4, 4],
        ),
    ];
    for (depth, min, max, ps, a, b, c) in rows {
        for rem in 1..=12 {
            let i = rem as usize - 1;
            assert_eq!(depth_rule(rem, true, true, min, max, depth, ps), a[i]);
            assert_eq!(depth_rule(rem, false, true, min, max, depth, ps), b[i]);
            assert_eq!(depth_rule(rem, true, false, min, max, depth, ps), b[i]);
            assert_eq!(depth_rule(rem, false, false, min, max, depth, ps), c[i]);
        }
    }
}

#[test]
fn middle_row_vectors_and_unsigned_cross_street_bounds() {
    for (last, middle) in [(15, 10), (17, 12), (21, 14), (11, 10)] {
        let mut w = World::new([10, 10, last, 13], 3, 0, [1, 0, 0, 0], true);
        unsafe {
            let settings = ParcelSettings::from_ptr(w.dev.ptr());
            settings.set_middle_street(1);
            settings.set_block(100);
        }
        let calls = geometry(&w.run());
        assert!(calls.contains(&Call::Street([middle + 2, 10, middle + 2, 13], 0)));
        let index = calls
            .iter()
            .position(|c| *c == Call::Street([middle + 2, 10, middle + 2, 13], 0))
            .unwrap();
        assert_eq!(
            calls[..index].iter().rev().find(|c| matches!(c, Call::Attempt(..))),
            Some(&Call::Attempt([middle, 12, 2, 1, 2]))
        );
    }
    let mut w = World::new([10, 0, 15, 0], 0, 3, [1, 0, 0, 0], true);
    unsafe {
        ParcelSettings::from_ptr(w.dev.ptr()).set_block(0);
    }
    assert!(geometry(&w.run()).contains(&Call::Street([10, 2, 15, 2], 0)));
}
