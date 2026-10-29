// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Rail occupant counts, damage sets, and repair notifications.

use super::*;
use core::mem::MaybeUninit;

layout! {
    /// Fields used by rail occupant changes, relative to the primary interface.
    pub struct RailSim {
        0x18 => server / set_server: *mut u8,
        0x318 => potholes / set_potholes: u16,
        0x744 => rail_cells / set_rail_cells: u32,
        0x7E0 => cell_count / set_cell_count: u32,
        0x7E4 => damaged_count / set_damaged_count: u32,
    }
}

game_fn! { fn grow_cells(this: *mut u8, count: u32) = "thiscall" 0x0071_0DD0; }
game_fn! { fn insert_cell(this: *mut u8, result: *mut u32, key: *const u32) = "thiscall" 0x0060_D270; }
game_fn! { fn find_cell(this: *mut u8, result: *mut u32, key: *const u32) -> *mut u32 = "thiscall" 0x0060_D170; }
game_fn! { fn erase_cell(this: *mut u8, iterator: *const u32) = "thiscall" 0x0060_D1F0; }
game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { fn update_budget(this: *mut u8) -> u64 = "thiscall" 0x0070_DA40; }

port! {
    /// Counts every rail change, excluding level crossings from the damage sets. Removing damaged rail
    /// also decrements the road pothole count and posts the occupant's own cell. Budget refresh is last.
    /// The two caller argument words are reused for x and the packed cell key.
    #[port(address = 0x0071_5790, owner = "traffic", name = "cSC4TrafficSimulator::DoRailChanged", naked)]
    pub unsafe extern "C" fn fc4p_00715790() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 8", sym rail_changed);
    }
}

unsafe extern "fastcall" fn rail_changed(this: *mut u8, stack: *mut u32) -> u64 {
    // SAFETY: the entry supplies the live simulator and two writable caller argument words.
    unsafe {
        let sim = RailSim::from_ptr(this);
        let occupant = stack.add(1).read() as *mut u8;
        vcall!(occupant, 0xC0 => fn());
        let inserted = stack.add(2).read() as u8 != 0;
        let count = sim.rail_cells();
        if inserted {
            sim.set_rail_cells(count.wrapping_add(1));
        } else if count != 0 {
            sim.set_rail_cells(count.wrapping_sub(1));
        }
        if vcall!(occupant, 0x24 => fn(u32) -> u32, 9) as u8 == 0 {
            let mut z = MaybeUninit::<u32>::uninit();
            vcall!(occupant, 0xEC => fn(*mut u32, *mut u32), stack.add(1), z.as_mut_ptr());
            let key = ((stack.add(1).read() & 0xFF) << 8) | z.assume_init_read();
            stack.add(2).write(key);
            let intact = this.add(0x7EC);
            let mut result = MaybeUninit::<[u32; 3]>::uninit();
            if inserted {
                grow_cells(intact, word(intact, 0x10).wrapping_add(1));
                insert_cell(intact, result.as_mut_ptr().cast(), stack.add(2));
                sim.set_cell_count(sim.cell_count().wrapping_add(1));
            } else {
                let found = find_cell(intact, result.as_mut_ptr().cast(), stack.add(2));
                let mut iterator = [found.read(), found.add(1).read()];
                if iterator[0] != 0 {
                    erase_cell(intact, iterator.as_ptr());
                } else {
                    let damaged = this.add(0x800);
                    let found = find_cell(damaged, result.as_mut_ptr().cast(), stack.add(2));
                    iterator = [found.read(), found.add(1).read()];
                    if iterator[0] != 0 {
                        erase_cell(damaged, iterator.as_ptr());
                        let count = sim.damaged_count().wrapping_sub(1);
                        sim.set_potholes(sim.potholes().wrapping_sub(1));
                        sim.set_damaged_count(count);
                        let allocation = allocate_message(0x2C);
                        let message = if allocation.is_null() {
                            allocation
                        } else {
                            construct_message(allocation)
                        };
                        vcall!(message, 4 => fn());
                        vcall!(message, 0x14 => fn(u32), 0x6924_7DC8);
                        vcall!(message, 0x2C => fn(u32), stack.add(1).read());
                        vcall!(message, 0x40 => fn(u32), z.assume_init_read());
                        vcall!(message, 0x54 => fn(u32), 0);
                        vcall!(sim.server(), 0x10 => fn(*mut u8, u32), message, 0);
                        vcall!(message, 8 => fn());
                    }
                }
                sim.set_cell_count(sim.cell_count().wrapping_sub(1));
            }
        }
        update_budget(this)
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_00715790::PORT];

#[cfg(test)]
mod tests;
