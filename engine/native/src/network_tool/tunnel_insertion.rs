// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tunnel span previews, portal pairing and temporary list lifetime.

use super::*;
use core::ptr::null_mut;

layout! {
    /// Inclusive cell ranges for the dragged steps.
    struct TunnelInsertionTool {
        0x54 => steps / set_steps: *mut u8,
        0x60 => cells / set_cells: *mut u8,
        0x78 => valid_steps / set_valid_steps: i32,
    }
}

game_fn! { fn cell_info(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_fn! { fn erase_preview(this: *mut u8, cell: *mut u32) = "thiscall" 0x0062_0CF0; }
game_fn! { fn add_selection(this: *mut u8, cell: *mut u32, color: u32, heights: u32, layer: u32, flags: u32) = "thiscall" 0x0062_0820; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn insert_portal(this: *mut u8, direction: u32, index: u32, cell: *mut u8) -> *mut u8 = "thiscall" 0x0062_8390; }
game_fn! { fn pair_portal(this: *mut u8, other: *mut u8) = "thiscall" 0x0064_7530; }
game_data! { static preview_layers: u32 = 0x00AA_83F0; }

unsafe fn step(tool: TunnelInsertionTool, index: u32) -> *mut u8 {
    tool.steps().wrapping_add(index.wrapping_mul(12) as usize)
}

unsafe fn coordinates(tool: TunnelInsertionTool, index: u32) -> [u32; 2] {
    unsafe {
        let p = tool.cells().wrapping_add(index.wrapping_mul(8) as usize);
        [read(p, 0), read(p, 4)]
    }
}

fn cell_key(cell: [u32; 2]) -> u32 {
    cell[1].wrapping_shl(16).wrapping_add(cell[0])
}

fn range(first: u32, last: u32, reverse: bool) -> (u32, u32, u32) {
    if reverse && (first as i32) < (last as i32) {
        (last, first.wrapping_sub(1), u32::MAX)
    } else {
        (first, last.wrapping_add(1), 1)
    }
}

port! {
    /// Preview each covered cell or insert and pair the two portal runs. Invalid last-step indices and
    /// diagonal endpoints stop the complete pass. Preserve the unused bytes of the direction word
    /// that is passed to the first portal, together with the entry EDX for an empty pass.
    #[port(address = 0x0062_87D0, owner = "networktool", name = "cSC4NetworkTool::InsertTunnelPieces", naked)]
    pub unsafe extern "C" fn fc4p_006287d0() {
        core::arch::naked_asm!(
            "push edx", "push eax",
            "lea eax, [esp + 4]", "and eax, -8", "mov eax, [eax - 0x18]", "push eax",
            "mov edx, esp", "call {body}", "add esp, 12", "ret 4",
            body = sym insert,
        );
    }
}

// Frame words: saved direction storage, entry EAX, entry EDX, return address, layer argument.
unsafe extern "fastcall" fn insert(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let drag = TunnelInsertionTool::from_ptr(this);
        let mut record = tool.tunnel_begin();
        let end = tool.tunnel_end();
        let mut eax = end as u32;
        let mut edx = frame.add(2).read();
        let mut direction = frame.read();
        while record != end {
            let last: u32 = read(record, 12);
            eax = last;
            if (last as i32) >= drag.valid_steps() {
                break;
            }
            if tool.real() == 0 {
                let mut index: u32 = read(record, 8);
                if (index as i32) <= (last as i32) {
                    // Retain the steps pointer between empty intervals; a cell callback refreshes it.
                    let mut steps = drag.steps();
                    let mut offset = index.wrapping_mul(12);
                    loop {
                        let mut cell_index: u32 = read(steps.wrapping_add(offset as usize), 0);
                        if cell_index <= read::<u32>(steps.wrapping_add(offset as usize), 4) {
                            loop {
                                let mut cell = coordinates(drag, cell_index);
                                erase_preview(tool.preview(), cell.as_mut_ptr());
                                let layer = preview_layers().wrapping_add(frame.add(4).read() as usize).read();
                                add_selection(tool.preview(), cell.as_mut_ptr(), 0x8000_FF00, 0, layer, 0);
                                steps = drag.steps();
                                cell_index = cell_index.wrapping_add(1);
                                if cell_index > read::<u32>(steps.wrapping_add(offset as usize), 4) {
                                    break;
                                }
                            }
                        }
                        edx = record as u32;
                        let last: i32 = read(record, 12);
                        index = index.wrapping_add(1);
                        offset = offset.wrapping_add(12);
                        if (index as i32) > last {
                            break;
                        }
                    }
                }
            } else {
                let first_step: u32 = read(record, 8);
                let first_cell: u32 = read(step(drag, first_step), 0);
                let last_cell: u32 = read(step(drag, last), 0);
                let first = coordinates(drag, first_cell);
                let last = coordinates(drag, last_cell);
                let reverse = if first[1] == last[1] {
                    direction = (direction & !0xFF) | (u32::from((last[0] as i32) <= (first[0] as i32)) * 2 + 1);
                    (last[0] as i32) < (first[0] as i32)
                } else {
                    if first[0] != last[0] {
                        return registers(last[1], first[1]);
                    }
                    direction = (direction & !0xFF) | if (last[1] as i32) <= (first[1] as i32) { 0 } else { 2 };
                    (first[1] as i32) < (last[1] as i32)
                };
                let head = allocate(12);
                write(head, 0, head);
                write(head, 4, head);
                // Allocate before fetching these bounds: the allocator is a game call.
                let bounds = step(drag, read(record, 8));
                let (mut index, stop, increment) = range(read(bounds, 0), read(bounds, 4), reverse);
                let mut ordinal = 0u32;
                while index != stop {
                    let cell = cell_info(this, cell_key(coordinates(drag, index)));
                    write(cell, 0x51, 1u8);
                    if tool.real() != 0 {
                        let occupant = insert_portal(this, direction, ordinal, cell);
                        ordinal = ordinal.wrapping_add(1);
                        let node = allocate(12);
                        if !node.wrapping_add(8).is_null() {
                            write(node, 8, occupant);
                        }
                        let previous: *mut u8 = read(head, 4);
                        write(node, 0, head);
                        write(node, 4, previous);
                        write(previous, 0, node);
                        write(head, 4, node);
                    }
                    index = index.wrapping_add(increment);
                }
                let bounds = step(drag, read(record, 12));
                let (mut index, stop, increment) = range(read(bounds, 0), read(bounds, 4), !reverse);
                let mut ordinal = 0u32;
                while index != stop {
                    let cell = cell_info(this, cell_key(coordinates(drag, index)));
                    write(cell, 0x51, 1u8);
                    if tool.real() != 0 {
                        // Only the low direction byte is used at this end, with a byte subtraction.
                        let opposite = u32::from((direction as u8).wrapping_sub(2) & 3);
                        let occupant = insert_portal(this, opposite, ordinal, cell);
                        ordinal = ordinal.wrapping_add(1);
                        let node: *mut u8 = read(head, 4);
                        let next: *mut u8 = read(node, 0);
                        let previous: *mut u8 = read(node, 4);
                        let other: *mut u8 = read(node, 8);
                        write(previous, 0, next);
                        write(next, 4, previous);
                        free(node);
                        let mut first_interface = null_mut();
                        let mut second_interface: *mut u8 = null_mut();
                        if vcall!(other, 0 => fn(u32, *mut *mut u8) -> u32, 0x8A4B_D52B, &mut first_interface) as u8
                            != 0
                        {
                            if !second_interface.is_null() {
                                let old = second_interface;
                                second_interface = null_mut();
                                vcall!(old, 8 => fn());
                            }
                            if vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0x8A4B_D52B, &mut second_interface)
                                as u8
                                != 0
                            {
                                pair_portal(first_interface, occupant);
                                pair_portal(second_interface, other);
                                let manager = vcall!(first_interface, 0xC4 => fn() -> *mut u8);
                                if !manager.is_null() {
                                    vcall!(manager, 0x80 => fn(*mut u8, *mut u8), first_interface, second_interface);
                                }
                                let manager = vcall!(second_interface, 0xC4 => fn() -> *mut u8);
                                if !manager.is_null() {
                                    vcall!(manager, 0x80 => fn(*mut u8, *mut u8), second_interface, first_interface);
                                }
                            }
                        }
                        if !second_interface.is_null() {
                            vcall!(second_interface, 8 => fn());
                        }
                        if !first_interface.is_null() {
                            vcall!(first_interface, 8 => fn());
                        }
                    }
                    index = index.wrapping_add(increment);
                }
                let mut node: *mut u8 = read(head, 0);
                while node != head {
                    let next = read(node, 0);
                    free(node);
                    node = next;
                }
                write(head, 0, head);
                write(head, 4, head);
                edx = (free(head) >> 32) as u32;
            }
            record = record.wrapping_add(16);
            eax = end as u32;
        }
        registers(eax, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006287d0::PORT];

#[cfg(test)]
mod tests;
