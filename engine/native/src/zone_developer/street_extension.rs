// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Extend dead ends across the current rectangle.

use super::Port;
use core::ptr::read_unaligned;

game_data! { static network_flags: u32 = 0x00AB_3680; }
game_fn! {
    fn connection(this: *mut u8, x: i32, z: i32, flags: u32, side: i32, kind: u32) -> u32
        = "thiscall" 0x0072_FE80;
}
game_fn! {
    fn draw_street(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, per_cell: u32)
        = "thiscall" 0x0072_A2A0;
}

port! {
    /// Scan x before z, count connected sides in order, and stop at two. A single connected side
    /// draws to its opposite rectangle edge, unless already there. Avenue connections count only
    /// when x + z is odd. Bounds are read again after calls, so new streets affect later cells.
    #[port(address = 0x0073_0BA0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::ExtendAsStreets")]
    pub unsafe extern "thiscall" fn fc4p_00730ba0(this: *mut u8, rect: *const i32, kind: i32) {
        // SAFETY: the rectangle has four inclusive coordinates and the game owns the flag table.
        unsafe {
            let flags = read_unaligned(network_flags().wrapping_offset(kind as isize));
            let mut x = read_unaligned(rect);
            while x <= read_unaligned(rect.add(2)) {
                let mut z = read_unaligned(rect.add(1));
                let mut parity = x.wrapping_add(z);
                while z <= read_unaligned(rect.add(3)) {
                    if vcall!(this, 0x20 => fn(i32, i32, u32) -> u32, x, z, flags) as u8 != 0 {
                        let mut count = 0;
                        let mut dead_side = -1;
                        for side in 0..4 {
                            if connection(this, x, z, flags, side, 0) as u8 != 0
                                || connection(this, x, z, flags, side, 10) as u8 != 0
                                || connection(this, x, z, flags, side, 3) as u8 != 0
                                || connection(this, x, z, flags, side, 11) as u8 != 0
                                || (parity & 1 != 0 && connection(this, x, z, flags, side, 6) as u8 != 0)
                            {
                                count += 1;
                                if count > 1 { break; }
                                dead_side = side;
                            }
                        }
                        if count == 1 {
                            let (end_x, end_z) = match dead_side {
                                0 => (x, read_unaligned(rect.add(3))),
                                1 => (read_unaligned(rect), z),
                                2 => (x, read_unaligned(rect.add(1))),
                                3 => (read_unaligned(rect.add(2)), z),
                                _ => (x, z),
                            };
                            if (x, z) != (end_x, end_z) {
                                draw_street(this, x, z, end_x, end_z, 0);
                            }
                        }
                    }
                    z = z.wrapping_add(1);
                    parity = parity.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00730ba0::PORT];

#[cfg(test)]
mod tests;
