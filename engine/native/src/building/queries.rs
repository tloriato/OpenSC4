// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Idle progression, family lookup, and maximum capacity queries.

use super::{trees::Node, BuildingSim};
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::ptr::read_unaligned as read;

game_fn! { fn plan_growth(this: *mut u8) -> u64 = "thiscall" 0x0069_9440; }
game_fn! { fn plan_decline(this: *mut u8) -> u64 = "thiscall" 0x0069_9890; }
game_fn! { #[allow(dead_code)] fn capacity_profile(this: *mut u8, key: *mut u8) -> u64 = "thiscall" 0x0069_5F10; }
game_data! { static capacity_map: [u32; 2] = 0x00B4_85AC; }
game_data! { static family_buckets: *mut *mut u8 = 0x00B4_8588; }
game_data! { static family_buckets_end: *mut *mut u8 = 0x00B4_858C; }

port! {
    /// Finds a family by unsigned remainder bucket and returns its embedded member vector.
    /// The matched node is retained in EDX, or zero when the chain is exhausted.
    #[port(address = 0x0069_85C0, owner = "building", name = "FindBuildingFamily")]
    pub unsafe extern "stdcall" fn fc4p_006985c0(family: u32) -> u64 {
        unsafe {
            let begin = read(family_buckets());
            let end = read(family_buckets_end());
            let count = ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32;
            // Hardware division also retains the divide error for an empty bucket array.
            let bucket: u32;
            core::arch::asm!("div {count}", count = in(reg) count, inout("eax") family => _,
                inout("edx") 0u32 => bucket, options(nomem, nostack));
            let mut p = read(begin.wrapping_add(bucket as usize));
            while !p.is_null() && read(p.add(8).cast::<u32>()) != family { p = read(p.cast::<*mut u8>()); }
            let eax = if p.is_null() { 0 } else { (p as u32).wrapping_add(16) };
            (u64::from(p as u32) << 32) | u64::from(eax)
        }
    }
}

port! {
    /// Advances developers until one yields, then performs growth and decline on separate idle entries.
    /// The unused idle-budget argument is popped. AL and the upper EAX bytes retain each branch's result.
    #[port(address = 0x0069_9EA0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::DoIdle")]
    pub unsafe extern "fastcall" fn fc4p_00699ea0(this: *mut u8, entry_edx: u32, _budget: u32) -> u64 {
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let mut eax = sim.state().wrapping_sub(1);
            let mut edx = entry_edx;
            if eax > 3 { return (u64::from(edx) << 32) | u64::from(eax); }
            if sim.state() == 1 {
                eax = sim.developers() as u32;
                let first = Node::from_ptr(sim.developers()).left();
                sim.set_developer_iterator(first);
                sim.set_state(2);
            }
            if sim.state() == 2 {
                edx = sim.developer_iterator() as u32;
                while sim.developer_iterator() != sim.developers() {
                    let n = Node::from_ptr(sim.developer_iterator());
                    let developer = n.value() as *mut u8;
                    let result = vcall!(developer, 0x70 => fn() -> u64);
                    eax = result as u32; edx = (result >> 32) as u32;
                    if eax as u8 == 0 { return (u64::from(edx) << 32) | u64::from(eax); }
                    let mut p = sim.developer_iterator();
                    let right = Node::from_ptr(p).right();
                    if !right.is_null() {
                        p = right;
                        while !Node::from_ptr(p).left().is_null() { p = Node::from_ptr(p).left(); }
                    } else {
                        let mut parent = Node::from_ptr(p).parent();
                        while p == Node::from_ptr(parent).right() { p = parent; parent = Node::from_ptr(parent).parent(); }
                        if Node::from_ptr(p).right() != parent { p = parent; }
                    }
                    sim.set_developer_iterator(p);
                    eax = p as u32;
                }
                sim.set_state(3);
            }
            if sim.state() == 3 {
                eax = (eax & !0xFF) | u32::from(sim.use_growth_plan());
                if eax as u8 != 0 {
                    let result = plan_growth(this);
                    eax = result as u32; edx = (result >> 32) as u32;
                }
                sim.set_state(4);
            } else if sim.state() == 4 {
                let result = plan_decline(this);
                eax = result as u32; edx = (result >> 32) as u32;
                sim.set_state(0);
            }
            (u64::from(edx) << 32) | u64::from(eax)
        }
    }
}

port! {
    /// Returns an integer-truncated capacity in ST0, preserving the lookup or conversion in EDX:EAX.
    #[port(address = 0x0069_6C20, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetMaxCapacity", naked)]
    pub unsafe extern "C" fn fc4p_00696c20() {
        core::arch::naked_asm!(
            "sub esp, 12", "lea eax, [esp]", "push dword ptr [esp + 24]",
            "push dword ptr [esp + 24]", "push dword ptr [esp + 24]", "push eax",
            "call {body}", "fld tbyte ptr [esp]", "add esp, 12", "ret 12",
            body = sym max_capacity,
        );
    }
}

unsafe extern "fastcall" fn max_capacity(
    this: *mut u8,
    _entry_edx: u32,
    output: *mut F80,
    developer: u32,
    purpose: u32,
    wealth: u32,
) -> u64 {
    unsafe {
        let args = [purpose, wealth];
        let mut c = RawCall::new(Conv::Thiscall, vslot(this, 0xA8), this as u32, &args);
        c.eax = purpose;
        c.edx = read(this.cast::<u32>());
        call_raw(&mut c);
        let mut key = [developer, c.eax & 0xFF];
        let args = [key.as_mut_ptr() as u32];
        let mut lookup = RawCall::new(
            Conv::Thiscall,
            crate::abi::target(0x0069_5F10),
            capacity_map() as u32,
            &args,
        );
        lookup.eax = c.eax;
        lookup.edx = c.edx;
        call_raw(&mut lookup);
        let mut value = 0i32;
        if lookup.eax != read(capacity_map().cast::<u32>()) {
            let converted = F80::from_f32(read((lookup.eax as *const u8).add(24).cast::<f32>())).ftol();
            value = converted as i32;
            lookup.eax = converted as u32;
            lookup.edx = ((converted as u64) >> 32) as u32;
        }
        output.write(F80::from_i32(value));
        (u64::from(lookup.edx) << 32) | u64::from(lookup.eax)
    }
}

pub const PORTS: &[Port] = &[fc4p_00696c20::PORT, fc4p_006985c0::PORT, fc4p_00699ea0::PORT];

#[cfg(test)]
mod tests;
