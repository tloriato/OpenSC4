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

//! `cSC4LotDeveloper`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;

layout! {
    /// The lot developer, measured from its primary interface. The object occupies 0x108 bytes.
    pub struct LotDeveloper {
        0x10 => references / set_references: i32,
        0x18 => building_development / set_building_development: *mut u8,
        0x20 => demolition / set_demolition: *mut u8,
        0x24 => lot_manager / set_lot_manager: *mut u8,
        0x28 => occupant_manager / set_occupant_manager: *mut u8,
        0x30 => prop_manager / set_prop_manager: *mut u8,
        0x3C => terrain / set_terrain: *mut u8,
        0x40 => vertices_x / set_vertices_x: i32,
        0x5C => force_empty / set_force_empty: u8,
        0xD4 => foundation_slope / set_foundation_slope: f32,
        0xDC => building_foundation_type / set_building_foundation_type: u32,
    }
}

/// A virtual call with the register inputs used by its caller. The two result words are retained even
/// for methods whose declared result uses only AL. `take_st0` also captures and pops a floating result.
unsafe fn virtual_call(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32, take_st0: bool) -> RawCall {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call.take_st0 = u32::from(take_st0);
        call_raw(&mut call);
        call
    }
}

unsafe fn table(object: *mut u8) -> u32 {
    unsafe { object.cast::<u32>().read_unaligned() }
}

fn result(call: RawCall) -> u64 {
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

pub mod foundation;
pub mod lots;
pub mod small;

const ALL: [Port; foundation::PORTS.len() + lots::PORTS.len() + small::PORTS.len()] =
    crate::port::concat(&[small::PORTS, lots::PORTS, foundation::PORTS]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;

#[cfg(test)]
mod tests;
