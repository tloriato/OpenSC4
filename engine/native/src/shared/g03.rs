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

use crate::abi::{Conv, RawCall, call_raw, vslot};
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

// Some helpers leave a callee's EDX visible, including when that callee changes only a byte.
fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

fn unsigned_remainder(value: u32, divisor: u32) -> u32 {
    let remainder: u32;
    // SAFETY: unsigned division with a zero high half. A zero divisor raises the processor's divide
    // exception, as it does in the game's bucket iterator, rather than entering a Rust panic path.
    unsafe {
        core::arch::asm!("xor edx, edx", "div {divisor:e}", divisor = in(reg) divisor,
            inout("eax") value => _, out("edx") remainder, options(nomem, nostack));
    }
    remainder
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
game_fn! { #[allow(dead_code)] fn move_bytes(destination: u32, source: u32, size: u32) -> u64 = "C" 0x009E_EB70; }
game_fn! { #[allow(dead_code)] fn retain_primary_interface(this: *mut u8) -> u32 = "thiscall" 0x008A_E800; }
game_fn! { #[allow(dead_code)] fn release_primary_interface(this: *mut u8) -> u32 = "thiscall" 0x0051_B1C0; }
game_fn! { #[allow(dead_code)] fn retain_advisor_primary(this: *mut u8) -> u32 = "thiscall" 0x0069_5330; }
game_fn! { #[allow(dead_code)] fn delete_bus_primary(this: *mut u8, flags: u32) -> *mut u8 = "thiscall" 0x0052_4410; }

game_data! { static source_building_service: *mut u8 = 0x00B4_3D28; }
game_data! { static positive_speed_threshold: f32 = 0x00A8_1054; }
game_data! { static interval_numerator: f32 = 0x00A8_1228; }
game_data! { static interval_scale: f32 = 0x00A9_7D74; }

port! {
    /// Removes matching objects from the current stack and pointer vector, releases the selected
    /// object after clearing its field, and returns one in AL with the remaining EAX bytes intact.
    #[port(address = 0x0050_B290, owner = "shared", name = "RemoveObjectReferences", naked)]
    pub unsafe extern "C" fn fc4p_0050b290() {
        // Supply both caller-saved inputs without adding words to the original interface.
        core::arch::naked_asm!("push [esp + 4]", "push eax", "call {body}", "ret 4", body = sym remove_object_references);
    }
}

unsafe extern "fastcall" fn remove_object_references(
    this: *mut u8,
    entry_edx: u32,
    entry_eax: u32,
    object: *mut u8,
) -> u64 {
    // SAFETY: the game's owner, stack accessors, pointer vector and retained object.
    unsafe {
        if object.is_null() {
            return registers((entry_eax & !0xFF) | 1, entry_edx);
        }
        let mut result = virtual_call(this, 0xE0, word(this, 0), entry_edx, &[]);
        while result as u32 != 0 && word(result as u32 as *mut u8, 8) == object as u32 {
            result = virtual_call(this, 0xD4, result as u32, word(this, 0), &[]);
            result = virtual_call(this, 0xE0, word(this, 0), (result >> 32) as u32, &[]);
        }
        let mut cursor = word(this, 0xB4);
        let mut next = cursor.wrapping_add(4);
        while cursor != word(this, 0xB8) {
            let entry = word(cursor as *mut u8, 0);
            if word(entry as *mut u8, 8) == object as u32 {
                virtual_call(object, 8, entry, word(object, 0), &[]);
                let end = word(this, 0xB8);
                if next != end {
                    invoke(
                        Conv::Cdecl,
                        crate::abi::target(0x009E_EB70),
                        object,
                        end.wrapping_sub(next),
                        0,
                        &[cursor, next, end.wrapping_sub(next)],
                    );
                }
                let end = word(this, 0xB8).wrapping_sub(4);
                store(this, 0xB8, end);
                result = invoke(
                    Conv::Cdecl,
                    crate::abi::target(0x005E_5620),
                    object,
                    entry,
                    end,
                    &[entry],
                );
            } else {
                cursor = cursor.wrapping_add(4);
                next = next.wrapping_add(4);
            }
        }
        let selected = word(this, 0x98);
        result = registers(selected, (result >> 32) as u32);
        if selected == object as u32 {
            store(this, 0x98, 0);
            result = virtual_call(object, 8, selected, word(object, 0), &[]);
        }
        (result & !0xFF) | 1
    }
}

/// Compares a register value to the stored threshold while retaining the original one-value x87
/// stack depth in AX. The compared value is popped only after saving the status word.
fn speed_status(speed: F80, threshold: f32) -> u16 {
    let speed = speed.to_bytes();
    let status: u16;
    // SAFETY: the local operands cover their values; one value is pushed and popped.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{speed}]", "fcom dword ptr [{threshold}]", "fnstsw ax", "fstp st(0)",
            speed = in(reg) speed.as_ptr(), threshold = in(reg) &threshold, out("ax") status, out("st(0)") _);
    }
    status
}

fn scale_interval(value: F80, scale: f32) -> f32 {
    let value = value.to_bytes();
    let mut output = 0.0f32;
    // SAFETY: the local operands cover their values; FMUL uses the stored operand before rounding.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fmul dword ptr [{scale}]", "fstp dword ptr [{output}]",
            value = in(reg) value.as_ptr(), scale = in(reg) &scale, output = in(reg) &mut output, out("st(0)") _);
    }
    output
}

port! {
    /// Uses a positive speed to calculate numerator / speed * scale, otherwise 0.1. Sends the stored
    /// interval to +0x8C, rounds that slot's ST0 result to f32, and sends it to +0x90 through the same
    /// saved vtable. Both float results retain x87 precision until the original store points.
    #[port(address = 0x0054_DA50, owner = "shared", name = "UpdateSpeedInterval")]
    pub unsafe extern "fastcall" fn fc4p_0054da50(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original object's float-returning slots and the game's stored float constants.
        unsafe {
            let mut call = RawCall::new(Conv::Thiscall, vslot(this, 0x1F4), this as u32, &[]);
            call.eax = word(this, 0);
            call.edx = entry_edx;
            call.take_st0 = 1;
            call_raw(&mut call);
            let status = speed_status(call.st0, positive_speed_threshold().read());
            let interval = if status & 0x4100 == 0 {
                scale_interval(F80::from_f32(interval_numerator().read()) / call.st0, interval_scale().read())
            } else { f32::from_bits(0x3DCC_CCCD) };
            let table = word(this, 0) as *mut u8;
            let args = [interval.to_bits()];
            let mut transform = RawCall::new(Conv::Thiscall, word(table, 0x8C) as usize, this as u32, &args);
            transform.eax = (call.eax & 0xFFFF_0000) | u32::from(status);
            transform.edx = call.edx;
            transform.take_st0 = 1;
            call_raw(&mut transform);
            let output = transform.st0.to_f32();
            invoke(Conv::Thiscall, word(table, 0x90) as usize, this,
                transform.eax, transform.edx, &[output.to_bits()])
        }
    }
}

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
                    let mut bucket = unsigned_remainder(word(node as *mut u8, 4), count);
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
    ($export:ident, $address:literal, $offset:literal, $target:literal, $name:literal) => {
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

adjust_interface!(fc4p_005183b0, 0x0051_83B0, 4, 0x008A_E800, "AddRefPrimaryFromSecondary");
adjust_interface!(
    fc4p_0051b3c0,
    0x0051_B3C0,
    8,
    0x0051_B1C0,
    "ReleasePrimaryFromThirdInterface"
);
adjust_interface!(
    fc4p_005200d0,
    0x0052_00D0,
    4,
    0x0069_5330,
    "AddRefPrimaryFromSecondInterface"
);
adjust_interface!(
    fc4p_005200f0,
    0x0052_00F0,
    8,
    0x0069_5330,
    "AddRefPrimaryFromThirdInterface"
);
adjust_interface!(
    fc4p_00520110,
    0x0052_0110,
    12,
    0x0069_5330,
    "AddRefPrimaryFromFourthInterface"
);
adjust_interface!(
    fc4p_005243e0,
    0x0052_43E0,
    12,
    0x0052_4410,
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

game_fn! { #[allow(dead_code)] fn allocate_object(size: u32) -> u64 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn construct_action_eleven(this: *mut u8) -> u64 = "thiscall" 0x0050_5BF0; }
game_fn! { #[allow(dead_code)] fn construct_action_eight(this: *mut u8) -> u64 = "thiscall" 0x0050_5290; }
game_fn! { #[allow(dead_code)] fn construct_action_fifteen(this: *mut u8) -> u64 = "thiscall" 0x0050_5C40; }
game_fn! { #[allow(dead_code)] fn construct_action_fourteen(this: *mut u8) -> u64 = "thiscall" 0x0050_5C70; }
game_fn! { #[allow(dead_code)] fn construct_action_sixteen(this: *mut u8) -> u64 = "thiscall" 0x0050_6370; }
game_fn! { #[allow(dead_code)] fn construct_action_seventeen(this: *mut u8) -> u64 = "thiscall" 0x0050_5CA0; }
game_fn! { #[allow(dead_code)] fn construct_action_twenty(this: *mut u8) -> u64 = "thiscall" 0x0050_52D0; }
game_fn! { #[allow(dead_code)] fn construct_action_nineteen(this: *mut u8) -> u64 = "thiscall" 0x0050_9A20; }
game_fn! { #[allow(dead_code)] fn construct_action_fifty_two(this: *mut u8) -> u64 = "thiscall" 0x0050_6420; }
game_fn! { #[allow(dead_code)] fn construct_action_twenty_one(this: *mut u8) -> u64 = "thiscall" 0x0050_6330; }
game_fn! { #[allow(dead_code)] fn destroy_automaton_tree(this: *mut u8) -> u64 = "thiscall" 0x0051_3E00; }
game_fn! { #[allow(dead_code)] fn clear_lot_list(this: *mut u8) -> u64 = "thiscall" 0x0047_3EC0; }
game_fn! { #[allow(dead_code)] fn destroy_model_processor(this: *mut u8) -> u64 = "thiscall" 0x0080_30A0; }
game_fn! { #[allow(dead_code)] fn destroy_triple_vector(this: *mut u8) -> u64 = "thiscall" 0x005E_8C40; }
game_fn! { #[allow(dead_code)] fn erase_eight_byte_range(this: *mut u8, first: u32, last: u32) -> u64 = "thiscall" 0x0071_0B40; }
game_fn! { #[allow(dead_code)] fn insert_eight_byte_values(this: *mut u8, position: u32, count: u32, value: *mut u8) -> u64 = "thiscall" 0x0063_8E80; }

port! {
    /// Allocates an action of the selected kind, runs its constructor, and sets its kind field.
    /// Other kinds use the base action, whose three padding bytes after +0x14 remain untouched.
    #[port(address = 0x0050_B7F0, owner = "shared", name = "CreateAction")]
    pub unsafe extern "fastcall" fn fc4p_0050b7f0(entry_ecx: *mut u8, entry_edx: u32, kind: u32) -> u64 {
        let (size, constructor) = match kind {
            8 => (0x34, 0x0050_5290),
            11 => (0x40, 0x0050_5BF0),
            14 => (0x40, 0x0050_5C70),
            15 => (0x30, 0x0050_5C40),
            16 => (0x274, 0x0050_6370),
            17 => (0x58, 0x0050_5CA0),
            19 => (0x34, 0x0050_9A20),
            20 => (0x2C, 0x0050_52D0),
            21 => (0x44, 0x0050_6330),
            52 => (0x2C, 0x0050_6420),
            _ => (0x28, 0),
        };
        // SAFETY: allocation and construction use the game's entries and layouts.
        unsafe {
            let mut result = invoke(Conv::Cdecl, crate::abi::target(0x005E_55E0), entry_ecx,
                kind.wrapping_sub(8), entry_edx, &[size]);
            let allocation = result as u32 as *mut u8;
            if allocation.is_null() { return result; }
            if constructor != 0 {
                result = invoke(Conv::Thiscall, crate::abi::target(constructor), allocation,
                    allocation as u32, (result >> 32) as u32, &[]);
            } else {
                store(allocation, 0, 0x00A9_5384);
                for offset in [4, 8, 12, 16, 24, 28, 32, 36] { store(allocation, offset, 0); }
                allocation.add(0x14).write(0);
            }
            if result as u32 != 0 { store(result as u32 as *mut u8, 4, kind); }
            result
        }
    }
}

port! {
    /// Restores the automaton's six interface tables, destroys embedded containers and processors,
    /// and releases retained interfaces. Pointer fields retain their values throughout destruction.
    #[port(address = 0x0051_3E60, owner = "shared", name = "DestroyAutomaton")]
    pub unsafe extern "fastcall" fn fc4p_00513e60(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the game's complete automaton, embedded objects, and retained interfaces.
        unsafe {
            for (offset, table) in [(0, 0x00A9_5AC0), (4, 0x00A9_5A38), (8, 0x00A9_5A1C),
                (12, 0x00A9_59B8), (0x20, 0x00A9_59A0), (0x38, 0x00A9_5990)] {
                store(this, offset, table);
            }
            let buffer = word(this, 0x1C8);
            let mut result = registers(buffer, entry_edx);
            if buffer != 0 {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this,
                    buffer, entry_edx, &[buffer]);
            }
            result = invoke(Conv::Thiscall, crate::abi::target(0x0051_3E00), this.add(0x18C),
                result as u32, (result >> 32) as u32, &[]);
            result = invoke(Conv::Thiscall, crate::abi::target(0x0047_3EC0), this.add(0x188),
                result as u32, (result >> 32) as u32, &[]);
            let buffer = word(this, 0x188);
            if buffer != 0 {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this.add(0x188),
                    result as u32, (result >> 32) as u32, &[buffer]);
            }
            for (processor, buffer) in [(0x12C, 0x110), (0xD0, 0xCC)] {
                result = invoke(Conv::Thiscall, crate::abi::target(0x0080_30A0), this.add(processor),
                    result as u32, (result >> 32) as u32, &[]);
                let value = word(this, buffer);
                result = registers(value, (result >> 32) as u32);
                if value != 0 {
                    if buffer == 0xCC {
                        let adjustment = word(word(value as *mut u8, 4) as *mut u8, 4);
                        let base = value.wrapping_add(adjustment).wrapping_add(4) as *mut u8;
                        result = virtual_call(base, 8, word(base, 0), adjustment, &[]);
                    } else {
                        result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this.add(processor),
                            value, (result >> 32) as u32, &[value]);
                    }
                }
            }
            for offset in [0xC0, 0xB4] {
                let buffer = word(this, offset);
                result = registers(buffer, (result >> 32) as u32);
                if buffer != 0 {
                    result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this,
                        buffer, (result >> 32) as u32, &[buffer]);
                }
            }
            for offset in [0xB0, 0x98] {
                let retained = word(this, offset) as *mut u8;
                if !retained.is_null() {
                    let table = word(retained, 0);
                    result = if offset == 0xB0 {
                        virtual_call(retained, 8, result as u32, table, &[])
                    } else { virtual_call(retained, 8, table, (result >> 32) as u32, &[]) };
                }
            }
            invoke(Conv::Thiscall, crate::abi::target(0x005E_8C40), this.add(4),
                result as u32, (result >> 32) as u32, &[])
        }
    }
}

port! {
    /// Shrinks eight-byte entries by erasing a suffix, or inserts the requested number at the end.
    /// The insertion value is an uninitialized eight-byte temporary supplied to the original helper.
    #[port(address = 0x0051_FC30, owner = "shared", name = "ResizeEightByteVector", naked)]
    pub unsafe extern "C" fn fc4p_0051fc30() {
        // Keep the temporary at its original stack address, including the bytes left by the caller.
        core::arch::naked_asm!("sub esp, 8", "mov edx, esp", "push [esp + 12]", "call {body}",
            "add esp, 8", "ret 4", body = sym resize_eight_byte_vector);
    }
}

unsafe extern "fastcall" fn resize_eight_byte_vector(this: *mut u8, value: *mut u8, count: u32) -> u64 {
    // SAFETY: the original vector fields and range/insert signatures. The temporary is never read here.
    unsafe {
        let begin = word(this, 0);
        let end = word(this, 4);
        let size = (end.wrapping_sub(begin) as i32 >> 3) as u32;
        if count < size {
            let first = begin.wrapping_add(count.wrapping_mul(8));
            invoke(
                Conv::Thiscall,
                crate::abi::target(0x0071_0B40),
                this,
                first,
                begin,
                &[first, end],
            )
        } else {
            let additional = count.wrapping_sub(size);
            invoke(
                Conv::Thiscall,
                crate::abi::target(0x0063_8E80),
                this,
                additional,
                begin,
                &[end, additional, value as u32],
            )
        }
    }
}

game_fn! { #[allow(dead_code)] fn erase_unsigned_tree(this: *mut u8, root: u32) -> u64 = "thiscall" 0x0046_F820; }
game_fn! { #[allow(dead_code)] fn reserve_bucket_vector(this: *mut u8, count: u32) -> u64 = "thiscall" 0x0045_D4F0; }
game_fn! { #[allow(dead_code)] fn insert_bucket_values(this: *mut u8, position: u32, count: u32, value: *const u32) -> u64 = "thiscall" 0x004F_BA20; }
game_fn! { #[allow(dead_code)] fn lower_bound_bucket_count(first: *const u32, last: *const u32, key: *const u32, hint: u32, comparator: u32) -> u64 = "C" 0x0079_FA90; }
game_fn! { #[allow(dead_code)] fn construct_bucket_vector(this: *mut u8, count: u32, value: *const u32, allocator: *const u32) -> u64 = "thiscall" 0x0051_C950; }
game_data! { static bucket_prime_first: u32 = 0x00A8_29B0; }
game_data! { static bucket_prime_last: u32 = 0x00A8_2A20; }

/// The successor links use +4 for parent, +8 for left, and +12 for right. The header participates
/// in the parent chain, so the final right-link comparison also handles the largest element.
unsafe fn unsigned_tree_successor(node: u32) -> u32 {
    // SAFETY: the caller supplies a live node in the game's unsigned-key tree.
    unsafe {
        let mut node = node;
        let mut next = word(node as *mut u8, 12);
        if next != 0 {
            node = next;
            while word(node as *mut u8, 8) != 0 {
                node = word(node as *mut u8, 8);
            }
        } else {
            next = word(node as *mut u8, 4);
            while node == word(next as *mut u8, 12) {
                node = next;
                next = word(next as *mut u8, 4);
            }
            if word(node as *mut u8, 12) != next {
                node = next;
            }
        }
        node
    }
}

port! {
    /// Collects an object's unsigned keys into a temporary tree, increments existing source counts,
    /// destroys the temporary tree, and returns one in AL while keeping the final free's upper bytes.
    #[port(address = 0x0050_C6A0, owner = "shared", name = "CountObjectSourceKeys")]
    pub unsafe extern "thiscall" fn fc4p_0050c6a0(this: *mut u8, object: *mut u8) -> u64 {
        // SAFETY: the original object, key collector, unsigned-key tree, and source-count hash table.
        unsafe {
            let header = allocate_array(20);
            let mut tree = [header as u32, 0u32];
            header.write(0);
            store(header, 4, 0);
            store(header, 8, header as u32);
            store(header, 12, header as u32);
            let keys = virtual_call(object, 12, header as u32, word(object, 0), &[]);
            let keys = keys as u32 as *mut u8;
            let collected = virtual_call(keys, 0x60, keys as u32, word(keys, 0), &[tree.as_mut_ptr() as u32]);
            if collected as u8 != 0 {
                let sentinel = tree[0];
                let mut node = word(sentinel as *mut u8, 8);
                while node != sentinel {
                    let begin = word(this, 0xA68);
                    let buckets = (word(this, 0xA6C).wrapping_sub(begin) as i32 >> 2) as u32;
                    let key = word(node as *mut u8, 16);
                    let bucket = unsigned_remainder(key, buckets);
                    let mut entry = word(begin.wrapping_add(bucket.wrapping_mul(4)) as *mut u8, 0);
                    while entry != 0 && word(entry as *mut u8, 4) != key {
                        entry = word(entry as *mut u8, 0);
                    }
                    if entry != 0 { store(entry as *mut u8, 8, word(entry as *mut u8, 8).wrapping_add(1)); }
                    node = unsigned_tree_successor(node);
                }
            }
            let mut result = collected;
            if tree[1] != 0 {
                result = invoke(Conv::Thiscall, crate::abi::target(0x0046_F820), tree.as_mut_ptr().cast(),
                    result as u32, word(tree[0] as *mut u8, 4), &[word(tree[0] as *mut u8, 4)]);
                store(tree[0] as *mut u8, 8, tree[0]);
                store(tree[0] as *mut u8, 4, 0);
                store(tree[0] as *mut u8, 12, tree[0]);
                tree[1] = 0;
            }
            result = registers(tree[0], (result >> 32) as u32);
            if tree[0] != 0 {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this,
                    tree[0], (result >> 32) as u32, &[tree[0]]);
            }
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Copies a four-byte payload hash table, keeping chain order and allocating eight bytes per node.
    /// Existing bucket storage is reset and reserved through its original vector helpers.
    #[port(address = 0x0050_DA70, owner = "shared", name = "CopyWordHashTable")]
    pub unsafe extern "fastcall" fn fc4p_0050da70(this: *mut u8, entry_edx: u32, source: *mut u8) -> u64 {
        // SAFETY: the original two hash tables, vector helpers, and eight-byte node allocator.
        unsafe {
            store(this, 8, word(this, 4));
            let size = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
            invoke(Conv::Thiscall, crate::abi::target(0x0045_D4F0), this.add(4),
                size, entry_edx, &[size]);
            let count = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
            let zero = 0u32;
            let end = word(this, 8);
            let mut result = invoke(Conv::Thiscall, crate::abi::target(0x004F_BA20), this.add(4),
                end, count, &[end, count, &zero as *const u32 as u32]);
            let mut count = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
            let mut index = 0u32;
            while index < count {
                let mut node = word(word(source, 4).wrapping_add(index.wrapping_mul(4)) as *mut u8, 0);
                let mut previous = 0u32;
                while node != 0 {
                    result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF54), this,
                        result as u32, (result >> 32) as u32, &[8]);
                    let allocation = result as u32;
                    store(allocation as *mut u8, 0, 0);
                    if allocation.wrapping_add(4) != 0 {
                        store(allocation as *mut u8, 4, word(node as *mut u8, 4));
                    }
                    if previous == 0 {
                        store(word(this, 4).wrapping_add(index.wrapping_mul(4)) as *mut u8, 0, allocation);
                    } else { store(previous as *mut u8, 0, allocation); }
                    previous = allocation;
                    node = word(node as *mut u8, 0);
                }
                count = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
                index = index.wrapping_add(1);
            }
            let elements = word(source, 16);
            store(this, 16, elements);
            if count == 0 { registers(elements, (result >> 32) as u32) }
                else { registers(count, this as u32) }
        }
    }
}

port! {
    /// Grows a byte-key hash table to the selected prime bucket count. Moves existing nodes by
    /// prepending them to new chains, empties old buckets, and frees only the old bucket array.
    #[port(address = 0x0051_CE70, owner = "shared", name = "GrowByteKeyHashTable")]
    pub unsafe extern "fastcall" fn fc4p_0051ce70(this: *mut u8, entry_edx: u32, requested: u32) -> u64 {
        // SAFETY: the original table, prime range, vector constructor, and chained byte-key nodes.
        unsafe {
            let old_count = (word(this, 8).wrapping_sub(word(this, 4)) as i32 >> 2) as u32;
            if requested <= old_count { return registers(requested, entry_edx); }
            let mut requested = requested;
            let first = bucket_prime_first();
            let last = bucket_prime_last();
            let result = invoke(Conv::Cdecl, crate::abi::target(0x0079_FA90), (&mut requested as *mut u32).cast(),
                requested, entry_edx, &[first as u32, last as u32, &requested as *const u32 as u32, requested, 0]);
            let selected = result as u32 as *mut u32;
            requested = if selected == last { 0xFFFF_FFFB } else { selected.read_unaligned() };
            if requested <= old_count { return registers(requested, (result >> 32) as u32); }
            let zero = 0u32;
            let mut temporary = MaybeUninit::<[u32; 3]>::uninit();
            construct_bucket_vector(temporary.as_mut_ptr().cast(), requested, &zero, &requested);
            let temporary = temporary.assume_init();
            let new_begin = temporary[0];
            for index in 0..old_count {
                let mut old_bucket = word(this, 4).wrapping_add(index.wrapping_mul(4)) as *mut u8;
                let mut node = word(old_bucket, 0);
                while node != 0 {
                    let key = (node as *const u8).add(4).read();
                    let bucket = unsigned_remainder(u32::from(key), requested);
                    store(old_bucket, 0, word(node as *mut u8, 0));
                    let new_bucket = new_begin.wrapping_add(bucket.wrapping_mul(4)) as *mut u8;
                    store(node as *mut u8, 0, word(new_bucket, 0));
                    store(new_bucket, 0, node);
                    old_bucket = word(this, 4).wrapping_add(index.wrapping_mul(4)) as *mut u8;
                    node = word(old_bucket, 0);
                }
            }
            let old_begin = word(this, 4);
            store(this, 4, temporary[0]);
            store(this, 8, temporary[1]);
            store(this, 12, temporary[2]);
            if old_begin == 0 { registers(0, temporary[1]) } else {
                invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), temporary[2] as *mut u8,
                    old_begin, temporary[1], &[old_begin])
            }
        }
    }
}

game_fn! { #[allow(dead_code)] fn update_automata(this: *mut u8) -> u64 = "thiscall" 0x0050_F2C0; }

port! {
    /// Updates the manager, then refreshes each automaton's source interface, cached item and
    /// optional primary interface. Replacements retain the new pointer before storing and releasing.
    #[port(address = 0x0054_F7B0, owner = "shared", name = "RefreshAutomatonInterfaces", naked)]
    pub unsafe extern "C" fn fc4p_0054f7b0() {
        core::arch::naked_asm!("push eax", "call {body}", "ret", body = sym refresh_automaton_interfaces);
    }
}

unsafe extern "fastcall" fn refresh_automaton_interfaces(this: *mut u8, entry_edx: u32, entry_eax: u32) -> u64 {
    // SAFETY: the original manager's list, automata and reference-counted interfaces.
    unsafe {
        let mut result = invoke(
            Conv::Thiscall,
            crate::abi::target(0x0050_F2C0),
            this,
            entry_eax,
            entry_edx,
            &[],
        );
        let active = this.add(0x10).read();
        result = (result & !0xFF) | u64::from(active);
        if active == 0 {
            return result;
        }
        let header = word(this, 0x44);
        result = registers(header, (result >> 32) as u32);
        let mut node = word(header as *mut u8, 0);
        while node != word(this, 0x44) {
            let automaton = word(node as *mut u8, 8) as *mut u8;
            let source = word(automaton, 0x1EC) as *mut u8;
            if !source.is_null() {
                result = virtual_call(source, 0xC4, word(source, 0), (result >> 32) as u32, &[]);
                let replacement = result as u32;
                let previous = word(automaton, 0x1F0);
                if replacement != previous {
                    if replacement != 0 {
                        result = virtual_call(
                            replacement as *mut u8,
                            4,
                            replacement,
                            word(replacement as *mut u8, 0),
                            &[],
                        );
                    }
                    store(automaton, 0x1F0, replacement);
                    if previous != 0 {
                        result = virtual_call(
                            previous as *mut u8,
                            8,
                            word(previous as *mut u8, 0),
                            (result >> 32) as u32,
                            &[],
                        );
                    }
                }
                let retained = word(automaton, 0x1F0) as *mut u8;
                if !retained.is_null() {
                    let key = word(automaton, 0x1F4);
                    result = virtual_call(retained, 12, key, word(retained, 0), &[key, 0]);
                    store(automaton, 0x1F8, result as u32);
                }
            }
            result = virtual_call(automaton, 0x258, result as u32, word(automaton, 0), &[]);
            if result as u8 != 0 {
                result = virtual_call(automaton, 0x250, word(automaton, 0), (result >> 32) as u32, &[]);
                let replacement = result as u32;
                let previous = word(automaton, 0xB0);
                if replacement != previous {
                    if replacement != 0 {
                        result = virtual_call(
                            replacement as *mut u8,
                            4,
                            replacement,
                            word(replacement as *mut u8, 0),
                            &[],
                        );
                    }
                    store(automaton, 0xB0, replacement);
                    if previous != 0 {
                        result = virtual_call(
                            previous as *mut u8,
                            8,
                            word(previous as *mut u8, 0),
                            (result >> 32) as u32,
                            &[],
                        );
                    }
                }
            }
            node = word(node as *mut u8, 0);
        }
        result
    }
}

game_fn! { fn count_automaton_pointer(this: *mut u8, value: *const u32) -> u32 = "thiscall" 0x0052_5E80; }
game_fn! { fn insert_automaton_pointer(this: *mut u8, out: *mut u32, value: *const u32) = "thiscall" 0x0045_D650; }
game_fn! { #[allow(dead_code)] fn copy_triples(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0050_BCB0; }

layout! {
    /// Manager fields used by collection, exemplar selection and building refresh.
    struct SharedAutomataManager {
        0x044 => list / set_list: u32,
        0xA18 => exemplars / set_exemplars: u32,
        0xA24 => groups / set_groups: u32,
        0xA44 => default_group / set_default_group: u32,
        0xA54 => key_buckets / set_key_buckets: u32,
        0xA58 => key_buckets_end / set_key_buckets_end: u32,
        0xA7C => cell_service / set_cell_service: *mut u8,
    }
}

layout! {
    /// A three-word vector header, also used for bucket arrays.
    struct SharedWordVector {
        0x00 => begin / set_begin: u32,
        0x04 => end / set_end: u32,
        0x08 => capacity / set_capacity: u32,
    }
}

layout! {
    /// The two common words of chained key, pointer and exemplar nodes.
    struct SharedWordChain {
        0x00 => next / set_next: u32,
        0x04 => value / set_value: u32,
    }
}

layout! {
    /// The circular list node, whose value is an automaton interface.
    struct SharedAutomatonListNode {
        0x00 => next / set_next: u32,
        0x04 => previous / set_previous: u32,
        0x08 => object / set_object: u32,
    }
}

layout! {
    /// Hash table fields surrounding its bucket vector at +4.
    struct SharedTripleHashTable {
        0x04 => buckets / set_buckets: u32,
        0x08 => buckets_end / set_buckets_end: u32,
        0x10 => count / set_count: u32,
    }
}

layout! {
    /// A hash node with its triple-vector header at +8, word payload and byte flag.
    struct SharedTripleHashNode {
        0x00 => next / set_next: u32,
        0x04 => key / set_key: u32,
        0x14 => extra / set_extra: u32,
        0x18 => flag / set_flag: u8,
    }
}

layout! {
    /// Cell masks selecting exemplar categories.
    struct SharedExemplarCell {
        0x20 => first_mask / set_first_mask: u32,
        0x24 => second_mask / set_second_mask: u32,
        0x28 => third_mask / set_third_mask: u32,
    }
}

layout! {
    /// The mask words at the start of each 108-byte exemplar record.
    struct SharedExemplarMasks {
        0x00 => first / set_first: u32,
        0x04 => second / set_second: u32,
        0x08 => third / set_third: u32,
        0x0C => styles / set_styles: u32,
    }
}

/// Adds one live, matching member if the destination does not already own it. The value is a local
/// word because the membership and insertion helpers take its address.
unsafe fn collect_matching_automaton(object: u32, key: u32, destination: *mut u8) {
    // SAFETY: the original automaton interfaces and pointer-set operations.
    unsafe {
        let value = object;
        let object = object as *mut u8;
        if !object.is_null() && vcall!(object, 0x7C => fn(u32) -> u32, 1) as u8 == 0 {
            let properties = vcall!(object, 0x14 => fn() -> *mut u8);
            if vcall!(properties, 0x58 => fn(u32) -> u32, key) as u8 != 0
                && count_automaton_pointer(destination, &value) == 0
            {
                vcall!(object, 4 => fn() -> u32);
                // The insertion result holds a two-word iterator followed by an inserted byte.
                let mut out = MaybeUninit::<[u32; 3]>::uninit();
                insert_automaton_pointer(destination, out.as_mut_ptr().cast(), &value);
            }
        }
    }
}

port! {
    /// Collects matching retained automata into a set. A cached key selects a hash-set traversal;
    /// missing keys scan the manager's list. Only new members are retained. The result is whether
    /// the destination count is nonzero, with EDX cleared even when no member was visited.
    #[port(address = 0x0051_0040, owner = "shared", name = "CollectMatchingAutomata")]
    pub unsafe extern "thiscall" fn fc4p_00510040(this: *mut u8, key: u32, destination: *mut u8) -> u64 {
        // SAFETY: the manager's bucket array, chains, circular list, and live destination set.
        unsafe {
            let manager = SharedAutomataManager::from_ptr(this);
            let buckets = manager.key_buckets();
            let count = (manager.key_buckets_end().wrapping_sub(buckets) as i32 >> 2) as u32;
            let bucket = unsigned_remainder(key, count);
            let mut node = word(buckets.wrapping_add(bucket.wrapping_mul(4)) as *mut u8, 0);
            while node != 0 && SharedWordChain::from_ptr(node as *mut u8).value() != key {
                node = SharedWordChain::from_ptr(node as *mut u8).next();
            }
            if node == 0 {
                let mut cursor = word(manager.list() as *mut u8, 0);
                while cursor != manager.list() {
                    let member = SharedAutomatonListNode::from_ptr(cursor as *mut u8);
                    collect_matching_automaton(member.object(), key, destination);
                    cursor = member.next();
                }
            } else {
                let mut iterator = MaybeUninit::<[u32; 2]>::uninit();
                begin_pointer_set((node as *mut u8).add(8), iterator.as_mut_ptr().cast());
                let [mut cursor, table] = iterator.assume_init();
                while cursor != 0 {
                    let member = SharedWordChain::from_ptr(cursor as *mut u8);
                    collect_matching_automaton(member.value(), key, destination);
                    let next = member.next();
                    if next != 0 {
                        cursor = next;
                    } else {
                        let buckets_view = SharedWordVector::from_ptr((table as *mut u8).add(4));
                        let buckets = buckets_view.begin();
                        let count = (buckets_view.end().wrapping_sub(buckets) as i32 >> 2) as u32;
                        let mut bucket = unsigned_remainder(member.value(), count);
                        let count = (buckets_view.end().wrapping_sub(buckets) as i32 >> 2) as u32;
                        cursor = 0;
                        loop {
                            bucket = bucket.wrapping_add(1);
                            if bucket >= count { break; }
                            cursor = word(buckets_view.begin().wrapping_add(bucket.wrapping_mul(4)) as *mut u8, 0);
                            if cursor != 0 { break; }
                        }
                    }
                }
            }
            registers(u32::from(word(destination, 0x10) != 0), 0)
        }
    }
}

port! {
    /// Copies hash buckets and their 28-byte nodes, preserving chain order, nested triple vectors,
    /// and the untouched padding after each node's final byte. ECX is also an observable result.
    #[port(address = 0x0054_0160, owner = "shared", name = "CopyTripleVectorHashTable", naked)]
    pub unsafe extern "C" fn fc4p_00540160() {
        // Marshal the third result register through a word in this entry's dead stack.
        core::arch::naked_asm!("sub esp, 4", "lea eax, [esp]", "push eax", "push [esp + 12]",
            "call {body}", "pop ecx", "ret 4", body = sym copy_triple_vector_hash_table);
    }
}

unsafe extern "fastcall" fn copy_triple_vector_hash_table(
    this: *mut u8,
    entry_edx: u32,
    source: *mut u8,
    output_ecx: *mut u32,
) -> u64 {
    // SAFETY: the original tables, bucket-vector helpers, node allocator and nested-vector copier.
    unsafe {
        let destination_view = SharedTripleHashTable::from_ptr(this);
        let source_view = SharedTripleHashTable::from_ptr(source);
        destination_view.set_buckets_end(destination_view.buckets());
        let count = (source_view.buckets_end().wrapping_sub(source_view.buckets()) as i32 >> 2) as u32;
        invoke(
            Conv::Thiscall,
            crate::abi::target(0x0045_D4F0),
            this.add(4),
            count,
            entry_edx,
            &[count],
        );
        let count = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
        let zero = 0u32;
        let end = word(this, 8);
        let result = invoke(
            Conv::Thiscall,
            crate::abi::target(0x004F_BA20),
            this.add(4),
            end,
            count,
            &[end, count, &zero as *const u32 as u32],
        );
        let mut count = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
        if count == 0 {
            let elements = word(source, 16);
            store(this, 16, elements);
            output_ecx.write(0);
            return registers(elements, (result >> 32) as u32);
        }
        let mut index = 0u32;
        while index < count {
            let mut node = word(word(source, 4).wrapping_add(index.wrapping_mul(4)) as *mut u8, 0);
            let mut previous = 0u32;
            while node != 0 {
                let allocation = allocate_array(0x1C);
                let allocated_node = SharedTripleHashNode::from_ptr(allocation);
                let source_node = SharedTripleHashNode::from_ptr(node as *mut u8);
                allocated_node.set_next(0);
                if (allocation as u32).wrapping_add(4) != 0 {
                    allocated_node.set_key(source_node.key());
                    copy_triples(allocation.add(8), (node as *mut u8).add(8));
                    allocated_node.set_extra(source_node.extra());
                    allocated_node.set_flag(source_node.flag());
                }
                if previous == 0 {
                    store(
                        word(this, 4).wrapping_add(index.wrapping_mul(4)) as *mut u8,
                        0,
                        allocation as u32,
                    );
                } else {
                    store(previous as *mut u8, 0, allocation as u32);
                }
                previous = allocation as u32;
                node = word(node as *mut u8, 0);
            }
            count = (word(source, 8).wrapping_sub(word(source, 4)) as i32 >> 2) as u32;
            index = index.wrapping_add(1);
        }
        let elements = word(source, 16);
        store(this, 16, elements);
        output_ecx.write(elements);
        registers(count, this as u32)
    }
}

game_fn! { #[allow(dead_code)] fn handle_building_message(this: *mut u8, building: *mut u8) -> u64 = "thiscall" 0x004F_C900; }
game_data! { static default_automaton_position: u32 = 0x00B2_293C; }
game_data! { static position_cell_scale: f32 = 0x00A9_BCF4; }
game_data! { static position_cell_half: f32 = 0x00A8_4D2C; }
game_data! { static position_cell_bias: f32 = 0x00A9_BCF8; }

/// Converts a world coordinate with the game's biased float-bit rounding and exact-integer fixup.
/// The scaled value stays extended until the comparison; only the biased sum is stored as f32.
unsafe fn automaton_position_cell(coordinate: f32) -> (i32, u16) {
    let mut rounded = 0u32;
    let mut scaled = F80::ZERO;
    // SAFETY: all operands cover their stored values and the two x87 values are popped.
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{coordinate}]", "fmul dword ptr [{scale}]",
            "fld dword ptr [{half}]", "fsubr st(0), st(1)", "fadd dword ptr [{bias}]",
            "fstp dword ptr [{rounded}]", "fstp tbyte ptr [{scaled}]",
            coordinate = in(reg) &coordinate, scale = in(reg) position_cell_scale(),
            half = in(reg) position_cell_half(), bias = in(reg) position_cell_bias(),
            rounded = in(reg) &mut rounded, scaled = in(reg) &mut scaled,
            out("st(0)") _, out("st(1)") _,
        );
    }
    let cell = rounded.wrapping_add(0xB4C0_0000) as i32;
    let candidate = cell.wrapping_add(1);
    let status: u16;
    // SAFETY: the scaled value is compared with an exact signed integer load, using the quiet
    // comparison so a quiet NaN does not add an invalid-operation exception.
    unsafe {
        core::arch::asm!("fild dword ptr [{candidate}]", "fld tbyte ptr [{scaled}]", "fucompp", "fnstsw ax",
            candidate = in(reg) &candidate, scaled = in(reg) &scaled, out("ax") status,
            out("st(0)") _, out("st(1)") _);
    }
    (if status & 0x4400 == 0x4000 { candidate } else { cell }, status)
}

port! {
    /// Clears references to a building and refreshes eligible automata within its inclusive cell
    /// rectangle. The next list link is saved before callbacks; the sentinel is read again after.
    /// The entry preserves the four incoming stack words used as bounds when the building is null.
    #[port(address = 0x0054_ED90, owner = "shared", name = "RefreshAutomataForBuilding", naked)]
    pub unsafe extern "C" fn fc4p_0054ed90() {
        core::arch::naked_asm!(
            "sub esp, 16", "push [esp + 20]", "push eax", "lea eax, [esp + 8]", "push eax",
            "call {body}", "add esp, 16", "ret 4", body = sym refresh_automata_for_building);
    }
}

unsafe extern "fastcall" fn refresh_automata_for_building(
    this: *mut u8,
    entry_edx: u32,
    bounds: *mut i32,
    entry_eax: u32,
    building: *mut u8,
) -> u64 {
    // SAFETY: the manager's circular list, automaton interfaces and optional building interface.
    unsafe {
        let mut result = invoke(
            Conv::Thiscall,
            crate::abi::target(0x004F_C900),
            this,
            entry_eax,
            entry_edx,
            &[building as u32],
        );
        // The saved answer occupies the low byte of the original, callee-popped argument word.
        let answer = bounds.add(5).cast::<u8>();
        answer.write(result as u8);
        if !building.is_null() {
            result = virtual_call(building, 0xC0, word(building, 0), (result >> 32) as u32, &[]);
            let footprint = result as u32 as *mut u8;
            result = virtual_call(footprint, 0x2C, footprint as u32, word(footprint, 0), &[bounds as u32]);
        }
        let sentinel = word(this, 0x44);
        let mut cursor = word(sentinel as *mut u8, 0);
        let mut eax = sentinel;
        while cursor != word(this, 0x44) {
            let object = word(cursor as *mut u8, 8) as *mut u8;
            cursor = word(cursor as *mut u8, 0);
            if word(object, 0x1EC) == building as u32 {
                result = virtual_call(object, 0x210, eax, word(object, 0), &[0, 0, 0, 0]);
                result = virtual_call(object, 0x21C, word(object, 0), (result >> 32) as u32, &[]);
            } else {
                result = virtual_call(object, 0x7C, eax, word(object, 0), &[0x2000]);
                if result as u8 != 0 {
                    result = virtual_call(object, 0x7C, word(object, 0), (result >> 32) as u32, &[0x20000]);
                    if result as u8 == 0 {
                        let initial = default_automaton_position().read();
                        let mut position = [initial; 3];
                        result = virtual_call(
                            object,
                            0x1C,
                            position.as_mut_ptr() as u32,
                            word(object, 0),
                            &[position.as_mut_ptr() as u32],
                        );
                        let (x, _) = automaton_position_cell(f32::from_bits(position[0]));
                        let (z, status) = automaton_position_cell(f32::from_bits(position[2]));
                        let status_eax = (result as u32 & 0xFFFF_0000) | u32::from(status);
                        result = registers(status_eax, x as u32);
                        if x >= bounds.read()
                            && z >= bounds.add(1).read()
                            && x <= bounds.add(2).read()
                            && z <= bounds.add(3).read()
                        {
                            result = virtual_call(object, 0x21C, status_eax, word(object, 0), &[]);
                        }
                    }
                }
            }
            eax = this as u32;
        }
        registers((eax & !0xFF) | u32::from(answer.read()), (result >> 32) as u32)
    }
}

game_fn! { #[allow(dead_code)] fn append_word(this: *mut u8, value: *const u32) -> u64 = "thiscall" 0x007E_BD30; }
game_fn! { #[allow(dead_code)] fn insert_words(this: *mut u8, position: u32, source: *const u32, out: *mut u32, count: u32, stride: u32) -> u64 = "thiscall" 0x0051_CA60; }
game_fn! { #[allow(dead_code)] fn random_below(this: *mut u8, limit: u32) -> u64 = "thiscall" 0x0091_3A06; }

port! {
    /// Chooses an exemplar from a group, optionally filtering three cell masks and one style bit.
    /// Zero cell masks bypass cell filtering; an absent cell uses all-one masks. Temporary candidate
    /// vectors are destroyed after selection. Group zero uses the manager's configured default.
    #[port(address = 0x0050_BE90, owner = "shared", name = "ChooseGroupExemplar")]
    pub unsafe extern "thiscall" fn fc4p_0050be90(
        this: *mut u8, group: u32, style: u32, cell_x: i32, cell_z: i32,
    ) -> u64 {
        // SAFETY: the manager's group tree, exemplar array, cell service, random source and game vectors.
        unsafe {
            let manager = SharedAutomataManager::from_ptr(this);
            let mut value = if group == 0 { manager.default_group() } else { group };
            let mut found = MaybeUninit::<u32>::uninit();
            let out = found.as_mut_ptr();
            let mut result = invoke(Conv::Thiscall, crate::abi::target(0x0049_7520), this.add(0xA24),
                value, out as u32, &[out as u32, &value as *const u32 as u32]);
            let node = found.assume_init();
            if node == manager.groups() { return registers(0, (result >> 32) as u32); }
            let mut temporary = [0u32; 3];
            let temp = temporary.as_mut_ptr().cast::<u8>();
            let mut vector = (node as *mut u8).add(20);
            if cell_x >= -1 && cell_z >= -1 {
                let service = manager.cell_service();
                result = virtual_call(service, 0x38, cell_z as u32, word(service, 0), &[cell_x as u32, cell_z as u32]);
                let cell = result as u32 as *mut u8;
                let masks = if cell.is_null() { [u32::MAX; 3] } else {
                    let cell = SharedExemplarCell::from_ptr(cell);
                    [cell.first_mask(), cell.second_mask(), cell.third_mask()]
                };
                if masks.iter().all(|&mask| mask != 0) {
                    let mut cursor = word(vector, 0);
                    while cursor != word(vector, 4) {
                        value = word(cursor as *mut u8, 0);
                        let data = manager.exemplars();
                        let exemplar = data.wrapping_add(value.wrapping_mul(0x6C)) as *mut u8;
                        let fields = SharedExemplarMasks::from_ptr(exemplar);
                        result = registers(exemplar as u32, data);
                        if fields.third() & masks[2] != 0 && fields.first() & masks[0] != 0 {
                            result = registers(exemplar as u32, masks[1]);
                            if fields.second() & masks[1] != 0 {
                                result = invoke(Conv::Thiscall, crate::abi::target(0x007E_BD30), temp,
                                    &value as *const u32 as u32, masks[1], &[&value as *const u32 as u32]);
                            }
                        }
                        cursor = cursor.wrapping_add(4);
                    }
                    vector = temp;
                }
            }
            if style != 0 {
                let mask = 1u32.wrapping_shl(style);
                let mut cursor = word(vector, 0);
                if vector == temp {
                    let mut next = cursor.wrapping_add(4);
                    while cursor != word(vector, 4) {
                        let offset = word(cursor as *mut u8, 0).wrapping_mul(0x6C);
                        let data = word(this, 0xA18);
                        result = registers(this as u32, offset);
                        if word(data.wrapping_add(offset) as *mut u8, 12) & mask == 0 {
                            let end = word(vector, 4);
                            if next != end {
                                let size = end.wrapping_sub(next);
                                result = invoke(Conv::Cdecl, crate::abi::target(0x009E_EB70), data as *mut u8,
                                    size, offset, &[cursor, next, size]);
                            }
                            store(vector, 4, word(vector, 4).wrapping_sub(4));
                        } else {
                            cursor = cursor.wrapping_add(4);
                            next = next.wrapping_add(4);
                        }
                    }
                } else {
                    while cursor != word(vector, 4) {
                        value = word(cursor as *mut u8, 0);
                        let offset = value.wrapping_mul(0x6C);
                        let data = word(this, 0xA18);
                        result = registers(value, offset);
                        if word(data.wrapping_add(offset) as *mut u8, 12) & mask != 0 {
                            let end = word(temp, 4);
                            if end != word(temp, 8) {
                                if end != 0 { store(end as *mut u8, 0, value); }
                                store(temp, 4, end.wrapping_add(4));
                            } else {
                                let ptr = &mut value as *mut u32 as u32;
                                result = invoke(Conv::Thiscall, crate::abi::target(0x0051_CA60), temp,
                                    ptr, ptr, &[end, ptr, ptr, 1, 1]);
                            }
                        }
                        cursor = cursor.wrapping_add(4);
                    }
                    vector = temp;
                }
            }
            let begin = word(vector, 0);
            let end = word(vector, 4);
            let mut selected = 0;
            if begin != end {
                let count = (end.wrapping_sub(begin) as i32 >> 2) as u32;
                result = invoke(Conv::Thiscall, crate::abi::target(0x0091_3A06), this.add(0x50),
                    count, (result >> 32) as u32, &[count]);
                let index = word(word(vector, 0).wrapping_add((result as u32).wrapping_mul(4)) as *mut u8, 0);
                selected = word(this, 0xA18).wrapping_add(index.wrapping_mul(0x6C));
                result = registers(result as u32, this as u32);
            }
            let allocation = word(temp, 0);
            if allocation != 0 {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), word(vector, 0) as *mut u8,
                    allocation, (result >> 32) as u32, &[allocation]);
            }
            registers(selected, (result >> 32) as u32)
        }
    }
}

game_fn! { #[allow(dead_code)] fn construct_model_bounds(this: *mut u8) -> u64 = "thiscall" 0x0046_6E10; }
game_fn! { #[allow(dead_code)] fn light_attachment(this: *mut u8, lights: *mut u8, flags: u32) -> u64 = "thiscall" 0x0080_3E10; }
game_data! { static attachments_enabled: u8 = 0x00B0_9968; }
game_data! { static attachment_service: *mut u8 = 0x00B4_3D44; }
game_data! { static selected_automaton_owner: *mut u8 = 0x00B2_1D74; }
game_data! { static attachment_coordinates: *mut u8 = 0x00B4_3D0C; }
game_data! { static attachment_direction: *mut u8 = 0x00B4_3DD8; }
game_data! { static attachment_renderer: *mut u8 = 0x00B4_3DD0; }
game_data! { static attachment_lights: *mut u8 = 0x00B4_3DDC; }
game_data! { static attachment_extent_scale: f32 = 0x00A8_80D4; }

/// Moves the translation by 1.5 times the model's y extent along the stored direction. The first
/// two direction products are rounded before subtraction; the third stays in an x87 register.
unsafe fn offset_attachment_transform(bounds: *mut u8, transform: *mut u8, direction: *mut u8) {
    let mut delta = [0u32; 3];
    // SAFETY: six bounds words, the original transform, and the direction's three float fields.
    // Every pushed x87 value is popped; the stores and register arithmetic keep their original order.
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{bounds} + 16]", "fsub dword ptr [{bounds} + 4]",
            "fmul dword ptr [{scale}]", "fld st(0)", "fmul dword ptr [{direction} + 0x54]",
            "fstp dword ptr [{delta}]", "fld st(0)", "fmul dword ptr [{direction} + 0x58]",
            "fstp dword ptr [{delta} + 4]", "fmul dword ptr [{direction} + 0x5c]",
            "fld dword ptr [{transform} + 40]", "fsub dword ptr [{delta}]",
            "fld dword ptr [{transform} + 44]", "fsub dword ptr [{delta} + 4]",
            "fld dword ptr [{transform} + 48]", "fsub st(0), st(3)",
            "fstp dword ptr [{delta} + 8]", "fxch st(1)",
            "fstp dword ptr [{transform} + 40]", "fstp dword ptr [{transform} + 44]",
            "fstp st(0)",
            bounds = in(reg) bounds, transform = in(reg) transform, direction = in(reg) direction,
            scale = in(reg) attachment_extent_scale(), delta = in(reg) delta.as_mut_ptr(),
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
        );
        store(transform, 48, delta[2]);
    }
}

port! {
    /// Enables or disables the selected model attachment, updates its transform from the first
    /// model's bounds, submits it to the renderer, and lights its processor when present.
    #[port(address = 0x0050_AFD0, owner = "shared", name = "UpdateSelectedModelAttachment", naked)]
    pub unsafe extern "C" fn fc4p_0050afd0() {
        // Keep the transform's untouched padding at the original stack location.
        core::arch::naked_asm!(
            "sub esp, 0x78", "lea eax, [esp + 0x40]", "push eax",
            "lea eax, [esp + 0x2c]", "push eax", "call {body}", "add esp, 0x78", "ret",
            body = sym update_selected_model_attachment,
        );
    }
}

unsafe extern "fastcall" fn update_selected_model_attachment(
    this: *mut u8,
    entry_edx: u32,
    bounds: *mut u8,
    transform: *mut u8,
) -> u64 {
    // SAFETY: the original automaton, its services, model interfaces and caller's temporary storage.
    unsafe {
        let mut result = virtual_call(this.add(4), 0x1C, word(this, 4), entry_edx, &[]);
        if result as u32 == 0x496E_5783 {
            result = virtual_call(this.add(4), 0x1C, result as u32, word(this, 4), &[]);
            if result as u32 == 0x897F_7EC2 {
                return result;
            }
        }
        let enabled = *attachments_enabled();
        result = (result & !0xFF) | u64::from(enabled);
        if enabled == 0 {
            return virtual_call(this, 0x1FC, word(this, 0), (result >> 32) as u32, &[]);
        }
        let service = *attachment_service();
        result = virtual_call(service, 0x34, result as u32, word(service, 0), &[]);
        let active = result as u8 != 0;
        result = registers(word(this, 0xCC), (result >> 32) as u32);
        if !active {
            return if result as u32 == 0 {
                result
            } else {
                virtual_call(this, 0x1FC, word(this, 0), (result >> 32) as u32, &[])
            };
        }
        if result as u32 == 0 && word(this, 0x110) != word(this, 0x114) {
            let mut queried = 0u32;
            let service = *attachment_service();
            result = virtual_call(
                service,
                0,
                &mut queried as *mut u32 as u32,
                word(service, 0),
                &[0x098F_9BF4, &mut queried as *mut u32 as u32],
            );
            if queried != 0 {
                let mut matched = false;
                if !(*selected_automaton_owner()).is_null() {
                    let interface = queried as *mut u8;
                    result = virtual_call(interface, 0x34, result as u32, word(interface, 0), &[]);
                    if result as u8 != 0 {
                        let owner = *selected_automaton_owner();
                        result = registers(owner as u32, (result >> 32) as u32);
                        let selected = word(owner, 0x30) as *mut u8;
                        if !selected.is_null() {
                            result = virtual_call(selected, 0xE4, result as u32, word(selected, 0), &[0x10]);
                            let selected = result as u32 as *mut u8;
                            if !selected.is_null() {
                                let coordinates = *attachment_coordinates();
                                let mut cell = [0u32; 2];
                                result = virtual_call(
                                    coordinates,
                                    0x44,
                                    word(coordinates, 0),
                                    word(this, 0x40),
                                    &[
                                        word(this, 0x40),
                                        word(this, 0x48),
                                        cell.as_mut_ptr() as u32,
                                        cell.as_mut_ptr().add(1) as u32,
                                    ],
                                );
                                let x = word(selected, 0x26C);
                                result = registers(x, (result >> 32) as u32);
                                matched = x == cell[0] && word(selected, 0x270) == cell[1];
                            }
                        }
                    }
                }
                if queried != 0 {
                    let interface = queried as *mut u8;
                    result = virtual_call(interface, 8, result as u32, word(interface, 0), &[]);
                }
                if matched && word(this, 0xCC) == 0 {
                    return virtual_call(this, 0x1F8, word(this, 0), (result >> 32) as u32, &[]);
                }
            }
        }
        if word(this, 0xCC) == 0 || word(this, 0x110) == word(this, 0x114) {
            return result;
        }
        let model = word(word(this, 0x110) as *mut u8, 0) as *mut u8;
        for (index, value) in [
            0x3F80_0000,
            0,
            0,
            0,
            0x3F80_0000,
            0,
            0,
            0,
            0x3F80_0000,
            0,
            0,
            0,
            0x3F80_0000,
        ]
        .into_iter()
        .enumerate()
        {
            store(transform, 4 + index * 4, value);
        }
        transform.add(1).write(0);
        transform.write(0);
        let adjustment = word(word(model, 4) as *mut u8, 4);
        let interface = model.wrapping_add(adjustment as usize).wrapping_add(4);
        result = virtual_call(
            interface,
            0x24,
            transform as u32,
            word(interface, 0),
            &[transform as u32],
        );
        result = invoke(
            Conv::Thiscall,
            crate::abi::target(0x0046_6E10),
            bounds,
            result as u32,
            (result >> 32) as u32,
            &[],
        );
        result = virtual_call(model, 0, result as u32, word(model, 0), &[]);
        let shape = result as u32 as *mut u8;
        virtual_call(shape, 0x1C, shape as u32, word(shape, 0), &[bounds as u32]);
        transform.add(1).write(transform.add(1).read() | 1);
        transform.write(transform.read().wrapping_add(1));
        offset_attachment_transform(bounds, transform, *attachment_direction());
        let attachment = word(this, 0xCC) as *mut u8;
        let adjustment = word(word(attachment, 4) as *mut u8, 4);
        let interface = attachment.wrapping_add(adjustment as usize).wrapping_add(4);
        result = virtual_call(
            interface,
            0x28,
            word(interface, 0),
            transform as u32,
            &[transform as u32],
        );
        let attachment = word(this, 0xCC) as *mut u8;
        let renderer = *attachment_renderer();
        let target = vslot(renderer, 0x78);
        result = virtual_call(attachment, 0, word(attachment, 0), (result >> 32) as u32, &[]);
        result = invoke(
            Conv::Thiscall,
            target,
            renderer,
            result as u32,
            (result >> 32) as u32,
            &[result as u32],
        );
        result = registers(word(this, 0xD0), (result >> 32) as u32);
        if result as u32 != 0 {
            result = invoke(
                Conv::Thiscall,
                crate::abi::target(0x0080_3E10),
                this.add(0xD0),
                result as u32,
                *attachment_lights() as u32,
                &[*attachment_lights() as u32, 0],
            );
        }
        result
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_0050afd0::PORT,
    fc4p_0050be90::PORT,
    fc4p_0054ed90::PORT,
    fc4p_00510040::PORT,
    fc4p_00540160::PORT,
    fc4p_0050b7f0::PORT,
    fc4p_00513e60::PORT,
    fc4p_0051fc30::PORT,
    fc4p_0050c6a0::PORT,
    fc4p_0050da70::PORT,
    fc4p_0051ce70::PORT,
    fc4p_0054f7b0::PORT,
    fc4p_0050b290::PORT,
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
    fc4p_0054da50::PORT,
    fc4p_0054ec60::PORT,
    fc4p_0054ed10::PORT,
];

#[cfg(test)]
mod tests {
    thread_local! {
        static ATTACHMENT_TYPES: RefCell<std::collections::VecDeque<u32>> = const { RefCell::new(std::collections::VecDeque::new()) };
        static ATTACHMENT_TRANSFORMS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    }

    extern "thiscall" fn attachment_type(this: *mut u8) -> u64 {
        log(0x1C, this, &[]);
        registers(
            ATTACHMENT_TYPES.with(|q| q.borrow_mut().pop_front().unwrap_or(0)),
            0x1122_3344,
        )
    }
    extern "thiscall" fn attachment_active(this: *mut u8) -> u64 {
        log(0x34, this, &[]);
        registers(unsafe { word(this, 8) }, 0x2233_4455)
    }
    extern "thiscall" fn attachment_query(this: *mut u8, iid: u32, out: *mut u32) -> u64 {
        log(0, this, &[iid]);
        unsafe { out.write(word(this, 4)) };
        RESULT
    }
    extern "thiscall" fn attachment_selected(this: *mut u8, flags: u32) -> u64 {
        log(0xE4, this, &[flags]);
        registers(unsafe { word(this, 8) }, 0x3344_5566)
    }
    extern "thiscall" fn attachment_cell(this: *mut u8, x: u32, z: u32, a: *mut u32, b: *mut u32) -> u64 {
        log(0x44, this, &[x, z]);
        unsafe {
            a.write(word(this, 4));
            b.write(word(this, 8));
        }
        RESULT
    }
    extern "thiscall" fn attachment_enable(this: *mut u8) -> u64 {
        log(0x1F8, this, &[]);
        RESULT
    }
    extern "thiscall" fn attachment_disable(this: *mut u8) -> u64 {
        log(0x1FC, this, &[]);
        RESULT
    }
    extern "thiscall" fn attachment_shape(this: *mut u8) -> u64 {
        log(0x1000, this, &[]);
        registers(unsafe { word(this, 8) }, 0x5566_7788)
    }
    extern "thiscall" fn attachment_bounds_new(this: *mut u8) -> u64 {
        log(0x0046_6E10, this, &[]);
        unsafe {
            for i in 0..6 {
                store(this, i * 4, if i < 3 { 1f32.to_bits() } else { 0 });
            }
        }
        RESULT
    }
    extern "thiscall" fn attachment_bounds(this: *mut u8, out: *mut u8) -> u64 {
        log(0x101C, this, &[]);
        unsafe {
            for (i, f) in [91f32, 2.0, 7.0, 98.0, 6.0, 17.0].into_iter().enumerate() {
                store(out, i * 4, f.to_bits());
            }
        }
        RESULT
    }
    extern "thiscall" fn attachment_get_transform(this: *mut u8, out: *mut u8) -> u64 {
        log(0x24, this, &[]);
        unsafe {
            assert_eq!(out.read(), 0);
            assert_eq!(out.add(1).read(), 0);
            for i in 0..13 {
                assert_eq!(word(out, 4 + i * 4), if i % 4 == 0 { 1f32.to_bits() } else { 0 });
            }
            out.write(255);
            out.add(1).write(0x80);
            store(out, 4, 123f32.to_bits());
            for (i, f) in [7f32, 8.0, 9.0].into_iter().enumerate() {
                store(out, 40 + i * 4, f.to_bits());
            }
        }
        RESULT
    }
    extern "thiscall" fn attachment_set_transform(this: *mut u8, input: *mut u8) -> u64 {
        log(0x28, this, &[]);
        ATTACHMENT_TRANSFORMS.with(|t| t.borrow_mut().push(bytes(input, 56)));
        RESULT
    }
    extern "thiscall" fn attachment_submit(this: *mut u8, shape: u32) -> u64 {
        log(0x78, this, &[shape]);
        RESULT
    }
    extern "thiscall" fn attachment_light(this: *mut u8, lights: u32, flags: u32) -> u64 {
        log(0x0080_3E10, this, &[lights, flags]);
        RESULT
    }

    struct AttachmentFixture {
        object: Object,
        service: Object,
        queried: Object,
        owner: Object,
        selector: Object,
        selected: Object,
        coordinates: Object,
        model: Object,
        shape: Object,
        model_interface: Object,
        attached: Object,
        attached_interface: Object,
        renderer: Object,
        direction: Object,
        table: Box<[usize; 128]>,
        globals: Box<[u32; 8]>,
        models: Box<[u32; 1]>,
        adjustment: Box<[u32; 2]>,
        _fakes: Vec<crate::testing::Fake>,
    }
    impl AttachmentFixture {
        fn new() -> Self {
            let mut f = Self {
                object: Object::new(0x1D0),
                service: Object::new(16),
                queried: Object::new(16),
                owner: Object::new(0x34),
                selector: Object::new(16),
                selected: Object::new(0x274),
                coordinates: Object::new(16),
                model: Object::new(16),
                shape: Object::new(16),
                model_interface: Object::new(16),
                attached: Object::new(16),
                attached_interface: Object::new(16),
                renderer: Object::new(16),
                direction: Object::new(0x60),
                table: Box::new([0; 128]),
                globals: Box::new([0; 8]),
                models: Box::new([0]),
                adjustment: Box::new([0; 2]),
                _fakes: Vec::new(),
            };
            for (slot, function) in [
                (0, attachment_query as *const ()),
                (8, release as *const ()),
                (0x1C, attachment_type as *const ()),
                (0x24, attachment_get_transform as *const ()),
                (0x28, attachment_set_transform as *const ()),
                (0x34, attachment_active as *const ()),
                (0x44, attachment_cell as *const ()),
                (0x78, attachment_submit as *const ()),
                (0xE4, attachment_selected as *const ()),
                (0x1F8, attachment_enable as *const ()),
                (0x1FC, attachment_disable as *const ()),
            ] {
                f.table[slot / 4] = function as usize;
            }
            unsafe {
                for object in [
                    &mut f.object,
                    &mut f.service,
                    &mut f.queried,
                    &mut f.selector,
                    &mut f.coordinates,
                    &mut f.model_interface,
                    &mut f.attached_interface,
                    &mut f.renderer,
                ] {
                    store(object.ptr(), 0, f.table.as_ptr() as u32);
                }
                store(f.object.ptr(), 4, f.table.as_ptr() as u32);
                store(f.service.ptr(), 4, f.queried.ptr() as u32);
                store(f.service.ptr(), 8, 1);
                store(f.queried.ptr(), 8, 1);
                store(f.owner.ptr(), 0x30, f.selector.ptr() as u32);
                store(f.selector.ptr(), 8, f.selected.ptr() as u32);
                store(f.selected.ptr(), 0x26C, 0x7FC0_1234);
                store(f.selected.ptr(), 0x270, 0x8000_0000);
                store(f.coordinates.ptr(), 4, 0x7FC0_1234);
                store(f.coordinates.ptr(), 8, 0x8000_0000);
                f.adjustment[1] = (f.model_interface.ptr() as u32)
                    .wrapping_sub(f.model.ptr() as u32)
                    .wrapping_sub(4);
                // Both model bases are separately allocated; each has its own adjustment table below.
                store(f.model.ptr(), 4, f.adjustment.as_ptr() as u32);
                store(f.model.ptr(), 0, f.table.as_ptr() as u32);
                store(f.model.ptr(), 8, f.shape.ptr() as u32);
                store(f.attached.ptr(), 0, f.table.as_ptr() as u32);
                store(f.attached.ptr(), 8, f.shape.ptr() as u32);
                f.models[0] = f.model.ptr() as u32;
                store(f.object.ptr(), 0x110, f.models.as_ptr() as u32);
                store(f.object.ptr(), 0x114, f.models.as_ptr() as u32 + 4);
                for (i, v) in [0.25f32, -0.5, 1.25].into_iter().enumerate() {
                    store(f.direction.ptr(), 0x54 + 4 * i, v.to_bits());
                }
            }
            f.globals.copy_from_slice(&[
                1,
                f.service.ptr() as u32,
                f.owner.ptr() as u32,
                f.coordinates.ptr() as u32,
                f.direction.ptr() as u32,
                f.renderer.ptr() as u32,
                0x1234_5678,
                1.5f32.to_bits(),
            ]);
            for (i, addr) in [
                0x00B0_9968,
                0x00B4_3D44,
                0x00B2_1D74,
                0x00B4_3D0C,
                0x00B4_3DD8,
                0x00B4_3DD0,
                0x00B4_3DDC,
                0x00A8_80D4,
            ]
            .into_iter()
            .enumerate()
            {
                f._fakes.push(crate::testing::fake_data(addr, unsafe {
                    f.globals.as_mut_ptr().add(i).cast()
                }));
            }
            f._fakes
                .push(fake(0x0046_6E10, attachment_bounds_new as *const () as usize));
            f._fakes.push(fake(0x0080_3E10, attachment_light as *const () as usize));
            f
        }
        fn run(&mut self) -> u64 {
            run(fc4p_0050afd0 as *const (), self.object.ptr(), &[], 0xDEAD_BEEF, 9)
        }
    }

    #[test]
    fn attachment_type_checks_disabled_service_and_empty_model_paths_preserve_return_words() {
        let mut f = AttachmentFixture::new();
        let p = f.object.ptr();
        ATTACHMENT_TYPES.with(|q| q.borrow_mut().extend([0x496E_5783, 0x897F_7EC2]));
        assert_eq!(f.run(), registers(0x897F_7EC2, 0x1122_3344));
        assert_eq!(
            calls(),
            [Call(0x1C, p as usize + 4, vec![]), Call(0x1C, p as usize + 4, vec![])]
        );
        f.globals[0] = 0;
        ATTACHMENT_TYPES.with(|q| q.borrow_mut().extend([0x496E_5783, 0x1234]));
        assert_eq!(f.run(), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x1C, p as usize + 4, vec![]),
                Call(0x1C, p as usize + 4, vec![]),
                Call(0x1FC, p as usize, vec![])
            ]
        );
        f.globals[0] = 1;
        unsafe {
            store(f.service.ptr(), 8, 0);
        }
        for attachment in [0u32, 0x1234] {
            unsafe {
                store(p, 0xCC, attachment);
            }
            let before = bytes(p, 0x1D0);
            assert_eq!(
                f.run(),
                if attachment == 0 {
                    registers(0, 0x2233_4455)
                } else {
                    RESULT
                }
            );
            let mut expected = vec![
                Call(0x1C, p as usize + 4, vec![]),
                Call(0x34, f.service.ptr() as usize, vec![]),
            ];
            if attachment != 0 {
                expected.push(Call(0x1FC, p as usize, vec![]));
            }
            assert_eq!(calls(), expected);
            assert_eq!(bytes(p, 0x1D0), before);
        }
        unsafe {
            store(p, 0xCC, 0);
            store(f.service.ptr(), 8, 1);
            store(p, 0x114, f.models.as_ptr() as u32);
        }
        assert_eq!(f.run(), registers(0, 0x2233_4455));
        assert_eq!(calls().iter().map(|c| c.0).collect::<Vec<_>>(), [0x1C, 0x34]);
    }

    #[test]
    fn attachment_selection_compares_coordinate_bits_and_releases_the_queried_interface() {
        for route in 0..8 {
            let mut f = AttachmentFixture::new();
            let p = f.object.ptr();
            unsafe {
                match route {
                    0 => store(f.service.ptr(), 4, 0),
                    1 => f.globals[2] = 0,
                    2 => store(f.queried.ptr(), 8, 0),
                    3 => store(f.owner.ptr(), 0x30, 0),
                    4 => store(f.selector.ptr(), 8, 0),
                    5 => store(f.coordinates.ptr(), 4, 0x7FC0_1235),
                    6 => store(f.coordinates.ptr(), 8, 0),
                    _ => (),
                }
            }
            let before = bytes(p, 0x1D0);
            let result = f.run();
            let mut expected = vec![
                Call(0x1C, p as usize + 4, vec![]),
                Call(0x34, f.service.ptr() as usize, vec![]),
                Call(0, f.service.ptr() as usize, vec![0x098F_9BF4]),
            ];
            if route >= 2 {
                expected.push(Call(0x34, f.queried.ptr() as usize, vec![]));
            }
            if route >= 4 {
                expected.push(Call(0xE4, f.selector.ptr() as usize, vec![0x10]));
            }
            if route >= 5 {
                expected.push(Call(0x44, f.coordinates.ptr() as usize, vec![0, 0]));
            }
            if route != 0 {
                expected.push(Call(8, f.queried.ptr() as usize, vec![]));
            }
            if route == 7 {
                expected.push(Call(0x1F8, p as usize, vec![]));
            }
            assert_eq!(calls(), expected, "route {route}");
            assert_eq!(result, RESULT);
            assert_eq!(bytes(p, 0x1D0), before);
        }
    }

    #[test]
    fn attachment_transform_uses_adjusted_interfaces_then_submits_and_optionally_lights() {
        let _fpu = crate::testing::GameFpu::new();
        for light in [false, true] {
            let mut f = AttachmentFixture::new();
            let mut shape_table = [0usize; 8];
            shape_table[7] = attachment_bounds as *const () as usize;
            let mut model_table = [0usize; 1];
            model_table[0] = attachment_shape as *const () as usize;
            let mut attached_adjustment = [
                0u32,
                (f.attached_interface.ptr() as u32)
                    .wrapping_sub(f.attached.ptr() as u32)
                    .wrapping_sub(4),
            ];
            let p = f.object.ptr();
            unsafe {
                store(f.model.ptr(), 0, model_table.as_ptr() as u32);
                store(f.attached.ptr(), 0, model_table.as_ptr() as u32);
                store(f.attached.ptr(), 4, attached_adjustment.as_mut_ptr() as u32);
                store(f.shape.ptr(), 0, shape_table.as_ptr() as u32);
                store(p, 0xCC, f.attached.ptr() as u32);
                store(p, 0xD0, u32::from(light));
            }
            let before = bytes(p, 0x1D0);
            assert_eq!(
                f.run(),
                if light {
                    RESULT
                } else {
                    registers(0, (RESULT >> 32) as u32)
                }
            );
            let observed = calls();
            let mut ids = vec![0x1C, 0x34, 0x24, 0x0046_6E10, 0x1000, 0x101C, 0x28, 0x1000, 0x78];
            if light {
                ids.push(0x0080_3E10);
            }
            assert_eq!(observed.iter().map(|c| c.0).collect::<Vec<_>>(), ids);
            assert_eq!(observed[2].1, f.model_interface.ptr() as usize);
            assert_eq!(observed[6], Call(0x28, f.attached_interface.ptr() as usize, vec![]));
            assert_eq!(
                observed[8],
                Call(0x78, f.renderer.ptr() as usize, vec![f.shape.ptr() as u32])
            );
            if light {
                assert_eq!(observed[9], Call(0x0080_3E10, p as usize + 0xD0, vec![0x1234_5678, 0]));
            }
            let transforms = ATTACHMENT_TRANSFORMS.with(|t| std::mem::take(&mut *t.borrow_mut()));
            assert_eq!(transforms.len(), 1);
            let t = &transforms[0];
            assert_eq!(&t[..2], &[0, 0x81]);
            for (offset, expected) in [(4, 123f32), (40, 5.5), (44, 11.0), (48, 1.5), (52, 1.0)] {
                assert_eq!(&t[offset..offset + 4], &expected.to_bits().to_le_bytes());
            }
            assert_eq!(bytes(p, 0x1D0), before);
        }
    }

    #[test]
    fn attachment_third_product_keeps_the_x87_exponent_range_until_subtraction() {
        let _fpu = crate::testing::GameFpu::new();
        let mut f = AttachmentFixture::new();
        let mut bounds = [0f32, 0.0, 0.0, 0.0, 2.0e38, 0.0];
        let mut transform = [0u32; 14];
        for i in 0..3 {
            transform[10 + i] = 3.0e38f32.to_bits();
            unsafe {
                store(f.direction.ptr(), 0x54 + 4 * i, 2f32.to_bits());
            }
        }
        unsafe {
            offset_attachment_transform(
                bounds.as_mut_ptr().cast(),
                transform.as_mut_ptr().cast(),
                f.direction.ptr(),
            );
        }
        assert_eq!(&transform[10..12], &[f32::NEG_INFINITY.to_bits(); 2]);
        assert!(f32::from_bits(transform[12]).is_finite());
        assert!(f32::from_bits(transform[12]) < -2.9e38);
    }

    #[test]
    fn building_refresh_saves_successor_before_callbacks_and_rereads_the_sentinel() {
        let _fake = fake(0x004F_C900, building_message as *const () as usize);
        let mut table = [0usize; 0x220 / 4];
        table[0x210 / 4] = building_clear as *const () as usize;
        table[0x21C / 4] = building_refresh as *const () as usize;
        for replace_sentinel in [false, true] {
            let mut manager = Object::new(0x50);
            let mut first = Object::new(0x1F0);
            let mut second = Object::new(0x1F0);
            let m = manager.ptr();
            let a = first.ptr();
            let b = second.ptr();
            let mut header = [0u32; 3];
            let mut last = [header.as_mut_ptr() as u32, 0, b as u32];
            let mut node = [last.as_mut_ptr() as u32, 0, a as u32];
            header[0] = node.as_mut_ptr() as u32;
            unsafe {
                store(m, 0x44, header.as_mut_ptr() as u32);
                store(a, 0, table.as_ptr() as u32);
                store(b, 0, table.as_ptr() as u32);
            }
            if replace_sentinel {
                mutation(0x210, m, 0x44, last.as_mut_ptr() as u32);
            } else {
                mutation(0x210, node.as_mut_ptr().cast(), 0, header.as_mut_ptr() as u32);
            }
            assert_eq!(
                run(fc4p_0054ed90 as *const (), m, &[0], 0, 0),
                registers((m as u32 & !0xFF) | RESULT as u8 as u32, 0x6655_4433)
            );
            let mut expected = vec![
                Call(0x004F_C900, m as usize, vec![0]),
                Call(0x210, a as usize, vec![0; 4]),
                Call(0x21C, a as usize, vec![]),
            ];
            if !replace_sentinel {
                expected.extend([Call(0x210, b as usize, vec![0; 4]), Call(0x21C, b as usize, vec![])]);
            }
            assert_eq!(calls(), expected);
        }
    }

    #[test]
    fn matching_collection_scans_later_hash_buckets_after_callbacks() {
        let _fakes = [
            fake(0x0052_5E80, collect_count as *const () as usize),
            fake(0x0045_D650, collect_insert as *const () as usize),
            fake(0x004B_0390, collect_begin as *const () as usize),
        ];
        let mut object_table = [0usize; 0x80 / 4];
        object_table[1] = collect_retain as *const () as usize;
        object_table[0x14 / 4] = collect_properties as *const () as usize;
        object_table[0x7C / 4] = collect_flags as *const () as usize;
        let mut property_table = [0usize; 0x5C / 4];
        property_table[0x58 / 4] = collect_property_matches as *const () as usize;
        let mut properties = [property_table.as_ptr() as u32, 1];
        let mut objects = [[object_table.as_ptr() as u32, 0, properties.as_mut_ptr() as u32, 0]; 7];
        let first = objects
            .iter_mut()
            .find(|o| (o.as_ptr() as u32).is_multiple_of(7))
            .unwrap()
            .as_mut_ptr() as u32;
        let last = objects
            .iter_mut()
            .find(|o| o.as_ptr() as u32 % 7 == 6)
            .unwrap()
            .as_mut_ptr() as u32;
        for remove_last in [false, true] {
            let mut nodes = [[0u32, first], [0, last]];
            let mut buckets = [
                nodes[0].as_mut_ptr() as u32,
                0,
                0,
                0,
                0,
                0,
                nodes[1].as_mut_ptr() as u32,
            ];
            let mut pointer_table = [
                0u32,
                buckets.as_mut_ptr() as u32,
                buckets.as_mut_ptr() as u32 + 28,
                buckets.as_mut_ptr() as u32 + 28,
            ];
            let mut cache = [0u32, 1, nodes[0].as_mut_ptr() as u32, pointer_table.as_mut_ptr() as u32];
            let mut cache_buckets = [cache.as_mut_ptr() as u32];
            let mut manager = Object::new(0xA80);
            let m = manager.ptr();
            unsafe {
                store(m, 0xA54, cache_buckets.as_mut_ptr() as u32);
                store(m, 0xA58, cache_buckets.as_mut_ptr() as u32 + 4);
            }
            let mut destination = [0u32; 5];
            let d = destination.as_mut_ptr().cast::<u8>();
            if remove_last {
                mutation(4, buckets.as_mut_ptr().cast(), 24, 0);
            }
            assert_eq!(run(fc4p_00510040 as *const (), m, &[1, d as u32], 0, 0), 1);
            let mut expected = vec![Call(0x004B_0390, cache.as_ptr() as usize + 8, vec![])];
            for object in if remove_last { vec![first] } else { vec![first, last] } {
                expected.extend([
                    Call(0x7C, object as usize, vec![1]),
                    Call(0x14, object as usize, vec![]),
                    Call(0x58, properties.as_ptr() as usize, vec![1]),
                    Call(0x0052_5E80, d as usize, vec![object]),
                    Call(4, object as usize, vec![]),
                    Call(0x0045_D650, d as usize, vec![object]),
                ]);
            }
            assert_eq!(calls(), expected);
            assert_eq!(destination[4], if remove_last { 1 } else { 2 });
        }
    }

    #[test]
    fn triple_hash_self_copy_uses_the_reset_source_and_copy_callbacks_can_replace_bucket_storage() {
        let _fakes = [
            fake(0x0045_D4F0, bucket_reserve as *const () as usize),
            fake(0x004F_BA20, bucket_insert as *const () as usize),
            fake(0x0090_CF54, queued_allocate as *const () as usize),
            fake(0x0050_BCB0, clone_nested_triples as *const () as usize),
        ];
        let mut buckets = [0xABCD_1234u32; 2];
        let mut table = [
            0x1111u32,
            buckets.as_mut_ptr() as u32,
            buckets.as_mut_ptr() as u32 + 8,
            0x2222,
            7,
        ];
        let p = table.as_mut_ptr().cast::<u8>();
        assert_eq!(
            run(fc4p_00540160 as *const (), p, &[p as u32], 0, 0),
            registers(7, (RESULT >> 32) as u32)
        );
        assert_eq!(
            calls(),
            [
                Call(0x0045_D4F0, p as usize + 4, vec![0]),
                Call(0x004F_BA20, p as usize + 4, vec![buckets.as_ptr() as u32, 0, 0])
            ]
        );
        assert_eq!(table[2], table[1]);
        assert_eq!(buckets, [0xABCD_1234; 2]);
        let mut source_node = [0u32, 42, 1, 2, 3, 4, 5];
        let mut source_buckets = [source_node.as_mut_ptr() as u32];
        let mut source = [
            0u32,
            source_buckets.as_mut_ptr() as u32,
            source_buckets.as_mut_ptr() as u32 + 4,
            0,
            1,
        ];
        let mut old = [0xCCCC_CCCCu32; 2];
        let mut replacement = [0xDDDD_DDDDu32; 2];
        let mut destination = [0u32, old.as_mut_ptr() as u32, 0, old.as_mut_ptr() as u32 + 8, 9];
        let d = destination.as_mut_ptr().cast::<u8>();
        let mut allocation = [0xEEEE_EEEEu32; 7];
        ALLOCATIONS.with(|q| q.borrow_mut().push_back((28, allocation.as_mut_ptr() as u32)));
        mutation(0x0050_BCB0, d, 4, replacement.as_mut_ptr() as u32);
        assert_eq!(
            run(fc4p_00540160 as *const (), d, &[source.as_mut_ptr() as u32], 0, 0),
            registers(1, d as u32)
        );
        assert_eq!(
            calls(),
            [
                Call(0x0045_D4F0, d as usize + 4, vec![1]),
                Call(0x004F_BA20, d as usize + 4, vec![old.as_ptr() as u32, 1, 0]),
                Call(0x0090_CF54, 0, vec![28]),
                Call(
                    0x0050_BCB0,
                    allocation.as_ptr() as usize + 8,
                    vec![source_node.as_ptr() as u32 + 8]
                )
            ]
        );
        assert_eq!(old, [0, 0xCCCC_CCCC]);
        assert_eq!(replacement, [allocation.as_ptr() as u32, 0xDDDD_DDDD]);
        assert_eq!(destination[1], replacement.as_ptr() as u32);
        assert_eq!(destination[2], old.as_ptr() as u32 + 4);
    }

    thread_local! {
        static SELECTION_STORAGE: RefCell<u32> = const { RefCell::new(0) };
        static SELECTION_TEMP: RefCell<u32> = const { RefCell::new(0) };
        static SELECTION_VECTOR: RefCell<u32> = const { RefCell::new(0) };
        static SELECTION_OBSERVED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    extern "thiscall" fn selection_find(this: *mut u8, out: *mut u32, key: *const u32) -> u64 {
        log(0x0049_7520, this, &[unsafe { key.read() }]);
        unsafe { out.write(FOUND.with(|v| *v.borrow())) };
        registers(0xAABB_CCDD, 0xCAFE_1234)
    }

    extern "thiscall" fn selection_cell(this: *mut u8, x: i32, z: i32) -> u64 {
        log(0x38, this, &[x as u32, z as u32]);
        registers(unsafe { word(this, 4) }, 0xCDEF_5678)
    }

    unsafe fn selection_append_value(this: *mut u8, value: u32) {
        unsafe {
            if word(this, 0) == 0 {
                let storage = SELECTION_STORAGE.with(|v| *v.borrow());
                store(this, 0, storage);
                store(this, 4, storage);
                store(this, 8, storage + 64);
            }
            let end = word(this, 4);
            store(end as *mut u8, 0, value);
            store(this, 4, end + 4);
        }
        SELECTION_TEMP.with(|v| *v.borrow_mut() = this as u32);
        SELECTION_VECTOR.with(|v| *v.borrow_mut() = this as u32);
    }

    extern "thiscall" fn selection_append(this: *mut u8, value: *const u32) -> u64 {
        let value = unsafe { value.read() };
        log(0x007E_BD30, this, &[value]);
        unsafe { selection_append_value(this, value) };
        RESULT
    }

    extern "thiscall" fn selection_insert(
        this: *mut u8,
        position: u32,
        source: *const u32,
        out: *mut u32,
        count: u32,
        stride: u32,
    ) -> u64 {
        assert_eq!(source, out.cast_const());
        let value = unsafe { source.read() };
        log(0x0051_CA60, this, &[position, value, count, stride]);
        unsafe {
            selection_append_value(this, value);
            out.write(0x1234_5678);
        }
        RESULT
    }

    extern "thiscall" fn selection_random(this: *mut u8, limit: u32) -> u64 {
        log(0x0091_3A06, this, &[limit]);
        let vector = SELECTION_VECTOR.with(|v| *v.borrow()) as *mut u8;
        unsafe {
            let begin = word(vector, 0);
            assert_eq!(word(vector, 4) - begin, limit * 4);
            let values = core::slice::from_raw_parts(begin as *const u32, limit as usize).to_vec();
            SELECTION_OBSERVED.with(|v| *v.borrow_mut() = values);
        }
        registers(limit - 1, 0x1234_5678)
    }

    #[test]
    fn exemplar_selection_covers_default_groups_cell_masks_styles_compaction_and_empty_results() {
        let _fakes = [
            fake(0x0049_7520, selection_find as *const () as usize),
            fake(0x007E_BD30, selection_append as *const () as usize),
            fake(0x0051_CA60, selection_insert as *const () as usize),
            fake(0x0091_3A06, selection_random as *const () as usize),
            fake(0x009E_EB70, memmove as *const () as usize),
            fake(0x0090_CF63, free_array as *const () as usize),
        ];
        let masks = [[1u32, 1, 1], [2, 1, 1], [1, 2, 1], [1, 1, 2], [1, 1, 1], [1, 1, 1]];
        let styles = [4u32, 4, 4, 4, 2, 1];
        let mut exemplars = [[0xA5A5_A5A5u32; 27]; 6];
        for (index, exemplar) in exemplars.iter_mut().enumerate() {
            exemplar[..3].copy_from_slice(&masks[index]);
            exemplar[3] = styles[index];
        }
        let mut service_table = [0usize; 0x3C / 4];
        service_table[0x38 / 4] = selection_cell as *const () as usize;
        let mut cell = [0u32; 11];
        let mut service = [service_table.as_ptr() as u32, cell.as_mut_ptr() as u32];
        let mut indices = [0u32, 1, 2, 3, 4, 5];
        let mut node = [0x1357_2468u32; 8];
        node[5] = indices.as_mut_ptr() as u32;
        node[7] = indices.as_mut_ptr() as u32 + 24;
        let mut manager = Object::new(0xA80);
        let m = manager.ptr();
        unsafe {
            store(m, 0xA18, exemplars.as_mut_ptr() as u32);
            store(m, 0xA24, 0xF00D);
            store(m, 0xA44, 0xBEEF_CAFE);
            store(m, 0xA7C, service.as_mut_ptr() as u32);
        }
        let before_manager = bytes(m, 0xA80);
        let before_exemplars = exemplars;
        for length in [0usize, 6] {
            node[6] = node[5] + length as u32 * 4;
            let before_node = node;
            for (x, z) in [(0i32, 0i32), (-1, -1), (-2, 0), (0, -2), (i32::MIN, i32::MAX)] {
                for filter in 0..5 {
                    let absent_cell = filter == 4;
                    cell[8..11].fill(1);
                    if (1..4).contains(&filter) {
                        cell[7 + filter] = 0;
                    }
                    service[1] = if absent_cell { 0 } else { cell.as_mut_ptr() as u32 };
                    for style in [0u32, 2, 32, 33, 31] {
                        for group in [0u32, 0xABCD_1234] {
                            let mut storage = [0xCCCC_CCCCu32; 16];
                            SELECTION_STORAGE.with(|v| *v.borrow_mut() = storage.as_mut_ptr() as u32);
                            SELECTION_TEMP.with(|v| *v.borrow_mut() = 0);
                            SELECTION_VECTOR.with(|v| *v.borrow_mut() = node.as_mut_ptr() as u32 + 20);
                            SELECTION_OBSERVED.with(|v| v.borrow_mut().clear());
                            FOUND.with(|v| *v.borrow_mut() = node.as_mut_ptr() as u32);
                            let result = run(fc4p_0050be90 as *const (), m, &[group, style, x as u32, z as u32], 3, 4);
                            let spatial = x >= -1 && z >= -1;
                            let cell_filter = spatial && (filter == 0 || absent_cell);
                            let spatial_candidates: Vec<u32> = (0..length as u32)
                                .filter(|&index| !cell_filter || absent_cell || masks[index as usize] == [1, 1, 1])
                                .collect();
                            let candidates: Vec<u32> = spatial_candidates
                                .iter()
                                .copied()
                                .filter(|&index| style == 0 || styles[index as usize] & 1u32.wrapping_shl(style) != 0)
                                .collect();
                            let allocated = if cell_filter {
                                !spatial_candidates.is_empty()
                            } else {
                                style != 0 && !candidates.is_empty()
                            };
                            let expected_eax = candidates
                                .last()
                                .map_or(0, |&index| (exemplars.as_ptr() as u32).wrapping_add(index * 0x6C));
                            let expected_edx = if allocated {
                                (RESULT >> 32) as u32
                            } else if !candidates.is_empty() {
                                m as u32
                            } else if length != 0 && style != 0 {
                                (length as u32 - 1) * 0x6C
                            } else if spatial {
                                0xCDEF_5678
                            } else {
                                0xCAFE_1234
                            };
                            assert_eq!(
                                result,
                                registers(expected_eax, expected_edx),
                                "len={length} cell=({x},{z}) filter={filter} style={style}"
                            );
                            let temp = SELECTION_TEMP.with(|v| *v.borrow());
                            let mut expected = vec![Call(
                                0x0049_7520,
                                m as usize + 0xA24,
                                vec![if group == 0 { 0xBEEF_CAFE } else { group }],
                            )];
                            if spatial {
                                expected.push(Call(0x38, service.as_ptr() as usize, vec![x as u32, z as u32]));
                            }
                            let mut expected_storage = [0xCCCC_CCCCu32; 16];
                            if cell_filter {
                                for (position, index) in spatial_candidates.iter().copied().enumerate() {
                                    expected.push(Call(0x007E_BD30, temp as usize, vec![index]));
                                    expected_storage[position] = index;
                                }
                                if style != 0 {
                                    let mut position = 0;
                                    let mut live = spatial_candidates.len();
                                    while position < live {
                                        if styles[expected_storage[position] as usize] & 1u32.wrapping_shl(style) == 0 {
                                            if position + 1 != live {
                                                expected.push(Call(
                                                    0x009E_EB70,
                                                    storage.as_ptr() as usize + position * 4,
                                                    vec![
                                                        storage.as_ptr() as u32 + (position as u32 + 1) * 4,
                                                        (live as u32 - position as u32 - 1) * 4,
                                                    ],
                                                ));
                                                expected_storage.copy_within(position + 1..live, position);
                                            }
                                            live -= 1;
                                        } else {
                                            position += 1;
                                        }
                                    }
                                }
                            } else if style != 0 && !candidates.is_empty() {
                                expected.push(Call(0x0051_CA60, temp as usize, vec![0, candidates[0], 1, 1]));
                                expected_storage[..candidates.len()].copy_from_slice(&candidates);
                            }
                            if !candidates.is_empty() {
                                expected.push(Call(0x0091_3A06, m as usize + 0x50, vec![candidates.len() as u32]));
                            }
                            if allocated {
                                expected.push(Call(0x0090_CF63, storage.as_ptr() as usize, vec![]));
                            }
                            assert_eq!(calls(), expected);
                            assert_eq!(storage, expected_storage);
                            assert_eq!(SELECTION_OBSERVED.with(|v| v.borrow().clone()), candidates);
                            assert_eq!(bytes(m, 0xA80), before_manager);
                            assert_eq!(exemplars, before_exemplars);
                            assert_eq!(node, before_node);
                            assert_eq!(indices, [0, 1, 2, 3, 4, 5]);
                        }
                    }
                }
            }
        }
        FOUND.with(|v| *v.borrow_mut() = 0xF00D);
        assert_eq!(
            run(fc4p_0050be90 as *const (), m, &[0, u32::MAX, 0, 0], 0, 0),
            registers(0, 0xCAFE_1234)
        );
        assert_eq!(calls(), [Call(0x0049_7520, m as usize + 0xA24, vec![0xBEEF_CAFE])]);
    }

    extern "thiscall" fn building_message(this: *mut u8, building: *mut u8) -> u64 {
        log(0x004F_C900, this, &[building as u32]);
        RESULT
    }

    extern "thiscall" fn building_footprint(this: *mut u8) -> u64 {
        log(0xC0, this, &[]);
        registers(unsafe { word(this, 4) }, 0x1122_3344)
    }

    extern "thiscall" fn building_bounds(this: *mut u8, bounds: *mut i32) -> u64 {
        log(0x2C, this, &[]);
        unsafe {
            for index in 0..4 {
                bounds.add(index).write(word(this, 4 + index * 4) as i32);
            }
        }
        registers(0xABCD_1234, 0x5566_7788)
    }

    extern "thiscall" fn building_object_flags(this: *mut u8, mask: u32) -> u64 {
        log(0x7C, this, &[mask]);
        registers(unsafe { word(this, if mask == 0x2000 { 4 } else { 8 }) }, 0xA1B2_C3D4)
    }

    extern "thiscall" fn building_position(this: *mut u8, position: *mut u32) -> u64 {
        unsafe {
            let initial = [position.read(), position.add(1).read(), position.add(2).read()];
            log(0x1C, this, &initial);
            for index in 0..3 {
                position.add(index).write(word(this, 0x10 + index * 4));
            }
        }
        registers(0xF123_4567, 0xABC1_2345)
    }

    extern "thiscall" fn building_clear(this: *mut u8, a: u32, b: u32, c: u32, d: u32) -> u64 {
        log(0x210, this, &[a, b, c, d]);
        unsafe { store(this, 0x1EC, 0) };
        registers(0xEEFF_0011, 0xBBCC_DDEE)
    }

    extern "thiscall" fn building_refresh(this: *mut u8) -> u64 {
        log(0x21C, this, &[]);
        registers(0x9876_5432, 0x6655_4433)
    }

    #[unsafe(naked)]
    unsafe extern "C" fn building_null_bounds() {
        // Seed the bounds at exactly the stack addresses the called entry uses on a null building.
        core::arch::naked_asm!("push [esp + 4]", "mov dword ptr [esp - 20], -1",
            "mov dword ptr [esp - 16], -2", "mov dword ptr [esp - 12], 1",
            "mov dword ptr [esp - 8], 2", "call {port}", "ret 4", port = sym fc4p_0054ed90);
    }

    #[test]
    fn building_refresh_handles_clear_flags_inclusive_cells_null_building_and_empty_list() {
        let _fpu = crate::testing::GameFpu::new();
        let mut defaults = 0x7FC1_2345u32;
        let mut scale = 0.0625f32;
        let mut half = 0.5f32;
        let mut bias = 12_582_912.0f32;
        let _data = [
            crate::testing::fake_data(0x00B2_293C, (&mut defaults as *mut u32).cast()),
            crate::testing::fake_data(0x00A9_BCF4, (&mut scale as *mut f32).cast()),
            crate::testing::fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast()),
            crate::testing::fake_data(0x00A9_BCF8, (&mut bias as *mut f32).cast()),
        ];
        let _fake = fake(0x004F_C900, building_message as *const () as usize);
        let mut object_table = [0usize; 0x220 / 4];
        object_table[0x7C / 4] = building_object_flags as *const () as usize;
        object_table[0x1C / 4] = building_position as *const () as usize;
        object_table[0x210 / 4] = building_clear as *const () as usize;
        object_table[0x21C / 4] = building_refresh as *const () as usize;
        let mut footprint_table = [0usize; 0x30 / 4];
        footprint_table[0x2C / 4] = building_bounds as *const () as usize;
        let mut footprint = [footprint_table.as_ptr() as u32, (-1i32) as u32, (-2i32) as u32, 1, 2];
        let mut building_table = [0usize; 0xC4 / 4];
        building_table[0xC0 / 4] = building_footprint as *const () as usize;
        let mut building = [building_table.as_ptr() as u32, footprint.as_mut_ptr() as u32];
        let b = building.as_mut_ptr() as u32;
        for null_building in [false, true] {
            let building_arg = if null_building { 0 } else { b };
            let entry = if null_building {
                building_null_bounds as *const ()
            } else {
                fc4p_0054ed90 as *const ()
            };
            for (same_building, flags, hidden, x, z, cell_x, refresh) in [
                (true, 0, 0, 0.0f32, 0.0f32, 0i32, true),
                (false, 0xFFFF_0000, 0, 0.0, 0.0, 0, false),
                (false, 1, 0xABCD_0001, 0.0, 0.0, 0, false),
                (false, 1, 0xABCD_0000, -16.0, -32.0, -1, true),
                (false, 1, 0, 16.0, 32.0, 1, true),
                (false, 1, 0, 31.999, 47.999, 1, true),
                (false, 1, 0, -16.001, 0.0, -2, false),
                (false, 1, 0, 32.0, 0.0, 2, false),
                (false, 1, 0, 0.0, -32.001, 0, false),
                (false, 1, 0, 0.0, 48.0, 0, false),
            ] {
                let mut manager = Object::new(0x50);
                let mut object = Object::new(0x1F0);
                let m = manager.ptr();
                let o = object.ptr();
                let mut header = [0u32; 3];
                let mut node = [header.as_mut_ptr() as u32, 0, o as u32];
                header[0] = node.as_mut_ptr() as u32;
                unsafe {
                    store(m, 0x44, header.as_mut_ptr() as u32);
                    store(o, 0, object_table.as_ptr() as u32);
                    store(o, 4, flags);
                    store(o, 8, hidden);
                    store(o, 0x10, x.to_bits());
                    store(o, 0x14, f32::INFINITY.to_bits());
                    store(o, 0x18, z.to_bits());
                    store(o, 0x1EC, if same_building { building_arg } else { 0xBAD0_CAFE });
                }
                let before_manager = bytes(m, 0x50);
                let mut expected_object = bytes(o, 0x1F0);
                if same_building {
                    expected_object[0x1EC..0x1F0].fill(0);
                }
                let result = run(entry, m, &[building_arg], 0xAAAA_BBBB, 0xCCCC_DDDD);
                let float_path = !same_building && flags as u8 != 0 && hidden as u8 == 0;
                let edx = if refresh {
                    0x6655_4433
                } else if float_path {
                    cell_x as u32
                } else {
                    0xA1B2_C3D4
                };
                assert_eq!(result, registers((m as u32 & !0xFF) | RESULT as u8 as u32, edx));
                let mut expected = vec![Call(0x004F_C900, m as usize, vec![building_arg])];
                if !null_building {
                    expected.push(Call(0xC0, b as usize, vec![]));
                    expected.push(Call(0x2C, footprint.as_ptr() as usize, vec![]));
                }
                if same_building {
                    expected.push(Call(0x210, o as usize, vec![0; 4]));
                } else {
                    expected.push(Call(0x7C, o as usize, vec![0x2000]));
                    if flags as u8 != 0 {
                        expected.push(Call(0x7C, o as usize, vec![0x20000]));
                    }
                    if float_path {
                        expected.push(Call(0x1C, o as usize, vec![defaults; 3]));
                    }
                }
                if refresh {
                    expected.push(Call(0x21C, o as usize, vec![]));
                }
                assert_eq!(calls(), expected);
                assert_eq!(bytes(m, 0x50), before_manager);
                assert_eq!(bytes(o, 0x1F0), expected_object);
                unsafe { store(header.as_mut_ptr().cast(), 0, header.as_ptr() as u32) };
                assert_eq!(
                    run(entry, m, &[building_arg], 3, 4),
                    registers(
                        (header.as_ptr() as u32 & !0xFF) | RESULT as u8 as u32,
                        if null_building {
                            (RESULT >> 32) as u32
                        } else {
                            0x5566_7788
                        }
                    )
                );
                let mut empty_calls = vec![Call(0x004F_C900, m as usize, vec![building_arg])];
                if !null_building {
                    empty_calls.push(Call(0xC0, b as usize, vec![]));
                    empty_calls.push(Call(0x2C, footprint.as_ptr() as usize, vec![]));
                }
                assert_eq!(calls(), empty_calls);
            }
        }
    }

    #[test]
    fn position_cells_keep_negative_boundaries_float_bits_and_quiet_nan_status() {
        let _fpu = crate::testing::GameFpu::new();
        let mut scale = 0.0625f32;
        let mut half = 0.5f32;
        let mut bias = 12_582_912.0f32;
        let _data = [
            crate::testing::fake_data(0x00A9_BCF4, (&mut scale as *mut f32).cast()),
            crate::testing::fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast()),
            crate::testing::fake_data(0x00A9_BCF8, (&mut bias as *mut f32).cast()),
        ];
        for (value, expected) in [
            (0.0, 0i32),
            (-0.0, 0),
            (-0.001, -1),
            (-16.0, -1),
            (-16.001, -2),
            (15.999, 0),
            (16.0, 1),
            (32.0, 2),
            (f32::INFINITY, 0x3440_0000),
            (f32::NEG_INFINITY, 0xB440_0000u32 as i32),
            (f32::from_bits(0x7FC1_2345), 0x3481_2345),
        ] {
            unsafe { core::arch::asm!("fnclex", options(nostack)) };
            let (cell, status) = unsafe { automaton_position_cell(value) };
            assert_eq!(cell, expected, "input {:08x}", value.to_bits());
            assert_eq!(status & 1, 0, "input {:08x}", value.to_bits());
        }
        unsafe { core::arch::asm!("fnclex", options(nostack)) };
        let (_, status) = unsafe { automaton_position_cell(f32::from_bits(0x7F81_2345)) };
        assert_ne!(status & 1, 0);
    }

    extern "thiscall" fn collect_flags(this: *mut u8, mask: u32) -> u64 {
        log(0x7C, this, &[mask]);
        registers(unsafe { word(this, 4) }, 0xABCD_1234)
    }

    extern "thiscall" fn collect_properties(this: *mut u8) -> u64 {
        log(0x14, this, &[]);
        registers(unsafe { word(this, 8) }, 0xABCD_2345)
    }

    extern "thiscall" fn collect_property_matches(this: *mut u8, key: u32) -> u64 {
        log(0x58, this, &[key]);
        registers(unsafe { word(this, 4) }, 0xABCD_3456)
    }

    extern "thiscall" fn collect_count(this: *mut u8, value: *const u32) -> u64 {
        let value = unsafe { value.read() };
        log(0x0052_5E80, this, &[value]);
        registers(unsafe { word(value as *mut u8, 12) }, 0xABCD_4567)
    }

    extern "thiscall" fn collect_retain(this: *mut u8) -> u64 {
        log(4, this, &[]);
        RESULT
    }

    extern "thiscall" fn collect_insert(this: *mut u8, out: *mut u32, value: *const u32) -> u64 {
        log(0x0045_D650, this, &[unsafe { value.read() }]);
        unsafe {
            out.write(0x1122_3344);
            out.add(1).write(0x5566_7788);
            out.add(2).cast::<u8>().write(1);
            store(this, 16, word(this, 16).wrapping_add(1));
        }
        RESULT
    }

    extern "thiscall" fn collect_begin(this: *mut u8, out: *mut u32) -> u64 {
        log(0x004B_0390, this, &[]);
        unsafe {
            out.write(word(this, 0));
            out.add(1).write(word(this, 4));
        }
        RESULT
    }

    #[test]
    fn matching_collection_filters_null_flagged_nonmatching_and_duplicate_members_in_both_sources() {
        let _fakes = [
            fake(0x0052_5E80, collect_count as *const () as usize),
            fake(0x0045_D650, collect_insert as *const () as usize),
            fake(0x004B_0390, collect_begin as *const () as usize),
        ];
        let mut object_table = [0usize; 0x80 / 4];
        object_table[1] = collect_retain as *const () as usize;
        object_table[0x14 / 4] = collect_properties as *const () as usize;
        object_table[0x7C / 4] = collect_flags as *const () as usize;
        let mut property_table = [0usize; 0x5C / 4];
        property_table[0x58 / 4] = collect_property_matches as *const () as usize;
        let mut yes = [property_table.as_ptr() as u32, 0xFFFF_0001];
        let mut no = [property_table.as_ptr() as u32, 0xFFFF_0000];
        let mut objects = [
            [object_table.as_ptr() as u32, 1, 0, 0],
            [object_table.as_ptr() as u32, 0xFFFF_0000, no.as_mut_ptr() as u32, 0],
            [object_table.as_ptr() as u32, 0, yes.as_mut_ptr() as u32, 3],
            [object_table.as_ptr() as u32, 0, yes.as_mut_ptr() as u32, 0],
        ];
        let object_ptrs: Vec<u32> = objects.iter_mut().map(|o| o.as_mut_ptr() as u32).collect();
        let mut nodes = [[0u32; 3]; 5];
        let node_ptrs: Vec<u32> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        let mut header = [node_ptrs[0], 0, 0];
        for index in 0..5 {
            nodes[index][0] = if index == 4 {
                header.as_mut_ptr() as u32
            } else {
                node_ptrs[index + 1]
            };
            nodes[index][2] = if index == 0 { 0 } else { object_ptrs[index - 1] };
        }
        let mut set_nodes = [[0u32; 2]; 5];
        let set_ptrs: Vec<u32> = set_nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        for index in 0..5 {
            set_nodes[index][0] = if index == 4 { 0 } else { set_ptrs[index + 1] };
            set_nodes[index][1] = if index == 0 { 0 } else { object_ptrs[index - 1] };
        }
        let mut set_buckets = [0u32; 3];
        let mut table = [
            0u32,
            set_buckets.as_mut_ptr() as u32,
            set_buckets.as_mut_ptr() as u32 + 12,
            set_buckets.as_mut_ptr() as u32 + 12,
        ];
        let mut cache_nodes = [
            [0u32, 1, set_ptrs[0], table.as_mut_ptr() as u32],
            [0u32, 4, 0, table.as_mut_ptr() as u32],
        ];
        cache_nodes[0][0] = cache_nodes[1].as_mut_ptr() as u32;
        let mut buckets = [0u32, cache_nodes[0].as_mut_ptr() as u32, 0];
        let mut manager = Object::new(0xA80);
        let m = manager.ptr();
        unsafe {
            store(m, 0x44, header.as_mut_ptr() as u32);
            store(m, 0xA54, buckets.as_mut_ptr() as u32);
            store(m, 0xA58, buckets.as_mut_ptr() as u32 + 12);
        }
        let manager_before = bytes(m, 0xA80);
        let before_objects = objects;
        for (key, cached, empty) in [(7u32, false, false), (1, true, false), (4, true, true)] {
            let mut destination = [0xDEAD_BEEFu32; 5];
            destination[4] = 0;
            let d = destination.as_mut_ptr().cast::<u8>();
            let result = run(
                fc4p_00510040 as *const (),
                m,
                &[key, d as u32],
                0xABCD_EF01,
                0x1234_5678,
            );
            assert_eq!(result, registers(u32::from(!empty), 0));
            let mut expected = vec![];
            if cached {
                let node = if empty {
                    cache_nodes[1].as_ptr()
                } else {
                    cache_nodes[0].as_ptr()
                };
                expected.push(Call(0x004B_0390, node as usize + 8, vec![]));
            }
            if !empty {
                for (index, object) in object_ptrs.iter().copied().enumerate() {
                    expected.push(Call(0x7C, object as usize, vec![1]));
                    if index == 0 {
                        continue;
                    }
                    expected.push(Call(0x14, object as usize, vec![]));
                    let properties = if index == 1 {
                        no.as_ptr() as usize
                    } else {
                        yes.as_ptr() as usize
                    };
                    expected.push(Call(0x58, properties, vec![key]));
                    if index == 1 {
                        continue;
                    }
                    expected.push(Call(0x0052_5E80, d as usize, vec![object]));
                    if index == 2 {
                        continue;
                    }
                    expected.push(Call(4, object as usize, vec![]));
                    expected.push(Call(0x0045_D650, d as usize, vec![object]));
                }
            }
            assert_eq!(calls(), expected);
            assert_eq!(
                destination,
                [0xDEAD_BEEF, 0xDEAD_BEEF, 0xDEAD_BEEF, 0xDEAD_BEEF, u32::from(!empty)]
            );
            assert_eq!(bytes(m, 0xA80), manager_before);
            assert_eq!(objects, before_objects);
        }
        // A missing key with an empty list preserves pre-existing destination membership.
        unsafe { store(header.as_mut_ptr().cast(), 0, header.as_ptr() as u32) };
        let mut destination = [0u32; 5];
        destination[4] = u32::MAX;
        assert_eq!(
            run(
                fc4p_00510040 as *const (),
                m,
                &[7, destination.as_mut_ptr() as u32],
                0,
                9
            ),
            1
        );
        assert!(calls().is_empty());
    }

    extern "thiscall" fn clone_nested_triples(this: *mut u8, source: *mut u8) -> u64 {
        log(0x0050_BCB0, this, &[source as u32]);
        unsafe {
            for offset in [0, 4, 8] {
                store(this, offset, word(source, offset) ^ 0xFFFF_0000);
            }
        }
        RESULT
    }

    #[unsafe(naked)]
    unsafe extern "C" fn clone_ecx_result() {
        core::arch::naked_asm!("push [esp + 4]", "call {port}", "mov eax, ecx", "ret 4", port = sym fc4p_00540160);
    }

    #[test]
    fn triple_hash_copy_preserves_collisions_nested_calls_padding_and_all_three_result_registers() {
        let _fakes = [
            fake(0x0045_D4F0, bucket_reserve as *const () as usize),
            fake(0x004F_BA20, bucket_insert as *const () as usize),
            fake(0x0090_CF54, queued_allocate as *const () as usize),
            fake(0x0050_BCB0, clone_nested_triples as *const () as usize),
        ];
        for count in [0u32, 5] {
            for ecx_result in [false, true] {
                let mut nodes = [[0u32; 7]; 3];
                for (index, node) in nodes.iter_mut().enumerate() {
                    for (field, value) in node.iter_mut().enumerate().skip(1) {
                        *value = 0x1234_5600 + (index * 16 + field) as u32;
                    }
                }
                nodes[0][0] = nodes[1].as_ptr() as u32;
                let mut buckets = [nodes[0].as_ptr() as u32, 0, 0, nodes[2].as_ptr() as u32, 0];
                let mut storage = [0xAABB_CCDDu32; 6];
                let mut allocations = [[0xCCCC_CCCCu32; 7]; 3];
                let mut source = [
                    0x1357u32,
                    buckets.as_mut_ptr() as u32,
                    buckets.as_mut_ptr() as u32 + count * 4,
                    0x2468,
                    0x89AB_CDEF,
                ];
                let mut destination = [
                    0xFFFFu32,
                    storage.as_mut_ptr() as u32,
                    0x5555_5555,
                    storage.as_mut_ptr() as u32 + 24,
                    9,
                ];
                if count != 0 {
                    ALLOCATIONS.with(|q| {
                        q.borrow_mut()
                            .extend(allocations.iter_mut().map(|n| (28, n.as_mut_ptr() as u32)))
                    });
                }
                let s = source.as_mut_ptr().cast::<u8>();
                let d = destination.as_mut_ptr().cast::<u8>();
                let before_source = source;
                let before_nodes = nodes;
                let entry = if ecx_result {
                    clone_ecx_result as *const ()
                } else {
                    fc4p_00540160 as *const ()
                };
                let result = run(entry, d, &[s as u32], 0x1234_5678, 0xABCD_EF00);
                let edx = if count == 0 { (RESULT >> 32) as u32 } else { d as u32 };
                let eax = if ecx_result {
                    if count == 0 { 0 } else { source[4] }
                } else if count == 0 {
                    source[4]
                } else {
                    count
                };
                assert_eq!(result, registers(eax, edx));
                let mut expected = vec![
                    Call(0x0045_D4F0, d as usize + 4, vec![count]),
                    Call(0x004F_BA20, d as usize + 4, vec![storage.as_ptr() as u32, count, 0]),
                ];
                if count != 0 {
                    for index in 0..3 {
                        expected.push(Call(0x0090_CF54, 0, vec![28]));
                        expected.push(Call(
                            0x0050_BCB0,
                            allocations[index].as_ptr() as usize + 8,
                            vec![nodes[index].as_ptr() as u32 + 8],
                        ));
                        let next = if index == 0 { allocations[1].as_ptr() as u32 } else { 0 };
                        assert_eq!(
                            allocations[index],
                            [
                                next,
                                nodes[index][1],
                                nodes[index][2] ^ 0xFFFF_0000,
                                nodes[index][3] ^ 0xFFFF_0000,
                                nodes[index][4] ^ 0xFFFF_0000,
                                nodes[index][5],
                                0xCCCC_CC00 | (nodes[index][6] & 0xFF)
                            ]
                        );
                    }
                    assert_eq!(
                        storage,
                        [
                            allocations[0].as_ptr() as u32,
                            0,
                            0,
                            allocations[2].as_ptr() as u32,
                            0,
                            0xAABB_CCDD
                        ]
                    );
                } else {
                    assert_eq!(storage, [0xAABB_CCDD; 6]);
                }
                assert_eq!(calls(), expected);
                assert_eq!(source, before_source);
                assert_eq!(nodes, before_nodes);
                assert_eq!(
                    destination,
                    [
                        0xFFFF,
                        storage.as_ptr() as u32,
                        storage.as_ptr() as u32 + count * 4,
                        storage.as_ptr() as u32 + 24,
                        source[4]
                    ]
                );
                assert!(ALLOCATIONS.with(|q| q.borrow().is_empty()));
            }
        }
    }
    #[unsafe(naked)]
    unsafe extern "C" fn update_preserving_registers() {
        core::arch::naked_asm!("ret");
    }
    #[unsafe(naked)]
    unsafe extern "C" fn absent_interface_preserving_upper_eax() {
        core::arch::naked_asm!("mov al, 0", "ret");
    }

    #[test]
    fn interface_refresh_preserves_partial_eax_results_and_the_last_edx() {
        let _update = fake(0x0050_F2C0, update_preserving_registers as *const () as usize);
        let mut manager = Object::new(0x50);
        let m = manager.ptr();
        assert_eq!(
            run(fc4p_0054f7b0 as *const (), m, &[], 0xFEDC_BA98, 0x1234_5678),
            registers(0xFEDC_BA00, 0x1234_5678)
        );
        let mut table = [0usize; 0x25C / 4];
        table[0x258 / 4] = absent_interface_preserving_upper_eax as *const () as usize;
        let mut automaton = Object::new(0x210);
        let a = automaton.ptr();
        let mut header = [0u32; 3];
        let mut node = [header.as_mut_ptr() as u32, 0, a as u32];
        header[0] = node.as_mut_ptr() as u32;
        unsafe {
            m.add(0x10).write(1);
            store(m, 0x44, header.as_mut_ptr() as u32);
            store(a, 0, table.as_ptr() as u32);
        }
        assert_eq!(
            run(fc4p_0054f7b0 as *const (), m, &[], 0xFEDC_BA98, 0x1234_5678),
            registers(header.as_ptr() as u32 & !0xFF, table.as_ptr() as u32)
        );
    }

    #[test]
    fn interface_refresh_rereads_list_links_and_the_sentinel_after_callbacks() {
        let _update = fake(0x0050_F2C0, manager_update as *const () as usize);
        let mut table = [0usize; 0x25C / 4];
        table[0x258 / 4] = has_primary_interface as *const () as usize;
        for change in [0, 1, 2] {
            let mut manager = Object::new(0x50);
            let m = manager.ptr();
            let mut first = Object::new(0x210);
            let mut second = Object::new(0x210);
            let a = first.ptr();
            let b = second.ptr();
            let mut header = [0u32; 3];
            let mut last_node = [header.as_mut_ptr() as u32, 0, b as u32];
            let mut first_node = [last_node.as_mut_ptr() as u32, 0, a as u32];
            header[0] = first_node.as_mut_ptr() as u32;
            unsafe {
                m.add(0x10).write(1);
                store(m, 0x44, header.as_mut_ptr() as u32);
                store(a, 0, table.as_ptr() as u32);
                store(b, 0, table.as_ptr() as u32);
                store(a, 0x200, 0x1234_5600);
                store(b, 0x200, 0xABCD_EF00);
            }
            if change == 1 {
                mutation(0x258, first_node.as_mut_ptr().cast(), 0, header.as_mut_ptr() as u32);
            }
            if change == 2 {
                mutation(0x258, m, 0x44, last_node.as_mut_ptr() as u32);
            }
            let expected_result = registers(if change == 0 { 0xABCD_EF00 } else { 0x1234_5600 }, 0x8765_4333);
            assert_eq!(run(fc4p_0054f7b0 as *const (), m, &[], 0, 0), expected_result);
            let mut expected = vec![Call(0x0050_F2C0, m as usize, vec![]), Call(0x258, a as usize, vec![])];
            if change == 0 {
                expected.push(Call(0x258, b as usize, vec![]));
            }
            assert_eq!(calls(), expected);
        }
    }

    type ReferenceCheck = (u32, *mut u8, usize, u32);
    thread_local! {
        static REFERENCE_CHECKS: RefCell<std::collections::VecDeque<ReferenceCheck>> = const { RefCell::new(std::collections::VecDeque::new()) };
    }

    extern "thiscall" fn manager_update(this: *mut u8) -> u64 {
        log(0x0050_F2C0, this, &[]);
        RESULT
    }
    extern "thiscall" fn get_source_interface(this: *mut u8) -> u64 {
        log(0xC4, this, &[]);
        registers(unsafe { word(this, 4) }, 0x1234_ABCD)
    }
    fn check_reference_field(id: u32) {
        REFERENCE_CHECKS.with(|q| {
            if let Some((expected_id, owner, offset, expected_value)) = q.borrow_mut().pop_front() {
                assert_eq!(id, expected_id);
                assert_eq!(unsafe { word(owner, offset) }, expected_value);
            }
        });
    }
    extern "thiscall" fn refresh_retain(this: *mut u8) -> u64 {
        check_reference_field(4);
        log(4, this, &[]);
        registers(0xABCD_1234, 0x5432_1000)
    }
    extern "thiscall" fn refresh_release(this: *mut u8) -> u64 {
        check_reference_field(8);
        log(8, this, &[]);
        registers(0xBCDE_2345, 0x6543_2111)
    }
    extern "thiscall" fn get_cached_item(this: *mut u8, key: u32, unused: u32) -> u64 {
        log(12, this, &[key, unused]);
        registers(0xCDEF_3456, 0x7654_3222)
    }
    extern "thiscall" fn has_primary_interface(this: *mut u8) -> u64 {
        log(0x258, this, &[]);
        registers(unsafe { word(this, 0x200) }, 0x8765_4333)
    }
    extern "thiscall" fn get_primary_interface(this: *mut u8) -> u64 {
        log(0x250, this, &[]);
        registers(unsafe { word(this, 0x204) }, 0x9876_5444)
    }

    #[test]
    fn interface_refresh_runs_update_before_checking_active_and_handles_an_empty_list() {
        let _update = fake(0x0050_F2C0, manager_update as *const () as usize);
        let mut header = [0u32; 3];
        header[0] = header.as_mut_ptr() as u32;
        let mut manager = Object::new(0x50);
        let p = manager.ptr();
        unsafe {
            store(p, 0x44, header.as_mut_ptr() as u32);
        }
        for active in [0u8, 1, 0xFF] {
            unsafe {
                p.add(0x10).write(active);
            }
            let before = bytes(p, 0x50);
            let expected = if active == 0 {
                RESULT & !0xFF
            } else {
                registers(header.as_ptr() as u32, (RESULT >> 32) as u32)
            };
            assert_eq!(run(fc4p_0054f7b0 as *const (), p, &[], 0x1234, 0x5678), expected);
            assert_eq!(calls(), [Call(0x0050_F2C0, p as usize, vec![])]);
            assert_eq!(bytes(p, 0x50), before);
        }
        unsafe {
            p.add(0x10).write(0);
        }
        mutation(0x0050_F2C0, p, 0x10, 1);
        assert_eq!(
            run(fc4p_0054f7b0 as *const (), p, &[], 0, 0),
            registers(header.as_ptr() as u32, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), [Call(0x0050_F2C0, p as usize, vec![])]);
    }

    #[test]
    fn interface_refresh_handles_null_equal_and_replaced_interfaces_with_retain_store_release_order() {
        let _update = fake(0x0050_F2C0, manager_update as *const () as usize);
        let mut source_table = [0usize; 0xC8 / 4];
        source_table[0xC4 / 4] = get_source_interface as *const () as usize;
        let reference_table = [
            0usize,
            refresh_retain as *const () as usize,
            refresh_release as *const () as usize,
            get_cached_item as *const () as usize,
        ];
        let mut automaton_table = [0usize; 0x25C / 4];
        automaton_table[0x258 / 4] = has_primary_interface as *const () as usize;
        automaton_table[0x250 / 4] = get_primary_interface as *const () as usize;
        let mut old_source = [reference_table.as_ptr() as u32];
        let mut new_source = [reference_table.as_ptr() as u32];
        let mut old_primary = [reference_table.as_ptr() as u32];
        let mut new_primary = [reference_table.as_ptr() as u32];
        let old_s = old_source.as_mut_ptr() as u32;
        let new_s = new_source.as_mut_ptr() as u32;
        let old_p = old_primary.as_mut_ptr() as u32;
        let new_p = new_primary.as_mut_ptr() as u32;
        for source_present in [false, true] {
            for previous_s in [0, old_s] {
                for replacement_s in [0, old_s, new_s] {
                    for previous_p in [0, old_p] {
                        for replacement_p in [0, old_p, new_p] {
                            for flag in [0xAAAA_BB00u32, 0xAAAA_BB01] {
                                let mut source = [source_table.as_ptr() as u32, replacement_s];
                                let mut automaton = Object::new(0x210);
                                let a = automaton.ptr();
                                unsafe {
                                    store(a, 0, automaton_table.as_ptr() as u32);
                                    store(a, 0x1EC, if source_present { source.as_mut_ptr() as u32 } else { 0 });
                                    store(a, 0x1F0, previous_s);
                                    store(a, 0x1F4, 0xFEDC_BA98);
                                    store(a, 0x1F8, 0x1357_9BDF);
                                    store(a, 0xB0, previous_p);
                                    store(a, 0x200, flag);
                                    store(a, 0x204, replacement_p);
                                }
                                let mut manager = Object::new(0x50);
                                let m = manager.ptr();
                                let mut header = [0u32; 3];
                                let mut node = [header.as_mut_ptr() as u32, 0, a as u32];
                                header[0] = node.as_mut_ptr() as u32;
                                unsafe {
                                    m.add(0x10).write(1);
                                    store(m, 0x44, header.as_mut_ptr() as u32);
                                }
                                let manager_before = bytes(m, 0x50);
                                let mut expected_bytes = bytes(a, 0x210);
                                let mut expected_calls = vec![Call(0x0050_F2C0, m as usize, vec![])];
                                let mut checks = std::collections::VecDeque::new();
                                let mut result = registers(header.as_ptr() as u32, (RESULT >> 32) as u32);
                                if source_present {
                                    expected_calls.push(Call(0xC4, source.as_ptr() as usize, vec![]));
                                    result = registers(replacement_s, 0x1234_ABCD);
                                    if replacement_s != previous_s {
                                        if replacement_s != 0 {
                                            expected_calls.push(Call(4, replacement_s as usize, vec![]));
                                            checks.push_back((4, a, 0x1F0, previous_s));
                                            result = registers(0xABCD_1234, 0x5432_1000);
                                        }
                                        expected_bytes[0x1F0..0x1F4].copy_from_slice(&replacement_s.to_le_bytes());
                                        if previous_s != 0 {
                                            expected_calls.push(Call(8, previous_s as usize, vec![]));
                                            checks.push_back((8, a, 0x1F0, replacement_s));
                                            result = registers(0xBCDE_2345, 0x6543_2111);
                                        }
                                    }
                                    if replacement_s != 0 {
                                        expected_calls.push(Call(12, replacement_s as usize, vec![0xFEDC_BA98, 0]));
                                        expected_bytes[0x1F8..0x1FC].copy_from_slice(&0xCDEF_3456u32.to_le_bytes());
                                    }
                                }
                                // The flag getter replaces both caller-saved result registers.
                                let _ = result;
                                result = registers(flag, 0x8765_4333);
                                expected_calls.push(Call(0x258, a as usize, vec![]));
                                if flag as u8 != 0 {
                                    expected_calls.push(Call(0x250, a as usize, vec![]));
                                    result = registers(replacement_p, 0x9876_5444);
                                    if replacement_p != previous_p {
                                        if replacement_p != 0 {
                                            expected_calls.push(Call(4, replacement_p as usize, vec![]));
                                            checks.push_back((4, a, 0xB0, previous_p));
                                            result = registers(0xABCD_1234, 0x5432_1000);
                                        }
                                        expected_bytes[0xB0..0xB4].copy_from_slice(&replacement_p.to_le_bytes());
                                        if previous_p != 0 {
                                            expected_calls.push(Call(8, previous_p as usize, vec![]));
                                            checks.push_back((8, a, 0xB0, replacement_p));
                                            result = registers(0xBCDE_2345, 0x6543_2111);
                                        }
                                    }
                                }
                                REFERENCE_CHECKS.with(|q| *q.borrow_mut() = checks);
                                assert_eq!(run(fc4p_0054f7b0 as *const (), m, &[], 0, 0), result);
                                assert_eq!(calls(), expected_calls);
                                assert!(REFERENCE_CHECKS.with(|q| q.borrow().is_empty()));
                                assert_eq!(bytes(a, 0x210), expected_bytes);
                                assert_eq!(bytes(m, 0x50), manager_before);
                            }
                        }
                    }
                }
            }
        }
    }

    thread_local! {
        static ALLOCATIONS: RefCell<std::collections::VecDeque<(u32, u32)>> = const { RefCell::new(std::collections::VecDeque::new()) };
        static COLLECTED_NODES: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
        static COLLECT_SUCCESS: RefCell<bool> = const { RefCell::new(true) };
        static NEW_BUCKETS: RefCell<[u32; 3]> = const { RefCell::new([0; 3]) };
    }

    extern "C" fn queued_allocate(size: u32) -> u64 {
        log(0x0090_CF54, core::ptr::null_mut(), &[size]);
        let (expected_size, output) = ALLOCATIONS.with(|queue| queue.borrow_mut().pop_front().unwrap());
        assert_eq!(size, expected_size);
        registers(output, 0x1357_2468)
    }

    extern "thiscall" fn get_key_service(this: *mut u8) -> *mut u8 {
        log(12, this, &[]);
        // SAFETY: the fixture supplies its service pointer after the vtable.
        unsafe { word(this, 4) as *mut u8 }
    }

    extern "thiscall" fn collect_keys(this: *mut u8, tree: *mut u8) -> u64 {
        // SAFETY: the original temporary starts with a header pointer and a zero count.
        unsafe {
            let header = word(tree, 0);
            assert_eq!(word(tree, 4), 0);
            assert_eq!((header as *mut u8).read(), 0);
            assert_eq!(word(header as *mut u8, 4), 0);
            assert_eq!(word(header as *mut u8, 8), header);
            assert_eq!(word(header as *mut u8, 12), header);
            log(0x60, this, &[header]);
            COLLECTED_NODES.with(|nodes| {
                let nodes = nodes.borrow();
                if !nodes.is_empty() {
                    store(header as *mut u8, 4, nodes[nodes.len() / 2]);
                    store(header as *mut u8, 8, nodes[0]);
                    store(header as *mut u8, 12, nodes[nodes.len() - 1]);
                    store(tree, 4, nodes.len() as u32);
                }
            });
            registers(
                0xAABB_CC00 | u32::from(COLLECT_SUCCESS.with(|v| *v.borrow())),
                0x1234_5678,
            )
        }
    }

    extern "thiscall" fn erase_key_tree(this: *mut u8, root: u32) -> u64 {
        log(0x0046_F820, core::ptr::null_mut(), &[root]);
        // SAFETY: the tree still owns its populated root while erasure is in progress.
        assert_eq!(unsafe { word(word(this, 0) as *mut u8, 4) }, root);
        RESULT
    }

    #[test]
    fn source_key_counting_visits_tree_successors_matches_collision_chains_and_wraps_counts() {
        let _fakes = [
            fake(0x0090_CF54, queued_allocate as *const () as usize),
            fake(0x0090_CF63, free_array as *const () as usize),
            fake(0x0046_F820, erase_key_tree as *const () as usize),
        ];
        let object_table = [0usize, 0, 0, get_key_service as *const () as usize];
        let mut key_table = [0usize; 25];
        key_table[24] = collect_keys as *const () as usize;
        let mut service = [key_table.as_ptr() as u32];
        let mut object = [object_table.as_ptr() as u32, service.as_mut_ptr() as u32];
        for (nonempty, success) in [(false, true), (false, false), (true, false), (true, true)] {
            let mut header = [0xA5u8; 20];
            let h = header.as_mut_ptr() as u32;
            let mut nodes = [[0u32; 5]; 7];
            let pointers: Vec<u32> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for (index, key) in [1u32, 3, 6, 8, 9, 12, u32::MAX].into_iter().enumerate() {
                nodes[index][4] = key;
            }
            for (index, parent, left, right) in [
                (3, h, Some(1), Some(5)),
                (1, pointers[3], Some(0), Some(2)),
                (5, pointers[3], Some(4), Some(6)),
                (0, pointers[1], None, None),
                (2, pointers[1], None, None),
                (4, pointers[5], None, None),
                (6, pointers[5], None, None),
            ] {
                nodes[index][1] = parent;
                nodes[index][2] = left.map_or(0, |i| pointers[i]);
                nodes[index][3] = right.map_or(0, |i| pointers[i]);
            }
            let mut counts = [[0u32, 3, u32::MAX], [0, 8, 0], [0, 12, 19], [0, u32::MAX, 5]];
            let mut buckets = [0u32; 3];
            for node in &mut counts {
                let bucket = node[1] % 3;
                node[0] = buckets[bucket as usize];
                buckets[bucket as usize] = node.as_mut_ptr() as u32;
            }
            let before_nodes = nodes;
            let mut expected_counts = counts;
            if nonempty && success {
                for n in &mut expected_counts {
                    n[2] = n[2].wrapping_add(1);
                }
            }
            let mut owner = Object::new(0xA80);
            let p = owner.ptr();
            unsafe {
                store(p, 0xA68, buckets.as_mut_ptr() as u32);
                store(p, 0xA6C, buckets.as_mut_ptr() as u32 + 12);
            }
            let owner_before = bytes(p, 0xA80);
            COLLECTED_NODES.with(|v| *v.borrow_mut() = if nonempty { pointers.clone() } else { vec![] });
            COLLECT_SUCCESS.with(|v| *v.borrow_mut() = success);
            ALLOCATIONS.with(|q| q.borrow_mut().push_back((20, h)));
            assert_eq!(
                run(fc4p_0050c6a0 as *const (), p, &[object.as_mut_ptr() as u32], 0, 0),
                (RESULT & !0xFF) | 1
            );
            let mut expected = vec![
                Call(0x0090_CF54, 0, vec![20]),
                Call(12, object.as_ptr() as usize, vec![]),
                Call(0x60, service.as_ptr() as usize, vec![h]),
            ];
            if nonempty {
                expected.push(Call(0x0046_F820, 0, vec![pointers[3]]));
            }
            expected.push(Call(0x0090_CF63, h as usize, vec![]));
            assert_eq!(calls(), expected);
            assert_eq!(counts, expected_counts);
            assert_eq!(nodes, before_nodes);
            assert_eq!(bytes(p, 0xA80), owner_before);
            let mut expected_header = [0xA5u8; 20];
            expected_header[0] = 0;
            expected_header[4..8].copy_from_slice(&0u32.to_le_bytes());
            expected_header[8..12].copy_from_slice(&h.to_le_bytes());
            expected_header[12..16].copy_from_slice(&h.to_le_bytes());
            assert_eq!(header, expected_header);
        }
    }

    extern "thiscall" fn bucket_reserve(this: *mut u8, count: u32) -> u64 {
        // SAFETY: the destination end has already been reset to its begin pointer.
        assert_eq!(unsafe { word(this, 0) }, unsafe { word(this, 4) });
        log(0x0045_D4F0, this, &[count]);
        RESULT
    }
    extern "thiscall" fn bucket_insert(this: *mut u8, position: u32, count: u32, value: *const u32) -> u64 {
        // SAFETY: the port supplies its zero initializer and the fixture supplies enough buckets.
        unsafe {
            assert_eq!(value.read(), 0);
            log(0x004F_BA20, this, &[position, count, 0]);
            for i in 0..count {
                store(position.wrapping_add(i * 4) as *mut u8, 0, 0);
            }
            store(this, 4, position.wrapping_add(count * 4));
        }
        RESULT
    }

    #[test]
    fn word_hash_copy_keeps_chain_order_skips_empty_buckets_and_only_sets_live_storage() {
        let _fakes = [
            fake(0x0045_D4F0, bucket_reserve as *const () as usize),
            fake(0x004F_BA20, bucket_insert as *const () as usize),
            fake(0x0090_CF54, queued_allocate as *const () as usize),
        ];
        for count in [0usize, 5] {
            let mut source_nodes = [[0u32, 0xFFFF_1234], [0, 7], [0, 0x8000_0000]];
            source_nodes[0][0] = source_nodes[1].as_mut_ptr() as u32;
            let mut source_buckets = [
                source_nodes[0].as_mut_ptr() as u32,
                0,
                0,
                source_nodes[2].as_mut_ptr() as u32,
                0,
            ];
            let mut destination_buckets = [0xAAAA_BBBBu32; 6];
            let mut allocated = [[0xCCCC_DDDDu32; 2]; 3];
            let mut source = [
                0x1234u32,
                source_buckets.as_mut_ptr() as u32,
                source_buckets.as_mut_ptr() as u32 + count as u32 * 4,
                0x5678,
                if count == 0 { 0 } else { 3 },
            ];
            let mut destination = [
                0x9ABCu32,
                destination_buckets.as_mut_ptr() as u32,
                0xEEEE_FFFF,
                destination_buckets.as_mut_ptr() as u32 + 24,
                99,
            ];
            let p = destination.as_mut_ptr().cast();
            let before_source = source;
            let before_nodes = source_nodes;
            let before_buckets = source_buckets;
            if count != 0 {
                ALLOCATIONS.with(|q| {
                    q.borrow_mut()
                        .extend(allocated.iter_mut().map(|n| (8, n.as_mut_ptr() as u32)))
                });
            }
            run(fc4p_0050da70 as *const (), p, &[source.as_mut_ptr() as u32], 0, 0);
            let mut expected = vec![
                Call(0x0045_D4F0, p as usize + 4, vec![count as u32]),
                Call(
                    0x004F_BA20,
                    p as usize + 4,
                    vec![destination_buckets.as_ptr() as u32, count as u32, 0],
                ),
            ];
            if count != 0 {
                expected.extend((0..3).map(|_| Call(0x0090_CF54, 0, vec![8])));
            }
            assert_eq!(calls(), expected);
            assert_eq!(source, before_source);
            assert_eq!(source_nodes, before_nodes);
            assert_eq!(source_buckets, before_buckets);
            assert_eq!(
                destination,
                [
                    0x9ABC,
                    destination_buckets.as_ptr() as u32,
                    destination_buckets.as_ptr() as u32 + count as u32 * 4,
                    destination_buckets.as_ptr() as u32 + 24,
                    source[4]
                ]
            );
            if count == 0 {
                assert_eq!(destination_buckets, [0xAAAA_BBBB; 6]);
            } else {
                assert_eq!(
                    destination_buckets,
                    [
                        allocated[0].as_ptr() as u32,
                        0,
                        0,
                        allocated[2].as_ptr() as u32,
                        0,
                        0xAAAA_BBBB
                    ]
                );
                assert_eq!(allocated[0], [allocated[1].as_ptr() as u32, 0xFFFF_1234]);
                assert_eq!(allocated[1], [0, 7]);
                assert_eq!(allocated[2], [0, 0x8000_0000]);
            }
        }
    }

    extern "C" fn choose_bucket_count(first: u32, last: u32, key: *const u32, hint: u32, comparator: u32) -> u64 {
        // SAFETY: key is the port's requested-count local.
        log(
            0x0079_FA90,
            core::ptr::null_mut(),
            &[first, last, unsafe { key.read() }, hint, comparator],
        );
        registers(FOUND.with(|p| *p.borrow()), 0x1122_3344)
    }
    extern "thiscall" fn bucket_vector_new(this: *mut u8, count: u32, zero: *const u32, allocator: *const u32) {
        // SAFETY: the port supplies the zero value and selected count; all fixture bucket cells exist.
        unsafe {
            assert_eq!(zero.read(), 0);
            assert_eq!(allocator.read(), count);
            log(0x0051_C950, core::ptr::null_mut(), &[count]);
            NEW_BUCKETS.with(|v| {
                let v = *v.borrow();
                for (i, value) in v.into_iter().enumerate() {
                    store(this, i * 4, value);
                }
                // The maximum-count boundary deliberately supplies no storage and no old nodes.
                if v[0] != 0 {
                    for i in 0..count {
                        store(v[0].wrapping_add(i * 4) as *mut u8, 0, 0);
                    }
                }
            });
        }
    }

    #[test]
    fn byte_hash_growth_reverses_colliding_chains_uses_only_the_key_byte_and_preserves_payloads() {
        let _fakes = [
            fake(0x0079_FA90, choose_bucket_count as *const () as usize),
            fake(0x0051_C950, bucket_vector_new as *const () as usize),
            fake(0x0090_CF63, free_array as *const () as usize),
        ];
        let mut primes = [5u32];
        let mut last = 0u32;
        let _data = [
            crate::testing::fake_data(0x00A8_29B0, primes.as_mut_ptr().cast()),
            crate::testing::fake_data(0x00A8_2A20, (&mut last as *mut u32).cast()),
        ];
        let mut nodes = [[0u32, 5, 91], [0, 10, 92], [0, 255, 93], [0, 0x1234_0105, 94]];
        nodes[0][0] = nodes[1].as_mut_ptr() as u32;
        nodes[2][0] = nodes[3].as_mut_ptr() as u32;
        let mut old = [nodes[0].as_mut_ptr() as u32, nodes[2].as_mut_ptr() as u32];
        let mut new = [0xAAAA_BBBBu32; 6];
        let mut table = [
            0x1234u32,
            old.as_mut_ptr() as u32,
            old.as_mut_ptr() as u32 + 8,
            0x5678,
            4,
        ];
        let before = nodes;
        FOUND.with(|p| *p.borrow_mut() = primes.as_mut_ptr() as u32);
        NEW_BUCKETS.with(|p| {
            *p.borrow_mut() = [
                new.as_mut_ptr() as u32,
                new.as_mut_ptr() as u32 + 20,
                new.as_mut_ptr() as u32 + 24,
            ]
        });
        assert_eq!(
            run(fc4p_0051ce70 as *const (), table.as_mut_ptr().cast(), &[3], 0, 0),
            RESULT
        );
        assert_eq!(
            calls(),
            [
                Call(
                    0x0079_FA90,
                    0,
                    vec![primes.as_ptr() as u32, &last as *const u32 as u32, 3, 3, 0]
                ),
                Call(0x0051_C950, 0, vec![5]),
                Call(0x0090_CF63, old.as_ptr() as usize, vec![])
            ]
        );
        assert_eq!(old, [0, 0]);
        assert_eq!(new, [nodes[3].as_ptr() as u32, 0, 0, 0, 0, 0xAAAA_BBBB]);
        assert_eq!(nodes[3][0], nodes[2].as_ptr() as u32);
        assert_eq!(nodes[2][0], nodes[1].as_ptr() as u32);
        assert_eq!(nodes[1][0], nodes[0].as_ptr() as u32);
        assert_eq!(nodes[0][0], 0);
        for i in 0..4 {
            assert_eq!(nodes[i][1..], before[i][1..]);
        }
        assert_eq!(
            table,
            [
                0x1234,
                new.as_ptr() as u32,
                new.as_ptr() as u32 + 20,
                new.as_ptr() as u32 + 24,
                4
            ]
        );
    }

    #[test]
    fn byte_hash_growth_handles_no_growth_selected_smaller_prime_empty_storage_and_prime_range_end() {
        let _fakes = [
            fake(0x0079_FA90, choose_bucket_count as *const () as usize),
            fake(0x0051_C950, bucket_vector_new as *const () as usize),
        ];
        let mut prime = 1u32;
        let mut last = 0u32;
        let _data = [
            crate::testing::fake_data(0x00A8_29B0, (&mut prime as *mut u32).cast()),
            crate::testing::fake_data(0x00A8_2A20, (&mut last as *mut u32).cast()),
        ];
        let mut buckets = [0u32; 2];
        let mut table = [
            0x1234u32,
            buckets.as_mut_ptr() as u32,
            buckets.as_mut_ptr() as u32 + 8,
            0x5678,
            0,
        ];
        let before = table;
        for requested in [0, 2] {
            assert_eq!(
                run(
                    fc4p_0051ce70 as *const (),
                    table.as_mut_ptr().cast(),
                    &[requested],
                    0,
                    77
                ),
                registers(requested, 77)
            );
            assert!(calls().is_empty());
            assert_eq!(table, before);
        }
        FOUND.with(|p| *p.borrow_mut() = &prime as *const u32 as u32);
        run(fc4p_0051ce70 as *const (), table.as_mut_ptr().cast(), &[3], 0, 77);
        assert_eq!(calls().len(), 1);
        assert_eq!(table, before);
        table = [0x1234, 0, 0, 0, 0];
        FOUND.with(|p| *p.borrow_mut() = &last as *const u32 as u32);
        NEW_BUCKETS.with(|p| *p.borrow_mut() = [0, 0xFFFF_FFEC, 0xFFFF_FFEC]);
        run(
            fc4p_0051ce70 as *const (),
            table.as_mut_ptr().cast(),
            &[u32::MAX],
            0,
            77,
        );
        assert_eq!(
            calls(),
            [
                Call(
                    0x0079_FA90,
                    0,
                    vec![
                        &prime as *const u32 as u32,
                        &last as *const u32 as u32,
                        u32::MAX,
                        u32::MAX,
                        0
                    ]
                ),
                Call(0x0051_C950, 0, vec![0xFFFF_FFFB])
            ]
        );
        assert_eq!(table, [0x1234, 0, 0xFFFF_FFEC, 0xFFFF_FFEC, 0]);
    }

    thread_local! {
        static ACTION_OUTPUT: RefCell<Option<u32>> = const { RefCell::new(None) };
        static EIGHT_BYTE_TEMPORARY: RefCell<Option<[u32; 2]>> = const { RefCell::new(None) };
    }

    extern "C" fn action_allocate(size: u32) -> u64 {
        log(0x005E_55E0, core::ptr::null_mut(), &[size]);
        registers(FOUND.with(|p| *p.borrow()), 0x1234_ABCD)
    }

    macro_rules! action_constructor_fake {
        ($name:ident, $address:literal) => {
            extern "thiscall" fn $name(this: *mut u8) -> u64 {
                log($address, this, &[]);
                registers(
                    ACTION_OUTPUT.with(|p| p.borrow().unwrap_or(this as u32)),
                    0x8765_4321,
                )
            }
        };
    }
    action_constructor_fake!(action_eight, 0x0050_5290);
    action_constructor_fake!(action_eleven, 0x0050_5BF0);
    action_constructor_fake!(action_fourteen, 0x0050_5C70);
    action_constructor_fake!(action_fifteen, 0x0050_5C40);
    action_constructor_fake!(action_sixteen, 0x0050_6370);
    action_constructor_fake!(action_seventeen, 0x0050_5CA0);
    action_constructor_fake!(action_nineteen, 0x0050_9A20);
    action_constructor_fake!(action_twenty, 0x0050_52D0);
    action_constructor_fake!(action_twenty_one, 0x0050_6330);
    action_constructor_fake!(action_fifty_two, 0x0050_6420);

    #[test]
    fn action_factory_selects_every_constructor_tags_its_result_and_handles_allocation_failure() {
        let _allocator = fake(0x005E_55E0, action_allocate as *const () as usize);
        let cases = [
            (8, 0x34, 0x0050_5290, action_eight as *const ()),
            (11, 0x40, 0x0050_5BF0, action_eleven as *const ()),
            (14, 0x40, 0x0050_5C70, action_fourteen as *const ()),
            (15, 0x30, 0x0050_5C40, action_fifteen as *const ()),
            (16, 0x274, 0x0050_6370, action_sixteen as *const ()),
            (17, 0x58, 0x0050_5CA0, action_seventeen as *const ()),
            (19, 0x34, 0x0050_9A20, action_nineteen as *const ()),
            (20, 0x2C, 0x0050_52D0, action_twenty as *const ()),
            (21, 0x44, 0x0050_6330, action_twenty_one as *const ()),
            (52, 0x2C, 0x0050_6420, action_fifty_two as *const ()),
        ];
        for (kind, size, constructor, function) in cases {
            let _constructor = fake(constructor, function as usize);
            let mut allocation = [0xA5u8; 0x280];
            let mut output = [0xCDu8; 0x280];
            let p = allocation.as_mut_ptr();
            for destination in [p as u32, output.as_mut_ptr() as u32, 0] {
                FOUND.with(|v| *v.borrow_mut() = p as u32);
                ACTION_OUTPUT.with(|v| *v.borrow_mut() = Some(destination));
                let before = allocation;
                let output_before = output;
                assert_eq!(
                    run(fc4p_0050b7f0 as *const (), core::ptr::null_mut(), &[kind], 0, 9),
                    registers(destination, 0x8765_4321)
                );
                assert_eq!(
                    calls(),
                    [Call(0x005E_55E0, 0, vec![size]), Call(constructor, p as usize, vec![])]
                );
                let mut expected = before;
                let mut expected_output = output_before;
                if destination == p as u32 {
                    expected[4..8].copy_from_slice(&kind.to_le_bytes());
                }
                if destination == output.as_mut_ptr() as u32 {
                    expected_output[4..8].copy_from_slice(&kind.to_le_bytes());
                }
                assert_eq!(allocation, expected);
                assert_eq!(output, expected_output);
            }
            FOUND.with(|v| *v.borrow_mut() = 0);
            let before = allocation;
            assert_eq!(
                run(fc4p_0050b7f0 as *const (), core::ptr::null_mut(), &[kind], 0, 9),
                registers(0, 0x1234_ABCD)
            );
            assert_eq!(calls(), [Call(0x005E_55E0, 0, vec![size])]);
            assert_eq!(allocation, before);
        }
    }

    #[test]
    fn base_action_factory_initializes_only_its_fields_and_keeps_padding_and_allocator_edx() {
        let _allocator = fake(0x005E_55E0, action_allocate as *const () as usize);
        for kind in [0u32, 7, 9, 10, 12, 13, 18, 22, 51, 53, u32::MAX] {
            let mut allocation = [0xA5u8; 0x30];
            let p = allocation.as_mut_ptr();
            FOUND.with(|v| *v.borrow_mut() = p as u32);
            assert_eq!(
                run(fc4p_0050b7f0 as *const (), core::ptr::null_mut(), &[kind], 0, 9),
                registers(p as u32, 0x1234_ABCD)
            );
            let mut expected = [0xA5u8; 0x30];
            expected[..0x28].fill(0);
            expected[..4].copy_from_slice(&0x00A9_5384u32.to_le_bytes());
            expected[4..8].copy_from_slice(&kind.to_le_bytes());
            expected[0x15..0x18].fill(0xA5);
            assert_eq!(allocation, expected);
            assert_eq!(calls(), [Call(0x005E_55E0, 0, vec![0x28])]);
        }
    }

    extern "thiscall" fn automaton_tree_destroy(this: *mut u8) -> u64 {
        log(0x0051_3E00, this, &[]);
        RESULT
    }
    extern "thiscall" fn lot_list_clear(this: *mut u8) -> u64 {
        log(0x0047_3EC0, this, &[]);
        RESULT
    }
    extern "thiscall" fn model_processor_destroy(this: *mut u8) -> u64 {
        log(0x0080_30A0, this, &[]);
        RESULT
    }
    extern "thiscall" fn triple_vector_destroy(this: *mut u8) -> u64 {
        log(0x005E_8C40, this, &[]);
        RESULT
    }

    #[test]
    fn automaton_destruction_restores_tables_releases_adjusted_interfaces_and_preserves_fields() {
        let _fakes = [
            fake(0x0051_3E00, automaton_tree_destroy as *const () as usize),
            fake(0x0047_3EC0, lot_list_clear as *const () as usize),
            fake(0x0080_30A0, model_processor_destroy as *const () as usize),
            fake(0x005E_8C40, triple_vector_destroy as *const () as usize),
            fake(0x0090_CF63, free_array as *const () as usize),
        ];
        let tables = [
            (0, 0x00A9_5AC0u32),
            (4, 0x00A9_5A38),
            (8, 0x00A9_5A1C),
            (12, 0x00A9_59B8),
            (0x20, 0x00A9_59A0),
            (0x38, 0x00A9_5990),
        ];
        let release_table = [0usize, 0, release as *const () as usize];
        let adjustment_table = [0u32, 8];
        let mut adjusted = [0u32, adjustment_table.as_ptr() as u32, 0, release_table.as_ptr() as u32];
        let mut retained = [release_table.as_ptr() as u32];
        for populated in [false, true] {
            let mut owner = Object::new(0x1D0);
            let p = owner.ptr();
            let mut expected_calls = vec![];
            if populated {
                for offset in [0x1C8, 0x188, 0x110, 0xC0, 0xB4] {
                    unsafe { store(p, offset, 0x8000_0000 + offset as u32) };
                }
                unsafe {
                    store(p, 0xCC, adjusted.as_mut_ptr() as u32);
                    store(p, 0xB0, retained.as_mut_ptr() as u32);
                    store(p, 0x98, retained.as_mut_ptr() as u32);
                }
                expected_calls.push(Call(0x0090_CF63, 0x8000_01C8, vec![]));
            }
            let mut expected_bytes = bytes(p, 0x1D0);
            for (offset, value) in tables {
                expected_bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            expected_calls.extend([
                Call(0x0051_3E00, p as usize + 0x18C, vec![]),
                Call(0x0047_3EC0, p as usize + 0x188, vec![]),
            ]);
            if populated {
                expected_calls.push(Call(0x0090_CF63, 0x8000_0188, vec![]));
            }
            expected_calls.push(Call(0x0080_30A0, p as usize + 0x12C, vec![]));
            if populated {
                expected_calls.push(Call(0x0090_CF63, 0x8000_0110, vec![]));
            }
            expected_calls.push(Call(0x0080_30A0, p as usize + 0xD0, vec![]));
            if populated {
                expected_calls.extend([
                    Call(8, adjusted.as_ptr() as usize + 12, vec![]),
                    Call(0x0090_CF63, 0x8000_00C0, vec![]),
                    Call(0x0090_CF63, 0x8000_00B4, vec![]),
                    Call(8, retained.as_ptr() as usize, vec![]),
                    Call(8, retained.as_ptr() as usize, vec![]),
                ]);
            }
            expected_calls.push(Call(0x005E_8C40, p as usize + 4, vec![]));
            assert_eq!(run(fc4p_00513e60 as *const (), p, &[], 0, 77), RESULT);
            assert_eq!(calls(), expected_calls);
            assert_eq!(bytes(p, 0x1D0), expected_bytes);
        }
    }

    #[test]
    fn automaton_destruction_rereads_the_list_sentinel_after_clearing_it() {
        let _fakes = [
            fake(0x0051_3E00, automaton_tree_destroy as *const () as usize),
            fake(0x0047_3EC0, lot_list_clear as *const () as usize),
            fake(0x0080_30A0, model_processor_destroy as *const () as usize),
            fake(0x005E_8C40, triple_vector_destroy as *const () as usize),
            fake(0x0090_CF63, free_array as *const () as usize),
        ];
        let mut owner = Object::new(0x1D0);
        let p = owner.ptr();
        unsafe { store(p, 0x188, 0x1234) };
        mutation(0x0047_3EC0, p, 0x188, 0x5678);
        run(fc4p_00513e60 as *const (), p, &[], 0, 0);
        let observed = calls();
        assert_eq!(observed[2], Call(0x0090_CF63, 0x5678, vec![]));
        assert_eq!(unsafe { word(p, 0x188) }, 0x5678);
    }

    extern "thiscall" fn erase_eights(this: *mut u8, first: u32, last: u32) -> u64 {
        log(0x0071_0B40, this, &[first, last]);
        RESULT
    }
    extern "thiscall" fn insert_eights(this: *mut u8, position: u32, count: u32, value: *mut u8) -> u64 {
        assert!(!value.is_null());
        EIGHT_BYTE_TEMPORARY.with(|expected| {
            if let Some(expected) = *expected.borrow() {
                // SAFETY: this case supplies initialized words at the caller's temporary address.
                assert_eq!(unsafe { [word(value, 0), word(value, 4)] }, expected);
            }
        });
        log(0x0063_8E80, this, &[position, count]);
        RESULT
    }

    #[unsafe(naked)]
    unsafe extern "C" fn resize_with_stack_bytes() {
        core::arch::naked_asm!("mov dword ptr [esp - 8], 0x12345678",
            "mov dword ptr [esp - 4], 0x87654321", "jmp {entry}", entry = sym fc4p_0051fc30);
    }

    #[test]
    fn eight_byte_resize_passes_the_bytes_at_the_original_stack_temporary() {
        let _insert = fake(0x0063_8E80, insert_eights as *const () as usize);
        let mut vector = [0x1000u32, 0x1008, 0x2000];
        EIGHT_BYTE_TEMPORARY.with(|v| *v.borrow_mut() = Some([0x1234_5678, 0x8765_4321]));
        assert_eq!(
            run(
                resize_with_stack_bytes as *const (),
                vector.as_mut_ptr().cast(),
                &[3],
                0,
                0
            ),
            RESULT
        );
        EIGHT_BYTE_TEMPORARY.with(|v| *v.borrow_mut() = None);
        assert_eq!(calls(), [Call(0x0063_8E80, vector.as_ptr() as usize, vec![0x1008, 2])]);
    }

    #[test]
    fn eight_byte_resize_dispatches_unsigned_counts_including_equal_empty_and_wrapping_ranges() {
        let _fakes = [
            fake(0x0071_0B40, erase_eights as *const () as usize),
            fake(0x0063_8E80, insert_eights as *const () as usize),
        ];
        for (begin, end, requested, erase, count) in [
            (0x1000u32, 0x1018u32, 1, true, 1),
            (0x1000, 0x1018, 0, true, 0),
            (0x1000, 0x1018, 3, false, 0),
            (0x1000, 0x1018, 5, false, 2),
            (0, 0, 0, false, 0),
            (0, 0, u32::MAX, false, u32::MAX),
            (0xFFFF_FFF8, 0x10, 1, true, 1),
            (0x1000, 0x0FF8, 0, true, 0),
            (0x1000, 0x0FF8, u32::MAX, false, 0),
        ] {
            let mut vector = [begin, end, 0xDEAD_BEEF];
            let p = vector.as_mut_ptr().cast();
            let before = vector;
            assert_eq!(run(fc4p_0051fc30 as *const (), p, &[requested], 1, 2), RESULT);
            let expected = if erase {
                Call(0x0071_0B40, p as usize, vec![begin.wrapping_add(count * 8), end])
            } else {
                Call(0x0063_8E80, p as usize, vec![end, count])
            };
            assert_eq!(calls(), [expected]);
            assert_eq!(vector, before);
        }
    }

    #[test]
    fn bucket_remainders_use_unsigned_operands_with_a_zero_high_half() {
        for (value, divisor, expected) in [
            (0u32, 1u32, 0u32),
            (u32::MAX, 5, 0),
            (0x8000_0000, 3, 2),
            (u32::MAX, 0x8000_0001, 0x7FFF_FFFE),
            (0xFFFF_FFF0, 16, 0),
        ] {
            assert_eq!(unsigned_remainder(value, divisor), expected);
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn speed_with_registers() {
        core::arch::naked_asm!(
            "mov eax, 0xabcd1234",
            "mov edx, 0x76543210",
            "fld qword ptr [ecx + 8]",
            "ret"
        );
    }
    #[unsafe(naked)]
    unsafe extern "C" fn transform_preserving_registers() {
        core::arch::naked_asm!("fld1", "ret 4");
    }
    #[unsafe(naked)]
    unsafe extern "C" fn set_preserving_registers() {
        core::arch::naked_asm!("ret 4");
    }

    fn float_environment() -> (u16, u16, u32, u32) {
        let (mut cw, mut mxcsr) = (0u16, 0u32);
        let (sw, flags): (u16, u32);
        // SAFETY: writable local operands and a balanced flags push/pop.
        unsafe {
            core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
                cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr, out("ax") sw, flags = lateout(reg) flags);
        }
        (cw, sw & 0x3800, mxcsr, flags & 0x400)
    }

    #[test]
    fn speed_interval_keeps_callee_register_residue_and_the_callers_float_environment() {
        let _fpu = crate::testing::GameFpu::new();
        let (mut threshold, mut numerator, mut scale) = (0.0f32, 1.0f32, 1.0f32);
        let _threshold = crate::testing::fake_data(0x00A8_1054, (&mut threshold as *mut f32).cast());
        let _numerator = crate::testing::fake_data(0x00A8_1228, (&mut numerator as *mut f32).cast());
        let _scale = crate::testing::fake_data(0x00A9_7D74, (&mut scale as *mut f32).cast());
        let mut table = [0usize; 0x1F8 / 4];
        table[0x1F4 / 4] = speed_with_registers as *const () as usize;
        table[0x8C / 4] = transform_preserving_registers as *const () as usize;
        table[0x90 / 4] = set_preserving_registers as *const () as usize;
        // Use the speed's C-layout offset explicitly in the fixture for the naked getter above.
        let mut object = [table.as_ptr() as u32, 0, 0, 0];
        let p = object.as_mut_ptr().cast::<u8>();
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        // SAFETY: there is room on the x87 stack; pop this sentinel before asserting.
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        let before = float_environment();
        let mut outcomes = vec![];
        for (speed, condition) in [(2.0f64, 0), (0.0, 0x4000), (-1.0, 0x100), (f64::NAN, 0x4500)] {
            unsafe { p.add(8).cast::<f64>().write_unaligned(speed) };
            let result = run(fc4p_0054da50 as *const (), p, &[], 0, 0);
            outcomes.push((result, condition, float_environment()));
        }
        // SAFETY: the sentinel remains on the stack through all port calls and is popped here.
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        for (result, condition, environment) in outcomes {
            assert_eq!(result as u32 & 0xFFFF_0000, 0xABCD_0000);
            assert_eq!(result as u32 & 0x4500, condition);
            assert_eq!(result >> 32, 0x7654_3210);
            assert_eq!(environment, before);
        }
        assert_eq!(restored.to_bits(), sentinel.to_bits());
    }

    extern "thiscall" fn release_selected(this: *mut u8) -> u64 {
        let selected = FOUND.with(|owner| unsafe { word(*owner.borrow() as *mut u8, 0x98) });
        log(8, this, &[selected]);
        RESULT
    }

    #[test]
    fn selected_object_is_cleared_before_its_release_callback() {
        let mut owner_table = [0usize; 0xE4 / 4];
        owner_table[0xE0 / 4] = stack_top as *const () as usize;
        let retained_table = [0usize, 0, release_selected as *const () as usize];
        let mut retained = [retained_table.as_ptr() as usize];
        let object = retained.as_mut_ptr().cast::<u8>();
        let mut owner = Object::new(0xC0);
        let p = owner.ptr();
        unsafe {
            store(p, 0, owner_table.as_ptr() as u32);
            store(p, 0x98, object as u32);
        }
        FOUND.with(|owner| *owner.borrow_mut() = p as u32);
        assert_eq!(
            run(fc4p_0050b290 as *const (), p, &[object as u32], 0, 0),
            (RESULT & !0xFF) | 1
        );
        assert_eq!(
            calls(),
            [Call(0xE0, p as usize, vec![]), Call(8, object as usize, vec![0])]
        );
    }

    extern "C" fn memmove(destination: u32, source: u32, size: u32) -> u64 {
        log(0x009E_EB70, destination as *mut u8, &[source, size]);
        // SAFETY: the vector fixture owns the overlapping ranges passed by the port.
        unsafe { core::ptr::copy(source as *const u8, destination as *mut u8, size as usize) };
        registers(destination, 0xDEAD_BEEF)
    }
    extern "thiscall" fn stack_top(this: *mut u8) -> u64 {
        log(0xE0, this, &[]);
        // SAFETY: the fixture keeps its current top pointer at +0x20.
        registers(unsafe { word(this, 0x20) }, 0x1234_5678)
    }
    extern "thiscall" fn stack_pop(this: *mut u8) -> u64 {
        log(0xD4, this, &[]);
        // SAFETY: the fixture links its current top to the next top through its first word.
        unsafe { store(this, 0x20, word(word(this, 0x20) as *mut u8, 0)) };
        RESULT
    }

    #[test]
    fn null_object_removal_preserves_upper_eax_and_edx_without_calls() {
        for eax in [0, 0x1234_5678, u32::MAX] {
            assert_eq!(
                run(
                    fc4p_0050b290 as *const (),
                    core::ptr::null_mut(),
                    &[0],
                    eax,
                    0xAABB_CCDD
                ),
                registers((eax & !0xFF) | 1, 0xAABB_CCDD)
            );
        }
        assert!(calls().is_empty());
    }

    #[test]
    fn object_removal_pops_matching_tops_and_compacts_repeated_vector_matches() {
        let _free = fake(0x005E_5620, free as *const () as usize);
        let _move = fake(0x009E_EB70, memmove as *const () as usize);
        let mut table = [0usize; 0xE4 / 4];
        table[0xE0 / 4] = stack_top as *const () as usize;
        table[0xD4 / 4] = stack_pop as *const () as usize;
        let retained_table = [0usize, 0, release as *const () as usize];
        let mut retained = [retained_table.as_ptr() as usize];
        let object = retained.as_mut_ptr().cast::<u8>();
        let mut unmatching_top = [0u32, 0, 0x4321];
        let mut second_top = [unmatching_top.as_mut_ptr() as u32, 0, object as u32];
        let mut first_top = [second_top.as_mut_ptr() as u32, 0, object as u32];
        let mut matching = [0u32, 0, object as u32];
        let mut unmatching = [0u32, 0, 0x4321];
        let (a, b) = (matching.as_mut_ptr() as u32, unmatching.as_mut_ptr() as u32);
        let mut data = [a, a, b, a];
        let begin = data.as_mut_ptr() as u32;
        let mut owner = Object::new(0xC0);
        let p = owner.ptr();
        unsafe {
            store(p, 0, table.as_ptr() as u32);
            store(p, 0x20, first_top.as_mut_ptr() as u32);
            store(p, 0x98, object as u32);
            store(p, 0xB4, begin);
            store(p, 0xB8, begin + 16);
        }
        assert_eq!(
            run(fc4p_0050b290 as *const (), p, &[object as u32], 0, 0),
            (RESULT & !0xFF) | 1
        );
        assert_eq!(data, [b, a, a, a]);
        assert_eq!(
            unsafe { (word(p, 0x98), word(p, 0xB8), word(p, 0x20)) },
            (0, begin + 4, unmatching_top.as_mut_ptr() as u32)
        );
        assert_eq!(
            calls(),
            [
                Call(0xE0, p as usize, vec![]),
                Call(0xD4, p as usize, vec![]),
                Call(0xE0, p as usize, vec![]),
                Call(0xD4, p as usize, vec![]),
                Call(0xE0, p as usize, vec![]),
                Call(8, object as usize, vec![]),
                Call(0x009E_EB70, begin as usize, vec![begin + 4, 12]),
                Call(0x005E_5620, a as usize, vec![]),
                Call(8, object as usize, vec![]),
                Call(0x009E_EB70, begin as usize, vec![begin + 4, 8]),
                Call(0x005E_5620, a as usize, vec![]),
                Call(8, object as usize, vec![]),
                Call(0x005E_5620, a as usize, vec![]),
                Call(8, object as usize, vec![])
            ]
        );
    }

    #[test]
    fn absent_object_removal_returns_selected_pointer_upper_bytes_and_top_edx() {
        let mut table = [0usize; 0xE4 / 4];
        table[0xE0 / 4] = stack_top as *const () as usize;
        let mut owner = Object::new(0xC0);
        let p = owner.ptr();
        unsafe {
            store(p, 0, table.as_ptr() as u32);
            store(p, 0x98, 0x1122_33FE);
        }
        let before = bytes(p, 0xC0);
        assert_eq!(
            run(fc4p_0050b290 as *const (), p, &[0x4567], 0, 0),
            registers(0x1122_3301, 0x1234_5678)
        );
        assert_eq!(calls(), [Call(0xE0, p as usize, vec![])]);
        assert_eq!(bytes(p, 0xC0), before);
    }

    #[repr(C)]
    struct SpeedObject {
        table: *const usize,
        speed: f64,
    }
    extern "thiscall" fn speed(this: *mut SpeedObject) -> f64 {
        log(0x1F4, this.cast(), &[]);
        // SAFETY: the test's speed object.
        unsafe { (*this).speed }
    }
    extern "thiscall" fn transform_interval(this: *mut u8, value: f32) -> f64 {
        log(0x8C, this, &[value.to_bits()]);
        0.333_333_333_333_333_3
    }
    extern "thiscall" fn set_interval(this: *mut u8, value: f32) -> u64 {
        log(0x90, this, &[value.to_bits()]);
        RESULT
    }
    extern "thiscall" fn wrong_set_interval(this: *mut u8, value: f32) -> u64 {
        log(0x190, this, &[value.to_bits()]);
        0
    }

    #[test]
    fn speed_interval_covers_unordered_nonpositive_infinite_and_unrounded_speeds_and_saved_vtable() {
        let _fpu = crate::testing::GameFpu::new();
        let (mut threshold, mut numerator, mut scale) = (0.0f32, 1.0f32, 1.0f32);
        let _threshold = crate::testing::fake_data(0x00A8_1054, (&mut threshold as *mut f32).cast());
        let _numerator = crate::testing::fake_data(0x00A8_1228, (&mut numerator as *mut f32).cast());
        let _scale = crate::testing::fake_data(0x00A9_7D74, (&mut scale as *mut f32).cast());
        let mut table = [0usize; 0x1F8 / 4];
        table[0x1F4 / 4] = speed as *const () as usize;
        table[0x8C / 4] = transform_interval as *const () as usize;
        table[0x90 / 4] = set_interval as *const () as usize;
        let mut replacement = table;
        replacement[0x90 / 4] = wrong_set_interval as *const () as usize;
        let mut object = SpeedObject {
            table: table.as_ptr(),
            speed: 0.0,
        };
        let p = (&mut object as *mut SpeedObject).cast::<u8>();
        for (value, expected) in [
            (2.0, 0.5f32),
            (1.0, 1.0),
            (0.0, 0.1),
            (-0.0, 0.1),
            (-2.0, 0.1),
            (f64::NAN, 0.1),
            (f64::INFINITY, 0.0),
            (f64::NEG_INFINITY, 0.1),
            (f64::from(f32::from_bits(1)), f32::INFINITY),
        ] {
            object.speed = value;
            let before = bytes(p, core::mem::size_of::<SpeedObject>());
            assert_eq!(run(fc4p_0054da50 as *const (), p, &[], 0, 0), RESULT);
            assert_eq!(
                calls(),
                [
                    Call(0x1F4, p as usize, vec![]),
                    Call(0x8C, p as usize, vec![expected.to_bits()]),
                    Call(0x90, p as usize, vec![0x3EAA_AAAB])
                ]
            );
            assert_eq!(bytes(p, core::mem::size_of::<SpeedObject>()), before);
        }
        threshold = 1.0;
        object.speed = 1.000_000_000_1;
        assert_eq!(run(fc4p_0054da50 as *const (), p, &[], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x1F4, p as usize, vec![]),
                Call(0x8C, p as usize, vec![1.0f32.to_bits()]),
                Call(0x90, p as usize, vec![0x3EAA_AAAB])
            ]
        );
        assert_eq!(threshold, 1.0);
        numerator = 3.0;
        scale = 2.0;
        threshold = 0.0;
        object.speed = 4.0;
        mutation(0x8C, p, 0, replacement.as_ptr() as u32);
        assert_eq!(run(fc4p_0054da50 as *const (), p, &[], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x1F4, p as usize, vec![]),
                Call(0x8C, p as usize, vec![1.5f32.to_bits()]),
                Call(0x90, p as usize, vec![0x3EAA_AAAB])
            ]
        );
        assert_eq!(object.table, replacement.as_ptr());
        assert_eq!((numerator, scale, threshold), (3.0, 2.0, 0.0));
    }

    use super::*;
    use crate::testing::{Object, fake};
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
        for node in &mut nodes {
            let key = node[1];
            let bucket = key as usize % buckets.len();
            node[0] = buckets[bucket];
            buckets[bucket] = node.as_mut_ptr() as u32;
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
