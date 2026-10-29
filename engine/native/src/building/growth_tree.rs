// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Hinted insertion and default construction of the per-developer tried-tract trees.

use super::trees::{Node, Tree};
use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::{null_mut, read_unaligned as read, write_unaligned as write};

game_fn! { #[allow(dead_code)] fn insert(this: *mut u8, out: *mut u8, child: *mut u8, parent: *mut u8, source: *mut u8, force: u32) -> u64 = "thiscall" 0x0069_A610; }
game_fn! { #[allow(dead_code)] fn unique(this: *mut u8, out: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0069_A6E0; }
game_fn! { #[allow(dead_code)] fn previous(iterator: *mut u8) -> u64 = "thiscall" 0x0068_09E0; }
game_fn! { #[allow(dead_code)] fn next(iterator: *mut u8) -> u64 = "thiscall" 0x005D_17C0; }
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn copy_tree(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0069_8A80; }
game_fn! { #[allow(dead_code)] fn insert_hint(this: *mut u8, out: *mut u8, hint: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0069_A8F0; }
game_fn! { #[allow(dead_code)] fn delete_pairs(this: *mut u8, root: *mut u8) -> u64 = "thiscall" 0x0069_5E10; }

unsafe fn call(address: u32, conv: Conv, this: u32, words: &[u32], r: u64) -> u64 {
    let mut c = RawCall::new(conv, crate::abi::target(address), this, words);
    c.eax = r as u32;
    c.edx = (r >> 32) as u32;
    // SAFETY: every call site declares the game helper with this convention and these words.
    unsafe { call_raw(&mut c) };
    (u64::from(c.edx) << 32) | u64::from(c.eax)
}

unsafe fn fallback(this: *mut u8, out: *mut u32, source: *mut u8, r: u64, copy_edx: bool) -> u64 {
    let mut result = MaybeUninit::<[u8; 8]>::uninit();
    // SAFETY: unique insertion fills its iterator result; out is the caller's writable iterator.
    unsafe {
        let r = call(
            0x0069_A6E0,
            Conv::Thiscall,
            this as u32,
            &[result.as_mut_ptr() as u32, source as u32],
            r,
        );
        let node = read((r as u32 as *mut u8).cast::<u32>());
        write(out, node);
        let edx = if copy_edx { node } else { (r >> 32) as u32 };
        (u64::from(edx) << 32) | u64::from(out as u32)
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn insert_at(
    this: *mut u8,
    out: *mut u32,
    child: *mut u8,
    parent: *mut u8,
    source: *mut u8,
    force: u32,
    r: u64,
) -> u64 {
    // SAFETY: the hint validation selected these original insertion arguments.
    let r = unsafe {
        call(
            0x0069_A610,
            Conv::Thiscall,
            this as u32,
            &[out as u32, child as u32, parent as u32, source as u32, force],
            r,
        )
    };
    (r & !0xFFFF_FFFF) | u64::from(out as u32)
}

unsafe fn after(this: *mut u8, out: *mut u32, hint: *mut u8, successor: *mut u8, source: *mut u8, r: u64) -> u64 {
    // SAFETY: the adjacent nodes and source belong to the hinted insertion.
    unsafe {
        if Node::from_ptr(hint).right().is_null() {
            insert_at(this, out, null_mut(), hint, source, hint as u32, r)
        } else {
            insert_at(this, out, successor, successor, source, 0, r)
        }
    }
}

port! {
    /// Uses an unsigned-key hint when adjacent keys leave room, returns equal keys, or searches anew.
    #[port(address = 0x0069_A8F0, owner = "building", name = "InsertHintedNestedPairTree")]
    pub unsafe extern "fastcall" fn fc4p_0069a8f0(this: *mut u8, entry_edx: u32, out: *mut u32, hint: *mut u8, source: *mut u8) -> u64 {
        // SAFETY: the original map, hint, source pair and writable output iterator.
        unsafe {
            let map = Tree::from_ptr(this);
            let header = map.header();
            let mut r = (u64::from(entry_edx) << 32) | u64::from(header as u32);
            if hint == Node::from_ptr(header).left() {
                if map.count() == 0 { return fallback(this, out, source, r & !0xFFFF_FFFF, true); }
                let key = read(source.cast::<u32>());
                let current_key = Node::from_ptr(hint).key();
                r = (r & !0xFFFF_FFFF) | u64::from(current_key);
                if key < current_key { return insert_at(this, out, hint, hint, source, 0, r); }
                if key == current_key { write(out, hint as u32); return (r & !0xFFFF_FFFF) | u64::from(out as u32); }
                let mut successor = hint;
                r = call(0x005D_17C0, Conv::Thiscall, &mut successor as *mut _ as u32, &[], r);
                r = (r & !0xFFFF_FFFF) | u64::from(successor as u32);
                if successor == map.header() { return insert_at(this, out, null_mut(), hint, source, hint as u32, r); }
                if key < Node::from_ptr(successor).key() { return after(this, out, hint, successor, source, r); }
                return fallback(this, out, source, r, false);
            }
            if hint == header {
                let last = Node::from_ptr(header).right();
                let last_key = Node::from_ptr(last).key();
                r = (u64::from(last_key) << 32) | u64::from(source as u32);
                if last_key < read(source.cast::<u32>()) { return insert_at(this, out, null_mut(), last, source, hint as u32, r); }
                return fallback(this, out, source, r, false);
            }
            let mut adjacent = hint;
            r = call(0x0068_09E0, Conv::Thiscall, &mut adjacent as *mut _ as u32, &[], r);
            let key = read(source.cast::<u32>());
            let less = key < Node::from_ptr(hint).key();
            if less && Node::from_ptr(adjacent).key() < key {
                return if Node::from_ptr(adjacent).right().is_null() {
                    insert_at(this, out, null_mut(), adjacent, source, adjacent as u32, r)
                } else { insert_at(this, out, hint, hint, source, 0, r) };
            }
            adjacent = hint;
            r = call(0x005D_17C0, Conv::Thiscall, &mut adjacent as *mut _ as u32, &[], r);
            r = (r & !(255u64 << 32)) | (u64::from(less) << 32);
            if !less {
                if Node::from_ptr(hint).key() >= key {
                    write(out, hint as u32);
                    return (r & !0xFFFF_FFFF) | u64::from(out as u32);
                }
                if adjacent == map.header() || key < Node::from_ptr(adjacent).key() {
                    return after(this, out, hint, adjacent, source, r);
                }
            }
            fallback(this, out, source, r, false)
        }
    }
}

unsafe fn dispose_tree(tree: *mut u8, mut r: u64) -> u64 {
    // SAFETY: each temporary tree owns its header and any copied subtree.
    unsafe {
        let t = Tree::from_ptr(tree);
        if t.count() != 0 {
            let h = Node::from_ptr(t.header());
            call(
                0x0069_5E10,
                Conv::Thiscall,
                tree as u32,
                &[h.parent() as u32],
                (r & !0xFFFF_FFFF) | u64::from(t.header() as u32),
            );
            let header = Node::from_ptr(t.header());
            header.set_left(t.header());
            header.set_parent(null_mut());
            header.set_right(t.header());
            t.set_count(0);
            r = (u64::from(t.header() as u32) << 32) | u64::from(t.header() as u32);
        } else {
            r = (r & !0xFFFF_FFFF) | u64::from(t.header() as u32);
        }
        if !t.header().is_null() {
            r = call(0x0090_CF63, Conv::Cdecl, 0, &[t.header() as u32], r);
        }
        r
    }
}

port! {
    /// Returns a developer's tried-tract tree, constructing and inserting an empty value when absent.
    /// Both temporary copies are disposed even when insertion finds an existing key.
    #[port(address = 0x0069_AB40, owner = "building", name = "GetOrInsertTriedTractTree")]
    pub unsafe extern "fastcall" fn fc4p_0069ab40(this: *mut u8, entry_edx: u32, key: *mut u32) -> u64 {
        // SAFETY: the original tree and key, game allocator, copier and insertion helpers.
        unsafe {
            let map = Tree::from_ptr(this);
            let header = map.header();
            let mut node = Node::from_ptr(header).parent();
            let mut candidate = header;
            let mut edx = entry_edx;
            if !node.is_null() {
                edx = read(key);
                while !node.is_null() {
                    let n = Node::from_ptr(node);
                    if n.key() >= edx { candidate = node; node = n.left(); } else { node = n.right(); }
                }
            }
            if candidate != header && read(key) >= Node::from_ptr(candidate).key() {
                return (u64::from(edx) << 32) | u64::from((candidate as u32).wrapping_add(20));
            }
            let mut r = call(0x0090_CF54, Conv::Cdecl, header as u32, &[24], u64::from(edx) << 32);
            let allocation = r as u32 as *mut u8;
            let h = Node::from_ptr(allocation);
            h.set_color(0); h.set_parent(null_mut()); h.set_left(allocation); h.set_right(allocation);
            let mut empty = [allocation as u32, 0];
            let mut pair = MaybeUninit::<[u32; 3]>::uninit();
            let pair_ptr = pair.as_mut_ptr().cast::<u8>();
            write(pair_ptr.cast::<u32>(), read(key));
            r = call(0x0069_8A80, Conv::Thiscall, pair_ptr.add(4) as u32, &[empty.as_mut_ptr() as u32],
                (u64::from(read(key)) << 32) | u64::from(empty.as_mut_ptr() as u32));
            let mut result = MaybeUninit::<u32>::uninit();
            r = call(0x0069_A8F0, Conv::Thiscall, this as u32,
                &[result.as_mut_ptr() as u32, candidate as u32, pair_ptr as u32], r);
            let inserted = result.assume_init();
            r = dispose_tree(pair_ptr.add(4), r);
            r = dispose_tree(empty.as_mut_ptr().cast(), r);
            (r & !0xFFFF_FFFF) | u64::from(inserted.wrapping_add(20))
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0069a8f0::PORT, fc4p_0069ab40::PORT];

#[cfg(test)]
mod tests;
