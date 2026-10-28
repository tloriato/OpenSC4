// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Demolishing aggregated sites and creating their replacement lot.

use super::*;

game_data! { static demolition: u32 = 0x00B4_3D00; }
game_data! { static terrain: u32 = 0x00B4_3CF4; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn unlink_tree(node: u32, root: u32, first: u32, last: u32) -> u32 = "C" 0x0040_DAF0; }

/// Release every list entry and free its node, then reset and free the list header.
pub(super) unsafe fn dispose_list<const N: usize>(r: &mut Entry<N>, list: &u32) {
    // SAFETY: the enclosing port owns this circular list of retained lot pointers.
    unsafe {
        r.eax = *list;
        let mut node = word(r.eax, 0);
        while node != r.eax {
            r.ecx = word(node, 8);
            let next = word(node, 0);
            if r.ecx != 0 {
                r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 8) as usize, &[]);
            }
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[node]);
            r.eax = *list;
            node = next;
        }
        store(r.eax, 0, r.eax);
        r.eax = *list;
        store(r.eax, 4, r.eax);
        r.eax = *list;
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]);
    }
}

register_port! {
    /// Retain and demolish each constituent lot, remove its suitability-cache entry, and notify the
    /// lot developer. Create the replacement rectangle after subtracting the occupied capacity.
    /// Industrial square configurations request automatic facing. Terrain adjustment runs last.
    0x0070_68E0, fc4p_007068e0, aggregate, 2, 8, "cSC4TractDeveloper::PerformAggregation", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let candidate = r.args[0];
        let output = r.args[1];
        r.eax = word(candidate, 0x18);
        r.ecx = word(r.eax, 8);
        let mut iterator = r.ecx;
        if r.ecx != r.eax {
            loop {
                r.ecx = iterator;
                let lot = word(r.ecx, 0x10);
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 4) as usize, &[]);
                r.edx = td.suitability_header(); r.eax = word(r.edx, 4); r.ecx = r.edx;
                while r.eax != 0 {
                    if word(r.eax, 0x10) >= lot { r.ecx = r.eax; r.eax = word(r.eax, 8); }
                    else { r.eax = word(r.eax, 0xC); }
                }
                if r.ecx == r.edx || lot < word(r.ecx, 0x10) { r.ecx = r.edx; }
                r.eax = td.suitability_header();
                if r.ecx != r.eax {
                    r.edx = r.eax.wrapping_add(0xC);
                    let last = r.edx; r.edx = r.eax.wrapping_add(8);
                    let first = r.edx; r.eax = r.eax.wrapping_add(4);
                    invoke(r, Conv::Cdecl, crate::abi::target(0x0040_DAF0), &[r.ecx, r.eax, first, last]);
                    if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
                    td.set_suitability_count(td.suitability_count().wrapping_sub(1));
                }
                invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0xC]);
                store(r.eax, 0, r.eax); store(r.eax, 4, r.eax); let mut list = r.eax;
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 4) as usize, &[]);
                let header = list;
                invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0xC]);
                let node = r.eax; r.eax = node.wrapping_add(8);
                if r.eax != 0 {
                    store(r.eax, 0, lot); r.edx = word(lot, 0); r.ecx = lot;
                    invoke(r, Conv::Thiscall, word(r.edx, 4) as usize, &[]);
                }
                r.eax = word(header, 4);
                store(node, 0, header); store(node, 4, r.eax); store(r.eax, 0, node); store(header, 4, node);
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
                r.ecx = read_unaligned(demolition()); r.edx = word(r.ecx, 0);
                r.eax = &mut list as *mut u32 as u32;
                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[1, r.eax, 0, 0x800, 0, 0, 0, 0]);
                r.ecx = word(this, 0x2C); r.edx = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.edx, 0x60) as usize, &[lot]);
                r.eax = word(lot, 0); r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
                dispose_list(r, &list);
                r.eax = iterator; r.ecx = word(r.eax, 0xC);
                if r.ecx != 0 {
                    r.eax = r.ecx; r.ecx = word(r.eax, 8);
                    while r.ecx != 0 { r.eax = r.ecx; r.ecx = word(r.eax, 8); }
                } else {
                    r.ecx = word(r.eax, 4);
                    while r.eax == word(r.ecx, 0xC) { r.eax = r.ecx; r.ecx = word(r.ecx, 4); }
                    if word(r.eax, 0xC) != r.ecx { r.eax = r.ecx; }
                }
                r.ecx = candidate;
                iterator = r.eax;
                if r.eax == word(r.ecx, 0x18) { break; }
            }
        }
        r.edx = word(candidate, 0x24);
        td.set_built(td.built().wrapping_sub(r.edx as i32));
        store(output, 0, 0);
        r.ecx = td.developer(); r.eax = word(r.ecx, 0);
        let mut facing = word(candidate, 0x10);
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
        if (5..=8).contains(&(r.eax as i32)) {
            r.ecx = td.chosen_configuration(); r.edx = word(r.ecx, 0);
            let mut size = [0u32; 2];
            r.eax = size.as_mut_ptr().wrapping_add(1) as u32;
            let depth = r.eax; r.eax = size.as_mut_ptr() as u32;
            invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[r.eax, depth]);
            r.ecx = size[0]; if r.ecx == size[1] { facing = u32::MAX; }
        }
        r.ecx = word(this, 0x2C); r.eax = word(candidate, 0x44); r.edx = word(r.ecx, 0);
        r.eax = r.eax.wrapping_sub(word(candidate, 0x3C)).wrapping_add(1);
        let depth = r.eax;
        r.eax = word(candidate, 0x40).wrapping_sub(word(candidate, 0x38)).wrapping_add(1);
        let width = r.eax; r.eax = word(candidate, 0x38);
        invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize,
            &[r.eax, word(candidate, 0x3C), width, depth, facing, output, 0, 0, 0]);
        r.ecx = read_unaligned(terrain());
        if r.ecx != 0 {
            r.eax = word(candidate, 0x40); r.edx = word(candidate, 0x38);
            let mut rectangle = [r.edx, word(candidate, 0x3C), r.eax, word(candidate, 0x44)];
            r.eax = rectangle.as_mut_ptr() as u32; r.edx = word(candidate, 0x3C);
            r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x110) as usize, &[0, 0, r.eax, 0]);
        }
        r.ecx = word(output, 0); r.eax = u32::from(r.ecx != 0);
    }
}

pub const PORTS: &[Port] = &[fc4p_007068e0::PORT];

#[cfg(test)]
mod tests;
