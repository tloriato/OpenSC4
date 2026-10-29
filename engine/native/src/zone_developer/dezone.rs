// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Delete eligible lots, restore their remnants and optionally remove streets on the selected cells.

use super::{region::Region, Port, ZoneDeveloper};
use core::mem::MaybeUninit;
use core::ptr::read_unaligned;

game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { fn construct_bits(this: *mut u8, width: i32, height: i32, fill: *const u8) = "thiscall" 0x0044_22A0; }
game_fn! { fn destroy_bits(this: *mut u8) -> u64 = "thiscall" 0x0044_1A60; }
game_fn! { fn create_remnants(this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: u32) = "thiscall" 0x0072_D440; }
game_data! { static zone_manager: *mut u8 = 0x00B4_3D00; }

unsafe fn defer_operations(dev: ZoneDeveloper, value: u32) {
    unsafe {
        let allocation = allocate_message(0x2C);
        let message = if allocation.is_null() {
            core::ptr::null_mut()
        } else {
            construct_message(allocation)
        };
        vcall!(message, 4 => fn());
        vcall!(message, 0x14 => fn(u32), 0x49D9_68C4);
        vcall!(message, 0x2C => fn(u32), value);
        vcall!(dev.message_server(), 0x10 => fn(*mut u8, u32), message, 0);
        vcall!(message, 8 => fn());
    }
}

port! {
    /// Bracket the operation with defer-occupant messages. Visit set cells Z before X, delete RCI and
    /// zones 11/12 lots, and re-create their remnants with the lot's zone in current_zone. Clear that
    /// scratch zone after every encountered lot, including ineligible ones. Optionally collect cells
    /// with street/dirt networks and no protected network and demolish that packed region once.
    #[port(address = 0x0072_E6A0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::OnDezone")]
    pub unsafe extern "thiscall" fn fc4p_0072e6a0(this: *mut u8, source: *mut u8, remove_streets: u32) -> u64 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            defer_operations(dev, 1);
            let region = Region::from_ptr(source);
            let (x1, z1) = (region.x1(), region.z1());
            let (x2, z2) = (region.x2().max(x1), region.z2().max(z1));
            let mut memory = MaybeUninit::<[u32; 9]>::uninit();
            let temporary = Region::from_ptr(memory.as_mut_ptr().cast());
            temporary.set_x1(region.x1());
            temporary.set_z1(region.z1());
            temporary.set_x2(region.x2());
            temporary.set_z2(region.z2());
            let fill = 0u8;
            construct_bits(temporary.as_ptr().add(16),
                region.x2().wrapping_sub(region.x1()).wrapping_add(1),
                region.z2().wrapping_sub(region.z1()).wrapping_add(1), &fill);
            let mut any = false;
            let mut z = z1;
            loop {
                let mut x = x1;
                loop {
                    if region.includes(x, z) {
                        let rows = read_unaligned(this.add(0x54).cast::<*mut u8>());
                        let row = read_unaligned(rows.wrapping_offset(x.wrapping_mul(12) as isize).cast::<*mut u8>());
                        let lot = read_unaligned(row.wrapping_offset(z.wrapping_mul(4) as isize).cast::<*mut u8>());
                        if !lot.is_null() {
                            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
                            vcall!(lot, 0x38 => fn(*mut i32), bounds.as_mut_ptr().cast());
                            let facing = vcall!(lot, 0x5C => fn() -> u32);
                            let zone = vcall!(lot, 0x7C => fn() -> u32);
                            dev.set_current_zone(zone);
                            let eligible = vcall!(dev.zone_manager(), 0x34 => fn(u32) -> u32, zone);
                            if eligible as u8 != 0 || dev.current_zone() == 11 || dev.current_zone() == 12 {
                                vcall!(this.add(4), 0x60 => fn(*mut u8), lot);
                                let [lx, lz, rx, rz] = bounds.assume_init();
                                create_remnants(this, lx, lz,
                                    rx.wrapping_sub(lx).wrapping_add(1), rz.wrapping_sub(lz).wrapping_add(1), facing);
                            }
                            dev.set_current_zone(0);
                        }
                        if remove_streets as u8 != 0
                            && vcall!(this, 0x20 => fn(i32, i32, u32) -> u32, x, z, 0x808) as u8 != 0
                            && vcall!(this, 0x20 => fn(i32, i32, u32) -> u32, x, z, 0x1747) as u8 == 0 {
                            temporary.set(x.wrapping_sub(temporary.x1()) as u32, z.wrapping_sub(temporary.z1()) as u32, true);
                            any = true;
                        }
                    }
                    x = x.wrapping_add(1);
                    if x > x2 { break; }
                }
                z = z.wrapping_add(1);
                if z > z2 { break; }
            }
            if any {
                vcall!(read_unaligned(zone_manager()), 0x18 => fn(u32, *mut u8, u32, u32, u32, u32, u32, u32, u32, u32, u32),
                    1, temporary.as_ptr(), 1, 1, 0, 0, 0, 0, 0, 0, 0);
            }
            defer_operations(dev, 0);
            temporary.set_vtable(0x00A8_6364);
            destroy_bits(temporary.as_ptr().add(16))
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072e6a0::PORT];

#[cfg(test)]
mod tests;
