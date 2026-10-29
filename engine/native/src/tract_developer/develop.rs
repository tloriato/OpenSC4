// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The bounded sequence of strict and relaxed growth attempts.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Growth tuning, pass state and diagnostic snapshot fields.
    pub struct GrowthState {
        0x40 => overbuild / set_overbuild: f32,
        0xC4 => secondary_built / set_secondary_built: i32,
        0x104 => outcomes / set_outcomes: u32,
        0x108 => out_flag / set_out_flag: u8,
        0x140 => date / set_date: u32,
        0x144 => recorded_developer / set_recorded_developer: u32,
        0x148 => recorded_x / set_recorded_x: i32,
        0x14C => recorded_z / set_recorded_z: i32,
        0x150 => capacity_delta / set_capacity_delta: i32,
        0x154 => recorded_built / set_recorded_built: i32,
        0x158 => recorded_existing / set_recorded_existing: i32,
        0x15C => abandoned / set_abandoned: u32,
        0x160 => desirability / set_desirability: i32,
        0x164 => recorded_desired / set_recorded_desired: i32,
        0x168 => resources / set_resources: u32,
    }
}

game_data! { static calendar: u32 = 0x00B4_3D5C; }
game_data! { static building_development: u32 = 0x00B4_3D68; }
game_fn! { #[allow(dead_code)] fn date(this: u32, source: u32) -> u32 = "thiscall" 0x0091_26F3; }
game_fn! { #[allow(dead_code)] fn clear_builds(this: u32) -> u32 = "thiscall" 0x0070_75C0; }
game_fn! { #[allow(dead_code)] fn resize_histogram(this: u32, rows: u32, value: u32) -> u32 = "thiscall" 0x0046_2B20; }
game_fn! { #[allow(dead_code)] fn fill_row(this: u32, end: u32, count: u32, value: u32) -> u32 = "thiscall" 0x0042_3320; }
game_fn! { #[allow(dead_code)] fn clear_lots(this: u32, root: u32) -> u32 = "thiscall" 0x0070_4990; }
game_fn! { #[allow(dead_code)] fn clear_configurations(this: u32, root: u32) -> u32 = "thiscall" 0x0070_49D0; }
game_fn! { #[allow(dead_code)] fn clear_cache(this: u32, root: u32) -> u32 = "thiscall" 0x0070_4A10; }
game_fn! { #[allow(dead_code)] fn clear_candidates(vector: u32) -> u32 = "C" 0x0070_7370; }
game_fn! { #[allow(dead_code)] fn free(node: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn list_lots(this: u32) -> u32 = "thiscall" 0x0070_7FB0; }
game_fn! { #[allow(dead_code)] fn reoccupy(this: u32) -> u32 = "thiscall" 0x0070_A3D0; }
game_fn! { #[allow(dead_code)] fn list_configurations(this: u32) -> u32 = "thiscall" 0x0070_9650; }
game_fn! { #[allow(dead_code)] fn list_existing(this: u32) -> u32 = "thiscall" 0x0070_8110; }
game_fn! { #[allow(dead_code)] fn list_aggregation(this: u32) -> u32 = "thiscall" 0x0070_97E0; }
game_fn! { #[allow(dead_code)] fn list_subdivision(this: u32) -> u32 = "thiscall" 0x0070_8C00; }
game_fn! { #[allow(dead_code)] fn score(this: u32) -> u32 = "thiscall" 0x0070_5750; }
game_fn! { #[allow(dead_code)] fn compare(a: u32, b: u32) -> u32 = "C" 0x0070_4770; }
game_fn! { #[allow(dead_code)] fn sort(first: u32, last: u32, unused: u32, depth: u32, compare: u32) -> u32 = "C" 0x0070_7F20; }
game_fn! { #[allow(dead_code)] fn insertion_sort(first: u32, last: u32, compare: u32) -> u32 = "C" 0x0070_62D0; }
game_fn! { #[allow(dead_code)] fn insertion_tail(first: u32, last: u32, unused: u32, compare: u32) -> u32 = "C" 0x0070_4A90; }
game_fn! { #[allow(dead_code)] fn build(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_B440; }
game_fn! { #[allow(dead_code)] fn farm(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_B640; }
game_fn! { #[allow(dead_code)] fn record_build(this: u32) -> u32 = "thiscall" 0x0070_AD20; }
game_fn! { #[allow(dead_code)] fn record_growth(this: u32) -> u32 = "thiscall" 0x0070_CFA0; }

/// Take a virtual method's ST0 without rounding it, retaining its integer register outputs as well.
unsafe fn float_result(r: &mut Entry<3>, target: usize) -> F80 {
    let mut value = [0u8; 10];
    // SAFETY: this target returns one x87 value, which this block immediately pops.
    unsafe {
        invoke(r, Conv::Thiscall, target, &[]);
        core::arch::asm!("fstp tbyte ptr [{p}]", p = in(reg) value.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(value)
}

fn scaled_demand(value: i32, scale: f32) -> F80 {
    let input = F80::from_i32(value).to_bytes();
    let mut output = [0u8; 10];
    // SAFETY: one pushed x87 value; the stored scale remains a memory operand.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{input}]", "fmul dword ptr [{scale}]", "fstp tbyte ptr [{output}]",
            input = in(reg) input.as_ptr(), scale = in(reg) &scale,
            output = in(reg) output.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(output)
}

/// Dispose each right subtree through the game helper, then free the nodes on the left spine.
unsafe fn empty_tree(r: &mut Entry<3>, tree: u32, helper: u32, cache: bool) {
    // SAFETY: the enclosing pass supplies one of its three live tree headers.
    unsafe {
        if !cache {
            r.eax = word(tree, 0);
        }
        let mut node = word(word(tree, 0), 4);
        while node != 0 {
            if cache {
                r.edx = word(node, 0xC);
            } else {
                r.ecx = word(node, 0xC);
            }
            let right = if cache { r.edx } else { r.ecx };
            r.ecx = tree;
            invoke(r, Conv::Thiscall, crate::abi::target(helper), &[right]);
            let left = word(node, 8);
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[node]);
            node = left;
        }
        r.eax = word(tree, 0);
        store(r.eax, 8, r.eax);
        if cache {
            r.eax = word(tree, 0);
            store(r.eax, 4, 0);
        } else {
            r.edx = word(tree, 0);
            store(r.edx, 4, 0);
        }
        r.eax = word(tree, 0);
        store(r.eax, 0xC, r.eax);
        store(tree, 4, 0);
    }
}

unsafe fn append_outcome(r: &mut Entry<3>, header: u32, code: u32, load_edx: bool) {
    // SAFETY: the caller's circular outcome list uses the game's twelve-byte nodes.
    unsafe {
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[12]);
        r.ecx = r.eax.wrapping_add(8);
        if r.ecx != 0 {
            if load_edx {
                r.edx = code;
            }
            store(r.ecx, 0, code);
        }
        r.ecx = word(header, 4);
        if load_edx {
            r.edx = code;
        }
        store(r.eax, 4, r.ecx);
        store(r.eax, 0, header);
        store(r.ecx, 0, r.eax);
        store(header, 4, r.eax);
    }
}

register_port! {
    /// Initialize the tract's demand and diagnostic snapshot, clear all thirteen histogram rows,
    /// then try at most size squared plus two passes. The first pass is strict. Each build result
    /// is appended to the caller's existing list; a successful reoccupation returns immediately.
    /// Industrial success, enough capacity, and relaxed resource failures stop further attempts.
    /// The return is signed built capacity. The caller's profile slot doubles as the stop byte.
    0x0070_D130, fc4p_0070d130, grow, 3, 12, "cSC4TractDeveloper::Grow", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let state = GrowthState::from_ptr(this as *mut u8);
        r.eax = r.args[0];
        let z = word(r.eax, 8); let x = word(r.eax, 4); let developer = word(r.eax, 0);
        r.eax = word(developer, 0); r.ecx = developer;
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
        let purpose = r.eax;
        r.ecx = r.args[0];
        td.set_developer(developer); td.set_tract_x(x as i32); td.set_tract_z(z as i32);
        r.edx = u32::from(*(r.ecx.wrapping_add(0xC) as *const u8));
        r.ecx = this.wrapping_add(0x90); td.set_maximum_stage(r.edx as i32);
        r.eax = word(developer, 0);
        let profile = r.ecx; r.ecx = developer;
        invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[profile]);
        r.edx = word(developer, 0); r.ecx = developer;
        invoke(r, Conv::Thiscall, word(r.edx, 0x38) as usize, &[]);
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[x, z]);
        r.eax &= 0xFFFF; td.set_existing(r.eax as i32);
        r.edx = word(developer, 0); r.ecx = developer;
        invoke(r, Conv::Thiscall, word(r.edx, 0x48) as usize, &[]);
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        let demand = float_result(r, word(r.edx, 0x30) as usize).ftol();
        r.eax = demand as u32; r.edx = (demand as u64 >> 32) as u32;
        td.set_desired((r.eax as i32).max(1));
        let maximum = scaled_demand(td.desired(), state.overbuild()).ftol();
        r.eax = maximum as u32; r.edx = (maximum as u64 >> 32) as u32;
        td.set_desired_maximum(r.eax as i32);
        r.eax = 0; td.set_built(0); state.set_secondary_built(0); td.set_chosen_configuration(0);
        r.eax = r.args[1]; state.set_outcomes(r.eax); state.set_out_flag(0);
        r.ecx = read_unaligned(calendar()); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
        let source = r.eax;
        let mut local = MaybeUninit::<[u32; 3]>::uninit();
        let local = local.as_mut_ptr() as u32;
        r.ecx = local;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0091_26F3), &[source]);
        r.eax = word(local, 4); state.set_date(r.eax);
        state.set_recorded_developer(developer); state.set_recorded_x(x as i32); state.set_recorded_z(z as i32);
        r.edx = word(developer, 0); r.ecx = developer;
        invoke(r, Conv::Thiscall, word(r.edx, 0x3C) as usize, &[]);
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[x, z]);
        r.ecx = td.existing() as u32; r.eax = (r.eax as i16 as i32) as u32;
        state.set_recorded_existing(r.ecx as i32); state.set_capacity_delta(r.eax as i32); state.set_recorded_built(0);
        for (slot, unsigned) in [(0x40, true), (0x34, false)] {
            r.edx = word(developer, 0); r.ecx = developer;
            invoke(r, Conv::Thiscall, word(r.edx, slot) as usize, &[]);
            r.edx = word(r.eax, 0); r.ecx = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x58) as usize, &[x, z]);
            if unsigned { r.eax &= 0xFFFF; state.set_abandoned(r.eax); }
            else {
                r.ecx = td.desired() as u32; r.eax = (r.eax as i16 as i32) as u32;
                state.set_desirability(r.eax as i32); state.set_recorded_desired(r.ecx as i32);
            }
        }
        r.ecx = read_unaligned(building_development()); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0xC0) as usize, &[]);
        r.eax &= 0xFFFF; r.ecx = this.wrapping_add(0x16C); state.set_resources(r.eax);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_75C0), &[]);
        r.edx = local; r.ecx = this.wrapping_add(0x170);
        store(local, 0, 0); store(local, 4, 0); store(local, 8, 0);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0046_2B20), &[13, r.edx]);
        store(local, 0, 0);
        for row in (0..0x9C).step_by(12) {
            let vector = word(this, 0x170).wrapping_add(row);
            r.ecx = word(vector, 4); r.edx = word(vector, 0);
            r.eax = (r.ecx.wrapping_sub(r.edx) as i32 >> 2) as u32;
            if r.eax > 28 {
                r.eax = r.edx.wrapping_add(0x70); store(vector, 4, r.eax);
            } else {
                r.edx = local; let value = r.edx;
                r.edx = 28u32.wrapping_sub(r.eax);
                let end = r.ecx; r.ecx = vector;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0042_3320), &[end, r.edx, value]);
            }
            r.ecx = 0; r.eax = 0;
            while (r.eax as i32) < 0x70 {
                for offset in [0, 4, 8, 12] {
                    r.edx = word(this, 0x170); r.edx = word(r.edx, row);
                    store(r.edx, r.eax.wrapping_add(offset), r.ecx);
                }
                r.eax = r.eax.wrapping_add(16);
            }
        }
        r.eax = word(developer, 0); r.ecx = developer;
        let mut first = 1u8;
        r.args[0] &= !0xFF;
        invoke(r, Conv::Thiscall, word(r.eax, 0x34) as usize, &[]);
        r.edx = word(r.eax, 0); r.ecx = r.eax;
        invoke(r, Conv::Thiscall, word(r.edx, 0x28) as usize, &[]);
        r.ecx = r.eax.wrapping_mul(r.eax).wrapping_add(1);
        let mut passes = r.ecx;
        loop {
            r.eax = word(this, 0xD0);
            if r.eax != 0 { empty_tree(r, this.wrapping_add(0xCC), 0x0070_4990, false); }
            r.eax = word(this, 0xDC);
            if r.eax != 0 { empty_tree(r, this.wrapping_add(0xD8), 0x0070_49D0, false); }
            r.eax = this.wrapping_add(0xE4);
            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7370), &[r.eax]);
            r.ecx = (r.ecx & !0xFF) | u32::from(first);
            r.eax = (r.eax & !0xFF) | u32::from(first == 0); td.set_relaxed(r.eax as u8);
            r.eax = u32::MAX; td.set_observed_land_value(0); td.set_minimum_building_capacity(-1);
            td.set_maximum_building_capacity(-1); td.set_chosen_building(0);
            if word(this, 0x110) != 0 {
                r.ecx = word(this, 0x10C);
                empty_tree(r, this.wrapping_add(0x10C), 0x0070_4A10, true);
            }
            let mut best = 0i32;
            r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7FB0), &[]);
            r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_A3D0), &[]);
            if r.eax as i32 > best { best = r.eax as i32; }
            if r.eax == 27 {
                r.ecx = r.args[1]; let header = word(r.ecx, 0);
                append_outcome(r, header, 27, false);
                r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_AD20), &[]);
                r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_CFA0), &[]);
                r.edx = (r.edx & !0xFF) | u32::from(state.out_flag()); r.eax = r.args[2];
                write_unaligned(r.eax as *mut u8, r.edx as u8);
                r.eax = td.built() as u32;
                return;
            }
            r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_9650), &[]);
            if r.eax == 27 {
                for address in [0x0070_8110, 0x0070_97E0, 0x0070_8C00] {
                    if address == 0x0070_8C00 && purpose == 5 { break; }
                    r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(address), &[]);
                    if (r.eax as i32) > best { best = r.eax as i32; }
                }
                r.ecx = td.candidates_begin();
                if r.ecx == td.candidates_end() {
                    if best == 27 { best = 3; }
                    r.edx = r.args[1]; let header = word(r.edx, 0);
                    append_outcome(r, header, best as u32, true);
                    r.eax = td.debug_context(); r.ecx = word(r.eax, 0);
                    r.eax = r.ecx.wrapping_add(r.edx.wrapping_mul(4));
                    store(r.eax, 0, word(r.eax, 0).wrapping_add(1));
                    r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_AD20), &[]);
                } else {
                    r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5750), &[]);
                    r.eax = td.candidates_begin(); r.ecx = td.candidates_end();
                    if r.eax != r.ecx {
                        let begin = r.eax; let end = r.ecx;
                        let count = end.wrapping_sub(begin) as i32 >> 2;
                        r.eax = count as u32; r.ecx = 0;
                        while r.eax != 1 { r.eax = (r.eax as i32 >> 1) as u32; r.ecx = r.ecx.wrapping_add(1); }
                        r.ecx = r.ecx.wrapping_mul(2);
                        invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7F20), &[begin, end, 0, r.ecx, 0x0070_4770]);
                        if count > 16 {
                            let middle = begin.wrapping_add(0x40);
                            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_62D0), &[begin, middle, 0x0070_4770]);
                            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_4A90), &[middle, end, 0, 0x0070_4770]);
                        } else {
                            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_62D0), &[begin, end, 0x0070_4770]);
                        }
                    }
                    let mut iterator = td.candidates_begin();
                    while iterator != td.candidates_end() {
                        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
                        let candidate = word(iterator, 0);
                        invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
                        let address = if r.eax == 5 { 0x0070_B640 } else { 0x0070_B440 };
                        r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(address), &[candidate]);
                        best = r.eax as i32;
                        r.eax = r.args[1]; let header = word(r.eax, 0);
                        append_outcome(r, header, best as u32, false);
                        r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_AD20), &[]);
                        if best == 20 {
                            r.edx = (r.edx & !0xFF) | u32::from(td.relaxed());
                            r.args[0] = (r.args[0] & !0xFF) | u32::from(r.edx as u8);
                            break;
                        }
                        r.ecx = td.built() as u32;
                        if (r.ecx as i32) >= td.desired() { r.args[0] = (r.args[0] & !0xFF) | 1; break; }
                        if best == 27 {
                            r.ecx = td.developer(); r.eax = word(r.ecx, 0);
                            invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
                            if (5..=8).contains(&(r.eax as i32)) { r.args[0] = (r.args[0] & !0xFF) | 1; }
                            break;
                        }
                        r.eax = td.candidates_end(); iterator = iterator.wrapping_add(4);
                    }
                }
            }
            r.eax = (r.eax & !0xFF) | u32::from(first);
            if first == 0 && best != 27 { r.args[0] = (r.args[0] & !0xFF) | 1; }
            passes = passes.wrapping_sub(1); r.eax = passes; first = 0;
            if passes as i32 >= 0 {
                r.eax = (r.eax & !0xFF) | (r.args[0] & 0xFF);
                if r.eax as u8 == 0 { continue; }
            }
            r.ecx = this; invoke(r, Conv::Thiscall, crate::abi::target(0x0070_CFA0), &[]);
            r.ecx = (r.ecx & !0xFF) | u32::from(state.out_flag()); r.edx = r.args[2];
            write_unaligned(r.edx as *mut u8, r.ecx as u8);
            r.eax = td.built() as u32;
            return;
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0070d130::PORT];

#[cfg(test)]
mod tests;
