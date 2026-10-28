// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Shared helpers, group 04: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { #[allow(dead_code)] fn destroy_vehicle(this: *mut u8, flags: u32) -> u32 = "thiscall" 0x0053_CCE0; }
game_fn! { #[allow(dead_code)] fn retain_dispatch(this: *mut u8) -> u32 = "thiscall" 0x004B_D280; }
game_fn! { #[allow(dead_code)] fn release_dispatch(this: *mut u8) -> u32 = "thiscall" 0x004B_D290; }
game_fn! { #[allow(dead_code)] fn retain_effect(this: *mut u8) -> u32 = "thiscall" 0x0057_EB00; }
game_fn! { #[allow(dead_code)] fn release_effect(this: *mut u8) -> u32 = "thiscall" 0x0057_EB70; }
game_fn! { fn allocate_with_tag(size: u32, tag: u32) -> u64 = "C" 0x0090_D91C; }
game_fn! { fn free_vehicle_pool(this: *mut u8, storage: *mut u8) -> u32 = "thiscall" 0x0055_F600; }
game_fn! { fn free_vehicle(storage: *mut u8) -> u32 = "C" 0x005E_5620; }
game_fn! { fn free(storage: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn grow_deque_map(this: *mut u8, count: u32, front: u32) = "thiscall" 0x006D_B4A0; }
game_fn! { fn append_deque_block(this: *mut u8, item: *const u8) -> u32 = "thiscall" 0x0056_0250; }
game_fn! { fn insert_vector(this: *mut u8, position: *mut u8, value: *const u8, guard: *mut u8, count: u32, capacity: u32) -> u32 = "thiscall" 0x0043_0A90; }
game_data! { static vehicle_pool: u8 = 0x00B0_A0F8; }

// All field accesses use the game's storage, including fields a callback may change.
unsafe fn pointer(p: *mut u8, offset: usize) -> *mut u8 {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

unsafe fn put_pointer(p: *mut u8, offset: usize, value: *mut u8) {
    unsafe { write_unaligned(p.wrapping_add(offset).cast(), value) }
}

layout! {
    /// Iterators of a deque with sixteen eight-byte entries per block.
    struct EntryDeque {
        0x00 => first / set_first: *mut u8,
        0x04 => first_block / set_first_block: *mut u8,
        0x08 => first_limit / set_first_limit: *mut u8,
        0x0C => first_map / set_first_map: *mut u8,
        0x10 => last / set_last: *mut u8,
        0x14 => last_block / set_last_block: *mut u8,
        0x18 => last_limit / set_last_limit: *mut u8,
        0x1C => last_map / set_last_map: *mut u8,
        0x20 => map / set_map: *mut u8,
        0x24 => map_capacity / set_map_capacity: u32,
    }
}

// The forwarding entries retain every register and the original stack arguments.
macro_rules! adjusted_entry {
    ($export:ident, $address:literal, $adjust:literal, $target:literal, $name:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!("sub ecx, {adjust}", "push {target}", "ret",
                    adjust = const $adjust, target = const $target);
                #[cfg(test)]
                core::arch::naked_asm!("sub ecx, {adjust}", "push {target}", "jmp {resolve}",
                    adjust = const $adjust, target = const $target, resolve = sym forward_test_target);
            }
        }
    };
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn forward_test_target() {
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push [esp + 12]",
        "call {resolve}", "mov [esp + 12], eax", "pop edx", "pop ecx", "pop eax", "ret",
        resolve = sym resolve_test_target);
}

#[cfg(test)]
extern "stdcall" fn resolve_test_target(address: u32) -> usize {
    crate::abi::target(address)
}

adjusted_entry!(
    fc4p_0055b3a0,
    0x0055_B3A0,
    12,
    0x0053_CCE0,
    "DestroyVehicleFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_00565580,
    0x0056_5580,
    40,
    0x004B_D280,
    "RetainDispatchFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_00565590,
    0x0056_5590,
    40,
    0x004B_D290,
    "ReleaseDispatchFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_0057db80,
    0x0057_DB80,
    16,
    0x0057_EB00,
    "RetainEffectFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_0057db90,
    0x0057_DB90,
    16,
    0x0057_EB70,
    "ReleaseEffectFromSecondaryInterface"
);

port! {
    /// Allocates with tag 16; the caller's single argument is removed on return.
    #[port(address = 0x0055_B7C0, owner = "shared", name = "AllocateWithTag16")]
    pub unsafe extern "stdcall" fn fc4p_0055b7c0(size: u32) -> u64 {
        unsafe { allocate_with_tag(size, 16) }
    }
}

port! {
    /// Returns a vehicle allocation to the size-specific pool or the general allocator.
    #[port(address = 0x0055_CA60, owner = "shared", name = "FreeVehicleBySize")]
    pub unsafe extern "C" fn fc4p_0055ca60(storage: *mut u8, size: u32) -> u32 {
        unsafe {
            if storage.is_null() { 0 }
            else if size == 0x2B8 { free_vehicle_pool(vehicle_pool(), storage) }
            else { free_vehicle(storage) }
        }
    }
}

port! {
    /// Releases the first entry and advances the front iterator, freeing a completed block.
    #[port(address = 0x0055_FAF0, owner = "shared", name = "PopFrontEntryDeque")]
    pub unsafe extern "thiscall" fn fc4p_0055faf0(this: *mut u8) -> *mut u8 {
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            let at_end = deque.first() == deque.first_limit().wrapping_sub(8);
            let item = pointer(deque.first(), 0);
            if !item.is_null() { vcall!(item, 8 => fn() -> u32); }
            if !at_end {
                let next = deque.first().wrapping_add(8);
                deque.set_first(next);
                return next;
            }
            if !deque.first_block().is_null() { free(deque.first_block()); }
            let next_map = deque.first_map().wrapping_add(4);
            deque.set_first_map(next_map);
            let block = pointer(next_map, 0);
            deque.set_first_block(block);
            deque.set_first_limit(block.wrapping_add(0x80));
            let next = deque.first_block();
            deque.set_first(next);
            next
        }
    }
}

port! {
    /// Adds a new back block and constructs the final entry in the old block, retaining its interface.
    #[port(address = 0x0056_0250, owner = "shared", name = "AppendEntryDequeBlock")]
    pub unsafe extern "thiscall" fn fc4p_00560250(this: *mut u8, value: *const u8) -> u32 {
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            let item = pointer(value.cast_mut(), 0);
            if !item.is_null() { vcall!(item, 4 => fn() -> u32); }
            let payload = read_unaligned(value.add(4).cast::<u32>());
            let used = (deque.last_map() as u32).wrapping_sub(deque.map() as u32) as i32 >> 2;
            if deque.map_capacity().wrapping_sub(used as u32) < 2 { grow_deque_map(this, 1, 0); }
            let block = allocate(0x80);
            put_pointer(deque.last_map(), 4, block);
            let destination = deque.last();
            if !destination.is_null() {
                put_pointer(destination, 0, item);
                if !item.is_null() { vcall!(item, 4 => fn() -> u32); }
                write_unaligned(destination.add(4).cast::<u32>(), payload);
            }
            let next_map = deque.last_map().wrapping_add(4);
            deque.set_last_map(next_map);
            let block = pointer(next_map, 0);
            deque.set_last_block(block);
            deque.set_last_limit(block.wrapping_add(0x80));
            let next = deque.last_block();
            deque.set_last(next);
            if !item.is_null() { vcall!(item, 8 => fn() -> u32) } else { next as u32 }
        }
    }
}

port! {
    /// Appends an interface and payload, delegating the last slot of a block to the growth helper.
    #[port(address = 0x0056_14B0, owner = "shared", name = "AppendEntryDeque")]
    pub unsafe extern "thiscall" fn fc4p_005614b0(this: *mut u8, value: *const u8) -> u32 {
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            let limit = deque.last_limit().wrapping_sub(8);
            let destination = deque.last();
            if destination == limit { return append_deque_block(this, value); }
            let mut result = limit as u32;
            if !destination.is_null() {
                let item = pointer(value.cast_mut(), 0);
                put_pointer(destination, 0, item);
                if !item.is_null() { vcall!(item, 4 => fn() -> u32); }
                result = read_unaligned(value.add(4).cast::<u32>());
                write_unaligned(destination.add(4).cast::<u32>(), result);
            }
            deque.set_last(deque.last().wrapping_add(8));
            result
        }
    }
}

port! {
    /// Returns the address of the embedded instance collection.
    #[port(address = 0x0057_7450, owner = "shared", name = "GetInstances", naked)]
    pub unsafe extern "C" fn fc4p_00577450() {
        core::arch::naked_asm!("lea eax, [ecx + 12]", "ret");
    }
}

port! {
    /// Returns the fixed class identifier while retaining the other registers.
    #[port(address = 0x0057_F300, owner = "shared", name = "GetEffectClassId", naked)]
    pub unsafe extern "C" fn fc4p_0057f300() {
        core::arch::naked_asm!("mov eax, 0xc4c90997", "ret");
    }
}

port! {
    /// Reads the flag byte, retaining the caller's upper EAX bytes.
    #[port(address = 0x0058_0B50, owner = "shared", name = "GetFlagByte", naked)]
    pub unsafe extern "C" fn fc4p_00580b50() {
        core::arch::naked_asm!("mov al, [ecx + 0x1c]", "ret");
    }
}

port! {
    /// Appends one retained interface pointer, growing the vector when full.
    #[port(address = 0x0059_D7E0, owner = "shared", name = "AppendInterfaceVector")]
    pub unsafe extern "thiscall" fn fc4p_0059d7e0(this: *mut u8, value: *const *mut u8) -> u32 {
        unsafe {
            let end = pointer(this, 4);
            if end == pointer(this, 8) {
                let mut guard = 0u8;
                return insert_vector(this, end, value.cast(), &mut guard, 1, 1);
            }
            let mut result = end as u32;
            if !end.is_null() {
                let item = read_unaligned(value);
                put_pointer(end, 0, item);
                if !item.is_null() { result = vcall!(item, 4 => fn() -> u32); }
            }
            put_pointer(this, 4, pointer(this, 4).wrapping_add(4));
            result
        }
    }
}

port! {
    /// Returns the city's zone-manager interface without retaining it.
    #[port(address = 0x005A_4DE0, owner = "shared", name = "GetZoneManager", naked)]
    pub unsafe extern "C" fn fc4p_005a4de0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x134]", "ret");
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_0055b3a0::PORT,
    fc4p_0055b7c0::PORT,
    fc4p_0055ca60::PORT,
    fc4p_0055faf0::PORT,
    fc4p_00560250::PORT,
    fc4p_005614b0::PORT,
    fc4p_00565580::PORT,
    fc4p_00565590::PORT,
    fc4p_00577450::PORT,
    fc4p_0057db80::PORT,
    fc4p_0057db90::PORT,
    fc4p_0057f300::PORT,
    fc4p_00580b50::PORT,
    fc4p_0059d7e0::PORT,
    fc4p_005a4de0::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{call_thiscall_with_eax, fake, fake_data, Object};
    use std::cell::RefCell;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Call {
        Retain(usize),
        Release(usize),
        Free(usize),
        Allocate(u32),
        Tag(u32, u32),
        VehiclePool(usize, usize),
        VehicleFree(usize),
        Forward(usize, u32),
        GrowMap(usize, u32, u32),
        AppendBlock(usize, usize),
        Insert(usize, usize, usize, u8, u32, u32),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static ALLOCATION: RefCell<usize> = const { RefCell::new(0) };
        // A call may change one word in the caller's object or source entry.
        static MUTATION: RefCell<Option<(Call, usize, u32)>> = const { RefCell::new(None) };
        static WATCH: RefCell<Option<(usize, usize)>> = const { RefCell::new(None) };
        static SNAPSHOTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    }

    fn log(call: Call) {
        WATCH.with(|w| {
            if let Some((p, n)) = *w.borrow() {
                let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, n) }.to_vec();
                SNAPSHOTS.with(|s| s.borrow_mut().push(bytes));
            }
        });
        MUTATION.with(|m| {
            let mut m = m.borrow_mut();
            if let Some((at, address, value)) = m.as_ref() {
                if *at == call {
                    unsafe { write_unaligned(*address as *mut u32, *value) };
                    *m = None;
                }
            }
        });
        CALLS.with(|c| c.borrow_mut().push(call));
    }

    fn take_calls() -> Vec<Call> {
        CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
    }

    extern "thiscall" fn retain(this: *mut u8) -> u64 {
        log(Call::Retain(this as usize));
        0x9876_5432_AABB_CCDD
    }
    extern "thiscall" fn release(this: *mut u8) -> u64 {
        log(Call::Release(this as usize));
        0x8765_4321_1122_3344
    }
    extern "C" fn delete(p: *mut u8) -> u64 {
        log(Call::Free(p as usize));
        0x7654_3210_1234_5678
    }
    extern "C" fn allocation(size: u32) -> *mut u8 {
        log(Call::Allocate(size));
        ALLOCATION.with(|p| *p.borrow() as *mut u8)
    }
    extern "C" fn tag(size: u32, tag: u32) -> u64 {
        log(Call::Tag(size, tag));
        0xABC0_9876_5432_1000
    }
    extern "thiscall" fn pooled(this: *mut u8, p: *mut u8) -> u32 {
        log(Call::VehiclePool(this as usize, p as usize));
        0x1234_ABCD
    }
    extern "C" fn vehicle_delete(p: *mut u8) -> u32 {
        log(Call::VehicleFree(p as usize));
        0xABCD_1234
    }
    extern "thiscall" fn forwarded(this: *mut u8) -> u64 {
        log(Call::Forward(this as usize, 0));
        0x1122_3344_5566_7788
    }
    extern "thiscall" fn forwarded_flag(this: *mut u8, flag: u32) -> u64 {
        log(Call::Forward(this as usize, flag));
        0x1122_3344_5566_7788
    }
    extern "thiscall" fn map_grow(this: *mut u8, count: u32, front: u32) {
        log(Call::GrowMap(this as usize, count, front));
    }
    extern "thiscall" fn block_append(this: *mut u8, value: *const u8) -> u32 {
        log(Call::AppendBlock(this as usize, value as usize));
        0xFEDC_BA98
    }
    extern "thiscall" fn vector_insert(
        this: *mut u8,
        at: *mut u8,
        value: *const u8,
        guard: *mut u8,
        count: u32,
        capacity: u32,
    ) -> u32 {
        log(Call::Insert(
            this as usize,
            at as usize,
            value as usize,
            unsafe { *guard },
            count,
            capacity,
        ));
        0x1357_9BDF
    }

    fn table() -> [usize; 3] {
        [0, retain as *const () as usize, release as *const () as usize]
    }
    fn interface(table: &[usize]) -> [usize; 1] {
        [table.as_ptr() as usize]
    }
    unsafe fn bytes(p: *mut u8, n: usize) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(p, n) }.to_vec()
    }
    fn mutation(call: Call, p: *mut u8, value: u32) {
        MUTATION.with(|m| *m.borrow_mut() = Some((call, p as usize, value)));
    }

    #[test]
    fn adjusted_entries_forward_the_same_arguments_and_results() {
        let mut object = Object::new(64);
        for (entry, target, offset, flag) in [
            (fc4p_0055b3a0 as *const () as usize, 0x0053_CCE0, 12, Some(0xF123_4567)),
            (fc4p_00565580 as *const () as usize, 0x004B_D280, 40, None),
            (fc4p_00565590 as *const () as usize, 0x004B_D290, 40, None),
            (fc4p_0057db80 as *const () as usize, 0x0057_EB00, 16, None),
            (fc4p_0057db90 as *const () as usize, 0x0057_EB70, 16, None),
        ] {
            let f = if flag.is_some() {
                forwarded_flag as *const () as usize
            } else {
                forwarded as *const () as usize
            };
            let _fake = fake(target, f);
            let args: Vec<_> = flag.into_iter().collect();
            let result = unsafe { call_thiscall_with_eax(entry, object.ptr().add(offset), &args, 0xDEAD_BEEF) };
            assert_eq!(result, (0x5566_7788, 0x1122_3344));
            assert_eq!(take_calls(), [Call::Forward(object.ptr() as usize, flag.unwrap_or(0))]);
        }
    }

    #[test]
    fn small_getters_preserve_upper_bytes_edx_and_memory() {
        let mut memory = Object::new(0x140);
        let p = memory.ptr();
        unsafe {
            core::ptr::write_bytes(p, 0xA5, 0x140);
            put_pointer(p, 0x134, 0x1234_5678usize as *mut u8);
        }
        let before = unsafe { bytes(p, 0x140) };
        for (entry, want) in [
            (fc4p_00577450 as *const () as usize, (p as u32).wrapping_add(12)),
            (fc4p_0057f300 as *const () as usize, 0xC4C9_0997),
            (fc4p_00580b50 as *const () as usize, 0x1234_56A5),
            (fc4p_005a4de0 as *const () as usize, 0x1234_5678),
        ] {
            let mut call = RawCall::new(Conv::Thiscall, entry, p as u32, &[]);
            call.eax = 0x1234_5678;
            call.edx = 0xBEEF_1234;
            unsafe {
                call_raw(&mut call);
            }
            assert_eq!((call.eax, call.edx), (want, 0xBEEF_1234));
            assert_eq!(unsafe { bytes(p, 0x140) }, before);
        }
        for flag in [0, 1, 0x7F, 0x80, 0xFF] {
            unsafe {
                *p.add(0x1C) = flag;
            }
            assert_eq!(
                unsafe { call_thiscall_with_eax(fc4p_00580b50 as *const () as usize, p, &[], 0xDEAD_BE00).0 },
                0xDEAD_BE00 | u32::from(flag)
            );
        }
    }

    #[test]
    fn tagged_allocation_and_size_specific_vehicle_free() {
        let _tag = fake(0x0090_D91C, tag as *const () as usize);
        let _pool = fake(0x0055_F600, pooled as *const () as usize);
        let _free = fake(0x005E_5620, vehicle_delete as *const () as usize);
        let mut pool = 0u32;
        let _global = fake_data(0x00B0_A0F8, (&mut pool as *mut u32).cast());
        for size in [0, 1, 0x2B8, u32::MAX] {
            assert_eq!(unsafe { fc4p_0055b7c0(size) }, 0xABC0_9876_5432_1000);
            assert_eq!(take_calls(), [Call::Tag(size, 16)]);
        }
        let storage = 0x8765_4321usize as *mut u8;
        for size in [0, 0x2B7, 0x2B8, 0x2B9, u32::MAX] {
            assert_eq!(unsafe { fc4p_0055ca60(core::ptr::null_mut(), size) }, 0);
            assert!(take_calls().is_empty());
            let result = unsafe { fc4p_0055ca60(storage, size) };
            if size == 0x2B8 {
                assert_eq!(result, 0x1234_ABCD);
                assert_eq!(
                    take_calls(),
                    [Call::VehiclePool((&mut pool as *mut u32) as usize, storage as usize)]
                );
            } else {
                assert_eq!(result, 0xABCD_1234);
                assert_eq!(take_calls(), [Call::VehicleFree(storage as usize)]);
            }
        }
    }

    #[test]
    fn deque_front_advances_or_retires_one_block_and_rereads_after_release() {
        let _free = fake(0x0090_CF63, delete as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        for boundary in [false, true] {
            for present in [false, true] {
                let mut memory = Object::new(0x28);
                let mut block = Object::new(0x80);
                let mut next = Object::new(0x80);
                let mut map = [block.ptr(), next.ptr()];
                let p = memory.ptr();
                let d = unsafe { EntryDeque::from_ptr(p) };
                let at = block.ptr().wrapping_add(if boundary { 0x78 } else { 0x10 });
                unsafe {
                    put_pointer(at, 0, if present { ip } else { core::ptr::null_mut() });
                }
                d.set_first(at);
                d.set_first_block(block.ptr());
                d.set_first_limit(block.ptr().wrapping_add(0x80));
                d.set_first_map(map.as_mut_ptr().cast());
                let mut expected = unsafe { bytes(p, 0x28) };
                let want = if boundary { next.ptr() } else { at.wrapping_add(8) };
                expected[..4].copy_from_slice(&(want as u32).to_le_bytes());
                if boundary {
                    expected[4..8].copy_from_slice(&(next.ptr() as u32).to_le_bytes());
                    expected[8..12].copy_from_slice(&(next.ptr().wrapping_add(0x80) as u32).to_le_bytes());
                    expected[12..16].copy_from_slice(&(map.as_mut_ptr().wrapping_add(1) as u32).to_le_bytes());
                }
                assert_eq!(unsafe { fc4p_0055faf0(p) }, want);
                let mut calls = Vec::new();
                if present {
                    calls.push(Call::Release(ip as usize));
                }
                if boundary {
                    calls.push(Call::Free(block.ptr() as usize));
                }
                assert_eq!(take_calls(), calls);
                assert_eq!(unsafe { bytes(p, 0x28) }, expected);
            }
        }
        let mut memory = Object::new(0x28);
        let mut block = Object::new(0x80);
        let d = unsafe { EntryDeque::from_ptr(memory.ptr()) };
        d.set_first(block.ptr());
        d.set_first_limit(block.ptr().wrapping_add(0x80));
        unsafe {
            put_pointer(block.ptr(), 0, ip);
        }
        mutation(
            Call::Release(ip as usize),
            memory.ptr(),
            block.ptr().wrapping_add(0x20) as u32,
        );
        assert_eq!(unsafe { fc4p_0055faf0(memory.ptr()) }, block.ptr().wrapping_add(0x28));
        assert_eq!(take_calls(), [Call::Release(ip as usize)]);
    }

    #[test]
    fn deque_append_reads_payload_after_retain_and_delegates_at_boundary() {
        let _grow = fake(0x0056_0250, block_append as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        let mut memory = Object::new(0x28);
        let mut block = Object::new(0x80);
        let d = unsafe { EntryDeque::from_ptr(memory.ptr()) };
        d.set_last(block.ptr());
        d.set_last_limit(block.ptr().wrapping_add(0x80));
        let mut value = [ip as u32, 0x1122_3344];
        mutation(
            Call::Retain(ip as usize),
            value.as_mut_ptr().wrapping_add(1).cast(),
            0x5566_7788,
        );
        assert_eq!(
            unsafe { fc4p_005614b0(memory.ptr(), value.as_ptr().cast()) },
            0x5566_7788
        );
        assert_eq!(take_calls(), [Call::Retain(ip as usize)]);
        assert_eq!(
            unsafe { read_unaligned(block.ptr().cast::<[u32; 2]>()) },
            [ip as u32, 0x5566_7788]
        );
        assert_eq!(d.last(), block.ptr().wrapping_add(8));
        value = [0, 0xFEDC_BA98];
        assert_eq!(unsafe { fc4p_005614b0(memory.ptr(), value.as_ptr().cast()) }, value[1]);
        assert!(take_calls().is_empty());
        d.set_last(block.ptr().wrapping_add(0x78));
        assert_eq!(
            unsafe { fc4p_005614b0(memory.ptr(), value.as_ptr().cast()) },
            0xFEDC_BA98
        );
        assert_eq!(
            take_calls(),
            [Call::AppendBlock(memory.ptr() as usize, value.as_ptr() as usize)]
        );
        d.set_last(core::ptr::null_mut());
        assert_eq!(
            unsafe { fc4p_005614b0(memory.ptr(), core::ptr::null()) },
            block.ptr().wrapping_add(0x78) as u32
        );
        assert_eq!(d.last(), 8usize as *mut u8);
    }

    #[test]
    fn deque_block_append_retains_temporary_and_grows_map_before_allocation() {
        let _allocate = fake(0x0090_CF54, allocation as *const () as usize);
        let _grow = fake(0x006D_B4A0, map_grow as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        for (capacity, present, destination_present) in
            [(3, true, true), (2, true, true), (3, false, true), (3, true, false)]
        {
            let mut memory = Object::new(0x28);
            let mut block = Object::new(0x80);
            let mut next = Object::new(0x80);
            let mut map = [block.ptr(), core::ptr::null_mut(), core::ptr::null_mut()];
            let p = memory.ptr();
            let d = unsafe { EntryDeque::from_ptr(p) };
            d.set_map(map.as_mut_ptr().cast());
            d.set_last_map(map.as_mut_ptr().wrapping_add(1).cast());
            d.set_map_capacity(capacity);
            let destination = if destination_present {
                block.ptr().wrapping_add(0x78)
            } else {
                core::ptr::null_mut()
            };
            d.set_last(destination);
            let mut value = [if present { ip as u32 } else { 0 }, 123];
            if present {
                mutation(
                    Call::Retain(ip as usize),
                    value.as_mut_ptr().wrapping_add(1).cast(),
                    456,
                );
            }
            ALLOCATION.with(|a| *a.borrow_mut() = next.ptr() as usize);
            let result = unsafe { fc4p_00560250(p, value.as_ptr().cast()) };
            let mut calls = Vec::new();
            if present {
                calls.push(Call::Retain(ip as usize));
            }
            if capacity == 2 {
                calls.push(Call::GrowMap(p as usize, 1, 0));
            }
            calls.push(Call::Allocate(0x80));
            if present && destination_present {
                calls.push(Call::Retain(ip as usize));
            }
            if present {
                calls.push(Call::Release(ip as usize));
            }
            assert_eq!(take_calls(), calls);
            assert_eq!(result, if present { 0x1122_3344 } else { next.ptr() as u32 });
            assert_eq!(map[2], next.ptr());
            assert_eq!(
                (d.last(), d.last_block(), d.last_limit(), d.last_map()),
                (
                    next.ptr(),
                    next.ptr(),
                    next.ptr().wrapping_add(0x80),
                    map.as_mut_ptr().wrapping_add(2).cast()
                )
            );
            if destination_present {
                assert_eq!(
                    unsafe { read_unaligned(destination.cast::<[u32; 2]>()) },
                    [value[0], if present { 456 } else { 123 }]
                );
            }
        }
    }

    #[test]
    fn vector_append_writes_before_retain_rereads_end_and_grows_with_zero_guard() {
        let _insert = fake(0x0043_0A90, vector_insert as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        let mut values = [0u32; 4];
        let mut vector = [
            values.as_mut_ptr() as usize,
            values.as_mut_ptr() as usize,
            values.as_mut_ptr().wrapping_add(4) as usize,
        ];
        let vp = vector.as_mut_ptr().cast::<u8>();
        WATCH.with(|w| *w.borrow_mut() = Some((values.as_mut_ptr() as usize, 16)));
        mutation(
            Call::Retain(ip as usize),
            unsafe { vp.add(4) },
            values.as_mut_ptr().wrapping_add(1) as u32,
        );
        assert_eq!(unsafe { fc4p_0059d7e0(vp, &ip) }, 0xAABB_CCDD);
        WATCH.with(|w| *w.borrow_mut() = None);
        assert_eq!(take_calls(), [Call::Retain(ip as usize)]);
        SNAPSHOTS.with(|s| assert_eq!(s.borrow_mut().pop().unwrap()[..4], (ip as u32).to_le_bytes()));
        assert_eq!(vector[1], values.as_mut_ptr().wrapping_add(2) as usize);
        let null = core::ptr::null_mut();
        unsafe {
            fc4p_0059d7e0(vp, &null);
        }
        assert_eq!(values[2], 0);
        assert!(take_calls().is_empty());
        vector[1] = vector[2];
        assert_eq!(unsafe { fc4p_0059d7e0(vp, &ip) }, 0x1357_9BDF);
        assert_eq!(
            take_calls(),
            [Call::Insert(vp as usize, vector[2], &ip as *const _ as usize, 0, 1, 1)]
        );
        vector[1] = 0;
        vector[2] = 4;
        unsafe {
            fc4p_0059d7e0(vp, core::ptr::null());
        }
        assert_eq!(vector[1], 4);
    }
}
