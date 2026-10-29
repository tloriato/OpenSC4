// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Solved-cell insertion, connection refresh, and support previews.

use super::*;

layout! {
    /// Cached cell coordinates, occupant, type connections and corner vertices.
    struct InsertionCell {
        0x00 => x / set_x: u32,
        0x04 => z / set_z: u32,
        0x08 => flags / set_flags: u32,
        0x0C => occupant / set_occupant: *mut u8,
        0x4C => connections / set_connections: u32,
        0x50 => direction / set_direction: u8,
        0x51 => placed / set_placed: u8,
        0x56 => connection / set_connection: u8,
    }
}

game_fn! { fn cell_info(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_fn! { fn network_manager() -> *mut u8 = "C" 0x0060_2680; }
game_fn! { fn separate(this: *mut u8, cell: *mut u8, kind: u32, piece: *mut u8, rotation: *mut u8, flip: *mut u8) -> u32 = "thiscall" 0x0063_3FA0; }
game_fn! { fn untextured(this: *mut u8, cell: *mut u8, color: u32, heights: *mut u8, layer: u32, flags: u32) = "thiscall" 0x0062_0820; }
game_fn! { #[allow(clippy::too_many_arguments)] fn textured(this: *mut u8, cell: *mut u8, color: u32, heights: *mut u8, tgi: *mut u8, rotation: u32, flip: u32, layer: u32, flags: u32) = "thiscall" 0x0062_0BC0; }
game_data! { static resources: *mut u8 = 0x00B4_3CA8; }
game_data! { static occupants: *mut u8 = 0x00B4_3D0C; }
game_data! { static type_bits: u32 = 0x00AA_835C; }
game_data! { static type_groups: u8 = 0x00B4_52D4; }
game_data! { static support_buckets: *mut u8 = 0x00B4_52B4; }
game_data! { static support_buckets_end: *mut u8 = 0x00B4_52B8; }
game_data! { static layers: u32 = 0x00AA_83F0; }

port! {
    /// Insert solved cells in vector order, or refresh their four heights and draw previews. Connection
    /// nodes are refreshed in place; prebuilt one-way pieces only change direction. The scratch words
    /// retain their original upper bytes when only a byte is assigned before a virtual call.
    #[port(address = 0x0062_58C0, owner = "networktool", name = "cSC4NetworkTool::InsertOccupantsForSolvedCells", naked)]
    pub unsafe extern "C" fn fc4p_006258c0() {
        core::arch::naked_asm!(
            "push ebp", "mov ebp, esp", "and esp, -8", "sub esp, 0x58",
            "mov edx, esp", "lea eax, [ebp + 8]", "push eax", "call {body}",
            "mov esp, ebp", "pop ebp", "ret 12", body = sym insert_solved,
        );
    }
}

unsafe fn refresh_heights(this: *mut u8, cell: *mut u8) {
    unsafe {
        let offset = if InsertionCell::from_ptr(cell).connection() != 0 {
            100_000
        } else {
            0
        };
        for i in 0..4 {
            let vertex = read::<u32>(cell, 0xA8 + 4 * i).wrapping_add(offset);
            let height = vcall_st0!(this, 0x124, vertex).to_f32();
            write(cell, 0x58 + 4 * i, height);
        }
    }
}

fn bucket(value: u32, count: u32) -> usize {
    let remainder: u32;
    unsafe {
        core::arch::asm!("div {count:e}", count = in(reg) count,
            inout("eax") value => _, inout("edx") 0u32 => remainder, options(nomem, nostack));
    }
    remainder.wrapping_mul(4) as usize
}

unsafe fn refresh_connection(this: *mut u8, cell: *mut u8, request: *mut u8, s: *mut u8) {
    unsafe {
        let occupant = InsertionCell::from_ptr(cell).occupant();
        write(s, 0x28, 0u32);
        write(s, 0x13, 0u8);
        write(s, 0x30, *resources());
        for kind in 0..13 {
            if InsertionCell::from_ptr(cell).flags() & type_bits().add(kind).read() != 0 {
                let connections: u32 = read(cell, 0x18 + kind * 4);
                write(s, 0x28, connections);
                vcall!(occupant, 0x94 => fn(u32, u32), kind as u32, connections);
                write(s, 0x54, read::<u32>(request, 0));
                write(s, 0x50, read::<u32>(type_groups(), kind * 0x114));
                write(s, 0x4C, 0x6534_284Au32);
                let receiver = read(s, 0x30);
                if vcall!(receiver, 0xC => fn(*mut u8, u32, *mut u8, u32, u32) -> u32,
                    s.add(0x4C), 0xA521_60F5, s.add(0x24), 0, 0) as u8
                    != 0
                {
                    let resource: *mut u8 = read(s, 0x24);
                    vcall!(resource, 8 => fn());
                    write(s, 0x24, 0u32);
                    write(s, 0x13, 1u8);
                }
            }
        }
        let got = vcall!(occupant, 0x34 => fn(*mut u8, *mut u8) -> u32, s.add(0x44), s.add(0x34));
        let same = got as u8 != 0 && read::<u32>(s, 0x28) == read::<u32>(s, 0x34);
        write(s, 0x2C, u8::from(same));
        let direction = if vcall!(occupant, 0x3C => fn() -> u32) != 0 {
            vcall!(occupant, 0x68 => fn() -> u32)
        } else {
            u32::from(read::<u8>(request, 4))
        };
        vcall!(occupant, 0x78 => fn(u32, u32), read::<u32>(request, 0), direction);
        vcall!(occupant, 0x28 => fn(u32), 0x0800_0000);
        vcall!(occupant, 0x2C => fn(u32), 0x0200_0000);
        vcall!(occupant, 0x2C => fn(u32), 0x0400_0000);
        if vcall!(occupant, 0x3C => fn() -> u32) == 0 {
            let model = if read::<u8>(s, 0x13) != 0 { read(request, 0) } else { 0 };
            vcall!(occupant, 0xCC => fn(u32), model);
        }
        let manager = *occupants();
        let table: *mut u8 = read(manager, 0);
        let object = vcall!(occupant, 0xC0 => fn() -> *mut u8);
        let remove: unsafe extern "thiscall" fn(*mut u8, *mut u8, u32, u32) =
            core::mem::transmute(read::<usize>(table, 0x54));
        remove(manager, object, 0, 0);
        vcall!(occupant, 0x50 => fn(u32), 0);
        let manager = *occupants();
        let table: *mut u8 = read(manager, 0);
        let object = vcall!(occupant, 0xC0 => fn() -> *mut u8);
        let insert: unsafe extern "thiscall" fn(*mut u8, *mut u8, u32) =
            core::mem::transmute(read::<usize>(table, 0x50));
        insert(manager, object, 0);
        vcall!(occupant, 0x50 => fn(u32), read::<u32>(s, 0x2C));
        let _ = this;
    }
}

unsafe extern "fastcall" fn insert_solved(this: *mut u8, s: *mut u8, args: *mut u32) -> u32 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let vector = args.read() as *mut u8;
        let output = args.add(1).read();
        let preview_only = args.add(2).read() as u8;
        let mut request: *mut u8 = read(vector, 0);
        let end: *mut u8 = read(vector, 4);
        if tool.real() != 0 && preview_only == 0 {
            while request != end {
                let cell = cell_info(this, read(request, 8));
                if !cell.is_null() {
                    let view = InsertionCell::from_ptr(cell);
                    let occupant = view.occupant();
                    view.set_placed(1);
                    if !occupant.is_null() && vcall!(occupant, 0x20 => fn(u32) -> u32, 0x0020_0000) as u8 != 0 {
                        refresh_connection(this, cell, request, s);
                    } else if !occupant.is_null() && vcall!(occupant, 0x20 => fn(u32) -> u32, 0x0040_0000) as u8 != 0 {
                        if view.flags() & 0x400 != 0 {
                            let old = vcall!(occupant, 0x114 => fn() -> u32) as u8;
                            let direction = view.direction();
                            write(s, 0x38, direction);
                            if old != direction {
                                vcall!(occupant, 0x118 => fn(u32), read::<u32>(s, 0x38));
                                vcall!(occupant, 0x50 => fn(u32), 0);
                                vcall!(occupant, 0x50 => fn(u32), 1);
                            }
                        }
                    } else {
                        vcall!(this, 0x120 => fn(*mut u8, *mut u8, u32), cell, request, output);
                    }
                }
                request = request.wrapping_add(12);
            }
            return end as u32;
        }
        while request != end {
            let cell = cell_info(this, read(request, 8));
            if !cell.is_null() {
                refresh_heights(this, cell);
                if preview_only == 0 {
                    InsertionCell::from_ptr(cell).set_placed(1);
                }
                vcall!(this, 0xE8 => fn(*mut u8, *mut u8), cell, request);
            }
            request = request.wrapping_add(12);
        }
        let mut support = tool.support_begin();
        let support_end = tool.support_end();
        let manager = network_manager();
        if support == support_end {
            return manager as u32;
        }
        while support != support_end {
            let buckets = *support_buckets();
            let count = ((*support_buckets_end() as u32).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
            let key: u32 = read(support, 0);
            let mut node: *mut u8 = read(buckets, bucket(key, count));
            while !node.is_null() && read::<u32>(node, 4) != key {
                node = read(node, 0);
            }
            let definition = if node.is_null() { node } else { node.wrapping_add(8) };
            let mut key_ptr: *mut u8 = read(support, 0x1C);
            let keys_end: *mut u8 = read(support, 0x20);
            write(s, 0x4C, 0x7AB5_0E44u32);
            write(s, 0x50, 0x1ABE_787Du32);
            write(s, 0x54, 0u32);
            while key_ptr != keys_end {
                let cell = cell_info(this, read(key_ptr, 0));
                if !cell.is_null() {
                    refresh_heights(this, cell);
                    let view = InsertionCell::from_ptr(cell);
                    if preview_only == 0 {
                        view.set_placed(1);
                    }
                    let width: u32 = read(definition, 0xC4);
                    let x = view
                        .x()
                        .wrapping_sub(read(definition, 0x18))
                        .wrapping_sub(read(support, 4));
                    if x < width {
                        let index = view
                            .z()
                            .wrapping_sub(read(definition, 0x1C))
                            .wrapping_sub(read(support, 8))
                            .wrapping_mul(width)
                            .wrapping_add(x);
                        let bytes: *mut u8 = read(definition, 0xB8);
                        if index < read::<u32>(definition, 0xBC).wrapping_sub(bytes as u32) {
                            let id = read::<u8>(bytes, index as usize);
                            let buckets: *mut u8 = read(definition, 0xD4);
                            let count = (read::<u32>(definition, 0xD8).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
                            let mut node: *mut u8 = read(buckets, bucket(u32::from(id), count));
                            while !node.is_null() && read::<u8>(node, 4) != id {
                                node = read(node, 0);
                            }
                            if !node.is_null() {
                                let value = node.wrapping_add(8);
                                if !value.is_null() && view.connections() != 0 {
                                    for kind in 0..13 {
                                        if read::<u32>(cell, 0x18 + 4 * kind) != 0 {
                                            write(s, 0x44, view.x());
                                            write(s, 0x48, view.z());
                                            let mode = read::<u8>(value, 4);
                                            write(s, 0x28, mode >> 7);
                                            write(s, 0x13, mode & 3);
                                            let ok = separate(
                                                this,
                                                s.add(0x44),
                                                kind as u32,
                                                s.add(0x34),
                                                s.add(0x13),
                                                s.add(0x28),
                                            );
                                            if ok as u8 != 0 {
                                                let piece = read::<u32>(this, 0xFC).wrapping_add(read(s, 0x34));
                                                let texture = vcall!(manager, 0x34 => fn(u32, *mut u8) -> u32, piece, s.add(0x13));
                                                write(s, 0x54, texture);
                                                textured(
                                                    tool.preview(),
                                                    s.add(0x44),
                                                    0xFF00_00FF,
                                                    cell.add(0x58),
                                                    s.add(0x4C),
                                                    u32::from(read::<u8>(s, 0x13)),
                                                    read(s, 0x28),
                                                    layers().add(kind).read(),
                                                    0,
                                                );
                                            } else {
                                                untextured(
                                                    tool.preview(),
                                                    s.add(0x44),
                                                    0xFF80_80FF,
                                                    cell.add(0x58),
                                                    layers().add(kind).read(),
                                                    0,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                key_ptr = key_ptr.wrapping_add(4);
            }
            support = support.wrapping_add(40);
        }
        support_end as u32
    }
}

pub const PORTS: &[Port] = &[fc4p_006258c0::PORT];

#[cfg(test)]
mod tests;
