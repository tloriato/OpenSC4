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

//! Power simulator methods on the game's original objects.

use crate::port::Port;
use crate::x87::F80;

layout! {
    /// Offsets from the primary power simulator interface.
    pub struct PowerSim {
        0x00 => primary_vtable / set_primary_vtable: *const usize,
        0x10 => references / set_references: i32,
        0x18 => city / set_city: *mut u8,
        0x1C => budget / set_budget: *mut u8,
        0x20 => department / set_department: *mut u8,
        0x2C => zones / set_zones: *mut u8,
        0x38 => ring_index / set_ring_index: i32,
        0x3C => production / set_production: i32,
        0x40 => consumption / set_consumption: i32,
        0x60 => monthly_demand / set_monthly_demand: u32,
        0x64 => deficit / set_deficit: u32,
        0x78 => infrastructure_cost / set_infrastructure_cost: u64,
        0x90 => usage_ring / set_usage_ring: [i32; 120],
        0x270 => annual_usage / set_annual_usage: [i32; 10],
        0x298 => plant_header / set_plant_header: *mut u8,
        0x29C => poles_begin / set_poles_begin: *mut *mut u8,
        0x2A0 => poles_end / set_poles_end: *mut *mut u8,
        0x2AC => ordinance_factor / set_ordinance_factor: f32,
        0x2B0 => radius / set_radius: u32,
        0x2B8 => pole_count / set_pole_count: u32,
        0x2BC => broken_count / set_broken_count: u32,
        0x2C0 => broken_fraction / set_broken_fraction: f32,
        0x2C8 => wire / set_wire: *mut u32,
        0x2D4 => powered / set_powered: *mut u32,
        0x338 => recompute / set_recompute: u8,
        0x339 => age_plants / set_age_plants: u8,
        0x378 => funding_curve / set_funding_curve: [u32; 4],
    }
}

/// Inclusive cell bounds, stored as four consecutive words.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x0: u32,
    pub z0: u32,
    pub x1: u32,
    pub z1: u32,
}

// Keep the stored operand in memory: loading it first changes signaling NaN payload selection.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            // SAFETY: both pointers cover the operands; the x87 push and pop balance.
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) &value, operand = in(reg) &operand,
                    output = in(reg) &mut output, out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_op!(multiply_float, "fmul dword ptr [{operand}]");
stored_op!(divide_into_float, "fdivr dword ptr [{operand}]");
stored_op!(add_float, "fadd dword ptr [{operand}]");

pub mod bitmap;
pub mod com;
pub mod grid;
pub mod monthly;
pub mod plants;

const COUNT: usize =
    bitmap::PORTS.len() + com::PORTS.len() + grid::PORTS.len() + plants::PORTS.len() + monthly::PORTS.len();
const ALL: [Port; COUNT] =
    crate::port::concat(&[bitmap::PORTS, com::PORTS, grid::PORTS, plants::PORTS, monthly::PORTS]);
/// This module's complete replacements.
pub const PORTS: &[Port] = &ALL;
