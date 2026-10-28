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

//! Building development and its shared object and container helpers.

use crate::port::Port;

layout! {
    /// Building development simulator, relative to its primary interface.
    pub struct BuildingSim {
        0x00 => vtable / set_vtable: *const usize,
        0x14 => references / set_references: i32,
        0x1C => city / set_city: *mut u8,
        0x20 => occupants / set_occupants: *mut u8,
        0x28 => zones / set_zones: *mut u8,
        0x2C => lots / set_lots: *mut u8,
        0x30 => land_value / set_land_value: *mut u8,
        0x34 => power / set_power: *mut u8,
        0x40 => cells_x / set_cells_x: i32,
        0x44 => cells_z / set_cells_z: i32,
        0x5C => developers / set_developers: *mut u8,
        0x70 => construction / set_construction: *mut u8,
        0x74 => state / set_state: u32,
        0x78 => developer_iterator / set_developer_iterator: *mut u8,
        0xB0 => last_build_days / set_last_build_days: *mut u8,
        0xC0 => resource_min / set_resource_min: u32,
        0xC4 => resource_max / set_resource_max: u32,
        0xC8 => resource_fraction / set_resource_fraction: f32,
        0xD0 => population_multipliers / set_population_multipliers: *const f32,
        0xDC => use_growth_plan / set_use_growth_plan: u8,
        0xDE => ignore_power / set_ignore_power: u8,
        0xDF => ignore_water / set_ignore_water: u8,
        0xE1 => bar_stuck / set_bar_stuck: u8,
    }
}

pub mod checks;
pub mod containers;
pub mod resources;
pub mod shared;
pub mod access;

const COUNT: usize = checks::PORTS.len() + containers::PORTS.len() + resources::PORTS.len() + shared::PORTS.len() + access::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[checks::PORTS, containers::PORTS, resources::PORTS, shared::PORTS, access::PORTS]);

/// This module's complete replacements.
pub const PORTS: &[Port] = &ALL;
