// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cell lookups, bridge insertion, and operations on hashed network sets.

use super::intersection_storage::call_registers;
use super::*;
use crate::abi::{Conv, RawCall};

// Keep the saved ECX word beside the entry frame so Rust can publish the original's final value.
macro_rules! ecx_register_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $ret:literal) => {
        port! {
            #[port(address = $address, owner = "networktool", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push eax", "push edx", "push ecx", "lea edx, [esp + 4]",
                    "call {body}", "pop ecx", "add esp, 8", $ret, body = sym $body,
                )
            }
        }
    };
}

game_fn! { #[allow(dead_code)] fn find_cell(this: *mut u8, key: u32) -> u64 = "thiscall" 0x0064_7A20; }
game_fn! { #[allow(dead_code)] fn compute_cell(this: *mut u8, key: u32) -> u64 = "thiscall" 0x0063_3030; }
game_fn! { #[allow(dead_code)] fn get_cell(this: *mut u8, key: u32) -> u64 = "thiscall" 0x0063_3220; }
game_fn! { #[allow(dead_code)] fn insert_bridge(this: *mut u8, piece: *mut u8, cell: *mut u8, descriptor: *mut u8, context: u32) -> u64 = "thiscall" 0x0062_EEB0; }
game_fn! { #[allow(dead_code)] fn begin_set(this: *mut u8, output: *mut u32) -> u64 = "thiscall" 0x004B_0390; }
game_fn! { #[allow(dead_code)] fn erase_key(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0045_D460; }
game_fn! { #[allow(dead_code)] fn resize_set(this: *mut u8, count: u32) -> u64 = "thiscall" 0x004F_0940; }
game_fn! { #[allow(dead_code)] fn insert_key(this: *mut u8, output: *mut u32, key: *mut u32) -> u64 = "thiscall" 0x0062_4DD0; }
game_fn! { #[allow(dead_code)] fn padded_bounds(this: *mut u8, x: u32, z: u32, output: *mut u32, padding: u32) -> u64 = "thiscall" 0x0062_3C20; }
game_fn! { #[allow(dead_code)] fn grow_words(this: *mut u8, position: u32, value: *mut u32, scratch: *mut u32, count: u32, stride: u32) -> u64 = "thiscall" 0x0051_CA60; }

unsafe fn invoke(c: &mut RawCall, address: u32, this: u32, args: &[u32]) {
    c.target = crate::abi::target(address);
    c.ecx = this;
    c.args = args.as_ptr();
    c.count = args.len() as u32;
    c.caller_pops = 0;
    unsafe { call_registers(c) };
}

fn remainder(value: u32, divisor: u32) -> u32 {
    let result;
    // The game uses unsigned division, including its exception for a zero bucket count.
    unsafe {
        core::arch::asm!("div {divisor:e}", divisor = in(reg) divisor,
            inout("eax") value => _, inout("edx") 0u32 => result, options(nomem, nostack));
    }
    result
}

unsafe fn bucket_count(this: *mut u8) -> u32 {
    unsafe { (read::<u32>(this, 8).wrapping_sub(read(this, 4)) as i32 >> 2) as u32 }
}

ecx_register_port!(
    0x0063_3220,
    fc4p_00633220,
    "cSC4NetworkTool::GetCellInfo",
    lookup_cell,
    "ret 4"
);
unsafe extern "fastcall" fn lookup_cell(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let key = *frame.add(3);
        let mut c = RawCall::new(Conv::Thiscall, 0, 0, &[]);
        c.eax = *frame.add(1);
        c.edx = *frame;
        invoke(&mut c, 0x0064_7A20, this.wrapping_add(0x1C) as u32, &[key]);
        if c.eax == 0 {
            invoke(&mut c, 0x0063_3030, this as u32, &[key]);
        }
        frame.sub(1).write(c.ecx);
        registers(c.eax, c.edx)
    }
}

register_port!(
    0x0062_F610,
    fc4p_0062f610,
    "cSC4NetworkTool::InsertBridgePieces",
    insert_bridges,
    "ret 4"
);
unsafe extern "fastcall" fn insert_bridges(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let mut record = tool.bridge_begin();
        let end = tool.bridge_end();
        let mut c = RawCall::new(Conv::Thiscall, 0, end as u32, &[]);
        c.eax = record as u32;
        c.edx = *frame;
        while record != end {
            c.edx = read(record, 8);
            if c.edx as i32 >= tool.valid_steps() {
                break;
            }
            let index: u32 = read(record, 12);
            c.edx = tool.type_info() as u32;
            let descriptor = (read::<u32>(tool.type_info(), 0x30)).wrapping_add(index.wrapping_mul(0x178));
            let mut piece: *mut u8 = read(record, 0x1C);
            let pieces_end: *mut u8 = read(record, 0x20);
            while piece != pieces_end {
                c.edx = read(piece, 4);
                c.eax = read::<u32>(piece, 8).wrapping_shl(16).wrapping_add(c.edx);
                let key = c.eax;
                invoke(&mut c, 0x0063_3220, this as u32, &[key]);
                let cell = c.eax;
                write(cell as *mut u8, 0x51, 1u8);
                invoke(
                    &mut c,
                    0x0062_EEB0,
                    this as u32,
                    &[piece as u32, cell, descriptor, *frame.add(3)],
                );
                piece = piece.wrapping_add(0x50);
            }
            record = record.wrapping_add(0x28);
            c.eax = record as u32;
        }
        registers(c.eax, c.edx)
    }
}

port! {
    /// Find a node keyed by two words, using the second plus twice the first as its hash.
    #[port(address = 0x0063_3970, owner = "networktool", name = "FindNetworkPairNode")]
    pub unsafe extern "thiscall" fn fc4p_00633970(this: *mut u8, key: *const u32) -> u64 {
        unsafe {
            let first = key.read_unaligned();
            let second = key.add(1).read_unaligned();
            let index = remainder(second.wrapping_add(first.wrapping_mul(2)), bucket_count(this));
            let mut node: *mut u8 = read(read(this, 4), index.wrapping_mul(4) as usize);
            while !node.is_null() {
                if read::<u32>(node, 4) == first && read::<u32>(node, 8) == second { break; }
                node = read(node, 0);
            }
            registers(node as u32, index)
        }
    }
}

ecx_register_port!(
    0x0063_39B0,
    fc4p_006339b0,
    "EqualNetworkKeyRange",
    equal_key_range,
    "ret 8"
);
/// Return the contiguous equal-key range, crossing to the next occupied bucket at its end.
unsafe extern "fastcall" fn equal_key_range(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let output = *frame.add(3) as *mut u8;
        let key = *frame.add(4) as *const u32;
        let key = key.read_unaligned();
        let buckets: *mut u8 = read(this, 4);
        let index = remainder(key, bucket_count(this));
        let mut node: *mut u8 = read(buckets, index.wrapping_mul(4) as usize);
        while !node.is_null() && read::<u32>(node, 4) != key {
            node = read(node, 0);
        }
        let mut next: *mut u8 = core::ptr::null_mut();
        if !node.is_null() {
            next = read(node, 0);
            while !next.is_null() && read::<u32>(next, 4) == key {
                next = read(next, 0);
            }
            if next.is_null() {
                let mut index = index.wrapping_add(1);
                while index < bucket_count(this) {
                    next = read(buckets, index.wrapping_mul(4) as usize);
                    if !next.is_null() {
                        break;
                    }
                    index = index.wrapping_add(1);
                }
            } else {
                write(output, 8, next);
                write(output, 0, node);
                write(output, 4, this);
                write(output, 12, this);
                return registers(output as u32, index);
            }
        }
        write(output, 0, node);
        write(output, 8, next);
        write(output, 4, this);
        write(output, 12, this);
        registers(output as u32, next as u32)
    }
}

register_port!(
    0x0063_3ED0,
    fc4p_00633ed0,
    "RemoveVectorFromNetworkSet",
    remove_vector,
    "ret"
);
unsafe extern "fastcall" fn remove_vector(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let set = *frame.add(3);
        let vector = *frame.add(4) as *mut u8;
        let mut first: *mut u8 = read(vector, 0);
        let end: *mut u8 = read(vector, 4);
        let mut c = RawCall::new(Conv::Cdecl, 0, this as u32, &[]);
        c.eax = vector as u32;
        c.edx = *frame;
        while first != end {
            c.eax = read(first, 0);
            *frame.add(4) = c.eax;
            invoke(&mut c, 0x0045_D460, set, &[frame.add(4) as u32]);
            first = first.wrapping_add(4);
        }
        registers(c.eax, c.edx)
    }
}

unsafe fn update_set(this: *mut u8, frame: *mut u32, add: bool) -> u64 {
    unsafe {
        let destination = *frame.add(3) as *mut u8;
        let source = *frame.add(4) as *mut u8;
        let mut iterator = core::mem::MaybeUninit::<[u32; 2]>::uninit();
        let mut insertion = core::mem::MaybeUninit::<[u32; 3]>::uninit();
        let mut c = RawCall::new(Conv::Cdecl, 0, this as u32, &[]);
        c.eax = iterator.as_mut_ptr() as u32;
        c.edx = *frame;
        invoke(&mut c, 0x004B_0390, source as u32, &[iterator.as_mut_ptr() as u32]);
        let mut node = (*iterator.as_ptr())[0] as *mut u8;
        if node.is_null() {
            return registers(c.eax, c.edx);
        }
        let source = (*iterator.as_ptr())[1] as *mut u8;
        loop {
            let mut key = core::ptr::null_mut::<u8>();
            while !node.is_null() {
                if add {
                    invoke(
                        &mut c,
                        0x004F_0940,
                        destination as u32,
                        &[read::<u32>(destination, 0x10).wrapping_add(1)],
                    );
                }
                key = node.wrapping_add(4);
                if add {
                    c.edx = insertion.as_mut_ptr() as u32;
                    invoke(
                        &mut c,
                        0x0062_4DD0,
                        destination as u32,
                        &[insertion.as_mut_ptr() as u32, key as u32],
                    );
                } else {
                    invoke(&mut c, 0x0045_D460, destination as u32, &[key as u32]);
                }
                node = read(node, 0);
            }
            let index = remainder(read(key, 0), bucket_count(source));
            c.eax = bucket_count(source);
            c.edx = index.wrapping_add(1);
            while c.edx < c.eax {
                node = read(read(source, 4), c.edx.wrapping_mul(4) as usize);
                if !node.is_null() {
                    break;
                }
                c.edx = c.edx.wrapping_add(1);
            }
            if node.is_null() {
                return registers(c.eax, c.edx);
            }
        }
    }
}

register_port!(
    0x0063_3E50,
    fc4p_00633e50,
    "cSC4NetworkTool::RemoveFromSet",
    remove_set,
    "ret"
);
unsafe extern "fastcall" fn remove_set(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe { update_set(this, frame, false) }
}
register_port!(0x0063_3F10, fc4p_00633f10, "cSC4NetworkTool::AddToSet", add_set, "ret");
unsafe extern "fastcall" fn add_set(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe { update_set(this, frame, true) }
}

fn rotated(value: u8, amount: u8) -> u8 {
    if value & 0x80 == 0 {
        value.wrapping_add(amount) & 0x83
    } else {
        value.wrapping_sub(amount).wrapping_add(8) & 0x83
    }
}

unsafe fn tree_successor(mut node: *mut u8) -> *mut u8 {
    unsafe {
        let mut link: *mut u8 = read(node, 12);
        if !link.is_null() {
            node = link;
            loop {
                link = read(node, 8);
                if link.is_null() {
                    return node;
                }
                node = link;
            }
        }
        link = read(node, 4);
        while node == read::<*mut u8>(link, 12) {
            node = link;
            link = read(link, 4);
        }
        if read::<*mut u8>(node, 12) != link {
            node = link;
        }
        node
    }
}

ecx_register_port!(
    0x0063_37E0,
    fc4p_006337e0,
    "cSC4NetworkTool::PatchTilePair",
    patch_pair,
    "ret 16"
);
unsafe extern "fastcall" fn patch_pair(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tree = *frame.add(3) as *mut u8;
        let left = *frame.add(4) as *mut u8;
        let right = *frame.add(5) as *mut u8;
        let amount = (*frame.add(6) as u8).wrapping_add(2);
        let left_orientation: u8 = read(left, 4);
        let right_orientation: u8 = read(right, 4);
        frame.add(6).cast::<u8>().write(amount);
        let turn = 8u8.wrapping_sub(amount);
        let normalized_left = rotated(left_orientation, turn);
        frame.add(5).cast::<u8>().write(normalized_left);
        let normalized_right = rotated(right_orientation, turn);
        frame.add(4).cast::<u8>().write(normalized_right);
        let mut edx = if left_orientation & 0x80 == 0 {
            0
        } else {
            *frame & !0xFF
        };
        edx |= u32::from(normalized_left);
        let mut node: *mut u8 = read(tree, 0);
        let end: *mut u8 = read(tree, 4);
        while node != end {
            if read::<u32>(right, 0) == read::<u32>(node, 0x1C) {
                frame.sub(1).cast::<u8>().add(3).write(normalized_right);
                frame.add(3).cast::<u8>().write(0);
                let flip = (read::<u8>(node, 0x18) ^ normalized_left) & 0x80 != 0;
                let a = normalized_left ^ if flip { 0x82 } else { 0 };
                let b = normalized_right ^ if flip { 0x82 } else { 0 };
                edx = (edx & !0xFF) | u32::from(b);
                if a == read::<u8>(node, 0x18) && (read::<u32>(right, 0) == 0 || b == read::<u8>(node, 0x20)) {
                    let replacement: u32 = read(node, 0x24);
                    if replacement == 0 {
                        return registers(2, edx);
                    }
                    write(left, 0, replacement);
                    edx = read(node, 0x2C);
                    write(right, 0, edx);
                    write(left, 4, read::<u8>(node, 0x28));
                    let orientation: u8 = read(node, 0x30);
                    edx = (edx & !0xFF) | u32::from(orientation);
                    write(right, 4, orientation);
                    if flip {
                        write(left, 4, read::<u8>(left, 4) ^ 0x82);
                        write(right, 4, read::<u8>(right, 4) ^ 0x82);
                    }
                    write(left, 4, rotated(read(left, 4), amount));
                    write(right, 4, rotated(read(right, 4), amount));
                    return registers(1, edx);
                }
                edx = (edx & !0xFF) | u32::from(normalized_left);
            }
            node = tree_successor(node);
        }
        registers(0, edx)
    }
}

register_port!(
    0x0063_4480,
    fc4p_00634480,
    "cSC4NetworkTool::BuildPaddedNetworkSet",
    build_padded,
    "ret 12"
);
unsafe extern "fastcall" fn build_padded(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let source = *frame.add(3) as *mut u8;
        let destination = *frame.add(4) as *mut u8;
        let mut iterator = core::mem::MaybeUninit::<[u32; 2]>::uninit();
        let mut insertion = core::mem::MaybeUninit::<[u32; 3]>::uninit();
        let mut bounds = core::mem::MaybeUninit::<[u32; 4]>::uninit();
        let mut c = RawCall::new(Conv::Thiscall, 0, 0, &[]);
        c.eax = iterator.as_mut_ptr() as u32;
        c.edx = *frame;
        invoke(&mut c, 0x004B_0390, source as u32, &[iterator.as_mut_ptr() as u32]);
        let mut node = (*iterator.as_ptr())[0] as *mut u8;
        if node.is_null() {
            return registers(c.eax, c.edx);
        }
        let source = (*iterator.as_ptr())[1] as *mut u8;
        loop {
            let key: u32 = read(node, 4);
            c.eax = key & 0xFFFF;
            c.edx = bounds.as_mut_ptr() as u32;
            invoke(
                &mut c,
                0x0062_3C20,
                this as u32,
                &[key & 0xFFFF, key >> 16, bounds.as_mut_ptr() as u32, *frame.add(5)],
            );
            let rectangle = bounds.as_mut_ptr().cast::<u32>();
            let mut z = *rectangle.add(1);
            c.eax = z;
            while z <= *rectangle.add(3) {
                let mut x = *rectangle;
                c.eax = x;
                let mut key = z.wrapping_shl(16).wrapping_add(x);
                while x <= *rectangle.add(2) {
                    invoke(&mut c, 0x0063_3220, this as u32, &[key]);
                    if c.eax != 0 {
                        let cell = c.eax as *mut u8;
                        if read::<u8>(cell, 0x53) == 0 || read::<u32>(node, 4) == key {
                            c.edx = read::<u32>(destination, 0x10).wrapping_add(1);
                            let count = c.edx;
                            invoke(&mut c, 0x004F_0940, destination as u32, &[count]);
                            let mut stored_key = key;
                            c.eax = &mut stored_key as *mut u32 as u32;
                            invoke(
                                &mut c,
                                0x0062_4DD0,
                                destination as u32,
                                &[insertion.as_mut_ptr() as u32, &mut stored_key as *mut u32 as u32],
                            );
                        }
                    }
                    x = x.wrapping_add(1);
                    key = key.wrapping_add(1);
                }
                c.eax = *rectangle;
                c.edx = *rectangle.add(3);
                z = z.wrapping_add(1);
            }
            c.eax = read(node, 0);
            if c.eax != 0 {
                node = c.eax as *mut u8;
                continue;
            }
            let index = remainder(read(node, 4), bucket_count(source));
            c.eax = bucket_count(source);
            c.edx = index.wrapping_add(1);
            node = core::ptr::null_mut();
            while c.edx < c.eax {
                node = read(read(source, 4), c.edx.wrapping_mul(4) as usize);
                if !node.is_null() {
                    break;
                }
                c.edx = c.edx.wrapping_add(1);
            }
            if node.is_null() {
                return registers(c.eax, c.edx);
            }
        }
    }
}

register_port!(
    0x0062_F9D0,
    fc4p_0062f9d0,
    "ClassifyBridgeTypes",
    classify_bridge_types,
    "ret"
);
unsafe extern "fastcall" fn classify_bridge_types(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let source = *frame.add(3) as *mut u8;
        let first = *frame.add(4) as *mut u8;
        let second = *frame.add(5) as *mut u8;
        write(first, 4, read::<u32>(first, 0));
        let mut c = RawCall::new(Conv::Cdecl, 0, this as u32, &[]);
        c.eax = read(second, 0);
        c.edx = *frame;
        write(second, 4, c.eax);
        if source.is_null() {
            return registers(c.eax, c.edx);
        }
        let distance = read::<u32>(source, 4).wrapping_sub(read(source, 0)) as i32;
        let product = i64::from(distance) * i64::from(0xAE4C_415Du32 as i32);
        c.eax = product as u32;
        c.edx = (((product >> 32) as i32).wrapping_add(distance) >> 8) as u32;
        let count = (c.edx as i32).wrapping_add((c.edx >> 31) as i32);
        if count <= 0 {
            return registers(c.eax, c.edx);
        }
        *frame.add(5) = 0;
        for index in 0..count as u32 {
            let source = *frame.add(3) as *mut u8;
            let record = (read::<u32>(source, 0)).wrapping_add(*frame.add(5)) as *mut u8;
            c.eax = (c.eax & !0xFF) | u32::from(read::<u8>(record, 0x98));
            if c.eax as u8 != 0 {
                c.eax = read(second, 4);
                if c.eax != read::<u32>(second, 8) {
                    if c.eax != 0 {
                        write(c.eax as *mut u8, 0, index);
                    }
                    write(second, 4, read::<u32>(second, 4).wrapping_add(4));
                } else {
                    let mut value = index;
                    c.edx = &mut value as *mut u32 as u32;
                    let position = c.eax;
                    invoke(
                        &mut c,
                        0x0051_CA60,
                        second as u32,
                        &[position, &mut value as *mut u32 as u32, frame.add(3) as u32, 1, 1],
                    );
                }
            }
            c.eax = (c.eax & !0xFF) | u32::from(read::<u8>(record, 0x99));
            if c.eax as u8 != 0 {
                c.eax = read(first, 4);
                if c.eax != read::<u32>(first, 8) {
                    if c.eax != 0 {
                        write(c.eax as *mut u8, 0, index);
                    }
                    write(first, 4, read::<u32>(first, 4).wrapping_add(4));
                } else {
                    let mut value = index;
                    c.edx = &mut value as *mut u32 as u32;
                    let position = c.eax;
                    invoke(
                        &mut c,
                        0x0051_CA60,
                        first as u32,
                        &[position, &mut value as *mut u32 as u32, frame.add(3) as u32, 1, 1],
                    );
                }
            }
            *frame.add(5) = (*frame.add(5)).wrapping_add(0x178);
        }
        registers(c.eax, c.edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0062f610::PORT,
    fc4p_00633220::PORT,
    fc4p_00633970::PORT,
    fc4p_006339b0::PORT,
    fc4p_00633e50::PORT,
    fc4p_00633ed0::PORT,
    fc4p_00633f10::PORT,
    fc4p_006337e0::PORT,
    fc4p_00634480::PORT,
    fc4p_0062f9d0::PORT,
];
