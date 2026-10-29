// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Recording growth results and releasing the persistence interface.

use super::*;
use core::mem::MaybeUninit;

game_fn! { #[allow(dead_code)] fn release(this: u32) -> u32 = "thiscall" 0x0070_48B0; }
game_fn! { #[allow(dead_code)] fn find_developer(this: u32, output: u32, key: u32) -> u32 = "thiscall" 0x006C_0030; }
game_fn! { #[allow(dead_code)] fn find_tract(this: u32, key: u32) -> u32 = "thiscall" 0x0069_5F70; }
game_fn! { #[allow(dead_code)] fn clear_attempt(this: u32, attempt: u32) -> u32 = "thiscall" 0x0070_AF20; }
game_fn! { #[allow(dead_code)] fn developer_entry(this: u32, key: u32, tract: u32, histogram: u32) -> u32 = "thiscall" 0x0070_CE60; }
game_fn! { #[allow(dead_code)] fn tract_entry(this: u32) -> u32 = "thiscall" 0x0070_C540; }
game_fn! { #[allow(dead_code)] fn copy_attempt(this: u32) -> u32 = "thiscall" 0x0070_BD30; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn allocate_candidate(bytes: u32) -> u32 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn construct_candidate(this: u32) -> u32 = "thiscall" 0x0070_7680; }
game_fn! { #[allow(dead_code)] fn assign_candidate(this: u32, source: u32) -> u32 = "thiscall" 0x0070_7240; }
game_fn! { #[allow(dead_code)] fn assign_set(this: u32, source: u32) -> u32 = "thiscall" 0x0070_5A10; }
game_fn! { #[allow(dead_code)] fn copy_set(this: u32, source: u32) -> u32 = "thiscall" 0x0070_5AD0; }
game_fn! { #[allow(dead_code)] fn copy_vector(this: u32, source: u32) -> u32 = "thiscall" 0x004B_03E0; }
game_fn! { #[allow(dead_code)] fn insert_vector(this: u32, end: u32, value: u32, allocator: u32, count: u32, unit: u32) -> u32 = "thiscall" 0x0051_CA60; }
game_fn! { #[allow(dead_code)] fn clear_set(this: u32, root: u32) -> u32 = "thiscall" 0x0070_49D0; }

layout! {
    /// Diagnostic containers, embedded in the primary tract-developer object.
    struct AttemptState {
        0x104 => selected_entry / set_selected_entry: u32,
        0x13C => histogram / set_histogram: u32,
        0x16C => builds / set_builds: u32,
        0x17C => developers / set_developers: u32,
    }
}

register_port! {
    /// Snapshot the configuration set, clone each candidate, and append the selected building and
    /// capacity limits to the build-history list. Temporary containers are released afterwards;
    /// cloned candidates remain owned by the copied vector in the new history node.
    0x0070_AD20, fc4p_0070ad20, record_build, 0, 0, "cSC4TractDeveloper::RecordBuildAttempt", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let state = AttemptState::from_ptr(this as *mut u8);
        r.ecx = td.building_development(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0xE4) as usize, &[]);
        if r.eax as u8 == 0 { return; }
        let mut local = MaybeUninit::<[u32; 13]>::uninit();
        let local = local.as_mut_ptr() as u32;
        store(local, 4, 0);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x14]);
        store(local, 4, r.eax); store(local, 8, 0);
        write_unaligned(r.eax as *mut u8, 0);
        r.ecx = word(local, 4); store(r.ecx, 4, 0);
        r.eax = word(local, 4); store(r.eax, 8, r.eax);
        r.eax = word(local, 4); store(r.eax, 0xC, r.eax);
        r.edx = (r.edx & !0xFF) | u32::from(td.relaxed());
        r.eax = this.wrapping_add(0xD8);
        r.ecx = local.wrapping_add(4);
        store(local, 0x10, 0); store(local, 0x14, 0); store(local, 0x18, 0);
        write_unaligned(local as *mut u8, r.edx as u8);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5A10), &[r.eax]);
        r.ecx = td.chosen_configuration(); r.edx = td.chosen_building();
        r.eax = state.selected_entry();
        let mut iterator = td.candidates_begin();
        store(local, 0x1C, r.ecx); store(local, 0x20, r.edx);
        r.ecx = word(r.eax, 0); r.edx = word(r.ecx, 4); r.eax = word(r.edx, 8);
        r.ecx = td.observed_land_value() as u32;
        r.edx = td.minimum_building_capacity() as u32;
        store(local, 0x30, r.eax);
        r.eax = td.maximum_building_capacity() as u32;
        store(local, 0x2C, r.eax); store(local, 0x24, r.ecx); store(local, 0x28, r.edx);
        while iterator != td.candidates_end() {
            let source = word(iterator, 0);
            invoke(r, Conv::Cdecl, crate::abi::target(0x005E_55E0), &[0xA4]);
            let candidate = if r.eax != 0 {
                r.ecx = r.eax;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7680), &[]);
                r.eax
            } else { 0 };
            let mut value = candidate;
            r.ecx = candidate;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7240), &[source]);
            r.eax = word(local, 0x14);
            if r.eax != word(local, 0x18) {
                if r.eax != 0 { store(r.eax, 0, candidate); r.eax = word(local, 0x14); }
                r.eax = r.eax.wrapping_add(4); store(local, 0x14, r.eax);
            } else {
                let mut allocator = MaybeUninit::<u8>::uninit();
                r.ecx = allocator.as_mut_ptr() as u32;
                let allocator = r.ecx;
                r.edx = &mut value as *mut u32 as u32;
                r.ecx = local.wrapping_add(0x10);
                invoke(r, Conv::Thiscall, crate::abi::target(0x0051_CA60), &[r.eax, r.edx, allocator, 1, 1]);
            }
            r.eax = td.candidates_end(); iterator = iterator.wrapping_add(4);
        }
        let history = state.builds();
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x3C]);
        let node = r.eax;
        let payload = node.wrapping_add(8);
        if payload != 0 {
            r.eax = (r.eax & !0xFF) | u32::from(read_unaligned(local as *const u8));
            r.ecx = local.wrapping_add(4);
            let source = r.ecx;
            r.ecx = payload.wrapping_add(4);
            write_unaligned(payload as *mut u8, r.eax as u8);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5AD0), &[source]);
            r.edx = local.wrapping_add(0x10);
            r.ecx = payload.wrapping_add(0x10);
            invoke(r, Conv::Thiscall, crate::abi::target(0x004B_03E0), &[r.edx]);
            r.eax = word(local, 0x1C); store(payload, 0x1C, r.eax);
            r.ecx = word(local, 0x20); store(payload, 0x20, r.ecx);
            r.edx = word(local, 0x24); store(payload, 0x24, r.edx);
            r.eax = word(local, 0x28); store(payload, 0x28, r.eax);
            r.ecx = word(local, 0x2C); store(payload, 0x2C, r.ecx);
            r.edx = word(local, 0x30); store(payload, 0x30, r.edx);
        }
        r.eax = word(history, 4);
        store(node, 0, history); store(node, 4, r.eax); store(r.eax, 0, node); store(history, 4, node);
        r.eax = word(local, 0x10);
        if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
        if word(local, 8) != 0 {
            r.eax = word(local, 4); r.ecx = word(r.eax, 4);
            let root = r.ecx; r.ecx = local.wrapping_add(4);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_49D0), &[root]);
            r.eax = word(local, 4); store(r.eax, 8, r.eax);
            r.edx = word(local, 4); store(r.edx, 4, 0);
            r.eax = word(local, 4); store(r.eax, 0xC, r.eax); store(local, 8, 0);
        }
        r.eax = word(local, 4);
        if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
    }
}

register_port! {
    /// Release the primary object from its persistence interface at primary +8.
    0x0070_C8C0, fc4p_0070c8c0, release_persistence, 0, 0, "ReleasePersistenceInterface", |r| {
        r.ecx = r.ecx.wrapping_sub(8);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_48B0), &[]);
    }
}

register_port! {
    /// When growth diagnostics are enabled, clear the previous result for this developer and tract,
    /// then copy this attempt into the positive-growth or nonpositive-growth half of the record.
    /// Query the developer type again after clearing; callbacks may change the working profile.
    0x0070_CFA0, fc4p_0070cfa0, record_growth, 0, 0, "cSC4TractDeveloper::RecordGrowthAttempt", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        let state = AttemptState::from_ptr(this as *mut u8);
        r.ecx = td.building_development(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0xE4) as usize, &[]);
        if r.eax as u8 == 0 { return; }
        r.eax = td.built() as u32;
        let positive = (r.eax as i32) > 0;
        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[]);
        let mut key = r.eax;
        let mut found = 0u32;
        r.eax = &mut key as *mut u32 as u32;
        r.ecx = &mut found as *mut u32 as u32;
        let output = r.ecx;
        let tree = this.wrapping_add(0x17C);
        r.ecx = tree;
        invoke(r, Conv::Thiscall, crate::abi::target(0x006C_0030), &[output, r.eax]);
        r.eax = found;
        if r.eax != state.developers() {
            r.edx = td.tract_x() as u32;
            let tracts = r.eax.wrapping_add(0x14);
            r.eax = td.tract_z() as u32;
            let mut coordinates = [r.edx, r.eax];
            r.ecx = coordinates.as_mut_ptr() as u32;
            let address = r.ecx;
            r.ecx = tracts;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0069_5F70), &[address]);
            if r.eax != word(tracts, 0) {
                r.eax = r.eax.wrapping_add(if positive { 0x18 } else { 0x58 });
                r.ecx = this;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_AF20), &[r.eax]);
            }
        }
        r.ecx = td.developer(); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x18) as usize, &[]);
        let mut key = r.eax;
        r.ecx = td.tract_z() as u32;
        r.eax = td.tract_x() as u32;
        let mut coordinates = [r.eax, r.ecx];
        r.edx = coordinates.as_mut_ptr() as u32;
        r.eax = &mut key as *mut u32 as u32;
        let address = r.eax;
        r.ecx = tree;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_CE60), &[address, r.edx, this.wrapping_add(0x13C)]);
        r.ecx = r.eax;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_C540), &[]);
        r.ecx = r.eax;
        if !positive { r.ecx = r.ecx.wrapping_add(0x40); }
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_BD30), &[]);
    }
}

pub const PORTS: &[Port] = &[fc4p_0070ad20::PORT, fc4p_0070c8c0::PORT, fc4p_0070cfa0::PORT];

#[cfg(test)]
mod tests;
