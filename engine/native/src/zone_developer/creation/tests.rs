// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lot-creation cases from the core's facing and highlight tests, on the game's interfaces.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Check(usize, [u32; 7]),
    Create(usize, [i32; 5]),
    Allocate(u32),
    Face(usize, [i32; 4]),
    Destroy,
    Virtual(usize, [i32; 2]),
}

struct State {
    calls: Vec<Call>,
    code: u32,
    creates: bool,
    header: [u32; 3],
    payloads: Vec<[i32; 3]>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        calls: Vec::new(), code: 8, creates: true, header: [0xA5A5_A5A5; 3], payloads: Vec::new()
    }) };
}

unsafe extern "thiscall" fn check(
    this: *mut u8, x: u32, z: u32, w: u32, h: u32, zone: u32, water: u32, cfg: u32,
) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Check(this as usize, [x, z, w, h, zone, water, cfg]));
        (0xDEAD_BEEFu64 << 32) | u64::from(s.code)
    })
}

unsafe extern "thiscall" fn create(this: *mut u8, x: i32, z: i32, w: i32, h: i32, f: i32, out: *mut *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Create(this as usize, [x, z, w, h, f]));
        if s.creates {
            unsafe { *out = 0x1234usize as *mut u8 };
        }
    });
}

extern "C" fn alloc(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        s.header.as_mut_ptr().cast()
    })
}

unsafe extern "thiscall" fn face(
    this: *mut u8, x: i32, z: i32, w: i32, h: i32, list: *mut *mut u8, out: *mut *mut u8,
) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let header = s.header.as_mut_ptr();
        unsafe {
            assert_eq!(*list, header.cast());
            assert_eq!(s.header, [header as u32, header as u32, 0xA5A5_A5A5]);
            assert!((*out).is_null());
            if s.creates { *out = 0x5678usize as *mut u8; }
        }
        s.calls.push(Call::Face(this as usize, [x, z, w, h]));
    });
}

unsafe extern "thiscall" fn destroy(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe { assert_eq!(*this.cast::<*mut u8>(), s.header.as_mut_ptr().cast()) };
        s.calls.push(Call::Destroy);
    });
}

unsafe extern "thiscall" fn insert(this: *mut u8, key: *const [i32; 2]) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Virtual(this as usize, unsafe { *key }));
        s.payloads.push([0x7777_7777; 3]);
        let payload = s.payloads.last_mut().unwrap().as_mut_ptr();
        (0x1234_ABCDu64 << 32) | u64::from(payload as u32)
    })
}

struct World {
    dev: Object,
    _table: Box<[usize; 24]>,
    _fakes: Vec<crate::testing::Fake>,
}

impl World {
    fn new(real: u8, code: u32) -> Self {
        STATE.with(|s| *s.borrow_mut() = State {
            calls: vec![], code, creates: true, header: [0xA5A5_A5A5; 3],
            payloads: Vec::with_capacity(1024),
        });
        let mut table = Box::new([0; 24]);
        table[0x50 / 4] = check as *const () as usize;
        table[0x5C / 4] = create as *const () as usize;
        let mut dev = Object::new(0x100);
        unsafe {
            let view = ZoneDeveloper::from_ptr(dev.ptr());
            view.set_real_mode(real);
            view.set_lot_manager_vtable(table.as_ptr() as u32);
        }
        Self { dev, _table: table, _fakes: vec![
            fake(0x0090_CF54, alloc as *const () as usize),
            fake(0x0072_D200, face as *const () as usize),
            fake(0x0056_50A0, destroy as *const () as usize),
            fake(0x0072_F7D0, insert as *const () as usize),
        ] }
    }

    fn run(&mut self, dimensions: [i32; 4], facing: i32) -> (u32, u32, *mut u8) {
        let mut out = 0xFFFFusize as *mut u8;
        let args = [dimensions[0] as u32, dimensions[1] as u32, dimensions[2] as u32,
            dimensions[3] as u32, facing as u32, &mut out as *mut *mut u8 as u32,
            0x1234_5601, 0xABCD_1200, 0x8765_4321];
        let (eax, edx) = unsafe {
            call_thiscall_with_eax(fc4p_00730a80 as *const () as usize,
                self.dev.ptr().add(4), &args, 0xFFFF_1234)
        };
        assert_eq!(edx, if STATE.with(|s| s.borrow().calls.iter().any(|c| matches!(c, Call::Virtual(..)))) {
            0x1234_ABCD
        } else if STATE.with(|s| s.borrow().code == 8) && unsafe { ZoneDeveloper::from_ptr(self.dev.ptr()).real_mode() } == 0 {
            if dimensions[2] > 0 { dimensions[0] as u32 } else { 0xDEAD_BEEF }
        } else { &mut out as *mut *mut u8 as u32 });
        (eax, edx, out)
    }
}

#[test]
fn rejects_existing_lot_and_always_clears_output() {
    let mut w = World::new(1, 7);
    assert_eq!(w.run([10, 20, 2, 3], -1).0, 0);
    STATE.with(|s| assert_eq!(s.borrow().calls, [Call::Check(
        unsafe { w.dev.ptr().add(4) } as usize,
        [10, 20, 2, 3, 0x1234_5601, 0xABCD_1200, 0]) ]));
}

#[test]
fn explicit_facing_uses_virtual_creation_and_output_controls_success() {
    let mut w = World::new(0x80, 8);
    assert_eq!(w.run([-5, 20, -2, 0], 3).0, 1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(s.calls[1], Call::Create(unsafe { w.dev.ptr().add(4) } as usize, [-5, 20, -2, 0, 3]));
        s.creates = false;
    });
    assert_eq!(w.run([10, 20, 2, 3], 0).0, 0);
}

#[test]
fn automatic_facing_uses_one_road_facing_call_and_destroys_the_list() {
    let mut w = World::new(1, 8);
    assert_eq!(w.run([10, 20, 2, 3], -1).2 as usize, 0x5678);
    STATE.with(|s| assert_eq!(&s.borrow().calls[1..], &[
        Call::Allocate(12), Call::Face(w.dev.ptr() as usize, [10, 20, 2, 3]), Call::Destroy,
    ]));
}

#[test]
fn highlight_records_each_cell_with_the_original_origin_and_facing() {
    let mut w = World::new(0, 8);
    let (eax, _, out) = w.run([10, 20, 2, 3], -1);
    assert_eq!(eax, 1);
    assert!(out.is_null());
    let tree = unsafe { w.dev.ptr().add(0xD8) } as usize;
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(&s.calls[1..], &[
            Call::Virtual(tree, [10, 20]), Call::Virtual(tree, [10, 21]),
            Call::Virtual(tree, [10, 22]), Call::Virtual(tree, [11, 20]),
            Call::Virtual(tree, [11, 21]), Call::Virtual(tree, [11, 22]),
        ]);
        assert_eq!(s.payloads, [[10, 20, -1]; 6]);
    });
}

#[test]
fn highlight_empty_dimensions_wrapping_coordinates_and_upper_result_bytes() {
    for dims in [[10, 20, 0, 3], [10, 20, -1, 3], [10, 20, 257, 0]] {
        let mut w = World::new(0, 8);
        assert_eq!(w.run(dims, 7).0, if dims[2] == 257 { 0x101 } else { 1 });
        STATE.with(|s| assert_eq!(s.borrow().calls.len(), 1));
    }
    let mut w = World::new(0, 8);
    assert_eq!(w.run([i32::MAX, i32::MAX, 257, 2], 7).0, 0x101);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.calls[4], Call::Virtual(unsafe { w.dev.ptr().add(0xD8) } as usize, [i32::MIN, i32::MIN]));
        assert!(s.payloads.iter().all(|p| *p == [i32::MAX, i32::MAX, 7]));
    });
}
