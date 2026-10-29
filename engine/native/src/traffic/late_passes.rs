// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Destination capacity, whole-table maintenance and per-subnetwork occupant totals.

use super::late_containers::{call, record_count, register_entry};
use super::*;
use crate::abi::{Conv, RawCall};

layout! {
    /// Fields used by the complete-table maintenance passes.
    pub struct PassSim {
        0x70 => subnetworks / set_subnetworks: *mut u8,
        0x74 => subnetworks_end / set_subnetworks_end: *mut u8,
        0x97C => density_reduction / set_density_reduction: f32,
    }
}
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }
game_fn! { #[allow(dead_code)] fn pool(this: *mut u8, travel: u32, x: i32, z: i32) -> *mut u8 = "thiscall" 0x0071_14E0; }
game_fn! { #[allow(dead_code)] fn available(this: *mut u8, holder: *mut u8, kind: u32) -> u32 = "thiscall" 0x0070_DFF0; }
game_fn! { #[allow(dead_code)] fn begin(this: *mut u8, pair: *mut u32) = "thiscall" 0x004B_0390; }
game_fn! { #[allow(dead_code)] fn resize(this: *mut u8, count: i32, initial: *const u32) = "thiscall" 0x006D_6920; }

pub(super) unsafe fn virtual_call(object: *mut u8, slot: usize, args: &[u32], r: &mut Registers) {
    let mut c = RawCall::new(
        Conv::Thiscall,
        unsafe { crate::abi::vslot(object, slot) },
        object as u32,
        args,
    );
    c.eax = r.eax;
    c.edx = r.edx;
    unsafe {
        *r = super::network::registers(c);
    }
}

register_entry!(
    0x0071_36B0,
    fc4p_007136b0,
    "cSC4TrafficSimulator::CapacityStillAvailable",
    capacity,
    4,
    [8]
);
unsafe extern "fastcall" fn capacity(this: *mut u8, out: *mut Registers, trip: *mut u8) {
    unsafe {
        let mut r = out.read();
        let sim = TrafficSim::from_ptr(this);
        let x = i32::from(read_unaligned(trip.add(32).cast::<i16>()));
        let z = i32::from(read_unaligned(trip.add(34).cast::<i16>()));
        r.eax = x as u32;
        r.edx = z as u32;
        r.ecx = sim.min_cell_x() as u32;
        let mut holder = 0;
        if x >= sim.min_cell_x() && z >= sim.min_cell_z() && x <= sim.max_cell_x() && z <= sim.max_cell_z() {
            let manager = read_unaligned(lot_manager());
            virtual_call(manager, 0x18, &[x as u32, z as u32, 0], &mut r);
            if r.eax != 0 {
                let lot = r.eax as *mut u8;
                r.edx = word(lot, 0);
                virtual_call(lot, 0x0C, &[], &mut r);
                holder = r.eax;
            }
        } else {
            r.ecx = this as u32;
            call(0x0071_14E0, Conv::Thiscall, &[u32::MAX, x as u32, z as u32], &mut r);
            holder = r.eax;
        }
        let mut remaining = 0u32;
        if holder != 0 {
            r.ecx = u32::from(trip.add(28).read());
            r.eax = word(this, 0);
            let kind = r.ecx;
            virtual_call(this, 0x50, &[holder, kind], &mut r);
            remaining = r.eax;
            r.edx = u32::from(trip.add(28).read());
            r.ecx = this as u32;
            call(0x0070_DFF0, Conv::Thiscall, &[holder, r.edx], &mut r);
            remaining = remaining.wrapping_sub(r.eax);
        }
        r.edx = word(trip, 24);
        r.eax = u32::from(remaining >= r.edx);
        out.write(r);
    }
}

/// DIV deliberately retains the processor's exception for an empty bucket array.
pub(super) unsafe fn remainder(value: u32, divisor: u32) -> u32 {
    let result: u32;
    unsafe {
        core::arch::asm!("xor edx, edx", "div {divisor:e}", divisor = in(reg) divisor,
        inout("eax") value => _, out("edx") result, options(nomem, nostack));
    }
    result
}

unsafe fn next_hash(node: *mut u8, table: *mut u8) -> (*mut u8, u32) {
    unsafe {
        let begin = word(table, 4);
        let count = (word(table, 8).wrapping_sub(begin) as i32 >> 2) as u32;
        let mut index = remainder(word(node, 4), count);
        let count = (word(table, 8).wrapping_sub(word(table, 4)) as i32 >> 2) as u32;
        loop {
            index = index.wrapping_add(1);
            if index >= count {
                return (core::ptr::null_mut(), index);
            }
            let node = word(word(table, 4) as *mut u8, index.wrapping_mul(4) as usize) as *mut u8;
            if !node.is_null() {
                return (node, index);
            }
        }
    }
}

register_entry!(
    0x0071_3750,
    fc4p_00713750,
    "cSC4TrafficSimulator::WorkOnReduceTransitSwitches",
    reduce,
    4,
    [8]
);
unsafe extern "fastcall" fn reduce(this: *mut u8, out: *mut Registers, _budget: u32) {
    unsafe {
        let mut r = out.read();
        let mut pair = core::mem::MaybeUninit::<[u32; 2]>::uninit();
        r.eax = pair.as_mut_ptr() as u32;
        r.ecx = this.wrapping_add(0x70C) as u32;
        call(0x004B_0390, Conv::Thiscall, &[r.eax], &mut r);
        let [first, table] = pair.assume_init();
        let mut node = first as *mut u8;
        r.ecx = table;
        while !node.is_null() {
            let factor = PassSim::from_ptr(this).density_reduction();
            let load = read_unaligned(node.add(12).cast::<f32>());
            write_unaligned(
                node.add(12).cast::<f32>(),
                multiply_float(F80::from_f32(factor), load).to_f32(),
            );
            r.edx = word(node, 0);
            node = if r.edx != 0 {
                r.edx as *mut u8
            } else {
                let (next, index) = next_hash(node, table as *mut u8);
                r.edx = index;
                next
            };
        }
        r.eax = 1;
        out.write(r);
    }
}

register_entry!(
    0x0071_37D0,
    fc4p_007137d0,
    "cSC4TrafficSimulator::WorkOnDumpEdgeDensities",
    dump,
    4,
    [8]
);
unsafe extern "fastcall" fn dump(this: *mut u8, out: *mut Registers, _budget: u32) {
    unsafe {
        let mut r = out.read();
        let mut pair = core::mem::MaybeUninit::<[u32; 2]>::uninit();
        r.eax = pair.as_mut_ptr() as u32;
        r.ecx = this.wrapping_add(0x178) as u32;
        call(0x004B_0390, Conv::Thiscall, &[r.eax], &mut r);
        let [first, table] = pair.assume_init();
        let mut node = first as *mut u8;
        while !node.is_null() {
            let mut sum = 0u32;
            for group in 0..4 {
                let src = node.add(42 + group * 8);
                let a = read_unaligned(src.cast::<u16>());
                let b = read_unaligned(src.add(2).cast::<u16>());
                let c = read_unaligned(src.add(4).cast::<u16>());
                let d = read_unaligned(src.add(6).cast::<u16>());
                let dest = node.add(8 + group * 8);
                write_unaligned(dest.add(2).cast::<u16>(), b);
                write_unaligned(dest.cast::<u16>(), a);
                write_unaligned(dest.add(4).cast::<u16>(), c);
                write_unaligned(dest.add(6).cast::<u16>(), d);
                r.edx = u32::from(d) + sum;
                sum = r.edx + u32::from(a) + u32::from(b) + u32::from(c);
            }
            write_unaligned(node.add(40).cast::<u16>(), sum.min(0xFFFF) as u16);
            r.ecx = word(node, 0);
            node = if r.ecx != 0 {
                r.ecx as *mut u8
            } else {
                let (next, index) = next_hash(node, table as *mut u8);
                r.edx = index;
                r.ecx = next as u32;
                next
            };
        }
        r.eax = 1;
        out.write(r);
    }
}

register_entry!(
    0x0071_43B0,
    fc4p_007143b0,
    "cSC4TrafficSimulator::GetOccupantCountForAllSubnetworks",
    occupant_counts,
    8,
    [12, 8]
);
unsafe extern "fastcall" fn occupant_counts(this: *mut u8, out: *mut Registers, kind: u32, vector: *mut u8) {
    unsafe {
        let mut r = out.read();
        let sim = PassSim::from_ptr(this);
        let (count, high) = record_count((sim.subnetworks_end() as u32).wrapping_sub(sim.subnetworks() as u32) as i32);
        let zero = 0u32;
        r.eax = &zero as *const u32 as u32;
        r.ecx = vector as u32;
        r.edx = high;
        call(
            0x006D_6920,
            Conv::Thiscall,
            &[count, &zero as *const u32 as u32],
            &mut r,
        );
        let mut index = 1i32;
        while index < count as i32 {
            r.ecx = word(sim.subnetworks().wrapping_offset(index.wrapping_mul(28) as isize), 0);
            let header = r.ecx as *mut u8;
            r.eax = word(header, 4);
            r.edx = header as u32;
            while r.eax != 0 {
                let node = r.eax as *mut u8;
                if word(node, 16) >= kind {
                    r.edx = r.eax;
                    r.eax = word(node, 8);
                } else {
                    r.eax = word(node, 12);
                }
            }
            if r.edx != header as u32 && kind < word(r.edx as *mut u8, 16) {
                r.edx = header as u32;
            }
            if r.edx != header as u32 {
                r.eax = word(vector, 0);
                r.ecx = word(r.edx as *mut u8, 20);
                store(r.eax as *mut u8, index.wrapping_mul(4) as usize, r.ecx);
            }
            index = index.wrapping_add(1);
        }
        r.ecx = this as u32;
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_007136b0::PORT,
    fc4p_00713750::PORT,
    fc4p_007137d0::PORT,
    fc4p_007143b0::PORT,
];

#[cfg(test)]
mod tests;
