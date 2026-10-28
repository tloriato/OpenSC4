// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Rail damage checks, lot subnetwork properties and traffic container copies.

use super::*;
use super::network::{register_port, registers};
use crate::abi::{call_raw, Conv, RawCall};

layout! {
    /// Rail damage hash buckets and the accident threshold.
    pub struct RailDamageSim {
        0x804 => buckets / set_buckets: *mut u8,
        0x808 => buckets_end / set_buckets_end: *mut u8,
        0xD04 => accident_factor / set_accident_factor: f32,
    }
}

game_fn! { #[allow(dead_code)] fn random_uniform(this: *mut u8) -> St0 = "thiscall" 0x0091_3A29; }
game_fn! { #[allow(dead_code)] fn subnetwork_property(holder: *mut u8, id: u32, output: *mut u8) -> u64 = "C" 0x005F_F510; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn clone_tree(this: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 = "thiscall" 0x0071_0B80; }
game_fn! { #[allow(dead_code)] fn copy_words(destination: *mut u8, source: *mut u8, bytes: u32) -> *mut u8 = "C" 0x009E_EB70; }
game_fn! { #[allow(dead_code)] fn assign_vector(this: *mut u8, source: *mut u8) -> *mut u8 = "thiscall" 0x0072_BD50; }

/// Unsigned division retains the processor's divide exception for an empty bucket array.
unsafe fn remainder(value: u32, divisor: u32) -> u32 {
    let rem: u32;
    unsafe { core::arch::asm!("xor edx, edx", "div {divisor:e}", divisor = in(reg) divisor,
        inout("eax") value => _, out("edx") rem, options(nomem, nostack)); }
    rem
}

port! {
    /// Undamaged cells leave the generator alone. Each matching chain consumes one uniform draw;
    /// the threshold must be strictly greater. The second coordinate's whole stack word is ORed in.
    #[port(address = 0x0071_1590, owner = "traffic", name = "cSC4TrafficSimulator::CheckRailAccident")]
    pub unsafe extern "thiscall" fn fc4p_00711590(this: *mut u8, x: u32, z: u32) -> u64 {
        unsafe {
            let sim = RailDamageSim::from_ptr(this);
            let key = ((x & 255) << 8) | z;
            let count = ((sim.buckets_end() as u32).wrapping_sub(sim.buckets() as u32) as i32 >> 2) as u32;
            let bucket = remainder(key & 0xFFFF, count);
            let mut node = word(sim.buckets(), bucket.wrapping_mul(4) as usize) as *mut u8;
            let mut matches = 0u32;
            while !node.is_null() {
                if read_unaligned(node.add(4).cast::<u16>()) == key as u16 { matches = matches.wrapping_add(1); }
                node = word(node, 0) as *mut u8;
            }
            if matches == 0 { return u64::from(sim.buckets() as u32 & !255); }
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0091_3A29),
                TrafficSim::from_ptr(this).random() as u32, &[]);
            call.eax = sim.buckets() as u32;
            call.take_st0 = 1;
            call_raw(&mut call);
            let sw = F80::from_f32(sim.accident_factor()).compare(call.st0).status_word();
            (u64::from(call.edx) << 32) | u64::from(sw & 0x4100 == 0)
        }
    }
}

port! {
    /// Clears the output's logical length before fetching the lot holder and its subnetwork property.
    /// Fastcall receives the incoming EDX without changing the two stack arguments or their callee pop.
    #[port(address = 0x0071_1B50, owner = "traffic", name = "cSC4TrafficSimulator::GetSubnetworksForLot")]
    pub unsafe extern "fastcall" fn fc4p_00711b50(_this: *mut u8, entry_edx: u32, lot: *mut u8, vector: *mut u8) -> u64 {
        unsafe {
            let begin = word(vector, 0);
            store(vector, 4, begin);
            let holder = virtual_registers(lot, 0x0C, &[], word(lot, 0), entry_edx);
            let args = [holder.eax, 0x2A4F_9C74, vector as u32];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x005F_F510), holder.ecx, &args);
            call.eax = holder.eax;
            call.edx = holder.edx;
            let result = registers(call);
            (u64::from(result.edx) << 32) | u64::from(result.eax)
        }
    }
}

port! {
    /// Allocates a 24-byte sentinel, clones a nonempty tree and links its extreme nodes.
    /// Sentinel padding and value bytes are retained from the allocator.
    #[port(address = 0x0071_2280, owner = "traffic", name = "CopyConstructTrafficPairTree")]
    pub unsafe extern "thiscall" fn fc4p_00712280(this: *mut u8, source: *mut u8) -> *mut u8 {
        unsafe {
            store(this, 0, 0);
            let header = allocate(24);
            store(this, 0, header as u32);
            store(this, 4, 0);
            let source_header = word(source, 0) as *mut u8;
            let root = word(source_header, 4) as *mut u8;
            header.write(0);
            if root.is_null() {
                store(header, 4, 0);
                let header = word(this, 0) as *mut u8;
                store(header, 8, header as u32);
                let header = word(this, 0) as *mut u8;
                store(header, 12, header as u32);
            } else {
                let source_header = word(source, 0) as *mut u8;
                let root = word(source_header, 4) as *mut u8;
                let header = word(this, 0) as *mut u8;
                let cloned = clone_tree(this, root, header);
                store(header, 4, cloned as u32);
                let header = word(this, 0) as *mut u8;
                let mut node = word(header, 4) as *mut u8;
                while word(node, 8) != 0 { node = word(node, 8) as *mut u8; }
                store(header, 8, node as u32);
                let header = word(this, 0) as *mut u8;
                let mut node = word(header, 4) as *mut u8;
                while word(node, 12) != 0 { node = word(node, 12) as *mut u8; }
                store(header, 12, node as u32);
            }
            store(this, 4, word(source, 4));
            this
        }
    }
}

register_port!(0x0071_2330, fc4p_00712330, "CopyConstructTrafficVectorRange", construct_vectors, 0, [16, 12, 8]);
unsafe extern "fastcall" fn construct_vectors(incoming_ecx: *mut u8, out: *mut Registers, mut first: *mut u8, last: *mut u8, mut destination: *mut u8) {
    unsafe {
        let mut result = Registers { eax: last as u32, ecx: incoming_ecx as u32, edx: 0 };
        while first != last {
            if !destination.is_null() {
                let count = (word(first, 4).wrapping_sub(word(first, 0)) as i32 >> 2) as u32;
                store(destination, 0, 0);
                store(destination, 4, 0);
                store(destination, 8, 0);
                let allocation = if count == 0 { core::ptr::null_mut() } else { allocate(count.wrapping_mul(4)) };
                store(destination, 0, allocation as u32);
                store(destination, 4, allocation as u32);
                store(destination, 8, (allocation as u32).wrapping_add(count.wrapping_mul(4)));
                let end = word(first, 4);
                let begin = word(first, 0);
                result.eax = allocation as u32;
                result.ecx = begin;
                if end != begin {
                    let bytes = end.wrapping_sub(begin);
                    let args = [allocation as u32, begin, bytes];
                    let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x009E_EB70), begin, &args);
                    call.eax = allocation as u32;
                    result = registers(call);
                    result.eax = result.eax.wrapping_add(bytes);
                }
                store(destination, 4, result.eax);
            }
            first = first.wrapping_add(12);
            destination = destination.wrapping_add(12);
        }
        result.eax = destination as u32;
        out.write(result);
    }
}

register_port!(0x0071_23B0, fc4p_007123b0, "AssignTrafficVectorRange", assign_vectors, 0, [16, 12, 8]);
unsafe extern "fastcall" fn assign_vectors(_this: *mut u8, out: *mut Registers, mut first: *mut u8, last: *mut u8, mut destination: *mut u8) {
    unsafe {
        let difference = (last as u32).wrapping_sub(first as u32);
        let count = difference as i32 / 12;
        let mut result = Registers { eax: count as u32, ecx: difference, edx: 0 };
        for _ in 0..count {
            let args = [first as u32];
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0072_BD50), destination as u32, &args);
            call.eax = result.eax;
            call.edx = result.edx;
            result = registers(call);
            first = first.wrapping_add(12);
            destination = destination.wrapping_add(12);
        }
        result.eax = destination as u32;
        out.write(result);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00711590::PORT, fc4p_00711b50::PORT, fc4p_00712280::PORT,
    fc4p_00712330::PORT, fc4p_007123b0::PORT,
];

#[cfg(test)]
mod tests;
