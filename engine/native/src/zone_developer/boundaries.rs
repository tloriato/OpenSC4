// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Boundary traversal and selection of the two parcel directions.

use super::{Port, ZoneDeveloper};
use core::ptr::read_unaligned;

layout! {
    /// The restriction's inclusive rectangle and column bit arrays.
    struct BoundaryRegion {
        0x00 => x_min / set_x_min: i32,
        0x04 => z_min / set_z_min: i32,
        0x08 => x_max / set_x_max: i32,
        0x0C => z_max / set_z_max: i32,
        0x20 => columns / set_columns: *mut *mut u32,
    }
}

game_fn! {
    fn examine_cell(this: *mut u8, x: i32, z: i32, condition: *mut i32, score: *mut i32)
        = "thiscall" 0x0072_FAB0;
}

unsafe fn column(dev: ZoneDeveloper, index: i32) -> *mut u32 {
    unsafe {
        let region = BoundaryRegion::from_ptr(dev.restriction());
        read_unaligned(region.columns().wrapping_offset(index as isize))
    }
}

unsafe fn includes(dev: ZoneDeveloper, x: i32, z: i32) -> bool {
    unsafe { read_unaligned(column(dev, x).wrapping_add((z as u32 >> 5) as usize)) & 1u32.wrapping_shl(z as u32) != 0 }
}

unsafe fn examine(dev: ZoneDeveloper, x: i32, z: i32, side: usize) {
    unsafe {
        examine_cell(
            dev.as_ptr(),
            x,
            z,
            dev.as_ptr().add(0xBC + 4 * side).cast(),
            dev.as_ptr().add(0xAC + 4 * side).cast(),
        );
    }
}

port! {
    /// Examine selected edge cells, growing the coordinates by one inside the city limits.
    /// Choose the first maximum score, then a perpendicular direction, with the residential and
    /// alternate-orientation swaps. Keep the final EDX and the partial-byte EAX result.
    #[port(address = 0x0073_1C80, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::ExamineBoundaries")]
    pub unsafe extern "thiscall" fn fc4p_00731c80(this: *mut u8) -> u64 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let region = BoundaryRegion::from_ptr(dev.restriction());
            let x0 = region.x_min();
            let x0 = if x0 > 0 { x0.wrapping_sub(1) } else { x0 };
            let x1 = region.x_max();
            let x1 = if x1 < dev.cells_x().wrapping_sub(1) { x1.wrapping_add(1) } else { x1 };
            let z0 = region.z_min();
            let z0 = if z0 > 0 { z0.wrapping_sub(1) } else { z0 };
            let z1 = region.z_max();
            let z1 = if z1 < dev.cells_z().wrapping_sub(1) { z1.wrapping_add(1) } else { z1 };
            dev.set_scores([0; 4]);
            dev.set_conditions([i32::from(dev.place_streets() == 0); 4]);

            let mut x = region.x_min();
            if x <= region.x_max() {
                loop {
                    if *column(dev, x.wrapping_sub(region.x_min())).cast::<u8>() & 1 != 0 {
                        examine(dev, x, z0, 0);
                    }
                    if includes(dev, x.wrapping_sub(region.x_min()), region.z_max().wrapping_sub(region.z_min())) {
                        examine(dev, x, z1, 2);
                    }
                    x = x.wrapping_add(1);
                    if x > region.x_max() { break; }
                }
            }
            let mut z = region.z_min();
            if z <= region.z_max() {
                loop {
                    if includes(dev, 0, z.wrapping_sub(region.z_min())) {
                        examine(dev, x0, z, 3);
                    }
                    if includes(dev, region.x_max().wrapping_sub(region.x_min()), z.wrapping_sub(region.z_min())) {
                        examine(dev, x1, z, 1);
                    }
                    z = z.wrapping_add(1);
                    if z > region.z_max() { break; }
                }
            }

            let scores = dev.scores();
            let mut eax = i32::MIN;
            let mut d = u32::MAX;
            for (side, &score) in scores[..3].iter().enumerate() {
                if score > eax {
                    eax = score;
                    d = side as u32;
                }
            }
            if scores[3] > eax { d = 3; }
            let mut edx = scores[2];
            let q = match d {
                0 | 2 => {
                    edx = scores[1];
                    eax = scores[3];
                    if scores[1] <= scores[3] { 3 } else { 1 }
                }
                1 | 3 => {
                    eax = scores[0];
                    if scores[0] > scores[2] { 0 } else { 2 }
                }
                _ => u32::MAX,
            };
            eax = ((eax as u32 & !0xFF) | u32::from(dev.place_streets())) as i32;
            dev.set_orientation_flag(1);
            dev.set_orientation_d(d);
            dev.set_orientation_q(q);
            if dev.place_streets() != 0 {
                eax = dev.current_zone() as i32;
                if eax > 0 && eax <= 2 {
                    edx = region.x_max().wrapping_sub(region.x_min()).wrapping_add(1);
                    if edx > 2 {
                        eax = region.z_max().wrapping_sub(region.z_min()).wrapping_add(1);
                        if eax > 2 {
                            dev.set_orientation_flag(0);
                            dev.set_orientation_q(d);
                            dev.set_orientation_d(q);
                        }
                    }
                }
            }
            eax = ((eax as u32 & !0xFF) | u32::from(dev.alternate_orientation())) as i32;
            if dev.alternate_orientation() != 0 {
                edx = dev.orientation_d() as i32;
                eax = dev.orientation_q() as i32;
                dev.set_orientation_flag(u8::from(dev.orientation_flag() == 0));
                dev.set_orientation_q(edx as u32);
                dev.set_orientation_d(eax as u32);
            }
            (u64::from(edx as u32) << 32) | u64::from(eax as u32)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00731c80::PORT];

#[cfg(test)]
mod tests;
