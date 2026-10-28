// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cell geometry and directional masks for lot placement records.

use super::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// A placement record's facing, floating bounds and three-word network payload.
    struct Placement {
        0x02 => facing / set_facing: u8,
        0x10 => x1 / set_x1: f32,
        0x14 => z1 / set_z1: f32,
        0x18 => x2 / set_x2: f32,
        0x1C => z2 / set_z2: f32,
        0x28 => payload / set_payload: *const u32,
    }
}

game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_fn! { fn rotate_rect(facing: i32, width: i32, height: i32, input: *const i32, output: *mut i32) = "C" 0x004B_DFD0; }

macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut result = [0u8;10];
            let value = value.to_bytes();
            // SAFETY: both operands and the destination cover their x87 storage formats.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{result}]",
                    value = in(reg) value.as_ptr(), operand = in(reg) &operand,
                    result = in(reg) result.as_mut_ptr(), out("st(0)") _);
            }
            F80::from_bytes(result)
        }
    };
}

stored_op!(add_stored, "fadd dword ptr [{operand}]");
stored_op!(multiply_stored, "fmul dword ptr [{operand}]");

port! {
    /// Convert the record centre to a cell, rotate it in the configuration's extent, add the lot origin,
    /// then write the type and directional masks. Rotate masks by the record's facing plus the lot facing.
    /// Outputs are written in argument order through type, first mask, second mask; masks rotate last.
    #[port(address = 0x0072_AD00, owner = "zonedeveloper", name = "PlacementNetworkCellAndMasks")]
    pub unsafe extern "C" fn fc4p_0072ad00(
        configuration: *mut u8, record: *mut u8, bounds: *const i32, facing: i32,
        out_x: *mut i32, out_z: *mut i32, out_mask1: *mut u32, out_mask2: *mut u32, out_type: *mut u32,
    ) -> u64 {
        unsafe {
            let mut width = MaybeUninit::<i32>::uninit();
            let mut height = MaybeUninit::<i32>::uninit();
            vcall!(configuration, 0x1C => fn(*mut i32, *mut i32), width.as_mut_ptr(), height.as_mut_ptr());
            let record = Placement::from_ptr(record);
            let z = multiply_stored(add_stored(F80::from_f32(record.z2()),record.z1()), *half()).to_f32();
            let x = multiply_stored(add_stored(F80::from_f32(record.x2()),record.x1()), *half()).to_f32();
            let mut cell_x = MaybeUninit::<i32>::uninit();
            let mut cell_z = MaybeUninit::<i32>::uninit();
            vcall!(*city(), 0x17C => fn(f32, f32, *mut i32, *mut i32), x, z, cell_x.as_mut_ptr(), cell_z.as_mut_ptr());
            let (x,z) = (cell_x.assume_init(),cell_z.assume_init());
            let rect = [x,z,x,z];
            let mut rotated = MaybeUninit::<[i32;4]>::uninit();
            rotate_rect(facing,width.assume_init(),height.assume_init(),rect.as_ptr(),rotated.as_mut_ptr().cast());
            let rotated = rotated.assume_init();
            let origin_x = read_unaligned(bounds);
            let origin_z = read_unaligned(bounds.add(1));
            write_unaligned(out_x,rotated[0].wrapping_add(origin_x));
            write_unaligned(out_z,rotated[1].wrapping_add(origin_z));
            write_unaligned(out_type,read_unaligned(record.payload()));
            write_unaligned(out_mask1,read_unaligned(record.payload().add(1)));
            write_unaligned(out_mask2,read_unaligned(record.payload().add(2)));
            let orientation = i32::from(record.facing()).wrapping_add(facing) % 4;
            let shift = orientation.wrapping_mul(8) as u32;
            let first = read_unaligned(out_mask1);
            write_unaligned(out_mask1,first.wrapping_shr(32u32.wrapping_sub(shift)) | first.wrapping_shl(shift));
            let second = read_unaligned(out_mask2);
            let eax = second.wrapping_shl(shift);
            write_unaligned(out_mask2,second.wrapping_shr(32u32.wrapping_sub(shift)) | eax);
            (u64::from(shift) << 32) | u64::from(eax)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0072ad00::PORT];

#[cfg(test)]
mod tests;
