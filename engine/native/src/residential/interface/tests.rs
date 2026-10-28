// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Original-layout counterparts of the core's identity, grid, and strike predicate cases.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Retain(usize),
    Shutdown(usize),
    Delete(usize, u32),
    Holder(usize),
    Property(usize, u32, u32),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
fn log(call: Call) {
    CALLS.with(|v| v.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}

#[repr(C)]
struct Retained {
    vtable: *const usize,
    result: u64,
    outputs: *mut *mut u8,
    replace: *mut u8,
}
unsafe extern "thiscall" fn retain(this: *mut Retained) -> u64 {
    unsafe {
        log(Call::Retain(this as usize));
        if !(*this).outputs.is_null() {
            (*this).outputs.add(1).write((*this).replace);
        }
        (*this).result
    }
}
extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
    log(Call::Shutdown(this as usize));
    0x1234
}
extern "thiscall" fn delete(this: *mut u8, flag: u32) -> u32 {
    log(Call::Delete(this as usize, flag));
    0x5678
}

#[test]
fn identity_interfaces_and_unknown_id_residue() {
    let mut table = [0usize; 2];
    table[1] = retain as *const () as usize;
    let mut obj = Retained {
        vtable: table.as_ptr(),
        result: 0x1234_5678,
        outputs: core::ptr::null_mut(),
        replace: core::ptr::null_mut(),
    };
    let p = (&mut obj as *mut Retained).cast::<u8>();
    unsafe {
        assert_eq!(
            call_thiscall_with_eax(fc4p_006f6e50 as *const () as usize, p, &[], 0).0,
            0x4990_C013
        );
        for (id, offset) in [(1, 0), (0x077A_C1EE, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
            let mut out = core::ptr::null_mut();
            assert_eq!(fc4p_006f6e60(p, id, &mut out), 0x1234_5601);
            assert_eq!(out, p.add(offset));
            assert_eq!(take(), [Call::Retain(p as usize)]);
        }
        for id in [0u32, 2, 0x077A_C1ED, 0x077A_C1EF, 0x4522_94A9, 0x4522_94AB, u32::MAX] {
            let mut out = 0x1234usize as *mut u8;
            let want = if id < 0x4522_94AA {
                id.wrapping_sub(0x077A_C1EE)
            } else {
                id
            } & !0xFF;
            assert_eq!(fc4p_006f6e60(p, id, &mut out), want);
            assert_eq!(out as usize, 0x1234);
            assert!(take().is_empty());
        }
    }
}

#[test]
fn release_signed_count_and_shutdown_before_delete() {
    let mut memory = Object::new(0x350);
    let p = memory.ptr();
    let sim = unsafe { ResidentialSim::from_ptr(p) };
    let mut table = [0usize; 61];
    table[4] = shutdown as *const () as usize;
    table[60] = delete as *const () as usize;
    unsafe {
        p.cast::<*const usize>().write(table.as_ptr());
    }
    for count in [3, 2, 1, 0, -1, i32::MIN] {
        sim.set_references(count);
        let result = unsafe { fc4p_006f7dd0(p) };
        if count > 1 {
            assert_eq!(result, (count - 1) as u32);
            assert_eq!(sim.references(), count - 1);
            assert!(take().is_empty());
        } else {
            assert_eq!(result, 0);
            assert_eq!(sim.references(), count);
            assert_eq!(take(), [Call::Shutdown(p as usize), Call::Delete(p as usize, 1)]);
        }
    }
}

#[test]
fn histogram_reset_changes_only_the_original_fields() {
    let mut memory = Object::new(0x360);
    let p = memory.ptr();
    unsafe {
        p.write_bytes(0xA5, 0x360);
    }
    let before = unsafe { core::slice::from_raw_parts(p, 0x360).to_vec() };
    unsafe {
        fc4p_006f6ee0(p);
    }
    for (i, &old) in before.iter().enumerate() {
        let cleared = (0x1C4..0x264).contains(&i) || (0x348..0x350).contains(&i) || (0xA8..0xAC).contains(&i);
        assert_eq!(unsafe { *p.add(i) }, if cleared { 0 } else { old }, "offset {i:x}");
    }
}

#[test]
fn strike_bytes_population_and_proximity_maps() {
    let mut memory = Object::new(0x350);
    let p = memory.ptr();
    let sim = unsafe { ResidentialSim::from_ptr(p) };
    for value in [0, 1, 2, 0x80, 0xFF] {
        sim.set_school_strike(value);
        sim.set_health_strike(value);
        for export in [fc4p_006f6f10 as *const () as usize, fc4p_006f6f20 as *const () as usize] {
            let mut c = RawCall::new(Conv::Thiscall, export, p as u32, &[]);
            c.eax = 0xAABB_CCDD;
            c.edx = 0x8765_4321;
            unsafe {
                call_raw(&mut c);
            }
            assert_eq!(c.eax, 0xAABB_CC00 | u32::from(value != 0));
            assert_eq!(c.edx, 0x8765_4321);
        }
    }
    for value in [0, -1, i32::MIN, i32::MAX] {
        sim.set_population(value);
        let mut c = RawCall::new(Conv::Thiscall, fc4p_006f6f70 as *const () as usize, p as u32, &[]);
        c.edx = 0x4321;
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (value as u32, 0x4321));
    }
    sim.set_proximity_low(0x1100usize as *mut u8);
    sim.set_proximity_medium(0x2200usize as *mut u8);
    sim.set_proximity_high(0x3300usize as *mut u8);
    for wealth in [0u32, 1, 2, 3, 4, u32::MAX] {
        let result = unsafe { fc4p_006f6f90(p, 0xDEAD_BEEF, wealth) };
        let want = match wealth {
            1 => 0x1100,
            2 => 0x2200,
            3 => 0x3300,
            _ => 0,
        };
        assert_eq!(result, 0xDEAD_BEEF_0000_0000 | want);
    }
}

#[test]
fn byte_callback_writes_one_byte_and_keeps_register_residue() {
    let mut dest = [0xAAu8; 3];
    let args = [unsafe { dest.as_mut_ptr().add(1) } as u32, 0x9876_5432, 0x1111];
    let mut c = RawCall::new(Conv::Cdecl, fc4p_006f6f80 as *const () as usize, 0, &args);
    c.eax = 0xAABB_CCDD;
    c.edx = 0x8765_4321;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!(dest, [0xAA, 0x32, 0xAA]);
    assert_eq!((c.eax, c.edx), (0xAABB_CC32, 0x8765_4321));
}

extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    log(Call::Holder(this as usize));
    0x2468usize as *mut u8
}
thread_local! { static PROPERTY: RefCell<(u32,u32,bool)> = const { RefCell::new((0,0,false)) }; }
unsafe extern "C" fn property(holder: *mut u8, id: u32, out: *mut u32) -> u32 {
    log(Call::Property(holder as *const () as usize, id, unsafe { *out }));
    PROPERTY.with(|p| {
        let (result, value, write) = *p.borrow();
        if write {
            unsafe {
                *out = value;
            }
        }
        result
    })
}

#[unsafe(naked)]
unsafe extern "thiscall" fn invoke_building_strike(_this: *mut u8, _building: *mut u8) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {port}", "mov edx, ecx", "ret 4",port=sym fc4p_006f6f30)
}

#[test]
fn building_strike_day_missing_zero_nonzero_and_failure_residue() {
    let _fake = fake(0x005F_D480, property as *const () as usize);
    let mut table = [0usize; 7];
    table[6] = holder as *const () as usize;
    let mut vptr = table.as_ptr();
    let p = (&mut vptr as *mut *const usize).cast::<u8>();
    for (result, value, write, want) in [
        (0xABCD_1200, 3, true, 0xABCD_1200),
        (1, 0, true, 0),
        (1, 1, true, 1),
        (1, u32::MAX, true, 1),
        (1, 0, false, 0),
    ] {
        PROPERTY.with(|p| *p.borrow_mut() = (result, value, write));
        let result = unsafe { invoke_building_strike(core::ptr::null_mut(), p) };
        assert_eq!(result as u32, want);
        assert_eq!((result >> 32) as u32, if write { value } else { 0 });
        assert_eq!(
            take(),
            [Call::Holder(p as usize), Call::Property(0x2468, 0xEAA6_9F83, 0)]
        );
    }
}

#[test]
fn grid_outputs_precede_retains_and_aliases_are_reread() {
    let mut table = [0usize; 2];
    table[1] = retain as *const () as usize;
    let mut grids: Vec<Retained> = (0..4)
        .map(|i| Retained {
            vtable: table.as_ptr(),
            result: 0xABC0_0000_1234_0000 | i,
            outputs: core::ptr::null_mut(),
            replace: core::ptr::null_mut(),
        })
        .collect();
    let pointers: Vec<*mut u8> = grids.iter_mut().map(|g| (g as *mut Retained).cast()).collect();
    let mut memory = Object::new(0x350);
    let p = memory.ptr();
    let sim = unsafe { ResidentialSim::from_ptr(p) };
    sim.set_eq_low(pointers[0]);
    sim.set_eq_medium(pointers[1]);
    sim.set_eq_high(pointers[2]);
    sim.set_population_low(pointers[0]);
    sim.set_population_medium(pointers[1]);
    sim.set_population_high(pointers[2]);
    for export in [fc4p_006f72c0, fc4p_006f7340] {
        let mut outputs = [core::ptr::null_mut(); 3];
        let result = unsafe {
            export(
                p,
                outputs.as_mut_ptr(),
                outputs.as_mut_ptr().add(1),
                outputs.as_mut_ptr().add(2),
            )
        };
        assert_eq!(outputs.as_slice(), &pointers[..3]);
        assert_eq!(result, grids[2].result);
        assert_eq!(
            take(),
            pointers[..3]
                .iter()
                .map(|p| Call::Retain(*p as usize))
                .collect::<Vec<_>>()
        );
        // The first retain changes the second output, which must be read only when it is used.
        grids[0].outputs = outputs.as_mut_ptr();
        grids[0].replace = pointers[3];
        unsafe {
            export(
                p,
                outputs.as_mut_ptr(),
                outputs.as_mut_ptr().add(1),
                outputs.as_mut_ptr().add(2),
            );
        }
        assert_eq!(outputs[1], pointers[3]);
        assert_eq!(
            take(),
            [
                Call::Retain(pointers[0] as usize),
                Call::Retain(pointers[3] as usize),
                Call::Retain(pointers[2] as usize)
            ]
        );
        grids[0].outputs = core::ptr::null_mut();
        // All outputs may alias; every retain then observes the final stored pointer.
        let mut out = core::ptr::null_mut();
        unsafe {
            export(p, &mut out, &mut out, &mut out);
        }
        assert_eq!(
            take(),
            [
                Call::Retain(pointers[2] as usize),
                Call::Retain(pointers[2] as usize),
                Call::Retain(pointers[2] as usize)
            ]
        );
    }
}
