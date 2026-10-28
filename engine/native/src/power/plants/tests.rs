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

//! Production and pole-cost cases use raw objects and ordered property/virtual fakes.
use super::*;
use crate::testing::{call_thiscall_with_eax, fake, GameFpu, Object};
use std::cell::{Cell, RefCell};
#[derive(Debug, PartialEq)]
struct Call(u32, usize, u32);
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const {RefCell::new(Vec::new())};
    static CAPACITY: Cell<*mut u32> = const {Cell::new(core::ptr::null_mut())};
}
fn log(slot: u32, this: *mut u8, arg: u32) {
    CALLS.with(|c| c.borrow_mut().push(Call(slot, this as usize, arg)));
}
fn calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
#[repr(C)]
struct Node {
    vt: *const usize,
    child: *mut u8,
    value: u64,
    success: u32,
    result: u32,
}
extern "thiscall" fn holder(p: *mut Node) -> *mut u8 {
    log(0x18, p.cast(), 0);
    unsafe { (*p).child }
}
extern "thiscall" fn occupant(p: *mut Node) -> *mut u8 {
    log(0x0C, p.cast(), 0);
    unsafe { (*p).child }
}
extern "C" fn uint(p: *mut Node, id: u32, out: *mut u32) -> u32 {
    log(0x005F_D480, p.cast(), id);
    CAPACITY.with(|c| c.set(out));
    unsafe {
        out.write_unaligned((*p).value as u32);
        (*p).success
    }
}
extern "C" fn int64(p: *mut Node, id: u32, out: *mut u64) -> u32 {
    log(0x005F_D4B0, p.cast(), id);
    unsafe {
        out.write_unaligned((*p).value);
        (*p).result
    }
}
extern "thiscall" fn efficiency(this: *mut u8, out: *mut f32, plant: *mut u8) -> *mut f32 {
    log(0xB4, this, plant as u32);
    unsafe {
        let replacement = this.add(0x48).cast::<u32>().read_unaligned();
        if replacement != 0 {
            CAPACITY.with(|c| c.get().write_unaligned(replacement));
        }
        let value = this.add(0x40).cast::<f32>().read_unaligned();
        out.write_unaligned(value);
        let alternate = this.add(0x44).cast::<*mut f32>().read_unaligned();
        if alternate.is_null() {
            out
        } else {
            alternate
        }
    }
}
#[test]
fn production_handles_null_missing_and_unsigned_capacity_with_original_call_order() {
    let _fpu = GameFpu::new();
    let _read = fake(0x005F_D480, uint as *const () as usize);
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0xB8 / 4];
    vt[0xB4 / 4] = efficiency as *const () as usize;
    unsafe {
        object.ptr().cast::<*const usize>().write(vt.as_ptr());
    }
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let mut props = Node {
        vt: core::ptr::null(),
        child: core::ptr::null_mut(),
        value: 0,
        success: 1,
        result: 0,
    };
    let props_ptr = (&mut props as *mut Node).cast();
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: props_ptr,
        value: 0,
        success: 0,
        result: 0,
    };
    let p = (&mut plant as *mut Node).cast();
    assert_eq!(unsafe { fc4p_006eb1c0(object.ptr(), core::ptr::null_mut()) }, 0);
    assert!(calls().is_empty());
    for (capacity, factor, success, expected) in [
        (100u32, 0.5f32, 1, 50u32),
        (100, 0.0, 1, 0),
        (100, -0.25, 1, (-25i32) as u32),
        (u32::MAX, 1.0, 1, 0),
        (0x8000_0000, 1.0, 1, 0x8000_0000),
        (100, f32::NAN, 1, 0),
        (100, f32::INFINITY, 1, 0),
        (42, 0.5, 0, 42),
        (43, 0.5, 0x100, 43),
    ] {
        unsafe {
            (*props_ptr.cast::<Node>()).value = u64::from(capacity);
            (*props_ptr.cast::<Node>()).success = success;
        }
        unsafe {
            object.ptr().add(0x40).cast::<f32>().write_unaligned(factor);
        }
        assert_eq!(unsafe { fc4p_006eb1c0(object.ptr(), p) }, expected);
        let mut expected_calls = vec![
            Call(0x18, p as usize, 0),
            Call(0x005F_D480, props_ptr as usize, 0x2781_2852),
        ];
        if success as u8 != 0 {
            expected_calls.push(Call(0xB4, object.ptr() as usize, p as u32));
        }
        assert_eq!(calls(), expected_calls);
    }
}
#[test]
fn production_reloads_capacity_and_uses_the_pointer_returned_by_efficiency() {
    let _fpu = GameFpu::new();
    let _read = fake(0x005F_D480, uint as *const () as usize);
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0xB8 / 4];
    vt[0xB4 / 4] = efficiency as *const () as usize;
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let mut props = Node {
        vt: core::ptr::null(),
        child: core::ptr::null_mut(),
        value: 100,
        success: 1,
        result: 0,
    };
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: (&mut props as *mut Node).cast(),
        value: 0,
        success: 0,
        result: 0,
    };
    let mut alternate = 0.25f32;
    unsafe {
        object.ptr().cast::<*const usize>().write(vt.as_ptr());
        object.ptr().add(0x40).cast::<f32>().write_unaligned(0.5);
        object
            .ptr()
            .add(0x44)
            .cast::<*mut f32>()
            .write_unaligned(&mut alternate);
        object.ptr().add(0x48).cast::<u32>().write_unaligned(80);
    }
    assert_eq!(
        unsafe { fc4p_006eb1c0(object.ptr(), (&mut plant as *mut Node).cast()) },
        20
    );
    assert_eq!(calls().len(), 3);
}
#[test]
fn infrastructure_costs_sum_low_words_wrap_clear_high_word_and_keep_last_result_bytes() {
    let _read = fake(0x005F_D4B0, int64 as *const () as usize);
    let mut object = Object::new(0x400);
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    let mut vt = [0usize; 7];
    vt[3] = occupant as *const () as usize;
    vt[6] = holder as *const () as usize;
    let mut h1 = Node {
        vt: vt.as_ptr(),
        child: core::ptr::null_mut(),
        value: 0xAAAA_AAAA_FFFF_FFFF,
        success: 0,
        result: 0x1122_3344,
    };
    let mut h2 = Node {
        vt: vt.as_ptr(),
        child: core::ptr::null_mut(),
        value: 0xBBBB_BBBB_0000_0003,
        success: 0,
        result: 0xAABB_CCDD,
    };
    let (ph1, ph2) = ((&mut h1 as *mut Node).cast(), (&mut h2 as *mut Node).cast());
    let mut o1 = Node {
        vt: vt.as_ptr(),
        child: ph1,
        value: 0,
        success: 0,
        result: 0,
    };
    let mut o2 = Node {
        vt: vt.as_ptr(),
        child: ph2,
        value: 0,
        success: 0,
        result: 0,
    };
    let (po1, po2) = ((&mut o1 as *mut Node).cast(), (&mut o2 as *mut Node).cast());
    let mut p1 = Node {
        vt: vt.as_ptr(),
        child: po1,
        value: 0,
        success: 0,
        result: 0,
    };
    let mut p2 = Node {
        vt: vt.as_ptr(),
        child: po2,
        value: 0,
        success: 0,
        result: 0,
    };
    let (pp1, pp2) = ((&mut p1 as *mut Node).cast(), (&mut p2 as *mut Node).cast());
    let mut poles = [pp1, pp2];
    sim.set_poles_begin(poles.as_mut_ptr());
    sim.set_poles_end(unsafe { poles.as_mut_ptr().add(2) });
    for (old, result) in [(0u64, 0xAABB_CC01u32), (2, 0), (0x1234_5600_0000_0002, 0x1234_5601)] {
        sim.set_infrastructure_cost(old);
        let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }.to_vec();
        let (eax, _) =
            unsafe { call_thiscall_with_eax(fc4p_006eb490 as *const () as usize, object.ptr(), &[], 0x9988_7766) };
        assert_eq!(eax, result);
        assert_eq!(sim.infrastructure_cost(), 2);
        let mut expected = before;
        expected[0x78..0x80].copy_from_slice(&2u64.to_le_bytes());
        assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }, expected);
        assert_eq!(
            calls(),
            [
                Call(0x0C, pp1 as usize, 0),
                Call(0x18, po1 as usize, 0),
                Call(0x005F_D4B0, ph1 as usize, 0x0913_2DE8),
                Call(0x0C, pp2 as usize, 0),
                Call(0x18, po2 as usize, 0),
                Call(0x005F_D4B0, ph2 as usize, 0x0913_2DE8)
            ]
        );
    }
    sim.set_poles_end(poles.as_mut_ptr());
    sim.set_infrastructure_cost(3);
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006eb490 as *const () as usize, object.ptr(), &[], 0x9988_7766) }.0,
        0x9988_7701
    );
    assert_eq!(sim.infrastructure_cost(), 0);
    assert!(calls().is_empty());
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006eb490 as *const () as usize, object.ptr(), &[], 0x9988_7766) }.0,
        0
    );
}
