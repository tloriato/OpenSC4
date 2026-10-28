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
game_data! { static cell_bias: f32 = 0x00A8_E178; }
game_data! { static navigation: *mut u8 = 0x00B4_3CF0; }
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
    /// Pass the original stack to the Rust body, preserving the first cdecl argument's cell output and
    /// the scratch rectangle used when the rotation helper leaves its destination unwritten.
    #[port(address = 0x0072_AD00, owner = "zonedeveloper", name = "PlacementNetworkCellAndMasks", naked)]
    pub unsafe extern "C" fn fc4p_0072ad00() {
        core::arch::naked_asm!("sub esp, 44", "lea ecx, [esp + 44]", "call {body}",
            "add esp, 44", "ret", body = sym network_cell_and_masks);
    }
}

/// Convert the centre, rotate and translate it, then copy and rotate the directional masks. The original
/// first argument is also the cell converter's X destination. All other argument pointers are read at use.
unsafe extern "thiscall" fn network_cell_and_masks(stack: *mut u32) -> u64 {
    unsafe {
        let configuration = read_unaligned(stack.add(1)) as *mut u8;
        let width = stack.sub(9).cast::<i32>();
        let height = stack.sub(10).cast::<i32>();
        vcall!(configuration,0x1C => fn(*mut i32,*mut i32),width,height);
        let record = Placement::from_ptr(read_unaligned(stack.add(2)) as *mut u8);
        let z = multiply_stored(add_stored(F80::from_f32(record.z2()), record.z1()), *half()).to_f32();
        let x = multiply_stored(add_stored(F80::from_f32(record.x2()), record.x1()), *half()).to_f32();
        let cell_x = stack.add(1).cast::<i32>();
        let cell_z = stack.sub(11).cast::<i32>();
        vcall!(*city(),0x17C => fn(f32,f32,*mut i32,*mut i32),x,z,cell_x,cell_z);
        let rect = stack.sub(8).cast::<i32>();
        let x = read_unaligned(cell_x);
        let z = read_unaligned(cell_z);
        write_unaligned(rect.cast::<[i32; 4]>(), [x, z, x, z]);
        let facing = read_unaligned(stack.add(4)) as i32;
        let rotated = stack.sub(4).cast::<i32>();
        rotate_rect(facing, read_unaligned(width), read_unaligned(height), rect, rotated);
        let bounds = read_unaligned(stack.add(3)) as *const i32;
        let origin_x = read_unaligned(bounds);
        let origin_z = read_unaligned(bounds.add(1));
        let out_x = read_unaligned(stack.add(5)) as *mut i32;
        let value_x = read_unaligned(rotated).wrapping_add(origin_x);
        let value_z = read_unaligned(rotated.add(1)).wrapping_add(origin_z);
        let out_mask2 = read_unaligned(stack.add(8)) as *mut u32;
        write_unaligned(out_x, value_x);
        let out_z = read_unaligned(stack.add(6)) as *mut i32;
        let out_mask1 = read_unaligned(stack.add(7)) as *mut u32;
        write_unaligned(out_z, value_z);
        let out_type = read_unaligned(stack.add(9)) as *mut u32;
        write_unaligned(out_type, read_unaligned(record.payload()));
        write_unaligned(out_mask1, read_unaligned(record.payload().add(1)));
        write_unaligned(out_mask2, read_unaligned(record.payload().add(2)));
        let orientation = i32::from(record.facing()).wrapping_add(facing) % 4;
        let shift = orientation.wrapping_mul(8) as u32;
        let first = read_unaligned(out_mask1);
        write_unaligned(
            out_mask1,
            first.wrapping_shr(32u32.wrapping_sub(shift)) | first.wrapping_shl(shift),
        );
        let second = read_unaligned(out_mask2);
        let eax = second.wrapping_shl(shift);
        write_unaligned(out_mask2, second.wrapping_shr(32u32.wrapping_sub(shift)) | eax);
        (u64::from(shift) << 32) | u64::from(eax)
    }
}

/// Convert both corners in their stored order, decrement the second cell pair, then rotate and translate.
unsafe fn constraint_rect(
    dev: super::ZoneDeveloper,
    record: Placement,
    extent: (i32, i32),
    origin: (i32, i32),
    facing: i32,
    destination: *mut i32,
) -> [i32; 4] {
    unsafe {
        let high_x = F80::from_f32(record.x1()).to_f32();
        let high_z = F80::from_f32(record.z1()).to_f32();
        let low_x = F80::from_f32(record.x2());
        let low_z = F80::from_f32(record.z2());
        let z = add_stored(low_z, *cell_bias()).to_f32();
        let x = add_stored(low_x, *cell_bias()).to_f32();
        let mut rect = MaybeUninit::<[i32; 4]>::uninit();
        let p = rect.as_mut_ptr().cast::<i32>();
        vcall!(dev.city(),0x17C => fn(f32,f32,*mut i32,*mut i32),x,z,p,p.add(1));
        let z = add_stored(F80::from_f32(high_z), *cell_bias()).to_f32();
        let x = add_stored(F80::from_f32(high_x), *cell_bias()).to_f32();
        vcall!(dev.city(),0x17C => fn(f32,f32,*mut i32,*mut i32),x,z,p.add(2),p.add(3));
        let mut rect = rect.assume_init();
        rect[2] = rect[2].wrapping_sub(1);
        rect[3] = rect[3].wrapping_sub(1);
        rotate_rect(facing, extent.0, extent.1, rect.as_ptr(), destination);
        let [x1, z1, x2, z2] = read_unaligned(destination.cast::<[i32; 4]>());
        let translated = [
            x1.wrapping_add(origin.0),
            z1.wrapping_add(origin.1),
            x2.wrapping_add(origin.0),
            z2.wrapping_add(origin.1),
        ];
        write_unaligned(destination.cast::<[i32; 4]>(), translated);
        translated
    }
}

/// Stop at a failed cell within each row, while continuing every subsequent row.
fn failed_rows(rect: [i32; 4], mut passes: impl FnMut(i32, i32) -> bool) -> bool {
    let [x0, z0, x1, z1] = rect;
    let mut failed = false;
    let mut z = z0;
    while z <= z1 {
        let mut x = x0;
        while x <= x1 {
            if !passes(x, z) {
                failed = true;
                break;
            }
            x = x.wrapping_add(1);
        }
        z = z.wrapping_add(1);
    }
    failed
}

layout! {
    /// A vector of 52-byte placement records.
    struct Placements {
        0x00 => first / set_first: *mut u8,
        0x04 => end / set_end: *mut u8,
    }
}

fn placement_count(records: Placements) -> u32 {
    ((records.end() as u32).wrapping_sub(records.first() as u32) as i32 / 52) as u32
}

port! {
    /// Check every water and land constraint record. Each failing row stops at its first failed cell.
    /// Water subtype one also requires two outward rows of navigable cells. Land failure takes precedence
    /// (6), followed by water failure (5), otherwise the placement is free (8). Width and height are unused.
    #[port(address = 0x0072_B480, owner = "zonedeveloper", name = "CheckConstraintTiles", naked)]
    pub unsafe extern "C" fn fc4p_0072b480() {
        core::arch::naked_asm!("mov edx, esp", "sub esp, 152", "call {body}",
            "add esp, 152", "ret 24", body = sym check_constraint_tiles);
    }
}

/// The protected original frame retains unwritten rotated rectangles across records and calls.
unsafe extern "fastcall" fn check_constraint_tiles(this: *mut u8, stack: *mut u32) -> u32 {
    unsafe {
        let x = read_unaligned(stack.add(1)) as i32;
        let z = read_unaligned(stack.add(2)) as i32;
        let facing = read_unaligned(stack.add(5)) as i32;
        let configuration = read_unaligned(stack.add(6)) as *mut u8;
        let dev = super::ZoneDeveloper::from_ptr(this);
        let records = Placements::from_ptr(vcall!(configuration,0x5C => fn()->*mut u8));
        let width = stack.sub(19).cast::<i32>();
        let height = stack.sub(18).cast::<i32>();
        vcall!(configuration,0x1C => fn(*mut i32,*mut i32),width,height);
        let extent = (read_unaligned(width), read_unaligned(height));
        if placement_count(records) == 0 {
            return 8;
        }
        let (mut water_failed, mut land_failed) = (false, false);
        let mut index = 0u32;
        loop {
            let p = records.first().wrapping_add(index.wrapping_mul(52) as usize);
            let kind = read_unaligned(p);
            if kind == 5 {
                let values = read_unaligned(p.add(0x28).cast::<*const u32>());
                let end = read_unaligned(p.add(0x2C).cast::<*const u32>());
                let subtype = if values == end { 0 } else { read_unaligned(values) };
                let rect = constraint_rect(
                    dev,
                    Placement::from_ptr(p),
                    extent,
                    (x, z),
                    facing,
                    stack.sub(33).cast(),
                );
                water_failed |= failed_rows(rect, |x, z| {
                    vcall!(dev.terrain(),0x88 => fn(i32,i32)->u32,x,z) as u8 != 0
                        && vcall!(dev.terrain(),0x8C => fn(i32,i32)->u32,x,z) as u8 == 0
                });
                if subtype == 1 {
                    let record = Placement::from_ptr(records.first().wrapping_add(index.wrapping_mul(52) as usize));
                    let orientation = i32::from(record.facing()).wrapping_add(facing).wrapping_add(2) % 4;
                    let [x1, z1, x2, z2] = rect;
                    let strip = match orientation {
                        0 => [x1, z1.wrapping_sub(2), x2, z1.wrapping_sub(1)],
                        1 => [x2.wrapping_add(1), z1, x2.wrapping_add(2), z2],
                        2 => [x1, z2.wrapping_add(1), x2, z2.wrapping_add(2)],
                        3 => [x1.wrapping_sub(2), z1, x1.wrapping_sub(1), z2],
                        _ => rect,
                    };
                    water_failed |= failed_rows(strip, |x, z| {
                        vcall!(dev.city(),0x18C => fn(i32,i32)->u32,x,z) as u8 != 0
                            && vcall!(*navigation(),0x20 => fn(i32,i32,u32,u32)->u32,x,z,0,0) as u8 != 0
                    });
                }
            } else if kind == 6 {
                let rect = constraint_rect(
                    dev,
                    Placement::from_ptr(p),
                    extent,
                    (x, z),
                    facing,
                    stack.sub(25).cast(),
                );
                land_failed |= failed_rows(rect, |x, z| vcall!(this,0x24 => fn(i32,i32)->u32,x,z) as u8 == 0);
            }
            index = index.wrapping_add(1);
            if index >= placement_count(records) {
                break;
            }
        }
        if land_failed {
            6
        } else if water_failed {
            5
        } else {
            8
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0072ad00::PORT, fc4p_0072b480::PORT];

#[cfg(test)]
mod tests;

#[cfg(test)]
mod constraint_tests;
