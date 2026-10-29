// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Straight drags, masked extensions and joins to other network tools.

use super::*;
use core::ptr::null_mut;

layout! {
    /// Step counts, anchors and the failure word used by a placement request.
    struct DragTool {
        0x54 => steps / set_steps: *mut u8,
        0x58 => steps_end / set_steps_end: *mut u8,
        0x70 => dragged / set_dragged: u32,
        0x74 => straight / set_straight: u32,
        0x118 => anchors / set_anchors: *mut u8,
        0x11C => anchors_end / set_anchors_end: *mut u8,
        0x120 => anchors_capacity / set_anchors_capacity: *mut u8,
        0x20C => failure / set_failure: u32,
    }
}

game_fn! { fn clear_steps(first: *mut u8, last: *mut u8, output: *mut u8, argument: *mut u32, flags: u32) -> *mut u8 = "C" 0x0061_43E0; }
game_fn! { fn clear_cache(this: *mut u8) = "thiscall" 0x0064_7B20; }
game_fn! { fn grow_anchors(this: *mut u8, end: *mut u8, point: *mut u32, argument: *mut u32, count: u32, copy: u32) = "thiscall" 0x007C_61A0; }
game_fn! { fn clear_anchors(this: *mut u8) = "thiscall" 0x0060_6870; }
game_fn! { fn push_anchor(this: *mut u8, point: *mut u32) = "thiscall" 0x0044_67A0; }
game_fn! { fn place_network(this: *mut u8, real: u32, crew: u32, flags: u32, neighbours: u32, network: u32) -> u32 = "thiscall" 0x0063_B830; }
game_fn! { fn allocate_filter(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn construct_filter(this: *mut u8) -> *mut u8 = "thiscall" 0x0061_F510; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(p: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn occupant_at(this: *mut u8, x: u32, z: u32, filter: *mut u8, output: *mut *mut u8) -> u32 = "thiscall" 0x0062_4B70; }
game_fn! { fn cache_cell(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0064_7A20; }
game_data! { static neighbour_x: [u32; 4] = 0x00AA_8390; }
game_data! { static neighbour_z: [u32; 4] = 0x00AA_83A0; }

fn clamp(value: u32, max: u32) -> u32 {
    (value as i32).min(max as i32).max(0) as u32
}

// The wrapping negation gives zero for INT_MIN, as does the game's pair of signed shifts.
fn direction(value: u32) -> u32 {
    ((value as i32) >> 31).wrapping_sub((value.wrapping_neg() as i32) >> 31) as u32
}

unsafe fn append_anchor(tool: DragTool, point: &mut [u32; 2], argument: *mut u32) {
    unsafe {
        let end = tool.anchors_end();
        if end == tool.anchors_capacity() {
            grow_anchors(tool.as_ptr().add(0x118), end, point.as_mut_ptr(), argument, 1, 1);
        } else {
            if !end.is_null() {
                write(end, 0, point[0]);
                write(end, 4, point[1]);
            }
            tool.set_anchors_end(tool.anchors_end().wrapping_add(8));
        }
    }
}

unsafe fn anchors(tool: DragTool, first: &mut [u32; 2], last: &mut [u32; 2], argument: *mut u32) {
    unsafe {
        tool.set_anchors_end(tool.anchors());
        append_anchor(tool, first, argument);
        append_anchor(tool, last, argument);
    }
}

unsafe fn draw_and_place(
    this: *mut u8,
    first: &mut [u32; 2],
    last: &mut [u32; 2],
    network: u32,
    args: *mut u32,
    store_drawn: bool,
) {
    unsafe {
        let drawn = vcall!(this, 0x48 => fn(*mut u32, *mut u32, u32, u32) -> u32,
            first.as_mut_ptr(), last.as_mut_ptr(), 0, network);
        if store_drawn || drawn as u8 == 0 {
            args.add(7).cast::<u8>().write(drawn as u8);
        }
        if drawn as u8 != 0 {
            let result = place_network(this, args.add(6).read(), 0, 0, 0, network);
            args.add(7).cast::<u8>().write(result as u8);
        }
    }
}

unsafe fn release_output(output: *mut *mut u8) {
    unsafe {
        let old = output.read();
        if !old.is_null() {
            output.write(null_mut());
            vcall!(old, 8 => fn());
        }
    }
}

unsafe fn append_join(head: *mut u8, a: [u32; 2], b: [u32; 2]) {
    unsafe {
        let node = allocate(24);
        if !node.wrapping_add(8).is_null() {
            write(node, 8, a[0]);
            write(node, 12, a[1]);
            write(node, 16, b[0]);
            write(node, 20, b[1]);
        }
        let previous: *mut u8 = read(head, 4);
        write(node, 0, head);
        write(node, 4, previous);
        write(previous, 0, node);
        write(head, 4, node);
    }
}

// Probe one end and either extend it or retain a join. The two ends store opposite join directions.
unsafe fn probe(
    this: *mut u8,
    point: &mut [u32; 2],
    outer: [u32; 2],
    filter: *mut u8,
    network: u32,
    args: *mut u32,
    at_start: bool,
) {
    unsafe {
        let output = args.add(3).cast::<*mut u8>();
        release_output(output);
        if occupant_at(this, outer[0], outer[1], filter, output) as u8 != 0 {
            if vcall!(output.read(), 0x58 => fn(u32) -> u32, network) as u8 != 0 {
                *point = outer;
            } else {
                let head = args.add(2).read() as *mut u8;
                if at_start {
                    append_join(head, *point, outer);
                } else {
                    append_join(head, outer, *point);
                }
            }
        } else if args.add(6).read() as u8 == 0 {
            let cell = cache_cell(this.add(0x1C), outer[1].wrapping_shl(16).wrapping_add(outer[0]));
            if !cell.is_null() && read::<u32>(cell, 8) & args.add(4).read() != 0 {
                *point = outer;
            }
        }
    }
}

port! {
    /// Clamp the requested endpoints, reset the step vector, and place each straight run. Masked drags
    /// may extend to matching occupants or cached cells and retain joins for the callback tool. Restore
    /// insertion data to one and empty the anchors after all cleanup. EDX is zero; EAX's upper bytes
    /// retain the last anchor-end pointer.
    #[port(address = 0x0062_9210, owner = "networktool", name = "cSC4NetworkTool::PlaceNetworkOccupants", naked)]
    pub unsafe extern "C" fn fc4p_00629210() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {body}", "ret 32", body = sym place);
    }
}

unsafe extern "fastcall" fn place(this: *mut u8, args: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let drag = DragTool::from_ptr(this);
        let network = if (args.add(7).read() as i32) < 0 {
            tool.network_type()
        } else {
            args.add(7).read()
        };
        let raw_end = [args.add(2).read(), args.add(3).read()];
        let max_z = tool.height().wrapping_sub(1);
        tool.set_insertion_data(3);
        drag.set_failure(0);
        args.add(7).cast::<u8>().write(1);
        let start_z = clamp(args.add(1).read(), max_z);
        let max_x = tool.width().wrapping_sub(1);
        let mut first = [clamp(args.read(), max_x), start_z];
        let mut last = [clamp(raw_end[0], max_x), clamp(raw_end[1], max_z)];
        args.add(3).write(max_x);
        let step_end = drag.steps_end();
        drag.set_steps_end(clear_steps(step_end, step_end, drag.steps(), args.add(6), 0));
        drag.set_dragged(0);
        drag.set_straight(0);
        if args.add(6).read() as u8 != 0 {
            clear_cache(this.add(0x1C));
        }
        if args.add(4).read() == 0 {
            anchors(drag, &mut first, &mut last, args.add(6));
            draw_and_place(this, &mut first, &mut last, network, args, true);
            if drag.failure() != 0 {
                args.add(7).cast::<u8>().write(0);
            }
        } else {
            let allocation = allocate_filter(16);
            let filter = if allocation.is_null() {
                null_mut()
            } else {
                construct_filter(allocation)
            };
            if !filter.is_null() {
                vcall!(filter, 4 => fn());
            }
            let mut dx = raw_end[0].wrapping_sub(args.read());
            let mut dz = raw_end[1].wrapping_sub(args.add(1).read());
            write(filter, 12, args.add(4).read());
            args.add(3).write(0);
            if dx == 0 || dz == 0 {
                let head = allocate(24);
                args.add(2).write(head as u32);
                write(head, 0, head);
                write(head, 4, head);
                if dx == 0 && dz == 0 {
                    let mut found = [0u8; 4];
                    for (index, found) in found.iter_mut().enumerate() {
                        let x = args.read().wrapping_add(neighbour_x().cast::<u32>().add(index).read());
                        let z = args
                            .add(1)
                            .read()
                            .wrapping_add(neighbour_z().cast::<u32>().add(index).read());
                        release_output(args.add(3).cast());
                        *found = occupant_at(this, x, z, filter, args.add(3).cast()) as u8;
                    }
                    if found[0] != 0 || found[2] != 0 {
                        dx = 1;
                    }
                    if found[1] != 0 || found[3] != 0 {
                        dz = 1;
                    }
                }
                if dx == 0 && dz == 0 {
                    clear_anchors(this.add(0x118));
                    push_anchor(this.add(0x118), first.as_mut_ptr());
                    push_anchor(this.add(0x118), last.as_mut_ptr());
                    draw_and_place(this, &mut first, &mut last, network, args, true);
                } else {
                    let mut directions = [[0u32; 2]; 2];
                    let mut count = 0usize;
                    if dx != 0 {
                        directions[count] = [direction(dx), 0];
                        count += 1;
                    }
                    if dz != 0 {
                        directions[count] = [0, direction(dz)];
                        count += 1;
                    }
                    args.add(1).write(count as u32);
                    args.write(0);
                    while (args.read() as i32) < (args.add(1).read() as i32) {
                        let d = directions.as_ptr().wrapping_add(args.read() as usize).read();
                        let mut a = first;
                        let mut b = last;
                        let before = [a[0].wrapping_sub(d[0]), a[1].wrapping_sub(d[1])];
                        let after = [b[0].wrapping_add(d[0]), b[1].wrapping_add(d[1])];
                        probe(this, &mut a, before, filter, network, args, true);
                        probe(this, &mut b, after, filter, network, args, false);
                        anchors(drag, &mut a, &mut b, args.add(6));
                        draw_and_place(this, &mut a, &mut b, network, args, false);
                        args.write(args.read().wrapping_add(1));
                    }
                }
                if drag.failure() != 0 {
                    args.add(7).cast::<u8>().write(0);
                }
                let head = args.add(2).read() as *mut u8;
                let callback = args.add(5).read() as *mut u8;
                if tool.real() != 0 && !callback.is_null() {
                    let mut node: *mut u8 = read(head, 0);
                    let real = args.add(6).read();
                    while node != head {
                        vcall!(callback, 0x1C => fn(u32, u32, u32, u32, u32, u32, u32, u32),
                            read::<u32>(node, 8), read::<u32>(node, 12), read::<u32>(node, 16), read::<u32>(node, 20), 0, 0, real, u32::MAX);
                        node = read(node, 0);
                    }
                }
                let mut node: *mut u8 = read(head, 0);
                while node != head {
                    let next = read(node, 0);
                    free(node);
                    node = next;
                }
                write(head, 0, head);
                write(head, 4, head);
                free(head);
                let output = args.add(3).read() as *mut u8;
                if !output.is_null() {
                    vcall!(output, 8 => fn());
                }
            }
            vcall!(filter, 8 => fn());
        }
        let end = drag.anchors_end() as u32;
        drag.set_anchors_end(drag.anchors());
        tool.set_insertion_data(1);
        registers((end & !0xFF) | u32::from(args.add(7).cast::<u8>().read()), 0)
    }
}

pub const PORTS: &[Port] = &[fc4p_00629210::PORT];

#[cfg(test)]
mod tests;
