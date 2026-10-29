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

//! The traffic map's tile chains, object interfaces and intersection maintenance.

use crate::port::Port;

// Some entries expose the secondary integer register even though their logical result fits EAX.
// Keep both results and supply the scratch registers the selected virtual method originally received.
unsafe fn virtual_words(this: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
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
        words(call.eax, call.edx)
    }
}

fn words(eax: u32, edx: u32) -> u64 {
    u64::from(eax) | (u64::from(edx) << 32)
}

unsafe fn table_word(this: *mut u8) -> u32 {
    unsafe { this.cast::<u32>().read_unaligned() }
}

layout! {
    /// Primary traffic map interface and the fields used by these operations.
    pub struct TrafficNetworkMap {
        0x00 => vtable / set_vtable: *const usize,
        0x14 => references / set_references: u32,
        0x18 => cells / set_cells: *mut *mut u8,
        0x1C => width / set_width: i32,
        0x20 => height / set_height: i32,
        0x3C => intersections_changed / set_intersections_changed: u8,
    }
}

layout! {
    /// A traffic tile, linked to the other networks in the same cell.
    pub struct TileInfo {
        0x00 => network / set_network: *mut u8,
        0x1C => automata / set_automata: *mut u8,
        0x54 => next / set_next: *mut u8,
    }
}

pub mod intersections;
pub mod lookup;
pub mod storage;

const COUNT: usize = lookup::PORTS.len() + storage::PORTS.len() + intersections::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[lookup::PORTS, storage::PORTS, intersections::PORTS]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;

#[cfg(test)]
unsafe fn stack_arguments(function: usize, this: *mut u8, args: [u32; 3], caller_pops: bool) -> (u32, u32, [u32; 3]) {
    let mut frame = [
        function as u32,
        this as u32,
        args[0],
        args[1],
        args[2],
        u32::from(caller_pops),
        0,
        0,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push ebx", "push dword ptr [edi + 16]", "push dword ptr [edi + 12]", "push dword ptr [edi + 8]",
            "mov ebx, esp", "mov ecx, dword ptr [edi + 4]", "call dword ptr [edi]",
            "mov dword ptr [edi + 24], eax", "mov dword ptr [edi + 40], edx",
            "mov eax, dword ptr [ebx]", "mov dword ptr [edi + 28], eax",
            "mov eax, dword ptr [ebx + 4]", "mov dword ptr [edi + 32], eax",
            "mov eax, dword ptr [ebx + 8]", "mov dword ptr [edi + 36], eax",
            "cmp dword ptr [edi + 20], 0", "je 2f", "add esp, 12", "2:", "pop ebx",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    (frame[6], frame[10], [frame[7], frame[8], frame[9]])
}
