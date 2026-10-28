// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Pollution values, garbage bookkeeping and the simulator's public interfaces.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// `cSC4PollutionSimulator`, relative to the primary pollution interface.
    pub struct PollutionSim {
        0x00 => primary_vtable / set_primary_vtable: u32,
        0x14 => references / set_references: i32,
        0x24 => zones / set_zones: *mut u8,
        0x34 => occupant_manager / set_occupant_manager: *mut u8,
        0x40 => air_source / set_air_source: *mut u8,
        0x44 => water_source / set_water_source: *mut u8,
        0x48 => garbage_source / set_garbage_source: *mut u8,
        0x4C => radiation_source / set_radiation_source: *mut u8,
        0x50 => air / set_air: *mut u8,
        0x54 => water / set_water: *mut u8,
        0x58 => garbage / set_garbage: *mut u8,
        0x5C => radiation / set_radiation: *mut u8,
        0x68 => treatment_list / set_treatment_list: *mut u8,
        0x6C => incinerator_list / set_incinerator_list: *mut u8,
        0x70 => energy_list / set_energy_list: *mut u8,
        0x74 => recycling_list / set_recycling_list: *mut u8,
        0x104 => average_water / set_average_water: i32,
        0x128 => average_garbage / set_average_garbage: u16,
        0x134 => recycling_effect / set_recycling_effect: f32,
        0x13C => uncollected / set_uncollected: u32,
        0x170 => landfilled / set_landfilled: i64,
        0x178 => produced_previous / set_produced_previous: i64,
        0x190 => recycled_previous / set_recycled_previous: f64,
        0x1A0 => energy_previous / set_energy_previous: i64,
        0x1A8 => landfilled_previous / set_landfilled_previous: i64,
        0x1B8 => abducted_previous / set_abducted_previous: u32,
        0x1BC => active_landfill_cells / set_active_landfill_cells: u32,
        0x209 => dirty_air / set_dirty_air: u8,
        0x20A => dirty_water / set_dirty_water: u8,
        0x20B => dirty_garbage / set_dirty_garbage: u8,
        0x210 => worst_air_x / set_worst_air_x: u16,
        0x212 => worst_air_z / set_worst_air_z: u16,
        0x214 => worst_air_value / set_worst_air_value: u32,
        0x218 => worst_water_x / set_worst_water_x: u16,
        0x21A => worst_water_z / set_worst_water_z: u16,
        0x21C => worst_water_value / set_worst_water_value: u32,
        0x220 => worst_garbage_x / set_worst_garbage_x: u16,
        0x222 => worst_garbage_z / set_worst_garbage_z: u16,
        0x224 => worst_garbage_value / set_worst_garbage_value: u32,
        0x22C => supply / set_supply: i32,
        0x230 => capacity / set_capacity: i32,
    }
}

layout! {
    /// The circular list sentinel and nodes; the third word holds the occupant pointer.
    struct ListNode {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => occupant / set_occupant: *mut u8,
    }
}

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

/// Supplies the volatile registers that the original call establishes, and retains both result words.
unsafe fn virtual_call(object: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> u64 {
    // SAFETY: each caller supplies a live game object and the slot's original stack arguments.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}

unsafe fn table(object: *mut u8) -> u32 {
    // SAFETY: callers supply live game objects, whose first word is their table address.
    unsafe { read_unaligned(object.cast()) }
}

pub mod access;
pub mod aging;
pub mod io;
pub mod maintenance;
pub mod release;
pub mod recycling;
pub mod capacity;
pub mod com;
pub mod garbage;
pub mod grids;
pub mod lists;
pub mod request;

const COUNT: usize = access::PORTS.len()
    + aging::PORTS.len()
    + io::PORTS.len()
    + maintenance::PORTS.len()
    + release::PORTS.len()
    + recycling::PORTS.len()
    + com::PORTS.len()
    + garbage::PORTS.len()
    + grids::PORTS.len()
    + request::PORTS.len()
    + lists::PORTS.len()
    + capacity::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[
    access::PORTS,
    aging::PORTS,
    io::PORTS,
    maintenance::PORTS,
    release::PORTS,
    recycling::PORTS,
    com::PORTS,
    garbage::PORTS,
    grids::PORTS,
    request::PORTS,
    lists::PORTS,
    capacity::PORTS,
]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;

#[cfg(test)]
mod tests;
