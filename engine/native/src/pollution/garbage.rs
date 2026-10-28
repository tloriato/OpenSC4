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

//! Garbage supply and the monthly landfill counter.

use super::*;

port! {
    /// Adds a signed delta to the monthly landfill counter, trimming only when signed 64-bit addition wraps.
    /// The return registers contain the updated high word in EAX and the adjusted delta's sign in EDX.
    #[port(address = 0x006E_3250, owner = "pollution", name = "cSC4PollutionSimulator::GarbagePutInLandfill")]
    pub unsafe extern "thiscall" fn fc4p_006e3250(this: *mut u8, delta: i32) -> u64 {
        // SAFETY: the game's simulator.
        let sim = unsafe { PollutionSim::from_ptr(this) };
        let old = sim.landfilled();
        let sum = old.wrapping_add(i64::from(delta));
        let adjusted = if delta > 0 && sum < old {
            (-1i32).wrapping_sub(old as i32)
        } else if delta < 0 && sum > old {
            (old as i32).wrapping_neg()
        } else {
            delta
        };
        let sum = old.wrapping_add(i64::from(adjusted));
        sim.set_landfilled(sum);
        registers((sum >> 32) as u32, (adjusted >> 31) as u32)
    }
}

port! {
    /// Returns the accepted delta before the final nonnegative clamp. Positive overflow caps at i32::MAX.
    /// Negative additions use the wrapped sum when deciding whether to replace the delta with minus supply.
    #[port(address = 0x006E_32E0, owner = "pollution", name = "cSC4PollutionSimulator::AddToGarbageSupply")]
    pub unsafe extern "thiscall" fn fc4p_006e32e0(this: *mut u8, delta: i32) -> u64 {
        // SAFETY: the game's simulator.
        let sim = unsafe { PollutionSim::from_ptr(this) };
        let old = sim.supply();
        let adjusted = if delta > 0 && old.wrapping_add(delta) < old {
            i32::MAX.wrapping_sub(old)
        } else if delta < 0 && old.wrapping_add(delta) < 0 {
            old.wrapping_neg()
        } else {
            delta
        };
        let sum = old.wrapping_add(adjusted);
        sim.set_supply(sum.max(0));
        registers(adjusted as u32, sum as u32)
    }
}

port! {
    /// Repairs a negative supply to zero before returning it, retaining entry EDX.
    #[port(address = 0x006E_3340, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageSupply")]
    pub unsafe extern "fastcall" fn fc4p_006e3340(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: the game's simulator.
        let sim = unsafe { PollutionSim::from_ptr(this) };
        if sim.supply() < 0 {
            sim.set_supply(0);
        }
        registers(sim.supply() as u32, edx)
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e3250::PORT, fc4p_006e32e0::PORT, fc4p_006e3340::PORT];
