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

//! Class identity, interface adjustment and reference release.

use super::*;

game_fn! {
    fn query_interface(this: *mut u8, entry_edx: u32, iid: u32, output: *mut *mut u8) -> u64 = "fastcall" 0x006E_2CD0;
}
game_fn! {
    fn destroy(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x006E_4E20;
}
game_fn! {
    #[allow(dead_code)]
    fn free(this: *mut u8) -> u64 = "C" 0x005E_5620;
}

port! {
    /// Returns the simulator's class identifier without changing EDX.
    #[port(address = 0x006E_2CC0, owner = "pollution", name = "cSC4PollutionSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006e2cc0() {
        core::arch::naked_asm!("mov eax, 0x8990c065", "ret");
    }
}

port! {
    /// Exposes the primary, message, prop-requester or serializable interface and retains the primary object.
    /// Unknown identifiers leave the output alone. AL reports success; the other bytes retain the last result.
    #[port(address = 0x006E_2CD0, owner = "pollution", name = "cSC4PollutionSimulator::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_006e2cd0(this: *mut u8, edx: u32, iid: u32, output: *mut *mut u8) -> u64 {
        let offset = match iid {
            1 | 0x8771_81EC => 0,
            0x4522_94AA => 4,
            0x89C4_98BE => 8,
            0xE4FD_A3D4 => 12,
            _ => {
                let eax = if iid < 0x8771_81EC { iid.wrapping_sub(1).wrapping_sub(0x4522_94A9) } else { iid };
                return registers(eax & !0xFF, edx);
            }
        };
        // SAFETY: the original interface output and the object's AddRef method.
        unsafe {
            let interface = if this.is_null() { this } else { this.add(offset) };
            write_unaligned(output, interface);
            let result = virtual_call(this, 4, table(this), output as u32, &[]);
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Adjusts the prop-requester interface to the primary interface before querying it at its game address.
    #[port(address = 0x006E_50B0, owner = "pollution", name = "prop requester QueryInterface adjustment")]
    pub unsafe extern "fastcall" fn fc4p_006e50b0(this: *mut u8, edx: u32, iid: u32, output: *mut *mut u8) -> u64 {
        // SAFETY: this interface is eight bytes into the object.
        unsafe { query_interface(this.wrapping_sub(8), edx, iid, output) }
    }
}

port! {
    /// Adjusts the serializable interface to the primary interface before querying it at its game address.
    #[port(address = 0x006E_50C0, owner = "pollution", name = "serializable QueryInterface adjustment")]
    pub unsafe extern "fastcall" fn fc4p_006e50c0(this: *mut u8, edx: u32, iid: u32, output: *mut *mut u8) -> u64 {
        // SAFETY: this interface is twelve bytes into the object.
        unsafe { query_interface(this.wrapping_sub(12), edx, iid, output) }
    }
}

port! {
    /// Returns the garbage prop requester's satisfaction identifier without changing EDX.
    #[port(address = 0x006E_5030, owner = "pollution", name = "cSC4PollutionSimulator::GetRequesterSatisfactionId", naked)]
    pub unsafe extern "C" fn fc4p_006e5030() {
        core::arch::naked_asm!("mov eax, 0xe9a4300b", "ret");
    }
}

port! {
    /// Decrements counts above one. Otherwise shuts down, destroys and frees the object, returning zero.
    #[port(address = 0x006E_5F20, owner = "pollution", name = "cSC4PollutionSimulator::Release")]
    pub unsafe extern "fastcall" fn fc4p_006e5f20(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: the simulator and its original lifecycle entry points.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            let count = sim.references();
            if count > 1 {
                sim.set_references(count - 1);
                return registers((count - 1) as u32, edx);
            }
            let result = virtual_call(this, 0x10, table(this), edx, &[]);
            let result = destroy(this, (result >> 32) as u32);
            let args = [this as u32];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x005E_5620), this as u32, &args);
            call.eax = result as u32;
            call.edx = (result >> 32) as u32;
            call_raw(&mut call);
            registers(0, call.edx)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006e2cc0::PORT,
    fc4p_006e2cd0::PORT,
    fc4p_006e5030::PORT,
    fc4p_006e50b0::PORT,
    fc4p_006e50c0::PORT,
    fc4p_006e5f20::PORT,
];
