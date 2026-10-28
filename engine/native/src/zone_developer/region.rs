// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Packed cell regions, cropping, lot filtering and zone reconciliation.

use super::{Port, ZoneDeveloper};
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Inclusive cell bounds followed by a column-major packed boolean map.
    pub struct Region {
        0x00 => x1 / set_x1: i32,
        0x04 => z1 / set_z1: i32,
        0x08 => x2 / set_x2: i32,
        0x0C => z2 / set_z2: i32,
        0x10 => vtable / set_vtable: u32,
        0x14 => width / set_width: i32,
        0x18 => height / set_height: i32,
        0x1C => words / set_words: u32,
        0x20 => columns / set_columns: *mut *mut u32,
    }
}

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn free(memory: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { fn construct_bits(this: *mut u8, width: i32, height: i32, fill: *const u8) -> u32 = "thiscall" 0x0044_22A0; }
game_fn! { fn destroy_bits(this: *mut u8) -> u64 = "thiscall" 0x0044_1A60; }
game_fn! { fn cell_occupied(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0072_C050; }

impl Region {
    /// The caller has already checked the local coordinates against the packed map's dimensions.
    pub(super) unsafe fn bit(self, x: u32, z: u32) -> *mut u32 {
        unsafe { read_unaligned(self.columns().wrapping_add(x as usize)).wrapping_add((z >> 5) as usize) }
    }

    pub(super) unsafe fn get(self, x: u32, z: u32) -> bool {
        unsafe { read_unaligned(self.bit(x, z)) & 1u32.wrapping_shl(z) != 0 }
    }

    pub(super) unsafe fn set(self, x: u32, z: u32, value: bool) {
        unsafe {
            let p = self.bit(x, z);
            let mask = 1u32.wrapping_shl(z);
            let old = read_unaligned(p);
            write_unaligned(p, if value { old | mask } else { old & !mask });
        }
    }

    pub(super) unsafe fn includes(self, x: i32, z: i32) -> bool {
        let x = x.wrapping_sub(self.x1()) as u32;
        let z = z.wrapping_sub(self.z1()) as u32;
        x < self.width() as u32 && z < self.height() as u32 && unsafe { self.get(x, z) }
    }
}

/// Allocate a region shell, copy the bounds one word at a time, and construct its packed map.
unsafe fn new_region(source: Region, fill: u8) -> Region {
    unsafe {
        let p = allocate(0x24);
        if !p.is_null() {
            let result = Region::from_ptr(p);
            result.set_x1(source.x1());
            result.set_z1(source.z1());
            result.set_x2(source.x2());
            result.set_z2(source.z2());
            construct_bits(
                p.add(16),
                source.x2().wrapping_sub(source.x1()).wrapping_add(1),
                source.z2().wrapping_sub(source.z1()).wrapping_add(1),
                &fill,
            );
        }
        Region::from_ptr(p)
    }
}

port! {
    /// Crop to the set cells' bounding box, preserving holes and untouched tail bits. An empty region
    /// crops to its origin cell. Keep the existing allocation when no edge can be removed.
    #[port(address = 0x0072_AF30, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CompressRegion")]
    pub unsafe extern "stdcall" fn fc4p_0072af30(slot: *mut *mut u8) -> u64 {
        unsafe {
            let source = Region::from_ptr(read_unaligned(slot));
            let (mut min_x, mut min_z, mut max_x, mut max_z) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            let mut empty = true;
            let mut x = 0i32;
            while x < source.width() {
                let mut z = 0i32;
                while z < source.height() {
                    if source.get(x as u32, z as u32) {
                        min_x = min_x.min(x);
                        min_z = min_z.min(z);
                        max_x = max_x.max(x);
                        max_z = max_z.max(z);
                        empty = false;
                    }
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            if empty {
                (min_x, min_z, max_x, max_z) = (0, 0, 0, 0);
            }
            if min_x <= 0 && min_z <= 0 && max_x >= source.width().wrapping_sub(1)
                && max_z >= source.height().wrapping_sub(1) {
                return (u64::from(max_x as u32) << 32) | u64::from(max_z as u32);
            }
            let bounds = [source.x1().wrapping_add(min_x), source.z1().wrapping_add(min_z),
                source.x1().wrapping_add(max_x), source.z1().wrapping_add(max_z)];
            let p = allocate(0x24);
            if !p.is_null() {
                write_unaligned(p.cast::<[i32; 4]>(), bounds);
                construct_bits(p.add(16), bounds[2].wrapping_sub(bounds[0]).wrapping_add(1),
                    bounds[3].wrapping_sub(bounds[1]).wrapping_add(1), &1);
            }
            let result = Region::from_ptr(p);
            let old = Region::from_ptr(read_unaligned(slot));
            let dx = result.x1().wrapping_sub(old.x1());
            let dz = result.z1().wrapping_sub(old.z1());
            let (width, height) = (result.width(), result.height());
            let mut x = 0i32;
            while x < width {
                let mut z = 0i32;
                while z < height {
                    let old = Region::from_ptr(read_unaligned(slot));
                    result.set(x as u32, z as u32, old.get(x.wrapping_add(dx) as u32, z.wrapping_add(dz) as u32));
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            let old = Region::from_ptr(read_unaligned(slot));
            let mut result_edx = slot as u32;
            if !old.as_ptr().is_null() {
                old.set_vtable(0x00A8_6364);
                if !old.columns().is_null() {
                    let mut x = 0u32;
                    while x < old.width() as u32 {
                        free(read_unaligned(old.columns().wrapping_add(x as usize)).cast());
                        x = x.wrapping_add(1);
                    }
                    free(old.columns().cast());
                }
                result_edx = (free(old.as_ptr()) >> 32) as u32;
            }
            write_unaligned(slot, result.as_ptr());
            (u64::from(result_edx) << 32) | u64::from(slot as u32)
        }
    }
}

port! {
    /// Build a fresh region in x-major order. Skip networks and water, retain compatible lots, and
    /// delete incompatible lots before setting their cells. Preview mode checks occupancy before lots.
    #[port(address = 0x0072_CB90, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CreateParcellizableRegion")]
    pub unsafe extern "thiscall" fn fc4p_0072cb90(this: *mut u8, input: *mut u8) -> *mut u8 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let input = Region::from_ptr(input);
            let result = new_region(input, 0);
            let mut x = 0i32;
            while x <= result.x2().wrapping_sub(result.x1()) {
                let cx = result.x1().wrapping_add(x);
                let mut z = 0i32;
                while z <= result.z2().wrapping_sub(result.z1()) {
                    let cz = result.z1().wrapping_add(z);
                    if input.get(x as u32, z as u32)
                        && (dev.current_zone() == 0 || vcall!(this, 0x20 => fn(i32, i32, u32) -> u32, cx, cz, 0x1F4F) as u8 == 0)
                        && vcall!(this, 0x24 => fn(i32, i32) -> u32, cx, cz) as u8 == 0 {
                        let mut keep = true;
                        if dev.real_mode() != 0 || cell_occupied(this, cx, cz) as u8 != 0 {
                            let manager = this.add(4);
                            let lot = vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, cx, cz, 0);
                            if !lot.is_null() {
                                let configuration = vcall!(lot, 0x24 => fn() -> *mut u8);
                                if !configuration.is_null() {
                                    if vcall!(configuration, 0x3C => fn(u32) -> u32, dev.current_zone()) as u8 != 0 {
                                        keep = false;
                                    }
                                } else if (1..=6).contains(&(dev.current_zone() as i32)) {
                                    let zone = vcall!(lot, 0x7C => fn() -> i32);
                                    if (1..=6).contains(&zone) {
                                        keep = false;
                                    }
                                }
                                if keep {
                                    vcall!(manager, 0x60 => fn(*mut u8), lot);
                                }
                            }
                        }
                        result.set(x as u32, z as u32, keep);
                    }
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            result.as_ptr()
        }
    }
}

port! {
    /// Update lots at their origins in z-major order. Refresh changed lots' base textures when allowed,
    /// and dezone all set cells left uncovered by any lot in one call. Destroy the temporary packed map.
    #[port(address = 0x0072_B180, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::UpdateZoneType")]
    pub unsafe extern "thiscall" fn fc4p_0072b180(this: *mut u8, input: *mut u8) -> u64 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let source = Region::from_ptr(input);
            let (x0, z0, x1, z1) = (source.x1(), source.z1(), source.x2(), source.z2());
            let (xend, zend) = (x0.max(x1), z0.max(z1));
            let mut storage = MaybeUninit::<[u32; 9]>::uninit();
            let p = storage.as_mut_ptr().cast::<u8>();
            write_unaligned(p.cast::<[i32; 4]>(), [x0, z0, x1, z1]);
            construct_bits(p.add(16), x1.wrapping_sub(x0).wrapping_add(1),
                z1.wrapping_sub(z0).wrapping_add(1), &0);
            let out = Region::from_ptr(p);
            let force = vcall!(dev.lot_configuration(), 0xB8 => fn() -> u32) as u8;
            let mut any = false;
            let mut z = z0;
            while z <= zend {
                let mut x = x0;
                while x <= xend {
                    if source.includes(x, z) {
                        let manager = this.add(4);
                        let lot = vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 1);
                        if lot.is_null() {
                            if vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 0).is_null() {
                                out.set(x.wrapping_sub(x0) as u32, z.wrapping_sub(z0) as u32, true);
                                any = true;
                            }
                        } else {
                            let old = vcall!(lot, 0x7C => fn() -> i32);
                            vcall!(lot, 0x80 => fn());
                            let new = vcall!(lot, 0x7C => fn() -> i32);
                            if old != new && (force != 0 || vcall!(lot, 0x84 => fn() -> i32) < 2) {
                                vcall!(dev.lot_configuration(), 0xAC => fn(*mut u8), lot);
                            }
                        }
                    }
                    x = x.wrapping_add(1);
                }
                z = z.wrapping_add(1);
            }
            if any {
                vcall!(dev.zone_manager(), 0x2C => fn(*mut u8, u32, u32, u32, u32, u32, u32, u32, u32, u32),
                    p, 0, 1, 1, 0, 1, 0, 0, 0, 0);
            }
            out.set_vtable(0x00A8_6364);
            destroy_bits(p.add(16))
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0072af30::PORT, fc4p_0072b180::PORT, fc4p_0072cb90::PORT];

#[cfg(test)]
mod tests;
