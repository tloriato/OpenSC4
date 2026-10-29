// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Remove elevated-path directions that are absent from neighboring network occupants.

use super::*;
use core::mem::MaybeUninit;

layout! {
    /// The simulator's array of 46-byte intersection records.
    pub struct ElevatedSim {
        0x58 => intersections / set_intersections: *mut u8,
    }
}

game_fn! { fn reserve_points(this: *mut u8, count: u32) = "thiscall" 0x0053_DC50; }
game_fn! { fn insert_point(this: *mut u8, end: u32, point: *const u8, scratch: *mut u32, a: u32, b: u32) = "thiscall" 0x0050_C3B0; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_holder(this: *mut u32) = "thiscall" 0x0041_1870; }
game_fn! { fn clear_occupants(this: *mut u32) = "thiscall" 0x0041_3E60; }
game_fn! { fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_data! { static node_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static occupants: *mut u8 = 0x00B4_3D0C; }
game_data! { static dx: [i32; 4] = 0x00AB_325C; }
game_data! { static dz: [i32; 4] = 0x00AB_326C; }

unsafe fn allocator(scratch: *mut u32) -> *mut u8 {
    // SAFETY: the framework fills a retained allocator interface, then the temporary holder releases it.
    unsafe {
        let mut result = node_allocator().read();
        if result.is_null() {
            scratch.write(0);
            let app = framework();
            if !app.is_null() {
                vcall!(app, 0x14 => fn(u32, u32, *mut u32), 0x3AE4_BEA3, 0x3AE4_BEAB, scratch);
            }
            node_allocator().write(scratch.read() as *mut u8);
            release_holder(scratch);
            result = node_allocator().read();
        }
        result
    }
}

port! {
    /// Collects matching path endpoints, queries occupants at the adjacent cell, and replaces the
    /// selected four-bit direction field with the paths those occupants provide. Releases every
    /// occupant and queried interface, and frees both vectors and the temporary occupant list.
    /// The third caller argument is reused as the list header; a second allocator lookup can reuse
    /// the fourth caller argument as an interface holder.
    #[port(address = 0x0071_5940, owner = "traffic", name = "cSC4TrafficSimulator::RemoveBadElevatedPaths", naked)]
    pub unsafe extern "C" fn fc4p_00715940() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 20", sym remove_paths);
    }
}

unsafe extern "fastcall" fn remove_paths(this: *mut u8, stack: *mut u32) {
    // SAFETY: the entry supplies the original simulator, path interface, and five caller arguments.
    unsafe {
        let path = stack.add(3).read() as *mut u8;
        let mut keys = [0u32; 3];
        let count = vcall!(path, 0x38 => fn(u32, u32, *mut u32) -> u32, 1, stack.add(4).read(), keys.as_mut_ptr());
        let mut points = [0u32; 3];
        reserve_points(points.as_mut_ptr().cast(), count);
        let mut end = points[1];
        for i in 0..count {
            let key = (keys[0] as *mut u32).wrapping_add(i as usize).read();
            if u32::from(key as u8) == stack.add(5).read() {
                let info = vcall!(path, 0x0C => fn(u32, u32) -> *mut u8, key, 0);
                if !info.is_null() {
                    let point = read_unaligned(info.cast::<*const u8>());
                    if end == points[2] {
                        insert_point(points.as_mut_ptr().cast(), end, point, stack.add(4), 1, 1);
                        end = points[1];
                    } else {
                        if end != 0 {
                            for j in 0..3 {
                                store(end as *mut u8, j * 4, word(point.cast_mut(), j * 4));
                            }
                        }
                        end = end.wrapping_add(12);
                        points[1] = end;
                    }
                }
            }
        }
        let mut begin = points[0];
        if begin != end {
            let point_count = end.wrapping_sub(begin) as i32 / 12;
            let direction = stack.add(4).read();
            let mut cell = [
                stack
                    .add(1)
                    .read()
                    .wrapping_add(dx().cast::<u32>().wrapping_add(direction as usize).read()),
                stack
                    .add(2)
                    .read()
                    .wrapping_add(dz().cast::<u32>().wrapping_add(direction as usize).read()),
            ];
            stack.add(3).write(0);
            let mut holder = 0u32;
            let pool = allocator(&mut holder);
            let head = vcall!(pool, 0x0C => fn(u32) -> *mut u8, 12);
            store(head, 0, head as u32);
            store(head, 4, head as u32);
            stack.add(3).write(head as u32);
            vcall!(occupants().read(), 0xA4 => fn(*mut u32, *mut u32, *mut u32, u32, u32),
                stack.add(3), cell.as_mut_ptr(), cell.as_mut_ptr(), 0xC772_BF98, u32::MAX);
            let sentinel = stack.add(3).read() as *mut u8;
            let mut node = word(sentinel, 0) as *mut u8;
            while node != sentinel {
                let occupant = word(node, 8) as *mut u8;
                let mut network = MaybeUninit::<*mut u8>::uninit();
                if vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0xA821_EF94, network.as_mut_ptr()) as u8 != 0 {
                    let network = network.assume_init();
                    if vcall!(network, 0x24 => fn(u32) -> u32, 0x1F4F) as u8 != 0 {
                        let paths = vcall!(network, 0xC4 => fn() -> *mut u8);
                        if !paths.is_null() {
                            let opposite = (stack.add(4).read() as i32).wrapping_add(2) % 4;
                            let mut mask = 0u32;
                            let mut point = points[0] as *mut u8;
                            for _ in 0..point_count {
                                let mut found = MaybeUninit::<[u32; 3]>::uninit();
                                vcall!(paths, 0x1C => fn(*mut u32, u32, *mut u8, u32, i32, u32),
                                    found.as_mut_ptr().cast(), 1, point, 255, opposite, 0x4080_0000);
                                let found = found.assume_init();
                                let mut id = found[0] as *mut u8;
                                while id as u32 != found[1] {
                                    mask |= 1u32.wrapping_shl(u32::from(id.add(1).read()));
                                    id = id.wrapping_add(4);
                                }
                                if found[0] != 0 {
                                    free(found[0] as *mut u8);
                                }
                                point = point.wrapping_add(12);
                            }
                            let key = stack.add(1).read().wrapping_shl(8) | stack.add(2).read();
                            let sim = ElevatedSim::from_ptr(this);
                            let dest = sim
                                .intersections()
                                .wrapping_add(
                                    key.wrapping_mul(46)
                                        .wrapping_add(0x24)
                                        .wrapping_add(stack.add(4).read().wrapping_mul(2))
                                        as usize,
                                )
                                .cast::<u16>();
                            let shift = stack.add(5).read().wrapping_mul(4);
                            let old = dest.read_unaligned();
                            dest.write_unaligned(
                                (u32::from(old) & !15u32.wrapping_shl(shift) | mask.wrapping_shl(shift)) as u16,
                            );
                        }
                    }
                    vcall!(network, 8 => fn());
                    begin = points[0];
                }
                vcall!(occupant, 8 => fn());
                node = word(node, 0) as *mut u8;
            }
            clear_occupants(stack.add(3));
            let header = stack.add(3).read();
            let pool = allocator(stack.add(4));
            vcall!(pool, 0x10 => fn(u32), header);
        }
        if begin != 0 {
            free(begin as *mut u8);
        }
        if keys[0] != 0 {
            free(keys[0] as *mut u8);
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_00715940::PORT];

#[cfg(test)]
mod tests;
