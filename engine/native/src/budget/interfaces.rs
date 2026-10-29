// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
//! Interface lifetime and funds-change notification.

use super::*;

game_fn! { fn allocate_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> u64 = "thiscall" 0x0091_34D6; }

port! {
    /// Exposes the primary, message and serializable interfaces, retaining AddRef's upper EAX bytes.
    /// An unknown IID leaves the output untouched and returns the comparison arithmetic with AL cleared.
    #[port(address = 0x0068_FC80, owner = "budget", name = "cSC4BudgetSimulator::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_0068fc80(this: *mut u8, edx: u32, iid: u32, out: *mut *mut u8) -> u64 {
        let offset = match iid {
            1 | 0x6772_BD23 => 0,
            0x4522_94AA => 4,
            0xE4FD_A3D4 => 8,
            _ => {
                let eax = if iid > 0x6772_BD23 { iid } else { iid.wrapping_sub(0x4522_94AA) };
                return pair(eax & !0xFF, edx);
            }
        };
        let interface = if this.is_null() { this } else { this.wrapping_add(offset) };
        // SAFETY: the output word and primary interface are supplied by the game.
        unsafe {
            out.write_unaligned(interface);
            let result = virtual_regs(this, 4, &[], field(this, 0), out as u32);
            (result & !0xFF) | 1
        }
    }
}
port! {
    /// Posts a retained standard message carrying the simulator in both pointer fields, then releases it.
    /// Without a message server EAX is zero and EDX is unchanged.
    #[port(address = 0x0068_FE10, owner = "budget", name = "cSC4BudgetSimulator::PostFundsChangedMessage")]
    pub unsafe extern "fastcall" fn fc4p_0068fe10(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: the simulator owns its live message server. Allocation failure follows the same null call.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            if sim.messages().is_null() { return pair(0, edx); }
            let storage = allocate_message(0x2C);
            let constructed = if storage as u32 == 0 { storage & 0xFFFF_FFFF_0000_0000 }
                else { construct_message(storage as u32 as *mut u8) };
            let message = constructed as u32 as *mut u8;
            let addref = virtual_regs(message, 4, &[], field(message, 0), (constructed >> 32) as u32);
            let typed = virtual_regs(message, 0x14, &[0x0772_FAD4], addref as u32, field(message, 0));
            let pointer = virtual_regs(message, 0x30, &[this as u32], field(message, 0), (typed >> 32) as u32);
            let unknown = virtual_regs(message, 0x84, &[this as u32], pointer as u32, field(message, 0));
            let server = sim.messages();
            let posted = virtual_regs(server, 0x10, &[message as u32, 0], field(server, 0), (unknown >> 32) as u32);
            virtual_regs(message, 8, &[], posted as u32, field(message, 0))
        }
    }
}
port! {
    /// Decrements signed counts above one, otherwise calls Shutdown and the deleting destructor with flag one.
    #[port(address = 0x0069_00C0, owner = "budget", name = "cSC4BudgetSimulator::Release")]
    pub unsafe extern "fastcall" fn fc4p_006900c0(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: the live simulator and its two lifetime slots.
        unsafe {
            let sim = BudgetSim::from_ptr(this);
            let count = sim.references();
            if count > 1 {
                let next = count.wrapping_sub(1);
                sim.set_references(next);
                return pair(next as u32, edx);
            }
            let stopped = virtual_regs(this, 0x10, &[], field(this, 0), edx);
            let deleted = virtual_regs(this, 0xEC, &[1], stopped as u32, field(this, 0));
            deleted & 0xFFFF_FFFF_0000_0000
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0068fc80::PORT, fc4p_0068fe10::PORT, fc4p_006900c0::PORT];
