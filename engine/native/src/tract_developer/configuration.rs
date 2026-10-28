// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Matching a lot configuration to one candidate site.

use super::*;
use crate::x87::F80;

game_data! { static fire: u32 = 0x00B4_3D78; }
game_data! { static aura: u32 = 0x00B4_3D60; }
game_fn! { #[allow(dead_code)] fn capacity(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_55B0; }
game_fn! { #[allow(dead_code)] fn road_bit(bits: u32, output: u32, index: u32) -> u32 = "thiscall" 0x0047_7A40; }
game_fn! { #[allow(dead_code)] fn corner(this: u32, configuration: u32, candidate: u32) -> u32 = "thiscall" 0x0070_5DA0; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }

unsafe fn outcome<const N: usize>(r: &mut Entry<N>, this: u32, code: u32, register: u8) {
    // SAFETY: the attempt's debug context supplies the configuration-compatibility histogram.
    unsafe {
        let context = word(this, 0x170);
        match register { 0 => r.eax = context, 1 => r.ecx = context, _ => r.edx = context }
        r.eax = word(context, 0x84); r.ecx = word(r.eax, code * 4).wrapping_add(1);
        store(r.eax, code * 4, r.ecx); r.eax = code;
    }
}

unsafe fn float_call<const N: usize>(r: &mut Entry<N>, target: usize, args: &[u32]) -> F80 {
    let mut c = RawCall::new(Conv::Thiscall, target, r.ecx, args);
    c.eax = r.eax; c.edx = r.edx; c.take_st0 = 1;
    // SAFETY: each caller identifies a virtual method returning one value in ST0.
    unsafe { crate::abi::call_raw(&mut c); }
    r.eax = c.eax; r.edx = c.edx;
    c.st0
}

register_port! {
    /// Check exact dimensions, zone, growth stage, slope, service coverage and required roads.
    /// Dimension mismatches count as outcome five but return zero. NaN slope bounds pass.
    /// Road failures increment their counter before freeing the temporary bit vector.
    0x0070_6500, fc4p_00706500, compatible, 3, 12, "cSC4TractDeveloper::IsLotConfigurationCompatible", |r| {
        let this = r.ecx;
        let [configuration, candidate, require_roads] = r.args;
        let x = word(candidate, 8); let z = word(candidate, 0xC);
        r.eax = candidate;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_55B0), &[r.eax]);
        r.eax = word(configuration, 0);
        let mut size = [0u32; 2];
        r.ecx = size.as_mut_ptr().wrapping_add(1) as u32;
        let depth = r.ecx; r.edx = size.as_mut_ptr() as u32; r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[r.edx, depth]);
        r.eax = candidate; r.ecx = u32::from(*(candidate.wrapping_add(0x14) as *const u8));
        if size[0] != r.ecx { outcome(r, this, 5, 1); r.eax = 0; return; }
        r.edx = u32::from(*(candidate.wrapping_add(0x15) as *const u8));
        if size[1] != r.edx { outcome(r, this, 5, 1); r.eax = 0; return; }
        r.ecx = word(this, 0x3C); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x30) as usize, &[x, z]);
        r.edx = word(configuration, 0); r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x3C) as usize, &[r.eax]);
        if r.eax as u8 == 0 { outcome(r, this, 6, 0); return; }
        r.edx = word(configuration, 0); r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
        let stage = r.eax as u8;
        r.ecx = candidate; r.edx = word(r.ecx, 0x30); r.eax = word(configuration, 0); r.ecx = configuration;
        invoke(r, Conv::Thiscall, word(r.eax, 0x40) as usize, &[r.edx]);
        let matching_wealth = r.eax as u8 != 0;
        r.eax = candidate;
        let stage_ok = if matching_wealth {
            r.ecx = u32::from(stage); let result = (r.ecx as i32) > word(candidate, 0x2C) as i32;
            r.ecx = (r.ecx & !0xFF) | u32::from(result); result
        } else {
            r.edx = u32::from(stage); r.ecx = word(candidate, 0x2C);
            let result = (r.edx as i32) >= r.ecx as i32;
            r.ecx = (r.ecx & !0xFF) | u32::from(result); result
        };
        if !stage_ok { outcome(r, this, 12, 1); return; }
        r.ecx = word(r.eax, 4);
        let slope = if r.ecx != 0 {
            r.edx = word(r.ecx, 0);
            float_call(r, word(r.edx, 0x48) as usize, &[]).to_f32()
        } else {
            r.ecx = word(this, 0x28); r.edx = word(r.ecx, 0); r.eax = r.eax.wrapping_add(0x38);
            float_call(r, word(r.edx, 0x74) as usize, &[r.eax, 0]).to_f32()
        };
        r.eax = word(configuration, 0); r.ecx = configuration;
        let minimum = float_call(r, word(r.eax, 0x24) as usize, &[]).to_f32();
        r.edx = word(configuration, 0); r.ecx = configuration;
        let maximum = float_call(r, word(r.edx, 0x20) as usize, &[]);
        let low = F80::from_f32(slope).compare(F80::from_f32(minimum));
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(low.status_word());
        if low.less() { outcome(r, this, 10, 0); return; }
        let high = F80::from_f32(slope).compare(maximum);
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(high.status_word());
        if !high.less() && !high.equal() && !high.unordered() { outcome(r, this, 10, 0); return; }
        r.ecx = word(this, 0x1C); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0xE0) as usize, &[]);
        if r.eax as u8 == 0 {
            r.ecx = word(this, 0x80); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x4C) as usize, &[]);
            if stage > r.eax as u8 {
                r.ecx = word(this, 0x1C); r.eax = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.eax, 0xDC) as usize, &[]);
                if r.eax as u8 == 0 {
                    r.ecx = word(this, 0x34); r.edx = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0x8C) as usize, &[]);
                    r.edx = word(r.eax, 0); r.ecx = r.eax;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
                    if (r.eax as u8) < 2 { outcome(r, this, 26, 0); return; }
                }
            }
            r.ecx = word(this, 0x80); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x50) as usize, &[]);
            if stage > r.eax as u8 {
                r.ecx = word(this, 0x1C); r.eax = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.eax, 0xD8) as usize, &[]);
                if r.eax as u8 == 0 {
                    r.ecx = word(this, 0x30); r.edx = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[x, z]);
                    if r.eax as u8 == 0 { outcome(r, this, 25, 0); return; }
                }
            }
            r.ecx = word(this, 0x80); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[]);
            if stage > r.eax as u8 {
                r.ecx = read_unaligned(fire()); r.eax = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.eax, 0x14) as usize, &[]);
                r.edx = word(r.eax, 0); r.ecx = r.eax;
                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
                if r.eax as u8 == 0 { outcome(r, this, 23, 0); return; }
            }
            r.ecx = word(this, 0x80); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[]);
            if stage > r.eax as u8 {
                r.ecx = read_unaligned(aura()); r.eax = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.eax, 0x38) as usize, &[]);
                r.edx = word(r.eax, 0); r.ecx = r.eax;
                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
                if r.eax as u16 == 0 { outcome(r, this, 24, 0); return; }
            }
        }
        r.ecx = word(this, 0x80); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x14) as usize, &[]);
        if r.eax != 3 {
            let mut bits = [0u32; 5];
            r.eax = word(configuration, 0); r.ecx = bits.as_mut_ptr() as u32;
            let output = r.ecx; r.ecx = configuration;
            invoke(r, Conv::Thiscall, word(r.eax, 0x54) as usize, &[output]);
            let mut has_roads = false;
            for side in 0..3 {
                let mut bit = [0u32; 2];
                r.edx = bit.as_mut_ptr() as u32; r.ecx = bits.as_mut_ptr() as u32;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0047_7A40), &[r.edx, side]);
                r.ecx = word(r.eax, 0); r.edx = word(r.eax, 4);
                if word(r.ecx, 0) & r.edx != 0 { has_roads = true; break; }
            }
            r.eax = (r.eax & !0xFF) | u32::from(require_roads as u8);
            let accepted = if require_roads as u8 != 0 && !has_roads { false } else {
                r.ecx = this;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5DA0), &[configuration, candidate]);
                r.eax as u8 != 0
            };
            if !accepted {
                if require_roads as u8 != 0 && !has_roads { r.eax = word(this, 0x170); }
                else { r.ecx = word(this, 0x170); }
                r.eax = word(word(this, 0x170), 0x84);
                store(r.eax, 0x54, word(r.eax, 0x54).wrapping_add(1));
                r.eax = bits[0];
                if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
                r.eax = 21; return;
            }
            if has_roads { *(candidate.wrapping_add(0xA0) as *mut u8) = 1; }
            r.eax = bits[0];
            if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
        }
        outcome(r, this, 27, 2);
    }
}

pub const PORTS: &[Port] = &[fc4p_00706500::PORT];

#[cfg(test)]
mod tests;
