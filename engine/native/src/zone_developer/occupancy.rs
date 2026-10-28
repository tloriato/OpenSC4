// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cell occupancy, placement codes and highlight cleanup.

use super::{Port, ZoneDeveloper};
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn find_cell(this: *mut u8, cell: *const i32) -> *mut u8 = "thiscall" 0x0072_AC60; }
game_fn! { fn clear_tree(this: *mut u8, root: *mut u8) -> u32 = "thiscall" 0x0072_ABE0; }
game_fn! { fn includes_cell(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0046_6610; }
game_fn! { fn matches_zone(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0072_A040; }
game_fn! { fn has_network(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0072_A010; }
game_fn! { fn occupied(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0072_C050; }
game_fn! { fn cell_code(this: *mut u8, entry_edx: u32, x: i32, z: i32, zone_not_needed: u32, water_allowed: u32) -> u64
= "fastcall" 0x0072_C200; }

port! {
    /// Test real lots and, in highlight mode, virtually deleted and newly created lots.
    /// AL is the occupancy result; the remaining EAX bytes retain the final lookup result.
    #[port(address = 0x0072_C050, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CanCreateLotOnCell")]
    pub unsafe extern "thiscall" fn fc4p_0072c050(this: *mut u8, x: i32, z: i32) -> u32 {
        unsafe {
            let developer = ZoneDeveloper::from_ptr(this);
            let cell = [x,z];
            let mut eax = 0;
            let test_real = if developer.real_mode() != 0 { true } else {
                eax = find_cell(this.add(0xE4), cell.as_ptr()) as u32;
                eax == developer.deleted_lot_header() as u32
            };
            if test_real {
                eax = vcall!(this.add(4), 0x18 => fn(i32, i32, u32) -> u32, x, z, 0);
                if eax != 0 { return (eax & !0xFF) | 1; }
            }
            if developer.real_mode() != 0 { return eax & !0xFF; }
            let node = find_cell(this.add(0xD8), cell.as_ptr());
            (node as u32 & !0xFF) | u32::from(node != developer.virtual_lot_header())
        }
    }
}

port! {
    /// Empty the virtual-lot maps, reset both network tools and clear the renderer's highlight.
    #[port(address = 0x0072_C0E0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::ClearHighlight")]
    pub unsafe extern "thiscall" fn fc4p_0072c0e0(this: *mut u8) -> u32 {
        unsafe {
            let developer = ZoneDeveloper::from_ptr(this);
            for offset in [0xD8, 0xE4] {
                let tree = this.add(offset);
                if read_unaligned(tree.add(4).cast::<u32>()) != 0 {
                    let header = read_unaligned(tree.cast::<*mut u8>());
                    clear_tree(tree, read_unaligned(header.add(4).cast::<*mut u8>()));
                    let header = read_unaligned(tree.cast::<*mut u8>());
                    write_unaligned(header.add(8).cast::<*mut u8>(), header);
                    let header = read_unaligned(tree.cast::<*mut u8>());
                    write_unaligned(header.add(4).cast::<u32>(), 0);
                    let header = read_unaligned(tree.cast::<*mut u8>());
                    write_unaligned(header.add(12).cast::<*mut u8>(), header);
                    write_unaligned(tree.add(4).cast::<u32>(), 0);
                }
            }
            if !developer.street_tool().is_null() { vcall!(developer.street_tool(), 0x14 => fn() -> u32); }
            if !developer.road_tool().is_null() { vcall!(developer.road_tool(), 0x14 => fn() -> u32); }
            if !developer.renderer().is_null() {
                vcall!(developer.renderer(), 0x24 => fn() -> u32)
            } else { 0 }
        }
    }
}

port! {
    /// Return a cell placement code: outside 0, water 1, wrong zone 2, network 3, occupied 7, free 8.
    #[port(address = 0x0072_C200, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CanCreateLot")]
    pub unsafe extern "thiscall" fn fc4p_0072c200(
        this: *mut u8, x: i32, z: i32, zone_not_needed: u32, water_allowed: u32
    ) -> i32 {
        unsafe {
            let developer = ZoneDeveloper::from_ptr(this);
            if x < 0 || x >= developer.cells_x() || z < 0 || z >= developer.cells_z() { return 0; }
            let restriction = developer.restriction();
            if !restriction.is_null() && includes_cell(restriction, x, z) as u8 == 0 { return 0; }
            if water_allowed as u8 == 0 && vcall!(this, 0x24 => fn(i32, i32) -> u32, x, z) as u8 != 0 {
                return 1;
            }
            if developer.real_mode() != 0 && zone_not_needed as u8 == 0 && matches_zone(this, x, z) as u8 == 0 {
                return 2;
            }
            if has_network(this, x, z) as u8 != 0 { return 3; }
            if occupied(this, x, z) as u8 != 0 { 7 } else { 8 }
        }
    }
}

port! {
    /// Return the minimum cell placement code across a rectangle, X outer and Z inner, starting at eight.
    /// Water is allowed when requested or when the configuration's low result byte allows it.
    /// The register signature captures incoming EDX; the seven stack arguments retain their thiscall positions.
    /// EDX at return is the last cell call's EDX, the height for an empty Z range, or its preceding value.
    #[port(address = 0x0072_C2D0, owner = "zonedeveloper", name = "CanCreateLotRect")]
    pub unsafe extern "fastcall" fn fc4p_0072c2d0(
        this: *mut u8, entry_edx: u32, x: i32, z: i32, width: i32, height: i32,
        zone_not_needed: u32, water_allowed: u32, config: *mut u8
    ) -> u64 {
        unsafe {
            let mut edx = entry_edx;
            let water = if water_allowed as u8 != 0 { 1 }
                else if !config.is_null() {
                    let result = super::virtual_registers(config, 0x38,
                        read_unaligned(config.cast::<u32>()), edx, &[]);
                    edx = (result >> 32) as u32;
                    result as u8
                }
                else { 0 };
            let water_word = (water_allowed & !0xFF) | u32::from(water);
            let end_x = x.wrapping_add(width);
            let end_z = z.wrapping_add(height);
            let mut result = 8;
            let mut cx = x;
            if cx < end_x { edx = height as u32; }
            while cx < end_x {
                let mut cz = z;
                while cz < end_z {
                    let code = cell_code(this.wrapping_sub(4), water_word, cx, cz, zone_not_needed, water_word);
                    edx = (code >> 32) as u32;
                    result = result.min(code as i32);
                    cz = cz.wrapping_add(1);
                }
                cx = cx.wrapping_add(1);
            }
            (u64::from(edx) << 32) | u64::from(result as u32)
        }
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_0072c050::PORT,
    fc4p_0072c0e0::PORT,
    fc4p_0072c200::PORT,
    fc4p_0072c2d0::PORT,
];

#[cfg(test)]
mod tests;
