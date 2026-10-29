// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

thread_local! {
    static ALLOCATION: RefCell<[u32; 2]> = const { RefCell::new([0xA5A5_A5A5; 2]) };
    static CALLS: RefCell<Vec<(u32, u32, u32)>> = const { RefCell::new(Vec::new()) };
}

extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    assert_eq!(size, 8);
    CALLS.with(|calls| calls.borrow_mut().push((0, size, 0)));
    ALLOCATION.with(|memory| {
        *memory.borrow_mut() = [0xA5A5_A5A5; 2];
        memory.as_ptr().cast()
    })
}

extern "thiscall" fn append_fake(this: *mut u8, out: *mut *mut u8, before: *mut u8, value: *const *mut u8) -> u64 {
    unsafe {
        let cursor = read_unaligned(value);
        assert_eq!(Cursor::from_ptr(cursor).owner(), this.sub(4));
        CALLS.with(|calls| calls.borrow_mut().push((this as u32, before as u32, cursor as u32)));
        core::ptr::write_unaligned(out, 0x1234_5678usize as *mut u8);
    }
    0xAABB_CCDD_1234_5678
}

#[test]
fn cursor_allocation_initialization_and_registration() {
    let _allocate = fake(0x005E_55E0, allocate_fake as *const () as usize);
    let _append = fake(0x0072_BA10, append_fake as *const () as usize);
    let mut memory = Object::new(0x100);
    let primary = memory.ptr();
    for thunk in [false, true] {
        for has_position in [false, true] {
            unsafe {
                let mut position = if has_position {
                    0x8765_4321usize as *mut u8
                } else {
                    core::ptr::null_mut()
                };
                let grid = primary.add(0x60);
                let owner = CursorOwner::from_ptr(grid);
                owner.set_storage(&mut position);
                owner.set_cursors(0x2345_6789usize as *mut u8);
                let snapshot = core::slice::from_raw_parts(primary, 0x100).to_vec();
                let (result, edx) = if thunk {
                    call_thiscall_with_eax(fc4p_00734850 as *const () as usize, primary.add(4), &[], 0xABCD_1234)
                } else {
                    let result = fc4p_00734860(grid);
                    (result as u32, (result >> 32) as u32)
                };
                assert_eq!(edx, 0xAABB_CCDD);
                ALLOCATION.with(|allocation| {
                    assert_eq!(result, allocation.as_ptr() as u32);
                    assert_eq!(*allocation.borrow(), [position as u32, grid as u32]);
                });
                assert_eq!(core::slice::from_raw_parts(primary, 0x100), snapshot);
                CALLS.with(|calls| {
                    assert_eq!(
                        std::mem::take(&mut *calls.borrow_mut()),
                        [(0, 8, 0), (grid.add(4) as u32, 0x2345_6789, result)]
                    )
                });
            }
        }
    }
}
