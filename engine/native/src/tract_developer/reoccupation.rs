// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Selecting and reoccupying abandoned lots and wealth upgrades.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_data! { static pool: [u32; 3] = 0x00B4_AFCC; }
game_data! { static fire: u32 = 0x00B4_3D78; }
game_data! { static aura: u32 = 0x00B4_3D60; }
game_fn! { #[allow(dead_code)] fn zone_type(zone: u32) -> u32 = "C" 0x006B_3890; }
game_fn! { #[allow(dead_code)] fn suitable(this: u32, lot: u32, check_wealth: u32) -> u32 = "thiscall" 0x0070_7A60; }
game_fn! { #[allow(dead_code)] fn candidate() -> u32 = "C" 0x0070_7D90; }
game_fn! { #[allow(dead_code)] fn capacity(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_55B0; }
game_fn! { #[allow(dead_code)] fn insert_pointer(this: u32, at: u32, value: u32, allocator: u32, count: u32, unit: u32) -> u32 = "thiscall" 0x0051_CA60; }
game_fn! { #[allow(dead_code)] fn score(this: u32) -> u32 = "thiscall" 0x0070_5750; }
game_fn! { #[allow(dead_code)] fn sort(begin: u32, end: u32, compare: u32) -> u32 = "C" 0x0070_9520; }
game_fn! { #[allow(dead_code)] fn compare(a: u32, b: u32) -> u32 = "C" 0x0070_4770; }
game_fn! { #[allow(dead_code)] fn clear(list: u32) -> u32 = "C" 0x0070_7370; }

layout! {
    /// Threshold multipliers used only when taking over lots.
    struct ReoccupationTuning {
        0x44 => minimum / set_minimum: f32,
        0x48 => maximum / set_maximum: f32,
        0xEC => candidates_limit / set_candidates_limit: u32,
    }
}

fn multiply_stored(value: F80, operand: f32) -> F80 {
    let mut result = [0u8; 10];
    let value = value.to_bytes();
    // SAFETY: one extended register is loaded and popped, with a stored single-precision multiplier.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value = in(reg) value.as_ptr(), operand = in(reg) &operand, result = in(reg) result.as_mut_ptr(),
            out("st(0)") _);
    }
    F80::from_bytes(result)
}

/// Count a failed cap without skipping any of the remaining cap queries.
unsafe fn cap_failure(r: &mut Entry<0>, this: u32, code: u32, best: &mut u32, register: u8) {
    // SAFETY: the enclosing port supplies the attempt's live diagnostic row.
    unsafe {
        let context = word(this, 0x170);
        match register {
            0 => r.eax = context,
            1 => r.ecx = context,
            _ => r.edx = context,
        }
        r.eax = word(context, 0xC);
        r.edx = word(r.eax, code * 4);
        r.ecx = *best;
        r.edx = r.edx.wrapping_add(1);
        store(r.eax, code * 4, r.edx);
        r.eax = code;
        if (r.ecx as i32) < code as i32 {
            *best = code;
        }
    }
}

register_port! {
    /// Collect eligible abandoned lots and occupied lots that can upgrade wealth. All utility caps
    /// are checked even after one fails. Score and sort only above the minimum threshold, then
    /// reoccupy until capacity strictly exceeds the maximum threshold. Recycle every candidate.
    0x0070_A3D0, fc4p_0070a3d0, take_over, 0, 0, "cSC4TractDeveloper::TakeOverAbandoned", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let tuning = ReoccupationTuning::from_ptr(this as *mut u8);
        r.ecx = td.developer(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x14) as usize, &[]);
        let target_zone = r.eax;
        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[]);
        let developer_type = r.eax;
        let built = td.built() as u32;
        r.eax = (td.desired() as u32).wrapping_sub(built);
        let threshold = multiply_stored(F80::from_i32(r.eax as i32), tuning.minimum()).ftol();
        let minimum = threshold as u32;
        r.eax = threshold as u32; r.edx = (threshold >> 32) as u32;
        r.ecx = (td.desired_maximum() as u32).wrapping_sub(built);
        let threshold = multiply_stored(F80::from_i32(r.ecx as i32), tuning.maximum()).ftol();
        let maximum = threshold as u32;
        r.edx = (threshold >> 32) as u32;
        r.eax = td.lots_header();
        let mut iterator = word(r.eax, 8);
        let mut best = 1u32;
        let mut sum = 0u32;
        while iterator != td.lots_header() {
            let lot = word(iterator, 0x10);
            r.edx = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x84) as usize, &[]);
            let state = r.eax;
            r.eax = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0xA4) as usize, &[]);
            let occupant_wealth = r.eax;
            r.edx = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0xB8) as usize, &[0]);
            let building_wealth = if r.eax != 0 {
                let building = r.eax;
                r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[building]);
                r.eax = word(r.eax, 4); r.eax
            } else { 0 };
            r.ecx = td.developer(); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[]);
            let mut eligible = state == 4;
            if !eligible {
                r.ecx = (r.ecx & !0xFF) | u32::from(td.kick_out());
                eligible = r.ecx as u8 != 0 && state == 3 && (occupant_wealth as i32) < r.eax as i32
                    && (r.eax as i32) <= building_wealth as i32;
            }
            if eligible {
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x7C) as usize, &[]);
                invoke(r, Conv::Cdecl, crate::abi::target(0x006B_3890), &[r.eax]);
                r.ecx = target_zone;
                if r.eax == r.ecx {
                    r.ecx = td.developer(); r.edx = word(r.ecx, 0);
                    // The lot table is read before the developer-type callback.
                    let lot_table = word(lot, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[]);
                    r.ecx = lot;
                    invoke(r, Conv::Thiscall, word(lot_table, 0xC8) as usize, &[r.eax, 1]);
                    if r.eax as u16 == 0 {
                        r.ecx = td.debug_context(); r.ecx = word(r.ecx, 0xC);
                        r.edx = word(r.ecx, 0x3C).wrapping_add(1); r.eax = 15;
                        store(r.ecx, 0x3C, r.edx);
                        if (r.eax as i32) > best as i32 { best = r.eax; }
                    } else {
                        r.ecx = this;
                        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7A60), &[lot, 0]);
                        if r.eax != 27 && r.eax != 18 {
                            if (r.eax as i32) > best as i32 { best = r.eax; }
                        } else {
                            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7D90), &[]);
                            let candidate = r.eax;
                            store(candidate, 0, 1); store(candidate, 4, lot);
                            r.eax = candidate.wrapping_add(8); r.edx = word(lot, 0); r.ecx = lot;
                            invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[r.eax, candidate.wrapping_add(0xC)]);
                            r.eax = word(lot, 0); r.ecx = candidate.wrapping_add(0x15);
                            let depth = r.ecx; r.edx = candidate.wrapping_add(0x14); r.ecx = lot;
                            invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[r.edx, depth]);
                            r.eax = word(lot, 0); r.ecx = lot;
                            invoke(r, Conv::Thiscall, word(r.eax, 0x5C) as usize, &[]);
                            r.ecx = this; store(candidate, 0x10, r.eax);
                            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_55B0), &[candidate]);
                            r.eax = word(candidate, 0x28);
                            if r.eax == 0 {
                                r.eax = developer_type; r.edx = word(lot, 0); r.ecx = lot;
                                invoke(r, Conv::Thiscall, word(r.edx, 0xD0) as usize, &[r.eax, 1]);
                                r.eax = u32::from(r.eax as u16); store(candidate, 0x28, r.eax);
                                if r.eax == 0 && state == 3 {
                                    r.eax = developer_type; r.edx = word(lot, 0); r.ecx = lot;
                                    invoke(r, Conv::Thiscall, word(r.edx, 0xC8) as usize, &[r.eax, 1]);
                                    r.ecx = u32::from(r.eax as u16); store(candidate, 0x28, r.ecx);
                                }
                            }
                            r.edx = word(candidate, 0x28);
                            let mut accepted = (r.edx as i32) > 0;
                            r.eax = (r.eax & !0xFF) | u32::from(accepted);
                            if accepted {
                                r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
                                invoke(r, Conv::Thiscall, word(r.edx, 0xE0) as usize, &[]);
                                if r.eax as u8 == 0 {
                                    r.eax = word(lot, 0); r.ecx = lot;
                                    invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[]);
                                    if r.eax != 0 {
                                        r.edx = word(r.eax, 0); r.ecx = r.eax;
                                        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                                        let stage = r.eax as u8;
                                        r.edx = word(lot, 0);
                                        r.eax = word(td.building_development(), 0);
                                        let table = r.eax; r.ecx = lot;
                                        invoke(r, Conv::Thiscall, word(r.edx, 0xB8) as usize, &[0]);
                                        r.ecx = td.building_development();
                                        let building = r.eax; r.eax = table;
                                        invoke(r, Conv::Thiscall, word(r.eax, 0x9C) as usize, &[building]);
                                        let wealth = word(r.eax, 4);
                                        r.ecx = td.building_development(); r.eax = word(r.eax, 0); r.edx = word(r.ecx, 0);
                                        invoke(r, Conv::Thiscall, word(r.edx, 0x84) as usize, &[r.eax, wealth]);
                                        let developer = r.eax;
                                        let x = word(candidate, 8); let z = word(candidate, 0xC);
                                        r.ecx = x; r.edx = word(developer, 0); r.ecx = developer;
                                        invoke(r, Conv::Thiscall, word(r.edx, 0x4C) as usize, &[]);
                                        if stage > r.eax as u8 {
                                            r.ecx = td.building_development(); r.eax = word(r.ecx, 0);
                                            invoke(r, Conv::Thiscall, word(r.eax, 0xDC) as usize, &[]);
                                            if r.eax as u8 == 0 {
                                                r.ecx = td.power(); r.edx = word(r.ecx, 0);
                                                invoke(r, Conv::Thiscall, word(r.edx, 0x8C) as usize, &[]);
                                                r.ecx = x; r.edx = word(r.eax, 0); r.ecx = r.eax;
                                                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
                                                if (r.eax as u8) < 2 { cap_failure(r, this, 26, &mut best, 2); accepted = false; }
                                            }
                                        }
                                        r.eax = word(developer, 0); r.ecx = developer;
                                        invoke(r, Conv::Thiscall, word(r.eax, 0x50) as usize, &[]);
                                        if stage > r.eax as u8 {
                                            r.ecx = td.building_development(); r.edx = word(r.ecx, 0);
                                            invoke(r, Conv::Thiscall, word(r.edx, 0xD8) as usize, &[]);
                                            if r.eax as u8 == 0 {
                                                r.ecx = td.plumbing(); r.eax = word(r.ecx, 0);
                                                invoke(r, Conv::Thiscall, word(r.eax, 0x2C) as usize, &[x, z]);
                                                if r.eax as u8 == 0 { cap_failure(r, this, 25, &mut best, 1); accepted = false; }
                                            }
                                        }
                                        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
                                        invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[]);
                                        if stage > r.eax as u8 {
                                            r.ecx = read_unaligned(fire()); r.eax = word(r.ecx, 0);
                                            invoke(r, Conv::Thiscall, word(r.eax, 0x14) as usize, &[]);
                                            r.edx = word(r.eax, 0); r.ecx = r.eax;
                                            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
                                            if r.eax as u8 == 0 { cap_failure(r, this, 23, &mut best, 0); accepted = false; }
                                        }
                                        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
                                        invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[]);
                                        if stage > r.eax as u8 {
                                            r.ecx = read_unaligned(aura()); r.eax = word(r.ecx, 0);
                                            invoke(r, Conv::Thiscall, word(r.eax, 0x38) as usize, &[]);
                                            r.edx = word(r.eax, 0); r.ecx = r.eax;
                                            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[x, z]);
                                            if r.eax as u16 == 0 { cap_failure(r, this, 24, &mut best, 0); accepted = false; }
                                        }
                                    }
                                }
                            }
                            if accepted {
                                r.eax = td.candidates_end(); r.edx = tuning.candidates_limit(); r.ecx = this.wrapping_add(0xE4);
                                if r.eax != r.edx {
                                    if r.eax != 0 { store(r.eax, 0, candidate); }
                                    store(r.ecx, 4, word(r.ecx, 4).wrapping_add(4));
                                } else {
                                    let mut value = candidate;
                                    let mut allocator = MaybeUninit::<u8>::uninit();
                                    r.edx = allocator.as_mut_ptr() as u32; let allocator = r.edx;
                                    r.edx = &mut value as *mut u32 as u32;
                                    invoke(r, Conv::Thiscall, crate::abi::target(0x0051_CA60), &[r.eax, r.edx, allocator, 1, 1]);
                                }
                                r.ecx = sum; r.eax = word(candidate, 0x28); r.ecx = r.ecx.wrapping_add(r.eax); sum = r.ecx;
                            } else {
                                r.eax = word(pool() as u32, 4);
                                if r.eax != word(pool() as u32, 8) {
                                    if r.eax != 0 { store(r.eax, 0, candidate); r.eax = word(pool() as u32, 4); }
                                    r.eax = r.eax.wrapping_add(4); store(pool() as u32, 4, r.eax);
                                } else {
                                    let mut value = candidate;
                                    let mut allocator = MaybeUninit::<u8>::uninit();
                                    r.ecx = allocator.as_mut_ptr() as u32; let allocator = r.ecx;
                                    r.edx = &mut value as *mut u32 as u32; r.ecx = pool() as u32;
                                    invoke(r, Conv::Thiscall, crate::abi::target(0x0051_CA60), &[r.eax, r.edx, allocator, 1, 1]);
                                }
                            }
                        }
                    }
                }
            }
            r.ecx = word(iterator, 0xC); r.eax = iterator;
            if r.ecx != 0 {
                r.eax = r.ecx; r.ecx = word(r.eax, 8);
                while r.ecx != 0 { r.eax = r.ecx; r.ecx = word(r.eax, 8); }
            } else {
                r.ecx = word(iterator, 4);
                while r.eax == word(r.ecx, 0xC) { r.eax = r.ecx; r.ecx = word(r.ecx, 4); }
                if word(r.eax, 0xC) != r.ecx { r.eax = r.ecx; }
            }
            iterator = r.eax;
            if iterator == td.lots_header() { break; }
            // The next lot's eligibility calls replace EAX before it is used.
        }
        r.eax = td.candidates_begin(); r.ecx = td.candidates_end();
        let list = this.wrapping_add(0xE4);
        if r.eax != r.ecx {
            r.ecx = minimum;
            if (sum as i32) > r.ecx as i32 {
                r.ecx = this;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5750), &[]);
                r.edx = word(list, 0);
                if r.edx != word(list, 4) {
                    r.eax = td.candidates_end(); r.ecx = r.edx;
                    invoke(r, Conv::Cdecl, crate::abi::target(0x0070_9520), &[r.ecx, r.eax, 0x0070_4770]);
                }
                let mut next = word(list, 0); r.eax = td.candidates_end();
                let mut taken = 0u32;
                while next != td.candidates_end() {
                    let candidate = word(next, 0); let lot = word(candidate, 4);
                    r.edx = word(lot, 0); r.ecx = lot;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x84) as usize, &[]);
                    if r.eax == 3 {
                        r.ecx = td.lot_developer(); r.eax = word(r.ecx, 0);
                        invoke(r, Conv::Thiscall, word(r.eax, 0x20) as usize, &[lot, 5]);
                    }
                    r.eax = developer_type; r.ecx = td.lot_developer(); r.edx = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[lot, r.eax]);
                    r.ecx = word(candidate, 0x28); r.edx = td.built() as u32; r.eax = maximum;
                    r.edx = r.edx.wrapping_add(r.ecx); td.set_built(r.edx as i32);
                    taken = taken.wrapping_add(word(candidate, 0x28));
                    if (taken as i32) > r.eax as i32 { break; }
                    r.eax = next; r.ecx = td.candidates_end(); r.eax = r.eax.wrapping_add(4); next = r.eax;
                    if r.eax == r.ecx { break; }
                }
                r.eax = list;
                invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7370), &[r.eax]);
                r.edx = td.debug_context(); r.eax = word(r.edx, 0xC); r.ecx = word(r.eax, 0x6C).wrapping_add(1);
                store(r.eax, 0x6C, r.ecx); r.eax = 27;
                return;
            }
            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7370), &[list]);
        }
        if best == 1 {
            r.eax = td.debug_context(); r.eax = word(r.eax, 0xC);
            store(r.eax, 4, word(r.eax, 4).wrapping_add(1));
        }
        r.eax = best;
    }
}

pub const PORTS: &[Port] = &[fc4p_0070a3d0::PORT];

#[cfg(test)]
mod tests;
