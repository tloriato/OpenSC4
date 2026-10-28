// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Tree insertion, range erasure, and copies of developer and tract maps.

use crate::abi::{Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(node: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn rebalance(node: *mut u8, root: *mut *mut u8) -> u64 = "C" 0x0040_D5E0; }
game_fn! { #[allow(dead_code)] fn unlink(node: *mut u8, root: *mut *mut u8, first: *mut *mut u8, last: *mut *mut u8) -> u64 = "C" 0x0040_DAF0; }
game_fn! { #[allow(dead_code)] fn delete_subtree(this: *mut u8, root: *mut u8) -> u64 = "thiscall" 0x0069_5CD0; }
game_fn! { #[allow(dead_code)] fn predecessor(iterator: *mut u8) -> u64 = "thiscall" 0x0068_09E0; }
game_fn! { #[allow(dead_code)] fn insert_pair(this: *mut u8, out: *mut u8, child: *mut u8, parent: *mut u8, value: *mut u8, force: u32) -> u64 = "thiscall" 0x0069_6060; }
game_fn! { #[allow(dead_code)] fn insert_signed(this: *mut u8, out: *mut u8, child: *mut u8, parent: *mut u8, value: *mut u8, force: u32) -> u64 = "thiscall" 0x006C_03E0; }
game_fn! { #[allow(dead_code)] fn insert_nested(this: *mut u8, out: *mut u8, child: *mut u8, parent: *mut u8, value: *mut u8, force: u32) -> u64 = "thiscall" 0x0069_A610; }
game_fn! { #[allow(dead_code)] fn copy_subtree(this: *mut u8, source: *mut u8, parent: *mut u8) -> u64 = "thiscall" 0x0069_6F10; }
game_fn! { #[allow(dead_code)] fn copy_pairs(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0069_8A80; }
game_fn! { fn construct_element(this: *mut u8, source: *mut u8) = "thiscall" 0x004C_9470; }
game_fn! { fn construct_range(begin: *mut u8, end: *mut u8, dest: *mut u8, cookie: *mut u8) -> *mut u8 = "C" 0x0069_9150; }
game_fn! { fn destroy_range(begin: *mut u8, end: *mut u8, cookie: *mut u8) = "C" 0x0045_9170; }

layout! {
    /// A tree's header pointer and wrapping element count.
    pub struct Tree {
        0x00 => header / set_header: *mut u8,
        0x04 => count / set_count: u32,
    }
}
layout! {
    /// A node's color byte, links and eight-byte payload. Padding is retained.
    pub struct Node {
        0x00 => color / set_color: u8,
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: u32,
        0x14 => value / set_value: u32,
    }
}

/// Entry saves ECX/EAX/EDX immediately before the return address. Arguments remain in their original
/// caller-owned slots, including the second argument used as iterator scratch by unique insertion.
#[derive(Clone, Copy)]
struct Frame(*mut u32);
impl Frame {
    unsafe fn this(self) -> *mut u8 {
        unsafe { self.0.read() as *mut u8 }
    }
    unsafe fn arg(self, n: usize) -> u32 {
        unsafe { self.0.add(4 + n).read() }
    }
    unsafe fn slot(self, n: usize) -> *mut u32 {
        unsafe { self.0.add(4 + n) }
    }
    unsafe fn registers(self) -> [u32; 3] {
        unsafe { [self.0.add(1).read(), self.0.read(), self.0.add(2).read()] }
    }
    unsafe fn finish(self, regs: [u32; 3]) -> u64 {
        unsafe { self.0.write(regs[1]) };
        (u64::from(regs[2]) << 32) | u64::from(regs[0])
    }
}

// The entry adapters only expose the original argument slots and restore the three return registers.
macro_rules! tree_port {
    ($address:literal, $export:ident, $name:literal, $ret:literal, $body:ident) => {
        port! {
            #[doc = $name]
            #[port(address = $address, owner = "building", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push eax", "push ecx", "mov ecx, esp", "call {body}",
                    "pop ecx", "add esp, 8", $ret, body = sym $body,
                );
            }
        }
    };
}

/// Like a raw game call, also retaining the callee's ECX for iterator helpers that expose it.
unsafe fn call(address: u32, conv: Conv, regs: [u32; 3], args: &[u32]) -> [u32; 3] {
    let mut c = RawCall::new(conv, crate::abi::target(address), regs[1], args);
    c.eax = regs[0];
    c.edx = regs[2];
    // SAFETY: callers declare the matching game function and supply its original convention/arguments.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov eax, [edi + 20]", "lea esp, [esp + eax*4]", "4:",
            in("edi") &mut c, clobber_abi("C"),
        );
    }
    [c.eax, c.ecx, c.edx]
}

unsafe fn node(p: *mut u8) -> Node {
    unsafe { Node::from_ptr(p) }
}
unsafe fn tree(p: *mut u8) -> Tree {
    unsafe { Tree::from_ptr(p) }
}

/// Allocates, links and rebalances an eight-byte or nested-tree payload.
unsafe fn insert(f: Frame, pair: bool, nested: bool) -> u64 {
    unsafe {
        let map = tree(f.this());
        let out = f.arg(0) as *mut u32;
        let child = f.arg(1);
        let parent = f.arg(2) as *mut u8;
        let source = f.arg(3) as *mut u8;
        let force = f.arg(4);
        let goes_left = parent == map.header()
            || (force == 0
                && (child != 0
                    || if pair {
                        (read(source.cast::<i32>()), read(source.add(4).cast::<i32>()))
                            < (node(parent).key() as i32, node(parent).value() as i32)
                    } else {
                        read(source.cast::<u32>()) < node(parent).key()
                    }));
        let mut r = call(0x0090_CF54, Conv::Cdecl, f.registers(), &[if nested { 32 } else { 24 }]);
        let p = r[0] as *mut u8;
        if (p as u32).wrapping_add(16) != 0 {
            write(p.add(16).cast::<u32>(), read(source.cast::<u32>()));
            if nested {
                r = call(
                    0x0069_8A80,
                    Conv::Thiscall,
                    [(p as u32).wrapping_add(16), (p as u32).wrapping_add(20), r[2]],
                    &[source.wrapping_add(4) as u32],
                );
            } else {
                r[2] = read(source.add(4).cast::<u32>());
                write(p.add(20).cast::<u32>(), r[2]);
            }
        }
        if goes_left {
            node(parent).set_left(p);
            let h = map.header();
            if parent == h {
                node(h).set_parent(p);
                node(map.header()).set_right(p);
            } else if parent == node(h).left() {
                node(h).set_left(p);
            }
        } else {
            node(parent).set_right(p);
            let h = map.header();
            if parent == node(h).right() {
                node(h).set_right(p);
            }
        }
        node(p).set_parent(parent);
        node(p).set_left(core::ptr::null_mut());
        node(p).set_right(core::ptr::null_mut());
        let h = map.header();
        let root = h.wrapping_add(4);
        r = call(
            0x0040_D5E0,
            Conv::Cdecl,
            [
                if nested { 0 } else { p as u32 },
                if nested { root as u32 } else { h as u32 },
                if nested { r[2] } else { root as u32 },
            ],
            &[p as u32, root as u32],
        );
        map.set_count(map.count().wrapping_add(1));
        write(out, p as u32);
        r[0] = out as u32;
        f.finish(r)
    }
}

tree_port!(
    0x0069_6060,
    fc4p_00696060,
    "InsertSignedPairNode",
    "ret 20",
    insert_pair_body
);
unsafe extern "thiscall" fn insert_pair_body(p: *mut u32) -> u64 {
    unsafe { insert(Frame(p), true, false) }
}
tree_port!(
    0x0069_6880,
    fc4p_00696880,
    "InsertUnsignedNode",
    "ret 20",
    insert_unsigned_body
);
unsafe extern "thiscall" fn insert_unsigned_body(p: *mut u32) -> u64 {
    unsafe { insert(Frame(p), false, false) }
}
tree_port!(
    0x0069_A610,
    fc4p_0069a610,
    "InsertNestedPairTreeNode",
    "ret 20",
    insert_nested_body
);
unsafe extern "thiscall" fn insert_nested_body(p: *mut u32) -> u64 {
    unsafe { insert(Frame(p), false, true) }
}

/// Unique insertion uses the caller's key-pointer slot as its predecessor iterator.
unsafe fn unique(f: Frame, pair: bool, nested: bool) -> u64 {
    unsafe {
        let this = f.this();
        let map = tree(this);
        let header = map.header();
        let source = f.arg(1) as *mut u8;
        let out = f.arg(0) as *mut u8;
        let mut current = node(header).parent();
        let mut parent = header;
        let mut less = true;
        while !current.is_null() {
            parent = current;
            less = if pair {
                (read(source.cast::<i32>()), read(source.add(4).cast::<i32>()))
                    < (node(current).key() as i32, node(current).value() as i32)
            } else if nested {
                read(source.cast::<u32>()) < node(current).key()
            } else {
                read(source.cast::<i32>()) < node(current).key() as i32
            };
            current = if less {
                node(current).left()
            } else {
                node(current).right()
            };
        }
        let scratch = f.slot(1);
        write(scratch, parent as u32);
        let mut r = f.registers();
        r[1] = header as u32;
        let shortcut = less && parent == node(header).left();
        let mut should_insert = false;
        if less {
            if parent == node(header).left() {
                should_insert = true;
            } else {
                r = call(0x0068_09E0, Conv::Thiscall, [r[0], scratch as u32, parent as u32], &[]);
            }
        }
        let candidate = read(scratch) as *mut u8;
        if !should_insert {
            should_insert = if pair {
                (node(candidate).key() as i32, node(candidate).value() as i32)
                    < (read(source.cast::<i32>()), read(source.add(4).cast::<i32>()))
            } else if nested {
                node(candidate).key() < read(source.cast::<u32>())
            } else {
                (node(candidate).key() as i32) < read(source.cast::<i32>())
            };
        }
        if should_insert {
            r = call(
                if pair {
                    0x0069_6060
                } else if nested {
                    0x0069_A610
                } else {
                    0x006C_03E0
                },
                Conv::Thiscall,
                [r[0], this as u32, candidate as u32],
                &[scratch as u32, current as u32, parent as u32, source as u32, 0],
            );
            let inserted = read(r[0] as *const u32);
            write(out.cast::<u32>(), inserted);
            write(out.add(4), 1);
            // The pair helper always loads EDX; the other helpers load ECX on the minimum shortcut.
            if pair || !shortcut {
                r[2] = inserted;
            } else {
                r[1] = inserted;
            }
        } else {
            write(out.cast::<u32>(), candidate as u32);
            write(out.add(4), 0);
            if pair {
                r[1] = candidate as u32;
                r[2] = read(source.cast::<u32>());
            } else {
                r[2] = candidate as u32;
            }
        }
        r[0] = out as u32;
        f.finish(r)
    }
}
tree_port!(
    0x0069_6300,
    fc4p_00696300,
    "InsertUniqueSignedPair",
    "ret 8",
    unique_pair_body
);
unsafe extern "thiscall" fn unique_pair_body(p: *mut u32) -> u64 {
    unsafe { unique(Frame(p), true, false) }
}
tree_port!(
    0x0069_67C0,
    fc4p_006967c0,
    "InsertUniqueSignedKey",
    "ret 8",
    unique_signed_body
);
unsafe extern "thiscall" fn unique_signed_body(p: *mut u32) -> u64 {
    unsafe { unique(Frame(p), false, false) }
}
tree_port!(
    0x0069_A6E0,
    fc4p_0069a6e0,
    "InsertUniqueNestedPairTree",
    "ret 8",
    unique_nested_body
);
unsafe extern "thiscall" fn unique_nested_body(p: *mut u32) -> u64 {
    unsafe { unique(Frame(p), false, true) }
}

/// Advances an iterator using child and parent links, including the header's self links.
unsafe fn successor(mut p: *mut u8) -> *mut u8 {
    unsafe {
        let right = node(p).right();
        if !right.is_null() {
            p = right;
            while !node(p).left().is_null() {
                p = node(p).left();
            }
        } else {
            let mut parent = node(p).parent();
            while p == node(parent).right() {
                p = parent;
                parent = node(parent).parent();
            }
            if node(p).right() != parent {
                p = parent;
            }
        }
        p
    }
}

tree_port!(0x0069_6240, fc4p_00696240, "EraseDeveloperRange", "ret 8", erase_body);
unsafe extern "thiscall" fn erase_body(p: *mut u32) -> u64 {
    unsafe {
        let f = Frame(p);
        let map = tree(f.this());
        let mut first = f.arg(0) as *mut u8;
        let last = f.arg(1) as *mut u8;
        let h = map.header();
        let mut r = [h as u32, f.this() as u32, f.registers()[2]];
        if first == node(h).left() && last == h {
            r[1] = map.count();
            if map.count() != 0 {
                r = call(
                    0x0069_5CD0,
                    Conv::Thiscall,
                    [node(h).parent() as u32, f.this() as u32, r[2]],
                    &[node(h).parent() as u32],
                );
                node(map.header()).set_left(map.header());
                node(map.header()).set_parent(core::ptr::null_mut());
                let h = map.header();
                node(h).set_right(h);
                map.set_count(0);
                r[0] = h as u32;
                r[1] = map.header() as u32;
            }
        } else {
            while first != last {
                let erase = first;
                first = successor(first);
                let h = map.header();
                r = call(
                    0x0040_DAF0,
                    Conv::Cdecl,
                    [h as u32 + 4, erase as u32, h as u32 + 8],
                    &[erase as u32, h as u32 + 4, h as u32 + 8, h as u32 + 12],
                );
                if r[0] != 0 {
                    r = call(0x0090_CF63, Conv::Cdecl, r, &[r[0]]);
                }
                map.set_count(map.count().wrapping_sub(1));
                r[1] = map.count();
            }
        }
        f.finish(r)
    }
}

tree_port!(
    0x0069_6F10,
    fc4p_00696f10,
    "CopyEightByteTreeSubtree",
    "ret 8",
    copy_body
);
unsafe extern "thiscall" fn copy_body(p: *mut u32) -> u64 {
    unsafe {
        let f = Frame(p);
        let this = f.this();
        let mut source = f.arg(0) as *mut u8;
        let parent = f.arg(1) as *mut u8;
        let mut r = call(0x0090_CF54, Conv::Cdecl, f.registers(), &[24]);
        let root = r[0] as *mut u8;
        if (root as u32).wrapping_add(16) != 0 {
            node(root).set_key(node(source).key());
            r[2] = node(source).value();
            node(root).set_value(r[2]);
        }
        node(root).set_color(node(source).color());
        node(root).set_left(core::ptr::null_mut());
        node(root).set_right(core::ptr::null_mut());
        node(root).set_parent(parent);
        if !node(source).right().is_null() {
            r = call(
                0x0069_6F10,
                Conv::Thiscall,
                [node(source).right() as u32, this as u32, r[2]],
                &[node(source).right() as u32, root as u32],
            );
            node(root).set_right(r[0] as *mut u8);
        }
        source = node(source).left();
        let mut previous = root;
        while !source.is_null() {
            r = call(0x0090_CF54, Conv::Cdecl, r, &[24]);
            let dest = r[0] as *mut u8;
            if (dest as u32).wrapping_add(16) != 0 {
                node(dest).set_key(node(source).key());
                r[2] = node(source).value();
                node(dest).set_value(r[2]);
            }
            node(dest).set_color(node(source).color());
            node(dest).set_left(core::ptr::null_mut());
            node(dest).set_right(core::ptr::null_mut());
            node(previous).set_left(dest);
            node(dest).set_parent(previous);
            if !node(source).right().is_null() {
                r = call(
                    0x0069_6F10,
                    Conv::Thiscall,
                    [node(source).right() as u32, this as u32, r[2]],
                    &[node(source).right() as u32, dest as u32],
                );
                node(dest).set_right(r[0] as *mut u8);
            }
            source = node(source).left();
            previous = dest;
        }
        r[0] = root as u32;
        r[1] = this as u32;
        f.finish(r)
    }
}

unsafe fn copy_tree(f: Frame) -> u64 {
    unsafe {
        let map = tree(f.this());
        let source = tree(f.arg(0) as *mut u8);
        map.set_header(core::ptr::null_mut());
        let mut r = call(0x0090_CF54, Conv::Cdecl, f.registers(), &[24]);
        let header = r[0] as *mut u8;
        map.set_header(header);
        map.set_count(0);
        node(header).set_color(0);
        r[1] = source.header() as u32;
        if node(source.header()).parent().is_null() {
            node(map.header()).set_parent(core::ptr::null_mut());
            let h = map.header();
            node(h).set_left(h);
            let h = map.header();
            node(h).set_right(h);
        } else {
            r = call(
                0x0069_6F10,
                Conv::Thiscall,
                [
                    source.header() as u32,
                    f.this() as u32,
                    node(source.header()).parent() as u32,
                ],
                &[node(source.header()).parent() as u32, header as u32],
            );
            node(header).set_parent(r[0] as *mut u8);
            let h = map.header();
            let mut first = node(h).parent();
            while !node(first).left().is_null() {
                first = node(first).left();
            }
            node(h).set_left(first);
            let h = map.header();
            let mut last = node(h).parent();
            while !node(last).right().is_null() {
                last = node(last).right();
            }
            node(h).set_right(last);
            r[1] = last as u32;
        }
        let count = source.count();
        map.set_count(count);
        r[0] = f.this() as u32;
        r[2] = count;
        f.finish(r)
    }
}
tree_port!(
    0x0069_89D0,
    fc4p_006989d0,
    "CopyDeveloperTree",
    "ret 4",
    copy_developers_body
);
unsafe extern "thiscall" fn copy_developers_body(p: *mut u32) -> u64 {
    unsafe { copy_tree(Frame(p)) }
}
tree_port!(
    0x0069_8A80,
    fc4p_00698a80,
    "CopySignedPairTree",
    "ret 4",
    copy_pairs_body
);
unsafe extern "thiscall" fn copy_pairs_body(p: *mut u32) -> u64 {
    unsafe { copy_tree(Frame(p)) }
}

tree_port!(
    0x0069_A7C0,
    fc4p_0069a7c0,
    "GrowAndInsertObjectVector",
    "ret 20",
    grow_vector_body
);
unsafe extern "thiscall" fn grow_vector_body(p: *mut u32) -> u64 {
    unsafe {
        let f = Frame(p);
        let vector = super::shared::ObjectVector::from_ptr(f.this());
        let old_count = (vector.end() as u32).wrapping_sub(vector.begin() as u32) as i32 / 12;
        let requested = f.arg(3);
        let count = (old_count as u32).wrapping_add(core::cmp::max(old_count as u32, requested));
        let allocation = if count == 0 {
            core::ptr::null_mut()
        } else {
            call(0x0090_CF54, Conv::Cdecl, f.registers(), &[count.wrapping_mul(12)])[0] as *mut u8
        };
        let position = f.arg(0) as *mut u8;
        let value = f.arg(1) as *mut u8;
        let mut source = vector.begin();
        let mut dest = allocation;
        while source != position {
            if !dest.is_null() {
                construct_element(dest, source);
            }
            source = source.wrapping_add(12);
            dest = dest.wrapping_add(12);
        }
        for _ in 0..requested {
            if !dest.is_null() {
                construct_element(dest, value);
            }
            dest = dest.wrapping_add(12);
        }
        let cookie = f.slot(4).cast::<u8>();
        if *cookie == 0 {
            dest = construct_range(position, vector.end(), dest, cookie);
        }
        destroy_range(vector.begin(), vector.end(), cookie);
        let old = vector.begin();
        if !old.is_null() {
            call(0x0090_CF63, Conv::Cdecl, f.registers(), &[old as u32]);
        }
        let limit = allocation.wrapping_add(count.wrapping_mul(12) as usize);
        vector.set_end(dest);
        vector.set_begin(allocation);
        vector.set_limit(limit);
        f.finish([allocation as u32, count.wrapping_mul(3), limit as u32])
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00696060::PORT,
    fc4p_00696240::PORT,
    fc4p_00696300::PORT,
    fc4p_006967c0::PORT,
    fc4p_00696880::PORT,
    fc4p_0069a610::PORT,
    fc4p_0069a6e0::PORT,
    fc4p_00696f10::PORT,
    fc4p_006989d0::PORT,
    fc4p_00698a80::PORT,
    fc4p_0069a7c0::PORT,
];

#[cfg(test)]
mod tests;
