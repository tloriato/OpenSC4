// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
//! Tax-rate history, default rates and income per group.

use super::*;
use crate::x87::F80;

game_data! { static history: *mut u8 = 0x00B4_3DC0; }
game_data! { static history_scale: f32 = 0x00A9_4D50; }
game_data! { static history_bias: f32 = 0x00AB_0888; }
game_data! { static inverse_1200: f32 = 0x00AB_08B8; }

// A stored operand stays in memory until its arithmetic operation, including signaling NaN handling.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let input = value.to_bytes();
            let mut output = [0u8; 10];
            // SAFETY: both operands cover their values and the single x87 register is popped.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) input.as_ptr(), operand = in(reg) &operand,
                    output = in(reg) output.as_mut_ptr(), out("st(0)") _);
            }
            F80::from_bytes(output)
        }
    };
}
stored_op!(multiply, "fmul dword ptr [{operand}]");
stored_op!(add, "fadd dword ptr [{operand}]");

port! {
    /// Records all twelve rates using the game's biased-float rounding sequence, rereading history each time.
    #[port(address = 0x0069_00F0, owner = "budget", name = "cSC4BudgetSimulator::DoDailySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006900f0(this: *mut u8) -> u64 {
        // SAFETY: the live simulator, history service and read-only constants.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let mut result = 0;
            for index in 0..12 {
                let rate = F80::from_f32(sim.tax_group(index).rate());
                let service = *history();
                let product = multiply(rate, *history_scale());
                let vtable: u32 = field(service, 0);
                let bits = add(product, *history_bias()).to_f32().to_bits();
                let value = bits.wrapping_add(0xB4C0_0000);
                let id = (index as u32).wrapping_add(0xAABA_AA9F);
                result = virtual_regs(service, 0x30, &[id, value], vtable, id);
            }
            result
        }
    }
}
port! {
    /// Reads city difficulty separately for every record and copies the selected default rate's raw word.
    #[port(address = 0x0069_0490, owner = "budget", name = "cSC4BudgetSimulator::ResetTaxRatesToDefault")]
    pub unsafe extern "fastcall" fn fc4p_00690490(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the live city and difficulty-indexed tuning.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let mut eax = 0;
            let mut edx = entry_edx;
            for index in 0..12 {
                let city = sim.city();
                let difficulty = virtual_regs(city, 0x60, &[], field(city, 0), edx) as u32;
                edx = sim.default_rates().wrapping_add(difficulty as usize).read_unaligned();
                write_unaligned(sim.tax_group(index).as_ptr().cast::<u32>(), edx);
                eax = difficulty;
            }
            pair(eax, edx)
        }
    }
}
port! {
    /// Computes assessed value times difficulty multiplier, rate and 1/1200, in that order on the x87.
    #[port(address = 0x0069_04C0, owner = "budget", name = "cSC4BudgetSimulator::GetTaxIncome")]
    pub unsafe extern "thiscall" fn fc4p_006904c0(this: *mut u8, index: i32) -> i64 {
        // SAFETY: the live city, selected tax record and tuning array.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let difficulty = vcall!(sim.city(), 0x60 => fn() -> u32);
            let multipliers = sim.tax_multipliers();
            let tax = sim.tax_group(index);
            let assessed = F80::from_i64(tax.assessed());
            let value = multiply(assessed, multipliers.wrapping_add(difficulty as usize).read_unaligned());
            multiply(multiply(value, tax.rate()), *inverse_1200()).ftol()
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006900f0::PORT, fc4p_00690490::PORT, fc4p_006904c0::PORT];
