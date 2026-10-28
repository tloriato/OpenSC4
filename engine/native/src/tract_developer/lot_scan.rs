// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The shared cell iterators used for existing lots and decline.

use super::*;

game_fn! { #[allow(dead_code)] fn register_exit(callback: u32) -> u32 = "C" 0x009E_F059; }
game_fn! { #[allow(dead_code)] fn reset(this: u32, random_start: u32) -> u32 = "thiscall" 0x0049_D1C0; }
game_fn! { #[allow(dead_code)] fn next(this: u32, x: u32, z: u32) -> u32 = "thiscall" 0x0049_CB10; }
game_fn! { #[allow(dead_code)] fn insert(this: u32, result: u32, lot: u32) -> u32 = "thiscall" 0x0049_A050; }
game_data! { static existing_iterator: [u8; 0x30] = 0x00B4_B098; }
game_data! { static existing_initialized: u32 = 0x00B4_B0C8; }
game_data! { static decline_iterator: [u8; 0x30] = 0x00B4_B0CC; }
game_data! { static decline_initialized: u32 = 0x00B4_B0FC; }

/// Initialize only the fields written by the first use. The iterator's padding byte is retained.
unsafe fn initialize<const N: usize>(r: &mut Entry<N>, iterator: u32, flag: *mut u32, exit: u32) {
    // SAFETY: both ports supply their game's writable iterator and initialization flag.
    unsafe {
        r.eax = (r.eax & !0xFF) | u32::from(*flag as u8);
        if r.eax & 1 != 0 {
            return;
        }
        r.edx = *flag;
        r.eax = 1;
        r.edx |= r.eax;
        *flag = r.edx;
        for offset in 0..3 {
            *(iterator.wrapping_add(offset) as *mut u8) = 1;
        }
        for offset in (4..0x2C).step_by(4) {
            store(iterator, offset, 0);
        }
        store(iterator, 0x2C, 1);
        invoke(r, Conv::Cdecl, crate::abi::target(0x009E_F059), &[exit]);
    }
}

/// Set inclusive cell bounds with wrapping shifts and signed upper-bound clamping, then reset.
unsafe fn bounds<const N: usize>(r: &mut Entry<N>, iterator: u32, x: u32, z: u32) {
    // SAFETY: the iterator is writable game state; r.eax holds its map's tract shift.
    unsafe {
        r.edx = z;
        r.ecx = r.eax;
        r.eax = x;
        let min_x = r.eax.wrapping_shl(r.ecx);
        let min_z = r.edx.wrapping_shl(r.ecx);
        r.eax = r.eax.wrapping_add(1).wrapping_shl(r.ecx);
        store(iterator, 4, min_x);
        store(iterator, 8, min_z);
        r.eax = r.eax.wrapping_sub(1);
        r.edx = r.edx.wrapping_add(1).wrapping_shl(r.ecx);
        store(iterator, 0xC, r.eax);
        r.edx = r.edx.wrapping_sub(1);
        store(iterator, 0x10, r.edx);
        if (r.eax as i32) < min_x as i32 {
            store(iterator, 0xC, min_x);
        }
        if (r.edx as i32) < min_z as i32 {
            store(iterator, 0x10, min_z);
        }
        r.ecx = iterator;
        *(iterator.wrapping_add(2) as *mut u8) = 1;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0049_D1C0), &[1]);
    }
}

unsafe fn get_next<const N: usize>(r: &mut Entry<N>, iterator: u32, x: *mut u32, z: *mut u32) {
    // SAFETY: both destinations remain live through the game's iterator call.
    unsafe {
        r.eax = z as u32;
        r.ecx = x as u32;
        let x = r.ecx;
        r.ecx = iterator;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0049_CB10), &[x, r.eax]);
    }
}

register_port! {
    /// Visit every cell in the tract's random order and insert each non-null borrowed lot into the set.
    /// The iterator is initialized once, and reset on every call. Duplicate lots still call insertion.
    0x0070_7FB0, fc4p_00707fb0, list_existing, 0, 0, "cSC4TractDeveloper::ListExistingLots", |r| {
        let this = r.ecx;
        let iterator = existing_iterator() as u32;
        initialize(r, iterator, existing_initialized(), 0x00A7_D290);
        r.ecx = word(this, 0x80); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x34) as usize, &[]);
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
        bounds(r, iterator, word(this, 0x84), word(this, 0x88));
        let (mut x, mut z) = (0u32, 0u32);
        let mut inserted = [0u32; 2];
        get_next(r, iterator, &mut x, &mut z);
        while r.eax as u8 != 0 {
            r.eax = z; r.ecx = word(this, 0x2C); r.edx = word(r.ecx, 0);
            let z_value = r.eax; r.eax = x;
            invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[r.eax, z_value, 0]);
            let mut lot = r.eax;
            if r.eax != 0 {
                r.ecx = &mut lot as *mut u32 as u32;
                r.edx = inserted.as_mut_ptr() as u32;
                let result = r.edx; let value = r.ecx; r.ecx = this.wrapping_add(0xCC);
                invoke(r, Conv::Thiscall, crate::abi::target(0x0049_A050), &[result, value]);
            }
            get_next(r, iterator, &mut x, &mut z);
        }
    }
}

register_port! {
    /// Abandon occupied lots at least sixty days old whose building purpose and wealth match the
    /// developer. Sum the low sixteen capacity bits, visiting repeated lots again in iterator order.
    /// The iterator writes its z coordinate over the original profile stack argument.
    0x0070_9200, fc4p_00709200, decline, 1, 4, "cSC4TractDeveloper::Decline", |r| {
        let this = r.ecx;
        let iterator = decline_iterator() as u32;
        initialize(r, iterator, decline_initialized(), 0x00A7_D270);
        let profile = r.args[0]; let developer = word(profile, 0);
        let mut total = 0u32;
        r.eax = word(developer, 0); r.ecx = developer;
        invoke(r, Conv::Thiscall, word(r.eax, 0x34) as usize, &[]);
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
        bounds(r, iterator, word(profile, 4), word(profile, 8));
        let mut x = 0u32;
        let z = &mut r.args[0] as *mut u32;
        get_next(r, iterator, &mut x, z);
        while r.eax as u8 != 0 {
            r.eax = *z; r.ecx = word(this, 0x2C); r.edx = word(r.ecx, 0);
            let z_value = r.eax; r.eax = x;
            invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[r.eax, z_value, 0]);
            let lot = r.eax;
            if lot != 0 {
                r.edx = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 0x84) as usize, &[]);
                if r.eax == 3 {
                    r.eax = word(lot, 0); r.ecx = lot;
                    invoke(r, Conv::Thiscall, word(r.eax, 0xA0) as usize, &[]);
                    if r.eax >= 60 {
                        r.edx = word(lot, 0); r.ecx = lot;
                        invoke(r, Conv::Thiscall, word(r.edx, 0xB8) as usize, &[0]);
                        if r.eax != 0 {
                            r.ecx = word(this, 0x1C); r.edx = word(r.ecx, 0);
                            invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[r.eax]);
                            let building = r.eax;
                            r.eax = word(lot, 0); r.ecx = lot;
                            invoke(r, Conv::Thiscall, word(r.eax, 0xA4) as usize, &[]);
                            let wealth = r.eax;
                            r.edx = word(developer, 0); r.ecx = developer;
                            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
                            if word(building, 0) == r.eax {
                                r.eax = word(developer, 0); r.ecx = developer;
                                invoke(r, Conv::Thiscall, word(r.eax, 0x20) as usize, &[]);
                                if wealth == r.eax {
                                    r.edx = word(developer, 0); let table = word(lot, 0); r.ecx = developer;
                                    invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[]);
                                    r.ecx = lot;
                                    invoke(r, Conv::Thiscall, word(table, 0xC8) as usize, &[r.eax, 0]);
                                    r.ecx = word(this, 0x28); r.edx = word(r.ecx, 0);
                                    r.eax &= 0xFFFF;
                                    total = total.wrapping_add(r.eax);
                                    invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[lot, 6]);
                                }
                            }
                        }
                    }
                }
            }
            get_next(r, iterator, &mut x, z);
        }
        r.eax = total;
    }
}

pub const PORTS: &[Port] = &[fc4p_00707fb0::PORT, fc4p_00709200::PORT];

#[cfg(test)]
mod tests;
