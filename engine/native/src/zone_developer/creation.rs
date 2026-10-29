// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Conditional lot creation through the lot-manager interface.

use super::{Port, ZoneDeveloper};
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn face_road(this: *mut u8, x: i32, z: i32, width: i32, height: i32,
        list: *mut *mut u8, out: *mut *mut u8) = "thiscall" 0x0072_D200;
}
game_fn! { fn destroy_list(this: *mut u8) = "thiscall" 0x0056_50A0; }
game_fn! { fn virtual_lot(this: *mut u8, key: *const [i32; 2]) -> u64 = "thiscall" 0x0072_F7D0; }

port! {
    /// Validate a rectangle, then create a real lot or record its virtual cells. The configuration
    /// argument is ignored. An automatic facing uses a fresh empty list and the road-facing helper.
    /// Virtual payloads retain the rectangle's origin and facing, even at later cells.
    /// EDX is the output pointer on the real and rejected paths; the highlight path retains the last
    /// insertion's EDX and replaces only AL of its completed width counter.
    #[port(address = 0x0073_0A80, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CreateLotIfPossible")]
    pub unsafe extern "thiscall" fn fc4p_00730a80(
        this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: i32,
        out: *mut *mut u8, zone_not_needed: u32, water_allowed: u32, _configuration: *mut u8
    ) -> u64 {
        // SAFETY: the game supplies its lot-manager interface and writable output pointer.
        unsafe {
            write_unaligned(out, core::ptr::null_mut());
            let checked = super::virtual_registers(this, 0x50, width as u32, read_unaligned(this.cast()),
                &[x as u32, z as u32, width as u32, height as u32, zone_not_needed,
                    water_allowed, 0]);
            if checked as u32 == 8 {
                let dev = ZoneDeveloper::from_ptr(this.sub(4));
                if dev.real_mode() == 0 {
                    let mut edx = (checked >> 32) as u32;
                    let mut column = 0i32;
                    while column < width {
                        edx = x as u32;
                        let mut row = 0i32;
                        while row < height {
                            let key = [x.wrapping_add(column), z.wrapping_add(row)];
                            let result = virtual_lot(this.add(0xD4), &key);
                            let payload = result as u32 as *mut i32;
                            write_unaligned(payload, x);
                            write_unaligned(payload.add(1), z);
                            write_unaligned(payload.add(2), facing);
                            edx = (result >> 32) as u32;
                            row = row.wrapping_add(1);
                        }
                        column = column.wrapping_add(1);
                    }
                    return (u64::from(edx) << 32) | u64::from((column as u32 & !0xFF) | 1);
                }
                if facing == -1 {
                    let header = allocate(12);
                    write_unaligned(header.cast::<*mut u8>(), header);
                    write_unaligned(header.add(4).cast::<*mut u8>(), header);
                    let mut list = header;
                    face_road(this.sub(4), x, z, width, height, &mut list, out);
                    destroy_list((&mut list as *mut *mut u8).cast());
                } else {
                    vcall!(this, 0x5C => fn(i32, i32, i32, i32, i32, *mut *mut u8),
                        x, z, width, height, facing, out);
                }
            }
            (u64::from(out as u32) << 32) | u64::from(!read_unaligned(out).is_null())
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00730a80::PORT];

#[cfg(test)]
mod tests;
