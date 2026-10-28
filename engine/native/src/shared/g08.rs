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
        0x10 => count / set_count: u32,
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

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_008ae800::PORT,
    fc4p_008b03d0::PORT,
    fc4p_008b15e0::PORT,
    fc4p_008b7f40::PORT,
    fc4p_008b87a0::PORT,
    fc4p_008d0fd0::PORT,
    fc4p_008d1030::PORT,
    fc4p_008d29b9::PORT,
    fc4p_008dc4c0::PORT,
    fc4p_008eb800::PORT,
    fc4p_008eb8d0::PORT,
    fc4p_008eb920::PORT,
    fc4p_008ebc60::PORT,
    fc4p_008ebb10::PORT,
    fc4p_008ec150::PORT,
    fc4p_008ec190::PORT,
    fc4p_008ec1e0::PORT,
    fc4p_008ec270::PORT,
    fc4p_008ec340::PORT,
    fc4p_008ec440::PORT,
];

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
