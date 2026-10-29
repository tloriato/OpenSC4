// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Counter cases from the core power tests, exercised through the original interfaces.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::Object;
use std::cell::RefCell;

game_fn! { fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 = "thiscall" 0x006E_ABC0; }
game_fn! { fn release(this: *mut u8) -> u32 = "thiscall" 0x006E_B190; }

fn invoke(entry: *const (), this: *mut u8, args: &[u32]) -> (u32, u32) {
    let mut c = RawCall::new(Conv::Thiscall, entry as usize, this as u32, args);
    c.eax = 0x1122_3344;
    c.edx = 0x8765_4321;
    unsafe { call_raw(&mut c) };
    (c.eax, c.edx)
}

#[test]
fn getters_return_all_bits_keep_edx_and_leave_memory_unchanged() {
    let mut object = Object::new(0x400);
    let p = object.ptr();
    let fields = [
        (fc4p_006ef9f0 as *const (), 0x2DC),
        (fc4p_006efa10 as *const (), 0x2B8),
        (fc4p_006efa20 as *const (), 0x308),
        (fc4p_006efa30 as *const (), 0x30C),
        (fc4p_006efa40 as *const (), 0x310),
        (fc4p_006efa50 as *const (), 0x80),
    ];
    for (entry, offset) in fields {
        for word in [0, 1, 0x7FFF_FFFF, 0x8000_0000, u32::MAX, 0xDEAD_BEEF] {
            unsafe { p.add(offset).cast::<u32>().write_unaligned(word) };
            let before = unsafe { std::slice::from_raw_parts(p, 0x400) }.to_vec();
            assert_eq!(invoke(entry, p, &[]), (word, 0x8765_4321));
            assert_eq!(unsafe { std::slice::from_raw_parts(p, 0x400) }, before);
        }
    }
}

#[test]
fn plant_types_include_negative_counts_and_unchecked_adjacent_words() {
    let mut object = Object::new(0x400);
    let p = object.ptr();
    let sim = unsafe { PowerSim::from_ptr(p) };
    // A plant changed from type 1 to type 3 before removal: one remains in type 1, minus one in type 3.
    sim.set_plant_counts([0, 1, 0, u32::MAX, 0, 0, 0, 0, 0, 0]);
    for (index, expected) in [(1, 1), (3, u32::MAX), (9, 0)] {
        assert_eq!(invoke(fc4p_006efa00 as *const (), p, &[index]), (expected, 0x8765_4321));
    }
    // No validation: -1 selects plantCount; 10 selects the overworked count. High index bits wrap away.
    sim.set_plant_count(0xABCD_EF01);
    sim.set_overworked_count(0x1020_3040);
    let before = unsafe { std::slice::from_raw_parts(p, 0x400) }.to_vec();
    for (index, expected) in [(u32::MAX, 0xABCD_EF01), (10, 0x1020_3040), (0x4000_0001, 1)] {
        assert_eq!(invoke(fc4p_006efa00 as *const (), p, &[index]), (expected, 0x8765_4321));
    }
    assert_eq!(unsafe { std::slice::from_raw_parts(p, 0x400) }, before);
}

thread_local! {
    static CALLS: RefCell<Vec<(u32, u32, u32)>> = const { RefCell::new(Vec::new()) };
}
fn calls() -> Vec<(u32, u32, u32)> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
extern "thiscall" fn retain(this: *mut u8) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((4, this as u32, 0)));
    0xABCD_EF34
}
extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((0x10, this as u32, 0)));
    0x1234_5678
}
extern "thiscall" fn destroy(this: *mut u8, flag: u32) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((0xD0, this as u32, flag)));
    0x8765_4321
}

#[test]
fn serializable_query_forwards_both_stack_arguments_to_the_primary_interface() {
    let vt = [0usize, retain as *const () as usize];
    let mut object = Object::new(0x400);
    let p = object.ptr();
    unsafe { p.cast::<*const usize>().write(vt.as_ptr()) };
    let mut out = core::ptr::null_mut();
    for (iid, offset) in [(1, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
        let result = invoke(
            fc4p_006efa80 as *const (),
            unsafe { p.add(8) },
            &[iid, &mut out as *mut _ as u32],
        );
        assert_eq!(result.0, 0xABCD_EF01);
        assert_eq!(out, unsafe { p.add(offset) });
        assert_eq!(calls(), [(4, p as u32, 0)]);
        assert_eq!(unsafe { query(p, iid, &mut out) }, result.0);
        calls();
    }
    out = 0x1234usize as *mut u8;
    let result = invoke(
        fc4p_006efa80 as *const (),
        unsafe { p.add(8) },
        &[0xDEAD_BEEF, &mut out as *mut _ as u32],
    );
    assert_eq!(result.0, unsafe { query(p, 0xDEAD_BEEF, &mut out) });
    assert_eq!(out as usize, 0x1234);
    assert!(calls().is_empty());
}

#[test]
fn release_thunks_adjust_each_interface_for_decrement_and_destruction() {
    let mut vt = [0usize; 0xD4 / 4];
    vt[0x10 / 4] = shutdown as *const () as usize;
    vt[0xD0 / 4] = destroy as *const () as usize;
    let mut object = Object::new(0x400);
    let p = object.ptr();
    unsafe { p.cast::<*const usize>().write(vt.as_ptr()) };
    let sim = unsafe { PowerSim::from_ptr(p) };
    for (entry, offset) in [(fc4p_006efa90 as *const (), 4), (fc4p_006efaa0 as *const (), 8)] {
        for count in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            sim.set_references(count);
            let result = invoke(entry, unsafe { p.add(offset) }, &[]).0;
            let trace = calls();
            sim.set_references(count);
            assert_eq!(result, unsafe { release(p) });
            assert_eq!(trace, calls());
            if count > 1 {
                assert_eq!(result, (count - 1) as u32);
                assert_eq!(sim.references(), count - 1);
                assert!(trace.is_empty());
            } else {
                assert_eq!(result, 0);
                assert_eq!(trace, [(0x10, p as u32, 0), (0xD0, p as u32, 1)]);
            }
        }
    }
}
