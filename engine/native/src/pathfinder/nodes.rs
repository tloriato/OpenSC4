// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Capacity, distance tables, route reversibility and node-page maintenance.

use super::*;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Fields read while following a result's predecessor chain or recomputing its score.
    struct Node {
        0x04 => parent / set_parent: *mut u8,
        0x08 => cost / set_cost: f32,
        0x0C => score / set_score: f32,
        0x10 => open / set_open: u8,
        0x12 => side / set_side: u8,
        0x13 => travel / set_travel: u8,
        0x14 => x / set_x: i16,
        0x16 => z / set_z: i16,
    }
}

game_fn! {
    fn used_capacity(traffic: *mut u8, holder: *mut u8, kind: u32) -> u64 = "thiscall" 0x0070_DFF0;
}
game_fn! {
    fn fill_words(dest: *mut u8, value: u32, count: u32) -> u32 = "C" 0x0091_0003;
}
game_fn! {
    fn update_open(heap: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x006D_6460;
}
game_data! {
    static zero: f32 = 0x00A8_1054;
}

port! {
    /// Compare both coordinates and city dimensions as unsigned words.
    #[port(address = 0x006D_5CF0, owner = "pathfinder", name = "CellInCity")]
    pub unsafe extern "thiscall" fn fc4p_006d5cf0(this: *mut u8, x: u32, z: u32) -> u32 {
        let pf = unsafe { Pathfinder::from_ptr(this) };
        u32::from(x < pf.count_x() && z < pf.count_z())
    }
}

port! {
    /// Query maximum and used capacity, accepting up to the requested density. Failed queries leave results intact.
    #[port(address = 0x006D_5D80, owner = "pathfinder", name = "AcceptDestinationCapacity")]
    pub unsafe extern "thiscall" fn fc4p_006d5d80(this: *mut u8, holder: *mut u8, kind: u32) -> u64 {
        unsafe {
            let pf = Pathfinder::from_ptr(this);
            let kind = u32::from(kind as u8);
            let maximum_result = vcall!(pf.traffic(), 0x50 => fn(*mut u8, u32) -> u64, holder, kind);
            let maximum = maximum_result as u32;
            if maximum == 0 { return maximum_result; }
            let used_result = used_capacity(pf.traffic(), holder, kind);
            let used = used_result as u32;
            let edx = used_result & 0xFFFF_FFFF_0000_0000;
            if used >= maximum { return edx | u64::from(used & !0xFF); }
            let remaining = maximum.wrapping_sub(used);
            let density = pf.density();
            pf.set_arrived(if remaining < density { remaining } else { density });
            pf.set_reached_type(kind);
            edx | u64::from((density & !0xFF) | 1)
        }
    }
}

/// Fill one biased table, starting at index -1. Each of the three runs keeps its original signed bounds.
unsafe fn distance_axis(pf: Pathfinder, x_axis: bool, low: i32, high: i32, limit: i32) -> i32 {
    unsafe {
        let mut i = -1i32;
        let table = || if x_axis { pf.distances_x() } else { pf.distances_z() };
        if low > -1 {
            let mut distance = low.wrapping_add(1);
            while i < low {
                write_unaligned(table().wrapping_offset(i as isize), distance);
                i = i.wrapping_add(1);
                distance = distance.wrapping_sub(1);
            }
        }
        while i <= high {
            write_unaligned(table().wrapping_offset(i as isize), 0);
            i = i.wrapping_add(1);
        }
        let mut distance = i.wrapping_sub(high);
        while i < limit {
            write_unaligned(table().wrapping_offset(i as isize), distance);
            i = i.wrapping_add(1);
            distance = distance.wrapping_add(1);
        }
        i
    }
}

port! {
    /// Fill x before z with signed distances to the end rectangle, including the one-cell outer border.
    #[port(address = 0x006D_6040, owner = "pathfinder", name = "cSC4PathFinder::SetupDistanceTables", naked)]
    pub unsafe extern "C" fn fc4p_006d6040() {
        core::arch::naked_asm!("push ecx", "call {body}", "pop ecx", "ret", body = sym setup_distance_tables)
    }
}

unsafe extern "thiscall" fn setup_distance_tables(this: *mut u8) -> u32 {
    unsafe {
        let pf = Pathfinder::from_ptr(this);
        distance_axis(pf, true, pf.end_x0(), pf.end_x1(), pf.count_x().wrapping_add(1) as i32);
        distance_axis(pf, false, pf.end_z0(), pf.end_z1(), pf.count_z().wrapping_add(1) as i32) as u32
    }
}

port! {
    /// Return zero inside the inclusive search rectangle, otherwise the wrapping table sum in ST0.
    #[port(address = 0x006D_6160, owner = "pathfinder", name = "SearchDistance", naked)]
    pub unsafe extern "C" fn fc4p_006d6160() {
        // Pass the original argument words: the outside branch overwrites the caller's x word.
        core::arch::naked_asm!(
            "mov edx, esp", "sub esp, 12", "lea eax, [esp]", "push eax", "call {body}",
            "fld tbyte ptr [esp]", "add esp, 12", "ret 8", body = sym search_distance,
        )
    }
}

unsafe extern "fastcall" fn search_distance(this: *mut u8, stack: *mut u32, output: *mut F80) -> u32 {
    unsafe {
        let (x, z) = (stack.add(1).read() as i32, stack.add(2).read() as i32);
        let pf = Pathfinder::from_ptr(this);
        if x >= pf.search_x0() && z >= pf.search_z0() && x <= pf.search_x1() && z <= pf.search_z1() {
            output.write(F80::from_f32(*zero()));
            x as u32
        } else {
            let dx = read_unaligned(pf.distances_x().wrapping_offset(x as isize));
            let sum = dx.wrapping_add(read_unaligned(pf.distances_z().wrapping_offset(z as isize)));
            stack.add(1).write(sum as u32);
            output.write(F80::from_i32(sum));
            sum as u32
        }
    }
}

unsafe fn connection_record(traffic: *mut u8, x: i32, z: i32) -> *mut u8 {
    unsafe {
        let cells = read_unaligned(traffic.add(0x58).cast::<*mut u8>());
        cells.wrapping_offset(((x.wrapping_shl(8) | z).wrapping_mul(0x2E)) as isize)
    }
}

port! {
    /// Check reverse turns between ordinary cells, resetting direction across equal, off-map and subway pairs.
    /// A type change checks the current cell's switch nibble; ordinary travel checks the reverse connection bit.
    #[port(address = 0x006D_61E0, owner = "pathfinder", name = "cSC4PathFinder::WasTripReversible", naked)]
    pub unsafe extern "C" fn fc4p_006d61e0() {
        core::arch::naked_asm!("push edx", "mov edx, eax", "call {body}", "ret", body = sym reversible)
    }
}

unsafe extern "fastcall" fn reversible(this: *mut u8, entry: u32, entry_edx: u32) -> u64 {
    unsafe {
        let result = |eax: u32, edx: u32| (u64::from(edx) << 32) | u64::from(eax);
        let pf = Pathfinder::from_ptr(this);
        let mut current = pf.result();
        if current.is_null() {
            return result((entry & !0xFF) | 1, entry_edx);
        }
        let mut node = Node::from_ptr(current);
        let mut previous_type = node.travel();
        let width = pf.count_x();
        let mut direction = u32::MAX;
        let mut edx = entry_edx;
        while !node.parent().is_null() {
            let parent = Node::from_ptr(node.parent());
            let kind = node.travel();
            let (x, z, px, pz) = (
                i32::from(node.x()),
                i32::from(node.z()),
                i32::from(parent.x()),
                i32::from(parent.z()),
            );
            edx = px as u32;
            if x as u32 >= width
                || z as u32 >= pf.count_z()
                || px as u32 >= width
                || pz as u32 >= pf.count_z()
                || (x == px && z == pz)
            {
                direction = u32::MAX;
            } else {
                let record = connection_record(pf.traffic(), x, z);
                let subway = if *record.add(0x2C) & 2 != 0 {
                    edx = (px.wrapping_shl(8) | pz).wrapping_mul(0x2E) as u32;
                    *connection_record(pf.traffic(), px, pz).add(0x2C) & 2 != 0
                } else {
                    false
                };
                if subway {
                    direction = u32::MAX;
                } else {
                    if direction != u32::MAX {
                        edx = u32::from(kind);
                        if previous_type != kind {
                            let switches = read_unaligned(record.add(0x12 + usize::from(kind) * 2).cast::<u16>());
                            let shift = u32::from((direction as u8).wrapping_shl(2));
                            let shifted = u32::from(switches.wrapping_shr(shift));
                            edx = (shifted & !0xFF) | (shifted & 0xF);
                            if edx as u8 == 0xF {
                                return result(record as u32 & !0xFF, edx);
                            }
                        } else if parent.travel() == kind {
                            let connections =
                                u32::from(read_unaligned(record.add(usize::from(kind) * 2).cast::<u16>()));
                            let shift = u32::from(node.side()).wrapping_add(direction.wrapping_shl(2));
                            if connections & 1u32.wrapping_shl(shift) == 0 {
                                return result(connections & !0xFF, edx);
                            }
                        }
                    }
                    direction = if x < px {
                        0
                    } else if x > px {
                        2
                    } else if z >= pz {
                        3
                    } else {
                        1
                    };
                    edx = direction;
                }
            }
            previous_type = kind;
            current = node.parent();
            node = Node::from_ptr(current);
        }
        result(1, edx)
    }
}

port! {
    /// Rewind allocation to the first retained page, clear 3079 bucket words, and forget the result and open count.
    #[port(address = 0x006D_6730, owner = "pathfinder", name = "cSC4PathFinder::ReleasePathNodes", naked)]
    pub unsafe extern "C" fn fc4p_006d6730() {
        core::arch::naked_asm!("jmp {body}", body = sym release_nodes)
    }
}

unsafe extern "fastcall" fn release_nodes(this: *mut u8, entry_edx: u32) -> u64 {
    unsafe {
        let pf = Pathfinder::from_ptr(this);
        let first = read_unaligned(pf.pages());
        pf.set_next_node(first);
        pf.set_page_end(first.wrapping_add(0x24000));
        pf.set_page_index(0);
        fill_words(this.add(0x12C), 0, 0xC07);
        let count = pf.open_count();
        pf.set_result(core::ptr::null_mut());
        if count != 0 {
            pf.set_open_count(0);
        }
        (u64::from(entry_edx) << 32) | u64::from(count)
    }
}

// Memory operands retain their role in the x87's NaN selection and do not first quiet signaling NaNs.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_op!(multiply_stored, "fmul dword ptr [{operand}]");
stored_op!(add_stored, "fadd dword ptr [{operand}]");

port! {
    /// Recompute every allocated node's score in page order, updating the heap only for nodes marked open.
    #[port(address = 0x006D_6780, owner = "pathfinder", name = "cSC4PathFinder::RecalculateNodeScores")]
    pub unsafe extern "thiscall" fn fc4p_006d6780(this: *mut u8) -> u32 {
        unsafe {
            let pf = Pathfinder::from_ptr(this);
            let (x0, z0) = (pf.search_x0(), pf.search_z0());
            let (width, height) = (pf.search_x1().wrapping_sub(x0) as u32, pf.search_z1().wrapping_sub(z0) as u32);
            let mut cursor = read_unaligned(pf.pages());
            let mut page_end = cursor.wrapping_add(0x24000);
            let mut page = 0usize;
            loop {
                let end = pf.next_node();
                if cursor == end { return end as u32; }
                if cursor == page_end {
                    page = page.wrapping_add(1);
                    cursor = read_unaligned(pf.pages().wrapping_add(page));
                    page_end = cursor.wrapping_add(0x24000);
                    if cursor == end { return end as u32; }
                }
                let p = cursor;
                cursor = cursor.wrapping_add(0x24);
                if p.is_null() { return 0; }
                let node = Node::from_ptr(p);
                let (x, z) = (i32::from(node.x()), i32::from(node.z()));
                let distance = if x.wrapping_sub(x0) as u32 <= width && z.wrapping_sub(z0) as u32 <= height {
                    F80::from_f32(*zero())
                } else {
                    let dz = read_unaligned(pf.distances_z().wrapping_offset(z as isize));
                    let dx = read_unaligned(pf.distances_x().wrapping_offset(x as isize));
                    multiply_stored(F80::from_i32(dz.wrapping_add(dx)), pf.weight())
                };
                let score = add_stored(distance, node.cost());
                let open = node.open();
                node.set_score(score.to_f32());
                if open != 0 { update_open(this.add(0x114), p); }
            }
        }
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_006d5cf0::PORT,
    fc4p_006d5d80::PORT,
    fc4p_006d6040::PORT,
    fc4p_006d6160::PORT,
    fc4p_006d61e0::PORT,
    fc4p_006d6730::PORT,
    fc4p_006d6780::PORT,
];

#[cfg(test)]
#[path = "nodes_tests.rs"]
mod tests;
