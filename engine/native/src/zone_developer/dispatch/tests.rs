// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Classify(u32),
    Clear,
    Zone(u32, u32),
    Dezone(u32, u32),
    AddRef(u32),
    Shutdown(u32),
    Destroy(u32, u32),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

#[repr(C)]
struct Manager {
    table: *const usize,
    developer: *mut u8,
    result: u32,
}

extern "thiscall" fn classify(this: *mut Manager, zone: u32) -> u64 {
    unsafe {
        assert_eq!(ZoneDeveloper::from_ptr((*this).developer).current_zone(), zone);
        CALLS.with(|calls| calls.borrow_mut().push(Call::Classify(zone)));
        0x1122_3344_0000_0000 | u64::from((*this).result)
    }
}

extern "thiscall" fn clear(this: *mut u8) -> u64 {
    unsafe { assert_ne!(ZoneDeveloper::from_ptr(this).current_zone(), 0) };
    CALLS.with(|calls| calls.borrow_mut().push(Call::Clear));
    0xAABB_CCDD_8877_6655
}

extern "thiscall" fn zone(this: *mut u8, region: *mut u8, out: *mut u8) -> u64 {
    unsafe { assert_ne!(ZoneDeveloper::from_ptr(this).current_zone(), 0) };
    CALLS.with(|calls| calls.borrow_mut().push(Call::Zone(region as u32, out as u32)));
    0x1234_5678_9ABC_DEF0
}

extern "thiscall" fn dezone(this: *mut u8, region: *mut u8, flag: u32) -> u64 {
    unsafe { assert_ne!(ZoneDeveloper::from_ptr(this).current_zone(), 0) };
    CALLS.with(|calls| calls.borrow_mut().push(Call::Dezone(region as u32, flag)));
    0xFEDC_BA98_7654_3210
}

#[test]
fn parcellization_dispatches_by_low_byte_and_resets_the_zone() {
    // The core's DoParcellization workflow, with calls observed at the game boundary.
    let _clear = fake(0x0072_C0E0, clear as *const () as usize);
    let _zone = fake(0x0073_3770, zone as *const () as usize);
    let _dezone = fake(0x0072_E6A0, dezone as *const () as usize);
    let mut memory = Object::new(0x100);
    let primary = memory.ptr();
    let mut table = [0usize; 14];
    table[13] = classify as *const () as usize;
    let mut manager = Manager {
        table: table.as_ptr(),
        developer: primary,
        result: 0,
    };
    unsafe {
        let dev = ZoneDeveloper::from_ptr(primary);
        dev.set_zone_manager((&mut manager as *mut Manager).cast());
        for (result, selected, flag) in [
            (0xABCD_0001, 1, 0xFFFF_FF00),
            (0xABCD_0100, 10, 0x1234_56FE),
            (0xABCD_0080, u32::MAX, 1),
        ] {
            manager.result = result;
            assert_eq!(manager.result, result);
            let snapshot = core::slice::from_raw_parts(primary, 0x100).to_vec();
            let returned = fc4p_00733fa0(primary, 0x1020_3040usize as *mut u8, selected, flag);
            assert_eq!(dev.current_zone(), 0);
            assert_eq!(core::slice::from_raw_parts(primary, 0x100), snapshot);
            let expected = if result as u8 != 0 {
                assert_eq!(returned, 0x1234_5678_9ABC_DEF0);
                vec![Call::Classify(selected), Call::Clear, Call::Zone(0x1020_3040, 0)]
            } else {
                assert_eq!(returned, 0xFEDC_BA98_7654_3210);
                vec![Call::Classify(selected), Call::Dezone(0x1020_3040, flag)]
            };
            CALLS.with(|calls| assert_eq!(std::mem::take(&mut *calls.borrow_mut()), expected));
        }
    }
}

extern "thiscall" fn add_ref(this: *mut u8) -> u32 {
    CALLS.with(|calls| calls.borrow_mut().push(Call::AddRef(this as u32)));
    0xABCD_EF77
}

extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
    CALLS.with(|calls| calls.borrow_mut().push(Call::Shutdown(this as u32)));
    0
}

extern "thiscall" fn destroy(this: *mut u8, flag: u32) -> u32 {
    CALLS.with(|calls| calls.borrow_mut().push(Call::Destroy(this as u32, flag)));
    0
}

#[test]
fn query_thunks_adjust_this_and_forward_both_stack_words() {
    let mut memory = Object::new(0x100);
    let primary = memory.ptr();
    let mut table = [0usize; 11];
    table[1] = add_ref as *const () as usize;
    unsafe {
        let dev = ZoneDeveloper::from_ptr(primary);
        dev.set_primary_vtable(table.as_ptr() as u32);
        for (function, offset) in [
            (fc4p_007347f0 as *const () as usize, 4),
            (fc4p_00734810 as *const () as usize, 12),
        ] {
            for (id, target) in [(1, 0), (0x6819_05BD, 4), (0x4522_94AA, 8), (0xE4FD_A3D4, 12)] {
                let mut out = core::ptr::null_mut::<u8>();
                let returned = call_thiscall_with_eax(
                    function,
                    primary.add(offset),
                    &[id, &mut out as *mut _ as u32],
                    0x2233_4455,
                )
                .0;
                assert_eq!(returned, 0xABCD_EF01);
                assert_eq!(out, primary.add(target));
                CALLS
                    .with(|calls| assert_eq!(std::mem::take(&mut *calls.borrow_mut()), [Call::AddRef(primary as u32)]));
            }
            let mut out = 0x1122_3344usize as *mut u8;
            assert_eq!(
                call_thiscall_with_eax(
                    function,
                    primary.add(offset),
                    &[0xFFFF_FFFF, &mut out as *mut _ as u32],
                    0
                )
                .0,
                0xFFFF_FF00
            );
            assert_eq!(out as u32, 0x1122_3344);
            CALLS.with(|calls| assert!(calls.borrow().is_empty()));
        }
    }
}

#[test]
fn release_thunks_use_the_shared_count_and_final_reference_calls() {
    let mut memory = Object::new(0x100);
    let primary = memory.ptr();
    let mut table = [0usize; 11];
    table[4] = shutdown as *const () as usize;
    table[10] = destroy as *const () as usize;
    unsafe {
        let dev = ZoneDeveloper::from_ptr(primary);
        dev.set_primary_vtable(table.as_ptr() as u32);
        for (function, offset) in [
            (fc4p_00734820 as *const () as usize, 4),
            (fc4p_00734830 as *const () as usize, 8),
            (fc4p_00734840 as *const () as usize, 12),
        ] {
            for count in [2, 1, 0, -1] {
                dev.set_references(count);
                let result = call_thiscall_with_eax(function, primary.add(offset), &[], 0xAABB_CCDD).0;
                if count > 1 {
                    assert_eq!((result, dev.references()), (1, 1));
                    CALLS.with(|calls| assert!(calls.borrow().is_empty()));
                } else {
                    assert_eq!((result, dev.references()), (0, count));
                    CALLS.with(|calls| {
                        assert_eq!(
                            std::mem::take(&mut *calls.borrow_mut()),
                            [Call::Shutdown(primary as u32), Call::Destroy(primary as u32, 1)]
                        )
                    });
                }
            }
        }
    }
}
