// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Enumerate lots inside a cell rectangle and around its perimeter.

use super::{manager::LotManager, network::virtual_call, Port};
use core::mem::MaybeUninit;
use core::ptr::{null_mut, read_unaligned, write_unaligned};

layout! {
    /// Rectangle iterator, including its current coordinate and mutable traversal step.
    struct RectangleIterator {
        0x00 => state / set_state: u8,
        0x01 => repeat / set_repeat: u8,
        0x04 => x / set_x: i32,
        0x08 => z / set_z: i32,
        0x0C => step / set_step: i32,
        0x10 => x1 / set_x1: i32,
        0x14 => z1 / set_z1: i32,
        0x18 => x2 / set_x2: i32,
        0x1C => z2 / set_z2: i32,
    }
}

game_fn! { #[allow(dead_code)] fn clear_lots(list: *mut *mut u8) -> u64 = "thiscall" 0x0047_3EC0; }
game_fn! { fn construct_iterator(this: *mut u8) = "thiscall" 0x0060_CE90; }
game_fn! { fn set_iterator_bounds(this: *mut u8, bounds: *const i32) = "thiscall" 0x0060_C890; }
game_fn! { fn advance_iterator(this: *mut u8, x: *mut i32, z: *mut i32) -> u64 = "thiscall" 0x0060_CEE0; }
game_fn! { fn append_reference(list: *mut *mut u8, lot: *const *mut u8) = "thiscall" 0x007E_98C0; }
game_fn! { fn allocate_node(size: u32) -> *mut u8 = "C" 0x0090_CF54; }

unsafe fn append_lot(header: *mut u8, lot: *mut u8) {
    unsafe {
        let node = allocate_node(12);
        let payload = node.wrapping_add(8).cast::<*mut u8>();
        if !payload.is_null() {
            write_unaligned(payload, lot);
            vcall!(lot, 4 => fn());
        }
        let previous = read_unaligned(header.add(4).cast::<*mut u8>());
        write_unaligned(node.cast::<*mut u8>(), header);
        write_unaligned(node.add(4).cast::<*mut u8>(), previous);
        write_unaligned(previous.cast::<*mut u8>(), node);
        write_unaligned(header.add(4).cast::<*mut u8>(), node);
    }
}

unsafe fn column(manager: LotManager, x: i32) -> *mut u8 {
    manager.rows().wrapping_offset(x.wrapping_mul(12) as isize)
}

unsafe fn cell(column: *mut u8, z: i32) -> *mut u8 {
    unsafe {
        let data = read_unaligned(column.cast::<*mut u8>());
        read_unaligned(data.wrapping_offset(z.wrapping_mul(4) as isize).cast::<*mut u8>())
    }
}

port! {
    /// Clear the output, clip the inclusive rectangle to the city, then enumerate lots. Origin-only mode
    /// visits Z before X and calls the reference-list append helper for matching anchors. Ordinary mode
    /// visits X before Z, suppressing a lot when it matches the preceding cell or the previous column.
    /// Return the appended count and retain EDX, including on empty clipped rectangles.
    #[port(address = 0x0072_E080, owner = "zonedeveloper", name = "GetLotsInCellRect")]
    pub unsafe extern "fastcall" fn fc4p_0072e080(
        this: *mut u8, entry_edx: u32, out: *mut *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, origins: u32
    ) -> u64 {
        unsafe {
            let manager = LotManager::from_ptr(this);
            let mut clear = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                crate::abi::target(0x0047_3EC0), out as u32, &[]);
            clear.edx = entry_edx;
            crate::abi::call_raw(&mut clear);
            let mut edx = clear.edx;
            let (x1, z1) = (x1.max(0), z1.max(0));
            let x2 = if x2 >= manager.cells_x() as i32 { (manager.cells_x() as i32).wrapping_sub(1) } else { x2 };
            let z2 = if z2 >= manager.cells_z() as i32 { (manager.cells_z() as i32).wrapping_sub(1) } else { z2 };
            if x1 > x2 || z1 > z2 { return u64::from(edx) << 32; }
            let mut count = 0u32;
            if origins as u8 != 0 {
                let mut storage = MaybeUninit::<[u32;8]>::uninit();
                let iterator = RectangleIterator::from_ptr(storage.as_mut_ptr().cast());
                construct_iterator(iterator.as_ptr());
                let bounds = [x1, z1, x2, z2];
                set_iterator_bounds(iterator.as_ptr(), bounds.as_ptr());
                let mut z = z1;
                let mut x = x1;
                loop {
                    edx = this as u32;
                    let lot = cell(column(manager, x), z);
                    if !lot.is_null() {
                        let (mut lx, mut lz) = (MaybeUninit::<i32>::uninit(), MaybeUninit::<i32>::uninit());
                        let call = virtual_call(lot, 0x54, lz.as_mut_ptr() as u32,
                            read_unaligned(lot.cast::<u32>()), &[lx.as_mut_ptr() as u32, lz.as_mut_ptr() as u32]);
                        edx = call.edx;
                        if lx.assume_init() == x && lz.assume_init() == z {
                            vcall!(lot, 4 => fn());
                            append_reference(out, &lot);
                            let call = virtual_call(lot, 8, 0, read_unaligned(lot.cast::<u32>()), &[]);
                            edx = call.edx;
                            count = count.wrapping_add(1);
                        }
                    }
                    x = x.wrapping_add(iterator.step());
                    if x > iterator.x2() {
                        x = iterator.x1();
                        z = z.wrapping_add(iterator.step());
                        if z > iterator.z2() { break; }
                    }
                }
            } else {
                let mut previous_column = null_mut::<u8>();
                let mut x = x1;
                let mut remaining = x2.wrapping_sub(x1).wrapping_add(1) as u32;
                loop {
                    let current_column = column(manager, x);
                    let mut previous = null_mut();
                    let mut z = z1;
                    loop {
                        let lot = cell(current_column, z);
                        if !lot.is_null() && lot != previous
                            && (previous_column.is_null() || lot != cell(previous_column, z)) {
                            vcall!(lot, 4 => fn());
                            append_lot(read_unaligned(out), lot);
                            vcall!(lot, 8 => fn());
                            count = count.wrapping_add(1);
                        }
                        previous = lot;
                        z = z.wrapping_add(1);
                        if z > z2 { break; }
                    }
                    edx = previous_column as u32;
                    previous_column = current_column;
                    x = x.wrapping_add(1);
                    remaining = remaining.wrapping_sub(1);
                    if remaining == 0 { break; }
                }
            }
            (u64::from(edx) << 32) | u64::from(count)
        }
    }
}

port! {
    /// Grow the rectangle by one cell and visit its perimeter through the game's iterator. Clip to the
    /// city, disable sides clipped at the city boundary, reject overlapping lots, then retain each new
    /// lot only once across the existing output list. Even duplicates receive a temporary reference.
    #[port(address = 0x0072_E2B0, owner = "zonedeveloper", name = "GetLotsSurroundingRect")]
    pub unsafe extern "thiscall" fn fc4p_0072e2b0(this: *mut u8, rect: *const i32, out: *mut *mut u8, flags: u32) -> u64 {
        unsafe {
            let manager = LotManager::from_ptr(this);
            let x1 = read_unaligned(rect).wrapping_sub(1);
            let z1 = read_unaligned(rect.add(1)).wrapping_sub(1);
            let x2 = read_unaligned(rect.add(2)).wrapping_add(1);
            let z2 = read_unaligned(rect.add(3)).wrapping_add(1);
            let mut flags = flags;
            let clipped_x1 = if x1 < 0 { flags &= !8; 0 } else { x1 };
            let clipped_z1 = if z1 < 0 { flags &= !1; 0 } else { z1 };
            let clipped_x2 = if x2 >= manager.cells_x() as i32 { flags &= !2; (manager.cells_x() as i32).wrapping_sub(1) } else { x2 };
            let clipped_z2 = if z2 >= manager.cells_z() as i32 { flags &= !4; (manager.cells_z() as i32).wrapping_sub(1) } else { z2 };
            let mut storage = MaybeUninit::<[u32;8]>::uninit();
            let iterator = RectangleIterator::from_ptr(storage.as_mut_ptr().cast());
            iterator.set_state(0);
            iterator.set_repeat(0);
            iterator.set_x(x1);
            iterator.set_z(z1);
            iterator.set_step(1);
            iterator.set_x1(x1);
            iterator.set_z1(z1);
            iterator.set_x2(x2.max(x1));
            iterator.set_z2(z2.max(z1));
            let (mut x, mut z) = (0i32, 0i32);
            let mut count = 0u32;
            loop {
                let result = advance_iterator(iterator.as_ptr(), &mut x, &mut z);
                if result as u8 == 0 { return (result & 0xFFFF_FFFF_0000_0000) | u64::from(count); }
                if x < clipped_x1 || z < clipped_z1 || x > clipped_x2 || z > clipped_z2 { continue; }
                let lot = cell(column(manager, x), z);
                if lot.is_null() { continue; }
                let mut bounds = MaybeUninit::<[i32;4]>::uninit();
                vcall!(lot, 0x38 => fn(*mut i32), bounds.as_mut_ptr().cast());
                let bounds = bounds.assume_init();
                if bounds[2] >= read_unaligned(rect) && bounds[0] <= read_unaligned(rect.add(2))
                    && bounds[3] >= read_unaligned(rect.add(1)) && bounds[1] <= read_unaligned(rect.add(3)) { continue; }
                if flags != 15 && ((z == z1 && flags & 1 == 0) || (x == x2 && flags & 2 == 0)
                    || (z == z2 && flags & 4 == 0) || (x == x1 && flags & 8 == 0)) { continue; }
                let sentinel = read_unaligned(out);
                vcall!(lot, 4 => fn());
                let current_header = read_unaligned(out);
                let mut node = read_unaligned(current_header.cast::<*mut u8>());
                while node != current_header && read_unaligned(node.add(8).cast::<*mut u8>()) != lot {
                    node = read_unaligned(node.cast::<*mut u8>());
                }
                let absent = node == sentinel;
                vcall!(lot, 8 => fn());
                if !absent { continue; }
                vcall!(lot, 4 => fn());
                append_lot(read_unaligned(out), lot);
                vcall!(lot, 8 => fn());
                count = count.wrapping_add(1);
            }
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072e080::PORT, fc4p_0072e2b0::PORT];

#[cfg(test)]
mod tests;
