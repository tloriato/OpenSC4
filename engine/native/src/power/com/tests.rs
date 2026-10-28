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

//! Interface cases moved from the core power identity test, with original objects and virtual calls.
use super::*;
use crate::abi::{call_raw, call_st0, Conv, RawCall};
use crate::testing::{fake_data, Object};
use std::cell::RefCell;

thread_local! { static CALLS: RefCell<Vec<(u32, u32, u32)>> = const { RefCell::new(Vec::new()) }; }
fn calls() -> Vec<(u32, u32, u32)> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
extern "thiscall" fn retain(this: *mut u8) -> u32 {
    let sim = unsafe { PowerSim::from_ptr(this) };
    CALLS.with(|c| c.borrow_mut().push((4, this as u32, sim.references() as u32)));
    sim.set_references(sim.references().wrapping_add(1));
    0xABCD_EF98
}
extern "thiscall" fn destroy(this: *mut u8, flag: u32) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((0xD0, this as u32, flag)));
    0xDEAD_BEEF
}
extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((0x10, this as u32, 0)));
    unsafe {
        let next = this.add(0x40).cast::<*const usize>().read_unaligned();
        this.cast::<*const usize>().write_unaligned(next);
    }
    0x5566_7788
}
extern "thiscall" fn supply(this: *mut u8, arg: u32) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((0x94, this as u32, arg)));
    0x1234_5678_ABCD_EF01
}
extern "thiscall" fn funding(this: *mut u8, out: *mut u32) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((0x28, this as u32, out as u32)));
    unsafe { out.write(0x3F60_0000) };
    (0x7654_3210u64 << 32) | u64::from(out as u32)
}

fn invoke(entry: *const (), this: *mut u8, args: &[u32]) -> (u32, u32) {
    let mut c = RawCall::new(Conv::Thiscall, entry as usize, this as u32, args);
    c.eax = 0xAB12_CD34;
    c.edx = 0x5678_9ABC;
    unsafe { call_raw(&mut c) };
    (c.eax, c.edx)
}

#[test]
fn monthly_demand_and_class_id_preserve_edx_and_do_not_write() {
    let mut object = Object::new(0x400);
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    for demand in [0, 1, u32::MAX, 0x8000_0000] {
        sim.set_monthly_demand(demand);
        let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }.to_vec();
        assert_eq!(
            invoke(fc4p_00477110 as *const (), object.ptr(), &[]),
            (demand, 0x5678_9ABC)
        );
        assert_eq!(
            invoke(fc4p_006eabb0 as *const (), object.ptr(), &[]),
            (0x0990_C005, 0x5678_9ABC)
        );
        assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }, before);
    }
}

#[test]
fn query_interface_selects_each_subobject_and_retains_the_primary() {
    let vt = [0usize, retain as *const () as usize];
    let mut object = Object::new(0x400);
    unsafe { object.ptr().cast::<*const usize>().write(vt.as_ptr()) };
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    for (iid, offset) in [(1, 0), (0xE9E7_BFF5, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
        sim.set_references(8);
        let mut out = core::ptr::null_mut();
        assert_eq!(unsafe { fc4p_006eabc0(object.ptr(), iid, &mut out) }, 0xABCD_EF01);
        assert_eq!(out, unsafe { object.ptr().add(offset) });
        assert_eq!(sim.references(), 9);
        assert_eq!(calls(), [(4, object.ptr() as u32, 8)]);
    }
}
#[test]
fn query_interface_failure_retains_iid_arithmetic_and_destination() {
    let mut out = 0x1234usize as *mut u8;
    for iid in [0u32, 2, 0x4522_94A9, 0x4522_94AB, 0xDEAD_BEEF, 0xE4FD_A3D5, u32::MAX] {
        let expected = if iid < 0xE4FD_A3D4 {
            iid.wrapping_sub(0x4522_94AA)
        } else {
            iid
        } & !255;
        assert_eq!(unsafe { fc4p_006eabc0(core::ptr::null_mut(), iid, &mut out) }, expected);
        assert_eq!(out as usize, 0x1234);
    }
    assert!(calls().is_empty());
}
#[test]
fn release_uses_signed_count_and_reloads_vtable_after_shutdown() {
    let mut vt = [0usize; 0xD4 / 4];
    vt[4] = shutdown as *const () as usize;
    let mut next = [0usize; 0xD4 / 4];
    next[0xD0 / 4] = destroy as *const () as usize;
    let mut object = Object::new(0x400);
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    for count in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        unsafe {
            object.ptr().cast::<*const usize>().write(vt.as_ptr());
            object.ptr().add(0x40).cast::<*const usize>().write(next.as_ptr());
        }
        sim.set_references(count);
        let result = unsafe { fc4p_006eb190(object.ptr()) };
        if count > 1 {
            assert_eq!((result, sim.references()), ((count - 1) as u32, count - 1));
            assert!(calls().is_empty());
        } else {
            assert_eq!((result, sim.references()), (0, count));
            assert_eq!(
                calls(),
                [(0x10, object.ptr() as u32, 0), (0xD0, object.ptr() as u32, 1)]
            );
        }
    }
}
#[test]
fn supply_and_funding_tail_calls_keep_arguments_and_both_return_registers() {
    let mut vt = [0usize; 0x98 / 4];
    vt[0x94 / 4] = supply as *const () as usize;
    let mut object = Object::new(0x400);
    unsafe { object.ptr().cast::<*const usize>().write(vt.as_ptr()) };
    assert_eq!(
        invoke(fc4p_006ead50 as *const (), object.ptr(), &[77]),
        (0xABCD_EF01, 0x1234_5678)
    );
    assert_eq!(calls(), [(0x94, object.ptr() as u32, 77)]);
    let mut dvt = [0usize; 0x2C / 4];
    dvt[0x28 / 4] = funding as *const () as usize;
    let mut department = [dvt.as_ptr() as usize];
    let d = department.as_mut_ptr().cast();
    unsafe { PowerSim::from_ptr(object.ptr()) }.set_department(d);
    let mut out = 0u32;
    assert_eq!(
        invoke(fc4p_006eb180 as *const (), object.ptr(), &[&mut out as *mut u32 as u32]),
        (out_address(&mut out), 0x7654_3210)
    );
    assert_eq!(out, 0x3F60_0000);
    assert_eq!(calls(), [(0x28, d as u32, &mut out as *mut u32 as u32)]);
}
fn out_address(v: &mut u32) -> u32 {
    v as *mut u32 as u32
}
#[test]
fn strike_chance_copies_raw_global_bits_and_keeps_edx() {
    let mut zero = 0x8000_0000u32;
    let _global = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let mut out = [0xA5A5_A5A5u32, 0, 0x5A5A_5A5A];
    let dest = &mut out[1] as *mut u32;
    assert_eq!(
        invoke(fc4p_006eb130 as *const (), core::ptr::null_mut(), &[dest as u32]),
        (dest as u32, 0x5678_9ABC)
    );
    assert_eq!(out, [0xA5A5_A5A5, 0x8000_0000, 0x5A5A_5A5A]);
}
extern "thiscall" fn floating_funding(_this: *mut u8) -> f64 {
    0.123_456_789_012_345
}
#[test]
fn funding_tail_call_also_keeps_an_unrounded_st0_result() {
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0x2C / 4];
    vt[0x28 / 4] = floating_funding as *const () as usize;
    let mut department = [vt.as_ptr() as usize];
    unsafe { PowerSim::from_ptr(object.ptr()) }.set_department(department.as_mut_ptr().cast());
    let result = unsafe {
        call_st0(
            Conv::Thiscall,
            fc4p_006eb180 as *const () as usize,
            object.ptr() as u32,
            &[],
        )
    };
    assert_eq!(result.to_f64(), 0.123_456_789_012_345);
}
