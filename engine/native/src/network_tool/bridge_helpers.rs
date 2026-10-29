// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tool deletion, float ordering, terrain minima, and plain bridge-record copying.

use super::intersection_storage::call_registers;
use super::*;
use crate::abi::{Conv, RawCall};
use crate::x87::F80;

game_fn! { #[allow(dead_code)] fn destroy_tool(this: *mut u8) -> u64 = "thiscall" 0x0062_CAF0; }
game_fn! { #[allow(dead_code)] fn delete_tool(storage: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { #[allow(dead_code)] fn copy_floats_backward(first: *mut u32, last: *mut u32, output_end: *mut u32, value: *mut u32, flags: u32) -> u64 = "C" 0x0062_DC30; }

// Keep the right operand in memory: loading a signaling NaN into a second register would change
// which payload an operation selects. These comparisons pop exactly the one operand they load.
pub(super) fn compare_float(value: F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand,
            out("ax") status, out("st(0)") _,
        );
    }
    status
}

pub(super) fn less(status: u16) -> bool {
    status & 0x0500 == 0x0100
}

macro_rules! stored_float_op {
    ($name:ident, $instruction:literal, $ty:ty) => {
        pub(super) fn $name(value: F80, operand: $ty) -> F80 {
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
stored_float_op!(add_float, "fadd dword ptr [{operand}]", f32);

register_port!(0x0062_CD30, fc4p_0062cd30, "DeleteNetworkTool", scalar_delete, "ret 4");
unsafe extern "fastcall" fn scalar_delete(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0062_CAF0), this as u32, &[]);
        call.eax = *frame.add(1);
        call.edx = *frame;
        call_registers(&mut call);
        if *frame.add(3) as u8 & 1 != 0 {
            let args = [this as u32];
            call.target = crate::abi::target(0x005E_5620);
            call.args = args.as_ptr();
            call.count = 1;
            call.caller_pops = 1;
            call_registers(&mut call);
        }
        registers(this as u32, call.edx)
    }
}

port! {
    /// Return the first strict minimum in a float range. Equal or unordered comparisons keep the
    /// earlier element; an empty range returns its first pointer without loading a float.
    #[port(address = 0x0062_DB90, owner = "networktool", name = "FirstMinimumFloat")]
    pub unsafe extern "C" fn fc4p_0062db90(first: *mut f32, last: *mut f32) -> *mut f32 {
        unsafe {
            let mut best = first;
            if first != last {
                let mut cursor = first.wrapping_add(1);
                while cursor != last {
                    if less(compare_float(F80::from_f32(cursor.read_unaligned()), best.read_unaligned())) {
                        best = cursor;
                    }
                    cursor = cursor.wrapping_add(1);
                }
            }
            best
        }
    }
}

port! {
    /// Query four terrain corners, select minima in pairs (3,2) and (1,0), then compare the pairs.
    /// Equality and unordered comparisons select the second operand. Stack arguments are caller-popped.
    #[port(address = 0x0062_E630, owner = "networktool", name = "MinimumTerrainCorner", naked)]
    pub unsafe extern "C" fn fc4p_0062e630() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {body}", body = sym minimum_terrain);
    }
}
unsafe extern "fastcall" fn minimum_terrain(this: *mut u8, args: *mut u32) -> f32 {
    unsafe {
        let mut heights = core::mem::MaybeUninit::<[f32; 4]>::uninit();
        vcall!(this, 0x44 => fn(u32, u32, *mut f32), *args, *args.add(1), heights.as_mut_ptr().cast());
        let heights = heights.assume_init();
        let high = if less(compare_float(F80::from_f32(heights[3]), heights[2])) {
            3
        } else {
            2
        };
        let low = if less(compare_float(F80::from_f32(heights[1]), heights[0])) {
            1
        } else {
            0
        };
        let result = if less(compare_float(F80::from_f32(heights[high]), heights[low])) {
            high
        } else {
            low
        };
        heights[result]
    }
}

port! {
    /// Copy eight words per record in forward order and return the next output address. Null output
    /// skips that record's stores but still advances by 32 bytes. Aliases follow the word store order.
    #[port(address = 0x0062_E970, owner = "networktool", name = "CopyBridgeRecords32")]
    pub unsafe extern "C" fn fc4p_0062e970(first: *mut u8, last: *mut u8, output: *mut u8) -> *mut u8 {
        unsafe {
            let (mut first, mut output) = (first, output);
            while first != last {
                if !output.is_null() {
                    for offset in (0..32).step_by(4) {
                        let value: u32 = read(first, offset);
                        write(output, offset, value);
                    }
                }
                first = first.wrapping_add(32);
                output = output.wrapping_add(32);
            }
            output
        }
    }
}

register_port!(0x0062_EDF0, fc4p_0062edf0, "InsertOrderedFloat", insert_float, "ret");
unsafe extern "fastcall" fn insert_float(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let first = *frame.add(3) as *mut u32;
        let hole = *frame.add(4) as *mut u32;
        let value = frame.add(5);
        if less(compare_float(
            F80::from_f32(f32::from_bits(*value)),
            f32::from_bits(*first),
        )) {
            let args = [first as u32, hole as u32, hole.wrapping_add(1) as u32, value as u32, 0];
            let mut call = RawCall::new(
                Conv::Cdecl,
                crate::abi::target(0x0062_DC30),
                hole.wrapping_add(1) as u32,
                &args,
            );
            call.eax = hole as u32;
            call.edx = *frame;
            call_registers(&mut call);
            let bits = *value;
            first.write_unaligned(bits);
            registers(call.eax, bits)
        } else {
            let mut output = hole;
            let mut previous = hole.wrapping_sub(1);
            let mut eax = *frame.add(1);
            loop {
                let status = compare_float(
                    F80::from_f32(f32::from_bits(*value)),
                    f32::from_bits(previous.read_unaligned()),
                );
                eax = (eax & 0xFFFF_0000) | u32::from(status);
                if !less(status) {
                    break;
                }
                eax = previous.read_unaligned();
                output.write_unaligned(eax);
                output = previous;
                previous = previous.wrapping_sub(1);
            }
            output.write_unaligned(*value);
            registers(eax, output as u32)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0062cd30::PORT,
    fc4p_0062db90::PORT,
    fc4p_0062e630::PORT,
    fc4p_0062e970::PORT,
    fc4p_0062edf0::PORT,
];

#[cfg(test)]
pub(super) mod tests;
