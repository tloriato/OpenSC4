// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Streets along rectangle edges reached by outside dead ends.

use super::{Port, ZoneDeveloper};
use core::{mem::MaybeUninit, ptr::read_unaligned};

game_data! { static network_flags: u32 = 0x00AB_3680; }
game_fn! { fn network(this: *mut u8, x: i32, z: i32, flags: u32) -> *mut u8 = "thiscall" 0x0072_FCA0; }
game_fn! {
    fn connection(this: *mut u8, x: i32, z: i32, flags: u32, side: i32, kind: u32) -> u32
        = "thiscall" 0x0072_FE80;
}
game_fn! {
    fn draw_street(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, per_cell: u32)
        = "thiscall" 0x0072_A2A0;
}

/// The street and dirt drafts are consulted in that order for each unconnected real edge.
unsafe fn parallel_edge(dev: ZoneDeveloper, x: i32, z: i32, flags: u32, edge: u32, kind: u32) -> bool {
    unsafe {
        let occupant = network(dev.as_ptr(), x, z, flags);
        if !occupant.is_null() && vcall!(occupant, 0xBC => fn(u32, u32) -> u32, edge, kind) as u8 != 0 {
            return true;
        }
        if flags & 0x808 != 0 && dev.real_mode() == 0 {
            let mut mask = MaybeUninit::<u32>::uninit();
            let street = vcall!(dev.street_tool(), 0x24 => fn(i32, i32, u32, *mut u32) -> u32,
                x, z, 8, mask.as_mut_ptr());
            if street as u8 != 0
                || vcall!(dev.dirt_tool(), 0x24 => fn(i32, i32, u32, *mut u32) -> u32,
                    x, z, 0x800, mask.as_mut_ptr()) as u8
                    != 0
            {
                return mask.assume_init() & 0xFFu32.wrapping_shl(edge * 8) != 0;
            }
        }
        false
    }
}

/// Scan one outside row or column, retaining the first dead end unless any parallel edge blocks it.
unsafe fn scan(dev: ZoneDeveloper, rect: *const i32, flags: u32, kind: u32, line: u32, fixed: i32) {
    unsafe {
        let horizontal = line < 2;
        let wanted = [0, 2, 3, 1][line as usize];
        let low = usize::from(!horizontal);
        let high = low + 2;
        let mut cursor = read_unaligned(rect.add(low));
        let mut dead = false;
        while cursor <= read_unaligned(rect.add(high)) {
            let (x, z) = if horizontal { (cursor, fixed) } else { (fixed, cursor) };
            if vcall!(dev.as_ptr(), 0x20 => fn(i32, i32, u32) -> u32, x, z, flags) as u8 != 0 {
                let mut count = 0;
                let mut side_found = -1;
                for side in 0..4 {
                    if connection(dev.as_ptr(), x, z, flags, side, kind) as u8 != 0 {
                        count += 1;
                        if count > 1 {
                            break;
                        }
                        side_found = side;
                    }
                }
                if count == 1 && side_found == wanted {
                    dead = true;
                }
                for (test_flags, test_kind) in [(0x40, 6), (1, 0), (0x400, 10), (8, 3), (0x800, 11)] {
                    if vcall!(dev.as_ptr(), 0x20 => fn(i32, i32, u32) -> u32, x, z, test_flags) as u8 != 0 {
                        let first_edge = u32::from(!horizontal);
                        if parallel_edge(dev, x, z, test_flags, first_edge, test_kind)
                            || parallel_edge(dev, x, z, test_flags, first_edge + 2, test_kind)
                        {
                            return;
                        }
                    }
                }
            }
            cursor = cursor.wrapping_add(1);
        }
        if dead {
            let x1 = read_unaligned(rect);
            let z1 = read_unaligned(rect.add(1));
            let x2 = read_unaligned(rect.add(2));
            let z2 = read_unaligned(rect.add(3));
            let (a, b, c, d) = match line {
                0 => (x1, z1, x2, z1),
                1 => (x1, z2, x2, z2),
                2 => (x1, z1, x1, z2),
                _ => (x2, z1, x2, z2),
            };
            draw_street(dev.as_ptr(), a, b, c, d, 1);
        }
    }
}

port! {
    /// Visit the north, south, west and east outside lines in that order. Draw an edge street when
    /// a single-ended network points toward the region and no parallel real or draft edge is found.
    /// Query the city's height before the south line and width before the east line, even for empty bounds.
    #[port(address = 0x0073_0D30, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::LayInJogStreets")]
    pub unsafe extern "thiscall" fn fc4p_00730d30(this: *mut u8, rect: *const i32, kind: i32) {
        // SAFETY: the game's developer, inclusive rectangle and network-kind table.
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let north = read_unaligned(rect.add(1));
            let flags = read_unaligned(network_flags().wrapping_offset(kind as isize));
            if north > 0 { scan(dev, rect, flags, kind as u32, 0, north.wrapping_sub(1)); }
            let south = read_unaligned(rect.add(3));
            let height = vcall!(dev.city(), 0x178 => fn() -> i32);
            if south < height.wrapping_sub(1) {
                scan(dev, rect, flags, kind as u32, 1, south.wrapping_add(1));
            }
            let west = read_unaligned(rect);
            if west > 0 { scan(dev, rect, flags, kind as u32, 2, west.wrapping_sub(1)); }
            let east = read_unaligned(rect.add(2));
            let width = vcall!(dev.city(), 0x174 => fn() -> i32);
            if east < width.wrapping_sub(1) {
                scan(dev, rect, flags, kind as u32, 3, east.wrapping_add(1));
            }
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00730d30::PORT];

#[cfg(test)]
mod tests;
