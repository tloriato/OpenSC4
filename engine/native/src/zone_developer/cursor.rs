// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Allocate and register a cursor on the embedded lot grid.

use super::Port;
use core::ptr::read_unaligned;

layout! {
    /// A grid's current storage and the list of registered cursors.
    struct CursorOwner {
        0x00 => storage / set_storage: *mut *mut u8,
        0x04 => cursors / set_cursors: *mut u8,
    }
}

layout! {
    /// The cursor's current position and its owning grid.
    struct Cursor {
        0x00 => position / set_position: *mut u8,
        0x04 => owner / set_owner: *mut u8,
    }
}

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn append_cursor(this: *mut u8, out: *mut *mut u8, before: *mut u8, value: *const *mut u8) = "thiscall" 0x0072_BA10; }
game_fn! { #[allow(dead_code)] fn create_cursor(this: *mut u8) -> *mut u8 = "thiscall" 0x0073_4860; }

port! {
    /// Adjust the lot-manager interface to its embedded grid and forward cursor construction.
    #[port(address = 0x0073_4850, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CreateLotCursor", naked)]
    pub unsafe extern "C" fn fc4p_00734850() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 0x5c", "jmp dword ptr [2f]", "2: .long 0x00734860");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 0x5c", "jmp {}", sym fc4p_00734860);
    }
}

port! {
    /// Allocate eight bytes, attach the grid, and copy its first position when storage is present.
    /// An absent storage pointer leaves the position word untouched. Register the cursor before
    /// the list's stored header and return the allocated cursor.
    #[port(address = 0x0073_4860, owner = "zonedeveloper", name = "CreateRegisteredLotCursor")]
    pub unsafe extern "thiscall" fn fc4p_00734860(this: *mut u8) -> *mut u8 {
        unsafe {
            let owner = CursorOwner::from_ptr(this);
            let cursor = allocate(8);
            Cursor::from_ptr(cursor).set_owner(this);
            let storage = owner.storage();
            if !storage.is_null() {
                Cursor::from_ptr(cursor).set_position(read_unaligned(storage));
            }
            let mut node = core::mem::MaybeUninit::<*mut u8>::uninit();
            append_cursor(this.add(4), node.as_mut_ptr(), owner.cursors(), &cursor);
            // The helper may use its pointer-to-cursor argument as scratch storage.
            read_unaligned(&cursor)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00734850::PORT, fc4p_00734860::PORT];

#[cfg(test)]
mod tests;
