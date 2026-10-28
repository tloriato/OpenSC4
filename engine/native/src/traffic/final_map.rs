// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Final pollution and congestion bytes and the map-change notification.

use super::*;
use super::finish_passes::{advance, byte, check_budget, pass_port, scope, PassScan};
use super::network::registers;
use crate::abi::{Conv, RawCall};

layout! {
    /// Final-map grids, scan and congestion scale.
    pub struct FinalMapSim {
        0x18 => messages / set_messages: *mut u8,
        0x30 => pollution / set_pollution: *mut u8,
        0x34 => volume / set_volume: *mut u8,
        0x48 => congestion / set_congestion: *mut u8,
        0x174 => ratio / set_ratio: *mut u8,
        0x818 => cursor / set_cursor: *mut u8,
        0xC58 => scale / set_scale: f32,
    }
}
game_fn! { fn curve_output(this: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { #[allow(dead_code)] fn finish_budget(this: *mut u32) -> u32 = "thiscall" 0x0068_B960; }
game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_data! { static pacing: [u32; 2] = 0x00B4_B30C; }
game_data! { static initialized: u32 = 0x00B4_B314; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }
game_data! { static air_scale: f32 = 0x00A8_C950; }
game_data! { static rounding_bias: f32 = 0x00AB_32D8; }
game_data! { static congestion_curve: u8 = 0x00B4_B2F0; }

unsafe fn cell<T>(vector: *mut u8, x: i32, z: i32) -> *mut T {
    unsafe {
        let rows = word(vector, 0) as *mut u8;
        let row = word(rows, x.wrapping_mul(12) as usize) as *mut T;
        row.wrapping_offset(z as isize)
    }
}

/// Quiet comparison: the volume's NaN follows the nonzero path.
fn equal_zero(value: F80, zero: f32) -> bool {
    let sw: u16;
    unsafe { core::arch::asm!("fld dword ptr [{zero}]", "fld tbyte ptr [{value}]", "fucompp", "fnstsw ax",
        zero = in(reg) &zero, value = in(reg) &value, out("ax") sw, out("st(0)") _, out("st(1)") _); }
    sw & 0x4400 == 0x4000
}

unsafe fn rounded_byte(value: F80, nonzero: bool) -> u8 {
    let value = add_float(value, unsafe { read_unaligned(rounding_bias()) }).to_f32().to_bits().wrapping_add(0xB4C0_0000);
    if value > 255 { 255 } else if nonzero && value == 0 { 1 } else { value as u8 }
}

pass_port!(0x0070_F460, fc4p_0070f460, "cSC4TrafficSimulator::WorkOnCreateFinalMap", final_map);
unsafe extern "fastcall" fn final_map(this: *mut u8, out: *mut Registers, budget: *mut u32, scope_pointer: *mut [u32; 6]) {
    unsafe {
        scope_pointer.write(scope(budget, pacing(), initialized()));
        let scope = &mut *scope_pointer;
        let sim = FinalMapSim::from_ptr(this);
        let air = vcall!(sim.pollution(), 0x60 => fn() -> *mut u8);
        let congestion = vcall!(sim.congestion(), 0x60 => fn() -> *mut u8);
        let volumes = vcall!(sim.volume(), 0x60 => fn() -> *mut u8);
        let ratios = vcall!(sim.ratio(), 0x60 => fn() -> *mut u8);
        let mut result = Registers::default();
        let completed = loop {
            scope[2] = scope[2].wrapping_add(scope[3]);
            if scope[2] & 1023 == 0 {
                result = check_budget(scope, result);
                if result.eax as u8 != 0 { scope[4] = byte(scope[4], 1); break false; }
            }
            let scan = PassScan::from_ptr(sim.cursor());
            if !advance(scan, &mut result) { break true; }
            let (x, z) = (scan.x(), scan.z());
            let volume = F80::from_f32(read_unaligned(cell::<f32>(volumes, x, z)));
            if equal_zero(volume, read_unaligned(zero())) {
                cell::<u8>(air, x, z).write(0);
                cell::<u8>(congestion, x, z).write(0);
            } else {
                cell::<u8>(air, x, z).write(rounded_byte(multiply_float(volume, read_unaligned(air_scale())), false));
                let ratio = read_unaligned(cell::<f32>(ratios, x, z));
                let curve = curve_output(congestion_curve(), ratio);
                let sw = curve.compare(F80::from_f32(read_unaligned(one()))).status_word();
                let congestion_byte = if sw & 0x4100 == 0 {
                    rounded_byte(multiply_float(subtract_float(curve, read_unaligned(one())), sim.scale()), true)
                } else { 1 };
                cell::<u8>(congestion, x, z).write(congestion_byte);
            }
        };
        if completed {
            let allocation = allocate_message(0x2C);
            let message = if allocation.is_null() { allocation } else { construct_message(allocation) };
            vcall!(message, 4 => fn());
            vcall!(message, 0x14 => fn(u32), 0x6924_7DC7);
            vcall!(message, 0x30 => fn(*mut u8), this);
            vcall!(message, 0x84 => fn(*mut u8), this);
            vcall!(sim.messages(), 0x10 => fn(*mut u8, u32), message, 0);
            result = virtual_registers(message, 8, &[], word(sim.messages(), 0), word(message, 0));
        }
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0068_B960), scope.as_mut_ptr() as u32, &[]);
        call.eax = result.eax;
        call.edx = result.edx;
        result = registers(call);
        result.eax = byte(result.eax, u8::from(completed));
        out.write(result);
    }
}

pub const PORTS: &[Port] = &[fc4p_0070f460::PORT];

#[cfg(test)]
mod tests;
