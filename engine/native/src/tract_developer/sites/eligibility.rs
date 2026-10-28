// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Eligibility checks expanded inside the three site-selection routines.

use super::*;

game_fn! { #[allow(dead_code)] fn fits_zone(target: u32, zone: i32) -> u32 = "C" 0x0069_5390; }
game_fn! { #[allow(dead_code)] fn cache_value(this: u32, lot: u32) -> u32 = "thiscall" 0x0049_7A00; }

/// The two listing routines search their suitability tree directly, with unsigned pointer ordering.
pub(super) unsafe fn cached<const N: usize>(r: &mut Entry<N>, this: u32, lot: u32) -> Option<u32> {
    // SAFETY: this owns the live suitability tree, with ordinary nodes at the original offsets.
    unsafe {
        r.ecx = word(this, 0x10C);
        r.eax = word(r.ecx, 4);
        r.edx = r.ecx;
        while r.eax != 0 {
            if word(r.eax, 0x10) >= lot {
                r.edx = r.eax;
                r.eax = word(r.eax, 8);
            } else {
                r.eax = word(r.eax, 0xC);
            }
        }
        if r.edx == r.ecx || lot < word(r.edx, 0x10) {
            r.edx = r.ecx;
        }
        if r.edx != word(this, 0x10C) {
            Some(word(r.edx, 0x14))
        } else {
            None
        }
    }
}

/// Check and cache a borrowed lot without calling the separately callable eligibility entry.
/// The area query's width is max-z minus min-x plus one, as used by these expanded checks.
pub(super) unsafe fn check<const N: usize>(r: &mut Entry<N>, this: u32, lot: u32, key: u32, existing: bool) -> u32 {
    // SAFETY: the enclosing site routine supplies the original services, live lot and key word.
    unsafe {
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let mut location = [0u32; 2];
        let mut info = [0u8; 20];
        let outcome = 'checks: {
            let x = location.as_mut_ptr() as u32;
            let z = location.as_mut_ptr().add(1) as u32;
            if existing {
                r.eax = word(lot, 0);
                r.ecx = z;
                r.edx = x;
            } else {
                r.edx = word(lot, 0);
                r.eax = z;
                r.ecx = x;
            }
            let table = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(table, 0x54) as usize, &[x, z]);
            r.ecx = td.developer();
            let table = word(r.ecx, 0);
            if existing {
                r.eax = table;
                r.edx = info.as_mut_ptr() as u32;
            } else {
                r.edx = table;
                r.eax = info.as_mut_ptr() as u32;
            }
            invoke(
                r,
                Conv::Thiscall,
                word(table, 0x24) as usize,
                &[info.as_mut_ptr() as u32],
            );
            r.ecx = word(this, 0x3C);
            let table = word(r.ecx, 0);
            if existing {
                r.eax = table;
                r.edx = location[0];
            } else {
                r.edx = table;
                r.eax = location[0];
            }
            invoke(r, Conv::Thiscall, word(table, 0x30) as usize, &location);
            let target = read_unaligned(info.as_ptr().cast::<u32>());
            let zone = r.eax as u8 as i8 as i32 as u32;
            if existing {
                r.ecx = target;
                r.eax = zone;
            } else {
                r.edx = target;
                r.ecx = zone;
            }
            invoke(r, Conv::Cdecl, crate::abi::target(0x0069_5390), &[target, zone]);
            if r.eax as u8 == 0 {
                break 'checks 6;
            }
            let table = word(lot, 0);
            r.ecx = lot;
            if existing {
                r.edx = table;
            } else {
                r.eax = table;
            }
            invoke(r, Conv::Thiscall, word(table, 0x94) as usize, &[]);
            if r.eax as u8 != 0 {
                let table = word(lot, 0);
                r.ecx = lot;
                if existing {
                    r.eax = table;
                } else {
                    r.edx = table;
                }
                invoke(r, Conv::Thiscall, word(table, 0xA0) as usize, &[]);
                if r.eax < 180 {
                    break 'checks 19;
                }
            }
            r.eax = (r.eax & !0xFF) | u32::from(info[0x10]);
            if info[0x10] == 0 {
                r.eax = (r.eax & !0xFF) | u32::from(info[0x11]);
            }
            if info[0x10] != 0 || info[0x11] != 0 {
                r.ecx = word(this, 0x20);
                let table = word(r.ecx, 0);
                if existing {
                    r.edx = table;
                    r.eax = location[0];
                } else {
                    r.eax = table;
                    r.edx = location[0];
                }
                invoke(r, Conv::Thiscall, word(table, 0x18) as usize, &location);
                r.ecx = u32::from(info[0x10]);
                r.eax &= 0xFF;
                if r.eax >= r.ecx {
                    r.edx = u32::from(info[0x11]);
                }
                if r.eax < u32::from(info[0x10]) || r.eax > u32::from(info[0x11]) {
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
            r.ecx = word(this, 0x2C);
            r.edx = word(r.ecx, 0);
            let depth = rectangle[3].wrapping_sub(rectangle[1]).wrapping_add(1);
            let width = rectangle[3].wrapping_sub(rectangle[0]).wrapping_add(1);
            r.eax = rectangle[0];
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
                r.ecx = word(this, 0x1C);
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
                        r.eax = (r.eax & !0xFF) | u32::from(r.ecx as i32 >= r.eax as i32);
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
                r.ecx = word(this, 0x38);
                r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x34) as usize, &[]);
                r.ecx = location[1];
                r.edx = word(r.eax, 0);
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
        r.edx = key;
        r.ecx = this.wrapping_add(0x10C);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0049_7A00), &[r.edx]);
        store(r.eax, 0, outcome);
        r.eax = td.debug_context();
        r.ecx = word(r.eax, 0x60);
        r.eax = r.ecx.wrapping_add(outcome * 4);
        store(r.eax, 0, word(r.eax, 0).wrapping_add(1));
        outcome
    }
}

#[cfg(test)]
mod tests;
