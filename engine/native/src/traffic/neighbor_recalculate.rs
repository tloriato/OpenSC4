// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Regional job allocation and border destination capacities.

use super::neighbor_add::{ConnectionRecord, PoolNode};
use super::neighbor_roll::write_capacity;
use super::*;
use core::mem::MaybeUninit;

layout! {
    /// Capacity recalculation fields relative to the traffic simulator's primary interface.
    pub struct CapacitySim {
        0x4C => destination_rows / set_destination_rows: *mut u8,
        0xB8 => pools / set_pools: *mut u8,
        0xC4 => default_pool / set_default_pool: *mut u8,
        0x194 => edge_length / set_edge_length: i32,
        0x198 => south_edge / set_south_edge: i32,
    }
}
game_data! { static allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static demand_manager: *mut u8 = 0x00B4_3D74; }
game_data! { static neighbors: *mut u8 = 0x00B4_3D8C; }
game_data! { static unsigned_correction: f64 = 0x00A8_0AB8; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_temporary(this: *mut *mut u8) -> u32 = "thiscall" 0x0041_1870; }
game_fn! { fn get_integer(holder: *mut u8, id: u32, out: *mut i32) -> u32 = "C" 0x005F_D450; }
game_fn! { fn has_map_exit(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0070_FA40; }
game_fn! { fn clear_list(this: *mut u32) -> u32 = "thiscall" 0x0041_1DF0; }
game_fn! { #[allow(dead_code)] fn set_capacity(index: u32) -> u32 = "C" 0x0070_DE70; }

unsafe fn acquire_allocator() -> *mut u8 {
    // SAFETY: the cache stores the game's retained allocator interface.
    unsafe {
        if (*allocator()).is_null() {
            let mut temporary = core::ptr::null_mut();
            let fw = framework();
            if !fw.is_null() {
                vcall!(fw, 0x14 => fn(u32, u32, *mut *mut u8) -> u32,
                    0x3AE4_BEA3, 0x3AE4_BEAB, &mut temporary);
            }
            allocator().write(temporary);
            release_temporary(&mut temporary);
        }
        *allocator()
    }
}

unsafe fn unsigned_integer(value: u32) -> F80 {
    let signed = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        return signed;
    }
    let mut result = F80::ZERO;
    // SAFETY: the original double constant is added as a memory operand; one x87 value is pushed and popped.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{signed}]", "fadd qword ptr [{correction}]", "fstp tbyte ptr [{result}]",
        signed = in(reg) &signed, correction = in(reg) unsigned_correction(), result = in(reg) &mut result,
        out("st(0)") _);
    }
    result
}

/// Advances an ordinary pool iterator through its right subtree or parent chain.
unsafe fn successor(mut node: *mut u8) -> *mut u8 {
    unsafe {
        let right = PoolNode::from_ptr(node).right();
        if !right.is_null() {
            node = right;
            while !PoolNode::from_ptr(node).left().is_null() {
                node = PoolNode::from_ptr(node).left();
            }
            return node;
        }
        let mut parent = PoolNode::from_ptr(node).parent();
        while node == PoolNode::from_ptr(parent).right() {
            node = parent;
            parent = PoolNode::from_ptr(parent).parent();
        }
        if PoolNode::from_ptr(node).right() != parent {
            parent
        } else {
            node
        }
    }
}

unsafe fn pool_for_key(header: *mut u8, key: u16) -> *mut u8 {
    unsafe {
        let mut node = PoolNode::from_ptr(header).parent();
        let mut lower = header;
        while !node.is_null() {
            let n = PoolNode::from_ptr(node);
            if n.key() < key {
                node = n.right();
            } else {
                lower = node;
                node = n.left();
            }
        }
        if lower != header && key < PoolNode::from_ptr(lower).key() {
            header
        } else {
            lower
        }
    }
}

port! {
    /// Shares each regional job total among connected cities by their signed populations and wrapping
    /// unsigned population totals. Resets arrivals, then marks usable border destinations and counts them.
    /// Missing pool keys become zero. Repeated calls count destinations again even if their bits were set.
    #[port(address = 0x0071_4E60, owner = "traffic", name = "cSC4TrafficSimulator::RecalculateNeighborConnectionCapacities", naked)]
    pub unsafe extern "C" fn fc4p_00714e60() {
        core::arch::naked_asm!("sub esp, 12", "mov edx, esp", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret", sym recalculate);
    }
}

unsafe extern "fastcall" fn recalculate(this: *mut u8, out: *mut Registers) {
    // SAFETY: the game supplies its simulator, services, list nodes, pool tree and border arrays.
    unsafe {
        let sim = CapacitySim::from_ptr(this);
        let alloc = acquire_allocator();
        let sentinel = vcall!(alloc, 0x0C => fn(u32) -> *mut u8, 12);
        store(sentinel, 0, sentinel as u32);
        store(sentinel, 4, sentinel as u32);
        let mut list = [sentinel as u32, 0];
        let dm = *demand_manager();
        let mut jobs = [0u32; 3];
        for (w, job) in jobs.iter_mut().enumerate() {
            let demand = vcall!(dm, 0x18 => fn(u32, u32) -> *mut u8, 0x2010 + w as u32 * 0x10, 0x10000);
            *job = vcall_st0!(demand, 0x24).ftol() as u32;
        }
        let manager = *neighbors();
        let list_result = virtual_registers(
            manager,
            0x54,
            &[list.as_mut_ptr() as u32, 0],
            word(manager, 0),
            list.as_mut_ptr() as u32,
        );
        let head = list[0] as *mut u8;
        let mut node = word(head, 0) as *mut u8;
        let mut totals = [0u32; 3];
        while node != head {
            let city = word(node, 8) as *mut u8;
            for (w, total) in totals.iter_mut().enumerate() {
                *total = total.wrapping_add(vcall!(city, 0x44 => fn(u32) -> u32, 0x2010 + w as u32 * 0x10));
            }
            node = word(node, 0) as *mut u8;
        }
        node = word(list[0] as *mut u8, 0) as *mut u8;
        let mut last_city_node = list_result.ecx;
        while node != head {
            let city = word(node, 8) as *mut u8;
            let (mut x, mut z) = (MaybeUninit::<i32>::uninit(), MaybeUninit::<i32>::uninit());
            vcall!(city, 0x14 => fn(*mut i32, *mut i32) -> u32, x.as_mut_ptr(), z.as_mut_ptr());
            let header = sim.pools();
            let mut pool = PoolNode::from_ptr(header).left();
            while pool != header {
                let holder = PoolNode::from_ptr(pool).holder();
                let (mut px, mut pz) = (MaybeUninit::<i32>::uninit(), MaybeUninit::<i32>::uninit());
                if get_integer(holder, 0x4A23_576B, px.as_mut_ptr()) as u8 != 0
                    && get_integer(holder, 0x4A23_576C, pz.as_mut_ptr()) as u8 != 0
                    && px.assume_init() == x.assume_init_read()
                    && pz.assume_init() == z.assume_init_read()
                {
                    if !holder.is_null() {
                        for w in 0..3 {
                            let population = vcall!(city, 0x44 => fn(u32) -> i32, 0x2010 + w as u32 * 0x10);
                            let ratio = F80::from_i32(population) / unsigned_integer(totals[w]);
                            let capacity = (ratio * unsigned_integer(jobs[w])).ftol() as u32;
                            let result = virtual_registers(
                                this,
                                0x4C,
                                &[holder as u32, w as u32, capacity],
                                capacity,
                                word(this, 0),
                            );
                            write_capacity(holder, w as u32 + 4, 0, result);
                        }
                        let result = virtual_registers(this, 0x4C, &[holder as u32, 3, u32::MAX], word(this, 0), 0);
                        write_capacity(holder, 7, 0, result);
                    }
                    break;
                }
                pool = successor(pool);
            }
            last_city_node = node as u32;
            node = word(node, 0) as *mut u8;
        }
        write_capacity(
            sim.default_pool(),
            7,
            0,
            Registers {
                eax: head as u32,
                ecx: last_city_node,
                edx: 0,
            },
        );
        for side in 0usize..4 {
            let vector = this.add(0x88 + side * 12);
            let mut index = 0i32;
            while index < sim.edge_length() {
                let records = word(vector, 0) as *mut u8;
                let record = ConnectionRecord::from_ptr(records.wrapping_offset(index.wrapping_mul(8) as isize));
                if record.mask() != 0 {
                    let (x, z) = match side {
                        0 => (-1, index),
                        1 => (index, -1),
                        2 => (sim.edge_length().wrapping_add(1), index),
                        _ => (index, sim.south_edge().wrapping_add(1)),
                    };
                    if has_map_exit(this, x, z) as u8 != 0 {
                        let holder = if record.pool() == 0 {
                            sim.default_pool()
                        } else {
                            let header = sim.pools();
                            let pool = pool_for_key(header, record.pool());
                            if pool == sim.pools() {
                                record.set_pool(0);
                                sim.default_pool()
                            } else {
                                PoolNode::from_ptr(pool).holder()
                            }
                        };
                        for kind in 0u32..4 {
                            if vcall!(this, 0x50 => fn(*mut u8, u32) -> u32, holder, kind) != 0 {
                                let row_index = x.wrapping_mul(3).wrapping_add(3).wrapping_mul(4);
                                let row =
                                    word(sim.destination_rows().wrapping_offset(row_index as isize), 0) as *mut u8;
                                let cell = row.wrapping_offset(z.wrapping_add(1) as isize);
                                cell.write(cell.read() | 1u8.wrapping_shl(kind));
                                let offset = 0x1B0 + kind as usize * 4;
                                store(this, offset, word(this, offset).wrapping_add(1));
                            }
                        }
                    }
                }
                index = index.wrapping_add(1);
            }
        }
        clear_list(list.as_mut_ptr());
        let alloc = acquire_allocator();
        out.write(virtual_registers(alloc, 0x10, &[list[0]], word(alloc, 0), 0));
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_00714e60::PORT];

#[cfg(test)]
mod tests;
