// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Connection masks, trip costs and regional destination pools.

use super::neighbor_changes::NeighborSim;
use super::*;
use core::mem::MaybeUninit;

game_data! { static neighbors: *mut u8 = 0x00B4_3D8C; }
game_fn! { fn connection_record(this: *mut u8, x: i32, z: i32) -> *mut u8 = "thiscall" 0x0070_F970; }
game_fn! { fn get_float(holder: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D4F0; }
game_fn! { fn get_integer(holder: *mut u8, id: u32, out: *mut i32) -> u32 = "C" 0x005F_D450; }
game_fn! { fn next_pool(iterator: *mut *mut u8) -> u32 = "thiscall" 0x005D_17C0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn construct_holder(this: *mut u8) -> *mut u8 = "thiscall" 0x005F_F100; }
game_fn! { fn set_integer(holder: *mut u8, id: u32, value: i32) -> u32 = "C" 0x005F_E330; }
game_fn! { fn insert_pool(this: *mut u8, key: *const u32) -> *mut *mut u8 = "thiscall" 0x0071_4560; }

layout! {
    /// The connection's eight-byte border record.
    pub struct ConnectionRecord {
        0x00 => mask / set_mask: u16,
        0x02 => pool / set_pool: u16,
        0x04 => cost_bits / set_cost_bits: u32,
    }
}
layout! {
    /// Links and value in the sixteen-bit-key pool tree.
    pub struct PoolNode {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: u16,
        0x14 => holder / set_holder: *mut u8,
    }
}

fn stored_less(a: u32, b: u32) -> bool {
    let status: u16;
    // SAFETY: one x87 register is loaded and popped, using the stored operand for the comparison.
    unsafe {
        core::arch::asm!("fld dword ptr [{a}]", "fcomp dword ptr [{b}]", "fnstsw ax",
        a = in(reg) &a, b = in(reg) &b, out("ax") status, out("st(0)") _);
    }
    (status >> 8 & 5).count_ones() & 1 != 0
}

port! {
    /// Adds a connection's type bit and raises its cost only when the stored comparison is less.
    /// An established neighboring city reuses its matching pool or allocates the first free nonzero key.
    /// The argument words retain the cost, city position, iterator or newly chosen key written during work.
    #[port(address = 0x0071_53C0, owner = "traffic", name = "cSC4TrafficSimulator::AddNeighborConnection", naked)]
    pub unsafe extern "C" fn fc4p_007153c0() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 12", sym add_neighbor);
    }
}

unsafe extern "fastcall" fn add_neighbor(this: *mut u8, stack: *mut u32) -> u32 {
    // SAFETY: the entry supplies the live simulator, its services and three writable argument words.
    unsafe {
        let sim = NeighborSim::from_ptr(this);
        let kind = stack.add(1).read();
        let x = stack.add(2).read() as i32;
        let z = stack.add(3).read() as i32;
        let connection = vcall!(*neighbors(), 0x2C => fn(u32, i32, i32) -> *mut u8, kind, x, z);
        if connection.is_null() {
            return 0;
        }
        let record = connection_record(this, x, z);
        if record.is_null() {
            return 0;
        }
        let record = ConnectionRecord::from_ptr(record);
        record.set_mask(record.mask() | 1u32.wrapping_shl(kind) as u16);
        stack.add(2).write(sim.commute_limit().to_bits());
        let other = vcall!(*neighbors(), 0x34 => fn(*mut u8) -> *mut u8, connection);
        if !other.is_null() {
            let holder = vcall!(other, 0x0C => fn() -> *mut u8);
            get_float(holder, 0xCA94_7077, stack.add(2));
        }
        let candidate = stack.add(2).read();
        if stored_less(record.cost_bits(), candidate) {
            record.set_cost_bits(candidate);
        }
        let city = vcall!(connection, 0x24 => fn() -> *mut u8);
        if city.is_null() {
            record.set_pool(0);
            return 0;
        }
        let established = vcall!(city, 0xAC => fn() -> u32);
        if established as u8 == 0 {
            record.set_pool(0);
            return established;
        }
        let mut city_z = MaybeUninit::<i32>::uninit();
        vcall!(city, 0x14 => fn(*mut u32, *mut i32) -> u32, stack.add(1), city_z.as_mut_ptr());
        let city_x = stack.add(1).read() as i32;
        let city_z = city_z.assume_init();
        let tree = this.add(0xB8);
        let header = sim.pools();
        let mut node = PoolNode::from_ptr(header).left();
        stack.add(3).write(node as u32);
        while node != header {
            let holder = PoolNode::from_ptr(node).holder();
            let (mut px, mut pz) = (MaybeUninit::<i32>::uninit(), MaybeUninit::<i32>::uninit());
            if get_integer(holder, 0x4A23_576B, px.as_mut_ptr()) as u8 != 0 {
                let eax = get_integer(holder, 0x4A23_576C, pz.as_mut_ptr());
                if eax as u8 != 0 && px.assume_init() == city_x && pz.assume_init() == city_z {
                    let key = PoolNode::from_ptr(node).key();
                    record.set_pool(key);
                    return (eax & 0xFFFF_0000) | u32::from(key);
                }
            }
            next_pool(stack.add(3).cast());
            node = stack.add(3).read() as *mut u8;
        }
        let allocation = allocate(0x14);
        let holder = if allocation.is_null() {
            allocation
        } else {
            construct_holder(allocation)
        };
        vcall!(holder, 4 => fn() -> u32);
        set_integer(holder, 0x4A23_576B, city_x);
        set_integer(holder, 0x4A23_576C, city_z);
        let header = sim.pools();
        let root = PoolNode::from_ptr(header).parent();
        let mut key = 1u32;
        stack.add(3).write(key);
        loop {
            let mut node = root;
            let mut lower = header;
            while !node.is_null() {
                let n = PoolNode::from_ptr(node);
                if n.key() < key as u16 {
                    node = n.right();
                } else {
                    lower = node;
                    node = n.left();
                }
            }
            if lower != header && (key as u16) < PoolNode::from_ptr(lower).key() {
                lower = header;
            }
            if lower == header {
                stack.add(3).write(key);
                record.set_pool(key as u16);
                let value = insert_pool(tree, stack.add(3));
                value.write_unaligned(holder);
                return value as u32;
            }
            key = key.wrapping_add(1);
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_007153c0::PORT];

#[cfg(test)]
mod tests;
