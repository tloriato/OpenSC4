// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The commercial traffic window, with independent morning and evening maxima.

use super::*;

layout! {
    /// The window's upper row bound, relative to the primary simulator interface.
    pub struct WindowSim {
        0x24 => row_limit / set_row_limit: i32,
    }
}

game_fn! { fn cell_value(this: *mut u8, x: i32, z: i32, evening: u32) -> u32 = "thiscall" 0x0071_4940; }

port! {
    /// Sums two unsigned 16-bit maxima over a cross-shaped window around a four-cell block. The row
    /// bound is clipped to the city; the column bound is not. Coordinates and bounds wrap at 32 bits.
    /// Reuses the first caller argument for the evening maximum and the second for the column bound.
    #[port(address = 0x0071_4AA0, owner = "traffic", name = "CommercialTrafficWindow", naked)]
    pub unsafe extern "C" fn fc4p_00714aa0() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 8", sym commercial_window);
    }
}

unsafe extern "fastcall" fn commercial_window(this: *mut u8, stack: *mut u32) -> u32 {
    // SAFETY: the entry supplies its simulator and two writable stack arguments.
    unsafe {
        let x = (stack.add(1).read() as i32).wrapping_shl(2);
        let z = (stack.add(2).read() as i32).wrapping_shl(2);
        stack.add(2).write(z.wrapping_sub(2) as u32);
        stack.add(1).write(0);
        let mut row = z.wrapping_sub(2).max(0);
        stack.add(1).write(z.wrapping_add(6) as u32);
        let end = WindowSim::from_ptr(this).row_limit().min(z.wrapping_add(6));
        let mut morning = 0u16;
        stack.add(1).write(0);
        while row < end {
            let (mut column, bound) = if row >= z && row < z.wrapping_add(4) {
                stack.add(2).write(0);
                (x.wrapping_sub(2).max(0), x.wrapping_add(6))
            } else {
                (x, x.wrapping_add(4))
            };
            stack.add(2).write(bound as u32);
            while column < stack.add(2).read() as i32 {
                let a = cell_value(this, column, row, 0);
                let b = cell_value(this, column, row, 1);
                morning = morning.max(a as u16);
                let evening = (stack.add(1).read() as u16).max(b as u16);
                stack.add(1).cast::<u16>().write(evening);
                column = column.wrapping_add(1);
            }
            row = row.wrapping_add(1);
        }
        stack.add(1).read().wrapping_add(u32::from(morning))
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_00714aa0::PORT];

#[cfg(test)]
mod tests;
