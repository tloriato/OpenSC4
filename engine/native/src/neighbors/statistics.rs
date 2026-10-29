// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Regional commodity totals, costs and history for the current city.

use super::{messages::publish_cities, pair, pointer, virtual_call, word, NeighborsSim};
use crate::abi::{call_raw, target, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_fn! { fn property_float(holder: *mut u8, id: u32, out: *mut f32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { fn date_from_time(this: *mut u8, edx: u32, time: u32) -> u64 = "fastcall" 0x0091_2DFA; }
game_fn! { fn copy_date(this: *mut u8, edx: u32, source: *mut u8) -> u64 = "fastcall" 0x0091_2E24; }
game_fn! { #[allow(dead_code)] fn destroy_date(this: *mut u8) = "thiscall" 0x0091_28BF; }
game_fn! { fn set_curve(this: *mut u8, property: *mut u8) -> u64 = "thiscall" 0x005F_4620; }
game_fn! { fn curve_output(this: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_data! { static current_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static region: *mut u8 = 0x00B4_3C94; }
game_data! { static time: *mut u8 = 0x00B4_3D5C; }
game_data! { static power: *mut u8 = 0x00B4_3DA0; }
game_data! { static water: *mut u8 = 0x00B4_3D94; }
game_data! { static garbage: *mut u8 = 0x00B4_3D9C; }
game_data! { static landfill: *mut u8 = 0x00B4_3D28; }
game_data! { static budget: *mut u8 = 0x00B4_3D64; }
game_data! { static twelve: f32 = 0x00AA_209C; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static default_cost: f64 = 0x00AB_1F80; }
game_data! { static one: f64 = 0x00A8_0AB0; }

fn double_words(value: f64) -> [u32; 2] {
    let bits = value.to_bits();
    [bits as u32, (bits >> 32) as u32]
}

unsafe fn unsigned(value: u32) -> F80 {
    let value80 = F80::from_i32(value as i32);
    if (value as i32) < 0 {
        value80 + F80::from_f32(unsafe { unsigned_bias().read() })
    } else {
        value80
    }
}

unsafe fn set_stat(city: *mut u8, slot: usize, commodity: u32, value: f64, result: u64, table_in_eax: bool) -> u64 {
    let bits = double_words(value);
    unsafe {
        virtual_call(
            city,
            slot,
            &[commodity, bits[0], bits[1]],
            if table_in_eax { word(city, 0) } else { result as u32 },
            if table_in_eax {
                (result >> 32) as u32
            } else {
                word(city, 0)
            },
        )
    }
}

unsafe fn history(history: *mut u8, id: u32, value: f64) -> u64 {
    let result = F80::from_f64(value).ftol() as u64;
    unsafe {
        virtual_call(
            history,
            0x30,
            &[id, result as u32],
            word(history, 0),
            (result >> 32) as u32,
        )
    }
}

unsafe fn destroy(date: *mut u8, result: u64) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, target(0x0091_28BF), date as u32, &[]);
    call.eax = result as u32;
    call.edx = (result >> 32) as u32;
    unsafe {
        call_raw(&mut call);
    }
    pair(call.eax, call.edx)
}

port! {
    /// Updates only the current city object. Stores finances, sums every buy deal without filtering
    /// flags, republishes deals, then stores power, water and garbage figures and history. Costs use
    /// the original x87 load, arithmetic and store sequence. Garbage reloads the shared price curve.
    /// A missing term property retains the word in the original aligned stack scratch location.
    #[port(address = 0x006D_2A20, owner = "neighbors", name = "cSC4NeighborsSimulator::UpdateCommodityStats", naked)]
    pub unsafe extern "C" fn fc4p_006d2a20() {
        core::arch::naked_asm!(
            "lea eax, [esp - 4]", "and eax, -8", "mov eax, [eax - 0x74]",
            "push eax", "push dword ptr [esp + 8]", "call {}", "ret 4", sym update,
        );
    }
}

unsafe extern "fastcall" fn update(this: *mut u8, entry_edx: u32, city: *mut u8, stack_term: u32) -> u64 {
    // SAFETY: the caller supplies the game's simulator and service objects. Required interfaces
    // and properties are used with the same assumptions as their original callers.
    unsafe {
        if city.is_null() || city != current_city().read() {
            return pair(city as u32, entry_edx);
        }
        let sim = NeighborsSim::from_ptr(this);
        let manager = region().read();
        let mut result = virtual_call(manager, 0x90, &[], word(manager, 0), entry_edx);
        let regional = result as u32 as *mut u8;
        let city = current_city().read();
        result = virtual_call(city, 0xD0, &[], result as u32, word(city, 0));
        let finance = result as u32 as *mut u8;
        let regional_table = word(regional, 0);
        result = virtual_call(finance, 0x18, &[], result as u32, word(finance, 0));
        let funds = double_words(F80::from_i64(result as i64).to_f64());
        let mut call = RawCall::new(
            Conv::Thiscall,
            word(regional_table as *mut u8, 0xCC) as usize,
            regional as u32,
            &funds,
        );
        call.eax = result as u32;
        call.edx = (result >> 32) as u32;
        call_raw(&mut call);
        let city = current_city().read();
        result = virtual_call(city, 0xD0, &[], word(city, 0), call.edx);
        let finance = result as u32 as *mut u8;
        let regional_table = word(regional, 0);
        result = virtual_call(finance, 0x40, &[], result as u32, word(finance, 0));
        let income = double_words(F80::from_i64(result as i64).to_f64());
        let mut call = RawCall::new(
            Conv::Thiscall,
            word(regional_table as *mut u8, 0xD4) as usize,
            regional as u32,
            &income,
        );
        call.eax = result as u32;
        call.edx = (result >> 32) as u32;
        call_raw(&mut call);
        result = virtual_call(this, 0x94, &[], word(this, 0), call.edx);
        let resource = result as u32 as *mut u8;
        result = virtual_call(resource, 0x18, &[], result as u32, word(resource, 0));
        let holder = result as u32 as *mut u8;
        let city = current_city().read();
        result = virtual_call(city, 0x80, &[], word(city, 0), (result >> 32) as u32);
        let history_object = result as u32 as *mut u8;
        let mut term = f32::from_bits(stack_term);
        property_float(holder, 0xEA46_934F, &mut term);
        let interval = (F80::from_f32(term) * F80::from_f32(twelve().read())).to_f32();
        let clock = time().read();
        result = virtual_call(clock, 0x1C, &[], result as u32, word(clock, 0));
        let mut today = MaybeUninit::<[u32; 4]>::uninit();
        let today = today.as_mut_ptr().cast::<u8>();
        result = date_from_time(today, (result >> 32) as u32, result as u32);
        let mut date = MaybeUninit::<[u32; 4]>::uninit();
        let date = date.as_mut_ptr().cast::<u8>();
        copy_date(date, (result >> 32) as u32, today);
        result = virtual_call(date.add(8), 0xA0, &[0x5B], date as u32, word(date, 8));
        let mut costs = [0.0f64; 3];
        let header = sim.deals();
        let mut node = pointer(header, 0);
        while node != header {
            let deal = pointer(node, 8);
            node = pointer(node, 0);
            result = virtual_call(deal, 0x58, &[], word(deal, 0), (result >> 32) as u32);
            let cost = F80::from_i64(result as i64).to_f32();
            result = virtual_call(deal, 0x20, &[], result as u32, word(deal, 0));
            let kind = result as u32;
            if let Some(index) = [0x4773_FFE0, 0x4773_FFE2, 0x4773_FFE4].iter().position(|&k| k == kind) {
                costs[index] = (F80::from_f32(cost) + F80::from_f64(costs[index])).to_f64();
            }
            result = pair(
                if [0x4773_FFE0, 0x4773_FFE2, 0x4773_FFE4].contains(&kind) {
                    0
                } else {
                    kind.wrapping_sub(0x4773_FFE4)
                },
                (result >> 32) as u32,
            );
        }
        let mut cities = 0u32;
        result = publish_cities(this, &mut cities, result, true);
        let utility = power().read();
        result = virtual_call(utility, 0x30, &[], result as u32, word(utility, 0));
        let produced = F80::from_i32(result as u32 as i32).to_f64();
        result = virtual_call(utility, 0x2C, &[], word(utility, 0), (result >> 32) as u32);
        let demanded = F80::from_i32(result as u32 as i32).to_f64();
        let imported = vcall_st0!(regional, 0xE0, 1u32).to_f64();
        let exported = vcall_st0!(regional, 0xD8, 1u32).to_f64();
        let budget_object = budget().read();
        result = virtual_call(
            budget_object,
            0x70,
            &[0x8910_BC8A],
            result as u32,
            word(budget_object, 0),
        );
        let power_department = result as u32 as *mut u8;
        let mut cost = default_cost().read();
        if !power_department.is_null() {
            result = virtual_call(power_department, 0x3C, &[], result as u32, word(power_department, 0));
            cost = ((F80::from_i64(result as i64) + F80::from_f64(costs[0]))
                / ((F80::from_f64(imported) + F80::from_f64(produced)) + F80::from_f64(one().read())))
            .to_f64();
        }
        result = set_stat(regional, 0xEC, 1, produced, result, true);
        result = set_stat(regional, 0xF4, 1, demanded, result, false);
        result = set_stat(regional, 0xDC, 1, exported, result, true);
        result = set_stat(regional, 0xE4, 1, imported, result, false);
        set_stat(regional, 0xFC, 1, cost, result, true);
        history(history_object, 0xAA11_F837, imported);
        result = history(history_object, 0xAA11_F83A, exported);
        result = virtual_call(
            power_department,
            0x50,
            &[0x1F47_0000],
            result as u32,
            word(power_department, 0),
        );
        let account = result as u32 as *mut u8;
        let mut waste_to_energy = 0.0f64;
        if !account.is_null() {
            result = virtual_call(account, 0x28, &[], result as u32, word(account, 0));
            waste_to_energy = F80::from_i64(result as i64).to_f64();
        }
        let utility = water().read();
        result = virtual_call(utility, 0x38, &[], word(utility, 0), (result >> 32) as u32);
        let produced = F80::from_i32(result as u32 as i32).to_f64();
        result = virtual_call(utility, 0x5C, &[], result as u32, word(utility, 0));
        let demanded = F80::from_i32(result as u32 as i32).to_f64();
        let imported = vcall_st0!(regional, 0xE0, 2u32).to_f64();
        let exported = vcall_st0!(regional, 0xD8, 2u32).to_f64();
        let budget_object = budget().read();
        result = virtual_call(
            budget_object,
            0x70,
            &[0xAA40_14B4],
            word(budget_object, 0),
            (result >> 32) as u32,
        );
        let department = result as u32 as *mut u8;
        let mut cost = default_cost().read();
        if !department.is_null() {
            result = virtual_call(department, 0x3C, &[], result as u32, word(department, 0));
            cost = ((F80::from_i64(result as i64) + F80::from_f64(costs[1]))
                / ((F80::from_f64(imported) + F80::from_f64(produced)) + F80::from_f64(one().read())))
            .to_f64();
        }
        result = set_stat(regional, 0xEC, 2, produced, result, true);
        result = set_stat(regional, 0xF4, 2, demanded, result, false);
        result = set_stat(regional, 0xDC, 2, exported, result, true);
        result = set_stat(regional, 0xE4, 2, imported, result, false);
        set_stat(regional, 0xFC, 2, cost, result, true);
        history(history_object, 0x0A14_EBB5, imported);
        history(history_object, 0xCA14_EBC6, exported);
        let mut used = 0u32;
        let mut capacity = 0u32;
        let utility = landfill().read();
        virtual_call(
            utility,
            0x28,
            &[&mut used as *mut u32 as u32, &mut capacity as *mut u32 as u32],
            &mut used as *mut u32 as u32,
            word(utility, 0),
        );
        let free = capacity.wrapping_sub(used);
        let utility = garbage().read();
        result = virtual_call(utility, 0x98, &[], free, word(utility, 0));
        let subtotal = F80::from_i64(result as i64).to_f32();
        result = virtual_call(utility, 0x88, &[], word(utility, 0), (result >> 32) as u32);
        let subtotal = (F80::from_i64(result as i64) + F80::from_f32(subtotal)).to_f32();
        result = virtual_call(utility, 0x80, &[], result as u32, word(utility, 0));
        let subtotal = (unsigned(result as u32) + F80::from_f32(subtotal)).to_f32();
        result = virtual_call(utility, 0x7C, &[], word(utility, 0), (result >> 32) as u32);
        let subtotal = unsigned(result as u32) + F80::from_f32(subtotal);
        let produced = (subtotal + unsigned(free) / F80::from_f32(interval)).to_f64();
        result = virtual_call(utility, 0x8C, &[], result as u32, word(utility, 0));
        let demanded = F80::from_i64(result as i64).to_f64();
        let imported = vcall_st0!(regional, 0xE0, 3u32).to_f64();
        let exported = vcall_st0!(regional, 0xD8, 3u32).to_f64();
        let budget_object = budget().read();
        result = virtual_call(
            budget_object,
            0x70,
            &[0x6A42_A8F5],
            word(budget_object, 0),
            (result >> 32) as u32,
        );
        let department = result as u32 as *mut u8;
        let mut cost = default_cost().read();
        if !department.is_null() {
            result = virtual_call(department, 0x3C, &[], result as u32, word(department, 0));
            cost = (((F80::from_i64(result as i64) + F80::from_f64(waste_to_energy)) + F80::from_f64(costs[2]))
                / ((F80::from_f64(imported) + F80::from_f64(produced)) + F80::from_f64(one().read())))
            .to_f64();
            result = virtual_call(holder, 0x24, &[0x4B61_FFC9], word(holder, 0), (result >> 32) as u32);
            let property = result as u32 as *mut u8;
            if !property.is_null() {
                set_curve(this.add(0x2C), property);
            }
            cost = curve_output(this.add(0x2C), F80::from_f64(cost).to_f32()).to_f64();
        }
        result = set_stat(regional, 0xEC, 3, produced, result, false);
        result = set_stat(regional, 0xF4, 3, demanded, result, true);
        result = set_stat(regional, 0xDC, 3, exported, result, false);
        result = set_stat(regional, 0xE4, 3, imported, result, true);
        set_stat(regional, 0xFC, 3, cost, result, false);
        history(history_object, 0xEA11_017B, exported);
        history(history_object, 0xEA11_017E, imported);
        history(history_object, 0xEB54_F32E, demanded);
        result = history(history_object, 0x4B54_F333, produced);
        result = destroy(date, result);
        destroy(today, result)
    }
}

pub const PORTS: &[Port] = &[fc4p_006d2a20::PORT];

#[cfg(test)]
mod tests;
