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

//! `cSC4BudgetSimulator`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Budget simulator, relative to its primary interface.
    pub struct BudgetSim {
        0x00 => vtable / set_vtable: u32,
        0x10 => references / set_references: i32,
        0x18 => city / set_city: *mut u8,
        0x1C => messages / set_messages: *mut u8,
        0x24 => ordinances / set_ordinances: *mut u8,
        0x28 => funds / set_funds: i64,
        0x30 => shadow_funds / set_shadow_funds: i64,
        0x54 => departments / set_departments: *mut u8,
        0x670 => tax_income / set_tax_income: i64,
        0x780 => bonds / set_bonds: *mut u8,
        0x790 => starting_funds / set_starting_funds: *mut i64,
        0x7B0 => borrowing_multiplier / set_borrowing_multiplier: i32,
        0x7B8 => borrowing_base / set_borrowing_base: i64,
        0x814 => tax_multipliers / set_tax_multipliers: *mut f32,
        0x820 => default_rates / set_default_rates: *mut u32,
    }
}

layout! {
    /// One 128-byte tax record; its first byte is at simulator +0x70.
    pub struct TaxGroup {
        0x00 => rate / set_rate: f32,
        0x70 => assessed / set_assessed: i64,
    }
}

impl BudgetSim {
    /// Tax indices use a wrapping 32-bit shift, including negative indices.
    ///
    /// # Safety
    /// The selected record must be readable and writable.
    pub unsafe fn tax_group(self, index: i32) -> TaxGroup {
        // SAFETY: the caller supplies the selected record's storage.
        unsafe {
            TaxGroup::from_ptr(
                self.as_ptr()
                    .wrapping_add(index.wrapping_shl(7) as usize)
                    .wrapping_add(0x70),
            )
        }
    }
}

/// Low and high integer return registers.
fn pair(eax: u32, edx: u32) -> u64 {
    u64::from(eax) | (u64::from(edx) << 32)
}

/// Calls a live virtual method with the original integer register inputs.
unsafe fn virtual_regs(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
    // SAFETY: each caller supplies a live interface and its slot's original arguments.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        pair(call.eax, call.edx)
    }
}

/// Reads one field from a live object or container node.
unsafe fn field<T: Copy>(p: *mut u8, offset: usize) -> T {
    // SAFETY: each caller supplies the field's live storage.
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

pub mod access;
pub mod funds;
pub mod interfaces;
pub mod tax;
pub mod totals;

const COUNT: usize =
    access::PORTS.len() + funds::PORTS.len() + interfaces::PORTS.len() + tax::PORTS.len() + totals::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[
    access::PORTS,
    funds::PORTS,
    interfaces::PORTS,
    tax::PORTS,
    totals::PORTS,
]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;

#[cfg(test)]
mod tests;
