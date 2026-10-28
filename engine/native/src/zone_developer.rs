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

//! `cSC4ZoneDeveloper`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::port::Port;

layout! {
    /// The zone developer's primary interface and the fields used by these ports.
    pub struct ZoneDeveloper {
        0x00 => primary_vtable / set_primary_vtable: u32,
        0x04 => lot_manager_vtable / set_lot_manager_vtable: u32,
        0x08 => message_vtable / set_message_vtable: u32,
        0x0C => serializable_vtable / set_serializable_vtable: u32,
        0x14 => references / set_references: i32,
        0x28 => cells_x / set_cells_x: i32,
        0x2C => cells_z / set_cells_z: i32,
        0x34 => terrain / set_terrain: *mut u8,
        0x38 => renderer / set_renderer: *mut u8,
        0x40 => zone_manager / set_zone_manager: *mut u8,
        0x48 => street_tool / set_street_tool: *mut u8,
        0x4C => road_tool / set_road_tool: *mut u8,
        0x99 => real_mode / set_real_mode: u8,
        0xA0 => current_zone / set_current_zone: u32,
        0xA4 => restriction / set_restriction: *mut u8,
        0xD8 => virtual_lot_header / set_virtual_lot_header: *mut u8,
        0xDC => virtual_lot_count / set_virtual_lot_count: u32,
        0xE4 => deleted_lot_header / set_deleted_lot_header: *mut u8,
        0xE8 => deleted_lot_count / set_deleted_lot_count: u32,
    }
}

mod altitude;
mod basic;
mod edges;
mod lot;
mod manager;
mod occupancy;

/// Call a virtual method with its incoming scratch-register values and retain EDX:EAX.
unsafe fn virtual_registers(this: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> u64 {
    unsafe {
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(this, slot),
            this as u32,
            args,
        );
        call.eax = eax;
        call.edx = edx;
        crate::abi::call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &crate::port::concat::<
    {
        basic::PORTS.len()
            + lot::PORTS.len()
            + edges::PORTS.len()
            + manager::PORTS.len()
            + occupancy::PORTS.len()
            + altitude::PORTS.len()
    },
>(&[
    basic::PORTS,
    lot::PORTS,
    edges::PORTS,
    manager::PORTS,
    occupancy::PORTS,
    altitude::PORTS,
]);
