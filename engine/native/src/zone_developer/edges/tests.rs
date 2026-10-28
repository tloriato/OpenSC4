// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, Object};
use std::cell::RefCell;

thread_local! {
    static CALLS: RefCell<Vec<(usize, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
}

extern "thiscall" fn bounds(this: *mut u8, rect: *mut i32) -> u32 {
    CALLS.with(|calls| calls.borrow_mut().push((this as usize, vec![])));
    unsafe { write_unaligned(rect.cast::<[i32; 4]>(), [-1, 2, 3, 4]) };
    0x1122_3344
}

extern "fastcall" fn surrounding(this: *mut u8, entry_edx: u32, rect: *const i32, list: *mut u8, flags: u32) -> u64 {
    let rect = unsafe { read_unaligned(rect.cast::<[u32; 4]>()) };
    CALLS.with(|calls| {
        calls.borrow_mut().push((
            this as usize,
            vec![rect[0], rect[1], rect[2], rect[3], list as u32, flags],
        ))
    });
    (u64::from(entry_edx) << 32) | 0xFEDC_BA98
}

#[test]
fn lot_bounds_then_surrounding_rect_preserves_arguments_and_result() {
    let mut lot_table = [0usize; 15];
    lot_table[14] = bounds as *const () as usize;
    let mut lot = [lot_table.as_ptr() as usize];
    let mut manager_table = [0usize; 11];
    manager_table[10] = surrounding as *const () as usize;
    let mut manager = [manager_table.as_ptr() as usize];
    let (lot, manager, list) = (
        lot.as_mut_ptr().cast(),
        manager.as_mut_ptr().cast(),
        0x1234usize as *mut u8,
    );
    assert_eq!(
        unsafe { fc4p_0072a750(manager, lot, list, 0xF0F1_F2F3) },
        0x0000_1234_FEDC_BA98
    );
    assert_eq!(
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut())),
        [
            (lot as usize, vec![]),
            (manager as usize, vec![u32::MAX, 2, 3, 4, 0x1234, 0xF0F1_F2F3])
        ]
    );
}

#[test]
fn vertex_edges_all_flag_bytes_wrap_and_keep_no_edge_eax() {
    let entry = 0xABCD_EF67;
    for rect in [
        [2i32, 3, 5, 7],
        [-3, -5, -1, -2],
        [i32::MIN, i32::MAX, i32::MAX, i32::MAX],
    ] {
        for flags in 0u32..256 {
            let [x1, z1, x2, z2] = rect;
            let mut out = [0x1234_5678i32; 4];
            let (want, eax) = if flags & 1 != 0 {
                ([x1, z1, x2.wrapping_add(1), z1], x2.wrapping_add(1) as u32)
            } else if flags & 2 != 0 {
                (
                    [x2.wrapping_add(1), z1, x2.wrapping_add(1), z2.wrapping_add(1)],
                    z2.wrapping_add(1) as u32,
                )
            } else if flags & 4 != 0 {
                (
                    [x1, z2.wrapping_add(1), x2.wrapping_add(1), z2.wrapping_add(1)],
                    x2.wrapping_add(1) as u32,
                )
            } else if flags & 8 != 0 {
                ([x1, z1, x1, z2.wrapping_add(1)], z2.wrapping_add(1) as u32)
            } else {
                (out, (entry & !255) | flags)
            };
            let got = unsafe {
                call_thiscall_with_eax(
                    fc4p_0072a790 as *const () as usize,
                    core::ptr::null_mut(),
                    &[rect.as_ptr() as u32, flags | 0x8765_0000, out.as_mut_ptr() as u32],
                    entry,
                )
            };
            let edx = if flags & 1 != 0 {
                z1 as u32
            } else if flags & 2 != 0 {
                x2.wrapping_add(1) as u32
            } else if flags & 4 != 0 {
                z2.wrapping_add(1) as u32
            } else if flags & 8 != 0 {
                x1 as u32
            } else {
                0
            };
            assert_eq!((out, got), (want, (eax, edx)));
            let mut alias = rect;
            unsafe {
                call_thiscall_with_eax(
                    fc4p_0072a790 as *const () as usize,
                    core::ptr::null_mut(),
                    &[alias.as_ptr() as u32, flags, alias.as_mut_ptr() as u32],
                    entry,
                )
            };
            assert_eq!(alias, if flags & 15 == 0 { rect } else { want });
        }
    }
}

#[test]
fn no_edge_keeps_entry_edx_and_does_not_read_the_rectangle() {
    let args = [0, 0x1234_0080, 0];
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        fc4p_0072a790 as *const () as usize,
        0,
        &args,
    );
    call.eax = 0xAABB_CC11;
    call.edx = 0x1234_5678;
    unsafe { crate::abi::call_raw(&mut call) };
    assert_eq!((call.eax, call.edx), (0xAABB_CC80, 0x1234_5678));
}

#[test]
fn neighbor_edges_city_limits_invalid_rectangles_and_output_aliasing() {
    let mut object = Object::new(0x30);
    let this = object.ptr();
    unsafe {
        write_unaligned(this.add(0x24).cast::<i32>(), 10);
        write_unaligned(this.add(0x28).cast::<i32>(), 12);
    }
    for rect in [
        [2i32, 3, 5, 7],
        [0, 0, 9, 11],
        [5, 7, 2, 3],
        [-2, -3, -1, -1],
        [0, 0, i32::MAX, i32::MAX],
    ] {
        for flags in 0u32..256 {
            let [x1, z1, x2, z2] = rect;
            let want = if flags & 1 != 0 {
                if z1 > 0 {
                    [x1, z1 - 1, x2, z1 - 1]
                } else {
                    [0, 0, -1, -1]
                }
            } else if flags & 2 != 0 {
                if x2 < 9 {
                    [x2 + 1, z1, x2 + 1, z2]
                } else {
                    [0, 0, -1, -1]
                }
            } else if flags & 4 != 0 {
                if z2 < 11 {
                    [x1, z2 + 1, x2, z2 + 1]
                } else {
                    [0, 0, -1, -1]
                }
            } else if flags & 8 != 0 {
                if x1 > 0 {
                    [x1 - 1, z1, x1 - 1, z2]
                } else {
                    [0, 0, -1, -1]
                }
            } else {
                [0, 0, -1, -1]
            };
            let mut out = [0x1234_5678i32; 4];
            let got = unsafe { fc4p_0072a820(this, rect.as_ptr(), flags, out.as_mut_ptr()) };
            assert_eq!(out, want);
            assert_eq!(
                got,
                (u64::from(want[2] >= want[0]) << 32)
                    | u64::from((out.as_ptr() as u32 & !255) | u32::from(want[2] >= want[0]))
            );
            // The empty output is written before the input is read, including when they are the same rectangle.
            let mut alias = rect;
            let got = unsafe { fc4p_0072a820(this, alias.as_ptr(), flags, alias.as_mut_ptr()) };
            let alias_want = if flags & 1 != 0 {
                [0, 0, -1, -1]
            } else if flags & 2 != 0 {
                [0, 0, 0, -1]
            } else if flags & 4 != 0 {
                [0, 0, -1, 0]
            } else {
                [0, 0, -1, -1]
            };
            assert_eq!(alias, alias_want);
            assert_eq!(got as u8, u8::from(alias_want[2] >= alias_want[0]));
        }
    }
}
