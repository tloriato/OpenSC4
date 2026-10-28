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

//! Plant production and infrastructure accounting.

use super::*;

game_fn! { fn get_uint(holder: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn get_int64(holder: *mut u8, id: u32, out: *mut u64) -> u32 = "C" 0x005F_D4B0; }

port! {
    /// Read unsigned capacity, then multiply it by the plant's current efficiency using x87 arithmetic.
    /// A failed property lookup returns the helper's output word even if it was changed.
    #[port(address = 0x006E_B1C0, owner = "power", name = "cSC4PowerSimulator::GetCurrentPowerProduced")]
    pub unsafe extern "thiscall" fn fc4p_006eb1c0(this: *mut u8, plant: *mut u8) -> u32 {
        unsafe {
            let mut capacity = 0u32;
            if plant.is_null() { return capacity; }
            let holder = vcall!(plant, 0x18 => fn() -> *mut u8);
            if get_uint(holder, 0x2781_2852, &mut capacity) as u8 == 0 { return capacity; }
            let mut temporary = core::mem::MaybeUninit::<f32>::uninit();
            let efficiency = vcall!(this, 0xB4 => fn(*mut f32, *mut u8) -> *mut f32, temporary.as_mut_ptr(), plant);
            let mut value = F80::from_i32(capacity as i32);
            if (capacity as i32) < 0 { value = add_float(value, 4_294_967_296.0); }
            multiply_float(value, efficiency.read_unaligned()).ftol() as u32
        }
    }
}
port! {
    /// Sum the low words of pole costs with wrapping, clear the high cost word, and report whether it changed.
    /// The upper EAX bytes retain the property reader, old high cost, or entry EAX on the empty path.
    #[port(address = 0x006E_B490, owner = "power", name = "cSC4PowerSimulator::RecomputeInfrastructureCosts", entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006eb490(this: *mut u8, [entry_eax: u32]) -> u32 {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let (mut cursor, end) = (sim.poles_begin(), sim.poles_end());
            let mut total = 0u32;
            let mut eax = entry_eax;
            while cursor != end {
                let pole = cursor.read_unaligned();
                let occupant = vcall!(pole, 0x0C => fn() -> *mut u8);
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let mut cost = 0u64;
                eax = get_int64(holder, 0x0913_2DE8, &mut cost);
                total = total.wrapping_add(cost as u32);
                cursor = cursor.wrapping_add(1);
            }
            let old = sim.infrastructure_cost();
            let changed = if old as u32 != total { true } else {
                eax = (old >> 32) as u32;
                eax != 0
            };
            sim.set_infrastructure_cost(u64::from(total));
            (eax & !255) | u32::from(changed)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006eb1c0::PORT, fc4p_006eb490::PORT];

#[cfg(test)]
mod tests;
