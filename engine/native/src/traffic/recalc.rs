// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cycle reset, funding damage and the supporting container operations.

use super::late_containers::call;
use super::*;
use crate::abi::Conv;

layout! {
    /// Fields used by the cycle reset and monthly damage updates.
    pub struct RecalcSim {
        0x134 => accidents_end / set_accidents_end: *mut u8,
        0x138 => accidents_limit / set_accidents_limit: *mut u8,
        0x318 => potholes / set_potholes: u16,
        0x790 => transit_sum / set_transit_sum: f64,
        0x798 => toll_sum / set_toll_sum: f64,
        0x7A0 => start_date / set_start_date: u32,
        0x7AC => road_cells / set_road_cells: u32,
        0x7B0 => road_damaged / set_road_damaged: u32,
        0x7B4 => road_rate / set_road_rate: f32,
        0x7E0 => rail_cells / set_rail_cells: u32,
        0x7E4 => rail_damaged / set_rail_damaged: u32,
        0x7E8 => rail_rate / set_rail_rate: f32,
        0x950 => reroute_bucket / set_reroute_bucket: u32,
    }
}

game_fn! { #[allow(dead_code)] fn grow_accidents(this: *mut u8, position: *mut u8, value: *const u32, scratch: *mut u32, count: u32, mode: u32) = "thiscall" 0x0062_6240; }
game_fn! { #[allow(dead_code)] fn insert_words(this: *mut u8, position: *mut u8, count: u32, value: *const u32) = "thiscall" 0x007D_A570; }
game_fn! { #[allow(dead_code)] fn construct_tree(this: *mut u8, source: *mut u8) = "thiscall" 0x0071_2280; }
game_fn! { #[allow(dead_code)] fn erase_tree(this: *mut u8, root: *mut u8) = "thiscall" 0x0042_6A60; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn recalculate_neighbors(this: *mut u8) = "thiscall" 0x0071_4E60; }
game_fn! { fn response(this: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { #[allow(dead_code)] fn apply_damage(this: *mut u8, intact: *mut u8, damaged: *mut u8, delta: i32, kind: u32) = "thiscall" 0x0071_5FB0; }
game_data! { static city: *mut u8 = 0x00B4_3D5C; }
game_data! { static double_zero: f64 = 0x00A8_0990; }
game_data! { static hundred: f32 = 0x00A8_0AA0; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }

// The adapter retains the original argument area and all three volatile result registers. All logic
// lives in Rust; the body receives the original return-address pointer as its sole stack argument.
macro_rules! entry {
    ($address:literal, $export:ident, $name:literal, $body:ident, $pop:literal) => {
        port! {
            #[port(address = $address, owner = "traffic", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push ebp", "mov ebp, esp", "sub esp, 12",
                    "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "lea edx, [esp]", "lea eax, [ebp + 4]", "push eax", "call {body}",
                    "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
                    "mov esp, ebp", "pop ebp", concat!("ret ", stringify!($pop)), body = sym $body,
                );
            }
        }
    };
}

entry!(
    0x0071_6950,
    fc4p_00716950,
    "cSC4TrafficSimulator::AddIntersectionToAccidentProneList",
    add_accident,
    12
);
unsafe extern "fastcall" fn add_accident(this: *mut u8, out: *mut Registers, stack: *mut u32) {
    // SAFETY: the simulator owns the vector and the entry supplies its three argument words.
    unsafe {
        let mut r = out.read();
        let sim = RecalcSim::from_ptr(this);
        let record = [stack.add(1).read(), stack.add(2).read(), stack.add(3).read()];
        r.ecx = this.wrapping_add(0x130) as u32;
        r.eax = sim.accidents_end() as u32;
        r.edx = record[0];
        if sim.accidents_end() != sim.accidents_limit() {
            let end = sim.accidents_end();
            if !end.is_null() {
                store(end, 0, r.edx);
                r.edx = record[2];
                store(end, 4, record[1]);
                store(end, 8, r.edx);
            }
            r.eax = (sim.accidents_end() as u32).wrapping_add(12);
            sim.set_accidents_end(r.eax as *mut u8);
        } else {
            r.edx = record.as_ptr() as u32;
            call(
                0x0062_6240,
                Conv::Thiscall,
                &[r.eax, r.edx, stack.add(1) as u32, 1, 1],
                &mut r,
            );
        }
        out.write(r);
    }
}

entry!(0x0071_69C0, fc4p_007169c0, "ResizeTrafficWordVector", resize_words, 8);
unsafe extern "fastcall" fn resize_words(this: *mut u8, out: *mut Registers, stack: *mut u32) {
    // SAFETY: the game supplies a three-word vector and the requested length/value pointer.
    unsafe {
        let mut r = out.read();
        r.eax = word(this, 4);
        let begin = word(this, 0);
        let desired = stack.add(1).read();
        r.edx = (r.eax.wrapping_sub(begin) as i32 >> 2) as u32;
        if desired < r.edx {
            r.edx = r.eax;
            r.eax = 0;
            store(this, 4, begin.wrapping_add(desired.wrapping_mul(4)));
        } else {
            call(
                0x007D_A570,
                Conv::Thiscall,
                &[r.eax, desired.wrapping_sub(r.edx), stack.add(2).read()],
                &mut r,
            );
        }
        out.write(r);
    }
}

entry!(
    0x0071_6A10,
    fc4p_00716a10,
    "ConstructTrafficSubnetworkRangeFromRange",
    construct_range,
    0
);
unsafe extern "fastcall" fn construct_range(_this: *mut u8, out: *mut Registers, stack: *mut u32) {
    // SAFETY: input and output are ranges of 28-byte records. A null destination skips construction.
    unsafe {
        let mut r = out.read();
        let mut source = stack.add(1).read() as *mut u8;
        let end = stack.add(2).read() as *mut u8;
        let mut destination = stack.add(3).read() as *mut u8;
        while source != end {
            if !destination.is_null() {
                r.ecx = destination as u32;
                call(0x0071_2280, Conv::Thiscall, &[source as u32], &mut r);
                r.ecx = source.wrapping_add(12) as u32;
                r.eax = destination.wrapping_add(12) as u32;
                for offset in [12, 16, 20] {
                    r.edx = word(source, offset);
                    store(destination, offset, r.edx);
                }
                r.ecx = word(source, 24);
                store(destination, 24, r.ecx);
            }
            source = source.wrapping_add(28);
            destination = destination.wrapping_add(28);
        }
        r.eax = destination as u32;
        out.write(r);
    }
}

entry!(
    0x0071_6A70,
    fc4p_00716a70,
    "DestroyTrafficSubnetworkRange",
    destroy_range,
    0
);
unsafe extern "fastcall" fn destroy_range(_this: *mut u8, out: *mut Registers, stack: *mut u32) {
    // SAFETY: each 28-byte record owns a tree header; deletion uses the game's own entries.
    unsafe {
        let mut r = out.read();
        r.eax = stack.add(2).read();
        let mut record = stack.add(1).read() as *mut u8;
        while record as u32 != r.eax {
            if word(record, 4) != 0 {
                r.eax = word(record, 0);
                let mut node = word(r.eax as *mut u8, 4) as *mut u8;
                while !node.is_null() {
                    r.ecx = record as u32;
                    call(0x0042_6A60, Conv::Thiscall, &[word(node, 12)], &mut r);
                    let next = word(node, 8) as *mut u8;
                    call(0x0090_CF63, Conv::Cdecl, &[node as u32], &mut r);
                    node = next;
                }
                r.eax = word(record, 0);
                store(r.eax as *mut u8, 8, r.eax);
                r.edx = word(record, 0);
                store(r.edx as *mut u8, 4, 0);
                r.eax = word(record, 0);
                store(r.eax as *mut u8, 12, r.eax);
                store(record, 4, 0);
            }
            r.eax = word(record, 0);
            if r.eax != 0 {
                call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
            }
            r.eax = stack.add(2).read();
            record = record.wrapping_add(28);
        }
        out.write(r);
    }
}

entry!(
    0x0071_7C90,
    fc4p_00717c90,
    "cSC4TrafficSimulator::WorkOnStartRecalc",
    start_recalc,
    4
);
unsafe extern "fastcall" fn start_recalc(this: *mut u8, out: *mut Registers, _stack: *mut u32) {
    // SAFETY: this is the simulator and the global points at its live city service.
    unsafe {
        let mut r = out.read();
        let sim = RecalcSim::from_ptr(this);
        let city = *city();
        r = virtual_registers(city, 0x20, &[], word(city, 0), r.edx);
        sim.set_transit_sum(F80::from_f64(*double_zero()).to_f64());
        sim.set_start_date(r.eax);
        sim.set_toll_sum(F80::from_f64(*double_zero()).to_f64());
        r.eax = 0;
        for offset in [0x1CC, 0x1D8, 0x1C0, 0x1D0, 0x1DC, 0x1C4, 0x1D4, 0x1E0, 0x1C8] {
            store(this, offset, 0);
        }
        sim.set_reroute_bucket(sim.reroute_bucket().wrapping_add(1) & 3);
        r.ecx = this as u32;
        for offset in [0x6F0, 0x6EC, 0x6F8, 0x6F4] {
            store(this, offset, 0);
        }
        call(0x0071_4E60, Conv::Thiscall, &[], &mut r);
        r.eax = (r.eax & !0xFF) | 1;
        out.write(r);
    }
}

/// Tests the ordered less-than flags with the second operand still in memory.
fn less_than_stored(value: F80, operand: f32) -> bool {
    let status: u16;
    // SAFETY: both operands are live and the one pushed register is popped by the comparison.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand, out("ax") status, out("st(0)") _);
    }
    status & 0x4500 == 0x0100
}

unsafe fn funding(this: *mut u8, out: *mut Registers, stack: *mut u32, rail: bool) {
    // SAFETY: the entry supplies a float pointer, the embedded response curve and damage collections.
    unsafe {
        let mut r = out.read();
        let (cells, damaged, rate) = if rail {
            (0x7E0, 0x7E4, 0x7E8)
        } else {
            (0x7AC, 0x7B0, 0x7B4)
        };
        let input = multiply_float(
            F80::from_f32((stack.add(1).read() as *const f32).read_unaligned()),
            *hundred(),
        )
        .to_f32();
        let change = response(this.add(0xD20), input);
        let sum = add_float(change, f32::from_bits(word(this, rate))).to_f32();
        stack.add(1).write(0);
        store(this, rate, sum.to_bits());
        let low = if less_than_stored(F80::from_f32(sum), *zero()) {
            0
        } else {
            word(this, rate)
        };
        store(this, rate, low);
        let high = less_than_stored(F80::from_f32(*one()), f32::from_bits(low));
        stack.add(1).write(0x3F80_0000);
        let selected = if high { 1.0 } else { f32::from_bits(word(this, rate)) };
        let selected = F80::from_f32(selected);
        store(this, rate, selected.to_f32().to_bits());
        let count = word(this, cells);
        let mut count_float = F80::from_i32(count as i32);
        if (count as i32) < 0 {
            count_float = add_float(count_float, *unsigned_bias());
        }
        let target = (count_float * selected).ftol();
        r.eax = target as u32;
        r.edx = (target as u64 >> 32) as u32;
        r.ecx = word(this, damaged);
        if r.eax != r.ecx {
            let delta = r.eax.wrapping_sub(r.ecx);
            if rail {
                store(this, damaged, r.ecx.wrapping_add(delta));
            }
            r.eax = delta;
            r.ecx = this as u32;
            r.edx = this.add(damaged + 8) as u32;
            call(
                0x0071_5FB0,
                Conv::Thiscall,
                &[r.edx, this.add(damaged + 28) as u32, delta, if rail { 2 } else { 1 }],
                &mut r,
            );
            if !rail {
                r.eax = word(this, damaged).wrapping_add(delta);
                store(this, damaged, r.eax);
                RecalcSim::from_ptr(this).set_potholes(r.eax as u16);
            }
        }
        out.write(r);
    }
}

entry!(
    0x0071_7D30,
    fc4p_00717d30,
    "cSC4TrafficSimulator::ApplyRoadFunding",
    road_funding,
    4
);
unsafe extern "fastcall" fn road_funding(this: *mut u8, out: *mut Registers, stack: *mut u32) {
    unsafe { funding(this, out, stack, false) };
}
entry!(
    0x0071_7E00,
    fc4p_00717e00,
    "cSC4TrafficSimulator::ApplyMassTransitFunding",
    rail_funding,
    4
);
unsafe extern "fastcall" fn rail_funding(this: *mut u8, out: *mut Registers, stack: *mut u32) {
    unsafe { funding(this, out, stack, true) };
}

pub const PORTS: &[Port] = &[
    fc4p_00716950::PORT,
    fc4p_007169c0::PORT,
    fc4p_00716a10::PORT,
    fc4p_00716a70::PORT,
    fc4p_00717c90::PORT,
    fc4p_00717d30::PORT,
    fc4p_00717e00::PORT,
];

#[cfg(test)]
mod tests;
