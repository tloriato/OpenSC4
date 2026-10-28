// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Incremental capacity rollover and edge-density clearing.

use super::*;
use super::network::registers;
use crate::abi::{Conv, RawCall};

layout! {
    /// Map-pass fields in the traffic simulator.
    pub struct PassSim {
        0x7C => network_rows / set_network_rows: *mut u8,
        0x104 => ferry_header / set_ferry_header: *mut u8,
        0x17C => edge_buckets / set_edge_buckets: *mut u8,
        0x180 => edge_buckets_end / set_edge_buckets_end: *mut u8,
        0x704 => switches / set_switches: *mut u8,
        0x818 => cursor / set_cursor: *mut u8,
    }
}
layout! {
    /// Inclusive cell scan, with x as the inner dimension.
    pub struct PassScan {
        0x00 => state / set_state: u8,
        0x04 => x / set_x: i32,
        0x08 => z / set_z: i32,
        0x0C => step / set_step: i32,
        0x10 => minimum_x / set_minimum_x: i32,
        0x18 => maximum_x / set_maximum_x: i32,
        0x1C => maximum_z / set_maximum_z: i32,
    }
}

game_fn! { #[allow(dead_code)] fn budget_expired(this: *mut u32) -> u32 = "thiscall" 0x0068_BF80; }
game_fn! { #[allow(dead_code)] fn set_capacity(index: u32) -> u32 = "C" 0x0070_DE70; }
game_data! { static clear_pacing: [u32; 2] = 0x00B4_B300; }
game_data! { static clear_initialized: u32 = 0x00B4_B308; }
game_data! { static edge_pacing: [u32; 2] = 0x00B4_B330; }
game_data! { static edge_initialized: u32 = 0x00B4_B338; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }

// The budget scope occupies the last 24 bytes below the entry stack pointer. Keeping it there
// also preserves the scope pointer that the budget helpers can leave in ECX.
macro_rules! pass_port {
    ($address:literal, $export:ident, $name:literal, $body:ident) => {
        port! {
            #[port(address = $address, owner = "traffic", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 24", "push ebp", "mov ebp, esp", "sub esp, 12", "lea edx, [esp]",
                    "lea eax, [ebp + 4]", "push eax", "push dword ptr [ebp + 32]", "call {body}",
                    "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
                    "mov esp, ebp", "pop ebp", "add esp, 24", "ret 4", body = sym $body,
                );
            }
        }
    };
}
pub(super) use pass_port;

pub(super) fn byte(value: u32, low: u8) -> u32 { (value & !255) | u32::from(low) }

pub(super) unsafe fn scope(budget: *mut u32, pacing: *mut [u32; 2], initialized: *mut u32) -> [u32; 6] {
    unsafe {
        if initialized.cast::<u8>().read() & 1 == 0 {
            write_unaligned(initialized, read_unaligned(initialized) | 1);
            write_unaligned(pacing, [1024, 0]);
        }
        [read_unaligned(budget), read_unaligned(budget.add(1)), 0,
            read_unaligned(pacing.cast::<u32>()), 0, pacing as u32]
    }
}

/// Advances the original cursor and the volatile registers it leaves around the scan.
pub(super) unsafe fn advance(scan: PassScan, result: &mut Registers) -> bool {
    result.eax = scan.as_ptr() as u32;
    result.ecx = byte(result.ecx, scan.state());
    if scan.state() == 2 { return false; }
    if scan.state() == 0 { scan.set_state(1); return true; }
    let x = scan.x().wrapping_add(scan.step());
    result.ecx = scan.step() as u32;
    result.edx = x as u32;
    scan.set_x(x);
    if x > scan.maximum_x() {
        let z = scan.z().wrapping_add(scan.step());
        scan.set_x(scan.minimum_x());
        scan.set_z(z);
        result.ecx = z as u32;
        result.edx = scan.maximum_z() as u32;
        if z > scan.maximum_z() { scan.set_state(2); return false; }
    }
    true
}

/// The inline pacing update preserves upper EAX bytes and only replaces AL with completion.
pub(super) unsafe fn paced_return(pacing: *mut [u32; 2], work: u32, completed: bool, double_in_ecx: bool, mut result: Registers) -> Registers {
    unsafe {
        let p = pacing.cast::<u8>();
        result.eax = work >> 10;
        if result.eax != 0 {
            if result.eax as i32 > 20 {
                result.eax = word(p, 4).wrapping_add(1);
                store(p, 4, result.eax);
                if result.eax as i32 >= 16 {
                    result.eax = word(p, 0);
                    store(p, 4, 0);
                    if result.eax > 1 { result.eax >>= 1; store(p, 0, result.eax); }
                }
            } else if (result.eax as i32) < 5 {
                result.eax = word(p, 4).wrapping_sub(1);
                store(p, 4, result.eax);
                if result.eax as i32 <= -4 {
                    result.eax = word(p, 0);
                    store(p, 4, 0);
                    if result.eax < 1024 {
                        let doubled = result.eax.wrapping_mul(2);
                        if double_in_ecx { result.ecx = doubled; } else { result.edx = doubled; }
                        store(p, 0, doubled);
                    }
                }
            }
        }
        result.eax = byte(result.eax, u8::from(completed));
        result
    }
}

pub(super) unsafe fn check_budget(scope: &mut [u32; 6], result: Registers) -> Registers {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0068_BF80), scope.as_mut_ptr() as u32, &[]);
    call.eax = result.eax;
    call.edx = result.edx;
    unsafe { registers(call) }
}

unsafe fn write_capacity(holder: *mut u8, index: u32, value: u32, result: Registers) -> Registers {
    let mut frame = [crate::abi::target(0x0070_DE70) as u32, holder as u32, value, index,
        result.eax, result.ecx, result.edx];
    unsafe {
        core::arch::asm!("push esi", "push edi", "push ebx", "mov esi, ecx",
            "mov edi, [esi + 4]", "mov ebx, [esi + 8]", "push dword ptr [esi + 12]",
            "mov eax, [esi + 16]", "mov edx, [esi + 24]", "mov ecx, [esi + 20]",
            "call dword ptr [esi]", "add esp, 4", "mov [esi + 16], eax", "mov [esi + 20], ecx",
            "mov [esi + 24], edx", "pop ebx", "pop edi", "pop esi",
            in("ecx") frame.as_mut_ptr(), clobber_abi("C"));
    }
    Registers { eax: frame[4], ecx: frame[5], edx: frame[6] }
}

pass_port!(0x0070_E4F0, fc4p_0070e4f0, "cSC4TrafficSimulator::WorkOnClearTripDestinations", clear_destinations);
unsafe extern "fastcall" fn clear_destinations(this: *mut u8, out: *mut Registers, budget: *mut u32, scope_pointer: *mut [u32; 6]) {
    unsafe {
        scope_pointer.write(scope(budget, clear_pacing(), clear_initialized()));
        let scope = &mut *scope_pointer;
        let sim = PassSim::from_ptr(this);
        let manager = read_unaligned(lot_manager());
        let mut work = 0u32;
        let mut result = Registers { eax: scope[3], ecx: scope[0], edx: scope[1] };
        let completed = loop {
            work = work.wrapping_add(scope[3]);
            scope[2] = work;
            if work & 1023 == 0 {
                result = check_budget(scope, result);
                if result.eax as u8 != 0 { break false; }
            }
            result.ecx = this as u32;
            let scan = PassScan::from_ptr(sim.cursor());
            if !advance(scan, &mut result) { break true; }
            result = virtual_registers(manager, 0x18, &[scan.x() as u32, scan.z() as u32, 1], scan.z() as u32, word(manager, 0));
            if result.eax == 0 { continue; }
            let lot = result.eax as *mut u8;
            result = virtual_registers(lot, 0x0C, &[], lot as u32, word(lot, 0));
            let holder = result.eax as *mut u8;
            if holder.is_null() { continue; }
            for index in 4..8u32 {
                result = virtual_registers(holder, 0x24, &[0x8A0B_4100], word(holder, 0), result.edx);
                let value = if result.eax == 0 { 0 } else {
                    let property = result.eax as *mut u8;
                    result = virtual_registers(property, 0x14, &[], property as u32, word(property, 0));
                    let variant = result.eax as *mut u8;
                    result = virtual_registers(variant, 0x0C, &[], word(variant, 0), result.edx);
                    if result.eax == 0x8004 {
                        result = virtual_registers(variant, 0x18C, &[], result.eax, word(variant, 0));
                        result.ecx = index.wrapping_mul(2);
                        u32::from(read_unaligned((result.eax as *mut u8).wrapping_add(index.wrapping_mul(2) as usize).cast::<u16>()))
                    } else {
                        result = virtual_registers(variant, 0x19C, &[], result.eax, word(variant, 0));
                        result.ecx = index.wrapping_mul(4);
                        word(result.eax as *mut u8, index.wrapping_mul(4) as usize)
                    }
                };
                result.edx = index + 4;
                result = write_capacity(holder, index + 4, value, result);
                result = write_capacity(holder, index, 0, result);
            }
            for category in 0..3u32 {
                result = virtual_registers(this, 0x50, &[holder as u32, category], word(this, 0), result.edx);
                let counter = this.add(0x1C0 + category as usize * 4);
                store(counter, 0, word(counter, 0).wrapping_add(result.eax));
            }
            work = scope[2];
        };
        out.write(paced_return(clear_pacing(), work, completed, completed, result));
    }
}

/// Looks up each of the eighteen keys and clears only the first matching record's current counters.
unsafe fn clear_edges(sim: PassSim, x: u8, z: u8, result: &mut Registers) {
    unsafe {
        for mode in 0..9u32 {
            for evening in 0..2u32 {
                let key = evening << 24 | mode << 16 | u32::from(x) << 8 | u32::from(z);
                let count = ((sim.edge_buckets_end() as u32).wrapping_sub(sim.edge_buckets() as u32) as i32 >> 2) as u32;
                let remainder: u32;
                core::arch::asm!("xor edx, edx", "div {count:e}", count = in(reg) count,
                    inout("eax") key => _, out("edx") remainder, options(nomem, nostack));
                result.eax = sim.edge_buckets() as u32;
                result.ecx = key;
                let mut node = word(sim.edge_buckets(), remainder.wrapping_mul(4) as usize) as *mut u8;
                while !node.is_null() && word(node, 4) != key { node = word(node, 0) as *mut u8; }
                result.edx = node as u32;
                if !node.is_null() {
                    result.eax = node.wrapping_add(0x2A) as u32;
                    result.ecx = 0;
                    for offset in (0..32).step_by(4) { store(node, 0x2A + offset, 0); }
                }
            }
            result.eax = mode + 1;
        }
    }
}

pass_port!(0x0071_10B0, fc4p_007110b0, "cSC4TrafficSimulator::WorkOnReduceEdgeDensities", reduce_edges);
unsafe extern "fastcall" fn reduce_edges(this: *mut u8, out: *mut Registers, budget: *mut u32, scope_pointer: *mut [u32; 6]) {
    unsafe {
        scope_pointer.write(scope(budget, edge_pacing(), edge_initialized()));
        let scope = &mut *scope_pointer;
        let sim = PassSim::from_ptr(this);
        let mut result = virtual_registers(sim.switches(), 0x60, &[], scope[3], word(sim.switches(), 0));
        let switches = result.eax as *mut u8;
        let mut work = 0u32;
        loop {
            let scan = PassScan::from_ptr(sim.cursor());
            if !advance(scan, &mut result) { break; }
            let (x, z) = (scan.x(), scan.z());
            result.edx = x as u32;
            result.eax = z as u32;
            result.ecx = x.wrapping_mul(12) as u32;
            let row = word(sim.network_rows(), x.wrapping_mul(12) as usize) as *mut u8;
            if word(row, z.wrapping_mul(4) as usize) == 0 {
                result.ecx = word(word(switches, 0) as *mut u8, x.wrapping_mul(12) as usize);
                if word(result.ecx as *mut u8, z.wrapping_mul(4) as usize) == 0 { continue; }
            }
            clear_edges(sim, x as u8, z as u8, &mut result);
            work = scope[2].wrapping_add(scope[3]);
            scope[2] = work;
            if work & 1023 == 0 {
                result = check_budget(scope, result);
                if result.eax as u8 != 0 {
                    out.write(paced_return(edge_pacing(), work, false, false, result));
                    return;
                }
            }
        }
        let header = sim.ferry_header();
        let mut ferry = word(header, 0) as *mut u8;
        let had_ferries = ferry != header;
        while ferry != header {
            clear_edges(sim, ferry.add(10).read(), ferry.add(11).read(), &mut result);
            ferry = word(ferry, 0) as *mut u8;
        }
        if had_ferries { work = scope[2]; }
        out.write(paced_return(edge_pacing(), work, true, false, result));
    }
}

pub const PORTS: &[Port] = &[fc4p_0070e4f0::PORT, fc4p_007110b0::PORT];

#[cfg(test)]
mod tests;
