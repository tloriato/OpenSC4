// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Demand-weighted monthly decline planning.

use super::{
    io::successor,
    trees::{Node, Tree},
    BuildingSim,
};
use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn cursor(this: *mut u8, developer: *mut *mut u8) -> *mut *mut u8 = "thiscall" 0x0049_7A00; }
game_fn! { fn random(this: *mut u8, limit: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { fn delete_cursor_subtree(this: *mut u8, root: *mut u8) -> u64 = "thiscall" 0x0069_5D90; }
game_data! { static rng: u8 = 0x00B4_85F0; }
game_data! { static simulator: *mut u8 = 0x00B4_3D5C; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }

unsafe fn demand(developer: *mut u8) -> i32 {
    // SAFETY: the developer's demand node returns its value in ST0.
    unsafe {
        let node = vcall!(developer, 0x48 => fn() -> *mut u8);
        vcall_st0!(node, 0x30).ftol() as i32
    }
}
unsafe fn unsigned(value: u32) -> F80 {
    let n = F80::from_i32(value as i32);
    // SAFETY: the game constant is the unsigned-integer conversion bias, 2^32.
    if (value as i32) < 0 {
        n + F80::from_f32(unsafe { read(unsigned_bias()) })
    } else {
        n
    }
}

port! {
    /// Replaces the decline plan using demand-weighted draws and per-developer candidate cursors.
    /// Exhausted developers retain their weight, so their draws may do no work.
    /// The entry retains the padding copied with each plan entry's zero stage byte.
    #[port(address = 0x0069_9890, owner = "building", name = "cSC4BuildingDevelopmentSimulator::PlanDecline", naked)]
    pub unsafe extern "C" fn fc4p_00699890() {
        core::arch::naked_asm!("push dword ptr [esp - 4]", "call {body}", "ret", body = sym plan_decline);
    }
}

/// # Safety
/// The original simulator and the retained bytes of its local stage word.
unsafe extern "fastcall" fn plan_decline(this: *mut u8, _entry_edx: u32, padding: u32) -> u64 {
    // SAFETY: the game owns the developer tree, candidate vectors and plan list; its helpers own the cursor tree.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let mut old = read(sim.decline_plan().cast::<*mut u8>());
        while old != sim.decline_plan() {
            let node = old;
            old = read(node.cast());
            free(node);
        }
        write(sim.decline_plan().cast::<*mut u8>(), sim.decline_plan());
        write(sim.decline_plan().add(4).cast::<*mut u8>(), sim.decline_plan());
        let header = allocate(24);
        let h = Node::from_ptr(header);
        h.set_color(0);
        h.set_parent(core::ptr::null_mut());
        h.set_left(header);
        h.set_right(header);
        let mut cursors = [header as u32, 0];
        let cursor_map = cursors.as_mut_ptr().cast::<u8>();
        let mut total = 0u32;
        let mut count = 0u32;
        let mut current = Node::from_ptr(sim.developers()).left();
        while current != sim.developers() {
            let mut developer = Node::from_ptr(current).value() as *mut u8;
            if vcall!(developer, 0x1C => fn() -> u32) != 5 {
                let adjusted = demand(developer);
                if adjusted < -20 {
                    total = total.wrapping_add(sim.demand_bias().wrapping_sub(adjusted) as u32);
                }
                let list = vcall!(developer, 0x30 => fn() -> *mut u8);
                let first = read(list.cast::<*mut u8>());
                write(cursor(cursor_map, &mut developer), first);
            }
            current = successor(current);
        }
        let date = vcall!(read(simulator()), 0x1C => fn() -> *mut u8);
        let today = vcall!(date, 0x68 => fn() -> u32);
        loop {
            let mut pick = random(rng(), total) as i32;
            let mut current = Node::from_ptr(sim.developers()).left();
            while current != sim.developers() {
                let mut developer = Node::from_ptr(current).value() as *mut u8;
                if vcall!(developer, 0x1C => fn() -> u32) != 5 {
                    let adjusted = demand(developer);
                    if adjusted < -20 {
                        let weight = sim.demand_bias().wrapping_sub(adjusted);
                        if pick < weight {
                            let list = vcall!(developer, 0x30 => fn() -> *mut u8);
                            let at = cursor(cursor_map, &mut developer);
                            let candidate = read(at);
                            if candidate != read(list.add(4).cast::<*mut u8>()) {
                                let x = read(candidate.cast::<u32>());
                                let z = read(candidate.add(4).cast::<u32>());
                                let day = count;
                                count = count.wrapping_add(1);
                                let tail = sim.decline_plan();
                                let node = allocate(28);
                                if !node.wrapping_add(8).is_null() {
                                    write(node.add(8).cast::<u32>(), day);
                                    write(node.add(12).cast::<u32>(), x);
                                    write(node.add(16).cast::<u32>(), z);
                                    write(node.add(20).cast::<*mut u8>(), developer);
                                    write(node.add(24).cast::<u32>(), padding & !255);
                                }
                                let previous: *mut u8 = read(tail.add(4).cast());
                                write(node.add(4).cast::<*mut u8>(), previous);
                                write(node.cast::<*mut u8>(), tail);
                                write(previous.cast::<*mut u8>(), node);
                                write(tail.add(4).cast::<*mut u8>(), node);
                                write(at, read(at).wrapping_add(12));
                            }
                            break;
                        }
                        pick = pick.wrapping_add(adjusted.wrapping_sub(sim.demand_bias()));
                    }
                }
                current = successor(current);
            }
            let mut unfinished = false;
            let mut current = Node::from_ptr(sim.developers()).left();
            while current != sim.developers() {
                let mut developer = Node::from_ptr(current).value() as *mut u8;
                if vcall!(developer, 0x1C => fn() -> u32) != 5 && demand(developer) < -20 {
                    let list = vcall!(developer, 0x30 => fn() -> *mut u8);
                    let end: *mut u8 = read(list.add(4).cast());
                    if read(cursor(cursor_map, &mut developer)) != end {
                        unfinished = true;
                        break;
                    }
                }
                current = successor(current);
            }
            if !unfinished {
                break;
            }
        }
        if count as i32 > 0 {
            let mut node = read(sim.decline_plan().cast::<*mut u8>());
            if node != sim.decline_plan() {
                let now = unsigned(today);
                while node != sim.decline_plan() {
                    let day = unsigned(read(node.add(8).cast::<u32>()));
                    write(node.add(8).cast::<u32>(), (day + now).ftol() as u32);
                    node = read(node.cast());
                }
            }
        }
        let map = Tree::from_ptr(cursor_map);
        if map.count() != 0 {
            let mut node = Node::from_ptr(map.header()).parent();
            while !node.is_null() {
                delete_cursor_subtree(cursor_map, Node::from_ptr(node).right());
                let left = Node::from_ptr(node).left();
                free(node);
                node = left;
            }
            let h = Node::from_ptr(map.header());
            h.set_left(map.header());
            h.set_parent(core::ptr::null_mut());
            h.set_right(map.header());
            map.set_count(0);
        }
        let header = map.header();
        if header.is_null() {
            return 0;
        }
        // The free helper supplies the final return registers.
        let words = [header as u32];
        let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &words);
        call.eax = header as u32;
        call.edx = header as u32;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

pub const PORTS: &[Port] = &[fc4p_00699890::PORT];

#[cfg(test)]
mod tests;
