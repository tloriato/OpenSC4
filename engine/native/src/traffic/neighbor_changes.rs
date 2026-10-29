// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Neighbor connections and road-cell bookkeeping.

use super::*;
use core::mem::MaybeUninit;

layout! {
    /// Traffic fields used by road changes and neighbor connections.
    pub struct NeighborSim {
        0x14 => city / set_city: *mut u8,
        0x18 => server / set_server: *mut u8,
        0xB8 => pools / set_pools: *mut u8,
        0x318 => potholes / set_potholes: u16,
        0x7AC => road_count / set_road_count: u32,
        0x7B0 => damaged_count / set_damaged_count: u32,
        0xC54 => commute_limit / set_commute_limit: f32,
    }
}

game_fn! { fn update_roads_budget(this: *mut u8) -> u32 = "thiscall" 0x0070_D8A0; }
game_fn! { fn insert_cell(this: *mut u8, result: *mut u32, key: *const u32) -> u32 = "thiscall" 0x0060_E210; }
game_fn! { fn find_cell(this: *mut u8, result: *mut u32, key: *const u32) -> *mut u32 = "thiscall" 0x0060_D170; }
game_fn! { fn erase_cell(this: *mut u8, iterator: *const u32) -> u32 = "thiscall" 0x0060_D1F0; }
game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }

port! {
    /// Adjusts the network count and budget, then tracks road insertion or removal. Removing a damaged
    /// cell decrements both damage counters and posts its cell coordinates. Types 2 and 12 skip cell sets.
    /// The first argument becomes the cell x coordinate; the second becomes the packed key for cell sets.
    #[port(address = 0x0071_55D0, owner = "traffic", name = "cSC4TrafficSimulator::DoRoadChanged", naked)]
    pub unsafe extern "C" fn fc4p_007155d0() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 12", sym road_changed);
    }
}

unsafe extern "fastcall" fn road_changed(this: *mut u8, stack: *mut u32) -> u32 {
    // SAFETY: the entry supplies the live simulator and three writable argument words.
    unsafe {
        let sim = NeighborSim::from_ptr(this);
        let occupant = stack.add(1).read() as *mut u8;
        let mut position = MaybeUninit::<[u32; 3]>::uninit();
        vcall!(occupant, 0x20 => fn(*mut u32) -> u32, position.as_mut_ptr().cast());
        let position = position.assume_init();
        let mut z = MaybeUninit::<u32>::uninit();
        vcall!(sim.city(), 0x17C => fn(u32, u32, *mut u32, *mut u32) -> u32,
            position[0], position[2], stack.add(1), z.as_mut_ptr());
        let z = z.assume_init();
        let inserted = stack.add(2).read() as u8 != 0;
        let kind = stack.add(3).read();
        let count = this.wrapping_add(0x740).cast::<u32>().wrapping_add(kind as usize);
        let old = count.read_unaligned();
        if inserted {
            count.write_unaligned(old.wrapping_add(1));
        } else if old != 0 {
            count.write_unaligned(old.wrapping_sub(1));
        }
        let mut eax = update_roads_budget(this);
        if kind == 2 || kind == 12 {
            return eax;
        }
        // The complete z word participates in OR, even when its upper bits are nonzero.
        let key = (stack.add(1).read() & 0xFF).wrapping_shl(8) | z;
        stack.add(2).write(key);
        let intact = this.add(0x7B8);
        let mut result = MaybeUninit::<[u32; 3]>::uninit();
        if inserted {
            insert_cell(intact, result.as_mut_ptr().cast(), stack.add(2));
            let count = sim.road_count().wrapping_add(1);
            sim.set_road_count(count);
            return count;
        }
        let found = find_cell(intact, result.as_mut_ptr().cast(), stack.add(2));
        let mut iterator = [found.read(), found.add(1).read()];
        if iterator[0] != 0 {
            eax = erase_cell(intact, iterator.as_ptr());
        } else {
            let damaged = this.add(0x7CC);
            let found = find_cell(damaged, result.as_mut_ptr().cast(), stack.add(2));
            iterator = [found.read(), found.add(1).read()];
            if iterator[0] != 0 {
                erase_cell(damaged, iterator.as_ptr());
                sim.set_potholes(sim.potholes().wrapping_sub(1));
                sim.set_damaged_count(sim.damaged_count().wrapping_sub(1));
                let allocation = allocate_message(0x2C);
                let message = if allocation.is_null() {
                    allocation
                } else {
                    construct_message(allocation)
                };
                vcall!(message, 4 => fn() -> u32);
                vcall!(message, 0x14 => fn(u32) -> u32, 0x6924_7DC8);
                vcall!(message, 0x2C => fn(u32) -> u32, stack.add(1).read());
                vcall!(message, 0x40 => fn(u32) -> u32, z);
                vcall!(message, 0x54 => fn(u32) -> u32, 0);
                vcall!(sim.server(), 0x10 => fn(*mut u8, u32) -> u32, message, 0);
                eax = vcall!(message, 8 => fn() -> u32);
            } else {
                eax = found as u32;
            }
        }
        sim.set_road_count(sim.road_count().wrapping_sub(1));
        eax
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_007155d0::PORT];

#[cfg(test)]
mod tests;
