// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Farm discovery, resource claims, and construction of surrounding fields.

use super::*;

game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn find(this: u32, output: u32, key: u32) -> u32 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn cache(this: u32, key: u32) -> u32 = "thiscall" 0x0049_7A00; }
game_fn! { #[allow(dead_code)] fn fits_zone(target: u32, zone: i32) -> u32 = "C" 0x0069_5390; }
game_fn! { #[allow(dead_code)] fn build(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_B440; }
game_fn! { #[allow(dead_code)] fn uniform(this: u32, count: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { #[allow(dead_code)] fn unlink(node: u32, root: u32, left: u32, right: u32) -> u32 = "C" 0x0040_DAF0; }
game_data! { static resources: u32 = 0x00B4_3CA8; }
game_data! { static random: u8 = 0x00B4_AFD8; }

/// Evaluate a cache miss using the farm's own eligibility sequence.
unsafe fn eligibility(r: &mut Entry<1>, this: u32, lot: u32, key: *mut u32) -> u32 {
    // SAFETY: the enclosing port supplies the live services, lot, and writable cache key.
    unsafe {
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let mut location = [0u32; 2];
        let mut info = [0u8; 20];
        let outcome = 'checks: {
            r.edx = word(lot, 0);
            r.eax = location.as_mut_ptr().add(1) as u32;
            let z = r.eax;
            r.ecx = location.as_mut_ptr() as u32;
            let x = r.ecx;
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[x, z]);
            r.ecx = td.developer();
            r.edx = word(r.ecx, 0);
            r.eax = info.as_mut_ptr() as u32;
            invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[r.eax]);
            r.eax = location[1];
            r.ecx = td.zone_manager();
            r.edx = word(r.ecx, 0);
            r.eax = location[0];
            invoke(r, Conv::Thiscall, word(r.edx, 0x30) as usize, &location);
            r.edx = read_unaligned(info.as_ptr().cast::<u32>());
            r.ecx = r.eax as u8 as i8 as i32 as u32;
            invoke(r, Conv::Cdecl, crate::abi::target(0x0069_5390), &[r.edx, r.ecx]);
            if r.eax as u8 == 0 {
                break 'checks 6;
            }
            r.eax = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x94) as usize, &[]);
            if r.eax as u8 != 0 {
                r.edx = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0xA0) as usize, &[]);
                if r.eax < 180 {
                    break 'checks 19;
                }
            }
            r.eax = (r.eax & !0xFF) | u32::from(info[16]);
            if info[16] == 0 {
                r.eax = (r.eax & !0xFF) | u32::from(info[17]);
            }
            if r.eax as u8 != 0 {
                r.edx = location[1];
                r.ecx = td.land_value();
                r.eax = word(r.ecx, 0);
                r.edx = location[0];
                invoke(r, Conv::Thiscall, word(r.eax, 0x18) as usize, &location);
                r.ecx = u32::from(info[16]);
                r.eax &= 0xFF;
                if r.eax >= r.ecx {
                    r.edx = u32::from(info[17]);
                }
                if r.eax < r.ecx || r.eax > u32::from(info[17]) {
                    td.set_observed_land_value(r.eax as i32);
                    break 'checks 22;
                }
            }
            r.eax = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x84) as usize, &[]);
            if r.eax == 2 || r.eax as i32 >= 5 {
                break 'checks 4;
            }
            let mut rectangle = [0u32; 4];
            r.edx = word(lot, 0);
            r.eax = rectangle.as_mut_ptr() as u32;
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax]);
            r.eax = rectangle[3];
            r.ecx = td.lot_manager();
            r.edx = word(r.ecx, 0);
            r.eax = r.eax.wrapping_sub(rectangle[1]).wrapping_add(1);
            let depth = r.eax;
            r.eax = rectangle[0];
            let width = rectangle[2].wrapping_sub(r.eax).wrapping_add(1);
            invoke(
                r,
                Conv::Thiscall,
                word(r.edx, 0x50) as usize,
                &[r.eax, rectangle[1], width, depth, 0, 0, 0],
            );
            if r.eax != 7 && r.eax != 8 {
                break 'checks match r.eax {
                    1 => 4,
                    2 => 6,
                    3 => 3,
                    _ => 0,
                };
            }
            r.edx = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x98) as usize, &[]);
            if r.eax as u8 != 0 {
                r.eax = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0xB8) as usize, &[0]);
                if r.eax == 0 {
                    break 'checks 4;
                }
                r.ecx = td.building_development();
                r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[r.eax]);
                let resident = r.eax;
                r.eax = word(resident, 0);
                if r.eax == 5 || (r.eax == 7 && td.purpose() == 6) {
                    break 'checks 15;
                }
                r.ecx = (r.ecx & !0xFF) | u32::from(td.kick_out());
                if td.kick_out() == 0 {
                    r.ecx = td.wealth() as u32;
                    if r.ecx != word(resident, 4) {
                        break 'checks 16;
                    }
                }
                r.ecx = td.wealth() as u32;
                r.edx = word(resident, 4);
                if (r.ecx as i32) < r.edx as i32 || (r.eax != td.purpose() as u32 && (r.ecx as i32) <= r.edx as i32) {
                    break 'checks 16;
                }
                r.edx = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0xAC) as usize, &[]);
                if r.eax as u8 != 0 {
                    break 'checks 18;
                }
                r.eax = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[]);
                if r.eax != 0 {
                    r.edx = word(r.eax, 0);
                    r.ecx = r.eax;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                    r.ecx = word(resident, 4);
                    if r.ecx == td.wealth() as u32 {
                        r.edx = r.eax & 0xFF;
                        r.eax = (r.eax & !0xFF) | u32::from(td.maximum_stage() > r.edx as i32);
                    } else {
                        r.ecx = td.maximum_stage() as u32;
                        r.eax &= 0xFF;
                        r.eax = u32::from(r.ecx as i32 >= r.eax as i32);
                    }
                    if r.eax as u8 == 0 {
                        break 'checks 17;
                    }
                }
            }
            r.ecx = td.developer();
            r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
            if r.eax == 1 {
                r.ecx = td.traffic();
                r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x34) as usize, &[]);
                r.ecx = location[1];
                r.edx = word(r.eax, 0);
                r.ecx = location[0];
                r.ecx = r.eax;
                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &location);
            } else if (6..=8).contains(&(r.eax as i32)) {
                r.edx = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0x74) as usize, &[r.eax]);
            } else {
                r.eax = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x70) as usize, &[]);
            }
            if r.eax as u8 == 0 {
                21
            } else {
                27
            }
        };
        r.edx = key as u32;
        r.ecx = this.wrapping_add(0x10C);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7A00), &[r.edx]);
        store(r.eax, 0, outcome);
        r.eax = td.debug_context();
        r.ecx = word(r.eax, 0x60);
        r.eax = r.ecx.wrapping_add(outcome * 4);
        store(r.eax, 0, word(r.eax, 0).wrapping_add(1));
        r.eax = outcome;
        outcome
    }
}

register_port! {
    /// Walk a growing neighbour list, retaining distinct suitable fields outside the candidate.
    /// At least fifteen fields are needed. Claim their resources plus thirty, release thirty,
    /// build the farm, then construct each field using one randomly selected field type.
    /// Neighbour references and both temporary lists are released on every exit.
    0x0070_B640, fc4p_0070b640, build_farm, 1, 4, "cSC4TractDeveloper::BuildFarm", |r| {
        let this = r.ecx;
        let candidate = r.args[0];
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let mut outcome = 0;
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[12]);
        let fields = r.eax;
        store(fields, 0, fields); store(fields, 4, fields);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[12]);
        store(r.eax, 0, r.eax); store(r.eax, 4, r.eax);
        let mut near = r.eax;
        let bounds = candidate.wrapping_add(0x38);
        for flag in [1, 2, 4, 8] {
            r.ecx = td.lot_manager(); r.edx = &mut near as *mut u32 as u32;
            r.eax = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.eax, 0x28) as usize, &[bounds, r.edx, flag]);
        }
        r.eax = near; r.ecx = word(r.eax, 0);
        let mut node = r.ecx;
        while node != r.eax {
            r.eax = node;
            let lot = word(node, 8);
            let mut rectangle = [0u32; 4];
            r.edx = word(lot, 0); r.eax = rectangle.as_mut_ptr() as u32; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax]);
            r.ecx = rectangle[2];
            let overlap = if (r.ecx as i32) < word(bounds, 0) as i32 { false } else {
                r.edx = rectangle[0];
                if (r.edx as i32) > word(bounds, 8) as i32 { false } else {
                    r.eax = rectangle[3];
                    if (r.eax as i32) < word(bounds, 4) as i32 { false } else {
                        r.ecx = rectangle[1]; (r.ecx as i32) <= word(bounds, 12) as i32
                    }
                }
            };
            if !overlap {
                let mut key = lot;
                let mut found = 0u32;
                r.edx = &mut key as *mut u32 as u32;
                r.eax = &mut found as *mut u32 as u32;
                r.ecx = this.wrapping_add(0x10C);
                invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7520), &[r.eax, r.edx]);
                r.eax = found;
                let suitability = if r.eax != td.suitability_header() {
                    r.eax = word(r.eax, 0x14); r.eax
                } else { eligibility(r, this, lot, &mut key) };
                if suitability == 27 || suitability == 21 {
                    r.eax = word(fields, 0);
                    while r.eax != fields && word(r.eax, 8) != lot { r.eax = word(r.eax, 0); }
                    if r.eax == fields {
                        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[12]);
                        r.ecx = r.eax.wrapping_add(8);
                        if r.ecx != 0 { store(r.ecx, 0, lot); }
                        r.ecx = word(fields, 4);
                        store(r.eax, 0, fields); store(r.eax, 4, r.ecx);
                        store(r.ecx, 0, r.eax); store(fields, 4, r.eax);
                        for flag in [1, 2, 4, 8] {
                            r.ecx = td.lot_manager(); r.edx = word(r.ecx, 0);
                            r.eax = &mut near as *mut u32 as u32;
                            invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[lot, r.eax, flag]);
                        }
                    }
                }
            }
            r.ecx = word(node, 0); node = r.ecx; r.eax = near;
        }
        r.ecx = word(fields, 0);
        let mut count = 0u32;
        if r.ecx != fields {
            r.edx = fields;
            loop { r.ecx = word(r.ecx, 0); count = count.wrapping_add(1); if r.ecx == r.edx { break; } }
        }
        if count as i32 >= 15 {
            r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
            r.eax = count.wrapping_add(30);
            invoke(r, Conv::Thiscall, word(r.edx, 0xC8) as usize, &[r.eax]);
            if r.eax as u8 != 0 {
                r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0xCC) as usize, &[30]);
                r.eax = candidate; r.ecx = this;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_B440), &[r.eax]);
                outcome = r.eax; r.ecx = td.building_development();
                if outcome == 27 { construct_fields(r, this, candidate, fields, count); }
                else {
                    r.edx = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0xCC) as usize, &[count]);
                }
            }
            r.eax = near;
        }
        // The neighbour list owns one reference per node; the fields list borrows those lots.
        let mut next = word(r.eax, 0);
        while next != r.eax {
            let dead = next; r.ecx = word(dead, 8); next = word(next, 0);
            if r.ecx != 0 {
                r.eax = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
            }
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[dead]);
            r.eax = near;
        }
        store(r.eax, 0, r.eax); r.eax = near; store(r.eax, 4, r.eax);
        r.ecx = near;
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.ecx]);
        let mut next = word(fields, 0);
        while next != fields {
            r.eax = next; next = word(next, 0);
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]);
        }
        store(fields, 0, fields); store(fields, 4, fields);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[fields]);
        r.eax = outcome;
    }
}

unsafe fn construct_fields(r: &mut Entry<1>, this: u32, candidate: u32, fields: u32, count: u32) {
    // SAFETY: successful farm construction leaves live services and the borrowed field list.
    unsafe {
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let mut resource_key = [0u32; 3];
        r.eax = resource_key.as_mut_ptr() as u32;
        let output = r.eax;
        r.eax = td.chosen_building();
        r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax, output]);
        r.ecx = read_unaligned(resources());
        let mut resource = 0u32;
        r.eax = &mut resource as *mut u32 as u32;
        let output = r.eax;
        r.eax = resource_key.as_mut_ptr() as u32;
        r.edx = word(r.ecx, 0);
        invoke(
            r,
            Conv::Thiscall,
            word(r.edx, 0xC) as usize,
            &[r.eax, 0xA521_60F5, output, 0, 0],
        );
        let mut property = 0;
        if r.eax as u8 != 0 {
            r.ecx = resource;
            r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
            r.edx = word(r.eax, 0);
            r.ecx = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[0x29B5_5F73]);
            property = r.eax;
        }
        let mut field_type = 0;
        if property != 0 {
            r.eax = word(property, 0);
            r.ecx = property;
            invoke(r, Conv::Thiscall, word(r.eax, 0x14) as usize, &[]);
            let variant = r.eax;
            r.edx = word(variant, 0);
            r.ecx = variant;
            invoke(r, Conv::Thiscall, word(r.edx, 0x19C) as usize, &[]);
            let data = r.eax;
            r.eax = word(variant, 0);
            r.ecx = variant;
            invoke(r, Conv::Thiscall, word(r.eax, 0x10) as usize, &[]);
            r.ecx = random() as u32;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0091_36B1), &[r.eax]);
            field_type = word(data, r.eax.wrapping_mul(4));
        }
        r.ecx = td.building_development();
        r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[field_type]);
        r.eax = u32::from(read_unaligned(r.eax.wrapping_add(8) as *const u16));
        let capacity = r.eax;
        r.ecx = td.lot_configuration_manager();
        r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[field_type]);
        let configuration = r.eax;
        if configuration == 0 {
            r.ecx = td.building_development();
            r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0xCC) as usize, &[count]);
        } else {
            r.eax = fields;
            let mut node = word(r.eax, 0);
            while node != fields {
                let lot = word(node, 8);
                let mut key = lot;
                r.ecx = candidate;
                r.edx = word(r.ecx, 0x10);
                r.eax = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x60) as usize, &[r.edx]);
                r.edx = field_type;
                r.ecx = td.lot_developer();
                r.eax = word(r.ecx, 0);
                r.edx = configuration;
                invoke(
                    r,
                    Conv::Thiscall,
                    word(r.eax, 0x18) as usize,
                    &[lot, r.edx, field_type, 0],
                );
                let mut found = 0u32;
                r.eax = &mut key as *mut u32 as u32;
                r.ecx = &mut found as *mut u32 as u32;
                let output = r.ecx;
                let cache = this.wrapping_add(0x10C);
                r.ecx = cache;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7520), &[output, r.eax]);
                r.ecx = found;
                r.eax = word(cache, 0);
                if r.ecx != r.eax {
                    r.edx = r.eax.wrapping_add(12);
                    let right = r.edx;
                    r.edx = r.eax.wrapping_add(8);
                    r.eax = r.eax.wrapping_add(4);
                    invoke(
                        r,
                        Conv::Cdecl,
                        crate::abi::target(0x0040_DAF0),
                        &[r.ecx, r.eax, r.edx, right],
                    );
                    if r.eax != 0 {
                        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]);
                    }
                    store(cache, 4, word(cache, 4).wrapping_sub(1));
                }
                r.eax = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x88) as usize, &[3, 0, 0]);
                r.eax = capacity;
                r.edx = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0xDC) as usize, &[0x4100, r.eax]);
                node = word(node, 0);
            }
        }
        r.ecx = resource;
        if r.ecx != 0 {
            r.eax = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0070b640::PORT];

#[cfg(test)]
mod tests;

#[cfg(test)]
mod eligibility_tests;
