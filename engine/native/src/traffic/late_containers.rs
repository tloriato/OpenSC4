// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tree sentinels, trip lists, segmented ranges and subnetwork array operations.

use super::network::registers;
use super::*;
use crate::abi::{Conv, RawCall};

// Capture the incoming volatile registers before entering Rust. Each body updates this small return
// record as the original operations do; the adapter retains the stack arguments and their callee pop.
macro_rules! register_entry {
    ($address:literal, $export:ident, $name:literal, $body:ident, $pop:literal, [$($offset:literal),*]) => {
        port! {
            #[port(address = $address, owner = "traffic", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push ebp", "mov ebp, esp", "sub esp, 12",
                    "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "lea edx, [esp]", $(concat!("push dword ptr [ebp + ", stringify!($offset), "]"),)*
                    "call {body}", "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
                    "mov esp, ebp", "pop ebp", concat!("ret ", stringify!($pop)), body = sym $body,
                );
            }
        }
    };
}
pub(super) use register_entry;

/// Calls an original entry with the integer register values left by preceding operations.
pub(super) unsafe fn call(address: u32, conv: Conv, args: &[u32], r: &mut Registers) {
    let mut c = RawCall::new(conv, crate::abi::target(address), r.ecx, args);
    c.eax = r.eax;
    c.edx = r.edx;
    unsafe {
        *r = registers(c);
    }
}

game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn erase_tree(this: *mut u8, root: *mut u8) = "thiscall" 0x0042_6A60; }
game_fn! { #[allow(dead_code)] fn assign_tree(this: *mut u8, source: *mut u8) = "thiscall" 0x0071_0FF0; }
game_fn! { #[allow(dead_code)] fn construct_tree(this: *mut u8, source: *mut u8) = "thiscall" 0x0071_2280; }
game_fn! { #[allow(dead_code)] fn walk_range(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, scratch: *mut u8) = "C" 0x0070_FEC0; }
game_fn! { #[allow(dead_code)] fn release_blocks(this: *mut u8, begin: u32, end: u32) = "thiscall" 0x006D_A8D0; }

layout! {
    /// Tree owner and its separately allocated sentinel.
    pub struct TreeOwner {
        0x00 => header / set_header: *mut u8,
        0x04 => count / set_count: u32,
    }
}
layout! {
    /// A trip-list node owns two independently allocated ranges.
    pub struct TripNode {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x14 => first_allocation / set_first_allocation: *mut u8,
        0x20 => second_allocation / set_second_allocation: *mut u8,
    }
}

register_entry!(0x0071_3620, fc4p_00713620, "ConstructTrafficPoolTree", construct, 0, []);
unsafe extern "fastcall" fn construct(this: *mut u8, out: *mut Registers) {
    unsafe {
        let mut r = out.read();
        let tree = TreeOwner::from_ptr(this);
        tree.set_header(core::ptr::null_mut());
        call(0x0090_CF54, Conv::Cdecl, &[24], &mut r);
        tree.set_header(r.eax as *mut u8);
        tree.set_count(0);
        (r.eax as *mut u8).write(0);
        let h = tree.header();
        store(h, 4, 0);
        let h = tree.header();
        store(h, 8, h as u32);
        let h = tree.header();
        store(h, 12, h as u32);
        r.eax = this as u32;
        out.write(r);
    }
}

register_entry!(0x0071_3660, fc4p_00713660, "DestroyTrafficPoolTree", destroy, 0, []);
unsafe extern "fastcall" fn destroy(this: *mut u8, out: *mut Registers) {
    unsafe {
        let mut r = out.read();
        let tree = TreeOwner::from_ptr(this);
        r.eax = tree.count();
        if r.eax != 0 {
            r.eax = tree.header() as u32;
            r.ecx = this as u32;
            call(0x0042_6A60, Conv::Thiscall, &[word(tree.header(), 4)], &mut r);
            r.eax = tree.header() as u32;
            store(r.eax as *mut u8, 8, r.eax);
            r.edx = tree.header() as u32;
            store(r.edx as *mut u8, 4, 0);
            r.eax = tree.header() as u32;
            store(r.eax as *mut u8, 12, r.eax);
            tree.set_count(0);
        }
        let h = tree.header();
        if !h.is_null() {
            call(0x0090_CF63, Conv::Cdecl, &[h as u32], &mut r);
        }
        out.write(r);
    }
}

register_entry!(
    0x0071_3E90,
    fc4p_00713e90,
    "DestroyTrafficSegmentedRange",
    destroy_range,
    0,
    []
);
unsafe extern "fastcall" fn destroy_range(this: *mut u8, out: *mut Registers) {
    unsafe {
        let mut r = out.read();
        let last = [word(this, 16), word(this, 20), word(this, 24), word(this, 28)];
        let first = [word(this, 0), word(this, 4), word(this, 8), word(this, 12)];
        let mut scratch = core::mem::MaybeUninit::<u8>::uninit();
        r.ecx = first[3];
        r.edx = first[1];
        call(
            0x0070_FEC0,
            Conv::Cdecl,
            &[
                first[0],
                first[1],
                first[2],
                first[3],
                last[0],
                last[1],
                last[2],
                last[3],
                scratch.as_mut_ptr() as u32,
            ],
            &mut r,
        );
        r.eax = word(this, 32);
        if r.eax != 0 {
            r.edx = word(this, 28).wrapping_add(4);
            r.eax = word(this, 12);
            r.ecx = this as u32;
            call(0x006D_A8D0, Conv::Thiscall, &[r.eax, r.edx], &mut r);
            let allocation = word(this, 32);
            if allocation != 0 {
                call(0x0090_CF63, Conv::Cdecl, &[allocation], &mut r);
            }
        }
        out.write(r);
    }
}

/// Signed division by the 28-byte record size, including the intermediate high register.
pub(super) fn record_count(bytes: i32) -> (u32, u32) {
    let high = ((i64::from(bytes) * -1_840_700_269i64) >> 32) as i32;
    let edx = high.wrapping_add(bytes) >> 4;
    (edx.wrapping_add(((edx as u32) >> 31) as i32) as u32, edx as u32)
}

unsafe fn assign_record(destination: *mut u8, source: *mut u8, r: &mut Registers, construct: bool) {
    unsafe {
        r.ecx = destination as u32;
        call(
            if construct { 0x0071_2280 } else { 0x0071_0FF0 },
            Conv::Thiscall,
            &[source as u32],
            r,
        );
        r.eax = word(source, 12);
        store(destination, 12, r.eax);
        r.ecx = word(source, 16);
        store(destination, 16, r.ecx);
        r.edx = word(source, 20);
        store(destination, 20, r.edx);
        r.eax = word(source, 24);
        store(destination, 24, r.eax);
    }
}

register_entry!(
    0x0071_4000,
    fc4p_00714000,
    "CopyTrafficSubnetworkRange",
    copy_range,
    0,
    [16, 12, 8]
);
unsafe extern "fastcall" fn copy_range(
    _this: *mut u8,
    out: *mut Registers,
    begin: *mut u8,
    end: *mut u8,
    destination: *mut u8,
) {
    unsafe {
        let mut r = out.read();
        r.ecx = (end as u32).wrapping_sub(begin as u32);
        (r.eax, r.edx) = record_count(r.ecx as i32);
        let mut source = begin;
        let mut dest = destination;
        let count = r.eax as i32;
        for _ in 0..count {
            assign_record(dest, source, &mut r, false);
            source = source.wrapping_add(28);
            dest = dest.wrapping_add(28);
        }
        r.eax = dest as u32;
        out.write(r);
    }
}

register_entry!(
    0x0071_45E0,
    fc4p_007145e0,
    "EraseTrafficTripNode",
    erase_node,
    8,
    [12, 8]
);
unsafe extern "fastcall" fn erase_node(_this: *mut u8, out: *mut Registers, destination: *mut u32, node: *mut u8) {
    unsafe {
        let mut r = out.read();
        let n = TripNode::from_ptr(node);
        let next = n.next();
        r.eax = n.previous() as u32;
        store(r.eax as *mut u8, 0, next as u32);
        store(next, 4, r.eax);
        r.eax = n.second_allocation() as u32;
        if r.eax != 0 {
            call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
        }
        r.eax = n.first_allocation() as u32;
        if r.eax != 0 {
            call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
        }
        call(0x0090_CF63, Conv::Cdecl, &[node as u32], &mut r);
        r.eax = destination as u32;
        destination.write_unaligned(next as u32);
        out.write(r);
    }
}

register_entry!(0x0071_4660, fc4p_00714660, "ClearTrafficTripList", clear_list, 0, []);
unsafe extern "fastcall" fn clear_list(this: *mut u8, out: *mut Registers) {
    unsafe {
        let mut r = out.read();
        r.eax = word(this, 0);
        let mut node = word(r.eax as *mut u8, 0) as *mut u8;
        while node as u32 != r.eax {
            let n = TripNode::from_ptr(node);
            r.eax = n.second_allocation() as u32;
            let next = n.next();
            if r.eax != 0 {
                call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
            }
            r.eax = n.first_allocation() as u32;
            if r.eax != 0 {
                call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
            }
            call(0x0090_CF63, Conv::Cdecl, &[node as u32], &mut r);
            r.eax = word(this, 0);
            node = next;
        }
        r.eax = word(this, 0);
        store(r.eax as *mut u8, 0, r.eax);
        r.eax = word(this, 0);
        store(r.eax as *mut u8, 4, r.eax);
        out.write(r);
    }
}

register_entry!(
    0x0071_46B0,
    fc4p_007146b0,
    "FillTrafficSubnetworkRange",
    fill_range,
    0,
    [16, 12, 8]
);
unsafe extern "fastcall" fn fill_range(
    _this: *mut u8,
    out: *mut Registers,
    begin: *mut u8,
    end: *mut u8,
    source: *mut u8,
) {
    unsafe {
        let mut r = out.read();
        let mut destination = begin;
        while destination != end {
            assign_record(destination, source, &mut r, false);
            destination = destination.wrapping_add(28);
        }
        out.write(r);
    }
}

register_entry!(
    0x0071_46F0,
    fc4p_007146f0,
    "ConstructTrafficSubnetworkRange",
    construct_range,
    0,
    [16, 12, 8]
);
unsafe extern "fastcall" fn construct_range(
    _this: *mut u8,
    out: *mut Registers,
    begin: *mut u8,
    count: u32,
    source: *mut u8,
) {
    unsafe {
        let mut r = out.read();
        let mut destination = begin;
        for _ in 0..count {
            if !destination.is_null() {
                // Construction copies the tail in two-register pairs, unlike assignment.
                r.ecx = destination as u32;
                call(0x0071_2280, Conv::Thiscall, &[source as u32], &mut r);
                r.eax = source.wrapping_add(12) as u32;
                r.edx = word(source, 12);
                r.ecx = destination.wrapping_add(12) as u32;
                store(destination, 12, r.edx);
                r.edx = word(source, 16);
                store(destination, 16, r.edx);
                r.edx = word(source, 20);
                store(destination, 20, r.edx);
                r.eax = word(source, 24);
                store(destination, 24, r.eax);
            }
            destination = destination.wrapping_add(28);
        }
        r.eax = destination as u32;
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00713620::PORT,
    fc4p_00713660::PORT,
    fc4p_00713e90::PORT,
    fc4p_00714000::PORT,
    fc4p_007145e0::PORT,
    fc4p_00714660::PORT,
    fc4p_007146b0::PORT,
    fc4p_007146f0::PORT,
];

#[cfg(test)]
mod tests;
