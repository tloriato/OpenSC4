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

//! Shared helpers, group 02: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::abi::{Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

/// Integer results include the scratch registers that these shared bodies leave for their callers.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

/// The entry saves the three scratch registers and supplies the original argument words to Rust. The
/// exit restores the computed register results and removes exactly the original number of arguments.
macro_rules! register_port {
    ($address:literal, $export:ident, $body:ident, $pop:literal, $name:literal,
        |$r:ident, $stack:ident| $code:block) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov ecx, esp",
                    "lea edx, [esp + 12]", "push edx", "call {}",
                    "pop eax", "pop ecx", "pop edx", "ret {}",
                    sym $body, const $pop,
                );
            }
        }
        unsafe extern "thiscall" fn $body($r: &mut Registers, $stack: *mut u32) $code
    };
}

/// Read or write a word without imposing alignment on a game object.
unsafe fn load(base: u32, offset: u32) -> u32 {
    unsafe { read_unaligned(base.wrapping_add(offset) as *const u32) }
}

unsafe fn store(base: u32, offset: u32, value: u32) {
    unsafe { write_unaligned(base.wrapping_add(offset) as *mut u32, value) }
}

/// Argument one is immediately after the original return address.
unsafe fn arg(stack: *mut u32, index: usize) -> u32 {
    unsafe { stack.add(index).read() }
}

/// A game call with the original scratch-register inputs and all three integer outputs. The other
/// registers are preserved by the game's ABI. Stack words remain in their original order.
unsafe fn invoke(r: &mut Registers, target: usize, args: &[u32], conv: Conv) {
    let mut c = RawCall::new(conv, target, r.ecx, args);
    c.eax = r.eax;
    c.edx = r.edx;
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f",
            "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
            in("edi") &mut c, clobber_abi("C"),
        );
    }
    *r = Registers {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
    };
}

/// Invoke a live object's virtual slot, retaining the registers left by that method.
unsafe fn virtual_call(r: &mut Registers, offset: usize, args: &[u32]) {
    unsafe {
        invoke(r, crate::abi::vslot(r.ecx as *mut u8, offset), args, Conv::Thiscall);
    }
}

game_fn! {
    #[allow(dead_code)]
    fn allocate(bytes: u32) -> *mut u8 = "C" 0x0090_CF54;
}
game_data! { static default_rectangle_word: u32 = 0x00B2_17FC; }
game_data! { static shared_status_byte: u8 = 0x00B2_1D28; }

game_fn! {
    #[allow(dead_code)]
    fn construct_range(this: *mut u8, begin: u32, end: u32, tag: *mut u32) -> *mut u8 = "thiscall" 0x0040_8420;
}
game_fn! {
    #[allow(dead_code)]
    fn range_error(message: *const u8) = "C" 0x009D_D0A0;
}
game_data! { static string_error_message: u8 = 0x00A8_06F0; }
game_fn! {
    #[allow(dead_code)]
    fn copy_records(begin: u32, end: u32, dest: u32, tag: *mut u32) -> u32 = "C" 0x0063_7E20;
}
game_fn! {
    #[allow(dead_code)]
    fn move_bytes(dest: u32, source: u32, count: u32) -> u32 = "C" 0x009E_EB70;
}
game_fn! {
    #[allow(dead_code)]
    fn allocate_pool(exponent: u32) -> *mut u8 = "C" 0x0090_D8E0;
}
game_fn! {
    #[allow(dead_code)]
    fn remove_empty_pool(this: *mut u8, pool: u32) -> u32 = "thiscall" 0x0090_D031;
}
game_fn! {
    #[allow(dead_code)]
    fn recycle_pool(this: *mut u8, pool: u32) -> u32 = "thiscall" 0x0090_CFF9;
}
game_fn! {
    #[allow(dead_code)]
    fn destroy_list(this: *mut u8) = "thiscall" 0x0056_50A0;
}
game_fn! {
    #[allow(dead_code)]
    fn return_large_pool_cell(this: *mut u8, cell: u32) = "thiscall" 0x004F_82A0;
}
game_data! { static large_pool_owner: [u32; 4] = 0x00B0_98D0; }
game_fn! {
    #[allow(dead_code)]
    fn handle_matching_message(this: *mut u8, id: u32) -> u32 = "thiscall" 0x004E_0BB0;
}

register_port!(
    0x0049_80A0,
    fc4p_004980a0,
    construct_substring,
    12,
    "ConstructSubstring",
    |r, stack| {
        unsafe {
            let source = r.ecx;
            r.eax = load(source, 4);
            r.edx = load(source, 0);
            r.eax = r.eax.wrapping_sub(r.edx);
            let position = arg(stack, 2);
            if position > r.eax {
                invoke(
                    r,
                    crate::abi::target(0x009D_D0A0),
                    &[string_error_message() as u32],
                    Conv::Cdecl,
                );
            }
            r.ecx = load(source, 0);
            r.eax = load(source, 4).wrapping_sub(r.ecx).wrapping_sub(position);
            r.edx = arg(stack, 3);
            stack.add(2).write(r.eax);
            let count = r.eax.min(r.edx);
            let output = arg(stack, 1);
            r.edx = 0;
            for offset in [0, 4, 8] {
                store(output, offset, 0);
            }
            r.ecx = r.ecx.wrapping_add(position);
            r.edx = stack.add(3) as u32;
            r.eax = count.wrapping_add(r.ecx);
            let begin = r.ecx;
            r.ecx = output;
            invoke(
                r,
                crate::abi::target(0x0040_8420),
                &[begin, r.eax, r.edx],
                Conv::Thiscall,
            );
            r.eax = output;
        }
    }
);

register_port!(
    0x004A_DEB0,
    fc4p_004adeb0,
    test_building_attribute,
    4,
    "TestBuildingAttribute",
    |r, stack| {
        unsafe {
            let object = arg(stack, 1);
            r.eax = load(object, 0);
            r.ecx = object;
            virtual_call(r, 0x1C, &[]);
            let mut found = false;
            if r.eax == 0xC772_BF98 {
                r.edx = load(object, 0);
                r.eax = stack.add(1) as u32;
                r.ecx = object;
                virtual_call(r, 0, &[0xA821_EF94, r.eax]);
                if r.eax as u8 != 0 {
                    r.ecx = arg(stack, 1);
                    r.edx = load(r.ecx, 0);
                    virtual_call(r, 0x24, &[0x1FCF]);
                    found = r.eax as u8 != 0;
                    r.ecx = arg(stack, 1);
                    r.eax = load(r.ecx, 0);
                    virtual_call(r, 8, &[]);
                }
            }
            r.eax = (r.eax & !0xFF) | u32::from(found);
        }
    }
);

register_port!(
    0x004B_00B0,
    fc4p_004b00b0,
    copy_record_vector,
    4,
    "CopyRecordVector16",
    |r, stack| {
        unsafe {
            let source = arg(stack, 1);
            let this = r.ecx;
            let count = (load(source, 4).wrapping_sub(load(source, 0)) as i32 >> 4) as u32;
            r.eax = 0;
            for offset in [0, 4, 8] {
                store(this, offset, 0);
            }
            if count != 0 {
                r.eax = count.wrapping_mul(16);
                invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], Conv::Cdecl);
            }
            let capacity = r.eax.wrapping_add(count.wrapping_mul(16));
            r.ecx = stack.add(1) as u32;
            store(this, 0, r.eax);
            store(this, 4, r.eax);
            store(this, 8, capacity);
            r.edx = load(source, 4);
            let dest = r.eax;
            r.eax = load(source, 0);
            invoke(
                r,
                crate::abi::target(0x0063_7E20),
                &[r.eax, r.edx, dest, r.ecx],
                Conv::Cdecl,
            );
            store(this, 4, r.eax);
            r.eax = this;
        }
    }
);

register_port!(
    0x004B_03E0,
    fc4p_004b03e0,
    copy_word_vector,
    4,
    "CopyWordVector",
    |r, stack| {
        unsafe {
            let source = arg(stack, 1);
            let this = r.ecx;
            let count = (load(source, 4).wrapping_sub(load(source, 0)) as i32 >> 2) as u32;
            r.eax = 0;
            for offset in [0, 4, 8] {
                store(this, offset, 0);
            }
            if count != 0 {
                r.eax = count.wrapping_mul(4);
                invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], Conv::Cdecl);
            }
            r.ecx = r.eax.wrapping_add(count.wrapping_mul(4));
            store(this, 0, r.eax);
            store(this, 4, r.eax);
            store(this, 8, r.ecx);
            let end = load(source, 4);
            r.ecx = load(source, 0);
            if end != r.ecx {
                let len = end.wrapping_sub(r.ecx);
                invoke(r, crate::abi::target(0x009E_EB70), &[r.eax, r.ecx, len], Conv::Cdecl);
                r.eax = r.eax.wrapping_add(len);
            }
            store(this, 4, r.eax);
            r.eax = this;
        }
    }
);

register_port!(0x004B_5CA0, fc4p_004b5ca0, string_less, 0, "StringLess", |r, stack| {
    unsafe {
        let first = arg(stack, 1);
        r.ecx = arg(stack, 2);
        r.edx = load(r.ecx, 4);
        let mut left = load(first, 0);
        r.eax = load(first, 4);
        let mut right = load(r.ecx, 0);
        r.eax = r.eax.wrapping_sub(left);
        r.edx = r.edx.wrapping_sub(right);
        stack.add(1).write(r.eax);
        stack.add(2).write(r.edx);
        r.ecx = (r.eax as i32).min(r.edx as i32) as u32;
        let mut order = 0i32;
        while r.ecx != 0 {
            let a = (left as *const u8).read();
            let b = (right as *const u8).read();
            left = left.wrapping_add(1);
            right = right.wrapping_add(1);
            r.ecx = r.ecx.wrapping_sub(1);
            if a != b {
                order = if a < b { -1 } else { 1 };
                break;
            }
        }
        if order == 0 {
            order = (r.eax as i32).cmp(&(r.edx as i32)) as i32;
        }
        r.eax = u32::from(order < 0);
    }
});

register_port!(
    0x004B_DFD0,
    fc4p_004bdfd0,
    rotate_rectangle,
    0,
    "RotateRectangle",
    |r, stack| {
        unsafe {
            r.eax = arg(stack, 2).wrapping_sub(1);
            r.ecx = arg(stack, 3).wrapping_sub(1);
            r.edx = arg(stack, 1);
            match r.edx {
                0 => {
                    r.edx = arg(stack, 4);
                    let out = arg(stack, 5);
                    store(out, 0, r.eax.wrapping_sub(load(r.edx, 8)));
                    r.eax = r.eax.wrapping_sub(load(r.edx, 0));
                    store(out, 8, r.eax);
                    r.eax = r.ecx.wrapping_sub(load(r.edx, 12));
                    store(out, 4, r.eax);
                    r.ecx = r.ecx.wrapping_sub(load(r.edx, 4));
                    store(out, 12, r.ecx);
                }
                1 => {
                    r.ecx = arg(stack, 4);
                    r.edx = arg(stack, 5);
                    store(r.edx, 0, load(r.ecx, 4));
                    store(r.edx, 8, load(r.ecx, 12));
                    store(r.edx, 4, r.eax.wrapping_sub(load(r.ecx, 8)));
                    r.eax = r.eax.wrapping_sub(load(r.ecx, 0));
                    store(r.edx, 12, r.eax);
                }
                2 => {
                    r.ecx = arg(stack, 4);
                    r.edx = arg(stack, 5);
                    for offset in [0, 4, 8] {
                        r.eax = load(r.ecx, offset);
                        store(r.edx, offset, r.eax);
                    }
                    r.ecx = load(r.ecx, 12);
                    store(r.edx, 12, r.ecx);
                }
                3 => {
                    r.eax = arg(stack, 4);
                    r.edx = arg(stack, 5);
                    store(r.edx, 4, load(r.eax, 0));
                    store(r.edx, 12, load(r.eax, 8));
                    store(r.edx, 0, r.ecx.wrapping_sub(load(r.eax, 12)));
                    r.ecx = r.ecx.wrapping_sub(load(r.eax, 4));
                    store(r.edx, 8, r.ecx);
                }
                _ => {}
            }
        }
    }
);

register_port!(
    0x004E_0EB0,
    fc4p_004e0eb0,
    dispatch_matching_message,
    4,
    "DispatchMatchingMessage",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.ecx = arg(stack, 1);
            r.eax = load(r.ecx, 0);
            virtual_call(r, 0x10, &[]);
            if r.eax == 0x2A94_96C9 {
                let id = r.eax;
                r.ecx = this.wrapping_sub(0xD8);
                invoke(r, crate::abi::target(0x004E_0BB0), &[id], Conv::Thiscall);
            }
            r.eax = (r.eax & !0xFF) | 1;
        }
    }
);

register_port!(
    0x004F_5290,
    fc4p_004f5290,
    initialize_pool,
    0,
    "InitializePool16",
    |r, _stack| {
        unsafe {
            invoke(r, crate::abi::target(0x0090_D8E0), &[11], Conv::Cdecl);
            r.edx = 0;
            r.ecx = r.eax.wrapping_add(0x7F0);
            let mut last = 0;
            for _ in 0..127 {
                store(r.ecx, 0, r.edx);
                last = r.ecx;
                r.edx = r.ecx;
                r.ecx = r.ecx.wrapping_sub(16);
            }
            store(r.eax, 8, last);
            store(r.eax, 12, 0);
        }
    }
);

register_port!(
    0x004F_5310,
    fc4p_004f5310,
    copy_records_backward,
    0,
    "CopyRecords16Backward",
    |r, stack| {
        unsafe {
            let first = arg(stack, 1);
            r.ecx = arg(stack, 2);
            r.eax = r.ecx.wrapping_sub(first);
            let count = r.eax as i32 >> 4;
            r.eax = count as u32;
            let groups = if count >= 4 { (count as u32 - 4) / 4 + 1 } else { 0 };
            let remainder = count.wrapping_sub(groups.wrapping_mul(4) as i32);
            r.eax = arg(stack, 3);
            if groups != 0 {
                r.edx = groups;
                while r.edx != 0 {
                    for back in [16u32, 32, 48, 64] {
                        for off in [0u32, 4, 8, 12] {
                            store(r.eax.wrapping_sub(back), off, load(r.ecx.wrapping_sub(back), off));
                        }
                    }
                    r.ecx = r.ecx.wrapping_sub(64);
                    r.eax = r.eax.wrapping_sub(64);
                    r.edx -= 1;
                }
            }
            if remainder > 0 {
                r.edx = remainder as u32;
                while r.edx != 0 {
                    let word = load(r.ecx.wrapping_sub(16), 0);
                    r.ecx = r.ecx.wrapping_sub(16);
                    r.eax = r.eax.wrapping_sub(16);
                    r.edx -= 1;
                    store(r.eax, 0, word);
                    for off in [4, 8, 12] {
                        store(r.eax, off, load(r.ecx, off));
                    }
                }
            }
        }
    }
);

unsafe fn return_pool_cell(r: &mut Registers, stack: *mut u32, mask: u32, threshold: u32) {
    unsafe {
        r.edx = arg(stack, 1);
        if r.edx == 0 {
            return;
        }
        r.eax = r.edx & mask;
        store(r.edx, 0, load(r.eax, 8));
        store(r.eax, 8, r.edx);
        r.edx = load(r.eax, 12).wrapping_sub(1);
        store(r.eax, 12, r.edx);
        let address = if r.edx == 0 {
            0x0090_D031
        } else if r.edx == threshold {
            0x0090_CFF9
        } else {
            return;
        };
        stack.add(1).write(r.eax);
        invoke(r, crate::abi::target(address), &[r.eax], Conv::Thiscall);
    }
}
register_port!(
    0x004F_59C0,
    fc4p_004f59c0,
    return_small_cell,
    4,
    "ReturnPoolCell16",
    |r, stack| {
        unsafe {
            return_pool_cell(r, stack, 0xFFFF_F800, 0x4000_005F);
        }
    }
);
register_port!(
    0x004F_82A0,
    fc4p_004f82a0,
    return_large_cell,
    4,
    "ReturnPoolCell128",
    |r, stack| {
        unsafe {
            return_pool_cell(r, stack, 0xFFFF_E000, 0x4000_003F);
        }
    }
);

register_port!(
    0x004F_88D0,
    fc4p_004f88d0,
    release_pooled_object,
    0,
    "ReleasePooledObject",
    |r, _stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0);
            if r.eax != 1 {
                r.eax = r.eax.wrapping_sub(1);
                store(this, 0, r.eax);
                return;
            }
            r.ecx = this.wrapping_add(0x4C);
            invoke(r, crate::abi::target(0x0056_50A0), &[], Conv::Thiscall);
            for offset in [0x14, 0x10, 0x0C] {
                r.ecx = load(this, offset);
                if r.ecx != 0 {
                    if offset == 0x10 {
                        r.edx = load(r.ecx, 0);
                    } else {
                        r.eax = load(r.ecx, 0);
                    }
                    virtual_call(r, 8, &[]);
                }
            }
            r.ecx = large_pool_owner() as u32;
            invoke(r, crate::abi::target(0x004F_82A0), &[this], Conv::Thiscall);
            r.eax = 0;
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn add_primary_reference(this: *mut u8) -> u32 = "thiscall" 0x004B_DF00;
}
game_fn! {
    #[allow(dead_code)]
    fn release_primary_dispatch(this: *mut u8) -> u32 = "thiscall" 0x004B_9E20;
}
game_fn! {
    #[allow(dead_code)]
    fn query_aircraft(this: *mut u8, iid: u32, out: *mut u32) -> u32 = "thiscall" 0x004F_A7A0;
}
game_fn! {
    #[allow(dead_code)]
    fn query_vehicle(this: *mut u8, iid: u32, out: *mut u32) -> u32 = "thiscall" 0x0055_B660;
}
game_fn! {
    #[allow(dead_code)]
    fn delete_vehicle(this: *mut u8, flags: u32) -> u32 = "thiscall" 0x0053_2270;
}

/// These entries forward the original stack unchanged, including any callee writes to argument words.
macro_rules! adjust_port {
    ($address:literal, $export:ident, $name:literal, $adjust:literal, $target:literal, $test_target:path) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!("sub ecx, {}", "jmp dword ptr [2f]", "2: .long {}",
                    const $adjust, const $target);
                #[cfg(test)]
                core::arch::naked_asm!("sub ecx, {}", "jmp {}", const $adjust, sym $test_target);
            }
        }
    };
}
adjust_port!(
    0x004B_FB10,
    fc4p_004bfb10,
    "AddReferenceFromInterface28",
    0x28,
    0x004B_DF00,
    tests::method_zero
);
adjust_port!(
    0x004B_FB20,
    fc4p_004bfb20,
    "ReleaseFromInterface28",
    0x28,
    0x004B_9E20,
    fc4p_004b9e20
);
adjust_port!(
    0x004F_AEA0,
    fc4p_004faea0,
    "QueryAircraftFromInterface4",
    4,
    0x004F_A7A0,
    tests::method_two
);
adjust_port!(
    0x0050_5080,
    fc4p_00505080,
    "QueryVehicleFromInterface4",
    4,
    0x0055_B660,
    tests::method_two
);
adjust_port!(
    0x0050_50D0,
    fc4p_005050d0,
    "DeleteVehicleFromInterface12",
    12,
    0x0053_2270,
    tests::method_one
);

register_port!(
    0x0049_CAF0,
    fc4p_0049caf0,
    allocate_words,
    4,
    "AllocateWords",
    |r, stack| {
        unsafe {
            r.eax = arg(stack, 1);
            if r.eax != 0 {
                r.eax = r.eax.wrapping_mul(2);
                invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], Conv::Cdecl);
            }
        }
    }
);

register_port!(
    0x0049_D9D0,
    fc4p_0049d9d0,
    copy_field_pair,
    8,
    "CopyFieldPair",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0x24);
            r.edx = arg(stack, 1);
            store(r.edx, 0, r.eax);
            r.eax = load(this, 0x28);
            r.ecx = arg(stack, 2);
            store(r.ecx, 0, r.eax);
        }
    }
);

port! {
    /// Returns the word at +0x2C, retaining the other registers.
    #[port(address = 0x004A_BAD0, owner = "shared", name = "GetCostWord", naked)]
    pub unsafe extern "C" fn fc4p_004abad0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x2c]", "ret");
    }
}

register_port!(
    0x004A_E420,
    fc4p_004ae420,
    repeat_record,
    0,
    "RepeatRecord16",
    |r, stack| {
        unsafe {
            r.ecx = arg(stack, 2);
            r.eax = arg(stack, 1);
            if r.ecx != 0 {
                r.edx = arg(stack, 3);
                loop {
                    if r.eax != 0 {
                        for offset in [0, 4, 8, 12] {
                            store(r.eax, offset, load(r.edx, offset));
                        }
                    }
                    r.eax = r.eax.wrapping_add(16);
                    r.ecx = r.ecx.wrapping_sub(1);
                    if r.ecx == 0 {
                        break;
                    }
                }
            }
        }
    }
);

register_port!(
    0x004B_8860,
    fc4p_004b8860,
    initialize_rectangle,
    0,
    "InitializeRectangleWords",
    |r, _stack| {
        unsafe {
            r.eax = r.ecx;
            r.ecx = default_rectangle_word().read();
            r.edx = r.ecx;
            for offset in [0, 4, 8, 12] {
                store(r.eax, offset, r.ecx);
            }
        }
    }
);

register_port!(
    0x004B_9E20,
    fc4p_004b9e20,
    release_dispatch,
    0,
    "ReleaseDispatch",
    |r, _stack| {
        unsafe {
            r.eax = load(r.ecx, 8);
            if r.eax > 1 {
                r.eax = r.eax.wrapping_sub(1);
                store(r.ecx, 8, r.eax);
            } else {
                r.eax = load(r.ecx, 0);
                virtual_call(r, 0x74, &[]);
                r.eax = 0;
            }
        }
    }
);

register_port!(
    0x004C_3730,
    fc4p_004c3730,
    float_greater,
    4,
    "FloatGreater",
    |r, stack| {
        unsafe {
            r.eax = arg(stack, 1);
            let a = F80::from_f32(read_unaligned(r.ecx as *const f32));
            let b = read_unaligned(r.eax as *const f32);
            r.eax = u32::from(compare_stored(a, b) & 0x4100 == 0);
        }
    }
);

port! {
    /// Returns zero without reading the object.
    #[port(address = 0x004E_13E0, owner = "shared", name = "ReturnZero", naked)]
    pub unsafe extern "C" fn fc4p_004e13e0() {
        core::arch::naked_asm!("xor eax, eax", "ret");
    }
}
port! {
    /// Returns the layer identifier word at +0x1FC.
    #[port(address = 0x004E_76A0, owner = "shared", name = "GetLayerId", naked)]
    pub unsafe extern "C" fn fc4p_004e76a0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x1fc]", "ret");
    }
}
port! {
    /// Returns the draw context word at +0x11C.
    #[port(address = 0x004E_82A0, owner = "shared", name = "GetDrawContext", naked)]
    pub unsafe extern "C" fn fc4p_004e82a0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x11c]", "ret");
    }
}

register_port!(
    0x004E_D910,
    fc4p_004ed910,
    release_owned_object,
    0,
    "ReleaseOwnedObject",
    |r, _stack| {
        unsafe {
            r.eax = load(r.ecx, 8);
            if r.eax == 1 {
                r.eax = load(r.ecx, 0);
                virtual_call(r, 0x1C, &[1]);
                r.eax = 0;
            } else {
                r.eax = r.eax.wrapping_sub(1);
                store(r.ecx, 8, r.eax);
            }
        }
    }
);

register_port!(
    0x004F_0240,
    fc4p_004f0240,
    add_event_reference,
    0,
    "AddEventReference",
    |r, _stack| {
        unsafe {
            r.eax = load(r.ecx, 0x1C).wrapping_add(1);
            store(r.ecx, 0x1C, r.eax);
        }
    }
);
register_port!(
    0x004F_7BA0,
    fc4p_004f7ba0,
    add_render_reference,
    0,
    "AddRenderReference",
    |r, _stack| {
        unsafe {
            r.eax = load(r.ecx, 0x10).wrapping_add(1);
            store(r.ecx, 0x10, r.eax);
        }
    }
);

register_port!(
    0x004F_B340,
    fc4p_004fb340,
    first_vector_word,
    0,
    "FirstVectorWord",
    |r, _stack| {
        unsafe {
            r.eax = load(r.ecx, 0x110);
            if r.eax == load(r.ecx, 0x114) {
                r.eax = 0;
            } else {
                r.ecx = r.eax;
                r.eax = load(r.ecx, 0);
            }
        }
    }
);

port! {
    /// Replaces AL with the byte at +0x4C, preserving the upper three bytes of EAX.
    #[port(address = 0x004F_BBE0, owner = "shared", name = "GetFlagByte", naked)]
    pub unsafe extern "C" fn fc4p_004fbbe0() {
        core::arch::naked_asm!("mov al, [ecx + 0x4c]", "ret");
    }
}

register_port!(
    0x004F_C8E0,
    fc4p_004fc8e0,
    inherited_word,
    0,
    "FindInheritedWord",
    |r, _stack| {
        unsafe {
            loop {
                r.eax = load(r.ecx, 0x60);
                if r.eax != 0 {
                    break;
                }
                r.ecx = load(r.ecx, 0x0C);
                if r.ecx == 0 {
                    break;
                }
            }
        }
    }
);

port! {
    /// Accepts one stack word and replaces only AL with one.
    #[port(address = 0x004F_C900, owner = "shared", name = "AcceptMessage", naked)]
    pub unsafe extern "C" fn fc4p_004fc900() {
        core::arch::naked_asm!("mov al, 1", "ret 4");
    }
}

register_port!(
    0x0050_4C40,
    fc4p_00504c40,
    advance_record_iterator,
    0,
    "AdvanceRecordIterator12",
    |r, _stack| {
        unsafe {
            r.eax = r.ecx;
            r.ecx = load(r.eax, 0).wrapping_add(12);
            r.edx = load(r.eax, 8);
            store(r.eax, 0, r.ecx);
            if r.ecx == r.edx {
                r.ecx = load(r.eax, 12).wrapping_add(4);
                store(r.eax, 12, r.ecx);
                r.ecx = load(r.ecx, 0);
                store(r.eax, 4, r.ecx);
                r.ecx = r.ecx.wrapping_add(0x78);
                store(r.eax, 8, r.ecx);
                r.ecx = load(r.eax, 4);
                store(r.eax, 0, r.ecx);
            }
        }
    }
);

register_port!(
    0x0050_4FA0,
    fc4p_00504fa0,
    query_flag_eight,
    0,
    "QueryFlagEight",
    |r, _stack| {
        unsafe {
            r.eax = load(r.ecx, 0);
            virtual_call(r, 0x7C, &[8]);
        }
    }
);
register_port!(
    0x0050_4FE0,
    fc4p_00504fe0,
    get_shared_status_byte,
    0,
    "GetSharedStatusByte",
    |r, _stack| {
        unsafe {
            r.eax = u32::from(shared_status_byte().read());
        }
    }
);

port! {
    /// Returns the address of the first embedded three-word value.
    #[port(address = 0x0050_4FF0, owner = "shared", name = "GetFirstTripleAddress", naked)]
    pub unsafe extern "C" fn fc4p_00504ff0() {
        core::arch::naked_asm!("lea eax, [ecx + 0x264]", "ret");
    }
}
port! {
    /// Returns the address of the second embedded three-word value.
    #[port(address = 0x0050_5000, owner = "shared", name = "GetSecondTripleAddress", naked)]
    pub unsafe extern "C" fn fc4p_00505000() {
        core::arch::naked_asm!("lea eax, [ecx + 0x270]", "ret");
    }
}

unsafe fn copy_triple(r: &mut Registers, stack: *mut u32, offset: u32) {
    unsafe {
        r.eax = arg(stack, 1);
        r.edx = load(r.eax, 0);
        store(r.ecx, offset, r.edx);
        r.edx = load(r.eax, 4);
        store(r.ecx, offset + 4, r.edx);
        r.eax = load(r.eax, 8);
        store(r.ecx, offset + 8, r.eax);
    }
}
register_port!(
    0x0050_5110,
    fc4p_00505110,
    set_first_triple,
    4,
    "SetFirstTriple",
    |r, stack| {
        unsafe {
            copy_triple(r, stack, 0x264);
        }
    }
);
register_port!(
    0x0050_5140,
    fc4p_00505140,
    set_second_triple,
    4,
    "SetSecondTriple",
    |r, stack| {
        unsafe {
            copy_triple(r, stack, 0x270);
        }
    }
);

register_port!(
    0x0050_5C70,
    fc4p_00505c70,
    initialize_small_object,
    0,
    "InitializeSmallObject",
    |r, _stack| {
        unsafe {
            r.eax = r.ecx;
            r.ecx = 0;
            for offset in [4, 8, 12, 16] {
                store(r.eax, offset, 0);
            }
            (r.eax.wrapping_add(20) as *mut u8).write(0);
            for offset in [24, 28, 32, 36] {
                store(r.eax, offset, 0);
            }
            store(r.eax, 0, 0x00A9_54DC);
        }
    }
);

/// Compare against the stored operand without first quieting its signaling NaN.
fn compare_stored(value: F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand, out("ax") status, out("st(0)") _);
    }
    status
}

fn subtract_stored(value: F80, operand: f32) -> F80 {
    let mut result = [0u8; 10];
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fsub dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value = in(reg) &value, operand = in(reg) &operand, result = in(reg) result.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(result)
}

unsafe fn compare_fields(r: &mut Registers, first: u32, second: u32) -> u16 {
    unsafe {
        let status = compare_stored(
            F80::from_f32(read_unaligned(first as *const f32)),
            read_unaligned(second as *const f32),
        );
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
        status
    }
}

register_port!(
    0x004E_8380,
    fc4p_004e8380,
    intersect_rectangles,
    8,
    "IntersectFloatRectangles",
    |r, stack| {
        unsafe {
            let output = r.ecx;
            r.edx = arg(stack, 1);
            let a = r.edx;
            let b = arg(stack, 2);
            let separated = compare_fields(r, a.wrapping_add(8), b) & 0x4500 == 0x0100
                || compare_fields(r, a, b.wrapping_add(8)) & 0x4100 == 0
                || compare_fields(r, a.wrapping_add(12), b.wrapping_add(4)) & 0x4500 == 0x0100
                || compare_fields(r, a.wrapping_add(4), b.wrapping_add(12)) & 0x4100 == 0;
            if separated {
                r.eax = 0;
                for offset in [12, 4, 8, 0] {
                    store(output, offset, 0);
                }
                return;
            }
            for offset in [0u32, 4, 8, 12] {
                let status = compare_fields(r, a.wrapping_add(offset), b.wrapping_add(offset));
                let pick_a = if offset < 8 {
                    status & 0x4100 == 0
                } else {
                    status & 0x4500 == 0x0100
                };
                let source = if pick_a { a } else { b };
                let value = F80::from_f32(read_unaligned(source.wrapping_add(offset) as *const f32)).to_f32();
                write_unaligned(output.wrapping_add(offset) as *mut f32, value);
            }
            r.eax = (r.eax & !0xFF) | 1;
        }
    }
);

register_port!(
    0x0049_EB90,
    fc4p_0049eb90,
    initialize_deque,
    4,
    "InitializeDeque8",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let blocks = (arg(stack, 1) >> 4) + 1;
            r.eax = (blocks + 2).max(8);
            store(this, 0x24, r.eax);
            r.eax = r.eax.wrapping_mul(4);
            invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], Conv::Cdecl);
            r.ecx = load(this, 0x24).wrapping_sub(blocks) >> 1;
            let first = r.eax.wrapping_add(r.ecx.wrapping_mul(4));
            let limit = first.wrapping_add(blocks.wrapping_mul(4));
            store(this, 0x20, r.eax);
            let mut node = first;
            while node < limit {
                invoke(r, crate::abi::target(0x0090_CF54), &[0x80], Conv::Cdecl);
                store(node, 0, r.eax);
                node = node.wrapping_add(4);
            }
            r.eax = arg(stack, 1);
            store(this, 0x0C, first);
            let first_block = load(first, 0);
            store(this, 4, first_block);
            store(this, 8, first_block.wrapping_add(0x80));
            let last = limit.wrapping_sub(4);
            store(this, 0x1C, last);
            let last_block = load(last, 0);
            store(this, 0x14, last_block);
            store(this, 0x18, last_block.wrapping_add(0x80));
            r.edx = load(this, 4);
            r.ecx = load(this, 0x14);
            store(this, 0, r.edx);
            r.eax &= 15;
            r.edx = r.ecx.wrapping_add(r.eax.wrapping_mul(8));
            store(this, 0x10, r.edx);
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn length_error(message: *const u8) = "C" 0x009D_D1D0;
}
register_port!(
    0x004D_7E50,
    fc4p_004d7e50,
    construct_filled_string,
    12,
    "ConstructFilledString",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let count = arg(stack, 1);
            let capacity = count.wrapping_add(1);
            r.eax = 0;
            for offset in [0, 4, 8] {
                store(this, offset, 0);
            }
            if capacity != 0 {
                invoke(r, crate::abi::target(0x0090_CF54), &[capacity], Conv::Cdecl);
                store(this, 0, r.eax);
                store(this, 4, r.eax);
                r.eax = r.eax.wrapping_add(capacity);
                store(this, 8, r.eax);
            } else {
                invoke(
                    r,
                    crate::abi::target(0x009D_D1D0),
                    &[string_error_message() as u32],
                    Conv::Cdecl,
                );
            }
            let mut end = load(this, 0);
            if count != 0 {
                let value = arg(stack, 2) as u8;
                for i in 0..count {
                    (end.wrapping_add(i) as *mut u8).write(value);
                }
                r.edx = count;
                r.ecx = 0;
                end = end.wrapping_add(arg(stack, 1));
            }
            store(this, 4, end);
            r.eax = this;
            (end as *mut u8).write(0);
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn construct_automaton(this: *mut u8, kind: u32, x: u32, y: u32, z: u32) -> *mut u8 = "thiscall" 0x0051_40A0;
}
game_data! { static initial_automaton_word: u32 = 0x00B0_98DC; }
register_port!(
    0x004F_AC60,
    fc4p_004fac60,
    construct_aircraft,
    0,
    "ConstructAircraft",
    |r, _stack| {
        unsafe {
            let this = r.ecx;
            let mut args = [0u32; 4];
            r.eax = args.as_mut_ptr().add(1) as u32;
            invoke(r, crate::abi::target(0x0051_40A0), &args, Conv::Thiscall);
            for (offset, table) in [
                (0, 0x00A9_4608),
                (4, 0x00A9_4580),
                (8, 0x00A9_4564),
                (12, 0x00A9_4500),
                (0x20, 0x00A9_44EC),
                (0x38, 0x00A9_44DC),
            ] {
                store(this, offset, table);
            }
            r.eax = initial_automaton_word().read();
            store(this, 0x1E4, r.eax);
            let flag = this.wrapping_add(0x33) as *mut u8;
            flag.write(flag.read() | 0x10);
            (this.wrapping_add(0x1E8) as *mut u8).write(0);
            store(this, 0x28, 0x480E_6151);
            r.eax = this;
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn destroy_vehicle_base(this: *mut u8) = "thiscall" 0x0050_4E70;
}
register_port!(
    0x0050_4EF0,
    fc4p_00504ef0,
    destroy_vehicle_references,
    0,
    "DestroyVehicleReferences",
    |r, _stack| {
        unsafe {
            let this = r.ecx;
            r.ecx = load(this, 0x2D4);
            if r.ecx != 0 {
                r.eax = load(r.ecx, 0);
                virtual_call(r, 8, &[]);
            }
            r.eax = load(this, 0x2D0);
            if r.eax != 0 {
                r.ecx = load(r.eax, 4);
                r.edx = load(r.ecx, 4);
                r.ecx = r.edx.wrapping_add(r.eax).wrapping_add(4);
                r.eax = load(r.ecx, 0);
                virtual_call(r, 8, &[]);
            }
            r.ecx = this;
            invoke(r, crate::abi::target(0x0050_4E70), &[], Conv::Thiscall);
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn free_heap(storage: u32) -> u32 = "C" 0x009F_015B;
}
register_port!(
    0x0050_5190,
    fc4p_00505190,
    free_tagged_allocation,
    4,
    "FreeTaggedAllocation",
    |r, stack| {
        unsafe {
            r.eax = arg(stack, 1);
            if r.eax == 0 {
                return;
            }
            r.edx = u32::from((r.eax.wrapping_sub(1) as *const u8).read());
            r.eax = r.eax.wrapping_sub(4);
            if r.edx < 9 {
                r.ecx = load(r.ecx, r.edx * 4 + 0x6C);
                r.edx = load(r.ecx, 0);
                stack.add(1).write(r.eax);
                virtual_call(r, 4, &[r.eax]);
            } else {
                let base = r.eax;
                invoke(r, crate::abi::target(0x009F_015B), &[base], Conv::Cdecl);
                r.ecx = base;
            }
        }
    }
);
register_port!(
    0x0050_51C0,
    fc4p_005051c0,
    free_tagged_with_lock,
    4,
    "FreeTaggedAllocationWithLock",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0);
            virtual_call(r, 4, &[]);
            r.eax = arg(stack, 1);
            if r.eax != 0 {
                r.ecx = u32::from((r.eax.wrapping_sub(1) as *const u8).read());
                r.eax = r.eax.wrapping_sub(4);
                if r.ecx < 9 {
                    r.ecx = load(this, r.ecx * 4 + 0x74);
                    r.edx = load(r.ecx, 0);
                    virtual_call(r, 4, &[r.eax]);
                } else {
                    invoke(r, crate::abi::target(0x009F_015B), &[r.eax], Conv::Cdecl);
                }
            }
            r.eax = load(this, 0);
            r.ecx = this;
            virtual_call(r, 8, &[]);
        }
    }
);

register_port!(
    0x0050_5370,
    fc4p_00505370,
    create_configured_child,
    28,
    "CreateConfiguredChild",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.edx = arg(stack, 1);
            r.eax = load(this, 0);
            virtual_call(
                r,
                0xB8,
                &[arg(stack, 1), arg(stack, 3), arg(stack, 4), 0, arg(stack, 6)],
            );
            let child = r.eax;
            if child != 0 {
                r.eax = load(child, 0);
                r.ecx = child;
                virtual_call(r, 0x9C, &[arg(stack, 5)]);
                r.eax = arg(stack, 2);
                r.edx = load(child, 0);
                r.ecx = child;
                virtual_call(r, 0x20, &[r.eax]);
                r.eax = arg(stack, 7);
                if r.eax != 0 {
                    r.edx = load(child, 0);
                    r.ecx = child;
                    virtual_call(r, 0x38, &[r.eax]);
                }
                r.eax = load(this, 0);
                r.ecx = this;
                virtual_call(r, 0x1C, &[child]);
                r.edx = load(child, 0);
                r.ecx = child;
                virtual_call(r, 8, &[]);
            }
            r.eax = child;
        }
    }
);

register_port!(
    0x0050_5820,
    fc4p_00505820,
    configure_active_flags,
    0,
    "ConfigureActiveFlags",
    |r, _stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0);
            virtual_call(r, 0x78, &[4, 1]);
            r.edx = load(this, 0);
            r.ecx = this;
            virtual_call(r, 0x78, &[0x20, 1]);
            r.eax = load(this, 0);
            r.ecx = this;
            virtual_call(r, 0x198, &[5, 0]);
            r.edx = load(this, 0);
            r.ecx = this;
            virtual_call(r, 0x7C, &[0x0100_0000]);
            if r.eax as u8 == 0 {
                r.eax = load(this, 0);
                r.ecx = this;
                virtual_call(r, 0x34, &[0]);
            }
        }
    }
);

game_data! { static selected_object_service: *mut u8 = 0x00B4_3DC4; }
register_port!(
    0x0050_6940,
    fc4p_00506940,
    selected_object_matches_class,
    0,
    "SelectedObjectMatchesClass",
    |r, _stack| {
        unsafe {
            r.ecx = selected_object_service().read() as u32;
            r.eax = load(r.ecx, 0);
            virtual_call(r, 0x60, &[]);
            let mut found = false;
            if r.eax != 0 {
                r.edx = load(r.eax, 0);
                r.ecx = r.eax;
                virtual_call(r, 0x14, &[]);
                found = r.eax == 0x2B70_9676;
            }
            r.eax = (r.eax & !0xFF) | u32::from(found);
        }
    }
);

register_port!(
    0x0050_9440,
    fc4p_00509440,
    notify_virtual_interfaces,
    4,
    "NotifyVirtualInterfaces",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let mut current = load(this, 0x110);
            let value = arg(stack, 1);
            while current != load(this, 0x114) {
                r.eax = load(current, 0);
                if r.eax != 0 {
                    r.ecx = load(r.eax, 4);
                    r.edx = load(r.ecx, 4);
                    r.ecx = r.edx.wrapping_add(r.eax).wrapping_add(4);
                    r.eax = load(r.ecx, 0);
                    virtual_call(r, 0x28, &[value]);
                }
                r.eax = load(this, 0x114);
                current = current.wrapping_add(4);
            }
            r.eax = load(this, 0x114);
            r.ecx = load(this, 0x110);
            r.eax = (r.eax & !0xFF) | u32::from(r.ecx != r.eax);
        }
    }
);

register_port!(
    0x0050_9600,
    fc4p_00509600,
    get_relative_position,
    4,
    "GetRelativePosition",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0);
            virtual_call(r, 0xEC, &[]);
            if r.eax == 0x34 {
                r.edx = load(this, 0);
                r.ecx = this;
                virtual_call(r, 0xE0, &[]);
                if r.eax != 0 {
                    r.ecx = load(r.eax, 0x28);
                    if r.ecx != 0 {
                        r.eax = load(r.ecx, 0);
                        let dest = arg(stack, 1);
                        virtual_call(r, 0x1C, &[dest]);
                        r.eax = (r.eax & !0xFF) | 1;
                        for offset in [0u32, 4, 8] {
                            let value = F80::from_f32(read_unaligned(dest.wrapping_add(offset) as *const f32));
                            let origin = read_unaligned(this.wrapping_add(0x40 + offset) as *const f32);
                            write_unaligned(
                                dest.wrapping_add(offset) as *mut f32,
                                subtract_stored(value, origin).to_f32(),
                            );
                        }
                        return;
                    }
                }
            }
            r.eax &= !0xFF;
        }
    }
);

register_port!(
    0x0050_9660,
    fc4p_00509660,
    notify_children_pair,
    8,
    "NotifyChildrenPair",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let mut current = load(this, 0x110);
            let args = [arg(stack, 1), arg(stack, 2)];
            while current != load(this, 0x114) {
                r.ecx = load(current, 0);
                if r.ecx != 0 {
                    r.eax = load(r.ecx, 0);
                    virtual_call(r, 0x20, &args);
                }
                r.eax = load(this, 0x114);
                current = current.wrapping_add(4);
            }
        }
    }
);

register_port!(
    0x0050_A670,
    fc4p_0050a670,
    notify_list_children,
    4,
    "NotifyListChildren",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let value = arg(stack, 1);
            r.eax = load(this, 0x44);
            let mut node = load(r.eax, 0);
            if node != r.eax {
                loop {
                    r.ecx = load(node, 8);
                    if r.ecx != 0 {
                        r.eax = load(r.ecx, 0);
                        virtual_call(r, 0x144, &[value]);
                    }
                    node = load(node, 0);
                    if node == load(this, 0x44) {
                        break;
                    }
                }
            }
            r.eax = (r.eax & !0xFF) | 1;
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    fn release_record_range(a0: u32, a1: u32, a2: u32, a3: u32,
        b0: u32, b1: u32, b2: u32, b3: u32, tag: *mut u8) = "C" 0x0050_4CB0;
}
game_fn! {
    #[allow(dead_code)]
    fn delete_deque_blocks(this: *mut u8, begin: u32, end: u32) = "thiscall" 0x006D_A8D0;
}
game_fn! {
    #[allow(dead_code)]
    fn delete_storage(storage: u32) -> u32 = "C" 0x0090_CF63;
}
register_port!(
    0x0050_4D90,
    fc4p_00504d90,
    destroy_record_deque,
    0,
    "DestroyRecordDeque12",
    |r, _stack| {
        unsafe {
            let this = r.ecx;
            let mut tag = core::mem::MaybeUninit::<u8>::uninit();
            let mut args = [
                load(this, 0),
                load(this, 4),
                load(this, 8),
                load(this, 12),
                load(this, 16),
                load(this, 20),
                load(this, 24),
                load(this, 28),
                tag.as_mut_ptr() as u32,
            ];
            r.eax = args.as_mut_ptr() as u32;
            r.ecx = args[3];
            r.edx = args[1];
            invoke(r, crate::abi::target(0x0050_4CB0), &args, Conv::Cdecl);
            r.eax = load(this, 0x20);
            if r.eax != 0 {
                r.edx = load(this, 0x1C).wrapping_add(4);
                r.eax = load(this, 0x0C);
                r.ecx = this;
                invoke(r, crate::abi::target(0x006D_A8D0), &[r.eax, r.edx], Conv::Thiscall);
                let map = load(this, 0x20);
                if map != 0 {
                    invoke(r, crate::abi::target(0x0090_CF63), &[map], Conv::Cdecl);
                }
            }
        }
    }
);

register_port!(
    0x0050_A6B0,
    fc4p_0050a6b0,
    set_filtered_children_enabled,
    4,
    "SetFilteredChildrenEnabled",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.eax = (r.eax & !0xFF) | (arg(stack, 1) & 0xFF);
            (this.wrapping_add(0x4C) as *mut u8).write(r.eax as u8);
            r.ecx = selected_object_service().read() as u32;
            r.eax = stack.add(1) as u32;
            stack.add(1).write(0);
            r.edx = load(r.ecx, 0);
            virtual_call(r, 0x48, &[r.eax]);
            r.eax = load(this, 0x44);
            let mut node = load(r.eax, 0);
            if node != r.eax {
                loop {
                    let child = load(node, 8);
                    if child != 0 {
                        r.eax = arg(stack, 1);
                        let mut accepted = true;
                        if r.eax != 0 {
                            r.edx = load(child, 0);
                            let table = load(r.eax, 0);
                            r.ecx = child;
                            virtual_call(r, 0x14, &[]);
                            let class = r.eax;
                            r.ecx = arg(stack, 1);
                            let method = load(table, 0x0C) as usize;
                            invoke(r, method, &[class], Conv::Thiscall);
                            accepted = r.eax as u8 != 0;
                        }
                        if accepted {
                            r.edx = load(child, 0);
                            r.eax = u32::from((this.wrapping_add(0x4C) as *const u8).read());
                        } else {
                            r.eax = load(child, 0);
                        }
                        r.ecx = child;
                        virtual_call(r, 0x24, &[if accepted { r.eax } else { 0 }]);
                    }
                    node = load(node, 0);
                    if node == load(this, 0x44) {
                        break;
                    }
                }
            }
            r.ecx = arg(stack, 1);
            if r.ecx != 0 {
                r.edx = load(r.ecx, 0);
                virtual_call(r, 8, &[]);
            }
            r.eax = (r.eax & !0xFF) | u32::from((this.wrapping_add(0x4C) as *const u8).read());
        }
    }
);

register_port!(
    0x0050_A740,
    fc4p_0050a740,
    evict_lower_priority_child,
    4,
    "EvictLowerPriorityChild",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let mut removed = false;
            r.eax = load(this, 0x48);
            if r.eax < load(this, 0x40) {
                let priority = arg(stack, 1) as u8;
                if priority != 0 {
                    r.eax = load(this, 0x44);
                    let mut node = load(r.eax, 0);
                    if node != r.eax {
                        loop {
                            let child = load(node, 8);
                            r.edx = load(child, 0);
                            r.ecx = child;
                            virtual_call(r, 0xFC, &[]);
                            if (r.eax as u8) < priority {
                                r.eax = load(child, 0);
                                r.ecx = child;
                                virtual_call(r, 0x34, &[0]);
                                removed = true;
                                break;
                            }
                            node = load(node, 0);
                            if node == load(this, 0x44) {
                                break;
                            }
                        }
                    }
                }
            }
            r.eax = (r.eax & !0xFF) | u32::from(removed);
            // The high byte of the saved ECX word is also the local result byte.
            r.ecx = (this & 0x00FF_FFFF) | (u32::from(removed) << 24);
        }
    }
);

game_data! { static simulation_service: *mut u8 = 0x00B4_3D5C; }
fn add_stored(value: F80, operand: f32) -> F80 {
    let mut result = [0u8; 10];
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fadd dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value = in(reg) &value, operand = in(reg) &operand, result = in(reg) result.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(result)
}
register_port!(
    0x0050_89F0,
    fc4p_005089f0,
    update_children_and_elapsed,
    4,
    "UpdateChildrenAndElapsed",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.ecx = simulation_service().read() as u32;
            r.eax = load(r.ecx, 0);
            virtual_call(r, 0x44, &[]);
            if r.eax as u8 == 0 {
                r.ecx = simulation_service().read() as u32;
                r.edx = load(r.ecx, 0);
                virtual_call(r, 0x3C, &[]);
                if r.eax as u8 != 0 {
                    r.eax = (r.eax & !0xFF) | 1;
                    return;
                }
            }
            r.eax = load(this, 0x114);
            let mut current = load(this, 0x110);
            if current != r.eax {
                let delta = arg(stack, 1);
                loop {
                    r.ecx = load(current, 0);
                    if r.ecx != 0 {
                        r.eax = load(r.ecx, 0);
                        virtual_call(r, 0x3C, &[delta]);
                    }
                    r.eax = load(this, 0x114);
                    current = current.wrapping_add(4);
                    if current == r.eax {
                        break;
                    }
                }
            }
            r.eax = load(this, 0x188);
            if load(r.eax, 0) != r.eax {
                r.ecx = load(this, 0x110);
                if r.ecx != load(this, 0x114) {
                    r.edx = r.ecx;
                    let child = load(r.edx, 0);
                    r.eax = load(child, 0);
                    r.ecx = child;
                    virtual_call(r, 0x30, &[]);
                    r.edx = load(child, 0);
                    r.ecx = child;
                    let threshold = r.eax.wrapping_sub(1) as i32;
                    virtual_call(r, 0x2C, &[]);
                    if r.eax as i32 >= threshold {
                        r.eax = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0x198, &[1, 0]);
                    }
                }
            }
            let delta = F80::from_f32(f32::from_bits(arg(stack, 1)));
            let elapsed = read_unaligned(this.wrapping_add(0x1C0) as *const f32);
            write_unaligned(
                this.wrapping_add(0x1C0) as *mut f32,
                add_stored(delta, elapsed).to_f32(),
            );
            r.eax = (r.eax & !0xFF) | 1;
        }
    }
);

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_004980a0::PORT,
    fc4p_0049caf0::PORT,
    fc4p_0049d9d0::PORT,
    fc4p_0049eb90::PORT,
    fc4p_004abad0::PORT,
    fc4p_004adeb0::PORT,
    fc4p_004ae420::PORT,
    fc4p_004b00b0::PORT,
    fc4p_004b03e0::PORT,
    fc4p_004b5ca0::PORT,
    fc4p_004b8860::PORT,
    fc4p_004b9e20::PORT,
    fc4p_004bdfd0::PORT,
    fc4p_004bfb10::PORT,
    fc4p_004bfb20::PORT,
    fc4p_004c3730::PORT,
    fc4p_004d7e50::PORT,
    fc4p_004e0eb0::PORT,
    fc4p_004e13e0::PORT,
    fc4p_004e76a0::PORT,
    fc4p_004e82a0::PORT,
    fc4p_004e8380::PORT,
    fc4p_004ed910::PORT,
    fc4p_004f0240::PORT,
    fc4p_004f5290::PORT,
    fc4p_004f5310::PORT,
    fc4p_004f59c0::PORT,
    fc4p_004f7ba0::PORT,
    fc4p_004f82a0::PORT,
    fc4p_004f88d0::PORT,
    fc4p_004fac60::PORT,
    fc4p_004faea0::PORT,
    fc4p_004fb340::PORT,
    fc4p_004fbbe0::PORT,
    fc4p_004fc8e0::PORT,
    fc4p_004fc900::PORT,
    fc4p_00504c40::PORT,
    fc4p_00504d90::PORT,
    fc4p_00504ef0::PORT,
    fc4p_00504fa0::PORT,
    fc4p_00504fe0::PORT,
    fc4p_00504ff0::PORT,
    fc4p_00505000::PORT,
    fc4p_00505080::PORT,
    fc4p_005050d0::PORT,
    fc4p_00505110::PORT,
    fc4p_00505140::PORT,
    fc4p_00505190::PORT,
    fc4p_005051c0::PORT,
    fc4p_00505370::PORT,
    fc4p_00505820::PORT,
    fc4p_00505c70::PORT,
    fc4p_00506940::PORT,
    fc4p_005089f0::PORT,
    fc4p_00509440::PORT,
    fc4p_00509600::PORT,
    fc4p_00509660::PORT,
    fc4p_0050a670::PORT,
    fc4p_0050a6b0::PORT,
    fc4p_0050a740::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;
    use std::collections::VecDeque;

    const ENTRY: Registers = Registers {
        eax: 0x1234_ABCD,
        ecx: 0,
        edx: 0x5678_CDEF,
    };
    const RESULT: Registers = Registers {
        eax: 0xABCD_EF80,
        ecx: 0x9876_5432,
        edx: 0x7654_3210,
    };

    #[derive(Debug, PartialEq, Eq)]
    struct Call {
        id: u32,
        registers: Registers,
        args: Vec<u32>,
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static RESULTS: RefCell<VecDeque<Registers>> = const { RefCell::new(VecDeque::new()) };
        static HOOK: RefCell<Option<fn(&Call)>> = const { RefCell::new(None) };
    }

    fn take_calls() -> Vec<Call> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }

    macro_rules! stub {
        ($name:ident, $pop:literal, $count:literal, $id:literal) => {
            #[unsafe(naked)]
            pub(super) unsafe extern "C" fn $name() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov ecx, esp",
                    "lea edx, [esp + 12]", "push edx", "call {}",
                    "pop eax", "pop ecx", "pop edx", "ret {}",
                    sym $name::body, const $pop,
                );
            }
            mod $name {
                use super::*;
                pub unsafe extern "thiscall" fn body(r: &mut Registers, stack: *mut u32) {
                    let args = unsafe { std::slice::from_raw_parts(stack.add(1), $count) }.to_vec();
                    let call = Call { id: $id, registers: *r, args };
                    HOOK.with(|h| { if let Some(hook) = *h.borrow() { hook(&call); } });
                    CALLS.with(|c| c.borrow_mut().push(call));
                    *r = RESULTS.with(|v| v.borrow_mut().pop_front()).unwrap_or(RESULT);
                }
            }
        };
    }
    stub!(allocate_stub, 0, 1, 1);
    stub!(method_zero, 0, 0, 2);
    stub!(method_one, 4, 1, 3);
    stub!(move_stub, 0, 3, 4);
    stub!(range_stub, 12, 3, 5);
    stub!(error_stub, 0, 1, 6);
    stub!(copy_stub, 0, 4, 7);
    stub!(method_two, 8, 2, 8);
    stub!(pool_allocate_stub, 0, 1, 9);
    stub!(pool_remove_stub, 4, 1, 10);
    stub!(pool_recycle_stub, 4, 1, 11);
    stub!(automaton_stub, 16, 4, 12);
    stub!(free_stub, 0, 1, 13);
    stub!(method_five, 20, 5, 14);
    stub!(range_release_stub, 0, 9, 15);
    stub!(lookup_filter_stub, 4, 1, 16);
    stub!(class_stub, 0, 0, 17);
    stub!(filter_stub, 4, 1, 18);
    stub!(enable_stub, 4, 1, 19);
    stub!(filter_release_stub, 0, 0, 20);
    stub!(priority_stub, 0, 0, 21);
    stub!(remove_stub, 4, 1, 22);
    stub!(simulation_first_stub, 0, 0, 23);
    stub!(simulation_second_stub, 0, 0, 24);
    stub!(update_stub, 4, 1, 25);
    stub!(count_stub, 0, 0, 26);
    stub!(index_stub, 0, 0, 27);
    stub!(set_flag_stub, 8, 2, 28);

    fn run(address: u32, this: u32, args: &[u32], cdecl: bool) -> Registers {
        let mut r = Registers { ecx: this, ..ENTRY };
        unsafe {
            invoke(
                &mut r,
                crate::testing::resolve(address),
                args,
                if cdecl { Conv::Cdecl } else { Conv::Thiscall },
            );
        }
        r
    }

    fn bytes(p: u32, size: usize) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(p as *const u8, size) }.to_vec()
    }

    #[test]
    fn word_and_byte_accessors_preserve_other_registers_and_all_memory() {
        let mut memory = Object::new(0x280);
        let p = memory.ptr() as u32;
        unsafe {
            core::ptr::write_bytes(p as *mut u8, 0xA5, 0x280);
        }
        for (off, value) in [(0x2C, 0x1234_5678), (0x1FC, 0x8765_4321), (0x11C, 0xF00D_FACE)] {
            unsafe {
                store(p, off, value);
            }
        }
        let before = bytes(p, 0x280);
        for (address, expected) in [
            (0x004A_BAD0, 0x1234_5678),
            (0x004E_76A0, 0x8765_4321),
            (0x004E_82A0, 0xF00D_FACE),
            (0x004F_BBE0, 0x1234_ABA5),
        ] {
            assert_eq!(
                run(address, p, &[], false),
                Registers {
                    eax: expected,
                    ecx: p,
                    ..ENTRY
                }
            );
        }
        assert_eq!(bytes(p, 0x280), before);
        assert_eq!(run(0x004E_13E0, 0, &[], false), Registers { eax: 0, ..ENTRY });
        assert_eq!(
            run(0x004F_C900, p, &[0xDEAD_BEEF], false),
            Registers {
                eax: 0x1234_AB01,
                ecx: p,
                ..ENTRY
            }
        );
        for (address, offset) in [(0x0050_4FF0, 0x264u32), (0x0050_5000, 0x270)] {
            for base in [p, 0, u32::MAX - 7] {
                assert_eq!(
                    run(address, base, &[], false),
                    Registers {
                        eax: base.wrapping_add(offset),
                        ecx: base,
                        ..ENTRY
                    }
                );
            }
        }
    }

    #[test]
    fn allocation_skips_zero_but_calls_for_nonzero_counts_even_when_size_wraps() {
        let _fake = fake(0x0090_CF54, allocate_stub as *const () as usize);
        take_calls();
        assert_eq!(
            run(0x0049_CAF0, 0x1020, &[0], false),
            Registers {
                eax: 0,
                ecx: 0x1020,
                ..ENTRY
            }
        );
        assert!(take_calls().is_empty());
        for count in [1u32, 0x8000_0000, u32::MAX] {
            assert_eq!(run(0x0049_CAF0, 0x1020, &[count], false), RESULT);
            let want = count.wrapping_mul(2);
            assert_eq!(
                take_calls(),
                [Call {
                    id: 1,
                    registers: Registers {
                        eax: want,
                        ecx: 0x1020,
                        ..ENTRY
                    },
                    args: vec![want]
                }]
            );
        }
    }

    #[test]
    fn field_pair_rereads_second_word_after_first_output_write() {
        let mut memory = Object::new(0x40);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0x24, 0x11);
            store(p, 0x28, 0x22);
        }
        let mut out = [0u32; 2];
        let q = out.as_mut_ptr() as u32;
        assert_eq!(
            run(0x0049_D9D0, p, &[q, q + 4], false),
            Registers {
                eax: 0x22,
                ecx: q + 4,
                edx: q
            }
        );
        assert_eq!(out, [0x11, 0x22]);
        assert_eq!(
            run(0x0049_D9D0, p, &[p + 0x28, q], false),
            Registers {
                eax: 0x11,
                ecx: q,
                edx: p + 0x28
            }
        );
        assert_eq!(out[0], 0x11);
    }

    #[test]
    fn repeat_record_handles_empty_null_and_overlapping_destinations_word_by_word() {
        assert_eq!(
            run(0x004A_E420, 0x5678, &[0x1234, 0, 0], true),
            Registers {
                eax: 0x1234,
                ecx: 0,
                ..ENTRY
            }
        );
        let mut source = [1u32, 2, 3, 4, 5];
        let p = source.as_mut_ptr() as u32;
        assert_eq!(
            run(0x004A_E420, 9, &[0, 1, p], true),
            Registers {
                eax: 16,
                ecx: 0,
                edx: p
            }
        );
        assert_eq!(source, [1, 2, 3, 4, 5]);
        assert_eq!(
            run(0x004A_E420, 9, &[p + 4, 1, p], true),
            Registers {
                eax: p + 20,
                ecx: 0,
                edx: p
            }
        );
        assert_eq!(source, [1; 5]);
        let mut dest = [0xA5A5_A5A5u32; 10];
        let q = unsafe { dest.as_mut_ptr().add(1) } as u32;
        run(0x004A_E420, 0, &[q, 2, p], true);
        assert_eq!(dest, [0xA5A5_A5A5, 1, 1, 1, 1, 1, 1, 1, 1, 0xA5A5_A5A5]);
    }

    #[test]
    fn rectangle_and_status_globals_are_read_each_time_without_extra_writes() {
        let mut value = 0x8000_0000u32;
        let mut status = 0xFFu8;
        let _a = fake_data(0x00B2_17FC, (&mut value as *mut u32).cast());
        let _b = fake_data(0x00B2_1D28, &mut status);
        let mut words = [0xA5u32; 6];
        let p = unsafe { words.as_mut_ptr().add(1) } as u32;
        for v in [0, 0xFF, 0xFFFF_FFFF] {
            value = v;
            assert_eq!(run(0x004B_8860, p, &[], false), Registers { eax: p, ecx: v, edx: v });
            assert_eq!(words, [0xA5, v, v, v, v, 0xA5]);
            assert_eq!(value, v);
        }
        assert_eq!(
            run(0x0050_4FE0, 0x1234, &[], false),
            Registers {
                eax: 0xFF,
                ecx: 0x1234,
                ..ENTRY
            }
        );
        status = 0;
        assert_eq!(run(0x0050_4FE0, 0, &[], false), Registers { eax: 0, ..ENTRY });
        assert_eq!(status, 0);
    }

    #[test]
    fn reference_increments_wrap_and_touch_only_the_selected_word() {
        let mut memory = Object::new(0x30);
        let p = memory.ptr() as u32;
        for (address, offset) in [(0x004F_0240, 0x1C), (0x004F_7BA0, 0x10)] {
            for count in [0u32, 1, 0x7FFF_FFFF, u32::MAX] {
                unsafe {
                    core::ptr::write_bytes(p as *mut u8, 0xA5, 0x30);
                    store(p, offset, count);
                }
                let mut expected = bytes(p, 0x30);
                expected[offset as usize..offset as usize + 4].copy_from_slice(&count.wrapping_add(1).to_le_bytes());
                assert_eq!(
                    run(address, p, &[], false),
                    Registers {
                        eax: count.wrapping_add(1),
                        ecx: p,
                        ..ENTRY
                    }
                );
                assert_eq!(bytes(p, 0x30), expected);
            }
        }
    }

    #[test]
    fn releases_distinguish_unsigned_at_most_one_from_exactly_one() {
        let mut table = [0usize; 30];
        table[0x74 / 4] = method_zero as *const () as usize;
        table[0x1C / 4] = method_one as *const () as usize;
        let mut memory = Object::new(0x20);
        let p = memory.ptr() as u32;
        let vt = table.as_ptr() as u32;
        unsafe {
            store(p, 0, vt);
        }
        for address in [0x004B_9E20, 0x004E_D910] {
            for count in [0u32, 1, 2, 0x8000_0000, u32::MAX] {
                unsafe {
                    store(p, 8, count);
                }
                take_calls();
                let result = run(address, p, &[], false);
                let destroys = count == 1 || (address == 0x004B_9E20 && count == 0);
                if destroys {
                    assert_eq!(result, Registers { eax: 0, ..RESULT });
                    assert_eq!(unsafe { load(p, 8) }, count);
                    assert_eq!(
                        take_calls(),
                        [Call {
                            id: if address == 0x004B_9E20 { 2 } else { 3 },
                            registers: Registers {
                                eax: vt,
                                ecx: p,
                                ..ENTRY
                            },
                            args: if address == 0x004B_9E20 { vec![] } else { vec![1] },
                        }]
                    );
                } else {
                    assert_eq!(
                        result,
                        Registers {
                            eax: count.wrapping_sub(1),
                            ecx: p,
                            ..ENTRY
                        }
                    );
                    assert_eq!(unsafe { load(p, 8) }, count.wrapping_sub(1));
                    assert!(take_calls().is_empty());
                }
            }
        }
    }

    #[test]
    fn greater_comparison_includes_equal_signed_zero_infinities_and_nan() {
        let _fpu = GameFpu::new();
        for (mut a, mut b, expected) in [
            (1.0f32, 0.0f32, 1),
            (-1.0, 0.0, 0),
            (2.0, 2.0, 0),
            (-0.0, 0.0, 0),
            (f32::INFINITY, f32::MAX, 1),
            (f32::NEG_INFINITY, 0.0, 0),
            (f32::from_bits(0x7F80_0001), 0.0, 0),
            (0.0, f32::from_bits(0xFF80_0001), 0),
            (f32::NAN, 0.0, 0),
            (0.0, f32::NAN, 0),
        ] {
            let p = &mut a as *mut f32 as u32;
            let q = &mut b as *mut f32 as u32;
            assert_eq!(
                run(0x004C_3730, p, &[q], false),
                Registers {
                    eax: expected,
                    ecx: p,
                    ..ENTRY
                }
            );
        }
    }

    #[test]
    fn vector_first_and_inherited_value_queries_leave_memory_untouched() {
        let mut objects: Vec<_> = (0..3).map(|_| Object::new(0x120)).collect();
        let pointers: Vec<_> = objects.iter_mut().map(|o| o.ptr() as u32).collect();
        let [a, b, c] = pointers[..] else { unreachable!() };
        unsafe {
            store(a, 0x0C, b);
            store(b, 0x0C, c);
            store(c, 0x60, 0xBEEF);
        }
        let before: Vec<_> = pointers.iter().map(|&p| bytes(p, 0x120)).collect();
        assert_eq!(
            run(0x004F_C8E0, a, &[], false),
            Registers {
                eax: 0xBEEF,
                ecx: c,
                ..ENTRY
            }
        );
        for (&p, want) in pointers.iter().zip(before) {
            assert_eq!(bytes(p, 0x120), want);
        }
        unsafe {
            store(b, 0x60, 42);
        }
        assert_eq!(
            run(0x004F_C8E0, a, &[], false),
            Registers {
                eax: 42,
                ecx: b,
                ..ENTRY
            }
        );
        unsafe {
            store(b, 0x60, 0);
            store(c, 0x60, 0);
        }
        assert_eq!(
            run(0x004F_C8E0, a, &[], false),
            Registers {
                eax: 0,
                ecx: 0,
                ..ENTRY
            }
        );
        let mut values = [0xABCDu32];
        let first = values.as_mut_ptr() as u32;
        unsafe {
            store(a, 0x110, first);
            store(a, 0x114, first);
        }
        assert_eq!(
            run(0x004F_B340, a, &[], false),
            Registers {
                eax: 0,
                ecx: a,
                ..ENTRY
            }
        );
        unsafe {
            store(a, 0x114, first + 4);
        }
        assert_eq!(
            run(0x004F_B340, a, &[], false),
            Registers {
                eax: 0xABCD,
                ecx: first,
                ..ENTRY
            }
        );
    }

    #[test]
    fn iterator_rollover_writes_only_cursor_or_all_four_words_at_block_end() {
        let mut rows = [0u8; 0x100];
        let p = rows.as_mut_ptr() as u32;
        let mut table = [p, p + 0x80];
        let table_p = table.as_mut_ptr() as u32;
        let mut iter = [p + 0x60, p, p + 0x78, table_p];
        let it = iter.as_mut_ptr() as u32;
        assert_eq!(
            run(0x0050_4C40, it, &[], false),
            Registers {
                eax: it,
                ecx: p + 0x6C,
                edx: p + 0x78
            }
        );
        assert_eq!(iter, [p + 0x6C, p, p + 0x78, table_p]);
        assert_eq!(
            run(0x0050_4C40, it, &[], false),
            Registers {
                eax: it,
                ecx: p + 0x80,
                edx: p + 0x78
            }
        );
        assert_eq!(iter, [p + 0x80, p + 0x80, p + 0xF8, table_p + 4]);
    }

    #[test]
    fn query_flag_forwards_eight_and_preserves_the_methods_three_outputs() {
        let mut table = [0usize; 32];
        table[31] = method_one as *const () as usize;
        let mut obj = [table.as_ptr() as usize];
        let p = obj.as_mut_ptr() as u32;
        take_calls();
        assert_eq!(run(0x0050_4FA0, p, &[], false), RESULT);
        assert_eq!(
            take_calls(),
            [Call {
                id: 3,
                registers: Registers {
                    eax: table.as_ptr() as u32,
                    ecx: p,
                    ..ENTRY
                },
                args: vec![8]
            }]
        );
    }

    #[test]
    fn triple_setters_keep_word_order_for_overlapping_inputs_and_return_last_two_words() {
        let mut obj = Object::new(0x280);
        let p = obj.ptr() as u32;
        let mut source = [1u32, 0x8000_0000, 0x7F80_0001];
        let q = source.as_mut_ptr() as u32;
        for (address, offset) in [(0x0050_5110, 0x264u32), (0x0050_5140, 0x270)] {
            unsafe {
                core::ptr::write_bytes(p as *mut u8, 0xA5, 0x280);
            }
            let mut expected = bytes(p, 0x280);
            for (i, v) in source.iter().enumerate() {
                let at = offset as usize + i * 4;
                expected[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
            assert_eq!(
                run(address, p, &[q], false),
                Registers {
                    eax: source[2],
                    ecx: p,
                    edx: source[1]
                }
            );
            assert_eq!(bytes(p, 0x280), expected);
            unsafe {
                store(p, offset - 4, 0xDEAD);
            }
            assert_eq!(
                run(address, p, &[p + offset - 4], false),
                Registers {
                    eax: 0xDEAD,
                    ecx: p,
                    edx: 0xDEAD
                }
            );
            assert_eq!(
                unsafe { (load(p, offset), load(p, offset + 4), load(p, offset + 8)) },
                (0xDEAD, 0xDEAD, 0xDEAD)
            );
        }
    }

    #[test]
    fn small_constructor_retains_padding_and_writes_the_original_vtable() {
        let mut memory = Object::new(0x30);
        let p = memory.ptr() as u32;
        unsafe {
            core::ptr::write_bytes(p as *mut u8, 0xA5, 0x30);
        }
        let mut expected = bytes(p, 0x30);
        expected[..4].copy_from_slice(&0x00A9_54DCu32.to_le_bytes());
        expected[4..21].fill(0);
        expected[24..40].fill(0);
        assert_eq!(
            run(0x0050_5C70, p, &[], false),
            Registers {
                eax: p,
                ecx: 0,
                ..ENTRY
            }
        );
        assert_eq!(bytes(p, 0x30), expected);
    }

    #[test]
    fn substring_clips_unsigned_count_zeros_output_and_passes_original_tag_word() {
        let _range = fake(0x0040_8420, range_stub as *const () as usize);
        let _error = fake(0x009D_D0A0, error_stub as *const () as usize);
        let mut message = 0u8;
        let _message = fake_data(0x00A8_06F0, &mut message);
        let mut chars = *b"abcdef";
        let begin = chars.as_mut_ptr() as u32;
        let mut source = [begin, begin + 6, begin + 6];
        let p = source.as_mut_ptr() as u32;
        let mut output = [0xA5u32; 3];
        let out = output.as_mut_ptr() as u32;
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|call| {
                if call.id == 5 {
                    assert_eq!(
                        unsafe {
                            (
                                load(call.registers.ecx, 0),
                                load(call.registers.ecx, 4),
                                load(call.registers.ecx, 8),
                            )
                        },
                        (0, 0, 0)
                    );
                }
            })
        });
        for (position, requested, length) in [(0, 2, 2), (2, u32::MAX, 4), (6, 9, 0)] {
            let mut stack = [0u32, out, position, requested];
            let mut r = Registers { ecx: p, ..ENTRY };
            take_calls();
            unsafe {
                construct_substring(&mut r, stack.as_mut_ptr());
            }
            assert_eq!(r, Registers { eax: out, ..RESULT });
            assert_eq!(stack[2], 6 - position);
            assert_eq!(stack[3], requested);
            let tag = unsafe { stack.as_mut_ptr().add(3) } as u32;
            assert_eq!(
                take_calls(),
                [Call {
                    id: 5,
                    registers: Registers {
                        eax: begin + position + length,
                        ecx: out,
                        edx: tag
                    },
                    args: vec![begin + position, begin + position + length, tag]
                }]
            );
        }
        take_calls();
        run(0x0049_80A0, p, &[out, 3, 2], false);
        assert_eq!(take_calls()[0].id, 5);
        let mut stack = [0u32, out, 7, 0];
        let mut r = Registers { ecx: p, ..ENTRY };
        take_calls();
        unsafe {
            construct_substring(&mut r, stack.as_mut_ptr());
        }
        let calls = take_calls();
        assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), [6, 5]);
        assert_eq!(calls[0].args, [&mut message as *mut u8 as u32]);
        assert_eq!(stack[2], u32::MAX);
        assert_eq!(calls[1].args[..2], [begin + 7, begin + 7]);
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn attribute_query_gates_interface_lookup_and_releases_it_after_testing() {
        let mut table = [0usize; 10];
        table[0x1C / 4] = method_zero as *const () as usize;
        table[0] = method_two as *const () as usize;
        table[0x24 / 4] = method_one as *const () as usize;
        table[2] = method_zero as *const () as usize;
        let mut object = [table.as_ptr() as u32];
        let p = object.as_mut_ptr() as u32;
        for (class, query, attribute, ids, expected) in [
            (0x1234_5678, 1, 1, vec![2], 0x1234_5600),
            (0xC772_BF98, 0xAABB_CC00, 1, vec![2, 8], 0xAABB_CC00),
            (0xC772_BF98, 1, 0xFFFF_0000, vec![2, 8, 3, 2], RESULT.eax & !0xFF),
            (0xC772_BF98, 1, 0xFFFF_0001, vec![2, 8, 3, 2], (RESULT.eax & !0xFF) | 1),
        ] {
            RESULTS.with(|v| {
                *v.borrow_mut() = [class, query, attribute, RESULT.eax]
                    .into_iter()
                    .map(|eax| Registers { eax, ..RESULT })
                    .collect()
            });
            take_calls();
            assert_eq!(
                run(0x004A_DEB0, 0x1234, &[p], false),
                Registers {
                    eax: expected,
                    ..RESULT
                }
            );
            let calls = take_calls();
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
            assert!(calls.iter().all(|c| c.registers.ecx == p));
            if calls.len() > 1 {
                assert_eq!(calls[1].args[0], 0xA821_EF94);
                assert_eq!(calls[1].registers.eax, calls[1].args[1]);
            }
            if calls.len() > 2 {
                assert_eq!(calls[2].args, [0x1FCF]);
            }
            RESULTS.with(|v| v.borrow_mut().clear());
        }
        // QueryInterface can replace the caller's object argument with a different interface.
        let mut r = ENTRY;
        RESULTS.with(|v| {
            *v.borrow_mut() = [
                Registers {
                    eax: 0xC772_BF98,
                    ..RESULT
                },
                RESULT,
                RESULT,
                RESULT,
            ]
            .into()
        });
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| {
                if c.id == 8 {
                    unsafe {
                        store(c.args[1], 0, c.args[1].wrapping_add(4));
                    }
                }
            })
        });
        // Provide an adjacent word holding the returned interface's table.
        let mut adjacent = [0u32, p, table.as_ptr() as u32];
        take_calls();
        unsafe {
            test_building_attribute(&mut r, adjacent.as_mut_ptr());
        }
        assert_eq!(adjacent[1], unsafe { adjacent.as_ptr().add(2) } as u32);
        let calls = take_calls();
        assert_eq!(calls[2].registers.ecx, adjacent[1]);
        assert_eq!(calls[3].registers.ecx, adjacent[1]);
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn vector_copy_constructs_empty_and_nonempty_storage_through_the_game_calls() {
        let _alloc = fake(0x0090_CF54, allocate_stub as *const () as usize);
        let _copy = fake(0x0063_7E20, copy_stub as *const () as usize);
        let _move = fake(0x009E_EB70, move_stub as *const () as usize);
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                if c.id == 4 {
                    core::ptr::copy(c.args[1] as *const u8, c.args[0] as *mut u8, c.args[2] as usize);
                }
                if c.id == 7 && c.args[0] != c.args[1] {
                    core::ptr::copy(
                        c.args[0] as *const u8,
                        c.args[2] as *mut u8,
                        c.args[1].wrapping_sub(c.args[0]) as usize,
                    );
                }
            })
        });
        let mut input = [0x1234_5678u32; 8];
        let source_begin = input.as_mut_ptr() as u32;
        let mut dest = [0xA5A5_A5A5u32; 10];
        let allocation = unsafe { dest.as_mut_ptr().add(1) } as u32;
        for address in [0x004B_00B0, 0x004B_03E0] {
            for length in [0u32, 32] {
                let mut source = [source_begin, source_begin + length, source_begin + 32];
                let source = source.as_mut_ptr() as u32;
                let mut object = [0xA5u32; 3];
                let p = object.as_mut_ptr() as u32;
                let mut results = VecDeque::new();
                if length != 0 {
                    results.push_back(Registers {
                        eax: allocation,
                        ..RESULT
                    });
                }
                if address == 0x004B_00B0 || length != 0 {
                    results.push_back(Registers {
                        eax: if address == 0x004B_00B0 {
                            if length == 0 {
                                0
                            } else {
                                allocation + length
                            }
                        } else {
                            allocation
                        },
                        ..RESULT
                    });
                }
                RESULTS.with(|v| *v.borrow_mut() = results);
                take_calls();
                let result = run(address, p, &[source], false);
                assert_eq!(result.eax, p);
                let calls = take_calls();
                assert_eq!(
                    calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                    match (length == 0, address == 0x004B_00B0) {
                        (true, true) => vec![7],
                        (true, false) => vec![],
                        (false, true) => vec![1, 7],
                        (false, false) => vec![1, 4],
                    }
                );
                if length == 0 {
                    assert_eq!(object, [0; 3]);
                    if address == 0x004B_03E0 {
                        assert_eq!(
                            result,
                            Registers {
                                eax: p,
                                ecx: source_begin,
                                ..ENTRY
                            }
                        );
                    }
                } else {
                    assert_eq!(object, [allocation, allocation + length, allocation + length]);
                    assert_eq!(calls[0].args, [length]);
                    assert_eq!(result, Registers { eax: p, ..RESULT });
                    assert_eq!(
                        dest,
                        [
                            0xA5A5_A5A5,
                            0x1234_5678,
                            0x1234_5678,
                            0x1234_5678,
                            0x1234_5678,
                            0x1234_5678,
                            0x1234_5678,
                            0x1234_5678,
                            0x1234_5678,
                            0xA5A5_A5A5
                        ]
                    );
                }
            }
        }
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn string_less_uses_unsigned_bytes_signed_lengths_and_preserves_remaining_count() {
        for (a, b, expected, remaining) in [
            (&b""[..], &b""[..], 0, 0),
            (&b"a"[..], &b""[..], 0, 0),
            (&b""[..], &b"a"[..], 1, 0),
            (&b"abc"[..], &b"abcd"[..], 1, 0),
            (&b"abd"[..], &b"abc"[..], 0, 0),
            (&b"a\0b"[..], &b"a\0c"[..], 1, 0),
            (&b"abzz"[..], &b"aczz"[..], 1, 2),
            (&b"\x80"[..], &b"\x7f"[..], 0, 0),
        ] {
            let mut left = [a.as_ptr() as u32, a.as_ptr() as u32 + a.len() as u32];
            let mut right = [b.as_ptr() as u32, b.as_ptr() as u32 + b.len() as u32];
            let p = left.as_mut_ptr() as u32;
            let q = right.as_mut_ptr() as u32;
            assert_eq!(
                run(0x004B_5CA0, 99, &[p, q], true),
                Registers {
                    eax: expected,
                    ecx: remaining,
                    edx: b.len() as u32
                }
            );
            let mut stack = [0, p, q];
            let mut r = ENTRY;
            unsafe {
                string_less(&mut r, stack.as_mut_ptr());
            }
            assert_eq!(&stack[1..], &[a.len() as u32, b.len() as u32]);
        }
    }

    #[test]
    fn rectangle_rotations_preserve_store_order_for_in_place_and_overlapping_outputs() {
        for rotation in [0u32, 1, 2, 3, 4, u32::MAX] {
            let mut original = [2u32, 3, 5, 7];
            let p = original.as_mut_ptr() as u32;
            let mut output = [0xA5u32; 6];
            let q = unsafe { output.as_mut_ptr().add(1) } as u32;
            let result = run(0x004B_DFD0, 0xDEAD, &[rotation, 20, 30, p, q], true);
            let want = match rotation {
                0 => [14, 22, 17, 26],
                1 => [3, 14, 7, 17],
                2 => [2, 3, 5, 7],
                3 => [22, 2, 26, 5],
                _ => [0xA5; 4],
            };
            assert_eq!(&output[1..5], &want);
            assert_eq!((output[0], output[5]), (0xA5, 0xA5));
            if rotation > 3 {
                assert_eq!(
                    result,
                    Registers {
                        eax: 19,
                        ecx: 29,
                        edx: rotation
                    }
                );
            }
            run(0x004B_DFD0, 0, &[rotation, 20, 30, p, p], true);
            assert_eq!(
                original,
                match rotation {
                    0 => [14, 22, 5, 7],
                    1 => [3, 12, 7, 16],
                    2 => [2, 3, 5, 7],
                    3 => [24, 2, 27, 5],
                    _ => [2, 3, 5, 7],
                }
            );
        }
    }

    #[test]
    fn backward_record_copy_covers_unrolled_groups_tails_and_overlap_in_both_directions() {
        for count in 0..10u32 {
            for shift in [-4i32, 0, 4, 16] {
                let mut storage: Vec<u32> = (0..100).collect();
                let p = unsafe { storage.as_mut_ptr().add(8) } as u32;
                let begin_index = 8usize;
                let dest_index = (8i32 + shift / 4) as usize;
                let mut expected = storage.clone();
                for record in (0..count as usize).rev() {
                    for word in 0..4 {
                        expected[dest_index + record * 4 + word] = expected[begin_index + record * 4 + word];
                    }
                }
                let end = p + count * 16;
                let dest_end = end.wrapping_add(shift as u32);
                let result = run(0x004F_5310, 0xABC, &[p, end, dest_end], true);
                assert_eq!(storage, expected, "count={count}, shift={shift}");
                assert_eq!(
                    result,
                    Registers {
                        eax: p.wrapping_add(shift as u32),
                        ecx: end - count * 16,
                        edx: if count == 0 { ENTRY.edx } else { 0 }
                    }
                );
            }
        }
        assert_eq!(
            run(0x004F_5310, 0, &[0x1000, 0xFF0, 0xDEAD], true),
            Registers {
                eax: 0xDEAD,
                ecx: 0xFF0,
                ..ENTRY
            }
        );
    }

    #[test]
    fn pool_initializer_links_exactly_127_cells_and_preserves_header_and_cell_padding() {
        let _alloc = fake(0x0090_D8E0, pool_allocate_stub as *const () as usize);
        let mut pool = Object::new(0x800);
        let p = pool.ptr() as u32;
        unsafe {
            core::ptr::write_bytes(p as *mut u8, 0xA5, 0x800);
        }
        let mut expected = bytes(p, 0x800);
        let mut next = 0u32;
        for at in (0x10usize..0x800).step_by(16).rev() {
            expected[at..at + 4].copy_from_slice(&next.to_le_bytes());
            next = p + at as u32;
        }
        expected[8..12].copy_from_slice(&(p + 16).to_le_bytes());
        expected[12..16].fill(0);
        RESULTS.with(|v| v.borrow_mut().push_back(Registers { eax: p, ..RESULT }));
        take_calls();
        assert_eq!(
            run(0x004F_5290, 0x1234, &[], true),
            Registers {
                eax: p,
                ecx: p,
                edx: p + 16
            }
        );
        assert_eq!(bytes(p, 0x800), expected);
        assert_eq!(
            take_calls(),
            [Call {
                id: 9,
                registers: Registers { ecx: 0x1234, ..ENTRY },
                args: vec![11]
            }]
        );
    }

    #[test]
    fn pool_return_handles_null_count_wrap_empty_and_recycle_boundaries() {
        let _empty = fake(0x0090_D031, pool_remove_stub as *const () as usize);
        let _recycle = fake(0x0090_CFF9, pool_recycle_stub as *const () as usize);
        let mut storage = Object::new(0x6000);
        let raw = storage.ptr() as u32;
        let pool = (raw + 0x1FFF) & !0x1FFF;
        let cell = pool + 0x30;
        for (address, threshold) in [(0x004F_59C0, 0x4000_005Fu32), (0x004F_82A0, 0x4000_003F)] {
            take_calls();
            assert_eq!(
                run(address, 0x9876, &[0], false),
                Registers {
                    eax: ENTRY.eax,
                    ecx: 0x9876,
                    edx: 0
                }
            );
            assert!(take_calls().is_empty());
            for count in [0u32, 1, 2, threshold, threshold + 1, threshold + 2] {
                unsafe {
                    store(pool, 8, 0x1234);
                    store(pool, 12, count);
                    store(cell, 0, 0xA5);
                }
                let mut expected = bytes(pool, 0x80);
                expected[8..12].copy_from_slice(&cell.to_le_bytes());
                expected[12..16].copy_from_slice(&count.wrapping_sub(1).to_le_bytes());
                expected[0x30..0x34].copy_from_slice(&0x1234u32.to_le_bytes());
                take_calls();
                let result = run(address, 0x9876, &[cell], false);
                assert_eq!(bytes(pool, 0x80), expected);
                if count == 1 || count == threshold + 1 {
                    assert_eq!(result, RESULT);
                    assert_eq!(
                        take_calls(),
                        [Call {
                            id: if count == 1 { 10 } else { 11 },
                            registers: Registers {
                                eax: pool,
                                ecx: 0x9876,
                                edx: count - 1
                            },
                            args: vec![pool]
                        }]
                    );
                } else {
                    assert_eq!(
                        result,
                        Registers {
                            eax: pool,
                            ecx: 0x9876,
                            edx: count.wrapping_sub(1)
                        }
                    );
                    assert!(take_calls().is_empty());
                }
            }
        }
    }

    #[test]
    fn pooled_release_destroys_list_releases_three_fields_then_returns_cell() {
        let _list = fake(0x0056_50A0, method_zero as *const () as usize);
        let _pool = fake(0x004F_82A0, method_one as *const () as usize);
        let mut pool_owner = [0u32; 4];
        let _owner = fake_data(0x00B0_98D0, pool_owner.as_mut_ptr().cast());
        let mut table = [0usize; 3];
        table[2] = method_zero as *const () as usize;
        let mut referenced = [table.as_ptr() as u32];
        let reference = referenced.as_mut_ptr() as u32;
        let mut object = Object::new(0x60);
        let p = object.ptr() as u32;
        for count in [0u32, 1, 2, u32::MAX] {
            for present in [false, true] {
                unsafe {
                    store(p, 0, count);
                    for offset in [0x0C, 0x10, 0x14] {
                        store(p, offset, if present { reference } else { 0 });
                    }
                }
                take_calls();
                let r = run(0x004F_88D0, p, &[], false);
                if count == 1 {
                    assert_eq!(r, Registers { eax: 0, ..RESULT });
                    let calls = take_calls();
                    assert_eq!(calls.len(), if present { 5 } else { 2 });
                    assert_eq!(calls[0].registers.ecx, p + 0x4C);
                    assert_eq!(calls[0].registers.eax, 1);
                    for c in &calls[1..calls.len() - 1] {
                        assert_eq!(c.registers.ecx, reference);
                        assert!(c.args.is_empty());
                    }
                    assert_eq!(calls.last().unwrap().registers.ecx, pool_owner.as_ptr() as u32);
                    assert_eq!(calls.last().unwrap().args, [p]);
                    assert_eq!(unsafe { load(p, 0) }, 1);
                } else {
                    assert_eq!(
                        r,
                        Registers {
                            eax: count.wrapping_sub(1),
                            ecx: p,
                            ..ENTRY
                        }
                    );
                    assert_eq!(unsafe { load(p, 0) }, count.wrapping_sub(1));
                    assert!(take_calls().is_empty());
                }
            }
        }
    }

    #[test]
    fn matching_message_adjusts_owner_and_only_forwards_the_selected_identifier() {
        let _handler = fake(0x004E_0BB0, method_one as *const () as usize);
        let mut table = [0usize; 5];
        table[4] = method_zero as *const () as usize;
        let mut message = [table.as_ptr() as u32];
        let p = message.as_mut_ptr() as u32;
        for id in [0x2A94_96C9, 0, 0x1234_5600] {
            RESULTS.with(|v| v.borrow_mut().push_back(Registers { eax: id, ..RESULT }));
            take_calls();
            let r = run(0x004E_0EB0, 0x2000, &[p], false);
            let calls = take_calls();
            assert_eq!(
                calls[0].registers,
                Registers {
                    eax: table.as_ptr() as u32,
                    ecx: p,
                    ..ENTRY
                }
            );
            if id == 0x2A94_96C9 {
                assert_eq!(
                    r,
                    Registers {
                        eax: (RESULT.eax & !0xFF) | 1,
                        ..RESULT
                    }
                );
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[1].args, [id]);
                assert_eq!(calls[1].registers.ecx, 0x2000 - 0xD8);
            } else {
                assert_eq!(
                    r,
                    Registers {
                        eax: (id & !0xFF) | 1,
                        ..RESULT
                    }
                );
                assert_eq!(calls.len(), 1);
            }
        }
    }

    #[test]
    fn interface_thunks_forward_adjusted_this_registers_and_exact_stack_words() {
        for (address, adjust, args, id) in [
            (0x004B_FB10, 0x28, vec![], 2),
            (0x004F_AEA0, 4, vec![1, 2], 8),
            (0x0050_5080, 4, vec![3, 4], 8),
            (0x0050_50D0, 12, vec![u32::MAX], 3),
        ] {
            take_calls();
            assert_eq!(run(address, 0x1000, &args, false), RESULT);
            assert_eq!(
                take_calls(),
                [Call {
                    id,
                    registers: Registers {
                        ecx: 0x1000 - adjust,
                        ..ENTRY
                    },
                    args
                }]
            );
        }
        let mut memory = Object::new(0x40);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 8, 3);
        }
        assert_eq!(
            run(0x004B_FB20, p + 0x28, &[], false),
            Registers {
                eax: 2,
                ecx: p,
                ..ENTRY
            }
        );
        assert_eq!(unsafe { load(p, 8) }, 2);
    }

    #[test]
    fn deque_constructor_centers_map_allocates_blocks_in_order_and_sets_both_iterators() {
        let _alloc = fake(0x0090_CF54, allocate_stub as *const () as usize);
        for size in [0u32, 15, 16, 95, 96, 128] {
            let block_count = (size >> 4) + 1;
            let capacity = (block_count + 2).max(8);
            let mut memory = Object::new(0x30);
            let p = memory.ptr() as u32;
            unsafe {
                core::ptr::write_bytes(p as *mut u8, 0xA5, 0x30);
            }
            let mut map = vec![0xA5A5_A5A5u32; capacity as usize];
            let map_p = map.as_mut_ptr() as u32;
            let mut blocks: Vec<_> = (0..block_count).map(|_| Object::new(0x80)).collect();
            let block_p: Vec<_> = blocks.iter_mut().map(|b| b.ptr() as u32).collect();
            RESULTS.with(|v| {
                *v.borrow_mut() = std::iter::once(map_p)
                    .chain(block_p.iter().copied())
                    .map(|eax| Registers { eax, ..RESULT })
                    .collect()
            });
            let index = (capacity - block_count) / 2;
            let first_node = map_p + index * 4;
            let last_node = first_node + (block_count - 1) * 4;
            let first = block_p[0];
            let last = *block_p.last().unwrap();
            let mut expected = bytes(p, 0x30);
            for (off, value) in [
                (0, first),
                (4, first),
                (8, first + 0x80),
                (0x0C, first_node),
                (0x10, last + (size & 15) * 8),
                (0x14, last),
                (0x18, last + 0x80),
                (0x1C, last_node),
                (0x20, map_p),
                (0x24, capacity),
            ] {
                expected[off..off + 4].copy_from_slice(&value.to_le_bytes());
            }
            take_calls();
            assert_eq!(
                run(0x0049_EB90, p, &[size], false),
                Registers {
                    eax: size & 15,
                    ecx: last,
                    edx: last + (size & 15) * 8
                }
            );
            assert_eq!(bytes(p, 0x30), expected);
            let mut expected_map = vec![0xA5A5_A5A5u32; capacity as usize];
            expected_map[index as usize..(index + block_count) as usize].copy_from_slice(&block_p);
            assert_eq!(map, expected_map);
            let calls = take_calls();
            assert_eq!(calls.len(), block_count as usize + 1);
            assert_eq!(calls[0].args, [capacity * 4]);
            assert_eq!(
                calls[0].registers,
                Registers {
                    eax: capacity * 4,
                    ecx: p,
                    ..ENTRY
                }
            );
            assert!(calls[1..].iter().all(|c| c.id == 1 && c.args == [0x80]));
        }
    }

    #[test]
    fn filled_string_allocates_extra_terminator_byte_and_ignores_high_character_bits_and_tag() {
        let _alloc = fake(0x0090_CF54, allocate_stub as *const () as usize);
        for count in [0u32, 1, 3, 4, 7, 32] {
            let mut text = [0xA5u8; 36];
            let buffer = unsafe { text.as_mut_ptr().add(1) } as u32;
            let mut object = [0xA5u32; 3];
            let p = object.as_mut_ptr() as u32;
            RESULTS.with(|v| v.borrow_mut().push_back(Registers { eax: buffer, ..RESULT }));
            take_calls();
            let r = run(0x004D_7E50, p, &[count, 0xDEAD_BE80, 0x9876], false);
            assert_eq!(
                r,
                if count == 0 {
                    Registers { eax: p, ..RESULT }
                } else {
                    Registers {
                        eax: p,
                        ecx: 0,
                        edx: count,
                    }
                }
            );
            assert_eq!(object, [buffer, buffer + count, buffer + count + 1]);
            let mut expected = [0xA5u8; 36];
            expected[1..count as usize + 1].fill(0x80);
            expected[count as usize + 1] = 0;
            assert_eq!(text, expected);
            assert_eq!(
                take_calls(),
                [Call {
                    id: 1,
                    registers: Registers {
                        eax: 0,
                        ecx: p,
                        ..ENTRY
                    },
                    args: vec![count + 1]
                }]
            );
        }
    }

    #[test]
    fn aircraft_constructor_calls_base_with_zero_position_then_sets_tables_flags_and_id() {
        let _base = fake(0x0051_40A0, automaton_stub as *const () as usize);
        let mut initial = 0x8765_4321u32;
        let _initial = fake_data(0x00B0_98DC, (&mut initial as *mut u32).cast());
        let mut memory = Object::new(0x200);
        let p = memory.ptr() as u32;
        unsafe {
            core::ptr::write_bytes(p as *mut u8, 0xA5, 0x200);
        }
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| {
                assert_eq!(c.id, 12);
                assert_eq!(c.args, [0; 4]);
                assert_eq!(
                    unsafe {
                        (
                            load(c.registers.eax, 0),
                            load(c.registers.eax, 4),
                            load(c.registers.eax, 8),
                        )
                    },
                    (0, 0, 0)
                );
                unsafe {
                    store(c.registers.ecx, 0x30, 0x0102_0304);
                }
            })
        });
        let mut expected = bytes(p, 0x200);
        expected[0x30..0x34].copy_from_slice(&0x1102_0304u32.to_le_bytes());
        for (off, value) in [
            (0, 0x00A9_4608u32),
            (4, 0x00A9_4580),
            (8, 0x00A9_4564),
            (12, 0x00A9_4500),
            (0x20, 0x00A9_44EC),
            (0x38, 0x00A9_44DC),
            (0x1E4, initial),
            (0x28, 0x480E_6151),
        ] {
            expected[off..off + 4].copy_from_slice(&value.to_le_bytes());
        }
        expected[0x1E8] = 0;
        take_calls();
        assert_eq!(run(0x004F_AC60, p, &[], false), Registers { eax: p, ..RESULT });
        assert_eq!(bytes(p, 0x200), expected);
        let calls = take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!((calls[0].registers.ecx, calls[0].registers.edx), (p, ENTRY.edx));
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn vehicle_destructor_releases_direct_then_virtual_base_interface_before_base_destructor() {
        let _base = fake(0x0050_4E70, method_zero as *const () as usize);
        let mut table = [0usize; 3];
        table[2] = method_zero as *const () as usize;
        let mut direct = [table.as_ptr() as u32];
        let direct_p = direct.as_mut_ptr() as u32;
        let mut virtual_offsets = [0u32, 12];
        let mut referenced = [0u32; 8];
        referenced[1] = virtual_offsets.as_mut_ptr() as u32;
        referenced[4] = table.as_ptr() as u32;
        let reference_p = referenced.as_mut_ptr() as u32;
        let mut memory = Object::new(0x2E0);
        let p = memory.ptr() as u32;
        for (has_direct, has_virtual) in [(false, false), (true, false), (false, true), (true, true)] {
            unsafe {
                store(p, 0x2D4, if has_direct { direct_p } else { 0 });
                store(p, 0x2D0, if has_virtual { reference_p } else { 0 });
            }
            let before = bytes(p, 0x2E0);
            take_calls();
            assert_eq!(run(0x0050_4EF0, p, &[], false), RESULT);
            let calls = take_calls();
            let mut receivers = vec![];
            if has_direct {
                receivers.push(direct_p);
            }
            if has_virtual {
                receivers.push(reference_p + 16);
            }
            receivers.push(p);
            assert_eq!(calls.iter().map(|c| c.registers.ecx).collect::<Vec<_>>(), receivers);
            assert!(calls.iter().all(|c| c.id == 2 && c.args.is_empty()));
            assert_eq!(bytes(p, 0x2E0), before);
        }
    }

    #[test]
    fn tagged_free_selects_all_nine_pools_and_heap_with_lock_even_for_null() {
        let _heap = fake(0x009F_015B, free_stub as *const () as usize);
        let mut lock_table = [0usize; 3];
        lock_table[1] = method_zero as *const () as usize;
        lock_table[2] = method_zero as *const () as usize;
        let mut pool_table = [0usize; 2];
        pool_table[1] = method_one as *const () as usize;
        let mut pool = [pool_table.as_ptr() as u32];
        let pool_p = pool.as_mut_ptr() as u32;
        let mut memory = Object::new(0xA0);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0, lock_table.as_ptr() as u32);
        }
        let mut block = [0u8; 16];
        let base = block.as_mut_ptr() as u32;
        for (address, offset, locked) in [(0x0050_5190, 0x6C, false), (0x0050_51C0, 0x74, true)] {
            for tag in 0..9u32 {
                unsafe {
                    store(p, offset + tag * 4, pool_p);
                }
            }
            for tag in [
                None,
                Some(0u8),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(8),
                Some(9),
                Some(0x80),
                Some(0xFF),
            ] {
                if let Some(tag) = tag {
                    block[3] = tag;
                }
                let input = if tag.is_some() { base + 4 } else { 0 };
                take_calls();
                let result = run(address, p, &[input], false);
                let calls = take_calls();
                let expected_ids: Vec<_> = if locked {
                    std::iter::once(2)
                        .chain(tag.map(|t| if t < 9 { 3 } else { 13 }))
                        .chain(std::iter::once(2))
                        .collect()
                } else {
                    tag.map(|t| if t < 9 { 3 } else { 13 }).into_iter().collect()
                };
                assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), expected_ids);
                if let Some(tag) = tag {
                    let free_call = &calls[usize::from(locked)];
                    assert_eq!(free_call.args, [base]);
                    assert_eq!(free_call.registers.eax, base);
                    assert_eq!(
                        free_call.registers.ecx,
                        if tag < 9 {
                            pool_p
                        } else if locked {
                            u32::from(tag)
                        } else {
                            p
                        }
                    );
                }
                assert_eq!(
                    result,
                    if locked || tag.is_some_and(|t| t < 9) {
                        RESULT
                    } else if tag.is_some() {
                        Registers { ecx: base, ..RESULT }
                    } else {
                        Registers {
                            eax: 0,
                            ecx: p,
                            ..ENTRY
                        }
                    }
                );
            }
        }
    }

    #[test]
    fn configured_child_creation_applies_callback_position_orientation_then_balances_ownership() {
        // The core's creation case, exercised at the manager boundary with the optional orientation.
        let mut manager_table = [0usize; 0xBC / 4];
        manager_table[0xB8 / 4] = method_five as *const () as usize;
        manager_table[0x1C / 4] = method_one as *const () as usize;
        let mut manager = [manager_table.as_ptr() as u32];
        let manager_p = manager.as_mut_ptr() as u32;
        let mut child_table = [0usize; 0xA0 / 4];
        for slot in [0x9C, 0x20, 0x38] {
            child_table[slot / 4] = method_one as *const () as usize;
        }
        child_table[2] = method_zero as *const () as usize;
        let mut child = [child_table.as_ptr() as u32];
        let child_p = child.as_mut_ptr() as u32;
        for (created, orientation) in [(false, 0), (true, 0), (true, 0x8000_1234)] {
            RESULTS.with(|v| {
                v.borrow_mut().push_back(Registers {
                    eax: if created { child_p } else { 0 },
                    ..RESULT
                })
            });
            take_calls();
            assert_eq!(
                run(
                    0x0050_5370,
                    manager_p,
                    &[0x4107, 22, 2, 44, 55, 0xFF, orientation],
                    false
                ),
                Registers {
                    eax: if created { child_p } else { 0 },
                    ..RESULT
                }
            );
            let calls = take_calls();
            assert_eq!(
                calls[0],
                Call {
                    id: 14,
                    registers: Registers {
                        eax: manager_table.as_ptr() as u32,
                        ecx: manager_p,
                        edx: 0x4107
                    },
                    args: vec![0x4107, 2, 44, 0, 0xFF]
                }
            );
            let expected_args = if !created {
                vec![]
            } else if orientation == 0 {
                vec![vec![55], vec![22], vec![child_p], vec![]]
            } else {
                vec![vec![55], vec![22], vec![orientation], vec![child_p], vec![]]
            };
            assert_eq!(
                calls[1..].iter().map(|c| c.args.clone()).collect::<Vec<_>>(),
                expected_args
            );
            if created {
                assert_eq!(calls.last().unwrap().registers.ecx, child_p);
                assert_eq!(calls[calls.len() - 2].registers.ecx, manager_p);
            }
        }
    }

    #[test]
    fn active_configuration_tests_low_byte_and_calls_remove_only_when_flag_is_absent() {
        let mut table = [0usize; 0x19C / 4];
        table[0x78 / 4] = method_two as *const () as usize;
        table[0x198 / 4] = method_two as *const () as usize;
        table[0x7C / 4] = method_one as *const () as usize;
        table[0x34 / 4] = method_one as *const () as usize;
        let mut object = [table.as_ptr() as u32];
        let p = object.as_mut_ptr() as u32;
        for flag in [0xFFFF_0000u32, 0xFFFF_0001] {
            RESULTS.with(|v| *v.borrow_mut() = [RESULT, RESULT, RESULT, Registers { eax: flag, ..RESULT }].into());
            take_calls();
            assert_eq!(
                run(0x0050_5820, p, &[], false),
                if flag as u8 == 0 {
                    RESULT
                } else {
                    Registers { eax: flag, ..RESULT }
                }
            );
            let calls = take_calls();
            let mut expected_args = vec![vec![4, 1], vec![0x20, 1], vec![5, 0], vec![0x0100_0000]];
            if flag as u8 == 0 {
                expected_args.push(vec![0]);
            }
            assert_eq!(calls.iter().map(|c| c.args.clone()).collect::<Vec<_>>(), expected_args);
            assert!(calls.iter().all(|c| c.registers.ecx == p));
        }
    }

    #[test]
    fn driving_mode_query_keeps_identifier_upper_bytes_and_handles_no_selected_object() {
        // Core automata's driving-mode predicate, including a missing selected object.
        let mut service_table = [0usize; 0x64 / 4];
        service_table[0x60 / 4] = method_zero as *const () as usize;
        let mut service = [service_table.as_ptr() as u32];
        let mut global = service.as_mut_ptr().cast::<u8>();
        let _global = fake_data(0x00B4_3DC4, (&mut global as *mut *mut u8).cast());
        let mut table = [0usize; 6];
        table[5] = method_zero as *const () as usize;
        let mut object = [table.as_ptr() as u32];
        let p = object.as_mut_ptr() as u32;
        for class in [None, Some(0x2B70_9676u32), Some(0x1234_56FF)] {
            let mut outputs = VecDeque::from([Registers {
                eax: if class.is_some() { p } else { 0 },
                ..RESULT
            }]);
            if let Some(class) = class {
                outputs.push_back(Registers { eax: class, ..RESULT });
            }
            RESULTS.with(|v| *v.borrow_mut() = outputs);
            take_calls();
            let expected = class.map(|c| (c & !0xFF) | u32::from(c == 0x2B70_9676)).unwrap_or(0);
            assert_eq!(
                run(0x0050_6940, 0, &[], true),
                Registers {
                    eax: expected,
                    ..RESULT
                }
            );
            let calls = take_calls();
            assert_eq!(calls.len(), if class.is_some() { 2 } else { 1 });
            assert_eq!(calls[0].registers.ecx, service.as_ptr() as u32);
            if class.is_some() {
                assert_eq!(calls[1].registers.ecx, p);
            }
        }
    }

    #[test]
    fn vector_notifications_skip_nulls_adjust_virtual_bases_and_observe_end_mutation() {
        let mut table = [0usize; 11];
        table[0x28 / 4] = method_one as *const () as usize;
        table[0x20 / 4] = method_two as *const () as usize;
        let mut offsets = [0u32, 12];
        let mut child = [0u32; 8];
        child[0] = table.as_ptr() as u32;
        child[1] = offsets.as_mut_ptr() as u32;
        child[4] = table.as_ptr() as u32;
        let c = child.as_mut_ptr() as u32;
        let mut children = [c, 0, c];
        let begin = children.as_mut_ptr() as u32;
        let end = begin + 12;
        let mut memory = Object::new(0x120);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0x110, begin);
            store(p, 0x114, begin);
        }
        take_calls();
        assert_eq!(
            run(0x0050_9440, p, &[99], false),
            Registers {
                eax: begin & !0xFF,
                ecx: begin,
                ..ENTRY
            }
        );
        assert_eq!(run(0x0050_9660, p, &[99, 100], false), Registers { ecx: p, ..ENTRY });
        assert!(take_calls().is_empty());
        for (address, args, receiver) in [(0x0050_9440, vec![99], c + 16), (0x0050_9660, vec![99, 100], c)] {
            unsafe {
                store(p, 0x114, end);
            }
            take_calls();
            let r = run(address, p, &args, false);
            let calls = take_calls();
            assert_eq!(calls.len(), 2);
            assert!(calls
                .iter()
                .all(|call| call.registers.ecx == receiver && call.args == args));
            assert_eq!(
                r,
                if address == 0x0050_9440 {
                    Registers {
                        eax: (end & !0xFF) | 1,
                        ecx: begin,
                        ..RESULT
                    }
                } else {
                    Registers { eax: end, ..RESULT }
                }
            );
        }
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|call| {
                let owner = call.args[0];
                unsafe {
                    store(owner, 0x114, load(owner, 0x110).wrapping_add(4));
                }
            })
        });
        unsafe {
            store(p, 0x114, end);
        }
        take_calls();
        run(0x0050_9660, p, &[p, 100], false);
        assert_eq!(take_calls().len(), 1);
        assert_eq!(unsafe { load(p, 0x114) }, begin + 4);
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn list_notifications_skip_null_values_follow_next_after_call_and_always_set_only_al() {
        let mut table = [0usize; 0x148 / 4];
        table[0x144 / 4] = method_one as *const () as usize;
        let mut child = [table.as_ptr() as u32];
        let child_p = child.as_mut_ptr() as u32;
        let mut head = [0u32; 3];
        let sentinel = head.as_mut_ptr() as u32;
        let mut nodes = [[0u32; 3]; 3];
        let n = nodes.as_mut_ptr() as u32;
        nodes[0] = [n + 12, 0, child_p];
        nodes[1] = [n + 24, 0, 0];
        nodes[2] = [sentinel, 0, child_p];
        let mut memory = Object::new(0x50);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0x44, sentinel);
        }
        head[0] = sentinel;
        assert_eq!(
            run(0x0050_A670, p, &[99], false),
            Registers {
                eax: (sentinel & !0xFF) | 1,
                ecx: p,
                ..ENTRY
            }
        );
        head[0] = n;
        assert_eq!(head[0], n);
        take_calls();
        assert_eq!(
            run(0x0050_A670, p, &[99], false),
            Registers {
                eax: (RESULT.eax & !0xFF) | 1,
                ..RESULT
            }
        );
        let calls = take_calls();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|c| c.registers.ecx == child_p && c.args == [99]));
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|call| {
                let owner = call.args[0];
                unsafe {
                    let sentinel = load(owner, 0x44);
                    let first = load(sentinel, 0);
                    store(first, 0, sentinel);
                }
            })
        });
        take_calls();
        run(0x0050_A670, p, &[p], false);
        assert_eq!(take_calls().len(), 1);
        assert_eq!(nodes[0][0], sentinel);
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn relative_position_gates_on_type_and_both_references_then_uses_x87_subtractions() {
        let _fpu = GameFpu::new();
        let mut table = [0usize; 0xF0 / 4];
        table[0xEC / 4] = method_zero as *const () as usize;
        table[0xE0 / 4] = method_zero as *const () as usize;
        table[0x1C / 4] = method_one as *const () as usize;
        let mut provider = [table.as_ptr() as u32];
        let provider_p = provider.as_mut_ptr() as u32;
        let mut reference = Object::new(0x30);
        let reference_p = reference.ptr() as u32;
        let mut memory = Object::new(0x60);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0, table.as_ptr() as u32);
        }
        let mut output = [0xA5u32; 5];
        let out = unsafe { output.as_mut_ptr().add(1) } as u32;
        for (kind, has_reference, has_provider, calls_expected) in [
            (0x1234_5678, false, false, 1),
            (0x34, false, false, 2),
            (0x34, true, false, 2),
            (0x34, true, true, 3),
        ] {
            unsafe {
                store(reference_p, 0x28, if has_provider { provider_p } else { 0 });
            }
            RESULTS.with(|v| {
                *v.borrow_mut() = [
                    Registers { eax: kind, ..RESULT },
                    Registers {
                        eax: if has_reference { reference_p } else { 0 },
                        ..RESULT
                    },
                    RESULT,
                ]
                .into_iter()
                .take(calls_expected)
                .collect()
            });
            HOOK.with(|h| {
                *h.borrow_mut() = Some(|call| {
                    if call.id == 3 {
                        for (i, v) in [5.5f32, 6.5, 7.5].into_iter().enumerate() {
                            unsafe {
                                store(call.args[0], i as u32 * 4, v.to_bits());
                            }
                        }
                    }
                })
            });
            for (i, v) in [1.0f32, 2.0, 3.0].into_iter().enumerate() {
                unsafe {
                    store(p, 0x40 + i as u32 * 4, v.to_bits());
                }
            }
            take_calls();
            let r = run(0x0050_9600, p, &[out], false);
            assert_eq!(take_calls().len(), calls_expected);
            if has_provider {
                assert_eq!(
                    r,
                    Registers {
                        eax: (RESULT.eax & !0xFF) | 1,
                        ..RESULT
                    }
                );
                assert_eq!(
                    output,
                    [0xA5, 4.5f32.to_bits(), 4.5f32.to_bits(), 4.5f32.to_bits(), 0xA5]
                );
            } else {
                assert_eq!(
                    r.eax,
                    if kind != 0x34 {
                        kind & !0xFF
                    } else if has_reference {
                        reference_p & !0xFF
                    } else {
                        0
                    }
                );
                assert_eq!(output, [0xA5; 5]);
            }
            HOOK.with(|h| *h.borrow_mut() = None);
        }
    }

    #[test]
    fn rectangle_intersection_accepts_touching_edges_and_nan_and_zeros_separated_results() {
        let _fpu = GameFpu::new();
        for (mut a, mut b, expected, success) in [
            (
                [0.0f32, 0.0, 10.0, 10.0],
                [2.0f32, 3.0, 8.0, 9.0],
                [2.0f32, 3.0, 8.0, 9.0],
                true,
            ),
            ([0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 2.0, 2.0], [1.0; 4], true),
            ([0.0, 0.0, 1.0, 1.0], [2.0, 0.0, 3.0, 1.0], [0.0; 4], false),
            ([2.0, 0.0, 3.0, 1.0], [0.0, 0.0, 1.0, 1.0], [0.0; 4], false),
            ([0.0, 0.0, 1.0, 1.0], [0.0, 2.0, 1.0, 3.0], [0.0; 4], false),
            ([0.0, 2.0, 1.0, 3.0], [0.0, 0.0, 1.0, 1.0], [0.0; 4], false),
            ([f32::NAN; 4], [2.0, 3.0, 8.0, 9.0], [2.0, 3.0, 8.0, 9.0], true),
        ] {
            let a_p = a.as_mut_ptr() as u32;
            let b_p = b.as_mut_ptr() as u32;
            for alias in [false, true] {
                let mut output = [0xA5u32; 6];
                let out = if alias {
                    a_p
                } else {
                    (unsafe { output.as_mut_ptr().add(1) }) as u32
                };
                let r = run(0x004E_8380, out, &[a_p, b_p], false);
                assert_eq!(r.ecx, out);
                assert_eq!(r.edx, a_p);
                assert_eq!(r.eax as u8, u8::from(success));
                assert_eq!(r.eax >> 16, if success { ENTRY.eax >> 16 } else { 0 });
                assert_eq!(
                    bytes(out, 16),
                    expected
                        .iter()
                        .flat_map(|v| v.to_bits().to_le_bytes())
                        .collect::<Vec<_>>()
                );
                assert_eq!((output[0], output[5]), (0xA5, 0xA5));
            }
        }
        let mut a = [0.0f32, 0.0, 10.0, 10.0, 0.0];
        let mut b = [2.0f32, 3.0, 8.0, 9.0];
        let a_p = a.as_mut_ptr() as u32;
        run(0x004E_8380, a_p + 4, &[a_p, b.as_mut_ptr() as u32], false);
        assert_eq!(a, [0.0, 2.0, 3.0, 3.0, 3.0]);
    }

    #[test]
    fn integer_ports_preserve_x87_stack_control_word_and_mxcsr() {
        #[repr(C, align(16))]
        struct State([u8; 512]);
        let mut memory = Object::new(0x280);
        let p = memory.ptr() as u32;
        for address in [
            0x004A_BAD0,
            0x004F_0240,
            0x004F_7BA0,
            0x004F_B340,
            0x004F_BBE0,
            0x0050_5C70,
        ] {
            let mut before = State([0; 512]);
            let mut after = State([0; 512]);
            unsafe {
                core::arch::asm!("fld1", "fxsave [{p}]", p = in(reg) &mut before, out("st(0)") _, options(nostack));
                run(address, p, &[], false);
                core::arch::asm!("fxsave [{p}]", "fstp st(0)", p = in(reg) &mut after, out("st(0)") _, options(nostack));
            }
            assert_eq!(before.0[..160], after.0[..160], "address={address:08x}");
        }
    }

    #[test]
    fn deque_destruction_passes_iterator_copies_then_rereads_owned_map_after_each_call() {
        let _range = fake(0x0050_4CB0, range_release_stub as *const () as usize);
        let _blocks = fake(0x006D_A8D0, method_two as *const () as usize);
        let _delete = fake(0x0090_CF63, free_stub as *const () as usize);
        for (has_map, clear_during_blocks) in [(false, false), (true, false), (true, true)] {
            let mut object = [10u32, 20, 30, 40, 50, 60, 70, 80, if has_map { 0xDEAD_BEEF } else { 0 }];
            let p = object.as_mut_ptr() as u32;
            if clear_during_blocks {
                HOOK.with(|h| {
                    *h.borrow_mut() = Some(|c| {
                        if c.id == 8 {
                            unsafe {
                                store(c.registers.ecx, 0x20, 0);
                            }
                        }
                    })
                });
            }
            take_calls();
            assert_eq!(
                run(0x0050_4D90, p, &[], false),
                if has_map {
                    RESULT
                } else {
                    Registers { eax: 0, ..RESULT }
                }
            );
            let calls = take_calls();
            assert_eq!(calls[0].id, 15);
            assert_eq!(&calls[0].args[..8], &[10, 20, 30, 40, 50, 60, 70, 80]);
            assert_eq!((calls[0].registers.ecx, calls[0].registers.edx), (40, 20));
            assert_eq!(
                calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                if !has_map {
                    vec![15]
                } else if clear_during_blocks {
                    vec![15, 8]
                } else {
                    vec![15, 8, 13]
                }
            );
            if has_map {
                assert_eq!(
                    calls[1].registers,
                    Registers {
                        eax: 40,
                        ecx: p,
                        edx: 84
                    }
                );
                assert_eq!(calls[1].args, [40, 84]);
                if !clear_during_blocks {
                    assert_eq!(calls[2].args, [0xDEAD_BEEF]);
                }
            }
            assert_eq!(&object[..8], &[10, 20, 30, 40, 50, 60, 70, 80]);
            assert_eq!(
                object[8],
                if has_map && !clear_during_blocks {
                    0xDEAD_BEEF
                } else {
                    0
                }
            );
            HOOK.with(|h| *h.borrow_mut() = None);
        }
    }

    #[test]
    fn filtered_enable_reuses_argument_for_filter_and_applies_raw_flag_only_to_accepted_classes() {
        let mut service_table = [0usize; 0x4C / 4];
        service_table[0x48 / 4] = lookup_filter_stub as *const () as usize;
        let mut service = [service_table.as_ptr() as u32, 0];
        let mut global = service.as_mut_ptr().cast::<u8>();
        let _global = fake_data(0x00B4_3DC4, (&mut global as *mut *mut u8).cast());
        let mut filter_table = [0usize; 4];
        filter_table[3] = filter_stub as *const () as usize;
        filter_table[2] = filter_release_stub as *const () as usize;
        let mut filter = [filter_table.as_ptr() as u32];
        let filter_p = filter.as_mut_ptr() as u32;
        let mut child_table = [0usize; 10];
        child_table[5] = class_stub as *const () as usize;
        child_table[9] = enable_stub as *const () as usize;
        let mut child = [child_table.as_ptr() as u32];
        let child_p = child.as_mut_ptr() as u32;
        let mut head = [0u32; 3];
        let sentinel = head.as_mut_ptr() as u32;
        let mut nodes = [[0u32; 3]; 2];
        let node = nodes.as_mut_ptr() as u32;
        nodes[0] = [node + 12, 0, 0];
        nodes[1] = [sentinel, 0, child_p];
        head[0] = node;
        let nodes_before = nodes;
        let head_before = head;
        let mut memory = Object::new(0x60);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0x44, sentinel);
        }
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| {
                if c.id == 16 {
                    unsafe {
                        store(c.args[0], 0, load(c.registers.ecx, 4));
                    }
                }
            })
        });
        for (has_filter, accepted) in [(false, false), (true, false), (true, true)] {
            service[1] = if has_filter { filter_p } else { 0 };
            assert_eq!(service[1] != 0, has_filter);
            let mut responses = VecDeque::from([RESULT]);
            if has_filter {
                responses.push_back(Registers {
                    eax: 0x1234_5678,
                    ..RESULT
                });
                responses.push_back(Registers {
                    eax: if accepted { 0x8000_0001 } else { 0x8000_0000 },
                    ..RESULT
                });
            }
            responses.push_back(RESULT);
            if has_filter {
                responses.push_back(RESULT);
            }
            RESULTS.with(|v| *v.borrow_mut() = responses);
            let mut stack = [0u32, 0xDEAD_BE80];
            let mut r = Registers { ecx: p, ..ENTRY };
            take_calls();
            unsafe {
                set_filtered_children_enabled(&mut r, stack.as_mut_ptr());
            }
            assert_eq!(stack[1], service[1]);
            assert_eq!(
                r,
                Registers {
                    eax: (RESULT.eax & !0xFF) | 0x80,
                    ecx: if has_filter { RESULT.ecx } else { 0 },
                    ..RESULT
                }
            );
            assert_eq!(unsafe { (p.wrapping_add(0x4C) as *const u8).read() }, 0x80);
            let calls = take_calls();
            assert_eq!(
                calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                if has_filter {
                    vec![16, 17, 18, 19, 20]
                } else {
                    vec![16, 19]
                }
            );
            let enable = calls.iter().find(|c| c.id == 19).unwrap();
            assert_eq!(enable.args, [if !has_filter || accepted { 0x80 } else { 0 }]);
            assert_eq!(enable.registers.ecx, child_p);
            if has_filter {
                assert_eq!(calls[2].args, [0x1234_5678]);
                assert_eq!(calls[2].registers.ecx, filter_p);
                assert_eq!(calls.last().unwrap().registers.ecx, filter_p);
            }
        }
        take_calls();
        service[1] = 0;
        assert_eq!(service[1], 0);
        run(0x0050_A6B0, p, &[1], false);
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [16, 19]);
        assert_eq!(nodes, nodes_before);
        assert_eq!(head, head_before);
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn eviction_checks_hard_limit_unsigned_priority_and_first_eligible_child_and_saved_ecx_byte() {
        let mut table = [0usize; 0x100 / 4];
        table[0xFC / 4] = priority_stub as *const () as usize;
        table[0x34 / 4] = remove_stub as *const () as usize;
        let mut child_a = [table.as_ptr() as u32];
        let mut child_b = [table.as_ptr() as u32];
        let a = child_a.as_mut_ptr() as u32;
        let b = child_b.as_mut_ptr() as u32;
        let mut head = [0u32; 3];
        let sentinel = head.as_mut_ptr() as u32;
        let mut nodes = [[0u32; 3]; 2];
        let node = nodes.as_mut_ptr() as u32;
        nodes[0] = [node + 12, 0, a];
        nodes[1] = [sentinel, 0, b];
        let nodes_before = nodes;
        let mut memory = Object::new(0x50);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0x44, sentinel);
            store(p, 0x40, 0x1000);
        }
        for (count, priority, empty, priorities, removed, ids) in [
            (0x1000u32, 128u32, false, vec![], false, vec![]),
            (u32::MAX, 128, false, vec![], false, vec![]),
            (0x123, 0x100, false, vec![], false, vec![]),
            (0x123, 128, true, vec![], false, vec![]),
            (0x123, 128, false, vec![0xABCD_0080, 0xABCD_00FF], false, vec![21, 21]),
            (0x123, 128, false, vec![0xABCD_007F], true, vec![21, 22]),
            (
                0x123,
                128,
                false,
                vec![0xABCD_0080, 0xABCD_0001],
                true,
                vec![21, 21, 22],
            ),
        ] {
            head[0] = if empty { sentinel } else { node };
            assert_eq!(head[0] == sentinel, empty);
            unsafe {
                store(p, 0x48, count);
            }
            let mut responses: VecDeque<_> = priorities.into_iter().map(|eax| Registers { eax, ..RESULT }).collect();
            if removed {
                responses.push_back(RESULT);
            }
            RESULTS.with(|v| *v.borrow_mut() = responses);
            take_calls();
            let result = run(0x0050_A740, p, &[priority], false);
            let calls = take_calls();
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
            assert_eq!(result.eax as u8, u8::from(removed));
            assert_eq!(result.ecx, (p & 0x00FF_FFFF) | (u32::from(removed) << 24));
            assert_eq!(unsafe { load(p, 0x48) }, count);
            if removed {
                assert_eq!(calls.last().unwrap().args, [0]);
                assert_eq!(
                    calls.last().unwrap().registers.ecx,
                    if calls.len() == 2 { a } else { b }
                );
            }
            assert_eq!(nodes, nodes_before);
        }
    }

    #[test]
    fn child_update_observes_simulation_gates_null_children_pending_list_and_signed_boundary() {
        let _fpu = GameFpu::new();
        let mut sim_table = [0usize; 0x48 / 4];
        sim_table[0x44 / 4] = simulation_first_stub as *const () as usize;
        sim_table[0x3C / 4] = simulation_second_stub as *const () as usize;
        let mut sim = [sim_table.as_ptr() as u32];
        let mut global = sim.as_mut_ptr().cast::<u8>();
        let _global = fake_data(0x00B4_3D5C, (&mut global as *mut *mut u8).cast());
        let mut child_table = [0usize; 16];
        child_table[0x3C / 4] = update_stub as *const () as usize;
        child_table[0x30 / 4] = count_stub as *const () as usize;
        child_table[0x2C / 4] = index_stub as *const () as usize;
        let mut child = [child_table.as_ptr() as u32];
        let child_p = child.as_mut_ptr() as u32;
        let mut children = [child_p, 0];
        let begin = children.as_mut_ptr() as u32;
        let mut manager_table = [0usize; 0x19C / 4];
        manager_table[0x198 / 4] = set_flag_stub as *const () as usize;
        let mut memory = Object::new(0x1D0);
        let p = memory.ptr() as u32;
        let mut sentinel = [0u32];
        let sentinel_p = sentinel.as_mut_ptr() as u32;
        for (first, second, has_pending, count, index, flag) in [
            (0u32, 1u32, false, 0u32, 0u32, false),
            (1, 0, false, 0, 0, false),
            (0, 0, false, 0, 0, false),
            (1, 0, true, 4, 2, false),
            (1, 0, true, 4, 3, true),
            (1, 0, true, 0x8000_0000, 0x7FFF_FFFF, true),
        ] {
            unsafe {
                store(p, 0, manager_table.as_ptr() as u32);
                store(p, 0x110, begin);
                store(p, 0x114, begin + 8);
                store(p, 0x188, sentinel_p);
                store(p, 0x1C0, 1.0f32.to_bits());
            }
            sentinel[0] = if has_pending { sentinel_p + 4 } else { sentinel_p };
            assert_eq!(sentinel[0] != sentinel_p, has_pending);
            let stopped = first as u8 == 0 && second as u8 != 0;
            let mut responses = VecDeque::from([Registers { eax: first, ..RESULT }]);
            let mut ids = vec![23];
            if first as u8 == 0 {
                responses.push_back(Registers { eax: second, ..RESULT });
                ids.push(24);
            }
            if !stopped {
                responses.push_back(RESULT);
                ids.push(25);
                if has_pending {
                    responses.push_back(Registers { eax: count, ..RESULT });
                    ids.push(26);
                    responses.push_back(Registers { eax: index, ..RESULT });
                    ids.push(27);
                    if flag {
                        responses.push_back(RESULT);
                        ids.push(28);
                    }
                }
            }
            RESULTS.with(|v| *v.borrow_mut() = responses);
            take_calls();
            let result = run(0x0050_89F0, p, &[0.25f32.to_bits()], false);
            assert_eq!(result.eax as u8, 1);
            assert_eq!(
                unsafe { load(p, 0x1C0) },
                if stopped { 1.0f32 } else { 1.25f32 }.to_bits()
            );
            let calls = take_calls();
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
            for c in &calls {
                if c.id == 25 {
                    assert_eq!(c.args, [0.25f32.to_bits()]);
                    assert_eq!(c.registers.ecx, child_p);
                }
                if c.id == 28 {
                    assert_eq!(c.args, [1, 0]);
                    assert_eq!(c.registers.ecx, p);
                }
            }
        }
    }

    #[test]
    fn notification_arguments_are_captured_before_callbacks_can_rewrite_the_original_stack() {
        let mut table = [0usize; 0x148 / 4];
        table[0x20 / 4] = method_two as *const () as usize;
        table[0x28 / 4] = method_one as *const () as usize;
        table[0x144 / 4] = method_one as *const () as usize;
        let mut offsets = [0u32, 12];
        let mut child = [
            table.as_ptr() as u32,
            offsets.as_mut_ptr() as u32,
            0,
            0,
            table.as_ptr() as u32,
        ];
        let c = child.as_mut_ptr() as u32;
        let mut children = [c, c];
        let begin = children.as_mut_ptr() as u32;
        let mut sentinel = [0u32; 3];
        let sentinel_p = sentinel.as_mut_ptr() as u32;
        let mut nodes = [[0u32; 3]; 2];
        let node = nodes.as_mut_ptr() as u32;
        nodes[0] = [node + 12, 0, c];
        nodes[1] = [sentinel_p, 0, c];
        sentinel[0] = node;
        let nodes_before = nodes;
        let sentinel_before = sentinel;
        let mut memory = Object::new(0x120);
        let p = memory.ptr() as u32;
        unsafe {
            store(p, 0x110, begin);
            store(p, 0x114, begin + 8);
            store(p, 0x44, sentinel_p);
        }
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                store(c.args[0], 4, 0xCC);
                store(c.args[0], 8, 0xDD);
            })
        });
        for (body, pair) in [
            (
                notify_children_pair as unsafe extern "thiscall" fn(&mut Registers, *mut u32),
                true,
            ),
            (notify_virtual_interfaces, false),
            (notify_list_children, false),
        ] {
            let mut stack = [0u32, 0, 0xBB];
            let stack_p = stack.as_mut_ptr() as u32;
            stack[1] = stack_p;
            let mut r = Registers { ecx: p, ..ENTRY };
            take_calls();
            unsafe {
                body(&mut r, stack.as_mut_ptr());
            }
            let calls = take_calls();
            assert_eq!(calls.len(), 2);
            let args = if pair { vec![stack_p, 0xBB] } else { vec![stack_p] };
            assert!(calls.iter().all(|c| c.args == args));
            assert_eq!(&stack[1..], &[0xCC, 0xDD]);
            assert_eq!(nodes, nodes_before);
            assert_eq!(sentinel, sentinel_before);
        }
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn stored_arithmetic_matches_native_memory_operands_for_rounding_zeros_overflow_and_nan_payloads() {
        let _fpu = GameFpu::new();
        for control in [0x007Fu16, 0x047F, 0x087F, 0x0C7F] {
            crate::x87::set_control_word(control);
            for (left, right) in [
                (0u32, 0x8000_0000u32),
                (0x7F7F_FFFF, 0xFF7F_FFFF),
                (0x3F80_0000, 0x3380_0000),
                (0x7F80_0000, 0xFF80_0000),
                (0x7F80_0001, 0x7FC0_1234),
                (0x7FC0_1234, 0x7F80_0001),
                (0xFFC0_1234, 0x7F80_0001),
                (0x7FC0_0001, 0xFFC0_4321),
            ] {
                let a = f32::from_bits(left);
                let b = f32::from_bits(right);
                let (mut sub, mut add) = (0f32, 0f32);
                unsafe {
                    core::arch::asm!("fld dword ptr [{a}]", "fsub dword ptr [{b}]", "fstp dword ptr [{out}]",
                        a = in(reg) &a, b = in(reg) &b, out = in(reg) &mut sub, out("st(0)") _);
                    core::arch::asm!("fld dword ptr [{a}]", "fadd dword ptr [{b}]", "fstp dword ptr [{out}]",
                        a = in(reg) &a, b = in(reg) &b, out = in(reg) &mut add, out("st(0)") _);
                }
                assert_eq!(subtract_stored(F80::from_f32(a), b).to_f32().to_bits(), sub.to_bits());
                assert_eq!(add_stored(F80::from_f32(a), b).to_f32().to_bits(), add.to_bits());
                assert_eq!(crate::x87::control_word(), control);
            }
        }
    }
}
