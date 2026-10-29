// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Network-tool construction, member destruction, parameter copying, and highway insertion.

use super::intersection_storage::call_registers;
use super::*;
use crate::abi::{Conv, RawCall};
use core::mem::MaybeUninit;

game_fn! { fn construct_cache(this: *mut u8) = "thiscall" 0x0064_8000; }
game_fn! { fn resize_set(this: *mut u8, count: u32) = "thiscall" 0x0082_9BC0; }
game_fn! { fn construct_constraints(this: *mut u8) = "thiscall" 0x0066_0810; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free_storage(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn shutdown(this: *mut u8) = "thiscall" 0x0062_6D30; }
game_fn! { fn destroy_parameters(this: *mut u8) = "thiscall" 0x0062_4F70; }
game_fn! { fn erase_tree(this: *mut u8, root: *mut u8) = "thiscall" 0x0046_76A0; }
game_fn! { fn destroy_constraints(this: *mut u8) = "thiscall" 0x0066_02C0; }
game_fn! { fn destroy_nested(first: *mut u8, last: *mut u8, scratch: *mut u8) = "C" 0x0062_C590; }
game_fn! { fn destroy_intersections(this: *mut u8) = "thiscall" 0x0062_BE40; }
game_fn! { fn clear_set(this: *mut u8) = "thiscall" 0x008E_BA60; }
game_fn! { fn destroy_cache(this: *mut u8) -> u64 = "thiscall" 0x0064_7AE0; }
game_fn! { #[allow(dead_code)] fn copy_parameters(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0062_6860; }
game_fn! { #[allow(dead_code)] fn insert_intersection(this: *mut u8, x: u32, z: u32, id: u32, list: *mut u8, real: u32, flags: u32) -> u64 = "thiscall" 0x0062_C3A0; }
game_data! { static type_masks: [u32; 13] = 0x00AA_835C; }

unsafe fn construct_tree(this: *mut u8, offset: usize) {
    unsafe {
        write(this, offset, 0u32);
        let header = allocate(0x14);
        write(this, offset, header);
        write(this, offset + 4, 0u32);
        write(header, 0, 0u8);
        let h: *mut u8 = read(this, offset);
        write(h, 4, 0u32);
        let h: *mut u8 = read(this, offset);
        write(h, 8, h);
        let h: *mut u8 = read(this, offset);
        write(h, 12, h);
    }
}

port! {
    /// Initialize the interface tables, cache, sets, constraints, and two tree headers. Only explicit
    /// defaults are written: padding and fields filled later by initialization retain their bytes.
    #[port(address = 0x0062_C5F0, owner = "networktool", name = "cSC4NetworkTool::cSC4NetworkTool")]
    pub unsafe extern "thiscall" fn fc4p_0062c5f0(this: *mut u8, network_type: u32) -> *mut u8 {
        unsafe {
            core::ptr::write_volatile(this.add(4).cast::<u32>(), 0x00A8_1174);
            write(this, 0, 0x00AA_8748u32);
            write(this, 4, 0x00AA_8738u32);
            for offset in [8, 12, 16, 20] { write(this, offset, 0u32); }
            write(this, 0x18, 0u8);
            construct_cache(this.add(0x1C));
            for offset in [0x44, 0x48, 0x4C] { write(this, offset, 0u32); }
            write(this, 0x50, 0u8); write(this, 0x52, 1u8); write(this, 0x53, 0u8);
            for offset in (0x54..=0x68).step_by(4) { write(this, offset, 0u32); }
            for offset in [0x84, 0xA4, 0xB8] {
                if offset == 0xA4 {
                    for field in [0x98, 0x9C, 0xA0] { write(this, field, 0u32); }
                }
                for field in [4, 8, 12, 16] { write(this, offset + field, 0u32); }
                resize_set(this.add(offset), 100);
            }
            for offset in (0xCC..=0x120).step_by(4) { write(this, offset, 0u32); }
            construct_constraints(this.add(0x124));
            construct_constraints(this.add(0x188));
            let mask = type_masks().cast::<u32>().wrapping_add(network_type as usize).read_unaligned();
            write(this, 0x1EC, !mask & 0x1F4F);
            for offset in [0x1F0, 0x1F4, 0x1F8, 0x1FC] { write(this, offset, 0u32); }
            write(this, 0x200, u32::MAX); write(this, 0x204, 0xBF80_0000u32);
            write(this, 0x208, 1u32); write(this, 0x210, 0u32);
            construct_tree(this, 0x214);
            write(this, 0x220, 0u32); write(this, 0x224, 0u32);
            construct_tree(this, 0x228);
            write(this, 0x240, 0u32); write(this, 0x244, 0u32); write(this, 0x234, 1u8);
            for offset in [0x2B8, 0x2BC, 0x2C0] { write(this, offset, 0u32); }
            for offset in (0x2E4..=0x304).step_by(4) { write(this, offset, 0u32); }
            for offset in (0x330..=0x350).step_by(4) { write(this, offset, 0u32); }
            write(this, 0x35C, network_type);
            write(this, 0x354, 0u8); write(this, 0x358, 0u32);
            this
        }
    }
}

unsafe fn destroy_tree(this: *mut u8, offset: usize) {
    unsafe {
        if read::<u32>(this, offset + 4) != 0 {
            let header: *mut u8 = read(this, offset);
            erase_tree(this.add(offset), read(header, 4));
            let header: *mut u8 = read(this, offset);
            write(header, 8, header);
            let header: *mut u8 = read(this, offset);
            write(header, 4, 0u32);
            let header: *mut u8 = read(this, offset);
            write(header, 12, header);
            write(this, offset + 4, 0u32);
        }
        let header = read::<*mut u8>(this, offset);
        if !header.is_null() {
            free_storage(header);
        }
    }
}
unsafe fn free_member(this: *mut u8, offset: usize) {
    let p: *mut u8 = unsafe { read(this, offset) };
    if !p.is_null() {
        unsafe { free_storage(p) };
    }
}

port! {
    /// Destroy the initialized tool and owned members in reverse order. Empty trees retain their
    /// headers' contents; populated trees reset their links and count before releasing the header.
    #[port(address = 0x0062_CAF0, owner = "networktool", name = "DestroyNetworkTool", naked)]
    pub unsafe extern "C" fn fc4p_0062caf0() {
        core::arch::naked_asm!(
            "push ecx", "call {body}", "pop ecx", "ret", body = sym destroy,
        )
    }
}
unsafe extern "thiscall" fn destroy(this: *mut u8) -> u64 {
    unsafe {
        write(this, 0, 0x00AA_8748u32);
        write(this, 4, 0x00AA_8738u32);
        shutdown(this);
        free_member(this, 0x348);
        free_member(this, 0x33C);
        destroy_parameters(this.add(0x290));
        destroy_tree(this, 0x228);
        destroy_tree(this, 0x214);
        free_member(this, 0x1F4);
        let preview: *mut u8 = read(this, 0x1F0);
        if !preview.is_null() {
            vcall!(preview, 8 => fn());
        }
        destroy_constraints(this.add(0x188));
        destroy_constraints(this.add(0x124));
        for offset in [0x118, 0x10C, 0x100, 0xF0, 0xE4] {
            free_member(this, offset);
        }
        let mut scratch = MaybeUninit::<u8>::uninit();
        destroy_nested(read(this, 0xD8), read(this, 0xDC), scratch.as_mut_ptr());
        free_member(this, 0xD8);
        destroy_intersections(this.add(0xCC));
        for offset in [0xB8, 0xA4] {
            clear_set(this.add(offset));
            free_member(this, offset + 4);
        }
        free_member(this, 0x98);
        clear_set(this.add(0x84));
        free_member(this, 0x88);
        free_member(this, 0x60);
        free_member(this, 0x54);
        let result = destroy_cache(this.add(0x1C));
        let storage: *mut u8 = read(this, 8);
        if storage.is_null() {
            result
        } else {
            free_storage(storage)
        }
    }
}

register_port!(
    0x0062_CAD0,
    fc4p_0062cad0,
    "cSC4NetworkTool::GetPlacementParams",
    placement_parameters,
    "ret 4"
);
unsafe extern "fastcall" fn placement_parameters(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let args = [this.wrapping_add(0x290) as u32];
        let mut c = RawCall::new(Conv::Thiscall, crate::abi::target(0x0062_6860), *frame.add(3), &args);
        c.eax = *frame.add(1);
        c.edx = *frame;
        call_registers(&mut c);
        registers(c.eax, c.edx)
    }
}

register_port!(
    0x0062_C4B0,
    fc4p_0062c4b0,
    "cSC4NetworkTool::InsertHighwayIntersectionPieces",
    insert_highway,
    "ret 4"
);
unsafe extern "fastcall" fn insert_highway(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let mut cursor = tool.support_begin();
        let end = tool.support_end();
        let mut c = RawCall::new(Conv::Thiscall, 0, this as u32, &[]);
        c.eax = *frame.add(1);
        c.edx = *frame;
        while cursor != end {
            c.eax = read(cursor, 0x14);
            if c.eax as i32 >= tool.valid_steps() {
                break;
            }
            c.edx = read(cursor, 0);
            c.eax = read(cursor, 8);
            let real = u32::from(tool.real());
            let x: u32 = read(cursor, 4);
            let args = [x, c.eax, c.edx, *frame.add(3), real, 0];
            c.target = crate::abi::target(0x0062_C3A0);
            c.ecx = this as u32;
            c.args = args.as_ptr();
            c.count = 6;
            c.caller_pops = 0;
            call_registers(&mut c);
            cursor = cursor.wrapping_add(0x28);
        }
        registers(c.eax, c.edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0062c4b0::PORT,
    fc4p_0062c5f0::PORT,
    fc4p_0062cad0::PORT,
    fc4p_0062caf0::PORT,
];

#[cfg(test)]
mod tests;
