// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Ordered map searches, subtree disposal, candidate scans, and catalogue lookups.

use super::*;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn free(node: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn delete_developer_subtree(this: *mut u8, node: *mut u8) = "thiscall" 0x0069_5CD0; }
game_fn! { fn delete_stage_subtree(this: *mut u8, node: *mut u8) = "thiscall" 0x0069_5D90; }
game_fn! { fn delete_profile_subtree(this: *mut u8, node: *mut u8) = "thiscall" 0x0069_5E10; }
game_fn! { #[allow(dead_code)] fn developer_find(this: *mut u8, out: *mut *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn day_find(this: *mut u8, out: *mut *mut u8, key: *mut u32) -> u64 = "thiscall" 0x006C_0030; }
game_data! { static catalogue_buckets: *mut *mut u8 = 0x00B4_854C; }
game_data! { static catalogue_buckets_end: *mut *mut u8 = 0x00B4_8550; }

layout! {
    /// Shared tree header and ordinary node links, followed by an ordinary node's key.
    pub struct TreeNode {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => first / set_first: i32,
        0x14 => second / set_second: i32,
        0x14 => second_byte / set_second_byte: u8,
    }
}

type SubtreeDelete = unsafe fn(*mut u8, *mut u8);

unsafe fn delete_subtree(this: *mut u8, mut node: *mut u8, recurse: SubtreeDelete) {
    // SAFETY: the caller supplies live tree nodes; the matching game helper owns the right subtrees.
    unsafe {
        while !node.is_null() {
            let n = TreeNode::from_ptr(node);
            recurse(this, n.right());
            let left = n.left();
            free(node);
            node = left;
        }
    }
}

port! {
    /// Disposes a developer-map subtree in right, node, left order.
    #[port(address = 0x0069_5CD0, owner = "building", name = "DeleteDeveloperSubtree")]
    pub unsafe extern "thiscall" fn fc4p_00695cd0(this: *mut u8, node: *mut u8) {
        // SAFETY: this map and its subtree belong to the game.
        unsafe { delete_subtree(this, node, delete_developer_subtree); }
    }
}
port! {
    /// Disposes a growth-stage-map subtree in right, node, left order.
    #[port(address = 0x0069_5D90, owner = "building", name = "DeleteStageSubtree")]
    pub unsafe extern "thiscall" fn fc4p_00695d90(this: *mut u8, node: *mut u8) {
        // SAFETY: this map and its subtree belong to the game.
        unsafe { delete_subtree(this, node, delete_stage_subtree); }
    }
}
port! {
    /// Disposes a profile-map subtree in right, node, left order.
    #[port(address = 0x0069_5E10, owner = "building", name = "DeleteProfileSubtree")]
    pub unsafe extern "thiscall" fn fc4p_00695e10(this: *mut u8, node: *mut u8) {
        // SAFETY: this map and its subtree belong to the game.
        unsafe { delete_subtree(this, node, delete_profile_subtree); }
    }
}

port! {
    /// Finds the first twelve-byte item accepted by a cdecl predicate, testing its low result byte.
    #[port(address = 0x0069_5E50, owner = "building", name = "FindCandidate")]
    pub unsafe extern "C" fn fc4p_00695e50(begin: *mut u8, end: *mut u8, predicate: unsafe extern "C" fn(*mut u8) -> u32) -> *mut u8 {
        let mut current = begin;
        let count = (end as u32).wrapping_sub(begin as u32) as i32 / 12;
        let groups = count >> 2;
        // SAFETY: the game supplies the accessible range and cdecl predicate.
        unsafe {
            for _ in 0..groups {
                for _ in 0..4 {
                    if predicate(current) as u8 != 0 { return current; }
                    current = current.wrapping_add(12);
                }
            }
            let remaining = (end as u32).wrapping_sub(current as u32) as i32 / 12;
            if (1..=3).contains(&remaining) {
                for _ in 0..remaining {
                    if predicate(current) as u8 != 0 { return current; }
                    current = current.wrapping_add(12);
                }
            }
        }
        end
    }
}

port! {
    /// Looks up a signed developer key paired with an unsigned one-byte growth stage.
    #[port(address = 0x0069_5F10, owner = "building", name = "FindCapacityProfile")]
    pub unsafe extern "thiscall" fn fc4p_00695f10(this: *mut u8, key: *mut u8) -> *mut u8 {
        // SAFETY: the map header, links, and five-byte key are game-owned storage.
        unsafe {
            let header = read_unaligned(this.cast::<*mut u8>());
            let mut node = TreeNode::from_ptr(header).parent();
            let mut result = header;
            if !node.is_null() {
                let first = read_unaligned(key.cast::<i32>());
                loop {
                    let n = TreeNode::from_ptr(node);
                    if first > n.first() || (first == n.first() && n.second_byte() < *key.add(4)) {
                        node = n.right();
                    } else { result = node; node = n.left(); }
                    if node.is_null() { break; }
                }
            }
            if result != header {
                let first = read_unaligned(key.cast::<i32>());
                let n = TreeNode::from_ptr(result);
                if n.first() > first || (n.first() == first && *key.add(4) < n.second_byte()) { result = header; }
            }
            result
        }
    }
}

port! {
    /// Captures incoming EDX and restores the search's ECX result as well as EDX:EAX.
    #[port(address = 0x0069_5F70, owner = "building", name = "FindTractPair", naked)]
    pub unsafe extern "C" fn fc4p_00695f70() {
        core::arch::naked_asm!(
            "sub esp, 4", "lea eax, [esp]", "push dword ptr [esp + 8]", "push eax",
            "call {body}", "mov ecx, dword ptr [esp]", "add esp, 4", "ret 4",
            body = sym find_tract_pair,
        );
    }
}

/// # Safety
/// The entry supplies a live map, an eight-byte key, and a writable ECX result location.
unsafe extern "fastcall" fn find_tract_pair(this: *mut u8, entry_edx: u32, out_ecx: *mut u32, key: *mut u8) -> u64 {
    // SAFETY: the entry's contract covers the header, tree links, and key words.
    unsafe {
        let header = read_unaligned(this.cast::<*mut u8>());
        let mut node = TreeNode::from_ptr(header).parent();
        let mut result = header;
        let mut edx = entry_edx;
        let mut ecx = node as u32;
        if !node.is_null() {
            let first = read_unaligned(key.cast::<i32>());
            loop {
                let n = TreeNode::from_ptr(node);
                edx = n.first() as u32;
                let right = if first > edx as i32 {
                    true
                } else if first < edx as i32 {
                    false
                } else {
                    edx = n.second() as u32;
                    (edx as i32) < read_unaligned(key.add(4).cast::<i32>())
                };
                if right {
                    node = n.right();
                } else {
                    result = node;
                    node = n.left();
                }
                ecx = node as u32;
                if node.is_null() {
                    break;
                }
            }
        }
        if result != header {
            ecx = read_unaligned(key.cast::<u32>());
            let n = TreeNode::from_ptr(result);
            edx = n.first() as u32;
            if (edx as i32) > ecx as i32 {
                result = header;
            } else if edx == ecx {
                ecx = read_unaligned(key.add(4).cast::<u32>());
                if (ecx as i32) < n.second() {
                    result = header;
                }
            }
        }
        write_unaligned(out_ecx, ecx);
        (u64::from(edx) << 32) | u64::from(result as u32)
    }
}

port! {
    /// Keeps the original key word address visible to the map lookup helper.
    #[port(address = 0x0069_6C80, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetDeveloper(DeveloperType)", naked)]
    pub unsafe extern "C" fn fc4p_00696c80() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push dword ptr [esp + 4]", "push eax",
            "call {body}", "ret 4", body = sym get_developer,
        );
    }
}
/// # Safety
/// The entry supplies a live simulator and its original key argument word.
unsafe extern "fastcall" fn get_developer(this: *mut u8, _entry_edx: u32, key: *mut u32, kind: u32) -> u64 {
    // SAFETY: the matching game helper initializes the local iterator.
    unsafe {
        let map = this.add(0x5C);
        let mut found = core::mem::MaybeUninit::<*mut u8>::uninit();
        let args = [found.as_mut_ptr() as u32, key as u32];
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0049_7520),
            map as u32,
            &args,
        );
        call.eax = kind;
        call.edx = found.as_mut_ptr() as u32;
        crate::abi::call_raw(&mut call);
        let found = found.assume_init();
        let header = read_unaligned(map.cast::<*mut u8>());
        let result = if found == header {
            0
        } else {
            read_unaligned(found.add(0x14).cast::<u32>())
        };
        (u64::from(call.edx) << 32) | u64::from(result)
    }
}

port! {
    /// Keeps the original developer key address visible to the day-map lookup helper.
    #[port(address = 0x0069_6CC0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetDayOfLastSuccessfulBuild", naked)]
    pub unsafe extern "C" fn fc4p_00696cc0() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push dword ptr [esp + 4]", "push eax",
            "call {body}", "ret 4", body = sym get_build_day,
        );
    }
}
/// # Safety
/// The entry supplies a live simulator and its original key argument word.
unsafe extern "fastcall" fn get_build_day(this: *mut u8, entry_edx: u32, key: *mut u32, _kind: u32) -> u64 {
    // SAFETY: the matching game helper initializes the local iterator.
    unsafe {
        let map = this.add(0xB0);
        let mut found = core::mem::MaybeUninit::<*mut u8>::uninit();
        let args = [found.as_mut_ptr() as u32, key as u32];
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x006C_0030),
            map as u32,
            &args,
        );
        call.eax = key as u32;
        call.edx = entry_edx;
        crate::abi::call_raw(&mut call);
        let found = found.assume_init();
        let header = read_unaligned(map.cast::<*mut u8>());
        let result = if found == header {
            0
        } else {
            read_unaligned(found.add(0x14).cast::<u32>())
        };
        (u64::from(call.edx) << 32) | u64::from(result)
    }
}

/// Searches the bucket selected by the unsigned key. The table has at least one bucket.
unsafe fn catalogue_find(key: u32) -> (u32, *mut u8) {
    // SAFETY: the process-wide catalogue supplies the bucket array and linked nodes.
    unsafe {
        let buckets = read_unaligned(catalogue_buckets());
        let end = read_unaligned(catalogue_buckets_end());
        let count = ((end as u32).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
        let quotient: u32;
        let remainder: u32;
        // Use the machine's unsigned division, including its fault for an empty bucket table.
        core::arch::asm!("div {count:e}", count = in(reg) count,
            inlateout("eax") key => quotient, inlateout("edx") 0u32 => remainder,
            options(nomem, nostack));
        let mut node = read_unaligned(buckets.wrapping_add(remainder as usize));
        while !node.is_null() {
            if read_unaligned(node.add(4).cast::<u32>()) == key {
                break;
            }
            node = read_unaligned(node.cast::<*mut u8>());
        }
        (quotient, node)
    }
}

port! {
    /// Copies a catalogue resource key on a hit. A miss leaves the output untouched.
    #[port(address = 0x0069_7DD0, owner = "building", name = "GetCatalogueResourceKey")]
    pub unsafe extern "thiscall" fn fc4p_00697dd0(_this: *mut u8, key: u32, out: *mut u32) -> u64 {
        // SAFETY: the catalogue nodes contain three resource-key words and out has room for all three.
        unsafe {
            let (quotient, node) = catalogue_find(key);
            if node.is_null() { return u64::from(quotient & !255); }
            write_unaligned(out, read_unaligned(node.add(8).cast::<u32>()));
            write_unaligned(out.add(1), read_unaligned(node.add(12).cast::<u32>()));
            let third = read_unaligned(node.add(16).cast::<u32>());
            write_unaligned(out.add(2), third);
            (u64::from(third) << 32) | u64::from((out as u32 & !255) | 1)
        }
    }
}

port! {
    /// Tests for a catalogue key, leaving its matching node address in EDX.
    #[port(address = 0x0069_7E30, owner = "building", name = "CatalogueContainsKey")]
    pub unsafe extern "thiscall" fn fc4p_00697e30(_this: *mut u8, key: u32) -> u64 {
        // SAFETY: the process-wide catalogue is initialized by the game.
        unsafe {
            let (_, node) = catalogue_find(key);
            (u64::from(node as u32) << 32) | u64::from(!node.is_null())
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00695cd0::PORT,
    fc4p_00695d90::PORT,
    fc4p_00695e10::PORT,
    fc4p_00695e50::PORT,
    fc4p_00695f10::PORT,
    fc4p_00695f70::PORT,
    fc4p_00696c80::PORT,
    fc4p_00696cc0::PORT,
    fc4p_00697dd0::PORT,
    fc4p_00697e30::PORT,
];

#[cfg(test)]
mod tests;
