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

//! Identity, reference counting, and small power accessors.

use super::*;

game_data! { static zero_percent: u32 = 0x00B4_A5B0; }

port! {
    /// Return the stored monthly demand, leaving EDX untouched.
    #[port(address = 0x0047_7110, owner = "power", name = "cSC4PowerSimulator::GetMonthlyDemand", naked)]
    pub unsafe extern "C" fn fc4p_00477110() {
        core::arch::naked_asm!("mov eax, [ecx + 0x60]", "ret");
    }
}
port! {
    /// The power simulator class identifier; EDX is untouched.
    #[port(address = 0x006E_ABB0, owner = "power", name = "cSC4PowerSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006eabb0() {
        core::arch::naked_asm!("mov eax, 0x0990c005", "ret");
    }
}
port! {
    /// Select an interface and retain the primary object. Failed lookups leave the destination untouched.
    /// AL is the answer; the remaining bytes retain the IID arithmetic or AddRef's result.
    #[port(address = 0x006E_ABC0, owner = "power", name = "cSC4PowerSimulator::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_006eabc0(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        unsafe {
            let offset = match iid {
                1 | 0xE9E7_BFF5 => 0,
                0x4522_94AA => 4,
                0xE4FD_A3D4 => 8,
                _ => {
                    let eax = if iid < 0xE4FD_A3D4 { iid.wrapping_sub(0x4522_94AA) } else { iid };
                    return eax & !255;
                }
            };
            out.write_unaligned(if this.is_null() { this } else { this.add(offset) });
            let retained = vcall!(this, 4 => fn() -> u32);
            (retained & !255) | 1
        }
    }
}
port! {
    /// Forward the supply query through the primary interface, preserving the return registers.
    #[port(address = 0x006E_AD50, owner = "power", name = "cSC4PowerSimulator::GetSupplyValue", naked)]
    pub unsafe extern "C" fn fc4p_006ead50() {
        core::arch::naked_asm!("mov eax, [ecx]", "jmp dword ptr [eax + 0x94]");
    }
}
port! {
    /// Copy the game's zero percentage word and return its destination. EDX is unchanged.
    #[port(address = 0x006E_B130, owner = "power", name = "cSC4PowerSimulator::GetChanceOfStrike")]
    pub unsafe extern "fastcall" fn fc4p_006eb130(_this: *mut u8, entry_edx: u32, out: *mut u32) -> u64 {
        unsafe { out.write_unaligned(*zero_percent()) };
        (u64::from(entry_edx) << 32) | u64::from(out as u32)
    }
}
port! {
    /// Forward to the power department's funding query, including its ST0 result.
    #[port(address = 0x006E_B180, owner = "power", name = "cSC4PowerSimulator::GetFundingPercent", naked)]
    pub unsafe extern "C" fn fc4p_006eb180() {
        core::arch::naked_asm!("mov ecx, [ecx + 0x20]", "mov eax, [ecx]", "jmp dword ptr [eax + 0x28]");
    }
}
port! {
    /// Decrement a shared reference, or shut down and invoke the deleting destructor when count <= 1.
    #[port(address = 0x006E_B190, owner = "power", name = "cSC4PowerSimulator::Release")]
    pub unsafe extern "thiscall" fn fc4p_006eb190(this: *mut u8) -> u32 {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let count = sim.references();
            if count > 1 {
                let next = count.wrapping_sub(1);
                sim.set_references(next);
                next as u32
            } else {
                vcall!(this, 0x10 => fn() -> u32);
                vcall!(this, 0xD0 => fn(u32) -> u32, 1);
                0
            }
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00477110::PORT,
    fc4p_006eabb0::PORT,
    fc4p_006eabc0::PORT,
    fc4p_006ead50::PORT,
    fc4p_006eb130::PORT,
    fc4p_006eb180::PORT,
    fc4p_006eb190::PORT,
];

#[cfg(test)]
mod tests;
