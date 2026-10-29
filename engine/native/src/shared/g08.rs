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

//! Shared helpers, group 08: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

// All field accesses use the live objects supplied by the game.
unsafe fn read<T: Copy>(p: *mut u8, offset: usize) -> T {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}
unsafe fn write<T>(p: *mut u8, offset: usize, value: T) {
    unsafe { write_unaligned(p.wrapping_add(offset).cast(), value) }
}

/// Unsigned x86 division keeps the processor's divide-error behavior for zero divisors.
fn divide(numerator: u32, denominator: u32) -> (u32, u32) {
    let quotient: u32;
    let remainder: u32;
    unsafe {
        core::arch::asm!("div {divisor:e}", divisor = in(reg) denominator,
            inlateout("eax") numerator => quotient, inlateout("edx") 0u32 => remainder,
            options(nomem, nostack));
    }
    (quotient, remainder)
}

layout! {
    /// Lua owner reference count and Lua thread state belong to separate object layouts.
    struct LuaOwner { 0x20 => refs / set_refs: u32 }
}
layout! {
    struct LuaThread { 0x08 => state / set_state: *mut u8 }
}
layout! {
    struct ParameterSet { 0x04 => refs / set_refs: u32 }
}

game_fn! { #[allow(dead_code)] fn owner_add_ref(this: *mut u8) -> u64 = "thiscall" 0x008A_E800; }
game_fn! { fn precision() -> u32 = "C" 0x0089_2A0A; }
// The setter preserves EAX. Raw calls below supply the value it must preserve.
game_fn! { #[allow(dead_code)] fn precision_set(mode: u32) -> u32 = "C" 0x0089_29E0; }
game_fn! { fn lua_call(state: *mut u8, arguments: u32, results: u32) -> u32 = "C" 0x008B_B3E0; }
game_fn! { fn lua_push_user_tag(state: *mut u8, pointer: u32, tag: u32) -> u32 = "C" 0x008B_F100; }
game_fn! { fn free(p: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn allocate(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn service_provider() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_reference(reference: *mut *mut u8) -> u32 = "thiscall" 0x0041_1870; }
game_fn! { fn interface_add_ref(this: *mut u8) -> u32 = "thiscall" 0x0096_8B60; }
game_fn! { fn find_key(this: *mut u8, key: *const u32) -> *mut u8 = "thiscall" 0x008E_B8D0; }
game_fn! { fn decode_pixels(destination: *mut u8, stride: u32, indices: u32, palette: *const u32) -> u32 = "stdcall" 0x008D_0FD0; }
game_data! { static node_allocator: *mut u8 = 0x00B5_C42C; }

port! {
    /// Increment the owner's count with 32-bit wrapping, preserving EDX.
    #[port(address = 0x008A_E800, owner = "shared", name = "IncrementLuaOwnerReference")]
    pub unsafe extern "fastcall" fn fc4p_008ae800(this: *mut u8, entry_edx: u32) -> u64 {
        let owner = unsafe { LuaOwner::from_ptr(this) };
        let count = owner.refs().wrapping_add(1);
        owner.set_refs(count);
        (u64::from(entry_edx) << 32) | u64::from(count)
    }
}
port! {
    /// Adjust the secondary interface to its owner and forward reference counting.
    #[port(address = 0x008B_03D0, owner = "shared", name = "IncrementLuaSecondaryReference")]
    pub unsafe extern "fastcall" fn fc4p_008b03d0(this: *mut u8, entry_edx: u32) -> u64 {
        // The owner entry preserves EDX, so pass it through the call explicitly.
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x008A_E800), this.wrapping_sub(8) as u32, &[]);
        call.edx = entry_edx;
        unsafe { call_raw(&mut call) };
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

port! {
    /// Destroy both strings in every list node, free the nodes, and reconnect the sentinel.
    #[port(address = 0x008B_15E0, owner = "shared", name = "ClearStringPairList")]
    pub unsafe extern "thiscall" fn fc4p_008b15e0(this: *mut u8) -> u32 {
        unsafe {
            let header = read::<*mut u8>(this, 0);
            let mut node = read::<*mut u8>(header, 0);
            while node != read::<*mut u8>(this, 0) {
                let next = read::<*mut u8>(node, 0);
                write(node, 0x28, 0x00A8_0810u32);
                let text = read::<*mut u8>(node, 0x2C);
                if !text.is_null() { free(text); }
                write(node, 0x0C, 0x00A8_0810u32);
                let text = read::<*mut u8>(node, 0x10);
                if !text.is_null() { free(text); }
                if (*node_allocator()).is_null() {
                    let mut reference = core::ptr::null_mut();
                    let provider = service_provider();
                    if !provider.is_null() {
                        vcall!(provider, 0x14 => fn(u32, u32, *mut *mut u8) -> u32,
                            0x3AE4_BEA3, 0x3AE4_BEAB, &mut reference);
                    }
                    *node_allocator() = reference;
                    release_reference(&mut reference);
                }
                vcall!(*node_allocator(), 0x10 => fn(*mut u8) -> u32, node);
                node = next;
            }
            let header = read::<*mut u8>(this, 0);
            write(header, 0, header);
            let header = read::<*mut u8>(this, 0);
            write(header, 4, header);
            header as u32
        }
    }
}

unsafe fn set_precision(mode: u32, eax: u32) -> u32 {
    let args = [mode];
    let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0089_29E0), 0, &args);
    call.eax = eax;
    unsafe { call_raw(&mut call) };
    call.eax
}
port! {
    /// Call Lua under double precision when the caller uses single precision, then restore it.
    #[port(address = 0x008B_7F40, owner = "shared", name = "CallLuaWithPrecisionGuard")]
    pub unsafe extern "thiscall" fn fc4p_008b7f40(this: *mut u8, arguments: u32, results: u32) -> u32 {
        unsafe {
            let saved = precision();
            if saved == 0 { set_precision(1, saved); }
            let result = lua_call(LuaThread::from_ptr(this).state(), arguments, results);
            if saved == 0 { set_precision(saved, result) } else { result }
        }
    }
}
port! {
    /// Push tagged userdata under the same precision guard as Lua calls.
    #[port(address = 0x008B_87A0, owner = "shared", name = "PushLuaUserTagWithPrecisionGuard")]
    pub unsafe extern "thiscall" fn fc4p_008b87a0(this: *mut u8, pointer: u32, tag: u32) -> u32 {
        unsafe {
            let saved = precision();
            if saved == 0 { set_precision(1, saved); }
            let result = lua_push_user_tag(LuaThread::from_ptr(this).state(), pointer, tag);
            if saved == 0 { set_precision(saved, result) } else { result }
        }
    }
}
port! {
    /// Expand sixteen two-bit palette indices into four rows of four pixels.
    #[port(address = 0x008D_0FD0, owner = "shared", name = "ExpandFourColorBlock")]
    pub unsafe extern "stdcall" fn fc4p_008d0fd0(destination: *mut u8, stride: u32, indices: u32, palette: *const u32) -> u32 {
        unsafe {
            let mut indices = indices;
            let mut row = destination;
            let mut last = 0;
            for _ in 0..4 {
                for pair in 0..2 {
                    let a = read_unaligned(palette.wrapping_add((indices & 3) as usize));
                    let b = read_unaligned(palette.wrapping_add(((indices >> 2) & 3) as usize));
                    indices >>= 4;
                    write(row, pair * 8, a);
                    write(row, pair * 8 + 4, b);
                    last = a;
                }
                row = row.wrapping_add(stride as usize);
            }
            last
        }
    }
}

fn rgb565(value: u16) -> u32 {
    let value = u32::from(value);
    let rb = (((value & 0xF800) << 5) + (value & 0x1F)) << 3;
    let rb = rb + (rb >> 5);
    let green = value & 0x7E0;
    (rb & !0xFF00) | (((green << 5) + (green >> 1)) & 0xFF00) | 0xFF00_0000
}
fn interpolate(a: u32, b: u32) -> u32 {
    let mut result = 0xFF00_0000;
    for shift in [0, 8, 16] {
        result |= ((((a >> shift) & 255) * 171 + ((b >> shift) & 255) * 85 + 128) >> 8) << shift;
    }
    result
}
port! {
    /// Decode a rectangular DXT1 block region. The entry exposes the caller-owned argument words.
    #[port(address = 0x008D_1030, owner = "shared", name = "DecodeDxt1Region", naked)]
    pub unsafe extern "C" fn fc4p_008d1030() {
        core::arch::naked_asm!("mov ecx, esp", "mov edx, eax", "jmp {body}", body = sym decode_dxt1_region)
    }
}
unsafe extern "fastcall" fn decode_dxt1_region(stack: *mut u32, entry_eax: u32) -> u32 {
    unsafe {
        let columns = stack.add(5).read();
        if columns == 0 {
            return entry_eax;
        }
        let mut rows = stack.add(6).read();
        if rows == 0 {
            return rows;
        }
        let mut destination = stack.add(1).read() as *mut u8;
        let destination_skip = stack
            .add(2)
            .read()
            .wrapping_sub(columns.wrapping_mul(4))
            .wrapping_mul(4);
        let mut source = stack.add(3).read() as *mut u8;
        let source_skip = stack.add(4).read().wrapping_sub(columns).wrapping_mul(8);
        loop {
            let mut left = stack.add(5).read();
            loop {
                let first = read::<u16>(source, 0);
                let second = read::<u16>(source, 2);
                let a = rgb565(first);
                let b = rgb565(second);
                let palette = if first > second {
                    [a, b, interpolate(a, b), interpolate(b, a)]
                } else {
                    [a, b, (a | b).wrapping_sub(((a ^ b) >> 1) & 0x7F7F_7F7F), 0]
                };
                decode_pixels(destination, stack.add(2).read(), read(source, 4), palette.as_ptr());
                destination = destination.wrapping_add(16);
                source = source.wrapping_add(8);
                left = left.wrapping_sub(1);
                if left == 0 {
                    break;
                }
            }
            destination = destination.wrapping_add(destination_skip as usize);
            source = source.wrapping_add(source_skip as usize);
            rows = stack.add(6).read().wrapping_sub(1);
            stack.add(6).write(rows);
            if rows == 0 {
                return 0;
            }
        }
    }
}

port! {
    /// Decrement counts above one; zero and one both invoke the deleting destructor.
    #[port(address = 0x008D_29B9, owner = "shared", name = "ReleaseCommandParameterSet")]
    pub unsafe extern "fastcall" fn fc4p_008d29b9(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe {
            let object = ParameterSet::from_ptr(this);
            if object.refs() > 1 {
                let refs = object.refs().wrapping_sub(1);
                object.set_refs(refs);
                return (u64::from(entry_edx) << 32) | u64::from(refs);
            }
            let result = vcall!(this, 0x3C => fn(u32) -> u64, 1);
            result & 0xFFFF_FFFF_0000_0000
        }
    }
}
port! {
    /// Query the three driver interfaces, keeping upper EAX bytes from the reference-count result.
    #[port(address = 0x008D_C4C0, owner = "shared", name = "QueryDriverInterfaces")]
    pub unsafe extern "thiscall" fn fc4p_008dc4c0(this: *mut u8, iid: u32, output: *mut *mut u8) -> u32 {
        unsafe {
            let adjustment = match iid {
                1 | 0xA455_4849 => 8,
                0x6695_65FE => 0,
                0xE69B_FE2A => 4,
                _ => {
                    let eax = if iid > 0xA455_4849 { iid } else { iid.wrapping_sub(1).wrapping_sub(0x6695_65FD) };
                    return eax & !255;
                }
            };
            output.write(if this.is_null() && adjustment != 0 { core::ptr::null_mut() } else { this.wrapping_add(adjustment) });
            (interface_add_ref(this.wrapping_add(12)) & !255) | 1
        }
    }
}

layout! {
    /// Hash buckets and cache fields used by the resource cache helpers.
    struct ResourceCache {
        0x04 => buckets / set_buckets: *mut u8,
        0x08 => buckets_end / set_buckets_end: *mut u8,
        0x0C => buckets_capacity / set_buckets_capacity: *mut u8,
        0x10 => count / set_count: u32,
        0x14 => free_first / set_free_first: *mut u8,
        0x24 => free_end / set_free_end: *mut u8,
        0x28 => free_end_block / set_free_end_block: *mut u8,
        0x2C => free_end_limit / set_free_end_limit: *mut u8,
        0x30 => free_end_node / set_free_end_node: *mut u8,
        0x40 => age / set_age: u32,
        0x50 => key_low / set_key_low: u32,
        0x54 => key_high / set_key_high: u32,
        0x58 => cached / set_cached: *mut u8,
    }
}
port! {
    /// Find the first matching pair in its hash chain, adding the key words with wrapping.
    #[port(address = 0x008E_B8D0, owner = "shared", name = "FindResourceKey")]
    pub unsafe extern "thiscall" fn fc4p_008eb8d0(this: *mut u8, key: *const u32) -> *mut u8 {
        unsafe {
            let cache = ResourceCache::from_ptr(this);
            let low = read_unaligned(key);
            let high = read_unaligned(key.add(1));
            let count = (cache.buckets_end() as u32).wrapping_sub(cache.buckets() as u32) as i32 >> 2;
            let bucket = divide(low.wrapping_add(high), count as u32).1;
            let mut node = read::<*mut u8>(cache.buckets(), bucket.wrapping_mul(4) as usize);
            while !node.is_null() {
                if read::<u32>(node, 8) == low && read::<u32>(node, 12) == high { break; }
                node = read(node, 0);
            }
            node
        }
    }
}
port! {
    /// Insert a resource pair at the head of its bucket unless the key already exists.
    #[port(address = 0x008E_B920, owner = "shared", name = "InsertResourceKey")]
    pub unsafe extern "thiscall" fn fc4p_008eb920(this: *mut u8, result: *mut u8, pair: *mut u8) -> *mut u8 {
        unsafe {
            let cache = ResourceCache::from_ptr(this);
            let low = read::<u32>(pair, 0);
            let high = read::<u32>(pair, 4);
            let count = (cache.buckets_end() as u32).wrapping_sub(cache.buckets() as u32) as i32 >> 2;
            let bucket = divide(low.wrapping_add(high), count as u32).1;
            let head = read::<*mut u8>(cache.buckets(), bucket.wrapping_mul(4) as usize);
            let mut node = head;
            while !node.is_null() {
                if read::<u32>(node, 8) == low && read::<u32>(node, 12) == high {
                    write(result, 4, this);
                    write(result, 0, node);
                    write(result, 8, 0u8);
                    return result;
                }
                node = read(node, 0);
            }
            node = allocate(0x20);
            write(node, 0, 0u32);
            if !node.wrapping_add(8).is_null() {
                for i in 0..5 { let value = read::<u32>(pair, i * 4); write(node, 8 + i * 4, value); }
            }
            write(node, 0, head);
            write(cache.buckets(), bucket.wrapping_mul(4) as usize, node);
            cache.set_count(cache.count().wrapping_add(1));
            write(result, 4, this);
            write(result, 0, node);
            write(result, 8, 1u8);
            result
        }
    }
}
port! {
    /// Look up and touch a cached resource, leaving output unchanged when the key is absent.
    #[port(address = 0x008E_BC60, owner = "shared", name = "FindAndTouchResource")]
    pub unsafe extern "thiscall" fn fc4p_008ebc60(this: *mut u8, low: u32, high: u32, output: *mut u32) -> u32 {
        unsafe {
            let cache = ResourceCache::from_ptr(this);
            if cache.key_low() != low || cache.key_high() != high || cache.cached().is_null() {
                let key = [low, high];
                let node = find_key(this, key.as_ptr());
                if node.is_null() { return 0; }
                cache.set_key_low(low);
                cache.set_key_high(high);
                cache.set_cached(node.wrapping_add(16));
            }
            write(cache.cached(), 4, cache.age());
            output.write(read(cache.cached(), 8));
            read(cache.cached(), 0)
        }
    }
}

port! {
    /// Build a five-byte instruction operand with a register mask and an unaligned immediate.
    #[port(address = 0x008E_C150, owner = "shared", name = "BuildInstructionOperand")]
    pub unsafe extern "thiscall" fn fc4p_008ec150(this: *mut u8, register: u32, mode: u32, immediate: u32) -> *mut u8 {
        unsafe {
            write(this, 5, 4u8);
            write(this, 4, 5u8);
            write(this, 0, 1u32.wrapping_shl(register));
            write(this, 6, (mode as u8).wrapping_mul(8).wrapping_add(register as u8).wrapping_mul(8).wrapping_add(5));
            write(this, 7, immediate);
            this
        }
    }
}
port! {
    /// Append an opcode, combined flags, and the operand bytes. Length is read again after each write.
    #[port(address = 0x008E_C190, owner = "shared", name = "AppendInstructionOperand")]
    pub unsafe extern "thiscall" fn fc4p_008ec190(this: *mut u8, opcode: u32, flags: u32, operand: *mut u8) -> u64 {
        unsafe {
            let cursor = read::<u32>(this, 8);
            write(this, cursor.wrapping_add(16) as usize, opcode as u8);
            write(this, 8, read::<u32>(this, 8).wrapping_add(1));
            let combined = read::<u8>(operand, 5) | flags as u8;
            let cursor = read::<u32>(this, 8);
            write(this, cursor.wrapping_add(16) as usize, combined);
            write(this, 8, read::<u32>(this, 8).wrapping_add(1));
            let length = read::<u8>(operand, 4);
            let mut edx = (cursor & !255) | u32::from(length);
            let mut copied = 0u32;
            if length != 0 {
                loop {
                    let value = read::<u8>(operand, copied.wrapping_add(6) as usize);
                    let cursor = read::<u32>(this, 8);
                    write(this, cursor.wrapping_add(16) as usize, value);
                    write(this, 8, read::<u32>(this, 8).wrapping_add(1));
                    edx = u32::from(read::<u8>(operand, 4));
                    copied = copied.wrapping_add(1);
                    if copied as i32 >= edx as i32 { break; }
                }
            }
            (u64::from(edx) << 32) | u64::from(copied)
        }
    }
}
port! {
    /// Move an iterator among blocks containing three forty-byte elements.
    #[port(address = 0x008E_C1E0, owner = "shared", name = "AdvanceThreeElementIterator")]
    pub unsafe extern "thiscall" fn fc4p_008ec1e0(this: *mut u8, offset: i32) -> u32 {
        unsafe {
            let current = read::<u32>(this, 0);
            let first = read::<u32>(this, 4);
            let position = (current.wrapping_sub(first) as i32 / 40).wrapping_add(offset);
            if (0..3).contains(&position) {
                write(this, 0, current.wrapping_add(offset.wrapping_mul(40) as u32));
                return offset.wrapping_mul(5) as u32;
            }
            let blocks = if position > 0 { position / 3 } else { position.wrapping_add(1) / 3 - 1 };
            let map = read::<*mut u8>(this, 12).wrapping_offset(blocks.wrapping_mul(4) as isize);
            write(this, 12, map);
            let first = read::<u32>(map, 0);
            write(this, 4, first);
            write(this, 8, first.wrapping_add(120));
            let position = position.wrapping_sub(blocks.wrapping_mul(3));
            write(this, 0, read::<u32>(this, 4).wrapping_add(position.wrapping_mul(40) as u32));
            read(this, 4)
        }
    }
}

unsafe fn initialize_deque(this: *mut u8, count: u32, per_block: u32, element_size: u32) -> u64 {
    unsafe {
        let (quotient, remainder) = divide(count, per_block);
        let blocks = quotient.wrapping_add(1);
        let map_size = (blocks + 2).max(8);
        write(this, 0x24, map_size);
        let map = allocate(map_size.wrapping_mul(4));
        let start = map.wrapping_add((read::<u32>(this, 0x24).wrapping_sub(blocks) >> 1).wrapping_mul(4) as usize);
        let end = start.wrapping_add(blocks.wrapping_mul(4) as usize);
        write(this, 0x20, map);
        let mut slot = start;
        while (slot as u32) < end as u32 {
            let block = allocate(120);
            write(slot, 0, block);
            slot = slot.wrapping_add(4);
        }
        write(this, 0x0C, start);
        let first = read::<*mut u8>(start, 0);
        write(this, 4, first);
        write(this, 8, first.wrapping_add(120));
        let last_slot = end.wrapping_sub(4);
        write(this, 0x1C, last_slot);
        let last = read::<*mut u8>(last_slot, 0);
        write(this, 0x14, last);
        write(this, 0x18, last.wrapping_add(120));
        write(this, 0, read::<u32>(this, 4));
        let last = read::<u32>(this, 0x14);
        let end_cursor = last.wrapping_add(remainder.wrapping_mul(element_size));
        write(this, 0x10, end_cursor);
        (u64::from(end_cursor) << 32) | u64::from(remainder.wrapping_mul(if element_size == 40 { 5 } else { 3 }))
    }
}
port! {
    /// Allocate centered map storage and blocks for ten twelve-byte elements per block.
    #[port(address = 0x008E_B800, owner = "shared", name = "InitializeTwelveByteDeque")]
    pub unsafe extern "thiscall" fn fc4p_008eb800(this: *mut u8, count: u32) -> u64 {
        unsafe { initialize_deque(this, count, 10, 12) }
    }
}
port! {
    /// Allocate centered map storage and blocks for three forty-byte elements per block.
    #[port(address = 0x008E_C270, owner = "shared", name = "InitializeFortyByteDeque")]
    pub unsafe extern "thiscall" fn fc4p_008ec270(this: *mut u8, count: u32) -> u64 {
        unsafe { initialize_deque(this, count, 3, 40) }
    }
}

port! {
    /// Walk a range of twelve-byte elements and copy the first block base to the end iterator argument.
    #[port(address = 0x008E_BB10, owner = "shared", name = "WalkTwelveByteRange", naked)]
    pub unsafe extern "C" fn fc4p_008ebb10() {
        core::arch::naked_asm!("mov ecx, esp", "jmp {body}", body = sym walk_twelve_byte_range)
    }
}
unsafe extern "thiscall" fn walk_twelve_byte_range(stack: *mut u32) -> u32 {
    unsafe {
        let mut current = stack.add(1).read();
        let first = stack.add(2).read();
        let mut last = stack.add(3).read();
        let mut node = stack.add(4).read() as *mut u32;
        let end = stack.add(5).read();
        stack.add(6).write(first);
        while current != end {
            current = current.wrapping_add(12);
            if current == last {
                node = node.wrapping_add(1);
                current = read_unaligned(node);
                last = current.wrapping_add(120);
            }
        }
        current
    }
}

/// Copy field words and the embedded sixteen bytes in their original order, including overlaps.
unsafe fn copy_forty_bytes(source: *mut u8, destination: *mut u8) {
    unsafe {
        write(destination, 0, read::<u32>(source, 0));
        write(destination, 4, read::<u32>(source, 4));
        for offset in 8..24 {
            write(destination, offset, read::<u8>(source, offset));
        }
        for offset in [24, 28, 32, 36] {
            write(destination, offset, read::<u32>(source, offset));
        }
    }
}

/// Number of elements between the two four-word iterators on a caller's stack.
unsafe fn forty_byte_range_length(stack: *mut u32) -> i32 {
    unsafe {
        let nodes = stack.add(9).read().wrapping_sub(stack.add(5).read()) as i32 >> 2;
        let tail = stack.add(6).read().wrapping_sub(stack.add(7).read()) as i32 / 40;
        let head = stack.add(4).read().wrapping_sub(stack.add(2).read()) as i32 / 40;
        nodes
            .wrapping_mul(3)
            .wrapping_add(tail)
            .wrapping_add(head)
            .wrapping_sub(3)
    }
}
port! {
    /// Copy a forty-byte deque range forward, returning the final destination iterator.
    #[port(address = 0x008E_C440, owner = "shared", name = "CopyFortyByteRangeForward", naked)]
    pub unsafe extern "C" fn fc4p_008ec440() {
        core::arch::naked_asm!("mov ecx, esp", "jmp {body}", body = sym copy_forty_byte_range_forward)
    }
}
unsafe extern "thiscall" fn copy_forty_byte_range_forward(stack: *mut u32) -> *mut u8 {
    unsafe {
        let count = forty_byte_range_length(stack);
        let mut source = stack.add(2).read() as *mut u8;
        let mut source_node = stack.add(5).read() as *mut u32;
        let mut destination = stack.add(10).read() as *mut u8;
        if count > 0 {
            stack.add(6).write(count as u32);
            loop {
                copy_forty_bytes(source, destination);
                source = source.wrapping_add(40);
                if source as u32 == stack.add(4).read() {
                    source_node = source_node.wrapping_add(1);
                    source = read_unaligned(source_node) as *mut u8;
                    stack.add(4).write(source.wrapping_add(120) as u32);
                }
                destination = destination.wrapping_add(40);
                if destination as u32 == stack.add(12).read() {
                    let node = (stack.add(13).read() as *mut u32).wrapping_add(1);
                    let first = read_unaligned(node);
                    stack.add(13).write(node as u32);
                    stack.add(11).write(first);
                    stack.add(12).write(first.wrapping_add(120));
                    destination = first as *mut u8;
                }
                let left = stack.add(6).read().wrapping_sub(1);
                stack.add(6).write(left);
                if left == 0 {
                    break;
                }
            }
        }
        let output = stack.add(1).read() as *mut u8;
        let first = stack.add(11).read();
        write(output, 0, destination);
        let last = stack.add(12).read();
        write(output, 4, first);
        let node = stack.add(13).read();
        write(output, 8, last);
        write(output, 12, node);
        output
    }
}
port! {
    /// Copy a forty-byte deque range backward, returning the final destination iterator.
    #[port(address = 0x008E_C340, owner = "shared", name = "CopyFortyByteRangeBackward", naked)]
    pub unsafe extern "C" fn fc4p_008ec340() {
        core::arch::naked_asm!("mov ecx, esp", "jmp {body}", body = sym copy_forty_byte_range_backward)
    }
}
unsafe extern "thiscall" fn copy_forty_byte_range_backward(stack: *mut u32) -> *mut u8 {
    unsafe {
        let count = forty_byte_range_length(stack);
        let mut source = stack.add(6).read() as *mut u8;
        let mut destination = stack.add(10).read() as *mut u8;
        let mut destination_first = stack.add(11).read();
        if count > 0 {
            stack.add(2).write(count as u32);
            loop {
                if source as u32 == stack.add(7).read() {
                    let node = (stack.add(9).read() as *mut u32).wrapping_sub(1);
                    stack.add(9).write(node as u32);
                    let first = read_unaligned(node);
                    stack.add(7).write(first);
                    source = first.wrapping_add(120) as *mut u8;
                }
                source = source.wrapping_sub(40);
                if destination as u32 == destination_first {
                    let node = (stack.add(13).read() as *mut u32).wrapping_sub(1);
                    destination_first = read_unaligned(node);
                    destination = destination_first.wrapping_add(120) as *mut u8;
                    stack.add(13).write(node as u32);
                    stack.add(12).write(destination as u32);
                }
                destination = destination.wrapping_sub(40);
                copy_forty_bytes(source, destination);
                let left = stack.add(2).read().wrapping_sub(1);
                stack.add(2).write(left);
                if left == 0 {
                    break;
                }
            }
        }
        let output = stack.add(1).read() as *mut u8;
        write(output, 0, destination);
        let last = stack.add(12).read();
        write(output, 4, destination_first);
        let node = stack.add(13).read();
        write(output, 8, last);
        write(output, 12, node);
        output
    }
}

layout! {
    /// Fixed-point state shared by the depth, texture and color edge initializers.
    struct RasterEdge {
        0x04 => x_slope / set_x_slope: i32,
        0x08 => texture_u / set_texture_u: u32,
        0x0C => texture_v / set_texture_v: u32,
        0x10 => texture_slope_u / set_texture_slope_u: u32,
        0x14 => texture_slope_v / set_texture_slope_v: u32,
        0x18 => colors / set_colors: [u32; 4],
        0x28 => color_slopes / set_color_slopes: [u32; 4],
        0x38 => depth / set_depth: u32,
        0x3C => depth_slope / set_depth_slope: u32,
        0x40 => fraction_x / set_fraction_x: i32,
        0x44 => fraction_y / set_fraction_y: i32,
    }
}
layout! {
    struct RasterVertex {
        0x08 => depth / set_depth: u16,
        0x0C => color / set_color: u32,
        0x10 => texture_u / set_texture_u: u32,
        0x14 => texture_v / set_texture_v: u32,
    }
}
game_data! { static depth_step_x: i32 = 0x00B5_CA00; }
game_data! { static depth_step_y: i32 = 0x00B5_CA04; }
game_data! { static texture_steps_x: [i32; 2] = 0x00B5_C9F0; }
game_data! { static texture_steps_y: [i32; 2] = 0x00B5_C9F8; }
game_data! { static color_steps_x: [i32; 4] = 0x00B5_CA08; }
game_data! { static color_steps_y: [i32; 4] = 0x00B5_CA18; }

/// Signed products retain all 64 bits before the fixed-point rounding bias is added.
fn rounded_depth_product(a: i32, b: i32) -> u64 {
    (i64::from(a).wrapping_mul(i64::from(b)) as u64).wrapping_add(0x4000)
}
port! {
    /// Set a raster edge's depth slope and starting depth with 15-bit fixed-point rounding.
    /// The edge and vertex arrive in EBP and ESI; EDI, EDX and EAX hold arithmetic results.
    #[port(address = 0x008D_D620, owner = "shared", name = "SetRasterDepthEdge", naked)]
    pub unsafe extern "C" fn fc4p_008dd620() {
        core::arch::naked_asm!(
            "push ecx", "mov ecx, ebp", "mov edx, esi", "call {body}",
            "pop ecx", "mov edi, [ebp + 0x38]", "ret", body = sym set_raster_depth_edge)
    }
}
unsafe extern "fastcall" fn set_raster_depth_edge(edge: *mut u8, vertex: *mut u8) -> u64 {
    unsafe {
        let edge = RasterEdge::from_ptr(edge);
        let vertex = RasterVertex::from_ptr(vertex);
        let slope = (rounded_depth_product(edge.x_slope(), *depth_step_x()) >> 15) as u32;
        let step_y = *depth_step_y();
        edge.set_depth_slope(slope.wrapping_sub(step_y as u32));
        let vertical = (rounded_depth_product(step_y, edge.fraction_y()) >> 15) as u32;
        let base = (u32::from(vertex.depth()) << 16).wrapping_sub(vertical);
        let horizontal = rounded_depth_product(*depth_step_x(), edge.fraction_x());
        let shifted = (horizontal >> 15) as u32;
        edge.set_depth(base.wrapping_add(shifted));
        (u64::from(shifted) << 32) | u64::from(horizontal as u32)
    }
}

fn fixed_texture_product(a: i32, b: i32) -> u64 {
    i64::from(a).wrapping_mul(i64::from(b)) as u64
}
port! {
    /// Set the texture-coordinate slopes and starting coordinates of a raster edge.
    /// EBP and ESI select the edge and vertex; EBX supplies the vertical fraction.
    #[port(address = 0x008D_D680, owner = "shared", name = "SetRasterTextureEdge", naked)]
    pub unsafe extern "C" fn fc4p_008dd680() {
        core::arch::naked_asm!(
            "push ecx", "sub esp, 4", "lea eax, [esp]", "push eax", "push ebx",
            "push dword ptr [esp + 0x3c]", "mov ecx, ebp", "mov edx, esi", "call {body}",
            "pop edi", "pop ecx", "ret", body = sym set_raster_texture_edge)
    }
}
unsafe extern "fastcall" fn set_raster_texture_edge(
    edge: *mut u8,
    vertex: *mut u8,
    flags: u32,
    fraction_y: i32,
    result_edi: *mut u32,
) -> u64 {
    unsafe {
        let vertex = RasterVertex::from_ptr(vertex);
        let slope_u = (fixed_texture_product(read(edge, 4), (*texture_steps_x())[0]) >> 15) as u32;
        let u = vertex.texture_u();
        write(edge, 0x10, slope_u.wrapping_sub((*texture_steps_y())[0] as u32));
        let shift = if flags & 8 == 0 { 20 } else { 16 };
        let u = u
            .wrapping_shl(shift)
            .wrapping_sub((fixed_texture_product((*texture_steps_y())[0], fraction_y) >> 15) as u32)
            .wrapping_add((fixed_texture_product((*texture_steps_x())[0], read(edge, 0x40)) >> 15) as u32);
        write(edge, 8, u);
        let slope_v = (fixed_texture_product(read(edge, 4), (*texture_steps_x())[1]) >> 15) as u32;
        let v = vertex.texture_v();
        write(edge, 0x14, slope_v.wrapping_sub((*texture_steps_y())[1] as u32));
        let v = v
            .wrapping_shl(shift)
            .wrapping_sub((fixed_texture_product((*texture_steps_y())[1], fraction_y) >> 15) as u32);
        let horizontal_v = fixed_texture_product((*texture_steps_x())[1], read(edge, 0x40));
        let edx = (horizontal_v >> 15) as u32;
        let edi = v.wrapping_add(edx);
        write(edge, 12, edi);
        result_edi.write(edi);

        // The second coordinate reverses its direction before the pixel-boundary correction.
        let dx = [(*texture_steps_x())[0], !(*texture_steps_x())[1]];
        let dy = [(*texture_steps_y())[0], !(*texture_steps_y())[1]];
        let mask = [
            if dx[0] == 0 { u32::MAX } else { 0 },
            if dx[1] == 0 { u32::MAX } else { 0 },
        ];
        let corrected_x = [
            read::<u32>(edge, 8).wrapping_add((dx[0] >> 31) as u32),
            read::<u32>(edge, 12).wrapping_add((dx[1] >> 31) as u32),
        ];
        let corrected_y = [
            corrected_x[0].wrapping_add(((dy[0] >> 31) as u32) & mask[0]),
            corrected_x[1].wrapping_add(((dy[1] >> 31) as u32) & mask[1]),
        ];
        write(edge, 8, corrected_y[0]);
        write(edge, 12, corrected_y[1]);
        let mm = [
            corrected_x[0],
            corrected_x[1],
            corrected_y[0],
            corrected_y[1],
            0,
            0,
            mask[0],
            mask[1],
        ];
        // MMX results are live inputs to the following span renderer, including the full tag state.
        core::arch::asm!(
            "movq mm0, qword ptr [{mm}]", "movq mm1, qword ptr [{mm} + 8]",
            "movq mm2, qword ptr [{mm} + 16]", "movq mm3, qword ptr [{mm} + 24]",
            mm = in(reg) mm.as_ptr(), out("mm0") _, out("mm1") _, out("mm2") _, out("mm3") _, options(nostack, readonly));
        (u64::from(edx) << 32) | u64::from((horizontal_v as u32) >> 15)
    }
}

port! {
    /// Set four color slopes and starting color channels for a raster edge.
    /// The entry uses EBP and ESI and leaves all color accumulators in MMX registers.
    #[port(address = 0x008D_D760, owner = "shared", name = "SetRasterColorEdge", naked)]
    pub unsafe extern "C" fn fc4p_008dd760() {
        core::arch::naked_asm!(
            "push ecx", "sub esp, 16", "lea eax, [esp]", "push eax", "mov ecx, ebp", "mov edx, esi", "call {body}",
            "pop eax", "pop edx", "pop ebx", "pop edi", "pop ecx", "ret", body = sym set_raster_color_edge)
    }
}
unsafe extern "fastcall" fn set_raster_color_edge(edge: *mut u8, vertex: *mut u8, registers: *mut u32) {
    unsafe {
        for channel in 0..4 {
            let product = rounded_depth_product((*color_steps_x())[channel], read(edge, 4));
            write(
                edge,
                0x28 + channel * 4,
                ((product >> 15) as u32).wrapping_sub((*color_steps_y())[channel] as u32),
            );
        }
        let dx = *color_steps_x();
        let fraction_x = read::<i32>(edge, 0x40);
        let horizontal = [
            color_horizontal(dx[0], fraction_x),
            color_horizontal(dx[1], fraction_x),
            color_horizontal(dx[2], fraction_x),
            color_horizontal(dx[3], fraction_x),
        ];
        let dy = *color_steps_y();
        let fraction_y = read::<i32>(edge, 0x44);
        let vertical = [
            (dy[0] >> 12).wrapping_mul(fraction_y),
            (dy[1] >> 12).wrapping_mul(fraction_y),
            (dy[2] >> 12).wrapping_mul(fraction_y),
            (dy[3] >> 12).wrapping_mul(fraction_y),
        ];
        let offset = [
            (horizontal[0].wrapping_sub(vertical[0]) >> 3) as u32,
            (horizontal[1].wrapping_sub(vertical[1]) >> 3) as u32,
            (horizontal[2].wrapping_sub(vertical[2]) >> 3) as u32,
            (horizontal[3].wrapping_sub(vertical[3]) >> 3) as u32,
        ];
        let color = RasterVertex::from_ptr(vertex).color();
        let channels = [
            ((color & 255) << 19).wrapping_add(offset[0]),
            ((color >> 8 & 255) << 19).wrapping_add(offset[1]),
            ((color >> 16 & 255) << 19).wrapping_add(offset[2]),
            ((color >> 24) << 19).wrapping_add(offset[3]),
        ];
        write(edge, 0x18, channels[0]);
        write(edge, 0x1C, channels[1]);
        write(edge, 0x20, channels[2]);
        write(edge, 0x24, channels[3]);
        registers.write(vertical[1] as u32);
        registers.add(1).write(vertical[0] as u32);
        registers.add(2).write(vertical[2] as u32);
        registers.add(3).write(vertical[3] as u32);
        let mm = [
            vertical[2] as u32,
            vertical[3] as u32,
            channels[2],
            channels[3],
            channels[0],
            channels[1],
            0,
            0,
            offset[2],
            offset[3],
            vertical[0] as u32,
            vertical[1] as u32,
            offset[0],
            offset[1],
        ];
        core::arch::asm!(
            "movq mm0, qword ptr [{mm}]", "movq mm1, qword ptr [{mm} + 8]", "movq mm2, qword ptr [{mm} + 16]",
            "movq mm4, qword ptr [{mm} + 24]", "movq mm5, qword ptr [{mm} + 32]", "movq mm6, qword ptr [{mm} + 40]", "movq mm7, qword ptr [{mm} + 48]",
            mm = in(reg) mm.as_ptr(), out("mm0") _, out("mm1") _, out("mm2") _, out("mm4") _, out("mm5") _, out("mm6") _, out("mm7") _, options(nostack, readonly));
    }
}
fn color_horizontal(step: i32, fraction: i32) -> i32 {
    (step >> 12).wrapping_mul(fraction).wrapping_add(0x20_4000)
}

layout! {
    struct FontNameOwner { 0x08 => face / set_face: *mut u8 }
}
layout! {
    struct FontNameFace {
        0x50 => byte_name / set_byte_name: *mut u8,
        0x54 => wide_name / set_wide_name: *mut u8,
        0x180 => naming_mode / set_naming_mode: *mut u8,
    }
}
layout! {
    struct FontNameString {
        0x04 => first / set_first: *mut u8,
        0x08 => end / set_end: *mut u8,
    }
}
// These calls use RawCall below because EDX survives into the caller-visible result.
game_fn! { #[allow(dead_code)] fn select_font_name(face: *mut u8, language: u32, kind: u32, result: *mut u32) -> u64 = "C" 0x0089_F353; }
game_fn! { #[allow(dead_code)] fn wide_name_vector(this: *mut u32, text: *mut u8, allocator: *mut u8) -> u64 = "thiscall" 0x0079_CA50; }
game_fn! { #[allow(dead_code)] fn byte_name_vector(this: *mut u32, text: *mut u8, allocator: *mut u8) -> u64 = "thiscall" 0x0040_8000; }
game_fn! { #[allow(dead_code)] fn convert_font_name(text: *mut u8, count: u32, encoding: u32, output: *mut u8) -> u64 = "C" 0x0091_1358; }
game_fn! { #[allow(dead_code)] fn erase_name_range(this: *mut u8, start: u32, count: u32) -> u64 = "thiscall" 0x0044_25C0; }
game_fn! { #[allow(dead_code)] fn trim_font_name(this: *mut u8) -> u64 = "thiscall" 0x0090_E8A6; }
game_data! { static bold_name: u8 = 0x00AA_1260; }
game_data! { static italic_name: u8 = 0x00AA_1258; }

unsafe fn font_name_call(conv: Conv, target: usize, this: u32, args: &[u32], edx: &mut u32) -> u32 {
    let mut call = RawCall::new(conv, target, this, args);
    call.edx = *edx;
    unsafe {
        call_raw(&mut call);
    }
    *edx = call.edx;
    call.eax
}
unsafe fn missing_font_name(this: *mut u8) -> bool {
    unsafe {
        let face = FontNameFace::from_ptr(FontNameOwner::from_ptr(this).face());
        face.wide_name().is_null() && face.byte_name().is_null()
    }
}
port! {
    /// Select a font name through the language and encoding fallbacks, then separate family and style.
    #[port(address = 0x0089_801E, owner = "shared", name = "ReadFontName", naked)]
    pub unsafe extern "C" fn fc4p_0089801e() {
        core::arch::naked_asm!(
            "push edx", "lea eax, [esp + 4]", "push eax", "push ecx",
            "call {body}", "ret 8", body = sym read_font_name)
    }
}
unsafe extern "stdcall" fn read_font_name(this: *mut u8, stack: *mut u32, mut edx: u32) -> u64 {
    unsafe {
        let output = stack.add(1).read() as *mut u8;
        let output_view = FontNameString::from_ptr(output);
        let kind = stack.add(2).read();
        if !read::<*mut u8>(this, 8).is_null() {
            let mut selected = 0u32;
            for index in 0..8 {
                if index != 0 && !missing_font_name(this) {
                    continue;
                }
                let language = if index & 2 == 0 { 0x409 } else { 0 };
                let requested_kind = if index < 4 { kind } else { 4 };
                let alternate = index & 1 != 0;
                if alternate {
                    let face = read::<*mut u8>(this, 8);
                    write(read::<*mut u8>(face, 0x180), 0x76, 0u16);
                }
                font_name_call(
                    Conv::Cdecl,
                    crate::abi::target(0x0089_F353),
                    0,
                    &[
                        read::<u32>(this, 8),
                        language,
                        requested_kind,
                        &mut selected as *mut u32 as u32,
                    ],
                    &mut edx,
                );
                if alternate {
                    let face = read::<*mut u8>(this, 8);
                    write(read::<*mut u8>(face, 0x180), 0x76, 1u16);
                }
            }
            let face = read::<*mut u8>(this, 8);
            let wide = read::<u32>(face, 0x54);
            let text = if wide != 0 { wide } else { read::<u32>(face, 0x50) };
            if text != 0 {
                let mut vector = [0u32; 3];
                font_name_call(
                    Conv::Thiscall,
                    crate::abi::target(if wide != 0 { 0x0079_CA50 } else { 0x0040_8000 }),
                    vector.as_mut_ptr() as u32,
                    &[text, stack.cast::<u8>().add(7) as u32],
                    &mut edx,
                );
                let bytes = vector[1].wrapping_sub(vector[0]);
                let length = if wide != 0 { (bytes as i32 >> 1) as u32 } else { bytes };
                font_name_call(
                    Conv::Cdecl,
                    crate::abi::target(0x0091_1358),
                    0,
                    &[vector[0], length, if wide != 0 { 0x10 } else { 1252 }, output as u32],
                    &mut edx,
                );
                if vector[0] != 0 {
                    font_name_call(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &[vector[0]], &mut edx);
                }
            } else {
                let first = output_view.first();
                if first != output_view.end() {
                    first.write(0);
                    output_view.set_end(output_view.first());
                }
            }
            if kind == 1 || kind == 2 {
                let bold = font_name_call(
                    Conv::Thiscall,
                    crate::abi::vslot(output, 0x68),
                    output as u32,
                    &[bold_name() as u32, u32::MAX, 0],
                    &mut edx,
                );
                stack.add(1).write(bold);
                let italic = font_name_call(
                    Conv::Thiscall,
                    crate::abi::vslot(output, 0x68),
                    output as u32,
                    &[italic_name() as u32, u32::MAX, 0],
                    &mut edx,
                );
                stack.add(2).write(italic);
                if kind == 1 {
                    if bold != u32::MAX {
                        font_name_call(
                            Conv::Thiscall,
                            crate::abi::target(0x0044_25C0),
                            output.wrapping_add(4) as u32,
                            &[bold, u32::MAX],
                            &mut edx,
                        );
                    }
                    let italic = stack.add(2).read();
                    if italic != u32::MAX {
                        font_name_call(
                            Conv::Thiscall,
                            crate::abi::target(0x0044_25C0),
                            output.wrapping_add(4) as u32,
                            &[italic, u32::MAX],
                            &mut edx,
                        );
                    }
                } else {
                    let first_style = bold.min(italic);
                    if first_style != u32::MAX {
                        font_name_call(
                            Conv::Thiscall,
                            crate::abi::target(0x0044_25C0),
                            output.wrapping_add(4) as u32,
                            &[0, first_style],
                            &mut edx,
                        );
                    }
                }
                font_name_call(
                    Conv::Thiscall,
                    crate::abi::target(0x0090_E8A6),
                    output as u32,
                    &[],
                    &mut edx,
                );
            }
        }
        let first = output_view.first() as u32;
        let nonempty = u32::from(first != output_view.end() as u32);
        (u64::from(edx) << 32) | u64::from((first & !255) | nonempty)
    }
}

game_fn! { fn prime_lower_bound(first: *const u32, last: *const u32, key: *const u32, count: u32, unused: u32) -> *const u32 = "C" 0x0079_FA90; }
game_fn! { fn bucket_vector(this: *mut u32, count: u32, fill: *const u32, allocator: *const u32) -> u32 = "thiscall" 0x0051_C950; }
game_fn! { fn resource_begin(this: *mut u8, result: *mut u32) -> u32 = "thiscall" 0x004B_0390; }
game_fn! { fn resource_next_bucket(this: *mut u32) -> *mut u8 = "thiscall" 0x008E_BAC0; }
game_fn! { fn resource_erase(this: *mut u8, iterator: *const u32) -> u32 = "thiscall" 0x008E_B9E0; }
game_fn! { fn resource_push_back(this: *mut u8, value: *mut u8) -> u32 = "thiscall" 0x008E_BD70; }
game_fn! { fn resource_reserve(this: *mut u8, count: u32) -> u32 = "thiscall" 0x008E_BB70; }
game_fn! { fn resource_insert(this: *mut u8, result: *mut u32, value: *const u32) -> u64 = "thiscall" 0x008E_B920; }
game_data! { static resource_primes: u32 = 0x00A8_29B0; }
game_data! { static resource_primes_end: u32 = 0x00A8_2A20; }
layout! {
    struct ResourceNode {
        0x00 => next / set_next: *mut u8,
        0x08 => key_low / set_key_low: u32,
        0x0C => key_high / set_key_high: u32,
        0x10 => slot / set_slot: u32,
        0x14 => age / set_age: u32,
        0x18 => value / set_value: u32,
    }
}

port! {
    /// Grow the cache's hash buckets to the next prime, reversing each moved chain.
    #[port(address = 0x008E_BB70, owner = "shared", name = "GrowResourceBuckets", naked)]
    pub unsafe extern "C" fn fc4p_008ebb70() {
        core::arch::naked_asm!("mov edx, esp", "jmp {body}", body = sym grow_resource_buckets)
    }
}
unsafe extern "fastcall" fn grow_resource_buckets(this: *mut u8, stack: *mut u32, _count: u32) -> u32 {
    unsafe {
        let old_count = (read::<u32>(this, 8).wrapping_sub(read(this, 4)) as i32 >> 2) as u32;
        let requested = stack.add(1).read();
        if requested <= old_count {
            return requested;
        }
        // The argument also serves as the allocator token supplied to the vector constructor.
        stack.add(1).write(requested);
        let position = prime_lower_bound(resource_primes(), resource_primes_end(), stack.add(1), requested, 0);
        let count = if position == resource_primes_end() {
            0xFFFF_FFFB
        } else {
            position.read()
        };
        stack.add(1).write(count);
        if count <= old_count {
            return count;
        }
        let zero = 0;
        let mut vector = [0u32; 3];
        bucket_vector(vector.as_mut_ptr(), count, &zero, stack.add(1));
        let buckets = vector[0] as *mut u8;
        let mut index = 0;
        while index < old_count {
            let mut node = read::<*mut u8>(read(this, 4), index as usize * 4);
            while !node.is_null() {
                let slot = read::<*mut u8>(this, 4).wrapping_add(index as usize * 4);
                let view = ResourceNode::from_ptr(node);
                let hash = view.key_high().wrapping_add(view.key_low());
                let bucket = divide(hash, stack.add(1).read()).1;
                write(slot, 0, read::<u32>(node, 0));
                write(node, 0, read::<u32>(buckets, bucket as usize * 4));
                write(buckets, bucket as usize * 4, node);
                node = read(read::<*mut u8>(this, 4).wrapping_add(index as usize * 4), 0);
            }
            index = index.wrapping_add(1);
        }
        let old = read::<*mut u8>(this, 4);
        write(this, 4, buckets);
        write(this, 8, vector[1]);
        write(this, 12, vector[2]);
        if old.is_null() {
            0
        } else {
            free(old)
        }
    }
}

/// Append one free cache entry, using the deque's boundary helper when its final slot is reached.
unsafe fn append_free_resource(this: *mut u8, value: *mut u8) {
    unsafe {
        let destination = read::<*mut u8>(this, 0x24);
        if destination == read::<*mut u8>(this, 0x2C).wrapping_sub(12) {
            resource_push_back(this.wrapping_add(0x14), value);
        } else {
            if !destination.is_null() {
                write(destination, 0, read::<u32>(value, 0));
                write(destination, 4, read::<u32>(value, 4));
                write(destination, 8, read::<u32>(value, 8));
            }
            write(this, 0x24, read::<u32>(this, 0x24).wrapping_add(12));
        }
    }
}

port! {
    /// Recycle an old cache slot, falling back to the first entry when no old slot was collected.
    /// An erased entry's successor is advanced once more before the age scan continues.
    #[port(address = 0x008E_BE30, owner = "shared", name = "RecycleResourceSlot")]
    pub unsafe extern "thiscall" fn fc4p_008ebe30(this: *mut u8, low: u32, high: u32, value: u32) -> u64 {
        unsafe {
            if read::<u32>(this, 0x24) == read::<u32>(this, 0x14) {
                let mut iterator = [0u32; 2];
                resource_begin(this, iterator.as_mut_ptr());
                while iterator[0] != 0 {
                    let node = iterator[0] as *mut u8;
                    if ResourceNode::from_ptr(node).age() < read::<u32>(this, 0x40).wrapping_sub(2) {
                        let mut next = [read::<u32>(node, 0), iterator[1]];
                        if next[0] == 0 { next[0] = resource_next_bucket(iterator.as_mut_ptr()) as u32; }
                        append_free_resource(this, node.wrapping_add(0x10));
                        resource_erase(this, iterator.as_ptr());
                        write(this, 0x58, 0u32);
                        iterator = next;
                        if iterator[0] == 0 { break; }
                    }
                    let next = read::<u32>(iterator[0] as *mut u8, 0);
                    iterator[0] = if next == 0 { resource_next_bucket(iterator.as_mut_ptr()) as u32 } else { next };
                }
                if read::<u32>(this, 0x24) == read::<u32>(this, 0x14) {
                    resource_begin(this, iterator.as_mut_ptr());
                    append_free_resource(this, (iterator[0] as *mut u8).wrapping_add(0x10));
                    resource_begin(this, iterator.as_mut_ptr());
                    resource_erase(this, iterator.as_ptr());
                    write(this, 0x58, 0u32);
                }
            }
            let mut end = read::<*mut u8>(this, 0x24);
            if end == read::<*mut u8>(this, 0x28) {
                end = read::<*mut u8>(read::<*mut u8>(this, 0x30).wrapping_sub(4), 0).wrapping_add(120);
            }
            let slot = read::<u32>(end.wrapping_sub(12), 0);
            let stamp = read::<u32>(end.wrapping_sub(8), 0);
            if read::<u32>(this, 0x24) != read::<u32>(this, 0x28) {
                write(this, 0x24, read::<u32>(this, 0x24).wrapping_sub(12));
            } else {
                let block = read::<*mut u8>(this, 0x28);
                if !block.is_null() { free(block); }
                let map_slot = read::<*mut u8>(this, 0x30).wrapping_sub(4);
                write(this, 0x30, map_slot);
                let block = read::<*mut u8>(map_slot, 0);
                write(this, 0x28, block);
                write(this, 0x2C, block.wrapping_add(120));
                write(this, 0x24, block.wrapping_add(108));
            }
            let pair = [low, high, slot, stamp, value];
            resource_reserve(this, read::<u32>(this, 0x10).wrapping_add(1));
            let mut result = [slot, stamp, value];
            let returned = resource_insert(this, result.as_mut_ptr(), pair.as_ptr());
            (returned & 0xFFFF_FFFF_0000_0000) | u64::from(slot)
        }
    }
}

layout! {
    /// Command records are 44 bytes; input and output parameter sets are retained separately.
    struct LuaCommandGenerator {
        0x10 => main_thread / set_main_thread: *mut u8,
        0x14 => records / set_records: *mut u8,
        0x18 => records_end / set_records_end: *mut u8,
        0x38 => input / set_input: *mut u8,
        0x3C => output / set_output: *mut u8,
        0x40 => server / set_server: *mut u8,
    }
}
layout! {
    /// A temporary command string owns the allocation in its second word.
    struct CommandString {
        0x00 => vtable / set_vtable: u32,
        0x04 => buffer / set_buffer: *mut u8,
    }
}

game_fn! { fn command_variant_new(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_66D4; }
game_fn! { fn command_variant_bool(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0091_6657; }
game_fn! { fn command_variant_number(this: *mut u8, value: f64) -> u32 = "thiscall" 0x0091_66AA; }
game_fn! { fn command_variant_string(this: *mut u8, value: *mut u8) -> u32 = "thiscall" 0x0091_66CF; }
game_fn! { fn command_variant_destroy(this: *mut u8) -> u32 = "thiscall" 0x0091_5EB5; }
game_fn! { fn command_string_from_chars(this: *mut u8, value: *const u8) -> *mut u8 = "thiscall" 0x0040_8480; }
game_fn! { fn command_string_new(this: *mut u8) -> *mut u8 = "thiscall" 0x0057_8390; }
game_fn! { fn command_string_chars(this: *mut u8) -> *const u8 = "thiscall" 0x009D_7E63; }
game_data! { static command_zero: f64 = 0x00A8_0990; }
game_data! { static command_invalid_status: f64 = 0x00A8_0AC8; }

/// Call a saved virtual table: upvalue and value conversions can change the object's current table.
unsafe fn command_saved_virtual(this: *mut u8, table: *mut u8, slot: usize, args: &[u32]) -> u32 {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, read(table, slot), this as u32, args);
        call_raw(&mut call);
        call.eax
    }
}

/// The iterator methods receive their table in EDX, and its returned value survives cleanup.
unsafe fn command_iterator_call(this: *mut u8, slot: usize, args: &[u32], eax: u32) -> u64 {
    unsafe {
        let table = read::<*mut u8>(this, 0);
        let mut call = RawCall::new(Conv::Thiscall, read(table, slot), this as u32, args);
        call.eax = eax;
        call.edx = table as u32;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

unsafe fn command_failure(thread: *mut u8) -> u64 {
    unsafe {
        let bits = crate::x87::F80::from_f64(*command_invalid_status()).to_f64().to_bits();
        let result = command_iterator_call(thread, 0xC4, &[bits as u32, (bits >> 32) as u32], 0);
        (result & 0xFFFF_FFFF_0000_0000) | 1
    }
}

port! {
    /// Convert Lua arguments to command parameters, execute the captured record, and push its results.
    #[port(address = 0x008B_5B70, owner = "shared", name = "InvokeLuaCommand")]
    pub unsafe extern "C" fn fc4p_008b5b70(state: *mut u8) -> u64 {
        unsafe {
            let thread = read::<*mut u8>(state.wrapping_sub(4), 0);
            let table = read::<*mut u8>(thread, 0);
            let index = command_saved_virtual(thread, table, 0x148, &[2]);
            let generator = command_saved_virtual(thread, table, 0xB8, &[index]) as *mut u8;
            let table = read::<*mut u8>(thread, 0);
            let index = command_saved_virtual(thread, table, 0x148, &[3]);
            let main = command_saved_virtual(thread, table, 0xB8, &[index]) as *mut u8;
            if generator.is_null() { return command_failure(thread); }
            let generator = LuaCommandGenerator::from_ptr(generator);
            if generator.server().is_null() || main != generator.main_thread() {
                return command_failure(thread);
            }
            let table = read::<*mut u8>(thread, 0);
            let index = command_saved_virtual(thread, table, 0x148, &[1]);
            let number = crate::abi::call_st0(Conv::Thiscall, read(table, 0xA4), thread as u32, &[index]);
            let index = number.ftol() as i32;
            let records = generator.records();
            let length = (generator.records_end() as u32).wrapping_sub(records as u32) as i32 / 44;
            if index >= length || index < 0 { return command_failure(thread); }
            let record = records.wrapping_add((index as usize).wrapping_mul(44));
            vcall!(generator.input(), 0x10 => fn(u32), 0);
            vcall!(generator.output(), 0x10 => fn(u32), 0);
            let arguments = vcall!(thread, 0x38 => fn() -> i32);
            let mut temporaries = core::mem::MaybeUninit::<[u32; 15]>::uninit();
            let variant = temporaries.as_mut_ptr().cast::<u8>();
            let string = variant.wrapping_add(20);
            let input_string = variant.wrapping_add(40);
            let mut argument = 1i32;
            while argument <= arguments {
                command_variant_new(variant);
                match vcall!(thread, 0x5C => fn(i32) -> u32, argument) {
                    1 => {
                        let value = vcall!(thread, 0xA8 => fn(i32) -> u32, argument);
                        command_variant_bool(variant, value);
                    }
                    3 => {
                        let value = vcall_st0!(thread, 0xA4, argument).to_f64();
                        command_variant_number(variant, value);
                    }
                    4 => {
                        let value = vcall!(thread, 0xAC => fn(i32) -> *const u8, argument);
                        let value = command_string_from_chars(input_string, value);
                        command_variant_string(variant, value);
                        let temporary = CommandString::from_ptr(input_string);
                        let buffer = temporary.buffer();
                        temporary.set_vtable(0x00A8_0810);
                        if !buffer.is_null() { free(buffer); }
                    }
                    _ => {
                        let value = crate::x87::F80::from_f64(*command_zero()).to_f64();
                        command_variant_number(variant, value);
                    }
                }
                vcall!(generator.input(), 0x1C => fn(i32, *mut u8), argument.wrapping_sub(1), variant);
                command_variant_destroy(variant);
                argument = argument.wrapping_add(1);
            }
            let output = generator.output();
            let input = generator.input();
            let status = vcall!(generator.server(), 0x50 => fn(u32, *mut u8, *mut u8) -> i32,
                read::<u32>(record, 0), input, output);
            let status = crate::x87::F80::from_i32(status).to_f64();
            vcall!(thread, 0xC4 => fn(f64), status);
            command_string_new(string);
            let mut count = 1u32;
            let mut result = command_iterator_call(generator.output(), 0x34, &[], string as u32);
            let mut key = result as u32;
            while key != u32::MAX {
                if key != 0x2B99_9D2B {
                    let value = vcall!(generator.output(), 0x18 => fn(u32) -> *mut u8, key);
                    if value.is_null() {
                        vcall!(thread, 0xC0 => fn());
                    } else {
                        let kind = vcall!(value, 0x0C => fn() -> i32);
                        match kind {
                            1 => {
                                let table = read::<*mut u8>(thread, 0);
                                let value = vcall!(value, 0x24 => fn() -> u32);
                                command_saved_virtual(thread, table, 0xD4, &[value]);
                            }
                            2..=14 => {
                                let table = read::<*mut u8>(thread, 0);
                                let value = vcall_st0!(value, 0x9C).to_f64().to_bits();
                                command_saved_virtual(thread, table, 0xC4, &[value as u32, (value >> 32) as u32]);
                            }
                            _ => {
                                vcall!(value, 0xDC => fn(*mut u8), string);
                                let table = read::<*mut u8>(thread, 0);
                                let value = command_string_chars(string);
                                command_saved_virtual(thread, table, 0xCC, &[value as u32]);
                            }
                        }
                    }
                    count = count.wrapping_add(1);
                }
                result = command_iterator_call(generator.output(), 0x38, &[key], key);
                key = result as u32;
            }
            let temporary = CommandString::from_ptr(string);
            let buffer = temporary.buffer();
            temporary.set_vtable(0x00A8_0810);
            if !buffer.is_null() {
                let args = [buffer as u32];
                let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
                call.eax = buffer as u32;
                call.edx = (result >> 32) as u32;
                call_raw(&mut call);
                result = (u64::from(call.edx) << 32) | u64::from(call.eax);
            }
            if count == 0 { return command_failure(thread); }
            (result & 0xFFFF_FFFF_0000_0000) | u64::from(count)
        }
    }
}

layout! {
    struct RasterQueue {
        0x14 => triangles / set_triangles: *mut u8,
        0x20 => pending / set_pending: u32,
        0x24 => head / set_head: u32,
        0x1D8 => filter_enabled / set_filter_enabled: u8,
        0x3EC => pixels / set_pixels: *mut u8,
        0x410 => stride / set_stride: i32,
    }
}
layout! {
    /// Three consecutive 24-byte vertices precede the texture and span-program keys.
    struct QueuedTriangle {
        0x00 => vertices / set_vertices: [[i32; 6]; 3],
        0x48 => texture / set_texture: i32,
        0x4D => texture_kind / set_texture_kind: u8,
        0x4E => cull_back / set_cull_back: u8,
        0x4F => options / set_options: u8,
        0x50 => key_low / set_key_low: u32,
        0x54 => key_high / set_key_high: u32,
    }
}
game_fn! { fn raster_texture_size(this: *mut u8, texture: i32, width: *mut u32, height: *mut u32) = "thiscall" 0x008D_E900; }
game_fn! { fn raster_texture_level(this: *mut u8, texture: i32, level: u32) -> *mut u8 = "thiscall" 0x008D_F820; }
game_fn! { fn raster_span_lookup(this: *mut u8, low: u32, high: u32, payload: *mut u32) -> *mut u8 = "thiscall" 0x008E_BC60; }
game_fn! { fn raster_code_allocate(bytes: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn raster_code_free(code: *mut u8) = "C" 0x005E_5620; }
game_fn! { fn raster_code_generate(code: *mut u8, low: u32, high: u32, size: *mut u32) -> u32 = "C" 0x008E_FFF0; }
game_fn! { fn raster_span_insert(this: *mut u8, low: u32, high: u32, payload: u32) -> *mut u8 = "thiscall" 0x008E_BE30; }
game_fn! { fn raster_multiply(a: u64, b: u64) -> u64 = "stdcall" 0x009F_0D10; }
game_fn! { #[allow(dead_code)] fn raster_triangle(a: *mut u8, b: *mut u8, c: *mut u8, flags: u32) = "C" 0x008D_D8B0; }
game_data! { static raster_texture_mask: u32 = 0x00B5_C580; }
game_data! { static raster_texture_offset: u32 = 0x00B5_C584; }
game_data! { static raster_texture_bias: u64 = 0x00B5_C588; }
game_data! { static raster_texture: *mut u8 = 0x00B5_C5AC; }
game_data! { static raster_span: *mut u8 = 0x00B5_C5B8; }
game_data! { static raster_texture_kind: u32 = 0x00B5_C5BC; }
game_data! { static raster_default_texture: u8 = 0x00AC_C740; }

unsafe fn submit_raster_triangle(vertices: [*mut u8; 3], flags: u32, ecx: u32, edx: u32) {
    // Triangle setup preserves these two registers, which remain visible after draining the queue.
    let args = [vertices[0] as u32, vertices[1] as u32, vertices[2] as u32, flags];
    let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x008D_D8B0), ecx, &args);
    call.eax = flags;
    call.edx = edx;
    unsafe { call_raw(&mut call); }
}

port! {
    /// Drain queued triangles, choose texture levels, cache span programs, and clear covered pixels.
    /// Preserve EDX on an empty queue and return the final span inputs in ECX and EDX.
    #[port(address = 0x008E_0100, owner = "shared", name = "DrainRasterTriangles", naked)]
    pub unsafe extern "C" fn fc4p_008e0100() {
        core::arch::naked_asm!(
            "mov eax, [esp - 0x30]", "push ecx", "push esp", "push eax", "call {body}", "pop ecx", "ret",
            body = sym drain_raster_triangles);
    }
}

/// The seed retains the previous payload slot if both lookup and allocation fail to produce a payload.
unsafe extern "fastcall" fn drain_raster_triangles(this: *mut u8, entry_edx: u32, mut payload: u32, result_ecx: *mut u32) -> u64 {
    unsafe {
        let queue = RasterQueue::from_ptr(this);
        let mut ecx = this as u32;
        let mut edx = entry_edx;
        if queue.pending() == 0 {
            result_ecx.write(ecx);
            return u64::from(edx) << 32;
        }
        while queue.pending() != 0 {
            let triangle = queue.triangles().wrapping_add((queue.head() & 1023) as usize * 88);
            queue.set_head(queue.head().wrapping_add(1));
            let queued = QueuedTriangle::from_ptr(triangle);
            let texture = queued.texture();
            let options = queued.options();
            let mut low = queued.key_low() & !0x10_0000;
            let mut high = queued.key_high() & !0x1000;
            let colors = [read::<u32>(triangle, 0x0C), read::<u32>(triangle, 0x24), read::<u32>(triangle, 0x3C)];
            if colors[0] & colors[1] & colors[2] == u32::MAX && low & 0xE_0000 != 0x8_0000 {
                low &= !0x8000;
            } else {
                low &= !0x1_0000;
                if colors[0] != colors[1] || colors[1] != colors[2] { low |= 0x1_0000; }
            }
            let mut clockwise = true;
            if texture >= 0 {
                let mut width = 0u32;
                let mut height = 0u32;
                raster_texture_size(this.wrapping_add(0x35C), texture, &mut width, &mut height);
                let mut width_mask = width.wrapping_sub(1);
                let mut height_mask = height.wrapping_sub(1);
                let x0 = read::<i32>(triangle, 0);
                let y0 = read::<i32>(triangle, 4);
                let area = read::<i32>(triangle, 0x34).wrapping_sub(y0)
                    .wrapping_mul(read::<i32>(triangle, 0x18).wrapping_sub(x0))
                    .wrapping_sub(read::<i32>(triangle, 0x1C).wrapping_sub(y0)
                        .wrapping_mul(read::<i32>(triangle, 0x30).wrapping_sub(x0)));
                clockwise = area < 0;
                let u0 = read::<i32>(triangle, 0x10);
                let v0 = read::<i32>(triangle, 0x14);
                let uv_area = read::<i32>(triangle, 0x44).wrapping_sub(v0)
                    .wrapping_mul(read::<i32>(triangle, 0x28).wrapping_sub(u0))
                    .wrapping_sub(read::<i32>(triangle, 0x40).wrapping_sub(u0)
                        .wrapping_mul(read::<i32>(triangle, 0x2C).wrapping_sub(v0)));
                let uv_area = uv_area.wrapping_abs();
                let mut area = area.wrapping_abs();
                if options & 0x10 != 0 { low |= 0x10_0000; }
                if queue.filter_enabled() != 0 && options & (if uv_area < area { 4 } else { 2 }) != 0 {
                    high |= 0x1000;
                }
                let mut level = 0u32;
                let mut data;
                if options & 8 != 0 {
                    area = area.wrapping_mul(2);
                    while area < uv_area && level < 8 {
                        area = area.wrapping_mul(4);
                        level += 1;
                    }
                    data = raster_texture_level(this.wrapping_add(0x35C), texture, level);
                    while data.is_null() && level > 0 {
                        level -= 1;
                        data = raster_texture_level(this.wrapping_add(0x35C), texture, level);
                    }
                    // Each V coordinate is stored before its U coordinate.
                    for offset in [0x14, 0x10, 0x2C, 0x28, 0x44, 0x40] {
                        write(triangle, offset, read::<i32>(triangle, offset) >> level);
                    }
                    width_mask >>= level;
                    height_mask >>= level;
                } else {
                    data = raster_texture_level(this.wrapping_add(0x35C), texture, 0);
                }
                if high & 0x1000 != 0 {
                    for offset in [0x10, 0x14, 0x28, 0x2C, 0x40, 0x44] {
                        write(triangle, offset, read::<i32>(triangle, offset).wrapping_sub(8));
                    }
                }
                if data.is_null() {
                    width_mask = 0;
                    height_mask = 0;
                    high &= !0x1000;
                    data = raster_default_texture();
                }
                *raster_texture() = data;
                *raster_texture_mask() = (height_mask << 8) | width_mask;
                *raster_texture_kind() = u32::from(queued.texture_kind());
                *raster_texture_offset() = 0;
                let product = raster_multiply(u64::from(height_mask), 0xFFF0_0000_0000_0000);
                *raster_texture_bias() = product.wrapping_sub(u64::from(width_mask.wrapping_shl(20)))
                    .wrapping_add(0x7FFF_7FFF_7FFF_7FFF);
            }
            let mut code = raster_span_lookup(this.wrapping_add(0x38), low, high, &mut payload);
            if code.is_null() {
                let scratch = raster_code_allocate(0x1000);
                if !scratch.is_null() {
                    let mut size = 0;
                    payload = raster_code_generate(scratch, low, high, &mut size);
                    code = raster_span_insert(this.wrapping_add(0x38), low, high, payload);
                    // The program is copied in words followed by its final bytes, with forward overlap behavior.
                    core::arch::asm!(
                        "push esi", "mov esi, {source:e}", "rep movsd", "mov ecx, edx",
                        "and ecx, 3", "rep movsb", "pop esi",
                        source = in(reg) scratch, inout("edi") code => _,
                        inout("ecx") (size >> 2) => _, in("edx") size);
                    raster_code_free(scratch);
                }
            }
            *raster_span() = code;
            let x = [read::<i32>(triangle, 0), read::<i32>(triangle, 0x18), read::<i32>(triangle, 0x30)];
            let y = [read::<i32>(triangle, 4), read::<i32>(triangle, 0x1C), read::<i32>(triangle, 0x34)];
            let first_x = x[0].min(x[1].min(x[2])) >> 8;
            let last_x = x[0].max(x[1].max(x[2])).wrapping_sub(1) >> 8;
            let first_y = y[0].min(y[1].min(y[2])) >> 8;
            let last_y = y[0].max(y[1].max(y[2])).wrapping_sub(1) >> 8;
            ecx = last_x as u32;
            edx = first_y as u32;
            let mut row = first_y;
            while row <= last_y {
                let mut column = first_x;
                while column <= last_x {
                    let index = queue.stride().wrapping_mul(row).wrapping_add(column) as u32;
                    write(queue.pixels(), index.wrapping_mul(4) as usize, 0u32);
                    column = column.wrapping_add(1);
                }
                row = row.wrapping_add(1);
                edx = row as u32;
            }
            if clockwise {
                submit_raster_triangle([triangle.wrapping_add(48), triangle.wrapping_add(24), triangle], payload, ecx, edx);
                if texture < 0 && queued.cull_back() == 0 {
                    ecx = payload;
                    submit_raster_triangle([triangle, triangle.wrapping_add(24), triangle.wrapping_add(48)], payload, ecx, edx);
                }
            } else if queued.cull_back() == 0 {
                edx = payload;
                submit_raster_triangle([triangle, triangle.wrapping_add(24), triangle.wrapping_add(48)], payload, ecx, edx);
            }
            queue.set_pending(queue.pending().wrapping_sub(1));
        }
        core::arch::asm!("emms", options(nostack));
        result_ecx.write(ecx);
        u64::from(edx) << 32
    }
}


// Triangle setup shares this block with the edge initializers and generated spans.
game_data! { static triangle_state: [u8; 0x5E0] = 0x00B5_C4A8; }
game_data! { static triangle_edge_order: [u8; 0x40] = 0x00AC_C67C; }
game_fn! { #[allow(dead_code)] fn triangle_color_edge() = "C" 0x008D_D760; }
game_fn! { #[allow(dead_code)] fn triangle_texture_edge() = "C" 0x008D_D680; }
game_fn! { #[allow(dead_code)] fn triangle_depth_edge() = "C" 0x008D_D620; }

layout! {
    /// A triangle vertex keeps screen coordinates, depth, packed color, and two texture coordinates.
    struct TriangleVertex {
        0x00 => x / set_x: i32,
        0x04 => y / set_y: i32,
        0x08 => depth / set_depth: i32,
        0x0C => color / set_color: u32,
        0x10 => texture_u / set_texture_u: i32,
        0x14 => texture_v / set_texture_v: i32,
    }
}

/// Packed integer lanes. All products and sums wrap at their destination width.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct TriangleMmx(u64);
impl TriangleMmx {
    fn pair(a: u32, b: u32) -> Self {
        Self(u64::from(a) | (u64::from(b) << 32))
    }
    fn d(self, i: usize) -> u32 {
        (self.0 >> (i * 32)) as u32
    }
    fn w(self, i: usize) -> u16 {
        (self.0 >> (i * 16)) as u16
    }
    fn map_d(self, f: impl Fn(u32) -> u32) -> Self {
        Self::pair(f(self.d(0)), f(self.d(1)))
    }
    fn add(self, b: Self) -> Self {
        Self::pair(self.d(0).wrapping_add(b.d(0)), self.d(1).wrapping_add(b.d(1)))
    }
    fn sub(self, b: Self) -> Self {
        Self::pair(self.d(0).wrapping_sub(b.d(0)), self.d(1).wrapping_sub(b.d(1)))
    }
    fn shl(self, n: u32) -> Self {
        self.map_d(|v| v << n)
    }
    fn sar(self, n: u32) -> Self {
        self.map_d(|v| ((v as i32) >> n) as u32)
    }
    fn half_words(self) -> Self {
        Self((self.0 >> 1) & 0x7FFF_7FFF_7FFF_7FFF)
    }
    fn madd(self, b: Self) -> Self {
        let product = |i| i32::from(self.w(i) as i16).wrapping_mul(i32::from(b.w(i) as i16));
        Self::pair(
            product(0).wrapping_add(product(1)) as u32,
            product(2).wrapping_add(product(3)) as u32,
        )
    }
    fn low_d(self) -> Self {
        Self::pair(self.d(0), self.d(0))
    }
    fn high_d(self) -> Self {
        Self::pair(self.d(1), self.d(1))
    }
    fn words(a: i32, b: i32) -> Self {
        Self::pair(u32::from(a as u16), u32::from(b as u16))
    }
    fn saturated(self, b: Self) -> Self {
        let values = [self.d(0), self.d(1), b.d(0), b.d(1)];
        Self(values.into_iter().enumerate().fold(0, |v, (i, n)| {
            v | (u64::from((n as i32).clamp(-32768, 32767) as u16) << (i * 16))
        }))
    }
}

/// The first eight words use PUSHAD order; the bridge also carries every live MMX register.
#[repr(C)]
#[derive(Default)]
struct TriangleRegisters {
    edi: u32,
    esi: u32,
    ebp: u32,
    esp: u32,
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,
    target: u32,
    flags: u32,
    mm: [TriangleMmx; 8],
}

const _: () = {
    assert!(core::mem::offset_of!(TriangleRegisters, mm) == 40);
    assert!(core::mem::size_of::<TriangleRegisters>() == 104);
};

/// Invoke a register-based edge initializer or span, retaining its complete register results.
/// The texture initializer also reads the flags eleven words above its return address.
#[unsafe(naked)]
unsafe extern "thiscall" fn triangle_call(_: *mut TriangleRegisters) {
    core::arch::naked_asm!(
        "pushad",
        "push ecx",
        "push dword ptr [ecx + 32]",
        "sub esp, 44",
        "mov eax, [ecx + 36]",
        "mov [esp + 40], eax",
        "movq mm0, [ecx + 40]",
        "movq mm1, [ecx + 48]",
        "movq mm2, [ecx + 56]",
        "movq mm3, [ecx + 64]",
        "movq mm4, [ecx + 72]",
        "movq mm5, [ecx + 80]",
        "movq mm6, [ecx + 88]",
        "movq mm7, [ecx + 96]",
        "mov edi, [ecx]",
        "mov esi, [ecx + 4]",
        "mov ebp, [ecx + 8]",
        "mov ebx, [ecx + 16]",
        "mov edx, [ecx + 20]",
        "mov eax, [ecx + 28]",
        "mov ecx, [ecx + 24]",
        "call dword ptr [esp + 44]",
        "pushad",
        "mov ecx, [esp + 80]",
        "mov eax, [esp]",
        "mov [ecx], eax",
        "mov eax, [esp + 4]",
        "mov [ecx + 4], eax",
        "mov eax, [esp + 8]",
        "mov [ecx + 8], eax",
        "mov eax, [esp + 16]",
        "mov [ecx + 16], eax",
        "mov eax, [esp + 20]",
        "mov [ecx + 20], eax",
        "mov eax, [esp + 24]",
        "mov [ecx + 24], eax",
        "mov eax, [esp + 28]",
        "mov [ecx + 28], eax",
        "movq [ecx + 40], mm0",
        "movq [ecx + 48], mm1",
        "movq [ecx + 56], mm2",
        "movq [ecx + 64], mm3",
        "movq [ecx + 72], mm4",
        "movq [ecx + 80], mm5",
        "movq [ecx + 88], mm6",
        "movq [ecx + 96], mm7",
        "popad",
        "add esp, 52",
        "popad",
        "ret"
    )
}

impl TriangleRegisters {
    unsafe fn call(&mut self, target: usize) {
        self.target = target as u32;
        unsafe { triangle_call(self) };
    }
    unsafe fn restore_mmx(&self, preserve_mm2: bool) {
        unsafe {
            core::arch::asm!(
                "movq mm0, [{p}]", "movq mm1, [{p} + 8]", "movq mm3, [{p} + 24]",
                "movq mm4, [{p} + 32]", "movq mm5, [{p} + 40]", "movq mm6, [{p} + 48]", "movq mm7, [{p} + 56]",
                p = in(reg) self.mm.as_ptr(), out("mm0") _, out("mm1") _, out("mm3") _,
                out("mm4") _, out("mm5") _, out("mm6") _, out("mm7") _, options(nostack, readonly));
            if !preserve_mm2 {
                core::arch::asm!("movq mm2, [{p}]", p = in(reg) &self.mm[2], out("mm2") _, options(nostack, readonly));
            }
        }
    }
}

unsafe fn triangle_load<T: Copy>(address: u32) -> T {
    unsafe { read(triangle_state().cast(), address.wrapping_sub(0x00B5_C4A8) as usize) }
}
unsafe fn triangle_store<T>(address: u32, value: T) {
    unsafe {
        write(
            triangle_state().cast(),
            address.wrapping_sub(0x00B5_C4A8) as usize,
            value,
        )
    }
}
fn triangle_pointer(address: u32) -> u32 {
    triangle_state()
        .cast::<u8>()
        .wrapping_add(address.wrapping_sub(0x00B5_C4A8) as usize) as u32
}

/// Signed division deliberately retains the processor's divide-error behavior on overflow.
fn triangle_divide(numerator: i32, denominator: i32) -> i32 {
    let quotient: i32;
    unsafe {
        core::arch::asm!("cdq", "idiv {divisor:e}", divisor = in(reg) denominator,
            inlateout("eax") numerator => quotient, out("edx") _, options(nomem, nostack));
    }
    quotient
}

/// Compute horizontal gradients for two color channels, retaining the packed intermediates.
fn triangle_color_pair(mm: &mut [TriangleMmx; 8], b: TriangleMmx, a: TriangleMmx) -> TriangleMmx {
    mm[2] = b;
    mm[4] = a;
    mm[3] = mm[2];
    mm[0] = mm[0].madd(mm[2]);
    mm[3] = mm[3].shl(16).madd(mm[1]);
    mm[6] = mm[6].half_words().madd(mm[4]);
    mm[5] = mm[4].shl(16).madd(mm[7]);
    mm[3] = mm[3].shl(15).add(mm[0]);
    mm[5] = mm[5].shl(15);
    mm[6] = mm[6].add(mm[5]);
    // The caller reloads the two Y coefficients before the second plane.
    mm[3] = mm[3].sub(mm[6]);
    mm[3]
}

port! {
    /// Set up fixed-point triangle edges and attribute gradients, then draw the two scanline ranges.
    /// All general registers except EAX are preserved; the stack arguments remain caller-owned.
    #[port(address = 0x008D_D8B0, owner = "shared", name = "RasterizeTriangle", naked)]
    pub unsafe extern "C" fn fc4p_008dd8b0() {
        core::arch::naked_asm!(
            "pushad", "mov ecx, esp", "lea edx, [esp + 36]", "call {body}",
            "mov [esp + 28], eax", "popad", "ret", body = sym rasterize_triangle)
    }
}

unsafe extern "fastcall" fn rasterize_triangle(entry: *const u32, arguments: *const u32) -> u32 {
    unsafe {
        let vertices: [*mut u8; 3] = core::array::from_fn(|i| arguments.add(i).read() as *mut u8);
        // Color, texture and depth planes use bits 0..2; bit 3 selects the texture scale.
        let flags = arguments.add(3).read();
        let vertex = |p| TriangleVertex::from_ptr(p);
        let xy = |p| TriangleMmx::pair(vertex(p).x() as u32, vertex(p).y() as u32);
        let mut r = TriangleRegisters {
            flags,
            ecx: entry.add(6).read(),
            ..TriangleRegisters::default()
        };
        let m = &mut r.mm;
        m[7] = xy(vertices[2]);
        m[3] = xy(vertices[1]);
        m[1] = m[7];
        m[4] = xy(vertices[0]);
        let first_y = m[4].d(1) as i32;
        m[7] = m[7].sub(m[3]);
        m[1] = m[1].sub(m[4]);
        m[5] = m[7];
        m[6] = m[1];
        m[7] = m[7].saturated(m[1]);
        m[1] = TriangleMmx(m[1].0 << 32);
        m[6] = TriangleMmx::pair(m[6].d(1), m[1].d(1)).shl(16);
        m[1] = TriangleMmx(0);
        m[5] = m[5].shl(16);
        m[4] = m[7];
        m[5] = m[5].madd(m[6]);
        m[7] = TriangleMmx::words(i32::from(m[7].w(0)), i32::from(m[7].w(1)));
        m[4] = TriangleMmx::words(i32::from(m[4].w(2)), i32::from(m[4].w(3)));
        m[3] = m[7];
        m[6] = m[5];
        m[5] = TriangleMmx(m[5].0 >> 32).sub(m[6]);
        // The area uses signed low words, while the later coefficients use saturated words.
        let area = m[5].d(0) as i32;
        if area < 0 {
            triangle_store(0x00B5_C5C0, 0u32);
            // MM2 still carries its entry value, including its upper x87 bits.
            r.restore_mmx(true);
            return 0;
        }
        let reciprocal = if area > 256 {
            let quotient: u32;
            core::arch::asm!("div {divisor:e}", divisor = in(reg) area as u32,
                inlateout("eax") 0u32 => quotient, inlateout("edx") 128u32 => _, options(nomem, nostack));
            quotient
        } else {
            0
        };
        m[0] = TriangleMmx::pair(reciprocal, reciprocal);
        triangle_store(0x00B5_C5C0, area);
        m[6] = m[0];
        m[0] = m[0].half_words();
        m[7] = m[7].madd(m[0]);
        m[3] = m[3].shl(16).madd(m[6]);
        m[0] = m[0].madd(m[4]);
        m[4] = m[4].shl(16).madd(m[6]);
        m[7] = m[7].sar(14);
        m[3] = m[3].add(m[3]);
        m[0] = m[0].sar(14);
        m[4] = m[4].add(m[4]);
        m[7] = m[7].add(m[3]);
        m[0] = m[0].add(m[4]);
        m[5] = m[0];
        let ys = [
            first_y.wrapping_sub(8) >> 4,
            vertex(vertices[1]).y().wrapping_sub(8) >> 4,
            vertex(vertices[2]).y().wrapping_sub(8) >> 4,
        ];
        triangle_store(0x00B5_CA60, ys[1]);
        triangle_store(0x00B5_CA5C, ys[0]);
        triangle_store(0x00B5_CA64, ys[2]);
        let (mut low, mut high, mut mid) = (1usize, 0usize, 2usize);
        let (mut bottom, mut top) = (ys[0], ys[1]);
        if bottom < top {
            core::mem::swap(&mut bottom, &mut top);
            high = 1;
            low = 0;
        }
        if bottom <= ys[2] {
            bottom = ys[2];
            mid = high;
            high = 2;
        }
        if top >= ys[2] {
            top = ys[2];
            mid = low;
            low = 2;
        }
        let permutation = (((u32::from(mid < high) * 2) + u32::from(low < mid)) * 2) + u32::from(low < high);
        let middle = triangle_load::<i32>(0x00B5_CA5C + mid as u32 * 4);
        m[1] = TriangleMmx(u64::from(bottom as u32));
        m[6] = m[7];
        triangle_store(0x00B5_CA38, bottom.wrapping_sub(middle));
        triangle_store(0x00B5_CA34, middle.wrapping_sub(top));
        r.esi = vertices[high] as u32;
        r.edi = vertices[low] as u32;
        r.ecx = vertices[mid] as u32;
        let upper_fraction = vertex(r.esi as *mut u8)
            .y()
            .wrapping_sub(8)
            .wrapping_sub(bottom.wrapping_shl(4))
            .wrapping_shl(11);
        let lower_fraction = vertex(r.ecx as *mut u8)
            .y()
            .wrapping_sub(8)
            .wrapping_sub(middle.wrapping_shl(4))
            .wrapping_shl(11);
        triangle_store(0x00B5_C4EC, upper_fraction);
        triangle_store(0x00B5_C57C, upper_fraction);
        triangle_store(0x00B5_C534, lower_fraction);
        r.ebx = read(triangle_edge_order().cast(), 0x20 + permutation as usize * 4);
        m[1] = m[1].low_d().madd(TriangleMmx(triangle_load(0x00B5_C5A0)));
        m[5] = m[5].low_d();
        m[3] = TriangleMmx(triangle_load(0x00B5_C598));
        m[0] = m[0].high_d();
        m[2] = TriangleMmx(read(triangle_edge_order().cast(), r.ebx as usize));
        m[6] = m[6].high_d();
        triangle_store(0x00B5_CA44, m[2].0);
        m[7] = m[7].low_d();
        m[2] = TriangleMmx(read(triangle_edge_order().cast(), r.ebx as usize + 8));
        m[1] = m[1].add(m[3]);
        triangle_store(0x00B5_CA3C, m[2].0);
        for (start, end, edge, fraction) in [
            (r.esi, r.ecx, 0x00B5_C4A8, 0x00B5_C4EC),
            (r.ecx, r.edi, 0x00B5_C4F0, 0x00B5_C534),
            (r.esi, r.edi, 0x00B5_C538, 0x00B5_C57C),
        ] {
            let start = start as *mut u8;
            let end = end as *mut u8;
            let end_x = vertex(end).x();
            let start_x = vertex(start).x();
            let numerator = end_x.wrapping_sub(start_x).wrapping_shl(15);
            let denominator = vertex(start).y().wrapping_sub(vertex(end).y());
            let slope = if denominator == 0 {
                numerator
            } else {
                triangle_divide(numerator, denominator)
            };
            triangle_store(edge + 4, slope);
            let product = i64::from(slope).wrapping_mul(i64::from(triangle_load::<i32>(fraction)));
            r.eax = (product as u32) >> 15;
            r.edx = ((product >> 15) as u64) as u32;
            let x = (start_x as u32)
                .wrapping_shl(11)
                .wrapping_add(r.edx)
                .wrapping_sub(0x4000);
            triangle_store(edge, x);
            triangle_store(edge + 0x40, r.edx);
        }
        r.edi = vertices[0] as u32;
        r.edx = vertices[1] as u32;
        r.eax = vertices[2] as u32;
        triangle_store(0x00B5_C9E4, m[1].d(0));
        m[1] = TriangleMmx(m[1].0 >> 32);
        triangle_store(0x00B5_C9EC, m[1].d(0));
        m[3] = TriangleMmx(0);
        let colors: [u32; 3] = core::array::from_fn(|i| vertex(vertices[i]).color());
        m[1] = m[0];
        triangle_store(0x00B5_CA78, m[5].0);
        triangle_store(0x00B5_CA70, m[6].0);
        triangle_store(0x00B5_CA80, m[7].0);
        triangle_store(0x00B5_CA68, m[1].0);
        // Byte differences are signed words, with zero words between them after unpacking.
        let packed_diff = |v: u32| {
            (0..4).fold(0u64, |value, i| {
                value
                    | (u64::from((((v >> (i * 8)) & 255) as u16).wrapping_sub(((colors[2] >> (i * 8)) & 255) as u16))
                        << (i * 16))
            })
        };
        m[2] = TriangleMmx(packed_diff(colors[1]));
        m[4] = TriangleMmx(packed_diff(colors[0]));
        m[5] = TriangleMmx((0..4).fold(0u64, |v, i| v | (u64::from((colors[2] >> (i * 8)) as u8) << (i * 16))));
        m[0] = m[0].half_words();
        if flags & 1 != 0 {
            for shift in [0u32, 16] {
                if shift != 0 {
                    m[0] = TriangleMmx(triangle_load(0x00B5_CA68));
                    m[6] = TriangleMmx(triangle_load(0x00B5_CA70));
                    m[1] = m[0];
                    m[0] = m[0].half_words();
                    m[7] = m[6];
                } else {
                    m[7] = m[6];
                }
                let pair_colors: [u32; 3] = if shift == 0 {
                    colors
                } else {
                    core::array::from_fn(|i| vertex(vertices[i]).color())
                };
                let diff = |v: u32| {
                    TriangleMmx::words(
                        ((v >> shift) & 255) as i32 - ((pair_colors[2] >> shift) & 255) as i32,
                        ((v >> (shift + 8)) & 255) as i32 - ((pair_colors[2] >> (shift + 8)) & 255) as i32,
                    )
                };
                let b = diff(pair_colors[1]);
                let a = diff(pair_colors[0]);
                let x = triangle_color_pair(m, b, a);
                triangle_store(0x00B5_CA08 + shift / 2, x.0);
                m[0] = TriangleMmx(triangle_load(0x00B5_CA78));
                m[6] = TriangleMmx(triangle_load(0x00B5_CA80));
                m[1] = m[0];
                m[0] = m[0].half_words();
                m[7] = m[6];
                m[0] = m[0].madd(m[2]);
                m[2] = m[2].shl(16);
                m[1] = m[1].madd(m[2]);
                m[6] = m[6].half_words().madd(m[4]);
                m[4] = m[4].shl(16);
                m[7] = m[7].madd(m[4]);
                m[1] = m[1].shl(15);
                let y = if shift == 0 {
                    m[1] = m[1].add(m[0]);
                    m[7] = m[7].shl(15).add(m[6]);
                    m[7] = m[7].sub(m[1]);
                    m[7]
                } else {
                    m[0] = m[0].add(m[1]);
                    m[7] = m[7].shl(15);
                    m[6] = m[6].add(m[7]).sub(m[0]);
                    m[6]
                };
                triangle_store(0x00B5_CA18 + shift / 2, y.0);
                if shift == 0 {
                    m[0] = TriangleMmx(triangle_load(0x00B5_CA68));
                    m[6] = TriangleMmx(triangle_load(0x00B5_CA70));
                }
            }
        }
        if flags & 2 != 0 {
            for i in 0..2 {
                let texture = |p| {
                    if i == 0 {
                        vertex(p).texture_u()
                    } else {
                        vertex(p).texture_v()
                    }
                };
                let c = texture(vertices[2]);
                let b = texture(vertices[1]).wrapping_sub(c);
                let a = texture(vertices[0]).wrapping_sub(c);
                let gx = triangle_load::<i32>(0x00B5_CA68)
                    .wrapping_mul(b)
                    .wrapping_sub(triangle_load::<i32>(0x00B5_CA70).wrapping_mul(a));
                let gy = triangle_load::<i32>(0x00B5_CA80)
                    .wrapping_mul(a)
                    .wrapping_sub(triangle_load::<i32>(0x00B5_CA78).wrapping_mul(b));
                let (gx, gy) = if flags & 8 != 0 { (gx >> 4, gy >> 4) } else { (gx, gy) };
                triangle_store(0x00B5_C9F0 + i as u32 * 4, gx);
                triangle_store(0x00B5_C9F8 + i as u32 * 4, gy);
                triangle_store(0x00B5_CA28 + i as u32 * 4, (gx >> 17) & 0xFFFF);
            }
        }
        if flags & 4 != 0 {
            let c = vertex(vertices[2]).depth();
            let a = c.wrapping_sub(vertex(vertices[0]).depth());
            let b = c.wrapping_sub(vertex(vertices[1]).depth());
            let gx = triangle_load::<i32>(0x00B5_CA70)
                .wrapping_mul(a)
                .wrapping_sub(triangle_load::<i32>(0x00B5_CA68).wrapping_mul(b))
                .wrapping_add(8)
                >> 4;
            let gy = triangle_load::<i32>(0x00B5_CA78)
                .wrapping_mul(b)
                .wrapping_sub(triangle_load::<i32>(0x00B5_CA80).wrapping_mul(a))
                .wrapping_add(8)
                >> 4;
            triangle_store(0x00B5_CA00, gx);
            triangle_store(0x00B5_CA04, gy);
            r.eax = gy as u32;
            r.edx = triangle_load::<i32>(0x00B5_CA80).wrapping_mul(a) as u32;
        }
        if r.ebx == 0 {
            r.ebp = triangle_pointer(0x00B5_C538);
            if flags & 1 != 0 {
                r.call(crate::abi::target(0x008D_D760));
            }
            if flags & 2 != 0 {
                r.ebx = triangle_load(0x00B5_C57C);
                r.call(crate::abi::target(0x008D_D680));
            }
            if flags & 4 != 0 {
                r.ebx = triangle_pointer(0x00B5_C538);
                r.call(crate::abi::target(0x008D_D620));
            }
        } else {
            for (bit, target) in [(1, 0x008D_D760), (2, 0x008D_D680), (4, 0x008D_D620)] {
                if flags & bit == 0 {
                    continue;
                }
                core::mem::swap(&mut r.esi, &mut r.ecx);
                r.ebp = triangle_pointer(0x00B5_C4F0);
                if bit == 2 {
                    r.ebx = triangle_load(0x00B5_C534);
                }
                r.call(crate::abi::target(target));
                core::mem::swap(&mut r.esi, &mut r.ecx);
                r.ebp = triangle_pointer(0x00B5_C4A8);
                if bit == 2 {
                    r.ebx = triangle_load(0x00B5_C4EC);
                }
                r.call(crate::abi::target(target));
            }
        }
        if flags & 4 != 0 {
            r.eax = ((triangle_load::<i32>(0x00B5_CA00) >> 31) & 255) as u32;
            r.esi = (triangle_load::<i32>(0x00B5_CA00).wrapping_add(r.eax as i32) >> 8) as u32;
            triangle_store(0x00B5_CA30, r.esi);
            r.esi = triangle_load::<u32>(0x00B5_CA00).rotate_left(16);
            triangle_store(0x00B5_CA00, r.esi);
        }
        r.mm[0] = TriangleMmx(triangle_load(0x00B5_C9F0));
        r.mm[4] = TriangleMmx(0);
        r.ebp = triangle_load(0x00B5_C9E4);
        r.edi = triangle_load(0x00B5_CA38);
        if r.edi as i32 > 0 {
            r.ebx = triangle_load(0x00B5_CA3C);
            r.edx = triangle_load(0x00B5_CA40);
            r.call(*raster_span() as usize);
        }
        r.edi = triangle_load(0x00B5_CA34);
        if r.edi as i32 > 0 {
            r.ebx = triangle_load(0x00B5_CA44);
            r.edx = triangle_load(0x00B5_CA48);
            triangle_store(0x00B5_CA3C, r.ebx);
            triangle_store(0x00B5_CA40, r.edx);
            r.call(*raster_span() as usize);
        }
        r.restore_mmx(false);
        r.eax
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_0089801e::PORT,
    fc4p_008ae800::PORT,
    fc4p_008b03d0::PORT,
    fc4p_008b15e0::PORT,
    fc4p_008b5b70::PORT,
    fc4p_008b7f40::PORT,
    fc4p_008b87a0::PORT,
    fc4p_008d0fd0::PORT,
    fc4p_008d1030::PORT,
    fc4p_008d29b9::PORT,
    fc4p_008dd620::PORT,
    fc4p_008dd680::PORT,
    fc4p_008dd760::PORT,
    fc4p_008dd8b0::PORT,
    fc4p_008e0100::PORT,
    fc4p_008dc4c0::PORT,
    fc4p_008eb800::PORT,
    fc4p_008eb8d0::PORT,
    fc4p_008eb920::PORT,
    fc4p_008ebc60::PORT,
    fc4p_008ebb10::PORT,
    fc4p_008ebb70::PORT,
    fc4p_008ebe30::PORT,
    fc4p_008ec150::PORT,
    fc4p_008ec190::PORT,
    fc4p_008ec1e0::PORT,
    fc4p_008ec270::PORT,
    fc4p_008ec340::PORT,
    fc4p_008ec440::PORT,
];

#[cfg(test)]
mod raster_queue_tests {
    use super::*;
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Event {
        Size(i32), Level(i32, u32), Lookup(u32, u32), Allocate(u32), Generate(u32, u32),
        Insert(u32, u32, u32), Free, Multiply(u64, u64), Triangle([u32; 3], u32, u32, u32),
    }
    #[derive(Default)]
    struct State {
        this: u32, width: u32, height: u32, missing: [bool; 9], cache_miss: bool, allocation_fails: bool,
        scratch: u32, destination: u32, size: u32, events: Vec<Event>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn event(e: Event) { STATE.with(|s| s.borrow_mut().events.push(e)); }
    fn events() -> Vec<Event> { STATE.with(|s| core::mem::take(&mut s.borrow_mut().events)) }
    extern "thiscall" fn size(this: *mut u8, texture: i32, width: *mut u32, height: *mut u32) {
        STATE.with(|s| {
            let s = s.borrow(); assert_eq!(this as u32, s.this + 0x35C);
            unsafe { width.write(s.width); height.write(s.height); }
        }); event(Event::Size(texture));
    }
    extern "thiscall" fn level(this: *mut u8, texture: i32, level: u32) -> *mut u8 {
        event(Event::Level(texture, level));
        STATE.with(|s| {
            let s = s.borrow(); assert_eq!(this as u32, s.this + 0x35C);
            if s.missing[level as usize] { core::ptr::null_mut() } else { 0x3141_5926 as *mut u8 }
        })
    }
    extern "thiscall" fn lookup(this: *mut u8, low: u32, high: u32, payload: *mut u32) -> *mut u8 {
        event(Event::Lookup(low, high));
        STATE.with(|s| {
            let s = s.borrow(); assert_eq!(this as u32, s.this + 0x38);
            if s.cache_miss { core::ptr::null_mut() }
            else { unsafe { payload.write(0x44); } s.destination as *mut u8 }
        })
    }
    extern "C" fn allocate_code(bytes: u32) -> *mut u8 {
        event(Event::Allocate(bytes));
        STATE.with(|s| if s.borrow().allocation_fails { core::ptr::null_mut() } else { s.borrow().scratch as *mut u8 })
    }
    extern "C" fn generate(code: *mut u8, low: u32, high: u32, size: *mut u32) -> u32 {
        event(Event::Generate(low, high));
        STATE.with(|s| {
            let s = s.borrow(); assert_eq!(code as u32, s.scratch);
            unsafe { size.write(s.size); }
        }); 0x77
    }
    extern "thiscall" fn insert(this: *mut u8, low: u32, high: u32, payload: u32) -> *mut u8 {
        event(Event::Insert(low, high, payload));
        STATE.with(|s| { let s = s.borrow(); assert_eq!(this as u32, s.this + 0x38); s.destination as *mut u8 })
    }
    extern "C" fn free_code(code: *mut u8) {
        STATE.with(|s| assert_eq!(code as u32, s.borrow().scratch)); event(Event::Free);
    }
    extern "stdcall" fn multiply(a: u64, b: u64) -> u64 { event(Event::Multiply(a, b)); a.wrapping_mul(b) }
    #[unsafe(naked)]
    unsafe extern "C" fn triangle() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push dword ptr [esp + 4]", "push dword ptr [esp + 4]",
            "push dword ptr [esp + 32]", "push dword ptr [esp + 32]",
            "push dword ptr [esp + 32]", "push dword ptr [esp + 32]",
            "call {record}", "add esp, 24", "pop ecx", "pop edx", "ret", record = sym record_triangle);
    }
    extern "C" fn record_triangle(a: u32, b: u32, c: u32, flags: u32, ecx: u32, edx: u32) {
        event(Event::Triangle([a, b, c], flags, ecx, edx));
    }
    #[unsafe(naked)]
    unsafe extern "fastcall" fn invoke(_: *mut u32) {
        core::arch::naked_asm!(
            "push ebx", "push esi", "push edi", "push ebp", "mov edi, ecx",
            "mov ecx, [edi + 4]", "mov edx, [edi + 8]", "call dword ptr [edi]",
            "mov [edi + 12], eax", "mov [edi + 16], ecx", "mov [edi + 20], edx",
            "pushfd", "pop dword ptr [edi + 24]", "pop ebp", "pop edi", "pop esi", "pop ebx", "ret");
    }
    struct Fixture {
        object: Object, triangles: Vec<u32>, pixels: Vec<u32>, globals: Box<[u32; 16]>,
        default_texture: Box<u8>, scratch: Vec<u8>, code: Vec<u8>, _fakes: Vec<crate::testing::Fake>,
    }
    impl Fixture {
        fn new() -> Self {
            let mut object = Object::new(0x430);
            let this = object.ptr();
            let mut triangles = vec![0xA5A5_A5A5u32; 1024 * 22];
            let mut pixels = vec![0xABCD_EFFF; 128];
            let mut globals = Box::new([0xA5A5_A5A5u32; 16]);
            let mut default_texture = Box::new(0xCCu8);
            let mut scratch: Vec<u8> = (0..64).collect();
            let mut code = vec![0xCDu8; 64];
            unsafe {
                let queue = RasterQueue::from_ptr(this);
                queue.set_triangles(triangles.as_mut_ptr().cast()); queue.set_pending(1);
                queue.set_pixels(pixels.as_mut_ptr().add(48).cast()); queue.set_stride(8);
            }
            STATE.with(|s| *s.borrow_mut() = State {
                this: this as u32, width: 16, height: 32, scratch: scratch.as_mut_ptr() as u32,
                destination: code.as_mut_ptr() as u32, size: 7, ..State::default()
            });
            let mut fakes = vec![
                fake(0x008D_E900, size as *const () as usize), fake(0x008D_F820, level as *const () as usize),
                fake(0x008E_BC60, lookup as *const () as usize), fake(0x005E_55E0, allocate_code as *const () as usize),
                fake(0x008E_FFF0, generate as *const () as usize), fake(0x008E_BE30, insert as *const () as usize),
                fake(0x005E_5620, free_code as *const () as usize), fake(0x009F_0D10, multiply as *const () as usize),
                fake(0x008D_D8B0, triangle as *const () as usize), fake_data(0x00AC_C740, default_texture.as_mut()),
            ];
            for (address, offset) in [(0x00B5_C580, 0), (0x00B5_C584, 1), (0x00B5_C588, 2),
                (0x00B5_C5AC, 11), (0x00B5_C5B8, 14), (0x00B5_C5BC, 15)] {
                fakes.push(fake_data(address, unsafe { globals.as_mut_ptr().add(offset) }.cast()));
            }
            let mut fixture = Self { object, triangles, pixels, globals, default_texture, scratch, code, _fakes: fakes };
            fixture.set_triangle(0, -1, 0, false, [u32::MAX; 3], 0, 0);
            fixture
        }
        #[allow(clippy::too_many_arguments)]
        fn set_triangle(&mut self, index: usize, texture: i32, options: u8, cull: bool, colors: [u32; 3], low: u32, high: u32) {
            let record = &mut self.triangles[index * 22..index * 22 + 22];
            for (vertex, (x, y)) in [(0, 0), (0, 512), (512, 0)].into_iter().enumerate() {
                record[vertex * 6] = x; record[vertex * 6 + 1] = y; record[vertex * 6 + 3] = colors[vertex];
                record[vertex * 6 + 4] = x; record[vertex * 6 + 5] = y;
            }
            record[18] = texture as u32;
            record[19] = 0x2A00 | u32::from(cull) << 16 | u32::from(options) << 24;
            record[20] = low; record[21] = high;
        }
        fn call(&mut self) -> [u32; 7] {
            let mut frame = [fc4p_008e0100 as *const () as u32, self.object.ptr() as u32, 0x1234_5678, 0, 0, 0, 0];
            unsafe { invoke(frame.as_mut_ptr()); }
            frame
        }
        fn triangle_ptr(&mut self, index: usize) -> u32 { (unsafe { self.triangles.as_mut_ptr().add(index * 22) }) as u32 }
    }

    #[test]
    fn raster_queue_untextured_ring_wrap_vertex_order_pixel_rectangle_and_return_registers() {
        let mut f = Fixture::new();
        f.set_triangle(1023, -1, 0, false, [u32::MAX; 3], 0x10_8000, 0x1000);
        f.set_triangle(0, -1, 0, true, [0; 3], 0x1_0000, 0);
        unsafe { let q = RasterQueue::from_ptr(f.object.ptr()); q.set_head(u32::MAX); q.set_pending(2); }
        let a = f.triangle_ptr(1023); let b = f.triangle_ptr(0);
        let triangles_before = f.triangles.clone();
        let mut object_before = unsafe { core::slice::from_raw_parts(f.object.ptr(), 0x430).to_vec() };
        object_before[0x20..0x24].copy_from_slice(&0u32.to_le_bytes());
        object_before[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
        let frame = f.call();
        assert_eq!(&frame[3..6], &[0, 1, 2]);
        assert_eq!(events(), vec![Event::Lookup(0, 0), Event::Triangle([a + 48, a + 24, a], 0x44, 1, 2),
            Event::Triangle([a, a + 24, a + 48], 0x44, 0x44, 2), Event::Lookup(0, 0),
            Event::Triangle([b + 48, b + 24, b], 0x44, 1, 2)]);
        assert_eq!(f.triangles, triangles_before);
        assert_eq!(unsafe { core::slice::from_raw_parts(f.object.ptr(), 0x430) }, object_before);
        let mut expected = vec![0xABCD_EFFF; 128];
        for index in [48, 49, 56, 57] { expected[index] = 0; }
        assert_eq!(f.pixels, expected);
        assert_eq!(unsafe { RasterQueue::from_ptr(f.object.ptr()).pending() }, 0);
        assert_eq!(unsafe { RasterQueue::from_ptr(f.object.ptr()).head() }, 1);
        let mut expected_globals = [0xA5A5_A5A5u32; 16]; expected_globals[14] = f.code.as_ptr() as u32;
        assert_eq!(*f.globals, expected_globals);
    }

    #[test]
    fn raster_queue_mipmap_selection_backoff_signed_coordinates_filter_and_texture_globals() {
        for missing in [false, true] {
            let mut f = Fixture::new();
            f.set_triangle(0, 7, 0x1A, false, [1, 2, 3], 0x8_0000, 0x1000);
            unsafe { RasterQueue::from_ptr(f.object.ptr()).set_filter_enabled(1); }
            // Texture area is 128 times screen area: level 3 is selected, then level 2 is tried.
            for (offset, value) in [(4, -40i32), (5, -24), (10, -40), (11, 8168), (16, 4056), (17, -24)] {
                f.triangles[offset] = value as u32;
            }
            STATE.with(|s| { let mut s = s.borrow_mut(); s.missing[3] = true; if missing { s.missing = [true; 9]; } });
            let frame = f.call();
            assert_eq!(frame[3], 0);
            assert_eq!(&frame[4..6], &[1, 2]);
            let mut expected = vec![Event::Size(7), Event::Level(7, 3), Event::Level(7, 2)];
            if missing { expected.extend([Event::Level(7, 1), Event::Level(7, 0)]); }
            let width = if missing { 0 } else { 3u32 };
            let height = if missing { 0 } else { 7u32 };
            expected.extend([Event::Multiply(u64::from(height), 0xFFF0_0000_0000_0000), Event::Lookup(0x19_0000, if missing { 0 } else { 0x1000 })]);
            let p = f.triangle_ptr(0); expected.push(Event::Triangle([p + 48, p + 24, p], 0x44, 1, 2));
            assert_eq!(events(), expected);
            for (offset, value) in [(4, -40i32), (5, -24), (10, -40), (11, 8168), (16, 4056), (17, -24)] {
                assert_eq!(f.triangles[offset], ((value >> if missing { 0 } else { 2 }).wrapping_sub(8)) as u32);
            }
            assert_eq!(f.globals[0], height << 8 | width);
            assert_eq!(f.globals[1], 0);
            let expected_bias = u64::from(height).wrapping_mul(0xFFF0_0000_0000_0000)
                .wrapping_sub(u64::from(width << 20)).wrapping_add(0x7FFF_7FFF_7FFF_7FFF);
            assert_eq!(u64::from(f.globals[2]) | u64::from(f.globals[3]) << 32, expected_bias);
            assert_eq!(f.globals[11], if missing { f.default_texture.as_ref() as *const u8 as u32 } else { 0x3141_5926 });
            assert_eq!(f.globals[15], 0x2A);
        }
    }

    #[test]
    fn raster_queue_cache_generation_copies_word_and_byte_tails_and_retains_failed_lookup_payload() {
        for size in 0..9 {
            let mut f = Fixture::new();
            STATE.with(|s| { let mut s = s.borrow_mut(); s.cache_miss = true; s.size = size; });
            let p = f.triangle_ptr(0);
            let frame = f.call();
            assert_eq!(&frame[3..6], &[0, 0x77, 2]);
            assert_eq!(events(), vec![Event::Lookup(0, 0), Event::Allocate(0x1000), Event::Generate(0, 0),
                Event::Insert(0, 0, 0x77), Event::Free, Event::Triangle([p + 48, p + 24, p], 0x77, 1, 2),
                Event::Triangle([p, p + 24, p + 48], 0x77, 0x77, 2)]);
            assert_eq!(&f.code[..size as usize], &f.scratch[..size as usize]);
            assert_eq!(&f.code[size as usize..], &vec![0xCD; 64 - size as usize]);
        }
        let mut f = Fixture::new();
        STATE.with(|s| { let mut s = s.borrow_mut(); s.cache_miss = true; s.allocation_fails = true; });
        let mut ecx = 0;
        assert_eq!(unsafe { drain_raster_triangles(f.object.ptr(), 99, 0xAABB_CCDD, &mut ecx) }, 2u64 << 32);
        assert_eq!(ecx, 0xAABB_CCDD);
        assert_eq!(f.globals[14], 0);
        assert!(events().contains(&Event::Triangle([f.triangle_ptr(0), f.triangle_ptr(0) + 24, f.triangle_ptr(0) + 48], 0xAABB_CCDD, 0xAABB_CCDD, 2)));
    }

    #[test]
    fn raster_queue_culling_and_degenerate_negative_pixel_bounds_keep_last_span_inputs() {
        for (cull, coords) in [(false, [(-256i32, -256i32), (0, -256), (-256, 0)]), (true, [(0, 0), (512, 0), (0, 512)]),
            (true, [(0, 0), (0, 0), (0, 0)])] {
            let mut f = Fixture::new();
            f.set_triangle(0, 0, 0, cull, [u32::MAX; 3], 0, 0);
            for (vertex, (x, y)) in coords.into_iter().enumerate() { f.triangles[vertex * 6] = x as u32; f.triangles[vertex * 6 + 1] = y as u32; }
            let frame = f.call();
            let trace = events();
            assert_eq!(trace.iter().filter(|e| matches!(e, Event::Triangle(..))).count(), usize::from(!cull));
            if !cull { assert_eq!(&frame[4..6], &[u32::MAX, 0x44]); assert_eq!(f.pixels[39], 0); }
            else { assert_eq!(frame[4], if coords[1].0 == 512 { 1 } else { u32::MAX }); }
        }
    }

    #[test]
    fn raster_queue_empty_keeps_mmx_and_entry_registers_and_nonempty_empties_tags() {
        let _fpu = GameFpu::new();
        #[repr(C, align(16))] struct Fx([u8; 512]);
        for count in [0, 1] {
            let mut f = Fixture::new();
            unsafe { RasterQueue::from_ptr(f.object.ptr()).set_pending(count); }
            let mut before = Fx([0; 512]); let mut after = Fx([0; 512]);
            let snapshots = [&mut before as *mut Fx, &mut after as *mut Fx];
            let mut frame = [fc4p_008e0100 as *const () as u32, f.object.ptr() as u32, 0x1234_5678, 0, 0, 0, 0];
            unsafe { core::arch::asm!(
                "pxor mm0, mm0", "mov eax, [edi]", "fxsave [eax]", "call {invoke}",
                "mov eax, [edi + 4]", "fxsave [eax]", "emms",
                invoke = sym invoke, in("ecx") frame.as_mut_ptr(), in("edi") snapshots.as_ptr(),
                clobber_abi("C"), out("mm0") _);
            }
            assert_eq!(after.0[4], if count == 0 { 0xFF } else { 0 });
            assert_eq!(&before.0[..2], &after.0[..2]);
            assert_eq!(&before.0[24..28], &after.0[24..28]);
            assert_eq!(frame[6] & 0x400, 0);
            if count == 0 { assert_eq!(&frame[3..6], &[0, f.object.ptr() as u32, 0x1234_5678]); assert!(events().is_empty()); }
        }
    }

    #[test]
    fn raster_queue_color_key_cases_texture_area_overflow_and_level_limit() {
        for (colors, low, expected) in [
            ([u32::MAX; 3], 0x1_8000, 0x1_0000),
            ([u32::MAX; 3], 0x8_8000, 0x8_8000),
            ([0; 3], 0x1_8000, 0x8000),
            ([1, 1, 2], 0x8000, 0x1_8000),
        ] {
            let mut f = Fixture::new();
            f.set_triangle(0, -1, 0, true, colors, low | 0x10_0000, 0x1000);
            f.call();
            assert_eq!(events()[0], Event::Lookup(expected, 0));
        }
        let mut f = Fixture::new();
        f.set_triangle(0, 2, 0xA, true, [u32::MAX; 3], 0, 0);
        for offset in [0, 1, 6, 7, 12, 13] { f.triangles[offset] = 0; }
        f.call();
        assert!(events().contains(&Event::Level(2, 8)));
        assert_eq!(f.globals[0], 0);

        let mut f = Fixture::new();
        f.set_triangle(0, 3, 0xC, true, [u32::MAX; 3], 0, 0);
        unsafe { RasterQueue::from_ptr(f.object.ptr()).set_filter_enabled(1); }
        // The signed absolute value of this wrapped texture area remains i32::MIN.
        for (offset, value) in [(4, 0), (5, 0), (10, 0), (11, 1), (16, 0x8000_0000), (17, 0)] { f.triangles[offset] = value; }
        f.call();
        assert_eq!(&events()[..4], &[Event::Size(3), Event::Level(3, 0),
            Event::Multiply(31, 0xFFF0_0000_0000_0000), Event::Lookup(0, 0x1000)]);
    }

    #[test]
    fn raster_queue_texture_masks_keep_unsigned_width_and_bias_carries() {
        for (width, height) in [(0u32, 0u32), (1, 1), (0xFFFF_FFFF, 0x8000_0000)] {
            let mut f = Fixture::new();
            f.set_triangle(0, 5, 0, true, [u32::MAX; 3], 0, 0);
            STATE.with(|s| { let mut s = s.borrow_mut(); s.width = width; s.height = height; });
            f.call();
            let width = width.wrapping_sub(1); let height = height.wrapping_sub(1);
            let bias = u64::from(height).wrapping_mul(0xFFF0_0000_0000_0000)
                .wrapping_sub(u64::from(width.wrapping_shl(20))).wrapping_add(0x7FFF_7FFF_7FFF_7FFF);
            assert_eq!(f.globals[0], height.wrapping_shl(8) | width);
            assert_eq!(f.globals[2], bias as u32); assert_eq!(f.globals[3], (bias >> 32) as u32);
        }
    }

    #[test]
    fn raster_queue_generated_code_keeps_forward_word_overlap_behavior() {
        let mut f = Fixture::new();
        let before = f.scratch.clone();
        STATE.with(|s| {
            let mut s = s.borrow_mut(); s.cache_miss = true; s.size = 11;
            s.destination = s.scratch + 4;
        });
        f.call();
        assert_eq!(&f.scratch[..4], &before[..4]);
        assert_eq!(&f.scratch[4..15], &[0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2]);
        assert_eq!(&f.scratch[15..], &before[15..]);
        assert_eq!(f.code, vec![0xCD; 64]);
    }
}

#[cfg(test)]
mod lua_command_tests {
    use super::*;
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    const RETURN_EDX: u32 = 0xBADC_0FFE;
    const FREE_EDX: u32 = 0x1234_5678;
    const BUFFER: u32 = 0x1122_3344;
    const TEXT: &[u8] = b"command text\0";

    #[derive(Debug, PartialEq)]
    enum Event {
        Upvalue(u32), Userdata(u32), Clear(u32, u32), Top, Type(i32),
        ToBool(i32), ToNumber(i32), ToString(i32), NewVariant, SetBool(u32),
        SetNumber(u64), NewString(bool), SetString, SetParameter(u32), DestroyVariant,
        Command(u32, u32, u32), PushNumber(u64), First, Get(u32), Kind(i32),
        GetBool, GetNumber, GetString, Chars, PushBool(u32), PushString(u32), PushNil,
        Next(u32), Free(u32),
    }
    #[derive(Default)]
    struct State {
        thread: u32, generator: u32, main: u32, input: u32, output: u32, server: u32,
        index: f64, arguments: Vec<u32>, top: i32, status: i32, keys: Vec<u32>,
        variants: Vec<u32>, tables: Vec<u32>, buffer: bool,
        variant: u32, output_string: u32, input_string: u32, events: Vec<Event>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn event(e: Event) { STATE.with(|s| s.borrow_mut().events.push(e)); }
    fn take_events() -> Vec<Event> { STATE.with(|s| core::mem::take(&mut s.borrow_mut().events)) }
    fn thread(p: *mut u8) { STATE.with(|s| assert_eq!(p as u32, s.borrow().thread)); }
    fn variant(p: *mut u8) { STATE.with(|s| assert_eq!(p as u32, s.borrow().variant)); }
    extern "thiscall" fn upvalue(this: *mut u8, which: u32) -> u32 {
        thread(this);
        event(Event::Upvalue(which));
        STATE.with(|s| {
            if let Some(table) = s.borrow_mut().tables.pop() {
                unsafe { write(this, 0, table); }
            }
        });
        1000 + which
    }
    extern "thiscall" fn userdata(this: *mut u8, token: u32) -> u32 {
        thread(this);
        event(Event::Userdata(token));
        STATE.with(|s| match token {
            1002 => s.borrow().generator,
            1003 => s.borrow().main,
            _ => panic!("unexpected userdata token"),
        })
    }
    extern "thiscall" fn wrong_userdata(_: *mut u8, _: u32) -> u32 {
        panic!("the callback must use the table saved before the upvalue call")
    }
    extern "thiscall" fn number(this: *mut u8, index: i32) -> f64 {
        thread(this);
        event(Event::ToNumber(index));
        STATE.with(|s| if index == 1001 { s.borrow().index } else { -123.25 })
    }
    extern "thiscall" fn boolean(this: *mut u8, index: i32) -> u32 {
        thread(this); event(Event::ToBool(index)); 0xABCD_0001
    }
    extern "thiscall" fn text(this: *mut u8, index: i32) -> *const u8 {
        thread(this); event(Event::ToString(index)); TEXT.as_ptr()
    }
    extern "thiscall" fn clear(this: *mut u8, flags: u32) {
        STATE.with(|s| assert!([s.borrow().input, s.borrow().output].contains(&(this as u32))));
        event(Event::Clear(this as u32, flags));
    }
    extern "thiscall" fn top(this: *mut u8) -> i32 {
        thread(this); event(Event::Top); STATE.with(|s| s.borrow().top)
    }
    extern "thiscall" fn kind(this: *mut u8, index: i32) -> u32 {
        thread(this); event(Event::Type(index)); STATE.with(|s| s.borrow().arguments[index as usize - 1])
    }
    extern "thiscall" fn new_variant(this: *mut u8) -> *mut u8 {
        STATE.with(|s| s.borrow_mut().variant = this as u32);
        event(Event::NewVariant);
        unsafe { write(this, 0, [0x00AC_ECD0u32, 0, 0, 0, 0]); }
        this
    }
    extern "thiscall" fn set_bool(this: *mut u8, value: u32) -> u32 {
        variant(this); event(Event::SetBool(value)); unsafe { write(this, 4, value); } 0
    }
    extern "thiscall" fn set_number(this: *mut u8, value: f64) -> u32 {
        variant(this); event(Event::SetNumber(value.to_bits())); unsafe { write(this, 4, value); } 0
    }
    extern "thiscall" fn new_text(this: *mut u8, value: *const u8) -> *mut u8 {
        assert_eq!(value, TEXT.as_ptr());
        STATE.with(|s| {
            assert_eq!(this as u32, s.borrow().variant + 40);
            s.borrow_mut().input_string = this as u32;
        });
        event(Event::NewString(true));
        unsafe { write(this, 0, [0x00A8_0810u32, BUFFER, 0, 0, 0]); }
        this
    }
    extern "thiscall" fn set_string(this: *mut u8, value: *mut u8) -> u32 {
        variant(this);
        STATE.with(|s| assert_eq!(value as u32, s.borrow().input_string));
        event(Event::SetString); 0
    }
    extern "thiscall" fn set_parameter(this: *mut u8, index: u32, value: *mut u8) {
        STATE.with(|s| assert_eq!(this as u32, s.borrow().input));
        variant(value); event(Event::SetParameter(index));
    }
    extern "thiscall" fn destroy_variant(this: *mut u8) -> u32 {
        variant(this); event(Event::DestroyVariant); 0
    }
    extern "thiscall" fn command(this: *mut u8, id: u32, input: *mut u8, output: *mut u8) -> i32 {
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(this as u32, s.server);
            assert_eq!(input as u32, s.input);
            assert_eq!(output as u32, s.output);
        });
        event(Event::Command(id, input as u32, output as u32));
        STATE.with(|s| s.borrow().status)
    }
    extern "thiscall" fn push_number(this: *mut u8, value: f64) -> u64 {
        thread(this); event(Event::PushNumber(value.to_bits())); u64::from(RETURN_EDX) << 32 | 0xA5A5_A5A5
    }
    extern "thiscall" fn new_string(this: *mut u8) -> *mut u8 {
        STATE.with(|s| s.borrow_mut().output_string = this as u32);
        event(Event::NewString(false));
        let buffer = STATE.with(|s| if s.borrow().buffer { BUFFER } else { 0 });
        unsafe { write(this, 0, [0x00A8_0810u32, buffer, 0, 0, 0]); }
        this
    }
    extern "thiscall" fn first(this: *mut u8) -> u64 {
        STATE.with(|s| assert_eq!(this as u32, s.borrow().output));
        event(Event::First);
        let key = STATE.with(|s| s.borrow().keys.first().copied().unwrap_or(u32::MAX));
        u64::from(RETURN_EDX) << 32 | u64::from(key)
    }
    extern "thiscall" fn get(this: *mut u8, key: u32) -> u32 {
        STATE.with(|s| assert_eq!(this as u32, s.borrow().output));
        event(Event::Get(key));
        STATE.with(|s| {
            let s = s.borrow(); let index = s.keys.iter().position(|&k| k == key).unwrap(); s.variants[index]
        })
    }
    extern "thiscall" fn next(this: *mut u8, key: u32) -> u64 {
        STATE.with(|s| assert_eq!(this as u32, s.borrow().output));
        event(Event::Next(key));
        let key = STATE.with(|s| {
            let s = s.borrow(); let index = s.keys.iter().position(|&k| k == key).unwrap();
            s.keys.get(index + 1).copied().unwrap_or(u32::MAX)
        });
        u64::from(RETURN_EDX) << 32 | u64::from(key)
    }
    extern "thiscall" fn variant_kind(this: *mut u8) -> i32 {
        let kind = unsafe { read(this, 4) }; event(Event::Kind(kind)); kind
    }
    extern "thiscall" fn get_bool(_: *mut u8) -> u32 { event(Event::GetBool); 0xF0F0_0001 }
    extern "thiscall" fn get_number(_: *mut u8) -> f64 { event(Event::GetNumber); 42.125 }
    extern "thiscall" fn get_string(_: *mut u8, string: *mut u8) {
        STATE.with(|s| assert_eq!(string as u32, s.borrow().output_string));
        event(Event::GetString);
    }
    extern "thiscall" fn chars(this: *mut u8) -> *const u8 {
        STATE.with(|s| assert_eq!(this as u32, s.borrow().output_string));
        event(Event::Chars); TEXT.as_ptr()
    }
    extern "thiscall" fn push_bool(this: *mut u8, value: u32) { thread(this); event(Event::PushBool(value)); }
    extern "thiscall" fn push_string(this: *mut u8, value: u32) { thread(this); event(Event::PushString(value)); }
    extern "thiscall" fn push_nil(this: *mut u8) { thread(this); event(Event::PushNil); }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn first_preserves_edx(_: *mut u8) -> u64 {
        core::arch::naked_asm!("mov eax, -1", "ret");
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn extended_number(_: *mut u8, _: i32) -> f64 {
        core::arch::naked_asm!(
            "call {record}", "sub esp, 12", "mov dword ptr [esp], 0x400",
            "mov dword ptr [esp + 4], 0x80000000", "mov word ptr [esp + 8], 0x3fff",
            "fld tbyte ptr [esp]", "add esp, 12", "ret 4", record = sym extended_number_event);
    }
    extern "C" fn extended_number_event() { event(Event::ToNumber(1)); }
    extern "C" fn free_string(buffer: *mut u8) -> u64 {
        assert_eq!(buffer as u32, BUFFER);
        STATE.with(|s| {
            let s = s.borrow();
            let p = if matches!(s.events.last(), Some(Event::SetString)) { s.input_string } else { s.output_string };
            assert_eq!(unsafe { read::<u32>(p as *mut u8, 0) }, 0x00A8_0810);
        });
        event(Event::Free(buffer as u32)); u64::from(FREE_EDX) << 32 | 0xFFFF_FFFF
    }

    struct Fixture {
        _objects: Vec<Object>, tables: Vec<Vec<usize>>, state: [u32; 2], records: [u8; 88],
        _fakes: Vec<crate::testing::Fake>, _constants: Box<[f64; 2]>,
    }
    impl Fixture {
        fn new() -> Self {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut tables = vec![vec![0usize; 0x150 / 4]; 4];
            for (offset, method) in [
                (0x148, upvalue as *const ()), (0xB8, userdata as *const ()), (0xA4, number as *const ()),
                (0xA8, boolean as *const ()), (0xAC, text as *const ()), (0x38, top as *const ()),
                (0x5C, kind as *const ()), (0xC4, push_number as *const ()), (0xD4, push_bool as *const ()),
                (0xCC, push_string as *const ()), (0xC0, push_nil as *const ()),
            ] { tables[0][offset / 4] = method as usize; }
            tables[1][0x10 / 4] = clear as *const () as usize;
            tables[1][0x1C / 4] = set_parameter as *const () as usize;
            tables[2][0x10 / 4] = clear as *const () as usize;
            tables[2][0x34 / 4] = first as *const () as usize;
            tables[2][0x18 / 4] = get as *const () as usize;
            tables[2][0x38 / 4] = next as *const () as usize;
            tables[3][0x50 / 4] = command as *const () as usize;
            let mut objects: Vec<_> = (0..5).map(|_| Object::new(0x60)).collect();
            let pointers: Vec<_> = objects.iter_mut().map(|o| o.ptr()).collect();
            unsafe {
                for (object, table) in [0, 2, 3, 4].into_iter().zip(&tables) { write(pointers[object], 0, table.as_ptr()); }
                let generator = LuaCommandGenerator::from_ptr(pointers[1]);
                generator.set_main_thread(0x1000 as *mut u8);
                generator.set_input(pointers[2]); generator.set_output(pointers[3]); generator.set_server(pointers[4]);
            }
            let mut constants = Box::new([0., 2.]);
            let fakes = vec![
                fake(0x0091_66D4, new_variant as *const () as usize), fake(0x0091_6657, set_bool as *const () as usize),
                fake(0x0091_66AA, set_number as *const () as usize), fake(0x0091_66CF, set_string as *const () as usize),
                fake(0x0091_5EB5, destroy_variant as *const () as usize), fake(0x0040_8480, new_text as *const () as usize),
                fake(0x0057_8390, new_string as *const () as usize), fake(0x009D_7E63, chars as *const () as usize),
                fake(0x0090_CF63, free_string as *const () as usize),
                fake_data(0x00A8_0990, constants.as_mut_ptr().cast()),
                fake_data(0x00A8_0AC8, unsafe { constants.as_mut_ptr().add(1) }.cast()),
            ];
            STATE.with(|s| *s.borrow_mut() = State {
                thread: pointers[0] as u32, generator: pointers[1] as u32, main: 0x1000,
                input: pointers[2] as u32, output: pointers[3] as u32, server: pointers[4] as u32,
                status: -7, ..State::default()
            });
            Self { _objects: objects, tables, state: [pointers[0] as u32, 0], records: [0xA5; 88],
                _fakes: fakes, _constants: constants }
        }
        fn invoke(&mut self) -> u64 {
            let generator = STATE.with(|s| s.borrow().generator) as *mut u8;
            if !generator.is_null() { unsafe {
                LuaCommandGenerator::from_ptr(generator).set_records(self.records.as_mut_ptr());
                LuaCommandGenerator::from_ptr(generator).set_records_end(self.records.as_mut_ptr().add(88));
            } }
            let mut call = RawCall::new(Conv::Cdecl, fc4p_008b5b70 as *const () as usize, 0, &[]);
            let args = [unsafe { self.state.as_mut_ptr().add(1) } as u32];
            call.args = args.as_ptr(); call.count = 1;
            call.eax = 0xAAAA_AAAA; call.edx = 0xBBBB_BBBB;
            unsafe { call_raw(&mut call); }
            u64::from(call.edx) << 32 | u64::from(call.eax)
        }
    }

    #[test]
    fn lua_command_converts_arguments_and_preserves_full_boolean_words_and_call_order() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new();
        STATE.with(|s| { let mut s = s.borrow_mut(); s.arguments = vec![1, 3, 4, 0, 2, 5]; s.top = 6; s.index = 1.9; });
        f.records[44..48].copy_from_slice(&0x8765_4321u32.to_le_bytes());
        let before: Vec<_> = f._objects.iter_mut().map(|o| unsafe { core::slice::from_raw_parts(o.ptr(), 0x60).to_vec() }).collect();
        assert_eq!(f.invoke(), u64::from(RETURN_EDX) << 32 | 1);
        for (index, object) in f._objects.iter_mut().enumerate() {
            let bytes = unsafe { core::slice::from_raw_parts(object.ptr(), 0x60) };
            if index == 1 {
                assert_eq!(&bytes[..0x14], &before[index][..0x14]);
                assert_eq!(&bytes[0x1C..], &before[index][0x1C..]);
            } else { assert_eq!(bytes, before[index]); }
        }
        let (input, output) = STATE.with(|s| (s.borrow().input, s.borrow().output));
        assert_eq!(take_events(), vec![
            Event::Upvalue(2), Event::Userdata(1002), Event::Upvalue(3), Event::Userdata(1003),
            Event::Upvalue(1), Event::ToNumber(1001), Event::Clear(input, 0), Event::Clear(output, 0), Event::Top,
            Event::NewVariant, Event::Type(1), Event::ToBool(1), Event::SetBool(0xABCD_0001), Event::SetParameter(0), Event::DestroyVariant,
            Event::NewVariant, Event::Type(2), Event::ToNumber(2), Event::SetNumber((-123.25f64).to_bits()), Event::SetParameter(1), Event::DestroyVariant,
            Event::NewVariant, Event::Type(3), Event::ToString(3), Event::NewString(true), Event::SetString, Event::Free(BUFFER), Event::SetParameter(2), Event::DestroyVariant,
            Event::NewVariant, Event::Type(4), Event::SetNumber(0), Event::SetParameter(3), Event::DestroyVariant,
            Event::NewVariant, Event::Type(5), Event::SetNumber(0), Event::SetParameter(4), Event::DestroyVariant,
            Event::NewVariant, Event::Type(6), Event::SetNumber(0), Event::SetParameter(5), Event::DestroyVariant,
            Event::Command(0x8765_4321, input, output), Event::PushNumber((-7f64).to_bits()), Event::NewString(false), Event::First,
        ]);
        assert_eq!(&f.records[..44], &[0xA5; 44]);
        assert_eq!(&f.records[48..], &[0xA5; 40]);
    }

    #[test]
    fn lua_command_output_types_reserved_key_and_cleanup_keep_order_and_edx() {
        let _fpu = GameFpu::new();
        for buffer in [false, true] {
            let mut f = Fixture::new();
            let mut table = vec![0usize; 0xE0 / 4];
            for (slot, fun) in [(0x0C, variant_kind as *const ()), (0x24, get_bool as *const ()),
                (0x9C, get_number as *const ()), (0xDC, get_string as *const ())] { table[slot / 4] = fun as usize; }
            let kinds = [1i32, 2, 14, 0, -1, 15];
            let mut values: Vec<_> = kinds.iter().map(|&k| [table.as_ptr() as u32, k as u32]).collect();
            STATE.with(|s| {
                let mut s = s.borrow_mut(); s.buffer = buffer; s.top = -1; s.status = i32::MIN;
                s.keys = vec![0x2B99_9D2B, 10, 11, 12, 13, 14, 15, 16];
                s.variants = vec![0, 0]; s.variants.extend(values.iter_mut().map(|v| v.as_mut_ptr() as u32));
            });
            assert_eq!(f.invoke(), u64::from(if buffer { FREE_EDX } else { RETURN_EDX }) << 32 | 8);
            let (input, output) = STATE.with(|s| (s.borrow().input, s.borrow().output));
            let mut expected = vec![Event::Upvalue(2), Event::Userdata(1002), Event::Upvalue(3), Event::Userdata(1003),
                Event::Upvalue(1), Event::ToNumber(1001), Event::Clear(input, 0), Event::Clear(output, 0), Event::Top,
                Event::Command(0xA5A5_A5A5, input, output), Event::PushNumber((i32::MIN as f64).to_bits()),
                Event::NewString(false), Event::First, Event::Next(0x2B99_9D2B), Event::Get(10), Event::PushNil, Event::Next(10),
                Event::Get(11), Event::Kind(1), Event::GetBool, Event::PushBool(0xF0F0_0001), Event::Next(11)];
            for (key, kind) in [(12, 2), (13, 14)] {
                expected.extend([Event::Get(key), Event::Kind(kind), Event::GetNumber, Event::PushNumber(42.125f64.to_bits()), Event::Next(key)]);
            }
            for (key, kind) in [(14, 0), (15, -1), (16, 15)] {
                expected.extend([Event::Get(key), Event::Kind(kind), Event::GetString, Event::Chars, Event::PushString(TEXT.as_ptr() as u32), Event::Next(key)]);
            }
            if buffer { expected.push(Event::Free(BUFFER)); }
            assert_eq!(take_events(), expected);
        }
    }

    #[test]
    fn lua_command_rejects_missing_owner_server_thread_and_out_of_range_indices() {
        let _fpu = GameFpu::new();
        for failure in 0..3 {
            let mut f = Fixture::new();
            STATE.with(|s| { let mut s = s.borrow_mut(); match failure {
                0 => s.generator = 0,
                1 => unsafe { LuaCommandGenerator::from_ptr(s.generator as *mut u8).set_server(core::ptr::null_mut()); },
                _ => s.main = 0x9999,
            } });
            assert_eq!(f.invoke(), u64::from(RETURN_EDX) << 32 | 1);
            assert_eq!(take_events(), vec![Event::Upvalue(2), Event::Userdata(1002), Event::Upvalue(3), Event::Userdata(1003), Event::PushNumber(2f64.to_bits())]);
        }
        // Truncation keeps the low 32 bits, including integer-indefinite and values past i32.
        for (index, valid) in [(-1., false), (-0.9, true), (0., true), (1.999, true), (2., false),
            (f64::NAN, true), (f64::INFINITY, true), (4294967296., true), (2147483648., false)] {
            let mut f = Fixture::new();
            STATE.with(|s| s.borrow_mut().index = index);
            assert_eq!(f.invoke() as u32, 1);
            let events = take_events();
            if valid { assert!(events.contains(&Event::First), "index {index}"); }
            else { assert_eq!(&events[6..], &[Event::PushNumber(2f64.to_bits())], "index {index}"); }
        }
    }

    #[test]
    fn lua_command_saved_upvalue_table_survives_a_table_change() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new();
        let mut next_table = f.tables[0].clone();
        next_table[0xB8 / 4] = wrong_userdata as *const () as usize;
        // The first two upvalues replace the table before their userdata conversion.
        let mut middle_table = f.tables[0].clone();
        middle_table[0xA4 / 4] = number as *const () as usize;
        STATE.with(|s| s.borrow_mut().tables = vec![next_table.as_mut_ptr() as u32, middle_table.as_mut_ptr() as u32]);
        assert_eq!(f.invoke(), u64::from(RETURN_EDX) << 32 | 1);
        assert_eq!(take_events().iter().filter(|e| matches!(e, Event::Userdata(_))).count(), 2);
    }

    #[test]
    fn lua_command_empty_output_preserves_iterator_edx_and_rounds_extended_st0_only_at_store() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new();
        f.tables[2][0x34 / 4] = first_preserves_edx as *const () as usize;
        assert_eq!(f.invoke(), (f.tables[2].as_ptr() as u64) << 32 | 1);
        take_events();
        f.tables[0][0xA4 / 4] = extended_number as *const () as usize;
        STATE.with(|s| { let mut s = s.borrow_mut(); s.arguments = vec![3]; s.top = 1; });
        assert_eq!(f.invoke() as u32, 1);
        assert!(take_events().contains(&Event::SetNumber(1f64.to_bits())));
    }

    #[test]
    fn lua_command_all_numeric_output_kinds_skip_a_reserved_key_between_values() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new();
        let mut table = [0usize; 0xA0 / 4];
        table[0x0C / 4] = variant_kind as *const () as usize;
        table[0x9C / 4] = get_number as *const () as usize;
        let mut values: Vec<_> = (2..=14).map(|kind| [table.as_ptr() as u32, kind]).collect();
        STATE.with(|s| {
            let mut s = s.borrow_mut(); s.keys = (2..=14).collect();
            s.variants = values.iter_mut().map(|v| v.as_mut_ptr() as u32).collect();
            s.keys.insert(5, 0x2B99_9D2B); s.variants.insert(5, 0);
        });
        assert_eq!(f.invoke(), u64::from(RETURN_EDX) << 32 | 14);
        let trace = take_events();
        assert!(!trace.contains(&Event::Get(0x2B99_9D2B)));
        for kind in 2..=14 { assert!(trace.contains(&Event::Kind(kind))); }
        assert_eq!(trace.iter().filter(|e| **e == Event::GetNumber).count(), 13);
    }
}

#[cfg(test)]
mod depth_edge_tests {
    use super::*;
    use crate::testing::fake_data;

    #[repr(C, align(16))]
    struct SimdState([u8; 512]);
    #[unsafe(naked)]
    unsafe extern "fastcall" fn invoke_simd(_: *mut u32) {
        core::arch::naked_asm!(
            "push ebp",
            "push ebx",
            "push esi",
            "push edi",
            "push ecx",
            "mov edx, [ecx + 64]",
            "movq mm0, [edx + 32]",
            "movq mm1, [edx + 48]",
            "movq mm2, [edx + 64]",
            "movq mm3, [edx + 80]",
            "movq mm4, [edx + 96]",
            "movq mm5, [edx + 112]",
            "movq mm6, [edx + 128]",
            "movq mm7, [edx + 144]",
            "mov ebp, [ecx + 4]",
            "mov esi, [ecx + 8]",
            "mov ebx, [ecx + 16]",
            "mov edi, [ecx + 20]",
            "mov eax, [ecx + 24]",
            "mov edx, [ecx + 28]",
            "push dword ptr [ecx]",
            "push dword ptr [ecx + 60]",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "push 0",
            "mov ecx, [ecx + 12]",
            "call dword ptr [esp + 44]",
            "add esp, 48",
            "push eax",
            "mov eax, [esp + 4]",
            "mov [eax + 32], edx",
            "mov [eax + 36], ecx",
            "mov [eax + 40], ebx",
            "mov [eax + 44], esi",
            "mov [eax + 48], edi",
            "mov [eax + 52], ebp",
            "mov edx, [eax + 64]",
            "fxsave [edx]",
            "emms",
            "pop edx",
            "mov [eax + 56], edx",
            "pop ecx",
            "pop edi",
            "pop esi",
            "pop ebx",
            "pop ebp",
            "ret"
        )
    }
    fn simd_state() -> (SimdState, [u64; 8]) {
        let mut state = SimdState([0u8; 512]);
        let seed = core::array::from_fn(|i| 0x1234_5678_90AB_CDEFu64.wrapping_mul(i as u64 + 1));
        for (i, value) in seed.iter().enumerate() {
            state.0[32 + i * 16..40 + i * 16].copy_from_slice(&value.to_le_bytes());
        }
        (state, seed)
    }
    fn assert_simd(state: &SimdState, expected: [u64; 8], mxcsr: u32) {
        assert_eq!(
            u16::from_le_bytes(state.0[0..2].try_into().unwrap()),
            crate::x87::GAME_CONTROL_WORD
        );
        assert_eq!(u16::from_le_bytes(state.0[2..4].try_into().unwrap()) & 0x3800, 0);
        assert_eq!(state.0[4], 0xFF);
        assert_eq!(u32::from_le_bytes(state.0[24..28].try_into().unwrap()), mxcsr);
        for (i, value) in expected.into_iter().enumerate() {
            assert_eq!(
                u64::from_le_bytes(state.0[32 + i * 16..40 + i * 16].try_into().unwrap()),
                value,
                "MM{i}"
            );
            assert_eq!(&state.0[40 + i * 16..42 + i * 16], &[0xFF, 0xFF], "MM{i} exponent");
        }
    }
    fn mxcsr() -> u32 {
        let mut value = 0u32;
        unsafe {
            core::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut value, options(nostack));
        }
        value
    }
    fn pack(a: u32, b: u32) -> u64 {
        u64::from(a) | (u64::from(b) << 32)
    }

    #[test]
    fn texture_edge_covers_coordinate_modes_direction_ties_registers_and_live_mmx() {
        let _fpu = crate::testing::GameFpu::new();
        for (dx, dy, slope, fraction_x, fraction_y, coordinates) in [
            ([0, 0], [0, 0], 0, 0, 0, [0, 0]),
            ([0, -1], [-1, 0], 0x12345, 0x7FFF, -0x4000, [0xFFFF, 0x8000]),
            ([-1, 0], [0, -1], -0x8000, -0x7FFF, 0x4000, [1, u32::MAX]),
            (
                [0x8000, -0x8000],
                [0x4000, -0x4000],
                0x4000,
                0x4000,
                0x4000,
                [0x1234, 0x5678],
            ),
            (
                [i32::MIN, i32::MAX],
                [i32::MAX, i32::MIN],
                i32::MIN,
                i32::MAX,
                i32::MIN,
                [u32::MAX, 0x8000_0000],
            ),
            (
                [i32::MAX, i32::MIN],
                [i32::MIN, i32::MAX],
                i32::MAX,
                i32::MIN,
                i32::MAX,
                [0x7FFF_FFFF, 0xFFFF_FFFF],
            ),
        ] {
            let mut dx = dx;
            let mut dy = dy;
            let _dx = fake_data(0x00B5_C9F0, dx.as_mut_ptr().cast());
            let _dy = fake_data(0x00B5_C9F8, dy.as_mut_ptr().cast());
            for flags in [0, 8, u32::MAX, 16] {
                let mut edge = [0xA5A5_A5A5u32; 20];
                edge[1] = slope as u32;
                edge[16] = fraction_x as u32;
                let mut vertex = [0x5A5A_5A5Au32; 8];
                vertex[4..6].copy_from_slice(&coordinates);
                let vertex_before = vertex;
                let mut expected = edge;
                let multiply = |a: i32, b: i32| ((i128::from(a) * i128::from(b)) >> 15) as u32;
                let mut raw = [0u32; 2];
                let mut sign_corrected = [0u32; 2];
                let mut masks = [0u32; 2];
                for i in 0..2 {
                    expected[4 + i] = multiply(slope, dx[i]).wrapping_sub(dy[i] as u32);
                    raw[i] = coordinates[i]
                        .wrapping_shl(if flags & 8 == 0 { 20 } else { 16 })
                        .wrapping_sub(multiply(dy[i], fraction_y))
                        .wrapping_add(multiply(dx[i], fraction_x));
                    let forward = if i == 0 { dx[i] } else { !dx[i] };
                    let downward = if i == 0 { dy[i] } else { !dy[i] };
                    masks[i] = if forward == 0 { u32::MAX } else { 0 };
                    sign_corrected[i] = raw[i].wrapping_sub(u32::from(forward < 0));
                    expected[2 + i] = sign_corrected[i].wrapping_sub(u32::from(forward == 0 && downward < 0));
                }
                let (mut simd, seed) = simd_state();
                let csr = mxcsr();
                let mut registers = [0u32; 17];
                registers[..8].copy_from_slice(&[
                    fc4p_008dd680 as *const () as u32,
                    edge.as_mut_ptr() as u32,
                    vertex.as_mut_ptr() as u32,
                    0x1234_5678,
                    fraction_y as u32,
                    0x3456_789A,
                    0x4567_89AB,
                    0x5678_9ABC,
                ]);
                registers[15] = flags;
                registers[16] = simd.0.as_mut_ptr() as u32;
                unsafe {
                    invoke_simd(registers.as_mut_ptr());
                }
                assert_eq!(edge, expected, "flags {flags:X}, dx {dx:?}, dy {dy:?}");
                assert_eq!(vertex, vertex_before);
                assert_eq!(registers[8], multiply(dx[1], fraction_x));
                assert_eq!(registers[9], registers[3]);
                assert_eq!(registers[10], fraction_y as u32);
                assert_eq!(registers[11], registers[2]);
                assert_eq!(registers[12], raw[1]);
                assert_eq!(registers[13], registers[1]);
                assert_eq!(
                    registers[14],
                    ((i128::from(dx[1]) * i128::from(fraction_x)) as u32) >> 15
                );
                assert_simd(
                    &simd,
                    [
                        pack(sign_corrected[0], sign_corrected[1]),
                        pack(expected[2], expected[3]),
                        0,
                        pack(masks[0], masks[1]),
                        seed[4],
                        seed[5],
                        seed[6],
                        seed[7],
                    ],
                    csr,
                );
            }
        }
    }

    #[test]
    fn color_edge_covers_rounding_overflow_all_channels_registers_and_live_mmx() {
        let _fpu = crate::testing::GameFpu::new();
        for (dx, dy, slope, fraction_x, fraction_y, color) in [
            ([0; 4], [0; 4], 0, 0, 0, 0),
            (
                [0x8000, -0x8000, 1, -1],
                [-0x4000, 0x4000, -1, 1],
                0x4000,
                0x7FFF,
                -0x4000,
                0xFF_80_01_00,
            ),
            (
                [i32::MIN, i32::MAX, 0x7FFF_F000, -0x1000],
                [i32::MAX, i32::MIN, -0x1000, 0x1000],
                i32::MIN,
                i32::MAX,
                i32::MIN,
                0x12_34_56_78,
            ),
            (
                [-0x1001, 0x1001, -0xFFF, 0xFFF],
                [0x7FFF, -0x7FFF, 0x8000, -0x8000],
                i32::MAX,
                i32::MIN,
                i32::MAX,
                u32::MAX,
            ),
        ] {
            let mut dx = dx;
            let mut dy = dy;
            let _dx = fake_data(0x00B5_CA08, dx.as_mut_ptr().cast());
            let _dy = fake_data(0x00B5_CA18, dy.as_mut_ptr().cast());
            let mut edge = [0xA5A5_A5A5u32; 20];
            edge[1] = slope as u32;
            edge[16] = fraction_x as u32;
            edge[17] = fraction_y as u32;
            let mut vertex = [0x5A5A_5A5Au32; 8];
            vertex[3] = color;
            let vertex_before = vertex;
            let mut expected = edge;
            let mut offset = [0u32; 4];
            let mut vertical = [0u32; 4];
            for i in 0..4 {
                expected[10 + i] =
                    (((i128::from(dx[i]) * i128::from(slope) + 16384) >> 15) as u32).wrapping_sub(dy[i] as u32);
                let horizontal = ((i128::from(dx[i]) >> 12) * i128::from(fraction_x) + 0x204000) as u32;
                vertical[i] = ((i128::from(dy[i]) >> 12) * i128::from(fraction_y)) as u32;
                offset[i] = (horizontal.wrapping_sub(vertical[i]) as i32 >> 3) as u32;
                expected[6 + i] = ((color >> (i * 8) & 255) << 19).wrapping_add(offset[i]);
            }
            let (mut simd, seed) = simd_state();
            let csr = mxcsr();
            let mut registers = [0u32; 17];
            registers[..8].copy_from_slice(&[
                fc4p_008dd760 as *const () as u32,
                edge.as_mut_ptr() as u32,
                vertex.as_mut_ptr() as u32,
                0x1234_5678,
                0x2345_6789,
                0x3456_789A,
                0x4567_89AB,
                0x5678_9ABC,
            ]);
            registers[16] = simd.0.as_mut_ptr() as u32;
            unsafe {
                invoke_simd(registers.as_mut_ptr());
            }
            assert_eq!(edge, expected);
            assert_eq!(vertex, vertex_before);
            assert_eq!(registers[8], vertical[0]);
            assert_eq!(registers[9], registers[3]);
            assert_eq!(registers[10], vertical[2]);
            assert_eq!(registers[11], registers[2]);
            assert_eq!(registers[12], vertical[3]);
            assert_eq!(registers[13], registers[1]);
            assert_eq!(registers[14], vertical[1]);
            assert_simd(
                &simd,
                [
                    pack(vertical[2], vertical[3]),
                    pack(expected[8], expected[9]),
                    pack(expected[6], expected[7]),
                    seed[3],
                    0,
                    pack(offset[2], offset[3]),
                    pack(vertical[0], vertical[1]),
                    pack(offset[0], offset[1]),
                ],
                csr,
            );
        }
    }

    // The harness gives the edge helper its own register convention, then records all live registers.
    #[unsafe(naked)]
    unsafe extern "fastcall" fn invoke(_: *mut u32) {
        core::arch::naked_asm!(
            "push ebp",
            "push ebx",
            "push esi",
            "push edi",
            "push ecx",
            "mov ebp, [ecx + 4]",
            "mov esi, [ecx + 8]",
            "mov ebx, [ecx + 16]",
            "mov edi, [ecx + 20]",
            "mov eax, [ecx + 24]",
            "mov edx, [ecx + 28]",
            "push dword ptr [ecx]",
            "mov ecx, [ecx + 12]",
            "call dword ptr [esp]",
            "add esp, 4",
            "push eax",
            "mov eax, [esp + 4]",
            "mov [eax + 32], edx",
            "mov [eax + 36], ecx",
            "mov [eax + 40], ebx",
            "mov [eax + 44], esi",
            "mov [eax + 48], edi",
            "mov [eax + 52], ebp",
            "pop edx",
            "mov [eax + 56], edx",
            "pop ecx",
            "pop edi",
            "pop esi",
            "pop ebx",
            "pop ebp",
            "ret"
        )
    }
    #[test]
    fn raster_depth_edge_keeps_full_signed_products_rounding_carries_and_live_registers() {
        let mut dx = 0i32;
        let mut dy = 0i32;
        let _dx = fake_data(0x00B5_CA00, (&mut dx as *mut i32).cast());
        let _dy = fake_data(0x00B5_CA04, (&mut dy as *mut i32).cast());
        for (x, y, slope, fraction_x, fraction_y, depth) in [
            (0, 0, 0, 0, 0, 0),
            (1, 1, 0x4000, 0x3FFF, 0x4000, 0xFFFF),
            (-1, 2, 0x4000, 0x4001, -0x4000, 0x8000),
            (0x8000, -0x8000, -0x10000, 0xFFFF, 1, 1),
            (i32::MIN, i32::MAX, i32::MAX, i32::MIN, i32::MAX, 0x1234),
            (i32::MAX, i32::MIN, i32::MIN, i32::MAX, i32::MIN, 0xABCD),
            (-0x4000, -0x4000, 3, -3, -3, 0xFFFF),
            (0x7FFF_FFFF, 0x7FFF_FFFE, 7, 9, 11, 0xFFFF),
        ] {
            unsafe {
                core::ptr::write_volatile(&mut dx, x);
                core::ptr::write_volatile(&mut dy, y);
            }
            let mut edge = [0xA5A5_A5A5u32; 20];
            edge[1] = slope as u32;
            edge[16] = fraction_x as u32;
            edge[17] = fraction_y as u32;
            let mut vertex = [0x5A5A_5A5Au32; 8];
            vertex[2] = 0xCDCD_0000 | depth;
            let vertex_before = vertex;
            let mut expected = edge;
            // The independent reference retains the signed 64-bit result before shifting.
            let product = |a: i32, b: i32| i64::from(a) * i64::from(b) + 16384;
            expected[15] = ((product(x, slope) >> 15) as u32).wrapping_sub(y as u32);
            expected[14] = (depth << 16)
                .wrapping_sub((product(y, fraction_y) >> 15) as u32)
                .wrapping_add((product(x, fraction_x) >> 15) as u32);
            let mut registers = [0u32; 15];
            registers[..8].copy_from_slice(&[
                fc4p_008dd620 as *const () as u32,
                edge.as_mut_ptr() as u32,
                vertex.as_mut_ptr() as u32,
                0x1234_5678,
                0x2345_6789,
                0x3456_789A,
                0x4567_89AB,
                0x5678_9ABC,
            ]);
            unsafe {
                invoke(registers.as_mut_ptr());
            }
            assert_eq!(edge, expected);
            assert_eq!(vertex, vertex_before);
            assert_eq!(registers[8], (product(x, fraction_x) >> 15) as u32);
            assert_eq!(registers[9], registers[3]);
            assert_eq!(registers[10], registers[4]);
            assert_eq!(registers[11], registers[2]);
            assert_eq!(registers[12], expected[14]);
            assert_eq!(registers[13], registers[1]);
            assert_eq!(registers[14], product(x, fraction_x) as u32);
            assert_eq!((dx, dy), (x, y));
        }
    }
    #[test]
    fn raster_depth_edge_reloads_an_aliased_step_after_writing_the_slope() {
        let mut edge = [0u32; 20];
        edge[1] = 0x10000;
        edge[15] = 0x8000;
        edge[16] = 0x4000;
        edge[17] = 0x4000;
        let mut dy = 0x4000i32;
        let _dx = fake_data(0x00B5_CA00, unsafe { edge.as_mut_ptr().add(15).cast() });
        let _dy = fake_data(0x00B5_CA04, (&mut dy as *mut i32).cast());
        let mut vertex = [0u32; 3];
        vertex[2] = 1;
        let result = unsafe { set_raster_depth_edge(edge.as_mut_ptr().cast(), vertex.as_mut_ptr().cast()) };
        assert_eq!(edge[15], 0xC000);
        assert_eq!(edge[14], 0x1_4000);
        assert_eq!(result, 0x0000_6000_3000_4000);
    }
}

#[cfg(test)]
mod font_name_tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Event {
        Select(u32, u32, u16, u32),
        Vector(u32, bool),
        Convert(u32, u32, u32),
        Free(u32),
        Find(bool),
        Erase(u32, u32),
        Trim,
    }
    struct State {
        events: Vec<Event>,
        selected_at: usize,
        selections: usize,
        wide: bool,
        extracted: u32,
        allocator: u32,
        output: u32,
        bold_marker: u32,
        positions: [u32; 2],
        erase_clears: bool,
    }
    impl Default for State {
        fn default() -> Self {
            Self {
                events: vec![],
                selected_at: usize::MAX,
                selections: 0,
                wide: false,
                extracted: 0x1234_5000,
                allocator: 0,
                output: 0,
                bold_marker: 0,
                positions: [u32::MAX; 2],
                erase_clears: false,
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }
    fn events() -> Vec<Event> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
    }
    extern "C" fn select(face: *mut u8, language: u32, kind: u32, out: *mut u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let mode = unsafe { read::<u16>(read(face, 0x180), 0x76) };
            s.events
                .push(Event::Select(language, kind, mode, unsafe { out.read() }));
            if s.selections == s.selected_at {
                unsafe {
                    write(face, if s.wide { 0x54 } else { 0x50 }, 0x1122_3344u32);
                }
            }
            s.selections += 1;
            unsafe {
                out.write(s.selections as u32);
            }
        });
        0x1111_2222_AABB_CCDD
    }
    extern "thiscall" fn wide_vector(this: *mut u32, text: *mut u8, allocator: *mut u8) -> u64 {
        make_vector(this, text, allocator, true)
    }
    extern "thiscall" fn byte_vector(this: *mut u32, text: *mut u8, allocator: *mut u8) -> u64 {
        make_vector(this, text, allocator, false)
    }
    fn make_vector(this: *mut u32, text: *mut u8, allocator: *mut u8, wide: bool) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(allocator as u32, s.allocator);
            s.events.push(Event::Vector(text as u32, wide));
            unsafe {
                this.write(s.extracted);
                this.add(1).write(s.extracted.wrapping_add(10));
                this.add(2).write(0xDEAD_BEEF);
            }
        });
        0x3333_4444_AABB_CCDD
    }
    extern "C" fn convert(text: *mut u8, count: u32, encoding: u32, out: *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(out as u32, s.output);
            s.events.push(Event::Convert(text as u32, count, encoding));
        });
        unsafe {
            write(out, 8, read::<u32>(out, 4).wrapping_add(6));
        }
        0x5555_6666_AABB_CCDD
    }
    extern "C" fn free_buffer(text: *mut u8) -> u64 {
        STATE.with(|s| s.borrow_mut().events.push(Event::Free(text as u32)));
        0x7777_8888_AABB_CCDD
    }
    extern "thiscall" fn find(_: *mut u8, text: *const u8, length: u32, start: u32) -> u64 {
        assert_eq!((length, start), (u32::MAX, 0));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let bold = text as u32 == s.bold_marker;
            s.events.push(Event::Find(bold));
            (0x9999_AAAAu64 << 32) | u64::from(s.positions[usize::from(!bold)])
        })
    }
    extern "thiscall" fn erase(this: *mut u8, start: u32, count: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.output + 4);
            s.events.push(Event::Erase(start, count));
            if s.erase_clears {
                unsafe {
                    write(this, 4, read::<u32>(this, 0));
                }
            }
        });
        0xBBBB_CCCC_AABB_CCDD
    }
    extern "thiscall" fn trim(this: *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as u32, s.output);
            s.events.push(Event::Trim);
        });
        0xDDDD_EEEE_AABB_CCDD
    }
    fn fakes() -> Vec<Fake> {
        vec![
            fake(0x0089_F353, select as *const () as usize),
            fake(0x0079_CA50, wide_vector as *const () as usize),
            fake(0x0040_8000, byte_vector as *const () as usize),
            fake(0x0091_1358, convert as *const () as usize),
            fake(0x0090_CF63, free_buffer as *const () as usize),
            fake(0x0044_25C0, erase as *const () as usize),
            fake(0x0090_E8A6, trim as *const () as usize),
        ]
    }
    #[test]
    fn font_name_without_face_keeps_string_stack_arguments_and_entry_edx() {
        reset();
        let _fakes = fakes();
        for nonempty in [false, true] {
            let mut text = [0x5Au8; 24];
            let first = text.as_mut_ptr() as u32 + 1;
            let mut output = [0xDEAD_BEEF, first, first + u32::from(nonempty), first + 20, 0xABCD_EF01];
            let before = output;
            let mut object = [0xA5A5_A5A5u32, 0, 0];
            let args = [output.as_mut_ptr() as u32, 1];
            let mut call = RawCall::new(
                Conv::Thiscall,
                fc4p_0089801e as *const () as usize,
                object.as_mut_ptr() as u32,
                &args,
            );
            call.edx = 0x1357_9BDF;
            unsafe {
                call_raw(&mut call);
            }
            assert_eq!(call.eax, (first & !255) | u32::from(nonempty));
            assert_eq!(call.edx, 0x1357_9BDF);
            assert_eq!(output, before);
            assert_eq!(text, [0x5A; 24]);
            assert!(events().is_empty());
        }
    }
    #[test]
    fn font_name_tries_each_language_and_mode_then_converts_the_selected_encoding() {
        let _fakes = fakes();
        for selected_at in 0..=8 {
            for wide in [false, true] {
                reset();
                let mut mode = [0xA5u8; 128];
                unsafe {
                    write(mode.as_mut_ptr(), 0x76, 7u16);
                }
                let mut face = [0u32; 100];
                face[0x180 / 4] = mode.as_mut_ptr() as u32;
                let mut owner = [0, 0, face.as_mut_ptr() as u32];
                let mut text = [0x5Au8; 24];
                let first = text.as_mut_ptr() as u32 + 1;
                let mut output = [0, first, first + 8, first + 20, 0xDEAD_BEEF];
                let mut stack = [0xDEAD_BEEF, output.as_mut_ptr() as u32, 99, 0xCAFE_BABE];
                let before_stack = stack;
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.selected_at = selected_at;
                    s.wide = wide;
                    s.output = output.as_ptr() as u32;
                    s.allocator = stack.as_ptr() as u32 + 7;
                });
                let returned = unsafe { read_font_name(owner.as_mut_ptr().cast(), stack.as_mut_ptr(), 0x1357_9BDF) };
                assert_eq!(returned as u32, (first & !255) | u32::from(selected_at < 8));
                assert_eq!(stack, before_stack);
                let attempts = (selected_at + 1).min(8);
                let mut expected = Vec::new();
                for i in 0..attempts {
                    expected.push(Event::Select(
                        if i % 4 < 2 { 0x409 } else { 0 },
                        if i < 4 { 99 } else { 4 },
                        if i == 0 {
                            7
                        } else if i % 2 == 1 {
                            0
                        } else {
                            1
                        },
                        i as u32,
                    ));
                }
                if selected_at < 8 {
                    expected.extend([
                        Event::Vector(0x1122_3344, wide),
                        Event::Convert(0x1234_5000, if wide { 5 } else { 10 }, if wide { 16 } else { 1252 }),
                        Event::Free(0x1234_5000),
                    ]);
                    assert_eq!(returned >> 32, 0x7777_8888);
                    assert_eq!(output[2], first + 6);
                    assert_eq!(text, [0x5A; 24]);
                } else {
                    assert_eq!(returned >> 32, 0x1111_2222);
                    assert_eq!(output[2], first);
                    assert_eq!(text[1], 0);
                    assert_eq!(text[0], 0x5A);
                    assert_eq!(&text[2..], &[0x5A; 22]);
                }
                assert_eq!(events(), expected, "selected {selected_at}, wide {wide}");
                assert_eq!(
                    unsafe { read::<u16>(mode.as_mut_ptr(), 0x76) },
                    if selected_at == 0 { 7 } else { 1 }
                );
                assert_eq!(output[3..], [first + 20, 0xDEAD_BEEF]);
            }
        }
    }
    #[test]
    fn font_family_and_style_edit_ranges_use_unsigned_positions_and_update_argument_slots() {
        let _fakes = fakes();
        let mut bold_marker = 0u8;
        let mut italic_marker = 0u8;
        let _bold = fake_data(0x00AA_1260, &mut bold_marker);
        let _italic = fake_data(0x00AA_1258, &mut italic_marker);
        for kind in [1, 2] {
            for positions in [
                [u32::MAX; 2],
                [3, u32::MAX],
                [u32::MAX, 5],
                [3, 5],
                [5, 3],
                [0x8000_0000, 0x7FFF_FFFF],
                [0, 0],
            ] {
                reset();
                let mut table = [0usize; 27];
                table[26] = find as *const () as usize;
                let mut text = [0x5Au8; 24];
                let first = text.as_mut_ptr() as u32;
                let mut output = [table.as_ptr() as u32, first, first + 8, first + 20, 0xDEAD_BEEF];
                let mut face = [0u32; 100];
                let mut mode = [0u8; 128];
                face[0x180 / 4] = mode.as_mut_ptr() as u32;
                let mut owner = [0, 0, face.as_mut_ptr() as u32];
                let mut stack = [0xDEAD_BEEF, output.as_mut_ptr() as u32, kind, 0xCAFE_BABE];
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.selected_at = 0;
                    s.output = output.as_ptr() as u32;
                    s.allocator = stack.as_ptr() as u32 + 7;
                    s.bold_marker = &bold_marker as *const u8 as u32;
                    s.positions = positions;
                    s.erase_clears = true;
                });
                let returned = unsafe { read_font_name(owner.as_mut_ptr().cast(), stack.as_mut_ptr(), 0x1357_9BDF) };
                let mut expected = vec![
                    Event::Select(0x409, kind, 0, 0),
                    Event::Vector(0x1122_3344, false),
                    Event::Convert(0x1234_5000, 10, 1252),
                    Event::Free(0x1234_5000),
                    Event::Find(true),
                    Event::Find(false),
                ];
                if kind == 1 {
                    for position in positions {
                        if position != u32::MAX {
                            expected.push(Event::Erase(position, u32::MAX));
                        }
                    }
                } else if positions != [u32::MAX; 2] {
                    expected.push(Event::Erase(0, positions[0].min(positions[1])));
                }
                expected.push(Event::Trim);
                assert_eq!(events(), expected);
                assert_eq!(returned >> 32, 0xDDDD_EEEE);
                assert_eq!(returned as u32, (first & !255) | u32::from(positions == [u32::MAX; 2]));
                assert_eq!(stack, [0xDEAD_BEEF, positions[0], positions[1], 0xCAFE_BABE]);
                assert_eq!(text, [0x5A; 24]);
            }
        }
    }
    #[test]
    fn font_name_empty_conversion_buffer_is_not_freed_and_wide_name_has_priority() {
        reset();
        let _fakes = fakes();
        let mut text = [0x5Au8; 24];
        let first = text.as_mut_ptr() as u32;
        let mut output = [0, first, first, first + 20];
        let mut face = [0u32; 100];
        face[0x50 / 4] = 0xAAAA_BBBB;
        face[0x54 / 4] = 0xCCCC_DDDD;
        let mut mode = [0u8; 128];
        face[0x180 / 4] = mode.as_mut_ptr() as u32;
        let mut owner = [0, 0, face.as_mut_ptr() as u32];
        let mut stack = [0, output.as_mut_ptr() as u32, 99];
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.extracted = 0;
            s.output = output.as_ptr() as u32;
            s.allocator = stack.as_ptr() as u32 + 7;
        });
        let returned = unsafe { read_font_name(owner.as_mut_ptr().cast(), stack.as_mut_ptr(), 0) };
        assert_eq!(returned >> 32, 0x5555_6666);
        assert_eq!(
            events(),
            vec![
                Event::Select(0x409, 99, 0, 0),
                Event::Vector(0xCCCC_DDDD, true),
                Event::Convert(0, 5, 16)
            ]
        );
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Event {
        Prime(u32),
        Vector(u32, u32, u32),
        Free(u32),
        Begin(u32),
        Next(u32),
        Erase(u32),
        Push(u32, [u32; 3]),
        Reserve(u32),
        Insert([u32; 5]),
    }
    #[derive(Default)]
    struct State {
        events: Vec<Event>,
        allocated: Vec<Box<[u32]>>,
        prime_result: u32,
        begin: u32,
        next: u32,
        pushed_block: u32,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }
    fn event(e: Event) {
        STATE.with(|s| s.borrow_mut().events.push(e));
    }
    fn events() -> Vec<Event> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
    }
    extern "C" fn prime(first: *const u32, last: *const u32, key: *const u32, count: u32, unused: u32) -> *const u32 {
        assert_eq!(unsafe { key.read() }, count);
        assert_eq!(unused, 0);
        assert_eq!(last as u32 - first as u32, 12);
        event(Event::Prime(count));
        STATE.with(|s| s.borrow().prime_result as *const u32)
    }
    extern "thiscall" fn vector(this: *mut u32, count: u32, fill: *const u32, allocator: *const u32) -> u32 {
        let fill = unsafe { fill.read() };
        let allocator = unsafe { allocator.read() };
        event(Event::Vector(count, fill, allocator));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let mut memory = vec![fill; if count == 0xFFFF_FFFB { 1 } else { count as usize }].into_boxed_slice();
            let start = memory.as_mut_ptr() as u32;
            unsafe {
                this.write(start);
                this.add(1).write(start.wrapping_add(count.wrapping_mul(4)));
                this.add(2)
                    .write(start.wrapping_add(count.wrapping_mul(4).wrapping_add(16)));
            }
            s.allocated.push(memory);
        });
        this as u32
    }
    extern "C" fn free_old(p: *mut u8) -> u32 {
        event(Event::Free(p as u32));
        0xABCD_1234
    }
    extern "thiscall" fn begin(this: *mut u8, out: *mut u32) -> u32 {
        event(Event::Begin(this as u32));
        unsafe {
            out.write(STATE.with(|s| s.borrow().begin));
            out.add(1).write(this as u32);
        }
        out as u32
    }
    extern "thiscall" fn next(iterator: *mut u32) -> *mut u8 {
        event(Event::Next(unsafe { iterator.read() }));
        STATE.with(|s| s.borrow().next as *mut u8)
    }
    extern "thiscall" fn erase(this: *mut u8, iterator: *const u32) -> u32 {
        assert_eq!(unsafe { iterator.add(1).read() }, this as u32);
        event(Event::Erase(unsafe { iterator.read() }));
        unsafe {
            write(this, 0x10, read::<u32>(this, 0x10).wrapping_sub(1));
        }
        0
    }
    extern "thiscall" fn push(this: *mut u8, value: *mut u8) -> u32 {
        let data = unsafe { [read(value, 0), read(value, 4), read(value, 8)] };
        event(Event::Push(this as u32, data));
        let next = STATE.with(|s| s.borrow().pushed_block);
        unsafe {
            let destination = read::<*mut u8>(this, 0x10);
            for (i, word) in data.into_iter().enumerate() {
                write(destination, i * 4, word);
            }
            let map = read::<*mut u8>(this, 0x1C).wrapping_add(4);
            write(map, 0, next);
            write(this, 0x1C, map);
            write(this, 0x14, next);
            write(this, 0x18, next.wrapping_add(120));
            write(this, 0x10, next);
        }
        0
    }
    extern "thiscall" fn reserve(_: *mut u8, count: u32) -> u32 {
        event(Event::Reserve(count));
        0
    }
    extern "thiscall" fn insert(_: *mut u8, out: *mut u32, pair: *const u32) -> u64 {
        event(Event::Insert(unsafe {
            [
                pair.read(),
                pair.add(1).read(),
                pair.add(2).read(),
                pair.add(3).read(),
                pair.add(4).read(),
            ]
        }));
        unsafe {
            out.write(0x1122_3344);
            out.add(1).write(0x5566_7788);
            *out.cast::<u8>().add(8) = 1;
        }
        0xFACE_B00C_ABCD_EF01
    }
    fn recycle_fakes() -> Vec<Fake> {
        vec![
            fake(0x004B_0390, begin as *const () as usize),
            fake(0x008E_BAC0, next as *const () as usize),
            fake(0x008E_B9E0, erase as *const () as usize),
            fake(0x008E_BD70, push as *const () as usize),
            fake(0x008E_BB70, reserve as *const () as usize),
            fake(0x008E_B920, insert as *const () as usize),
            fake(0x0090_CF63, free_old as *const () as usize),
        ]
    }
    fn deque(cache: &mut [u32; 24], block: &mut [u32; 30], map: &mut [u32; 3], used: u32) {
        let first = block.as_mut_ptr() as u32;
        map[0] = first;
        cache[5..9].copy_from_slice(&[first, first, first + 120, map.as_mut_ptr() as u32]);
        cache[9..13].copy_from_slice(&[first + used * 12, first, first + 120, map.as_mut_ptr() as u32]);
    }
    #[test]
    fn bucket_growth_reverses_collisions_wraps_hashes_and_only_changes_bucket_fields() {
        reset();
        let mut primes = [3, 7, 11];
        let _start = fake_data(0x00A8_29B0, primes.as_mut_ptr().cast());
        let _end = fake_data(0x00A8_2A20, unsafe { primes.as_mut_ptr().add(3).cast() });
        let _prime = fake(0x0079_FA90, prime as *const () as usize);
        let _vector = fake(0x0051_C950, vector as *const () as usize);
        let _free = fake(0x0090_CF63, free_old as *const () as usize);
        STATE.with(|s| s.borrow_mut().prime_result = primes.as_ptr() as u32 + 4);
        let mut a = [0xA5A5_A5A5u32; 8];
        let mut b = [0x5A5A_5A5Au32; 8];
        let ap = a.as_mut_ptr() as u32;
        let bp = b.as_mut_ptr() as u32;
        a[0] = bp;
        a[2] = u32::MAX;
        a[3] = 8;
        b[0] = 0;
        b[2] = 3;
        b[3] = 4;
        let mut buckets = [ap, 0, 0];
        let old = buckets.as_mut_ptr() as u32;
        let mut object = [0xCDCD_CDCDu32; 24];
        object[1..4].copy_from_slice(&[old, old + 12, old + 12]);
        let before = object;
        let mut stack = [0xDEAD_BEEF, 4, 0xCAFE_BABE];
        assert_eq!(
            unsafe { grow_resource_buckets(object.as_mut_ptr().cast(), stack.as_mut_ptr(), 4) },
            0xABCD_1234
        );
        assert_eq!(stack, [0xDEAD_BEEF, 7, 0xCAFE_BABE]);
        assert_eq!(buckets, [0; 3]);
        assert_eq!(a[0], 0);
        assert_eq!(b[0], ap);
        STATE.with(|s| assert_eq!(&*s.borrow().allocated[0], &[bp, 0, 0, 0, 0, 0, 0]));
        assert_eq!(object[2], object[1] + 28);
        assert_eq!(object[3], object[1] + 44);
        for i in (0..24).filter(|&i| !(1..4).contains(&i)) {
            assert_eq!(object[i], before[i]);
        }
        assert_eq!(
            events(),
            vec![Event::Prime(4), Event::Vector(7, 0, 7), Event::Free(old)]
        );
        // A caller uses the exported thiscall entry, whose single stack word is popped.
        let args = [7];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_008ebb70 as *const () as usize,
            object.as_mut_ptr() as u32,
            &args,
        );
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!(call.eax, 7);
        assert!(events().is_empty());
    }
    #[test]
    fn bucket_growth_handles_no_growth_prime_and_empty_old_storage() {
        for old_count in [0, 7] {
            reset();
            let mut primes = [3, 7, 11];
            let _start = fake_data(0x00A8_29B0, primes.as_mut_ptr().cast());
            let _end = fake_data(0x00A8_2A20, unsafe { primes.as_mut_ptr().add(3).cast() });
            let _prime = fake(0x0079_FA90, prime as *const () as usize);
            let _vector = fake(0x0051_C950, vector as *const () as usize);
            STATE.with(|s| s.borrow_mut().prime_result = primes.as_ptr() as u32 + 4);
            let mut object = [0xCDCD_CDCDu32; 24];
            object[1] = 0;
            object[2] = old_count * 4;
            let before = object;
            let mut stack = [0, old_count + 1];
            let result = unsafe { grow_resource_buckets(object.as_mut_ptr().cast(), stack.as_mut_ptr(), 0) };
            if old_count == 7 {
                assert_eq!(result, 7);
                assert_eq!(object, before);
                assert_eq!(events(), vec![Event::Prime(8)]);
            } else {
                assert_eq!(result, 0);
                assert_eq!(events(), vec![Event::Prime(1), Event::Vector(7, 0, 7)]);
            }
            assert_eq!(stack[1], 7);
        }
    }
    #[test]
    fn bucket_growth_uses_the_terminal_bucket_count_after_the_prime_table() {
        reset();
        let mut primes = [3, 7, 11];
        let end = unsafe { primes.as_mut_ptr().add(3) };
        let _start = fake_data(0x00A8_29B0, primes.as_mut_ptr().cast());
        let _end = fake_data(0x00A8_2A20, end.cast());
        let _prime = fake(0x0079_FA90, prime as *const () as usize);
        let _vector = fake(0x0051_C950, vector as *const () as usize);
        STATE.with(|s| s.borrow_mut().prime_result = end as u32);
        let mut object = [0u32; 24];
        let mut stack = [0xDEAD_BEEF, u32::MAX, 0xCAFE_BABE];
        assert_eq!(
            unsafe { grow_resource_buckets(object.as_mut_ptr().cast(), stack.as_mut_ptr(), 0) },
            0
        );
        assert_eq!(stack, [0xDEAD_BEEF, 0xFFFF_FFFB, 0xCAFE_BABE]);
        assert_eq!(
            events(),
            vec![Event::Prime(u32::MAX), Event::Vector(0xFFFF_FFFB, 0, 0xFFFF_FFFB)]
        );
        assert_eq!(object[2], object[1].wrapping_sub(20));
        assert_eq!(object[3], object[1].wrapping_sub(4));
    }
    #[test]
    fn recycle_uses_last_free_entry_preserves_cache_and_passes_new_payload() {
        reset();
        let _fakes = recycle_fakes();
        let mut block = [0xCDCD_CDCDu32; 30];
        block[3..6].copy_from_slice(&[0x1000, 55, 0xAAAA]);
        let mut map = [0u32; 3];
        let mut cache = [0xA5A5_A5A5u32; 24];
        deque(&mut cache, &mut block, &mut map, 2);
        cache[4] = u32::MAX;
        let mut expected = cache;
        expected[9] -= 12;
        let result = unsafe { fc4p_008ebe30(cache.as_mut_ptr().cast(), 0x1122, 0x3344, 0x7788) };
        assert_eq!(result, 0xFACE_B00C_0000_1000);
        assert_eq!(cache, expected);
        assert_eq!(
            events(),
            vec![Event::Reserve(0), Event::Insert([0x1122, 0x3344, 0x1000, 55, 0x7788])]
        );
    }
    #[test]
    fn recycle_age_scan_skips_the_successor_of_each_erased_node() {
        reset();
        let _fakes = recycle_fakes();
        let mut block = [0u32; 30];
        let mut map = [0u32; 3];
        let mut cache = [0xA5A5_A5A5u32; 24];
        deque(&mut cache, &mut block, &mut map, 0);
        cache[4] = 5;
        cache[16] = 10;
        let mut nodes = [[0u32; 8]; 5];
        let pointers = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect::<Vec<_>>();
        for i in 0..5 {
            nodes[i][0] = pointers.get(i + 1).copied().unwrap_or(0);
            nodes[i][4] = 100 + i as u32;
            nodes[i][5] = [8, 7, 0, 1, u32::MAX][i];
            nodes[i][6] = 200 + i as u32;
        }
        STATE.with(|s| s.borrow_mut().begin = pointers[0]);
        let result = unsafe { fc4p_008ebe30(cache.as_mut_ptr().cast(), 11, 12, 13) };
        assert_eq!(result as u32, 103);
        assert_eq!(cache[4], 3);
        assert_eq!(cache[22], 0);
        assert_eq!(&block[..6], &[101, 7, 201, 103, 1, 203]);
        assert_eq!(cache[9], block.as_ptr() as u32 + 12);
        assert_eq!(
            events(),
            vec![
                Event::Begin(cache.as_ptr() as u32),
                Event::Erase(pointers[1]),
                Event::Erase(pointers[3]),
                Event::Next(pointers[4]),
                Event::Reserve(4),
                Event::Insert([11, 12, 103, 1, 13])
            ]
        );
    }
    #[test]
    fn recycle_falls_back_after_unsigned_age_scan_and_queries_first_entry_twice() {
        reset();
        let _fakes = recycle_fakes();
        let mut block = [0u32; 30];
        let mut map = [0u32; 3];
        let mut cache = [0xA5A5_A5A5u32; 24];
        deque(&mut cache, &mut block, &mut map, 0);
        cache[4] = 1;
        cache[16] = 1;
        let mut node = [0, 0, 3, 4, 123, u32::MAX, 456, 0];
        let np = node.as_mut_ptr() as u32;
        STATE.with(|s| s.borrow_mut().begin = np);
        assert_eq!(
            unsafe { fc4p_008ebe30(cache.as_mut_ptr().cast(), 11, 12, 13) } as u32,
            123
        );
        assert_eq!(
            events(),
            vec![
                Event::Begin(cache.as_ptr() as u32),
                Event::Next(np),
                Event::Begin(cache.as_ptr() as u32),
                Event::Begin(cache.as_ptr() as u32),
                Event::Erase(np),
                Event::Reserve(1),
                Event::Insert([11, 12, 123, u32::MAX, 13])
            ]
        );
        assert_eq!(&block[..3], &[123, u32::MAX, 456]);
        assert_eq!(cache[22], 0);
    }
    #[test]
    fn recycle_append_and_pop_cross_deque_block_boundaries() {
        reset();
        let _fakes = recycle_fakes();
        let mut block = [0u32; 30];
        let mut new_block = [0u32; 30];
        let mut map = [0u32; 3];
        let mut cache = [0xA5A5_A5A5u32; 24];
        deque(&mut cache, &mut block, &mut map, 9);
        // The deque is empty at its final slot, so the collected value causes a new block.
        cache[5] = cache[9];
        cache[4] = 1;
        cache[16] = 10;
        let mut node = [0, 0, 3, 4, 123, 0, 456, 0];
        let np = node.as_mut_ptr() as u32;
        let new = new_block.as_mut_ptr() as u32;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.begin = np;
            s.pushed_block = new;
        });
        assert_eq!(
            unsafe { fc4p_008ebe30(cache.as_mut_ptr().cast(), 11, 12, 13) } as u32,
            123
        );
        assert_eq!(
            events(),
            vec![
                Event::Begin(cache.as_ptr() as u32),
                Event::Next(np),
                Event::Push(cache.as_ptr() as u32 + 0x14, [123, 0, 456]),
                Event::Erase(np),
                Event::Free(new),
                Event::Reserve(1),
                Event::Insert([11, 12, 123, 0, 13])
            ]
        );
        assert_eq!(cache[9], block.as_ptr() as u32 + 108);
        assert_eq!(cache[10], block.as_ptr() as u32);
        assert_eq!(cache[11], block.as_ptr() as u32 + 120);
        assert_eq!(cache[12], map.as_ptr() as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Allocate(u32),
        Free(u32),
        Provider,
        Query(u32, u32, u32),
        ReleaseReference(u32),
        FreeNode(u32, u32),
        Precision(u32),
        SetPrecision(u32),
        Lua(u32, u32, u32, bool),
        AddRef(u32),
        Destroy(u32, u32),
        Find(u32, [u32; 2]),
        Pixels(u32, u32, u32, [u32; 4]),
    }
    #[derive(Default)]
    struct State {
        events: Vec<Event>,
        allocations: Vec<Box<[u8]>>,
        precision: u32,
        provider: u32,
        service: u32,
        global: u32,
        find: u32,
        changed_pair: u32,
        changed_thread: u32,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }
    fn events() -> Vec<Event> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
    }
    fn event(event: Event) {
        STATE.with(|s| s.borrow_mut().events.push(event));
    }
    fn address<T>(value: T) -> usize
    where
        T: Copy,
    {
        assert_eq!(core::mem::size_of::<T>(), 4);
        unsafe { core::mem::transmute_copy(&value) }
    }
    fn allocator() -> Fake {
        fake(0x0090_CF54, allocate_fake as *const () as usize)
    }
    extern "C" fn allocate_fake(bytes: u32) -> *mut u8 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.events.push(Event::Allocate(bytes));
            if s.changed_pair != 0 {
                unsafe { write(s.changed_pair as *mut u8, 8, 0x1234_5678u32) };
            }
            let mut allocation = vec![0xCD; bytes as usize].into_boxed_slice();
            let p = allocation.as_mut_ptr();
            s.allocations.push(allocation);
            p
        })
    }
    extern "C" fn free_fake(p: *mut u8) -> u32 {
        event(Event::Free(p as u32));
        0xAABB_CCDD
    }
    extern "C" fn provider_fake() -> *mut u8 {
        event(Event::Provider);
        STATE.with(|s| s.borrow().provider as *mut u8)
    }
    extern "thiscall" fn query_fake(this: *mut u8, class: u32, iid: u32, out: *mut *mut u8) -> u32 {
        event(Event::Query(this as u32, class, iid));
        unsafe { out.write(STATE.with(|s| s.borrow().service as *mut u8)) };
        1
    }
    extern "thiscall" fn release_reference_fake(reference: *mut *mut u8) -> u32 {
        let value = unsafe { reference.read() };
        event(Event::ReleaseReference(value as u32));
        STATE.with(|s| assert_eq!(unsafe { *(s.borrow().global as *const u32) }, value as u32));
        unsafe { reference.write(core::ptr::null_mut()) };
        0
    }
    extern "thiscall" fn free_node_fake(this: *mut u8, node: *mut u8) -> u32 {
        event(Event::FreeNode(this as u32, node as u32));
        assert_eq!(unsafe { read::<u32>(node, 0x28) }, 0x00A8_0810);
        assert_eq!(unsafe { read::<u32>(node, 0x0C) }, 0x00A8_0810);
        0
    }

    #[test]
    fn lua_owner_reference_counts_wrap_and_adjust_secondary_interface() {
        let mut object = Object::new(0x30);
        let p = object.ptr();
        let mut call = RawCall::new(Conv::Thiscall, fc4p_008ae800 as *const () as usize, p as u32, &[]);
        call.edx = 0xF123_4567;
        for count in [0, 1, 0x7FFF_FFFF, u32::MAX] {
            unsafe {
                write(p, 0x20, count);
                call_raw(&mut call);
            }
            assert_eq!(call.eax, count.wrapping_add(1));
            assert_eq!(call.edx, 0xF123_4567);
            assert_eq!(unsafe { read::<u32>(p, 0x20) }, count.wrapping_add(1));
        }
        let _fake = fake(0x008A_E800, add_owner_fake as *const () as usize);
        reset();
        let value = unsafe { fc4p_008b03d0(p.add(8), 0x1122_3344) };
        assert_eq!(value, 0x1122_3344_8765_4321);
        assert_eq!(events(), vec![Event::AddRef(p as u32)]);
    }
    extern "fastcall" fn add_owner_fake(this: *mut u8, edx: u32) -> u64 {
        event(Event::AddRef(this as u32));
        (u64::from(edx) << 32) | 0x8765_4321
    }

    #[test]
    fn parameter_release_destroys_zero_and_one_and_preserves_return_edx() {
        reset();
        let mut table = [0usize; 16];
        table[15] = destroy_fake as *const () as usize;
        let mut object = [table.as_ptr() as u32, 0u32, 0xABCD_EF01];
        let p = object.as_mut_ptr().cast::<u8>();
        for count in [0, 1, 2, u32::MAX] {
            object[1] = count;
            let result = unsafe { fc4p_008d29b9(p, 0x1234_5678) };
            if count > 1 {
                assert_eq!(result, (0x1234_5678u64 << 32) | u64::from(count - 1));
                assert_eq!(object[1], count - 1);
                assert!(events().is_empty());
            } else {
                assert_eq!(result, 0xDEAD_BEEF_0000_0000);
                assert_eq!(object[1], count);
                assert_eq!(events(), vec![Event::Destroy(p as u32, 1)]);
            }
            assert_eq!(object[2], 0xABCD_EF01);
        }
    }
    extern "thiscall" fn destroy_fake(this: *mut u8, flags: u32) -> u64 {
        event(Event::Destroy(this as u32, flags));
        0xDEAD_BEEF_AABB_CCDD
    }

    #[test]
    fn string_pair_list_clears_empty_and_nonempty_and_lazily_acquires_allocator() {
        for lazy in [false, true] {
            reset();
            let _free = fake(0x0090_CF63, free_fake as *const () as usize);
            let _provider = fake(0x0087_93EC, provider_fake as *const () as usize);
            let _release = fake(0x0041_1870, release_reference_fake as *const () as usize);
            let mut provider_table = [0usize; 6];
            provider_table[5] = query_fake as *const () as usize;
            let mut provider = [provider_table.as_ptr() as u32];
            let mut service_table = [0usize; 5];
            service_table[4] = free_node_fake as *const () as usize;
            let mut service = [service_table.as_ptr() as u32];
            let service_p = service.as_mut_ptr().cast::<u8>();
            let mut global = if lazy { core::ptr::null_mut() } else { service_p };
            let _data = fake_data(0x00B5_C42C, (&mut global as *mut *mut u8).cast());
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.provider = provider.as_mut_ptr() as u32;
                s.service = service_p as u32;
                s.global = &mut global as *mut *mut u8 as u32;
            });
            let mut header = [0u32; 2];
            let hp = header.as_mut_ptr().cast::<u8>();
            let mut list = [hp as u32];
            let lp = list.as_mut_ptr().cast::<u8>();
            header.fill(hp as u32);
            assert_eq!(unsafe { fc4p_008b15e0(lp) }, hp as u32);
            assert!(events().is_empty());
            let mut first = [0xA5u8; 0x48];
            let a = first.as_mut_ptr();
            let mut second = [0x5Au8; 0x48];
            let b = second.as_mut_ptr();
            unsafe {
                write(hp, 0, a);
                write(hp, 4, b);
                write(a, 0, b);
                write(b, 0, hp);
                write(a, 0x2C, 0x1000u32);
                write(a, 0x10, 0x2000u32);
                write(b, 0x2C, 0u32);
                write(b, 0x10, 0x3000u32);
                fc4p_008b15e0(lp);
            }
            assert_eq!(header, [hp as u32; 2]);
            let mut expected = vec![Event::Free(0x1000), Event::Free(0x2000)];
            if lazy {
                expected.extend([
                    Event::Provider,
                    Event::Query(provider.as_ptr() as u32, 0x3AE4_BEA3, 0x3AE4_BEAB),
                    Event::ReleaseReference(service_p as u32),
                ]);
            }
            expected.extend([
                Event::FreeNode(service_p as u32, a as u32),
                Event::Free(0x3000),
                Event::FreeNode(service_p as u32, b as u32),
            ]);
            assert_eq!(events(), expected);
            assert_eq!(&first[0x30..], &[0xA5; 0x18]);
            assert_eq!(&second[0x30..], &[0x5A; 0x18]);
        }
    }

    extern "C" fn precision_fake() -> u32 {
        let mode = STATE.with(|s| s.borrow().precision);
        event(Event::Precision(mode));
        mode
    }
    #[unsafe(naked)]
    unsafe extern "C" fn set_precision_fake() {
        core::arch::naked_asm!("push eax", "push dword ptr [esp + 8]", "call {log}", "add esp, 4", "pop eax", "ret", log = sym set_precision_log)
    }
    extern "C" fn set_precision_log(mode: u32) {
        event(Event::SetPrecision(mode));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.precision = mode;
            if s.changed_thread != 0 {
                unsafe { write(s.changed_thread as *mut u8, 8, 0x4455_6677u32) };
            }
        });
        let word = crate::x87::control_word() & !0x300;
        crate::x87::set_control_word(word | if mode == 1 { 0x200 } else { 0 });
    }
    extern "C" fn lua_call_fake(state: *mut u8, a: u32, b: u32) -> u32 {
        lua_fake(state, a, b, false)
    }
    extern "C" fn lua_tag_fake(state: *mut u8, a: u32, b: u32) -> u32 {
        lua_fake(state, a, b, true)
    }
    fn lua_fake(state: *mut u8, a: u32, b: u32, tag: bool) -> u32 {
        event(Event::Lua(state as u32, a, b, tag));
        if STATE.with(|s| s.borrow().precision) == 1 {
            assert_eq!(crate::x87::control_word() & 0x300, 0x200);
        }
        0x89AB_CDEF
    }
    #[test]
    fn lua_wrappers_restore_single_precision_and_keep_full_call_result() {
        let _fpu = GameFpu::new();
        let _get = fake(0x0089_2A0A, precision_fake as *const () as usize);
        let _set = fake(0x0089_29E0, set_precision_fake as *const () as usize);
        let _call = fake(0x008B_B3E0, lua_call_fake as *const () as usize);
        let _tag = fake(0x008B_F100, lua_tag_fake as *const () as usize);
        for mode in [0, 1, 2] {
            for tag in [false, true] {
                reset();
                let mut thread = [0u32, 0, 0x1122_3344];
                let p = thread.as_mut_ptr().cast::<u8>();
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.precision = mode;
                    s.changed_thread = p as u32;
                });
                crate::x87::set_control_word(
                    0x7F | match mode {
                        1 => 0x200,
                        2 => 0x300,
                        _ => 0,
                    },
                );
                let before = crate::x87::control_word();
                let result = unsafe {
                    if tag {
                        fc4p_008b87a0(p, 0xFFFF_FFFF, 0x8000_0000)
                    } else {
                        fc4p_008b7f40(p, 0xFFFF_FFFF, 0x8000_0000)
                    }
                };
                assert_eq!(result, 0x89AB_CDEF);
                assert_eq!(crate::x87::control_word(), before);
                let mut expected = vec![Event::Precision(mode)];
                if mode == 0 {
                    expected.push(Event::SetPrecision(1));
                }
                expected.push(Event::Lua(
                    if mode == 0 { 0x4455_6677 } else { 0x1122_3344 },
                    u32::MAX,
                    0x8000_0000,
                    tag,
                ));
                if mode == 0 {
                    expected.push(Event::SetPrecision(0));
                }
                assert_eq!(events(), expected);
            }
        }
    }

    extern "thiscall" fn interface_add_fake(this: *mut u8) -> u32 {
        event(Event::AddRef(this as u32));
        // Null interface arithmetic is checked without dereferencing its adjusted address.
        if this as u32 == 12 {
            return 0xCAFE_BABE;
        }
        unsafe {
            let value = read::<u32>(this, 4).wrapping_add(1);
            write(this, 4, value);
            value
        }
    }
    #[test]
    fn driver_query_interfaces_preserve_failure_output_and_return_upper_bytes() {
        reset();
        let _add = fake(0x0096_8B60, interface_add_fake as *const () as usize);
        let mut object = [0xA5A5_A5A5u32; 8];
        let p = object.as_mut_ptr().cast::<u8>();
        for (iid, offset) in [(1, 8), (0xA455_4849, 8), (0x6695_65FE, 0), (0xE69B_FE2A, 4)] {
            object[4] = 0x1122_33FF;
            let mut out = 0xDEAD_BEEF as *mut u8;
            assert_eq!(unsafe { fc4p_008dc4c0(p, iid, &mut out) }, 0x1122_3401);
            assert_eq!(out, p.wrapping_add(offset));
            assert_eq!(object[4], 0x1122_3400);
            assert_eq!(events(), vec![Event::AddRef(p as u32 + 12)]);
            let result = unsafe { fc4p_008dc4c0(core::ptr::null_mut(), iid, &mut out) };
            assert_eq!(result, 0xCAFE_BA01);
            assert!(out.is_null());
            assert_eq!(events(), vec![Event::AddRef(12)]);
        }
        for (iid, expected) in [
            (0, 0x996A_9A00),
            (2, 0x996A_9A00),
            (0x6695_65FD, 0xFFFF_FF00),
            (0xA455_4848, 0x3DBF_E200),
            (u32::MAX, 0xFFFF_FF00),
        ] {
            let mut out = 0xDEAD_BEEF as *mut u8;
            assert_eq!(unsafe { fc4p_008dc4c0(p, iid, &mut out) }, expected, "{iid:08X}");
            assert_eq!(out as u32, 0xDEAD_BEEF);
            assert!(events().is_empty());
        }
    }

    #[test]
    fn block_expansion_covers_indices_stride_padding_and_unaligned_pixels() {
        let palette = [0xFF12_3456, 0xFFAA_55FF, 0x0000_00AA, 0];
        let indices = 0x1BE4_72C9u32;
        let mut output = [0xCDu8; 105];
        unsafe {
            fc4p_008d0fd0(output.as_mut_ptr().add(1), 24, indices, palette.as_ptr());
        }
        let mut expected = [0xCDu8; 105];
        for i in 0..16 {
            let at = 1 + (i / 4) * 24 + (i % 4) * 4;
            expected[at..at + 4].copy_from_slice(&palette[((indices >> (i * 2)) & 3) as usize].to_le_bytes());
        }
        assert_eq!(output, expected);
    }
    #[test]
    fn block_expansion_loads_both_colors_before_each_pair_of_writes() {
        let mut words = [0x10u32, 0x20, 0x30, 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut expected = words;
        let indices = 0x0123_4567u32;
        let mut indices_left = indices;
        for pair in 0..8 {
            let a = expected[(indices_left & 3) as usize];
            let b = expected[((indices_left >> 2) & 3) as usize];
            expected[pair * 2] = a;
            expected[pair * 2 + 1] = b;
            indices_left >>= 4;
        }
        let p = words.as_mut_ptr();
        unsafe {
            fc4p_008d0fd0(p.cast(), 16, indices, p);
        }
        assert_eq!(words, expected);
    }
    extern "stdcall" fn pixels_fake(destination: *mut u8, stride: u32, indices: u32, palette: *const u32) -> u32 {
        let palette = unsafe {
            [
                palette.read(),
                palette.add(1).read(),
                palette.add(2).read(),
                palette.add(3).read(),
            ]
        };
        event(Event::Pixels(destination as u32, stride, indices, palette));
        0x1234_5678
    }
    #[test]
    fn dxt1_region_palette_modes_row_skips_and_caller_argument_write() {
        reset();
        let _pixels = fake(0x008D_0FD0, pixels_fake as *const () as usize);
        let source = [
            0x0000_F800u32,
            0x1111_1111,
            0xFFFF_0000,
            0x2222_2222,
            0,
            0,
            0x07E0_07E0,
            0x3333_3333,
            0x001F_FFFF,
            0x4444_4444,
            0,
            0,
        ];
        let mut output = [0u8; 160];
        let p = output.as_mut_ptr();
        let mut stack = [0, p as u32, 20, source.as_ptr() as u32, 3, 2, 2];
        assert_eq!(unsafe { decode_dxt1_region(stack.as_mut_ptr(), 0xDEAD_BEEF) }, 0);
        assert_eq!(stack[6], 0);
        assert_eq!(
            events(),
            vec![
                Event::Pixels(
                    p as u32,
                    20,
                    0x1111_1111,
                    [0xFFFF_0000, 0xFF00_0000, 0xFFAA_0000, 0xFF55_0000]
                ),
                Event::Pixels(
                    p as u32 + 16,
                    20,
                    0x2222_2222,
                    [0xFF00_0000, 0xFFFF_FFFF, 0xFF80_8080, 0]
                ),
                Event::Pixels(
                    p as u32 + 80,
                    20,
                    0x3333_3333,
                    [0xFF00_FF00, 0xFF00_FF00, 0xFF00_FF00, 0]
                ),
                Event::Pixels(
                    p as u32 + 96,
                    20,
                    0x4444_4444,
                    [0xFFFF_FFFF, 0xFF00_00FF, 0xFFAA_AAFF, 0xFF55_55FF]
                ),
            ]
        );
        // Also enter through the cdecl export and check the no-op paths' EAX.
        for (columns, rows, expected) in [(0, 7, 0xABCD_EF01), (3, 0, 0)] {
            let args = [0, 0, 0, 0, columns, rows];
            let mut call = RawCall::new(Conv::Cdecl, fc4p_008d1030 as *const () as usize, 0, &args);
            call.eax = 0xABCD_EF01;
            unsafe { call_raw(&mut call) };
            assert_eq!(call.eax, expected);
        }
    }
    #[test]
    fn rgb565_all_endpoints_replicate_bits_and_interpolation_rounds_by_256() {
        for v in 0..=u16::MAX {
            let r = u32::from((v >> 11) & 31);
            let g = u32::from((v >> 5) & 63);
            let b = u32::from(v & 31);
            assert_eq!(
                rgb565(v),
                0xFF00_0000 | ((r << 3 | r >> 2) << 16) | ((g << 2 | g >> 4) << 8) | (b << 3 | b >> 2)
            );
        }
        for a in 0..=255u32 {
            for b in 0..=255u32 {
                assert_eq!(interpolate(a, b) & 255, (a * 171 + b * 85 + 128) >> 8);
            }
        }
    }

    fn cache_with_buckets(cache: &mut Object, buckets: &mut [u32]) -> *mut u8 {
        let p = cache.ptr();
        unsafe {
            write(p, 4, buckets.as_mut_ptr());
            write(p, 8, buckets.as_mut_ptr().add(buckets.len()));
        }
        p
    }
    #[test]
    fn hash_lookup_handles_empty_collision_chains_both_key_halves_and_wrap() {
        let mut cache = Object::new(0x60);
        let mut buckets = [0u32; 3];
        let p = cache_with_buckets(&mut cache, &mut buckets);
        let mut a = [0u32, 0xA5, u32::MAX, 2, 0, 0, 0, 0];
        let mut b = [0u32, 0xB5, u32::MAX, 5, 0, 0, 0, 0];
        let mut c = [0u32, 0xC5, 2, 2, 0, 0, 0, 0];
        a[0] = b.as_mut_ptr() as u32;
        b[0] = c.as_mut_ptr() as u32;
        unsafe {
            write(buckets.as_mut_ptr().cast(), 4, a.as_mut_ptr());
        }
        for (key, expected) in [
            ([u32::MAX, 2], a.as_mut_ptr()),
            ([u32::MAX, 5], b.as_mut_ptr()),
            ([2, 2], c.as_mut_ptr()),
            ([1, 0], core::ptr::null_mut()),
            ([0, 0], core::ptr::null_mut()),
        ] {
            assert_eq!(unsafe { fc4p_008eb8d0(p, key.as_ptr()) }, expected.cast::<u8>());
        }
    }
    #[test]
    fn hash_insert_reuses_duplicates_and_copies_payload_after_allocation() {
        reset();
        let _allocator = allocator();
        let mut cache = Object::new(0x60);
        let mut buckets = [0u32; 3];
        let p = cache_with_buckets(&mut cache, &mut buckets);
        let mut old = [0u32, 0, 1, 2, 3, 4, 5, 6];
        buckets[0] = old.as_mut_ptr() as u32;
        let mut pair = [1u32, 2, 99, 100, 101];
        let mut result = [0xA5u8; 12];
        let before = unsafe { core::slice::from_raw_parts(p, 0x60).to_vec() };
        assert_eq!(
            unsafe { fc4p_008eb920(p, result.as_mut_ptr(), pair.as_mut_ptr().cast()) },
            result.as_mut_ptr()
        );
        assert_eq!(unsafe { read::<u32>(result.as_mut_ptr(), 0) }, old.as_ptr() as u32);
        assert_eq!(result[8..], [0, 0xA5, 0xA5, 0xA5]);
        assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x60) }, before);
        assert!(events().is_empty());
        pair[0] = 4;
        STATE.with(|s| s.borrow_mut().changed_pair = pair.as_mut_ptr() as u32);
        unsafe {
            write(p, 16, u32::MAX);
            fc4p_008eb920(p, result.as_mut_ptr(), pair.as_mut_ptr().cast());
        }
        let node = buckets[0] as *mut u8;
        assert_eq!(events(), vec![Event::Allocate(32)]);
        assert_eq!(unsafe { read::<u32>(node, 0) }, old.as_ptr() as u32);
        assert_eq!(unsafe { read::<u32>(node, 4) }, 0xCDCD_CDCD);
        assert_eq!(
            unsafe { core::slice::from_raw_parts(node.add(8).cast::<u32>(), 5) },
            pair
        );
        assert_eq!(unsafe { read::<u32>(node, 28) }, 0xCDCD_CDCD);
        assert_eq!(unsafe { read::<u32>(p, 16) }, 0);
        assert_eq!(unsafe { read::<u32>(result.as_mut_ptr(), 0) }, node as u32);
        assert_eq!(unsafe { read::<u32>(result.as_mut_ptr(), 4) }, p as u32);
        assert_eq!(result[8..], [1, 0xA5, 0xA5, 0xA5]);
    }
    extern "thiscall" fn find_fake(this: *mut u8, key: *const u32) -> *mut u8 {
        event(Event::Find(this as u32, unsafe { [key.read(), key.add(1).read()] }));
        STATE.with(|s| s.borrow().find as *mut u8)
    }
    #[test]
    fn resource_touch_hits_cache_refreshes_keys_and_preserves_misses() {
        reset();
        let _find = fake(0x008E_B8D0, find_fake as *const () as usize);
        let mut object = Object::new(0x60);
        let p = object.ptr();
        let mut node = [0u32, 0, 1, 2, 0x1122_3344, 3, 0x5566_7788, 0];
        STATE.with(|s| s.borrow_mut().find = node.as_mut_ptr() as u32);
        let mut output = 0xA5A5_A5A5;
        unsafe {
            write(p, 0x40, 77u32);
        }
        assert_eq!(unsafe { fc4p_008ebc60(p, 1, 2, &mut output) }, 0x1122_3344);
        assert_eq!(output, 0x5566_7788);
        assert_eq!(node[5], 77);
        assert_eq!(events(), vec![Event::Find(p as u32, [1, 2])]);
        unsafe {
            write(p, 0x40, 78u32);
        }
        assert_eq!(unsafe { fc4p_008ebc60(p, 1, 2, &mut output) }, 0x1122_3344);
        assert_eq!(node[5], 78);
        assert!(events().is_empty());
        // Both key halves independently invalidate the cached entry.
        for key in [[9, 2], [9, 7]] {
            assert_eq!(unsafe { fc4p_008ebc60(p, key[0], key[1], &mut output) }, 0x1122_3344);
            assert_eq!(events(), vec![Event::Find(p as u32, key)]);
        }
        STATE.with(|s| s.borrow_mut().find = 0);
        let before = unsafe { core::slice::from_raw_parts(p, 0x60).to_vec() };
        output = 0xA5A5_A5A5;
        assert_eq!(unsafe { fc4p_008ebc60(p, 10, 11, &mut output) }, 0);
        assert_eq!(output, 0xA5A5_A5A5);
        assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x60) }, before);
        assert_eq!(events(), vec![Event::Find(p as u32, [10, 11])]);
    }

    #[test]
    fn instruction_operand_writes_only_fields_and_wraps_register_and_mode() {
        for register in [0u32, 7, 31, 32, 255, u32::MAX] {
            for mode in [0u32, 1, 255, u32::MAX] {
                let mut operand = [0xA5u8; 16];
                let p = operand.as_mut_ptr();
                assert_eq!(unsafe { fc4p_008ec150(p, register, mode, 0x1234_5678) }, p);
                let mut expected = [0xA5u8; 16];
                expected[..4].copy_from_slice(&(1u32 << (register & 31)).to_le_bytes());
                expected[4] = 5;
                expected[5] = 4;
                expected[6] = (mode
                    .wrapping_mul(64)
                    .wrapping_add(register.wrapping_mul(8))
                    .wrapping_add(5)) as u8;
                expected[7..11].copy_from_slice(&0x1234_5678u32.to_le_bytes());
                assert_eq!(operand, expected);
            }
        }
    }
    #[test]
    fn instruction_append_handles_empty_full_length_and_returns_edx() {
        for count in [0u8, 1, 5, 128, 255] {
            let mut object = vec![0xA5u8; 600];
            let p = object.as_mut_ptr();
            let mut operand = vec![0u8; 262];
            operand[4] = count;
            operand[5] = 0x42;
            for i in 0..255 {
                operand[6 + i] = i as u8;
            }
            unsafe {
                write(p, 8, 260u32);
            }
            let mut expected = object.clone();
            expected[276] = 0x78;
            expected[277] = 0xC2;
            expected[278..278 + count as usize].copy_from_slice(&operand[6..6 + count as usize]);
            expected[8..12].copy_from_slice(&(262 + u32::from(count)).to_le_bytes());
            let result = unsafe { fc4p_008ec190(p, 0x1234_5678, 0x80, operand.as_mut_ptr()) };
            assert_eq!(result as u32, u32::from(count));
            assert_eq!((result >> 32) as u32, if count == 0 { 0x100 } else { u32::from(count) });
            assert_eq!(object, expected);
        }
    }
    #[test]
    fn instruction_append_rereads_aliased_operand_length() {
        let mut object = [0u8; 64];
        let p = object.as_mut_ptr();
        unsafe {
            write(p, 8, 0u32);
        }
        // The opcode overwrites the operand length before that length is inspected.
        let operand = p.wrapping_add(12); // length at output[0], flags at output[1]
        let result = unsafe { fc4p_008ec190(p, 2, 0, operand) };
        assert_eq!(result, 0x0000_0002_0000_0002);
        assert_eq!(unsafe { read::<u32>(p, 8) }, 4);
        assert_eq!(&object[16..20], &[2, 0, 0, 0]);
    }

    #[test]
    fn iterator_moves_within_and_across_blocks_in_both_directions() {
        let mut blocks = [[0u8; 120]; 11];
        let mut map = blocks.iter_mut().map(|b| b.as_mut_ptr() as u32).collect::<Vec<_>>();
        for position in 0..3i32 {
            for offset in -12..=12i32 {
                let mut iter = [
                    map[5] + position as u32 * 40,
                    map[5],
                    map[5] + 120,
                    unsafe { map.as_mut_ptr().add(5) } as u32,
                ];
                let original = iter;
                unsafe {
                    fc4p_008ec1e0(iter.as_mut_ptr().cast(), offset);
                }
                let absolute = 15 + position + offset;
                let block = (absolute / 3) as usize;
                let index = absolute % 3;
                assert_eq!(
                    iter,
                    [map[block] + index as u32 * 40, map[block], map[block] + 120, unsafe {
                        map.as_mut_ptr().add(block)
                    }
                        as u32],
                    "position {position}, offset {offset}, before {original:?}"
                );
            }
        }
    }
    #[test]
    fn deque_initialization_centers_map_and_allocates_exact_blocks() {
        let _allocator = allocator();
        for (per_block, size, function) in [
            (
                10u32,
                12u32,
                address(fc4p_008eb800 as unsafe extern "thiscall" fn(*mut u8, u32) -> u64),
            ),
            (
                3,
                40,
                address(fc4p_008ec270 as unsafe extern "thiscall" fn(*mut u8, u32) -> u64),
            ),
        ] {
            for count in [0, 1, per_block - 1, per_block, per_block + 1, per_block * 7] {
                reset();
                let mut object = [0xA5u8; 48];
                let p = object.as_mut_ptr();
                let args = [count];
                let mut call = RawCall::new(Conv::Thiscall, function, p as u32, &args);
                unsafe {
                    call_raw(&mut call);
                }
                let blocks = count / per_block + 1;
                let map_size = (blocks + 2).max(8);
                let first_slot = (map_size - blocks) / 2;
                let map = unsafe { read::<*mut u8>(p, 0x20) };
                let first = unsafe { read::<u32>(map, first_slot as usize * 4) };
                let last_slot = first_slot + blocks - 1;
                let last = unsafe { read::<u32>(map, last_slot as usize * 4) };
                let end = last + count % per_block * size;
                assert_eq!(unsafe { read::<u32>(p, 0) }, first);
                assert_eq!(unsafe { read::<u32>(p, 4) }, first);
                assert_eq!(unsafe { read::<u32>(p, 8) }, first + 120);
                assert_eq!(unsafe { read::<u32>(p, 12) }, map as u32 + first_slot * 4);
                assert_eq!(unsafe { read::<u32>(p, 16) }, end);
                assert_eq!(unsafe { read::<u32>(p, 20) }, last);
                assert_eq!(unsafe { read::<u32>(p, 24) }, last + 120);
                assert_eq!(unsafe { read::<u32>(p, 28) }, map as u32 + last_slot * 4);
                assert_eq!(unsafe { read::<u32>(p, 36) }, map_size);
                assert_eq!(&object[40..], &[0xA5; 8]);
                assert_eq!(call.edx, end);
                assert_eq!(call.eax, count % per_block * if size == 40 { 5 } else { 3 });
                let mut expected = vec![Event::Allocate(map_size * 4)];
                expected.extend((0..blocks).map(|_| Event::Allocate(120)));
                assert_eq!(events(), expected);
                for slot in 0..map_size {
                    if slot < first_slot || slot > last_slot {
                        assert_eq!(unsafe { read::<u32>(map, slot as usize * 4) }, 0xCDCD_CDCD);
                    }
                }
            }
        }
    }
    fn iterator_at(map: &mut [u32], index: usize, size: usize, per_block: usize) -> [u32; 4] {
        let block = index / per_block;
        [
            map[block] + (index % per_block * size) as u32,
            map[block],
            map[block] + 120,
            unsafe { map.as_mut_ptr().add(block) } as u32,
        ]
    }
    #[test]
    fn twelve_byte_range_walk_updates_only_end_first_argument_and_returns_end() {
        let mut blocks = [[0xCDu8; 120]; 4];
        let mut map = blocks.iter_mut().map(|b| b.as_mut_ptr() as u32).collect::<Vec<_>>();
        for (start, end) in [(0, 0), (0, 1), (9, 10), (3, 27), (20, 30)] {
            let first = iterator_at(&mut map, start, 12, 10);
            let last = iterator_at(&mut map, end, 12, 10);
            let mut stack = [0xA5A5_A5A5; 9];
            stack[1..5].copy_from_slice(&first);
            stack[5..9].copy_from_slice(&last);
            let mut expected = stack;
            expected[6] = first[1];
            assert_eq!(unsafe { walk_twelve_byte_range(stack.as_mut_ptr()) }, last[0]);
            assert_eq!(stack, expected);
            let mut call = RawCall::new(Conv::Cdecl, fc4p_008ebb10 as *const () as usize, 0, &stack[1..]);
            unsafe {
                call_raw(&mut call);
            }
            assert_eq!(call.eax, last[0]);
        }
        assert_eq!(blocks, [[0xCD; 120]; 4]);
    }
    #[test]
    fn forty_byte_copies_cross_blocks_return_iterators_and_preserve_other_stack_words() {
        for backward in [false, true] {
            for (start, end, dest) in [(0, 0, 0), (0, 1, 1), (2, 8, 1), (3, 9, 2), (1, 3, 3), (4, 3, 1)] {
                let mut source = [[0u8; 120]; 5];
                for (block, bytes) in source.iter_mut().enumerate() {
                    for (offset, value) in bytes.iter_mut().enumerate() {
                        *value = (block * 120 + offset) as u8;
                    }
                }
                let source_before = source;
                let mut destination = [[0xCDu8; 120]; 5];
                let mut expected_destination = destination;
                let mut source_map = source.iter_mut().map(|b| b.as_mut_ptr() as u32).collect::<Vec<_>>();
                let mut destination_map = destination
                    .iter_mut()
                    .map(|b| b.as_mut_ptr() as u32)
                    .collect::<Vec<_>>();
                let first = iterator_at(&mut source_map, start, 40, 3);
                let last = iterator_at(&mut source_map, end, 40, 3);
                let length = end.saturating_sub(start);
                let destination_index = if backward { dest + length } else { dest };
                let destination_iter = iterator_at(&mut destination_map, destination_index, 40, 3);
                let mut out = [0xA5A5_A5A5u32; 5];
                let mut stack = [0xA5A5_A5A5; 14];
                stack[1] = out.as_mut_ptr() as u32;
                stack[2..6].copy_from_slice(&first);
                stack[6..10].copy_from_slice(&last);
                stack[10..14].copy_from_slice(&destination_iter);
                let before = stack;
                let function = if backward {
                    copy_forty_byte_range_backward
                } else {
                    copy_forty_byte_range_forward
                };
                assert_eq!(unsafe { function(stack.as_mut_ptr()) }, out.as_mut_ptr().cast::<u8>());
                for i in 0..length {
                    let source_element = start + i;
                    let destination_element = dest + i;
                    expected_destination[destination_element / 3]
                        [destination_element % 3 * 40..destination_element % 3 * 40 + 40]
                        .copy_from_slice(
                            &source_before[source_element / 3][source_element % 3 * 40..source_element % 3 * 40 + 40],
                        );
                }
                assert_eq!(destination, expected_destination, "{backward} {start} {end} {dest}");
                assert_eq!(source, source_before);
                let final_index = if backward { dest } else { dest + length };
                let expected = iterator_at(&mut destination_map, final_index, 40, 3);
                assert_eq!(&out[..4], &expected);
                assert_eq!(out[4], 0xA5A5_A5A5);
                if length == 0 {
                    assert_eq!(stack, before);
                } else if backward {
                    assert_eq!(stack[2], 0);
                    assert_eq!(stack[7], first[1]);
                    assert_eq!(stack[9], first[3]);
                    assert_eq!(stack[12], expected[2]);
                    assert_eq!(stack[13], expected[3]);
                    for i in [0, 1, 3, 4, 5, 6, 8, 10, 11] {
                        assert_eq!(stack[i], before[i], "word {i}");
                    }
                } else {
                    assert_eq!(stack[6], 0);
                    assert_eq!(stack[4], last[2]);
                    assert_eq!(&stack[11..14], &expected[1..]);
                    for i in [0, 1, 2, 3, 5, 7, 8, 9, 10] {
                        assert_eq!(stack[i], before[i], "word {i}");
                    }
                }
            }
        }
    }
    #[test]
    fn forty_byte_range_copies_support_overlaps_in_the_traversal_direction() {
        for backward in [false, true] {
            let mut data = [[0u8; 120]; 5];
            for (block, bytes) in data.iter_mut().enumerate() {
                for (offset, value) in bytes.iter_mut().enumerate() {
                    *value = (block * 120 + offset) as u8;
                }
            }
            let mut expected = data;
            let (start, end, dest) = if backward { (1, 8, 3) } else { (3, 10, 1) };
            let original = data;
            for i in 0..end - start {
                let from = start + i;
                let to = dest + i;
                expected[to / 3][to % 3 * 40..to % 3 * 40 + 40]
                    .copy_from_slice(&original[from / 3][from % 3 * 40..from % 3 * 40 + 40]);
            }
            let mut map = data.iter_mut().map(|b| b.as_mut_ptr() as u32).collect::<Vec<_>>();
            let first = iterator_at(&mut map, start, 40, 3);
            let last = iterator_at(&mut map, end, 40, 3);
            let destination = iterator_at(&mut map, if backward { dest + end - start } else { dest }, 40, 3);
            let mut result = [0u32; 4];
            let mut args = vec![result.as_mut_ptr() as u32];
            args.extend(first);
            args.extend(last);
            args.extend(destination);
            let entry = if backward {
                fc4p_008ec340 as *const ()
            } else {
                fc4p_008ec440 as *const ()
            };
            let mut call = RawCall::new(Conv::Cdecl, entry as usize, 0, &args);
            unsafe {
                call_raw(&mut call);
            }
            assert_eq!(call.eax, result.as_mut_ptr() as u32);
            assert_eq!(data, expected);
            assert_eq!(
                result,
                iterator_at(&mut map, if backward { dest } else { dest + end - start }, 40, 3)
            );
        }
    }
}


#[cfg(test)]
mod triangle_tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake, GameFpu};
    use std::cell::RefCell;

    #[derive(Debug, Clone)]
    struct Event {
        kind: u32,
        regs: [u32; 8],
        mm: [u64; 8],
        flags: u32,
    }
    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
        static SPAN_MUTATION: RefCell<Option<(u32, u32)>> = const { RefCell::new(None) };
    }
    fn events() -> Vec<Event> {
        EVENTS.with(|v| core::mem::take(&mut *v.borrow_mut()))
    }

    unsafe extern "fastcall" fn record(stack: *const u32, kind: u32) {
        let mut mm = [0u64; 8];
        unsafe {
            core::arch::asm!(
                "movq [{p}], mm0", "movq [{p} + 8], mm1", "movq [{p} + 16], mm2", "movq [{p} + 24], mm3",
                "movq [{p} + 32], mm4", "movq [{p} + 40], mm5", "movq [{p} + 48], mm6", "movq [{p} + 56], mm7",
                p = in(reg) mm.as_mut_ptr(), options(nostack));
            EVENTS.with(|v| {
                v.borrow_mut().push(Event {
                    kind,
                    regs: core::array::from_fn(|i| stack.add(i).read()),
                    mm,
                    flags: stack.add(19).read(),
                })
            });
            if kind == 16 {
                SPAN_MUTATION.with(|v| {
                    if let Some((address, value)) = v.borrow_mut().take() {
                        triangle_store(address, value);
                    }
                });
            }
        }
    }
    macro_rules! edge_fake {
        ($name:ident, $kind:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
                core::arch::naked_asm!("pushad", "mov ecx, esp", "mov edx, {kind}", "call {record}",
                    "popad", "ret", kind = const $kind, record = sym record)
            }
        };
    }
    edge_fake!(color, 1);
    edge_fake!(texture, 2);
    edge_fake!(depth, 4);
    #[unsafe(naked)]
    unsafe extern "C" fn span() {
        core::arch::naked_asm!(
            "pushad", "mov ecx, esp", "mov edx, 16", "call {record}", "popad",
            "inc eax", "inc ecx", "inc edx", "inc ebx", "inc esi", "inc ebp", "paddd mm0, mm1", "ret",
            record = sym record)
    }

    edge_fake!(second_span, 32);

    #[repr(C, align(16))]
    struct FpuState([u8; 512]);
    #[repr(C)]
    struct Invocation {
        regs: TriangleRegisters,
        vertices: [u32; 3],
        fpu: *mut FpuState,
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn invoke(_: *mut Invocation) {
        core::arch::naked_asm!(
            "pushad",
            "push ecx",
            "push dword ptr [ecx + 32]",
            "push dword ptr [ecx + 36]",
            "push dword ptr [ecx + 112]",
            "push dword ptr [ecx + 108]",
            "push dword ptr [ecx + 104]",
            "mov edx, [ecx + 116]",
            "fxrstor [edx]",
            "mov edi, [ecx]",
            "mov esi, [ecx + 4]",
            "mov ebp, [ecx + 8]",
            "mov ebx, [ecx + 16]",
            "mov edx, [ecx + 20]",
            "mov eax, [ecx + 28]",
            "mov ecx, [ecx + 24]",
            "call dword ptr [esp + 16]",
            "pushad",
            "mov ebx, [esp + 52]",
            "mov edi, ebx",
            "mov esi, esp",
            "mov ecx, 8",
            "rep movsd",
            "mov edx, [ebx + 116]",
            "fxsave [edx]",
            "emms",
            "add esp, 32",
            "add esp, 20",
            "pop ecx",
            "popad",
            "ret"
        )
    }
    struct Fixture {
        state: Box<[u8; 0x5E0]>,
        _table: Box<[u32; 16]>,
        _guards: Vec<Fake>,
    }
    impl Fixture {
        fn new(real_edges: bool) -> Self {
            events();
            let mut state = Box::new([0xA5u8; 0x5E0]);
            let p = state.as_mut_ptr();
            let edge = |a: u32| p.wrapping_add((a - 0x00B5_C4A8) as usize) as u32;
            let mut table = Box::new([
                edge(0x00B5_C538),
                edge(0x00B5_C4F0),
                edge(0x00B5_C538),
                edge(0x00B5_C4A8),
                edge(0x00B5_C4F0),
                edge(0x00B5_C538),
                edge(0x00B5_C4A8),
                edge(0x00B5_C538),
                16,
                0,
                0,
                16,
                16,
                0,
                0,
                0,
            ]);
            let mut guards = vec![
                fake_data(0x00B5_C4A8, p),
                fake_data(0x00AC_C67C, table.as_mut_ptr().cast()),
            ];
            for address in [
                0x00B5_C5B8,
                0x00B5_CA00,
                0x00B5_CA04,
                0x00B5_C9F0,
                0x00B5_C9F8,
                0x00B5_CA08,
                0x00B5_CA18,
            ] {
                guards.push(fake_data(address, p.wrapping_add((address - 0x00B5_C4A8) as usize)));
            }
            if !real_edges {
                guards.extend([
                    fake(0x008D_D760, color as *const () as usize),
                    fake(0x008D_D680, texture as *const () as usize),
                    fake(0x008D_D620, depth as *const () as usize),
                ]);
            }
            unsafe {
                write(p, 0xF0, 0x1234_5678u32);
                write(p, 0xF4, 0x8765_4321u32);
                write(p, 0xF8, 0xFFFD_0007u32);
                write(p, 0xFC, 0x000B_FFFBu32);
                write(p, 0x110, span as *const () as u32);
            }
            Self {
                state,
                _table: table,
                _guards: guards,
            }
        }
        fn pointer(&self, a: u32) -> u32 {
            self.state.as_ptr().wrapping_add((a - 0x00B5_C4A8) as usize) as u32
        }
        fn word(&self, a: u32) -> u32 {
            unsafe { read(self.state.as_ptr() as *mut u8, (a - 0x00B5_C4A8) as usize) }
        }
        fn run(&self, vertices: &mut [[u32; 6]; 3], flags: u32) -> (TriangleRegisters, FpuState) {
            self.run_with_mm2_exponent(vertices, flags, 0xFFFF)
        }
        fn run_with_mm2_exponent(
            &self,
            vertices: &mut [[u32; 6]; 3],
            flags: u32,
            exponent: u16,
        ) -> (TriangleRegisters, FpuState) {
            let mut fpu = FpuState([0; 512]);
            let mut call = Invocation {
                regs: TriangleRegisters {
                    edi: 0x1020_3040,
                    esi: 0x2345_6789,
                    ebp: 0x3456_789A,
                    ebx: 0x4567_89AB,
                    edx: 0x5678_9ABC,
                    ecx: 0x6789_ABCD,
                    eax: 0x789A_BCDE,
                    target: fc4p_008dd8b0 as *const () as u32,
                    flags,
                    mm: core::array::from_fn(|i| TriangleMmx(0x1234_5678_90AB_CDEFu64.wrapping_mul(i as u64 + 1))),
                    ..TriangleRegisters::default()
                },
                vertices: core::array::from_fn(|i| vertices[i].as_mut_ptr() as u32),
                fpu: &mut fpu,
            };
            let expected = [
                call.regs.edi,
                call.regs.esi,
                call.regs.ebp,
                call.regs.ebx,
                call.regs.edx,
                call.regs.ecx,
            ];
            let mut saved_csr = 0u32;
            unsafe {
                core::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut saved_csr, options(nostack));
            }
            fpu.0[..2].copy_from_slice(&crate::x87::GAME_CONTROL_WORD.to_le_bytes());
            fpu.0[4] = 255;
            fpu.0[24..28].copy_from_slice(&saved_csr.to_le_bytes());
            for i in 0..8 {
                fpu.0[32 + i * 16..40 + i * 16].copy_from_slice(&call.regs.mm[i].0.to_le_bytes());
                let e = if i == 2 { exponent } else { 0xFFFF };
                fpu.0[40 + i * 16..42 + i * 16].copy_from_slice(&e.to_le_bytes());
            }
            unsafe { invoke(&mut call) };
            assert_eq!(
                [
                    call.regs.edi,
                    call.regs.esi,
                    call.regs.ebp,
                    call.regs.ebx,
                    call.regs.edx,
                    call.regs.ecx
                ],
                expected
            );
            assert_eq!(
                u16::from_le_bytes(fpu.0[..2].try_into().unwrap()),
                crate::x87::GAME_CONTROL_WORD
            );
            assert_eq!(fpu.0[4], 0xFF);
            assert_eq!(u16::from_le_bytes(fpu.0[2..4].try_into().unwrap()) & 0x3800, 0);
            assert_eq!(u32::from_le_bytes(fpu.0[24..28].try_into().unwrap()), saved_csr);
            for i in 0..8 {
                call.regs.mm[i] = TriangleMmx(u64::from_le_bytes(fpu.0[32 + i * 16..40 + i * 16].try_into().unwrap()));
                let e = if i == 2 && exponent != 0xFFFF { exponent } else { 0xFFFF };
                assert_eq!(&fpu.0[40 + i * 16..42 + i * 16], &e.to_le_bytes());
            }
            (call.regs, fpu)
        }
    }
    fn vertices() -> [[u32; 6]; 3] {
        [
            [24, 40, 0x1122_3344, 0x1234_5678, 0x1234, 0x5678],
            [248, 88, 0x5566_7788, 0xFEDC_BA98, 0x9876, 0x5432],
            [56, 296, 0x99AA_BBCC, 0x9182_7364, 0xABCD, 0xEF01],
        ]
    }
    fn put(expected: &mut [u8], address: u32, value: u32) {
        let offset = (address - 0x00B5_C4A8) as usize;
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn triangle_all_attribute_flags_write_only_the_selected_planes_and_exact_setup_words() {
        let _fpu = GameFpu::new();
        for flags in 0..16 {
            let f = Fixture::new(false);
            let mut expected = *f.state;
            let mut vertices = vertices();
            let before_vertices = vertices;
            let (result, _) = f.run(&mut vertices, flags);
            assert_eq!(vertices, before_vertices);
            for (address, value) in [
                (0x00B5_C4A8, 0x18000),
                (0x00B5_C4AC, 0x7627),
                (0x00B5_C4E8, 0),
                (0x00B5_C4EC, 0),
                (0x00B5_C4F0, 0x78000),
                (0x00B5_C4F4, 0xFFFDAAAB),
                (0x00B5_C530, 0),
                (0x00B5_C534, 0),
                (0x00B5_C538, 0x18000),
                (0x00B5_C53C, 0xFFFFF000),
                (0x00B5_C578, 0),
                (0x00B5_C57C, 0),
                (0x00B5_C5C0, 0xDA00),
                (0x00B5_C9E4, 0x1234_56F6),
                (0x00B5_C9EC, 0x8765_42C7),
                (0x00B5_CA34, 3),
                (0x00B5_CA38, 13),
                (0x00B5_CA5C, 2),
                (0x00B5_CA60, 5),
                (0x00B5_CA64, 18),
                (0x00B5_CA68, 0x12C9F),
                (0x00B5_CA6C, 0x12C9F),
                (0x00B5_CA70, 0xF441),
                (0x00B5_CA74, 0xF441),
                (0x00B5_CA78, 0x2593),
                (0x00B5_CA7C, 0x2593),
                (0x00B5_CA80, 0xFFFF1E88),
                (0x00B5_CA84, 0xFFFF1E88),
            ] {
                put(&mut expected, address, value);
            }
            for (address, value) in [
                (0x00B5_CA3C, f.pointer(0x00B5_C538)),
                (0x00B5_CA40, f.pointer(0x00B5_C4F0)),
                (0x00B5_CA44, f.pointer(0x00B5_C538)),
                (0x00B5_CA48, f.pointer(0x00B5_C4F0)),
            ] {
                put(&mut expected, address, value);
            }
            if flags & 1 != 0 {
                for (i, value) in [
                    0x14FD8C, 0x378589, 0x5A0D86, 0x7C9583, 0xFFF3607C, 0x78F8D, 0x1BBE9E, 0x2FEDAF,
                ]
                .into_iter()
                .enumerate()
                {
                    put(&mut expected, 0x00B5_CA08 + i as u32 * 4, value);
                }
            }
            if flags & 2 != 0 {
                let values = if flags & 8 == 0 {
                    [0x7BD6DCD0, 0xDBBE9438, 0x8A1E26AD, 0x9D10B915, 0x3DEB, 0xEDDF]
                } else {
                    [0x07BD6DCD, 0xFDBBE943, 0xF8A1E26A, 0xF9D10B91, 0x03DE, 0xFEDD]
                };
                for (address, value) in [
                    0x00B5_C9F0,
                    0x00B5_C9F4,
                    0x00B5_C9F8,
                    0x00B5_C9FC,
                    0x00B5_CA28,
                    0x00B5_CA2C,
                ]
                .into_iter()
                .zip(values)
                {
                    put(&mut expected, address, value);
                }
            }
            if flags & 4 != 0 {
                for (address, value) in [
                    (0x00B5_CA00, 0x6345_0666),
                    (0x00B5_CA04, 0x0111_08ED),
                    (0x00B5_CA30, 0x0006_6663),
                ] {
                    put(&mut expected, address, value);
                }
            }
            for (i, (actual, expected)) in f
                .state
                .as_chunks::<4>()
                .0
                .iter()
                .zip(expected.as_chunks::<4>().0.iter())
                .enumerate()
            {
                assert_eq!(actual, expected, "flags {flags}, address {:08x}", 0x00B5_C4A8 + i * 4);
            }
            let calls = events();
            let mut kinds = Vec::new();
            for bit in [1, 2, 4] {
                if flags & bit != 0 {
                    kinds.push(bit);
                }
            }
            kinds.extend([16, 16]);
            assert_eq!(calls.iter().map(|c| c.kind).collect::<Vec<_>>(), kinds);
            for e in calls.iter().filter(|e| e.kind != 16) {
                assert_eq!(e.regs[2], f.pointer(0x00B5_C538));
                assert_eq!(e.regs[1], vertices[2].as_ptr() as u32);
                assert_eq!(e.regs[6], vertices[1].as_ptr() as u32);
                assert_eq!(e.flags, flags);
            }
            let spans: Vec<_> = calls.iter().filter(|e| e.kind == 16).collect();
            assert_eq!([spans[0].regs[0], spans[1].regs[0]], [13, 3]);
            assert_eq!([spans[0].regs[2], spans[1].regs[2]], [0x1234_56F6, 0x1234_56F7]);
            assert_eq!(spans[1].regs[6], spans[0].regs[6].wrapping_add(1));
            assert_eq!(
                result.eax,
                if flags & 4 != 0 {
                    2
                } else {
                    vertices[2].as_ptr() as u32 + 2
                }
            );
            let remaining_mmx = if flags & 1 == 0 {
                [
                    0x0001_2C9F_0001_2C9F,
                    0x006D_005A_0047_0034,
                    0,
                    0,
                    0x0091_0082_0073_0064,
                    0x0000_F441_0000_F441,
                    0xFFFF_1E88_FFFF_1E88,
                ]
            } else {
                [
                    0,
                    0x006D_0000_005A_0000,
                    0x007C_9583_005A_0D86,
                    0,
                    0,
                    0x002F_EDAF_001B_BE9E,
                    0x003F_8000_0027_0000,
                ]
            };
            assert_eq!(result.mm[1..].iter().map(|m| m.0).collect::<Vec<_>>(), remaining_mmx);
            assert_eq!(result.mm[4], TriangleMmx(0));
            assert_eq!(
                result.mm[0],
                TriangleMmx(spans[0].mm[0])
                    .add(TriangleMmx(spans[0].mm[1]))
                    .add(TriangleMmx(spans[0].mm[1]))
            );
        }
    }

    #[test]
    fn triangle_rejected_winding_only_clears_area_and_keeps_entry_mm2() {
        let _fpu = GameFpu::new();
        let f = Fixture::new(false);
        let mut v = vertices();
        v.swap(0, 2);
        let mut expected = *f.state;
        put(&mut expected, 0x00B5_C5C0, 0);
        let (result, _) = f.run_with_mm2_exponent(&mut v, 15, 0x1234);
        assert_eq!(f.state[..], expected);
        assert_eq!(result.eax, 0);
        assert!(events().is_empty());
        assert_eq!(result.mm[0], TriangleMmx(0));
        assert_eq!(result.mm[1], TriangleMmx(0));
        assert_eq!(result.mm[2].0, 0x1234_5678_90AB_CDEFu64.wrapping_mul(3));
    }

    #[test]
    fn triangle_area_thresholds_keep_zero_gradients_until_257_and_preserve_empty_ranges() {
        let _fpu = GameFpu::new();
        for area in [0u32, 1, 255, 256, 257] {
            let f = Fixture::new(false);
            let mut v = [[0, 8, 0, 0, 0, 0], [area, 8, 0, 0, 0, 0], [0, 9, 0, 0, 0, 0]];
            let (r, _) = f.run(&mut v, 0);
            assert_eq!(f.word(0x00B5_C5C0), area);
            assert!(events().is_empty());
            assert_eq!(r.eax, v[2].as_ptr() as u32);
            if area <= 256 {
                assert_eq!(f.word(0x00B5_CA68), 0);
                assert_eq!(f.word(0x00B5_CA80), 0);
            } else {
                assert_ne!(f.word(0x00B5_CA68), 0);
            }
            assert_eq!(f.word(0x00B5_CA34), 0);
            assert_eq!(f.word(0x00B5_CA38), 0);
        }
    }

    #[test]
    fn triangle_vertex_permutations_tied_rows_and_both_edge_sides_keep_call_order() {
        let _fpu = GameFpu::new();
        for rows in [[40, 88, 296], [40, 40, 296], [40, 296, 296], [40, 40, 40]] {
            for permutation in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                let f = Fixture::new(false);
                let mut original = vertices();
                for i in 0..3 {
                    original[i][1] = rows[i];
                }
                let mut v = permutation.map(|i| original[i]);
                let (r, _) = f.run(&mut v, 7);
                let calls = events();
                if calls.is_empty() && r.eax == 0 {
                    continue;
                }
                let edge_calls: Vec<_> = calls.iter().filter(|c| c.kind != 16).collect();
                let count = edge_calls.len() / 3;
                assert!([1, 2].contains(&count));
                let expected: Vec<_> = [1, 2, 4]
                    .into_iter()
                    .flat_map(|kind| core::iter::repeat_n(kind, count))
                    .collect();
                assert_eq!(edge_calls.iter().map(|c| c.kind).collect::<Vec<_>>(), expected);
                for group in edge_calls.chunks(count) {
                    if count == 1 {
                        assert_eq!(group[0].regs[2], f.pointer(0x00B5_C538));
                    } else {
                        assert_eq!(
                            [group[0].regs[2], group[1].regs[2]],
                            [f.pointer(0x00B5_C4F0), f.pointer(0x00B5_C4A8)]
                        );
                        assert_eq!(
                            [group[0].regs[1], group[1].regs[1]],
                            [group[1].regs[6], group[0].regs[6]]
                        );
                    }
                }
                let spans = calls.iter().filter(|c| c.kind == 16).count();
                assert_eq!(
                    spans,
                    usize::from(f.word(0x00B5_CA34) as i32 > 0) + usize::from(f.word(0x00B5_CA38) as i32 > 0)
                );
            }
        }
    }

    #[test]
    fn triangle_runs_real_edge_initializers_with_fractional_coordinates() {
        let _fpu = GameFpu::new();
        for flags in [1, 2, 4, 7, 15] {
            let f = Fixture::new(true);
            let mut v = vertices();
            v[0][0] += 3;
            v[0][1] += 3;
            v[1][0] += 3;
            v[1][1] += 4;
            v[2][0] += 3;
            v[2][1] += 3;
            let (r, _) = f.run(&mut v, flags);
            assert_eq!(events().iter().map(|e| e.kind).collect::<Vec<_>>(), [16, 16]);
            assert_ne!(f.word(0x00B5_C578), 0);
            assert_ne!(f.word(0x00B5_C57C), 0);
            assert_eq!(r.mm[4], TriangleMmx(0));
            if flags & 1 != 0 {
                assert_ne!(f.word(0x00B5_C550), 0xA5A5_A5A5);
            }
            if flags & 2 != 0 {
                assert_ne!(f.word(0x00B5_C540), 0xA5A5_A5A5);
            }
            if flags & 4 != 0 {
                assert_ne!(f.word(0x00B5_C570), 0xA5A5_A5A5);
            }
        }
    }

    #[test]
    fn triangle_reloads_lower_range_and_span_target_after_the_first_span() {
        let _fpu = GameFpu::new();
        for skip in [false, true] {
            let f = Fixture::new(false);
            let mut v = vertices();
            SPAN_MUTATION.with(|m| {
                *m.borrow_mut() = Some(if skip {
                    (0x00B5_CA34, 0xFFFF_FFFF)
                } else {
                    (0x00B5_C5B8, second_span as *const () as u32)
                })
            });
            let (r, _) = f.run(&mut v, 0);
            let expected = if skip { vec![16] } else { vec![16, 32] };
            assert_eq!(events().iter().map(|e| e.kind).collect::<Vec<_>>(), expected);
            assert_eq!(r.eax, v[2].as_ptr() as u32 + 1);
        }
    }

    #[test]
    fn triangle_area_uses_low_signed_words_while_coefficients_saturate() {
        let _fpu = GameFpu::new();
        let f = Fixture::new(false);
        let mut v = [
            [0, 8, 0, 0x1122_3344, 0, 0],
            [65537, 8, 0, 0x5566_7788, 0, 0],
            [0, 265, 0, 0x99AA_BBCC, 0, 0],
        ];
        f.run(&mut v, 1);
        assert_eq!(f.word(0x00B5_C5C0), 257);
        for (address, value) in [
            (0x00B5_CA68, 0x00FF_FFFF),
            (0x00B5_CA70, 0x00FF_FFFF),
            (0x00B5_CA78, 0),
            (0x00B5_CA80, 0x807F_8080),
        ] {
            assert_eq!(f.word(address), value);
        }
        for i in 0..4 {
            assert_eq!(f.word(0x00B5_CA08 + i * 4), 0x21FF_FFBC);
            assert_eq!(f.word(0x00B5_CA18 + i * 4), 0xDE21_DE00);
        }
    }
}
