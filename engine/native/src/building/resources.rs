// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Construction demand and the monthly resource supply.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

game_data! {
    static demand_simulator: *mut u8 = 0x00B4_3D74;
}

/// Captures both integer return registers as well as an optional unrounded ST0 result.
unsafe fn demand_call(node: *mut u8, slot: usize, args: &[u32], floating: bool, eax: u32, edx: u32) -> (u64, F80) {
    // SAFETY: the enclosing port supplies a live demand interface and the slot's exact stack words.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(node, slot), node as u32, args);
        call.eax = eax;
        call.edx = edx;
        call.take_st0 = u32::from(floating);
        call_raw(&mut call);
        ((u64::from(call.edx) << 32) | u64::from(call.eax), call.st0)
    }
}

port! {
    /// Returns the city's selected stage multiplier in ST0 while retaining the city's EDX:EAX.
    #[port(address = 0x0069_6040, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetStagePopulationMultiplier", naked)]
    pub unsafe extern "C" fn fc4p_00696040() {
        core::arch::naked_asm!(
            "sub esp, 12", "lea eax, [esp]", "push eax", "call {body}",
            "fld tbyte ptr [esp]", "add esp, 12", "ret", body = sym stage_multiplier,
        );
    }
}

/// # Safety
/// The entry supplies a live simulator and storage for the ST0 result.
unsafe extern "fastcall" fn stage_multiplier(this: *mut u8, entry_edx: u32, out: *mut F80) -> u64 {
    // SAFETY: the city selects an accessible entry in the simulator's multiplier table.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let city = sim.city();
        let mut call = RawCall::new(Conv::Thiscall, vslot(city, 0x60), city as u32, &[]);
        call.eax = read_unaligned(city.cast());
        call.edx = entry_edx;
        call_raw(&mut call);
        let value = read_unaligned(
            sim.population_multipliers()
                .cast::<u8>()
                .wrapping_add(call.eax.wrapping_mul(4) as usize)
                .cast::<f32>(),
        );
        write_unaligned(out, F80::from_f32(value));
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

port! {
    /// Truncates float32 supply minus unrounded demand; clamps only the signed low word to zero.
    #[port(address = 0x0069_58E0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetFreeConstructionResources")]
    pub unsafe extern "thiscall" fn fc4p_006958e0(this: *mut u8) -> u64 {
        // SAFETY: the simulator owns a live demand node.
        unsafe {
            let node = BuildingSim::from_ptr(this).construction();
            let supply = demand_call(node, 0x20, &[], true, read_unaligned(node.cast()), 0).1.to_f32();
            let demand = demand_call(node, 0x24, &[], true, 0, read_unaligned(node.cast())).1;
            let result = (F80::from_f32(supply) - demand).ftol() as u64;
            if (result as i32) < 0 { result & 0xFFFF_FFFF_0000_0000 } else { result }
        }
    }
}

port! {
    /// Truncates the supply to the helper's full EDX:EAX integer result.
    #[port(address = 0x0069_5910, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetTotalConstructionResources")]
    pub unsafe extern "thiscall" fn fc4p_00695910(this: *mut u8) -> i64 {
        // SAFETY: the simulator owns a live demand node.
        unsafe {
            let node = BuildingSim::from_ptr(this).construction();
            demand_call(node, 0x20, &[], true, read_unaligned(node.cast()), 0).1.ftol()
        }
    }
}

port! {
    /// Passes the caller's amount word by address because conversion overwrites that word.
    #[port(address = 0x0069_5920, owner = "building", name = "cSC4BuildingDevelopmentSimulator::ClaimConstructionResources", naked)]
    pub unsafe extern "C" fn fc4p_00695920() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {body}", body = sym claim);
    }
}

/// # Safety
/// The entry supplies its original writable stack argument and a live simulator.
unsafe extern "fastcall" fn claim(this: *mut u8, amount_word: *mut u32, amount: u32) -> u64 {
    // SAFETY: the entry's contract and the demand interface's method signatures.
    unsafe {
        let count = amount & 0xFFFF;
        write_unaligned(amount_word, count);
        let sim = BuildingSim::from_ptr(this);
        let node = sim.construction();
        let count = F80::from_i32(count as i32).to_f32();
        write_unaligned(amount_word, count.to_bits());
        let (demand_regs, demand) = demand_call(node, 0x24, &[], true, amount & 0xFFFF, read_unaligned(node.cast()));
        let prospective = (demand + F80::from_f32(f32::from_bits(read_unaligned(amount_word)))).to_f32();
        let (supply_regs, supply) = demand_call(
            node,
            0x20,
            &[],
            true,
            read_unaligned(node.cast()),
            (demand_regs >> 32) as u32,
        );
        let comparison = supply.compare(F80::from_f32(prospective));
        if comparison.status_word() & 0x0100 != 0 {
            let eax = (supply_regs as u32 & 0xFFFF_0000) | u32::from(comparison.status_word() & 0xFF00);
            return (supply_regs & 0xFFFF_FFFF_0000_0000) | u64::from(eax);
        }
        let node = sim.construction();
        let changed = demand_call(
            node,
            0x44,
            &[read_unaligned(amount_word)],
            false,
            read_unaligned(amount_word),
            read_unaligned(node.cast()),
        )
        .0;
        let node = sim.construction();
        let result = demand_call(node, 0x78, &[], false, changed as u32, read_unaligned(node.cast())).0;
        (result & !255) | 1
    }
}

port! {
    /// The writable amount word receives the negated uint16 count before demand changes.
    #[port(address = 0x0069_5980, owner = "building", name = "cSC4BuildingDevelopmentSimulator::ReleaseConstructionResources", naked)]
    pub unsafe extern "C" fn fc4p_00695980() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {body}", body = sym release_resources);
    }
}

/// # Safety
/// The entry supplies its original writable stack argument and a live simulator.
unsafe extern "fastcall" fn release_resources(this: *mut u8, amount_word: *mut u32, amount: u32) -> u64 {
    // SAFETY: the entry's contract and the demand interface's method signatures.
    unsafe {
        let count = (amount & 0xFFFF).wrapping_neg();
        let sim = BuildingSim::from_ptr(this);
        let node = sim.construction();
        write_unaligned(amount_word, count);
        let change = F80::from_i32(read_unaligned(amount_word) as i32).to_f32();
        let changed = demand_call(
            node,
            0x44,
            &[change.to_bits()],
            false,
            read_unaligned(node.cast()),
            count,
        )
        .0;
        let node = sim.construction();
        demand_call(
            node,
            0x78,
            &[],
            false,
            read_unaligned(node.cast()),
            (changed >> 32) as u32,
        )
        .0
    }
}

port! {
    /// Once per cycle, recomputes resource supply, updates developers in map order, then sets state 1.
    /// The census additions run industry + commerce + residences before multiplication.
    #[port(address = 0x0069_6130, owner = "building", name = "cSC4BuildingDevelopmentSimulator::DoMonthlySimulation")]
    pub unsafe extern "thiscall" fn fc4p_00696130(this: *mut u8) -> u32 {
        // SAFETY: the simulator owns its demand interface and ordered developer map.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            if sim.state() != 0 { return sim.state(); }
            let census = read_unaligned(demand_simulator());
            let residential = vcall!(census, 0x18 => fn(u32, u32) -> *mut u8, 0x1001, 0);
            let residences = vcall_st0!(residential, 0x20).to_f32();
            let commercial = vcall!(census, 0x18 => fn(u32, u32) -> *mut u8, 0x3001, 0);
            let commerce = vcall_st0!(commercial, 0x20).to_f32();
            let industrial = vcall!(census, 0x18 => fn(u32, u32) -> *mut u8, 0x4001, 0);
            let industry = vcall_st0!(industrial, 0x20);
            let total = ((industry + F80::from_f32(commerce) + F80::from_f32(residences))
                * F80::from_f32(sim.resource_fraction())).ftol() as u32;
            let supply = total.max(sim.resource_min()).min(sim.resource_max());
            let supply = F80::from_i32(supply as i32).to_f32();
            vcall!(sim.construction(), 0x48 => fn(f32), supply);
            let header = sim.developers();
            let mut current = read_unaligned(header.add(8).cast::<*mut u8>());
            if current == header {
                sim.set_state(1);
                return header as u32;
            }
            loop {
                let developer = read_unaligned(current.add(0x14).cast::<*mut u8>());
                vcall!(developer, 0x6C => fn());
                let right = read_unaligned(current.add(12).cast::<*mut u8>());
                let mut next = current;
                if !right.is_null() {
                    next = right;
                    loop {
                        let left = read_unaligned(next.add(8).cast::<*mut u8>());
                        if left.is_null() { break; }
                        next = left;
                    }
                } else {
                    let mut parent = read_unaligned(current.add(4).cast::<*mut u8>());
                    while next == read_unaligned(parent.add(12).cast::<*mut u8>()) {
                        next = parent;
                        parent = read_unaligned(parent.add(4).cast::<*mut u8>());
                    }
                    if read_unaligned(next.add(12).cast::<*mut u8>()) != parent { next = parent; }
                }
                current = next;
                if current == sim.developers() { break; }
            }
            sim.set_state(1);
            current as u32
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006958e0::PORT,
    fc4p_00695910::PORT,
    fc4p_00695920::PORT,
    fc4p_00695980::PORT,
    fc4p_00696040::PORT,
    fc4p_00696130::PORT,
];

#[cfg(test)]
mod tests;
