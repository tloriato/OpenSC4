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

//! Network tool helpers, preview lifetime, and rule records on the game's own objects.

use crate::port::Port;

// These views intentionally describe only fields used by the completed ports.
layout! {
    /// Primary `cISC4NetworkTool` interface.
    pub struct NetworkTool {
        0x00 => vtable / set_vtable: u32,
        0x18 => initialized / set_initialized: u8,
        0x44 => terrain / set_terrain: *mut u8,
        0x48 => occupant_manager / set_occupant_manager: *mut u8,
        0x50 => real / set_real: u8,
        0x54 => steps / set_steps: *mut u8,
        0x60 => cells / set_cells: *mut u8,
        0x74 => straight_steps / set_straight_steps: i32,
        0x78 => valid_steps / set_valid_steps: i32,
        0x7C => pen / set_pen: u8,
        0x98 => solved_begin / set_solved_begin: *mut u8,
        0x9C => solved_end / set_solved_end: *mut u8,
        0xCC => support_begin / set_support_begin: *mut u8,
        0xD0 => support_end / set_support_end: *mut u8,
        0xD8 => bridge_begin / set_bridge_begin: *mut u8,
        0xDC => bridge_end / set_bridge_end: *mut u8,
        0xE4 => tunnel_begin / set_tunnel_begin: *mut u8,
        0xE8 => tunnel_end / set_tunnel_end: *mut u8,
        0x1F0 => preview / set_preview: *mut u8,
        0x208 => insertion_data / set_insertion_data: u32,
        0x220 => fixed_cost / set_fixed_cost: u32,
        0x228 => retained_header / set_retained_header: *mut u8,
        0x22C => retained_count / set_retained_count: u32,
        0x240 => width / set_width: u32,
        0x244 => height / set_height: u32,
        0x248 => vertex_stride / set_vertex_stride: u32,
        0x2C4 => tolerance / set_tolerance: f32,
        0x2C8 => elevation_threshold / set_elevation_threshold: f32,
        0x320 => bridge_unit_extra / set_bridge_unit_extra: u32,
        0x324 => tunnel_unit_cost / set_tunnel_unit_cost: u32,
        0x328 => terrain_unit_cost / set_terrain_unit_cost: f32,
        0x32C => support_id / set_support_id: u32,
        0x358 => type_info / set_type_info: *mut u8,
        0x35C => network_type / set_network_type: u32,
    }
}

unsafe fn read<T: Copy>(p: *const u8, offset: usize) -> T {
    // SAFETY: callers provide live game storage of the requested width.
    unsafe { p.add(offset).cast::<T>().read_unaligned() }
}

unsafe fn write<T>(p: *mut u8, offset: usize, value: T) {
    // SAFETY: callers provide writable game storage of the requested width.
    unsafe { p.add(offset).cast::<T>().write_unaligned(value) }
}

fn registers(eax: u32, edx: u32) -> u64 {
    u64::from(eax) | (u64::from(edx) << 32)
}

// Pass the entry registers and original argument words to Rust. The body has no stack arguments of its
// own; its u64 return supplies EDX:EAX. All decisions and object accesses remain in the Rust body.
macro_rules! register_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $ret:literal) => {
        port! {
            #[port(address = $address, owner = "networktool", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push eax", "push edx", "mov edx, esp", "call {body}", "add esp, 8", $ret,
                    body = sym $body,
                )
            }
        }
    };
}

mod clear;
mod control;
mod cost;
mod drag_placement;
mod geometry;
mod highlights;
mod helpers;
mod init;
mod lifetime;
mod occupants;
mod placement;
mod preview;
mod records;
mod reload;
mod rules;
mod storage;
mod tunnels;
mod type_reload;
mod tunnel_piece;
mod tunnel_insertion;
mod solution_records;
mod refresh;
mod solved_insertion;
mod occupant_insertion;
mod intersection_storage;
mod construction;
mod bridge_helpers;
mod bridge_steps;
mod suspension;

const COUNT: usize = helpers::PORTS.len()
    + preview::PORTS.len()
    + rules::PORTS.len()
    + geometry::PORTS.len()
    + highlights::PORTS.len()
    + occupants::PORTS.len()
    + storage::PORTS.len()
    + cost::PORTS.len()
    + drag_placement::PORTS.len()
    + placement::PORTS.len()
    + records::PORTS.len()
    + control::PORTS.len()
    + lifetime::PORTS.len()
    + reload::PORTS.len()
    + init::PORTS.len()
    + clear::PORTS.len()
    + tunnels::PORTS.len()
    + type_reload::PORTS.len()
    + tunnel_piece::PORTS.len()
    + tunnel_insertion::PORTS.len()
    + solution_records::PORTS.len()
    + refresh::PORTS.len()
    + solved_insertion::PORTS.len()
    + occupant_insertion::PORTS.len()
    + intersection_storage::PORTS.len()
    + construction::PORTS.len()
    + bridge_helpers::PORTS.len()
    + bridge_steps::PORTS.len()
    + suspension::PORTS.len();
/// Completed replacements, ordered by original address within each group.
pub const PORTS: &[Port] = &crate::port::concat::<COUNT>(&[
    helpers::PORTS,
    preview::PORTS,
    rules::PORTS,
    geometry::PORTS,
    highlights::PORTS,
    occupants::PORTS,
    storage::PORTS,
    cost::PORTS,
    drag_placement::PORTS,
    placement::PORTS,
    records::PORTS,
    control::PORTS,
    lifetime::PORTS,
    reload::PORTS,
    init::PORTS,
    clear::PORTS,
    tunnels::PORTS,
    type_reload::PORTS,
    tunnel_piece::PORTS,
    tunnel_insertion::PORTS,
    solution_records::PORTS,
    refresh::PORTS,
    solved_insertion::PORTS,
    occupant_insertion::PORTS,
    intersection_storage::PORTS,
    construction::PORTS,
    bridge_helpers::PORTS,
    bridge_steps::PORTS,
    suspension::PORTS,
]);
