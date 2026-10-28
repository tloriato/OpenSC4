// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Copy the game's two network record vectors, and remove retained preview occupants.

use super::*;

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn copy_large(begin: *mut u8, end: *mut u8, destination: *mut u8, scratch: *mut *mut u8) -> *mut u8 = "C" 0x0062_E970; }
game_fn! { fn copy_small(begin: *mut u8, end: *mut u8, destination: *mut u8, scratch: *mut *mut u8) -> *mut u8 = "C" 0x0062_4E60; }
game_fn! { fn clear_preview(this: *mut u8) -> u32 = "thiscall" 0x0062_08D0; }
game_fn! { fn erase_occupants(this: *mut u8, root: *mut u8) = "thiscall" 0x0046_76A0; }
game_data! { static occupant_manager: *mut u8 = 0x00B4_3D0C; }

unsafe fn copy_vector(this: *mut u8, source: *mut u8, shift: u32) -> *mut u8 {
    unsafe {
        let begin: u32 = read(source, 0);
        let end: u32 = read(source, 4);
        let count = (end.wrapping_sub(begin) as i32) >> shift;
        write(this, 0, 0u32);
        write(this, 4, 0u32);
        write(this, 8, 0u32);
        let buffer = if count == 0 {
            core::ptr::null_mut()
        } else {
            allocate((count as u32).wrapping_shl(shift))
        };
        let capacity = buffer.wrapping_add((count as u32).wrapping_shl(shift) as usize);
        write(this, 0, buffer);
        write(this, 4, buffer);
        write(this, 8, capacity);
        let end = read(source, 4);
        let begin = read(source, 0);
        let mut scratch = source;
        let copied = if shift == 5 {
            copy_large(begin, end, buffer, &mut scratch)
        } else {
            copy_small(begin, end, buffer, &mut scratch)
        };
        write(this, 4, copied);
        this
    }
}
port! {
    /// Copy-construct a vector of 32-byte network records, allocating only for nonzero signed length.
    #[port(address = 0x0062_4EA0, owner = "networktool", name = "CopyLargeNetworkRecordVector")]
    pub unsafe extern "thiscall" fn fc4p_00624ea0(this: *mut u8, source: *mut u8) -> *mut u8 {
        unsafe { copy_vector(this, source, 5) }
    }
}
port! {
    /// Copy-construct a vector of 16-byte network records. The copy helper supplies the final end cursor.
    #[port(address = 0x0062_5190, owner = "networktool", name = "CopySmallNetworkRecordVector")]
    pub unsafe extern "thiscall" fn fc4p_00625190(this: *mut u8, source: *mut u8) -> *mut u8 {
        unsafe { copy_vector(this, source, 4) }
    }
}
unsafe fn successor(mut node: *mut u8) -> *mut u8 {
    unsafe {
        let mut link: *mut u8 = read(node, 0xC);
        if !link.is_null() {
            node = link;
            loop {
                link = read(node, 8);
                if link.is_null() {
                    return node;
                }
                node = link;
            }
        }
        link = read(node, 4);
        while node == read::<*mut u8>(link, 0xC) {
            node = link;
            link = read(link, 4);
        }
        if read::<*mut u8>(node, 0xC) != link {
            node = link;
        }
        node
    }
}
port! {
    /// Clear preview rules, remove each retained occupant with `(true, 0)`, release it, and erase the
    /// occupant tree. Keep empty headers unchanged; EAX retains the count or last reset header.
    #[port(address = 0x0062_5580, owner = "networktool", name = "ClearRetainedNetworkOccupants")]
    pub unsafe extern "thiscall" fn fc4p_00625580(this: *mut u8) -> u32 {
        unsafe {
            clear_preview(read(this, 0x1F0));
            let head: *mut u8 = read(this, 0x228);
            let manager = *occupant_manager();
            let mut node: *mut u8 = read(head, 8);
            while node != head {
                let occupant: *mut u8 = read(node, 0x10);
                vcall!(manager, 0x54 => fn(*mut u8, u32, u32), occupant, 1, 0);
                vcall!(occupant, 8 => fn());
                node = successor(node);
            }
            let count: u32 = read(this, 0x22C);
            if count == 0 { return 0; }
            let head: *mut u8 = read(this, 0x228);
            erase_occupants(this.add(0x228), read(head, 4));
            let head: *mut u8 = read(this, 0x228); write(head, 8, head);
            let head: *mut u8 = read(this, 0x228); write(head, 4, 0u32);
            let head: *mut u8 = read(this, 0x228); write(head, 0xC, head);
            write(this, 0x22C, 0u32);
            head as u32
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00624ea0::PORT, fc4p_00625190::PORT, fc4p_00625580::PORT];
#[cfg(test)]
mod tests;
