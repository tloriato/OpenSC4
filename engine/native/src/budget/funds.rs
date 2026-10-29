// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
//! Funds updates and starting balance.

use super::*;

game_fn! { fn post_funds(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x0068_FE10; }

port! {
    /// Writes funds and the shadow balance and posts only if the balance changes.
    #[port(address = 0x0069_0140, owner = "budget", name = "cSC4BudgetSimulator::SetTotalFunds")]
    pub unsafe extern "thiscall" fn fc4p_00690140(this: *mut u8, amount: i64) -> u64 {
        // SAFETY: the live simulator and its notification method.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            if sim.funds() == amount { return amount as u64; }
            sim.set_funds(amount);
            sim.set_shadow_funds(amount);
            post_funds(this, (amount as u64 >> 32) as u32)
        }
    }
}
port! {
    /// Positive withdrawals fail without writes if funds are insufficient. Zero succeeds without posting.
    /// Negative amounts are negated with wrapping arithmetic and forwarded to DepositFunds.
    #[port(address = 0x0069_0200, owner = "budget", name = "cSC4BudgetSimulator::WithdrawFunds")]
    pub unsafe extern "fastcall" fn fc4p_00690200(this: *mut u8, entry_edx: u32, amount: i64) -> u64 {
        // SAFETY: the live simulator and its money-flow methods.
        unsafe {
            if amount < 0 {
                let negated = amount.wrapping_neg() as u64;
                return virtual_regs(this, 0x1C, &[negated as u32, (negated >> 32) as u32],
                    (negated >> 32) as u32, field(this, 0));
            }
            if amount == 0 { return pair(1, entry_edx); }
            let sim = BudgetSim::from_ptr(this);
            let funds = sim.funds();
            if funds < amount {
                return pair((amount as u64 >> 32) as u32 & !0xFF, (funds as u64 >> 32) as u32);
            }
            let remaining = funds.wrapping_sub(amount);
            sim.set_funds(remaining);
            sim.set_shadow_funds(remaining);
            (post_funds(this, (remaining as u64 >> 32) as u32) & !0xFF) | 1
        }
    }
}
port! {
    /// Reads the current difficulty and copies its starting balance to both fund fields, always posting.
    #[port(address = 0x0069_0450, owner = "budget", name = "cSC4BudgetSimulator::ResetFundsToStartingLevel")]
    pub unsafe extern "thiscall" fn fc4p_00690450(this: *mut u8) -> u64 {
        // SAFETY: the live city, difficulty-indexed tuning, and notification method.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let index = vcall!(sim.city(), 0x60 => fn() -> u32);
            let source = sim.starting_funds().cast::<u8>().wrapping_add(index.wrapping_mul(8) as usize);
            // The four stores retain their individual read order even if the tuning aliases the balances.
            write_unaligned(this.add(0x28).cast::<u32>(), field(source, 0));
            write_unaligned(this.add(0x2C).cast::<u32>(), field(source, 4));
            let low: u32 = field(source, 0);
            write_unaligned(this.add(0x30).cast::<u32>(), low);
            write_unaligned(this.add(0x34).cast::<u32>(), field(source, 4));
            post_funds(this, low)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00690140::PORT, fc4p_00690200::PORT, fc4p_00690450::PORT];
