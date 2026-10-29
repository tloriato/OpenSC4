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

game_data! { static audio_song_tables: [u8; 0x60] = 0x00B2_1A88; }
game_data! { static audio_song_bases: [u32; 2] = 0x00B0_987C; }
game_data! { static audio_resource_manager: *mut u8 = 0x00B4_3CA8; }
game_fn! {
    #[allow(dead_code)]
    fn append_song_record(this: *mut u8, record: *const u32) = "thiscall" 0x004B_2400;
}

port! {
    /// Load up to thirty song properties, carrying each explicit enable byte into subsequent records.
    /// The resource output reuses the caller's argument. Reserve the original local words so the three
    /// untouched padding bytes of each appended record keep their incoming values.
    #[port(address = 0x004F_2910, owner = "shared", name = "LoadAudioSongTable", naked)]
    pub unsafe extern "C" fn fc4p_004f2910() {
        core::arch::naked_asm!(
            "sub esp, 0x28", "push edx", "push ecx", "push eax", "mov ecx, esp",
            "lea edx, [esp + 0x34]", "push edx", "call {}",
            "pop eax", "pop ecx", "pop edx", "add esp, 0x28", "ret",
            sym load_audio_song_table,
        );
    }
}

unsafe extern "thiscall" fn load_audio_song_table(r: &mut Registers, stack: *mut u32) {
    unsafe {
        r.eax = arg(stack, 1);
        if (r.eax as i32) < 0 || r.eax as i32 >= 2 {
            return;
        }
        r.ecx = (audio_song_tables() as u32).wrapping_add(r.eax * 0x30);
        stack.sub(8).write(r.ecx);
        if r.ecx == 0 {
            return;
        }
        let mut id = audio_song_bases().cast::<u32>().add(r.eax as usize).read();
        r.eax = load(r.ecx, 0x14);
        r.ecx = audio_resource_manager().read() as u32;
        stack.sub(5).write(r.eax);
        r.eax = stack.add(1) as u32;
        stack.add(1).write(0);
        let key = stack.sub(7);
        r.eax = key as u32;
        key.write(0x6534_284A);
        key.add(1).write(0xCA25_875D);
        r.edx = load(r.ecx, 0);
        for i in 2..=4 {
            stack.sub(i).write(0);
        }
        stack.sub(1).cast::<u8>().write(1);
        virtual_call(r, 0x0C, &[r.eax, 0xA521_60F5, stack.add(1) as u32, 0, 0]);
        r.ecx = arg(stack, 1);
        if r.eax as u8 == 0 {
            if r.ecx != 0 {
                r.edx = load(r.ecx, 0);
                virtual_call(r, 8, &[]);
            }
            return;
        }
        r.eax = load(r.ecx, 0);
        virtual_call(r, 0x1C, &[]);
        let holder = r.eax;
        let delta = 0x8A5E_7B50u32.wrapping_sub(id);
        r.eax = delta;
        for remaining in (0..30u32).rev() {
            r.edx = load(holder, 0);
            r.eax = r.eax.wrapping_add(id);
            r.ecx = holder;
            virtual_call(r, 0x24, &[r.eax]);
            if r.eax != 0 {
                r.edx = load(r.eax, 0);
                r.ecx = r.eax;
                virtual_call(r, 0x14, &[]);
                let variant = r.eax;
                r.eax = load(variant, 0);
                r.ecx = variant;
                virtual_call(r, 0x19C, &[]);
                let data = r.eax;
                r.edx = load(variant, 0);
                r.ecx = variant;
                virtual_call(r, 0x10, &[]);
                if r.eax >= 2 {
                    r.ecx = load(data, 0);
                    r.edx = load(data, 4);
                    stack.sub(4).write(id);
                    stack.sub(2).write(r.ecx);
                    stack.sub(3).write(r.edx);
                    if r.eax >= 3 {
                        let enabled = (data.wrapping_add(8) as *const u8).read();
                        r.eax = (r.eax & !0xFF) | u32::from(enabled);
                        stack.sub(1).cast::<u8>().write(enabled);
                    }
                    r.ecx = stack.sub(4) as u32;
                    let record = r.ecx;
                    r.ecx = stack.sub(8).read().wrapping_add(8);
                    invoke(r, crate::abi::target(0x004B_2400), &[record], Conv::Thiscall);
                }
            }
            id = id.wrapping_add(1);
            r.eax = remaining;
            if remaining != 0 {
                r.eax = delta;
            }
        }
        r.ecx = arg(stack, 1);
        if r.ecx != 0 {
            stack.add(1).write(0);
            r.edx = load(r.ecx, 0);
            virtual_call(r, 8, &[]);
            r.ecx = arg(stack, 1);
            if r.ecx != 0 {
                r.eax = load(r.ecx, 0);
                virtual_call(r, 8, &[]);
            }
        }
    }
}

game_data! { static simulation_clock: *mut u8 = 0x00B4_3D44; }
game_data! { static city_preferences: *mut u8 = 0x00B4_3C94; }
game_data! { static automaton_time_scale: u32 = 0x00B0_9964; }
game_data! { static automata_stopped: u8 = 0x00B2_1B9C; }
game_data! { static automata_speed_scales: [u32; 4] = 0x00B2_1D18; }
game_data! { static occupant_manager: *mut u8 = 0x00B4_3D0C; }
game_data! { static model_manager: *mut u8 = 0x00B4_3DD0; }
game_data! { static automaton_random: [u32; 4] = 0x00B2_1D78; }
game_data! { static speed_conversion_scale: f32 = 0x00A9_52E8; }
game_data! { static unit_opacity: f32 = 0x00A8_1228; }
game_data! { static retirement_delay: f32 = 0x00A9_52D8; }

game_fn! {
    #[allow(dead_code)]
    fn random_integer_range(this: *mut u8, low: i32, high: i32) -> i32 = "thiscall" 0x0091_3A14;
}
game_fn! {
    #[allow(dead_code)]
    fn random_float_range(this: *mut u8, low_lo: u32, low_hi: u32, high_lo: u32, high_hi: u32) -> St0 = "thiscall" 0x0091_3A46;
}
game_fn! {
    #[allow(dead_code)]
    fn current_opacity(this: *mut u8) -> St0 = "thiscall" 0x004F_A960;
}

/// Take a game's floating return without rounding it or losing its integer scratch results.
unsafe fn invoke_st0(r: &mut Registers, target: usize, args: &[u32]) -> F80 {
    let mut value = [0u8; 10];
    unsafe {
        invoke(r, target, args, Conv::Thiscall);
        core::arch::asm!("fstp tbyte ptr [{value}]", value = in(reg) value.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(value)
}

fn multiply_stored(value: F80, operand: f32) -> F80 {
    let mut result = [0u8; 10];
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value = in(reg) &value, operand = in(reg) &operand,
            result = in(reg) result.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(result)
}

register_port!(
    0x0050_7BF0,
    fc4p_00507bf0,
    update_automaton_time_scale,
    0,
    "UpdateAutomatonTimeScale",
    |r, _stack| {
        unsafe {
            let simulation = simulation_service().read() as u32;
            r.eax &= !0xFF;
            automaton_time_scale().write(1.0f32.to_bits());
            automata_stopped().write(0);
            if simulation == 0 {
                return;
            }
            r.ecx = simulation_clock().read() as u32;
            r.eax = load(r.ecx, 0);
            virtual_call(r, 0x34, &[]);
            if r.eax as u8 != 0 {
                r.ecx = automata_speed_scales().cast::<u32>().add(3).read();
                r.eax = (r.eax & !0xFF) | u32::from(automata_stopped().read());
                automaton_time_scale().write(r.ecx);
                return;
            }
            r.edx = load(simulation, 0);
            r.ecx = simulation;
            virtual_call(r, 0x44, &[]);
            if r.eax as u8 == 0 {
                r.ecx = city_preferences().read() as u32;
                r.eax = load(r.ecx, 0);
                virtual_call(r, 0x98, &[]);
                let enabled = (r.eax.wrapping_add(0xF18) as *const u8).read();
                r.ecx = (r.ecx & !0xFF) | u32::from(enabled);
                if enabled != 0 {
                    r.edx = load(simulation, 0);
                    r.ecx = simulation;
                    virtual_call(r, 0x3C, &[]);
                    if r.eax as u8 != 0 {
                        r.eax = (r.eax & !0xFF) | 1;
                        automaton_time_scale().write(0);
                        automata_stopped().write(1);
                        return;
                    }
                    r.eax = load(simulation, 0);
                    r.ecx = simulation;
                    virtual_call(r, 0x6C, &[]);
                    if r.eax == 0 {
                        r.eax = automata_speed_scales().cast::<u32>().read();
                        automaton_time_scale().write(r.eax);
                        r.eax = (r.eax & !0xFF) | u32::from(automata_stopped().read());
                        return;
                    }
                    r.eax = r.eax.wrapping_sub(2);
                    let speed_two = r.eax == 0;
                    r.eax = (r.eax & !0xFF) | u32::from(automata_stopped().read());
                    if speed_two {
                        r.edx = automata_speed_scales().cast::<u32>().add(2).read();
                        automaton_time_scale().write(r.edx);
                    } else {
                        r.ecx = automata_speed_scales().cast::<u32>().add(1).read();
                        automaton_time_scale().write(r.ecx);
                    }
                    return;
                }
            }
            r.ecx = automata_speed_scales().cast::<u32>().read();
            r.eax = (r.eax & !0xFF) | u32::from(automata_stopped().read());
            automaton_time_scale().write(r.ecx);
        }
    }
);

register_port!(
    0x0050_5780,
    fc4p_00505780,
    configure_random_speed,
    4,
    "ConfigureRandomSpeed",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let tuning = arg(stack, 1);
            let mut speed = f32::from_bits(load(this, 0x80));
            r.eax = load(tuning, 0x38);
            let mut count = r.eax;
            stack.add(1).write(count);
            if r.eax as i32 > 0 {
                r.ecx = load(tuning, 0x3C);
                if r.ecx as i32 > 0 {
                    if r.eax != r.ecx {
                        let high = r.ecx;
                        r.ecx = automaton_random() as u32;
                        invoke(r, crate::abi::target(0x0091_3A14), &[r.eax, high], Conv::Thiscall);
                        count = r.eax;
                        stack.add(1).write(count);
                    }
                    speed = multiply_stored(F80::from_i32(count as i32), speed_conversion_scale().read()).to_f32();
                }
            }
            let low = f32::from_bits(load(tuning, 0x30));
            let high = f32::from_bits(load(tuning, 0x34));
            let status: u16;
            core::arch::asm!("fld dword ptr [{high}]", "fld dword ptr [{low}]", "fucompp", "fnstsw ax",
            high = in(reg) &high, low = in(reg) &low, out("ax") status,
            out("st(0)") _, out("st(1)") _);
            r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
            let scaled = if status & 0x4400 == 0 || status & 0x4400 == 0x4400 {
                // Unequal or unordered endpoints draw once; equal endpoints use the first value.
                let high_bits = F80::from_f32(high).to_f64().to_bits();
                let low_bits = F80::from_f32(low).to_f64().to_bits();
                r.ecx = automaton_random() as u32;
                let factor = invoke_st0(
                    r,
                    crate::abi::target(0x0091_3A46),
                    &[
                        low_bits as u32,
                        (low_bits >> 32) as u32,
                        high_bits as u32,
                        (high_bits >> 32) as u32,
                    ],
                );
                multiply_stored(factor, speed)
            } else {
                multiply_stored(F80::from_f32(speed), low)
            };
            r.eax = load(tuning, 0x40);
            let scaled_bits = scaled.to_f32().to_bits();
            r.ecx = scaled_bits;
            r.edx = load(this, 0);
            let value = r.ecx;
            r.ecx = this;
            virtual_call(r, 0x90, &[value, r.eax]);
            r.ecx = scaled_bits;
        }
    }
);

register_port!(
    0x0050_7E60,
    fc4p_00507e60,
    set_occupant_registration,
    4,
    "SetOccupantRegistration",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let flags = (this.wrapping_add(0x33) as *const u8).read();
            r.edx = (r.edx & !0xFF) | u32::from(flags);
            r.ecx = (r.ecx & !0xFF) | (arg(stack, 1) & 0xFF);
            r.eax = (r.eax & !0xFF) | u32::from(flags & 1);
            if r.eax as u8 == r.ecx as u8 {
                r.eax = (r.eax & !0xFF) | 1;
                return;
            }
            r.ecx = occupant_manager().read() as u32;
            if r.ecx == 0 {
                r.eax &= !0xFF;
                return;
            }
            if r.eax as u8 != 0 {
                if flags & 2 != 0 {
                    r.eax = model_manager().read() as u32;
                    if r.eax != 0 {
                        r.eax = load(this, 0x114);
                        let mut cursor = load(this, 0x110);
                        while cursor != r.eax {
                            r.ecx = load(cursor, 0);
                            if r.ecx != 0 {
                                let manager = model_manager().read() as u32;
                                r.eax = load(r.ecx, 0);
                                let table = load(manager, 0);
                                virtual_call(r, 0, &[]);
                                r.ecx = manager;
                                invoke(r, load(table, 0x74) as usize, &[r.eax], Conv::Thiscall);
                            }
                            r.eax = load(this, 0x114);
                            cursor = cursor.wrapping_add(4);
                        }
                    }
                    r.edx = load(this, 4);
                    r.ecx = this.wrapping_add(4);
                    virtual_call(r, 0x7C, &[2]);
                    r.ecx = occupant_manager().read() as u32;
                }
                r.eax = load(r.ecx, 0);
                virtual_call(r, 0x54, &[this.wrapping_add(4), 0, 0]);
                r.edx = load(this, 4);
                r.ecx = this.wrapping_add(4);
                virtual_call(r, 0x7C, &[1]);
                r.eax = (r.eax & !0xFF) | 1;
                return;
            }
            r.eax = load(this, 0x110);
            if r.eax != load(this, 0x114) {
                r.edx = r.eax;
                r.eax = load(r.edx, 0);
                if r.eax != 0 {
                    r.edx = load(r.eax, 4);
                    r.edx = load(r.edx, 4);
                    r.eax = r.edx.wrapping_add(r.eax).wrapping_add(4);
                    if r.eax != 0 {
                        r.edx = load(r.eax, 0);
                        let output = this.wrapping_add(0x170);
                        r.ecx = r.eax;
                        virtual_call(r, 0x20, &[output]);
                        r.ecx = occupant_manager().read() as u32;
                    }
                }
            }
            r.edx = load(r.ecx, 0);
            virtual_call(r, 0x50, &[this.wrapping_add(4), 0]);
            if r.eax as u8 == 0 {
                r.eax &= !0xFF;
                return;
            }
            r.eax = load(this, 4);
            r.ecx = this.wrapping_add(4);
            virtual_call(r, 0x74, &[1]);
            r.eax = (r.eax & !0xFF) | 1;
        }
    }
);

register_port!(
    0x0050_7F70,
    fc4p_00507f70,
    set_model_registration,
    4,
    "SetModelRegistration",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.edx = (r.edx & !0xFF) | (arg(stack, 1) & 0xFF);
            let flags = (this.wrapping_add(0x33) as *const u8).read();
            r.ecx = (r.ecx & !0xFF) | u32::from(flags);
            r.eax = (r.eax & !0xFF) | u32::from((flags >> 1) & 1);
            if r.eax as u8 == r.edx as u8 {
                r.eax = (r.eax & !0xFF) | 1;
                r.ecx = this;
                return;
            }
            r.edx = model_manager().read() as u32;
            if r.edx == 0 {
                r.eax &= !0xFF;
                r.ecx = this;
                return;
            }
            if r.eax as u8 != 0 {
                let mut cursor = load(this, 0x110);
                while cursor != load(this, 0x114) {
                    r.ecx = load(cursor, 0);
                    if r.ecx != 0 {
                        let manager = model_manager().read() as u32;
                        r.eax = load(r.ecx, 0);
                        let table = load(manager, 0);
                        virtual_call(r, 0, &[]);
                        r.ecx = manager;
                        invoke(r, load(table, 0x74) as usize, &[r.eax], Conv::Thiscall);
                    }
                    r.eax = load(this, 0x114);
                    cursor = cursor.wrapping_add(4);
                }
                r.edx = load(this, 4);
                r.ecx = this.wrapping_add(4);
                virtual_call(r, 0x7C, &[2]);
                r.eax = (r.eax & !0xFF) | 1;
                r.ecx = this;
                return;
            }
            if flags & 1 == 0 {
                r.eax = load(this, 0);
                r.ecx = this;
                virtual_call(r, 0x118, &[1]);
            }
            if (this.wrapping_add(0x33) as *const u8).read() & 1 == 0 {
                r.eax &= !0xFF;
                r.ecx = this;
                return;
            }
            r.ecx = this.wrapping_add(0x1B0);
            let opacity = invoke_st0(r, crate::abi::target(0x004F_A960), &[]).to_f32();
            stack.add(1).write(opacity.to_bits());
            let mut scratch = this;
            let mut cursor = load(this, 0x110);
            r.eax = (r.eax & !0xFF) | 1;
            let mut complete = true;
            while cursor != load(this, 0x114) {
                if r.eax as u8 == 0 {
                    complete = false;
                    break;
                }
                let model = load(cursor, 0);
                if model != 0 {
                    r.edx = load(model, 0);
                    r.ecx = model;
                    let status = compare_stored(F80::from_f32(opacity), unit_opacity().read());
                    r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
                    if status & 0x0500 == 0x0100 || status & 0x0500 == 0x0400 {
                        // Ordered opacity below one selects the tweened value.
                        r.eax = opacity.to_bits();
                        virtual_call(r, 0x48, &[1, r.eax]);
                    } else {
                        virtual_call(r, 0x48, &[0, 1.0f32.to_bits()]);
                    }
                    let manager = model_manager().read() as u32;
                    r.eax = load(manager, 0);
                    r.edx = load(model, 0);
                    r.ecx = model;
                    let table = r.eax;
                    scratch = table;
                    virtual_call(r, 0, &[]);
                    let handle = r.eax;
                    r.eax = table;
                    r.ecx = manager;
                    invoke(r, load(table, 0x70) as usize, &[handle], Conv::Thiscall);
                    r.eax = (r.eax & !0xFF) | u32::from(r.eax as u8 != 0);
                }
                r.ecx = load(this, 0x114);
                cursor = cursor.wrapping_add(4);
            }
            if complete && r.eax as u8 != 0 {
                r.eax = load(this, 4);
                r.ecx = this.wrapping_add(4);
                virtual_call(r, 0x74, &[2]);
            }
            r.eax &= !0xFF;
            r.ecx = scratch;
        }
    }
);

game_data! { static movement_direction_tolerance: f32 = 0x00A9_52DC; }
game_data! { static movement_matrix_default: u32 = 0x00B2_1BA4; }
game_data! { static movement_cell_scale: f32 = 0x00A9_52F0; }
game_data! { static movement_integer_bias: f32 = 0x00A9_52F4; }
game_data! { static movement_obstacles: *mut u8 = 0x00B4_3D08; }

game_fn! {
    #[allow(dead_code)]
    fn direction_to_matrix(direction: *const f32, matrix: *mut f32) = "C" 0x0080_B830;
}

layout! {
    /// Movement fields from the automaton's primary interface. Vector words retain their float bits.
    struct MovementAutomaton {
        0x00 => vtable / set_vtable: u32,
        0x40 => position_x / set_position_x: u32,
        0x44 => position_y / set_position_y: u32,
        0x48 => position_z / set_position_z: u32,
        0x58 => direction_x / set_direction_x: u32,
        0x5C => direction_y / set_direction_y: u32,
        0x60 => direction_z / set_direction_z: u32,
        0x110 => models_begin / set_models_begin: u32,
        0x114 => models_end / set_models_end: u32,
    }
}

port! {
    /// Update optional direction and position, publish each model's transform, test the city and terrain
    /// limits, then notify movement observers. An invalid position removes the automaton immediately.
    /// The two change words and transform header retain their untouched stack bytes. The adapter keeps
    /// that storage at the same distance below the caller's return address; the logic runs in Rust.
    #[port(address = 0x0050_80C0, owner = "shared", name = "MoveAutomaton", naked)]
    pub unsafe extern "C" fn fc4p_005080c0() {
        core::arch::naked_asm!(
            "sub esp, 0x74", "push edx", "push ecx", "push eax", "mov ecx, esp",
            "lea edx, [esp + 12]", "call {}",
            "pop eax", "pop ecx", "pop edx", "add esp, 0x74", "ret 8",
            sym move_automaton,
        );
    }
}

/// Reset the matrix, translation and scale, and only the low half of the transform header.
unsafe fn initialize_movement_transform(frame: u32) {
    unsafe {
        for offset in (0x40..=0x70).step_by(4) {
            store(
                frame,
                offset,
                if matches!(offset, 0x40 | 0x50 | 0x60 | 0x70) {
                    0x3F80_0000
                } else {
                    0
                },
            );
        }
        write_unaligned(frame.wrapping_add(0x3C) as *mut u16, 0);
    }
}

/// Read the model's virtual-base displacement again before each transform method.
unsafe fn movement_model_interface(model: u32) -> u32 {
    unsafe { model.wrapping_add(load(load(model, 4), 4)).wrapping_add(4) }
}

/// The integer cell uses a biased float store and an exact-integer correction under the caller's
/// rounding mode. Keep the scaled coordinate unrounded until the final unordered comparison.
unsafe fn movement_cell(coordinate: f32, scratch: u32) -> i32 {
    unsafe {
        let scaled = multiply_stored(F80::from_f32(coordinate), movement_cell_scale().read());
        let half = F80::from_f32(footprint_half_scale().read());
        let mut difference = F80::ZERO;
        core::arch::asm!(
            "fld tbyte ptr [{scaled}]", "fld tbyte ptr [{half}]", "fsubr st(0), st(1)",
            "fstp tbyte ptr [{difference}]", "fstp st(0)",
            scaled = in(reg) &scaled, half = in(reg) &half, difference = in(reg) &mut difference,
            out("st(0)") _, out("st(1)") _,
        );
        let bits = add_stored(difference, movement_integer_bias().read())
            .to_f32()
            .to_bits();
        store(scratch, 0, bits);
        let cell = bits.wrapping_add(0xB4C0_0000) as i32;
        let next = cell.wrapping_add(1);
        store(scratch, 0, next as u32);
        let status: u16;
        core::arch::asm!(
            "fild dword ptr [{scratch}]", "fld tbyte ptr [{scaled}]", "fucompp", "fnstsw ax",
            scratch = in(reg) scratch, scaled = in(reg) &scaled, out("ax") status,
            out("st(0)") _, out("st(1)") _,
        );
        if status & 0x4400 == 0x4000 {
            next
        } else {
            cell
        }
    }
}

unsafe extern "fastcall" fn move_automaton(r: &mut Registers, frame: u32) {
    unsafe {
        let this = r.ecx;
        let automaton = MovementAutomaton::from_ptr(this as *mut u8);
        let direction = load(frame, 0x7C);
        r.ecx = direction;
        (frame.wrapping_add(0x10) as *mut u8).write(0);
        (frame.wrapping_add(0x14) as *mut u8).write(0);
        if direction != 0 {
            r.eax = load(direction, 4);
            r.edx = r.eax & 0x7F80_0000;
            store(frame, 0x7C, r.eax);
            if r.edx != 0x7F80_0000 || r.eax & 0x007F_FFFF == 0 {
                let mut accepted = false;
                for offset in [0, 4, 8] {
                    let status = compare_stored(
                        F80::from_f32(f32::from_bits(load(direction, offset))).abs(),
                        movement_direction_tolerance().read(),
                    );
                    r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
                    if status & 0x0500 != 0x0100 && status & 0x0500 != 0x0400 {
                        accepted = true;
                        break;
                    }
                }
                if accepted {
                    r.eax = movement_matrix_default().read();
                    for offset in [0x1C, 0x24, 0x2C, 0x34] {
                        store(frame, offset, r.eax);
                    }
                    for offset in [0, 4, 8] {
                        store(this, 0x58 + offset, load(direction, offset));
                    }
                    for offset in [0x18, 0x20, 0x28, 0x30, 0x38] {
                        store(frame, offset, r.eax);
                    }
                    let z = F80::from_f32(f32::from_bits(automaton.direction_z()));
                    let y = F80::from_f32(f32::from_bits(automaton.direction_y()));
                    let x = F80::from_f32(f32::from_bits(automaton.direction_x()));
                    let length = add_popped(add_popped(x * x, y * y), z * z).sqrt();
                    let reciprocal = divide_into_stored(length, footprint_unit_length().read());
                    let x = multiply_stored(reciprocal, f32::from_bits(load(this, 0x58)));
                    store(this, 0x58, x.to_f32().to_bits());
                    let y = multiply_stored(reciprocal, f32::from_bits(load(this, 0x5C)));
                    store(this, 0x5C, y.to_f32().to_bits());
                    let z = multiply_stored(reciprocal, f32::from_bits(load(this, 0x60)));
                    store(this, 0x60, z.to_f32().to_bits());
                    store(this, 0x58, x.to_f32().to_bits());
                    r.eax = this.wrapping_add(0x58);
                    r.edx = load(r.eax, 4);
                    store(r.eax, 4, r.edx);
                    r.ecx = load(r.eax, 8);
                    r.edx = frame.wrapping_add(0x18);
                    store(r.eax, 8, r.ecx);
                    invoke(r, crate::abi::target(0x0080_B830), &[r.eax, r.edx], Conv::Cdecl);
                    let mut cursor = automaton.models_begin();
                    r.eax = automaton.models_end();
                    while cursor != r.eax {
                        let model = load(cursor, 0);
                        if model != 0 {
                            initialize_movement_transform(frame);
                            r.ecx = movement_model_interface(model);
                            r.edx = load(r.ecx, 0);
                            r.eax = frame.wrapping_add(0x3C);
                            virtual_call(r, 0x24, &[r.eax]);
                            for index in 0..9 {
                                store(frame, 0x40 + index * 4, load(frame, 0x18 + index * 4));
                            }
                            let header = frame.wrapping_add(0x3C) as *mut u8;
                            header.add(1).write(header.add(1).read() | 4);
                            header.write(header.read().wrapping_add(1));
                            r.ecx = movement_model_interface(model);
                            r.eax = load(r.ecx, 0);
                            r.edx = header as u32;
                            virtual_call(r, 0x28, &[r.edx]);
                        }
                        r.eax = automaton.models_end();
                        cursor = cursor.wrapping_add(4);
                    }
                    (frame.wrapping_add(0x10) as *mut u8).write(1);
                }
            }
        }
        let position = load(frame, 0x78);
        if position != 0 {
            for offset in [0, 8] {
                let status = compare_stored(
                    F80::from_f32(f32::from_bits(load(position, offset))),
                    footprint_zero().read(),
                );
                r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
                if status & 0x0500 == 0x0100 || status & 0x0500 == 0x0400 {
                    r.eax = load(this, 0);
                    r.ecx = this;
                    virtual_call(r, 0x34, &[0]);
                    return;
                }
            }
            for (offset, slot) in [(0, 0x164), (8, 0x168)] {
                r.ecx = automaton_city().read() as u32;
                r.eax = load(position, offset);
                r.edx = load(r.ecx, 0);
                store(frame, 0x7C, r.eax);
                let bound = invoke_st0(r, crate::abi::vslot(r.ecx as *mut u8, slot), &[]);
                let status = compare_stored(bound, f32::from_bits(load(frame, 0x7C)));
                r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
                if matches!(status & 0x4100, 0x0100 | 0x4000) {
                    r.eax = load(this, 0);
                    r.ecx = this;
                    virtual_call(r, 0x34, &[0]);
                    return;
                }
            }
            r.eax = load(position, 0);
            automaton.set_position_x(r.eax);
            r.ecx = load(position, 4);
            automaton.set_position_y(r.ecx);
            r.edx = load(position, 8);
            automaton.set_position_z(r.edx);
            let mut cursor = automaton.models_begin();
            while cursor != automaton.models_end() {
                let model = load(cursor, 0);
                if model != 0 {
                    initialize_movement_transform(frame);
                    r.ecx = movement_model_interface(model);
                    r.edx = load(r.ecx, 0);
                    r.eax = frame.wrapping_add(0x3C);
                    virtual_call(r, 0x24, &[r.eax]);
                    r.ecx = load(position, 0);
                    r.eax = load(position, 8);
                    r.edx = load(position, 4);
                    store(frame, 0x64, r.ecx);
                    store(frame, 0x6C, r.eax);
                    store(frame, 0x68, r.edx);
                    let header = frame.wrapping_add(0x3C) as *mut u8;
                    header.add(1).write(header.add(1).read() | 1);
                    header.write(header.read().wrapping_add(1));
                    r.ecx = movement_model_interface(model);
                    r.eax = load(r.ecx, 0);
                    r.edx = header as u32;
                    virtual_call(r, 0x28, &[r.edx]);
                }
                r.eax = automaton.models_end();
                cursor = cursor.wrapping_add(4);
            }
            r.eax = load(this, 0);
            r.ecx = this;
            virtual_call(r, 0x7C, &[1]);
            if r.eax as u8 == 0 {
                r.edx = load(this, 0);
                r.ecx = this;
                virtual_call(r, 0x7C, &[0x0040_0000]);
                let mut check_terrain = r.eax as u8 != 0;
                if !check_terrain {
                    r.eax = load(this, 0);
                    r.ecx = this;
                    virtual_call(r, 0x7C, &[0x0080_0000]);
                    check_terrain = r.eax as u8 != 0;
                }
                if check_terrain {
                    r.ecx = automaton_city().read() as u32;
                    r.edx = load(r.ecx, 0);
                    let obstacles = movement_obstacles().read() as u32;
                    virtual_call(r, 0xA0, &[]);
                    store(frame, 0x78, r.eax);
                    let x = movement_cell(f32::from_bits(automaton.position_x()), frame.wrapping_add(0x7C));
                    let z = movement_cell(f32::from_bits(automaton.position_z()), frame.wrapping_add(0x7C));
                    r.eax = load(this, 0);
                    r.ecx = this;
                    virtual_call(r, 0x7C, &[0x0040_0000]);
                    let mut remove = false;
                    if r.eax as u8 != 0 {
                        r.edx = load(obstacles, 0);
                        r.ecx = obstacles;
                        virtual_call(r, 0x18, &[x as u32, z as u32, 0]);
                        remove = r.eax != 0;
                        if remove {
                            r.eax = load(this, 0);
                            r.ecx = this;
                            virtual_call(r, 0x34, &[0]);
                        }
                    }
                    if !remove {
                        r.edx = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0x7C, &[0x0080_0000]);
                        if r.eax as u8 != 0 {
                            r.ecx = load(frame, 0x78);
                            r.eax = load(r.ecx, 0);
                            virtual_call(r, 0x38, &[x as u32, z as u32]);
                            if r.eax != 0 {
                                r.edx = load(this, 0);
                                r.ecx = this;
                                virtual_call(r, 0x34, &[0]);
                            }
                        }
                    }
                }
            }
            (frame.wrapping_add(0x14) as *mut u8).write(1);
        }
        r.eax = (r.eax & !0xFF) | u32::from((frame.wrapping_add(0x10) as *const u8).read());
        if r.eax as u8 != 0 || position != 0 {
            r.edx = load(this, 0);
            r.ecx = this;
            virtual_call(r, 0x1AC, &[]);
            r.eax = load(this, 0);
            r.ecx = this;
            virtual_call(r, 0x1A0, &[]);
        }
        r.eax = load(frame, 0x10);
        r.ecx = load(frame, 0x14);
        r.edx = load(this, 0);
        let changes = [r.ecx, r.eax];
        r.ecx = this;
        virtual_call(r, 0x190, &changes);
    }
}

register_port!(
    0x0050_8610,
    fc4p_00508610,
    publish_automaton_models,
    8,
    "PublishAutomatonModels",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0x110);
            if r.eax != load(this, 0x114) {
                r.ecx = r.eax;
                r.eax = load(r.ecx, 0);
                if r.eax == 0 {
                    r.ecx = 0;
                } else {
                    r.edx = load(r.eax, 4);
                    r.ecx = load(r.edx, 4).wrapping_add(r.eax).wrapping_add(4);
                }
                r.edx = load(r.ecx, 0);
                r.eax = this.wrapping_add(0x170);
                virtual_call(r, 0x20, &[r.eax]);
            }
            r.edx = load(this, 4);
            r.ecx = this.wrapping_add(4);
            virtual_call(r, 0x80, &[4]);
            if r.eax as u8 != 0 {
                let flags = (this.wrapping_add(0x33) as *const u8).read();
                r.eax = (r.eax & !0xFF) | u32::from(flags);
                if flags & 2 != 0 {
                    r.eax = load(this, 0x114);
                    let mut cursor = load(this, 0x110);
                    while cursor != r.eax {
                        r.ecx = load(cursor, 0);
                        if r.ecx != 0 {
                            r.eax = (r.eax & !0xFF) | (arg(stack, 1) & 0xFF);
                            if r.eax as u8 != 0 {
                                let manager = model_manager().read() as u32;
                                r.eax = load(r.ecx, 0);
                                let table = load(manager, 0);
                                virtual_call(r, 0, &[]);
                                r.ecx = manager;
                                invoke(r, load(table, 0x78) as usize, &[r.eax], Conv::Thiscall);
                            } else {
                                r.eax = (r.eax & !0xFF) | (arg(stack, 2) & 0xFF);
                                if r.eax as u8 != 0 {
                                    let manager = model_manager().read() as u32;
                                    r.edx = load(r.ecx, 0);
                                    let table = load(manager, 0);
                                    virtual_call(r, 0, &[]);
                                    r.ecx = manager;
                                    invoke(r, load(table, 0x7C) as usize, &[r.eax, 3], Conv::Thiscall);
                                }
                            }
                        }
                        r.eax = load(this, 0x114);
                        cursor = cursor.wrapping_add(4);
                    }
                    if (this.wrapping_add(0x33) as *const u8).read() & 1 != 0 {
                        r.ecx = occupant_manager().read() as u32;
                        r.edx = load(r.ecx, 0);
                        r.eax = this.wrapping_add(4);
                        virtual_call(r, 0x60, &[r.eax, 0]);
                    }
                } else {
                    if flags & 1 != 0 {
                        r.ecx = occupant_manager().read() as u32;
                        r.eax = load(r.ecx, 0);
                        virtual_call(r, 0x60, &[this.wrapping_add(4), 0]);
                    } else {
                        r.edx = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0x118, &[1]);
                    }
                    if (this.wrapping_add(0x33) as *const u8).read() & 1 != 0 {
                        r.eax = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0x11C, &[1]);
                    }
                }
            }
            let elapsed = f32::from_bits(load(this, 0x1C0));
            let status = compare_stored(F80::from_f32(elapsed), retirement_delay().read());
            r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
            if status & 0x4100 == 0 {
                r.edx = load(this, 0);
                r.ecx = this;
                virtual_call(r, 0x198, &[9, 0]);
                store(this, 0x1C0, 0);
            }
        }
    }
);

game_data! { static footprint_wide_margin: f32 = 0x00A8_825C; }
game_data! { static footprint_half_scale: f32 = 0x00A8_4D2C; }
game_data! { static footprint_unit_length: f32 = 0x00B0_9978; }
game_data! { static footprint_zero: f32 = 0x00A8_1054; }
game_fn! {
    #[allow(dead_code)]
    fn multiply_footprint_axes(this: *mut u8, output: *mut f32, other: *const f32) -> *mut f32 = "thiscall" 0x0050_5240;
}

fn divide_into_stored(value: F80, numerator: f32) -> F80 {
    let mut result = [0u8; 10];
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fdivr dword ptr [{numerator}]", "fstp tbyte ptr [{result}]",
            value = in(reg) &value, numerator = in(reg) &numerator,
            result = in(reg) result.as_mut_ptr(), out("st(0)") _);
    }
    F80::from_bytes(result)
}

fn add_popped(first: F80, second: F80) -> F80 {
    let mut result = [0u8; 10];
    unsafe {
        core::arch::asm!("fld tbyte ptr [{first}]", "fld tbyte ptr [{second}]", "faddp st(1), st(0)",
            "fstp tbyte ptr [{result}]", first = in(reg) &first, second = in(reg) &second,
            result = in(reg) result.as_mut_ptr(), out("st(0)") _, out("st(1)") _);
    }
    F80::from_bytes(result)
}

fn subtract_popped(first: F80, second: F80) -> F80 {
    let mut result = [0u8; 10];
    unsafe {
        core::arch::asm!("fld tbyte ptr [{first}]", "fld tbyte ptr [{second}]", "fsubp st(1), st(0)",
            "fstp tbyte ptr [{result}]", first = in(reg) &first, second = in(reg) &second,
            result = in(reg) result.as_mut_ptr(), out("st(0)") _, out("st(1)") _);
    }
    F80::from_bytes(result)
}

port! {
    /// Test the four separating axes in order. Invalid footprints reject immediately; touching edges
    /// and unordered comparisons overlap. The caller's second-footprint argument becomes the negative
    /// sine of the relative axes, even when a later axis rejects the overlap.
    #[port(address = 0x0050_6450, owner = "shared", name = "FootprintsOverlap", naked)]
    pub unsafe extern "C" fn fc4p_00506450() {
        core::arch::naked_asm!(
            "sub esp, 0x18", "push edx", "push ecx", "push eax", "mov ecx, esp",
            "lea edx, [esp + 0x24]", "push edx", "call {}",
            "pop eax", "pop ecx", "pop edx", "add esp, 0x18", "ret 4",
            sym footprints_overlap,
        );
    }
}

unsafe extern "thiscall" fn footprints_overlap(r: &mut Registers, stack: *mut u32) {
    unsafe {
        let first = r.ecx;
        if (first as *const u8).read() == 0 {
            r.eax &= !0xFF;
            return;
        }
        let second = arg(stack, 1);
        if (second as *const u8).read() == 0 {
            r.eax &= !0xFF;
            return;
        }
        let dx = subtract_stored(
            F80::from_f32(f32::from_bits(load(second, 4))),
            f32::from_bits(load(first, 4)),
        );
        r.eax = load(first, 0x1C);
        r.ecx = r.eax;
        let dz = subtract_stored(
            F80::from_f32(f32::from_bits(load(second, 8))),
            f32::from_bits(load(first, 8)),
        );
        let ax = f32::from_bits(r.eax);
        let az = (-F80::from_f32(f32::from_bits(load(first, 0x20)))).to_f32();
        let axes = stack.sub(6).cast::<f32>();
        axes.write(ax);
        axes.add(1).write(az);
        let output = stack.sub(4).cast::<f32>();
        output.write(ax);
        output.add(1).write(az);
        let tx = subtract_popped(F80::from_f32(ax) * dx, F80::from_f32(az) * dz).to_f32();
        stack.sub(2).write(tx.to_bits());
        let tz = add_popped(multiply_stored(dz, ax), F80::from_f32(az) * dx).to_f32();
        stack.sub(1).write(tz.to_bits());
        let az = (-F80::from_f32(f32::from_bits(load(first, 0x20)))).to_f32();
        axes.add(1).write(az);
        r.eax = second.wrapping_add(0x1C);
        r.edx = az.to_bits();
        r.ecx = axes as u32;
        invoke(
            r,
            crate::abi::target(0x0050_5240),
            &[output as u32, r.eax],
            Conv::Thiscall,
        );
        let m = output.read();
        let s = output.add(1).read();
        let negative_s = -F80::from_f32(s);
        stack.add(1).write(negative_s.to_f32().to_bits());
        let abs_tx = F80::from_f32(tx).abs();
        let other = multiply_stored(negative_s, f32::from_bits(load(second, 16))).abs();
        let term = multiply_stored(F80::from_f32(m), f32::from_bits(load(second, 12))).abs();
        let _ = term.to_f32();
        let radius = add_stored(add_popped(other, term), f32::from_bits(load(first, 12)));
        let status = compare_stored(subtract_popped(abs_tx, radius), footprint_zero().read());
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
        if status & 0x4100 == 0 {
            r.eax &= !0xFF;
            return;
        }
        let abs_tz = F80::from_f32(tz).abs();
        let other = multiply_stored(F80::from_f32(m), f32::from_bits(load(second, 16))).abs();
        let term = multiply_stored(F80::from_f32(s), f32::from_bits(load(second, 12))).abs();
        let _ = term.to_f32();
        let radius = add_stored(add_popped(other, term), f32::from_bits(load(first, 16)));
        let status = compare_stored(subtract_popped(abs_tz, radius), footprint_zero().read());
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
        if status & 0x4100 == 0 {
            r.eax &= !0xFF;
            return;
        }
        let abs_m = F80::from_f32(m).abs().to_f32();
        let projected = add_popped(
            multiply_stored(F80::from_f32(s), tz),
            multiply_stored(F80::from_f32(m), tx),
        )
        .abs();
        let radius = add_stored(
            add_popped(
                multiply_stored(F80::from_f32(s).abs(), f32::from_bits(load(first, 16))),
                multiply_stored(F80::from_f32(abs_m), f32::from_bits(load(first, 12))),
            ),
            f32::from_bits(load(second, 12)),
        );
        let status = compare_stored(subtract_popped(projected, radius), footprint_zero().read());
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
        if status & 0x4100 == 0 {
            r.eax &= !0xFF;
            return;
        }
        let negative_s = f32::from_bits(arg(stack, 1));
        let projected = add_popped(
            multiply_stored(F80::from_f32(m), tz),
            multiply_stored(F80::from_f32(negative_s), tx),
        )
        .abs();
        let radius = add_stored(
            add_popped(
                multiply_stored(F80::from_f32(negative_s).abs(), f32::from_bits(load(first, 12))),
                multiply_stored(F80::from_f32(abs_m), f32::from_bits(load(first, 16))),
            ),
            f32::from_bits(load(second, 16)),
        );
        let status = compare_stored(subtract_popped(projected, radius), footprint_zero().read());
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
        r.eax = (r.eax & !0xFF) | u32::from(status & 0x4100 != 0);
    }
}

port! {
    /// Build a horizontal footprint from the first model's bounds and transform. A missing model keeps
    /// validity and axis bytes; successful construction ignores the vertical offset and retains the
    /// original argument scratch writes. Local transform padding keeps its incoming bytes.
    #[port(address = 0x0050_A310, owner = "shared", name = "BuildModelFootprint", naked)]
    pub unsafe extern "C" fn fc4p_0050a310() {
        core::arch::naked_asm!(
            "sub esp, 0x5C", "push edx", "push ecx", "push eax", "mov ecx, esp",
            "lea edx, [esp + 0x68]", "push edx", "call {}",
            "pop eax", "pop ecx", "pop edx", "add esp, 0x5C", "ret 20",
            sym build_model_footprint,
        );
    }
}

unsafe extern "thiscall" fn build_model_footprint(r: &mut Registers, stack: *mut u32) {
    unsafe {
        let this = r.ecx;
        let automaton = arg(stack, 1);
        for offset in [4, 8, 12, 16, 20, 24] {
            store(this, offset, 0);
        }
        let bounds = stack.sub(20).cast::<f32>();
        for i in 0..6 {
            bounds.add(i).write(if i < 3 { 1.0 } else { 0.0 });
        }
        r.eax = load(automaton, 0);
        r.ecx = automaton;
        virtual_call(r, 0x148, &[]);
        if r.eax == 0 {
            r.eax = this;
            return;
        }
        r.ecx = load(r.eax, 4);
        r.edx = load(r.ecx, 4);
        let geometry = r.edx.wrapping_add(r.eax).wrapping_add(4);
        if geometry == 0 {
            r.eax = this;
            return;
        }
        r.eax = load(geometry, 0);
        r.ecx = geometry;
        virtual_call(r, 0x1C, &[bounds as u32]);
        let dx = stack.sub(23).cast::<f32>();
        let dz = stack.sub(21).cast::<f32>();
        dx.write(subtract_stored(F80::from_f32(bounds.add(3).read()), bounds.read()).to_f32());
        r.edx = load(automaton, 0);
        r.eax = stack.add(1) as u32;
        r.ecx = automaton;
        stack.add(1).write(0);
        dz.write(subtract_stored(F80::from_f32(bounds.add(5).read()), bounds.add(2).read()).to_f32());
        virtual_call(r, 0, &[0x4A41_6254, r.eax]);
        let x_margin = if r.eax as u8 != 0 {
            footprint_wide_margin().read()
        } else {
            footprint_half_scale().read()
        };
        dx.write(subtract_stored(F80::from_f32(dx.read()), x_margin).to_f32());
        let z_margin = if r.eax as u8 != 0 {
            unit_opacity().read()
        } else {
            footprint_half_scale().read()
        };
        dz.write(subtract_stored(F80::from_f32(dz.read()), z_margin).to_f32());
        r.ecx = arg(stack, 1);
        if r.ecx != 0 {
            r.edx = load(r.ecx, 0);
            virtual_call(r, 8, &[]);
        }
        let transform = stack.sub(14).cast::<u8>();
        for i in 0..13 {
            let value = if [0, 4, 8, 12].contains(&i) { 1.0f32 } else { 0.0 };
            write_unaligned(transform.add(4 + i * 4).cast::<f32>(), value);
        }
        transform.write(0);
        transform.add(1).write(0);
        r.ecx = transform as u32;
        r.eax = load(geometry, 0);
        let transform_p = r.ecx;
        r.ecx = geometry;
        virtual_call(r, 0x24, &[transform_p]);
        let tx = read_unaligned(transform.add(0x28).cast::<f32>());
        let tz = read_unaligned(transform.add(0x30).cast::<f32>());
        let x = add_stored(F80::from_f32(f32::from_bits(arg(stack, 2))), tx);
        let z = add_stored(F80::from_f32(f32::from_bits(arg(stack, 4))), tz);
        store(this, 4, x.to_f32().to_bits());
        store(this, 8, z.to_f32().to_bits());
        let half_x = multiply_stored(F80::from_f32(dx.read()), footprint_half_scale().read());
        let half_z = multiply_stored(F80::from_f32(dz.read()), footprint_half_scale().read()).to_f32();
        stack.add(3).write(half_z.to_bits());
        let extent_x = multiply_stored(half_x, f32::from_bits(arg(stack, 5)));
        let extent_z = multiply_stored(F80::from_f32(half_z), f32::from_bits(arg(stack, 5)));
        store(this, 12, extent_x.to_f32().to_bits());
        store(this, 16, extent_z.to_f32().to_bits());
        let axis_z = (-F80::from_f32(read_unaligned(transform.add(0xC).cast::<f32>()))).to_f32();
        stack.add(3).write(axis_z.to_bits());
        let axis_x = read_unaligned(transform.add(4).cast::<f32>());
        let x2 = multiply_stored(F80::from_f32(axis_x), axis_x);
        let z2 = multiply_stored(F80::from_f32(axis_z), axis_z);
        // The reciprocal uses the length, with a stored rounding step before either axis product.
        let reciprocal = divide_into_stored(add_popped(x2, z2).sqrt(), footprint_unit_length().read()).to_f32();
        (this as *mut u8).write(1);
        stack.add(5).write(reciprocal.to_bits());
        let x = multiply_stored(F80::from_f32(axis_x), reciprocal);
        let z = multiply_stored(F80::from_f32(axis_z), reciprocal);
        store(this, 0x1C, x.to_f32().to_bits());
        store(this, 0x20, z.to_f32().to_bits());
        r.eax = this;
    }
}

game_data! { static automaton_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static finished_animation_threshold: f32 = 0x00A8_C950; }
game_fn! {
    #[allow(dead_code)]
    fn random_unit_interval(this: *mut u8) -> St0 = "thiscall" 0x0091_3A29;
}
game_fn! {
    #[allow(dead_code)]
    fn refresh_automaton_clock() -> u32 = "C" 0x0050_7BF0;
}

register_port!(
    0x0050_96B0,
    fc4p_005096b0,
    update_location_behavior,
    0,
    "UpdateLocationBehavior",
    |r, _stack| {
        unsafe {
            let this = r.ecx;
            r.eax = load(this, 0);
            virtual_call(r, 0x7C, &[0x4000]);
            if r.eax as u8 != 0 {
                return;
            }
            r.ecx = automaton_city().read() as u32;
            r.edx = load(r.ecx, 0);
            virtual_call(r, 0xBC, &[]);
            r.edx = load(r.eax, 0);
            r.ecx = r.eax;
            virtual_call(
                r,
                0x58,
                &[
                    this.wrapping_add(4),
                    this.wrapping_add(0xA4),
                    this.wrapping_add(0x9C),
                    this.wrapping_add(0xA0),
                ],
            );
            let location = r.eax;
            if location != 0 {
                r.edx = load(location, 0);
                r.ecx = location;
                virtual_call(r, 4, &[]);
            }
            r.eax = load(this, 0x98);
            if location != r.eax {
                if r.eax != 0 {
                    r.eax = load(this, 0);
                    r.ecx = this;
                    virtual_call(r, 0xE0, &[]);
                    while r.eax != 0 {
                        r.ecx = load(r.eax, 8);
                        if r.ecx != load(this, 0x98) {
                            break;
                        }
                        r.edx = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0xD4, &[]);
                        r.eax = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0xE0, &[]);
                    }
                }
                let old = load(this, 0x98);
                if location != old {
                    if location != 0 {
                        r.edx = load(location, 0);
                        r.ecx = location;
                        virtual_call(r, 4, &[]);
                    }
                    store(this, 0x98, location);
                    if old != 0 {
                        r.eax = load(old, 0);
                        r.ecx = old;
                        virtual_call(r, 8, &[]);
                    }
                }
            }
            r.ecx = load(this, 0x98);
            if r.ecx != 0 {
                r.edx = load(r.ecx, 0);
                r.eax = load(this, 0x9C);
                virtual_call(r, 0x80, &[r.eax, this.wrapping_add(0x40)]);
                let behavior = r.eax;
                if behavior != 0 {
                    let probability = f32::from_bits(load(behavior, 8));
                    r.ecx = automaton_random() as u32;
                    let draw = invoke_st0(r, crate::abi::target(0x0091_3A29), &[]);
                    let status = F80::from_f32(probability).compare(draw).status_word();
                    r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
                    if status & 0x0100 == 0 {
                        r.edx = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0xE0, &[]);
                        let mut action = r.eax;
                        let mut already_active = false;
                        while action != 0 {
                            r.eax = load(action, 8);
                            if r.eax != 0 {
                                let current_location = load(this, 0x98);
                                r.ecx = r.eax;
                                r.eax = load(r.ecx, 0);
                                virtual_call(r, 0x18, &[]);
                                let action_location_id = r.eax;
                                r.edx = load(current_location, 0);
                                r.ecx = current_location;
                                virtual_call(r, 0x18, &[]);
                                r.ecx = action_location_id;
                                if r.ecx == r.eax {
                                    r.ecx = behavior;
                                    r.eax = load(action, 4);
                                    if r.eax != load(behavior, 0) {
                                        r.edx = load(this, 0);
                                        r.ecx = this;
                                        virtual_call(r, 0xD4, &[]);
                                        r.eax = load(this, 0);
                                        r.ecx = this;
                                        virtual_call(r, 0xE0, &[]);
                                        action = r.eax;
                                        continue;
                                    }
                                }
                            }
                            r.ecx = load(action, 0x24);
                            already_active = r.ecx == behavior;
                            break;
                        }
                        if !already_active {
                            r.eax = load(this, 0x98);
                            r.ecx = behavior;
                            r.edx = load(this, 0);
                            let current_location = r.eax;
                            r.ecx = this;
                            virtual_call(r, 0xDC, &[behavior, current_location]);
                        }
                    }
                }
            }
            if location != 0 {
                r.edx = load(location, 0);
                r.ecx = location;
                virtual_call(r, 8, &[]);
            }
        }
    }
);

register_port!(
    0x0050_A7B0,
    fc4p_0050a7b0,
    tick_managed_automata,
    8,
    "TickManagedAutomata",
    |r, stack| {
        unsafe {
            let this = r.ecx;
            let mut scratch = this;
            invoke(r, crate::abi::target(0x0050_7BF0), &[], Conv::Cdecl);
            r.eax = load(this, 0x48);
            if r.eax == 0 {
                r.ecx = scratch;
                return;
            }
            let paused = (this.wrapping_add(0x4D) as *const u8).read();
            r.eax = (r.eax & !0xFF) | u32::from(paused);
            if paused != 0 {
                r.ecx = scratch;
                return;
            }
            r.ecx = selected_object_service().read() as u32;
            r.edx = &mut scratch as *mut u32 as u32;
            scratch = 0;
            r.eax = load(r.ecx, 0);
            virtual_call(r, 0x48, &[r.edx]);
            r.eax = load(this, 0x44);
            let mut node = load(r.eax, 0);
            while node != load(this, 0x44) {
                let automaton = load(node, 8);
                r.eax = load(automaton, 4);
                node = load(node, 0);
                r.ecx = automaton.wrapping_add(4);
                virtual_call(r, 0x84, &[]);
                let mut tick = r.eax as u8 & 4 != 0;
                if !tick {
                    r.edx = load(automaton, 0);
                    r.ecx = automaton;
                    virtual_call(r, 0x154, &[]);
                    if r.eax as u8 != 0 {
                        r.eax = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0x20, &[automaton, 1]);
                    } else {
                        let stopped = automata_stopped().read();
                        r.eax = (r.eax & !0xFF) | u32::from(stopped);
                        if stopped == 0 {
                            r.edx = load(automaton, 0);
                            r.ecx = automaton;
                            virtual_call(r, 0x7C, &[0x21_0040]);
                            tick = r.eax as u8 != 0;
                        }
                    }
                }
                if tick {
                    r.eax = load(automaton, 0);
                    r.ecx = automaton;
                    virtual_call(r, 0x7C, &[0x0400_0000]);
                    if r.eax as u8 == 0 {
                        r.edx = load(automaton, 0);
                        r.ecx = automaton;
                        virtual_call(r, 0x78, &[0x0400_0000, 1]);
                        r.eax = load(automaton, 0);
                        r.ecx = automaton;
                        virtual_call(r, 0x194, &[]);
                        if r.eax as u8 == 0 {
                            r.edx = load(this, 0);
                            r.ecx = this;
                            virtual_call(r, 0x20, &[automaton, 0]);
                        }
                    }
                    r.eax = load(automaton, 4);
                    r.ecx = automaton.wrapping_add(4);
                    virtual_call(r, 0x14, &[]);
                    if r.eax as u8 != 0 {
                        r.eax = arg(stack, 2);
                        r.ecx = arg(stack, 1);
                        r.edx = load(automaton, 0);
                        r.ecx = automaton;
                        virtual_call(r, 0x120, &[arg(stack, 1), r.eax]);
                        if r.eax as u8 == 0 {
                            r.edx = load(this, 0);
                            r.ecx = this;
                            virtual_call(r, 0x20, &[automaton, 0]);
                        }
                    }
                    r.eax = load(automaton, 0);
                    r.ecx = automaton;
                    virtual_call(r, 0x154, &[]);
                    let mut remove_finished = false;
                    if r.eax as u8 != 0 {
                        r.edx = load(automaton, 0);
                        r.ecx = automaton;
                        let value = invoke_st0(r, crate::abi::vslot(automaton as *mut u8, 0x80), &[]);
                        let status = compare_stored(value, finished_animation_threshold().read());
                        r.eax = (r.eax & 0xFFFF_0000) | u32::from(status);
                        remove_finished = status & 0x0500 == 0x0100 || status & 0x0500 == 0x0400;
                    }
                    if remove_finished {
                        r.eax = load(this, 0);
                        r.ecx = this;
                        virtual_call(r, 0x20, &[automaton, 1]);
                    } else {
                        r.edx = load(automaton, 0);
                        r.ecx = automaton;
                        virtual_call(r, 0x7C, &[0x1_0000]);
                        if r.eax as u8 == 0 {
                            r.eax = load(this, 0);
                            r.ecx = this;
                            virtual_call(r, 0xA4, &[automaton]);
                            if r.eax as u8 == 0 {
                                r.edx = load(this, 0);
                                r.ecx = this;
                                virtual_call(r, 0x20, &[automaton, 0]);
                            }
                        }
                    }
                }
            }
            r.ecx = scratch;
            if r.ecx != 0 {
                r.eax = load(r.ecx, 0);
                virtual_call(r, 8, &[]);
            }
            r.ecx = scratch;
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
    fc4p_004f2910::PORT,
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
    fc4p_00505780::PORT,
    fc4p_00505820::PORT,
    fc4p_00505c70::PORT,
    fc4p_00506450::PORT,
    fc4p_00506940::PORT,
    fc4p_00507bf0::PORT,
    fc4p_00507e60::PORT,
    fc4p_00507f70::PORT,
    fc4p_005080c0::PORT,
    fc4p_00508610::PORT,
    fc4p_005089f0::PORT,
    fc4p_00509440::PORT,
    fc4p_00509600::PORT,
    fc4p_00509660::PORT,
    fc4p_005096b0::PORT,
    fc4p_0050a310::PORT,
    fc4p_0050a670::PORT,
    fc4p_0050a6b0::PORT,
    fc4p_0050a740::PORT,
    fc4p_0050a7b0::PORT,
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
    stub!(clock_mode_stub, 0, 0, 29);
    stub!(simulation_paused_stub, 0, 0, 30);
    stub!(simulation_speed_stub, 0, 0, 31);
    stub!(preferences_stub, 0, 0, 32);
    stub!(simulation_gate_stub, 0, 0, 33);
    stub!(integer_range_stub, 8, 2, 34);
    stub!(speed_stub, 8, 2, 35);
    stub!(model_handle_stub, 0, 0, 36);
    stub!(model_remove_stub, 4, 1, 37);
    stub!(occupant_remove_stub, 12, 3, 38);
    stub!(occupant_add_stub, 8, 2, 39);
    stub!(flag_clear_stub, 4, 1, 40);
    stub!(flag_add_stub, 4, 1, 41);
    stub!(model_opacity_stub, 8, 2, 42);
    stub!(model_add_stub, 4, 1, 43);
    stub!(occupant_enable_stub, 4, 1, 44);
    stub!(retirement_gate_stub, 4, 1, 45);
    stub!(model_retire_stub, 4, 1, 46);
    stub!(model_fade_stub, 8, 2, 47);
    stub!(occupant_retire_stub, 8, 2, 48);
    stub!(retirement_state_stub, 8, 2, 49);
    stub!(model_disable_stub, 4, 1, 50);
    stub!(model_position_stub, 4, 1, 51);

    thread_local! {
        static FLOAT_RESULT: RefCell<u32> = const { RefCell::new(0) };
    }
    macro_rules! float_stub {
        ($name:ident, $pop:literal, $count:literal, $id:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
                core::arch::naked_asm!(
                    "sub esp, 4", "push edx", "push ecx", "push eax", "mov ecx, esp",
                    "lea edx, [esp + 16]", "push edx", "call {}",
                    "pop eax", "pop ecx", "pop edx", "fld dword ptr [esp]", "add esp, 4", "ret {}",
                    sym $name::body, const $pop,
                );
            }
            mod $name {
                use super::*;
                pub unsafe extern "thiscall" fn body(r: &mut Registers, stack: *mut u32) {
                    let call = Call { id: $id, registers: *r,
                        args: unsafe { std::slice::from_raw_parts(stack.add(1), $count) }.to_vec() };
                    HOOK.with(|h| { if let Some(hook) = *h.borrow() { hook(&call); } });
                    CALLS.with(|c| c.borrow_mut().push(call));
                    *r = RESULTS.with(|v| v.borrow_mut().pop_front()).unwrap_or(RESULT);
                    let value = FLOAT_RESULT.with(|v| *v.borrow());
                    unsafe { (r as *mut Registers).cast::<u32>().add(3).write(value); }
                }
            }
        };
    }
    float_stub!(float_range_stub, 16, 4, 52);
    float_stub!(opacity_stub, 0, 0, 53);
    stub!(song_lookup_stub, 4, 1, 54);
    stub!(song_variant_stub, 0, 0, 55);
    stub!(song_data_stub, 0, 0, 56);
    stub!(song_length_stub, 0, 0, 57);
    stub!(song_append_stub, 4, 1, 58);
    stub!(clock_refresh_stub, 0, 0, 59);
    stub!(location_service_stub, 0, 0, 60);
    stub!(location_lookup_stub, 16, 4, 61);
    stub!(location_add_ref_stub, 0, 0, 62);
    stub!(location_release_stub, 0, 0, 63);
    stub!(top_action_stub, 0, 0, 64);
    stub!(pop_action_stub, 0, 0, 65);
    stub!(location_behavior_stub, 8, 2, 66);
    stub!(location_id_stub, 0, 0, 67);
    stub!(push_behavior_stub, 8, 2, 68);
    float_stub!(unit_random_stub, 0, 0, 69);
    stub!(animation_flags_stub, 0, 0, 70);
    stub!(animation_finished_stub, 0, 0, 71);
    stub!(automaton_flag_stub, 4, 1, 72);
    stub!(automaton_set_flag_stub, 8, 2, 73);
    stub!(animation_initialize_stub, 0, 0, 74);
    stub!(animation_active_stub, 0, 0, 75);
    stub!(animation_tick_stub, 8, 2, 76);
    float_stub!(animation_progress_stub, 0, 0, 77);
    stub!(manager_remove_stub, 8, 2, 78);
    stub!(manager_post_tick_stub, 4, 1, 79);
    stub!(selected_filter_stub, 4, 1, 80);
    stub!(footprint_model_stub, 0, 0, 81);
    stub!(footprint_query_stub, 8, 2, 82);
    stub!(footprint_bounds_stub, 4, 1, 83);
    stub!(footprint_transform_stub, 4, 1, 84);
    stub!(footprint_axes_stub, 8, 2, 85);

    thread_local! {
        static FOOTPRINT_BOUNDS: RefCell<[f32; 6]> = const { RefCell::new([-4.0, -1.0, -8.0, 6.0, 3.0, 12.0]) };
        static FOOTPRINT_TRANSFORM: RefCell<[f32; 13]> = const { RefCell::new([
            1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 100.0, 5.0, 200.0, 1.0]) };
        static FOOTPRINT_AXES: RefCell<[f32; 2]> = const { RefCell::new([1.0, 0.0]) };
    }

    #[test]
    fn model_footprint_moves_core_bounds_margin_and_rotation_cases_and_keeps_failure_fields_and_stack_scratch() {
        let _fpu = GameFpu::new();
        let mut object = Object::new(0x30);
        let p = object.ptr() as u32;
        let mut source_table = [0usize; 83];
        source_table[0] = footprint_query_stub as *const () as usize;
        source_table[0x148 / 4] = footprint_model_stub as *const () as usize;
        let mut source = [source_table.as_ptr() as u32];
        let source_p = source.as_mut_ptr() as u32;
        let mut geometry_table = [0usize; 10];
        geometry_table[0x1C / 4] = footprint_bounds_stub as *const () as usize;
        geometry_table[0x24 / 4] = footprint_transform_stub as *const () as usize;
        let mut vbtable = [0u32, (-4i32) as u32];
        let mut geometry = [geometry_table.as_ptr() as u32, vbtable.as_mut_ptr() as u32];
        let geometry_p = geometry.as_mut_ptr() as u32;
        let mut resource_table = [0usize; 3];
        resource_table[2] = location_release_stub as *const () as usize;
        let mut resource = [resource_table.as_ptr() as u32];
        SONG_RESOURCE.with(|v| *v.borrow_mut() = resource.as_mut_ptr() as u32);
        let mut half = 0.5f32;
        let mut one = 1.0f32;
        let mut wide = 2.0f32;
        let _data = [
            fake_data(0x00A8_825C, (&mut wide as *mut f32).cast()),
            fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast()),
            fake_data(0x00A8_1228, (&mut one as *mut f32).cast()),
            fake_data(0x00B0_9978, (&mut one as *mut f32).cast()),
        ];
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                match c.id {
                    82 => {
                        assert_eq!(c.args[0], 0x4A41_6254);
                        assert_eq!(load(c.args[1], 0), 0);
                        store(c.args[1], 0, SONG_RESOURCE.with(|v| *v.borrow()));
                    }
                    83 => {
                        assert_eq!(
                            std::slice::from_raw_parts(c.args[0] as *const f32, 6),
                            &[1.0, 1.0, 1.0, 0.0, 0.0, 0.0]
                        );
                        FOOTPRINT_BOUNDS.with(|v| {
                            for (i, &f) in v.borrow().iter().enumerate() {
                                store(c.args[0], i as u32 * 4, f.to_bits());
                            }
                        });
                    }
                    84 => {
                        assert_eq!(load(c.args[0], 0) & 0xFFFF, 0);
                        assert_eq!(
                            std::slice::from_raw_parts((c.args[0] + 4) as *const f32, 13),
                            &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
                        );
                        FOOTPRINT_TRANSFORM.with(|v| {
                            for (i, &f) in v.borrow().iter().enumerate() {
                                store(c.args[0], 4 + i as u32 * 4, f.to_bits());
                            }
                        });
                    }
                    _ => {}
                }
            })
        });
        for tagged in [false, true] {
            unsafe {
                core::ptr::write_bytes(p as *mut u8, 0xCC, 0x30);
            }
            responses(&[geometry_p, 0, u32::from(tagged), 0, RESULT.eax]);
            let mut original_stack = [0xA5A5_A5A5u32; 32];
            let stack = unsafe { original_stack.as_mut_ptr().add(24) };
            for (i, value) in [
                source_p,
                3.0f32.to_bits(),
                99.0f32.to_bits(),
                (-4.0f32).to_bits(),
                2.0f32.to_bits(),
            ]
            .into_iter()
            .enumerate()
            {
                unsafe {
                    stack.add(i + 1).write(value);
                }
            }
            let mut r = Registers { ecx: p, ..ENTRY };
            unsafe {
                build_model_footprint(&mut r, stack);
            }
            assert_eq!(r, Registers { eax: p, ..RESULT });
            let calls = take_calls();
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), [81, 83, 82, 63, 84]);
            assert_eq!(
                unsafe { (load(p, 4), load(p, 8)) },
                (103.0f32.to_bits(), 196.0f32.to_bits())
            );
            let extents = if tagged { [8.0f32, 19.0f32] } else { [9.5f32, 19.5f32] };
            assert_eq!(
                unsafe { (load(p, 12), load(p, 16)) },
                (extents[0].to_bits(), extents[1].to_bits())
            );
            assert_eq!(unsafe { load(p, 0) }, 0xCCCC_CC01);
            assert_eq!(
                unsafe { (load(p, 0x14), load(p, 0x18), load(p, 0x1C), load(p, 0x20)) },
                (0, 0, 1.0f32.to_bits(), (-0.0f32).to_bits())
            );
            assert_eq!(unsafe { stack.add(1).read() }, resource.as_ptr() as u32);
            assert_eq!(unsafe { stack.add(3).read() }, (-0.0f32).to_bits());
            assert_eq!(unsafe { stack.add(5).read() }, 1.0f32.to_bits());
            assert_eq!(unsafe { stack.sub(14).read() }, 0xA5A5_0000);
        }
        FOOTPRINT_TRANSFORM.with(|v| {
            let mut v = v.borrow_mut();
            v[0] = 0.0;
            v[2] = 2.0;
        });
        responses(&[geometry_p, 0, 0, 0, RESULT.eax]);
        run(0x0050_A310, p, &[source_p, 0, 0, 0, 1.0f32.to_bits()], false);
        assert_eq!(
            unsafe { (load(p, 0x1C), load(p, 0x20)) },
            (0.0f32.to_bits(), (-1.0f32).to_bits())
        );
        take_calls();
        let before = bytes(p, 0x30);
        responses(&[0]);
        run(0x0050_A310, p, &[source_p, 1, 2, 3, 4], false);
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [81]);
        let after = bytes(p, 0x30);
        assert_eq!(&after[..4], &before[..4]);
        assert_eq!(&after[4..0x1C], &[0; 24]);
        assert_eq!(&after[0x1C..], &before[0x1C..]);
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn footprint_overlap_moves_core_touch_and_invalid_cases_and_rejects_each_separating_axis_in_order() {
        let _fpu = GameFpu::new();
        let mut first = [0u32; 9];
        first[0] = 1;
        first[3] = 1.0f32.to_bits();
        first[4] = 1.0f32.to_bits();
        first[7] = 1.0f32.to_bits();
        let p = first.as_mut_ptr() as u32;
        let mut second = first;
        let q = second.as_mut_ptr() as u32;
        let mut zero = 0.0f32;
        let _data = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let _axes = fake(0x0050_5240, footprint_axes_stub as *const () as usize);
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                if c.id == 85 {
                    assert_eq!(
                        std::slice::from_raw_parts(c.registers.ecx as *const f32, 2),
                        &[1.0, -0.0]
                    );
                    FOOTPRINT_AXES.with(|v| {
                        for (i, &f) in v.borrow().iter().enumerate() {
                            store(c.args[0], i as u32 * 4, f.to_bits());
                        }
                    });
                }
            })
        });
        FOOTPRINT_AXES.with(|v| *v.borrow_mut() = [1.0, 0.0]);
        for (x, z, overlap) in [
            (2.0f32, 0.0f32, true),
            (2.0001, 0.0, false),
            (0.0, 2.0001, false),
            (0.0, 0.0, true),
            (f32::NAN, 0.0, true),
        ] {
            second[1] = x.to_bits();
            second[2] = z.to_bits();
            assert_eq!(second[1], x.to_bits());
            responses(&[RESULT.eax]);
            let mut scratch = [0u32; 10];
            let stack = unsafe { scratch.as_mut_ptr().add(7) };
            unsafe {
                stack.add(1).write(q);
            }
            let mut r = Registers { ecx: p, ..ENTRY };
            unsafe {
                footprints_overlap(&mut r, stack);
            }
            assert_eq!(r.eax as u8 != 0, overlap);
            assert_eq!((r.ecx, r.edx), (RESULT.ecx, RESULT.edx));
            assert_eq!(unsafe { stack.add(1).read() }, (-0.0f32).to_bits());
            let calls = take_calls();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].args[1], q + 0x1C);
        }
        first[3] = 2.0f32.to_bits();
        first[4] = 0.2f32.to_bits();
        let diagonal = core::f32::consts::FRAC_1_SQRT_2;
        FOOTPRINT_AXES.with(|v| *v.borrow_mut() = [diagonal, diagonal]);
        for (bh, bk, z, status) in [(0.2f32, 2.0f32, 1.0f32, 0u32), (2.0, 0.2, -1.0, 0)] {
            second[1] = 3.0f32.to_bits();
            second[2] = z.to_bits();
            second[3] = bh.to_bits();
            second[4] = bk.to_bits();
            assert_eq!(second[4], bk.to_bits());
            responses(&[RESULT.eax]);
            assert_eq!(run(0x0050_6450, p, &[q], false).eax & 0xFFFF, status);
            assert_eq!(take_calls().len(), 1);
        }
        first[0] = 0;
        assert_eq!(first[0], 0);
        responses(&[]);
        assert_eq!(run(0x0050_6450, p, &[0], false).eax, ENTRY.eax & !0xFF);
        first[0] = 1;
        second[0] = 0;
        assert_eq!((first[0], second[0]), (1, 0));
        assert_eq!(run(0x0050_6450, p, &[q], false).eax, ENTRY.eax & !0xFF);
        assert!(take_calls().is_empty());
        HOOK.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn location_behavior_balances_references_pops_old_actions_and_obeys_probability_and_identity_gates() {
        let _fpu = GameFpu::new();
        let mut table = [0usize; 57];
        table[0x7C / 4] = automaton_flag_stub as *const () as usize;
        table[0xE0 / 4] = top_action_stub as *const () as usize;
        table[0xD4 / 4] = pop_action_stub as *const () as usize;
        table[0xDC / 4] = push_behavior_stub as *const () as usize;
        let mut object = Object::new(0xB0);
        let p = object.ptr() as u32;
        let mut location_table = [0usize; 33];
        location_table[1] = location_add_ref_stub as *const () as usize;
        location_table[2] = location_release_stub as *const () as usize;
        location_table[6] = location_id_stub as *const () as usize;
        location_table[32] = location_behavior_stub as *const () as usize;
        let mut old = [location_table.as_ptr() as u32];
        let old_p = old.as_mut_ptr() as u32;
        let mut new = [location_table.as_ptr() as u32];
        let new_p = new.as_mut_ptr() as u32;
        let mut city_table = [0usize; 48];
        city_table[47] = location_service_stub as *const () as usize;
        let mut city = [city_table.as_ptr() as u32];
        let mut city_global = city.as_mut_ptr() as u32;
        let mut service_table = [0usize; 23];
        service_table[22] = location_lookup_stub as *const () as usize;
        let mut service = [service_table.as_ptr() as u32];
        let service_p = service.as_mut_ptr() as u32;
        let mut random = [0u32; 4];
        let _data = [
            fake_data(0x00B4_3CEC, (&mut city_global as *mut u32).cast()),
            fake_data(0x00B2_1D78, random.as_mut_ptr().cast()),
        ];
        let _fake = fake(0x0091_3A29, unit_random_stub as *const () as usize);
        unsafe {
            store(p, 0, table.as_ptr() as u32);
        }
        responses(&[0xAABB_CC01]);
        assert_eq!(run(0x0050_96B0, p, &[], false).eax, 0xAABB_CC01);
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [72]);
        let mut action = [0u32; 10];
        action[2] = old_p;
        let action_p = action.as_mut_ptr() as u32;
        unsafe {
            store(p, 0x98, old_p);
        }
        responses(&[0, service_p, new_p, 1, action_p, 1, 0, 1, 1, 0, 0xAABB_CC80]);
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                if c.id == 61 {
                    store(c.args[1], 0, 0xAA);
                    store(c.args[2], 0, 0xBB);
                    store(c.args[3], 0, 0xCC);
                }
            })
        });
        assert_eq!(run(0x0050_96B0, p, &[], false).eax, 0xAABB_CC80);
        HOOK.with(|h| *h.borrow_mut() = None);
        let calls = take_calls();
        assert_eq!(
            calls.iter().map(|c| c.id).collect::<Vec<_>>(),
            [72, 60, 61, 62, 64, 65, 64, 62, 63, 66, 63]
        );
        assert_eq!(calls[2].args, [p + 4, p + 0xA4, p + 0x9C, p + 0xA0]);
        assert_eq!(calls[9].args, [0xBB, p + 0x40]);
        assert_eq!(
            unsafe { (load(p, 0x98), load(p, 0x9C), load(p, 0xA0), load(p, 0xA4)) },
            (new_p, 0xBB, 0xCC, 0xAA)
        );
        let mut behavior = [0x44u32, 0, 0.5f32.to_bits()];
        let behavior_p = behavior.as_mut_ptr() as u32;
        for probability in [0.0f32, 0.5, 1.0, f32::NAN] {
            behavior[2] = probability.to_bits();
            assert_eq!(behavior[2], probability.to_bits());
            FLOAT_RESULT.with(|v| *v.borrow_mut() = 0.5f32.to_bits());
            let admitted = probability >= 0.5;
            let mut values = vec![0, service_p, new_p, 1, behavior_p, 0];
            if admitted {
                values.extend([0, 1]);
            }
            values.push(0x1234_5680);
            responses(&values);
            assert_eq!(run(0x0050_96B0, p, &[], false).eax, 0x1234_5680);
            let calls = take_calls();
            assert_eq!(
                calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                if admitted {
                    vec![72, 60, 61, 62, 66, 69, 64, 68, 63]
                } else {
                    vec![72, 60, 61, 62, 66, 69, 63]
                }
            );
            if admitted {
                assert_eq!(calls[7].args, [behavior_p, new_p]);
            }
        }
        action[2] = old_p;
        action[1] = 0x45;
        action[9] = 0;
        behavior[2] = 0.5f32.to_bits();
        assert_eq!(behavior[2], 0.5f32.to_bits());
        assert_eq!(action[1], 0x45);
        responses(&[
            0,
            service_p,
            new_p,
            1,
            behavior_p,
            0,
            action_p,
            7,
            7,
            1,
            0,
            1,
            0x9876_5480,
        ]);
        run(0x0050_96B0, p, &[], false);
        assert_eq!(
            take_calls().iter().map(|c| c.id).collect::<Vec<_>>(),
            [72, 60, 61, 62, 66, 69, 64, 67, 67, 65, 64, 68, 63]
        );
        action[2] = 0;
        action[9] = behavior_p;
        assert_eq!(action[9], behavior_p);
        responses(&[0, service_p, new_p, 1, behavior_p, 0, action_p, 1]);
        run(0x0050_96B0, p, &[], false);
        assert_eq!(
            take_calls().iter().map(|c| c.id).collect::<Vec<_>>(),
            [72, 60, 61, 62, 66, 69, 64, 63]
        );
        // Losing a location releases the old owner and never queries a behavior.
        responses(&[0, service_p, 0, 0, 1]);
        run(0x0050_96B0, p, &[], false);
        assert_eq!(unsafe { load(p, 0x98) }, 0);
        assert_eq!(
            take_calls().iter().map(|c| c.id).collect::<Vec<_>>(),
            [72, 60, 61, 64, 63]
        );
    }

    #[test]
    fn manager_tick_routes_inactive_finished_frozen_and_active_nodes_and_releases_selected_filter() {
        let _fpu = GameFpu::new();
        let mut table = [0usize; 42];
        table[0x20 / 4] = manager_remove_stub as *const () as usize;
        table[0xA4 / 4] = manager_post_tick_stub as *const () as usize;
        let mut object = Object::new(0x60);
        let p = object.ptr() as u32;
        let mut automaton_table = [0usize; 102];
        automaton_table[0x154 / 4] = animation_finished_stub as *const () as usize;
        automaton_table[0x7C / 4] = automaton_flag_stub as *const () as usize;
        automaton_table[0x78 / 4] = automaton_set_flag_stub as *const () as usize;
        automaton_table[0x194 / 4] = animation_initialize_stub as *const () as usize;
        automaton_table[0x120 / 4] = animation_tick_stub as *const () as usize;
        automaton_table[0x80 / 4] = animation_progress_stub as *const () as usize;
        let mut interface_table = [0usize; 34];
        interface_table[0x84 / 4] = animation_flags_stub as *const () as usize;
        interface_table[0x14 / 4] = animation_active_stub as *const () as usize;
        let mut automaton = [automaton_table.as_ptr() as u32, interface_table.as_ptr() as u32];
        let a = automaton.as_mut_ptr() as u32;
        let mut sentinel = [0u32; 3];
        let sentinel_p = sentinel.as_mut_ptr() as u32;
        let mut node = [sentinel_p, 0, a];
        sentinel[0] = node.as_mut_ptr() as u32;
        assert_eq!(sentinel[0], node.as_ptr() as u32);
        let mut filter_table = [0usize; 19];
        filter_table[0x48 / 4] = selected_filter_stub as *const () as usize;
        let mut filter_service = [filter_table.as_ptr() as u32];
        let mut filter_global = filter_service.as_mut_ptr() as u32;
        let mut retained_table = [0usize; 3];
        retained_table[2] = location_release_stub as *const () as usize;
        let mut retained = [retained_table.as_ptr() as u32];
        let mut stopped = 0u8;
        let mut threshold = 0.1f32;
        let _data = [
            fake_data(0x00B4_3DC4, (&mut filter_global as *mut u32).cast()),
            fake_data(0x00B2_1B9C, &mut stopped),
            fake_data(0x00A8_C950, (&mut threshold as *mut f32).cast()),
        ];
        let _clock = fake(0x0050_7BF0, clock_refresh_stub as *const () as usize);
        SONG_RESOURCE.with(|v| *v.borrow_mut() = retained.as_mut_ptr() as u32);
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                if c.id == 80 {
                    store(c.args[0], 0, SONG_RESOURCE.with(|v| *v.borrow()));
                }
            })
        });
        unsafe {
            store(p, 0, table.as_ptr() as u32);
            store(p, 0x44, sentinel_p);
            store(p, 0x48, 0);
        }
        responses(&[1]);
        assert_eq!(
            run(0x0050_A7B0, p, &[0xAB, 0xCD], false),
            Registers {
                eax: 0,
                ecx: p,
                ..RESULT
            }
        );
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [59]);
        unsafe {
            store(p, 0x48, 1);
            (p.wrapping_add(0x4D) as *mut u8).write(1);
        }
        responses(&[1]);
        assert_eq!(run(0x0050_A7B0, p, &[0xAB, 0xCD], false).eax, 1);
        assert_eq!(take_calls().len(), 1);
        unsafe {
            (p.wrapping_add(0x4D) as *mut u8).write(0);
        }
        for (values, expected_ids) in [
            (vec![1, 1, 0, 1, 1, 0xAABB_CC80], vec![59, 80, 70, 71, 78, 63]),
            (vec![1, 1, 0, 0, 0, 0xAABB_CC80], vec![59, 80, 70, 71, 72, 63]),
            (
                vec![1, 1, 4, 1, 0, 0, 1, 0xAABB_CC80],
                vec![59, 80, 70, 72, 75, 71, 72, 63],
            ),
            (
                vec![1, 1, 4, 1, 1, 0, 1, 0, 1, 0xAABB_CC80],
                vec![59, 80, 70, 72, 75, 76, 78, 71, 72, 63],
            ),
            (
                vec![1, 1, 4, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0xAABB_CC80],
                vec![59, 80, 70, 72, 73, 74, 78, 75, 71, 72, 79, 78, 63],
            ),
        ] {
            responses(&values);
            let result = run(0x0050_A7B0, p, &[0xAB, 0xCD], false);
            assert_eq!(result.ecx, retained.as_ptr() as u32);
            let calls = take_calls();
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), expected_ids);
            if let Some(c) = calls.iter().find(|c| c.id == 76) {
                assert_eq!(c.args, [0xAB, 0xCD]);
                assert_eq!(c.registers.ecx, a);
            }
        }
        stopped = 1;
        assert_eq!(stopped, 1);
        responses(&[1, 1, 0, 0, 0xAABB_CC80]);
        run(0x0050_A7B0, p, &[0, 0], false);
        assert_eq!(
            take_calls().iter().map(|c| c.id).collect::<Vec<_>>(),
            [59, 80, 70, 71, 63]
        );
        for progress in [0.0f32, 0.1, 0.2, f32::NAN] {
            FLOAT_RESULT.with(|v| *v.borrow_mut() = progress.to_bits());
            let mut values = vec![1, 1, 4, 1, 0, 1, 0];
            if progress < 0.1 {
                values.push(1);
            } else {
                values.extend([0, 0, 1]);
            }
            values.push(RESULT.eax);
            responses(&values);
            run(0x0050_A7B0, p, &[0, 0], false);
            let calls = take_calls();
            assert_eq!(
                calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                if progress < 0.1 {
                    vec![59, 80, 70, 72, 75, 71, 77, 78, 63]
                } else {
                    vec![59, 80, 70, 72, 75, 71, 77, 72, 79, 78, 63]
                }
            );
        }
        HOOK.with(|h| *h.borrow_mut() = None);
        RESULTS.with(|r| r.borrow_mut().clear());
    }

    thread_local! {
        static SONG_RESOURCE: RefCell<u32> = const { RefCell::new(0) };
        static SONG_RECORDS: RefCell<Vec<[u32; 4]>> = const { RefCell::new(Vec::new()) };
    }

    #[test]
    fn song_table_carries_enable_bytes_skips_short_and_missing_properties_and_retains_record_padding() {
        // Song discovery and inherited enable flags follow the audio preferences worked case.
        let mut tables = [0xA5u8; 0x60];
        let table_p = tables.as_mut_ptr() as u32;
        let mut bases = [0x2A5B_A8A7u32, 0x3A5B_A8A7];
        let mut resource_table = [0usize; 8];
        resource_table[0x1C / 4] = method_zero as *const () as usize;
        resource_table[8 / 4] = filter_release_stub as *const () as usize;
        let mut resource = [resource_table.as_ptr() as u32];
        let resource_p = resource.as_mut_ptr() as u32;
        let mut manager_table = [0usize; 4];
        manager_table[3] = method_five as *const () as usize;
        let mut manager = [manager_table.as_ptr() as u32];
        let mut manager_global = manager.as_mut_ptr() as u32;
        let mut holder_table = [0usize; 10];
        holder_table[9] = song_lookup_stub as *const () as usize;
        let mut holder = [holder_table.as_ptr() as u32];
        let holder_p = holder.as_mut_ptr() as u32;
        let mut property_table = [0usize; 6];
        property_table[5] = song_variant_stub as *const () as usize;
        let mut property = [property_table.as_ptr() as u32];
        let property_p = property.as_mut_ptr() as u32;
        let mut variant_table = [0usize; 104];
        variant_table[0x19C / 4] = song_data_stub as *const () as usize;
        variant_table[0x10 / 4] = song_length_stub as *const () as usize;
        let mut variant = [variant_table.as_ptr() as u32];
        let variant_p = variant.as_mut_ptr() as u32;
        let mut values = [[10u32, 11, 0], [20, 21, 99], [30, 31, 99], [40, 41, 1], [50, 51, 99]];
        let _data = [
            fake_data(0x00B2_1A88, tables.as_mut_ptr()),
            fake_data(0x00B0_987C, bases.as_mut_ptr().cast()),
            fake_data(0x00B4_3CA8, (&mut manager_global as *mut u32).cast()),
        ];
        let _fake = fake(0x004B_2400, song_append_stub as *const () as usize);
        SONG_RESOURCE.with(|v| *v.borrow_mut() = resource_p);
        HOOK.with(|h| {
            *h.borrow_mut() = Some(|c| unsafe {
                if c.id == 14 {
                    assert_eq!(c.args[1], 0xA521_60F5);
                    assert_eq!(&c.args[3..], &[0, 0]);
                    assert_eq!(load(c.args[0], 0), 0x6534_284A);
                    assert_eq!(load(c.args[0], 4), 0xCA25_875D);
                    store(c.args[2], 0, SONG_RESOURCE.with(|v| *v.borrow()));
                } else if c.id == 58 {
                    let p = c.args[0];
                    SONG_RECORDS.with(|v| v.borrow_mut().push([load(p, 0), load(p, 4), load(p, 8), load(p, 12)]));
                }
            })
        });
        for bank in 0..2 {
            let mut results = vec![1, holder_p];
            for (i, count) in [3u32, 2, 1, 3, 2].into_iter().enumerate() {
                results.extend([property_p, variant_p, values[i].as_mut_ptr() as u32, count]);
                if count >= 2 {
                    results.push(RESULT.eax);
                }
            }
            results.extend([0; 25]);
            results.push(RESULT.eax);
            responses(&results);
            SONG_RECORDS.with(|v| v.borrow_mut().clear());
            let mut original_stack = [0xA5A5_A5A5u32; 12];
            let stack = unsafe { original_stack.as_mut_ptr().add(9) };
            unsafe {
                stack.add(1).write(bank);
            }
            let mut r = Registers {
                ecx: 0x8765_4321,
                ..ENTRY
            };
            unsafe {
                load_audio_song_table(&mut r, stack);
            }
            assert_eq!(unsafe { stack.add(1).read() }, 0);
            assert_eq!(r, Registers { ecx: 0, ..RESULT });
            let expected: Vec<_> = [(0, 11, 10, 0), (1, 21, 20, 0), (3, 41, 40, 1), (4, 51, 50, 1)]
                .into_iter()
                .map(|(i, file, title, enabled)| [bases[bank as usize] + i, file, title, 0xA5A5_A500 | enabled])
                .collect();
            assert_eq!(SONG_RECORDS.with(|v| v.borrow().clone()), expected);
            let calls = take_calls();
            let lookups: Vec<_> = calls.iter().filter(|c| c.id == 54).map(|c| c.args[0]).collect();
            assert_eq!(lookups, (0..30).map(|i| 0x8A5E_7B50 + i).collect::<Vec<_>>());
            assert!(calls
                .iter()
                .filter(|c| c.id == 58)
                .all(|c| c.registers.ecx == table_p + bank * 0x30 + 8));
            assert_eq!(calls.last().unwrap().id, 20);
        }
        responses(&[0, RESULT.eax]);
        run(0x004F_2910, 0x8765, &[0], true);
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [14, 20]);
        SONG_RESOURCE.with(|v| *v.borrow_mut() = 0);
        responses(&[0xAABB_CC00]);
        assert_eq!(run(0x004F_2910, 0x8765, &[0], true).eax, 0xAABB_CC00);
        assert_eq!(take_calls().len(), 1);
        HOOK.with(|h| *h.borrow_mut() = None);
        for index in [2u32, u32::MAX, 0x8000_0000] {
            assert_eq!(
                run(0x004F_2910, 0x8765, &[index], true),
                Registers {
                    eax: index,
                    ecx: 0x8765,
                    ..ENTRY
                }
            );
        }
        assert!(take_calls().is_empty());
        assert!(tables.iter().all(|&v| v == 0xA5));
    }

    fn responses(values: &[u32]) {
        RESULTS.with(|r| *r.borrow_mut() = values.iter().map(|&eax| Registers { eax, ..RESULT }).collect());
        take_calls();
    }

    #[test]
    fn time_scale_handles_missing_simulation_clock_override_pause_preferences_and_every_speed() {
        let mut sim_table = [0usize; 0x70 / 4];
        sim_table[0x44 / 4] = simulation_gate_stub as *const () as usize;
        sim_table[0x3C / 4] = simulation_paused_stub as *const () as usize;
        sim_table[0x6C / 4] = simulation_speed_stub as *const () as usize;
        let mut simulation = [sim_table.as_ptr() as u32];
        let sim_p = simulation.as_mut_ptr() as u32;
        let mut clock_table = [0usize; 14];
        clock_table[13] = clock_mode_stub as *const () as usize;
        let mut clock = [clock_table.as_ptr() as u32];
        let mut prefs_table = [0usize; 39];
        prefs_table[38] = preferences_stub as *const () as usize;
        let mut prefs = [prefs_table.as_ptr() as u32];
        let mut prefs_memory = Object::new(0xF20);
        let pref_p = prefs_memory.ptr() as u32;
        let mut sim_global = sim_p;
        let mut clock_global = clock.as_mut_ptr() as u32;
        let mut pref_global = prefs.as_mut_ptr() as u32;
        let mut scale = 0xDEAD_BEEFu32;
        let mut stopped = 0xCCu8;
        let mut scales = [0x3EAA_AAABu32, 0x3F00_0000, 0x3F80_0000, 0x4000_0000];
        let _data = [
            fake_data(0x00B4_3D5C, (&mut sim_global as *mut u32).cast()),
            fake_data(0x00B4_3D44, (&mut clock_global as *mut u32).cast()),
            fake_data(0x00B4_3C94, (&mut pref_global as *mut u32).cast()),
            fake_data(0x00B0_9964, (&mut scale as *mut u32).cast()),
            fake_data(0x00B2_1B9C, &mut stopped),
            fake_data(0x00B2_1D18, scales.as_mut_ptr().cast()),
        ];
        sim_global = 0;
        assert_eq!(sim_global, 0);
        responses(&[]);
        assert_eq!(
            run(0x0050_7BF0, 0x7654, &[], true),
            Registers {
                eax: ENTRY.eax & !0xFF,
                ecx: 0x7654,
                ..ENTRY
            }
        );
        assert_eq!((scale, stopped), (1.0f32.to_bits(), 0));
        assert!(take_calls().is_empty());
        sim_global = sim_p;
        assert_eq!(sim_global, sim_p);
        responses(&[0xAABB_CC01]);
        assert_eq!(
            run(0x0050_7BF0, 0, &[], true),
            Registers {
                eax: 0xAABB_CC00,
                ecx: scales[3],
                ..RESULT
            }
        );
        assert_eq!((scale, stopped), (scales[3], 0));
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [29]);
        for (gate, enabled, pause, speed, expected, ids) in [
            (1, 1, 0, 0, scales[0], vec![29, 33]),
            (0, 0, 0, 0, scales[0], vec![29, 33, 32]),
            (0, 1, 1, 0, 0, vec![29, 33, 32, 30]),
            (0, 1, 0, 0, scales[0], vec![29, 33, 32, 30, 31]),
            (0, 1, 0, 1, scales[1], vec![29, 33, 32, 30, 31]),
            (0, 1, 0, 2, scales[2], vec![29, 33, 32, 30, 31]),
            (0, 1, 0, u32::MAX, scales[1], vec![29, 33, 32, 30, 31]),
        ] {
            unsafe {
                (pref_p.wrapping_add(0xF18) as *mut u8).write(enabled);
            }
            responses(&[0, gate, pref_p, pause, speed]);
            run(0x0050_7BF0, 0, &[], true);
            assert_eq!((scale, stopped), (expected, u8::from(pause != 0)));
            assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), ids);
        }
        RESULTS.with(|r| r.borrow_mut().clear());
    }

    #[test]
    fn random_speed_uses_signed_ranges_double_endpoints_and_original_argument_scratch() {
        let _fpu = GameFpu::new();
        let mut table = [0usize; 37];
        table[36] = speed_stub as *const () as usize;
        let mut object = Object::new(0x90);
        let p = object.ptr() as u32;
        let mut tuning = [0u32; 17];
        let q = tuning.as_mut_ptr() as u32;
        let mut random = [0u32; 4];
        let mut scale = 0.001f32;
        let _data = [
            fake_data(0x00B2_1D78, random.as_mut_ptr().cast()),
            fake_data(0x00A9_52E8, (&mut scale as *mut f32).cast()),
        ];
        let _fakes = [
            fake(0x0091_3A14, integer_range_stub as *const () as usize),
            fake(0x0091_3A46, float_range_stub as *const () as usize),
        ];
        unsafe {
            store(p, 0, table.as_ptr() as u32);
            store(p, 0x80, 8.0f32.to_bits());
        }
        for (min, max, low, high, ids, expected, scratch) in [
            (0, 20, 2.0f32, 2.0f32, vec![35], 16.0f32, 0),
            (20, 0, 2.0, 2.0, vec![35], 16.0, 20),
            (u32::MAX, 20, 2.0, 2.0, vec![35], 16.0, u32::MAX),
            (1000, 1000, 2.0, 2.0, vec![35], 2.0, 1000),
            (500, 2000, 2.0, 2.0, vec![34, 35], 2.0, 1000),
            (0, 0, 1.0, 3.0, vec![52, 35], 12.0, 0),
            (0, 0, f32::NAN, 3.0, vec![52, 35], 12.0, 0),
        ] {
            tuning[12] = low.to_bits();
            tuning[13] = high.to_bits();
            tuning[14] = min;
            tuning[15] = max;
            tuning[16] = 0xDEAD_BEEF;
            FLOAT_RESULT.with(|v| *v.borrow_mut() = 1.5f32.to_bits());
            let values: Vec<u32> = ids.iter().map(|&id| if id == 34 { 1000 } else { RESULT.eax }).collect();
            responses(&values);
            let mut r = Registers { ecx: p, ..ENTRY };
            let mut stack = [0, q];
            unsafe {
                configure_random_speed(&mut r, stack.as_mut_ptr());
            }
            assert_eq!(stack[1], scratch);
            assert_eq!(
                r,
                Registers {
                    ecx: expected.to_bits(),
                    ..RESULT
                }
            );
            let calls = take_calls();
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
            assert_eq!(calls.last().unwrap().args, [expected.to_bits(), 0xDEAD_BEEF]);
            assert_eq!(calls.last().unwrap().registers.ecx, p);
            if let Some(draw) = calls.iter().find(|c| c.id == 52) {
                let lo = f64::from(low).to_bits();
                let hi = f64::from(high).to_bits();
                assert_eq!(draw.args, [lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32]);
                assert_eq!(draw.registers.ecx, random.as_ptr() as u32);
            }
        }
        responses(&[RESULT.eax]);
        tuning[12] = 2.0f32.to_bits();
        tuning[13] = tuning[12];
        assert_eq!(tuning[13], 2.0f32.to_bits());
        assert_eq!(run(0x0050_5780, p, &[q], false).ecx, 16.0f32.to_bits());
        take_calls();
    }

    #[test]
    fn occupant_registration_keeps_raw_boolean_bytes_and_releases_models_before_occupant() {
        let mut object = Object::new(0x1D0);
        let p = object.ptr() as u32;
        let mut interface_table = [0usize; 33];
        interface_table[0x74 / 4] = flag_add_stub as *const () as usize;
        interface_table[0x7C / 4] = flag_clear_stub as *const () as usize;
        let mut manager_table = [0usize; 22];
        manager_table[0x50 / 4] = occupant_add_stub as *const () as usize;
        manager_table[0x54 / 4] = occupant_remove_stub as *const () as usize;
        let mut manager = [manager_table.as_ptr() as u32];
        let mut manager_global = manager.as_mut_ptr() as u32;
        let mut model_table = [0usize; 30];
        model_table[0x74 / 4] = model_remove_stub as *const () as usize;
        let mut model_manager_object = [model_table.as_ptr() as u32];
        let mut model_global = model_manager_object.as_mut_ptr() as u32;
        let child_table = [model_handle_stub as *const () as usize];
        let mut child = [child_table.as_ptr() as u32];
        let c = child.as_mut_ptr() as u32;
        let mut children = [c, 0, c];
        let begin = children.as_mut_ptr() as u32;
        let _data = [
            fake_data(0x00B4_3D0C, (&mut manager_global as *mut u32).cast()),
            fake_data(0x00B4_3DD0, (&mut model_global as *mut u32).cast()),
        ];
        unsafe {
            store(p, 4, interface_table.as_ptr() as u32);
            store(p, 0x110, begin);
            store(p, 0x114, begin + 12);
        }
        for flags in [0u8, 1, 2, 3, 0xFF] {
            unsafe {
                (p.wrapping_add(0x33) as *mut u8).write(flags);
            }
            responses(&[]);
            assert_eq!(
                run(0x0050_7E60, p, &[u32::from(flags & 1) | 0xFFFF_0000], false).eax,
                (ENTRY.eax & !0xFF) | 1
            );
            assert!(take_calls().is_empty());
        }
        unsafe {
            (p.wrapping_add(0x33) as *mut u8).write(3);
        }
        responses(&[0x1234, 0x5678, 0x1235, 0x5679, 1, 2, 0xAABB_CC80]);
        assert_eq!(run(0x0050_7E60, p, &[0], false).eax, 0xAABB_CC01);
        let calls = take_calls();
        assert_eq!(
            calls.iter().map(|c| c.id).collect::<Vec<_>>(),
            [36, 37, 36, 37, 40, 38, 40]
        );
        assert_eq!(calls[1].args, [0x1234]);
        assert_eq!(calls[3].args, [0x1235]);
        assert_eq!(calls[4].args, [2]);
        assert_eq!(calls[5].args, [p + 4, 0, 0]);
        assert_eq!(calls[6].args, [1]);
        assert_eq!(calls[5].registers.ecx, manager_global);
        assert_eq!(calls[6].registers.ecx, p + 4);
        unsafe {
            (p.wrapping_add(0x33) as *mut u8).write(0);
            store(p, 0x114, begin);
        }
        for accepted in [0xAABB_CC00, 0xAABB_CC80] {
            responses(&[accepted, 0x1234_5680]);
            let result = run(0x0050_7E60, p, &[0xFF], false);
            assert_eq!(result.eax, if accepted as u8 == 0 { accepted } else { 0x1234_5601 });
            let calls = take_calls();
            assert_eq!(calls[0].args, [p + 4, 0]);
            assert_eq!(calls.len(), if accepted as u8 == 0 { 1 } else { 2 });
        }
        manager_global = 0;
        assert_eq!(manager_global, 0);
        responses(&[]);
        assert_eq!(run(0x0050_7E60, p, &[2], false).eax, ENTRY.eax & !0xFF);
        assert!(take_calls().is_empty());
    }

    #[test]
    fn model_registration_observes_occupant_flag_reloads_opacity_and_partial_add_failure() {
        let _fpu = GameFpu::new();
        let mut object = Object::new(0x1D0);
        let p = object.ptr() as u32;
        let mut main_table = [0usize; 72];
        main_table[0x118 / 4] = occupant_enable_stub as *const () as usize;
        let mut interface_table = [0usize; 32];
        interface_table[0x74 / 4] = flag_add_stub as *const () as usize;
        interface_table[0x7C / 4] = flag_clear_stub as *const () as usize;
        let mut manager_table = [0usize; 32];
        manager_table[0x70 / 4] = model_add_stub as *const () as usize;
        manager_table[0x74 / 4] = model_remove_stub as *const () as usize;
        let mut manager = [manager_table.as_ptr() as u32];
        let mut manager_global = manager.as_mut_ptr() as u32;
        let mut child_table = [0usize; 19];
        child_table[0] = model_handle_stub as *const () as usize;
        child_table[0x48 / 4] = model_opacity_stub as *const () as usize;
        let mut child = [child_table.as_ptr() as u32];
        let c = child.as_mut_ptr() as u32;
        let mut children = [c, 0, c];
        let begin = children.as_mut_ptr() as u32;
        let mut one = 1.0f32;
        let _data = [
            fake_data(0x00B4_3DD0, (&mut manager_global as *mut u32).cast()),
            fake_data(0x00A8_1228, (&mut one as *mut f32).cast()),
        ];
        let _fake = fake(0x004F_A960, opacity_stub as *const () as usize);
        unsafe {
            store(p, 0, main_table.as_ptr() as u32);
            store(p, 4, interface_table.as_ptr() as u32);
            store(p, 0x110, begin);
            store(p, 0x114, begin + 12);
        }
        for opacity in [0.0f32, 0.5, 1.0, f32::INFINITY, f32::NAN] {
            for success in [false, true] {
                unsafe {
                    (p.wrapping_add(0x33) as *mut u8).write(1);
                }
                FLOAT_RESULT.with(|v| *v.borrow_mut() = opacity.to_bits());
                responses(&[
                    RESULT.eax,
                    0xDEAD_BEEF,
                    0x77,
                    if success { 1 } else { 0 },
                    0xDEAD_BEEF,
                    0x78,
                    1,
                    0xABCD_EF80,
                ]);
                let mut r = Registers { ecx: p, ..ENTRY };
                let mut stack = [0, 0xFFFF_0001];
                unsafe {
                    set_model_registration(&mut r, stack.as_mut_ptr());
                }
                assert_eq!(stack[1], opacity.to_bits());
                assert_eq!(r.eax as u8, 0);
                assert_eq!(r.ecx, manager_table.as_ptr() as u32);
                let calls = take_calls();
                assert_eq!(
                    calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                    if success {
                        vec![53, 42, 36, 43, 42, 36, 43, 41]
                    } else {
                        vec![53, 42, 36, 43]
                    }
                );
                assert_eq!(calls[0].registers.ecx, p + 0x1B0);
                assert_eq!(
                    calls[1].args,
                    if opacity < 1.0 {
                        vec![1, opacity.to_bits()]
                    } else {
                        vec![0, 1.0f32.to_bits()]
                    }
                );
                assert_eq!(calls[3].args, [0x77]);
            }
        }
        unsafe {
            (p.wrapping_add(0x33) as *mut u8).write(0);
        }
        responses(&[0x1234_5680]);
        assert_eq!(run(0x0050_7F70, p, &[1], false).eax, 0x1234_5600);
        assert_eq!(take_calls().iter().map(|c| c.id).collect::<Vec<_>>(), [44]);
        unsafe {
            (p.wrapping_add(0x33) as *mut u8).write(2);
        }
        responses(&[7, 8, 9, 10, 0xAABB_CC80]);
        assert_eq!(
            run(0x0050_7F70, p, &[0], false),
            Registers {
                eax: 0xAABB_CC01,
                ecx: p,
                ..RESULT
            }
        );
        assert_eq!(
            take_calls().iter().map(|c| c.id).collect::<Vec<_>>(),
            [36, 37, 36, 37, 40]
        );
        manager_global = 0;
        assert_eq!(manager_global, 0);
        responses(&[]);
        assert_eq!(run(0x0050_7F70, p, &[0], false).eax, ENTRY.eax & !0xFF);
        assert!(take_calls().is_empty());
    }

    #[test]
    fn retirement_routes_model_actions_and_uses_strict_elapsed_delay_including_nan() {
        let _fpu = GameFpu::new();
        let mut object = Object::new(0x1D0);
        let p = object.ptr() as u32;
        let mut main_table = [0usize; 103];
        main_table[0x118 / 4] = occupant_enable_stub as *const () as usize;
        main_table[0x11C / 4] = model_disable_stub as *const () as usize;
        main_table[0x198 / 4] = retirement_state_stub as *const () as usize;
        let mut interface_table = [0usize; 33];
        interface_table[0x80 / 4] = retirement_gate_stub as *const () as usize;
        let mut manager_table = [0usize; 32];
        manager_table[0x78 / 4] = model_retire_stub as *const () as usize;
        manager_table[0x7C / 4] = model_fade_stub as *const () as usize;
        let mut manager = [manager_table.as_ptr() as u32];
        let mut manager_global = manager.as_mut_ptr() as u32;
        let mut occupant_table = [0usize; 25];
        occupant_table[0x60 / 4] = occupant_retire_stub as *const () as usize;
        let mut occupant = [occupant_table.as_ptr() as u32];
        let mut occupant_global = occupant.as_mut_ptr() as u32;
        let mut model_table = [0usize; 9];
        model_table[0] = model_handle_stub as *const () as usize;
        model_table[0x20 / 4] = model_position_stub as *const () as usize;
        let mut vbtable = [0u32, (-4i32) as u32];
        let mut model = [model_table.as_ptr() as u32, vbtable.as_mut_ptr() as u32];
        // The virtual base is the primary interface itself.
        let c = model.as_mut_ptr() as u32;
        let mut children = [c, 0, c];
        let begin = children.as_mut_ptr() as u32;
        let mut delay = 10.0f32;
        let _data = [
            fake_data(0x00B4_3DD0, (&mut manager_global as *mut u32).cast()),
            fake_data(0x00B4_3D0C, (&mut occupant_global as *mut u32).cast()),
            fake_data(0x00A9_52D8, (&mut delay as *mut f32).cast()),
        ];
        unsafe {
            store(p, 0, main_table.as_ptr() as u32);
            store(p, 4, interface_table.as_ptr() as u32);
            store(p, 0x110, begin);
            store(p, 0x114, begin + 12);
        }
        for (first, second, action) in [(1, 1, Some(46)), (0, 1, Some(47)), (0, 0, None)] {
            unsafe {
                (p.wrapping_add(0x33) as *mut u8).write(3);
                store(p, 0x1C0, 10.0f32.to_bits());
            }
            responses(&[1, 1, 0x55, 1, 0x56, 1, 1]);
            run(0x0050_8610, p, &[first, second], false);
            let calls = take_calls();
            let mut ids = vec![51, 45];
            if let Some(id) = action {
                ids.extend([36, id, 36, id]);
            }
            ids.push(48);
            assert_eq!(calls.iter().map(|c| c.id).collect::<Vec<_>>(), ids);
            assert_eq!(calls[0].args, [p + 0x170]);
            if action == Some(47) {
                assert_eq!(calls[3].args, [0x55, 3]);
            }
            assert_eq!(unsafe { load(p, 0x1C0) }, 10.0f32.to_bits());
        }
        unsafe {
            store(p, 0x114, begin);
        }
        for elapsed in [0.0f32, 10.0, 11.0, f32::INFINITY, f32::NAN] {
            unsafe {
                store(p, 0x1C0, elapsed.to_bits());
            }
            responses(&[0, 0xAABB_CC80]);
            run(0x0050_8610, p, &[0, 0], false);
            let calls = take_calls();
            assert_eq!(
                calls.iter().map(|c| c.id).collect::<Vec<_>>(),
                if elapsed > delay { vec![45, 49] } else { vec![45] }
            );
            assert_eq!(
                unsafe { load(p, 0x1C0) },
                if elapsed > delay { 0 } else { elapsed.to_bits() }
            );
        }
        for flags in [0u8, 1] {
            unsafe {
                (p.wrapping_add(0x33) as *mut u8).write(flags);
                store(p, 0x1C0, 0);
            }
            responses(&[1, 1, 1]);
            run(0x0050_8610, p, &[0, 0], false);
            assert_eq!(
                take_calls().iter().map(|c| c.id).collect::<Vec<_>>(),
                if flags == 0 { vec![45, 44] } else { vec![45, 48, 50] }
            );
        }
        RESULTS.with(|r| r.borrow_mut().clear());
    }

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

    mod movement {
        use super::*;

        stub!(flags, 4, 1, 100);
        stub!(remove, 4, 1, 101);
        float_stub!(width, 0, 0, 102);
        float_stub!(depth, 0, 0, 103);
        stub!(terrain, 0, 0, 104);
        stub!(obstacle, 12, 3, 105);
        stub!(water, 8, 2, 106);
        stub!(refresh, 0, 0, 107);
        stub!(rebuild, 0, 0, 108);
        stub!(notify, 8, 2, 109);
        stub!(get_transform, 4, 1, 110);
        stub!(set_transform, 4, 1, 111);
        stub!(matrix, 0, 2, 112);

        const MATRIX: [u32; 9] = [11, 22, 33, 44, 55, 66, 77, 88, 99];
        const TRANSFORM: [u32; 14] = [
            0xCDEF_12FF,
            101,
            102,
            103,
            104,
            105,
            106,
            107,
            108,
            109,
            201,
            202,
            203,
            0x3FC0_0000,
        ];

        #[derive(Default)]
        struct Observations {
            flags: u32,
            terrain: u32,
            obstacle: u32,
            water: u32,
            before: Vec<[u32; 14]>,
            after: Vec<[u32; 14]>,
            directions: Vec<[u32; 3]>,
            matrix_before: Vec<[u32; 9]>,
            rewrite_position: Option<(u32, [u32; 3])>,
            shorten_models: Option<(u32, u32)>,
            adjust_base: Option<u32>,
            redirect_position: Option<u32>,
        }

        thread_local! {
            static OBSERVED: RefCell<Observations> = RefCell::new(Observations::default());
        }

        fn hook(c: &Call) {
            OBSERVED.with(|o| {
                let mut o = o.borrow_mut();
                let answer = match c.id {
                    100 => Some(0xFACE_0000 | u32::from(o.flags & c.args[0] != 0)),
                    104 => Some(o.terrain),
                    105 => Some(o.obstacle),
                    106 => Some(o.water),
                    _ => None,
                };
                if let Some(eax) = answer {
                    RESULTS.with(|v| v.borrow_mut().push_front(Registers { eax, ..RESULT }));
                }
                unsafe {
                    match c.id {
                        110 => {
                            o.before.push(read_unaligned(c.args[0] as *const [u32; 14]));
                            write_unaligned(c.args[0] as *mut [u32; 14], TRANSFORM);
                            if let Some((p, value)) = o.rewrite_position {
                                write_unaligned(p as *mut [u32; 3], value);
                            }
                            if let Some(vbtable) = o.adjust_base {
                                store(vbtable, 4, 28);
                            }
                        }
                        111 => {
                            o.after.push(read_unaligned(c.args[0] as *const [u32; 14]));
                            if let Some((object, end)) = o.shorten_models {
                                store(object, 0x114, end);
                            }
                        }
                        112 => {
                            o.directions.push(read_unaligned(c.args[0] as *const [u32; 3]));
                            o.matrix_before.push(read_unaligned(c.args[1] as *const [u32; 9]));
                            write_unaligned(c.args[1] as *mut [u32; 9], MATRIX);
                            if let Some(position) = o.redirect_position {
                                store(c.args[1], 0x60, position);
                            }
                        }
                        _ => {}
                    }
                }
            });
        }

        struct Scene {
            object: Object,
            // The boxes keep every table and virtual base alive while callbacks use their addresses.
            _tables: Vec<Box<[usize]>>,
            models: [Box<[u32; 9]>; 2],
            bases: [Box<[u32; 2]>; 2],
            children: Box<[u32; 3]>,
            _services: [Box<[u32; 1]>; 3],
            _globals: [Box<*mut u8>; 2],
            _constants: [Box<u32>; 7],
            _fakes: Vec<crate::testing::Fake>,
        }

        impl Scene {
            fn new(with_models: bool) -> Self {
                take_calls();
                RESULTS.with(|v| v.borrow_mut().clear());
                FLOAT_RESULT.with(|v| *v.borrow_mut() = 256.0f32.to_bits());
                let mut table = vec![0usize; 0x1B0 / 4].into_boxed_slice();
                for (offset, function) in [
                    (0x34, remove as *const ()),
                    (0x7C, flags as *const ()),
                    (0x190, notify as *const ()),
                    (0x1A0, rebuild as *const ()),
                    (0x1AC, refresh as *const ()),
                ] {
                    table[offset / 4] = function as usize;
                }
                let mut model_table = vec![0usize; 11].into_boxed_slice();
                model_table[9] = get_transform as *const () as usize;
                model_table[10] = set_transform as *const () as usize;
                let bases = [Box::new([0u32, 12]), Box::new([0u32, 12])];
                let models: [Box<[u32; 9]>; 2] = core::array::from_fn(|index| {
                    let mut model = Box::new([0u32; 9]);
                    model[1] = bases[index].as_ptr() as u32;
                    model[4] = model_table.as_ptr() as u32;
                    model[8] = model_table.as_ptr() as u32;
                    model
                });
                let children = Box::new([models[0].as_ptr() as u32, 0, models[1].as_ptr() as u32]);
                let mut object = Object::new(0x1D0);
                let p = object.ptr() as u32;
                unsafe {
                    core::ptr::write_bytes(p as *mut u8, 0xCC, 0x1D0);
                    store(p, 0, table.as_ptr() as u32);
                    store(p, 0x110, children.as_ptr() as u32);
                    store(p, 0x114, children.as_ptr() as u32 + if with_models { 12 } else { 0 });
                }
                let mut city_table = vec![0usize; 0x16C / 4].into_boxed_slice();
                city_table[0x164 / 4] = width as *const () as usize;
                city_table[0x168 / 4] = depth as *const () as usize;
                city_table[0xA0 / 4] = terrain as *const () as usize;
                let mut terrain_table = vec![0usize; 15].into_boxed_slice();
                terrain_table[0x38 / 4] = water as *const () as usize;
                let mut obstacle_table = vec![0usize; 7].into_boxed_slice();
                obstacle_table[0x18 / 4] = obstacle as *const () as usize;
                let mut city = Box::new([city_table.as_ptr() as u32]);
                let ground = Box::new([terrain_table.as_ptr() as u32]);
                let mut obstacles = Box::new([obstacle_table.as_ptr() as u32]);
                let mut globals = [
                    Box::new(city.as_mut_ptr().cast::<u8>()),
                    Box::new(obstacles.as_mut_ptr().cast::<u8>()),
                ];
                let mut constants = [
                    1e-5f32.to_bits(),
                    0x1122_3344,
                    0.0625f32.to_bits(),
                    12582912.0f32.to_bits(),
                    0.5f32.to_bits(),
                    1.0f32.to_bits(),
                    0,
                ]
                .map(Box::new);
                let mut fakes = vec![fake(0x0080_B830, matrix as *const () as usize)];
                for (address, value) in [
                    0x00A9_52DC,
                    0x00B2_1BA4,
                    0x00A9_52F0,
                    0x00A9_52F4,
                    0x00A8_4D2C,
                    0x00B0_9978,
                    0x00A8_1054,
                ]
                .into_iter()
                .zip(constants.iter_mut())
                {
                    fakes.push(fake_data(address, (&mut **value as *mut u32).cast()));
                }
                for (address, value) in [0x00B4_3CEC, 0x00B4_3D08].into_iter().zip(globals.iter_mut()) {
                    fakes.push(fake_data(address, (&mut **value as *mut *mut u8).cast()));
                }
                OBSERVED.with(|o| {
                    *o.borrow_mut() = Observations {
                        flags: 1,
                        terrain: ground.as_ptr() as u32,
                        ..Default::default()
                    }
                });
                HOOK.with(|h| *h.borrow_mut() = Some(hook));
                Self {
                    object,
                    _tables: vec![table, model_table, city_table, terrain_table, obstacle_table],
                    models,
                    bases,
                    children,
                    _services: [city, ground, obstacles],
                    _globals: globals,
                    _constants: constants,
                    _fakes: fakes,
                }
            }

            fn ptr(&mut self) -> u32 {
                self.object.ptr() as u32
            }

            fn call(&mut self, position: u32, direction: u32) -> ([u32; 32], Registers, Vec<Call>) {
                let mut frame = [0xA5A5_A5A5u32; 32];
                frame[30] = position;
                frame[31] = direction;
                let mut r = Registers {
                    ecx: self.ptr(),
                    ..ENTRY
                };
                unsafe {
                    move_automaton(&mut r, frame.as_mut_ptr() as u32);
                }
                (frame, r, take_calls())
            }
        }

        impl Drop for Scene {
            fn drop(&mut self) {
                HOOK.with(|h| *h.borrow_mut() = None);
                RESULTS.with(|r| r.borrow_mut().clear());
            }
        }

        fn ids(calls: &[Call]) -> Vec<u32> {
            calls.iter().map(|c| c.id).collect()
        }

        #[test]
        fn core_orientation_vector_and_position_are_stored_without_touching_other_fields() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            let p = scene.ptr();
            let position = [10.0f32, 20.0, 30.0];
            let direction = [3.0f32, 0.0, 4.0];
            let before = bytes(p, 0x1D0);
            let (frame, result, calls) = scene.call(position.as_ptr() as u32, direction.as_ptr() as u32);
            assert_eq!(ids(&calls), [112, 102, 103, 100, 107, 108, 109]);
            assert_eq!(result, RESULT);
            assert_eq!(
                &bytes(p, 0x1D0)[0x40..0x4C],
                &position
                    .map(f32::to_bits)
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
            let normalized = [0.6f32.to_bits(), 0, 0.8f32.to_bits()];
            assert_eq!(unsafe { read_unaligned((p + 0x58) as *const [u32; 3]) }, normalized);
            let mut expected = before;
            expected[0x40..0x4C].copy_from_slice(&bytes(p + 0x40, 12));
            expected[0x58..0x64].copy_from_slice(&bytes(p + 0x58, 12));
            assert_eq!(bytes(p, 0x1D0), expected);
            assert_eq!(calls[0].args[0], p + 0x58);
            assert_eq!(calls.last().unwrap().args, [0xA5A5_A501, 0xA5A5_A501]);
            assert_eq!(frame[31], 30.0f32.to_bits());
            OBSERVED.with(|o| {
                assert_eq!(o.borrow().directions, [normalized]);
                assert_eq!(o.borrow().matrix_before, [[0x1122_3344; 9]]);
            });
            let before = bytes(p, 0x1D0);
            let zero = [0.0f32; 3];
            let (_, _, calls) = scene.call(0, zero.as_ptr() as u32);
            assert_eq!(ids(&calls), [109]);
            assert_eq!(calls[0].args, [0xA5A5_A500, 0xA5A5_A500]);
            assert_eq!(bytes(p, 0x1D0), before);
        }

        #[test]
        fn model_getters_preserve_translation_or_rotation_and_setters_reread_virtual_bases() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(true);
            let p = scene.ptr();
            let position = [16.0f32, 7.0, 32.0];
            let direction = [0.0f32, 1.0, 0.0];
            let model_p = scene.models[0].as_ptr() as u32;
            OBSERVED.with(|o| o.borrow_mut().adjust_base = Some(scene.bases[0].as_ptr() as u32));
            let (_, _, calls) = scene.call(position.as_ptr() as u32, direction.as_ptr() as u32);
            assert_eq!(
                ids(&calls),
                [112, 110, 111, 110, 111, 102, 103, 110, 111, 110, 111, 100, 107, 108, 109]
            );
            assert_eq!(calls[1].registers.ecx, model_p + 16);
            assert_eq!(calls[2].registers.ecx, model_p + 32);
            OBSERVED.with(|o| {
                let o = o.borrow();
                assert_eq!(o.before.len(), 4);
                for (index, initial) in o.before.iter().enumerate() {
                    assert_eq!(initial[0], if index == 0 { 0xA5A5_0000 } else { 0xCDEF_0000 });
                    assert_eq!(
                        &initial[1..],
                        &[
                            0x3F80_0000,
                            0,
                            0,
                            0,
                            0x3F80_0000,
                            0,
                            0,
                            0,
                            0x3F80_0000,
                            0,
                            0,
                            0,
                            0x3F80_0000
                        ]
                    );
                }
                for (index, output) in o.after.iter().enumerate() {
                    let mut expected = TRANSFORM;
                    if index < 2 {
                        expected[0] = 0xCDEF_1600;
                        expected[1..10].copy_from_slice(&MATRIX);
                    } else {
                        expected[0] = 0xCDEF_1300;
                        expected[10..13].copy_from_slice(&position.map(f32::to_bits));
                    }
                    assert_eq!(*output, expected);
                }
            });
            assert_eq!(
                unsafe { read_unaligned((p + 0x58) as *const [u32; 3]) },
                direction.map(f32::to_bits)
            );
        }

        #[test]
        fn callbacks_can_shorten_the_model_array_and_rewrite_the_input_position() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(true);
            let p = scene.ptr();
            let mut position = [16.0f32, 7.0, 32.0];
            let original = position.map(f32::to_bits);
            let changed = [48.0f32, 9.0, 64.0].map(f32::to_bits);
            OBSERVED.with(|o| {
                let mut o = o.borrow_mut();
                o.rewrite_position = Some((position.as_mut_ptr() as u32, changed));
                o.shorten_models = Some((p, scene.children.as_ptr() as u32 + 4));
            });
            let (_, _, calls) = scene.call(position.as_ptr() as u32, 0);
            assert_eq!(ids(&calls), [102, 103, 110, 111, 100, 107, 108, 109]);
            assert_eq!(unsafe { read_unaligned((p + 0x40) as *const [u32; 3]) }, original);
            OBSERVED.with(|o| assert_eq!(&o.borrow().after[0][10..13], &changed));
            let replacement = [80.0f32, 10.0, 96.0];
            let direction = [0.0f32, 0.0, 1.0];
            OBSERVED.with(|o| {
                let mut o = o.borrow_mut();
                o.rewrite_position = None;
                o.redirect_position = Some(replacement.as_ptr() as u32);
            });
            let (_, _, calls) = scene.call(0, direction.as_ptr() as u32);
            assert_eq!(ids(&calls), [112, 110, 111, 102, 103, 110, 111, 100, 107, 108, 109]);
            assert_eq!(
                unsafe { read_unaligned((p + 0x40) as *const [u32; 3]) },
                replacement.map(f32::to_bits)
            );
        }

        #[unsafe(naked)]
        unsafe extern "C" fn extended_width() {
            core::arch::naked_asm!(
                "sub esp, 12", "push edx", "push ecx", "push eax", "mov ecx, esp",
                "lea edx, [esp + 24]", "push edx", "call {}",
                "pop eax", "pop ecx", "pop edx", "fld tbyte ptr [esp]", "add esp, 12", "ret",
                sym extended_width_body,
            );
        }

        unsafe extern "thiscall" fn extended_width_body(r: &mut Registers, _stack: *mut u32) {
            CALLS.with(|c| {
                c.borrow_mut().push(Call {
                    id: 102,
                    registers: *r,
                    args: vec![],
                })
            });
            *r = RESULT;
            // 256 + 2^-20 is above 256 in ST0 and rounds to 256 when stored as binary32.
            let value = [0u8, 0, 0, 0, 8, 0, 0, 0x80, 7, 0x40];
            unsafe {
                core::ptr::copy_nonoverlapping(value.as_ptr(), (r as *mut Registers).cast::<u8>().add(12), 10);
            }
        }

        #[test]
        fn city_limit_keeps_the_unrounded_st0_return() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            scene._tables[2][0x164 / 4] = extended_width as *const () as usize;
            let position = [256.0f32, 0.0, 0.0];
            let (_, _, calls) = scene.call(position.as_ptr() as u32, 0);
            assert_eq!(ids(&calls), [102, 103, 100, 107, 108, 109]);
            assert_eq!(unsafe { load(scene.ptr(), 0x40) }, 256.0f32.to_bits());
        }

        #[test]
        fn city_bounds_reject_negative_and_equal_edges_and_allow_unordered_coordinates() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            let p = scene.ptr();
            for (x, z, expected) in [
                (-1.0f32, 0.0f32, vec![101]),
                (0.0, -1.0, vec![101]),
                (256.0, 0.0, vec![102, 101]),
                (257.0, 0.0, vec![102, 101]),
                (0.0, 256.0, vec![102, 103, 101]),
                (f32::INFINITY, 0.0, vec![102, 101]),
                (-0.0, 0.0, vec![102, 103, 100, 107, 108, 109]),
                (f32::NAN, f32::NAN, vec![102, 103, 100, 107, 108, 109]),
            ] {
                let position = [x, f32::from_bits(0x7F80_0001), z];
                let before = bytes(p, 0x1D0);
                let (_, result, calls) = scene.call(position.as_ptr() as u32, 0);
                assert_eq!(ids(&calls), expected, "x={x}, z={z}");
                assert_eq!(result, RESULT);
                if calls.last().unwrap().id == 101 {
                    assert_eq!(calls.last().unwrap().args, [0]);
                    assert_eq!(bytes(p, 0x1D0), before);
                } else {
                    assert_eq!(
                        unsafe { read_unaligned((p + 0x40) as *const [u32; 3]) },
                        position.map(f32::to_bits)
                    );
                }
            }
        }

        #[test]
        fn terrain_flags_choose_obstacles_then_water_and_removal_still_finishes_notifications() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            let p = scene.ptr();
            let position = [32.0f32, 5.0, 48.0];
            for (mask, obstacle_result, water_result, expected) in [
                (0, 0, 0, vec![102, 103, 100, 100, 100, 107, 108, 109]),
                (1, 1, 1, vec![102, 103, 100, 107, 108, 109]),
                (
                    0x400000,
                    0,
                    1,
                    vec![102, 103, 100, 100, 104, 100, 105, 100, 107, 108, 109],
                ),
                (
                    0x400000,
                    0x80000000,
                    1,
                    vec![102, 103, 100, 100, 104, 100, 105, 101, 107, 108, 109],
                ),
                (
                    0x800000,
                    1,
                    0,
                    vec![102, 103, 100, 100, 100, 104, 100, 100, 106, 107, 108, 109],
                ),
                (
                    0x800000,
                    1,
                    0x80000000,
                    vec![102, 103, 100, 100, 100, 104, 100, 100, 106, 101, 107, 108, 109],
                ),
                (
                    0xC00000,
                    0,
                    1,
                    vec![102, 103, 100, 100, 104, 100, 105, 100, 106, 101, 107, 108, 109],
                ),
            ] {
                OBSERVED.with(|o| {
                    let mut o = o.borrow_mut();
                    o.flags = mask;
                    o.obstacle = obstacle_result;
                    o.water = water_result;
                });
                let (frame, result, calls) = scene.call(position.as_ptr() as u32, 0);
                assert_eq!(ids(&calls), expected, "mask={mask:x}");
                assert_eq!(result, RESULT);
                for c in &calls {
                    match c.id {
                        105 => assert_eq!(c.args, [2, 3, 0]),
                        106 => assert_eq!(c.args, [2, 3]),
                        101 => {
                            assert_eq!(c.args, [0]);
                            assert_eq!(c.registers.ecx, p);
                        }
                        100 | 107 | 108 | 109 => assert_eq!(c.registers.ecx, p),
                        _ => {}
                    }
                }
                if mask & 0xC00000 != 0 && mask & 1 == 0 {
                    OBSERVED.with(|o| assert_eq!(frame[30], o.borrow().terrain));
                    assert_eq!(frame[31], 3);
                }
                assert_eq!(calls.last().unwrap().args, [0xA5A5_A501, 0xA5A5_A500]);
                assert_eq!(unsafe { load(p, 0x40) }, position[0].to_bits());
            }
        }

        #[test]
        fn direction_tolerance_is_inclusive_y_nan_rejects_and_x_or_z_nan_is_accepted() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            let p = scene.ptr();
            let tolerance = 1e-5f32;
            let below = f32::from_bits(tolerance.to_bits() - 1);
            for (direction, accepted) in [
                ([0.0f32; 3], false),
                ([-0.0, below, -below], false),
                ([tolerance, 0.0, 0.0], true),
                ([0.0, -tolerance, 0.0], true),
                ([0.0, 0.0, tolerance], true),
                ([1.0, f32::NAN, 1.0], false),
                ([1.0, f32::from_bits(0x7F80_0001), 1.0], false),
                ([f32::NAN, 0.0, 0.0], true),
                ([0.0, 0.0, f32::NAN], true),
                ([0.0, f32::INFINITY, 0.0], true),
            ] {
                let before = bytes(p, 0x1D0);
                let (frame, result, calls) = scene.call(0, direction.as_ptr() as u32);
                assert_eq!(result, RESULT);
                assert_eq!(ids(&calls), if accepted { vec![112, 107, 108, 109] } else { vec![109] });
                assert_eq!(
                    calls.last().unwrap().args,
                    [0xA5A5_A500, 0xA5A5_A500 | u32::from(accepted)]
                );
                assert_eq!(frame[31], direction[1].to_bits());
                if !accepted {
                    assert_eq!(bytes(p, 0x1D0), before);
                }
            }
            let position = [-1.0f32, 0.0, 0.0];
            let direction = [3.0f32, 0.0, 4.0];
            let (_, _, calls) = scene.call(position.as_ptr() as u32, direction.as_ptr() as u32);
            assert_eq!(ids(&calls), [112, 101]);
            assert_eq!(
                unsafe { read_unaligned((p + 0x58) as *const [u32; 3]) },
                [0.6f32.to_bits(), 0, 0.8f32.to_bits()]
            );
        }

        fn normalize_on_x87(mut value: [u32; 3]) -> [u32; 3] {
            let unit = 1.0f32;
            unsafe {
                core::arch::asm!(
                    "fld dword ptr [{p} + 8]", "fld dword ptr [{p} + 4]", "fld dword ptr [{p}]",
                    "fld st(0)", "fmul st(0), st(1)", "fld st(2)", "fmul st(0), st(3)",
                    "faddp st(1), st(0)", "fld st(3)", "fmul st(0), st(4)", "faddp st(1), st(0)",
                    "fsqrt", "fstp st(3)", "fstp st(0)", "fstp st(0)", "fdivr dword ptr [{unit}]",
                    "fld st(0)", "fmul dword ptr [{p}]", "fst dword ptr [{p}]",
                    "fld st(1)", "fmul dword ptr [{p} + 4]", "fstp dword ptr [{p} + 4]",
                    "fxch st(1)", "fmul dword ptr [{p} + 8]", "fstp dword ptr [{p} + 8]", "fstp dword ptr [{p}]",
                    p = in(reg) value.as_mut_ptr(), unit = in(reg) &unit,
                    out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _,
                );
            }
            value
        }

        #[test]
        fn normalization_matches_x87_rounding_exponent_range_and_nan_payloads() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            let p = scene.ptr();
            for control in [0x007Fu16, 0x047F, 0x087F, 0x0C7F, 0x027F, 0x037F] {
                crate::x87::set_control_word(control);
                for direction in [
                    [3.0f32, -0.0, 4.0].map(f32::to_bits),
                    [1.0f32, 2.0, 3.0].map(f32::to_bits),
                    [f32::MAX, f32::MAX, f32::MAX].map(f32::to_bits),
                    [0x7FC0_1234, 0x3F80_0000, 0xFFC0_4321],
                    [0x7F80_0001, 0x3F80_0000, 0x7FC0_4321],
                    [0x3F80_0000, 0x7F80_0000, 0x3F80_0000],
                    [0x3F80_0000, 0x0000_0001, 0x0000_0001],
                ] {
                    let expected = normalize_on_x87(direction);
                    let (_, _, calls) = scene.call(0, direction.as_ptr() as u32);
                    assert_eq!(ids(&calls), [112, 107, 108, 109]);
                    assert_eq!(
                        unsafe { read_unaligned((p + 0x58) as *const [u32; 3]) },
                        expected,
                        "control={control:x}, input={direction:x?}"
                    );
                    assert_eq!(crate::x87::control_word(), control);
                }
            }
        }

        fn cell_on_x87(coordinate: f32) -> (i32, u32) {
            let operands = [coordinate, 0.0625f32, 0.5f32, 12582912.0f32];
            let mut scratch = 0u32;
            let (cell, status): (i32, u16);
            unsafe {
                core::arch::asm!(
                    "fld dword ptr [{operands}]", "fmul dword ptr [{operands} + 4]", "fld dword ptr [{operands} + 8]",
                    "fsubr st(0), st(1)", "fadd dword ptr [{operands} + 12]", "fstp dword ptr [{scratch}]",
                    "mov eax, [{scratch}]", "add eax, 0xb4c00000", "mov {cell:e}, eax", "inc eax",
                    "mov [{scratch}], eax", "fild dword ptr [{scratch}]", "fxch st(1)", "fucompp", "fnstsw ax",
                    operands = in(reg) operands.as_ptr(), scratch = in(reg) &mut scratch,
                    cell = out(reg) cell, out("ax") status, out("st(0)") _, out("st(1)") _,
                );
            }
            (
                if status & 0x4400 == 0x4000 {
                    cell.wrapping_add(1)
                } else {
                    cell
                },
                scratch,
            )
        }

        #[test]
        fn biased_cells_match_x87_at_boundaries_overflow_nan_and_all_rounding_modes() {
            let _fpu = GameFpu::new();
            let _scene = Scene::new(false);
            for control in [0x007Fu16, 0x047F, 0x087F, 0x0C7F, 0x027F, 0x037F] {
                crate::x87::set_control_word(control);
                for coordinate in [
                    0u32,
                    0x8000_0000,
                    1,
                    0x8000_0001,
                    0x7F7F_FFFF,
                    0xFF7F_FFFF,
                    0x7F80_0000,
                    0xFF80_0000,
                    0x7F80_0001,
                    0x7FC0_1234,
                    16.0f32.to_bits() - 1,
                    16.0f32.to_bits(),
                    16.0f32.to_bits() + 1,
                    32.0f32.to_bits() - 1,
                    32.0f32.to_bits(),
                    32.0f32.to_bits() + 1,
                    (-16.0f32).to_bits(),
                    134217728.0f32.to_bits(),
                ]
                .map(f32::from_bits)
                {
                    let expected = cell_on_x87(coordinate);
                    let mut scratch = 0u32;
                    let cell = unsafe { movement_cell(coordinate, &mut scratch as *mut u32 as u32) };
                    assert_eq!(
                        (cell, scratch),
                        expected,
                        "control={control:x}, coordinate={:x}",
                        coordinate.to_bits()
                    );
                    assert_eq!(crate::x87::control_word(), control);
                }
            }
        }

        #[unsafe(naked)]
        unsafe extern "C" fn seeded_entry() {
            core::arch::naked_asm!(
                "mov dword ptr [esp - 0x64], 0x11223344", "mov dword ptr [esp - 0x60], 0x55667788",
                "mov dword ptr [esp - 0x38], 0x99aabbcc", "jmp {}", sym fc4p_005080c0,
            );
        }

        #[unsafe(naked)]
        unsafe extern "fastcall" fn checked_entry(_this: u32, _output: *mut u32) {
            core::arch::naked_asm!(
                "push ebx", "push esi", "push edi", "push ebp", "push edx",
                "mov ebx, 0x12345678", "mov esi, 0x23456789", "mov edi, 0x3456789a", "mov ebp, esp",
                "push 0", "push 0", "call {}",
                "push edx", "mov edx, [esp + 4]", "mov [edx], eax", "mov [edx + 4], ecx",
                "pop eax", "mov [edx + 8], eax", "mov [edx + 12], ebx", "mov [edx + 16], esi",
                "mov [edx + 20], edi", "mov [edx + 24], ebp", "mov [edx + 28], esp",
                "pushfd", "pop eax", "mov [edx + 32], eax",
                "add esp, 4", "pop ebp", "pop edi", "pop esi", "pop ebx", "ret",
                sym seeded_entry,
            );
        }

        #[test]
        fn export_preserves_original_stack_bytes_and_integer_results_and_fpu_state() {
            let _fpu = GameFpu::new();
            let mut scene = Scene::new(false);
            let p = scene.ptr();
            #[repr(C, align(16))]
            struct State([u8; 512]);
            let mut before = State([0; 512]);
            let mut after = State([0; 512]);
            let mut r = Registers { ecx: p, ..ENTRY };
            unsafe {
                core::arch::asm!("fld1", "fxsave [{p}]", p = in(reg) &mut before, out("st(0)") _);
                invoke(&mut r, seeded_entry as *const () as usize, &[0, 0], Conv::Thiscall);
                core::arch::asm!("fxsave [{p}]", "fstp st(0)", p = in(reg) &mut after, out("st(0)") _);
            }
            assert_eq!(r, RESULT);
            assert_eq!(&before.0[..5], &after.0[..5]);
            assert_eq!(&before.0[24..160], &after.0[24..160]);
            let calls = take_calls();
            assert_eq!(ids(&calls), [109]);
            assert_eq!(calls[0].args, [0x5566_7700, 0x1122_3300]);
            assert_eq!(
                calls[0].registers,
                Registers {
                    eax: 0x1122_3300,
                    ecx: p,
                    edx: unsafe { load(p, 0) }
                }
            );
            let position = [0.0f32; 3];
            r.ecx = p;
            unsafe {
                invoke(
                    &mut r,
                    seeded_entry as *const () as usize,
                    &[position.as_ptr() as u32, 0],
                    Conv::Thiscall,
                );
            }
            let calls = take_calls();
            assert_eq!(calls.last().unwrap().args, [0x5566_7701, 0x1122_3300]);
            let mut output = [0u32; 9];
            unsafe {
                checked_entry(p, output.as_mut_ptr());
            }
            assert_eq!(&output[..3], &[RESULT.eax, RESULT.ecx, RESULT.edx]);
            assert_eq!(&output[3..6], &[0x1234_5678, 0x2345_6789, 0x3456_789A]);
            assert_eq!(output[6], output[7]);
            assert_eq!(output[8] & 0x400, 0);
        }
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
