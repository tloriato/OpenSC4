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
        0x44 => terrain / set_terrain: *mut u8,
        0x240 => width / set_width: u32,
        0x244 => height / set_height: u32,
        0x358 => type_info / set_type_info: *mut u8,
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

mod helpers;
mod preview;
mod rules;

const COUNT: usize = helpers::PORTS.len() + preview::PORTS.len() + rules::PORTS.len();
/// Completed replacements, ordered by original address within each group.
pub const PORTS: &[Port] = &crate::port::concat::<COUNT>(&[helpers::PORTS, preview::PORTS, rules::PORTS]);
