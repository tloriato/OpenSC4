// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Shared grid geometry and the four directional byte-distance sweeps.

use crate::port::Port;
use crate::x87::F80;
use core::ptr::read_unaligned;

layout! {
    /// Unsigned 16-bit grid geometry and its nested row vector.
    pub struct U16Grid {
        0x18 => tract_size / set_tract_size: i32,
        0x1C => tract_shift / set_tract_shift: u32,
        0x20 => count_x / set_count_x: i32,
        0x24 => count_z / set_count_z: i32,
        0x28 => width_x / set_width_x: f32,
        0x2C => width_z / set_width_z: f32,
        0x30 => inverse_x / set_inverse_x: f32,
        0x34 => inverse_z / set_inverse_z: f32,
        0x38 => rows / set_rows: *mut u8,
    }
}

game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_fn! { fn resize_rows(vector: *mut u8, count: i32) -> u64 = "thiscall" 0x0046_23B0; }
game_fn! { fn fill_row(row: *mut u8, count: i32, value: *const u32) -> u64 = "thiscall" 0x0046_5710; }

port! {
    /// Keep the caller's size argument address: growing a row writes zero to that word.
    /// The body returns EDX:EAX explicitly, including the unchanged-size path.
    #[port(address = 0x0046_5630, owner = "residential", name = "U16GridSetTractSize", naked)]
    pub unsafe extern "C" fn fc4p_00465630() {
        core::arch::naked_asm!(
            "push edx", "lea edx, [esp + 4]", "call {body}", "ret 4",
            body = sym set_tract_size,
        )
    }
}

unsafe extern "fastcall" fn set_tract_size(this: *mut u8, stack: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let grid = U16Grid::from_ptr(this);
        let size = stack.add(1).read();
        if size as i32 == grid.tract_size() {
            return (u64::from(entry_edx) << 32) | u64::from(size);
        }
        grid.set_tract_size(size as i32);
        grid.set_tract_shift(0);
        if size as i32 > 1 {
            let mut n = size as i32;
            let mut shift = 0;
            while n > 1 {
                n /= 2;
                shift += 1;
            }
            grid.set_tract_shift(shift);
        }
        let city = *city();
        grid.set_count_x(vcall!(city, 0x174 => fn() -> i32).wrapping_shr(grid.tract_shift()));
        grid.set_count_z(vcall!(city, 0x178 => fn() -> i32).wrapping_shr(grid.tract_shift()));
        grid.set_width_x((vcall_st0!(city, 0x16C) * F80::from_i32(grid.tract_size())).to_f32());
        let width_z = vcall_st0!(city, 0x170) * F80::from_i32(grid.tract_size());
        grid.set_width_z(width_z.to_f32());
        grid.set_inverse_x((F80::from_f32(1.0) / F80::from_f32(grid.width_x())).to_f32());
        grid.set_inverse_z((F80::from_f32(1.0) / width_z).to_f32());
        let mut result = resize_rows(this.add(0x38), grid.count_x());
        let mut x = 0i32;
        while x < grid.count_x() {
            let row = grid.rows().wrapping_offset(x.wrapping_mul(12) as isize);
            stack.add(1).write(0);
            result = fill_row(row, grid.count_z(), stack.add(1));
            x = x.wrapping_add(1);
        }
        (result & 0xFFFF_FFFF_0000_0000) | u64::from(grid.count_x() as u32)
    }
}

/// The outer vector contains twelve-byte row descriptors.
unsafe fn row(vector: *mut u8, x: i32) -> *mut u8 {
    unsafe {
        let rows = read_unaligned(vector.cast::<*mut u8>());
        read_unaligned(rows.wrapping_offset(x.wrapping_mul(12) as isize).cast::<*mut u8>())
    }
}

unsafe fn sweep_cell(p: *mut u8, distance: &mut i32, changed: &mut u32) {
    unsafe {
        let old = i32::from(p.read());
        *distance = distance.wrapping_add(1).min(old);
        *changed |= (old ^ *distance) as u32;
        p.write(*distance as u8);
    }
}

port! {
    /// Sweep each row forward and backward, then each column forward and backward.
    /// The running distance starts at 255 but does not wrap; EAX is the OR of all changes.
    #[port(address = 0x005E_4E80, owner = "residential", name = "ByteGridDistancePass")]
    pub unsafe extern "C" fn fc4p_005e4e80(vector: *mut u8, count_x: i32, count_z: i32) -> u32 {
        unsafe {
            let mut changed = 0;
            for x in 0..count_x {
                let p = row(vector, x);
                let mut d = 255;
                for z in 0..count_z { sweep_cell(p.wrapping_offset(z as isize), &mut d, &mut changed); }
                let mut z = count_z.wrapping_sub(1);
                d = 255;
                while z >= 0 {
                    sweep_cell(p.wrapping_offset(z as isize), &mut d, &mut changed);
                    z = z.wrapping_sub(1);
                }
            }
            for z in 0..count_z {
                let mut d = 255;
                for x in 0..count_x { sweep_cell(row(vector, x).wrapping_offset(z as isize), &mut d, &mut changed); }
                let mut x = count_x.wrapping_sub(1);
                d = 255;
                while x >= 0 {
                    sweep_cell(row(vector, x).wrapping_offset(z as isize), &mut d, &mut changed);
                    x = x.wrapping_sub(1);
                }
            }
            changed
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00465630::PORT, fc4p_005e4e80::PORT];

#[cfg(test)]
mod tests;
