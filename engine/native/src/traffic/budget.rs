// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Infrastructure charges, strikes and final statistics.

use super::*;

game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static budget_bias: f32 = 0x00AB_32D8; }
game_data! { static byte_scale: f32 = 0x00A9_D8B0; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static city: *mut u8 = 0x00B4_3D5C; }
game_data! { static ticks_scale: f32 = 0x00B0_A840; }
game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { fn uniform(this: *mut u8) -> St0 = "thiscall" 0x0091_3A29; }
game_fn! { fn update_transit(this: *mut u8) -> u32 = "thiscall" 0x0070_DA40; }

/// Unsigned conversion uses a signed load followed by a rounded addition for values with bit 31 set.
unsafe fn unsigned(value: u32) -> F80 {
    let loaded = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        loaded
    } else {
        add_float(loaded, unsafe { *unsigned_bias() })
    }
}

unsafe fn charge(sim: TrafficSim, index: usize) -> u32 {
    unsafe {
        let count = unsigned(word(sim.as_ptr(), 0x740 + index * 4));
        let price = f32::from_bits(word(sim.as_ptr(), 0xB54 + index * 4));
        add_float(multiply_float(count, price), *budget_bias())
            .to_f32()
            .to_bits()
            .wrapping_add(0xB4C0_0000)
    }
}

port! {
    /// Updates road, street, highway and avenue line items. Combined costs carry into the high word.
    #[port(address = 0x0070_D8A0, owner = "traffic", name = "cSC4TrafficSimulator::UpdateRoadsBudget")]
    pub unsafe extern "thiscall" fn fc4p_0070d8a0(this: *mut u8) -> u32 {
        unsafe {
            let s = TrafficSim::from_ptr(this);
            let a = charge(s, 0);
            let b = charge(s, 10);
            let item = vcall!(s.roads(), 0x50 => fn(u32) -> *mut u8, 2);
            let (low, carry) = a.overflowing_add(b);
            vcall!(item, 0x30 => fn(u32, u32) -> u32, low, u32::from(carry));
            let a = charge(s, 3);
            let item = vcall!(s.roads(), 0x50 => fn(u32) -> *mut u8, 1);
            vcall!(item, 0x30 => fn(u32, u32) -> u32, a, 0);
            let a = charge(s, 2);
            let b = charge(s, 12);
            let item = vcall!(s.roads(), 0x50 => fn(u32) -> *mut u8, 3);
            let (low, carry) = a.overflowing_add(b);
            vcall!(item, 0x30 => fn(u32, u32) -> u32, low, u32::from(carry));
            let a = charge(s, 6);
            let item = vcall!(s.roads(), 0x50 => fn(u32) -> *mut u8, 4);
            vcall!(item, 0x30 => fn(u32, u32) -> u32, a, 0)
        }
    }
}
port! {
    /// Updates rail, subway, elevated rail and monorail charges, creating the rail line item when absent.
    #[port(address = 0x0070_DA40, owner = "traffic", name = "cSC4TrafficSimulator::UpdateMassTransitBudget")]
    pub unsafe extern "thiscall" fn fc4p_0070da40(this: *mut u8) -> u32 {
        unsafe {
            let s = TrafficSim::from_ptr(this);
            let cost = charge(s, 1);
            let mut item = vcall!(s.transit(), 0x50 => fn(u32) -> *mut u8, 3);
            if item.is_null() {
                item = vcall!(s.transit(), 0x44 => fn(u32, u32) -> *mut u8, 3, 0);
                vcall!(item, 0x1C => fn(u32, u32) -> u32, 0x6A3F_F01C, 0x6A8B_0CFB);
            }
            vcall!(item, 0x30 => fn(u32, u32) -> u32, cost, 0);
            let mut eax = 0;
            for (id, index) in [(4, 7), (5, 8), (6, 9)] {
                let item = vcall!(s.transit(), 0x50 => fn(u32) -> *mut u8, id);
                let cost = charge(s, index);
                eax = vcall!(item, 0x30 => fn(u32, u32) -> u32, cost, 0);
            }
            eax
        }
    }
}
port! {
    /// Advances the strike state and posts a retained standard message for each state transition.
    #[port(address = 0x0070_DBA0, owner = "traffic", name = "cSC4TrafficSimulator::UpdateStrikes")]
    pub unsafe extern "thiscall" fn fc4p_0070dba0(this: *mut u8) -> u32 {
        unsafe {
            let s = TrafficSim::from_ptr(this);
            let funding = vcall!(s.transit(), 0x28 => fn() -> *mut f32);
            let message_type = match s.strike_state() {
                2 => {
                    let cmp = F80::from_f32(funding.read_unaligned()).compare(F80::from_f32(s.strike_start()));
                    if !cmp.less() || s.budget_total() <= 0 { return 0; }
                    let probability = multiply_float(subtract_float(F80::from_f32(s.strike_start()), funding.read_unaligned()), s.strike_scale()).to_f32();
                    s.set_strike_probability(probability);
                    let random = uniform(s.random());
                    let cmp = F80::from_f32(probability).compare(random);
                    if cmp.less() || cmp.equal() || cmp.unordered() { return 0; }
                    s.set_strike_state(0);
                    0x6924_7DC6
                }
                0 => {
                    let cmp = F80::from_f32(funding.read_unaligned()).compare(F80::from_f32(s.strike_keep()));
                    if (cmp.less() || cmp.unordered()) && s.budget_total() != 0 {
                        s.set_strike_state(1);
                        s.set_strike_months(0);
                        s.set_striking(1);
                        0x6924_7DC4
                    } else { s.set_strike_state(2); 0x6924_7DC3 }
                }
                1 => {
                    let cmp = F80::from_f32(funding.read_unaligned()).compare(F80::from_f32(s.strike_keep()));
                    if (cmp.less() || cmp.unordered()) && s.strike_months() < s.strike_limit() && s.budget_total() != 0 {
                        s.set_strike_months(s.strike_months().wrapping_add(1));
                        return 0;
                    }
                    s.set_striking(0);
                    s.set_strike_state(2);
                    0x6924_7DC5
                }
                _ => return 0,
            };
            let allocation = allocate_message(0x2C);
            let message = if allocation.is_null() { allocation } else { construct_message(allocation) };
            vcall!(message, 4 => fn() -> u32);
            vcall!(message, 0x14 => fn(u32) -> u32, message_type);
            vcall!(message, 0x30 => fn(*mut u8) -> u32, this);
            vcall!(message, 0x84 => fn(*mut u8) -> u32, this);
            vcall!(s.messages(), 0x10 => fn(*mut u8, u32) -> u32, message, 0);
            vcall!(message, 8 => fn() -> u32)
        }
    }
}
port! {
    /// Adds or removes one subway cell (removal saturates at zero), then recalculates the mass transit budget.
    #[port(address = 0x0070_EA70, owner = "traffic", name = "cSC4TrafficSimulator::DoSubwayChanged")]
    pub unsafe extern "thiscall" fn fc4p_0070ea70(this: *mut u8, _cell: u32, added: u32) -> u32 {
        unsafe {
            let s = TrafficSim::from_ptr(this);
            let n = s.subway_cells();
            if added as u8 != 0 { s.set_subway_cells(n.wrapping_add(1)); }
            else if n != 0 { s.set_subway_cells(n.wrapping_sub(1)); }
            update_transit(this)
        }
    }
}
port! {
    /// Converts the signed tick difference to a scaled duration, with a minimum low-word result of one.
    #[port(address = 0x0070_E2B0, owner = "traffic", name = "RemainingTrafficTime")]
    pub unsafe extern "thiscall" fn fc4p_0070e2b0(this: *mut u8) -> u64 {
        unsafe {
            let ticks = core::arch::x86::_rdtsc();
            let deadline = this.cast::<u64>().read_unaligned();
            let result = multiply_float(F80::from_i64(deadline.wrapping_sub(ticks) as i64), *ticks_scale()).ftol() as u64;
            (result & 0xFFFF_FFFF_0000_0000) | u64::from((result as i32).max(1) as u32)
        }
    }
}
port! {
    /// Publishes income and commute statistics. The saved ECX is restored and AL reports completion.
    #[port(address = 0x0070_E7D0, owner = "traffic", name = "cSC4TrafficSimulator::WorkOnFinalCalculations", naked)]
    pub unsafe extern "C" fn fc4p_0070e7d0() {
        core::arch::naked_asm!("push ecx", "call {}", "pop ecx", "ret 4", sym final_calculations);
    }
}
unsafe extern "thiscall" fn final_calculations(this: *mut u8) -> u64 {
    unsafe {
        let s = TrafficSim::from_ptr(this);
        vcall!(*city(), 0x20 => fn() -> u32);
        s.set_transit_income(F80::from_f64(s.transit_accumulator()).to_f32());
        s.set_toll_income(F80::from_f64(s.toll_accumulator()).to_f32());
        let count = unsigned(s.commute_count());
        s.set_average_commute(reverse_divide_float(count, s.commute_sum()).to_f32());
        let count = s.freight_count();
        let result = if count != 0 {
            let denominator = unsigned(count);
            let average = reverse_divide_float(denominator, s.freight_sum());
            multiply_float(divide_float(average, s.commute_maximum()), *byte_scale()).ftol() as u64
        } else {
            F80::from_f32(*zero()).ftol() as u64
        };
        s.set_freight_byte(result as u8);
        (result & !255) | 1
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0070d8a0::PORT,
    fc4p_0070da40::PORT,
    fc4p_0070dba0::PORT,
    fc4p_0070e2b0::PORT,
    fc4p_0070e7d0::PORT,
    fc4p_0070ea70::PORT,
];

#[cfg(test)]
mod tests;
