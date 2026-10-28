// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Per-tract population updates with a temporary game random generator.

use super::*;
use crate::x87::F80;

game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn clear_list(list: u32) -> u32 = "thiscall" 0x0047_3EC0; }
game_fn! { #[allow(dead_code)] fn construct_random(this: u32, seed: u32) -> u32 = "thiscall" 0x0091_3A5C; }
game_fn! { #[allow(dead_code)] fn random_range(this: u32, low_lo: u32, low_hi: u32, high_lo: u32, high_hi: u32) -> St0 = "thiscall" 0x0091_3761; }
game_data! { static percent: f32 = 0x00A8_23A4; }
game_data! { static double_one: f64 = 0x00A8_0AB0; }
game_data! { static float_zero: f32 = 0x00A8_1054; }
game_data! { static float_one: f32 = 0x00A8_1228; }
game_data! { static population_bias: f32 = 0x00AB_3048; }

unsafe fn draw<const N: usize>(r: &mut Entry<N>, low: F80, high: F80) -> F80 {
    let low = low.to_f64().to_bits();
    let high = high.to_f64().to_bits();
    let args = [low as u32, (low >> 32) as u32, high as u32, (high >> 32) as u32];
    let mut c = RawCall::new(Conv::Thiscall, crate::abi::target(0x0091_3761), r.ecx, &args);
    c.eax = r.eax;
    c.edx = r.edx;
    c.take_st0 = 1;
    // SAFETY: the game range method takes two stored doubles and returns exactly one x87 value.
    unsafe {
        crate::abi::call_raw(&mut c);
    }
    r.eax = c.eax;
    r.edx = c.edx;
    c.st0
}

register_port! {
    /// List and retain lots in the profile's tract. A temporary generator uses the wrapped tract
    /// seed, including zero. Eligible occupied lots draw once for occupancy and again at or above
    /// full occupancy. Store the rounded float's raw bits as the population argument.
    0x0070_6BC0, fc4p_00706bc0, update_population, 1, 4, "cSC4TractDeveloper::UpdatePopulation", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let profile = r.args[0];
        r.ecx = word(profile, 0); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x34) as usize, &[]);
        let map = r.eax;
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
        let shift = r.eax & 31;
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0xC]);
        store(r.eax, 0, r.eax); store(r.eax, 4, r.eax);
        let mut list = r.eax;
        r.ecx = word(profile, 4);
        r.edx = word(word(this, 0x2C), 0);
        r.eax = word(profile, 8);
        let z_max = r.eax.wrapping_add(1).wrapping_shl(shift).wrapping_sub(1);
        r.ecx = shift;
        let x = word(profile, 4);
        let x_max = x.wrapping_add(1).wrapping_shl(shift).wrapping_sub(1);
        r.eax = r.eax.wrapping_shl(shift);
        let z_min = r.eax;
        r.eax = x.wrapping_shl(shift);
        r.ecx = word(this, 0x2C);
        let x_min = r.eax;
        r.eax = &mut list as *mut u32 as u32;
        invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[r.eax, x_min, z_min, x_max, z_max, 1]);
        r.eax = list;
        if word(r.eax, 0) == r.eax {
            r.ecx = &mut list as *mut u32 as u32;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0047_3EC0), &[]);
            r.eax = list;
            if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
            return;
        }
        r.eax = word(profile, 8); r.ecx = map; r.edx = word(r.ecx, 0);
        let z = r.eax;
        r.eax = word(profile, 4);
        invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[r.eax, z]);
        r.ecx = r.eax as u16 as i16 as i32 as u32;
        let demand = (F80::from_i32(r.ecx as i32) * F80::from_f32(read_unaligned(percent()))).to_f32();
        r.ecx = word(profile, 0); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[]);
        let developer_type = r.eax;
        r.ecx = word(profile, 0); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x20) as usize, &[]);
        let wealth = r.eax;
        r.ecx = word(profile, 4).wrapping_shl(16).wrapping_add(word(profile, 8)).wrapping_shl(4);
        let seed = r.ecx;
        let mut random = [0u64; 2];
        r.ecx = random.as_mut_ptr() as u32;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0091_3A5C), &[seed]);
        r.eax = list;
        let mut node = word(r.eax, 0);
        while node != r.eax {
            let lot = word(node, 8);
            r.edx = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x98) as usize, &[]);
            if r.eax as u8 != 0 {
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0xA4) as usize, &[]);
                if r.eax == wealth {
                    r.eax = developer_type; r.edx = word(lot, 0); r.ecx = lot;
                    invoke(r, Conv::Thiscall, word(r.edx, 0xD0) as usize, &[r.eax, 0]);
                    let capacity = r.eax;
                    if capacity as u16 != 0 {
                        r.eax = this;
                        let variation = F80::from_f32(td.occupancy_variation());
                        r.ecx = random.as_mut_ptr() as u32;
                        let uniform = draw(r, -variation, variation);
                        let mut occupancy = (uniform + F80::from_f64(read_unaligned(double_one()))) * F80::from_f32(demand);
                        let comparison = occupancy.compare(F80::from_f32(read_unaligned(float_zero())));
                        r.eax = (r.eax & 0xFFFF_0000) | u32::from(comparison.status_word());
                        if comparison.less() {
                            occupancy = F80::from_f32(read_unaligned(float_zero()));
                        } else {
                            let comparison = occupancy.compare(F80::from_f32(read_unaligned(float_one())));
                            r.eax = (r.eax & 0xFFFF_0000) | u32::from(comparison.status_word());
                            if !comparison.less() && !comparison.unordered() {
                                r.eax = this; r.ecx = random.as_mut_ptr() as u32;
                                let vacancy = draw(r, F80::from_f32(td.minimum_vacancy()), F80::from_f32(td.maximum_vacancy()));
                                occupancy = F80::from_f64(read_unaligned(double_one())) - vacancy;
                            }
                        }
                        r.edx = word(lot, 0); r.ecx = u32::from(capacity as u16);
                        let population = (F80::from_i32(r.ecx as i32) * occupancy + F80::from_f32(read_unaligned(population_bias()))).to_f32();
                        r.ecx = developer_type; r.eax = population.to_bits();
                        let value = r.eax; let kind = r.ecx; r.ecx = lot;
                        invoke(r, Conv::Thiscall, word(r.edx, 0xDC) as usize, &[kind, value]);
                    }
                }
            }
            node = word(node, 0); r.eax = list;
        }
        super::growth::dispose_list(r, &list);
    }
}

pub const PORTS: &[Port] = &[fc4p_00706bc0::PORT];

#[cfg(test)]
mod tests;
