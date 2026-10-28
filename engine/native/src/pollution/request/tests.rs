// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Prop-share vectors from the core, with direct rectangle and virtual-getter fixtures.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Garbage(u32, i32, i32),
    Curve(u32, u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static STATE: RefCell<(u32, *mut u8, Option<f64>)> = const { RefCell::new((2048, core::ptr::null_mut(), None)) };
}
fn take() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
extern "thiscall" fn garbage(this: *mut u8, x: i32, z: i32, output: *mut u32) {
    CALLS.with(|c| c.borrow_mut().push(Call::Garbage(this as u32, x, z)));
    STATE.with(|s| {
        let (value, rectangle, _) = *s.borrow();
        unsafe {
            *output = value;
            if !rectangle.is_null() {
                Rectangle::from_ptr(rectangle).set_x1(0);
                Rectangle::from_ptr(rectangle).set_z1(1);
            }
        }
    });
}
#[unsafe(naked)]
unsafe extern "C" fn curve() {
    core::arch::naked_asm!(
        "push ecx", "push edx", "push dword ptr [esp + 12]", "call {body}", "add esp, 12",
        "mov edx, 0xaabbccdd", "ret 4", body = sym curve_body,
    );
}
extern "C" fn curve_body(input: f32, remainder: u32, this: u32) -> f64 {
    CALLS.with(|c| c.borrow_mut().push(Call::Curve(this, input.to_bits(), remainder)));
    STATE.with(|s| {
        s.borrow()
            .2
            .unwrap_or_else(|| (f64::from(input) * 80.0).clamp(0.0, 10.0))
    })
}

#[test]
fn requested_share_scans_inclusive_rectangles_and_uses_unsigned_wrapping_average() {
    let _fpu = GameFpu::new();
    let _curve = fake(0x005F_4250, curve as *const () as usize);
    let mut curve_pointer = 0x1234_5678usize as *mut u8;
    let mut scale = 1.0f32 / 32767.0;
    let _pointer = fake_data(0x00B4_9BAC, (&mut curve_pointer as *mut *mut u8).cast());
    let _scale = fake_data(0x00AB_25E4, (&mut scale as *mut f32).cast());
    let mut object = Object::new(0x250);
    let this = object.ptr();
    let mut vtable = [0usize; 16];
    vtable[0x24 / 4] = garbage as *const () as usize;
    unsafe {
        write_unaligned(this.cast(), vtable.as_ptr());
    }
    for (rectangle, value, quotient, share) in [
        ([0i32, 0, 3, 3], 2048, 2048u32, 5),
        ([3, 3, 0, 0], 2048, 0, 0),
        ([-1, -2, 0, -2], u32::MAX, 0x7FFF_FFFF, 10),
        ([0, 0, 0, 0], 0xFFFF_0064, 0xFFFF_0064, 10),
    ] {
        let mut rectangle = rectangle;
        STATE.with(|s| *s.borrow_mut() = (value, core::ptr::null_mut(), None));
        let result = unsafe { fc4p_006e3bc0(this.add(8), rectangle.as_mut_ptr().cast()) };
        assert_eq!(result, registers(share, 0xAABB_CCDD));
        let mut expected = vec![];
        for x in rectangle[0]..=rectangle[2] {
            for z in rectangle[1]..=rectangle[3] {
                expected.push(Call::Garbage(this as u32, x, z));
            }
        }
        let average = if (quotient as i32) < 0 {
            F80::from_i32(quotient as i32) + F80::from_f32(4_294_967_296.0)
        } else {
            F80::from_i32(quotient as i32)
        };
        let input = (average * F80::from_f32(scale)).to_f32();
        expected.push(Call::Curve(curve_pointer as u32, input.to_bits(), 0));
        assert_eq!(take(), expected);
    }
}

#[test]
fn requested_share_reloads_bounds_and_rounds_the_unrounded_curve_result_with_the_bias_formula() {
    let _fpu = GameFpu::new();
    let _curve = fake(0x005F_4250, curve as *const () as usize);
    let mut curve_pointer = 0x1234_5678usize as *mut u8;
    let mut scale = 1.0f32;
    let _pointer = fake_data(0x00B4_9BAC, (&mut curve_pointer as *mut *mut u8).cast());
    let _scale = fake_data(0x00AB_25E4, (&mut scale as *mut f32).cast());
    let mut object = Object::new(0x250);
    let this = object.ptr();
    let mut vtable = [0usize; 16];
    vtable[0x24 / 4] = garbage as *const () as usize;
    unsafe {
        write_unaligned(this.cast(), vtable.as_ptr());
    }
    let mut rectangle = [0i32, 0, 2, 2];
    STATE.with(|s| *s.borrow_mut() = (7, rectangle.as_mut_ptr().cast(), Some(0.5)));
    assert_eq!(
        unsafe { fc4p_006e3bc0(this.add(8), rectangle.as_mut_ptr().cast()) },
        registers(0, 0xAABB_CCDD)
    );
    assert_eq!(
        take(),
        [
            Call::Garbage(this as u32, 0, 0),
            Call::Garbage(this as u32, 0, 1),
            Call::Curve(curve_pointer as u32, 7.0f32.to_bits(), 0)
        ]
    );
    for (output, expected) in [
        (0.500_000_000_1, 1),
        (1.5, 2),
        (2.5, 2),
        (-1.5, -2),
        (5_000_000.0, 4_597_152),
    ] {
        STATE.with(|s| *s.borrow_mut() = (7, core::ptr::null_mut(), Some(output)));
        let result = unsafe { fc4p_006e3bc0(this.add(8), rectangle.as_mut_ptr().cast()) };
        assert_eq!(result, registers(expected as u32, 0xAABB_CCDD));
        take();
    }
}
