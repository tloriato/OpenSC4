// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Catalogue enumeration, family expansion, and name/profile entry access.

use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;
use core::ptr::{null_mut, read_unaligned as read, write_unaligned as write};

game_fn! { fn begin(this: *mut u8, iterator: *mut *mut u8) -> u64 = "thiscall" 0x004B_0390; }
game_fn! { fn assign_vector(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0046_2C10; }
game_fn! { fn insert_vector(this: *mut u8, position: *mut u8, cookie: *mut u8, key: *mut u32, a: u32, b: u32) -> u64 = "thiscall" 0x0051_CA60; }
game_fn! { fn reserve_set(this: *mut u8, count: u32) -> u64 = "thiscall" 0x004F_0940; }
game_fn! { fn insert_set(this: *mut u8, out: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0062_4DD0; }
game_fn! { #[allow(dead_code)] fn name_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0048_62F0; }
game_fn! { #[allow(dead_code)] fn profile_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0069_B720; }
game_fn! { fn tried_developer(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x0069_AB40; }
game_fn! { #[allow(dead_code)] fn find_tract(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0069_5F70; }

game_data! { static building_map: u8 = 0x00B4_8548; }
game_data! { static building_count: u32 = 0x00B4_8558; }
game_data! { static building_begin: *mut *mut u8 = 0x00B4_854C; }
game_data! { static building_end: *mut *mut u8 = 0x00B4_8550; }
game_data! { static family_map: u8 = 0x00B4_8598; }
game_data! { static family_begin: *mut *mut u8 = 0x00B4_859C; }
game_data! { static family_end: *mut *mut u8 = 0x00B4_85A0; }
game_data! { static name_map: u8 = 0x00B4_855C; }
game_data! { static profile_map: u8 = 0x00B4_8584; }
game_data! { static tried_map: u8 = 0x00B4_85E0; }
game_data! { static active_developer: u32 = 0x00B4_8544; }

layout! {
    /// A chained catalogue hash node, optionally holding a member vector.
    pub struct HashNode {
        0x00 => next / set_next: *mut u8,
        0x04 => key / set_key: u32,
        0x08 => members / set_members: *mut u32,
        0x0C => members_end / set_members_end: *mut u32,
    }
}
layout! {
    /// The bucket vector is preceded by the container's allocator word.
    pub struct HashMap {
        0x04 => buckets / set_buckets: *mut *mut u8,
        0x08 => buckets_end / set_buckets_end: *mut *mut u8,
    }
}
layout! {
    /// A vector of type IDs.
    pub struct TypeVector {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
        0x08 => limit / set_limit: *mut u32,
    }
}

fn length(begin: *mut u32, end: *mut u32) -> i32 {
    (end as u32).wrapping_sub(begin as u32) as i32 >> 2
}
unsafe fn divide(key: u32, count: u32) -> (u32, u32) {
    let (quotient, remainder): (u32, u32);
    // SAFETY: hardware division retains the original divide error for an empty map.
    unsafe {
        core::arch::asm!("div {count}", count = in(reg) count, inout("eax") key => quotient,
        inout("edx") 0u32 => remainder, options(nomem, nostack));
    }
    (quotient, remainder)
}
unsafe fn find(buckets: *mut *mut u8, end: *mut *mut u8, key: u32) -> (*mut u8, u32, u32) {
    // SAFETY: the caller supplies a readable game bucket vector and chains.
    unsafe {
        let (quotient, bucket) = divide(key, length(buckets.cast(), end.cast()) as u32);
        let mut node = read(buckets.wrapping_add(bucket as usize));
        while !node.is_null() && HashNode::from_ptr(node).key() != key {
            node = HashNode::from_ptr(node).next();
        }
        (node, quotient, bucket)
    }
}

/// Returns the next chain/bucket node, plus the scratch registers of the bucket scan.
unsafe fn next(node: *mut u8, map: *mut u8, edx: u32) -> (*mut u8, u32, u32) {
    // SAFETY: the iterator's map and current node are live game storage.
    unsafe {
        let n = HashNode::from_ptr(node);
        if !n.next().is_null() {
            return (n.next(), n.next() as u32, edx);
        }
        let m = HashMap::from_ptr(map);
        let count = length(m.buckets().cast(), m.buckets_end().cast()) as u32;
        let (_, mut bucket) = divide(n.key(), count);
        let mut result = null_mut();
        loop {
            bucket = bucket.wrapping_add(1);
            if bucket >= count {
                break;
            }
            result = read(m.buckets().wrapping_add(bucket as usize));
            if !result.is_null() {
                break;
            }
        }
        (result, count, bucket)
    }
}

unsafe fn append(vector: *mut u8, key: &mut u32, edx: u32) -> u64 {
    // SAFETY: the game supplies a writable vector with its reserved capacity.
    unsafe {
        let v = TypeVector::from_ptr(vector);
        let end = v.end();
        if end != v.limit() {
            if !end.is_null() {
                write(end, *key);
            }
            v.set_end(end.wrapping_add(1));
            (u64::from(edx) << 32) | u64::from(end as u32)
        } else {
            let mut cookie = core::mem::MaybeUninit::<u8>::uninit();
            insert_vector(vector, end.cast(), cookie.as_mut_ptr(), key, 1, 1)
        }
    }
}

port! {
    /// Clamps the caller's requested count and writes catalogue IDs in chain and bucket order.
    /// Returns the total catalogue size even for a short buffer, a null buffer or a damaged early end.
    #[port(address = 0x0069_7B80, owner = "building", name = "GetBuildingTypesArray")]
    pub unsafe extern "fastcall" fn fc4p_00697b80(_this: *mut u8, entry_edx: u32, out: *mut u32, count: *mut u32) -> u64 {
        // SAFETY: the caller's count and output cover the requested number of IDs.
        unsafe {
            let total = read(building_count());
            if read(count) > total { write(count, total); }
            let mut edx = entry_edx;
            if !out.is_null() && read(count) != 0 {
                let mut iterator = [null_mut(); 2];
                let result = begin(building_map(), iterator.as_mut_ptr());
                edx = (result >> 32) as u32;
                let [mut node, map] = iterator;
                let mut i = 0u32;
                while i < read(count) && !node.is_null() {
                    edx = HashNode::from_ptr(node).key();
                    write(out.wrapping_add(i as usize), edx);
                    let (new, _, scratch) = next(node, map, edx);
                    node = new; edx = scratch; i = i.wrapping_add(1);
                }
            }
            (u64::from(edx) << 32) | u64::from(total)
        }
    }
}
port! {
    /// Clears the destination's length and appends every family ID in hash iteration order.
    #[port(address = 0x0069_7D10, owner = "building", name = "GetBuildingFamilyTypesVector")]
    pub unsafe extern "stdcall" fn fc4p_00697d10(vector: *mut u8) -> u64 {
        // SAFETY: the destination vector and catalogue map belong to the game.
        unsafe {
            let v = TypeVector::from_ptr(vector); v.set_end(v.begin());
            let mut iterator = [null_mut(); 2];
            let mut r = begin(family_map(), iterator.as_mut_ptr());
            let [mut node, map] = iterator;
            while !node.is_null() {
                let mut key = HashNode::from_ptr(node).key();
                r = append(vector, &mut key, (r >> 32) as u32);
                let (new, eax, edx) = next(node, map, (r >> 32) as u32);
                node = new; r = (u64::from(edx) << 32) | u64::from(eax);
            }
            r
        }
    }
}
port! {
    /// Replaces a vector with the family's members, or a single known building type.
    /// Only the family path returns its size; a direct building still returns zero.
    #[port(address = 0x0069_7EF0, owner = "building", name = "ExpandBuildingFamilyVector")]
    pub unsafe extern "stdcall" fn fc4p_00697ef0(vector: *mut u8, key: u32) -> u64 {
        // SAFETY: the live maps and destination vector have their original layouts.
        unsafe {
            let mut key = key;
            let (family, _, _) = find(read(family_begin()), read(family_end()), key);
            if !family.is_null() {
                let n = HashNode::from_ptr(family);
                let count = length(n.members(), n.members_end()) as u32;
                let r = assign_vector(vector, family.add(8));
                return (r & !0xFFFF_FFFF) | u64::from(count);
            }
            let (building, _, _) = find(read(building_begin()), read(building_end()), key);
            if building.is_null() { return 0; }
            let v = TypeVector::from_ptr(vector); v.set_end(v.begin());
            append(vector, &mut key, building as u32) & !0xFFFF_FFFF
        }
    }
}
port! {
    /// Adds every family member to an existing set; direct known buildings are inserted once.
    #[port(address = 0x0069_9D20, owner = "building", name = "ExpandBuildingFamilySet")]
    pub unsafe extern "stdcall" fn fc4p_00699d20(set: *mut u8, key: u32) -> u64 {
        // SAFETY: the original set, catalogue vectors and insertion helpers.
        unsafe {
            let mut key = key;
            let (family, _, bucket) = find(read(family_begin()), read(family_end()), key);
            let mut cookie = core::mem::MaybeUninit::<[u8; 12]>::uninit();
            if !family.is_null() {
                let n = HashNode::from_ptr(family);
                let count = length(n.members(), n.members_end());
                let mut r = u64::from(bucket) << 32;
                for i in 0..count {
                    let source = n.members();
                    reserve_set(set, read(set.add(0x10).cast::<u32>()).wrapping_add(1));
                    r = insert_set(set, cookie.as_mut_ptr().cast(), source.wrapping_add(i as usize));
                }
                return (r & !0xFFFF_FFFF) | u64::from(count as u32);
            }
            let (building, _, _) = find(read(building_begin()), read(building_end()), key);
            if building.is_null() { return 0; }
            reserve_set(set, read(set.add(0x10).cast::<u32>()).wrapping_add(1));
            insert_set(set, cookie.as_mut_ptr().cast(), &mut key) & !0xFFFF_FFFF
        }
    }
}

port! {
    /// Returns the building name entry using the caller's type word.
    #[port(address = 0x0069_AB00, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetBuildingName", naked)]
    pub unsafe extern "C" fn fc4p_0069ab00() {
        core::arch::naked_asm!("lea ecx, [esp + 4]", "call {body}", "ret 4", body = sym name_entry_body);
    }
}
port! {
    /// Returns the building profile entry, inserting a default entry when the type is absent.
    #[port(address = 0x0069_CA60, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetBuildingProfile", naked)]
    pub unsafe extern "C" fn fc4p_0069ca60() {
        core::arch::naked_asm!("lea ecx, [esp + 4]", "call {body}", "ret 4", body = sym profile_entry_body);
    }
}

unsafe extern "fastcall" fn name_entry_body(key: *mut u32, entry_edx: u32) -> u64 {
    // SAFETY: the entry forwards its writable caller key.
    unsafe { entry(0x0048_62F0, name_map(), key, entry_edx) }
}
unsafe extern "fastcall" fn profile_entry_body(key: *mut u32, entry_edx: u32) -> u64 {
    // SAFETY: the entry forwards its writable caller key.
    unsafe { entry(0x0069_B720, profile_map(), key, entry_edx) }
}

unsafe fn entry(address: u32, map: *mut u8, key: *mut u32, edx: u32) -> u64 {
    let args = [key as u32];
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), map as u32, &args);
    call.eax = key as u32;
    call.edx = edx;
    // SAFETY: each entry accessor declares and invokes its original map helper.
    unsafe { call_raw(&mut call) };
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

port! {
    /// Tests whether a candidate tract is absent from the current developer's tried set.
    #[port(address = 0x0069_B070, owner = "building", name = "CandidateTractNotTried")]
    pub unsafe extern "C" fn fc4p_0069b070(candidate: *mut u32) -> u64 {
        // SAFETY: the candidate has two coordinate words; the game helper supplies the nested tree.
        unsafe {
            let tree = tried_developer(tried_map(), active_developer());
            let mut pair = [read(candidate), read(candidate.add(1))];
            let words = [pair.as_mut_ptr() as u32];
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0069_5F70), tree as u32, &words);
            call.eax = pair.as_mut_ptr() as u32; call.edx = pair[1];
            call_raw(&mut call);
            let header = read(tree.cast::<u32>());
            (u64::from(call.edx) << 32) | u64::from(call.eax == header)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00697b80::PORT,
    fc4p_00697d10::PORT,
    fc4p_00697ef0::PORT,
    fc4p_00699d20::PORT,
    fc4p_0069ab00::PORT,
    fc4p_0069b070::PORT,
    fc4p_0069ca60::PORT,
];

#[cfg(test)]
mod tests;
