// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Shared grid, record, tree and pool helpers used by traffic.

use super::*;

layout! {
    /// Four-byte grid, relative to its primary interface. Row descriptors are twelve bytes apart.
    pub struct Grid32 {
        0x00 => primary_table / set_primary_table: u32,
        0x04 => serial_table / set_serial_table: u32,
        0x08 => initialized / set_initialized: u8,
        0x0C => references / set_references: i32,
        0x10 => class_id / set_class_id: u32,
        0x14 => tract_size / set_tract_size: u32,
        0x18 => shift / set_shift: i32,
        0x1C => mask / set_mask: i32,
        0x20 => width / set_width: i32,
        0x24 => height / set_height: i32,
        0x38 => rows / set_rows: *mut u8,
    }
}
layout! {
    /// The same grid relative to its serializable interface, four bytes after the primary interface.
    pub struct Grid32Stream {
        0x04 => initialized / set_initialized: u8,
        0x0C => class_id / set_class_id: u32,
        0x10 => tract_size / set_tract_size: u32,
        0x14 => shift / set_shift: i32,
        0x18 => mask / set_mask: i32,
        0x1C => width / set_width: i32,
        0x20 => height / set_height: i32,
        0x34 => serialized_rows / set_serialized_rows: *mut u8,
    }
}
game_fn! { fn write_bool(stream: *mut u8, value: u32) -> u32 = "C" 0x005D_9FD0; }
game_fn! { #[allow(dead_code)] fn grid_release(this: *mut u8) -> u64 = "thiscall" 0x0045_E120; }
game_fn! { #[allow(dead_code)] fn destroy_rows(this: *mut u8) -> u64 = "thiscall" 0x0046_38B0; }
game_fn! { #[allow(dead_code)] fn heap_free(this: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { fn free(this: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn erase_tree(this: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x0070_EF50; }
game_fn! { fn remove_pool(this: *mut u8, block: *mut u8) = "thiscall" 0x0090_D031; }
game_fn! { fn reorder_pool(this: *mut u8, block: *mut u8) = "thiscall" 0x0090_CFF9; }
game_data! { static pool: u8 = 0x00B0_D484; }

port! {
    /// Writes the grid header and cells, reloading dimensions and rows after stream calls.
    /// The stream argument word becomes the completed row counter.
    #[port(address = 0x0045_FE60, owner = "traffic", name = "Grid32Write", naked)]
    pub unsafe extern "C" fn fc4p_0045fe60() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 4", sym write_grid);
    }
}
unsafe extern "fastcall" fn write_grid(this: *mut u8, stack: *mut u32) -> u64 {
    unsafe {
        let grid = Grid32Stream::from_ptr(this);
        let stream = stack.add(1).read() as *mut u8;
        vcall!(stream, 0x1C => fn(u32) -> u32, 1);
        write_bool(stream, u32::from(this.add(4).read()));
        for (offset, slot) in [
            (0x0C, 0x24),
            (0x10, 0x24),
            (0x14, 0x20),
            (0x18, 0x20),
            (0x1C, 0x20),
            (0x20, 0x20),
            (0x24, 0x30),
            (0x28, 0x30),
            (0x2C, 0x30),
        ] {
            vcall!(stream, slot => fn(u32) -> u32, word(this, offset));
        }
        let last = virtual_registers(stream, 0x30, &[word(this, 0x30)], word(this, 0x30), word(stream, 0));
        let mut eax = grid.width() as u32;
        let mut edx = last.edx;
        let mut row = 0i32;
        stack.add(1).write(0);
        if (eax as i32) > 0 {
            eax = grid.height() as u32;
            loop {
                let mut z = 0i32;
                while z < eax as i32 {
                    let cells = word(grid.serialized_rows(), row.wrapping_mul(12) as usize) as *mut u8;
                    vcall!(stream, 0x24 => fn(u32) -> u32, word(cells, z.wrapping_mul(4) as usize));
                    eax = grid.height() as u32;
                    z = z.wrapping_add(1);
                }
                row = (stack.add(1).read() as i32).wrapping_add(1);
                edx = grid.width() as u32;
                stack.add(1).write(row as u32);
                if row >= edx as i32 {
                    break;
                }
            }
        }
        (u64::from(edx) << 32) | u64::from((eax & !255) | 1)
    }
}
port! {
    /// Adjusts the serializable interface and forwards Release.
    #[port(address = 0x0045_FF40, owner = "traffic", name = "Grid32SerializableRelease", naked)]
    pub unsafe extern "C" fn fc4p_0045ff40() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0045e120");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym test_release);
    }
}
#[cfg(test)]
unsafe extern "fastcall" fn test_release(this: *mut u8, edx: u32) -> u64 {
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        crate::abi::target(0x0045_E120),
        this as u32,
        &[],
    );
    call.edx = edx;
    unsafe {
        crate::abi::call_raw(&mut call);
    }
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}
port! {
    /// Reads a grid word and leaves its row pointer in EDX.
    #[port(address = 0x0045_FF50, owner = "traffic", name = "Grid32GetCell")]
    pub unsafe extern "thiscall" fn fc4p_0045ff50(this: *mut u8, x: i32, z: i32) -> u64 {
        unsafe {
            let grid = Grid32::from_ptr(this);
            let row = word(grid.rows(), x.wrapping_mul(12) as usize) as *mut u8;
            (u64::from(row as u32) << 32) | u64::from(word(row, z.wrapping_mul(4) as usize))
        }
    }
}
port! {
    /// Restores the grid tables, destroys its row vector, and optionally frees the grid allocation.
    #[port(address = 0x0045_FF90, owner = "traffic", name = "Grid32DeletingDestructor")]
    pub unsafe extern "fastcall" fn fc4p_0045ff90(this: *mut u8, entry_edx: u32, flags: u32) -> u64 {
        unsafe {
            let grid = Grid32::from_ptr(this);
            grid.set_primary_table(0x00A8_7D40);
            grid.set_serial_table(0x00A8_7D24);
            let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                crate::abi::target(0x0046_38B0), this.add(0x38) as u32, &[]);
            call.edx = entry_edx;
            crate::abi::call_raw(&mut call);
            if flags as u8 & 1 != 0 {
                let args = [this as u32];
                let mut freeing = crate::abi::RawCall::new(crate::abi::Conv::Cdecl,
                    crate::abi::target(0x005E_5620), call.ecx, &args);
                freeing.eax = call.eax;
                freeing.edx = call.edx;
                crate::abi::call_raw(&mut freeing);
                call.edx = freeing.edx;
            }
            (u64::from(call.edx) << 32) | u64::from(this as u32)
        }
    }
}
port! {
    /// Writes two bytes and seven signed words from a trip record, and returns the stream.
    #[port(address = 0x005D_A390, owner = "traffic", name = "WriteTrafficRecord")]
    pub unsafe extern "C" fn fc4p_005da390(stream: *mut u8, record: *mut u8) -> *mut u8 {
        unsafe {
            vcall!(stream, 0x10 => fn(u32) -> u32, u32::from(record.read()));
            vcall!(stream, 0x10 => fn(u32) -> u32, u32::from(record.add(1).read()));
            for offset in (4..=0x1C).step_by(4) {
                vcall!(stream, 0x20 => fn(u32) -> u32, word(record, offset));
            }
        }
        stream
    }
}
port! {
    /// Deletes a tree in right, node, left order, loading the left link after the right subtree returns.
    #[port(address = 0x0070_EF50, owner = "traffic", name = "EraseTrafficTree", entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_0070ef50(this: *mut u8, [entry_eax: u32], node: *mut u8) -> u32 {
        let mut node = node;
        let mut eax = entry_eax;
        while !node.is_null() {
            unsafe {
                erase_tree(this, word(node, 0x0C) as *mut u8);
                let left = word(node, 8) as *mut u8;
                eax = free(node);
                node = left;
            }
        }
        eax
    }
}
port! {
    /// Returns a trip object to its 2048-byte pool block when flag bit zero is set.
    #[port(address = 0x0070_F040, owner = "traffic", name = "DeletePooledTrip")]
    pub unsafe extern "thiscall" fn fc4p_0070f040(this: *mut u8, flags: u32) -> *mut u8 {
        if flags as u8 & 1 != 0 && !this.is_null() {
            unsafe {
                let block = (this as u32 & !2047) as *mut u8;
                store(this, 0, word(block, 8));
                let count = word(block, 0x0C).wrapping_sub(1);
                store(block, 8, this as u32);
                store(block, 0x0C, count);
                if count == 0 { remove_pool(pool(), block); }
                else if count == 0x4000_003F { reorder_pool(pool(), block); }
            }
        }
        this
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0045fe60::PORT,
    fc4p_0045ff40::PORT,
    fc4p_0045ff50::PORT,
    fc4p_0045ff90::PORT,
    fc4p_005da390::PORT,
    fc4p_0070ef50::PORT,
    fc4p_0070f040::PORT,
];

#[cfg(test)]
mod tests;
