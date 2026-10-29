// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction of the lot object used by the zone developer.

use super::Port;
use core::ptr::write_unaligned;

layout! {
    /// A lot's bounds and oriented anchor, with the three owned collection headers.
    pub struct Lot {
        0x00 => primary_vtable / set_primary_vtable: u32,
        0x04 => secondary_vtable / set_secondary_vtable: u32,
        0x08 => occupant_vtable / set_occupant_vtable: u32,
        0x1C => serializable_vtable / set_serializable_vtable: u32,
        0x20 => bounds / set_bounds: [u8; 4],
        0x40 => first_header / set_first_header: *mut u8,
        0x60 => second_header / set_second_header: *mut u8,
        0x7A => anchor / set_anchor: [u8; 4],
        0x7E => facing / set_facing: u8,
        0x81 => zone / set_zone: u8,
        0x84 => list_header / set_list_header: *mut u8,
        0x88 => marker / set_marker: u8,
    }
}

game_fn! { fn init_occupant(this: *mut u8) -> u32 = "thiscall" 0x005F_F100; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_data! { static zone_manager: *mut u8 = 0x00B4_3D14; }

port! {
    /// Construct a lot and its empty collections, orient the byte-sized bounds, then read its anchor's zone.
    /// Padding, tree-header payloads and the list-header payload remain untouched.
    #[port(address = 0x006C_1E50, owner = "zonedeveloper", name = "ConstructLot")]
    pub unsafe extern "thiscall" fn fc4p_006c1e50(
        this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: u32
    ) -> *mut u8 {
        unsafe {
            let lot = Lot::from_ptr(this);
            lot.set_primary_vtable(0x00AB_16E8);
            lot.set_secondary_vtable(0x00A8_0784);
            init_occupant(this.add(8));
            lot.set_serializable_vtable(0x00AB_1800);
            lot.set_primary_vtable(0x00AB_19B0);
            lot.set_secondary_vtable(0x00AB_1994);
            lot.set_occupant_vtable(0x00AB_1930);
            lot.set_serializable_vtable(0x00AB_1914);
            for offset in (0x24..=0x40).step_by(4) {
                write_unaligned(this.add(offset).cast::<u32>(), 0);
            }
            let first = allocate(0x18);
            lot.set_first_header(first);
            write_unaligned(this.add(0x44).cast::<u32>(), 0);
            *first = 0;
            write_unaligned(first.add(4).cast::<u32>(), 0);
            write_unaligned(first.add(8).cast::<*mut u8>(), first);
            write_unaligned(first.add(12).cast::<*mut u8>(), first);
            for offset in (0x4C..=0x60).step_by(4) {
                write_unaligned(this.add(offset).cast::<u32>(), 0);
            }
            let second = allocate(0x18);
            lot.set_second_header(second);
            write_unaligned(this.add(0x64).cast::<u32>(), 0);
            *second = 0;
            write_unaligned(second.add(4).cast::<u32>(), 0);
            write_unaligned(second.add(8).cast::<*mut u8>(), second);
            write_unaligned(second.add(12).cast::<*mut u8>(), second);
            for offset in [0x6C, 0x70, 0x74] {
                write_unaligned(this.add(offset).cast::<u32>(), 0);
            }
            for offset in [0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7F, 0x80, 0x81, 0x82] {
                *this.add(offset) = 0;
            }
            lot.set_list_header(core::ptr::null_mut());
            let list = allocate(0x34);
            write_unaligned(list.cast::<*mut u8>(), list);
            write_unaligned(list.add(4).cast::<*mut u8>(), list);
            lot.set_list_header(list);
            lot.set_marker(0xFF);
            let x1 = x.wrapping_add(width).wrapping_sub(1) as u8;
            let z1 = z.wrapping_add(height).wrapping_sub(1) as u8;
            lot.set_bounds([x as u8, z as u8, x1, z1]);
            let facing = facing as u8;
            lot.set_facing(facing);
            match facing {
                0 => lot.set_anchor([x1, z as u8, width as u8, height as u8]),
                1 => lot.set_anchor([x1, z1, height as u8, width as u8]),
                2 => lot.set_anchor([x as u8, z1, width as u8, height as u8]),
                3 => lot.set_anchor([x as u8, z as u8, height as u8, width as u8]),
                _ => {}
            }
            let anchor = lot.anchor();
            let zone = vcall!(*zone_manager(), 0x30 => fn(u32, u32) -> u32,
                u32::from(anchor[0]), u32::from(anchor[1]));
            lot.set_zone(zone as u8);
            this
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006c1e50::PORT];

#[cfg(test)]
mod tests;
