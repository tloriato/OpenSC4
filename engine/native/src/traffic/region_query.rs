// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Region bit masks, directional car subnetworks and hash-order query output.

use super::late_containers::call;
use super::monthly_queries::{frame_entry, unsigned_division};
use super::*;
use crate::abi::Conv;

layout! {
    /// Rows and inclusive bounds of the network-info grid.
    pub struct RegionSim {
        0x7C => rows / set_rows: *mut u8,
        0x18C => min_x / set_min_x: i32,
        0x190 => min_z / set_min_z: i32,
        0x194 => max_x / set_max_x: i32,
        0x198 => max_z / set_max_z: i32,
    }
}
layout! {
    /// Region bounds, logical mask dimensions and packed rows of bits.
    pub struct Region {
        0x00 => min_x / set_min_x: i32,
        0x04 => min_z / set_min_z: i32,
        0x08 => max_x / set_max_x: i32,
        0x0C => max_z / set_max_z: i32,
        0x14 => width / set_width: u32,
        0x18 => height / set_height: u32,
        0x20 => rows / set_rows: *mut u32,
    }
}
game_fn! { #[allow(dead_code)] fn construct_set(this: *mut u32, hint: u32) = "thiscall" 0x0082_9BC0; }
game_fn! { #[allow(dead_code)] fn reserve_set(this: *mut u32, count: u32) = "thiscall" 0x004F_0940; }
game_fn! { #[allow(dead_code)] fn insert_set(this: *mut u32, result: *mut u32, key: *const u32) = "thiscall" 0x0062_4DD0; }
game_fn! { #[allow(dead_code)] fn clear_set(this: *mut u32) = "thiscall" 0x008E_BA60; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn copy(destination: *mut u8, source: *mut u8, bytes: u32) -> *mut u8 = "C" 0x009E_EB70; }

unsafe fn append(output: *mut u8, node: *mut u8, args: *mut u32, r: &mut Registers) {
    unsafe {
        let end = word(output, 4);
        if end != word(output, 8) {
            if end != 0 {
                r.edx = word(node, 4);
                store(end as *mut u8, 0, r.edx);
            }
            store(output, 4, word(output, 4).wrapping_add(4));
            return;
        }
        let len = (end.wrapping_sub(word(output, 0)) as i32 >> 2) as u32;
        args.write(1);
        args.add(1).write(len);
        let capacity = len.wrapping_add(if len < 1 { 1 } else { len });
        args.write(capacity);
        let fresh = if capacity == 0 {
            0
        } else {
            r.eax = capacity.wrapping_mul(4);
            r.ecx = capacity;
            call(0x0090_CF54, Conv::Cdecl, &[r.eax], r);
            r.eax
        };
        r.ecx = args.read();
        r.eax = word(output, 0);
        if end == r.eax {
            r.eax = fresh;
        } else {
            let bytes = end.wrapping_sub(r.eax);
            call(0x009E_EB70, Conv::Cdecl, &[fresh, r.eax, bytes], r);
            r.ecx = args.read();
            r.eax = r.eax.wrapping_add(bytes);
        }
        r.edx = word(node, 4);
        store(r.eax as *mut u8, 0, r.edx);
        let new_end = r.eax.wrapping_add(4);
        r.eax = word(output, 0);
        if r.eax != 0 {
            call(0x0090_CF63, Conv::Cdecl, &[r.eax], r);
            r.ecx = args.read();
        }
        r.eax = fresh.wrapping_add(r.ecx.wrapping_mul(4));
        store(output, 0, fresh);
        store(output, 4, new_end);
        store(output, 8, r.eax);
    }
}

frame_entry!(
    0x0071_8F90,
    fc4p_00718f90,
    "cSC4TrafficSimulator::GetSubnetworksInRegion",
    query,
    8
);
unsafe extern "fastcall" fn query(this: *mut u8, out: *mut Registers, args: *mut u32) {
    unsafe {
        let sim = RegionSim::from_ptr(this);
        let mut set = [0u32; 5];
        let mut r = out.read();
        r.ecx = set.as_mut_ptr() as u32;
        call(0x0082_9BC0, Conv::Thiscall, &[100], &mut r);
        let region = Region::from_ptr(args.read() as *mut u8);
        let mut x = region.min_x();
        args.write(x as u32);
        r.edx = x as u32;
        if x <= region.max_x() {
            let mut row_offset = x.wrapping_mul(12);
            loop {
                let mut z = region.min_z();
                if z <= region.max_z() {
                    loop {
                        let dx = x.wrapping_sub(region.min_x()) as u32;
                        let dz = z.wrapping_sub(region.min_z()) as u32;
                        if dx < region.width() && dz < region.height() {
                            let bits = region.rows().wrapping_add(dx as usize).read_unaligned() as *mut u32;
                            let selected =
                                bits.wrapping_add((dz >> 5) as usize).read_unaligned() & 1u32.wrapping_shl(dz) != 0;
                            if selected && x >= sim.min_x() && z >= sim.min_z() && x <= sim.max_x() && z <= sim.max_z()
                            {
                                let row = word(sim.rows(), row_offset as usize) as *mut u8;
                                let mut info = word(row, z.wrapping_mul(4) as usize) as *mut u8;
                                while !info.is_null() && info.add(6).read() != 1 {
                                    info = word(info, 0x14) as *mut u8;
                                }
                                if !info.is_null() {
                                    for side in 0..4 {
                                        let key = u32::from(info.add(0x0C + side * 2).cast::<u16>().read_unaligned());
                                        if key != 0 {
                                            r.edx = set[4].wrapping_add(1);
                                            r.ecx = set.as_mut_ptr() as u32;
                                            call(0x004F_0940, Conv::Thiscall, &[r.edx], &mut r);
                                            let mut result = [0u32; 3];
                                            r.ecx = set.as_mut_ptr() as u32;
                                            r.eax = &key as *const u32 as u32;
                                            call(
                                                0x0062_4DD0,
                                                Conv::Thiscall,
                                                &[result.as_mut_ptr() as u32, &key as *const u32 as u32],
                                                &mut r,
                                            );
                                            x = args.read() as i32;
                                            r.edx = x as u32;
                                        }
                                    }
                                }
                            }
                        }
                        z = z.wrapping_add(1);
                        if z > region.max_z() {
                            break;
                        }
                    }
                }
                x = x.wrapping_add(1);
                row_offset = row_offset.wrapping_add(12);
                args.write(x as u32);
                r.edx = x as u32;
                if x > region.max_x() {
                    break;
                }
            }
        }
        let output = args.add(1).read() as *mut u8;
        r.ecx = word(output, 4);
        r.eax = word(output, 0);
        store(output, 4, r.eax);
        let buckets = set[1] as *mut u32;
        let count = (set[2].wrapping_sub(set[1]) as i32 >> 2) as u32;
        let mut index = 0u32;
        let mut node = core::ptr::null_mut();
        r.eax = count;
        r.ecx = 0;
        while index < count {
            let candidate = buckets.wrapping_add(index as usize).read_unaligned() as *mut u8;
            if !candidate.is_null() {
                node = candidate;
                break;
            }
            index = index.wrapping_add(1);
        }
        while !node.is_null() {
            append(output, node, args, &mut r);
            r.eax = word(node, 0);
            if r.eax == 0 {
                r.ecx = (set[2].wrapping_sub(set[1]) as i32 >> 2) as u32;
                let key = word(node, 4);
                r.edx = unsigned_division(key, r.ecx).1;
                r.eax = 0;
                loop {
                    r.edx = r.edx.wrapping_add(1);
                    if r.edx >= r.ecx {
                        break;
                    }
                    r.eax = (set[1] as *mut u32).wrapping_add(r.edx as usize).read_unaligned();
                    if r.eax != 0 {
                        break;
                    }
                }
            }
            node = r.eax as *mut u8;
        }
        r.ecx = set.as_mut_ptr() as u32;
        call(0x008E_BA60, Conv::Thiscall, &[], &mut r);
        r.eax = set[1];
        if r.eax != 0 {
            call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
        }
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[fc4p_00718f90::PORT];

#[cfg(test)]
mod tests;
