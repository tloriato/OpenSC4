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

//! Pollution grid reads, cell writes and active-landfill flags.

use super::*;

port! {
    /// Reads a cell, sign-extends its low word, and stores the full result.
    #[port(address = 0x006e_2ef0, owner = "pollution", name = "cSC4PollutionSimulator::GetAirValue")]
    pub unsafe extern "thiscall" fn fc4p_006e2ef0(this: *mut u8, x: u32, z: u32, output: *mut i32) -> u64 {
        // SAFETY: the simulator's live grid and original output destination.
        unsafe {
            let grid = PollutionSim::from_ptr(this).air();
            let result = virtual_call(grid, 0x1C, table(grid), x, &[x, z]);
            let value = result as u16 as i16 as i32 as u32;
            write_unaligned(output, value as i32);
            (result & !0xFFFF_FFFF) | u64::from(value)
        }
    }
}

port! {
    /// Reads a cell, sign-extends its low word, and stores the full result.
    #[port(address = 0x006e_2f10, owner = "pollution", name = "cSC4PollutionSimulator::GetWaterValue")]
    pub unsafe extern "thiscall" fn fc4p_006e2f10(this: *mut u8, x: u32, z: u32, output: *mut i32) -> u64 {
        // SAFETY: the simulator's live grid and original output destination.
        unsafe {
            let grid = PollutionSim::from_ptr(this).water();
            let result = virtual_call(grid, 0x1C, table(grid), x, &[x, z]);
            let value = result as u16 as i16 as i32 as u32;
            write_unaligned(output, value as i32);
            (result & !0xFFFF_FFFF) | u64::from(value)
        }
    }
}

port! {
    /// Reads a cell, sign-extends its low word, then clears bit 15, and stores the full result.
    #[port(address = 0x006e_2f30, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageValue")]
    pub unsafe extern "thiscall" fn fc4p_006e2f30(this: *mut u8, x: u32, z: u32, output: *mut i32) -> u64 {
        // SAFETY: the simulator's live grid and original output destination.
        unsafe {
            let grid = PollutionSim::from_ptr(this).garbage();
            let result = virtual_call(grid, 0x1C, table(grid), x, &[x, z]);
            let value = (result as u16 as i16 as i32 as u32) & 0xFFFF_7FFF;
            write_unaligned(output, value as i32);
            (result & !0xFFFF_FFFF) | u64::from(value)
        }
    }
}

port! {
    /// Reads the tract shift, writes the cell's tract with logical coordinate shifts, then marks air dirty.
    /// The grid pointer is read again after the shift call. All value bits are passed to the setter.
    #[port(address = 0x006e_3070, owner = "pollution", name = "cSC4PollutionSimulator::SetAirValue")]
    pub unsafe extern "fastcall" fn fc4p_006e3070(this: *mut u8, edx: u32, x: u32, z: u32, value: u32) -> u64 {
        // SAFETY: the game's simulator and its live grid interfaces.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            let grid = sim.air();
            let shift = virtual_call(grid, 0x2C, table(grid), edx, &[]) as u32;
            let grid = sim.air();
            let result = virtual_call(grid, 0x68, grid as u32, table(grid), &[x.wrapping_shr(shift), z.wrapping_shr(shift), value]);
            sim.set_dirty_air(1);
            result
        }
    }
}

port! {
    /// Reads the tract shift, writes the cell's tract with logical coordinate shifts, then marks water dirty.
    /// The grid pointer is read again after the shift call. All value bits are passed to the setter.
    #[port(address = 0x006e_30b0, owner = "pollution", name = "cSC4PollutionSimulator::SetWaterValue")]
    pub unsafe extern "fastcall" fn fc4p_006e30b0(this: *mut u8, edx: u32, x: u32, z: u32, value: u32) -> u64 {
        // SAFETY: the game's simulator and its live grid interfaces.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            let grid = sim.water();
            let shift = virtual_call(grid, 0x2C, table(grid), edx, &[]) as u32;
            let grid = sim.water();
            let result = virtual_call(grid, 0x68, grid as u32, table(grid), &[x.wrapping_shr(shift), z.wrapping_shr(shift), value]);
            sim.set_dirty_water(1);
            result
        }
    }
}

port! {
    /// Retains the old signed high bits and ORs the supplied value without masking it before writing the tract.
    /// The old value is read with cell coordinates; only the final write uses the tract shift.
    #[port(address = 0x006E_30F0, owner = "pollution", name = "cSC4PollutionSimulator::SetGarbageValue")]
    pub unsafe extern "fastcall" fn fc4p_006e30f0(this: *mut u8, edx: u32, x: u32, z: u32, value: u32) -> u64 {
        // SAFETY: the game's simulator and its garbage grid.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            let grid = sim.garbage();
            let shift = virtual_call(grid, 0x2C, table(grid), edx, &[]) as u32;
            let grid = sim.garbage();
            let old = virtual_call(grid, 0x1C, table(grid), table(grid), &[x, z]) as u32;
            let grid = sim.garbage();
            let value = ((old as u16 as i16 as i32 as u32) & 0xFFFF_8000) | value;
            let result = virtual_call(grid, 0x68, table(grid), grid as u32, &[x.wrapping_shr(shift), z.wrapping_shr(shift), value]);
            sim.set_dirty_garbage(1);
            result
        }
    }
}

port! {
    /// Tests bit 15 in the cell's raw value. Other EAX bytes retain the grid result shifted right by 15.
    #[port(address = 0x006E_31A0, owner = "pollution", name = "cSC4PollutionSimulator::IsActiveLandfill")]
    pub unsafe extern "thiscall" fn fc4p_006e31a0(this: *mut u8, x: u32, z: u32) -> u64 {
        // SAFETY: the game's simulator and its garbage grid.
        unsafe {
            let grid = PollutionSim::from_ptr(this).garbage();
            let result = virtual_call(grid, 0x1C, table(grid), x, &[x, z]);
            let shifted = (result as u32) >> 15;
            registers((shifted & !0xFF) | (shifted & 1), (result >> 32) as u32)
        }
    }
}

port! {
    /// Enables landfill only in landfill zones. A real flag change updates the cell count and marks garbage dirty.
    /// Clearing the flag keeps the count at zero when it is already zero. The boolean uses only the low byte.
    #[port(address = 0x006E_31C0, owner = "pollution", name = "cSC4PollutionSimulator::SetIsActiveLandfill")]
    pub unsafe extern "fastcall" fn fc4p_006e31c0(this: *mut u8, edx: u32, x: u32, z: u32, active: u32) -> u64 {
        // SAFETY: the game's simulator, zone manager and garbage grid.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            let zones = sim.zones();
            let mut result = virtual_call(zones, 0x30, table(zones), edx, &[x, z]);
            let active = active as u8 != 0;
            if active && result as u32 != 14 {
                return result;
            }
            result = virtual_call(this, 0xC4, result as u32, table(this), &[x, z]);
            if active == (result as u8 != 0) {
                return result;
            }
            let grid = sim.garbage();
            result = virtual_call(grid, 0x1C, table(grid), (result >> 32) as u32, &[x, z]);
            let value = if active {
                sim.set_active_landfill_cells(sim.active_landfill_cells().wrapping_add(1));
                (result as u32) | 0x8000
            } else {
                let count = sim.active_landfill_cells();
                if count != 0 {
                    sim.set_active_landfill_cells(count - 1);
                }
                (result as u32) & 0x7FFF
            };
            let grid = sim.garbage();
            let result = virtual_call(grid, 0x68, value, table(grid), &[x, z, value]);
            sim.set_dirty_garbage(1);
            result
        }
    }
}

port! {
    /// Returns the radiation source grid for type three and the indexed value-grid pointer for every other type.
    /// The original index is unchecked and wraps at 32 bits.
    #[port(address = 0x006E_3360, owner = "pollution", name = "cSC4PollutionSimulator::GetPollutionGrid")]
    pub unsafe extern "fastcall" fn fc4p_006e3360(this: *mut u8, edx: u32, kind: u32) -> u64 {
        // SAFETY: the game supplies an accessible field for the requested index.
        unsafe {
            let offset = if kind == 3 { 0x4C } else { kind.wrapping_mul(4).wrapping_add(0x50) };
            registers(read_unaligned(this.wrapping_add(offset as usize).cast()), edx)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006e2ef0::PORT,
    fc4p_006e2f10::PORT,
    fc4p_006e2f30::PORT,
    fc4p_006e3070::PORT,
    fc4p_006e30b0::PORT,
    fc4p_006e30f0::PORT,
    fc4p_006e31a0::PORT,
    fc4p_006e31c0::PORT,
    fc4p_006e3360::PORT
];
