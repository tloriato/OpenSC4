// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Replacing one lot with a row of smaller lots.

use super::*;

game_data! { static demolition: u32 = 0x00B4_3D00; }
game_fn! { #[allow(dead_code)] fn find_cache(this: u32, output: u32, key: u32) -> u32 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn unlink(node: u32, root: u32, first: u32, last: u32) -> u32 = "C" 0x0040_DAF0; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn pick_building(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_9BA0; }

layout! {
    /// The capacity constructed during this subdivision alone.
    struct SubdivisionState {
        0xC4 => built / set_built: i32,
    }
}

port! {
    /// Demolish and delete the parent, create every requested piece, and build below the capacity
    /// target. The first piece reuses the selected building; later pieces select again. A failed
    /// selection still queries its building profile. Each created lot is released before advancing.
    #[port(address = 0x0070_A9B0, owner = "tractdeveloper", name = "cSC4TractDeveloper::PerformSubdivision", naked)]
    pub unsafe extern "C" fn fc4p_0070a9b0() {
        // Facing values outside 0..3 retain the two dimension words below the entry stack pointer.
        // Save them before the Rust frame can reuse that storage, alongside the volatile registers.
        core::arch::naked_asm!(
            "push eax", "push ecx", "push edx",
            "mov eax, [esp - 0x10]", "push eax",
            "mov eax, [esp - 8]", "push eax",
            "lea eax, [esp + 8]", "push eax", "call {body}",
            "pop edx", "pop ecx", "pop eax", "ret 4",
            body = sym subdivide,
        );
    }
}

unsafe extern "stdcall" fn subdivide(entry: *mut Entry<1>, old_width: u32, old_depth: u32) {
    // SAFETY: the adapter supplies the saved registers, original argument, and dimension scratch words.
    unsafe {
        let r = &mut *entry;
        let this = r.ecx;
        let candidate = r.args[0];
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let state = SubdivisionState::from_ptr(this as *mut u8);
        r.edx = r.args.as_mut_ptr() as u32;
        let depth_output = r.edx;
        r.ecx = word(candidate, 4);
        r.eax = word(r.ecx, 0);
        let mut unused_width = 0u8;
        r.edx = &mut unused_width as *mut u8 as u32;
        invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[r.edx, depth_output]);
        let mut x = word(candidate, 8);
        let mut z = word(candidate, 0xC);
        let facing = word(candidate, 0x10);
        let w = u32::from(read_unaligned(candidate.wrapping_add(0x14) as *const u8));
        let d = u32::from(read_unaligned(candidate.wrapping_add(0x15) as *const u8));
        let (width, depth, dx, dz) = match facing {
            0 => {
                x = x.wrapping_add(1u32.wrapping_sub(w));
                (w, d, w.wrapping_neg(), 0)
            }
            1 => {
                x = x.wrapping_add(1u32.wrapping_sub(d));
                z = z.wrapping_add(1u32.wrapping_sub(w));
                (d, w, 0, w.wrapping_neg())
            }
            2 => {
                z = z.wrapping_add(1u32.wrapping_sub(d));
                (w, d, w, 0)
            }
            3 => (d, w, 0, w),
            _ => (old_width, old_depth, 0, 0),
        };
        r.eax = if facing <= 3 { depth } else { facing };
        let mut found = 0u32;
        r.edx = &mut found as *mut u32 as u32;
        let output = r.edx;
        let cache = this.wrapping_add(0x10C);
        r.ecx = cache;
        invoke(
            r,
            Conv::Thiscall,
            crate::abi::target(0x0049_7520),
            &[output, candidate.wrapping_add(4)],
        );
        r.ecx = found;
        r.eax = word(cache, 0);
        if r.ecx != r.eax {
            r.edx = r.eax.wrapping_add(0xC);
            let last = r.edx;
            r.edx = r.eax.wrapping_add(8);
            let first = r.edx;
            r.eax = r.eax.wrapping_add(4);
            invoke(
                r,
                Conv::Cdecl,
                crate::abi::target(0x0040_DAF0),
                &[r.ecx, r.eax, first, last],
            );
            if r.eax != 0 {
                invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]);
            }
            td.set_suitability_count(td.suitability_count().wrapping_sub(1));
        }
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0xC]);
        store(r.eax, 0, r.eax);
        store(r.eax, 4, r.eax);
        let lot = word(candidate, 4);
        let mut list = r.eax;
        if lot != 0 {
            r.eax = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 4) as usize, &[]);
        }
        r.ecx = list;
        let header = r.ecx;
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0xC]);
        let node = r.eax;
        r.eax = node.wrapping_add(8);
        if r.eax != 0 {
            store(r.eax, 0, lot);
            if lot != 0 {
                r.edx = word(lot, 0);
                r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.edx, 4) as usize, &[]);
            }
        }
        r.ecx = header;
        r.eax = word(r.ecx, 4);
        store(node, 0, r.ecx);
        store(node, 4, r.eax);
        store(r.eax, 0, node);
        store(r.ecx, 4, node);
        if lot != 0 {
            r.eax = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
        }
        r.ecx = read_unaligned(demolition());
        r.edx = word(r.ecx, 0);
        r.eax = &mut list as *mut u32 as u32;
        invoke(
            r,
            Conv::Thiscall,
            word(r.edx, 0x1C) as usize,
            &[1, r.eax, 0, 0x800, 0, 0, 0, 0],
        );
        r.ecx = word(candidate, 0x24);
        let built = (td.built() as u32).wrapping_sub(r.ecx);
        r.ecx = td.lot_manager();
        td.set_built(built as i32);
        r.eax = word(candidate, 4);
        r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x60) as usize, &[r.eax]);
        r.ecx = td.built() as u32;
        r.eax = word(candidate, 0x24);
        let target = (td.desired_maximum() as u32).wrapping_sub(r.ecx).wrapping_add(r.eax);
        let target = if target == 0 { 1 } else { target };
        state.set_built(0);
        let mut index = 0u32;
        while index < u32::from(read_unaligned(candidate.wrapping_add(0x6C) as *const u8)) {
            r.ecx = td.lot_manager();
            let mut created = 0u32;
            let output = &mut created as *mut u32 as u32;
            r.eax = x;
            r.edx = word(r.ecx, 0);
            invoke(
                r,
                Conv::Thiscall,
                word(r.edx, 0x58) as usize,
                &[x, z, width, depth, word(candidate, 0x10), output, 0, 0, 0],
            );
            r.eax = created;
            if r.eax != 0 {
                if td.built() < target as i32 {
                    let mut selected = true;
                    if index > 0 {
                        r.ecx = this;
                        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_9BA0), &[candidate]);
                        selected = r.eax == 27;
                    }
                    r.ecx = td.building_development();
                    r.eax = td.chosen_building();
                    r.edx = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[r.eax]);
                    let capacity = u32::from(read_unaligned(r.eax.wrapping_add(8) as *const u16));
                    if selected {
                        r.ecx = td.building_development();
                        r.edx = word(r.ecx, 0);
                        invoke(r, Conv::Thiscall, word(r.edx, 0xC8) as usize, &[capacity]);
                        if r.eax as u8 != 0 {
                            r.edx = td.chosen_building();
                            r.ecx = td.lot_developer();
                            r.eax = word(r.ecx, 0);
                            let building = r.edx;
                            r.edx = td.chosen_configuration();
                            let configuration = r.edx;
                            r.edx = created;
                            invoke(
                                r,
                                Conv::Thiscall,
                                word(r.eax, 0x18) as usize,
                                &[r.edx, configuration, building, 1],
                            );
                            r.ecx = td.built() as u32;
                            r.eax = state.built() as u32;
                            r.ecx = r.ecx.wrapping_add(capacity);
                            r.eax = r.eax.wrapping_add(capacity);
                            td.set_built(r.ecx as i32);
                            state.set_built(r.eax as i32);
                        }
                    }
                }
                r.ecx = created;
                r.eax = word(r.ecx, 0);
                invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
            }
            r.edx = x;
            r.ecx = dx;
            r.eax = z;
            r.edx = r.edx.wrapping_add(r.ecx);
            x = r.edx;
            r.edx = dz;
            r.eax = r.eax.wrapping_add(r.edx);
            z = r.eax;
            r.eax = u32::from(read_unaligned(candidate.wrapping_add(0x6C) as *const u8));
            index = index.wrapping_add(1);
        }
        super::growth::dispose_list(r, &list);
    }
}

pub const PORTS: &[Port] = &[fc4p_0070a9b0::PORT];

#[cfg(test)]
mod tests;
