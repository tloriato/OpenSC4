// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Refresh cached network corners and append existing occupants to the solved vector.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::x87::F80;
use core::ptr::null_mut;

game_fn! { #[allow(dead_code)] fn begin_set(this: *mut u8, iterator: *mut u32) -> u64 = "thiscall" 0x004B_0390; }
game_fn! { fn find_cell(this: *mut u8, key: u32) -> u64 = "thiscall" 0x0063_3220; }
game_fn! { fn find_occupant(this: *mut u8, x: u32, z: u32, filter: u32, output: *mut *mut u8) = "thiscall" 0x0062_4B70; }
game_fn! { fn grow_solved(this: *mut u8, position: *mut u8, record: *mut u32, scratch: *mut u8, count: u32, copy: u32) = "thiscall" 0x0062_6240; }
game_fn! { fn get_height(this: *mut u8, vertex: u32, output: *mut u8) -> u64 = "thiscall" 0x0065_EB00; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }

layout! {
    /// Additional state used by the two post-grading refresh passes.
    struct RefreshTool {
        0xA0 => solved_capacity / set_solved_capacity: *mut u8,
        0x364 => filter / set_filter: u32,
    }
}
layout! {
    /// Coordinates, cached corners, vertex ids and solved-vector membership.
    struct RefreshCell {
        0x00 => x / set_x: u32,
        0x04 => z / set_z: u32,
        0x51 => placed / set_placed: u8,
        0x56 => connection / set_connection: u8,
        0xB8 => solved_index / set_solved_index: i32,
    }
}

// The orientation is stored as a byte in a record later copied as three words. Preserve the other
// three bytes from the original stack location before the ABI wrapper uses that stack space.
macro_rules! refresh_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $ret:literal) => {
        port! {
            #[port(address = $address, owner = "networktool", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push dword ptr [esp - 8]", "push eax", "push edx", "mov edx, esp",
                    "call {body}", "add esp, 12", $ret, body = sym $body,
                );
            }
        }
    };
}

fn record_count(begin: *mut u8, end: *mut u8) -> i32 {
    (end as u32).wrapping_sub(begin as u32) as i32 / 12
}

// Keep the CPU's division behavior, including its exception for a zero divisor.
fn divide(value: u32, divisor: u32) -> (u32, u32) {
    let mut quotient = value;
    let remainder: u32;
    unsafe {
        core::arch::asm!(
            "div {divisor:e}", divisor = in(reg) divisor,
            inout("eax") quotient, inout("edx") 0u32 => remainder,
            options(nomem, nostack),
        );
    }
    (quotient, remainder)
}

/// Compare the unrounded returned height with the stored corner, then store the returned height.
/// Quiet NaNs compare unequal without raising the invalid exception; signaling NaNs still raise it.
unsafe fn refresh_corner(height: F80, corner: *mut u8) -> bool {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{height}]", "fld dword ptr [{corner}]", "fld st(1)",
            "fucompp", "fnstsw ax", "fstp dword ptr [{corner}]",
            height = in(reg) &height, corner = in(reg) corner, out("ax") status,
            out("st(0)") _, out("st(1)") _, out("st(2)") _,
        );
    }
    // TEST AH,44 has even parity for both unequal and unordered comparisons.
    ((status >> 8) as u8 & 0x44).count_ones() & 1 == 0
}

unsafe fn store_record(destination: *mut u8, record: &[u32; 3]) {
    if !destination.is_null() {
        unsafe {
            write(destination, 0, record[0]);
            write(destination, 4, record[1]);
            write(destination, 8, record[2]);
        }
    }
}

refresh_port!(
    0x0062_9CF0,
    fc4p_00629cf0,
    "cSC4NetworkTool::MarkPositionChangedExistingOccupantsAsNewlySolved",
    position_changed,
    "ret"
);
unsafe extern "fastcall" fn position_changed(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let refresh = RefreshTool::from_ptr(this);
        let mut iterator = [0u32; 2];
        let args = [iterator.as_mut_ptr() as u32];
        let mut c = RawCall::new(
            Conv::Thiscall,
            crate::abi::target(0x004B_0390),
            this.add(0xA4) as u32,
            &args,
        );
        c.eax = args[0];
        c.edx = frame.read();
        call_raw(&mut c);
        let mut edx = c.edx;
        let mut record = [0, frame.add(2).read(), 0];
        while iterator[0] != 0 {
            let node = iterator[0] as *mut u8;
            let key: u32 = read(node, 4);
            let cell = find_cell(this, key) as u32 as *mut u8;
            let view = RefreshCell::from_ptr(cell);
            let offset = if view.connection() != 0 { 100_000u32 } else { 0 };
            let mut changed = 0u8;
            for corner in 0..4 {
                let vertex = read::<u32>(cell, 0xA8 + corner * 4).wrapping_add(offset);
                let height = vcall_st0!(this, 0x124, vertex);
                changed |= u8::from(refresh_corner(height, cell.add(0x58 + corner * 4)));
            }
            if changed != 0 && view.solved_index() < 0 && view.placed() != 0 {
                let mut occupant = null_mut();
                find_occupant(this, view.x(), view.z(), refresh.filter(), &mut occupant);
                record[0] = vcall!(occupant, 0x5C => fn() -> u32);
                let orientation = vcall!(occupant, 0x68 => fn() -> u32) as u8;
                record[1] = (record[1] & !255) | u32::from(orientation);
                view.set_solved_index(record_count(tool.solved_begin(), tool.solved_end()));
                let end = tool.solved_end();
                record[2] = key;
                if end != refresh.solved_capacity() {
                    store_record(end, &record);
                    tool.set_solved_end(end.wrapping_add(12));
                } else {
                    grow_solved(this.add(0x98), end, record.as_mut_ptr(), &mut changed, 1, 1);
                }
                if !occupant.is_null() {
                    vcall!(occupant, 8 => fn());
                }
            }
            let next: u32 = read(node, 0);
            if next != 0 {
                iterator[0] = next;
            } else {
                let table = iterator[1] as *mut u8;
                let begin: *mut u8 = read(table, 4);
                let end: *mut u8 = read(table, 8);
                let count = ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32;
                edx = divide(read::<u32>(node, 4), count).1;
                let count = ((read::<u32>(table, 8)).wrapping_sub(begin as u32) as i32 >> 2) as u32;
                iterator[0] = 0;
                loop {
                    edx = edx.wrapping_add(1);
                    if edx >= count {
                        break;
                    }
                    iterator[0] = read(read::<*mut u8>(table, 4), edx.wrapping_mul(4) as usize);
                    if iterator[0] != 0 {
                        break;
                    }
                }
            }
        }
        registers(0, edx)
    }
}

refresh_port!(
    0x0062_9EB0,
    fc4p_00629eb0,
    "cSC4NetworkTool::MarkTerrainChangedExistingOccupantsAsNewlySolved",
    terrain_changed,
    "ret 4"
);
unsafe extern "fastcall" fn terrain_changed(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let refresh = RefreshTool::from_ptr(this);
        let vertices = frame.add(4).read() as *mut u8;
        let mut cursor: *mut u8 = read(vertices, 0);
        let end: *mut u8 = read(vertices, 4);
        let mut edx = frame.read();
        let mut record = [0, frame.add(2).read(), 0];
        while cursor != end {
            let vertex: u32 = read(cursor, 0);
            let (vz, vx) = divide(vertex, tool.vertex_stride());
            let x0 = if (vx as i32) > 0 { vx.wrapping_sub(1) } else { 0 };
            let z0 = if (vz as i32) > 0 { vz.wrapping_sub(1) } else { 0 };
            let x1 = if vx < tool.width() {
                vx
            } else {
                tool.width().wrapping_sub(1)
            };
            edx = tool.height();
            let z1 = if vz < edx {
                vz
            } else {
                edx = edx.wrapping_sub(1);
                edx
            };
            let mut z = z0;
            let mut row_key = z0.wrapping_shl(16).wrapping_add(x0);
            while (z as i32) <= z1 as i32 {
                let mut x = x0;
                let mut key = row_key;
                while (x as i32) <= x1 as i32 {
                    let args = [key];
                    let mut c = RawCall::new(Conv::Thiscall, crate::abi::target(0x0063_3220), this as u32, &args);
                    c.eax = key;
                    c.edx = edx;
                    call_raw(&mut c);
                    edx = c.edx;
                    let cell = c.eax as *mut u8;
                    if !cell.is_null() {
                        let view = RefreshCell::from_ptr(cell);
                        if view.solved_index() < 0 && view.placed() != 0 {
                            let mut occupant = null_mut();
                            if x < tool.width() && z < tool.height() {
                                let mut base = null_mut();
                                let manager = tool.occupant_manager();
                                let found = vcall!(manager, 0x80 => fn(*mut *mut u8, u32, u32, u32) -> u32,
                                    &mut base, x, z, refresh.filter());
                                if found as u8 != 0 {
                                    vcall!(base, 0 => fn(u32, *mut *mut u8), 0xA821_EF94, &mut occupant);
                                }
                                if !base.is_null() {
                                    vcall!(base, 8 => fn());
                                }
                            }
                            record[0] = vcall!(occupant, 0x5C => fn() -> u32);
                            record[1] = (record[1] & !255) | u32::from(vcall!(occupant, 0x68 => fn() -> u32) as u8);
                            view.set_solved_index(record_count(tool.solved_begin(), tool.solved_end()));
                            for corner in 0..4 {
                                let vertex = read::<u32>(cell, 0xA8 + corner * 4);
                                edx = (get_height(this.add(0x124), vertex, cell.add(0x58 + corner * 4)) >> 32) as u32;
                            }
                            record[2] = key;
                            let old_end = tool.solved_end();
                            if old_end != refresh.solved_capacity() {
                                store_record(old_end, &record);
                                if !old_end.is_null() {
                                    edx = record[1];
                                }
                                tool.set_solved_end(old_end.wrapping_add(12));
                            } else {
                                let count = record_count(tool.solved_begin(), old_end) as u32;
                                let capacity = count.wrapping_add(if count < 1 { 1 } else { count });
                                let buffer = if capacity == 0 {
                                    null_mut()
                                } else {
                                    allocate(capacity.wrapping_mul(12))
                                };
                                let mut source = tool.solved_begin();
                                let mut destination = buffer;
                                while source != old_end {
                                    if !destination.is_null() {
                                        write(destination, 0, read::<u32>(source, 0));
                                        write(destination, 4, read::<u32>(source, 4));
                                        edx = read(source, 8);
                                        write(destination, 8, edx);
                                    }
                                    source = source.wrapping_add(12);
                                    destination = destination.wrapping_add(12);
                                }
                                if !destination.is_null() {
                                    store_record(destination, &record);
                                    edx = key;
                                }
                                let new_end = destination.wrapping_add(12);
                                let old_begin = tool.solved_begin();
                                if !old_begin.is_null() {
                                    edx = (free(old_begin) >> 32) as u32;
                                }
                                tool.set_solved_begin(buffer);
                                tool.set_solved_end(new_end);
                                refresh.set_solved_capacity(buffer.wrapping_add(capacity.wrapping_mul(12) as usize));
                            }
                            if !occupant.is_null() {
                                let r = super::occupants::virtual_call(occupant, 8, &[], 0, read(occupant, 0));
                                edx = r.edx;
                            }
                        }
                    }
                    x = x.wrapping_add(1);
                    key = key.wrapping_add(1);
                }
                z = z.wrapping_add(1);
                row_key = row_key.wrapping_add(0x10000);
            }
            cursor = cursor.wrapping_add(4);
        }
        registers(end as u32, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_00629cf0::PORT, fc4p_00629eb0::PORT];

#[cfg(test)]
mod tests;
