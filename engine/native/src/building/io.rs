// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Version-four serialization, retaining list, tree and hash-bucket iteration order.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use core::ptr::read_unaligned as read;

game_fn! { fn write_reference(stream: *mut u8, object: *mut u8) -> u64 = "C" 0x005D_99A0; }
game_fn! { fn write_bool(stream: *mut u8, value: u32) -> u64 = "C" 0x005D_9FD0; }
game_data! { static tried_header: *mut u8 = 0x00B4_85E0; }
game_data! { static tried_count: u32 = 0x00B4_85E4; }

layout! {
    /// Fields relative to the serializable interface, twelve bytes into the simulator.
    pub struct SaveView {
        0x3C => grid / set_grid: *mut u8,
        0x40 => special / set_special: *mut u8,
        0x44 => stages / set_stages: *mut u8,
        0x48 => stage_count / set_stage_count: u32,
        0x50 => developers / set_developers: *mut u8,
        0x54 => developer_count / set_developer_count: u32,
        0x64 => construction / set_construction: *mut u8,
        0x94 => plans / set_plans: *mut *mut u8,
        0x98 => plans_end / set_plans_end: *mut *mut u8,
        0xA0 => plan_count / set_plan_count: u32,
        0xD1 => ignore_plumbing / set_ignore_plumbing: u8,
        0xD2 => ignore_power / set_ignore_power: u8,
        0xD3 => ignore_caps / set_ignore_caps: u8,
    }
}
layout! {
    /// Ordered tree links and its key, value or embedded inner-tree header, and count.
    pub struct SaveNode {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: u32,
        0x14 => value / set_value: u32,
        0x14 => stage / set_stage: u8,
        0x18 => count / set_count: u32,
    }
}

pub(super) unsafe fn successor(mut node: *mut u8) -> *mut u8 {
    // SAFETY: all links belong to the live ordered game tree.
    unsafe {
        let right = SaveNode::from_ptr(node).right();
        if !right.is_null() {
            node = right;
            while !SaveNode::from_ptr(node).left().is_null() {
                node = SaveNode::from_ptr(node).left();
            }
        } else {
            let mut parent = SaveNode::from_ptr(node).parent();
            while node == SaveNode::from_ptr(parent).right() {
                node = parent;
                parent = SaveNode::from_ptr(parent).parent();
            }
            if SaveNode::from_ptr(node).right() != parent {
                node = parent;
            }
        }
        node
    }
}

unsafe fn transfer(stream: *mut u8, slot: usize, word: u32) -> u64 {
    // SAFETY: the original output-stream slot accepts a single argument word.
    unsafe {
        let args = [word];
        let mut call = RawCall::new(Conv::Thiscall, vslot(stream, slot), stream as u32, &args);
        call.eax = word;
        call.edx = read(stream.cast());
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

unsafe fn error(stream: *mut u8, eax: u32, edx: u32) -> u64 {
    // SAFETY: the original output stream's error slot accepts no stack arguments.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(stream, 0x48), stream as u32, &[]);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

fn bucket_count(begin: *mut *mut u8, end: *mut *mut u8) -> u32 {
    ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32
}

port! {
    /// Writes references, developer and stage trees, special buildings, tried tracts, planned growth and cheats.
    /// Initial stream errors suppress all writes. Subsequent failures do not interrupt the traversal.
    /// AL reports the final error check; the rest of EAX and EDX retain the error getter's result.
    #[port(address = 0x0069_7800, owner = "building", name = "cSC4BuildingDevelopmentSimulator::Write")]
    pub unsafe extern "fastcall" fn fc4p_00697800(this: *mut u8, entry_edx: u32, stream: *mut u8) -> u64 {
        // SAFETY: the game's serializable subobject, stream, container headers and nodes.
        unsafe {
            let sim = SaveView::from_ptr(this);
            let initial = error(stream, read(stream.cast()), entry_edx);
            if initial as u8 != 0 { return initial & !255; }
            transfer(stream, 0x1C, 4);
            write_reference(stream, sim.grid());
            write_reference(stream, sim.construction());
            transfer(stream, 0x24, sim.developer_count());
            let mut current = SaveNode::from_ptr(sim.developers()).left();
            while current != sim.developers() {
                let n = SaveNode::from_ptr(current);
                let object = n.value() as *mut u8;
                transfer(stream, 0x24, n.key());
                write_reference(stream, object);
                current = successor(current);
            }
            let header = sim.special();
            let mut node = read(header.cast::<*mut u8>());
            let mut count = 0u32;
            while node != header { node = read(node.cast::<*mut u8>()); count = count.wrapping_add(1); }
            transfer(stream, 0x24, count);
            let mut node = read(sim.special().cast::<*mut u8>());
            while node != sim.special() {
                write_reference(stream, read(node.add(8).cast()));
                node = read(node.cast());
            }
            transfer(stream, 0x24, sim.stage_count());
            let mut current = SaveNode::from_ptr(sim.stages()).left();
            while current != sim.stages() {
                let n = SaveNode::from_ptr(current);
                let stage = n.stage();
                transfer(stream, 0x24, n.key());
                transfer(stream, 0x14, (stream as u32 & !255) | u32::from(stage));
                current = successor(current);
            }
            transfer(stream, 0x24, read(tried_count()));
            let mut outer = SaveNode::from_ptr(read(tried_header())).left();
            while outer != read(tried_header()) {
                let n = SaveNode::from_ptr(outer);
                transfer(stream, 0x24, n.key());
                transfer(stream, 0x24, n.count());
                let mut inner = SaveNode::from_ptr(n.value() as *mut u8).left();
                while inner != n.value() as *mut u8 {
                    let pair = SaveNode::from_ptr(inner);
                    let z = pair.value();
                    transfer(stream, 0x20, pair.key());
                    transfer(stream, 0x20, z);
                    inner = successor(inner);
                }
                outer = successor(outer);
            }
            transfer(stream, 0x24, sim.plan_count());
            let count = bucket_count(sim.plans(), sim.plans_end());
            let mut plan = core::ptr::null_mut::<u8>();
            for i in 0..count {
                plan = read(sim.plans().wrapping_add(i as usize));
                if !plan.is_null() { break; }
            }
            while !plan.is_null() {
                let day = read(plan.add(0x14).cast::<u32>());
                write_reference(stream, read(plan.add(4).cast()));
                transfer(stream, 0x20, read(plan.add(8).cast()));
                transfer(stream, 0x20, read(plan.add(12).cast()));
                transfer(stream, 0x14, u32::from(*plan.add(16)));
                transfer(stream, 0x24, day);
                let next = read(plan.cast::<*mut u8>());
                if !next.is_null() { plan = next; continue; }
                let developer = read(plan.add(4).cast::<u32>());
                let x = read(plan.add(8).cast::<u32>());
                let z = read(plan.add(12).cast::<u32>());
                let stage = u32::from(*plan.add(16));
                let hash = (((developer.wrapping_shl(6) | (x & 63)).wrapping_shl(6) | (z & 63)).wrapping_shl(4)) | (stage & 15);
                let count = bucket_count(sim.plans(), sim.plans_end());
                let bucket: u32;
                core::arch::asm!("div {count}", count = in(reg) count, inout("eax") hash => _,
                    inout("edx") 0u32 => bucket, options(nomem, nostack));
                let start = bucket.wrapping_add(1);
                plan = core::ptr::null_mut();
                for i in start..count {
                    plan = read(sim.plans().wrapping_add(i as usize));
                    if !plan.is_null() { break; }
                }
            }
            write_bool(stream, u32::from(sim.ignore_plumbing()));
            write_bool(stream, u32::from(sim.ignore_power()));
            let last = write_bool(stream, u32::from(sim.ignore_caps()));
            let result = error(stream, last as u32, read(stream.cast()));
            (result & !255) | u64::from(result as u8 == 0)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00697800::PORT];

#[cfg(test)]
mod tests;
