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

//! `cSC4TrafficSimulator`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Traffic simulator fields, relative to the primary interface.
    pub struct TrafficSim {
        0x10 => references / set_references: i32,
        0x18 => messages / set_messages: *mut u8,
        0x28 => max_x / set_max_x: i32,
        0x2C => max_z / set_max_z: i32,
        0x30 => pollution_map / set_pollution_map: *mut u8,
        0x48 => congestion_map / set_congestion_map: *mut u8,
        0x58 => intersections / set_intersections: *mut u8,
        0x88 => west_connections / set_west_connections: *mut u8,
        0x94 => north_connections / set_north_connections: *mut u8,
        0xA0 => east_connections / set_east_connections: *mut u8,
        0xAC => south_connections / set_south_connections: *mut u8,
        0x108 => water_paths / set_water_paths: *mut u8,
        0x18C => min_cell_x / set_min_cell_x: i32,
        0x190 => min_cell_z / set_min_cell_z: i32,
        0x194 => max_cell_x / set_max_cell_x: i32,
        0x198 => max_cell_z / set_max_cell_z: i32,
        0x414 => striking / set_striking: u8,
        0x418 => strike_probability / set_strike_probability: f32,
        0x41C => freight_byte / set_freight_byte: u8,
        0x420 => average_commute / set_average_commute: f32,
        0x6EC => commute_sum / set_commute_sum: f32,
        0x6F0 => commute_count / set_commute_count: u32,
        0x6F4 => freight_sum / set_freight_sum: f32,
        0x6F8 => freight_count / set_freight_count: u32,
        0x6FC => random / set_random: *mut u8,
        0x724 => roads / set_roads: *mut u8,
        0x728 => transit / set_transit: *mut u8,
        0x740 => network_cell_counts / set_network_cell_counts: [u32; 13],
        0x75C => subway_cells / set_subway_cells: u32,
        0x778 => budget_total / set_budget_total: i64,
        0x780 => transit_income / set_transit_income: f32,
        0x784 => toll_income / set_toll_income: f32,
        0x790 => transit_accumulator / set_transit_accumulator: f64,
        0x798 => toll_accumulator / set_toll_accumulator: f64,
        0x7A4 => strike_state / set_strike_state: u32,
        0x7A8 => strike_months / set_strike_months: u32,
        0x81C => intersection_dirty / set_intersection_dirty: u8,
        0x820 => dirty_x0 / set_dirty_x0: i32,
        0x824 => dirty_z0 / set_dirty_z0: i32,
        0x828 => dirty_x1 / set_dirty_x1: i32,
        0x82C => dirty_z1 / set_dirty_z1: i32,
        0xB54 => maintenance_prices / set_maintenance_prices: [f32; 13],
        0xB94 => strike_keep / set_strike_keep: f32,
        0xB98 => strike_start / set_strike_start: f32,
        0xB9C => strike_limit / set_strike_limit: u32,
        0xBA0 => strike_scale / set_strike_scale: f32,
        0xC54 => commute_maximum / set_commute_maximum: f32,
        0xCE8 => background_weights / set_background_weights: [f32; 3],
    }
}

/// Integer registers that some of the shared helpers expose to their callers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

/// Calls a thiscall method and captures all three volatile integer registers.
unsafe fn virtual_registers(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> Registers {
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        unsafe { crate::abi::vslot(object, slot) },
        object as u32,
        args,
    );
    call.eax = eax;
    call.edx = edx;
    // SAFETY: the object's method consumes the supplied stack words and preserves EDI.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            in("edi") &mut call, clobber_abi("C"),
        );
    }
    Registers {
        eax: call.eax,
        ecx: call.ecx,
        edx: call.edx,
    }
}

unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

unsafe fn store(p: *mut u8, offset: usize, value: u32) {
    unsafe { write_unaligned(p.wrapping_add(offset).cast(), value) }
}

// Keep the second operand in memory, including its signaling-NaN behavior.
macro_rules! stored_op {
    ($name:ident, $op:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut out = F80::ZERO;
            unsafe { core::arch::asm!("fld tbyte ptr [{value}]", $op, "fstp tbyte ptr [{out}]",
                value = in(reg) &value, operand = in(reg) &operand, out = in(reg) &mut out,
                out("st(0)") _); }
            out
        }
    };
}
stored_op!(add_float, "fadd dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");
stored_op!(divide_float, "fdiv dword ptr [{operand}]");
stored_op!(reverse_divide_float, "fdivr dword ptr [{operand}]");
stored_op!(subtract_float, "fsub dword ptr [{operand}]");

pub mod budget;
pub mod capacity;
pub mod containers;
pub mod maps;
pub mod queries;
pub mod network;
pub mod trees;
pub mod trips;
pub mod refresh;
pub mod finish_helpers;
pub mod finish_passes;
pub mod final_map;
pub mod source_totals;
pub mod trip_map;
pub mod late_containers;
pub mod late_passes;
pub mod late_boundaries;
pub mod commercial;
pub mod destination;
pub mod tunnel;
pub mod neighbor_changes;
pub mod neighbor_add;
pub mod neighbor_roll;
pub mod neighbor_recalculate;
pub mod rail;
pub mod elevated_paths;
pub mod switch_rules;
pub mod commercial_window;
pub mod recalc;
pub mod subnetworks;
pub mod subnetwork_info;
pub mod monthly_queries;
pub mod accidents;
pub mod region_query;
pub mod archive_write;
pub mod apply_route;
pub mod create_maps;
#[cfg(test)]
mod route_map_support;
#[cfg(test)]
mod test_support;

/// This module's replacements.
pub const PORTS: &[Port] = &crate::port::concat::<
    {
        apply_route::PORTS.len() + create_maps::PORTS.len() + archive_write::PORTS.len() + region_query::PORTS.len() + accidents::PORTS.len() + monthly_queries::PORTS.len() + subnetwork_info::PORTS.len() + subnetworks::PORTS.len() + recalc::PORTS.len() +
        commercial_window::PORTS.len() + switch_rules::PORTS.len() + elevated_paths::PORTS.len() + rail::PORTS.len() + neighbor_recalculate::PORTS.len() + neighbor_roll::PORTS.len() + neighbor_add::PORTS.len() + neighbor_changes::PORTS.len() + tunnel::PORTS.len() + destination::PORTS.len() + commercial::PORTS.len() + late_boundaries::PORTS.len() + late_passes::PORTS.len() + late_containers::PORTS.len() + queries::PORTS.len() + containers::PORTS.len() + budget::PORTS.len() + capacity::PORTS.len() + maps::PORTS.len()
            + network::PORTS.len() + trees::PORTS.len() + trips::PORTS.len() + refresh::PORTS.len() + finish_helpers::PORTS.len() + finish_passes::PORTS.len() + final_map::PORTS.len() + source_totals::PORTS.len() + trip_map::PORTS.len()
    },
>(&[
    apply_route::PORTS,
    create_maps::PORTS,
    archive_write::PORTS,
    region_query::PORTS,
    accidents::PORTS,
    monthly_queries::PORTS,
    subnetwork_info::PORTS,
    subnetworks::PORTS,
    recalc::PORTS,
    late_containers::PORTS,
    late_passes::PORTS,
    late_boundaries::PORTS,
    commercial::PORTS,
    destination::PORTS,
    tunnel::PORTS,
    neighbor_changes::PORTS,
    neighbor_add::PORTS,
    neighbor_roll::PORTS,
    neighbor_recalculate::PORTS,
    rail::PORTS,
    elevated_paths::PORTS,
    switch_rules::PORTS,
    commercial_window::PORTS,
    queries::PORTS,
    containers::PORTS,
    budget::PORTS,
    capacity::PORTS,
    maps::PORTS,
    network::PORTS,
    trees::PORTS,
    trips::PORTS,
    refresh::PORTS,
    finish_helpers::PORTS,
    finish_passes::PORTS,
    final_map::PORTS,
    source_totals::PORTS,
    trip_map::PORTS,
]);
