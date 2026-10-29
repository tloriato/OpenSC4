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

game_fn! { #[allow(dead_code)] fn clear_steps(first: *mut u8, last: *mut u8, output: *mut u8, argument: *mut u32, flags: u32) -> *mut u8 = "C" 0x0061_43E0; }
game_fn! { #[allow(dead_code)] fn clear_cache(this: *mut u8) = "thiscall" 0x0064_7B20; }
game_fn! { #[allow(dead_code)] fn grow_anchors(this: *mut u8, end: *mut u8, point: *mut u32, argument: *mut u32, count: u32, copy: u32) = "thiscall" 0x007C_61A0; }
game_fn! { #[allow(dead_code)] fn clear_anchors(this: *mut u8) = "thiscall" 0x0060_6870; }
game_fn! { #[allow(dead_code)] fn push_anchor(this: *mut u8, point: *mut u32) = "thiscall" 0x0044_67A0; }
game_fn! { #[allow(dead_code)] fn place_network(this: *mut u8, real: u32, crew: u32, flags: u32, neighbours: u32, network: u32) -> u32 = "thiscall" 0x0063_B830; }
game_fn! { #[allow(dead_code)] fn allocate_filter(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn construct_filter(this: *mut u8) -> *mut u8 = "thiscall" 0x0061_F510; }
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(p: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn occupant_at(this: *mut u8, x: u32, z: u32, filter: *mut u8, output: *mut *mut u8) -> u32 = "thiscall" 0x0062_4B70; }
game_fn! { #[allow(dead_code)] fn cache_cell(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0064_7A20; }
game_data! { static neighbour_x: [u32; 4] = 0x00AA_8390; }
game_data! { static neighbour_z: [u32; 4] = 0x00AA_83A0; }

fn clamp(value: u32, max: u32) -> u32 {
    (value as i32).min(max as i32).max(0) as u32
}

// The wrapping negation gives zero for INT_MIN, as does the game's pair of signed shifts.
fn direction(value: u32) -> u32 {
    ((value as i32) >> 31).wrapping_sub((value.wrapping_neg() as i32) >> 31) as u32
}

// Caller-owned argument words and the four coordinate pairs keep their original stack locations.
// Preparation returns to the entry before every game call so nested callees see the original depth.
port! {
    /// Place the clamped drag, extend masked ends, publish retained joins, and release temporary
    /// occupants and filters. Restore insertion data and the empty anchor cursor before returning.
    #[port(address = 0x0062_9210, owner = "networktool", name = "cSC4NetworkTool::PlaceNetworkOccupants", naked)]
    pub unsafe extern "C" fn fc4p_00629210() {
        core::arch::naked_asm!(
            "sub esp, 68", "push ebx", "push ebp", "push esi", "push edi",
            "mov ebp, esp", "mov esi, ecx", "xor edi, edi", "xor ebx, ebx",
            "2:", "sub esp, 128", "mov ecx, esp", "push ecx", "push edx", "push eax", "push ebx", "push edi",
            "mov ecx, esi", "mov edx, ebp", "call {step}", "mov edi, eax", "mov ebx, edx",
            "cmp dword ptr [esp], 0", "jz 5f", "mov ecx, [esp + 16]", "mov edx, esp", "mov esp, ebp",
            "3:", "test ecx, ecx", "jz 4f", "dec ecx", "push dword ptr [edx + ecx*4 + 20]", "jmp 3b",
            "4:", "mov eax, [ebp - 124]", "mov ecx, [ebp - 120]", "mov edx, [ebp - 116]",
            "call dword ptr [ebp - 128]", "mov esp, ebp", "jmp 2b",
            "5:", "mov eax, [esp + 4]", "mov edx, [esp + 12]", "mov esp, ebp",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 68", "ret 32",
            step = sym drag_step,
        );
    }
}
unsafe fn append_anchor_command(
    tool: DragTool,
    point: *mut u32,
    real: *mut u32,
    c: &mut super::placement::PlacementCall,
) -> bool {
    unsafe {
        let end = tool.anchors_end();
        if end == tool.anchors_capacity() {
            c.set(
                crate::abi::target(0x007C_61A0),
                tool.as_ptr().add(0x118),
                &[end as u32, point as u32, real as u32, 1, 1],
                end as u32,
                point as u32,
            );
            true
        } else {
            if !end.is_null() {
                write(end, 0, point.read());
                write(end, 4, point.add(1).read());
            }
            tool.set_anchors_end(tool.anchors_end().wrapping_add(8));
            false
        }
    }
}
unsafe fn link_join(node: *mut u8, head: *mut u8, a: [u32; 2], b: [u32; 2]) {
    unsafe {
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
unsafe extern "fastcall" fn drag_step(
    this: *mut u8,
    frame: *mut u32,
    mut stage: u32,
    mut cursor: u32,
    eax: u32,
    edx: u32,
    command: *mut super::placement::PlacementCall,
) -> u64 {
    unsafe {
        let c = &mut *command;
        let tool = NetworkTool::from_ptr(this);
        let drag = DragTool::from_ptr(this);
        let args = frame.add(22);
        let real = args.add(6);
        let answer = args.add(7).cast::<u8>();
        let first = frame.add(13);
        let last = frame.add(15);
        let a = frame.add(9);
        let b = frame.add(11);
        let outer = frame.add(7);
        loop {
            match stage {
                0 => {
                    let network = if (args.add(7).read() as i32) < 0 {
                        tool.network_type()
                    } else {
                        args.add(7).read()
                    };
                    frame.add(5).write(network);
                    outer.write(args.add(2).read());
                    outer.add(1).write(args.add(3).read());
                    let max_z = tool.height().wrapping_sub(1);
                    let max_x = tool.width().wrapping_sub(1);
                    tool.set_insertion_data(3);
                    drag.set_failure(0);
                    answer.write(1);
                    first.write(clamp(args.read(), max_x));
                    first.add(1).write(clamp(args.add(1).read(), max_z));
                    last.write(clamp(outer.read(), max_x));
                    last.add(1).write(clamp(outer.add(1).read(), max_z));
                    args.add(3).write(max_x);
                    let end = drag.steps_end();
                    c.set(
                        crate::abi::target(0x0061_43E0),
                        drag.steps(),
                        &[end as u32, end as u32, drag.steps() as u32, real as u32, 0],
                        end as u32,
                        real as u32,
                    );
                    stage = 1;
                }
                1 => {
                    drag.set_steps_end(eax as *mut u8);
                    drag.set_dragged(0);
                    drag.set_straight(0);
                    if real.read() as u8 == 0 {
                        stage = 2;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0064_7B20),
                        this.add(0x1C),
                        &[],
                        eax & !255 | u32::from(real.read() as u8),
                        edx,
                    );
                    stage = 2;
                }
                2 => {
                    if args.add(4).read() == 0 {
                        drag.set_anchors_end(drag.anchors());
                        stage = 10;
                        continue;
                    }
                    c.set(crate::abi::target(0x005E_55E0), null_mut(), &[16], eax, edx);
                    stage = 20;
                }
                10 | 11 => {
                    let point = if stage == 10 { first } else { last };
                    stage += 1;
                    if append_anchor_command(drag, point, real, c) {
                        break;
                    }
                    continue;
                }
                12 | 33 | 55 => {
                    let (pa, pb) = if stage == 55 { (a, b) } else { (first, last) };
                    c.virtual_call(
                        this,
                        0x48,
                        &[pa as u32, pb as u32, 0, frame.add(5).read()],
                        if stage == 12 { read(this, 0) } else { pa as u32 },
                        if stage == 12 { pa as u32 } else { read(this, 0) },
                    );
                    stage = match stage {
                        12 => 13,
                        33 => 34,
                        _ => 56,
                    };
                }
                13 | 34 | 56 => {
                    if stage != 56 || eax as u8 == 0 {
                        answer.write(eax as u8);
                    }
                    if eax as u8 == 0 {
                        stage = match stage {
                            13 => 15,
                            34 => 70,
                            _ => 58,
                        };
                        continue;
                    }
                    let network = frame.add(5).read();
                    let mode = real.read();
                    c.set(
                        crate::abi::target(0x0063_B830),
                        this,
                        &[mode, 0, 0, 0, network],
                        mode,
                        edx,
                    );
                    stage = match stage {
                        13 => 14,
                        34 => 35,
                        _ => 57,
                    };
                }
                14 | 35 | 57 => {
                    answer.write(eax as u8);
                    stage = match stage {
                        14 => 15,
                        35 => 70,
                        _ => 58,
                    };
                    continue;
                }
                15 => {
                    if drag.failure() != 0 {
                        answer.write(0);
                    }
                    stage = 90;
                    continue;
                }
                20 => {
                    frame.add(6).write(eax);
                    if eax == 0 {
                        stage = 22;
                        continue;
                    }
                    c.set(crate::abi::target(0x0061_F510), eax as *mut u8, &[], eax, edx);
                    stage = 21;
                }
                21 => {
                    frame.add(6).write(eax);
                    if eax == 0 {
                        stage = 22;
                        continue;
                    }
                    let p = eax as *mut u8;
                    c.virtual_call(p, 4, &[], eax, read(p, 0));
                    stage = 22;
                }
                22 => {
                    let dx = outer.read().wrapping_sub(args.read());
                    let dz = outer.add(1).read().wrapping_sub(args.add(1).read());
                    write(frame.add(6).read() as *mut u8, 12, args.add(4).read());
                    args.add(3).write(0);
                    b.write(dx);
                    a.write(dz);
                    if dx != 0 && dz != 0 {
                        stage = 89;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0090_CF54),
                        null_mut(),
                        &[24],
                        args.add(4).read(),
                        edx,
                    );
                    stage = 23;
                }
                23 => {
                    args.add(2).write(eax);
                    let head = eax as *mut u8;
                    write(head, 0, head);
                    write(head, 4, head);
                    if b.read() == 0 && a.read() == 0 {
                        cursor = 0;
                        stage = 24;
                    } else {
                        stage = 30;
                    }
                    continue;
                }
                24 => {
                    b.write(
                        args.read()
                            .wrapping_add(neighbour_x().cast::<u32>().add(cursor as usize).read()),
                    );
                    a.write(
                        args.add(1)
                            .read()
                            .wrapping_add(neighbour_z().cast::<u32>().add(cursor as usize).read()),
                    );
                    outer.cast::<u8>().add(cursor as usize).write(0);
                    let output = args.add(3).read() as *mut u8;
                    if output.is_null() {
                        stage = 25;
                        continue;
                    }
                    args.add(3).write(0);
                    c.virtual_call(output, 8, &[], output as u32, read(output, 0));
                    stage = 25;
                }
                25 => {
                    c.set(
                        crate::abi::target(0x0062_4B70),
                        this,
                        &[b.read(), a.read(), frame.add(6).read(), args.add(3) as u32],
                        args.add(3) as u32,
                        edx,
                    );
                    stage = 26;
                }
                26 => {
                    outer.cast::<u8>().add(cursor as usize).write(eax as u8);
                    cursor += 1;
                    if cursor < 4 {
                        stage = 24;
                        continue;
                    }
                    let f = outer.cast::<u8>();
                    b.write(u32::from(f.read() != 0 || f.add(2).read() != 0));
                    a.write(u32::from(f.add(1).read() != 0 || f.add(3).read() != 0));
                    stage = 30;
                    continue;
                }
                30 => {
                    let dx = b.read();
                    let dz = a.read();
                    if dx == 0 && dz == 0 {
                        c.set(crate::abi::target(0x0060_6870), this.add(0x118), &[], eax, edx);
                        stage = 31;
                    } else {
                        let mut count = 0usize;
                        if dx != 0 {
                            frame.add(17).write(direction(dx));
                            frame.add(18).write(0);
                            count += 1;
                        }
                        if dz != 0 {
                            frame.add(17 + count * 2).write(0);
                            frame.add(18 + count * 2).write(direction(dz));
                            count += 1;
                        }
                        args.add(1).write(count as u32);
                        args.write(0);
                        stage = 40;
                        continue;
                    }
                }
                31 | 32 => {
                    let point = if stage == 31 { first } else { last };
                    c.set(
                        crate::abi::target(0x0044_67A0),
                        this.add(0x118),
                        &[point as u32],
                        point as u32,
                        edx,
                    );
                    stage += 1;
                }
                40 => {
                    if args.read() as i32 >= args.add(1).read() as i32 {
                        stage = 70;
                        continue;
                    }
                    let direction = frame.add(17).wrapping_add(args.read().wrapping_mul(2) as usize);
                    a.write(first.read());
                    a.add(1).write(first.add(1).read());
                    b.write(last.read());
                    b.add(1).write(last.add(1).read());
                    frame.add(4).write(first.read().wrapping_sub(direction.read()));
                    cursor = first.add(1).read().wrapping_sub(direction.add(1).read());
                    outer.write(last.read().wrapping_add(direction.read()));
                    outer
                        .add(1)
                        .write(last.add(1).read().wrapping_add(direction.add(1).read()));
                    let output = args.add(3).read() as *mut u8;
                    if output.is_null() {
                        stage = 41;
                        continue;
                    }
                    args.add(3).write(0);
                    c.virtual_call(output, 8, &[], output as u32, read(output, 0));
                    stage = 41;
                }
                41 => {
                    c.set(
                        crate::abi::target(0x0062_4B70),
                        this,
                        &[frame.add(4).read(), cursor, frame.add(6).read(), args.add(3) as u32],
                        args.add(3) as u32,
                        edx,
                    );
                    stage = 42;
                }
                42 | 48 => {
                    if eax as u8 != 0 {
                        let output = args.add(3).read() as *mut u8;
                        let network = frame.add(5).read();
                        c.virtual_call(output, 0x58, &[network], network, read(output, 0));
                        stage += 1;
                    } else if real.read() as u8 == 0 {
                        let (x, z) = if stage == 42 {
                            (frame.add(4).read(), cursor)
                        } else {
                            (outer.read(), outer.add(1).read())
                        };
                        let key = z.wrapping_shl(16).wrapping_add(x);
                        c.set(crate::abi::target(0x0064_7A20), this.add(0x1C), &[key], eax & !255, edx);
                        stage += 2;
                    } else {
                        stage = if stage == 42 { 46 } else { 52 };
                        continue;
                    }
                }
                43 | 49 => {
                    if eax as u8 != 0 {
                        if stage == 43 {
                            a.write(frame.add(4).read());
                            a.add(1).write(cursor);
                        } else {
                            b.write(outer.read());
                            b.add(1).write(outer.add(1).read());
                        }
                        stage = if stage == 43 { 46 } else { 52 };
                        continue;
                    }
                    c.set(crate::abi::target(0x0090_CF54), null_mut(), &[24], eax, edx);
                    stage += 2;
                }
                44 | 50 => {
                    if eax != 0 && read::<u32>(eax as *mut u8, 8) & args.add(4).read() != 0 {
                        if stage == 44 {
                            a.write(frame.add(4).read());
                            a.add(1).write(cursor);
                        } else {
                            b.write(outer.read());
                            b.add(1).write(outer.add(1).read());
                        }
                    }
                    stage = if stage == 44 { 46 } else { 52 };
                    continue;
                }
                45 | 51 => {
                    let (pa, pb) = if stage == 45 {
                        ([first.read(), first.add(1).read()], [frame.add(4).read(), cursor])
                    } else {
                        ([outer.read(), outer.add(1).read()], [last.read(), last.add(1).read()])
                    };
                    link_join(eax as *mut u8, args.add(2).read() as *mut u8, pa, pb);
                    stage = if stage == 45 { 46 } else { 52 };
                    continue;
                }
                46 => {
                    let output = args.add(3).read() as *mut u8;
                    if output.is_null() {
                        stage = 47;
                        continue;
                    }
                    args.add(3).write(0);
                    c.virtual_call(output, 8, &[], read(output, 0), edx);
                    stage = 47;
                }
                47 => {
                    c.set(
                        crate::abi::target(0x0062_4B70),
                        this,
                        &[
                            outer.read(),
                            outer.add(1).read(),
                            frame.add(6).read(),
                            args.add(3) as u32,
                        ],
                        eax,
                        frame.add(6).read(),
                    );
                    stage = 48;
                }
                52 => {
                    drag.set_anchors_end(drag.anchors());
                    stage = 53;
                    continue;
                }
                53 | 54 => {
                    let point = if stage == 53 { a } else { b };
                    stage += 1;
                    if append_anchor_command(drag, point, real, c) {
                        break;
                    }
                    continue;
                }
                58 => {
                    args.write(args.read().wrapping_add(1));
                    stage = 40;
                    continue;
                }
                70 => {
                    if drag.failure() != 0 {
                        answer.write(0);
                    }
                    frame.add(5).write(real.read());
                    first.write(args.add(5).read());
                    last.write(args.add(2).read());
                    cursor = read(last.read() as *mut u8, 0);
                    stage = if tool.real() != 0 && args.add(5).read() != 0 {
                        71
                    } else {
                        73
                    };
                    continue;
                }
                71 => {
                    if cursor == last.read() {
                        stage = 73;
                        continue;
                    }
                    let node = cursor as *mut u8;
                    let callback = first.read() as *mut u8;
                    c.virtual_call(
                        callback,
                        0x1C,
                        &[
                            read(node, 8),
                            read(node, 12),
                            read(node, 16),
                            read(node, 20),
                            0,
                            0,
                            frame.add(5).read(),
                            u32::MAX,
                        ],
                        read(callback, 0),
                        read(node, 8),
                    );
                    stage = 72;
                }
                72 => {
                    cursor = read(cursor as *mut u8, 0);
                    stage = 71;
                    continue;
                }
                73 => {
                    cursor = read(last.read() as *mut u8, 0);
                    stage = 74;
                    continue;
                }
                74 => {
                    let head = last.read();
                    if cursor == head {
                        write(head as *mut u8, 0, head);
                        write(head as *mut u8, 4, head);
                        c.set(crate::abi::target(0x0090_CF63), null_mut(), &[head], eax, edx);
                        stage = 76;
                    } else {
                        frame.add(4).write(read(cursor as *mut u8, 0));
                        c.set(crate::abi::target(0x0090_CF63), null_mut(), &[cursor], cursor, edx);
                        stage = 75;
                    }
                }
                75 => {
                    cursor = frame.add(4).read();
                    stage = 74;
                    continue;
                }
                76 => {
                    let output = args.add(3).read() as *mut u8;
                    if output.is_null() {
                        stage = 89;
                        continue;
                    }
                    c.virtual_call(output, 8, &[], read(output, 0), edx);
                    stage = 89;
                }
                89 => {
                    let filter = frame.add(6).read() as *mut u8;
                    c.virtual_call(filter, 8, &[], eax, read(filter, 0));
                    stage = 90;
                }
                _ => {
                    let end = drag.anchors_end() as u32;
                    drag.set_anchors_end(drag.anchors());
                    tool.set_insertion_data(1);
                    c.target = 0;
                    c.eax = end & !255 | u32::from(answer.read());
                    c.edx = 0;
                    break;
                }
            }
            break;
        }
        registers(stage, cursor)
    }
}

pub const PORTS: &[Port] = {
    let _ = fc4p_00629210::PORT;
    &[]
};

#[cfg(test)]
mod tests;
