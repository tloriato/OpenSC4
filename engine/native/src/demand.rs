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

//! The demand simulator's interfaces, demand map and jobs lookup.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Offsets from the primary demand-simulator interface.
    pub struct DemandSim {
        0x00 => vtable / set_vtable: u32,
        0x0D => initialized / set_initialized: u8,
        0x20 => current_census_count / set_current_census_count: u32,
        0x28 => previous_census / set_previous_census: *mut u8,
        0x2C => previous_census_count / set_previous_census_count: u32,
        0x3C => building_development / set_building_development: *mut u8,
        0x44 => neighbors / set_neighbors: *mut u8,
        0x48 => ordinances / set_ordinances: *mut u8,
        0x50 => nodes / set_nodes: *mut u8,
        0x54 => node_count / set_node_count: u32,
        0x98 => previous_cities / set_previous_cities: *mut u8,
        0x9C => export_cities / set_export_cities: *mut u8,
        0xA0 => commercial_bonus / set_commercial_bonus: u32,
        0xA4 => industrial_bonus / set_industrial_bonus: u32,
        0xA8 => commercial_relief / set_commercial_relief: u32,
        0xAC => industrial_relief / set_industrial_relief: u32,
        0xB0 => neutral_tax_curve / set_neutral_tax_curve: *mut u8,
        0xB4 => tax_curve / set_tax_curve: *mut u8,
        0xB8 => tax_penalty_days / set_tax_penalty_days: u32,
    }
}

layout! {
    /// The map header and ordinary nodes share the first four fields. Ordinary nodes hold a key and value.
    struct TreeNode {
        0x00 => color / set_color: u8,
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: u32,
        0x14 => value / set_value: u32,
    }
}

/// Entry adapters save the incoming registers before handing Rust the original argument words.
#[repr(C)]
struct Entry {
    eax: u32,
    edx: u32,
    return_address: u32,
    args: [u32; 0],
}

impl Entry {
    unsafe fn arg(&self, i: usize) -> u32 {
        // SAFETY: the enclosing port knows its original number of stack arguments.
        unsafe { read_unaligned(self.args.as_ptr().add(i)) }
    }

    unsafe fn put(&mut self, i: usize, value: u32) {
        // SAFETY: the enclosing port knows its original number of stack arguments.
        unsafe { write_unaligned(self.args.as_mut_ptr().add(i), value) }
    }
}

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

/// A virtual call with the register inputs used by the caller, retaining both result registers.
unsafe fn virtual_call(object: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> RawCall {
    // SAFETY: the enclosing port supplies the live object, slot and original argument words.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        call
    }
}

/// Advance an ordinary node using the header-linked tree's successor rule.
unsafe fn successor(node: *mut u8) -> *mut u8 {
    unsafe {
        let mut current = TreeNode::from_ptr(node);
        let mut next = current.right();
        if !next.is_null() {
            loop {
                current = TreeNode::from_ptr(next);
                next = current.left();
                if next.is_null() {
                    return current.as_ptr();
                }
            }
        }
        next = current.parent();
        while current.as_ptr() == TreeNode::from_ptr(next).right() {
            current = TreeNode::from_ptr(next);
            next = current.parent();
        }
        if current.right() != next {
            current = TreeNode::from_ptr(next);
        }
        current.as_ptr()
    }
}

mod capacity;
mod census;
mod effects;
mod extrapolate;
mod interface;
mod io;
mod jobs;
mod links;
mod occupant;
mod population;
mod queries;
mod regional;
mod scan;
mod sort;
mod tree;

#[cfg(test)]
mod test_support;

const COUNT: usize = capacity::PORTS.len()
    + census::PORTS.len()
    + effects::PORTS.len()
    + extrapolate::PORTS.len()
    + interface::PORTS.len()
    + io::PORTS.len()
    + jobs::PORTS.len()
    + links::PORTS.len()
    + occupant::PORTS.len()
    + population::PORTS.len()
    + queries::PORTS.len()
    + regional::PORTS.len()
    + scan::PORTS.len()
    + sort::PORTS.len()
    + tree::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[
    capacity::PORTS,
    census::PORTS,
    effects::PORTS,
    extrapolate::PORTS,
    interface::PORTS,
    io::PORTS,
    jobs::PORTS,
    links::PORTS,
    occupant::PORTS,
    population::PORTS,
    queries::PORTS,
    regional::PORTS,
    scan::PORTS,
    sort::PORTS,
    tree::PORTS,
]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;
