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

//! `cSC4PathFinder`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::port::Port;
use crate::x87::F80;

mod nodes;
mod settings;

layout! {
    /// Fields of the 0x3160-byte pathfinder, measured from its primary interface.
    pub struct Pathfinder {
        0x0C => traffic / set_traffic: *mut u8,
        0x10 => result / set_result: *mut u8,
        0x14 => weight / set_weight: f32,
        0x18 => start_x0 / set_start_x0: i32,
        0x1C => start_z0 / set_start_z0: i32,
        0x20 => start_x1 / set_start_x1: i32,
        0x24 => start_z1 / set_start_z1: i32,
        0x28 => end_x0 / set_end_x0: i32,
        0x2C => end_z0 / set_end_z0: i32,
        0x30 => end_x1 / set_end_x1: i32,
        0x34 => end_z1 / set_end_z1: i32,
        0x38 => search_x0 / set_search_x0: i32,
        0x3C => search_z0 / set_search_z0: i32,
        0x40 => search_x1 / set_search_x1: i32,
        0x44 => search_z1 / set_search_z1: i32,
        0x58 => end_x / set_end_x: i32,
        0x5C => end_z / set_end_z: i32,
        0x60 => trip_length_bits / set_trip_length_bits: u32,
        0x64 => allowed / set_allowed: u16,
        0x66 => starting / set_starting: u16,
        0x68 => density / set_density: u32,
        0x6C => arrived / set_arrived: u32,
        0x70 => flags / set_flags: u32,
        0x80 => filter / set_filter: u32,
        0x84 => destinations / set_destinations: u8,
        0x8C => reached_type / set_reached_type: u32,
        0x90 => starting_costs / set_starting_costs: *mut u8,
        0xB8 => count_x / set_count_x: u32,
        0xBC => count_z / set_count_z: u32,
        0xC0 => max_work_time / set_max_work_time: u32,
        0xC4 => suspended / set_suspended: u8,
        0x10C => distances_x / set_distances_x: *mut i32,
        0x110 => distances_z / set_distances_z: *mut i32,
        0x120 => open_count / set_open_count: u32,
        0x3148 => pages / set_pages: *mut *mut u8,
        0x3154 => next_node / set_next_node: *mut u8,
        0x3158 => page_end / set_page_end: *mut u8,
        0x315C => page_index / set_page_index: u32,
    }
}

/// This module's replacements.
pub const PORTS: &[Port] =
    &crate::port::concat::<{ settings::PORTS.len() + nodes::PORTS.len() }>(&[settings::PORTS, nodes::PORTS]);
