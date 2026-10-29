// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Claiming resources and starting construction on selected candidates.

use super::*;

game_data! { static demolition: u32 = 0x00B4_3D00; }
game_fn! { #[allow(dead_code)] fn pick_building(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_9BA0; }
game_fn! { #[allow(dead_code)] fn subdivide(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_A9B0; }
game_fn! { #[allow(dead_code)] fn aggregate(this: u32, candidate: u32, output: u32) -> u32 = "thiscall" 0x0070_68E0; }
game_fn! { #[allow(dead_code)] fn find_cache(this: u32, output: u32, key: u32) -> u32 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn erase_cache(this: u32, iterator: u32) -> u32 = "thiscall" 0x0071_08D0; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn append_lot(this: u32, lot: u32) -> u32 = "thiscall" 0x007E_98C0; }
game_fn! { #[allow(dead_code)] fn clear_list(this: u32) -> u32 = "thiscall" 0x0047_3EC0; }

register_port! {
    /// Select a building and claim its capacity. Subdivision releases the claim before delegating.
    /// Existing sites are retained, removed from the cache and demolished; aggregated sites delegate
    /// their replacement. Construction's result is ignored. Failed lot creation keeps the claim.
    0x0070_B440, fc4p_0070b440, build, 1, 4, "cSC4TractDeveloper::Build", |r| {
        let this = r.ecx;
        let candidate = r.args[0];
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let mut lot = 0u32;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_9BA0), &[candidate]);
        let outcome = r.eax;
        if outcome != 27 { return; }
        r.ecx = td.building_development(); r.edx = td.chosen_building(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x9C) as usize, &[r.edx]);
        let capacity = u32::from(read_unaligned(r.eax.wrapping_add(8) as *const u16));
        r.ecx = td.building_development(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0xC8) as usize, &[capacity]);
        if r.eax as u8 == 0 {
            r.edx = td.debug_context(); r.eax = word(r.edx, 0x48);
            r.ecx = word(r.eax, 0x50).wrapping_add(1); store(r.eax, 0x50, r.ecx);
            r.eax = 20;
            return;
        }
        r.eax = word(candidate, 0).wrapping_sub(2);
        if r.eax == 0 {
            r.ecx = word(candidate, 4); lot = r.ecx; r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 4) as usize, &[]);
            r.eax = &mut lot as *mut u32 as u32;
            let key = r.eax;
            let mut found = 0u32;
            r.ecx = &mut found as *mut u32 as u32;
            let output = r.ecx;
            let cache = this.wrapping_add(0x10C);
            r.ecx = cache;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7520), &[output, key]);
            r.eax = found;
            if r.eax != word(cache, 0) {
                r.ecx = cache;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0071_08D0), &[r.eax]);
            }
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0xC]);
            store(r.eax, 0, r.eax); store(r.eax, 4, r.eax);
            r.ecx = lot;
            let retained = r.ecx;
            let mut list = r.eax;
            let mut value = retained;
            if r.ecx != 0 {
                r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 4) as usize, &[]);
            }
            r.eax = &mut value as *mut u32 as u32;
            r.ecx = &mut list as *mut u32 as u32;
            invoke(r, Conv::Thiscall, crate::abi::target(0x007E_98C0), &[r.eax]);
            if retained != 0 {
                r.edx = word(retained, 0); r.ecx = retained;
                invoke(r, Conv::Thiscall, word(r.edx, 8) as usize, &[]);
            }
            r.ecx = read_unaligned(demolition()); r.eax = word(r.ecx, 0);
            r.edx = &mut list as *mut u32 as u32;
            invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[1, r.edx, 0, 0x800, 0, 0, 0, 0]);
            r.eax = word(candidate, 0x24); r.edx = td.built() as u32;
            r.edx = r.edx.wrapping_sub(r.eax);
            r.ecx = &mut list as *mut u32 as u32;
            td.set_built(r.edx as i32);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0047_3EC0), &[]);
            r.eax = list;
            if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
        } else {
            r.eax = r.eax.wrapping_sub(1);
            if r.eax == 0 {
                r.eax = &mut lot as *mut u32 as u32; r.ecx = this;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_68E0), &[candidate, r.eax]);
            } else {
                r.eax = r.eax.wrapping_sub(1);
                if r.eax == 0 {
                    r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0xCC) as usize, &[capacity]);
                    r.ecx = this;
                    invoke(r, Conv::Thiscall, crate::abi::target(0x0070_A9B0), &[candidate]);
                    r.eax = 27;
                    return;
                }
            }
        }
        r.eax = lot;
        if r.eax != 0 {
            let building = td.chosen_building();
            r.ecx = td.lot_developer(); r.edx = word(r.ecx, 0);
            let configuration = td.chosen_configuration();
            invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[r.eax, configuration, building, 1]);
            r.eax = td.built() as u32; r.ecx = lot;
            r.eax = r.eax.wrapping_add(capacity); td.set_built(r.eax as i32);
            r.eax = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
            r.eax = outcome;
        } else {
            r.ecx = td.debug_context(); r.eax = word(r.ecx, 0x48);
            r.ecx = word(r.eax, 0).wrapping_add(1); store(r.eax, 0, r.ecx);
            r.eax = 0;
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0070b440::PORT];

#[cfg(test)]
mod tests;
