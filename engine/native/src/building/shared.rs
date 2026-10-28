// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Shared occupant pool, unsigned sort, and twelve-byte container helpers.

use super::*;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { #[allow(dead_code)] fn occupant_query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 = "thiscall" 0x0045_87D0; }
game_fn! { #[allow(dead_code)] fn occupant_delete(this: *mut u8, flags: u32) -> u64 = "thiscall" 0x0045_8F80; }
game_fn! { fn base_construct(this: *mut u8) -> u64 = "thiscall" 0x005E_9220; }
game_fn! { fn base_destroy(this: *mut u8) -> u64 = "thiscall" 0x005E_8C40; }
game_fn! { fn pool_grow(this: *mut u8) -> *mut u8 = "thiscall" 0x0090_CF9A; }
game_fn! { fn pool_empty(this: *mut u8, page: *mut u8) -> u64 = "thiscall" 0x0090_D031; }
game_fn! { fn pool_reorder(this: *mut u8, page: *mut u8) -> u64 = "thiscall" 0x0090_CFF9; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(storage: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn element_construct(this: *mut u8, source: *mut u8) = "thiscall" 0x004C_9470; }
game_fn! { fn allocate_copy(this: *mut u8, count: u32, begin: *mut u8, end: *mut u8) -> *mut u8 = "thiscall" 0x004E_4040; }
game_fn! { fn destroy_range(begin: *mut u8, end: *mut u8, cookie: *mut u8) = "C" 0x0045_9170; }
game_fn! { fn assign_range(begin: *mut u8, end: *mut u8, dest: *mut u8, cookie: *mut u8, tag: u32) -> *mut u8 = "C" 0x006C_10A0; }
game_fn! { fn construct_range(begin: *mut u8, end: *mut u8, dest: *mut u8, cookie: *mut u8) -> *mut u8 = "C" 0x0069_9150; }
game_fn! { fn partition(begin: *mut u32, end: *mut u32, pivot: u32, compare: u32) -> *mut u32 = "C" 0x0061_1840; }
game_fn! { fn sort_part(begin: *mut u32, end: *mut u32, unused: u32, depth: u32, compare: u32) -> u32 = "C" 0x0047_EA40; }
game_fn! { fn heap_sort(begin: *mut u32, middle: *mut u32, end: *mut u32, unused: u32, compare: u32) -> u32 = "C" 0x0048_A4B0; }

game_data! { static pool_context: u8 = 0x00B0_7F1C; }
game_data! { static active_page: *mut u8 = 0x00B0_7F20; }
game_data! { static default_type: u32 = 0x00B0_7F18; }

#[cfg(test)]
unsafe extern "thiscall" fn query_thunk_target(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
    // SAFETY: the thunk forwards the original method's arguments.
    unsafe { occupant_query(this, iid, out) }
}

#[cfg(test)]
unsafe extern "thiscall" fn delete_thunk_target(this: *mut u8, flags: u32) -> u64 {
    // SAFETY: the thunk forwards the original method's arguments.
    unsafe { occupant_delete(this, flags) }
}

layout! {
    /// The shared resource occupant's own fields; its base constructor owns the intervening bytes.
    pub struct ResourceOccupant {
        0x00 => primary_table / set_primary_table: u32,
        0x04 => base_table / set_base_table: u32,
        0x08 => target_table / set_target_table: u32,
        0x0C => property_table / set_property_table: u32,
        0x20 => serializable_table / set_serializable_table: u32,
        0x28 => occupant_type / set_occupant_type: u32,
        0x38 => field_38 / set_field_38: u32,
        0x3C => field_3c / set_field_3c: u32,
        0x40 => field_40 / set_field_40: u32,
        0x44 => field_44 / set_field_44: u32,
        0x48 => field_48 / set_field_48: u32,
        0x4C => field_4c / set_field_4c: u32,
        0x50 => field_50 / set_field_50: u32,
        0x54 => orientation / set_orientation: u8,
        0x55 => flags / set_flags: u8,
        0x58 => resource_type / set_resource_type: u32,
    }
}

layout! {
    /// Vector of twelve-byte objects.
    pub struct ObjectVector {
        0x00 => begin / set_begin: *mut u8,
        0x04 => end / set_end: *mut u8,
        0x08 => limit / set_limit: *mut u8,
    }
}

port! {
    /// Adjusts the property interface to the primary interface before querying it.
    #[port(address = 0x0045_8A10, owner = "building", name = "ResourceOccupantQueryThunk", naked)]
    pub unsafe extern "C" fn fc4p_00458a10() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 12", "push 0x004587d0", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 12", "jmp {target}", target = sym query_thunk_target);
    }
}

port! {
    /// Adjusts the property interface to the primary interface before deleting it.
    #[port(address = 0x0045_8A20, owner = "building", name = "ResourceOccupantDeleteThunk", naked)]
    pub unsafe extern "C" fn fc4p_00458a20() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 12", "push 0x00458f80", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 12", "jmp {target}", target = sym delete_thunk_target);
    }
}

port! {
    /// Takes a free occupant from the current pool page, growing the pool when needed.
    #[port(address = 0x0045_8A60, owner = "building", name = "AllocateResourceOccupant")]
    pub unsafe extern "C" fn fc4p_00458a60() -> u64 {
        // SAFETY: these globals describe the game's pool pages and free-list objects.
        unsafe {
            let page = read_unaligned(active_page());
            let mut item = read_unaligned(page.add(8).cast::<*mut u8>());
            if item.is_null() { item = pool_grow(pool_context()); }
            let next = read_unaligned(item.cast::<*mut u8>());
            let page = read_unaligned(active_page());
            write_unaligned(page.add(8).cast::<*mut u8>(), next);
            let count_page = read_unaligned(active_page());
            let count = read_unaligned(count_page.add(12).cast::<u32>());
            write_unaligned(count_page.add(12).cast(), count.wrapping_add(1));
            (u64::from(page as u32) << 32) | u64::from(item as u32)
        }
    }
}

fn final_tables(occupant: ResourceOccupant) {
    occupant.set_primary_table(0x00A8_72C0);
    occupant.set_base_table(0x00A8_7238);
    occupant.set_target_table(0x00A8_721C);
    occupant.set_property_table(0x00A8_71B8);
    occupant.set_serializable_table(0x00A8_71A0);
}

port! {
    /// Constructs the base before installing the resource occupant's interface tables and fields.
    #[port(address = 0x0045_8E90, owner = "building", name = "ConstructResourceOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00458e90(this: *mut u8) -> u64 {
        // SAFETY: this is the game's allocated occupant storage.
        unsafe {
            let occupant = ResourceOccupant::from_ptr(this);
            occupant.set_primary_table(0x00A8_7320);
            let result = base_construct(this.add(4));
            occupant.set_field_38(0);
            final_tables(occupant);
            occupant.set_field_3c(0);
            occupant.set_field_40(0);
            occupant.set_field_44(0);
            occupant.set_field_48(0);
            occupant.set_field_4c(0);
            occupant.set_field_50(0);
            occupant.set_orientation(0);
            occupant.set_flags(0);
            occupant.set_resource_type(read_unaligned(default_type()));
            occupant.set_occupant_type(0x2781_28A0);
            (result & 0xFFFF_FFFF_0000_0000) | u64::from(this as u32)
        }
    }
}

port! {
    /// Destroys the base and optionally returns this occupant to its aligned pool page.
    #[port(address = 0x0045_8F80, owner = "building", name = "DeleteResourceOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00458f80(this: *mut u8, flags: u32) -> u64 {
        // SAFETY: the occupant belongs to an 8192-byte pool page when deletion is requested.
        unsafe {
            final_tables(ResourceOccupant::from_ptr(this));
            let mut result = base_destroy(this.add(4));
            if flags & 1 != 0 {
                let page = (this as u32 & 0xFFFF_E000) as *mut u8;
                let next = read_unaligned(page.add(8).cast::<*mut u8>());
                write_unaligned(this.cast::<*mut u8>(), next);
                let count = read_unaligned(page.add(12).cast::<u32>()).wrapping_sub(1);
                write_unaligned(page.add(8).cast::<*mut u8>(), this);
                write_unaligned(page.add(12).cast(), count);
                if count == 0 { result = pool_empty(pool_context(), page); }
                else if count == 0x4000_0042 { result = pool_reorder(pool_context(), page); }
            }
            (result & 0xFFFF_FFFF_0000_0000) | u64::from(this as u32)
        }
    }
}

port! {
    /// Partitions large unsigned ranges using a median pivot, recurses right, then continues left.
    /// A zero depth switches to heap sorting; ranges of at most sixteen words are left alone.
    #[port(address = 0x0047_EA40, owner = "building", name = "UnsignedSortPartitions", naked)]
    pub unsafe extern "C" fn fc4p_0047ea40() {
        core::arch::naked_asm!(
            "lea eax, [esp + 16]", "push eax",
            "push dword ptr [esp + 24]", "push dword ptr [esp + 24]",
            "push dword ptr [esp + 24]", "push dword ptr [esp + 24]",
            "push dword ptr [esp + 24]", "call {body}", "add esp, 24", "ret",
            body = sym sort_partitions,
        );
    }
}

/// # Safety
/// The entry supplies its original sort arguments and writable depth word.
unsafe extern "C" fn sort_partitions(
    begin: *mut u32,
    end: *mut u32,
    _unused: u32,
    depth: u32,
    compare: u32,
    depth_word: *mut u32,
) -> u32 {
    let mut end = end;
    let mut depth = depth;
    let mut eax = (end as u32).wrapping_sub(begin as u32) & !3;
    // SAFETY: the game's sort supplies a valid word range and its helper arguments.
    unsafe {
        while (eax as i32) > 64 {
            if depth == 0 {
                return heap_sort(begin, end, end, 0, compare);
            }
            let words = (end as u32).wrapping_sub(begin as u32) as i32 >> 2;
            let middle = begin.wrapping_offset((words / 2) as isize);
            let last = end.wrapping_sub(1);
            let first_value = read_unaligned(begin);
            let middle_value = read_unaligned(middle);
            let last_value = read_unaligned(last);
            let pivot = if first_value < middle_value {
                if middle_value < last_value {
                    middle
                } else if first_value >= last_value {
                    begin
                } else {
                    last
                }
            } else if first_value < last_value {
                begin
            } else if middle_value >= last_value {
                middle
            } else {
                last
            };
            depth = depth.wrapping_sub(1);
            write_unaligned(depth_word, depth);
            let split = partition(begin, end, read_unaligned(pivot), compare);
            let returned = sort_part(split, end, 0, depth, compare);
            end = split;
            let distance = (end as u32).wrapping_sub(begin as u32) & !3;
            if (distance as i32) <= 64 {
                return returned;
            }
            eax = distance;
        }
    }
    eax
}

port! {
    /// Allocates twelve-byte elements and copy constructs the supplied range in order.
    #[port(address = 0x004E_4040, owner = "building", name = "AllocateObjectVectorCopy")]
    pub unsafe extern "thiscall" fn fc4p_004e4040(_this: *mut u8, count: u32, begin: *mut u8, end: *mut u8) -> *mut u8 {
        // SAFETY: the game supplies the source range and allocator/copy-constructor contracts.
        unsafe {
            let storage = if count == 0 { core::ptr::null_mut() } else { allocate(count.wrapping_mul(12)) };
            let mut source = begin;
            let mut dest = storage;
            while source != end {
                if !dest.is_null() { element_construct(dest, source); }
                dest = dest.wrapping_add(12);
                source = source.wrapping_add(12);
            }
            storage
        }
    }
}

port! {
    /// Captures EDX for self assignment, which preserves that incoming register.
    #[port(address = 0x004E_40A0, owner = "building", name = "AssignObjectVector", naked)]
    pub unsafe extern "C" fn fc4p_004e40a0() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push dword ptr [esp + 4]", "push eax",
            "call {body}", "ret 4", body = sym assign_vector,
        );
    }
}

/// # Safety
/// Both arguments refer to live vectors with twelve-byte elements.
unsafe extern "fastcall" fn assign_vector(
    this: *mut u8,
    entry_edx: u32,
    source_word: *mut u32,
    source: *mut u8,
) -> u64 {
    let mut edx = entry_edx;
    // SAFETY: the entry's contract and the original game helper contracts.
    unsafe {
        if this != source {
            let dest = ObjectVector::from_ptr(this);
            let src = ObjectVector::from_ptr(source);
            let src_end = src.end();
            let src_begin = src.begin();
            write_unaligned(source_word, src_end as u32);
            let count = ((src_end as u32).wrapping_sub(src_begin as u32) as i32 / 12) as u32;
            let old_begin = dest.begin();
            let capacity = ((dest.limit() as u32).wrapping_sub(old_begin as u32) as i32 / 12) as u32;
            let cookie = source_word.cast::<u8>();
            if count > capacity {
                let storage = allocate_copy(this, count, src_begin, src_end);
                destroy_range(dest.begin(), dest.end(), cookie);
                let old = dest.begin();
                if !old.is_null() {
                    free(old);
                }
                dest.set_limit(storage.wrapping_add(count.wrapping_mul(12) as usize));
                dest.set_begin(storage);
                let new_end = storage.wrapping_add(count.wrapping_mul(12) as usize);
                dest.set_end(new_end);
                edx = new_end as u32;
            } else {
                let length = ((dest.end() as u32).wrapping_sub(old_begin as u32) as i32 / 12) as u32;
                if length >= count {
                    let assigned_end = assign_range(src_begin, src_end, old_begin, cookie, 0);
                    destroy_range(assigned_end, dest.end(), cookie);
                } else {
                    assign_range(
                        src_begin,
                        src_begin.wrapping_add(length.wrapping_mul(12) as usize),
                        old_begin,
                        cookie,
                        0,
                    );
                    let old_end = dest.end();
                    let src_end = src.end();
                    let length = (old_end as u32).wrapping_sub(dest.begin() as u32) as i32 / 12;
                    let src_tail = src.begin().wrapping_add(length.wrapping_mul(12) as usize);
                    construct_range(src_tail, src_end, old_end, cookie);
                }
                let new_end = dest.begin().wrapping_add(count.wrapping_mul(12) as usize);
                dest.set_end(new_end);
                edx = new_end as u32;
            }
        }
    }
    (u64::from(edx) << 32) | u64::from(this as u32)
}

pub const PORTS: &[Port] = &[
    fc4p_00458a10::PORT,
    fc4p_00458a20::PORT,
    fc4p_00458a60::PORT,
    fc4p_00458e90::PORT,
    fc4p_00458f80::PORT,
    fc4p_0047ea40::PORT,
    fc4p_004e4040::PORT,
    fc4p_004e40a0::PORT,
];

#[cfg(test)]
mod tests;
