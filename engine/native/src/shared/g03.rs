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

//! Shared helpers, group 03: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

// Some helpers leave a callee's EDX visible, including when that callee changes only a byte.
fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

unsafe fn word(object: *mut u8, offset: usize) -> u32 {
    // SAFETY: the enclosing port supplies a readable original field.
    unsafe { read_unaligned(object.wrapping_add(offset).cast()) }
}

unsafe fn store(object: *mut u8, offset: usize, value: u32) {
    // SAFETY: the enclosing port supplies a writable original field.
    unsafe { write_unaligned(object.wrapping_add(offset).cast(), value) }
}

unsafe fn invoke(conv: Conv, target: usize, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> u64 {
    let mut call = RawCall::new(conv, target, this as u32, args);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: the enclosing port supplies the original target, register inputs and argument words.
    unsafe { call_raw(&mut call) };
    registers(call.eax, call.edx)
}

unsafe fn virtual_call(this: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> u64 {
    // SAFETY: the enclosing port supplies a live object and its original virtual signature.
    unsafe { invoke(Conv::Thiscall, vslot(this, slot), this, eax, edx, args) }
}

game_fn! { #[allow(dead_code)] fn destroy_bus(this: *mut u8) = "thiscall" 0x0050_4E70; }
game_fn! { #[allow(dead_code)] fn destroy_ambulance(this: *mut u8) = "thiscall" 0x0050_4EF0; }
game_fn! { #[allow(dead_code)] fn destroy_train(this: *mut u8) = "thiscall" 0x0053_CB80; }
game_fn! { fn free_sized_bus(object: *mut u8, size: u32) = "C" 0x0055_CA60; }
game_fn! { #[allow(dead_code)] fn heap_free(object: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { #[allow(dead_code)] fn array_free(object: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn destroy_controller_list(this: *mut u8) -> u64 = "thiscall" 0x0056_50A0; }
game_fn! { #[allow(dead_code)] fn destroy_controller(this: *mut u8) -> u64 = "thiscall" 0x0051_7F80; }
game_fn! { fn write_vehicle(this: *mut u8, stream: *mut u8) -> u32 = "thiscall" 0x0055_F640; }
game_fn! { fn remove_automaton(this: *mut u8, first: u32, second: u32) -> u32 = "thiscall" 0x0050_C990; }
game_fn! { #[allow(dead_code)] fn message_allocate(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { #[allow(dead_code)] fn message_construct(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { #[allow(dead_code)] fn find_source_count(this: *mut u8, out: *mut u32, key: *const u32) -> u64 = "thiscall" 0x006F_1530; }
game_fn! { fn find_group(this: *mut u8, out: *mut u32, key: *const u32) = "thiscall" 0x0049_7520; }
game_fn! { #[allow(dead_code)] fn release_mesh_storage(this: *mut u8, data: u32, size: u32) -> u64 = "thiscall" 0x0061_4380; }
game_fn! { #[allow(dead_code)] fn free_mesh(object: *mut u8, size: u32) -> u64 = "C" 0x0080_0EC0; }
game_fn! { fn erase_string_tree(this: *mut u8, node: *mut u8) = "thiscall" 0x0051_1900; }
game_fn! { fn allocate_array(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn begin_pointer_set(this: *mut u8, out: *mut u32) -> u64 = "thiscall" 0x004B_0390; }
game_fn! { #[allow(dead_code)] fn destroy_pointer_set(this: *mut u8) -> u64 = "thiscall" 0x008E_BA60; }
game_fn! { fn reset_object_collection(this: *mut u8, count: u32) = "thiscall" 0x0080_38D0; }
game_fn! { fn insert_triple(this: *mut u8, position: u32, value: *mut u8, out: *mut u32, count: u32, stride: u32) -> u32 = "thiscall" 0x0050_C3B0; }

game_data! { static source_building_service: *mut u8 = 0x00B4_3D28; }

port! {
    /// Copies a twelve-byte-element vector into newly allocated storage. ECX and EDX both retain the
    /// source's end pointer; the entry marshals that extra ECX result after the Rust body returns.
    #[port(address = 0x0050_BCB0, owner = "shared", name = "CopyTripleVector", naked)]
    pub unsafe extern "C" fn fc4p_0050bcb0() {
        core::arch::naked_asm!("push [esp + 4]", "call {body}", "mov ecx, edx", "ret 4", body = sym copy_triple_vector);
    }
}

unsafe extern "thiscall" fn copy_triple_vector(this: *mut u8, source: *mut u8) -> u64 {
    // SAFETY: both vectors use three pointer words and twelve-byte elements. The game's allocator
    // supplies storage. Read source bounds again after allocation, including when source aliases this.
    unsafe {
        let count = (word(source, 4).wrapping_sub(word(source, 0)) as i32) / 12;
        store(this, 0, 0);
        store(this, 4, 0);
        store(this, 8, 0);
        let size = (count as u32).wrapping_mul(12);
        let allocation = if count == 0 {
            core::ptr::null_mut()
        } else {
            allocate_array(size)
        };
        let mut destination = allocation;
        store(this, 0, allocation as u32);
        store(this, 4, allocation as u32);
        store(this, 8, (allocation as u32).wrapping_add(size));
        let end = word(source, 4);
        let mut cursor = word(source, 0);
        while cursor != end {
            if !destination.is_null() {
                for offset in [0, 4, 8] {
                    store(destination, offset, word(cursor as *mut u8, offset));
                }
            }
            cursor = cursor.wrapping_add(12);
            destination = destination.wrapping_add(12);
        }
        store(this, 4, destination as u32);
        registers(this as u32, end)
    }
}

port! {
    /// Releases every nonnull member in hash iteration order, then destroys the pointer set.
    /// Successor links and bucket bounds are read after each Release, so callback changes are visible.
    #[port(address = 0x0050_CDE0, owner = "shared", name = "ReleasePointerSet")]
    pub unsafe extern "stdcall" fn fc4p_0050cde0(set: *mut u8) -> u64 {
        // SAFETY: the original hash set, its two-word iterator, and its retained member interfaces.
        unsafe {
            let mut iterator = MaybeUninit::<[u32; 2]>::uninit();
            let mut result = begin_pointer_set(set, iterator.as_mut_ptr().cast());
            let [mut node, table] = iterator.assume_init();
            let table = table as *mut u8;
            while node != 0 {
                let retained = word(node as *mut u8, 4) as *mut u8;
                if !retained.is_null() {
                    result = virtual_call(retained, 8, result as u32, word(retained, 0), &[]);
                }
                let next = word(node as *mut u8, 0);
                if next != 0 {
                    node = next;
                    result = registers(next, (result >> 32) as u32);
                } else {
                    let begin = word(table, 4);
                    let count = (word(table, 8).wrapping_sub(begin) as i32 >> 2) as u32;
                    let mut bucket = word(node as *mut u8, 4) % count;
                    let count_after = (word(table, 8).wrapping_sub(begin) as i32 >> 2) as u32;
                    node = 0;
                    loop {
                        bucket = bucket.wrapping_add(1);
                        if bucket >= count_after { break; }
                        node = word(word(table, 4).wrapping_add(bucket.wrapping_mul(4)) as *mut u8, 0);
                        if node != 0 { break; }
                    }
                    result = registers(count_after, bucket);
                }
            }
            invoke(Conv::Thiscall, crate::abi::target(0x008E_BA60), set,
                result as u32, (result >> 32) as u32, &[])
        }
    }
}

port! {
    /// Clears the object collection, releases every virtual-base interface in the pointer vector,
    /// resets both vector ends and calls the final owner hook. Only AL is replaced by one.
    #[port(address = 0x0050_D3C0, owner = "shared", name = "ClearOwnerObjectCollections")]
    pub unsafe extern "thiscall" fn fc4p_0050d3c0(this: *mut u8) -> u64 {
        // SAFETY: the original owner and its virtual-base interface layout.
        unsafe {
            if this.add(0x33).read() & 2 != 0 { vcall!(this, 0x11C => fn(u32), 0); }
            reset_object_collection(this.add(0x12C), 0);
            let mut cursor = word(this, 0x110);
            while cursor != word(this, 0x114) {
                let entry = word(cursor as *mut u8, 0) as *mut u8;
                if !entry.is_null() {
                    let base_table = word(entry, 4) as *mut u8;
                    let adjustment = word(base_table, 4);
                    let base = entry.wrapping_add(4).wrapping_add(adjustment as usize);
                    vcall!(base, 8 => fn());
                }
                cursor = cursor.wrapping_add(4);
            }
            store(this, 0x114, word(this, 0x110));
            store(this, 0xC4, word(this, 0xC0));
            let result = virtual_call(this, 0x1A8, word(this, 0), 0, &[]);
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Appends a twelve-byte element, calling the game's insertion helper only when full. The entry
    /// passes the caller's original argument slot because insertion can overwrite that output word.
    #[port(address = 0x0050_DA30, owner = "shared", name = "AppendTriple", naked)]
    pub unsafe extern "C" fn fc4p_0050da30() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {body}", "ret 4", body = sym append_triple);
    }
}

unsafe extern "fastcall" fn append_triple(this: *mut u8, argument: *mut u32) -> u32 {
    // SAFETY: the entry supplies the caller's original source-pointer word and three-word vector.
    unsafe {
        let end = word(this, 4);
        if end == word(this, 8) {
            return insert_triple(this, end, argument.read() as *mut u8, argument, 1, 1);
        }
        if end != 0 {
            let source = argument.read() as *mut u8;
            for offset in [0, 4, 8] {
                store(end as *mut u8, offset, word(source, offset));
            }
        }
        store(this, 4, word(this, 4).wrapping_add(12));
        end
    }
}

port! {
    /// Sends one standard message to each nonnull observer, taking each successor before the callback.
    /// Empty lists allocate nothing. The sentinel is read again after every callback.
    #[port(address = 0x0050_B540, owner = "shared", name = "NotifyObservers")]
    pub unsafe extern "fastcall" fn fc4p_0050b540(this: *mut u8, entry_edx: u32, first: u32, second: u32) -> u64 {
        // SAFETY: the original owner, observer list, message allocator and virtual signatures.
        unsafe {
            let header = word(this, 0x188);
            if word(header as *mut u8, 0) == header { return registers(header, entry_edx); }
            let allocation = invoke(Conv::Cdecl, crate::abi::target(0x0091_33DA), this, header, entry_edx, &[0x2C]);
            let constructed = if allocation as u32 == 0 { allocation } else {
                invoke(Conv::Thiscall, crate::abi::target(0x0091_34D6), allocation as u32 as *mut u8,
                    allocation as u32, (allocation >> 32) as u32, &[])
            };
            let message = constructed as u32 as *mut u8;
            let result = virtual_call(message, 4, word(message, 0), (constructed >> 32) as u32, &[]);
            let result = virtual_call(message, 0x14, result as u32, word(message, 0), &[0x09D9_2FC3]);
            virtual_call(message, 0x30, word(message, 0), (result >> 32) as u32, &[this as u32]);
            virtual_call(message, 0x40, first, word(message, 0), &[first]);
            let mut result = virtual_call(message, 0x54, second, word(message, 0), &[second]);
            let header = word(this, 0x188);
            let mut node = word(header as *mut u8, 0);
            result = registers(header, (result >> 32) as u32);
            while node != word(this, 0x188) {
                let observer = word(node as *mut u8, 8) as *mut u8;
                node = word(node as *mut u8, 0);
                if !observer.is_null() {
                    result = virtual_call(observer, 0x0C, result as u32, word(observer, 0), &[message as u32]);
                }
            }
            virtual_call(message, 8, word(message, 0), (result >> 32) as u32, &[])
        }
    }
}

port! {
    /// Returns the source-building count for a group. Groups 0x1402 and 0x1406 use the global service;
    /// the others look up a hash node and read its value at +8. Missing services and nodes yield zero.
    #[port(address = 0x0050_BE30, owner = "shared", name = "GetSourceBuildingCount")]
    pub unsafe extern "fastcall" fn fc4p_0050be30(this: *mut u8, entry_edx: u32, group: u32) -> u64 {
        // SAFETY: the game's manager, global service, hash lookup and returned node layout.
        unsafe {
            if group == 0x1402 || group == 0x1406 {
                let service = source_building_service().read();
                if service.is_null() { return registers(0, entry_edx); }
                return virtual_call(service, 0x2C, word(service, 0), entry_edx, &[]);
            }
            let mut output = MaybeUninit::<[u32; 2]>::uninit();
            let output = output.as_mut_ptr().cast::<u32>();
            let result = invoke(Conv::Thiscall, crate::abi::target(0x006F_1530), this.add(0xA64),
                &group as *const u32 as u32, output as u32, &[output as u32, &group as *const u32 as u32]);
            let node = output.read() as *mut u8;
            registers(if node.is_null() { 0 } else { word(node, 8) }, (result >> 32) as u32)
        }
    }
}

port! {
    /// Reports whether the group lookup returned a node other than the manager's current tree sentinel.
    /// EDX retains that returned node, even if the lookup has replaced the sentinel.
    #[port(address = 0x0050_C300, owner = "shared", name = "CanCreateGroup")]
    pub unsafe extern "thiscall" fn fc4p_0050c300(this: *mut u8, group: u32) -> u64 {
        let mut node = this as u32;
        // SAFETY: the original manager's group tree and single-word iterator output.
        unsafe {
            find_group(this.add(0xA24), &mut node, &group);
            registers(u32::from(node != word(this, 0xA24)), node)
        }
    }
}

port! {
    /// Decrements the unsigned 16-bit count when it exceeds one. Counts zero and one destroy storage
    /// when present, then free the 0x30-byte mesh without changing the count field, and return zero.
    #[port(address = 0x0050_C660, owner = "shared", name = "ReleaseMesh")]
    pub unsafe extern "fastcall" fn fc4p_0050c660(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the game's mesh fields and original storage/free signatures.
        unsafe {
            let count = this.add(0x2C).cast::<u16>().read_unaligned();
            if count > 1 {
                this.add(0x2C).cast::<u16>().write_unaligned(count - 1);
                return registers(u32::from(count - 1), entry_edx);
            }
            let data = word(this, 8);
            let result = if data == 0 { registers(0, entry_edx) } else {
                let size = word(this, 0x10).wrapping_sub(data);
                invoke(Conv::Thiscall, crate::abi::target(0x0061_4380), this.add(0x10), data, size, &[data, size])
            };
            let result = invoke(Conv::Cdecl, crate::abi::target(0x0080_0EC0), this,
                result as u32, (result >> 32) as u32, &[this as u32, 0x30]);
            result & 0xFFFF_FFFF_0000_0000
        }
    }
}

port! {
    /// Calls each nonnull vector entry's first slot with the owner, then frees it. The end is reloaded
    /// after each entry; clearing writes only the end pointer. EAX retains the previous end.
    #[port(address = 0x0050_D510, owner = "shared", name = "DestroyOwnedVectorEntries")]
    pub unsafe extern "fastcall" fn fc4p_0050d510(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original owner's pointer vector and entry destructor/free signatures.
        unsafe {
            let mut cursor = word(this, 0xB4);
            let mut edx = entry_edx;
            while cursor != word(this, 0xB8) {
                let entry = word(cursor as *mut u8, 0) as *mut u8;
                if !entry.is_null() {
                    let result = virtual_call(entry, 0, word(entry, 0), edx, &[this as u32]);
                    let result = invoke(Conv::Cdecl, crate::abi::target(0x005E_5620), this,
                        result as u32, (result >> 32) as u32, &[entry as u32]);
                    edx = (result >> 32) as u32;
                }
                cursor = cursor.wrapping_add(4);
            }
            let previous_end = word(this, 0xB8);
            store(this, 0xB8, word(this, 0xB4));
            registers(previous_end, edx)
        }
    }
}

port! {
    /// Erases right subtrees recursively through the original entry, then each node's two strings,
    /// then the node allocation, continuing down the saved left link. Null buffers are not freed.
    #[port(address = 0x0051_1900, owner = "shared", name = "EraseStringTree")]
    pub unsafe extern "thiscall" fn fc4p_00511900(this: *mut u8, node: *mut u8) {
        let mut node = node;
        // SAFETY: the original tree's node links and two embedded string layouts.
        unsafe {
            while !node.is_null() {
                erase_string_tree(this, word(node, 0x0C) as *mut u8);
                let left = word(node, 8) as *mut u8;
                store(node, 0x2C, 0x00A8_0810);
                let buffer = word(node, 0x30) as *mut u8;
                if !buffer.is_null() { array_free(buffer); }
                store(node, 0x10, 0x00A8_0810);
                let buffer = word(node, 0x14) as *mut u8;
                if !buffer.is_null() { array_free(buffer); }
                array_free(node);
                node = left;
            }
        }
    }
}

port! {
    /// Erases a nonempty string tree, restores its sentinel links, frees the sentinel if present,
    /// and releases the retained interface. The sentinel pointer and retained pointer are not cleared.
    #[port(address = 0x0051_3E00, owner = "shared", name = "DestroyStringTreeOwner")]
    pub unsafe extern "fastcall" fn fc4p_00513e00(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original owner, tree sentinel, and retained interface.
        unsafe {
            let count = word(this, 0x0C);
            let mut result = registers(count, entry_edx);
            if count != 0 {
                let header = word(this, 8) as *mut u8;
                erase_string_tree(this.add(8), word(header, 4) as *mut u8);
                let header = word(this, 8);
                store(header as *mut u8, 8, header);
                let header = word(this, 8);
                store(header as *mut u8, 4, 0);
                let last_header = word(this, 8);
                store(last_header as *mut u8, 0x0C, last_header);
                store(this, 0x0C, 0);
                result = registers(last_header, header);
            }
            let header = word(this, 8);
            if header != 0 {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this,
                    result as u32, (result >> 32) as u32, &[header]);
            }
            let retained = word(this, 4) as *mut u8;
            if retained.is_null() { result } else {
                virtual_call(retained, 8, word(retained, 0), (result >> 32) as u32, &[])
            }
        }
    }
}

layout! {
    /// Controller fields used by its destructor, from the primary interface.
    struct Controller {
        0x00 => primary_table / set_primary_table: u32,
        0x04 => secondary_table / set_secondary_table: u32,
        0x0C => lua / set_lua: *mut u8,
        0x28 => string_table / set_string_table: u32,
        0x2C => string_buffer / set_string_buffer: *mut u8,
        0x4C => retained / set_retained: *mut u8,
    }
}

port! {
    /// Returns the pointer at +0x0C without changing other registers.
    #[port(address = 0x0051_7F70, owner = "shared", name = "GetLuaPointer", naked)]
    pub unsafe extern "C" fn fc4p_00517f70() {
        core::arch::naked_asm!("mov eax, [ecx + 0x0c]", "ret");
    }
}

port! {
    /// Returns the address of the advisor collection at +0x4C.
    #[port(address = 0x0052_0080, owner = "shared", name = "GetAdvisorCollection", naked)]
    pub unsafe extern "C" fn fc4p_00520080() {
        core::arch::naked_asm!("lea eax, [ecx + 0x4c]", "ret");
    }
}

// A test-only target resolver keeps the forwarding entries independent of the target's own port.
#[cfg(test)]
extern "C" fn thunk_target(address: u32) -> usize {
    crate::abi::target(address)
}

macro_rules! adjust_interface {
    ($export:ident, $address:literal, $offset:literal, $target:literal, $declaration:ident, $name:literal) => {
        game_fn! { #[allow(dead_code)] fn $declaration(this: *mut u8) = "thiscall" $target; }
        port! {
            /// Adjusts the interface pointer and forwards the caller's registers and stack arguments.
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!(
                    "sub ecx, {offset}", "jmp dword ptr [2f]", "2: .long {target}",
                    offset = const $offset, target = const $target,
                );
                #[cfg(test)]
                core::arch::naked_asm!(
                    "sub ecx, {offset}", "push eax", "push ecx", "push edx",
                    "push {target}", "call {resolve}", "add esp, 4",
                    "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
                    offset = const $offset, target = const $target, resolve = sym thunk_target,
                );
            }
        }
    };
}

adjust_interface!(
    fc4p_005183b0,
    0x0051_83B0,
    4,
    0x008A_E800,
    retain_primary_interface,
    "AddRefPrimaryFromSecondary"
);
adjust_interface!(
    fc4p_0051b3c0,
    0x0051_B3C0,
    8,
    0x0051_B1C0,
    release_primary_interface,
    "ReleasePrimaryFromThirdInterface"
);
adjust_interface!(
    fc4p_005200d0,
    0x0052_00D0,
    4,
    0x0069_5330,
    retain_primary_from_second,
    "AddRefPrimaryFromSecondInterface"
);
adjust_interface!(
    fc4p_005200f0,
    0x0052_00F0,
    8,
    0x0069_5330,
    retain_primary_from_third,
    "AddRefPrimaryFromThirdInterface"
);
adjust_interface!(
    fc4p_00520110,
    0x0052_0110,
    12,
    0x0069_5330,
    retain_primary_from_fourth,
    "AddRefPrimaryFromFourthInterface"
);
adjust_interface!(
    fc4p_005243e0,
    0x0052_43E0,
    12,
    0x0052_4410,
    delete_bus_primary,
    "DeletePrimaryFromFourthInterface"
);

port! {
    /// Destroys a bus, conditionally frees its 0x2B8-byte allocation, and returns the original pointer.
    #[port(address = 0x0052_4410, owner = "shared", name = "DeleteBus")]
    pub unsafe extern "thiscall" fn fc4p_00524410(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game supplies the object and its deleting-destructor flags.
        unsafe {
            destroy_bus(this);
            if flags & 1 != 0 { free_sized_bus(this, 0x2B8); }
        }
        this
    }
}

port! {
    /// Destroys an ambulance and frees it only when the low flag bit is set.
    #[port(address = 0x0053_2270, owner = "shared", name = "DeleteAmbulance")]
    pub unsafe extern "thiscall" fn fc4p_00532270(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game's object and destructor/free pair.
        unsafe {
            destroy_ambulance(this);
            if flags & 1 != 0 { heap_free(this); }
        }
        this
    }
}

port! {
    /// Destroys a mass-transit train and frees it only when the low flag bit is set.
    #[port(address = 0x0053_CCE0, owner = "shared", name = "DeleteMassTransitTrain")]
    pub unsafe extern "thiscall" fn fc4p_0053cce0(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game's object and destructor/free pair.
        unsafe {
            destroy_train(this);
            if flags & 1 != 0 { heap_free(this); }
        }
        this
    }
}

port! {
    /// Writes version 1 before forwarding to the vehicle serializer, regardless of the version result.
    #[port(address = 0x0053_CE00, owner = "shared", name = "WriteVersionedVehicle")]
    pub unsafe extern "thiscall" fn fc4p_0053ce00(this: *mut u8, stream: *mut u8) -> u32 {
        // SAFETY: the original stream and vehicle serializer signatures.
        unsafe {
            vcall!(stream, 0x1C => fn(u32) -> u32, 1);
            write_vehicle(this, stream)
        }
    }
}

port! {
    /// Restores the controller tables, releases the retained object, destroys its list and string,
    /// then releases Lua. Retained pointer fields are left as they were.
    #[port(address = 0x0051_7F80, owner = "shared", name = "DestroyController")]
    pub unsafe extern "fastcall" fn fc4p_00517f80(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original controller and its retained objects.
        unsafe {
            let controller = Controller::from_ptr(this);
            controller.set_primary_table(0x00A9_5D90);
            controller.set_secondary_table(0x00A9_5D74);
            let retained = controller.retained();
            let result = if retained.is_null() {
                registers(0, entry_edx)
            } else {
                let table = word(retained, 0);
                virtual_call(retained, 8, table, entry_edx, &[])
            };
            let mut result = invoke(Conv::Thiscall, crate::abi::target(0x0056_50A0),
                this.add(0x44), result as u32, (result >> 32) as u32, &[]);
            controller.set_string_table(0x00A8_0810);
            let buffer = controller.string_buffer();
            if buffer.is_null() {
                result = registers(0, (result >> 32) as u32);
            } else {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this.add(0x44),
                    buffer as u32, (result >> 32) as u32, &[buffer as u32]);
            }
            let lua = controller.lua();
            if lua.is_null() { result } else {
                virtual_call(lua, 8, result as u32, word(lua, 0), &[])
            }
        }
    }
}

port! {
    /// Calls virtual shutdown, destroys the controller through its original entry, then frees it.
    #[port(address = 0x0051_9340, owner = "shared", name = "ShutdownAndDeleteController")]
    pub unsafe extern "fastcall" fn fc4p_00519340(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original primary object and its virtual shutdown slot.
        unsafe {
            let result = virtual_call(this, 0x10, word(this, 0), entry_edx, &[]);
            let result = invoke(Conv::Thiscall, crate::abi::target(0x0051_7F80), this,
                result as u32, (result >> 32) as u32, &[]);
            invoke(Conv::Cdecl, crate::abi::target(0x005E_5620), this,
                result as u32, (result >> 32) as u32, &[this as u32])
        }
    }
}

port! {
    /// Removes an automaton through the base manager, always calls the +0x114 hook, and returns
    /// the removal byte in the hook result's upper EAX bytes, together with the hook's EDX.
    #[port(address = 0x0054_D690, owner = "shared", name = "RemoveAutomatonAndNotify")]
    pub unsafe extern "thiscall" fn fc4p_0054d690(this: *mut u8, first: u32, second: u32) -> u64 {
        // SAFETY: the original manager and its removal and notification signatures.
        unsafe {
            let removed = remove_automaton(this, first, second);
            let result = virtual_call(this, 0x114, removed, word(this, 0), &[first]);
            (result & !0xFF) | u64::from(removed as u8)
        }
    }
}

port! {
    /// Calculates the inner square at max(radius - 1, 0) and the outer square at radius.
    /// Integer coordinates and extents wrap. AL is one; the upper EAX bytes retain the argument pointer.
    #[port(address = 0x0054_EC60, owner = "shared", name = "CalculateSquareBounds")]
    pub unsafe extern "stdcall" fn fc4p_0054ec60(context: *mut u8) -> u32 {
        // SAFETY: the caller supplies the eleven-word square context.
        unsafe {
            let x = word(context, 0);
            let z = word(context, 4);
            let radius = word(context, 8);
            let inner = (radius.wrapping_sub(1) as i32).max(0) as u32;
            store(context, 0x0C, x.wrapping_sub(inner));
            store(context, 0x10, z.wrapping_sub(inner));
            store(context, 0x18, z.wrapping_add(inner));
            store(context, 0x14, x.wrapping_add(inner));
            store(context, 0x1C, x.wrapping_sub(radius));
            store(context, 0x20, z.wrapping_sub(radius));
            store(context, 0x24, x.wrapping_add(radius));
            store(context, 0x28, z.wrapping_add(radius));
            (context as u32 & !0xFF) | 1
        }
    }
}

port! {
    /// Copies the square context word by word, retains its interface, then copies the flag and tail.
    /// Padding at +0x31..+0x33 is not copied; fields after the retain call are read after that call.
    #[port(address = 0x0054_ED10, owner = "shared", name = "CopySquareContext")]
    pub unsafe extern "thiscall" fn fc4p_0054ed10(this: *mut u8, source: *mut u8) -> *mut u8 {
        // SAFETY: both pointers cover the original context layout; overlapping contexts copy forward.
        unsafe {
            for offset in (0..=0x2C).step_by(4) { store(this, offset, word(source, offset)); }
            let retained = word(source, 0x2C) as *mut u8;
            if !retained.is_null() { vcall!(retained, 4 => fn() -> u32); }
            this.add(0x30).write(source.add(0x30).read());
            store(this, 0x34, word(source, 0x34));
        }
        this
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_0050b540::PORT,
    fc4p_0050bcb0::PORT,
    fc4p_0050be30::PORT,
    fc4p_0050c300::PORT,
    fc4p_0050c660::PORT,
    fc4p_0050cde0::PORT,
    fc4p_0050d3c0::PORT,
    fc4p_0050d510::PORT,
    fc4p_0050da30::PORT,
    fc4p_00511900::PORT,
    fc4p_00513e00::PORT,
    fc4p_00517f70::PORT,
    fc4p_00517f80::PORT,
    fc4p_005183b0::PORT,
    fc4p_00519340::PORT,
    fc4p_0051b3c0::PORT,
    fc4p_00520080::PORT,
    fc4p_005200d0::PORT,
    fc4p_005200f0::PORT,
    fc4p_00520110::PORT,
    fc4p_005243e0::PORT,
    fc4p_00524410::PORT,
    fc4p_00532270::PORT,
    fc4p_0053cce0::PORT,
    fc4p_0053ce00::PORT,
    fc4p_0054d690::PORT,
    fc4p_0054ec60::PORT,
    fc4p_0054ed10::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Object};
    use std::cell::RefCell;

    const RESULT: u64 = 0x7654_3210_ABCD_EF80;

    #[derive(Debug, PartialEq, Eq)]
    struct Call(u32, usize, Vec<u32>);

    type Mutation = (u32, *mut u8, usize, u32);
    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static MUTATION: RefCell<Option<Mutation>> = const { RefCell::new(None) };
        static FOUND: RefCell<u32> = const { RefCell::new(0) };
    }

    fn log(id: u32, this: *mut u8, args: &[u32]) {
        CALLS.with(|calls| calls.borrow_mut().push(Call(id, this as usize, args.to_vec())));
        MUTATION.with(|mutation| {
            let mut mutation = mutation.borrow_mut();
            if let Some((trigger, object, offset, value)) = *mutation {
                if id == trigger {
                    // SAFETY: the test installs a writable fixture field.
                    unsafe { store(object, offset, value) };
                    *mutation = None;
                }
            }
        });
    }

    fn calls() -> Vec<Call> {
        CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
    }

    fn mutation(trigger: u32, object: *mut u8, offset: usize, value: u32) {
        MUTATION.with(|m| *m.borrow_mut() = Some((trigger, object, offset, value)));
    }

    fn bytes(object: *mut u8, size: usize) -> Vec<u8> {
        // SAFETY: every use names a live fixture and its allocated size.
        unsafe { core::slice::from_raw_parts(object, size).to_vec() }
    }

    fn run(entry: *const (), this: *mut u8, args: &[u32], eax: u32, edx: u32) -> u64 {
        // SAFETY: each test passes the entry's original stack words and a live fixture.
        unsafe { invoke(Conv::Thiscall, entry as usize, this, eax, edx, args) }
    }

    #[test]
    fn getters_return_the_field_or_collection_address_without_writes() {
        let mut object = Object::new(0x60);
        let p = object.ptr();
        unsafe { store(p, 0x0C, 0xF123_4567) };
        let before = bytes(p, 0x60);
        assert_eq!(
            run(fc4p_00517f70 as *const (), p, &[], 7, 0x1234),
            registers(0xF123_4567, 0x1234)
        );
        assert_eq!(
            run(fc4p_00520080 as *const (), p, &[], 9, 0xABCD),
            registers(p as u32 + 0x4C, 0xABCD)
        );
        assert_eq!(bytes(p, 0x60), before);
        assert_eq!(
            run(fc4p_00520080 as *const (), 0xFFFF_FFF0usize as *mut u8, &[], 0, 9),
            registers(0x3C, 9)
        );
    }

    #[unsafe(naked)]
    unsafe extern "C" fn forwarded() {
        core::arch::naked_asm!("xor eax, [esp + 4]", "add eax, [esp + 8]", "xor edx, ecx", "ret 8");
    }

    #[test]
    fn interface_adjustments_forward_every_stack_word_and_return_register() {
        let entries = [
            (fc4p_005183b0 as *const (), 4u32, 0x008A_E800),
            (fc4p_0051b3c0 as *const (), 8, 0x0051_B1C0),
            (fc4p_005200d0 as *const (), 4, 0x0069_5330),
            (fc4p_005200f0 as *const (), 8, 0x0069_5330),
            (fc4p_00520110 as *const (), 12, 0x0069_5330),
            (fc4p_005243e0 as *const (), 12, 0x0052_4410),
        ];
        for (entry, adjustment, target) in entries {
            let _fake = fake(target, forwarded as *const () as usize);
            for this in [0u32, 4, 0x1234_5678, u32::MAX] {
                let result = run(
                    entry,
                    this as *mut u8,
                    &[0xFEDC_BA98, 0x5432_1001],
                    0x1234_5678,
                    0xAABB_CCDD,
                );
                assert_eq!(
                    result,
                    registers(
                        (0x1234_5678u32 ^ 0xFEDC_BA98).wrapping_add(0x5432_1001),
                        0xAABB_CCDD ^ this.wrapping_sub(adjustment)
                    )
                );
            }
        }
    }

    extern "thiscall" fn bus_destroy(this: *mut u8) {
        log(0x0050_4E70, this, &[]);
    }
    extern "thiscall" fn ambulance_destroy(this: *mut u8) {
        log(0x0050_4EF0, this, &[]);
    }
    extern "thiscall" fn train_destroy(this: *mut u8) {
        log(0x0053_CB80, this, &[]);
    }
    extern "C" fn free(this: *mut u8) -> u64 {
        log(0x005E_5620, this, &[]);
        RESULT
    }
    extern "C" fn free_array(this: *mut u8) -> u64 {
        log(0x0090_CF63, this, &[]);
        RESULT
    }
    extern "C" fn sized_free(this: *mut u8, size: u32) {
        log(0x0055_CA60, this, &[size]);
    }

    #[test]
    fn deleting_destructors_use_only_the_low_flag_bit_and_the_original_allocator() {
        let _free = fake(0x005E_5620, free as *const () as usize);
        let _sized = fake(0x0055_CA60, sized_free as *const () as usize);
        let _bus = fake(0x0050_4E70, bus_destroy as *const () as usize);
        let _ambulance = fake(0x0050_4EF0, ambulance_destroy as *const () as usize);
        let _train = fake(0x0053_CB80, train_destroy as *const () as usize);
        let mut object = Object::new(0x20);
        let p = object.ptr();
        let before = bytes(p, 0x20);
        for (entry, destroy, size) in [
            (fc4p_00524410 as *const (), 0x0050_4E70, Some(0x2B8)),
            (fc4p_00532270 as *const (), 0x0050_4EF0, None),
            (fc4p_0053cce0 as *const (), 0x0053_CB80, None),
        ] {
            for flags in [0, 1, 2, 3, 0x100, 0xFFFF_FFFE, u32::MAX] {
                let result = run(entry, p, &[flags], 0, 0);
                assert_eq!(result as u32, p as u32);
                let mut expected = vec![Call(destroy, p as usize, vec![])];
                if flags & 1 != 0 {
                    expected.push(match size {
                        Some(size) => Call(0x0055_CA60, p as usize, vec![size]),
                        None => Call(0x005E_5620, p as usize, vec![]),
                    });
                }
                assert_eq!(calls(), expected);
                assert_eq!(bytes(p, 0x20), before);
            }
        }
    }

    extern "thiscall" fn version(this: *mut u8, version: u32) -> u32 {
        log(0x1C, this, &[version]);
        0
    }

    extern "thiscall" fn vehicle_write(this: *mut u8, stream: *mut u8) -> u32 {
        log(0x0055_F640, this, &[stream as u32]);
        0x1234_AB80
    }

    #[test]
    fn versioned_vehicle_write_forwards_even_when_the_version_writer_returns_zero() {
        let _write = fake(0x0055_F640, vehicle_write as *const () as usize);
        let mut table = [0usize; 8];
        table[7] = version as *const () as usize;
        let mut stream = [table.as_ptr() as usize];
        let stream = stream.as_mut_ptr().cast::<u8>();
        let this = 0x1234usize as *mut u8;
        assert_eq!(unsafe { fc4p_0053ce00(this, stream) }, 0x1234_AB80);
        assert_eq!(
            calls(),
            [
                Call(0x1C, stream as usize, vec![1]),
                Call(0x0055_F640, this as usize, vec![stream as u32])
            ]
        );
    }

    extern "thiscall" fn release(this: *mut u8) -> u64 {
        log(8, this, &[]);
        RESULT
    }
    extern "thiscall" fn retain(this: *mut u8) -> u32 {
        log(4, this, &[]);
        99
    }
    extern "thiscall" fn list_destroy(this: *mut u8) -> u64 {
        log(0x0056_50A0, this, &[]);
        RESULT
    }
    extern "thiscall" fn controller_destroy(this: *mut u8) -> u64 {
        log(0x0051_7F80, this, &[]);
        RESULT
    }
    extern "thiscall" fn shutdown(this: *mut u8) -> u64 {
        log(0x10, this, &[]);
        RESULT
    }

    #[test]
    fn controller_destruction_restores_tables_and_preserves_retained_pointer_fields() {
        let _list = fake(0x0056_50A0, list_destroy as *const () as usize);
        let _free = fake(0x0090_CF63, free_array as *const () as usize);
        let mut table = [0usize; 3];
        table[2] = release as *const () as usize;
        let mut retained = [table.as_ptr() as usize];
        let retained = retained.as_mut_ptr().cast::<u8>();
        let mut lua = [table.as_ptr() as usize];
        let lua = lua.as_mut_ptr().cast::<u8>();
        let mut object = Object::new(0x60);
        let p = object.ptr();
        for with_retained in [false, true] {
            for with_buffer in [false, true] {
                for with_lua in [false, true] {
                    unsafe {
                        core::ptr::write_bytes(p, 0xA5, 0x60);
                        let c = Controller::from_ptr(p);
                        c.set_retained(if with_retained { retained } else { core::ptr::null_mut() });
                        c.set_lua(if with_lua { lua } else { core::ptr::null_mut() });
                        c.set_string_buffer(if with_buffer {
                            0x1122usize as *mut u8
                        } else {
                            core::ptr::null_mut()
                        });
                    }
                    let mut expected_bytes = bytes(p, 0x60);
                    for (offset, value) in [(0, 0x00A9_5D90u32), (4, 0x00A9_5D74), (0x28, 0x00A8_0810)] {
                        expected_bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                    }
                    let result = run(fc4p_00517f80 as *const (), p, &[], 0x9988_7766, 0x1234);
                    let mut expected = vec![];
                    if with_retained {
                        expected.push(Call(8, retained as usize, vec![]));
                    }
                    expected.push(Call(0x0056_50A0, p as usize + 0x44, vec![]));
                    if with_buffer {
                        expected.push(Call(0x0090_CF63, 0x1122, vec![]));
                    }
                    if with_lua {
                        expected.push(Call(8, lua as usize, vec![]));
                    }
                    assert_eq!(calls(), expected);
                    assert_eq!(bytes(p, 0x60), expected_bytes);
                    assert_eq!(result as u32, if with_buffer || with_lua { RESULT as u32 } else { 0 });
                }
            }
        }
    }

    #[test]
    fn controller_reloads_buffer_after_list_destruction() {
        let _list = fake(0x0056_50A0, list_destroy as *const () as usize);
        let _free = fake(0x0090_CF63, free_array as *const () as usize);
        let mut object = Object::new(0x60);
        let p = object.ptr();
        mutation(0x0056_50A0, p, 0x2C, 0x4455);
        assert_eq!(run(fc4p_00517f80 as *const (), p, &[], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x0056_50A0, p as usize + 0x44, vec![]),
                Call(0x0090_CF63, 0x4455, vec![])
            ]
        );
    }

    #[test]
    fn controller_delete_calls_shutdown_destroy_and_free_in_order() {
        let _destroy = fake(0x0051_7F80, controller_destroy as *const () as usize);
        let _free = fake(0x005E_5620, free as *const () as usize);
        let mut table = [0usize; 5];
        table[4] = shutdown as *const () as usize;
        let mut object = [table.as_ptr() as usize];
        let p = object.as_mut_ptr().cast();
        assert_eq!(run(fc4p_00519340 as *const (), p, &[], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x10, p as usize, vec![]),
                Call(0x0051_7F80, p as usize, vec![]),
                Call(0x005E_5620, p as usize, vec![])
            ]
        );
    }

    extern "thiscall" fn remove(this: *mut u8, first: u32, second: u32) -> u32 {
        log(0x0050_C990, this, &[first, second]);
        second
    }
    extern "thiscall" fn removed(this: *mut u8, first: u32) -> u64 {
        log(0x114, this, &[first]);
        RESULT
    }

    #[test]
    fn removal_hook_always_runs_and_only_its_low_eax_byte_is_replaced() {
        let _remove = fake(0x0050_C990, remove as *const () as usize);
        let mut table = [0usize; 0x118 / 4];
        table[0x114 / 4] = removed as *const () as usize;
        let mut object = [table.as_ptr() as usize];
        let p = object.as_mut_ptr().cast();
        for second in [0, 1, 0x1234_5600, 0x1234_56FE] {
            assert_eq!(
                run(fc4p_0054d690 as *const (), p, &[0x1122_3344, second], 0, 0),
                (RESULT & !0xFF) | u64::from(second as u8)
            );
            assert_eq!(
                calls(),
                [
                    Call(0x0050_C990, p as usize, vec![0x1122_3344, second]),
                    Call(0x114, p as usize, vec![0x1122_3344])
                ]
            );
        }
    }

    #[test]
    fn square_bounds_cover_zero_negative_and_wrapping_extents_without_extra_writes() {
        let mut object = Object::new(0x40);
        let p = object.ptr();
        for (x, z, radius, inner) in [
            (10u32, 20u32, 3u32, 2u32),
            (0, 0, 0, 0),
            (5, 7, 1, 0),
            (0, u32::MAX, u32::MAX, 0),
            (0x8000_0000, 0x7FFF_FFFF, 0x8000_0000, 0x7FFF_FFFF),
        ] {
            unsafe {
                core::ptr::write_bytes(p, 0xA5, 0x40);
                store(p, 0, x);
                store(p, 4, z);
                store(p, 8, radius);
            }
            let mut expected = bytes(p, 0x40);
            for (index, value) in [
                x.wrapping_sub(inner),
                z.wrapping_sub(inner),
                x.wrapping_add(inner),
                z.wrapping_add(inner),
                x.wrapping_sub(radius),
                z.wrapping_sub(radius),
                x.wrapping_add(radius),
                z.wrapping_add(radius),
            ]
            .into_iter()
            .enumerate()
            {
                expected[12 + index * 4..16 + index * 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(unsafe { fc4p_0054ec60(p) }, (p as u32 & !0xFF) | 1);
            assert_eq!(bytes(p, 0x40), expected);
        }
    }

    #[test]
    fn context_copy_retains_before_reading_the_flag_and_tail_and_leaves_padding() {
        let mut table = [0usize; 2];
        table[1] = retain as *const () as usize;
        let mut retained = [table.as_ptr() as usize];
        let retained = retained.as_mut_ptr().cast::<u8>();
        let mut source = Object::new(0x40);
        let mut dest = Object::new(0x40);
        let (s, d) = (source.ptr(), dest.ptr());
        for pointer in [0, retained as u32] {
            unsafe {
                for offset in 0..0x40 {
                    s.add(offset).write(offset as u8);
                }
                core::ptr::write_bytes(d, 0xA5, 0x40);
                store(s, 0x2C, pointer);
            }
            if pointer != 0 {
                mutation(4, s, 0x30, 0xDEAD_BE78);
            }
            let result = unsafe { fc4p_0054ed10(d, s) };
            assert_eq!(result, d);
            let mut expected = vec![0xA5; 0x40];
            expected[..0x30].copy_from_slice(&bytes(s, 0x30));
            expected[0x30] = unsafe { s.add(0x30).read() };
            expected[0x34..0x38].copy_from_slice(&bytes(s.wrapping_add(0x34), 4));
            assert_eq!(bytes(d, 0x40), expected);
            assert_eq!(
                calls(),
                if pointer == 0 {
                    vec![]
                } else {
                    vec![Call(4, retained as usize, vec![])]
                }
            );
        }
    }

    #[test]
    fn context_copy_supports_self_assignment_and_forward_overlap() {
        let mut storage = Object::new(0x80);
        let p = storage.ptr();
        for delta in [0usize, 4] {
            unsafe {
                core::ptr::write_bytes(p, 0, 0x80);
                for offset in (4..0x2C).step_by(4) {
                    store(p, offset, offset as u32 + 1);
                }
            }
            let mut expected = bytes(p, 0x80);
            for offset in (0..0x30).step_by(4) {
                let value: [u8; 4] = expected[offset..offset + 4].try_into().unwrap();
                expected[delta + offset..delta + offset + 4].copy_from_slice(&value);
            }
            expected[delta + 0x30] = expected[0x30];
            let tail: [u8; 4] = expected[0x34..0x38].try_into().unwrap();
            expected[delta + 0x34..delta + 0x38].copy_from_slice(&tail);
            unsafe { fc4p_0054ed10(p.add(delta), p) };
            assert_eq!(bytes(p, 0x80), expected);
        }
        assert!(calls().is_empty());
    }

    extern "C" fn allocate_message(size: u32) -> *mut u8 {
        log(0x0091_33DA, core::ptr::null_mut(), &[size]);
        FOUND.with(|found| *found.borrow() as *mut u8)
    }
    extern "thiscall" fn construct_message(this: *mut u8) -> *mut u8 {
        log(0x0091_34D6, this, &[]);
        this
    }
    extern "thiscall" fn message_type(this: *mut u8, value: u32) -> u64 {
        log(0x14, this, &[value]);
        RESULT
    }
    extern "thiscall" fn message_owner(this: *mut u8, value: u32) -> u64 {
        log(0x30, this, &[value]);
        RESULT
    }
    extern "thiscall" fn message_first(this: *mut u8, value: u32) -> u64 {
        log(0x40, this, &[value]);
        RESULT
    }
    extern "thiscall" fn message_second(this: *mut u8, value: u32) -> u64 {
        log(0x54, this, &[value]);
        RESULT
    }
    extern "thiscall" fn observe(this: *mut u8, value: u32) -> u64 {
        log(0x0C, this, &[value]);
        RESULT
    }

    #[test]
    fn observer_notification_skips_empty_and_null_entries_and_reuses_one_message() {
        let _allocate = fake(0x0091_33DA, allocate_message as *const () as usize);
        let _construct = fake(0x0091_34D6, construct_message as *const () as usize);
        let mut message_table = [0usize; 0x58 / 4];
        for (slot, entry) in [
            (4, retain as *const ()),
            (8, release as *const ()),
            (0x14, message_type as *const ()),
            (0x30, message_owner as *const ()),
            (0x40, message_first as *const ()),
            (0x54, message_second as *const ()),
        ] {
            message_table[slot / 4] = entry as usize;
        }
        let mut message = [message_table.as_ptr() as usize];
        let message = message.as_mut_ptr().cast::<u8>();
        FOUND.with(|found| *found.borrow_mut() = message as u32);
        let observer_table = [0usize, 0, 0, observe as *const () as usize];
        let mut observer = [observer_table.as_ptr() as usize];
        let observer = observer.as_mut_ptr().cast::<u8>();
        let mut owner = Object::new(0x190);
        let owner = owner.ptr();
        let mut header = [0u32; 3];
        let h = header.as_mut_ptr() as u32;
        header[0] = h;
        unsafe { store(owner, 0x188, h) };
        assert_eq!(
            run(fc4p_0050b540 as *const (), owner, &[0, u32::MAX], 0, 0xCAFE_BABE),
            registers(h, 0xCAFE_BABE)
        );
        assert!(calls().is_empty());
        let mut last = [h, 0, observer as u32];
        let mut middle = [last.as_mut_ptr() as u32, 0, 0];
        let mut first = [middle.as_mut_ptr() as u32, 0, observer as u32];
        header[0] = first.as_mut_ptr() as u32;
        let before = bytes(owner, 0x190);
        // The callback changes the first node's successor. Iteration must use the saved successor.
        mutation(0x0C, first.as_mut_ptr().cast(), 0, h);
        assert_eq!(
            run(fc4p_0050b540 as *const (), owner, &[0x1234_5678, 0x8765_4321], 0, 0),
            RESULT
        );
        assert_eq!(
            calls(),
            [
                Call(0x0091_33DA, 0, vec![0x2C]),
                Call(0x0091_34D6, message as usize, vec![]),
                Call(4, message as usize, vec![]),
                Call(0x14, message as usize, vec![0x09D9_2FC3]),
                Call(0x30, message as usize, vec![owner as u32]),
                Call(0x40, message as usize, vec![0x1234_5678]),
                Call(0x54, message as usize, vec![0x8765_4321]),
                Call(0x0C, observer as usize, vec![message as u32]),
                Call(0x0C, observer as usize, vec![message as u32]),
                Call(8, message as usize, vec![]),
            ]
        );
        assert_eq!(bytes(owner, 0x190), before);
        // Keep the rewritten header read visible to the fixture as well.
        assert_eq!(header[0], first.as_mut_ptr() as u32);
    }

    extern "thiscall" fn source_lookup(this: *mut u8, out: *mut u32, key: *const u32) -> u64 {
        // SAFETY: the port supplies its two-word iterator and readable key.
        unsafe {
            log(0x006F_1530, this, &[key.read()]);
            out.write(FOUND.with(|found| *found.borrow()));
            out.add(1).write(this as u32);
        }
        RESULT
    }
    extern "thiscall" fn group_lookup(this: *mut u8, out: *mut u32, key: *const u32) {
        // SAFETY: the port supplies a one-word iterator and readable key.
        unsafe {
            log(0x0049_7520, this, &[key.read()]);
            out.write(FOUND.with(|found| *found.borrow()));
        }
    }
    extern "thiscall" fn source_total(this: *mut u8) -> u64 {
        log(0x2C, this, &[]);
        RESULT
    }

    #[test]
    fn source_count_routes_special_groups_and_preserves_lookup_edx() {
        let _lookup = fake(0x006F_1530, source_lookup as *const () as usize);
        let mut service_pointer = core::ptr::null_mut::<u8>();
        let _global = crate::testing::fake_data(0x00B4_3D28, (&mut service_pointer as *mut *mut u8).cast());
        let mut table = [0usize; 12];
        table[0x2C / 4] = source_total as *const () as usize;
        let mut service = [table.as_ptr() as usize];
        let mut object = Object::new(0xA80);
        let p = object.ptr();
        for group in [0x1402, 0x1406] {
            assert_eq!(
                run(fc4p_0050be30 as *const (), p, &[group], 0, 0x1357_9BDF),
                registers(0, 0x1357_9BDF)
            );
            assert!(calls().is_empty());
        }
        service_pointer = service.as_mut_ptr().cast();
        for group in [0x1402, 0x1406] {
            assert_eq!(run(fc4p_0050be30 as *const (), p, &[group], 0, 7), RESULT);
            assert_eq!(calls(), [Call(0x2C, service_pointer as usize, vec![])]);
        }
        let mut node = [0, 0, 0xFFFF_FF80u32];
        for found in [0, node.as_mut_ptr() as u32] {
            FOUND.with(|slot| *slot.borrow_mut() = found);
            for group in [0, 0x1401, 0x1403, 0x1405, 0x1407, u32::MAX] {
                assert_eq!(
                    run(fc4p_0050be30 as *const (), p, &[group], 0, 0),
                    registers(if found == 0 { 0 } else { node[2] }, (RESULT >> 32) as u32)
                );
                assert_eq!(calls(), [Call(0x006F_1530, p as usize + 0xA64, vec![group])]);
            }
        }
    }

    #[test]
    fn group_presence_uses_the_current_sentinel_after_the_lookup() {
        let _lookup = fake(0x0049_7520, group_lookup as *const () as usize);
        let mut object = Object::new(0xA80);
        let p = object.ptr();
        unsafe { store(p, 0xA24, 0x1234) };
        for found in [0x1234, 0x5678, 0] {
            FOUND.with(|slot| *slot.borrow_mut() = found);
            assert_eq!(
                run(fc4p_0050c300 as *const (), p, &[0x8765_4321], 0, 0),
                registers(u32::from(found != 0x1234), found)
            );
            assert_eq!(calls(), [Call(0x0049_7520, p as usize + 0xA24, vec![0x8765_4321])]);
        }
        FOUND.with(|slot| *slot.borrow_mut() = 0x5678);
        mutation(0x0049_7520, p, 0xA24, 0x5678);
        assert_eq!(run(fc4p_0050c300 as *const (), p, &[7], 0, 0), registers(0, 0x5678));
        assert_eq!(calls(), [Call(0x0049_7520, p as usize + 0xA24, vec![7])]);
    }

    extern "thiscall" fn storage_release(this: *mut u8, data: u32, size: u32) -> u64 {
        log(0x0061_4380, this, &[data, size]);
        RESULT
    }
    extern "C" fn mesh_free(this: *mut u8, size: u32) -> u64 {
        log(0x0080_0EC0, this, &[size]);
        RESULT
    }

    #[test]
    fn mesh_release_decrements_unsigned_counts_or_frees_storage_in_order() {
        let _storage = fake(0x0061_4380, storage_release as *const () as usize);
        let _free = fake(0x0080_0EC0, mesh_free as *const () as usize);
        let mut object = Object::new(0x40);
        let p = object.ptr();
        for count in [2u16, 0x8000, u16::MAX] {
            unsafe {
                core::ptr::write_bytes(p, 0xA5, 0x40);
                p.add(0x2C).cast::<u16>().write_unaligned(count);
            }
            let mut expected = bytes(p, 0x40);
            expected[0x2C..0x2E].copy_from_slice(&(count - 1).to_le_bytes());
            assert_eq!(
                run(fc4p_0050c660 as *const (), p, &[], 0, 0x7654_3210),
                registers(u32::from(count - 1), 0x7654_3210)
            );
            assert_eq!(bytes(p, 0x40), expected);
            assert!(calls().is_empty());
        }
        for count in [0u16, 1] {
            for data in [0, 0x1234, 0xFFFF_FFF0] {
                unsafe {
                    p.add(0x2C).cast::<u16>().write_unaligned(count);
                    store(p, 8, data);
                    store(p, 0x10, 0x100);
                }
                let before = bytes(p, 0x40);
                assert_eq!(
                    run(fc4p_0050c660 as *const (), p, &[], 0, 0x1234),
                    RESULT & !0xFFFF_FFFF
                );
                let mut expected = vec![];
                if data != 0 {
                    expected.push(Call(
                        0x0061_4380,
                        p as usize + 0x10,
                        vec![data, 0x100u32.wrapping_sub(data)],
                    ));
                }
                expected.push(Call(0x0080_0EC0, p as usize, vec![0x30]));
                assert_eq!(calls(), expected);
                assert_eq!(bytes(p, 0x40), before);
            }
        }
    }

    extern "thiscall" fn entry_destroy(this: *mut u8, owner: *mut u8) -> u64 {
        log(0, this, &[owner as u32]);
        RESULT
    }

    #[test]
    fn vector_entry_destruction_skips_nulls_and_only_resets_the_end() {
        let _free = fake(0x005E_5620, free as *const () as usize);
        let table = [entry_destroy as *const () as usize];
        let mut entry = [table.as_ptr() as usize];
        let entry = entry.as_mut_ptr().cast::<u8>();
        let mut data = [entry as u32, 0, entry as u32];
        let begin = data.as_mut_ptr() as u32;
        let mut object = Object::new(0xC0);
        let p = object.ptr();
        for count in [0, 3] {
            unsafe {
                store(p, 0xB4, begin);
                store(p, 0xB8, begin + count * 4);
            }
            let mut expected = bytes(p, 0xC0);
            expected[0xB8..0xBC].copy_from_slice(&begin.to_le_bytes());
            assert_eq!(
                run(fc4p_0050d510 as *const (), p, &[], 0, 0x5678),
                registers(
                    begin + count * 4,
                    if count == 0 { 0x5678 } else { (RESULT >> 32) as u32 }
                )
            );
            let pair = [
                Call(0, entry as usize, vec![p as u32]),
                Call(0x005E_5620, entry as usize, vec![]),
            ];
            assert_eq!(
                calls(),
                if count == 0 {
                    vec![]
                } else {
                    pair.into_iter()
                        .chain([
                            Call(0, entry as usize, vec![p as u32]),
                            Call(0x005E_5620, entry as usize, vec![]),
                        ])
                        .collect()
                }
            );
            assert_eq!(bytes(p, 0xC0), expected);
        }
        unsafe { store(p, 0xB8, begin + 12) };
        mutation(0, p, 0xB8, begin + 4);
        assert_eq!(run(fc4p_0050d510 as *const (), p, &[], 0, 0) as u32, begin + 4);
        assert_eq!(
            calls(),
            [
                Call(0, entry as usize, vec![p as u32]),
                Call(0x005E_5620, entry as usize, vec![])
            ]
        );
    }

    extern "thiscall" fn erase_nodes(this: *mut u8, root: *mut u8) {
        log(0x0051_1900, this, &[root as u32]);
    }

    #[test]
    fn string_tree_erasure_visits_right_before_node_before_left_and_restores_both_string_tables() {
        let _free = fake(0x0090_CF63, free_array as *const () as usize);
        let mut root = Object::new(0x50);
        let mut right = Object::new(0x50);
        let mut left = Object::new(0x50);
        let (r, a, b) = (root.ptr(), right.ptr(), left.ptr());
        unsafe {
            store(r, 8, b as u32);
            store(r, 0x0C, a as u32);
            store(r, 0x14, 0x1000);
            store(r, 0x30, 0x2000);
            store(a, 0x30, 0x3000);
            store(b, 0x14, 0x4000);
        }
        let owner = 0x1234usize as *mut u8;
        unsafe { fc4p_00511900(owner, r) };
        assert_eq!(
            calls(),
            [
                Call(0x0090_CF63, 0x3000, vec![]),
                Call(0x0090_CF63, a as usize, vec![]),
                Call(0x0090_CF63, 0x2000, vec![]),
                Call(0x0090_CF63, 0x1000, vec![]),
                Call(0x0090_CF63, r as usize, vec![]),
                Call(0x0090_CF63, 0x4000, vec![]),
                Call(0x0090_CF63, b as usize, vec![])
            ]
        );
        for node in [r, a, b] {
            assert_eq!(
                unsafe { (word(node, 0x10), word(node, 0x2C)) },
                (0x00A8_0810, 0x00A8_0810)
            );
        }
        unsafe { fc4p_00511900(owner, core::ptr::null_mut()) };
        assert!(calls().is_empty());
    }

    #[test]
    fn tree_owner_destruction_restores_sentinel_only_for_nonempty_trees() {
        let _erase = fake(0x0051_1900, erase_nodes as *const () as usize);
        let _free = fake(0x0090_CF63, free_array as *const () as usize);
        let table = [0usize, 0, release as *const () as usize];
        let mut retained = [table.as_ptr() as usize];
        let retained = retained.as_mut_ptr().cast::<u8>();
        let mut header = [0xA5A5_A5A5u32, 0x1122, 0x3344, 0x5566];
        let h = header.as_mut_ptr() as u32;
        let mut object = Object::new(0x20);
        let p = object.ptr();
        for count in [0, 1, u32::MAX] {
            for has_retained in [false, true] {
                header[1..].copy_from_slice(&[0x1122, 0x3344, 0x5566]);
                unsafe {
                    store(p, 4, if has_retained { retained as u32 } else { 0 });
                    store(p, 8, h);
                    store(p, 0x0C, count);
                }
                let mut expected = bytes(p, 0x20);
                if count != 0 {
                    expected[0x0C..0x10].fill(0);
                }
                assert_eq!(run(fc4p_00513e00 as *const (), p, &[], 0, 0), RESULT);
                let mut expected_calls = vec![];
                if count != 0 {
                    expected_calls.push(Call(0x0051_1900, p as usize + 8, vec![0x1122]));
                }
                expected_calls.push(Call(0x0090_CF63, h as usize, vec![]));
                if has_retained {
                    expected_calls.push(Call(8, retained as usize, vec![]));
                }
                assert_eq!(calls(), expected_calls);
                assert_eq!(bytes(p, 0x20), expected);
                assert_eq!(
                    header,
                    if count == 0 {
                        [0xA5A5_A5A5, 0x1122, 0x3344, 0x5566]
                    } else {
                        [0xA5A5_A5A5, 0, h, h]
                    }
                );
            }
        }
        unsafe {
            store(p, 4, 0);
            store(p, 8, 0);
            store(p, 0x0C, 0);
        }
        assert_eq!(
            run(fc4p_00513e00 as *const (), p, &[], 0xFEDC_BA98, 0x1234_5678),
            registers(0, 0x1234_5678)
        );
        assert!(calls().is_empty());
    }

    extern "C" fn array_allocate(size: u32) -> *mut u8 {
        log(0x0090_CF54, core::ptr::null_mut(), &[size]);
        FOUND.with(|found| *found.borrow() as *mut u8)
    }

    fn vector_copy_registers(this: *mut u8, source: *mut u8) -> [u32; 3] {
        let mut frame = [this as u32, source as u32, 0, 0, 0];
        // SAFETY: one thiscall word, removed by the entry. EDI retains the result frame across the call.
        unsafe {
            core::arch::asm!(
                "mov ecx, [edi]", "push [edi + 4]", "call {entry}",
                "mov [edi + 8], eax", "mov [edi + 12], ecx", "mov [edi + 16], edx",
                in("edi") frame.as_mut_ptr(), entry = sym fc4p_0050bcb0, clobber_abi("C"),
            );
        }
        [frame[2], frame[3], frame[4]]
    }

    #[test]
    fn triple_vector_copy_allocates_exactly_the_live_elements_and_returns_source_end_in_ecx_and_edx() {
        let _allocate = fake(0x0090_CF54, array_allocate as *const () as usize);
        let mut storage = [0xA5A5_A5A5u32; 9];
        let allocation = storage.as_mut_ptr() as u32;
        FOUND.with(|found| *found.borrow_mut() = allocation);
        let mut source_data = [1u32, 2, 3, 4, 5, 6];
        let begin = source_data.as_mut_ptr() as u32;
        for count in [0u32, 1, 2] {
            storage.fill(0xA5A5_A5A5);
            let end = begin + count * 12;
            let mut source = [begin, end, begin + 100];
            let mut dest = [0xBBBB_BBBBu32; 4];
            let dest_pointer = dest.as_mut_ptr().cast::<u8>();
            assert_eq!(
                vector_copy_registers(dest_pointer, source.as_mut_ptr().cast()),
                [dest_pointer as u32, end, end]
            );
            if count == 0 {
                assert_eq!(dest, [0, 0, 0, 0xBBBB_BBBB]);
                assert!(calls().is_empty());
            } else {
                assert_eq!(
                    dest,
                    [
                        allocation,
                        allocation + count * 12,
                        allocation + count * 12,
                        0xBBBB_BBBB
                    ]
                );
                assert_eq!(calls(), [Call(0x0090_CF54, 0, vec![count * 12])]);
                assert_eq!(&storage[..count as usize * 3], &source_data[..count as usize * 3]);
            }
            assert!(storage[count as usize * 3..].iter().all(|&word| word == 0xA5A5_A5A5));
        }
    }

    #[test]
    fn triple_vector_copy_rereads_source_after_allocation_and_handles_one_element_allocation_failure() {
        let _allocate = fake(0x0090_CF54, array_allocate as *const () as usize);
        let mut source_data = [1u32, 2, 3];
        let begin = source_data.as_mut_ptr() as u32;
        let mut source = [begin, begin + 12, begin + 12];
        let mut dest = [0u32; 3];
        let d = dest.as_mut_ptr().cast::<u8>();
        FOUND.with(|found| *found.borrow_mut() = 0);
        assert_eq!(
            vector_copy_registers(d, source.as_mut_ptr().cast()),
            [d as u32, begin + 12, begin + 12]
        );
        assert_eq!(dest, [0, 12, 12]);
        assert_eq!(calls(), [Call(0x0090_CF54, 0, vec![12])]);
        let mut allocation = [0xA5A5_A5A5u32; 6];
        let a = allocation.as_mut_ptr() as u32;
        FOUND.with(|found| *found.borrow_mut() = a);
        mutation(0x0090_CF54, source.as_mut_ptr().cast(), 4, begin);
        assert_eq!(
            vector_copy_registers(d, source.as_mut_ptr().cast()),
            [d as u32, begin, begin]
        );
        assert_eq!(dest, [a, a, a + 12]);
        assert_eq!(allocation, [0xA5A5_A5A5; 6]);
        assert_eq!(calls(), [Call(0x0090_CF54, 0, vec![12])]);
    }

    extern "thiscall" fn insert_one(
        this: *mut u8,
        position: u32,
        value: *mut u8,
        out: *mut u32,
        count: u32,
        stride: u32,
    ) -> u32 {
        log(0x0050_C3B0, this, &[position, value as u32, count, stride]);
        // SAFETY: the entry passes its original argument word as this helper's output.
        unsafe { out.write(0xAABB_CCDD) };
        0x1122_3344
    }

    fn append_with_argument_slot(this: *mut u8, source: u32) -> [u32; 2] {
        let mut frame = [this as u32, source, 0, 0];
        // SAFETY: the original entry pops four bytes. Read that argument word immediately after the
        // return, before any instruction can reuse its stack storage.
        unsafe {
            core::arch::asm!(
                "push [edi + 4]", "mov ecx, [edi]", "call {entry}",
                "mov [edi + 8], eax", "mov eax, [esp - 4]", "mov [edi + 12], eax",
                in("edi") frame.as_mut_ptr(), entry = sym fc4p_0050da30, clobber_abi("C"),
            );
        }
        [frame[2], frame[3]]
    }

    #[test]
    fn append_triple_copies_forward_or_passes_the_original_argument_word_to_insertion() {
        let _insert = fake(0x0050_C3B0, insert_one as *const () as usize);
        let mut source = [0x1234_5678u32, 0x8765_4321, 0xFFFF_FFFE];
        let s = source.as_mut_ptr() as u32;
        let mut data = [0xA5A5_A5A5u32; 6];
        let begin = data.as_mut_ptr() as u32;
        let mut vector = [begin, begin, begin + 24];
        let p = vector.as_mut_ptr().cast::<u8>();
        assert_eq!(append_with_argument_slot(p, s), [begin, s]);
        assert_eq!(
            data,
            [source[0], source[1], source[2], 0xA5A5_A5A5, 0xA5A5_A5A5, 0xA5A5_A5A5]
        );
        assert_eq!(vector, [begin, begin + 12, begin + 24]);
        assert!(calls().is_empty());
        vector[1] = begin + 24;
        assert_eq!(append_with_argument_slot(p, s), [0x1122_3344, 0xAABB_CCDD]);
        assert_eq!(calls(), [Call(0x0050_C3B0, p as usize, vec![begin + 24, s, 1, 1])]);
        vector.copy_from_slice(&[0, 0, 12]);
        assert_eq!(append_with_argument_slot(p, 0), [0, 0]);
        assert_eq!(vector, [0, 12, 12]);
        assert!(calls().is_empty());
    }

    extern "thiscall" fn pointer_set_begin(this: *mut u8, out: *mut u32) -> u64 {
        log(0x004B_0390, this, &[]);
        // SAFETY: the port supplies a two-word iterator output; the test fixture has four words.
        unsafe {
            out.write(word(this, 0));
            out.add(1).write(this as u32);
        }
        RESULT
    }
    extern "thiscall" fn pointer_set_destroy(this: *mut u8) -> u64 {
        log(0x008E_BA60, this, &[]);
        RESULT
    }

    #[test]
    fn pointer_set_release_visits_chains_then_later_buckets_and_destroys_empty_sets_too() {
        let _begin = fake(0x004B_0390, pointer_set_begin as *const () as usize);
        let _destroy = fake(0x008E_BA60, pointer_set_destroy as *const () as usize);
        let table = [0usize, 0, release as *const () as usize];
        let mut interfaces = [[table.as_ptr() as usize]; 2];
        let (a, b) = (interfaces[0].as_mut_ptr() as u32, interfaces[1].as_mut_ptr() as u32);
        let mut nodes = [[0u32; 2]; 3];
        let mut buckets = [0u32; 5];
        nodes[0][1] = a;
        nodes[1][1] = b;
        // Null is a valid bucket-zero member and is skipped during Release.
        for index in 0..3 {
            let key = nodes[index][1];
            let bucket = key as usize % buckets.len();
            nodes[index][0] = buckets[bucket];
            buckets[bucket] = nodes[index].as_mut_ptr() as u32;
        }
        let mut set = [0u32, buckets.as_mut_ptr() as u32, buckets.as_mut_ptr() as u32 + 20, 0];
        let p = set.as_mut_ptr().cast::<u8>();
        assert_eq!(run(fc4p_0050cde0 as *const (), p, &[p as u32], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x004B_0390, p as usize, vec![]),
                Call(0x008E_BA60, p as usize, vec![])
            ]
        );
        set[0] = buckets.iter().copied().find(|&node| node != 0).unwrap();
        let before = set;
        let mut expected = vec![Call(0x004B_0390, p as usize, vec![])];
        for &head in &buckets {
            let mut node = head;
            while node != 0 {
                let value = unsafe { word(node as *mut u8, 4) };
                if value != 0 {
                    expected.push(Call(8, value as usize, vec![]));
                }
                node = unsafe { word(node as *mut u8, 0) };
            }
        }
        expected.push(Call(0x008E_BA60, p as usize, vec![]));
        assert_eq!(run(fc4p_0050cde0 as *const (), p, &[p as u32], 0, 0), RESULT);
        assert_eq!(calls(), expected);
        assert_eq!(set, before);
    }

    extern "thiscall" fn reset_collection(this: *mut u8, count: u32) {
        log(0x0080_38D0, this, &[count]);
    }
    extern "thiscall" fn disable(this: *mut u8, value: u32) {
        log(0x11C, this, &[value]);
    }
    extern "thiscall" fn finish_clear(this: *mut u8) -> u64 {
        log(0x1A8, this, &[]);
        RESULT
    }

    #[test]
    fn object_collection_clear_releases_adjusted_bases_and_resets_only_ends() {
        let _reset = fake(0x0080_38D0, reset_collection as *const () as usize);
        let mut owner_table = [0usize; 0x1AC / 4];
        owner_table[0x11C / 4] = disable as *const () as usize;
        owner_table[0x1A8 / 4] = finish_clear as *const () as usize;
        let base_table = [0usize, 0, release as *const () as usize];
        let adjustment_table = [0u32, 8];
        let mut entry = [
            0u32,
            adjustment_table.as_ptr() as u32,
            0xA5A5_A5A5,
            base_table.as_ptr() as u32,
        ];
        let mut data = [entry.as_mut_ptr() as u32, 0];
        let begin = data.as_mut_ptr() as u32;
        let mut owner = Object::new(0x140);
        let p = owner.ptr();
        for flags in [0u8, 2, 0xFF] {
            unsafe {
                store(p, 0, owner_table.as_ptr() as u32);
                p.add(0x33).write(flags);
                store(p, 0x110, begin);
                store(p, 0x114, begin + 8);
                store(p, 0xC0, 0x1234);
                store(p, 0xC4, 0x5678);
            }
            let mut expected_bytes = bytes(p, 0x140);
            expected_bytes[0x114..0x118].copy_from_slice(&begin.to_le_bytes());
            expected_bytes[0xC4..0xC8].copy_from_slice(&0x1234u32.to_le_bytes());
            assert_eq!(run(fc4p_0050d3c0 as *const (), p, &[], 0, 0), (RESULT & !0xFF) | 1);
            let mut expected = vec![];
            if flags & 2 != 0 {
                expected.push(Call(0x11C, p as usize, vec![0]));
            }
            expected.extend([
                Call(0x0080_38D0, p as usize + 0x12C, vec![0]),
                Call(8, entry.as_ptr() as usize + 12, vec![]),
                Call(0x1A8, p as usize, vec![]),
            ]);
            assert_eq!(calls(), expected);
            assert_eq!(bytes(p, 0x140), expected_bytes);
        }
        unsafe { store(p, 0x114, begin) };
        run(fc4p_0050d3c0 as *const (), p, &[], 0, 0);
        assert_eq!(
            calls(),
            [
                Call(0x11C, p as usize, vec![0]),
                Call(0x0080_38D0, p as usize + 0x12C, vec![0]),
                Call(0x1A8, p as usize, vec![])
            ]
        );
    }
}
