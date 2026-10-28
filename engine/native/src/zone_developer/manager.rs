// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lot-map serialization, random lookup and safe-iterator removal.

use super::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Fields relative to the lot-manager interface, four bytes after the primary interface.
    pub struct LotManager {
        0x24 => cells_x / set_cells_x: u32,
        0x28 => cells_z / set_cells_z: u32,
        0x50 => rows / set_rows: *mut u8,
    }
}

game_data! { static random: u8 = 0x00B4_B398; }
game_fn! { fn uniform(this: *mut u8, bound: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { fn write_reference(stream: *mut u8, reference: u32) -> u32 = "C" 0x005D_99A0; }
game_fn! { fn free_node(node: *mut u8) -> u32 = "C" 0x0090_CF63; }

port! {
    /// Save version one, the cell-map dimensions and each cell's lot reference, X outer and Z inner.
    /// Return whether the stream has no error in AL, retaining its upper EAX bytes.
    #[port(address = 0x0072_AB40, owner = "zonedeveloper", name = "WriteLotMap")]
    pub unsafe extern "thiscall" fn fc4p_0072ab40(this: *mut u8, stream: *mut u8) -> u64 {
        unsafe {
            vcall!(stream, 0x1C => fn(u32) -> u32, 1);
            let rows = read_unaligned(this.add(0x48).cast::<*mut u8>());
            let end = read_unaligned(this.add(0x4C).cast::<*mut u8>());
            let width = (end as u32).wrapping_sub(rows as u32) as i32 / 12;
            let height = if width != 0 {
                let row = read_unaligned(this.add(0x48).cast::<*mut u8>());
                let start = read_unaligned(row.cast::<u32>());
                let end = read_unaligned(row.add(4).cast::<u32>());
                (end.wrapping_sub(start) as i32) >> 2
            } else { 0 };
            vcall!(stream, 0x24 => fn(i32) -> u32, width);
            let mut eax = vcall!(stream, 0x24 => fn(i32) -> u32, height);
            let mut row = read_unaligned(this.add(0x48).cast::<*mut u8>());
            while row != read_unaligned(this.add(0x4C).cast::<*mut u8>()) {
                let mut cell = read_unaligned(row.cast::<*mut u32>());
                while cell != read_unaligned(row.add(4).cast::<*mut u32>()) {
                    write_reference(stream, read_unaligned(cell));
                    cell = cell.wrapping_add(1);
                }
                row = row.wrapping_add(12);
                eax = read_unaligned(this.add(0x4C).cast::<u32>());
            }
            let error = super::virtual_registers(stream, 0x48, eax, read_unaligned(stream.cast::<u32>()), &[]);
            (error & !0xFF) | u64::from(error as u8 == 0)
        }
    }
}

port! {
    /// Draw X then Z on each of at most 999 attempts, returning the first non-null lot pointer.
    #[port(address = 0x0072_B3C0, owner = "zonedeveloper", name = "GetRandomLot")]
    pub unsafe extern "thiscall" fn fc4p_0072b3c0(this: *mut u8) -> u64 {
        unsafe {
            let manager = LotManager::from_ptr(this);
            let mut row = core::ptr::null_mut::<u8>();
            for _ in 0..999 {
                let x = uniform(random(), manager.cells_x());
                let z = uniform(random(), manager.cells_z());
                row = read_unaligned(manager.rows().wrapping_add(x.wrapping_mul(12) as usize).cast::<*mut u8>());
                let lot = read_unaligned(row.wrapping_add(z.wrapping_mul(4) as usize).cast::<*mut u8>());
                if !lot.is_null() { return (u64::from(row as u32) << 32) | u64::from(lot as u32); }
            }
            u64::from(row as u32) << 32
        }
    }
}

port! {
    /// Remove the first matching list node, advancing registered safe iterators that point to it.
    /// Return the remaining count after removal, otherwise the sentinel pointer.
    #[port(address = 0x0072_B990, owner = "zonedeveloper", name = "RemoveLotListEntry")]
    pub unsafe extern "thiscall" fn fc4p_0072b990(this: *mut u8, value: *const u32) -> u32 {
        unsafe {
            let sentinel = read_unaligned(this.cast::<*mut u8>());
            let mut node = read_unaligned(sentinel.cast::<*mut u8>());
            if node == sentinel { return node as u32; }
            let value = read_unaligned(value);
            while read_unaligned(node.add(8).cast::<u32>()) != value {
                node = read_unaligned(node.cast::<*mut u8>());
                if node == read_unaligned(this.cast::<*mut u8>()) { break; }
            }
            if node == sentinel { return node as u32; }
            let iter_sentinel = read_unaligned(this.add(4).cast::<*mut u8>());
            let mut iter_node = read_unaligned(iter_sentinel.cast::<*mut u8>());
            while iter_node != iter_sentinel {
                let iter = read_unaligned(iter_node.add(8).cast::<*mut *mut u8>());
                if read_unaligned(iter) == node {
                    write_unaligned(iter, read_unaligned(node.cast::<*mut u8>()));
                }
                iter_node = read_unaligned(iter_node.cast::<*mut u8>());
            }
            let next = read_unaligned(node.cast::<*mut u8>());
            let prev = read_unaligned(node.add(4).cast::<*mut u8>());
            write_unaligned(prev.cast::<*mut u8>(), next);
            write_unaligned(next.add(4).cast::<*mut u8>(), prev);
            free_node(node);
            let count = this.add(8).cast::<u32>();
            let remaining = read_unaligned(count).wrapping_sub(1);
            write_unaligned(count, remaining);
            remaining
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072ab40::PORT, fc4p_0072b3c0::PORT, fc4p_0072b990::PORT];

#[cfg(test)]
mod tests;
