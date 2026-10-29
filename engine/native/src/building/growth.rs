// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Daily demand-weighted growth, tried-tract cycling and adaptive pacing.

use super::{
    io::successor,
    trees::{Node, Tree},
    BuildingSim,
};
use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { fn budget_new(this: *mut u8, category: u32) = "thiscall" 0x0068_BF50; }
game_return_fn! { fn grow_budget_expired(this: *mut u8) -> u32 = 0x0068_BF80, fc4r_0069b149; }
game_fn! { fn copy_developers(this: *mut u8, source: *mut u8) = "thiscall" 0x0069_89D0; }
game_fn! { fn random(this: *mut u8, limit: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { fn find_candidate(begin: *mut u8, end: *mut u8, predicate: u32, cookie: *mut u8) -> *mut u8 = "C" 0x0069_5E50; }
game_fn! { #[allow(dead_code)] fn candidate_not_tried(candidate: *mut u32) -> u64 = "C" 0x0069_B070; }
game_fn! { fn tried_tree(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x0069_AB40; }
game_fn! { fn mark_tried(this: *mut u8, out: *mut u8, key: *mut u32) = "thiscall" 0x0069_6300; }
game_fn! { fn delete_tried_subtree(this: *mut u8, node: *mut u8) = "thiscall" 0x0069_5E10; }
game_fn! { fn delete_developer_subtree(this: *mut u8, node: *mut u8) = "thiscall" 0x0069_5CD0; }
game_fn! { fn erase_developer_range(this: *mut u8, first: *mut u8, last: *mut u8) = "thiscall" 0x0069_6240; }
game_fn! { fn day_entry(this: *mut u8, key: *mut u32) -> *mut u32 = "thiscall" 0x0042_9760; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer: *mut u8) = "C" 0x0090_CF63; }
game_data! { static pace: u32 = 0x00B4_8608; }
game_data! { static adjustment: i32 = 0x00B4_860C; }
game_data! { static initialized: u32 = 0x00B4_8610; }
game_data! { static rng: u8 = 0x00B4_85F0; }
game_data! { static active_developer: u32 = 0x00B4_8544; }
game_data! { static tried: u8 = 0x00B4_85E0; }
game_data! { static simulator: *mut u8 = 0x00B4_3D5C; }

unsafe fn demand(developer: *mut u8) -> i32 {
    // SAFETY: the developer's demand node returns its value in ST0.
    unsafe {
        let node = vcall!(developer, 0x48 => fn() -> *mut u8);
        vcall_st0!(node, 0x30).ftol() as i32
    }
}

unsafe fn clear_tree(tree: *mut u8, nested: bool) {
    // SAFETY: the caller supplies an owned temporary developer tree or a game's tried-tract tree.
    unsafe {
        let t = Tree::from_ptr(tree);
        if t.count() == 0 {
            return;
        }
        let mut node = Node::from_ptr(t.header()).parent();
        while !node.is_null() {
            let right = Node::from_ptr(node).right();
            if nested {
                delete_tried_subtree(tree, right);
            } else {
                delete_developer_subtree(tree, right);
            }
            let left = Node::from_ptr(node).left();
            free(node);
            node = left;
        }
        let h = Node::from_ptr(t.header());
        h.set_left(t.header());
        h.set_parent(core::ptr::null_mut());
        h.set_right(t.header());
        t.set_count(0);
    }
}

unsafe fn remove_developer(tree: *mut u8, key: u32) {
    // SAFETY: the copied developer tree uses unsigned type keys and its original range-erase helper.
    unsafe {
        let header = Tree::from_ptr(tree).header();
        let root = Node::from_ptr(header).parent();
        let mut current = root;
        let mut upper = header;
        while !current.is_null() {
            let n = Node::from_ptr(current);
            if key < n.key() {
                upper = current;
                current = n.left();
            } else {
                current = n.right();
            }
        }
        let mut current = root;
        let mut lower = header;
        while !current.is_null() {
            let n = Node::from_ptr(current);
            if n.key() >= key {
                lower = current;
                current = n.left();
            } else {
                current = n.right();
            }
        }
        erase_developer_range(tree, lower, upper);
    }
}

unsafe fn attempt(sim: BuildingSim, developer: *mut u8, key: &mut u32, padding: u32) {
    // SAFETY: the selected developer's candidate vector, tried tree, tract developer and temporary list.
    unsafe {
        if vcall!(developer, 0x28 => fn() -> u32) as u8 == 0 {
            return;
        }
        let vector = vcall!(developer, 0x2C => fn() -> *mut u8);
        write(active_developer(), *key);
        let mut flag = MaybeUninit::<u8>::uninit();
        let mut candidate = find_candidate(
            read(vector.cast()),
            read(vector.add(4).cast()),
            0x0069_B070,
            flag.as_mut_ptr(),
        );
        if candidate == read(vector.add(4).cast::<*mut u8>()) {
            clear_tree(tried_tree(tried(), key), true);
            candidate = read(vector.cast());
            if candidate == read(vector.add(4).cast::<*mut u8>()) {
                return;
            }
        }
        let mut pair = [read(candidate.cast::<u32>()), read(candidate.add(4).cast::<u32>())];
        let tree = tried_tree(tried(), key);
        let mut result = MaybeUninit::<[u8; 8]>::uninit();
        mark_tried(tree, result.as_mut_ptr().cast(), pair.as_mut_ptr());
        let mut profile = [
            developer as u32,
            read(candidate.cast()),
            read(candidate.add(4).cast()),
            (padding & !255) | u32::from(*candidate.add(8)),
        ];
        let mut list = allocate(12);
        write(list.cast::<*mut u8>(), list);
        write(list.add(4).cast::<*mut u8>(), list);
        flag.as_mut_ptr().write(0);
        let built = vcall!(sim.tract_developer(), 0x14 => fn(*mut u8, *mut *mut u8, *mut u8) -> i32,
            profile.as_mut_ptr().cast(), &mut list, flag.as_mut_ptr());
        if built > 0 {
            let date = vcall!(read(simulator()), 0x1C => fn() -> *mut u8);
            let day = vcall!(date, 0x68 => fn() -> u32);
            write(day_entry(sim.as_ptr().add(0xB0), key), day);
        }
        let mut node = read(list.cast::<*mut u8>());
        while node != list {
            let old = node;
            node = read(old.cast());
            free(old);
        }
        write(list.cast::<*mut u8>(), list);
        write(list.add(4).cast::<*mut u8>(), list);
        free(list);
    }
}

port! {
    /// Gives each selected developer at most one growth attempt, yielding at the original budget checks.
    /// The entry retains the padding beside the candidate's stage byte in the aligned original frame.
    #[port(address = 0x0069_B0B0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::Grow", naked)]
    pub unsafe extern "C" fn fc4p_0069b0b0() {
        core::arch::naked_asm!("lea eax, [esp - 4]", "and eax, -8", "push dword ptr [eax - 28]", "call {body}", "ret", body = sym grow);
    }
}

/// # Safety
/// The original simulator and retained stage-word padding from its aligned stack frame.
unsafe extern "fastcall" fn grow(this: *mut u8, _entry_edx: u32, padding: u32) -> u32 {
    // SAFETY: game globals retain the process pacing state; all allocations, tree and budget operations use game helpers.
    unsafe {
        if read(initialized()) as u8 & 1 == 0 {
            write(initialized(), read(initialized()) | 1);
            write(pace(), 1024);
            write(adjustment(), 0);
        }
        let sim = BuildingSim::from_ptr(this);
        let mut budget = MaybeUninit::<[u64; 3]>::uninit();
        let budget = budget.as_mut_ptr().cast::<u8>();
        budget_new(budget, 3);
        write(budget.add(12).cast::<u32>(), read(pace()));
        *budget.add(16) = 0;
        write(budget.add(20).cast::<*mut u32>(), pace());
        let mut copied = MaybeUninit::<[u32; 2]>::uninit();
        let copied = copied.as_mut_ptr().cast::<u8>();
        copy_developers(copied, this.add(0x5C));
        let tree = Tree::from_ptr(copied);
        let mut steps = 0u32;
        while tree.count() != 0 {
            steps = steps.wrapping_add(read(budget.add(12).cast::<u32>()));
            write(budget.add(8).cast::<u32>(), steps);
            if steps & 1023 == 0 && grow_budget_expired(budget) as u8 != 0 {
                clear_tree(copied, false);
                break;
            }
            let mut total = 0u32;
            let mut node = Node::from_ptr(tree.header()).left();
            while node != tree.header() {
                let adjusted = demand(Node::from_ptr(node).value() as *mut u8);
                if adjusted > 5 {
                    total = total.wrapping_add(sim.demand_bias().wrapping_add(adjusted) as u32);
                }
                node = successor(node);
            }
            let mut pick = random(rng(), total) as i32;
            let mut node = Node::from_ptr(tree.header()).left();
            while node != tree.header() {
                let developer = Node::from_ptr(node).value() as *mut u8;
                let adjusted = demand(developer);
                if adjusted > 5 {
                    let weight = sim.demand_bias().wrapping_add(adjusted);
                    if pick < weight {
                        let mut key = vcall!(developer, 0x18 => fn() -> u32);
                        attempt(sim, developer, &mut key, padding);
                        remove_developer(copied, key);
                        break;
                    }
                    pick = pick.wrapping_sub(weight);
                }
                node = successor(node);
            }
            steps = read(budget.add(8).cast::<u32>());
        }
        if !tree.header().is_null() {
            free(tree.header());
        }
        let checks = steps >> 10;
        if checks == 0 {
            return 0;
        }
        if checks > 20 {
            let next = read(adjustment()).wrapping_add(1);
            write(adjustment(), next);
            if next < 16 {
                return next as u32;
            }
            let current = read(pace());
            write(adjustment(), 0);
            if current > 1 {
                write(pace(), current >> 1);
                return current >> 1;
            }
            current
        } else if checks < 5 {
            let next = read(adjustment()).wrapping_sub(1);
            write(adjustment(), next);
            if next > -4 {
                return next as u32;
            }
            let current = read(pace());
            write(adjustment(), 0);
            if current < 1024 {
                write(pace(), current.wrapping_mul(2));
            }
            current
        } else {
            checks
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0069b0b0::PORT];

#[cfg(test)]
mod tests;
