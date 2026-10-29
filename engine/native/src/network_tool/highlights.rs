// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate cells and demolition highlights for a solved drag.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use core::ptr::null_mut;

layout! {
    /// Drag spans, intersection records and the retained demolition results.
    struct HighlightTool {
        0x54 => spans / set_spans: *mut u8,
        0x60 => cells / set_cells: *mut u8,
        0x78 => span_count / set_span_count: i32,
        0xF0 => intersections / set_intersections: *mut u8,
        0xF4 => intersections_end / set_intersections_end: *mut u8,
        0x210 => region / set_region: *mut u8,
        0x214 => tree / set_tree: *mut u8,
        0x218 => tree_count / set_tree_count: u32,
        0x364 => filter / set_filter: u32,
    }
}

#[repr(C)]
struct CellVector {
    begin: *mut u8,
    end: *mut u8,
    capacity: *mut u8,
}

#[repr(C)]
struct Tree {
    head: *mut u8,
    count: u32,
}

game_fn! { fn cell_info(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_fn! { fn grow_cells(this: *mut CellVector, end: *mut u8, cell: *const u32, argument: *mut u32, count: u32, copy: u32) = "thiscall" 0x007C_61A0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn delete_region_storage(p: *mut u8) = "C" 0x005E_5620; }
game_fn! { fn demolition_region(cells: *mut CellVector, region: *mut *mut u8, cost: *mut u32, tree: *mut Tree, filter: u32) -> u32 = "C" 0x0061_F7C0; }
game_fn! { fn prepare_tree_swap(a: *mut u8, b: *mut Tree) = "C" 0x0061_F5D0; }
game_fn! { fn erase_tree(this: *mut Tree, root: *mut u8) -> u64 = "thiscall" 0x0046_76A0; }

unsafe fn append_inline(cells: &mut CellVector, cell: [u32; 2]) {
    unsafe {
        if cells.end != cells.capacity {
            if !cells.end.is_null() {
                write(cells.end, 0, cell[0]);
                write(cells.end, 4, cell[1]);
            }
            cells.end = cells.end.wrapping_add(8);
        } else {
            let size = (cells.end as u32).wrapping_sub(cells.begin as u32) as i32 >> 3;
            let capacity = (size as u32).wrapping_add(if (size as u32) < 1 { 1 } else { size as u32 });
            let begin = if capacity == 0 {
                null_mut()
            } else {
                allocate(capacity.wrapping_mul(8))
            };
            let mut source = cells.begin;
            let mut end = begin;
            while source != cells.end {
                if !end.is_null() {
                    write(end, 0, read::<u32>(source, 0));
                    write(end, 4, read::<u32>(source, 4));
                }
                source = source.wrapping_add(8);
                end = end.wrapping_add(8);
            }
            if !end.is_null() {
                write(end, 0, cell[0]);
                write(end, 4, cell[1]);
            }
            end = end.wrapping_add(8);
            if !cells.begin.is_null() {
                free(cells.begin);
            }
            cells.begin = begin;
            cells.end = end;
            cells.capacity = begin.wrapping_add(capacity.wrapping_mul(8) as usize);
        }
    }
}

port! {
    /// Build the candidate vector, replace the demolition region and tree, then free the temporary
    /// containers. The caller's word is passed directly to vector growth and supplies the return value;
    /// a failed region computation subtracts one with wrapping arithmetic.
    #[port(address = 0x0062_99A0, owner = "networktool", name = "cSC4NetworkTool::UpdateOccupantHighlights", naked)]
    pub unsafe extern "C" fn fc4p_006299a0() {
        core::arch::naked_asm!(
            "push edx", "mov edx, esp", "call {body}", "add esp, 4", "ret 4",
            body = sym update,
        );
    }
}

unsafe extern "fastcall" fn update(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let argument = frame.add(2);
        let tool = HighlightTool::from_ptr(this);
        if tool.span_count() == 0 {
            let mut call = RawCall::new(Conv::Thiscall, vslot(this, 0x44), this as u32, &[]);
            call.eax = read(this, 0);
            call.edx = frame.read();
            call_raw(&mut call);
            return registers(0, call.edx);
        }
        let mut cells = CellVector {
            begin: null_mut(),
            end: null_mut(),
            capacity: null_mut(),
        };
        let mut span = 0i32;
        let mut offset = 0u32;
        while span < tool.span_count() {
            let record = tool.spans().wrapping_add(offset as usize);
            if read::<u32>(record, 8) != 3 {
                let mut index: u32 = read(record, 0);
                if index <= read::<u32>(record, 4) {
                    loop {
                        let address = tool.cells().wrapping_add(index.wrapping_mul(8) as usize);
                        let cell = [read::<u32>(address, 0), read::<u32>(address, 4)];
                        let info = cell_info(this, cell[1].wrapping_shl(16).wrapping_add(cell[0]));
                        if info.is_null() || read::<u8>(info, 0x56) == 0 {
                            if cells.end == cells.capacity {
                                grow_cells(&mut cells, cells.end, cell.as_ptr(), argument, 1, 1);
                            } else {
                                append_inline(&mut cells, cell);
                            }
                        }
                        index = index.wrapping_add(1);
                        if index > read::<u32>(tool.spans().wrapping_add(offset as usize), 4) {
                            break;
                        }
                    }
                }
            }
            span = span.wrapping_add(1);
            offset = offset.wrapping_add(12);
        }
        let end = tool.intersections_end();
        let mut record = tool.intersections();
        while record != end {
            let key = read::<u32>(record, 4).wrapping_shl(16).wrapping_add(read(record, 0));
            let info = cell_info(this, key);
            if info.is_null() || read::<u8>(info, 0x56) == 0 {
                // Coordinates are fetched after the lookup, which can change the record.
                append_inline(&mut cells, [read(record, 0), read(record, 4)]);
            }
            record = record.wrapping_add(20);
        }
        let region = tool.region();
        if !region.is_null() {
            let storage: *mut u8 = read(region, 0x20);
            write(region, 0x10, 0x00A8_6364u32);
            if !storage.is_null() {
                let mut column = 0u32;
                while column < read::<u32>(region, 0x14) {
                    let storage: *mut u8 = read(region, 0x20);
                    delete_region_storage(read(storage, column.wrapping_mul(4) as usize));
                    column = column.wrapping_add(1);
                }
                delete_region_storage(read(region, 0x20));
            }
            delete_region_storage(region);
        }
        tool.set_region(null_mut());
        let mut tree = Tree {
            head: allocate(20),
            count: 0,
        };
        write(tree.head, 0, 0u8);
        write(tree.head, 4, 0u32);
        write(tree.head, 8, tree.head);
        write(tree.head, 12, tree.head);
        let success = demolition_region(
            &mut cells,
            this.add(0x210).cast(),
            this.add(0x220).cast(),
            &mut tree,
            tool.filter(),
        );
        prepare_tree_swap(this.add(0x214), &mut tree);
        let old_head = tool.tree();
        tool.set_tree(tree.head);
        let old_count = tool.tree_count();
        tool.set_tree_count(tree.count);
        let mut edx = tree.count;
        tree.head = old_head;
        tree.count = old_count;
        let result = argument.read().wrapping_sub(u32::from(success as u8 == 0));
        if old_count != 0 {
            edx = (erase_tree(&mut tree, read(old_head, 4)) >> 32) as u32;
            write(tree.head, 8, tree.head);
            write(tree.head, 4, 0u32);
            write(tree.head, 12, tree.head);
            tree.count = 0;
        }
        if !tree.head.is_null() {
            edx = (free(tree.head) >> 32) as u32;
        }
        if !cells.begin.is_null() {
            edx = (free(cells.begin) >> 32) as u32;
        }
        registers(result, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006299a0::PORT];

#[cfg(test)]
mod tests;
