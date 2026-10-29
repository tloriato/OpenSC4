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

game_data! { static depth_step_x: i32 = 0x00B5_CA00; }
game_data! { static depth_step_y: i32 = 0x00B5_CA04; }

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
        let slope = (rounded_depth_product(read(edge, 4), *depth_step_x()) >> 15) as u32;
        let step_y = *depth_step_y();
        write(edge, 0x3C, slope.wrapping_sub(step_y as u32));
        let vertical = (rounded_depth_product(step_y, read(edge, 0x44)) >> 15) as u32;
        let base = (u32::from(read::<u16>(vertex, 8)) << 16).wrapping_sub(vertical);
        let horizontal = rounded_depth_product(*depth_step_x(), read(edge, 0x40));
        let shifted = (horizontal >> 15) as u32;
        write(edge, 0x38, base.wrapping_add(shifted));
        (u64::from(shifted) << 32) | u64::from(horizontal as u32)
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
        let face = read::<*mut u8>(this, 8);
        read::<u32>(face, 0x54) == 0 && read::<u32>(face, 0x50) == 0
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
        let kind = stack.add(2).read();
        if !read::<*mut u8>(this, 8).is_null() {
            let mut selected = 0u32;
            for (index, (language, requested_kind, alternate)) in [
                (0x409, kind, false),
                (0x409, kind, true),
                (0, kind, false),
                (0, kind, true),
                (0x409, 4, false),
                (0x409, 4, true),
                (0, 4, false),
                (0, 4, true),
            ]
            .into_iter()
            .enumerate()
            {
                if index != 0 && !missing_font_name(this) {
                    continue;
                }
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
                let first = read::<*mut u8>(output, 4);
                if first != read::<*mut u8>(output, 8) {
                    first.write(0);
                    write(output, 8, read::<u32>(output, 4));
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
        let first = read::<u32>(output, 4);
        let nonempty = u32::from(first != read::<u32>(output, 8));
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
                let hash = read::<u32>(node, 12).wrapping_add(read(node, 8));
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
                    if read::<u32>(node, 0x14) < read::<u32>(this, 0x40).wrapping_sub(2) {
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

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_0089801e::PORT,
    fc4p_008ae800::PORT,
    fc4p_008b03d0::PORT,
    fc4p_008b15e0::PORT,
    fc4p_008b7f40::PORT,
    fc4p_008b87a0::PORT,
    fc4p_008d0fd0::PORT,
    fc4p_008d1030::PORT,
    fc4p_008d29b9::PORT,
    fc4p_008dd620::PORT,
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
mod depth_edge_tests {
    use super::*;
    use crate::testing::fake_data;

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
                let mut stack = [0xDEAD_BEEF, output.as_mut_ptr() as u32, 4, 0xCAFE_BABE];
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
                        4,
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
            let mut memory = vec![fill; count as usize].into_boxed_slice();
            let start = memory.as_mut_ptr() as u32;
            unsafe {
                this.write(start);
                this.add(1).write(start.wrapping_add(count * 4));
                this.add(2).write(start.wrapping_add(count * 4 + 16));
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
