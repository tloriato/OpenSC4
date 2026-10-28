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

//! Native neighbor interfaces, connection queries, and monthly accounting.

use crate::port::Port;

layout! {
    /// `cSC4NeighborsSimulator`, measured from its primary interface.
    pub struct NeighborsSim {
        0x00 => primary_vtable / set_primary_vtable: u32,
        0x04 => message_vtable / set_message_vtable: u32,
        0x08 => serializable_vtable / set_serializable_vtable: u32,
        0x10 => references / set_references: i32,
        0x14 => city / set_city: *mut u8,
        0x18 => message_server / set_message_server: *mut u8,
        0x1C => connections / set_connections: *mut u8,
        0x20 => deals / set_deals: *mut u8,
        0x3C => deals_dirty / set_deals_dirty: u8,
    }
}

mod queries;
mod surface;

const COUNT: usize = surface::PORTS.len() + queries::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[surface::PORTS, queries::PORTS]);
pub const PORTS: &[Port] = &ALL;

/// A virtual call retaining both integer return registers. The supplied register words are those
/// present at the original call, including values that a short virtual method may leave untouched.
unsafe fn virtual_call(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
    use crate::abi::{call_raw, vslot, Conv, RawCall};
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        pair(call.eax, call.edx)
    }
}

fn pair(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { core::ptr::read_unaligned(p.add(offset).cast()) }
}

unsafe fn pointer(p: *mut u8, offset: usize) -> *mut u8 {
    unsafe { word(p, offset) as *mut u8 }
}

#[cfg(test)]
mod tests;
