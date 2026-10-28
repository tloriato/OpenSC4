// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cached candidate bounds and the ascending score comparator.

use super::*;
use crate::x87::F80;

register_port! {
    /// Keeps a wholly nonnegative cached rectangle. Otherwise facing -1 asks the lot for facing,
    /// size, location and bounds in that order. Facings zero through three compute inclusive bounds;
    /// every other facing leaves the rectangle untouched.
    0x0070_4630, fc4p_00704630, bounding_rect, 1, 4, "cSC4TractDeveloper::GetBoundingRect", |r| {
        let candidate = r.args[0];
        let c = Candidate::from_ptr(candidate as *mut u8);
        r.eax = c.min_x() as u32;
        if r.eax as i32 >= 0 {
            r.eax = c.min_z() as u32;
            if r.eax as i32 >= 0 {
                r.eax = c.max_x() as u32;
                if r.eax as i32 >= 0 {
                    r.eax = c.max_z() as u32;
                    if r.eax as i32 >= 0 { return; }
                }
            }
        }
        r.eax = c.facing() as u32;
        if r.eax == u32::MAX {
            r.ecx = c.lot() as u32;
            r.eax = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.eax, 0x5C) as usize, &[]);
            r.ecx = c.lot() as u32;
            c.set_facing(r.eax as i32);
            r.edx = word(r.ecx, 0);
            r.eax = candidate.wrapping_add(0x14);
            invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize,
                &[r.eax, candidate.wrapping_add(0x15)]);
            r.ecx = c.lot() as u32;
            r.edx = word(r.ecx, 0);
            r.eax = candidate.wrapping_add(8);
            invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize,
                &[r.eax, candidate.wrapping_add(0xC)]);
            r.ecx = c.lot() as u32;
            r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize,
                &[candidate.wrapping_add(0x38)]);
            return;
        }
        match r.eax {
            0 => {
                r.eax = u32::from(c.width());
                r.ecx = (c.x() as u32).wrapping_sub(r.eax).wrapping_add(1);
                c.set_min_x(r.ecx as i32);
                r.edx = c.x() as u32;
                c.set_max_x(r.edx as i32);
                r.eax = c.z() as u32;
                c.set_min_z(r.eax as i32);
                r.ecx = u32::from(c.depth());
                r.edx = c.z() as u32;
                r.eax = r.ecx.wrapping_add(r.edx).wrapping_sub(1);
                c.set_max_z(r.eax as i32);
            }
            1 => {
                r.ecx = u32::from(c.depth());
                r.edx = (c.x() as u32).wrapping_sub(r.ecx).wrapping_add(1);
                c.set_min_x(r.edx as i32);
                r.eax = c.x() as u32;
                c.set_max_x(r.eax as i32);
                r.ecx = u32::from(c.width());
                r.edx = (c.z() as u32).wrapping_sub(r.ecx).wrapping_add(1);
                c.set_min_z(r.edx as i32);
                r.eax = c.z() as u32;
                c.set_max_z(r.eax as i32);
            }
            2 => {
                r.ecx = c.x() as u32;
                c.set_min_x(r.ecx as i32);
                r.edx = u32::from(c.width());
                r.eax = c.x() as u32;
                r.ecx = r.edx.wrapping_add(r.eax).wrapping_sub(1);
                c.set_max_x(r.ecx as i32);
                r.edx = u32::from(c.depth());
                r.eax = (c.z() as u32).wrapping_sub(r.edx).wrapping_add(1);
                c.set_min_z(r.eax as i32);
                r.ecx = c.z() as u32;
                c.set_max_z(r.ecx as i32);
            }
            3 => {
                r.edx = c.x() as u32;
                c.set_min_x(r.edx as i32);
                r.eax = u32::from(c.depth());
                r.ecx = c.x() as u32;
                r.edx = r.eax.wrapping_add(r.ecx).wrapping_sub(1);
                c.set_max_x(r.edx as i32);
                r.eax = c.z() as u32;
                c.set_min_z(r.eax as i32);
                r.ecx = u32::from(c.width());
                r.edx = c.z() as u32;
                r.eax = r.ecx.wrapping_add(r.edx).wrapping_sub(1);
                c.set_max_z(r.eax as i32);
            }
            _ => {}
        }
    }
}

register_port! {
    /// Compares candidate scores on the x87, with unordered values comparing false.
    /// ECX receives the second candidate; EDX retains its entry value.
    0x0070_4770, fc4p_00704770, compare_score, 2, 0, "CandidateScoreLess", |r| {
        r.ecx = r.args[1];
        let score = F80::from_f32(read_unaligned(r.args[0].wrapping_add(0x34) as *const f32));
        let bytes = score.to_bytes();
        let status: u16;
        // SAFETY: both candidates have a score at +0x34. Retain the stored operand's role so a
        // signaling NaN is first consumed by the comparison. One x87 register is pushed and popped.
        core::arch::asm!(
            "fld tbyte ptr [{score}]", "fcomp dword ptr [{other}]", "fnstsw ax",
            score = in(reg) bytes.as_ptr(), other = in(reg) r.ecx.wrapping_add(0x34),
            out("ax") status, out("st(0)") _,
        );
        r.eax = u32::from(status & 0x0500 == 0x0100);
    }
}

pub const PORTS: &[Port] = &[fc4p_00704630::PORT, fc4p_00704770::PORT];

#[cfg(test)]
mod tests;
