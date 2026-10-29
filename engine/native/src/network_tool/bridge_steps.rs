// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Bridge edge checks, alternating span extension, and deck elevation selection.

use super::bridge_helpers::{add_float, compare_float};
use super::*;
use crate::x87::F80;

game_fn! { fn get_cell_info(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_fn! { fn check_step_edges(this: *mut u8, step: i32, network_type: u32) -> u32 = "thiscall" 0x0062_EB80; }
game_data! { static network_bits: [u32; 13] = 0x00AA_88B8; }

layout! {
    /// Inclusive cell indices and the kind of one twelve-byte drag step.
    struct Step {
        0x00 => first / set_first: u32,
        0x04 => last / set_last: u32,
        0x08 => kind / set_kind: u32,
    }
}
layout! {
    /// The fields of a bridge record used while extending and positioning its span.
    struct Bridge {
        0x04 => start / set_start: i32,
        0x08 => end / set_end: i32,
        0x0C => index / set_index: u32,
        0x10 => deck / set_deck: f32,
    }
}

unsafe fn step_at(tool: NetworkTool, index: i32) -> *mut u8 {
    tool.steps().wrapping_add(index.wrapping_mul(12) as usize)
}

port! {
    /// Keep the override or the greatest last-cell surface plus the bridge's water/land clearance.
    /// The far step is excluded. Every cell is queried, and an empty step retains the preceding value.
    #[port(address = 0x0062_EA90, owner = "networktool", name = "SelectBridgeElevation")]
    pub unsafe extern "thiscall" fn fc4p_0062ea90(this: *mut u8, record: *mut u8) {
        unsafe {
            let tool = NetworkTool::from_ptr(this);
            let bridge = Bridge::from_ptr(record);
            let mut index = bridge.start();
            let types: *mut u8 = read(tool.type_info(), 0x30);
            let parameters = types.wrapping_add(bridge.index().wrapping_mul(0x178) as usize);
            // A bit copy preserves an override's signaling NaN until a comparison actually loads it.
            write(record, 0x10, read::<u32>(this, 0x204));
            if index >= bridge.end() { return; }
            let mut height = F80::from_f32(f32::from_bits(record as u32));
            loop {
                let mut cell = Step::from_ptr(step_at(tool, index)).first();
                if cell <= Step::from_ptr(step_at(tool, index)).last() {
                    loop {
                        let coordinates = tool.cells().wrapping_add(cell.wrapping_mul(8) as usize);
                        let wet = vcall!(tool.terrain(), 0x88 => fn(u32, u32) -> u32,
                            read(coordinates, 0), read(coordinates, 4));
                        // Terrain callbacks can change the coordinate table and the terrain service.
                        let coordinates = tool.cells().wrapping_add(cell.wrapping_mul(8) as usize);
                        height = vcall_st0!(tool.terrain(), 0x48, read::<u32>(coordinates, 0), read::<u32>(coordinates, 4));
                        height = add_float(height, read(parameters, if wet as u8 != 0 { 0x5C } else { 0x78 }));
                        cell = cell.wrapping_add(1);
                        if cell > Step::from_ptr(step_at(tool, index)).last() { break; }
                    }
                }
                if compare_float(height, bridge.deck()) & 0x4100 == 0 {
                    bridge.set_deck(height.to_f32());
                }
                index = index.wrapping_add(1);
                if index >= bridge.end() { break; }
            }
        }
    }
}

port! {
    /// Reject another low-thirteen network bit or an edge outside the mask for this cell's position.
    /// Missing cell records pass. AL is the answer, with upper EAX bytes from the last index or record.
    #[port(address = 0x0062_EB80, owner = "networktool", name = "CheckBridgeStepEdgeTypes")]
    pub unsafe extern "thiscall" fn fc4p_0062eb80(this: *mut u8, index: i32, network_type: u32) -> u32 {
        unsafe {
            let tool = NetworkTool::from_ptr(this);
            let step = Step::from_ptr(step_at(tool, index));
            let mut cell = step.first();
            let mut last = step.last();
            if cell <= last {
                loop {
                    let coordinates = tool.cells().wrapping_add(cell.wrapping_mul(8) as usize);
                    let key = read::<u32>(coordinates, 4).wrapping_shl(16).wrapping_add(read(coordinates, 0));
                    let info = get_cell_info(this, key);
                    if !info.is_null() {
                        let mask = network_bits().cast::<u32>().wrapping_add(network_type as usize).read_unaligned();
                        if read::<u32>(info, 8) & !mask & 0x1FFF != 0 { return info as u32 & !0xFF; }
                        let first = step.first();
                        let end = step.last();
                        let along_z = tool.pen() & 2 != 0;
                        let allowed = match (along_z, first == end, cell == first, cell == end) {
                            (false, true, _, _) => 0x0002_0002,
                            (false, false, true, _) => 0x0402_0002,
                            (false, false, false, true) => 0x0002_0402,
                            (false, false, false, false) => 0x0402_0402,
                            (true, true, _, _) => 0x0200_0200,
                            (true, false, true, _) => 0x0204_0200,
                            (true, false, false, true) => 0x0200_0204,
                            (true, false, false, false) => 0x0204_0204,
                        };
                        let edges: u32 = read(info, network_type.wrapping_mul(4).wrapping_add(0x18) as usize);
                        if edges & !allowed != 0 { return info as u32 & !0xFF; }
                    }
                    last = step.last();
                    cell = cell.wrapping_add(1);
                    if cell > last { break; }
                }
            }
            (last & !0xFF) | 1
        }
    }
}

register_port!(
    0x0062_ECD0,
    fc4p_0062ecd0,
    "PadStepsToMakeMinBridgeSpan",
    pad_span,
    "ret 16"
);
unsafe extern "fastcall" fn pad_span(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let minimum = *frame.add(3) as i32;
        let start = *frame.add(4) as *mut i32;
        let end = *frame.add(5) as *mut i32;
        let length = (*end).wrapping_sub(*start).wrapping_add(1);
        if length >= minimum {
            return registers((length as u32 & !0xFF) | 1, *frame);
        }
        let mut back = *start > 1;
        frame.add(5).cast::<u8>().write(u8::from(back));
        let mut remaining = minimum.wrapping_sub(length);
        let mut forward = true;
        let mut eax = length as u32;
        while remaining > 0 {
            eax = (eax & !0xFF) | u32::from(back);
            if !back && !forward {
                return registers(eax & !0xFF, *frame);
            }
            if back {
                let index = (*start).wrapping_sub(1);
                *start = index;
                eax = index as u32;
                let mut accepted = false;
                if index > 1 {
                    let step = step_at(tool, index);
                    if read::<u32>(step, 8) == 0 && read::<u32>(step.wrapping_sub(4), 0) == 0 {
                        eax = check_step_edges(this, index, *frame.add(6));
                        accepted = eax as u8 != 0;
                    }
                }
                if accepted {
                    remaining = remaining.wrapping_sub(1);
                } else {
                    eax = (*start).wrapping_add(1) as u32;
                    *start = eax as i32;
                    back = false;
                    frame.add(5).cast::<u8>().write(0);
                }
            }
            if remaining <= 0 {
                break;
            }
            if forward {
                let index = (*end).wrapping_add(1);
                *end = index;
                eax = index as u32;
                let mut accepted = false;
                if index < tool.valid_steps() && Step::from_ptr(step_at(tool, index)).kind() == 0 {
                    eax = check_step_edges(this, index, *frame.add(6));
                    accepted = eax as u8 != 0;
                }
                if accepted {
                    remaining = remaining.wrapping_sub(1);
                } else {
                    *end = (*end).wrapping_sub(1);
                    forward = false;
                }
            }
        }
        let yes = *end < tool.straight_steps() && (*end).wrapping_sub(*start).wrapping_add(1) >= minimum;
        registers(u32::from(yes), *frame)
    }
}

pub const PORTS: &[Port] = &[fc4p_0062ea90::PORT, fc4p_0062eb80::PORT, fc4p_0062ecd0::PORT];

#[cfg(test)]
mod tests;
