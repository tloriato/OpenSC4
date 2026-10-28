// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lot condition and desirability checks, including their property counters.

use super::*;

game_data! { static desirability_scale: f32 = 0x00A8_23A4; }
game_fn! { fn seed_tolerance(this: *mut u8, seed: u32) -> u64 = "thiscall" 0x0091_3A5C; }
game_fn! { fn draw_tolerance(this: *mut u8, low: i32, high: i32) -> u64 = "thiscall" 0x0091_36CA; }

layout! {
    /// The profile's first two full-width words are used by these checks.
    pub struct BuildingProfile {
        0x00 => purpose / set_purpose: u32,
        0x04 => wealth / set_wealth: i32,
    }
}

port! {
    /// Computes condition from habitation, elapsed days, lost wealth and occupancy. Abandoned lots
    /// normalize condition zero to one before their unsigned age check. Occupied lots use signed,
    /// wrapping wealth differences, strict occupancy thresholds, and the existing no-power property.
    /// The two location arguments are accepted but unused.
    #[port(address = 0x006C_4630, owner = "lotdeveloper", name = "cSC4LotDeveloper::TestCondition")]
    pub unsafe extern "fastcall" fn fc4p_006c4630(this: *mut u8, entry_edx: u32, lot: *mut u8, _x: i32, _z: i32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut call = virtual_call(lot, 0x84, &[], table(lot), entry_edx, false);
            let state = call.eax;
            call = virtual_call(lot, 0x8C, &[], call.eax, table(lot), false);
            let condition = call.eax;
            if state == 4 {
                let mut condition = if condition == 0 { 1 } else { condition };
                call = virtual_call(lot, 0xA0, &[], table(lot), call.edx, false);
                if call.eax > u32::from(developer.stay_new_days()) && condition <= 3 {
                    condition = condition.wrapping_add(1);
                }
                call.eax = condition;
                return result(call);
            }
            if state != 3 { return result(call); }
            let service = developer.building_development();
            let saved_table = table(service);
            call = virtual_call(lot, 0xB8, &[0], table(lot), service as u32, false);
            let args = [call.eax];
            let mut profile_call = RawCall::new(Conv::Thiscall,
                (saved_table as *const usize).add(0x9C / 4).read_unaligned(),
                developer.building_development() as u32, &args);
            profile_call.eax = call.eax;
            profile_call.edx = call.edx;
            call_raw(&mut profile_call);
            call = profile_call;
            let profile = BuildingProfile::from_ptr(call.eax as *mut u8);
            call = virtual_call(lot, 0xA4, &[], call.eax, table(lot), false);
            let mut loss = profile.wealth().wrapping_sub(call.eax as i32);
            let improving = (condition.wrapping_sub(1) as i32) > loss;
            let examine_occupancy = if improving { true } else {
                call = virtual_call(lot, 0xA0, &[], table(lot), call.edx, false);
                call.eax > u32::from(developer.stay_new_days())
            };
            if examine_occupancy {
                let mut occupancy = core::mem::MaybeUninit::<f32>::uninit();
                call = virtual_call(lot, 0xF4, &[occupancy.as_mut_ptr() as u32],
                    occupancy.as_mut_ptr() as u32, table(lot), false);
                let occupancy = occupancy.assume_init();
                let cmp = F80::from_f32(occupancy).compare(F80::from_f32(developer.distress_threshold()));
                call.eax = (call.eax & 0xFFFF_0000) | u32::from(cmp.status_word());
                let worsen = if cmp.less() { true } else if improving {
                    let cmp = F80::from_f32(occupancy).compare(F80::from_f32(developer.renewal_threshold()));
                    call.eax = (call.eax & 0xFFFF_0000) | u32::from(cmp.status_word());
                    cmp.less()
                } else { false };
                if worsen { loss = loss.wrapping_add(1); }
            }
            if loss == 0 {
                call = virtual_call(lot, 0x0C, &[], call.eax, table(lot), false);
                let holder = call.eax as *mut u8;
                call = virtual_call(holder, 0x24, &[0xE999_B11F], call.eax, table(holder), false);
                let property = call.eax as *mut u8;
                let unpowered = if property.is_null() { false } else {
                    call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                    let variant = call.eax as *mut u8;
                    call = virtual_call(variant, 0x124, &[], call.eax, table(variant), false);
                    (call.eax as *const u32).read_unaligned() != 0
                };
                call.eax = u32::from(unpowered);
            } else {
                call.eax = if loss > 2 { 3 } else if loss < 0 { condition } else { loss.wrapping_add(1) as u32 };
            }
            result(call)
        }
    }
}

port! {
    /// Creates a missing low-desirability counter, then checks the appropriate developer's signed grid
    /// value. Low desirability increments that counter and uses a location-seeded random tolerance.
    /// Ineligible or sufficiently desirable lots reset the counter; purpose five leaves it untouched.
    /// Only AL is replaced in the last property's return word.
    #[port(address = 0x006C_47C0, owner = "lotdeveloper", name = "cSC4LotDeveloper::TestDesirability")]
    pub unsafe extern "fastcall" fn fc4p_006c47c0(this: *mut u8, entry_edx: u32, lot: *mut u8, x: i32, z: i32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut call = virtual_call(lot, 0x0C, &[], table(lot), entry_edx, false);
            let holder = call.eax as *mut u8;
            call = virtual_call(holder, 0x24, &[0xC999_E206], call.eax, table(holder), false);
            if call.eax == 0 {
                call = virtual_call(holder, 0x34, &[0xC999_E206, 0, 0], table(holder), call.edx, false);
                call = virtual_call(holder, 0x24, &[0xC999_E206], call.eax, table(holder), false);
            }
            let property = call.eax as *mut u8;
            call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
            let variant = call.eax as *mut u8;
            call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
            let counter = (call.eax as *const u32).read_unaligned();
            call = virtual_call(lot, 0x94, &[], call.eax, table(lot), false);
            if call.eax as u8 != 0 {
                call = virtual_call(lot, 0xB8, &[0], table(lot), call.edx, false);
                let service = developer.building_development();
                call = virtual_call(service, 0x9C, &[call.eax], call.eax, table(service), false);
                let profile = BuildingProfile::from_ptr(call.eax as *mut u8);
                call = virtual_call(lot, 0xA4, &[], table(lot), call.edx, false);
                let service = developer.building_development();
                let purpose = profile.purpose();
                call = virtual_call(service, 0x78, &[purpose, call.eax], purpose, table(service), false);
                let service = developer.building_development();
                call = virtual_call(service, 0x80, &[call.eax], call.eax, table(service), false);
                let growth = call.eax as *mut u8;
                if !growth.is_null() {
                    call = virtual_call(growth, 0x1C, &[], table(growth), call.edx, false);
                    if call.eax == 5 {
                        call.eax = (call.eax & !255) | 1;
                        return result(call);
                    }
                    call = virtual_call(growth, 0x34, &[], call.eax, table(growth), false);
                    let grid = call.eax as *mut u8;
                    call = virtual_call(grid, 0x1C, &[x as u32, z as u32], call.eax, table(grid), false);
                    let desirability = F80::from_i32(i32::from(call.eax as i16)) * F80::from_f32(desirability_scale().read());
                    if desirability.compare(F80::from_f32(developer.abandonment_threshold())).less() {
                        let seed = (x as u32).wrapping_shl(16).wrapping_add(z as u32);
                        let mut random = core::mem::MaybeUninit::<[u32; 3]>::uninit();
                        let random = random.as_mut_ptr().cast::<u8>();
                        seed_tolerance(random, seed);
                        let tolerance = (draw_tolerance(random, 0, 7) as u32).wrapping_add(2);
                        let next = counter.wrapping_add(1);
                        call = virtual_call(variant, 0x68, &[next], tolerance.wrapping_sub(2), table(variant), false);
                        call.eax = (call.eax & !255) | u32::from(tolerance >= next);
                        return result(call);
                    }
                }
            }
            call = virtual_call(variant, 0x68, &[0], table(variant), call.edx, false);
            call.eax = (call.eax & !255) | 1;
            result(call)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c4630::PORT, fc4p_006c47c0::PORT];
