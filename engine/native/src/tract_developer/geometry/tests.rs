// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate geometry cases from the core tract-developer tests, at the game's field offsets.

use super::*;
use crate::testing::{GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, ECX, EDX};
use std::cell::RefCell;

#[test]
fn bounds_cover_all_facings_byte_sizes_and_wrapping_coordinates() {
    let mut object = Object::new(0xA4);
    let p = object.ptr();
    let c = unsafe { Candidate::from_ptr(p) };
    for facing in [0, 1, 2, 3] {
        for (x, z) in [(100i32, 200i32), (i32::MIN, i32::MAX), (0, 0)] {
            for (w, h) in [(0u8, 0u8), (1, 1), (2, 3), (255, 254)] {
                c.set_x(x);
                c.set_z(z);
                c.set_facing(facing);
                c.set_width(w);
                c.set_depth(h);
                c.set_min_x(-1);
                c.set_min_z(-2);
                c.set_max_x(-3);
                c.set_max_z(-4);
                let before = snapshot(p, 0xA4);
                let w = i32::from(w);
                let h = i32::from(h);
                let want = match facing {
                    0 => [
                        x.wrapping_sub(w).wrapping_add(1),
                        z,
                        x,
                        z.wrapping_add(h).wrapping_sub(1),
                    ],
                    1 => [
                        x.wrapping_sub(h).wrapping_add(1),
                        z.wrapping_sub(w).wrapping_add(1),
                        x,
                        z,
                    ],
                    2 => [
                        x,
                        z.wrapping_sub(h).wrapping_add(1),
                        x.wrapping_add(w).wrapping_sub(1),
                        z,
                    ],
                    _ => [
                        x,
                        z,
                        x.wrapping_add(h).wrapping_sub(1),
                        z.wrapping_add(w).wrapping_sub(1),
                    ],
                };
                let regs = run(0x00704630, Conv::Thiscall, ECX, &[p as u32]);
                assert_eq!([c.min_x(), c.min_z(), c.max_x(), c.max_z()], want);
                let expected_regs = match facing {
                    0 => (want[3] as u32, h as u32, z as u32),
                    1 => (z as u32, w as u32, want[1] as u32),
                    2 => (want[1] as u32, z as u32, h as u32),
                    _ => (want[3] as u32, w as u32, z as u32),
                };
                assert_eq!(regs, expected_regs);
                let mut bytes = before;
                for (i, value) in want.into_iter().enumerate() {
                    bytes[0x38 + i * 4..0x3C + i * 4].copy_from_slice(&value.to_le_bytes());
                }
                assert_eq!(snapshot(p, 0xA4), bytes);
            }
        }
    }
}

#[test]
fn bounds_retain_cache_and_do_not_normalize_invalid_facings() {
    let mut object = Object::new(0xA4);
    let p = object.ptr();
    let c = unsafe { Candidate::from_ptr(p) };
    for facing in [i32::MIN, -7, -2, 4, 5, i32::MAX] {
        c.set_facing(facing);
        for rect in [[1, 2, 3, 4], [-1, 2, 3, 4], [1, -2, 3, 4], [1, 2, -3, 4], [1, 2, 3, -4]] {
            c.set_min_x(rect[0]);
            c.set_min_z(rect[1]);
            c.set_max_x(rect[2]);
            c.set_max_z(rect[3]);
            let before = snapshot(p, 0xA4);
            let result = run(0x00704630, Conv::Thiscall, ECX, &[p as u32]);
            assert_eq!(
                result,
                (if rect.iter().all(|&v| v >= 0) { 4 } else { facing as u32 }, ECX, EDX)
            );
            assert_eq!(snapshot(p, 0xA4), before);
        }
    }
}

thread_local! {
    static CALLS: RefCell<Vec<(u32, u32, u32, u32)>> = const { RefCell::new(Vec::new()) };
}

#[repr(C)]
struct Lot {
    table: *const usize,
    next: *mut u8,
    candidate: *mut u8,
}

extern "thiscall" fn facing(p: *mut Lot) -> u64 {
    unsafe {
        CALLS.with(|calls| calls.borrow_mut().push((0x5C, p as u32, 0, 0)));
        Candidate::from_ptr((*p).candidate).set_lot((*p).next);
    }
    0xABCD_EF01_0000_0002
}
extern "thiscall" fn size(p: *mut Lot, width: *mut u8, depth: *mut u8) -> u64 {
    unsafe {
        CALLS.with(|calls| calls.borrow_mut().push((0x58, p as u32, width as u32, depth as u32)));
        *width = 9;
        *depth = 11;
        Candidate::from_ptr((*p).candidate).set_lot((*p).next);
    }
    0xABCD_EF01_0000_0002
}
extern "thiscall" fn location(p: *mut Lot, x: *mut i32, z: *mut i32) -> u64 {
    unsafe {
        CALLS.with(|calls| calls.borrow_mut().push((0x54, p as u32, x as u32, z as u32)));
        *x = 100;
        *z = 200;
        Candidate::from_ptr((*p).candidate).set_lot((*p).next);
    }
    0xABCD_EF01_0000_0002
}
extern "thiscall" fn rect(p: *mut Lot, bounds: *mut i32) -> u64 {
    unsafe {
        CALLS.with(|calls| calls.borrow_mut().push((0x38, p as u32, bounds as u32, 0)));
        for i in 0..4 {
            *bounds.add(i) = 300 + i as i32;
        }
    }
    0xAABB_CCDD_1122_3344
}

#[test]
fn bounds_query_lot_in_order_and_reload_lot_pointer_between_calls() {
    let mut vt = [0usize; 24];
    vt[0x5C / 4] = facing as *const () as usize;
    vt[0x58 / 4] = size as *const () as usize;
    vt[0x54 / 4] = location as *const () as usize;
    vt[0x38 / 4] = rect as *const () as usize;
    let mut object = Object::new(0xA4);
    let p = object.ptr();
    let c = unsafe { Candidate::from_ptr(p) };
    let mut lots: Vec<_> = (0..4)
        .map(|_| {
            Box::new(Lot {
                table: vt.as_ptr(),
                next: core::ptr::null_mut(),
                candidate: p,
            })
        })
        .collect();
    for i in 0..3 {
        lots[i].next = (&mut *lots[i + 1] as *mut Lot).cast();
    }
    let addresses: Vec<_> = lots.iter_mut().map(|lot| &mut **lot as *mut Lot as u32).collect();
    c.set_lot(addresses[0] as *mut u8);
    c.set_facing(-1);
    c.set_min_x(0);
    c.set_min_z(0);
    c.set_max_x(0);
    c.set_max_z(-1);
    let result = run(0x00704630, Conv::Thiscall, ECX, &[p as u32]);
    assert_eq!(result.0, 0x1122_3344);
    assert_eq!(result.2, 0xAABB_CCDD);
    assert_eq!((c.facing(), c.width(), c.depth(), c.x(), c.z()), (2, 9, 11, 100, 200));
    assert_eq!([c.min_x(), c.min_z(), c.max_x(), c.max_z()], [300, 301, 302, 303]);
    let a = p as u32;
    CALLS.with(|calls| {
        assert_eq!(
            std::mem::take(&mut *calls.borrow_mut()),
            [
                (0x5C, addresses[0], 0, 0),
                (0x58, addresses[1], a + 0x14, a + 0x15),
                (0x54, addresses[2], a + 8, a + 0xC),
                (0x38, addresses[3], a + 0x38, 0),
            ]
        )
    });
    let before = snapshot(p, 0xA4);
    assert_eq!(run(0x00704630, Conv::Thiscall, ECX, &[a]), (303, ECX, EDX));
    assert_eq!(snapshot(p, 0xA4), before);
}

#[test]
fn score_comparator_handles_equal_signed_zero_infinities_and_nan() {
    let _fpu = GameFpu::new();
    let mut a = Object::new(0xA4);
    let mut b = Object::new(0xA4);
    let ap = a.ptr();
    let bp = b.ptr();
    let ac = unsafe { Candidate::from_ptr(ap) };
    let bc = unsafe { Candidate::from_ptr(bp) };
    let values = [
        0.0,
        -0.0,
        -1.0,
        2.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0x7F80_0001),
        f32::from_bits(1),
    ];
    for x in values {
        for y in values {
            ac.set_score(x);
            bc.set_score(y);
            let before_a = snapshot(ap, 0xA4);
            let before_b = snapshot(bp, 0xA4);
            assert_eq!(
                run(0x00704770, Conv::Cdecl, ECX, &[ap as u32, bp as u32]),
                (u32::from(x < y), bp as u32, EDX)
            );
            assert_eq!(snapshot(ap, 0xA4), before_a);
            assert_eq!(snapshot(bp, 0xA4), before_b);
        }
    }
}

#[test]
fn bounds_and_comparison_preserve_x87_stack_control_word_mxcsr_and_direction() {
    #[repr(C, align(16))]
    struct State([u8; 512]);
    let _fpu = GameFpu::new();
    let mut a = Object::new(0xA4);
    let mut b = Object::new(0xA4);
    let ap = a.ptr();
    let bp = b.ptr();
    unsafe {
        Candidate::from_ptr(ap).set_score(1.0);
        Candidate::from_ptr(bp).set_score(2.0);
    }
    let (mut before, mut after) = (State([0; 512]), State([0; 512]));
    let mut saved = 0u64;
    let flags: u32;
    unsafe {
        core::arch::asm!("fld1", "fxsave [{p}]", p = in(reg) &mut before, out("st(0)") _);
        run(0x00704630, Conv::Thiscall, ECX, &[ap as u32]);
        run(0x00704770, Conv::Cdecl, ECX, &[ap as u32, bp as u32]);
        core::arch::asm!("fxsave [{p}]", "fstp qword ptr [{saved}]", "pushfd", "pop {flags}",
            p = in(reg) &mut after, saved = in(reg) &mut saved, flags = out(reg) flags, out("st(0)") _);
    }
    assert_eq!(saved, 1.0f64.to_bits());
    assert_eq!(&before.0[0..2], &after.0[0..2]);
    assert_eq!(before.0[4], after.0[4]);
    assert_eq!(&before.0[24..28], &after.0[24..28]);
    assert_eq!(flags & (1 << 10), 0);
}
