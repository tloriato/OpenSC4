// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Per-corner suspension weights between two bridge supports.

use super::bridge_helpers::{add_float, compare_float, less};
use super::*;
use crate::x87::F80;

game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static two: f64 = 0x00A8_0AC8; }
game_data! { static profile_scale: f64 = 0x00AA_8930; }
game_data! { static corner_offsets: [f32; 8] = 0x00AA_8938; }

macro_rules! stored_op {
    ($name:ident, $instruction:literal, $ty:ty) => {
        fn $name(value: F80, operand: $ty) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand,
                    output = in(reg) &mut output, out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_op!(sub_float, "fsub dword ptr [{operand}]", f32);
stored_op!(mul_float, "fmul dword ptr [{operand}]", f32);
stored_op!(div_float, "fdiv dword ptr [{operand}]", f32);
stored_op!(sub_double, "fsub qword ptr [{operand}]", f64);
stored_op!(mul_double, "fmul qword ptr [{operand}]", f64);

unsafe fn unsigned_integer(value: u32) -> F80 {
    let value80 = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        value80
    } else {
        add_float(value80, unsafe { *unsigned_bias() })
    }
}

fn exponential(value: F80) -> F80 {
    let mut log2_e = F80::ZERO;
    unsafe {
        core::arch::asm!("fldl2e", "fstp tbyte ptr [{out}]", out = in(reg) &mut log2_e, out("st(0)") _);
    }
    let power = value * log2_e;
    let integral = power.round_int();
    let fractional = (power - integral).exp2_m1();
    (fractional + F80::from_i32(1)).scale(integral)
}

register_port!(
    0x0062_E690,
    fc4p_0062e690,
    "SetSuspensionCornerWeights",
    suspension_weights,
    "ret"
);
unsafe extern "fastcall" fn suspension_weights(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let first = *frame.add(3) as *mut u8;
        let stride = *frame.add(5);
        let next = first.wrapping_add(stride.wrapping_mul(0x50) as usize);
        let start_x: u32 = read(first, 4);
        let next_x: u32 = read(next, 4);
        let next_z: u32 = read(next, 8);
        let delta_x = next_x.wrapping_sub(start_x);
        *frame.add(5) = delta_x;
        let dx = F80::from_i32(delta_x as i32).to_f32();
        let start_z: u32 = read(first, 8);
        let delta_z = next_z.wrapping_sub(start_z);
        *frame.add(5) = delta_z;
        *frame.add(5) = F80::from_i32(delta_z as i32).to_f32().to_bits();
        let dz = f32::from_bits(*frame.add(5));
        let mut x0 = unsigned_integer(start_x).to_f32();
        let mut z0 = unsigned_integer(start_z).to_f32();
        let count = *frame.add(4);
        let last = first.wrapping_add(count.wrapping_sub(1).wrapping_mul(stride).wrapping_mul(0x50) as usize);
        let mut x1 = unsigned_integer(read(last, 4)).to_f32();
        let end_z: u32 = read(last, 8);
        *frame.add(4) = end_z;
        *frame.add(4) = unsigned_integer(end_z).to_f32().to_bits();
        let mut z1 = f32::from_bits(*frame.add(4));
        if compare_float(F80::from_f32(dx), *zero()) & 0x4100 == 0 {
            x1 = add_float(F80::from_f32(x1), *one()).to_f32();
        } else if less(compare_float(F80::from_f32(dx), *zero())) {
            x0 = add_float(F80::from_f32(x0), *one()).to_f32();
        }
        if compare_float(F80::from_f32(dz), *zero()) & 0x4100 == 0 {
            z1 = add_float(F80::from_f32(z1), *one()).to_f32();
            *frame.add(4) = z1.to_bits();
        } else if less(compare_float(F80::from_f32(dz), *zero())) {
            z0 = add_float(F80::from_f32(z0), *one()).to_f32();
        }
        let dot = |x: f32, z: f32| {
            let z = mul_float(F80::from_f32(z), dz);
            let x = mul_float(F80::from_f32(x), dx);
            z + x
        };
        let (base, upper) = match *frame.add(7) {
            0 => {
                let base = dot(x0, z0);
                (base, sub_float(dot(x1, z1), *half()))
            }
            1 => {
                let z = mul_float(add_float(F80::from_f32(z1), z0), dz);
                let x = mul_float(add_float(F80::from_f32(x1), x0), dx);
                let base = mul_float(z + x, *half());
                (base, sub_float(dot(x1, z1), *half()))
            }
            2 => {
                let base = dot(x1, z1);
                (base, add_float(dot(x0, z0), *half()))
            }
            _ => (F80::from_f32(dz), F80::from_f32(dz)),
        };
        let mut remaining = count.wrapping_mul(stride) as i32;
        if remaining <= 0 {
            return registers(0, *frame);
        }
        let denominator = (upper - base).to_f32();
        *frame.add(7) = denominator.to_bits();
        let complement = sub_float(F80::from_f32(*one()), f32::from_bits(*frame.add(6))).to_f32();
        let mut record = first;
        loop {
            let x = unsigned_integer(read(record, 4));
            let z_word: u32 = read(record, 8);
            *frame.add(4) = unsigned_integer(z_word).to_f32().to_bits();
            for corner in 0..4usize {
                let offsets = corner_offsets().cast::<f32>().add(corner * 2);
                let z = mul_float(
                    add_float(F80::from_f32(f32::from_bits(*frame.add(4))), *offsets.add(1)),
                    dz,
                );
                let x = mul_float(add_float(x, *offsets), dx);
                let position = div_float((z + x) - base, f32::from_bits(*frame.add(7)));
                let negative = exponential(-position);
                let positive = exponential(position);
                let sum = negative + positive;
                let profile = mul_double(sub_double(sum, *two()), *profile_scale());
                let weight = add_float(mul_float(profile, complement), f32::from_bits(*frame.add(6)));
                let bits = weight.to_f32().to_bits();
                let output = record.add(0x2C + corner * 4);
                let old: u32 = read(output, 0);
                let selected = if less(compare_float(weight, f32::from_bits(old))) {
                    bits
                } else {
                    old
                };
                write(output, 0, selected);
            }
            record = record.wrapping_add(0x50);
            remaining = remaining.wrapping_sub(1);
            if remaining == 0 {
                break;
            }
        }
        registers(read(record.wrapping_sub(0x50), 0x38), 0x00AA_8958)
    }
}

pub const PORTS: &[Port] = &[fc4p_0062e690::PORT];

#[cfg(test)]
mod tests;
