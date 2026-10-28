// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration discovery and the candidate sort entry point.

use super::*;
use core::mem::MaybeUninit;

game_fn! { #[allow(dead_code)] fn introsort(first: u32, last: u32, unused: u32, depth: u32, compare: u32) -> u32 = "C" 0x0070_7F20; }
game_fn! { #[allow(dead_code)] fn finish_sort(first: u32, last: u32, compare: u32) -> u32 = "C" 0x0070_6EA0; }
game_fn! { #[allow(dead_code)] fn construct_set(this: u32, hint: u32) -> u32 = "thiscall" 0x0082_9BC0; }
game_fn! { #[allow(dead_code)] fn filter_entry(this: u32, key: u32) -> u32 = "thiscall" 0x0070_74D0; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn destroy_set(this: u32) -> u32 = "thiscall" 0x008E_BA60; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn begin(this: u32, iterator: u32) -> u32 = "thiscall" 0x004B_0390; }
game_fn! { #[allow(dead_code)] fn suitable(this: u32, configuration: u32) -> u32 = "thiscall" 0x0070_4C00; }
game_fn! { #[allow(dead_code)] fn insert(this: u32, result: u32, value: u32) -> u32 = "thiscall" 0x0049_A050; }
game_fn! { #[allow(dead_code)] fn next_bucket(this: u32) -> u32 = "thiscall" 0x0047_C1A0; }

register_port! {
    /// Sort a nonempty pointer range with twice its base-two logarithm as the partition depth,
    /// followed by the insertion pass. An empty range retains the incoming volatile registers.
    0x0070_9520, fc4p_00709520, sort, 3, 0, "SortCandidateLots", |r| {
        let [first, last, compare] = r.args;
        if first == last { return; }
        r.eax = (last.wrapping_sub(first) as i32 >> 2) as u32;
        r.ecx = 0;
        while r.eax != 1 { r.eax = (r.eax as i32 >> 1) as u32; r.ecx = r.ecx.wrapping_add(1); }
        r.eax = r.ecx.wrapping_mul(2);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7F20), &[first, last, 0, r.eax, compare]);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0070_6EA0), &[first, last, compare]);
    }
}

unsafe fn cleanup<const N: usize>(r: &mut Entry<N>, set: u32) {
    // SAFETY: set was constructed by the game's hash-set constructor.
    unsafe {
        r.ecx = set;
        invoke(r, Conv::Thiscall, crate::abi::target(0x008E_BA60), &[]);
        r.eax = word(set, 4);
        if r.eax != 0 { invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]); }
    }
}

register_port! {
    /// Cache and register a purpose/wealth filter, then query configurations in hash order.
    /// Count every suitability result, retain successful borrowed pointers, and release every result.
    /// Query failure returns five; a successful empty query returns zero. Temporary storage is freed.
    0x0070_9650, fc4p_00709650, configurations, 0, 0, "cSC4TractDeveloper::ListPossibleLotConfigurations", |r| {
        let this = r.ecx;
        let mut storage = MaybeUninit::<[u32; 5]>::uninit();
        let set = storage.as_mut_ptr() as u32;
        for offset in [4, 8, 12, 16] { store(set, offset, 0); }
        r.ecx = set;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0082_9BC0), &[100]);
        r.ecx = word(this, 0x80); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x20) as usize, &[]);
        let mut key = [r.eax, 0];
        r.ecx = word(this, 0x80); r.edx = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[]);
        key[1] = r.eax;
        r.eax = key.as_mut_ptr() as u32; r.ecx = this.wrapping_add(0x130);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_74D0), &[r.eax]);
        let filter_slot = r.eax;
        if word(filter_slot, 0) == 0 {
            invoke(r, Conv::Cdecl, crate::abi::target(0x005E_55E0), &[12]);
            let filter = r.eax;
            if filter != 0 {
                let developer = word(this, 0x80);
                store(filter, 0, 0x00AB_30DC);
                r.edx = word(developer, 0); r.ecx = developer;
                invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[]);
                store(filter, 4, r.eax);
                r.eax = word(developer, 0); r.ecx = developer;
                invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
                store(filter, 8, r.eax);
            }
            store(filter_slot, 0, filter);
            r.ecx = word(this, 0x24); r.edx = word(r.ecx, 0);
            invoke(r, Conv::Thiscall, word(r.edx, 0x48) as usize, &[filter]);
        }
        r.edx = word(filter_slot, 0); r.ecx = word(this, 0x24); r.eax = word(r.ecx, 0);
        let filter = r.edx; r.edx = set;
        invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[r.edx, filter]);
        r.ecx = set;
        if r.eax == 0 { cleanup(r, set); r.eax = 5; return; }
        let mut position = [0u32; 2];
        r.eax = position.as_mut_ptr() as u32;
        invoke(r, Conv::Thiscall, crate::abi::target(0x004B_0390), &[r.eax]);
        let mut node = position[0];
        r.ecx = position[1];
        let mut iterator = position;
        let mut best = 0i32;
        while node != 0 {
            let configuration = word(node, 4);
            r.ecx = this;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_4C00), &[configuration]);
            r.edx = word(this, 0x170); r.ecx = word(r.edx, 0x18);
            r.edx = word(r.ecx, r.eax.wrapping_mul(4));
            r.ecx = r.ecx.wrapping_add(r.eax.wrapping_mul(4)); r.edx = r.edx.wrapping_add(1);
            store(r.ecx, 0, r.edx);
            if r.eax as i32 > best { best = r.eax as i32; }
            if r.eax == 27 {
                let mut value_word = configuration;
                r.edx = &mut value_word as *mut u32 as u32;
                let value = r.edx;
                let mut inserted = [0u32; 2]; r.eax = inserted.as_mut_ptr() as u32;
                r.ecx = this.wrapping_add(0xD8);
                invoke(r, Conv::Thiscall, crate::abi::target(0x0049_A050), &[r.eax, value]);
            }
            r.edx = word(configuration, 0); r.ecx = configuration;
            invoke(r, Conv::Thiscall, word(r.edx, 8) as usize, &[]);
            node = word(node, 0);
            if node == 0 {
                r.ecx = iterator.as_mut_ptr() as u32;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0047_C1A0), &[]);
                node = r.eax;
            }
            iterator[0] = node;
        }
        cleanup(r, set);
        r.eax = best as u32;
    }
}

pub const PORTS: &[Port] = &[fc4p_00709520::PORT, fc4p_00709650::PORT];

#[cfg(test)]
mod tests;
