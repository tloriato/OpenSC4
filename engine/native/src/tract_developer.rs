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

//! `cSC4TractDeveloper`: methods and the shared container helpers used by growth attempts.

use crate::abi::{Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Primary tract-developer interface. Message and persistence interfaces are at +4 and +8.
    pub struct TractDeveloper {
        0x00 => vtable / set_vtable: u32,
        0x10 => references / set_references: i32,
        0x18 => city / set_city: u32,
        0x1C => building_development / set_building_development: u32,
        0x50 => occupied_cost / set_occupied_cost: f32,
        0x54 => abandoned_cost / set_abandoned_cost: f32,
        0x58 => aggregation_cost / set_aggregation_cost: f32,
        0x5C => subdivision_cost / set_subdivision_cost: f32,
        0x6C => kick_out / set_kick_out: u8,
        0x7C => years_between_changes / set_years_between_changes: i32,
        0x80 => developer / set_developer: u32,
        0x8C => maximum_stage / set_maximum_stage: i32,
        0x98 => wealth / set_wealth: i32,
        0x9C => purpose / set_purpose: i32,
        0xB4 => existing / set_existing: i32,
        0xB8 => desired / set_desired: i32,
        0xBC => desired_maximum / set_desired_maximum: i32,
        0xC8 => relaxed / set_relaxed: u8,
        0xE4 => candidates_begin / set_candidates_begin: u32,
        0xE8 => candidates_end / set_candidates_end: u32,
        0xF8 => observed_land_value / set_observed_land_value: i32,
        0xFC => minimum_building_capacity / set_minimum_building_capacity: i32,
        0x100 => maximum_building_capacity / set_maximum_building_capacity: i32,
        0x118 => styles_begin / set_styles_begin: u32,
        0x11C => styles_end / set_styles_end: u32,
        0x124 => style_index / set_style_index: i32,
        0x128 => years_since_change / set_years_since_change: i32,
        0x12C => single_style / set_single_style: u8,
        0x170 => debug_context / set_debug_context: u32,
    }
}

layout! {
    /// A growth candidate. Bytes +0x16..+0x17 and +0xA1..+0xA3 are padding.
    pub struct Candidate {
        0x00 => kind / set_kind: i32,
        0x04 => lot / set_lot: *mut u8,
        0x08 => x / set_x: i32,
        0x0C => z / set_z: i32,
        0x10 => facing / set_facing: i32,
        0x14 => width / set_width: u8,
        0x15 => depth / set_depth: u8,
        0x18 => lot_header / set_lot_header: u32,
        0x1C => lot_count / set_lot_count: u32,
        0x24 => occupied / set_occupied: i32,
        0x28 => abandoned / set_abandoned: i32,
        0x2C => stage / set_stage: i32,
        0x30 => wealth / set_wealth: i32,
        0x34 => score / set_score: f32,
        0x38 => min_x / set_min_x: i32,
        0x3C => min_z / set_min_z: i32,
        0x40 => max_x / set_max_x: i32,
        0x44 => max_z / set_max_z: i32,
    }
}

layout! {
    /// Links common to the tree nodes and their header.
    struct Node {
        0x04 => parent / set_parent: u32,
        0x08 => left / set_left: u32,
        0x0C => right / set_right: u32,
    }
}

/// Saved volatile registers followed by the original return address and stack arguments.
/// The entry adapter changes only the register save area; arguments retain their original addresses.
#[repr(C)]
struct Entry<const N: usize> {
    edx: u32,
    ecx: u32,
    eax: u32,
    return_address: u32,
    args: [u32; N],
}

macro_rules! register_port {
    ($(#[doc = $doc:literal])* $address:literal, $export:ident, $body:ident, $words:literal, $pop:literal, $name:literal, |$r:ident| $logic:block) => {
        port! {
            $(#[doc = $doc])*
            #[port(address = $address, owner = "tractdeveloper", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push eax", "push ecx", "push edx", "push esp", "call {body}",
                    "pop edx", "pop ecx", "pop eax", "ret {pop}",
                    body = sym $body, pop = const $pop,
                );
            }
        }
        unsafe extern "stdcall" fn $body(entry: *mut Entry<$words>) {
            // SAFETY: the entry adapter supplies the saved registers and original stack arguments.
            unsafe {
                let $r = &mut *entry;
                $logic
            }
        }
    };
}

/// Make a call while retaining all three volatile register outputs. Some helpers leave a register
/// partly unchanged, so both the inputs and outputs are explicit.
unsafe fn invoke<const N: usize>(r: &mut Entry<N>, conv: Conv, target: usize, args: &[u32]) {
    let mut c = RawCall::new(conv, target, r.ecx, args);
    c.eax = r.eax;
    c.edx = r.edx;
    // SAFETY: each caller supplies the original target, register inputs and stack words. EDI holds
    // the call record across the callee; EBX, ESI, EDI and EBP are preserved by the game's conventions.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx",
            "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f",
            "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
            in("edi") &mut c, clobber_abi("C"),
        );
    }
    r.eax = c.eax;
    r.ecx = c.ecx;
    r.edx = c.edx;
}

unsafe fn word(base: u32, offset: u32) -> u32 {
    // SAFETY: the enclosing port supplies the live object's field.
    unsafe { read_unaligned(base.wrapping_add(offset) as *const u32) }
}

unsafe fn store(base: u32, offset: u32, value: u32) {
    // SAFETY: the enclosing port supplies the live object's writable field.
    unsafe { write_unaligned(base.wrapping_add(offset) as *mut u32, value) }
}

pub mod compatibility;
pub mod cache;
pub mod copying;
pub mod evaluation;
pub mod geometry;
pub mod networks;
pub mod pool;
pub mod sorting;
pub mod suitability;
pub mod surface;
pub mod trees;

const COUNT: usize = compatibility::PORTS.len()
    + cache::PORTS.len()
    + copying::PORTS.len()
    + evaluation::PORTS.len()
    + geometry::PORTS.len()
    + networks::PORTS.len()
    + pool::PORTS.len()
    + sorting::PORTS.len()
    + suitability::PORTS.len()
    + surface::PORTS.len()
    + trees::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[
    compatibility::PORTS,
    cache::PORTS,
    copying::PORTS,
    evaluation::PORTS,
    geometry::PORTS,
    networks::PORTS,
    pool::PORTS,
    sorting::PORTS,
    suitability::PORTS,
    surface::PORTS,
    trees::PORTS,
]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;

#[cfg(test)]
mod testing {
    use super::*;

    pub const EAX: u32 = 0x1234_5678;
    pub const ECX: u32 = 0xABCD_EF01;
    pub const EDX: u32 = 0x9876_5432;

    pub fn run(address: u32, conv: Conv, this: u32, args: &[u32]) -> (u32, u32, u32) {
        let mut r = Entry {
            eax: EAX,
            ecx: this,
            edx: EDX,
            return_address: 0,
            args: [],
        };
        // SAFETY: each test supplies objects and arguments matching the selected port.
        unsafe { invoke(&mut r, conv, crate::abi::target(address), args) };
        (r.eax, r.ecx, r.edx)
    }

    pub fn snapshot(p: *mut u8, n: usize) -> Vec<u8> {
        // SAFETY: the test owns at least n bytes at p.
        unsafe { std::slice::from_raw_parts(p, n).to_vec() }
    }
}
