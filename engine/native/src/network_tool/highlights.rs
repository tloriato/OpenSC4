// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate cells and demolition highlights for a solved drag.

use super::*;
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

game_fn! { #[allow(dead_code)] fn cell_info(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_fn! { #[allow(dead_code)] fn grow_cells(this: *mut CellVector, end: *mut u8, cell: *const u32, argument: *mut u32, count: u32, copy: u32) = "thiscall" 0x007C_61A0; }
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn delete_region_storage(p: *mut u8) = "C" 0x005E_5620; }
game_fn! { #[allow(dead_code)] fn demolition_region(cells: *mut CellVector, region: *mut *mut u8, cost: *mut u32, tree: *mut Tree, filter: u32) -> u32 = "C" 0x0061_F7C0; }
game_fn! { #[allow(dead_code)] fn prepare_tree_swap(a: *mut u8, b: *mut Tree) = "C" 0x0061_F5D0; }
game_fn! { #[allow(dead_code)] fn erase_tree(this: *mut Tree, root: *mut u8) -> u64 = "thiscall" 0x0046_76A0; }

port! {
    /// Collect eligible drag and intersection cells, replace the demolition region and occupant tree,
    /// and destroy the temporary containers. Keep all escaping local addresses and call depths.
    #[port(address = 0x0062_99A0, owner = "networktool", name = "cSC4NetworkTool::UpdateOccupantHighlights", naked)]
    pub unsafe extern "C" fn fc4p_006299a0() {
        core::arch::naked_asm!(
            "sub esp, 44", "push ebp", "push ebx", "push esi", "push edi",
            "mov ebp, esp", "mov esi, ecx", "xor edi, edi", "xor ebx, ebx",
            "2:", "sub esp, 128", "mov ecx, esp", "push ecx", "push edx", "push eax", "push ebx", "push edi",
            "mov ecx, esi", "mov edx, ebp", "call {step}", "mov edi, eax", "mov ebx, edx",
            "cmp dword ptr [esp], 0", "jz 5f", "mov ecx, [esp + 16]", "mov edx, esp", "mov esp, ebp",
            "sub esp, dword ptr [ebp - 76]",
            "3:", "test ecx, ecx", "jz 4f", "dec ecx", "push dword ptr [edx + ecx*4 + 20]", "jmp 3b",
            "4:", "mov eax, [ebp - 124]", "mov ecx, [ebp - 120]", "mov edx, [ebp - 116]",
            "cmp edi, 99", "jne 6f",
            "mov ebx, [ebp]", "mov [ebp + 16], ebx", "mov ebx, [ebp + 4]", "mov [ebp + 20], ebx",
            "mov ebx, [ebp + 8]", "mov [ebp + 24], ebx", "6:",
            "call dword ptr [ebp - 128]",
            "cmp edi, 99", "jne 7f",
            "mov ebx, [ebp + 16]", "mov [ebp], ebx", "mov ebx, [ebp + 20]", "mov [ebp + 4], ebx",
            "mov ebx, [ebp + 24]", "mov [ebp + 8], ebx", "7:", "mov esp, ebp", "jmp 2b",
            "5:", "mov eax, [esp + 4]", "mov edx, [esp + 12]", "mov esp, ebp",
            "pop edi", "pop esi", "pop ebx", "pop ebp", "add esp, 44", "ret 4",
            step = sym highlight_step,
        );
    }
}
unsafe fn append_cell(vector: *mut CellVector, cell: *const u32) {
    unsafe {
        let end = (*vector).end;
        if !end.is_null() {
            write(end, 0, cell.read());
            write(end, 4, cell.add(1).read());
        }
        (*vector).end = (*vector).end.wrapping_add(8);
    }
}
unsafe extern "fastcall" fn highlight_step(
    this: *mut u8,
    frame: *mut u32,
    mut stage: u32,
    mut cursor: u32,
    eax: u32,
    mut edx: u32,
    command: *mut super::placement::PlacementCall,
) -> u64 {
    unsafe {
        let c = &mut *command;
        let tool = HighlightTool::from_ptr(this);
        let vector = frame.add(9).cast::<CellVector>();
        let tree = frame.add(12).cast::<Tree>();
        let coords = frame.add(7);
        loop {
            match stage {
                0 => {
                    if tool.span_count() == 0 {
                        c.virtual_call(this, 0x44, &[], read(this, 0), edx);
                        c.depth = -12;
                        stage = 99;
                    } else {
                        (*vector).begin = null_mut();
                        (*vector).end = null_mut();
                        (*vector).capacity = null_mut();
                        frame.add(4).write(0);
                        frame.add(5).write(0);
                        stage = 10;
                        continue;
                    }
                }
                10 => {
                    if frame.add(5).read() as i32 >= tool.span_count() {
                        stage = 20;
                        continue;
                    }
                    let span = tool.spans().wrapping_add(frame.add(4).read() as usize);
                    if read::<u32>(span, 8) != 3 && read::<u32>(span, 0) <= read::<u32>(span, 4) {
                        cursor = read(span, 0);
                        stage = 11;
                    } else {
                        frame.add(4).write(frame.add(4).read().wrapping_add(12));
                        frame.add(5).write(frame.add(5).read().wrapping_add(1));
                    }
                    continue;
                }
                11 => {
                    let cell = tool.cells().wrapping_add(cursor.wrapping_mul(8) as usize);
                    coords.write(read(cell, 0));
                    coords.add(1).write(read(cell, 4));
                    let key = coords.add(1).read().wrapping_shl(16).wrapping_add(coords.read());
                    c.set(crate::abi::target(0x0063_3220), this, &[key], key, tool.cells() as u32);
                    stage = 12;
                }
                12 => {
                    let info = eax as *mut u8;
                    if info.is_null() || read::<u8>(info, 0x56) == 0 {
                        if (*vector).end == (*vector).capacity {
                            let end = (*vector).end;
                            c.set(
                                crate::abi::target(0x007C_61A0),
                                vector.cast(),
                                &[end as u32, coords as u32, frame.add(16) as u32, 1, 1],
                                end as u32,
                                coords as u32,
                            );
                            stage = 13;
                            break;
                        }
                        append_cell(vector, coords);
                    }
                    stage = 13;
                    continue;
                }
                13 => {
                    cursor = cursor.wrapping_add(1);
                    let span = tool.spans().wrapping_add(frame.add(4).read() as usize);
                    if cursor <= read::<u32>(span, 4) {
                        stage = 11;
                    } else {
                        frame.add(4).write(frame.add(4).read().wrapping_add(12));
                        frame.add(5).write(frame.add(5).read().wrapping_add(1));
                        stage = 10;
                    }
                    continue;
                }
                20 => {
                    frame.add(6).write(tool.intersections_end() as u32);
                    cursor = tool.intersections() as u32;
                    stage = 21;
                    continue;
                }
                21 => {
                    if cursor == frame.add(6).read() {
                        stage = 30;
                        continue;
                    }
                    let p = cursor as *mut u8;
                    let key = read::<u32>(p, 4).wrapping_shl(16).wrapping_add(read(p, 0));
                    c.set(crate::abi::target(0x0063_3220), this, &[key], key, edx);
                    stage = 22;
                }
                22 => {
                    let info = eax as *mut u8;
                    if !info.is_null() && read::<u8>(info, 0x56) != 0 {
                        stage = 23;
                        continue;
                    }
                    coords.write(read(cursor as *mut u8, 0));
                    coords.add(1).write(read(cursor as *mut u8, 4));
                    if (*vector).end != (*vector).capacity {
                        append_cell(vector, coords);
                        stage = 23;
                        continue;
                    }
                    let count = ((*vector).end as u32).wrapping_sub((*vector).begin as u32) as i32 >> 3;
                    let capacity = (count as u32).wrapping_add((count as u32).max(1));
                    frame.add(5).write(capacity);
                    if capacity == 0 {
                        frame.add(14).write(0);
                        stage = 24;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0090_CF54),
                        null_mut(),
                        &[capacity.wrapping_mul(8)],
                        count as u32,
                        capacity.wrapping_mul(8),
                    );
                    stage = 24;
                }
                24 => {
                    let buffer = if frame.add(5).read() == 0 {
                        null_mut()
                    } else {
                        eax as *mut u8
                    };
                    frame.add(14).write(buffer as u32);
                    let mut source = (*vector).begin;
                    let mut output = buffer;
                    while source != (*vector).end {
                        if !output.is_null() {
                            write(output, 0, read::<u32>(source, 0));
                            write(output, 4, read::<u32>(source, 4));
                        }
                        source = source.wrapping_add(8);
                        output = output.wrapping_add(8);
                    }
                    if !output.is_null() {
                        write(output, 0, coords.read());
                        write(output, 4, coords.add(1).read());
                    }
                    frame.add(4).write(output.wrapping_add(8) as u32);
                    if (*vector).begin.is_null() {
                        stage = 25;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0090_CF63),
                        null_mut(),
                        &[(*vector).begin as u32],
                        (*vector).begin as u32,
                        edx,
                    );
                    stage = 25;
                }
                25 => {
                    (*vector).begin = frame.add(14).read() as *mut u8;
                    (*vector).end = frame.add(4).read() as *mut u8;
                    (*vector).capacity = (*vector)
                        .begin
                        .wrapping_add(frame.add(5).read().wrapping_mul(8) as usize);
                    stage = 23;
                    continue;
                }
                23 => {
                    cursor = cursor.wrapping_add(20);
                    stage = 21;
                    continue;
                }
                30 => {
                    cursor = tool.region() as u32;
                    if cursor == 0 {
                        stage = 34;
                        continue;
                    }
                    write(cursor as *mut u8, 0x10, 0x00A8_6364u32);
                    if read::<u32>(cursor as *mut u8, 0x20) == 0 {
                        stage = 33;
                        continue;
                    }
                    frame.add(4).write(0);
                    stage = 31;
                    continue;
                }
                31 => {
                    let region = cursor as *mut u8;
                    let column = frame.add(4).read();
                    if column < read::<u32>(region, 0x14) {
                        let storage: *mut u8 = read(region, 0x20);
                        let p: u32 = read(storage, column.wrapping_mul(4) as usize);
                        c.set(crate::abi::target(0x005E_5620), null_mut(), &[p], p, storage as u32);
                        stage = 32;
                    } else {
                        let p = read(region, 0x20);
                        c.set(
                            crate::abi::target(0x005E_5620),
                            null_mut(),
                            &[p],
                            read(region, 0x14),
                            edx,
                        );
                        stage = 33;
                    }
                }
                32 => {
                    frame.add(4).write(frame.add(4).read().wrapping_add(1));
                    stage = 31;
                    continue;
                }
                33 => {
                    c.set(crate::abi::target(0x005E_5620), null_mut(), &[cursor], eax, edx);
                    stage = 34;
                }
                34 => {
                    tool.set_region(null_mut());
                    (*tree).head = null_mut();
                    c.set(crate::abi::target(0x0090_CF54), null_mut(), &[20], eax, edx);
                    stage = 35;
                }
                35 => {
                    (*tree).head = eax as *mut u8;
                    (*tree).count = 0;
                    write((*tree).head, 0, 0u8);
                    write((*tree).head, 4, 0u32);
                    write((*tree).head, 8, (*tree).head);
                    write((*tree).head, 12, (*tree).head);
                    c.set(
                        crate::abi::target(0x0061_F7C0),
                        null_mut(),
                        &[
                            vector as u32,
                            this.add(0x210) as u32,
                            this.add(0x220) as u32,
                            tree as u32,
                            tool.filter(),
                        ],
                        vector as u32,
                        this.add(0x220) as u32,
                    );
                    c.depth = 4;
                    stage = 36;
                }
                36 => {
                    frame.add(14).write(eax & 255);
                    c.set(
                        crate::abi::target(0x0061_F5D0),
                        null_mut(),
                        &[this.add(0x214) as u32, tree as u32],
                        eax,
                        edx,
                    );
                    c.depth = 24;
                    stage = 37;
                }
                37 => {
                    let head = tool.tree();
                    let count = tool.tree_count();
                    tool.set_tree((*tree).head);
                    tool.set_tree_count((*tree).count);
                    edx = (*tree).count;
                    (*tree).head = head;
                    (*tree).count = count;
                    frame
                        .add(4)
                        .write(frame.add(16).read().wrapping_sub(u32::from(frame.add(14).read() == 0)));
                    if count == 0 {
                        stage = 39;
                        continue;
                    }
                    let root = read(head, 4);
                    c.set(crate::abi::target(0x0046_76A0), tree.cast(), &[root], root, edx);
                    stage = 38;
                }
                38 => {
                    write((*tree).head, 8, (*tree).head);
                    write((*tree).head, 4, 0u32);
                    write((*tree).head, 12, (*tree).head);
                    (*tree).count = 0;
                    stage = 39;
                    continue;
                }
                39 => {
                    if (*tree).head.is_null() {
                        stage = 40;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0090_CF63),
                        null_mut(),
                        &[(*tree).head as u32],
                        (*tree).head as u32,
                        edx,
                    );
                    stage = 40;
                }
                40 => {
                    if (*vector).begin.is_null() {
                        stage = 41;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0090_CF63),
                        null_mut(),
                        &[(*vector).begin as u32],
                        (*vector).begin as u32,
                        edx,
                    );
                    stage = 41;
                }
                _ => {
                    c.target = 0;
                    c.eax = if stage == 99 { 0 } else { frame.add(4).read() };
                    c.edx = edx;
                    break;
                }
            }
            break;
        }
        registers(stage, cursor)
    }
}

pub const PORTS: &[Port] = {
    let _ = fc4p_006299a0::PORT;
    &[]
};

#[cfg(test)]
mod tests;
