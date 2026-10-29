// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Stop filtering, lane enumeration and network connections in hash-map order.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn map_begin(this: *mut u8, out: *mut *mut u8) -> u64 = "thiscall" 0x004B_0390;
}

unsafe fn begin(this: *mut u8, iterator: *mut *mut u8, edx: u32) -> u64 {
    unsafe {
        let args = [iterator as u32];
        let mut call = RawCall::new(
            Conv::Thiscall,
            crate::abi::target(0x004B_0390),
            this.add(0x1C) as u32,
            &args,
        );
        call.eax = iterator as u32;
        call.edx = edx;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

// After a chain ends, iteration resumes after the bucket selected by its final key.
unsafe fn next_bucket(map: *mut u8, key: u32) -> (*mut u8, u32, u32) {
    unsafe {
        let info = PathInfo::from_ptr(map.wrapping_sub(0x1C));
        let mut index = unsigned_remainder(key, bucket_count(info));
        let count = bucket_count(info);
        loop {
            index = index.wrapping_add(1);
            if index >= count {
                return (core::ptr::null_mut(), index, count);
            }
            let node = read_unaligned(info.buckets().add(index as usize));
            if !node.is_null() {
                return (node, index, count);
            }
        }
    }
}

port! {
    /// Counts matching stop records. Type zero and byte 0xFF filters are wildcards; stored edge 0xFF bytes
    /// also match any requested edge. The packed filter replaces the caller's entry argument word.
    #[port(address = 0x0053_E550, owner = "pathinfo", name = "cSC4PathInfo::GetStopPointCount", naked)]
    pub unsafe extern "C" fn fc4p_0053e550() {
        core::arch::naked_asm!("sub esp, 28", "lea edx, [esp + 28]", "call {}", "add esp, 28", "ret 16",
            sym stop_count);
    }
}

unsafe extern "fastcall" fn stop_count(this: *mut u8, stack: *mut u32) -> u64 {
    unsafe {
        let kind = read_unaligned(stack.add(1));
        let entry = read_unaligned(stack.add(2)) & 0xFF;
        let exit = read_unaligned(stack.add(3)) & 0xFF;
        let lane = read_unaligned(stack.add(4)) & 0xFF;
        let mut mask = if kind == 0 { 0x0F00_0000 } else { 0 };
        if entry == 0xFF {
            mask |= 0xFF00;
        }
        if exit == 0xFF {
            mask |= 0xFF;
        }
        if lane == 0xFF {
            mask |= 0xFF0000;
        }
        let filter = path_key(kind | 0x10, entry, exit, lane);
        write_unaligned(stack.add(2), filter);
        let iterator = stack.cast::<u8>().sub(8).cast::<*mut u8>();
        let result = begin(this, iterator, exit);
        let mut edx = (result >> 32) as u32;
        let mut node = read_unaligned(iterator);
        let map = read_unaligned(iterator.add(1));
        let mut count = 0u32;
        while !node.is_null() {
            let item = PathNode::from_ptr(node);
            let key = item.key();
            let mut stored_mask = 0;
            if (key >> 8) as u8 == 0xFF {
                stored_mask |= 0xFF00;
            }
            if key as u8 == 0xFF {
                stored_mask |= 0xFF;
            }
            if key | stored_mask | mask == filter | stored_mask | mask {
                count = count.wrapping_add(1);
            }
            node = item.next();
            if node.is_null() {
                (node, edx, _) = next_bucket(map, key);
            }
        }
        (u64::from(edx) << 32) | u64::from(count)
    }
}

port! {
    /// Returns the greatest lane byte in any path or stop key. The three stack arguments are unused.
    /// ECX retains the iterator's map header; EDX retains the final bucket index or the empty-begin result.
    #[port(address = 0x0053_EB30, owner = "pathinfo", name = "maximum path lane byte", naked)]
    pub unsafe extern "C" fn fc4p_0053eb30() {
        core::arch::naked_asm!("sub esp, 16", "lea edx, [esp + 8]", "call {}",
            "mov ecx, [esp + 12]", "add esp, 16", "ret 12", sym max_lane);
    }
}

unsafe extern "fastcall" fn max_lane(this: *mut u8, iterator: *mut *mut u8) -> u64 {
    unsafe {
        let result = begin(this, iterator, 0);
        let mut edx = (result >> 32) as u32;
        let mut node = read_unaligned(iterator);
        let map = read_unaligned(iterator.add(1));
        let mut maximum = 0;
        while !node.is_null() {
            let item = PathNode::from_ptr(node);
            let key = item.key();
            maximum = maximum.max((key >> 16) & 0xFF);
            node = item.next();
            if node.is_null() {
                (node, edx, _) = next_bucket(map, key);
            }
        }
        (u64::from(edx) << 32) | u64::from(maximum)
    }
}

port! {
    /// Adds every ordinary path's type, entry and exit to the network bitmap. Stop keys are skipped.
    /// The callback receives the original full stack words, with the edge byte at each word's low end.
    #[port(address = 0x0053_EBB0, owner = "pathinfo", name = "cSC4PathInfo::InitializeNetworkConnectionBitmap", naked)]
    pub unsafe extern "C" fn fc4p_0053ebb0() {
        core::arch::naked_asm!("sub esp, 16", "lea edx, [esp + 8]", "push [esp + 20]",
            "call {}", "add esp, 16", "ret 4", sym initialize_bitmap);
    }
}

unsafe extern "fastcall" fn initialize_bitmap(this: *mut u8, iterator: *mut *mut u8, bitmap: *mut u8) -> u64 {
    unsafe {
        let result = begin(this, iterator, 0);
        let mut node = read_unaligned(iterator);
        let map = read_unaligned(iterator.add(1));
        if node.is_null() {
            return result;
        }
        loop {
            let item = PathNode::from_ptr(node);
            let key = item.key();
            if key & 0xF000_0000 == 0 {
                let kind = (key >> 24) & 0xF;
                let args = [kind, key >> 8, key];
                let mut call = RawCall::new(Conv::Thiscall, vslot(bitmap, 0xD8), bitmap as u32, &args);
                call.eax = kind;
                call.edx = read_unaligned(bitmap.cast());
                call_raw(&mut call);
            }
            node = item.next();
            if node.is_null() {
                let (next, index, count) = next_bucket(map, item.key());
                node = next;
                if node.is_null() {
                    return (u64::from(index) << 32) | u64::from(count);
                }
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0053e550::PORT, fc4p_0053eb30::PORT, fc4p_0053ebb0::PORT];

#[cfg(test)]
mod tests;
