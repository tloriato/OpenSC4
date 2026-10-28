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

//! Recycling capacity conversion.

use super::*;

game_data! { static default_recycling_capacity: u32 = 0x00B4_9BC4; }

port! {
    /// Queries the current recycling capacity and scales its unsigned word by one percent.
    /// Failure writes the default's raw bits. The query's EAX survives; EDX is the destination or default word.
    #[port(address = 0x006E_3720, owner = "pollution", name = "cSC4PollutionSimulator::GetRecyclingCenterCurrentCapacity")]
    pub unsafe extern "thiscall" fn fc4p_006e3720(this: *mut u8, occupant: *mut u8, output: *mut f32) -> u64 {
        // SAFETY: the original simulator, occupant and output destination.
        unsafe {
            let mut capacity = occupant as u32;
            let result = virtual_call(this, 0xD4, table(this), &mut capacity as *mut u32 as u32,
                &[0x88ED_1059, occupant as u32, &mut capacity as *mut u32 as u32]);
            if result as u8 != 0 {
                let signed = F80::from_i32(capacity as i32);
                let unsigned = if (capacity as i32) < 0 { signed + F80::from_f32(4_294_967_296.0) } else { signed };
                write_unaligned(output, (unsigned * F80::from_f32(0.01)).to_f32());
                registers(result as u32, output as u32)
            } else {
                let default = *default_recycling_capacity();
                write_unaligned(output.cast::<u32>(), default);
                registers(result as u32, default)
            }
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e3720::PORT];
