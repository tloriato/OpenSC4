// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate construction, recycling, and configuration-filter disposal.

use super::*;
use core::mem::MaybeUninit;

game_data! { static candidate_pool: [u32; 3] = 0x00B4_AFCC; }
game_data! { static candidate_template: [u8; 0xA4] = 0x00B4_AFF0; }
game_data! { static template_flags: u32 = 0x00B4_B094; }

// Register inputs and outputs are carried by invoke as well as these stack signatures.
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn allocate_candidate(bytes: u32) -> u32 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn move_bytes(to: u32, from: u32, bytes: u32) -> u32 = "C" 0x009E_EB70; }
game_fn! { #[allow(dead_code)] fn insert_pointer(vector: u32, at: u32, value: u32, temporary: u32, count: u32, flag: u32) -> u32 = "thiscall" 0x0051_CA60; }
game_fn! { #[allow(dead_code)] fn fill_vector(vector: u32, count: u32, value: u32) -> u32 = "thiscall" 0x006B_DF10; }
game_fn! { #[allow(dead_code)] fn construct_candidate(candidate: u32) -> u32 = "thiscall" 0x0070_7680; }
game_fn! { #[allow(dead_code)] fn assign_candidate(candidate: u32, source: u32) -> u32 = "thiscall" 0x0070_7240; }
game_fn! { #[allow(dead_code)] fn at_exit(callback: u32) -> u32 = "C" 0x009E_F059; }
game_fn! { #[allow(dead_code)] fn dispose_configurations(tree: u32, node: u32) -> u32 = "thiscall" 0x0070_49D0; }

register_port! {
    /// Return a candidate pointer to the shared pool. Spare capacity avoids any allocation.
    0x0070_7320, fc4p_00707320, recycle_candidate, 1, 0, "RecycleGrowthCandidate", |r| {
        let saved_ecx = r.ecx;
        let pool = candidate_pool() as u32;
        r.eax = word(pool, 4);
        if r.eax != word(pool, 8) {
            if r.eax != 0 {
                r.ecx = r.args[0];
                store(r.eax, 0, r.ecx);
                r.eax = word(pool, 4);
            }
            r.eax = r.eax.wrapping_add(4);
            store(pool, 4, r.eax);
        } else {
            let mut temporary = MaybeUninit::<u8>::uninit();
            r.edx = temporary.as_mut_ptr() as u32;
            r.ecx = r.args.as_ptr() as u32;
            let value = r.ecx;
            r.ecx = pool;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0051_CA60), &[r.eax, value, r.edx, 1, 1]);
        }
        r.ecx = saved_ecx;
    }
}

register_port! {
    /// Pop candidates in reverse order into the shared pool, doubling its storage when full.
    /// The source vector keeps its allocation and ends empty.
    0x0070_7370, fc4p_00707370, clear_candidates, 1, 0, "cSC4TractDeveloper::ClearCandidateList", |r| {
        let list = r.args[0];
        let pool = candidate_pool() as u32;
        r.ecx = list;
        r.eax = word(list, 0);
        r.edx = word(list, 4);
        if r.eax == r.edx { return; }
        loop {
            r.eax = word(r.ecx, 4);
            let candidate = word(r.eax.wrapping_sub(4), 0);
            r.eax = r.eax.wrapping_sub(4);
            store(r.ecx, 4, r.eax);
            r.eax = word(pool, 4);
            if r.eax != word(pool, 8) {
                if r.eax != 0 {
                    store(r.eax, 0, candidate);
                    r.eax = word(pool, 4);
                }
                r.eax = r.eax.wrapping_add(4);
                store(pool, 4, r.eax);
            } else {
                r.ecx = word(pool, 0);
                let mut old_end = r.eax;
                r.eax = (r.eax.wrapping_sub(r.ecx) as i32 >> 2) as u32;
                let mut one = 1u32;
                let mut count = r.eax;
                r.edx = if r.eax < 1 { &mut one } else { &mut count } as *mut u32 as u32;
                let new_count = word(r.edx, 0).wrapping_add(r.eax);
                let new_begin = if new_count != 0 {
                    r.ecx = new_count.wrapping_mul(4);
                    invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[r.ecx]);
                    r.ecx = word(pool, 0);
                    r.eax
                } else { 0 };
                if old_end == r.ecx {
                    r.eax = new_begin;
                } else {
                    old_end = old_end.wrapping_sub(r.ecx);
                    invoke(r, Conv::Cdecl, crate::abi::target(0x009E_EB70), &[new_begin, r.ecx, old_end]);
                    r.eax = r.eax.wrapping_add(old_end);
                }
                store(r.eax, 0, candidate);
                let new_end = r.eax.wrapping_add(4);
                r.eax = word(pool, 0);
                if r.eax != 0 {
                    invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]);
                }
                r.ecx = list;
                r.edx = new_begin.wrapping_add(new_count.wrapping_mul(4));
                store(pool, 0, new_begin);
                store(pool, 4, new_end);
                store(pool, 8, r.edx);
            }
            r.eax = word(r.ecx, 0);
            if r.eax == word(r.ecx, 4) { break; }
        }
    }
}

register_port! {
    /// Dispose cached filters, their configuration trees and list nodes; retain and reset the list header.
    0x0070_75C0, fc4p_007075c0, clear_filters, 0, 0, "ClearConfigurationFilterList", |r| {
        let this = r.ecx;
        r.eax = word(this, 0);
        let mut next = word(r.eax, 0);
        while next != r.eax {
            let node = next;
            r.eax = word(node, 0x18);
            next = word(next, 0);
            if r.eax != 0 {
                invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[r.eax]);
            }
            r.eax = word(node, 0x10);
            let tree = node.wrapping_add(0xC);
            if r.eax != 0 {
                r.eax = word(tree, 0);
                let mut child = word(r.eax, 4);
                while child != 0 {
                    r.ecx = word(child, 0xC);
                    let right = r.ecx;
                    r.ecx = tree;
                    invoke(r, Conv::Thiscall, crate::abi::target(0x0070_49D0), &[right]);
                    let left = word(child, 8);
                    invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[child]);
                    child = left;
                }
                r.eax = word(tree, 0);
                store(r.eax, 8, r.eax);
                r.edx = word(tree, 0);
                r.ecx = 0;
                store(r.edx, 4, r.ecx);
                r.eax = word(tree, 0);
                store(r.eax, 0xC, r.eax);
                store(tree, 4, r.ecx);
            }
            let header = word(tree, 0);
            if header != 0 {
                invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[header]);
            }
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[node]);
            r.eax = word(this, 0);
        }
        r.eax = word(this, 0);
        store(r.eax, 0, r.eax);
        r.eax = word(this, 0);
        store(r.eax, 4, r.eax);
    }
}

register_port! {
    /// Construct a candidate with its own empty lot tree and four zero-filled network vectors.
    /// Retain padding and the scalar words that are first assigned by later growth stages.
    0x0070_7680, fc4p_00707680, new_candidate, 0, 0, "ConstructGrowthCandidate", |r| {
        let this = r.ecx;
        store(this, 0, 0); store(this, 4, 0);
        for offset in [8, 0xC, 0x10] { store(this, offset, u32::MAX); }
        *(this.wrapping_add(0x14) as *mut u8) = 0;
        *(this.wrapping_add(0x15) as *mut u8) = 0;
        store(this, 0x18, 0);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x14]);
        store(this, 0x18, r.eax);
        store(this, 0x1C, 0);
        *(r.eax as *mut u8) = 0;
        r.eax = word(this, 0x18); store(r.eax, 4, 0);
        r.eax = word(this, 0x18); store(r.eax, 8, r.eax);
        r.eax = word(this, 0x18); store(r.eax, 0xC, r.eax);
        store(this, 0x24, u32::MAX); store(this, 0x28, u32::MAX);
        store(this, 0x34, 0xBF80_0000);
        for offset in [0x38, 0x3C, 0x40, 0x44] { store(this, offset, u32::MAX); }
        *(this.wrapping_add(0x6C) as *mut u8) = 0;
        for offset in (0x70..=0x9C).step_by(4) { store(this, offset, 0); }
        *(this.wrapping_add(0xA0) as *mut u8) = 0;
        let mut zero = 0u32;
        r.edx = &mut zero as *mut u32 as u32;
        r.ecx = this.wrapping_add(0x70);
        invoke(r, Conv::Thiscall, crate::abi::target(0x006B_DF10), &[4, r.edx]);
        for offset in [0x7C, 0x88] {
            zero = 0;
            let value = &mut zero as *mut u32 as u32;
            if offset == 0x7C { r.eax = value; }
            else { r.ecx = value; }
            r.ecx = this.wrapping_add(offset);
            invoke(r, Conv::Thiscall, crate::abi::target(0x006B_DF10), &[4, value]);
        }
        zero = 0;
        r.edx = &mut zero as *mut u32 as u32;
        r.ecx = this.wrapping_add(0x94);
        invoke(r, Conv::Thiscall, crate::abi::target(0x006B_DF10), &[4, r.edx]);
        r.eax = this;
        r.ecx = this;
    }
}

register_port! {
    /// Initialize the shared template once, refill an empty pool with ten candidates, then assign
    /// the template to the last available record and return it.
    0x0070_7D90, fc4p_00707d90, acquire_candidate, 0, 0, "cSC4TractDeveloper::NewCandidateLot", |r| {
        let flags = template_flags();
        let pool = candidate_pool() as u32;
        let template = candidate_template() as u32;
        r.eax = (r.eax & !0xFF) | u32::from(*(flags.cast::<u8>()));
        if r.eax & 1 == 0 {
            r.edx = read_unaligned(flags) | 1;
            r.ecx = template;
            write_unaligned(flags, r.edx);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7680), &[]);
            invoke(r, Conv::Cdecl, crate::abi::target(0x009E_F059), &[0x00A7_D260]);
        }
        r.ecx = word(pool, 4);
        if word(pool, 0) == r.ecx {
            for _ in 0..10 {
                invoke(r, Conv::Cdecl, crate::abi::target(0x005E_55E0), &[0xA4]);
                if r.eax != 0 {
                    r.ecx = r.eax;
                    invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7680), &[]);
                } else { r.eax = 0; }
                r.ecx = word(pool, 4);
                let mut value = r.eax;
                if r.ecx != word(pool, 8) {
                    if r.ecx != 0 {
                        store(r.ecx, 0, r.eax);
                        r.ecx = word(pool, 4);
                    }
                    r.ecx = r.ecx.wrapping_add(4);
                    store(pool, 4, r.ecx);
                } else {
                    let mut temporary = MaybeUninit::<u8>::uninit();
                    r.eax = temporary.as_mut_ptr() as u32;
                    r.edx = &mut value as *mut u32 as u32;
                    let position = r.ecx;
                    r.ecx = pool;
                    invoke(r, Conv::Thiscall, crate::abi::target(0x0051_CA60), &[position, r.edx, r.eax, 1, 1]);
                    r.ecx = word(pool, 4);
                }
            }
        }
        let candidate = word(r.ecx.wrapping_sub(4), 0);
        r.eax = r.ecx.wrapping_sub(4);
        r.ecx = candidate;
        store(pool, 4, r.eax);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_7240), &[template]);
        r.eax = candidate;
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00707320::PORT,
    fc4p_00707370::PORT,
    fc4p_007075c0::PORT,
    fc4p_00707680::PORT,
    fc4p_00707d90::PORT,
];

#[cfg(test)]
mod tests;
