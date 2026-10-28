// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lot edge vertices and neighboring cells.

use super::Port;
use core::ptr::{read_unaligned, write_unaligned};

port! {
    /// Read the lot bounds and forward them, the output list and flags to GetLotsSurroundingRect.
    #[port(address = 0x0072_A750, owner = "zonedeveloper", name = "GetLotsSurroundingLot")]
    pub unsafe extern "thiscall" fn fc4p_0072a750(this: *mut u8, lot: *mut u8, list: *mut u8, flags: u32) -> u64 {
        unsafe {
            let mut rect = [0i32; 4];
            vcall!(lot, 0x38 => fn(*mut i32) -> u32, rect.as_mut_ptr());
            super::virtual_registers(this, 0x28, read_unaligned(this.cast::<u32>()), list as u32,
                &[rect.as_ptr() as u32, list as u32, flags])
        }
    }
}

port! {
    /// Write vertices of the first selected edge (north, east, south, west). With no edge, leave output alone.
    /// EAX retains the incremented far coordinate, or its entry upper bytes and the flag byte for no edge.
    #[port(address = 0x0072_A790, owner = "zonedeveloper", name = "GetVerticesAtEdgeOfLot", naked)]
    pub unsafe extern "C" fn fc4p_0072a790() {
        core::arch::naked_asm!("mov ecx, eax", "jmp {body}", body = sym edge_vertices);
    }
}

unsafe extern "fastcall" fn edge_vertices(
    entry_eax: u32,
    entry_edx: u32,
    rect: *const i32,
    flags: u32,
    out: *mut i32,
) -> u64 {
    unsafe {
        let flags = flags as u8;
        let (result, eax, edx) = if flags & 1 != 0 {
            let x2 = read_unaligned(rect.add(2)).wrapping_add(1);
            let x1 = read_unaligned(rect);
            let z1 = read_unaligned(rect.add(1));
            ([x1, z1, x2, z1], x2 as u32, z1 as u32)
        } else if flags & 2 != 0 {
            let z2 = read_unaligned(rect.add(3)).wrapping_add(1);
            let x2 = read_unaligned(rect.add(2)).wrapping_add(1);
            let z1 = read_unaligned(rect.add(1));
            ([x2, z1, x2, z2], z2 as u32, x2 as u32)
        } else if flags & 4 != 0 {
            let z2 = read_unaligned(rect.add(3)).wrapping_add(1);
            let x1 = read_unaligned(rect);
            let x2 = read_unaligned(rect.add(2)).wrapping_add(1);
            ([x1, z2, x2, z2], x2 as u32, z2 as u32)
        } else if flags & 8 != 0 {
            let z2 = read_unaligned(rect.add(3)).wrapping_add(1);
            let x1 = read_unaligned(rect);
            let z1 = read_unaligned(rect.add(1));
            ([x1, z1, x1, z2], z2 as u32, x1 as u32)
        } else {
            return (u64::from(entry_edx) << 32) | u64::from((entry_eax & !0xFF) | u32::from(flags));
        };
        for (i, value) in result.into_iter().enumerate() {
            write_unaligned(out.add(i), value);
        }
        (u64::from(edx) << 32) | u64::from(eax)
    }
}

port! {
    /// Start with an empty rectangle, then write the first selected neighbor edge within the city.
    /// The success test compares only the resulting X bounds. EAX's upper bytes come from the output pointer.
    #[port(address = 0x0072_A820, owner = "zonedeveloper", name = "GetNeighborCellsAtEdgeOfLot")]
    pub unsafe extern "thiscall" fn fc4p_0072a820(this: *mut u8, rect: *const i32, flags: u32, out: *mut i32) -> u64 {
        unsafe {
            write_unaligned(out.add(2), -1);
            write_unaligned(out.add(3), -1);
            write_unaligned(out, 0);
            write_unaligned(out.add(1), 0);
            if flags & 1 != 0 {
                let z1 = read_unaligned(rect.add(1));
                if z1 > 0 {
                    let x2 = read_unaligned(rect.add(2));
                    let x1 = read_unaligned(rect);
                    write_unaligned(out.add(2), x2);
                    write_unaligned(out.add(3), z1.wrapping_sub(1));
                    write_unaligned(out, x1);
                    write_unaligned(out.add(1), z1.wrapping_sub(1));
                }
            } else if flags & 2 != 0 {
                let limit = read_unaligned(this.add(0x24).cast::<i32>()).wrapping_sub(1);
                let x2 = read_unaligned(rect.add(2));
                if x2 < limit {
                    let x = x2.wrapping_add(1);
                    let z2 = read_unaligned(rect.add(3));
                    let z1 = read_unaligned(rect.add(1));
                    write_unaligned(out.add(2), x);
                    write_unaligned(out.add(3), z2);
                    write_unaligned(out, x);
                    write_unaligned(out.add(1), z1);
                }
            } else if flags & 4 != 0 {
                let limit = read_unaligned(this.add(0x28).cast::<i32>()).wrapping_sub(1);
                let z2 = read_unaligned(rect.add(3));
                if z2 < limit {
                    let z = z2.wrapping_add(1);
                    let x2 = read_unaligned(rect.add(2));
                    let x1 = read_unaligned(rect);
                    write_unaligned(out, x1);
                    write_unaligned(out.add(1), z);
                    write_unaligned(out.add(2), x2);
                    write_unaligned(out.add(3), z);
                }
            } else if flags & 8 != 0 {
                let x1 = read_unaligned(rect);
                if x1 > 0 {
                    let x = x1.wrapping_sub(1);
                    let z2 = read_unaligned(rect.add(3));
                    let z1 = read_unaligned(rect.add(1));
                    write_unaligned(out.add(2), x);
                    write_unaligned(out.add(3), z2);
                    write_unaligned(out, x);
                    write_unaligned(out.add(1), z1);
                }
            }
            let valid = read_unaligned(out.add(2)) >= read_unaligned(out);
            (u64::from(u32::from(valid)) << 32) | u64::from((out as u32 & !0xFF) | u32::from(valid))
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072a750::PORT, fc4p_0072a790::PORT, fc4p_0072a820::PORT];

#[cfg(test)]
mod tests;
