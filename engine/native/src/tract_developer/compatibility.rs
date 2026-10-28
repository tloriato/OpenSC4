// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Building and existing-lot eligibility, including the per-pass lot cache and outcome counters.

use super::*;

game_data! { static resource_manager: u32 = 0x00B4_3CA8; }
game_fn! { #[allow(dead_code)] fn occupant_group(key: u32, group: u32) -> u32 = "C" 0x005E_8F70; }
game_fn! { #[allow(dead_code)] fn property_bool(holder: u32, id: u32, output: u32) -> u32 = "C" 0x005F_D390; }
game_fn! { #[allow(dead_code)] fn cache_find(tree: u32, output: u32, key: u32) -> u32 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn cache_value(tree: u32, key: u32) -> u32 = "thiscall" 0x0049_7A00; }
game_fn! { #[allow(dead_code)] fn fits_zone(target: u32, zone: i32) -> u32 = "C" 0x0069_5390; }

/// Record a building outcome, including the temporary registers used to reach its histogram row.
unsafe fn building_outcome<const N: usize>(r: &mut Entry<N>, this: u32, outcome: u32, context_register: u32) {
    // SAFETY: the enclosing port supplies the tract developer's live debug row.
    unsafe {
        let context = word(this, 0x170);
        match context_register {
            0 => r.eax = context,
            1 => r.ecx = context,
            _ => r.edx = context,
        }
        r.eax = word(context, 0x90);
        r.ecx = word(r.eax, outcome * 4).wrapping_add(1);
        store(r.eax, outcome * 4, r.ecx);
        r.eax = outcome;
    }
}

register_port! {
    /// Check purpose, wealth, active style, capacity and monopoly mode in that order. The second
    /// stack argument is unused. Property resources are released before the developer mode query.
    0x0070_4E00, fc4p_00704e00, building_compatible, 2, 8, "cSC4TractDeveloper::IsBuildingCompatible", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        r.edx = r.args[0];
        r.ecx = td.building_development(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x9C) as usize, &[r.edx]);
        let info = r.eax;
        r.ecx = td.developer(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
        if word(info, 0) != r.eax { building_outcome(r, this, 7, 1); return; }
        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[]);
        if word(info, 4) != r.eax { building_outcome(r, this, 8, 0); return; }
        r.ecx = td.styles_begin(); r.eax = td.styles_end();
        if r.ecx != r.eax {
            r.eax = word(info, 0);
            if (r.eax as i32) > 0 && (r.eax as i32) <= 3 {
                r.edx = td.style_index() as u32; r.eax = r.ecx;
                let group = word(r.eax, r.edx.wrapping_mul(4));
                let mut key = [0u32; 3];
                r.ecx = td.building_development();
                r.eax = key.as_mut_ptr() as u32;
                let output = r.eax;
                r.eax = r.args[0]; r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax, output]);
                r.ecx = key.as_mut_ptr() as u32;
                invoke(r, Conv::Cdecl, crate::abi::target(0x005E_8F70), &[r.ecx, group]);
                if r.eax as u8 == 0 { building_outcome(r, this, 9, 2); return; }
            }
        }
        r.eax = u32::from(read_unaligned(info.wrapping_add(8) as *const u16));
        if (r.eax as i32) < td.minimum_building_capacity() { building_outcome(r, this, 11, 0); return; }
        if (r.eax as i32) > td.maximum_building_capacity() { building_outcome(r, this, 13, 1); return; }
        let mut key = [0u32; 3];
        let mut monopoly = 0u8;
        r.ecx = td.building_development(); r.eax = key.as_mut_ptr() as u32;
        let output = r.eax;
        r.eax = r.args[0]; r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax, output]);
        let mut resource = 0u32;
        r.ecx = read_unaligned(resource_manager()); r.edx = word(r.ecx, 0);
        r.eax = key.as_mut_ptr() as u32;
        invoke(r, Conv::Thiscall, word(r.edx, 0xC) as usize,
            &[r.eax, 0xA521_60F5, &mut resource as *mut u32 as u32, 0, 0]);
        if r.eax as u8 != 0 {
            r.ecx = &mut monopoly as *mut u8 as u32;
            let value = r.ecx;
            r.ecx = resource; r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
            invoke(r, Conv::Cdecl, crate::abi::target(0x005F_D390), &[r.eax, 0x2781_283F, value]);
            r.ecx = resource; r.eax = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
        }
        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x64) as usize, &[]);
        if (0..=1).contains(&(r.eax as i32)) && monopoly != 0 {
            building_outcome(r, this, 2, 1);
        } else if r.eax == 2 && monopoly == 0 {
            building_outcome(r, this, 2, 0);
        } else {
            building_outcome(r, this, 27, 2);
        }
    }
}

register_port! {
    /// Check the lot's zone, age, land value, state, occupant and road access. A nonzero low flag
    /// byte enables both the suitability cache and the stricter resident-wealth checks.
    /// Cache hits return directly without incrementing the histogram.
    0x0070_7A60, fc4p_00707a60, lot_suitable, 2, 8, "cSC4TractDeveloper::IsLotSuitable", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let cached = r.args[1] as u8 != 0;
        r.eax = (r.eax & !0xFF) | u32::from(r.args[1] as u8);
        if cached {
            let mut node = 0u32;
            r.eax = r.args.as_ptr() as u32;
            r.ecx = &mut node as *mut u32 as u32;
            let output = r.ecx; r.ecx = this.wrapping_add(0x10C);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7520), &[output, r.eax]);
            r.eax = node;
            if r.eax != word(this, 0x10C) { r.eax = word(r.eax, 0x14); return; }
        }
        let lot = r.args[0];
        let mut x = 0u32;
        let mut z = 0u32;
        let mut info = [0u8; 0x14];
        let outcome = 'checks: {
            r.edx = word(lot, 0); r.eax = &mut z as *mut u32 as u32;
            r.ecx = &mut x as *mut u32 as u32;
            let x_out = r.ecx; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[x_out, r.eax]);
            r.ecx = td.developer(); r.edx = word(r.ecx, 0);
            r.eax = info.as_mut_ptr() as u32;
            invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[r.eax]);
            r.eax = z; r.ecx = word(this, 0x3C); r.edx = word(r.ecx, 0);
            r.eax = x;
            invoke(r, Conv::Thiscall, word(r.edx, 0x30) as usize, &[x, z]);
            r.edx = read_unaligned(info.as_ptr().cast::<u32>());
            r.ecx = r.eax as u8 as i8 as i32 as u32;
            invoke(r, Conv::Cdecl, crate::abi::target(0x0069_5390), &[r.edx, r.ecx]);
            if r.eax as u8 == 0 { break 'checks 6; }
            r.eax = (r.eax & !0xFF) | u32::from(r.args[1] as u8);
            if cached {
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x94) as usize, &[]);
                if r.eax as u8 != 0 {
                    r.edx = word(lot, 0); r.ecx = lot;
                    invoke(r, Conv::Thiscall, word(r.edx, 0xA0) as usize, &[]);
                    if r.eax < 180 { break 'checks 19; }
                }
            }
            r.eax = (r.eax & !0xFF) | u32::from(info[0x10]);
            if info[0x10] == 0 { r.eax = (r.eax & !0xFF) | u32::from(info[0x11]); }
            if info[0x10] != 0 || info[0x11] != 0 {
                r.edx = z; r.ecx = word(this, 0x20); r.eax = word(r.ecx, 0);
                r.edx = x;
                invoke(r, Conv::Thiscall, word(r.eax, 0x18) as usize, &[x, z]);
                r.ecx = u32::from(info[0x10]); r.eax &= 0xFF;
                if r.eax as i32 >= r.ecx as i32 { r.edx = u32::from(info[0x11]); }
                if r.eax < u32::from(info[0x10]) || r.eax > u32::from(info[0x11]) {
                    td.set_observed_land_value(r.eax as i32); break 'checks 22;
                }
            }
            r.eax = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x84) as usize, &[]);
            if r.eax == 2 || (r.eax as i32) >= 5 { break 'checks 4; }
            let mut rectangle = [0u32; 4];
            r.edx = word(lot, 0); r.eax = rectangle.as_mut_ptr() as u32; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax]);
            r.eax = rectangle[1]; r.ecx = word(this, 0x2C); r.edx = word(r.ecx, 0);
            let width = rectangle[2].wrapping_sub(rectangle[0]).wrapping_add(1);
            let depth = rectangle[3].wrapping_sub(rectangle[1]).wrapping_add(1);
            r.eax = rectangle[0];
            invoke(r, Conv::Thiscall, word(r.edx, 0x50) as usize, &[r.eax, rectangle[1], width, depth, 0, 0, 0]);
            if r.eax != 7 && r.eax != 8 {
                break 'checks match r.eax { 1 => 4, 2 => 6, 3 => 3, _ => 0 };
            }
            r.edx = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x98) as usize, &[]);
            if r.eax as u8 != 0 {
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0xB8) as usize, &[0]);
                if r.eax == 0 { break 'checks 4; }
                r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
                let building = r.eax;
                invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[building]);
                let resident = r.eax;
                r.eax = word(resident, 0);
                if r.eax == 5 || (r.eax == 7 && td.purpose() == 6) { break 'checks 15; }
                r.ecx = (r.ecx & !0xFF) | u32::from(r.args[1] as u8);
                if cached {
                    r.ecx = (r.ecx & !0xFF) | u32::from(td.kick_out());
                    if td.kick_out() == 0 {
                        r.ecx = td.wealth() as u32;
                        if r.ecx != word(resident, 4) { break 'checks 16; }
                    }
                    r.ecx = td.wealth() as u32; r.edx = word(resident, 4);
                    if (r.ecx as i32) < r.edx as i32 || (r.eax != td.purpose() as u32 && (r.ecx as i32) <= r.edx as i32) {
                        break 'checks 16;
                    }
                }
                r.edx = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0xAC) as usize, &[]);
                if r.eax as u8 != 0 { break 'checks 18; }
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[]);
                if r.eax != 0 {
                    r.edx = word(r.eax, 0); r.ecx = r.eax;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                    r.ecx = word(resident, 4);
                    if r.ecx == td.wealth() as u32 {
                        r.edx = r.eax & 0xFF;
                        r.eax = (r.eax & !0xFF) | u32::from(td.maximum_stage() > r.edx as i32);
                    } else {
                        r.ecx = td.maximum_stage() as u32; r.eax &= 0xFF;
                        let passes = r.ecx as i32 >= r.eax as i32;
                        r.eax = (r.eax & !0xFF) | u32::from(passes);
                    }
                    if r.eax as u8 == 0 { break 'checks 17; }
                }
            }
            r.ecx = td.developer(); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
            if r.eax == 1 {
                r.ecx = word(this, 0x38); r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x34) as usize, &[]);
                r.ecx = z; r.edx = word(r.eax, 0); r.ecx = x;
                let grid = r.eax; r.ecx = grid;
                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
            } else if (6..=8).contains(&(r.eax as i32)) {
                r.edx = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0x74) as usize, &[r.eax]);
            } else {
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x70) as usize, &[]);
            }
            if r.eax as u8 == 0 { break 'checks 21; }
            27
        };
        r.eax = (r.eax & !0xFF) | u32::from(r.args[1] as u8);
        if cached {
            r.edx = r.args.as_ptr() as u32; r.ecx = this.wrapping_add(0x10C);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7A00), &[r.edx]);
            store(r.eax, 0, outcome);
        }
        r.eax = td.debug_context(); r.ecx = word(r.eax, 0x60);
        r.eax = r.ecx.wrapping_add(outcome * 4);
        store(r.eax, 0, word(r.eax, 0).wrapping_add(1));
        r.eax = outcome;
    }
}

pub const PORTS: &[Port] = &[fc4p_00704e00::PORT, fc4p_00707a60::PORT];

#[cfg(test)]
mod tests;
