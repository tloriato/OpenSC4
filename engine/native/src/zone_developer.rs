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

//! `cSC4ZoneDeveloper`, converted one function at a time. Game functions and globals are declared beside the ports.

use crate::port::Port;

layout! {
    /// The zone developer's primary interface and the fields used by these ports.
    pub struct ZoneDeveloper {
        0x00 => primary_vtable / set_primary_vtable: u32,
        0x04 => lot_manager_vtable / set_lot_manager_vtable: u32,
        0x08 => message_vtable / set_message_vtable: u32,
        0x0C => serializable_vtable / set_serializable_vtable: u32,
        0x14 => references / set_references: i32,
        0x18 => message_server / set_message_server: *mut u8,
        0x24 => city / set_city: *mut u8,
        0x28 => cells_x / set_cells_x: i32,
        0x2C => cells_z / set_cells_z: i32,
        0x30 => occupant_manager / set_occupant_manager: *mut u8,
        0x34 => terrain / set_terrain: *mut u8,
        0x38 => renderer / set_renderer: *mut u8,
        0x40 => zone_manager / set_zone_manager: *mut u8,
        0x44 => dirt_tool / set_dirt_tool: *mut u8,
        0x48 => street_tool / set_street_tool: *mut u8,
        0x4C => road_tool / set_road_tool: *mut u8,
        0x50 => lot_configuration / set_lot_configuration: *mut u8,
        0x99 => real_mode / set_real_mode: u8,
        0x9A => place_streets / set_place_streets: u8,
        0xA0 => current_zone / set_current_zone: u32,
        0xA4 => restriction / set_restriction: *mut u8,
        0xCC => orientation_q / set_orientation_q: u32,
        0xD0 => orientation_d / set_orientation_d: u32,
        0xD8 => virtual_lot_header / set_virtual_lot_header: *mut u8,
        0xDC => virtual_lot_count / set_virtual_lot_count: u32,
        0xE4 => deleted_lot_header / set_deleted_lot_header: *mut u8,
        0xE8 => deleted_lot_count / set_deleted_lot_count: u32,
    }
}

mod altitude;
mod creation;
mod basic;
mod deletion;
mod dezone;
mod edges;
mod examine;
mod face;
mod lot;
mod manager;
mod network;
mod occupancy;
mod parcels;
mod placement;
mod region;
mod registers;
mod remnants;
mod search;
mod street;
mod street_extension;

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
            + creation::PORTS.len()
            + dezone::PORTS.len()
            + deletion::PORTS.len()
            + lot::PORTS.len()
            + edges::PORTS.len()
            + examine::PORTS.len()
            + manager::PORTS.len()
            + network::PORTS.len()
            + occupancy::PORTS.len()
            + parcels::PORTS.len()
            + altitude::PORTS.len()
            + face::PORTS.len()
            + region::PORTS.len()
            + search::PORTS.len()
            + placement::PORTS.len()
            + street::PORTS.len()
            + street_extension::PORTS.len()
            + remnants::PORTS.len()
    },
>(&[
    basic::PORTS,
    creation::PORTS,
    dezone::PORTS,
    deletion::PORTS,
    lot::PORTS,
    edges::PORTS,
    examine::PORTS,
    manager::PORTS,
    network::PORTS,
    occupancy::PORTS,
    parcels::PORTS,
    altitude::PORTS,
    face::PORTS,
    region::PORTS,
    search::PORTS,
    placement::PORTS,
    street::PORTS,
    street_extension::PORTS,
    remnants::PORTS,
]);
