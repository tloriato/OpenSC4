// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Counting the network tiles bordering a candidate's rectangle.

use super::*;

game_data! { static traffic: u32 = 0x00B4_3CEC; }
game_fn! { #[allow(dead_code)] fn bounding_rectangle(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_4630; }

unsafe fn count_tile<const N: usize>(r: &mut Entry<N>, grid: u32, candidate: u32, side: u32, x: u32, z: u32) {
    // SAFETY: the enclosing port supplies the grid and candidate network vectors.
    unsafe {
        r.edx = word(grid, 0);
        r.ecx = grid;
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[x, z, 0xC4B, 1]);
        let tile = r.eax;
        if tile == 0 {
            return;
        }
        r.eax = word(tile, 0);
        r.ecx = tile;
        invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[3]);
        if r.eax as u8 == 0 {
            r.edx = word(tile, 0);
            r.ecx = tile;
            invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[11]);
        }
        if r.eax as u8 != 0 {
            r.eax = word(candidate, 0x70);
            store(r.eax, side * 4, word(r.eax, side * 4).wrapping_add(1));
        }
        r.eax = word(tile, 0);
        r.ecx = tile;
        invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[0]);
        if r.eax as u8 == 0 {
            r.edx = word(tile, 0);
            r.ecx = tile;
            invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[10]);
        }
        if r.eax as u8 != 0 {
            r.eax = word(candidate, 0x7C);
            store(r.eax, side * 4, word(r.eax, side * 4).wrapping_add(1));
        }
        r.eax = word(tile, 0);
        r.ecx = tile;
        invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[6]);
        if r.eax as u8 != 0 {
            r.eax = word(candidate, 0x88);
            store(r.eax, side * 4, word(r.eax, side * 4).wrapping_add(1));
        }
        r.edx = word(tile, 0);
        r.ecx = tile;
        invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[1]);
        if r.eax as u8 != 0 {
            r.eax = word(candidate, 0x94);
            store(r.eax, side * 4, word(r.eax, side * 4).wrapping_add(1));
        }
    }
}

register_port! {
    /// Count north, east, south and west tiles in that order, excluding corners and city edges.
    /// Subdivisions scan the original lot's bounds. Counts accumulate without clearing the vectors.
    0x0070_5040, fc4p_00705040, find_networks, 1, 4, "cSC4TractDeveloper::FindNetworks", |r| {
        let this = r.ecx;
        let candidate = r.args[0];
        r.ecx = read_unaligned(traffic());
        r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0xA0) as usize, &[]);
        let grid = r.eax;
        r.ecx = this;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_4630), &[candidate]);
        let mut rectangle = [0u32; 4];
        if word(candidate, 0) == 4 {
            r.ecx = word(candidate, 4); r.edx = word(r.ecx, 0);
            r.eax = rectangle.as_mut_ptr() as u32;
            invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[r.eax]);
            r.eax = rectangle[0];
        } else {
            r.ecx = candidate.wrapping_add(0x38);
            r.eax = word(r.ecx, 0); r.edx = word(r.ecx, 4); rectangle[1] = r.edx;
            r.edx = word(r.ecx, 8); r.ecx = word(r.ecx, 0xC);
            rectangle[0] = r.eax; rectangle[2] = r.edx; rectangle[3] = r.ecx;
        }
        let [min_x, min_z, max_x, max_z] = rectangle;
        r.ecx = min_z;
        if (r.ecx as i32) > 0 && (r.eax as i32) <= max_x as i32 {
            let mut x = r.eax;
            loop {
                r.eax = min_z.wrapping_sub(1);
                count_tile(r, grid, candidate, 0, x, r.eax);
                r.eax = max_x;
                x = x.wrapping_add(1);
                if (x as i32) > r.eax as i32 { break; }
            }
        }
        r.ecx = word(this, 0x18); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x174) as usize, &[]);
        r.ecx = max_x; r.eax = r.eax.wrapping_sub(1);
        if (r.ecx as i32) < r.eax as i32 {
            r.eax = min_z;
            if (r.eax as i32) <= max_z as i32 {
                let mut z = r.eax;
                loop {
                    r.eax = max_x.wrapping_add(1);
                    count_tile(r, grid, candidate, 1, r.eax, z);
                    r.eax = max_z;
                    z = z.wrapping_add(1);
                    if (z as i32) > r.eax as i32 { break; }
                }
            }
        }
        r.ecx = word(this, 0x18); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x178) as usize, &[]);
        r.ecx = max_z; r.eax = r.eax.wrapping_sub(1);
        if (r.ecx as i32) < r.eax as i32 && (min_x as i32) <= max_x as i32 {
            let mut x = min_x;
            loop {
                r.eax = max_z.wrapping_add(1);
                count_tile(r, grid, candidate, 2, x, r.eax);
                r.eax = max_x;
                x = x.wrapping_add(1);
                if (x as i32) > r.eax as i32 { break; }
            }
        }
        r.eax = min_x;
        if (r.eax as i32) > 0 {
            r.eax = min_z;
            if (r.eax as i32) <= max_z as i32 {
                let mut z = r.eax;
                loop {
                    r.eax = min_x.wrapping_sub(1);
                    count_tile(r, grid, candidate, 3, r.eax, z);
                    r.eax = max_z;
                    z = z.wrapping_add(1);
                    if (z as i32) > r.eax as i32 { break; }
                }
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00705040::PORT];

#[cfg(test)]
mod tests;
