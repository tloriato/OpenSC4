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

//! Garbage prop shares over inclusive cell rectangles.

use super::*;

game_data! { static prop_curve: *mut u8 = 0x00B4_9BAC; }
game_data! { static prop_scale: f32 = 0x00AB_25E4; }
game_fn! {
    #[allow(dead_code)]
    fn curve_output(this: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250;
}

layout! {
    /// Inclusive cell rectangle supplied to the prop requester.
    struct Rectangle {
        0x00 => x0 / set_x0: i32,
        0x04 => z0 / set_z0: i32,
        0x08 => x1 / set_x1: i32,
        0x0C => z1 / set_z1: i32,
    }
}

port! {
    /// Sums garbage x before z, divides the wrapping unsigned sum by the wrapping rectangle area, and
    /// evaluates the prop curve on the scaled quotient. The result uses the float-bias rounding formula.
    /// Bounds are reread after calls, and inverted rectangles still reach division and the curve.
    #[port(address = 0x006E_3BC0, owner = "pollution", name = "cSC4PollutionSimulator::GetRequestedShare")]
    pub unsafe extern "thiscall" fn fc4p_006e3bc0(this: *mut u8, rectangle: *mut u8) -> u64 {
        // SAFETY: the prop-requester interface lies eight bytes into its simulator, and the rectangle is live.
        unsafe {
            let rectangle = Rectangle::from_ptr(rectangle);
            let primary = this.wrapping_sub(8);
            let mut sum = 0u32;
            let mut value = 0u32;
            let mut x = rectangle.x0();
            while x <= rectangle.x1() {
                let mut z = rectangle.z0();
                while z <= rectangle.z1() {
                    vcall!(primary, 0x24 => fn(i32, i32, *mut u32), x, z, &mut value);
                    sum = sum.wrapping_add(value);
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            let height = rectangle.z1().wrapping_sub(rectangle.z0()).wrapping_add(1);
            let width = rectangle.x1().wrapping_sub(rectangle.x0()).wrapping_add(1);
            let area = (height as u32).wrapping_mul(width as u32);
            let quotient: u32;
            let remainder: u32;
            // Retain the hardware divide-error behavior of a zero area.
            core::arch::asm!("xor edx, edx", "div {area:e}", area = in(reg) area,
                inlateout("eax") sum => quotient, out("edx") remainder, options(nomem, nostack));
            let signed = F80::from_i32(quotient as i32);
            let average = if (quotient as i32) < 0 { signed + F80::from_f32(4_294_967_296.0) } else { signed };
            let input = (average * F80::from_f32(*prop_scale())).to_f32();
            let args = [input.to_bits()];
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x005F_4250), *prop_curve() as u32, &args);
            call.eax = input.to_bits();
            call.edx = remainder;
            call.take_st0 = 1;
            call_raw(&mut call);
            let bits = (call.st0 + F80::from_f32(12_582_912.0)).to_f32().to_bits();
            registers(bits.wrapping_add(0xB4C0_0000), call.edx)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e3bc0::PORT];

#[cfg(test)]
mod tests;
