// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Aggregation bounds, zone consistency and whole-lot containment.

use super::*;

game_fn! { #[allow(dead_code)] fn bounding_rect(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_4630; }
game_fn! { #[allow(dead_code)] fn cache_find(this: u32, result: u32, key: u32) -> u32 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn insert_lot(this: u32, result: u32, lot: u32) -> u32 = "thiscall" 0x0049_A050; }
game_fn! { #[allow(dead_code)] fn next_lot(this: u32) -> u32 = "thiscall" 0x005D_17C0; }

unsafe fn count<const N: usize>(r: &mut Entry<N>, this: u32, code: u32, context_register: u8, load: bool) {
    // SAFETY: the attempt's debug context has its aggregation-legality row at +0x6C.
    unsafe {
        let context = word(this, 0x170);
        match context_register {
            1 => r.ecx = context,
            2 => r.edx = context,
            _ => r.eax = context,
        }
        r.eax = word(context, 0x6C);
        let value = word(r.eax, code.wrapping_mul(4)).wrapping_add(1);
        if load {
            r.ecx = value;
        }
        store(r.eax, code.wrapping_mul(4), value);
    }
}

#[cfg(test)]
mod tests;

register_port! {
    /// Require every cell to have a suitable lot (road failure is allowed), one matching facing,
    /// a common zone, and at least two distinct whole lots contained within the candidate bounds.
    /// Keep already inserted lots on failure. During the final containment pass the lot iterator
    /// overwrites the original candidate stack argument. Only AL is the boolean result.
    0x0070_8700, fc4p_00708700, aggregation_legal, 1, 4, "cSC4TractDeveloper::IsAggregationLegal", |r| {
        let this = r.ecx; let candidate = r.args[0];
        let mut common_zone = 0u32; let mut matching_facing = false;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_4630), &[candidate]);
        r.eax = word(candidate, 0x38);
        if (r.eax as i32) < 0 { r.eax &= !0xFF; return; }
        r.eax = word(candidate, 0x3C);
        if (r.eax as i32) < 0 { r.eax &= !0xFF; return; }
        r.ecx = word(this, 0x18); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x174) as usize, &[]);
        if word(candidate, 0x40) as i32 >= r.eax as i32 { r.eax &= !0xFF; return; }
        r.ecx = word(this, 0x18); r.edx = word(r.ecx, 0);
        let mut max_z = word(candidate, 0x44);
        invoke(r, Conv::Thiscall, word(r.edx, 0x178) as usize, &[]);
        if max_z as i32 >= r.eax as i32 { r.eax &= !0xFF; return; }
        r.edx = word(candidate, 0x38); let mut x = r.edx;
        if x as i32 <= word(candidate, 0x40) as i32 {
            loop {
                r.eax = word(candidate, 0x3C); let mut z = r.eax;
                if z as i32 <= max_z as i32 {
                    loop {
                        r.eax = z; r.ecx = word(this, 0x2C); let table = word(r.ecx, 0);
                        invoke(r, Conv::Thiscall, word(table, 0x18) as usize, &[r.edx, r.eax, 0]);
                        let lot = r.eax;
                        if lot == 0 { r.eax &= !0xFF; return; }
                        let mut key = lot; let mut found = 0u32;
                        r.eax = &mut key as *mut u32 as u32;
                        r.ecx = this.wrapping_add(0x10C);
                        invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7520), &[&mut found as *mut u32 as u32, r.eax]);
                        r.eax = found;
                        let outcome = if r.eax != word(this, 0x10C) { word(r.eax, 0x14) }
                            else { eligibility::check(r, this, lot, &mut key as *mut u32 as u32, false) };
                        if outcome == 27 {
                            r.edx = word(lot, 0); r.ecx = lot;
                            invoke(r, Conv::Thiscall, word(r.edx, 0x5C) as usize, &[]);
                            if r.eax == word(candidate, 0x10) { matching_facing = true; }
                        } else if outcome != 21 { r.eax &= !0xFF; return; }
                        r.eax = common_zone;
                        if r.eax == 0 {
                            r.eax = word(lot, 0); r.ecx = lot;
                            invoke(r, Conv::Thiscall, word(r.eax, 0x7C) as usize, &[]);
                            common_zone = r.eax;
                        }
                        r.edx = word(lot, 0); r.ecx = lot;
                        invoke(r, Conv::Thiscall, word(r.edx, 0x7C) as usize, &[]);
                        if r.eax != common_zone {
                            count(r, this, 6, 2, true); r.eax &= !0xFF; return;
                        }
                        let mut lot_word = lot; let mut inserted = [0u32; 2];
                        r.eax = &mut lot_word as *mut u32 as u32;
                        r.ecx = inserted.as_mut_ptr() as u32; let result = r.ecx;
                        r.ecx = candidate.wrapping_add(0x18);
                        invoke(r, Conv::Thiscall, crate::abi::target(0x0049_A050), &[result, r.eax]);
                        r.eax = z; max_z = word(candidate, 0x44); r.edx = x;
                        r.eax = r.eax.wrapping_add(1); z = r.eax;
                        if z as i32 > max_z as i32 { break; }
                    }
                }
                r.eax = word(candidate, 0x40); r.edx = r.edx.wrapping_add(1); x = r.edx;
                if x as i32 > r.eax as i32 { break; }
            }
            r.eax = (r.eax & !0xFF) | u32::from(matching_facing);
        }
        if !matching_facing { count(r, this, 21, 0, true); r.eax &= !0xFF; return; }
        if word(candidate, 0x1C) < 2 { count(r, this, 3, 1, false); r.eax &= !0xFF; return; }
        r.ecx = word(candidate, 0x18); r.eax = word(r.ecx, 8); r.args[0] = r.eax;
        if r.eax != r.ecx {
            loop {
                r.ecx = word(r.eax, 0x10); r.edx = word(r.ecx, 0);
                let mut rectangle = [0u32; 4]; r.eax = rectangle.as_mut_ptr() as u32;
                invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax]);
                r.ecx = rectangle[1]; let mut outside = (r.ecx as i32) < word(candidate, 0x3C) as i32;
                if !outside { r.edx = rectangle[3]; outside = r.edx as i32 > word(candidate, 0x44) as i32; }
                if !outside { r.eax = rectangle[0]; outside = (r.eax as i32) < word(candidate, 0x38) as i32; }
                if !outside { r.ecx = rectangle[2]; outside = r.ecx as i32 > word(candidate, 0x40) as i32; }
                if outside { count(r, this, 3, 2, true); r.eax &= !0xFF; return; }
                r.ecx = &mut r.args[0] as *mut u32 as u32;
                invoke(r, Conv::Thiscall, crate::abi::target(0x005D_17C0), &[]);
                r.eax = r.args[0];
                if r.eax == word(candidate, 0x18) { break; }
            }
        }
        count(r, this, 27, 0, true); r.eax = (r.eax & !0xFF) | 1;
    }
}
