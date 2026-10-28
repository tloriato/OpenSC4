// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration filtering and required-road checks for corner lots.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn free_required_roads(storage: *mut u8) -> u32 = "C" 0x0090_CF63;
}

/// Increment exactly one histogram counter and retain the register used to reach the row.
unsafe fn outcome<const N: usize>(r: &mut Entry<N>, td: TractDeveloper, code: u32, context_register: u8) {
    // SAFETY: the attempt's debug context holds its live counter row at +0x54.
    unsafe {
        let context = td.debug_context();
        match context_register {
            1 => {
                r.ecx = context;
                r.eax = word(r.ecx, 0x54);
            }
            2 => {
                r.edx = context;
                r.eax = word(r.edx, 0x54);
            }
            _ => {
                r.eax = context;
                r.eax = word(r.eax, 0x54);
            }
        }
        r.ecx = word(r.eax, code * 4).wrapping_add(1);
        store(r.eax, code * 4, r.ecx);
        r.eax = code;
    }
}

register_port! {
    /// Filter purpose, wealth, stage and style, then building count and capacity. Every result increments
    /// its outcome histogram counter. Stage and capacity use only their low byte and word respectively;
    /// comparisons against the attempt's demand fields are signed and the capacity threshold wraps.
    0x0070_4C00, fc4p_00704c00, configuration_suitable, 1, 4, "cSC4TractDeveloper::IsLotConfigurationSuitable", |r| {
        let td = TractDeveloper::from_ptr(r.ecx as *mut u8);
        let configuration = r.args[0];
        r.ecx = td.developer();
        r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
        let purpose = r.eax;
        r.edx = word(configuration, 0);
        r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x44) as usize, &[purpose]);
        if r.eax as u8 == 0 { outcome(r, td, 7, 0); return; }
        r.ecx = td.developer();
        r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[]);
        r.edx = word(configuration, 0);
        r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x40) as usize, &[r.eax]);
        if r.eax as u8 == 0 { outcome(r, td, 8, 0); return; }
        r.edx = word(configuration, 0);
        r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
        let stage = r.eax as u8;
        r.ecx = td.maximum_stage() as u32;
        r.eax = u32::from(stage);
        if r.eax as i32 > r.ecx as i32 { outcome(r, td, 14, 1); return; }
        r.edx = td.styles_begin();
        if r.edx != td.styles_end() && (1..=3).contains(&(purpose as i32)) {
            r.eax = (r.eax & !0xFF) | u32::from(td.single_style());
            if r.eax as u8 != 0 {
                r.eax = td.style_index() as u32;
                r.ecx = r.edx;
                r.eax = word(r.ecx, r.eax.wrapping_mul(4));
                r.edx = word(configuration, 0);
                r.ecx = configuration;
                invoke(r, Conv::Thiscall, word(r.edx, 0x48) as usize, &[r.eax]);
                if r.eax as u8 == 0 { outcome(r, td, 9, 0); return; }
            } else {
                let mut style = td.styles_begin();
                let mut matched = false;
                if style != td.styles_end() {
                    loop {
                        r.eax = word(style, 0);
                        r.edx = word(configuration, 0);
                        r.ecx = configuration;
                        invoke(r, Conv::Thiscall, word(r.edx, 0x48) as usize, &[r.eax]);
                        if r.eax as u8 != 0 { matched = true; break; }
                        r.eax = td.styles_end();
                        style = style.wrapping_add(4);
                        if style == r.eax { break; }
                    }
                }
                if !matched { outcome(r, td, 9, 0); return; }
            }
        }
        r.edx = word(configuration, 0);
        r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x60) as usize, &[]);
        if r.eax == 0 { outcome(r, td, 0, 0); return; }
        let minimum = if stage == 1 { 0 } else {
            r.eax = word(configuration, 0);
            r.ecx = configuration;
            invoke(r, Conv::Thiscall, word(r.eax, 0x30) as usize, &[]);
            r.eax
        };
        r.edx = word(configuration, 0);
        r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x34) as usize, &[]);
        r.ecx = (r.ecx & !0xFF) | u32::from(td.relaxed());
        if r.ecx as u8 == 0 {
            r.ecx = td.desired() as u32;
            r.eax &= 0xFFFF;
            if (r.eax as i32) < r.ecx as i32 {
                r.edx = word(configuration, 0);
                r.ecx = configuration;
                invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                r.ecx = td.maximum_stage() as u32;
                r.eax &= 0xFF;
                if (r.eax as i32) < r.ecx as i32 { outcome(r, td, 11, 1); return; }
            }
        }
        r.edx = td.desired_maximum() as u32;
        r.eax = minimum & 0xFFFF;
        r.edx = r.edx.wrapping_add(td.existing() as u32);
        if r.eax as i32 <= r.edx as i32 { outcome(r, td, 27, 2); }
        else { outcome(r, td, 13, 1); }
    }
}

register_port! {
    /// Query the four required-road bits, test avenue, road and street counts on the rotated next side,
    /// then free the returned bit storage. Any nonzero network count satisfies a required side.
    0x0070_5DA0, fc4p_00705da0, corner_lot, 2, 8, "cSC4TractDeveloper::TestForCornerLot", |r| {
        let mut bits = [0u32; 5];
        r.ecx = r.args[0];
        r.eax = word(r.ecx, 0);
        r.edx = bits.as_mut_ptr() as u32;
        invoke(r, Conv::Thiscall, word(r.eax, 0x54) as usize, &[r.edx]);
        let candidate = r.args[1];
        let storage = bits[0];
        let mut accepted = true;
        for side in 0..4u32 {
            r.eax = bits[1];
            r.ecx = r.eax.wrapping_add(side);
            r.eax = r.ecx;
            r.edx = if (r.eax as i32) < 0 { 31 } else { 0 };
            r.eax = (r.eax.wrapping_add(r.edx) as i32 >> 5) as u32;
            r.ecx &= 0x8000_001F;
            r.eax = storage.wrapping_add(r.eax.wrapping_mul(4));
            if (r.ecx as i32) < 0 {
                r.ecx = (r.ecx.wrapping_sub(1) | 0xFFFF_FFE0).wrapping_add(1);
                if (r.ecx as i32) < 0 {
                    r.ecx = r.ecx.wrapping_add(32);
                    r.eax = r.eax.wrapping_sub(4);
                }
            }
            r.edx = r.eax;
            r.eax = 1u32.wrapping_shl(r.ecx);
            if word(r.edx, 0) & r.eax != 0 {
                r.eax = word(candidate, 0x10).wrapping_add(side).wrapping_add(1);
                if (r.eax as i32) < 0 {
                    r.ecx = u32::MAX.wrapping_sub(r.eax) >> 2;
                    r.eax = r.eax.wrapping_add(r.ecx.wrapping_mul(4)).wrapping_add(4);
                }
                if r.eax as i32 > 3 {
                    r.edx = (r.eax.wrapping_sub(4) >> 2) << 2;
                    r.ecx = 0xFFFF_FFFCu32.wrapping_sub(r.edx);
                    r.eax = r.eax.wrapping_add(r.ecx);
                }
                r.edx = word(candidate, 0x88);
                if word(r.edx, r.eax.wrapping_mul(4)) == 0 {
                    r.ecx = word(candidate, 0x7C);
                    if word(r.ecx, r.eax.wrapping_mul(4)) == 0 {
                        r.edx = word(candidate, 0x70);
                        if word(r.edx, r.eax.wrapping_mul(4)) == 0 { accepted = false; break; }
                    }
                }
            }
        }
        if storage != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[storage]); }
        r.eax = (r.eax & !0xFF) | u32::from(accepted);
    }
}

pub const PORTS: &[Port] = &[fc4p_00704c00::PORT, fc4p_00705da0::PORT];

#[cfg(test)]
mod tests;
