// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Byte-grid resampling with callbacks.

use crate::port::Port;
use core::ptr::read_unaligned;

/// The first word of a vector holds its first element. Each row is a three-word vector.
unsafe fn row_data(vector: *mut u8, x: i32) -> *mut u8 {
    // SAFETY: the caller supplies the game's vector storage and an accessible row.
    unsafe {
        let rows = read_unaligned(vector.cast::<*mut u8>());
        read_unaligned(rows.wrapping_offset(x.wrapping_mul(12) as isize).cast::<*mut u8>())
    }
}

/// A callback receives a byte's address, one full stack word of source data, and the caller's context.
pub type GridCallback = unsafe extern "C" fn(*mut u8, i32, *mut u8);

port! {
    /// Visits matching grid tracts, x before z. Equal shifts pass an unsigned byte stack word;
    /// coarser destinations pass a wrapping sum shifted right twice; finer destinations
    /// receive each unsigned source value throughout its covered rectangle. Bounds are unchecked.
    /// Grid geometry and storage are queried once, destination first, in slots 11, 12, 13 and 24/25.
    /// Storage and row pointers are read again at each point that can follow a callback.
    /// EAX retains the original loop result, including on empty and negative dimensions.
    #[port(address = 0x006F_8970, owner = "residential", name = "ProcessTwoGridsWithCallback<uint8,uint8>")]
    pub unsafe extern "C" fn fc4p_006f8970(
        a: *mut u8, b: *mut u8, callback: GridCallback, context: *mut u8,
    ) -> u32 {
        // SAFETY: the game supplies live grids with these virtual methods and vector storage.
        unsafe {
            let sa = vcall!(a, 0x2C => fn() -> i32);
            let ax = vcall!(a, 0x30 => fn() -> i32);
            let az = vcall!(a, 0x34 => fn() -> i32);
            let av = vcall!(a, 0x60 => fn() -> *mut u8);
            let sb = vcall!(b, 0x2C => fn() -> i32);
            let bx = vcall!(b, 0x30 => fn() -> i32);
            let bz = vcall!(b, 0x34 => fn() -> i32);
            let bv = vcall!(b, 0x64 => fn() -> *mut u8);
            if sa == sb {
                if ax <= 0 {
                    return ax as u32;
                }
                let mut x = 0i32;
                while x < ax {
                    let mut z = 0i32;
                    while z < az {
                        let source = row_data(bv, x).wrapping_offset(z as isize);
                        let value = i32::from(read_unaligned(source.cast::<u8>()));
                        let dest = row_data(av, x).wrapping_offset(z as isize);
                        callback(dest, value, context);
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                0
            } else if sa > sb {
                let d = sa.wrapping_sub(sb) as u32;
                let mut x = 0i32;
                while x < ax {
                    let x0 = x.wrapping_shl(d);
                    let x1 = x.wrapping_add(1).wrapping_shl(d);
                    let mut z = 0i32;
                    while z < az {
                        let z0 = z.wrapping_shl(d);
                        let z1 = z.wrapping_add(1).wrapping_shl(d);
                        let mut sum = 0i32;
                        let mut sx = x0;
                        while sx < x1 {
                            if z0 < z1 {
                                let source = row_data(bv, sx);
                                let mut sz = z0;
                                while sz < z1 {
                                    let p = source.wrapping_offset(sz as isize);
                                    sum = sum.wrapping_add(i32::from(read_unaligned(p.cast::<u8>())));
                                    sz = sz.wrapping_add(1);
                                }
                            }
                            sx = sx.wrapping_add(1);
                        }
                        let value = sum.wrapping_shr(d).wrapping_shr(d);
                        callback(row_data(av, x).wrapping_offset(z as isize), value, context);
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                x as u32
            } else {
                let d = sb.wrapping_sub(sa) as u32;
                let mut x = 0i32;
                while x < bx {
                    let x0 = x.wrapping_shl(d);
                    let x1 = x.wrapping_add(1).wrapping_shl(d);
                    let mut z = 0i32;
                    while z < bz {
                        let source = row_data(bv, x).wrapping_offset(z as isize);
                        let value = i32::from(read_unaligned(source.cast::<u8>()));
                        let z0 = z.wrapping_shl(d);
                        let z1 = z.wrapping_add(1).wrapping_shl(d);
                        let mut dx = x0;
                        while dx < x1 {
                            let mut dz = z0;
                            while dz < z1 {
                                callback(row_data(av, dx).wrapping_offset(dz as isize), value, context);
                                dz = dz.wrapping_add(1);
                            }
                            dx = dx.wrapping_add(1);
                        }
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                bx as u32
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006f8970::PORT];

#[cfg(test)]
mod tests;
