// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Neighbor pool pruning and boundary-destination removal.

use super::late_containers::{call, register_entry};
use super::late_passes::virtual_call;
use super::*;
use crate::abi::Conv;

layout! {
    /// Neighbor-pool tree, destination rows and per-kind available-destination counters.
    pub struct BoundarySim {
        0x4C => destination_rows / set_destination_rows: *mut u8,
        0xB8 => pool_header / set_pool_header: *mut u8,
        0xBC => pool_count / set_pool_count: u32,
        0x1B0 => free_destinations / set_free_destinations: [u32; 4],
    }
}
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn insert_unique(this: *mut u8, result: *mut u8, key: *mut u16) = "thiscall" 0x0071_09B0; }
game_fn! { #[allow(dead_code)] fn detach(node: *mut u8, root: *mut u32, first: *mut u32, last: *mut u32) -> *mut u8 = "C" 0x0040_DAF0; }
game_fn! { #[allow(dead_code)] fn erase_subtree(this: *mut u8, root: *mut u8) = "thiscall" 0x0070_EF50; }
game_fn! { #[allow(dead_code)] fn prune(this: *mut u8) = "thiscall" 0x0071_38B0; }

unsafe fn successor(mut node: *mut u8) -> *mut u8 {
    unsafe {
        let right = word(node, 12) as *mut u8;
        if !right.is_null() {
            node = right;
            while word(node, 8) != 0 {
                node = word(node, 8) as *mut u8;
            }
        } else {
            let mut parent = word(node, 4) as *mut u8;
            while node as u32 == word(parent, 12) {
                node = parent;
                parent = word(parent, 4) as *mut u8;
            }
            if word(node, 12) != parent as u32 {
                node = parent;
            }
        }
        node
    }
}

register_entry!(
    0x0071_38B0,
    fc4p_007138b0,
    "PruneUnusedTrafficNeighborPools",
    prune_pools,
    0,
    []
);
unsafe extern "fastcall" fn prune_pools(this: *mut u8, out: *mut Registers) {
    unsafe {
        let mut r = out.read();
        let sim = TrafficSim::from_ptr(this);
        let boundary = BoundarySim::from_ptr(this);
        let mut temp = [0u32; 2];
        call(0x0090_CF54, Conv::Cdecl, &[20], &mut r);
        temp[0] = r.eax;
        let header = temp[0] as *mut u8;
        header.write(0);
        store(header, 4, 0);
        store(header, 8, header as u32);
        store(header, 12, header as u32);
        for side in 0..4 {
            let mut index = 0i32;
            r.eax = sim.max_cell_x() as u32;
            while index < sim.max_cell_x() {
                let base = word(this, 0x88 + side * 12) as *mut u8;
                r.ecx = base as u32;
                let key = base
                    .wrapping_offset(index.wrapping_mul(8).wrapping_add(2) as isize)
                    .cast::<u16>();
                r.eax = key as u32;
                if key.read_unaligned() != 0 {
                    let mut result = core::mem::MaybeUninit::<[u32; 2]>::uninit();
                    r.edx = result.as_mut_ptr() as u32;
                    r.ecx = temp.as_mut_ptr() as u32;
                    call(0x0071_09B0, Conv::Thiscall, &[r.edx, key as u32], &mut r);
                }
                r.eax = sim.max_cell_x() as u32;
                index = index.wrapping_add(1);
            }
        }
        let mut node = word(boundary.pool_header(), 8) as *mut u8;
        while node != boundary.pool_header() {
            let header = temp[0] as *mut u8;
            r.eax = word(header, 4);
            r.ecx = header as u32;
            while r.eax != 0 {
                let current = r.eax as *mut u8;
                r.edx = (r.edx & !0xFFFF) | u32::from(read_unaligned(node.add(16).cast::<u16>()));
                if read_unaligned(current.add(16).cast::<u16>()) >= r.edx as u16 {
                    r.ecx = current as u32;
                    r.eax = word(current, 8);
                } else {
                    r.eax = word(current, 12);
                }
            }
            let matched =
                r.ecx != header as u32 && r.edx as u16 >= read_unaligned((r.ecx as *mut u8).add(16).cast::<u16>());
            if matched {
                node = successor(node);
                r.eax = node as u32;
            } else {
                let holder = word(node, 20) as *mut u8;
                r.eax = word(holder, 0);
                virtual_call(holder, 8, &[], &mut r);
                let next = successor(node);
                let header = boundary.pool_header();
                r.ecx = header.add(12) as u32;
                r.edx = header.add(8) as u32;
                r.eax = header.add(4) as u32;
                call(0x0040_DAF0, Conv::Cdecl, &[node as u32, r.eax, r.edx, r.ecx], &mut r);
                if r.eax != 0 {
                    call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
                }
                r.eax = boundary.pool_count().wrapping_sub(1);
                boundary.set_pool_count(r.eax);
                node = next;
            }
        }
        r.eax = temp[1];
        if r.eax != 0 {
            let mut node = word(temp[0] as *mut u8, 4) as *mut u8;
            while !node.is_null() {
                r.eax = word(node, 12);
                r.ecx = temp.as_mut_ptr() as u32;
                call(0x0070_EF50, Conv::Thiscall, &[r.eax], &mut r);
                let left = word(node, 8);
                call(0x0090_CF63, Conv::Cdecl, &[node as u32], &mut r);
                node = left as *mut u8;
            }
            let header = temp[0] as *mut u8;
            store(header, 8, header as u32);
            r.ecx = temp[0];
            store(r.ecx as *mut u8, 4, 0);
            r.eax = temp[0];
            store(r.eax as *mut u8, 12, r.eax);
            temp[1] = 0;
        }
        if temp[0] != 0 {
            call(0x0090_CF63, Conv::Cdecl, &[temp[0]], &mut r);
        }
        out.write(r);
    }
}

register_entry!(
    0x0071_3AB0,
    fc4p_00713ab0,
    "cSC4TrafficSimulator::RemoveNeighborConnection",
    remove_connection,
    12,
    [16, 12, 8]
);
unsafe extern "fastcall" fn remove_connection(this: *mut u8, out: *mut Registers, kind: u32, x: i32, z: i32) {
    unsafe {
        let mut r = out.read();
        let sim = TrafficSim::from_ptr(this);
        let (base, index) = if x < 0 {
            (sim.west_connections(), z)
        } else if x > sim.max_cell_x() {
            r.ecx = sim.east_connections() as u32;
            (r.ecx as *mut u8, z)
        } else if z < 0 {
            r.edx = sim.north_connections() as u32;
            (r.edx as *mut u8, x)
        } else {
            (sim.south_connections(), x)
        };
        r.eax = base.wrapping_offset(index.wrapping_mul(8) as isize) as u32;
        if r.eax != 0 {
            let record = r.eax as *mut u8;
            r.ecx = kind;
            r.edx = !1u32.wrapping_shl(kind);
            let mask = read_unaligned(record.cast::<u16>()) & r.edx as u16;
            write_unaligned(record.cast::<u16>(), mask);
            if mask == 0 {
                write_unaligned(record.add(2).cast::<u16>(), 0);
                r.ecx = this as u32;
                call(0x0071_38B0, Conv::Thiscall, &[], &mut r);
                let sim = BoundarySim::from_ptr(this);
                r.ecx = sim.destination_rows() as u32;
                r.eax = x.wrapping_mul(12) as u32;
                r.edx = word(r.ecx as *mut u8, r.eax.wrapping_add(12) as usize);
                r.ecx = u32::from((r.edx as *mut u8).wrapping_offset(z.wrapping_add(1) as isize).read());
                for k in 0..4 {
                    if r.ecx & (1 << k) != 0 {
                        let offset = 0x1B0 + 4 * k;
                        store(this, offset, word(this, offset).wrapping_sub(1));
                    }
                }
                r.ecx = sim.destination_rows() as u32;
                r.edx = word(r.ecx as *mut u8, r.eax.wrapping_add(12) as usize);
                (r.edx as *mut u8).wrapping_offset(z.wrapping_add(1) as isize).write(0);
            }
        }
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[fc4p_007138b0::PORT, fc4p_00713ab0::PORT];

#[cfg(test)]
mod tests;
