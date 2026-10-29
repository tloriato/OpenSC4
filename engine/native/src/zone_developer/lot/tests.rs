// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

thread_local! {
    static ALLOCATIONS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static EVENTS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    static OWNER: RefCell<usize> = const { RefCell::new(0) };
}

extern "thiscall" fn base(this: *mut u8) -> u32 {
    let owner = unsafe { Lot::from_ptr(this.sub(8)) };
    assert_eq!(owner.primary_vtable(), 0x00AB_16E8);
    assert_eq!(owner.secondary_vtable(), 0x00A8_0784);
    // The base owns this span; the lot constructor preserves the fields it does not initialise itself.
    unsafe { core::ptr::write_bytes(this, 0xD3, 0x14) };
    OWNER.with(|o| *o.borrow_mut() = owner.as_ptr() as usize);
    EVENTS.with(|events| events.borrow_mut().push((0x005F_F100, vec![this as u32])));
    0x1234
}

extern "C" fn alloc(size: u32) -> *mut u8 {
    EVENTS.with(|events| events.borrow_mut().push((0x0090_CF54, vec![size])));
    let ptr = ALLOCATIONS.with(|a| a.borrow_mut().remove(0)) as *mut u8;
    let lot = unsafe { Lot::from_ptr(OWNER.with(|o| *o.borrow()) as *mut u8) };
    assert_eq!(lot.primary_vtable(), 0x00AB_19B0);
    assert_eq!(lot.secondary_vtable(), 0x00AB_1994);
    assert_eq!(lot.occupant_vtable(), 0x00AB_1930);
    assert_eq!(lot.serializable_vtable(), 0x00AB_1914);
    ptr
}

extern "thiscall" fn zone(this: *mut u8, x: u32, z: u32) -> u32 {
    EVENTS.with(|events| events.borrow_mut().push((0x30, vec![this as u32, x, z])));
    0xAABB_CC87
}

#[test]
fn constructor_orientations_wrapping_and_exact_byte_writes() {
    constructor_cases(
        &[(10, 20, 3, 5), (-1, 254, 0, 4), (i32::MAX, i32::MIN, 2, -1)],
        &[0, 1, 2, 3, 4, 255, 256, 259],
    );
}

#[test]
fn rectangular_lot_dimensions_for_facings_two_and_three() {
    constructor_cases(&[(10, 20, 3, 5), (2, 4, 7, 1)], &[2, 3]);
}

fn constructor_cases(cases: &[(i32, i32, i32, i32)], facings: &[u32]) {
    let _base = fake(0x005F_F100, base as *const () as usize);
    let _alloc = fake(0x0090_CF54, alloc as *const () as usize);
    let mut table = [0usize; 13];
    table[12] = zone as *const () as usize;
    let mut manager = [table.as_ptr() as usize];
    let mut manager_ptr = manager.as_mut_ptr().cast::<u8>();
    let _global = fake_data(0x00B4_3D14, (&mut manager_ptr as *mut *mut u8).cast());
    for &(x, z, width, height) in cases {
        for &facing in facings {
            let mut object = Object::new(0x90);
            let ptr = object.ptr();
            unsafe { core::ptr::write_bytes(ptr, 0xA5, 0x90) };
            let mut first = [0xCCu8; 0x18];
            let mut second = [0xCCu8; 0x18];
            let mut list = [0xCCu8; 0x34];
            ALLOCATIONS.with(|a| {
                *a.borrow_mut() = vec![
                    first.as_mut_ptr() as usize,
                    second.as_mut_ptr() as usize,
                    list.as_mut_ptr() as usize,
                ]
            });
            let args = [x as u32, z as u32, width as u32, height as u32, facing];
            let mut call = RawCall::new(Conv::Thiscall, fc4p_006c1e50 as *const () as usize, ptr as u32, &args);
            unsafe { call_raw(&mut call) };
            assert_eq!(call.eax, ptr as u32);
            let lot = unsafe { Lot::from_ptr(ptr) };
            let x1 = x.wrapping_add(width).wrapping_sub(1) as u8;
            let z1 = z.wrapping_add(height).wrapping_sub(1) as u8;
            let anchor = match facing as u8 {
                0 => [x1, z as u8, width as u8, height as u8],
                1 => [x1, z1, height as u8, width as u8],
                2 => [x as u8, z1, width as u8, height as u8],
                3 => [x as u8, z as u8, height as u8, width as u8],
                _ => [0; 4],
            };
            assert_eq!(lot.bounds(), [x as u8, z as u8, x1, z1]);
            assert_eq!(lot.anchor(), anchor);
            assert_eq!((lot.facing(), lot.zone(), lot.marker()), (facing as u8, 0x87, 255));
            assert_eq!(
                (lot.first_header(), lot.second_header(), lot.list_header()),
                (first.as_mut_ptr(), second.as_mut_ptr(), list.as_mut_ptr())
            );
            let mut expected = [0xA5u8; 0x90];
            expected[8..0x1C].fill(0xD3);
            for (offset, value) in [
                (0, 0x00AB_19B0u32),
                (4, 0x00AB_1994),
                (8, 0x00AB_1930),
                (0x1C, 0x00AB_1914),
            ] {
                expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            for (start, end) in [(0x24, 0x48), (0x4C, 0x68), (0x6C, 0x78), (0x78, 0x83)] {
                expected[start..end].fill(0);
            }
            for (offset, value) in [
                (0x40, first.as_mut_ptr() as u32),
                (0x60, second.as_mut_ptr() as u32),
                (0x84, list.as_mut_ptr() as u32),
            ] {
                expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            expected[0x20..0x24].copy_from_slice(&lot.bounds());
            expected[0x7A..0x7E].copy_from_slice(&anchor);
            expected[0x7E] = facing as u8;
            expected[0x81] = 0x87;
            expected[0x88] = 255;
            assert_eq!(unsafe { core::slice::from_raw_parts(ptr, 0x90) }, expected);
            for bytes in [&first, &second] {
                let mut expected = [0xCCu8; 0x18];
                expected[0] = 0;
                expected[4..8].fill(0);
                let self_ptr = bytes.as_ptr() as u32;
                expected[8..12].copy_from_slice(&self_ptr.to_le_bytes());
                expected[12..16].copy_from_slice(&self_ptr.to_le_bytes());
                assert_eq!(*bytes, expected);
            }
            let mut expected_list = [0xCCu8; 0x34];
            expected_list[..4].copy_from_slice(&(list.as_ptr() as u32).to_le_bytes());
            expected_list[4..8].copy_from_slice(&(list.as_ptr() as u32).to_le_bytes());
            assert_eq!(list, expected_list);
            assert_eq!(
                EVENTS.with(|e| core::mem::take(&mut *e.borrow_mut())),
                [
                    (0x005F_F100, vec![ptr.wrapping_add(8) as u32]),
                    (0x0090_CF54, vec![0x18]),
                    (0x0090_CF54, vec![0x18]),
                    (0x0090_CF54, vec![0x34]),
                    (
                        0x30,
                        vec![manager_ptr as u32, u32::from(anchor[0]), u32::from(anchor[1])]
                    )
                ]
            );
        }
    }
}
