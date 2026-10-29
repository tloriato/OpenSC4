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

//! Shared helpers, group 05: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

/// The three caller-visible integer registers used by these helpers.
#[repr(C)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

/// Keep the entry and exit registers while running the operation in Rust. Stack arguments retain their
/// original positions and the entry removes exactly the original number of bytes.
macro_rules! rust_port {
    ($address:literal, $export:ident, $name:ident, $label:literal, $pop:literal,
     ($this:ident, $regs:ident $(, $arg:ident: $ty:ty)*) $body:block) => {
        port! {
            #[port(address = $address, owner = "shared", name = $label, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 12", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "mov edx, esp", ".rept {count}", "push dword ptr [esp + {offset}]", ".endr",
                    "call {body}", "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]",
                    "add esp, 12", "ret {pop}",
                    count = const $pop / 4, offset = const 12 + $pop, pop = const $pop, body = sym $name,
                );
            }
        }
        unsafe extern "fastcall" fn $name($this: *mut u8, $regs: &mut Registers $(, $arg: $ty)*) $body
    };
}

macro_rules! leaf {
    ($address:literal, $export:ident, $name:literal, $($instruction:literal),+ $(,)?) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!($($instruction),+);
            }
        }
    };
}

// The tail targets are declared here as well as embedded in the forwarding entries. Tests resolve the
// target without changing the incoming registers or adding an extra call to the target itself.
#[cfg(test)]
extern "C" fn resolve_tail(address: u32) -> usize {
    crate::testing::resolve(address)
}

macro_rules! forward {
    ($address:literal, $export:ident, $name:literal, $adjust:literal,
     $target:literal, $callee:ident ($($arg:ident: $ty:ty),*)) => {
        game_fn! {
            #[allow(dead_code)]
            fn $callee(this: *mut u8 $(, $arg: $ty)*) -> u32 = "thiscall" $target;
        }
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!($adjust, "jmp dword ptr [2f]", "2: .long {target}", target = const $target);
                #[cfg(test)]
                core::arch::naked_asm!($adjust,
                    "pushfd", "pushad", "push {target}", "call {resolve}", "add esp, 4",
                    "mov [esp + 12], eax", "popad", "popfd", "jmp dword ptr [esp - 24]",
                    target = const $target, resolve = sym resolve_tail);
            }
        }
    };
}

// Direct field operations retain all register bits that they do not replace.
leaf!(
    0x005B_9D30,
    fc4p_005b9d30,
    "delayed stop byte",
    "mov al, [ecx + 0xc]",
    "ret"
);
forward!(
    0x005B_CB60,
    fc4p_005bcb60,
    "advice secondary release",
    "sub ecx, 4",
    0x005B_CB30,
    advice_release()
);
forward!(
    0x005B_CB70,
    fc4p_005bcb70,
    "advice secondary deleting destructor",
    "sub ecx, 4",
    0x005B_CB80,
    advice_delete(flags: u32)
);

// Clear only AL, with the same stack cleanup as the interface method.
leaf!(
    0x005B_E2E0,
    fc4p_005be2e0,
    "material variant unavailable",
    "xor al, al",
    "ret 12"
);
forward!(
    0x005B_E3F0,
    fc4p_005be3f0,
    "embedded reference count release",
    "add ecx, 4",
    0x0090_D96B,
    reference_release()
);
forward!(
    0x005B_E420,
    fc4p_005be420,
    "secondary vector element",
    "sub ecx, 4",
    0x005B_E3E0,
    vector_element(index: u32)
);
forward!(
    0x005C_0500,
    fc4p_005c0500,
    "particle secondary add reference",
    "sub ecx, 4",
    0x004B_DF00,
    particle_add_reference()
);
leaf!(
    0x005D_2070,
    fc4p_005d2070,
    "tutorial system pointer",
    "mov eax, [ecx + 0x1cc]",
    "ret"
);
leaf!(
    0x005D_25F0,
    fc4p_005d25f0,
    "pedestrian manager pointer",
    "mov eax, [ecx + 0x1d4]",
    "ret"
);
leaf!(
    0x005D_4AC0,
    fc4p_005d4ac0,
    "stored position x",
    "fld dword ptr [ecx + 0x3c]",
    "ret"
);
leaf!(
    0x005D_6140,
    fc4p_005d6140,
    "signed occupant byte",
    "movsx eax, byte ptr [ecx + 0x58]",
    "ret"
);
leaf!(
    0x005D_6150,
    fc4p_005d6150,
    "store occupant byte",
    "mov al, [esp + 4]",
    "mov [ecx + 0x58], al",
    "ret 4"
);
leaf!(
    0x005D_EB40,
    fc4p_005deb40,
    "ambience occupant unsupported",
    "xor al, al",
    "ret 4"
);
forward!(
    0x005E_28F0,
    fc4p_005e28f0,
    "automaton add reference",
    "add ecx, 4",
    0x005E_89B0,
    occupant_add_reference()
);
forward!(
    0x005E_2900,
    fc4p_005e2900,
    "automaton release",
    "add ecx, 4",
    0x005E_8DF0,
    occupant_release()
);
forward!(
    0x005E_29B0,
    fc4p_005e29b0,
    "automaton secondary add reference",
    "sub ecx, 4",
    0x005E_28F0,
    automaton_add_reference_4()
);
forward!(
    0x005E_29C0,
    fc4p_005e29c0,
    "automaton secondary release",
    "sub ecx, 4",
    0x005E_2900,
    automaton_release_4()
);
forward!(
    0x005E_29E0,
    fc4p_005e29e0,
    "automaton tertiary add reference",
    "sub ecx, 8",
    0x005E_28F0,
    automaton_add_reference_8()
);
forward!(
    0x005E_29F0,
    fc4p_005e29f0,
    "automaton tertiary release",
    "sub ecx, 8",
    0x005E_2900,
    automaton_release_8()
);
forward!(
    0x005E_2A10,
    fc4p_005e2a10,
    "automaton fourth add reference",
    "sub ecx, 12",
    0x005E_28F0,
    automaton_add_reference_12()
);
forward!(
    0x005E_2A20,
    fc4p_005e2a20,
    "automaton fourth release",
    "sub ecx, 12",
    0x005E_2900,
    automaton_release_12()
);
forward!(
    0x005E_CAF0,
    fc4p_005ecaf0,
    "prop controller add reference",
    "add ecx, 12",
    0x005E_89B0,
    controller_add_reference()
);
forward!(
    0x005E_CB00,
    fc4p_005ecb00,
    "prop controller release",
    "add ecx, 12",
    0x005E_8DF0,
    controller_release()
);
forward!(
    0x005E_D5C0,
    fc4p_005ed5c0,
    "prop controller secondary add reference",
    "sub ecx, 4",
    0x005E_CAF0,
    controller_add_reference_4()
);
forward!(
    0x005E_D5D0,
    fc4p_005ed5d0,
    "prop controller secondary release",
    "sub ecx, 4",
    0x005E_CB00,
    controller_release_4()
);
forward!(
    0x005E_D620,
    fc4p_005ed620,
    "prop controller tertiary add reference",
    "sub ecx, 12",
    0x005E_CAF0,
    controller_add_reference_12()
);
forward!(
    0x005E_D630,
    fc4p_005ed630,
    "prop controller tertiary release",
    "sub ecx, 12",
    0x005E_CB00,
    controller_release_12()
);
forward!(
    0x005E_D650,
    fc4p_005ed650,
    "prop controller fourth add reference",
    "sub ecx, 16",
    0x005E_CAF0,
    controller_add_reference_16()
);
forward!(
    0x005E_D660,
    fc4p_005ed660,
    "prop controller fourth release",
    "sub ecx, 16",
    0x005E_CB00,
    controller_release_16()
);
forward!(
    0x005E_D680,
    fc4p_005ed680,
    "prop controller fifth add reference",
    "sub ecx, 20",
    0x005E_CAF0,
    controller_add_reference_20()
);
forward!(
    0x005E_FE60,
    fc4p_005efe60,
    "prop controller secondary occupant",
    "sub ecx, 4",
    0x005F_8400,
    controller_as_occupant()
);
forward!(
    0x005F_8F80,
    fc4p_005f8f80,
    "file browser secondary add reference",
    "sub ecx, 4",
    0x0094_D34F,
    file_browser_add_reference()
);
forward!(
    0x005F_D720,
    fc4p_005fd720,
    "pooled secondary deleting destructor",
    "sub ecx, 4",
    0x005F_E980,
    pooled_delete(flags: u32)
);
leaf!(
    0x0060_CE80,
    fc4p_0060ce80,
    "thread identifier",
    "mov eax, [ecx + 4]",
    "ret"
);
leaf!(
    0x0060_FB50,
    fc4p_0060fb50,
    "variant boolean address",
    "lea eax, [ecx + 4]",
    "ret"
);
leaf!(
    0x0060_FB80,
    fc4p_0060fb80,
    "draw context engine",
    "mov eax, [ecx + 0x28]",
    "ret"
);
leaf!(
    0x0062_C8E0,
    fc4p_0062c8e0,
    "clear pending pointer",
    "mov dword ptr [ecx + 0x20c], 0",
    "ret"
);
leaf!(
    0x0062_C980,
    fc4p_0062c980,
    "window message unhandled",
    "xor al, al",
    "ret 8"
);
leaf!(
    0x0062_C990,
    fc4p_0062c990,
    "float argument identity",
    "fld dword ptr [esp + 4]",
    "ret 4"
);
forward!(
    0x0062_C9B0,
    fc4p_0062c9b0,
    "window secondary release",
    "sub ecx, 4",
    0x0062_3270,
    window_release()
);

// Object words are accessed separately: overlapping inputs observe earlier stores.
unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}
unsafe fn store(p: *mut u8, offset: usize, value: u32) {
    unsafe { write_unaligned(p.wrapping_add(offset).cast(), value) }
}

/// Invoke a game method and keep its ECX as well as EDX:EAX. Some helpers forward all three registers.
unsafe fn invoke(r: &mut Registers, target: usize, args: &[u32], caller_pops: bool) {
    let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, target, r.ecx, args);
    call.eax = r.eax;
    call.edx = r.edx;
    call.caller_pops = u32::from(caller_pops);
    // SAFETY: the enclosing port supplies the original call signature. EDI holds the call descriptor;
    // every argument is pushed once, and caller cleanup is performed only for a cdecl target.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f",
            "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
            in("edi") &mut call, clobber_abi("C"),
        );
    }
    r.eax = call.eax;
    r.ecx = call.ecx;
    r.edx = call.edx;
}

unsafe fn virtual_call(r: &mut Registers, offset: usize, args: &[u32]) {
    unsafe { invoke(r, crate::abi::vslot(r.ecx as *mut u8, offset), args, false) }
}

game_fn! { #[allow(dead_code)] fn destroy_unknown(this: *mut u8) -> u32 = "thiscall" 0x0090_D990; }
game_fn! { #[allow(dead_code)] fn delete_storage(p: *mut u8) -> u32 = "C" 0x005E_5620; }
game_fn! { #[allow(dead_code)] fn finish_occupant(this: *mut u8) -> u32 = "thiscall" 0x005D_3250; }
game_fn! { #[allow(dead_code)] fn query_occupant(this: *mut u8, iid: u32, out: *mut u8) -> u32 = "thiscall" 0x005E_89C0; }

rust_port!(0x005B_CB80, fc4p_005bcb80, destroy_advice, "destroy advice and optionally free storage", 4,
    (this, r, flags: u32) {
        unsafe {
            r.ecx = if this.is_null() { 0 } else { this.wrapping_add(4) as u32 };
            invoke(r, crate::abi::target(0x0090_D990), &[], false);
            if flags & 1 != 0 {
                invoke(r, crate::abi::target(0x005E_5620), &[this as u32], true);
            }
            r.eax = this as u32;
        }
    }
);

rust_port!(0x005D_3500, fc4p_005d3500, finish_conditional_occupant, "finish conditional occupant", 0,
    (this, r) {
        unsafe {
            let mut result = 1;
            if word(this, 0x20) == 2 {
                r.eax = (r.eax & !255) | u32::from(*this.add(0x29));
                if r.eax as u8 == 0 {
                    r.eax = word(this, 0);
                    virtual_call(r, 0x1c, &[]);
                    result = r.eax as u8;
                }
            }
            r.ecx = this.wrapping_sub(8) as u32;
            invoke(r, crate::abi::target(0x005D_3250), &[], false);
            r.eax = (r.eax & !255) | u32::from(result);
        }
    }
);

rust_port!(0x005D_6360, fc4p_005d6360, get_flora_position, "flora position", 4,
    (this, r, out: *mut u8) {
        unsafe {
            r.edx = word(this, 0x3c);
            r.eax = out as u32;
            store(out, 0, r.edx);
            r.edx = word(this, 0x40);
            store(out, 4, r.edx);
            r.ecx = word(this, 0x44);
            store(out, 8, r.ecx);
        }
    }
);

rust_port!(0x005D_6380, fc4p_005d6380, set_flora_position, "set flora position", 4,
    (this, r, source: *mut u8) {
        unsafe {
            r.edx = word(source, 0);
            store(this, 0x3c, r.edx);
            r.edx = word(source, 4);
            store(this, 0x40, r.edx);
            r.eax = word(source, 8);
            store(this, 0x44, r.eax);
        }
    }
);

rust_port!(0x005D_7B40, fc4p_005d7b40, add_glyph_texture_reference, "glyph texture add reference", 0,
    (this, r) {
        unsafe {
            r.eax = word(this, 0x24).wrapping_add(1);
            store(this, 0x24, r.eax);
        }
    }
);

rust_port!(0x005D_C950, fc4p_005dc950, rectangle_equal, "four word rectangle equality", 4,
    (this, r, other: *mut u8) {
        unsafe {
            r.eax = other as u32;
            for offset in [0, 4, 8] {
                r.edx = word(this, offset);
                if r.edx != word(other, offset) { r.eax = 0; return; }
            }
            r.ecx = word(this, 12);
            r.eax = u32::from(r.ecx == word(other, 12));
        }
    }
);

rust_port!(0x005D_C980, fc4p_005dc980, rectangle_union, "signed rectangle union", 8,
    (this, r, a: *mut u8, b: *mut u8) {
        unsafe {
            let left = (word(a, 0) as i32).min(word(b, 0) as i32);
            store(this, 0, left as u32);
            let top = (word(a, 4) as i32).min(word(b, 4) as i32);
            store(this, 4, top as u32);
            let right = (word(a, 8) as i32).max(word(b, 8) as i32);
            store(this, 8, right as u32);
            let bottom_a = word(a, 12) as i32;
            r.edx = word(b, 12);
            let bottom = bottom_a.max(r.edx as i32);
            store(this, 12, bottom as u32);
            r.eax = u32::from(left != right && top != bottom);
        }
    }
);

rust_port!(0x005D_ED60, fc4p_005ded60, get_lot_texture_bounds, "lot texture bounding box", 8,
    (this, r, low: *mut u8, high: *mut u8) {
        unsafe {
            for offset in [0, 4, 8] {
                r.edx = word(this, 0x34 + offset);
                store(low, offset, r.edx);
            }
            r.edx = word(this, 0x40);
            r.eax = high as u32;
            store(high, 0, r.edx);
            r.edx = word(this, 0x44);
            store(high, 4, r.edx);
            r.ecx = word(this, 0x48);
            store(high, 8, r.ecx);
        }
    }
);

rust_port!(0x005E_28C0, fc4p_005e28c0, query_retaining_wall, "retaining wall interface query", 8,
    (this, r, iid: u32, out: *mut u8) {
        unsafe {
            r.eax = iid;
            if iid == 0x295D_1162 {
                r.eax = out as u32;
                store(out, 0, this as u32);
                r.edx = word(this, 0);
                virtual_call(r, 4, &[]);
                r.eax = (r.eax & !255) | 1;
            } else {
                r.ecx = this.wrapping_add(4) as u32;
                invoke(r, crate::abi::target(0x005E_89C0), &[iid, out as u32], false);
            }
        }
    }
);

rust_port!(0x005E_C9E0, fc4p_005ec9e0, set_prop_orientation, "clamp prop orientation", 4,
    (this, r, orientation: i32) {
        r.eax = orientation.clamp(0, 3) as u32;
        unsafe { *this.add(0x61) = r.eax as u8; }
        r.eax = 1;
    }
);

rust_port!(0x005F_47F0, fc4p_005f47f0, add_command_system_reference, "command system add reference", 0,
    (this, r) {
        unsafe {
            r.eax = word(this, 4).wrapping_add(1);
            store(this, 4, r.eax);
        }
    }
);

rust_port!(0x005F_8400, fc4p_005f8400, as_prop_controller_occupant, "prop controller occupant interface", 0,
    (this, r) {
        r.eax = if this.is_null() { 0 } else { this.wrapping_add(12) as u32 };
    }
);

game_fn! {
    #[allow(dead_code)]
    fn allocate_pool_block(exponent: u32) -> *mut u8 = "C" 0x0090_D8E0;
}
game_fn! {
    #[allow(dead_code)]
    fn return_pool_block(block: *mut u8, exponent: u32) -> u32 = "C" 0x0090_D91C;
}
game_fn! {
    #[allow(dead_code)]
    fn discard_empty_pool(this: *mut u8, block: *mut u8) -> u32 = "thiscall" 0x0090_D031;
}
game_fn! {
    #[allow(dead_code)]
    fn reopen_pool(this: *mut u8, block: *mut u8) -> u32 = "thiscall" 0x0090_CFF9;
}

/// Chain slots from the highest address down. Only the link word of each slot and two header words
/// are initialized; payload and padding bytes retain the allocator's contents.
unsafe fn initialize_pool(r: &mut Registers, exponent: u32, last: u32, count: u32, stride: u32) {
    unsafe {
        invoke(r, crate::abi::target(0x0090_D8E0), &[exponent], true);
        let block = r.eax as *mut u8;
        r.edx = 0;
        r.ecx = r.eax.wrapping_add(last);
        let mut head = 0;
        for _ in 0..count {
            store(r.ecx as *mut u8, 0, r.edx);
            head = r.ecx;
            r.edx = r.ecx;
            r.ecx = r.ecx.wrapping_sub(stride);
        }
        store(block, 8, head);
        store(block, 12, 0);
    }
}

macro_rules! pool_constructor {
    ($address:literal, $export:ident, $body:ident, $label:literal,
     $exponent:literal, $last:literal, $count:literal, $stride:literal) => {
        rust_port!($address, $export, $body, $label, 0, (_this, r) {
            unsafe { initialize_pool(r, $exponent, $last, $count, $stride); }
        });
    };
}

pool_constructor!(
    0x005E_5670,
    fc4p_005e5670,
    initialize_eight_byte_pool,
    "initialize eight byte pool",
    11,
    0x7f8,
    254,
    8
);
pool_constructor!(
    0x005E_56D0,
    fc4p_005e56d0,
    initialize_32_byte_pool,
    "initialize 32 byte pool",
    12,
    0xfd0,
    127,
    32
);
pool_constructor!(
    0x005E_5730,
    fc4p_005e5730,
    initialize_64_byte_pool,
    "initialize 64 byte pool",
    13,
    0x1f90,
    127,
    64
);
pool_constructor!(
    0x005E_5790,
    fc4p_005e5790,
    initialize_128_byte_pool,
    "initialize 128 byte pool",
    14,
    0x3f10,
    127,
    128
);
pool_constructor!(
    0x005E_57D0,
    fc4p_005e57d0,
    initialize_256_byte_pool,
    "initialize 256 byte pool",
    15,
    0x7e10,
    127,
    256
);
pool_constructor!(
    0x005E_5830,
    fc4p_005e5830,
    initialize_512_byte_pool,
    "initialize 512 byte pool",
    16,
    0xfc10,
    127,
    512
);
pool_constructor!(
    0x005E_5870,
    fc4p_005e5870,
    initialize_1024_byte_pool,
    "initialize 1024 byte pool",
    16,
    0xf810,
    63,
    1024
);
pool_constructor!(
    0x005E_58B0,
    fc4p_005e58b0,
    initialize_2048_byte_pool,
    "initialize 2048 byte pool",
    16,
    0xf010,
    31,
    2048
);

macro_rules! pool_return {
    ($address:literal, $export:ident, $body:ident, $label:literal, $exponent:literal) => {
        rust_port!($address, $export, $body, $label, 4, (_this, r, block: u32) {
            r.eax = block;
            unsafe { invoke(r, crate::abi::target(0x0090_D91C), &[block, $exponent], true); }
        });
    };
}
pool_return!(
    0x005D_EB90,
    fc4p_005deb90,
    return_16k_pool_block,
    "return 16K pool block",
    14
);
pool_return!(
    0x005E_56B0,
    fc4p_005e56b0,
    return_2k_pool_block,
    "return 2K pool block",
    11
);
pool_return!(
    0x005E_5710,
    fc4p_005e5710,
    return_4k_pool_block,
    "return 4K pool block",
    12
);
pool_return!(
    0x005E_5810,
    fc4p_005e5810,
    return_32k_pool_block,
    "return 32K pool block",
    15
);

/// Return one slot to its block. Count zero discards the block, and the marked full-count transition
/// makes it available again. Null leaves EAX unchanged and clears EDX.
unsafe fn free_pool_slot(r: &mut Registers, slot: u32, mask: u32, full: u32) {
    r.edx = slot;
    if slot == 0 {
        return;
    }
    unsafe {
        r.eax = slot & mask;
        let block = r.eax as *mut u8;
        let head = word(block, 8);
        store(slot as *mut u8, 0, head);
        store(block, 8, slot);
        r.edx = word(block, 12).wrapping_sub(1);
        store(block, 12, r.edx);
        let target = if r.edx == 0 {
            0x0090_D031
        } else if r.edx == full {
            0x0090_CFF9
        } else {
            return;
        };
        // The forwarded argument replaces the original entry's argument word before the tail call.
        write_unaligned((r as *mut Registers).cast::<u8>().add(16).cast::<u32>(), r.eax);
        invoke(r, crate::abi::target(target), &[r.eax], false);
    }
}
macro_rules! pool_free {
    ($address:literal, $export:ident, $body:ident, $label:literal, $mask:literal, $full:literal) => {
        rust_port!($address, $export, $body, $label, 4, (_this, r, slot: u32) {
            unsafe { free_pool_slot(r, slot, $mask, $full); }
        });
    };
}
pool_free!(
    0x005E_5A40,
    fc4p_005e5a40,
    free_2k_pool_slot,
    "free 2K pool slot",
    0xffff_f800,
    0x4000_00be
);
pool_free!(
    0x005E_5A90,
    fc4p_005e5a90,
    free_4k_pool_slot,
    "free 4K pool slot",
    0xffff_f000,
    0x4000_005f
);
pool_free!(
    0x005E_5AE0,
    fc4p_005e5ae0,
    free_8k_pool_slot,
    "free 8K pool slot",
    0xffff_e000,
    0x4000_005f
);
pool_free!(
    0x005E_5B30,
    fc4p_005e5b30,
    free_16k_pool_slot,
    "free 16K pool slot",
    0xffff_c000,
    0x4000_005f
);
pool_free!(
    0x005E_5B80,
    fc4p_005e5b80,
    free_32k_pool_slot,
    "free 32K pool slot",
    0xffff_8000,
    0x4000_005f
);
pool_free!(
    0x005E_5BD0,
    fc4p_005e5bd0,
    free_64k_pool_slot,
    "free 64K pool slot",
    0xffff_0000,
    0x4000_005f
);
pool_free!(
    0x005E_5C20,
    fc4p_005e5c20,
    free_1024_byte_pool_slot,
    "free 1024 byte pool slot",
    0xffff_0000,
    0x4000_002f
);
pool_free!(
    0x005E_5C70,
    fc4p_005e5c70,
    free_2048_byte_pool_slot,
    "free 2048 byte pool slot",
    0xffff_0000,
    0x4000_0017
);

rust_port!(0x005F_D000, fc4p_005fd000, release_short_reference, "release unsigned short reference", 0,
    (this, r) {
        unsafe {
            r.eax = u32::from(read_unaligned(this.add(0x10).cast::<u16>()));
            if r.eax > 1 {
                r.eax -= 1;
                write_unaligned(this.add(0x10).cast::<u16>(), r.eax as u16);
            } else {
                r.eax = word(this, 4);
                r.ecx = this.wrapping_add(4) as u32;
                virtual_call(r, 0x1fc, &[1]);
                r.eax = 0;
            }
        }
    }
);
macro_rules! unsigned_release {
    ($address:literal, $export:ident, $body:ident, $label:literal, $offset:literal, $slot:literal) => {
        rust_port!($address, $export, $body, $label, 0, (this, r) {
            unsafe {
                r.eax = word(this, $offset);
                if r.eax > 1 { r.eax -= 1; store(this, $offset, r.eax); }
                else { r.eax = word(this, 0); virtual_call(r, $slot, &[1]); r.eax = 0; }
            }
        });
    };
}
unsigned_release!(
    0x005F_D370,
    fc4p_005fd370,
    release_property_holder,
    "release property holder",
    4,
    0x58
);
unsigned_release!(0x0062_3270, fc4p_00623270, release_window, "release window", 0x14, 0xcc);

macro_rules! cdecl_port {
    ($address:literal, $export:ident, $name:ident, $label:literal, $count:literal,
     ($r:ident $(, $arg:ident: $ty:ty)*) $body:block) => {
        port! {
            #[port(address = $address, owner = "shared", name = $label, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 12", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "mov edx, esp", ".rept {count}", "push dword ptr [esp + {offset}]", ".endr",
                    "call {body}", "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]",
                    "add esp, 12", "ret",
                    count = const $count, offset = const 12 + 4 * $count, body = sym $name,
                );
            }
        }
        unsafe extern "fastcall" fn $name(_this: *mut u8, $r: &mut Registers $(, $arg: $ty)*) $body
    };
}

cdecl_port!(0x0060_3DA0, fc4p_00603da0, set_grid_bit, "set packed grid bit", 4,
    (r, base: *mut u8, row: u32, column: u32, pitch: u32) {
        let index = row.wrapping_mul(pitch).wrapping_add(column) as i32 >> 5;
        r.eax = (base as u32).wrapping_add((index as u32).wrapping_mul(4));
        r.ecx = column & 31;
        r.edx = 1u32 << r.ecx;
        unsafe { store(r.eax as *mut u8, 0, word(r.eax as *mut u8, 0) | r.edx); }
    }
);

rust_port!(0x0060_C860, fc4p_0060c860, reset_cell_scan, "reset cell scan at coordinates", 8,
    (this, r, x: i32, z: i32) {
        unsafe {
            r.eax = x as u32;
            r.edx = z as u32;
            store(this, 4, r.eax);
            store(this, 8, r.edx);
            *this = 0;
            if x < word(this, 0x10) as i32 || z < word(this, 0x14) as i32
                || x > word(this, 0x18) as i32 || z > word(this, 0x1c) as i32 { *this = 2; }
        }
    }
);
rust_port!(0x0060_C890, fc4p_0060c890, set_cell_scan_bounds, "set cell scan bounds", 4,
    (this, r, source: *mut u8) {
        unsafe {
            for offset in [0,4,8,12] {
                let value = word(source, offset);
                store(this, 0x10 + offset, value);
                r.edx = value;
            }
            let low_x = word(this, 0x10) as i32;
            if (word(this, 0x18) as i32) < low_x { store(this, 0x18, low_x as u32); }
            r.eax = word(this, 0x14);
            if (word(this, 0x1c) as i32) < r.eax as i32 { store(this, 0x1c, r.eax); }
        }
    }
);
rust_port!(0x0060_C8D0, fc4p_0060c8d0, next_cell_scan, "next cell scan coordinates", 8,
    (this, r, out_x: *mut u8, out_z: *mut u8) {
        unsafe {
            let state = *this;
            r.eax = (r.eax & !255) | u32::from(state);
            if state == 2 { r.eax &= !255; return; }
            if state == 0 { *this = 1; }
            else {
                r.edx = word(this, 4);
                r.eax = word(this, 12);
                r.edx = r.edx.wrapping_add(r.eax);
                let max_x = word(this, 0x18) as i32;
                store(this, 4, r.edx);
                if r.edx as i32 > max_x {
                    let z = word(this, 8).wrapping_add(r.eax);
                    r.edx = word(this, 0x10);
                    store(this, 4, r.edx);
                    r.edx = word(this, 0x1c);
                    r.eax = z;
                    store(this, 8, z);
                    if z as i32 > r.edx as i32 { *this = 2; r.eax &= !255; return; }
                }
            }
            r.eax = word(this, 4);
            r.edx = out_x as u32;
            store(out_x, 0, r.eax);
            r.eax = word(this, 8);
            r.ecx = out_z as u32;
            store(out_z, 0, r.eax);
            r.eax = (r.eax & !255) | 1;
        }
    }
);

cdecl_port!(0x0061_4430, fc4p_00614430, copy_backward_twelve_bytes, "copy backward twelve byte records", 3,
    (r, begin: u32, end: u32, destination: u32) {
        r.ecx = end;
        let difference = end.wrapping_sub(begin) as i32;
        let high = ((i64::from(difference) * 0x2aaa_aaab) >> 32) as i32 >> 1;
        r.edx = high as u32;
        let count = high.wrapping_add((high as u32 >> 31) as i32);
        r.eax = destination;
        if count <= 0 { return; }
        r.edx = count as u32;
        while r.edx != 0 {
            r.ecx = r.ecx.wrapping_sub(12);
            r.eax = r.eax.wrapping_sub(12);
            r.edx -= 1;
            unsafe { for offset in [0,4,8] { store(r.eax as *mut u8, offset, word(r.ecx as *mut u8, offset)); } }
        }
    }
);

cdecl_port!(0x0062_3FA0, fc4p_00623fa0, copy_twelve_byte_records, "copy twelve byte records", 4,
    (r, begin: u32, end: u32, destination: u32, _context: u32) {
        r.ecx = begin;
        r.edx = end;
        r.eax = destination;
        while r.ecx != r.edx {
            if r.eax != 0 {
                unsafe { for offset in [0,4,8] { store(r.eax as *mut u8, offset, word(r.ecx as *mut u8, offset)); } }
            }
            r.ecx = r.ecx.wrapping_add(12);
            r.eax = r.eax.wrapping_add(12);
        }
    }
);
macro_rules! fill_records {
    ($address:literal, $export:ident, $body:ident, $label:literal, $size:literal) => {
        cdecl_port!($address, $export, $body, $label, 4,
            (r, destination: u32, count: u32, source: u32, _context: u32) {
                r.ecx = count;
                r.eax = destination;
                if count == 0 { return; }
                r.edx = source;
                while r.ecx != 0 {
                    if r.eax != 0 {
                        for offset in (0..$size).step_by(4) {
                            unsafe { store(r.eax as *mut u8, offset, word(source as *mut u8, offset)); }
                        }
                    }
                    r.eax = r.eax.wrapping_add($size as u32);
                    r.ecx -= 1;
                }
            }
        );
    };
}
fill_records!(
    0x0062_3FE0,
    fc4p_00623fe0,
    fill_twelve_byte_records,
    "fill twelve byte records",
    12
);
fill_records!(
    0x0062_4020,
    fc4p_00624020,
    fill_24_byte_records,
    "fill 24 byte records",
    24
);

cdecl_port!(0x0062_4E60, fc4p_00624e60, copy_padded_sixteen_byte_records, "copy padded sixteen byte records", 4,
    (r, begin: u32, end: u32, destination: u32, _context: u32) {
        r.ecx = begin;
        r.eax = destination;
        while r.ecx != end {
            unsafe {
                if r.eax != 0 {
                    r.edx = word(r.ecx as *mut u8, 0);
                    store(r.eax as *mut u8, 0, r.edx);
                    for offset in [4,5] {
                        r.edx = (r.edx & !255) | u32::from(*((r.ecx as *mut u8).add(offset)));
                        *((r.eax as *mut u8).add(offset)) = r.edx as u8;
                    }
                    for offset in [8,12] {
                        r.edx = word(r.ecx as *mut u8, offset);
                        store(r.eax as *mut u8, offset, r.edx);
                    }
                }
            }
            r.ecx = r.ecx.wrapping_add(16);
            r.eax = r.eax.wrapping_add(16);
        }
    }
);

rust_port!(0x0062_C8F0, fc4p_0062c8f0, has_pending_pointer, "pending pointer is nonnull", 0,
    (this, r) {
        r.edx = unsafe { word(this, 0x20c) };
        r.eax = u32::from(r.edx != 0);
    }
);
leaf!(
    0x0062_C910,
    fc4p_0062c910,
    "nested word at eight",
    "mov eax, [ecx + 0x358]",
    "mov eax, [eax + 8]",
    "ret"
);
leaf!(
    0x0062_C920,
    fc4p_0062c920,
    "nested word at sixteen",
    "mov eax, [ecx + 0x358]",
    "mov eax, [eax + 16]",
    "ret"
);
leaf!(
    0x0062_C930,
    fc4p_0062c930,
    "nested word at twenty",
    "mov eax, [ecx + 0x358]",
    "mov eax, [eax + 20]",
    "ret"
);

game_fn! {
    #[allow(dead_code)]
    fn assign_pair_vector(this: *mut u8, source: *mut u8) -> u32 = "thiscall" 0x0061_1B00;
}
rust_port!(0x0062_CA30, fc4p_0062ca30, copy_embedded_pair_vector, "copy embedded pair vector", 4,
    (this, r, destination: u32) {
        r.ecx = destination;
        unsafe { invoke(r, crate::abi::target(0x0061_1B00), &[(this as u32).wrapping_add(0x60)], false); }
    }
);

rust_port!(0x005F_B0E0, fc4p_005fb0e0, final_release_input_dispatch, "final release input dispatch", 0,
    (this, r) {
        unsafe {
            r.eax = word(this, 0);
            virtual_call(r, 0x10, &[]);
            r.edx = word(this, 0);
            r.ecx = this as u32;
            virtual_call(r, 0x68, &[1]);
        }
    }
);
game_fn! {
    #[allow(dead_code)]
    fn embedded_lookup(this: *mut u8, key: u32) -> u32 = "thiscall" 0x005B_9FC0;
}
rust_port!(0x005E_55B0, fc4p_005e55b0, lookup_with_retained_owner, "lookup with retained owner", 4,
    (this, r, key: u32) {
        unsafe {
            r.eax = word(this, 0);
            virtual_call(r, 4, &[]);
            r.ecx = this.wrapping_add(8) as u32;
            invoke(r, crate::abi::target(0x005B_9FC0), &[key], false);
            let result = r.eax;
            r.edx = word(this, 0);
            r.ecx = this as u32;
            virtual_call(r, 8, &[]);
            r.eax = result;
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn free_array_storage(p: *mut u8) -> u32 = "C" 0x0090_CF63;
}
game_fn! {
    #[allow(dead_code)]
    fn vertex_format_size(format: u32) -> u32 = "C" 0x008D_0370;
}
game_fn! {
    #[allow(dead_code)]
    fn allocate_array_storage(size: u32) -> *mut u8 = "C" 0x0090_CF54;
}
game_fn! {
    #[allow(dead_code)]
    fn grow_deque_map(this: *mut u8, front: u32, back: u32) -> u32 = "thiscall" 0x006D_B4A0;
}

rust_port!(0x005D_9430, fc4p_005d9430, release_registered_resource, "release registered resource", 0,
    (this, r) {
        unsafe {
            r.eax = word(this, 4);
            r.ecx = word(this, 8);
            if r.eax != 0 {
                if r.ecx != 0 {
                    r.eax = word(r.ecx as *mut u8, 0);
                    virtual_call(r, 0x0c, &[]);
                    let resource = r.eax;
                    r.ecx = word(this, 4);
                    r.edx = word(r.ecx as *mut u8, 0);
                    virtual_call(r, 0x60, &[resource]);
                    if r.eax as u8 != 0 {
                        r.eax = word(resource as *mut u8, 0);
                        r.ecx = resource;
                        virtual_call(r, 8, &[]);
                    }
                }
                r.ecx = word(this, 4);
                r.edx = word(r.ecx as *mut u8, 0);
                virtual_call(r, 8, &[]);
                store(this, 4, 0);
            } else if r.ecx != 0 {
                r.eax = word(r.ecx as *mut u8, 0);
                virtual_call(r, 8, &[]);
            }
            store(this, 8, 0);
        }
    }
);

cdecl_port!(0x005E_C430, fc4p_005ec430, fill_quad_triangle_indices, "fill quad triangle indices", 3,
    (r, base: u32, first: u32, last: u32) {
        let end = base.wrapping_add(last.wrapping_mul(2));
        r.eax = base.wrapping_add(first.wrapping_mul(2));
        r.ecx = base;
        r.edx = first.wrapping_mul(4) / 6;
        unsafe {
            while r.eax != end {
                for index in [r.edx, r.edx.wrapping_add(1), r.edx.wrapping_add(2),
                    r.edx, r.edx.wrapping_add(2), r.edx.wrapping_add(3)] {
                    write_unaligned(r.eax as *mut u16, index as u16);
                    r.eax = r.eax.wrapping_add(2);
                }
                r.ecx = r.edx.wrapping_add(3);
                r.edx = r.edx.wrapping_add(4);
            }
        }
    }
);

rust_port!(0x005E_C690, fc4p_005ec690, initialize_quad_batch, "initialize quad batch", 0,
    (this, r) {
        unsafe {
            store(this, 0, 0x00aa_44ac);
            store(this, 4, 6);
            store(this, 8, 4);
            store(this, 12, 0);
            store(this, 16, 10);
            for offset in (20..=60).step_by(4) { store(this, offset, 0); }
        }
        r.eax = this as u32;
        r.ecx = 0;
    }
);

rust_port!(0x005E_C6E0, fc4p_005ec6e0, destroy_quad_batch, "destroy quad batch", 0,
    (this, r) {
        unsafe {
            store(this, 0, 0x00aa_44ac);
            r.eax = word(this, 0x2c);
            if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
            let vertices = word(this, 0x14);
            if vertices != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[vertices], true); }
        }
    }
);

rust_port!(0x005E_C7C0, fc4p_005ec7c0, configure_quad_batch, "configure quad batch", 24,
    (this, r, primitive: u32, vertices: u32, flags: u32, format: u32, capacity: u32, context: u32) {
        unsafe {
            store(this, 4, primitive);
            store(this, 8, vertices);
            store(this, 12, flags);
            store(this, 16, format);
            r.eax = format;
            r.ecx = vertices;
            r.edx = flags;
            invoke(r, crate::abi::target(0x008D_0370), &[format], true);
            store(this, 0x20, r.eax);
            store(this, 0x28, capacity);
            store(this, 0x3c, context);
            store(this, 0x24, 0);
            store(this, 0x38, 0);
            r.ecx = word(this, 0x2c);
            store(this, 0x30, r.ecx);
            r.eax = word(this, 0x18);
            r.ecx = word(this, 0x14);
            store(this, 0x18, r.ecx);
        }
    }
);

rust_port!(0x0060_CB80, fc4p_0060cb80, set_lot_configuration, "set lot configuration", 4,
    (this, r, configuration: u32) {
        unsafe {
            let previous = word(this, 0x34);
            if configuration != previous {
                if configuration != 0 {
                    r.eax = word(configuration as *mut u8, 0);
                    r.ecx = configuration;
                    virtual_call(r, 4, &[]);
                }
                store(this, 0x34, configuration);
                if previous != 0 {
                    r.edx = word(previous as *mut u8, 0);
                    r.ecx = previous;
                    virtual_call(r, 8, &[]);
                }
            }
        }
    }
);

rust_port!(0x0062_3290, fc4p_00623290, query_window_interface, "query window interface", 8,
    (this, r, iid: u32, out: *mut u8) {
        r.eax = iid.wrapping_sub(1);
        let primary = r.eax == 0 || { r.eax = r.eax.wrapping_sub(0x0a74_ca14); r.eax == 0 };
        if !primary {
            r.eax = r.eax.wrapping_sub(0x3aad_ca95);
            if r.eax != 0 { r.eax &= !255; return; }
        }
        unsafe {
            r.edx = out as u32;
            let interface = if primary || this.is_null() { this as u32 } else { (this as u32).wrapping_add(4) };
            store(out, 0, interface);
            r.eax = word(this, 0);
            virtual_call(r, 4, &[]);
            r.eax = (r.eax & !255) | 1;
        }
    }
);

rust_port!(0x0062_4F70, fc4p_00624f70, destroy_window_arrays, "destroy window arrays", 0,
    (this, r) {
        unsafe {
            for offset in [0xa0, 0x6c, 0x60, 0x54] {
                r.eax = word(this, offset);
                if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
            }
            let last = word(this, 0x28);
            if last != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[last], true); }
        }
    }
);

rust_port!(0x0062_66E0, fc4p_006266e0, append_deque_block, "append deque block", 4,
    (this, r, value: *mut u8) {
        unsafe {
            r.eax = value as u32;
            r.ecx = word(this, 0x1c);
            r.edx = word(this, 0x24);
            let element = word(value, 0);
            r.ecx = (r.ecx.wrapping_sub(word(this, 0x20)) as i32 >> 2) as u32;
            r.edx = r.edx.wrapping_sub(r.ecx);
            if r.edx < 2 {
                r.ecx = this as u32;
                invoke(r, crate::abi::target(0x006D_B4A0), &[1, 0], false);
            }
            invoke(r, crate::abi::target(0x0090_CF54), &[0x80], true);
            r.ecx = word(this, 0x1c);
            store(r.ecx as *mut u8, 4, r.eax);
            r.eax = word(this, 0x10);
            if r.eax != 0 { store(r.eax as *mut u8, 0, element); }
            r.eax = word(this, 0x1c).wrapping_add(4);
            store(this, 0x1c, r.eax);
            r.eax = word(r.eax as *mut u8, 0);
            store(this, 0x14, r.eax);
            r.eax = r.eax.wrapping_add(0x80);
            store(this, 0x18, r.eax);
            r.edx = word(this, 0x14);
            store(this, 0x10, r.edx);
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn resize_pair_vector(this: *mut u8, count: u32) -> u32 = "thiscall" 0x0051_FC30;
}
game_fn! {
    #[allow(dead_code)]
    fn destroy_variant(this: *mut u8) -> u32 = "thiscall" 0x0091_5EB5;
}
game_fn! {
    #[allow(dead_code)]
    fn construct_property_pairs(first: u32, last: u32, out: u32, tag: *mut u32) -> u32 = "C" 0x0077_AC20;
}
game_fn! {
    #[allow(dead_code)]
    fn shift_property_pairs(first: u32, last: u32, out: u32, tag: *mut u32, unused: u32) -> u32 = "C" 0x0077_A120;
}
game_fn! {
    #[allow(dead_code)]
    fn destroy_string_tree(this: *mut u8, node: u32) -> u32 = "thiscall" 0x0062_2190;
}
game_data! { static pooled_variant_manager: u8 = 0x00B0_A9E8; }

rust_port!(0x005F_4760, fc4p_005f4760, copy_record_pairs, "copy record pairs", 4,
    (this, r, destination: *mut u8) {
        unsafe {
            r.eax = word(this, 12);
            r.ecx = (word(destination, 4).wrapping_sub(word(destination, 0)) as i32 >> 3) as u32;
            if r.ecx != r.eax {
                r.ecx = destination as u32;
                invoke(r, crate::abi::target(0x0051_FC30), &[r.eax], false);
            }
            r.eax = word(this, 12);
            r.ecx = 0;
            if r.eax as i32 > 0 {
                r.edx = 0;
                loop {
                    r.eax = word(this, 0);
                    let first = word(r.eax as *mut u8, r.edx as usize);
                    let out = word(destination, 0) as *mut u8;
                    r.eax = r.eax.wrapping_add(r.edx);
                    store(out, r.ecx.wrapping_mul(8) as usize, first);
                    r.eax = word(r.eax as *mut u8, 4);
                    let out = word(destination, 0) as *mut u8;
                    store(out, r.ecx.wrapping_mul(8).wrapping_add(4) as usize, r.eax);
                    r.eax = word(this, 12);
                    r.ecx = r.ecx.wrapping_add(1);
                    r.edx = r.edx.wrapping_add(20);
                    if r.ecx as i32 >= r.eax as i32 { break; }
                }
            }
        }
    }
);

rust_port!(0x005F_E980, fc4p_005fe980, delete_pooled_variant, "delete pooled variant", 4,
    (this, r, flags: u32) {
        unsafe {
            r.ecx = (this as u32).wrapping_add(4);
            store(this, 0, 0x00aa_6988);
            store(this, 4, 0x00aa_6788);
            invoke(r, crate::abi::target(0x0091_5EB5), &[], false);
            r.eax = this as u32;
            if flags & 1 != 0 {
                r.eax &= 0xffff_f800;
                let block = r.eax as *mut u8;
                r.ecx = word(block, 8);
                store(this, 0, r.ecx);
                r.ecx = word(block, 12).wrapping_sub(1);
                store(block, 8, this as u32);
                store(block, 12, r.ecx);
                if r.ecx == 0 || r.ecx == 0x4000_0036 {
                    let target = if r.ecx == 0 { 0x0090_D031 } else { 0x0090_CFF9 };
                    r.ecx = pooled_variant_manager() as u32;
                    invoke(r, crate::abi::target(target), &[r.eax], false);
                }
                r.eax = this as u32;
            }
        }
    }
);

rust_port!(0x005F_EA70, fc4p_005fea70, copy_property_pair_vector, "copy property pair vector", 4,
    (this, r, source: *mut u8) {
        unsafe {
            let count = (word(source, 4).wrapping_sub(word(source, 0)) as i32 >> 3) as u32;
            r.eax = 0;
            store(this, 0, 0);
            store(this, 4, 0);
            store(this, 8, 0);
            if count != 0 {
                r.eax = count.wrapping_mul(8);
                invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], true);
            }
            let allocation = r.eax;
            r.edx = (r as *mut Registers).cast::<u8>().add(16) as u32;
            store(this, 0, allocation);
            store(this, 4, allocation);
            store(this, 8, allocation.wrapping_add(count.wrapping_mul(8)));
            r.eax = word(source, 4);
            r.ecx = word(source, 0);
            invoke(r, crate::abi::target(0x0077_AC20), &[r.ecx, r.eax, allocation, r.edx], true);
            store(this, 4, r.eax);
            r.eax = this as u32;
        }
    }
);

rust_port!(0x005F_EAD0, fc4p_005fead0, erase_property_pair, "erase property pair", 4,
    (this, r, erased: u32) {
        unsafe {
            r.eax = word(this, 4);
            r.ecx = erased.wrapping_add(8);
            if r.ecx != r.eax {
                r.edx = (r as *mut Registers).cast::<u8>().add(16) as u32;
                invoke(r, crate::abi::target(0x0077_A120), &[r.ecx, r.eax, erased, r.edx, 0], true);
            }
            r.ecx = word(this, 4).wrapping_sub(8);
            store(this, 4, r.ecx);
            r.ecx = word(r.ecx as *mut u8, 4);
            if r.ecx != 0 {
                r.eax = word(r.ecx as *mut u8, 0);
                virtual_call(r, 8, &[]);
            }
            r.eax = erased;
        }
    }
);

rust_port!(0x0062_2190, fc4p_00622190, destroy_string_tree_nodes, "destroy string tree nodes", 4,
    (this, r, root: u32) {
        let mut node = root;
        unsafe {
            while node != 0 {
                let p = node as *mut u8;
                r.eax = word(p, 12);
                r.ecx = this as u32;
                invoke(r, crate::abi::target(0x0062_2190), &[r.eax], false);
                r.eax = word(p, 0x24);
                let left = word(p, 8);
                if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
                r.eax = word(p, 0x18);
                if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
                invoke(r, crate::abi::target(0x0090_CF63), &[node], true);
                node = left;
            }
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn copy_grid_cells(destination: u32, source: u32, context: u32) -> u32 = "C" 0x005E_4FA0;
}
game_fn! {
    #[allow(dead_code)]
    fn reduce_grid_cells(storage: u32, width: u32, height: u32) -> u32 = "C" 0x005E_4D60;
}
game_fn! {
    #[allow(dead_code)]
    fn assign_string_range(this: *mut u8, first: u32, last: u32) -> u32 = "thiscall" 0x0044_D3A0;
}
game_fn! {
    #[allow(dead_code)]
    fn query_variant_interface(this: *mut u8, iid: u32, out: u32) -> u32 = "thiscall" 0x0091_4E56;
}
game_fn! {
    #[allow(dead_code)]
    fn allocate_object_storage(size: u32) -> *mut u8 = "C" 0x005E_55E0;
}
game_fn! {
    #[allow(dead_code)]
    fn construct_window_proxy(this: *mut u8) -> u32 = "thiscall" 0x0064_5D50;
}
game_fn! {
    #[allow(dead_code)]
    fn bind_window_proxy(this: *mut u8, owner: u32, context: u32) -> u32 = "thiscall" 0x0064_5240;
}
game_fn! {
    #[allow(dead_code)]
    fn construct_message_target(this: *mut u8) -> u32 = "thiscall" 0x005E_A080;
}
game_fn! {
    #[allow(dead_code)]
    fn allocate_pair_copy(this: *mut u8, count: u32, first: u32, last: u32) -> u32 = "thiscall" 0x007C_EC30;
}
game_fn! {
    #[allow(dead_code)]
    fn assign_pairs(first: u32, last: u32, out: u32, tag: *mut u32) -> u32 = "C" 0x0046_3DF0;
}
game_fn! {
    #[allow(dead_code)]
    fn construct_pairs_tail(first: u32, last: u32, out: u32, tag: *mut u32) -> u32 = "C" 0x0063_BFF0;
}
game_data! { static window_message_server: *mut u8 = 0x00B4_3D00; }

cdecl_port!(0x005E_5130, fc4p_005e5130, copy_and_reduce_grid, "copy and reduce grid", 3,
    (r, destination: u32, source: u32, context: u32) {
        r.eax = source;
        if source == 0 || destination == 0 { return; }
        unsafe {
            r.ecx = context;
            invoke(r, crate::abi::target(0x005E_4FA0), &[destination, source, context], true);
            r.edx = word(destination as *mut u8, 0);
            r.ecx = destination;
            virtual_call(r, 0x34, &[]);
            let height = r.eax;
            r.eax = word(destination as *mut u8, 0);
            r.ecx = destination;
            virtual_call(r, 0x30, &[]);
            let width = r.eax;
            r.edx = word(destination as *mut u8, 0);
            r.ecx = destination;
            virtual_call(r, 0x60, &[]);
            invoke(r, crate::abi::target(0x005E_4D60), &[r.eax, width, height], true);
        }
    }
);

rust_port!(0x005F_A540, fc4p_005fa540, assign_string_bytes, "assign string bytes", 8,
    (_this, r, first: u32, count: u32) {
        r.eax = first;
        r.edx = count.wrapping_add(first);
        unsafe {
            // The range adapter replaces the caller's count word with the end pointer.
            write_unaligned((r as *mut Registers).cast::<u8>().add(20).cast::<u32>(), r.edx);
            invoke(r, crate::abi::target(0x0044_D3A0), &[first, r.edx], false);
        }
    }
);

rust_port!(0x005F_D030, fc4p_005fd030, query_property_interface, "query property interface", 8,
    (_this, r, iid: u32, out: *mut u8) {
        r.edx = iid;
        r.eax = iid.wrapping_sub(1);
        if r.eax != 0 { r.eax = r.eax.wrapping_sub(0x4521_60eb); }
        unsafe {
            if r.eax != 0 {
                r.ecx = r.ecx.wrapping_add(4);
                invoke(r, crate::abi::target(0x0091_4E56), &[iid, out as u32], false);
            } else {
                r.edx = out as u32;
                store(out, 0, r.ecx);
                r.eax = word(r.ecx as *mut u8, 0);
                virtual_call(r, 4, &[]);
                r.eax = (r.eax & !255) | 1;
            }
        }
    }
);

rust_port!(0x0061_1B00, fc4p_00611b00, assign_eight_byte_vector, "assign pair vector", 4,
    (this, r, source: *mut u8) {
        unsafe {
            if source != this {
                r.eax = word(source, 0);
                r.edx = word(this, 0);
                r.ecx = word(this, 8).wrapping_sub(r.edx);
                let last = word(source, 4);
                let count = (last.wrapping_sub(r.eax) as i32 >> 3) as u32;
                r.ecx = (r.ecx as i32 >> 3) as u32;
                if count > r.ecx {
                    r.ecx = this as u32;
                    invoke(r, crate::abi::target(0x007C_EC30), &[count, r.eax, last], false);
                    let allocation = r.eax;
                    r.eax = word(this, 0);
                    if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
                    r.edx = allocation.wrapping_add(count.wrapping_mul(8));
                    r.ecx = r.edx;
                    store(this, 0, allocation);
                    store(this, 8, r.edx);
                    store(this, 4, r.ecx);
                } else {
                    r.ecx = (word(this, 4).wrapping_sub(r.edx) as i32 >> 3) as u32;
                    // The allocator tag shares the argument word used by both range helpers.
                    let tag = (r as *mut Registers).cast::<u8>().add(16) as u32;
                    if r.ecx >= count {
                        r.ecx = tag;
                        invoke(r, crate::abi::target(0x0046_3DF0), &[r.eax, last, r.edx, tag], true);
                    } else {
                        let out = r.edx;
                        r.edx = r.eax.wrapping_add(r.ecx.wrapping_mul(8));
                        invoke(r, crate::abi::target(0x0046_3DF0), &[r.eax, r.edx, out, tag], true);
                        r.eax = word(this, 4);
                        let begin = word(this, 0);
                        r.edx = word(source, 4);
                        r.ecx = word(source, 0);
                        r.eax = (r.eax.wrapping_sub(begin) as i32 >> 3) as u32;
                        r.edx = r.ecx.wrapping_add(r.eax.wrapping_mul(8));
                        let out = word(this, 4);
                        invoke(r, crate::abi::target(0x0063_BFF0), &[r.edx, word(source, 4), out, tag], true);
                    }
                    r.eax = word(this, 0);
                    r.ecx = r.eax.wrapping_add(count.wrapping_mul(8));
                    store(this, 4, r.ecx);
                }
            }
            r.eax = this as u32;
        }
    }
);

rust_port!(0x0062_3B20, fc4p_00623b20, create_window_proxy, "create window proxy", 4,
    (this, r, out: *mut u8) {
        unsafe {
            invoke(r, crate::abi::target(0x005E_55E0), &[0x134], true);
            let proxy = if r.eax == 0 { 0 } else {
                r.ecx = r.eax;
                invoke(r, crate::abi::target(0x0064_5D50), &[], false);
                r.eax
            };
            r.eax = word(proxy as *mut u8, 0);
            r.ecx = proxy;
            virtual_call(r, 4, &[]);
            r.ecx = word(this, 0x35c);
            let context = r.ecx;
            r.ecx = proxy;
            invoke(r, crate::abi::target(0x0064_5240), &[this as u32, context], false);
            r.edx = out as u32;
            store(out, 0, proxy);
            r.eax = (r.eax & !255) | 1;
        }
    }
);

rust_port!(0x0062_4920, fc4p_00624920, close_window_with_notification, "close window with notification", 0,
    (this, r) {
        unsafe {
            r.eax = word(this, 0x210);
            if r.eax != 0 {
                let server = *window_message_server() as u32;
                invoke(r, crate::abi::target(0x005E_55E0), &[12], true);
                let target = r.eax;
                if target != 0 {
                    r.ecx = target;
                    invoke(r, crate::abi::target(0x005E_A080), &[], false);
                    store(target as *mut u8, 0, 0x00aa_825c);
                    store(target as *mut u8, 4, 0x00aa_8244);
                }
                r.ecx = word(this, 0x210);
                let destination = r.ecx;
                r.eax = word(server as *mut u8, 0);
                r.ecx = server;
                virtual_call(r, 0x18, &[1, destination, 1, 0x2001, 0, target, 0, 0, 0, 0, 0]);
            }
            r.edx = word(this, 0);
            r.ecx = this as u32;
            virtual_call(r, 0x44, &[]);
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn insert_string_range(this: *mut u8, at: u32, first: u32, last: u32, tag: *mut u32) -> u32 = "thiscall" 0x005C_2470;
}
game_data! { static center_scale: f32 = 0x00A8_4D2C; }

rust_port!(0x005C_2910, fc4p_005c2910, insert_string_range_adapter, "insert string range adapter", 12,
    (_this, r, at: u32, first: u32, last: u32) {
        unsafe {
            let tag = (r as *mut Registers).cast::<u8>().add(24) as u32;
            r.eax = first;
            r.edx = at;
            invoke(r, crate::abi::target(0x005C_2470), &[at, first, last, tag], false);
        }
    }
);

/// Add and multiply stored floats without rounding either intermediate to a stored float.
unsafe fn scaled_sum(first: *const f32, second: *const f32) -> crate::x87::F80 {
    let mut value = crate::x87::F80::ZERO.to_bytes();
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{first}]", "fadd dword ptr [{second}]",
            "fmul dword ptr [{scale}]", "fstp tbyte ptr [{value}]",
            first = in(reg) first, second = in(reg) second, scale = in(reg) center_scale(),
            value = in(reg) value.as_mut_ptr(), out("st(0)") _,
        );
    }
    crate::x87::F80::from_bytes(value)
}

rust_port!(0x005E_D1F0, fc4p_005ed1f0, get_horizontal_bounds_center, "horizontal bounds center", 4,
    (this, r, out: *mut u8) {
        unsafe {
            let x = scaled_sum(this.wrapping_add(0x40).cast(), this.wrapping_add(0x34).cast()).to_f32();
            store(out, 0, x.to_bits());
            r.edx = word(this, 0x38);
            store(out, 4, r.edx);
            let z = scaled_sum(this.wrapping_add(0x48).cast(), this.wrapping_add(0x3c).cast()).to_f32();
            store(out, 8, z.to_bits());
            r.eax = out as u32;
        }
    }
);

layout! {
    /// The pointer triple shared by byte strings and contiguous vectors.
    struct ByteRangeBuffer {
        0x00 => begin / set_begin: u32,
        0x04 => end / set_end: u32,
        0x08 => limit / set_limit: u32,
    }
}
layout! {
    /// Window-owned auxiliary storage and its ordered child tree.
    struct WindowCleanup {
        0x210 => pool / set_pool: u32,
        0x214 => sentinel / set_sentinel: u32,
        0x218 => count / set_count: u32,
    }
}
layout! {
    /// Tree links and the child window stored in a node.
    struct WindowTreeLinks {
        0x04 => parent / set_parent: u32,
        0x08 => left / set_left: u32,
        0x0C => right / set_right: u32,
        0x10 => window / set_window: u32,
    }
}
layout! {
    /// Pool interface table, block count and block-pointer array.
    struct WindowPool {
        0x10 => table / set_table: u32,
        0x14 => count / set_count: u32,
        0x20 => blocks / set_blocks: u32,
    }
}

game_fn! {
    #[allow(dead_code)]
    fn find_byte_sequence(first: u32, last: u32, needle: u32, needle_end: u32, tag: u32) -> u32 = "C" 0x0044_26C0;
}
game_fn! {
    #[allow(dead_code)]
    fn find_last_byte_sequence(first: u32, last: u32, needle: u32, needle_end: u32, tag1: u32, tag2: u32, tag3: u32) -> u32 = "C" 0x005C_2380;
}
game_fn! {
    #[allow(dead_code)]
    fn find_byte_outside_set(first: u32, last: u32, set: u32, set_end: u32, tag: u32) -> u32 = "C" 0x005C_6060;
}

/// The caller's actual argument words, including words passed onward by address.
fn argument_words(r: &mut Registers) -> *mut u8 {
    (r as *mut Registers).cast::<u8>().wrapping_add(16)
}

cdecl_port!(0x005C_2380, fc4p_005c2380, find_last_subsequence, "find last byte subsequence", 7,
    (r, first: u32, last: u32, needle: u32, needle_end: u32, _tag1: u32, _tag2: u32, tag: u32) {
        if needle == needle_end { r.eax = last; return; }
        r.eax = tag;
        r.ecx = first;
        let mut found = last;
        unsafe {
            invoke(r, crate::abi::target(0x0044_26C0), &[first, last, needle, needle_end, tag], true);
            while r.eax != last {
                found = r.eax;
                r.edx = word(argument_words(r), 24);
                r.eax = r.eax.wrapping_add(1);
                invoke(r, crate::abi::target(0x0044_26C0), &[r.eax, last, needle, needle_end, r.edx], true);
            }
        }
        r.eax = found;
    }
);

rust_port!(0x005C_23E0, fc4p_005c23e0, string_reverse_find, "string reverse find range", 12,
    (this, r, needle: u32, position: u32, length: u32) {
        unsafe {
            let buffer = ByteRangeBuffer::from_ptr(this);
            let begin = buffer.begin();
            let size = buffer.end().wrapping_sub(begin);
            r.eax = if length > size { u32::MAX }
                else if length == 0 { position.min(size) }
                else {
                    let remaining = size.wrapping_sub(length);
                    let args = argument_words(r);
                    store(args, 8, remaining);
                    let end = begin.wrapping_add(position.min(remaining)).wrapping_add(length);
                    let tag = args.wrapping_add(4) as u32;
                    r.edx = tag;
                    r.eax = needle;
                    r.ecx = needle.wrapping_add(length);
                    invoke(r, crate::abi::target(0x005C_2380),
                        &[begin, end, needle, r.ecx, tag, tag, position], true);
                    if r.eax == end { u32::MAX } else { r.eax.wrapping_sub(begin) }
                };
            r.ecx = size;
        }
    }
);

rust_port!(0x005C_6610, fc4p_005c6610, string_find_first_outside_set, "string find first byte outside set", 8,
    (this, r, set: u32, position: u32) {
        unsafe {
            let mut end = set;
            loop {
                let byte = read_unaligned(end as *const u8);
                r.edx = (r.edx & !255) | u32::from(byte);
                end = end.wrapping_add(1);
                if byte == 0 { break; }
            }
            let buffer = ByteRangeBuffer::from_ptr(this);
            let begin = buffer.begin();
            let last = buffer.end();
            r.eax = end.wrapping_sub(set.wrapping_add(1));
            r.ecx = position;
            r.edx = last.wrapping_sub(begin);
            if position > r.edx { r.eax = u32::MAX; return; }
            r.edx = argument_words(r) as u32;
            let set_end = set.wrapping_add(r.eax);
            r.eax = begin.wrapping_add(position);
            invoke(r, crate::abi::target(0x005C_6060), &[r.eax, last, set, set_end, r.edx], true);
            r.eax = if r.eax == last { u32::MAX } else { r.eax.wrapping_sub(begin) };
        }
    }
);

/// Compare a stored bound against a stored coordinate, using one x87 stack slot. The coordinate stays a
/// memory operand, so signaling NaNs and denormals raise the same status flags as other coordinates.
unsafe fn compare_bound(bound: *const u8, coordinate: *const u8) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!("fld dword ptr [{bound}]", "fcomp dword ptr [{coordinate}]", "fnstsw ax",
            bound = in(reg) bound, coordinate = in(reg) coordinate, out("ax") status, out("st(0)") _);
    }
    status
}

cdecl_port!(0x005D_6BA0, fc4p_005d6ba0, bound_two_dimensional_points, "bound two dimensional points", 4,
    (r, points: *mut u8, count: i32, low: *mut u8, high: *mut u8) {
        unsafe {
            r.edx = low as u32;
            r.eax = word(points, 0); store(low, 0, r.eax);
            r.ecx = word(points, 4); store(low, 4, r.ecx);
            r.eax = word(points, 0); store(high, 0, r.eax);
            r.ecx = word(points, 4); store(high, 4, r.ecx);
            r.eax = count.wrapping_sub(1) as u32;
            // Groups of four use the same comparison order as the final one-to-three points.
            let grouped = if count.wrapping_sub(1) >= 4 {
                ((count.wrapping_sub(5) as u32 >> 2).wrapping_add(1)).wrapping_mul(4).wrapping_add(1)
            } else { 1 };
            let mut index = 1u32;
            while index < grouped || (index as i32) < count {
                let point = points.wrapping_add(index.wrapping_mul(8) as usize);
                if index < grouped { r.ecx = points.wrapping_add(((index - 1) / 4 * 4 + 2).wrapping_mul(8) as usize) as u32; }
                for (bound, offset, minimum) in [(low, 0, true), (low, 4, true), (high, 0, false), (high, 4, false)] {
                    let status = compare_bound(bound.wrapping_add(offset), point.wrapping_add(offset));
                    r.eax = (r.eax & 0xffff_0000) | u32::from(status);
                    let replace = if minimum { status & 0x4100 == 0 } else { status & 0x0500 == 0x0100 };
                    if replace {
                        let bits = word(point, offset);
                        // The last group loads X into ECX rather than EAX.
                        if index >= grouped && offset == 0 { r.ecx = bits; } else { r.eax = bits; }
                        store(bound, offset, bits);
                    }
                }
                if index < grouped && index & 3 == 0 { r.ecx = r.ecx.wrapping_add(32); }
                index = index.wrapping_add(1);
                if index > grouped { r.eax = count as u32; }
            }
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn fill_flagged_records(out: u32, count: u32, value: u32, tag: u32) -> u32 = "C" 0x0062_1550;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_flagged_records(first: u32, last: u32, out: u32, tag: u32) -> u32 = "C" 0x0062_1580;
}
game_fn! {
    #[allow(dead_code)]
    fn fill_six_byte_records(out: u32, count: u32, value: u32, tag: u32) -> u32 = "C" 0x0062_1150;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_six_byte_records(first: u32, last: u32, out: u32, tag: u32) -> u32 = "C" 0x0062_1110;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_three_word_records(first: u32, last: u32, out: u32, tag: u32) -> u32 = "C" 0x0062_3FA0;
}
game_fn! {
    #[allow(dead_code)]
    fn fill_three_word_records(out: u32, count: u32, value: u32, tag: u32) -> u32 = "C" 0x0062_3FE0;
}

/// Construct only an element's fields, in their original order. Padding retains the allocation's bytes.
unsafe fn construct_flagged_record(r: &mut Registers, out: u32, source: u32, narrow: bool, single: bool) {
    if out == 0 {
        return;
    }
    unsafe {
        r.edx = (r.edx & !255)
            | u32::from(read_unaligned(
                (source.wrapping_add(if narrow { 4 } else { 0 })) as *const u8,
            ));
        write_unaligned((out.wrapping_add(if narrow { 4 } else { 0 })) as *mut u8, r.edx as u8);
        r.edx = word(source as *mut u8, if narrow { 0 } else { 4 });
        store(out as *mut u8, if narrow { 0 } else { 4 }, r.edx);
        if narrow {
            let byte = read_unaligned(source.wrapping_add(5) as *const u8);
            if single {
                r.ecx = (r.ecx & !255) | u32::from(byte);
            } else {
                r.edx = (r.edx & !255) | u32::from(byte);
            }
            write_unaligned(out.wrapping_add(5) as *mut u8, byte);
        } else {
            let value = word(source as *mut u8, 8);
            if single {
                r.ecx = value;
            } else {
                r.edx = value;
            }
            store(out as *mut u8, 8, value);
        }
    }
}

/// Grow vectors whose elements have a flag and payload. Count and flag arguments remain at their
/// caller-owned addresses throughout the fill and tail-copy calls.
unsafe fn grow_flagged_vector(this: *mut u8, r: &mut Registers, at: u32, value: u32, count: u32, narrow: bool) {
    unsafe {
        let stride = if narrow { 8u32 } else { 12u32 };
        let delta = word(this, 4).wrapping_sub(word(this, 0)) as i32;
        let size = if narrow { delta >> 3 } else { delta / 12 } as u32;
        let capacity = size.wrapping_add(size.max(count));
        let args = argument_words(r);
        let tag = args.wrapping_add(16) as u32;
        r.eax = size;
        // The entry's multiply leaves its high half in EDX before allocation.
        if !narrow {
            r.edx = (((i64::from(delta) * 0x2aaa_aaab) >> 32) as i32 >> 1) as u32;
        }
        r.ecx = capacity;
        if narrow {
            store(args, 12, capacity);
        }
        let allocation = if capacity == 0 {
            0
        } else {
            r.eax = capacity.wrapping_mul(stride);
            invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], true);
            r.eax
        };
        if !narrow {
            store(args, 12, allocation);
        }
        r.ecx = word(this, 0);
        r.eax = allocation;
        while r.ecx != at {
            construct_flagged_record(r, r.eax, r.ecx, narrow, false);
            r.ecx = r.ecx.wrapping_add(stride);
            r.eax = r.eax.wrapping_add(stride);
        }
        let mut end = if count == 1 {
            if r.eax != 0 {
                r.ecx = value;
                construct_flagged_record(r, r.eax, value, narrow, true);
            }
            r.eax.wrapping_add(stride)
        } else {
            r.ecx = value;
            r.edx = tag;
            invoke(
                r,
                crate::abi::target(if narrow { 0x0062_1150 } else { 0x0062_1550 }),
                &[r.eax, count, value, tag],
                true,
            );
            r.eax
        };
        let skip_tail = read_unaligned(tag as *const u8);
        r.eax = (r.eax & !255) | u32::from(skip_tail);
        if skip_tail == 0 {
            r.eax = word(this, 4);
            r.edx = tag;
            invoke(
                r,
                crate::abi::target(if narrow { 0x0062_1110 } else { 0x0062_1580 }),
                &[at, r.eax, end, tag],
                true,
            );
            end = r.eax;
        }
        r.eax = word(this, 0);
        if r.eax != 0 {
            invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true);
        }
        // Eight-byte vectors keep the free result in EAX; twelve-byte vectors return the allocation.
        if !narrow {
            r.eax = word(args, 12);
        }
        let capacity = if narrow { word(args, 12) } else { capacity };
        r.edx = allocation.wrapping_add(capacity.wrapping_mul(stride));
        if narrow {
            store(this, 0, allocation);
            store(this, 4, end);
            store(this, 8, r.edx);
        } else {
            store(this, 4, end);
            store(this, 0, r.eax);
            store(this, 8, r.edx);
        }
        r.ecx = size;
    }
}

rust_port!(0x0062_1A40, fc4p_00621a40, grow_twelve_byte_flagged_vector, "grow twelve byte flagged vector", 20,
    (this, r, at: u32, value: u32, _unused: u32, count: u32, _skip_tail: u32) {
        unsafe { grow_flagged_vector(this, r, at, value, count, false); }
    }
);
rust_port!(0x0062_1B40, fc4p_00621b40, grow_eight_byte_flagged_vector, "grow eight byte flagged vector", 20,
    (this, r, at: u32, value: u32, _unused: u32, count: u32, _skip_tail: u32) {
        unsafe { grow_flagged_vector(this, r, at, value, count, true); }
    }
);

rust_port!(0x0062_6240, fc4p_00626240, grow_three_word_vector, "grow three word vector", 20,
    (this, r, at: u32, value: u32, _unused: u32, count: u32, _skip_tail: u32) {
        unsafe {
            let delta = word(this, 4).wrapping_sub(word(this, 0)) as i32;
            let size = (delta / 12) as u32;
            let capacity = size.wrapping_add(size.max(count));
            let tag = argument_words(r).wrapping_add(16) as u32;
            r.edx = (((i64::from(delta) * 0x2aaa_aaab) >> 32) as i32 >> 1) as u32;
            r.ecx = count;
            r.eax = size;
            let allocation = if capacity == 0 { 0 } else {
                r.eax = capacity.wrapping_mul(12);
                invoke(r, crate::abi::target(0x0090_CF54), &[r.eax], true);
                r.eax
            };
            r.edx = at;
            r.eax = word(this, 0);
            r.ecx = tag;
            invoke(r, crate::abi::target(0x0062_3FA0), &[r.eax, at, allocation, tag], true);
            let mut end = r.eax;
            r.eax = word(argument_words(r), 12);
            if r.eax == 1 {
                if end != 0 {
                    r.ecx = value;
                    r.eax = word(value as *mut u8, 0); r.edx = end; store(end as *mut u8, 0, r.eax);
                    r.eax = word(value as *mut u8, 4); store(end as *mut u8, 4, r.eax);
                    r.ecx = word(value as *mut u8, 8); store(end as *mut u8, 8, r.ecx);
                }
                end = end.wrapping_add(12);
            } else {
                r.ecx = value; r.edx = tag;
                invoke(r, crate::abi::target(0x0062_3FE0), &[end, r.eax, value, tag], true);
                end = r.eax;
            }
            let skip = read_unaligned(tag as *const u8);
            r.eax = (r.eax & !255) | u32::from(skip);
            if skip == 0 {
                r.eax = word(this, 4); r.ecx = at; r.edx = tag;
                invoke(r, crate::abi::target(0x0062_3FA0), &[at, r.eax, end, tag], true);
                end = r.eax;
            }
            r.eax = word(this, 0);
            if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
            r.edx = capacity.wrapping_mul(3);
            store(this, 4, end);
            r.eax = allocation.wrapping_add(r.edx.wrapping_mul(4));
            store(this, 0, allocation); store(this, 8, r.eax);
            r.ecx = size;
        }
    }
);

/// Each grid row is a twelve-byte vector whose first word points to its bytes.
unsafe fn byte_grid_row(storage: u32, row: u32) -> u32 {
    unsafe { word(word(storage as *mut u8, 0) as *mut u8, row.wrapping_mul(12) as usize) }
}

cdecl_port!(0x005E_4D60, fc4p_005e4d60, relax_byte_grid, "relax byte grid in four directions", 3,
    (r, storage: u32, width: i32, height: i32) {
        let entry_ecx = r.ecx;
        unsafe {
            loop {
                let mut changed = 0u32;
                r.eax = width as u32;
                let mut x = 0i32;
                while x < width {
                    r.eax = storage;
                    r.ecx = word(storage as *mut u8, 0);
                    let row = word(r.ecx as *mut u8, (x as u32).wrapping_mul(12) as usize);
                    r.edx = 0;
                    r.ecx = 255;
                    while (r.edx as i32) < height {
                        let cell = row.wrapping_add(r.edx) as *mut u8;
                        r.eax = u32::from(read_unaligned(cell));
                        r.ecx = r.ecx.wrapping_add(1);
                        if (r.ecx as i32) >= (r.eax as i32) { r.ecx = r.eax; }
                        r.eax ^= r.ecx; changed |= r.eax;
                        write_unaligned(cell, r.ecx as u8);
                        r.edx = r.edx.wrapping_add(1);
                    }
                    r.edx = (height as u32).wrapping_sub(1);
                    r.ecx = 255;
                    while (r.edx as i32) >= 0 {
                        let cell = row.wrapping_add(r.edx) as *mut u8;
                        r.eax = u32::from(read_unaligned(cell));
                        r.ecx = r.ecx.wrapping_add(1);
                        if (r.ecx as i32) >= (r.eax as i32) { r.ecx = r.eax; }
                        r.eax ^= r.ecx; changed |= r.eax;
                        write_unaligned(cell, r.ecx as u8);
                        r.edx = r.edx.wrapping_sub(1);
                    }
                    r.eax = (width as u32).wrapping_sub(x as u32).wrapping_sub(1);
                    x = x.wrapping_add(1);
                }
                let mut z = 0i32;
                while z < height {
                    r.eax = width as u32;
                    r.ecx = 255;
                    let mut x = 0i32;
                    while x < width {
                        r.edx = storage;
                        r.eax = word(storage as *mut u8, 0);
                        r.edx = word(r.eax as *mut u8, (x as u32).wrapping_mul(12) as usize);
                        r.edx = r.edx.wrapping_add(z as u32);
                        r.eax = u32::from(read_unaligned(r.edx as *const u8));
                        r.ecx = r.ecx.wrapping_add(1);
                        if (r.ecx as i32) >= (r.eax as i32) { r.ecx = r.eax; }
                        r.eax ^= r.ecx; changed |= r.eax;
                        write_unaligned(r.edx as *mut u8, r.ecx as u8);
                        x = x.wrapping_add(1);
                    }
                    r.eax = (width as u32).wrapping_sub(1);
                    r.ecx = 255;
                    let mut x = r.eax as i32;
                    while x >= 0 {
                        r.eax = storage;
                        r.edx = word(storage as *mut u8, 0);
                        r.eax = word(r.edx as *mut u8, (x as u32).wrapping_mul(12) as usize);
                        r.edx = r.eax.wrapping_add(z as u32);
                        r.eax = u32::from(read_unaligned(r.edx as *const u8));
                        r.ecx = r.ecx.wrapping_add(1);
                        if (r.ecx as i32) >= (r.eax as i32) { r.ecx = r.eax; }
                        r.eax ^= r.ecx; changed |= r.eax;
                        write_unaligned(r.edx as *mut u8, r.ecx as u8);
                        x = x.wrapping_sub(1);
                    }
                    z = z.wrapping_add(1);
                }
                if changed == 0 { break; }
            }
        }
        r.ecx = if width > 0 { 0 } else { entry_ecx };
    }
);

cdecl_port!(0x005E_4FA0, fc4p_005e4fa0, make_byte_grid_mask, "make byte grid mask across tract sizes", 3,
    (r, destination: u32, source: u32, match_byte: u32) {
        if source == 0 || destination == 0 { return; }
        unsafe {
            r.eax = word(destination as *mut u8, 0); r.ecx = destination; virtual_call(r, 0x60, &[]);
            let out = r.eax;
            r.edx = word(destination as *mut u8, 0); r.ecx = destination; virtual_call(r, 0x30, &[]);
            let width = r.eax as i32;
            r.eax = word(destination as *mut u8, 0); r.ecx = destination; virtual_call(r, 0x34, &[]);
            let height = r.eax as i32;
            r.edx = word(destination as *mut u8, 0); r.ecx = destination; virtual_call(r, 0x2c, &[]);
            let shift = r.eax;
            r.eax = word(source as *mut u8, 0); r.ecx = source; virtual_call(r, 0x2c, &[]);
            let difference = shift.wrapping_sub(r.eax) as i32;
            r.edx = word(source as *mut u8, 0); r.ecx = source; virtual_call(r, 0x64, &[]);
            let input = r.eax;
            r.eax = width as u32;
            // The first destination byte is revisited for every z in this helper.
            let mut x = 0i32;
            let args = argument_words(r);
            store(args, 4, 0);
            while x < width {
                r.ecx = height as u32;
                r.eax = 0;
                let mut z = 0i32;
                while z < height {
                    r.ecx = (x as u32).wrapping_mul(12);
                    r.edx = word(input as *mut u8, 0);
                    let cell = word(word(out as *mut u8, 0) as *mut u8, r.ecx as usize);
                    let value = if difference == 0 {
                        r.edx = byte_grid_row(input, x as u32);
                        if read_unaligned(r.edx as *const u8) == match_byte as u8 { 0 } else { 255 }
                    } else if difference < 0 {
                        r.ecx = difference.wrapping_neg() as u32;
                        r.edx = (x >> (r.ecx & 31)) as u32;
                        let row = byte_grid_row(input, r.edx);
                        r.edx = (r.edx.wrapping_mul(3) & !255) | u32::from(match_byte as u8);
                        if read_unaligned(row.wrapping_add((z >> (r.ecx & 31)) as u32) as *const u8) == match_byte as u8 { 0 } else { 255 }
                    } else {
                        let shift = difference as u32;
                        let end_x = x.wrapping_add(1).wrapping_shl(shift);
                        let start_z = z.wrapping_shl(shift);
                        let end_z = z.wrapping_add(1).wrapping_shl(shift);
                        let mut sx = x.wrapping_shl(shift);
                        let mut found = false;
                        r.edx = sx as u32;
                        r.ecx = shift;
                        while sx < end_x {
                            let mut sz = start_z;
                            r.ecx = sz as u32;
                            while sz < end_z {
                                r.eax = byte_grid_row(input, sx as u32);
                                if read_unaligned(r.eax.wrapping_add(sz as u32) as *const u8) == match_byte as u8 {
                                    found = true; sx = end_x; sz = end_z;
                                }
                                sz = sz.wrapping_add(1);
                                r.ecx = sz as u32;
                            }
                            if start_z < end_z { r.eax = z as u32; }
                            sx = sx.wrapping_add(1);
                            r.edx = sx as u32;
                        }
                        r.edx = cell;
                        if found { 0 } else { 255 }
                    };
                    r.ecx = (r.ecx & !255) | u32::from(value);
                    // Both direct branches mask ECX to one byte before storing.
                    if difference <= 0 { r.ecx = u32::from(value); }
                    write_unaligned(cell as *mut u8, value);
                    r.ecx = height as u32;
                    r.eax = (z as u32).wrapping_add(1);
                    z = z.wrapping_add(1);
                }
                r.eax = width as u32;
                x = x.wrapping_add(1);
                store(args, 4, x as u32);
            }
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn allocate_occupant_message(bytes: u32) -> u32 = "C" 0x0091_33DA;
}
game_fn! {
    #[allow(dead_code)]
    fn construct_occupant_message(this: *mut u8) -> u32 = "thiscall" 0x0091_34D6;
}
game_data! { static occupant_message_server: u32 = 0x00B2_3194; }
game_data! { static default_occupant_message_server: u32 = 0x00B4_3CCC; }

rust_port!(0x005E_9010, fc4p_005e9010, notify_occupant_change, "notify occupant change", 8,
    (this, r, message_id: u32, data: u32) {
        unsafe {
            r.eax = *occupant_message_server();
            if r.eax == 0 { r.eax = *default_occupant_message_server(); *occupant_message_server() = r.eax; }
            invoke(r, crate::abi::target(0x0091_33DA), &[0x2c], true);
            let message = if r.eax == 0 { 0 } else {
                r.ecx = r.eax;
                invoke(r, crate::abi::target(0x0091_34D6), &[], false);
                r.eax
            };
            r.edx = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 4, &[]);
            r.eax = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 0x14, &[message_id]);
            r.edx = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 0x30, &[this as u32]);
            r.eax = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 0x84, &[this as u32]);
            r.edx = word(this, 0);
            let attribute_setter = crate::abi::vslot(message as *mut u8, 0x40);
            r.ecx = this as u32; virtual_call(r, 0x80, &[4]);
            r.eax &= 255;
            r.ecx = message; invoke(r, attribute_setter, &[r.eax], false);
            r.edx = word(this, 0);
            let type_setter = crate::abi::vslot(message as *mut u8, 0x54);
            r.ecx = this as u32; virtual_call(r, 0x1c, &[]);
            r.ecx = message; invoke(r, type_setter, &[r.eax], false);
            r.ecx = data; r.eax = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 0x68, &[data]);
            if message_id == 0x99ef_1146 {
                r.eax = word(this, 0x30);
                if r.eax != 0 {
                    r.ecx = *occupant_message_server(); r.edx = word(r.ecx as *mut u8, 0);
                    virtual_call(r, 0x0c, &[message]);
                    r.edx = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 8, &[]);
                    return;
                }
            }
            r.ecx = *occupant_message_server(); r.eax = word(r.ecx as *mut u8, 0);
            virtual_call(r, 0x10, &[message, 0]);
            r.edx = word(message as *mut u8, 0); r.ecx = message; virtual_call(r, 8, &[]);
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn move_property_pairs_backward(first: u32, last: u32, out: u32, tag: u32, unused: u32) -> u32 = "C" 0x005F_DD40;
}
game_fn! {
    #[allow(dead_code)]
    fn assign_property_pair(this: *mut u8, value: u32) -> u32 = "thiscall" 0x005F_DCF0;
}
game_fn! {
    #[allow(dead_code)]
    fn grow_property_pairs(this: *mut u8, at: u32, value: u32, tag: u32, count: u32, skip_tail: u32) -> u32 = "thiscall" 0x0077_D7E0;
}

rust_port!(0x005F_F5D0, fc4p_005ff5d0, insert_property_pair, "insert retained property pair", 8,
    (this, r, at: u32, value: u32) {
        unsafe {
            r.eax = word(this, 0); r.ecx = word(this, 8);
            let index = (at.wrapping_sub(r.eax) as i32 >> 3) as u32;
            r.eax = word(this, 4);
            let tag = argument_words(r).wrapping_add(4) as u32;
            if r.eax == r.ecx {
                r.ecx = value;
                r.eax = tag;
                r.ecx = this as u32;
                invoke(r, crate::abi::target(0x0077_D7E0), &[at, value, tag, 1, 0], false);
            } else if at == r.eax {
                if r.eax != 0 {
                    r.ecx = value;
                    r.edx = word(value as *mut u8, 0); store(r.eax as *mut u8, 0, r.edx);
                    r.ecx = word(value as *mut u8, 4); store(r.eax as *mut u8, 4, r.ecx);
                    if r.ecx != 0 { r.eax = word(r.ecx as *mut u8, 0); virtual_call(r, 4, &[]); }
                }
                r.eax = word(this, 4).wrapping_add(8);
                r.edx = word(this, 0);
                store(this, 4, r.eax);
                r.eax = r.edx.wrapping_add(index.wrapping_mul(8));
                return;
            } else {
                if r.eax != 0 {
                    r.ecx = word(r.eax.wrapping_sub(8) as *mut u8, 0); store(r.eax as *mut u8, 0, r.ecx);
                    r.ecx = word(r.eax.wrapping_sub(4) as *mut u8, 0); store(r.eax as *mut u8, 4, r.ecx);
                    if r.ecx != 0 { r.edx = word(r.ecx as *mut u8, 0); virtual_call(r, 4, &[]); }
                }
                r.ecx = word(this, 4).wrapping_add(8); r.eax = value; store(this, 4, r.ecx);
                r.ecx = word(value as *mut u8, 0);
                let mut temporary = [r.ecx, 0];
                r.ecx = word(value as *mut u8, 4); temporary[1] = r.ecx;
                if r.ecx != 0 { r.edx = word(r.ecx as *mut u8, 0); virtual_call(r, 4, &[]); }
                r.eax = word(this, 4);
                r.ecx = tag;
                r.edx = r.eax.wrapping_sub(8);
                r.eax = r.eax.wrapping_sub(16);
                invoke(r, crate::abi::target(0x005F_DD40), &[at, r.eax, r.edx, tag, 0], true);
                r.eax = temporary.as_mut_ptr() as u32; r.ecx = at;
                invoke(r, crate::abi::target(0x005F_DCF0), &[r.eax], false);
                r.ecx = temporary[1];
                if r.ecx != 0 { r.edx = word(r.ecx as *mut u8, 0); virtual_call(r, 8, &[]); }
            }
            r.edx = word(this, 0);
            r.eax = r.edx.wrapping_add(index.wrapping_mul(8));
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn destroy_window_tree(this: *mut u8, node: u32) -> u32 = "thiscall" 0x0046_76A0;
}

rust_port!(0x0062_5F10, fc4p_00625f10, clear_window_tree_and_pool, "clear window tree and pool", 0,
    (this, r) {
        unsafe {
            let owner = WindowCleanup::from_ptr(this);
            r.eax = owner.sentinel();
            let mut node = word(r.eax as *mut u8, 8);
            let tree = this.wrapping_add(0x214);
            while node != r.eax {
                r.ecx = WindowTreeLinks::from_ptr(node as *mut u8).window(); r.eax = word(r.ecx as *mut u8, 0);
                virtual_call(r, 0x44, &[0, 0]);
                r.ecx = word(node as *mut u8, 0x0c); r.eax = node;
                if r.ecx != 0 {
                    r.eax = r.ecx;
                    r.ecx = word(r.eax as *mut u8, 8);
                    while r.ecx != 0 { r.eax = r.ecx; r.ecx = word(r.eax as *mut u8, 8); }
                } else {
                    r.ecx = word(node as *mut u8, 4);
                    while r.eax == word(r.ecx as *mut u8, 0x0c) {
                        r.eax = r.ecx; r.ecx = word(r.ecx as *mut u8, 4);
                    }
                    if word(r.eax as *mut u8, 0x0c) != r.ecx { r.eax = r.ecx; }
                }
                node = r.eax;
                if node == word(tree, 0) { break; }
                // The next comparison is against the tree's current sentinel.
                r.eax = word(tree, 0);
            }
            r.eax = word(tree, 4);
            if r.eax != 0 {
                r.ecx = word(tree, 0); r.edx = word(r.ecx as *mut u8, 4); r.ecx = tree as u32;
                invoke(r, crate::abi::target(0x0046_76A0), &[r.edx], false);
                r.eax = word(tree, 0); store(r.eax as *mut u8, 8, r.eax);
                r.eax = word(tree, 0); store(r.eax as *mut u8, 4, 0);
                r.eax = word(tree, 0); store(r.eax as *mut u8, 0x0c, r.eax);
                store(tree, 4, 0);
            }
            let pool = owner.pool();
            if pool == 0 { return; }
            let pool_fields = WindowPool::from_ptr(pool as *mut u8);
            r.eax = pool_fields.blocks();
            pool_fields.set_table(0x00a8_6364);
            if r.eax != 0 {
                r.eax = word(pool as *mut u8, 0x14);
                let mut index = 0u32;
                while index < r.eax {
                    r.ecx = word(pool as *mut u8, 0x20);
                    r.edx = word(r.ecx as *mut u8, index.wrapping_mul(4) as usize);
                    invoke(r, crate::abi::target(0x005E_5620), &[r.edx], true);
                    r.eax = word(pool as *mut u8, 0x14);
                    index = index.wrapping_add(1);
                }
                r.eax = word(pool as *mut u8, 0x20);
                invoke(r, crate::abi::target(0x005E_5620), &[r.eax], true);
            }
            invoke(r, crate::abi::target(0x005E_5620), &[pool], true);
            store(this, 0x210, 0);
        }
    }
);

game_data! { static read_windows_version: u32 = 0x00A8_00F4; }
game_data! { static get_windows_module: u32 = 0x00A8_00F0; }
game_data! { static install_windows_hook: u32 = 0x00A8_0500; }
game_data! { static remove_windows_hook: u32 = 0x00A8_0504; }
game_data! { static windows_system_parameters: u32 = 0x00A8_04C0; }
game_data! { static app_switch_disable_count: u32 = 0x00B4_3C24; }
game_data! { static app_switch_hook: u32 = 0x00B4_3C20; }
game_fn! {
    #[allow(dead_code)]
    fn app_switch_hook_callback(code: u32, message: u32, data: u32) -> u32 = "stdcall" 0x005F_86E0;
}

cdecl_port!(0x005F_87D0, fc4p_005f87d0, set_app_switching_disabled, "set application switching disabled", 1,
    (r, disable: u32) {
        unsafe {
            let mut version = [0u32; 37];
            version[0] = 0x94;
            r.eax = version.as_mut_ptr() as u32;
            r.ecx = 0;
            invoke(r, *read_windows_version() as usize, &[r.eax], false);
            r.edx = version[1];
            r.ecx = (r.ecx & !255) | u32::from(disable as u8);
            r.eax = (r.eax & !255) | u32::from(r.edx > 4);
            let modern = r.eax as u8 != 0;
            let mut previous = core::mem::MaybeUninit::<u32>::uninit();
            if disable as u8 != 0 {
                r.ecx = (*app_switch_disable_count()).wrapping_add(1);
                *app_switch_disable_count() = r.ecx;
                if r.ecx != 1 { return; }
                if modern {
                    invoke(r, *get_windows_module() as usize, &[0], false);
                    invoke(r, *install_windows_hook() as usize, &[13, 0x005f_86e0, r.eax, 0], false);
                    *app_switch_hook() = r.eax;
                } else {
                    r.ecx = previous.as_mut_ptr() as u32;
                    invoke(r, *windows_system_parameters() as usize, &[0x61, 1, r.ecx, 0], false);
                }
            } else {
                *app_switch_disable_count() = (*app_switch_disable_count()).wrapping_sub(1);
                if *app_switch_disable_count() != 0 { return; }
                if modern {
                    r.eax = *app_switch_hook();
                    if r.eax == 0 { return; }
                    invoke(r, *remove_windows_hook() as usize, &[r.eax], false);
                    *app_switch_hook() = 0;
                } else {
                    r.edx = previous.as_mut_ptr() as u32;
                    invoke(r, *windows_system_parameters() as usize, &[0x61, 0, r.edx, 0], false);
                }
            }
        }
    }
);

game_fn! {
    #[allow(dead_code)]
    fn find_byte_in_range(first: u32, last: u32, byte: u32, tag: u32) -> u32 = "C" 0x0044_A4B0;
}

port! {
    /// Retain three scratch words below the entry stack, including the bytes outside their low byte.
    #[port(address = 0x005C_6060, owner = "shared", name = "find first byte outside set", naked)]
    pub unsafe extern "C" fn fc4p_005c6060() {
        core::arch::naked_asm!(
            "sub esp, 24", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
            "mov edx, esp", ".rept 5", "push dword ptr [esp + 44]", ".endr",
            "call {body}", "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]",
            "add esp, 24", "ret", body = sym find_first_byte_outside_set,
        );
    }
}

/// A scratch word's upper bytes can come from the incoming stack. Read those machine bytes without
/// requiring a Rust value to have been initialized there first.
unsafe fn scratch_word(pointer: *const u8) -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!("mov {value}, dword ptr [{pointer}]", value = out(reg) value, pointer = in(reg) pointer, options(readonly, nostack, preserves_flags));
    }
    value
}

unsafe extern "fastcall" fn find_first_byte_outside_set(
    _this: *mut u8,
    r: &mut Registers,
    first: u32,
    last: u32,
    set: u32,
    set_end: u32,
    _tag: u32,
) {
    unsafe {
        let frame = (r as *mut Registers).cast::<u8>();
        let args = frame.wrapping_add(28);
        let tag = args.wrapping_add(4) as u32;
        let mut current = first;
        let mut groups = last.wrapping_sub(first) as i32 >> 2;
        while groups > 0 {
            for phase in 0..4 {
                let byte = read_unaligned(current as *const u8);
                r.eax = (r.eax & !255) | u32::from(byte);
                r.ecx = tag;
                let scratch = if phase == 0 {
                    args
                } else {
                    frame.wrapping_add(8 + phase * 4)
                };
                write_unaligned(scratch, byte);
                r.edx = scratch_word(scratch);
                invoke(r, crate::abi::target(0x0044_A4B0), &[set, set_end, r.edx, tag], true);
                if r.eax == set_end {
                    r.eax = current;
                    return;
                }
                current = current.wrapping_add(1);
            }
            groups -= 1;
        }
        let remaining = word(args, 4).wrapping_sub(current);
        r.eax = remaining.wrapping_sub(1);
        let tail = if r.eax == 0 {
            1
        } else {
            r.eax = r.eax.wrapping_sub(1);
            if r.eax == 0 {
                2
            } else {
                r.eax = r.eax.wrapping_sub(1);
                if r.eax == 0 {
                    3
                } else {
                    0
                }
            }
        };
        for _ in 0..tail {
            let byte = read_unaligned(current as *const u8);
            r.eax = (r.eax & !255) | u32::from(byte);
            r.ecx = tag;
            write_unaligned(args, byte);
            r.edx = scratch_word(args);
            invoke(r, crate::abi::target(0x0044_A4B0), &[set, set_end, r.edx, tag], true);
            if r.eax == set_end {
                r.eax = current;
                return;
            }
            current = current.wrapping_add(1);
        }
        r.eax = word(args, 4);
    }
}

game_fn! {
    #[allow(dead_code)]
    fn copy_byte_range(first: u32, last: u32, out: u32) -> u32 = "C" 0x006D_6880;
}
game_fn! {
    #[allow(dead_code)]
    fn move_bytes(out: u32, source: u32, count: u32) -> u32 = "C" 0x009E_EB70;
}

/// Copy forward in four-byte units followed by bytes. This also keeps the unit size when the ranges overlap.
unsafe fn copy_string_units(mut out: u32, mut source: u32, count: u32) {
    unsafe {
        for _ in 0..count / 4 {
            let value = word(source as *mut u8, 0);
            store(out as *mut u8, 0, value);
            out = out.wrapping_add(4);
            source = source.wrapping_add(4);
        }
        for _ in 0..count % 4 {
            write_unaligned(out as *mut u8, read_unaligned(source as *const u8));
            out = out.wrapping_add(1);
            source = source.wrapping_add(1);
        }
    }
}

rust_port!(0x005C_2470, fc4p_005c2470, insert_string_bytes_range, "insert string byte range", 16,
    (this, r, at: u32, first: u32, last: u32, _tag: u32) {
        unsafe {
            r.eax = last; r.edx = first;
            if first == last { return; }
            r.ecx = word(this, 8);
            let end = word(this, 4);
            let mut count = last.wrapping_sub(first);
            r.ecx = r.ecx.wrapping_sub(end);
            let args = argument_words(r);
            if (r.ecx as i32) >= (count.wrapping_add(1) as i32) {
                let tail = end.wrapping_sub(at);
                if (tail as i32) >= (count as i32) {
                    r.eax = end.wrapping_add(1);
                    invoke(r, crate::abi::target(0x006D_6880),
                        &[end.wrapping_sub(count).wrapping_add(1), r.eax, r.eax], true);
                    r.edx = word(this, 4).wrapping_add(count);
                    let remaining = tail.wrapping_sub(count).wrapping_add(1);
                    store(this, 4, r.edx);
                    if remaining != 0 {
                        r.edx = at.wrapping_add(count);
                        invoke(r, crate::abi::target(0x009E_EB70), &[r.edx, at, remaining], true);
                    }
                    if count != 0 {
                        let source = word(args, 4);
                        r.eax = count;
                        copy_string_units(at, source, count);
                    }
                } else {
                    let split = tail.wrapping_add(first).wrapping_add(1);
                    r.ecx = split;
                    store(args, 8, split);
                    if last != split {
                        r.eax = last.wrapping_sub(split);
                        r.ecx = end.wrapping_add(1);
                        invoke(r, crate::abi::target(0x009E_EB70), &[r.ecx, split, r.eax], true);
                    }
                    count = count.wrapping_sub(tail);
                    r.edx = word(this, 4).wrapping_add(count);
                    r.eax = r.edx;
                    store(this, 4, r.edx);
                    r.edx = at;
                    invoke(r, crate::abi::target(0x006D_6880), &[at, end.wrapping_add(1), r.eax], true);
                    r.edx = word(this, 4).wrapping_add(tail);
                    r.eax = word(args, 8);
                    let source = word(args, 4);
                    r.eax = r.eax.wrapping_sub(source);
                    store(this, 4, r.edx);
                    if r.eax != 0 { copy_string_units(at, source, r.eax); }
                }
                r.ecx = this as u32;
            } else {
                let size = end.wrapping_sub(word(this, 0));
                let extra = size.max(count);
                let capacity = extra.wrapping_add(size).wrapping_add(1);
                r.ecx = extra;
                r.eax = capacity;
                store(args, 8, count);
                let allocation = if capacity != 0 {
                    invoke(r, crate::abi::target(0x0090_CF54), &[capacity], true);
                    r.edx = word(args, 4);
                    r.eax
                } else { 0 };
                store(args, 8, allocation);
                let mut source = word(this, 0);
                r.ecx = allocation;
                r.eax = at.wrapping_sub(source);
                while (r.eax as i32) > 0 {
                    r.edx = (r.edx & !255) | u32::from(read_unaligned(source as *const u8));
                    source = source.wrapping_add(1);
                    write_unaligned(r.ecx as *mut u8, r.edx as u8);
                    r.ecx = r.ecx.wrapping_add(1); r.eax = r.eax.wrapping_sub(1);
                }
                if (at.wrapping_sub(word(this, 0)) as i32) > 0 { r.edx = word(args, 4); }
                invoke(r, crate::abi::target(0x009E_EB70), &[r.ecx, r.edx, count], true);
                let mut new_end = r.eax.wrapping_add(count);
                r.eax = word(this, 4).wrapping_sub(at);
                let mut remaining = r.eax as i32;
                if remaining > 0 {
                    r.ecx = at.wrapping_sub(new_end);
                    while remaining > 0 {
                        r.eax = (r.eax & !255) | u32::from(read_unaligned(r.ecx.wrapping_add(new_end) as *const u8));
                        write_unaligned(new_end as *mut u8, r.eax as u8);
                        new_end = new_end.wrapping_add(1); remaining -= 1;
                    }
                }
                write_unaligned(new_end as *mut u8, 0);
                r.eax = word(this, 0);
                if r.eax != 0 { invoke(r, crate::abi::target(0x0090_CF63), &[r.eax], true); }
                r.eax = word(args, 8); r.ecx = capacity;
                store(this, 0, r.eax);
                r.eax = r.eax.wrapping_add(capacity);
                store(this, 4, new_end); store(this, 8, r.eax);
            }
        }
    }
);

/// This module's complete replacements.
pub const PORTS: &[Port] = &[
    fc4p_005c2380::PORT,
    fc4p_005c23e0::PORT,
    fc4p_005c2470::PORT,
    fc4p_005c6060::PORT,
    fc4p_005c6610::PORT,
    fc4p_005d6ba0::PORT,
    fc4p_005e4d60::PORT,
    fc4p_005e4fa0::PORT,
    fc4p_005e9010::PORT,
    fc4p_005f87d0::PORT,
    fc4p_005ff5d0::PORT,
    fc4p_00621a40::PORT,
    fc4p_00621b40::PORT,
    fc4p_00625f10::PORT,
    fc4p_00626240::PORT,
    fc4p_005d9430::PORT,
    fc4p_005ec430::PORT,
    fc4p_005ec690::PORT,
    fc4p_005ec6e0::PORT,
    fc4p_005ec7c0::PORT,
    fc4p_0060cb80::PORT,
    fc4p_00623290::PORT,
    fc4p_00624f70::PORT,
    fc4p_006266e0::PORT,
    fc4p_005f4760::PORT,
    fc4p_005fe980::PORT,
    fc4p_005fea70::PORT,
    fc4p_005fead0::PORT,
    fc4p_00622190::PORT,
    fc4p_005e5130::PORT,
    fc4p_005fa540::PORT,
    fc4p_005fd030::PORT,
    fc4p_00611b00::PORT,
    fc4p_00623b20::PORT,
    fc4p_00624920::PORT,
    fc4p_005c2910::PORT,
    fc4p_005ed1f0::PORT,
    fc4p_005b9d30::PORT,
    fc4p_005bcb60::PORT,
    fc4p_005bcb70::PORT,
    fc4p_005bcb80::PORT,
    fc4p_005be2e0::PORT,
    fc4p_005be3f0::PORT,
    fc4p_005be420::PORT,
    fc4p_005c0500::PORT,
    fc4p_005d2070::PORT,
    fc4p_005d25f0::PORT,
    fc4p_005d3500::PORT,
    fc4p_005d4ac0::PORT,
    fc4p_005d6140::PORT,
    fc4p_005d6150::PORT,
    fc4p_005d6360::PORT,
    fc4p_005d6380::PORT,
    fc4p_005d7b40::PORT,
    fc4p_005dc950::PORT,
    fc4p_005dc980::PORT,
    fc4p_005deb40::PORT,
    fc4p_005deb90::PORT,
    fc4p_005ded60::PORT,
    fc4p_005e28c0::PORT,
    fc4p_005e28f0::PORT,
    fc4p_005e2900::PORT,
    fc4p_005e29b0::PORT,
    fc4p_005e29c0::PORT,
    fc4p_005e29e0::PORT,
    fc4p_005e29f0::PORT,
    fc4p_005e2a10::PORT,
    fc4p_005e2a20::PORT,
    fc4p_005e55b0::PORT,
    fc4p_005e5670::PORT,
    fc4p_005e56b0::PORT,
    fc4p_005e56d0::PORT,
    fc4p_005e5710::PORT,
    fc4p_005e5730::PORT,
    fc4p_005e5790::PORT,
    fc4p_005e57d0::PORT,
    fc4p_005e5810::PORT,
    fc4p_005e5830::PORT,
    fc4p_005e5870::PORT,
    fc4p_005e58b0::PORT,
    fc4p_005e5a40::PORT,
    fc4p_005e5a90::PORT,
    fc4p_005e5ae0::PORT,
    fc4p_005e5b30::PORT,
    fc4p_005e5b80::PORT,
    fc4p_005e5bd0::PORT,
    fc4p_005e5c20::PORT,
    fc4p_005e5c70::PORT,
    fc4p_005ec9e0::PORT,
    fc4p_005ecaf0::PORT,
    fc4p_005ecb00::PORT,
    fc4p_005ed5c0::PORT,
    fc4p_005ed5d0::PORT,
    fc4p_005ed620::PORT,
    fc4p_005ed630::PORT,
    fc4p_005ed650::PORT,
    fc4p_005ed660::PORT,
    fc4p_005ed680::PORT,
    fc4p_005efe60::PORT,
    fc4p_005f47f0::PORT,
    fc4p_005f8400::PORT,
    fc4p_005f8f80::PORT,
    fc4p_005fb0e0::PORT,
    fc4p_005fd000::PORT,
    fc4p_005fd370::PORT,
    fc4p_005fd720::PORT,
    fc4p_00603da0::PORT,
    fc4p_0060c860::PORT,
    fc4p_0060c890::PORT,
    fc4p_0060c8d0::PORT,
    fc4p_0060ce80::PORT,
    fc4p_0060fb50::PORT,
    fc4p_0060fb80::PORT,
    fc4p_00614430::PORT,
    fc4p_00623270::PORT,
    fc4p_00623fa0::PORT,
    fc4p_00623fe0::PORT,
    fc4p_00624020::PORT,
    fc4p_00624e60::PORT,
    fc4p_0062c8e0::PORT,
    fc4p_0062c8f0::PORT,
    fc4p_0062c910::PORT,
    fc4p_0062c920::PORT,
    fc4p_0062c930::PORT,
    fc4p_0062c980::PORT,
    fc4p_0062c990::PORT,
    fc4p_0062c9b0::PORT,
    fc4p_0062ca30::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, GameFpu};
    use std::cell::{Cell, RefCell};
    const EAX: u32 = 0x1234_56ab;
    const EDX: u32 = 0x9876_5432;
    const RESULT: [u32; 3] = [0xa1b2_c3d4, 0x1020_3040, 0x5566_7788];
    fn run(entry: unsafe extern "C" fn(), this: *mut u8, args: &[u32]) -> [u32; 3] {
        let mut r = Registers {
            eax: EAX,
            ecx: this as u32,
            edx: EDX,
        };
        unsafe {
            invoke(&mut r, entry as *const () as usize, args, false);
        }
        [r.eax, r.ecx, r.edx]
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Call {
        address: u32,
        registers: [u32; 3],
        args: Vec<u32>,
    }
    thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
    thread_local! {
        static ALLOCATED: Cell<u32> = const { Cell::new(0) };
        static NEXT_TABLE: Cell<u32> = const { Cell::new(0) };
        static POOL_AT_CALL: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    }
    extern "C" fn record(address: u32, r: *const Registers, args: *const u32, count: u32) -> u32 {
        unsafe {
            CALLS.with(|calls| {
                calls.borrow_mut().push(Call {
                    address,
                    registers: [(*r).eax, (*r).ecx, (*r).edx],
                    args: core::slice::from_raw_parts(args, count as usize).to_vec(),
                })
            });
            if address == 0x10 {
                let next = NEXT_TABLE.with(Cell::get);
                if next != 0 {
                    store((*r).ecx as *mut u8, 0, next);
                }
            }
            if address == 0x0090_d031 || address == 0x0090_cff9 {
                POOL_AT_CALL.with(|v| {
                    v.borrow_mut()
                        .push(core::slice::from_raw_parts(*args as *const u8, 0x40).to_vec())
                });
            }
        }
        ALLOCATED.with(Cell::get)
    }
    fn calls() -> Vec<Call> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }
    macro_rules! stub {
        ($name:ident, $address:literal, $count:literal, $pop:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
                core::arch::naked_asm!(
                    "sub esp, 12", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "lea eax, [esp + 16]", "push {count}", "push eax", "lea eax, [esp + 8]",
                    "push eax", "push {address}", "call {record}", "add esp, 28",
                    "mov eax, 0xa1b2c3d4", "mov ecx, 0x10203040", "mov edx, 0x55667788", "ret {pop}",
                    address = const $address, count = const $count, pop = const $pop, record = sym record,
                );
            }
        };
    }
    stub!(no_args, 0, 0, 0);
    stub!(one_arg, 0, 1, 4);
    stub!(unknown_destroy, 0x0090_d990, 0, 0);
    stub!(storage_delete, 0x005e_5620, 1, 0);
    stub!(finish, 0x005d_3250, 0, 0);
    stub!(virtual_finish, 0x1c, 0, 0);
    stub!(add_reference, 4, 0, 0);
    stub!(query_base, 0x005e_89c0, 2, 8);
    #[test]
    fn forwarding_keeps_arguments_registers_and_immediate_targets() {
        let cases: &[(unsafe extern "C" fn(), u32, i32, bool)] = &[
            (fc4p_005bcb60, 0x005B_CB30, -4, false),
            (fc4p_005bcb70, 0x005B_CB80, -4, true),
            (fc4p_005be3f0, 0x0090_D96B, 4, false),
            (fc4p_005be420, 0x005B_E3E0, -4, true),
            (fc4p_005c0500, 0x004B_DF00, -4, false),
            (fc4p_005e28f0, 0x005E_89B0, 4, false),
            (fc4p_005e2900, 0x005E_8DF0, 4, false),
            (fc4p_005e29b0, 0x005E_28F0, -4, false),
            (fc4p_005e29c0, 0x005E_2900, -4, false),
            (fc4p_005e29e0, 0x005E_28F0, -8, false),
            (fc4p_005e29f0, 0x005E_2900, -8, false),
            (fc4p_005e2a10, 0x005E_28F0, -12, false),
            (fc4p_005e2a20, 0x005E_2900, -12, false),
            (fc4p_005ecaf0, 0x005E_89B0, 12, false),
            (fc4p_005ecb00, 0x005E_8DF0, 12, false),
            (fc4p_005ed5c0, 0x005E_CAF0, -4, false),
            (fc4p_005ed5d0, 0x005E_CB00, -4, false),
            (fc4p_005ed620, 0x005E_CAF0, -12, false),
            (fc4p_005ed630, 0x005E_CB00, -12, false),
            (fc4p_005ed650, 0x005E_CAF0, -16, false),
            (fc4p_005ed660, 0x005E_CB00, -16, false),
            (fc4p_005ed680, 0x005E_CAF0, -20, false),
            (fc4p_005efe60, 0x005F_8400, -4, false),
            (fc4p_005f8f80, 0x0094_D34F, -4, false),
            (fc4p_005fd720, 0x005F_E980, -4, true),
            (fc4p_0062c9b0, 0x0062_3270, -4, false),
        ];
        assert_eq!(cases.len(), 26);
        for &(entry, target, adjustment, argument) in cases {
            let mut object = [0xdddd_ddddu32; 8];
            let adjusted = object.as_mut_ptr() as u32;
            let this = adjusted.wrapping_sub(adjustment as u32) as *mut u8;
            let target_stub = if argument { one_arg } else { no_args };
            let _fake = fake(target, target_stub as *const () as usize);
            let args: &[u32] = if argument { &[0x8000_00ff] } else { &[] };
            assert_eq!(run(entry, this, args), RESULT);
            assert_eq!(
                calls(),
                [Call {
                    address: 0,
                    registers: [EAX, adjusted, EDX],
                    args: args.to_vec()
                }]
            );
            assert_eq!(object, [0xdddd_dddd; 8]);
        }
    }
    #[test]
    fn leaf_fields_keep_other_register_bits_and_neighboring_bytes() {
        let mut bytes = [0xa5u8; 0x400];
        let p = bytes.as_mut_ptr();
        let before = bytes;
        for (entry, offset) in [
            (fc4p_005d2070 as unsafe extern "C" fn(), 0x1cc),
            (fc4p_005d25f0, 0x1d4),
            (fc4p_0060ce80, 4),
            (fc4p_0060fb80, 0x28),
        ] {
            unsafe {
                store(p, offset, 0xfedc_ba98);
            }
            assert_eq!(run(entry, p, &[]), [0xfedc_ba98, p as u32, EDX]);
            unsafe {
                store(p, offset, 0xa5a5_a5a5);
            }
        }
        assert_eq!(run(fc4p_0060fb50, p, &[]), [p as u32 + 4, p as u32, EDX]);
        // The particle stop query retains the raw byte, including noncanonical true values.
        for value in [0, 1, 0x80, 0xff] {
            bytes[0xc] = value;
            assert_eq!(
                run(fc4p_005b9d30, p, &[]),
                [(EAX & !255) | u32::from(value), p as u32, EDX]
            );
        }
        bytes[0xc] = 0xa5;
        for value in [0, 1, 0x7f, 0x80, 0xff] {
            bytes[0x58] = value;
            assert_eq!(
                run(fc4p_005d6140, p, &[]),
                [i32::from(value as i8) as u32, p as u32, EDX]
            );
            assert_eq!(
                run(fc4p_005d6150, p, &[0xdead_0000 | u32::from(value)]),
                [(EAX & !255) | u32::from(value), p as u32, EDX]
            );
            assert_eq!(bytes[0x58], value);
        }
        bytes[0x58] = 0xa5;
        assert_eq!(bytes, before);
        for (entry, args) in [
            (fc4p_005be2e0 as unsafe extern "C" fn(), vec![1, 2, 3]),
            (fc4p_005deb40, vec![4]),
            (fc4p_0062c980, vec![5, 6]),
        ] {
            assert_eq!(run(entry, p, &args), [EAX & !255, p as u32, EDX]);
        }
        assert_eq!(run(fc4p_0062c8e0, p, &[]), [EAX, p as u32, EDX]);
        let mut expected = before;
        expected[0x20c..0x210].fill(0);
        assert_eq!(bytes, expected);
    }
    #[test]
    fn float_leaves_keep_x87_bits_and_integer_registers() {
        let _fpu = GameFpu::new();
        let mut object = [0xa5a5_a5a5u32; 32];
        let p = object.as_mut_ptr().cast::<u8>();
        for bits in [0, 0x8000_0000, 1, 0x7f7f_ffff, 0x7f80_0000, 0x7f80_0123, 0xffc1_2345] {
            object[0x3c / 4] = bits;
            for (entry, args) in [
                (fc4p_005d4ac0 as unsafe extern "C" fn(), vec![]),
                (fc4p_0062c990, vec![bits]),
            ] {
                let mut call =
                    crate::abi::RawCall::new(crate::abi::Conv::Thiscall, entry as *const () as usize, p as u32, &args);
                call.eax = EAX;
                call.edx = EDX;
                call.take_st0 = 1;
                unsafe {
                    crate::abi::call_raw(&mut call);
                }
                assert_eq!((call.eax, call.edx), (EAX, EDX));
                assert_eq!(call.st0, crate::x87::F80::from_f32(f32::from_bits(bits)));
            }
        }
    }
    #[test]
    fn advice_deletion_uses_low_flag_bit_and_null_base_adjustment() {
        let _destroy = fake(0x0090_d990, unknown_destroy as *const () as usize);
        let _delete = fake(0x005e_5620, storage_delete as *const () as usize);
        let mut object = [0u32; 8];
        for this in [object.as_mut_ptr().cast::<u8>(), core::ptr::null_mut()] {
            for flags in [0, 1, 2, 3, 0x8000_0000, u32::MAX] {
                assert_eq!(run(fc4p_005bcb80, this, &[flags]), [this as u32, RESULT[1], RESULT[2]]);
                let mut expected = vec![Call {
                    address: 0x0090_d990,
                    registers: [EAX, if this.is_null() { 0 } else { this as u32 + 4 }, EDX],
                    args: vec![],
                }];
                if flags & 1 != 0 {
                    expected.push(Call {
                        address: 0x005e_5620,
                        registers: RESULT,
                        args: vec![this as u32],
                    });
                }
                assert_eq!(calls(), expected);
            }
        }
    }
    #[test]
    fn conditional_finish_only_calls_virtual_for_inactive_type_two() {
        let _finish = fake(0x005d_3250, finish as *const () as usize);
        let mut table = [0usize; 8];
        table[7] = virtual_finish as *const () as usize;
        let mut object = [0u32; 20];
        let p = object.as_mut_ptr().cast::<u8>();
        object[0] = table.as_ptr() as u32;
        for (kind, active, needs_virtual) in [(0, 0, false), (2, 1, false), (2, 0, true), (u32::MAX, 0, false)] {
            object[0x20 / 4] = kind;
            unsafe {
                *p.add(0x29) = active;
            }
            let got = run(fc4p_005d3500, p, &[]);
            assert_eq!(
                got,
                [
                    (RESULT[0] & !255) | if needs_virtual { RESULT[0] & 255 } else { 1 },
                    RESULT[1],
                    RESULT[2]
                ]
            );
            let log = calls();
            assert_eq!(log.len(), if needs_virtual { 2 } else { 1 });
            assert_eq!(log.last().unwrap().address, 0x005d_3250);
            assert_eq!(log.last().unwrap().registers[1], p as u32 - 8);
            if needs_virtual {
                assert_eq!(
                    log[0],
                    Call {
                        address: 0x1c,
                        registers: [table.as_ptr() as u32, p as u32, EDX],
                        args: vec![]
                    }
                );
            }
        }
    }
    #[test]
    fn positions_and_bounds_copy_raw_words_with_overlapping_storage() {
        for offset in [0, 4, 0x38, 0x3c, 0x40] {
            for get in [false, true] {
                let mut object = [0u32; 40];
                for (i, v) in object.iter_mut().enumerate() {
                    *v = 0x7f80_0001 + i as u32;
                }
                // Flora occupant's ordinary position vector.
                object[15..18].copy_from_slice(&[(-12.5f32).to_bits(), 63f32.to_bits(), 4096f32.to_bits()]);
                let p = object.as_mut_ptr().cast::<u8>();
                let q = p.wrapping_add(offset);
                let mut expected = object;
                let (source, dest) = if get { (15, offset / 4) } else { (offset / 4, 15) };
                for i in 0..3 {
                    expected[dest + i] = expected[source + i];
                }
                let got = run(if get { fc4p_005d6360 } else { fc4p_005d6380 }, p, &[q as u32]);
                assert_eq!(object, expected);
                assert_eq!(
                    got,
                    if get {
                        [q as u32, expected[dest + 2], expected[dest + 1]]
                    } else {
                        [expected[17], p as u32, expected[16]]
                    }
                );
            }
        }
        for (lo, hi) in [(0, 12), (0x38, 0x44), (0x40, 0x34)] {
            let mut object = [0u32; 40];
            for (i, v) in object.iter_mut().enumerate() {
                *v = 0x8000_0000 + i as u32;
            }
            let p = object.as_mut_ptr().cast::<u8>();
            let mut expected = object;
            for (dest, source) in [(lo / 4, 13), (hi / 4, 16)] {
                for i in 0..3 {
                    expected[dest + i] = expected[source + i];
                }
            }
            let got = run(fc4p_005ded60, p, &[p as u32 + lo as u32, p as u32 + hi as u32]);
            assert_eq!(object, expected);
            assert_eq!(got, [p as u32 + hi as u32, expected[hi / 4 + 2], expected[hi / 4 + 1]]);
        }
    }
    #[test]
    fn reference_increments_wrap_and_only_write_the_count_word() {
        for (entry, offset) in [(fc4p_005d7b40 as unsafe extern "C" fn(), 9), (fc4p_005f47f0, 1)] {
            for count in [0u32, 1, 0x7fff_ffff, u32::MAX] {
                let mut object = [0xa5a5_a5a5u32; 32];
                object[offset] = count;
                let mut expected = object;
                expected[offset] = count.wrapping_add(1);
                let p = object.as_mut_ptr().cast::<u8>();
                assert_eq!(run(entry, p, &[]), [expected[offset], p as u32, EDX]);
                assert_eq!(object, expected);
            }
        }
    }
    #[test]
    fn rectangle_equality_short_circuits_and_preserves_last_loaded_registers() {
        let mut a = [0x8000_0000u32, 1, 2, u32::MAX];
        let p = a.as_mut_ptr().cast::<u8>();
        for mismatch in 0..=4 {
            let mut b = a;
            if mismatch < 4 {
                b[mismatch] ^= 1;
            }
            let q = b.as_mut_ptr().cast::<u8>();
            assert_eq!(
                run(fc4p_005dc950, p, &[q as u32]),
                [
                    u32::from(mismatch == 4),
                    if mismatch < 3 { p as u32 } else { a[3] },
                    a[mismatch.min(2)]
                ]
            );
        }
    }
    #[test]
    fn rectangle_union_uses_signed_extents_even_for_inverted_or_degenerate_boxes() {
        for (a, b, expected, valid) in [
            ([-5, -2, 4, 9], [1, -7, 8, 2], [-5, -7, 8, 9], 1),
            ([1, 2, 1, 4], [1, 2, 1, 4], [1, 2, 1, 4], 0),
            ([5, 9, 1, 2], [6, 10, 0, 1], [5, 9, 1, 2], 1),
            (
                [i32::MIN, i32::MAX, i32::MIN, i32::MAX],
                [0, -1, 0, 1],
                [i32::MIN, -1, 0, i32::MAX],
                1,
            ),
        ] {
            let mut a = a;
            let mut b = b;
            let mut out = [0x5555_5555u32; 6];
            let p = out[1..].as_mut_ptr().cast::<u8>();
            assert_eq!(
                run(fc4p_005dc980, p, &[a.as_mut_ptr() as u32, b.as_mut_ptr() as u32]),
                [valid, p as u32, b[3] as u32]
            );
            assert_eq!(&out[1..5], &expected.map(|v| v as u32));
            assert_eq!((out[0], out[5]), (0x5555_5555, 0x5555_5555));
        }
    }
    #[test]
    fn retaining_wall_query_returns_its_interface_or_forwards_to_base() {
        let _query = fake(0x005e_89c0, query_base as *const () as usize);
        let table = [0, add_reference as *const () as usize];
        let mut object = [table.as_ptr() as u32, 0xdead_beef];
        let p = object.as_mut_ptr().cast::<u8>();
        let mut out = 0xfeed_faceu32;
        let q = &mut out as *mut u32 as u32;
        assert_eq!(
            run(fc4p_005e28c0, p, &[0x295d_1162, q]),
            [(RESULT[0] & !255) | 1, RESULT[1], RESULT[2]]
        );
        assert_eq!(out, p as u32);
        assert_eq!(
            calls(),
            [Call {
                address: 4,
                registers: [q, p as u32, table.as_ptr() as u32],
                args: vec![]
            }]
        );
        for iid in [0, 1, 0x295d_1161, u32::MAX] {
            assert_eq!(run(fc4p_005e28c0, p, &[iid, q]), RESULT);
            assert_eq!(
                calls(),
                [Call {
                    address: 0x005e_89c0,
                    registers: [iid, p as u32 + 4, EDX],
                    args: vec![iid, q]
                }]
            );
        }
    }
    #[test]
    fn prop_orientation_clamps_core_vectors_and_signed_extremes() {
        for (input, expected) in [
            (i32::MIN, 0),
            (-1, 0),
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 3),
            (i32::MAX, 3),
        ] {
            let mut bytes = [0xa5u8; 128];
            let p = bytes.as_mut_ptr();
            assert_eq!(run(fc4p_005ec9e0, p, &[input as u32]), [1, p as u32, EDX]);
            let mut wanted = [0xa5u8; 128];
            wanted[0x61] = expected;
            assert_eq!(bytes, wanted);
        }
    }
    #[test]
    fn occupant_interface_adjusts_nonnull_and_keeps_null() {
        let mut object = [0u32; 8];
        for p in [core::ptr::null_mut(), object.as_mut_ptr().cast::<u8>()] {
            assert_eq!(
                run(fc4p_005f8400, p, &[]),
                [if p.is_null() { 0 } else { p as u32 + 12 }, p as u32, EDX]
            );
        }
    }
    #[unsafe(naked)]
    unsafe extern "C" fn pool_allocate() {
        core::arch::naked_asm!(
            "sub esp, 12", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
            "lea eax, [esp + 16]", "push 1", "push eax", "lea eax, [esp + 8]",
            "push eax", "push 0x0090d8e0", "call {record}", "add esp, 28", "ret",
            record = sym record,
        );
    }
    stub!(pool_returned, 0x0090_d91c, 2, 0);
    stub!(pool_discarded, 0x0090_d031, 1, 4);
    stub!(pool_reopened, 0x0090_cff9, 1, 4);
    stub!(destruct, 0xd, 1, 4);

    #[test]
    fn every_pool_constructor_chains_slots_downward_without_touching_payloads() {
        let _fake = fake(0x0090_d8e0, pool_allocate as *const () as usize);
        let cases: &[(unsafe extern "C" fn(), u32, usize, usize, usize)] = &[
            (fc4p_005e5670, 11, 0x7f8, 254, 8),
            (fc4p_005e56d0, 12, 0xfd0, 127, 32),
            (fc4p_005e5730, 13, 0x1f90, 127, 64),
            (fc4p_005e5790, 14, 0x3f10, 127, 128),
            (fc4p_005e57d0, 15, 0x7e10, 127, 256),
            (fc4p_005e5830, 16, 0xfc10, 127, 512),
            (fc4p_005e5870, 16, 0xf810, 63, 1024),
            (fc4p_005e58b0, 16, 0xf010, 31, 2048),
        ];
        let mut storage = vec![0xa5u8; 131072];
        let offset = (65536 - (storage.as_ptr() as usize & 65535)) & 65535;
        let pool = &mut storage[offset..offset + 65536];
        let p = pool.as_mut_ptr();
        ALLOCATED.with(|a| a.set(p as u32));
        for &(entry, exponent, last, count, stride) in cases {
            pool.fill(0xa5);
            let mut expected = vec![0xa5u8; 65536];
            let mut previous = 0u32;
            for i in 0..count {
                let offset = last - i * stride;
                expected[offset..offset + 4].copy_from_slice(&previous.to_le_bytes());
                previous = p as u32 + offset as u32;
            }
            expected[8..12].copy_from_slice(&previous.to_le_bytes());
            expected[12..16].fill(0);
            assert_eq!(
                run(entry, 0x1122_3344 as *mut u8, &[]),
                [p as u32, previous.wrapping_sub(stride as u32), previous]
            );
            assert_eq!(&*pool, expected);
            assert_eq!(
                calls(),
                [Call {
                    address: 0x0090_d8e0,
                    registers: [EAX, 0x1122_3344, EDX],
                    args: vec![exponent]
                }]
            );
        }
    }

    #[test]
    fn pool_returns_pass_pointer_and_size_class_and_forward_all_results() {
        let _fake = fake(0x0090_d91c, pool_returned as *const () as usize);
        for (entry, exponent) in [
            (fc4p_005deb90 as unsafe extern "C" fn(), 14),
            (fc4p_005e56b0, 11),
            (fc4p_005e5710, 12),
            (fc4p_005e5810, 15),
        ] {
            for block in [0, 0x1234_5000, u32::MAX] {
                assert_eq!(run(entry, 0x1122_3344 as *mut u8, &[block]), RESULT);
                assert_eq!(
                    calls(),
                    [Call {
                        address: 0x0090_d91c,
                        registers: [block, 0x1122_3344, EDX],
                        args: vec![block, exponent]
                    }]
                );
            }
        }
    }

    #[test]
    fn pool_free_handles_null_wrapping_counts_and_both_block_transitions() {
        let _empty = fake(0x0090_d031, pool_discarded as *const () as usize);
        let _full = fake(0x0090_cff9, pool_reopened as *const () as usize);
        let cases: &[(unsafe extern "C" fn(), u32, usize)] = &[
            (fc4p_005e5a40, 0x4000_00be, 2048),
            (fc4p_005e5a90, 0x4000_005f, 4096),
            (fc4p_005e5ae0, 0x4000_005f, 8192),
            (fc4p_005e5b30, 0x4000_005f, 16384),
            (fc4p_005e5b80, 0x4000_005f, 32768),
            (fc4p_005e5bd0, 0x4000_005f, 65536),
            (fc4p_005e5c20, 0x4000_002f, 65536),
            (fc4p_005e5c70, 0x4000_0017, 65536),
        ];
        let mut storage = vec![0xa5u8; 131072];
        let offset = (65536 - (storage.as_ptr() as usize & 65535)) & 65535;
        let root = &mut storage[offset..offset + 65536];
        for &(entry, threshold, span) in cases {
            root.fill(0xa5);
            let base_offset = if span < 65536 { span } else { 0 };
            let pool = &mut root[base_offset..base_offset + span];
            let p = pool.as_mut_ptr();
            let slot = p as u32 + 0x20;
            assert_eq!(run(entry, 0x1122_3344 as *mut u8, &[0]), [EAX, 0x1122_3344, 0]);
            assert!(calls().is_empty());
            for count in [0u32, 1, 2, threshold, threshold + 1, u32::MAX] {
                pool.fill(0xa5);
                unsafe {
                    store(p, 8, 0xdead_beef);
                    store(p, 12, count);
                }
                let mut expected = pool.to_vec();
                expected[0x20..0x24].copy_from_slice(&0xdead_beefu32.to_le_bytes());
                expected[8..12].copy_from_slice(&slot.to_le_bytes());
                let next = count.wrapping_sub(1);
                expected[12..16].copy_from_slice(&next.to_le_bytes());
                let target = if next == 0 {
                    Some(0x0090_d031)
                } else if next == threshold {
                    Some(0x0090_cff9)
                } else {
                    None
                };
                let got = run(entry, 0x1122_3344 as *mut u8, &[slot]);
                assert_eq!(
                    got,
                    if target.is_some() {
                        RESULT
                    } else {
                        [p as u32, 0x1122_3344, next]
                    }
                );
                assert_eq!(&*pool, expected);
                let log = calls();
                if let Some(address) = target {
                    assert_eq!(
                        log,
                        [Call {
                            address,
                            registers: [p as u32, 0x1122_3344, next],
                            args: vec![p as u32]
                        }]
                    );
                    assert_eq!(
                        POOL_AT_CALL.with(|v| core::mem::take(&mut *v.borrow_mut())),
                        [expected[..0x40].to_vec()]
                    );
                } else {
                    assert!(log.is_empty());
                }
            }
            assert!(root[..base_offset].iter().all(|&v| v == 0xa5));
            assert!(root[base_offset + span..].iter().all(|&v| v == 0xa5));
        }
    }

    #[test]
    fn unsigned_reference_releases_keep_high_counts_and_destroy_zero_or_one() {
        for (entry, offset, slot, adjustment, short) in [
            (fc4p_005fd000 as unsafe extern "C" fn(), 0x10, 0x1fc, 4, true),
            (fc4p_005fd370, 4, 0x58, 0, false),
            (fc4p_00623270, 0x14, 0xcc, 0, false),
        ] {
            let mut table = [0usize; 128];
            table[slot / 4] = destruct as *const () as usize;
            for count in [0u32, 1, 2, 0x7fff, 0x8000, 0xffff, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
                let mut object = [0xa5a5_a5a5u32; 32];
                let p = object.as_mut_ptr().cast::<u8>();
                object[adjustment / 4] = table.as_ptr() as u32;
                unsafe {
                    if short {
                        write_unaligned(p.add(offset).cast::<u16>(), count as u16);
                    } else {
                        store(p, offset, count);
                    }
                }
                let mut expected = object;
                let effective = if short { count & 0xffff } else { count };
                let got = run(entry, p, &[]);
                if effective > 1 {
                    unsafe {
                        if short {
                            write_unaligned(
                                expected.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
                                (effective - 1) as u16,
                            );
                        } else {
                            write_unaligned(
                                expected.as_mut_ptr().cast::<u8>().add(offset).cast::<u32>(),
                                effective - 1,
                            );
                        }
                    }
                    assert_eq!(got, [effective - 1, p as u32, EDX]);
                    assert!(calls().is_empty());
                } else {
                    assert_eq!(got, [0, RESULT[1], RESULT[2]]);
                    assert_eq!(
                        calls(),
                        [Call {
                            address: 0xd,
                            registers: [table.as_ptr() as u32, p as u32 + adjustment as u32, EDX],
                            args: vec![1]
                        }]
                    );
                }
                assert_eq!(object, expected);
            }
        }
    }
    fn run_cdecl(entry: unsafe extern "C" fn(), args: &[u32]) -> [u32; 3] {
        let mut r = Registers {
            eax: EAX,
            ecx: 0x1122_3344,
            edx: EDX,
        };
        unsafe {
            invoke(&mut r, entry as *const () as usize, args, true);
        }
        [r.eax, r.ecx, r.edx]
    }

    #[test]
    fn packed_grid_bit_uses_signed_word_offset_and_column_bit_number() {
        for (row, column, pitch, offset, bit) in [
            (0, 0, 32, 0, 0),
            (1, 0, 33, 1, 0),
            (1, 31, 33, 2, 31),
            (-1, 0, 32, -1, 0),
            (0, -1, 32, -1, 31),
            (i32::MIN, 0, 2, 0, 0),
            (0, 32, 1, 1, 0),
            (i32::MAX, 2, 2, 0, 2),
        ] {
            let mut words = [0xa5a5_a5a5u32; 64];
            let p = words[16..].as_mut_ptr().cast::<u8>();
            let mut expected = words;
            expected[(16 + offset) as usize] |= 1 << bit;
            assert_eq!(
                run_cdecl(fc4p_00603da0, &[p as u32, row as u32, column as u32, pitch as u32]),
                [(p as u32).wrapping_add((offset * 4) as u32), bit, 1 << bit]
            );
            assert_eq!(words, expected);
        }
    }

    #[test]
    fn copy_backward_twelve_handles_overlap_empty_and_nonpositive_signed_ranges() {
        for bytes in [-13i32, -12, -11, -1, 0, 1, 11, 12, 13, 24, 36] {
            for overlap in [false, true] {
                let mut words = [0u32; 64];
                for (i, v) in words.iter_mut().enumerate() {
                    *v = i as u32;
                }
                let p = words.as_mut_ptr().cast::<u8>();
                let end_offset = 80 + bytes;
                let dest_offset = if overlap { end_offset + 4 } else { 160 };
                let mut expected = unsafe { core::slice::from_raw_parts(p, 256) }.to_vec();
                let count = bytes / 12;
                for n in 1..=count {
                    let source = (end_offset - n * 12) as usize;
                    let dest = (dest_offset - n * 12) as usize;
                    for j in 0..3 {
                        let word = expected[source + j * 4..source + j * 4 + 4].to_vec();
                        expected[dest + j * 4..dest + j * 4 + 4].copy_from_slice(&word);
                    }
                }
                let result = run_cdecl(
                    fc4p_00614430,
                    &[
                        p as u32 + 80,
                        (p as u32).wrapping_add(end_offset as u32),
                        p as u32 + dest_offset as u32,
                    ],
                );
                assert_eq!(unsafe { core::slice::from_raw_parts(p, 256) }, expected);
                assert_eq!(
                    result,
                    [
                        p as u32 + (dest_offset - count.max(0) * 12) as u32,
                        p as u32 + (end_offset - count.max(0) * 12) as u32,
                        if count > 0 {
                            0
                        } else if bytes < 0 {
                            (count - 1) as u32
                        } else {
                            0
                        }
                    ]
                );
            }
        }
    }

    #[test]
    fn forward_record_copy_and_fill_observe_each_overlapping_word() {
        for dest in [0usize, 4, 8, 12, 16, 64] {
            let mut words = [0u32; 64];
            for (i, v) in words.iter_mut().enumerate() {
                *v = 0x7f80_0000 + i as u32;
            }
            let p = words.as_mut_ptr().cast::<u8>();
            let mut expected = words;
            for n in 0..3 {
                for j in 0..3 {
                    expected[dest / 4 + n * 3 + j] = expected[n * 3 + j];
                }
            }
            assert_eq!(
                run_cdecl(
                    fc4p_00623fa0,
                    &[p as u32, p as u32 + 36, p as u32 + dest as u32, 0xabcdef01]
                ),
                [p as u32 + dest as u32 + 36, p as u32 + 36, p as u32 + 36]
            );
            assert_eq!(words, expected);
            for (entry, size) in [(fc4p_00623fe0 as unsafe extern "C" fn(), 12usize), (fc4p_00624020, 24)] {
                for count in [0u32, 1, 3] {
                    for (i, v) in words.iter_mut().enumerate() {
                        *v = 0x8000_0000 + i as u32;
                    }
                    let mut expected = words;
                    for n in 0..count as usize {
                        for j in 0..size / 4 {
                            expected[dest / 4 + n * size / 4 + j] = expected[j];
                        }
                    }
                    assert_eq!(
                        run_cdecl(entry, &[p as u32 + dest as u32, count, p as u32, 0]),
                        [
                            p as u32 + dest as u32 + count * size as u32,
                            0,
                            if count == 0 { EDX } else { p as u32 }
                        ]
                    );
                    assert_eq!(words, expected);
                }
            }
        }
        assert_eq!(run_cdecl(fc4p_00623fa0, &[0, 0, 0, 0]), [0, 0, 0]);
        assert_eq!(
            run_cdecl(fc4p_00623fa0, &[0x1111_1111, 0x1111_111d, 0, 0]),
            [12, 0x1111_111d, 0x1111_111d]
        );
        for (entry, size) in [(fc4p_00623fe0 as unsafe extern "C" fn(), 12), (fc4p_00624020, 24)] {
            assert_eq!(run_cdecl(entry, &[0, 0, 1, 0]), [0, 0, EDX]);
            assert_eq!(run_cdecl(entry, &[0, 1, 1, 0]), [size, 0, 1]);
        }
    }

    #[test]
    fn padded_record_copy_preserves_two_padding_bytes_and_empty_edx() {
        for dest in [0usize, 4, 6, 16, 64] {
            let mut bytes = [0u8; 128];
            for (i, v) in bytes.iter_mut().enumerate() {
                *v = i as u8;
            }
            let p = bytes.as_mut_ptr();
            let mut expected = bytes;
            let mut last = EDX;
            for n in 0..3 {
                for (offset, size) in [(0, 4), (4, 1), (5, 1), (8, 4), (12, 4)] {
                    for j in 0..size {
                        expected[dest + n * 16 + offset + j] = expected[n * 16 + offset + j];
                    }
                    if offset == 12 {
                        last = u32::from_le_bytes(expected[dest + n * 16 + 12..dest + n * 16 + 16].try_into().unwrap());
                    }
                }
            }
            assert_eq!(
                run_cdecl(fc4p_00624e60, &[p as u32, p as u32 + 48, p as u32 + dest as u32, 0]),
                [p as u32 + dest as u32 + 48, p as u32 + 48, last]
            );
            assert_eq!(bytes, expected);
        }
        assert_eq!(run_cdecl(fc4p_00624e60, &[0, 0, 0, 0]), [0, 0, EDX]);
        assert_eq!(run_cdecl(fc4p_00624e60, &[1, 17, 0, 0]), [16, 17, EDX]);
    }

    fn scan(bounds: [i32; 4], step: i32, x: i32, z: i32, state: u8) -> [u32; 12] {
        let mut object = [0xa5a5_a5a5u32; 12];
        object[0] = (object[0] & !255) | u32::from(state);
        object[1] = x as u32;
        object[2] = z as u32;
        object[3] = step as u32;
        object[4..8].copy_from_slice(&bounds.map(|v| v as u32));
        object
    }
    #[test]
    fn scan_reset_includes_edges_and_bounds_clamp_each_inverted_axis() {
        for (x, z, inside) in [
            (-1, 0, false),
            (0, -1, false),
            (0, 0, true),
            (2, 1, true),
            (3, 1, false),
            (2, 2, false),
        ] {
            let mut object = scan([0, 0, 2, 1], 1, 8, 9, 0x80);
            let p = object.as_mut_ptr().cast::<u8>();
            let mut expected = object;
            expected[1] = x as u32;
            expected[2] = z as u32;
            expected[0] = (expected[0] & !255) | if inside { 0 } else { 2 };
            assert_eq!(
                run(fc4p_0060c860, p, &[x as u32, z as u32]),
                [x as u32, p as u32, z as u32]
            );
            assert_eq!(object, expected);
        }
        for bounds in [[-5, -3, 8, 9], [4, 6, -2, 1], [i32::MAX, i32::MIN, i32::MIN, i32::MAX]] {
            let mut object = scan([0; 4], 1, 3, 4, 0);
            let p = object.as_mut_ptr().cast::<u8>();
            let mut input = bounds;
            let q = input.as_mut_ptr() as u32;
            let mut expected = object;
            expected[4..8].copy_from_slice(
                &[bounds[0], bounds[1], bounds[0].max(bounds[2]), bounds[1].max(bounds[3])].map(|v| v as u32),
            );
            assert_eq!(
                run(fc4p_0060c890, p, &[q]),
                [bounds[1] as u32, p as u32, bounds[3] as u32]
            );
            assert_eq!(object, expected);
        }
        // The source overlaps the newly written bounds, so subsequent source words change.
        let mut object = scan([1, 2, 3, 4], 7, 8, 9, 0);
        let p = object.as_mut_ptr().cast::<u8>();
        assert_eq!(run(fc4p_0060c890, p, &[p as u32 + 12]), [7, p as u32, 7]);
        assert_eq!(&object[4..8], &[7, 7, 7, 7]);
    }

    #[test]
    fn scan_core_order_and_outside_reset_leave_the_original_exhausted_coordinates() {
        let mut object = scan([0, 0, 2, 1], 1, 0, 0, 0);
        let p = object.as_mut_ptr().cast::<u8>();
        let mut out = [0xdead_beefu32; 2];
        let q = out.as_mut_ptr() as u32;
        for (x, z) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
            assert_eq!(run(fc4p_0060c8d0, p, &[q, q + 4]), [(z & !255) | 1, q + 4, q]);
            assert_eq!(out, [x, z]);
        }
        assert_eq!(run(fc4p_0060c8d0, p, &[q, q + 4]), [0, p as u32, 1]);
        assert_eq!((object[0] as u8, object[1], object[2]), (2, 0, 2));
        assert_eq!(out, [2, 1]);
        assert_eq!(run(fc4p_0060c8d0, p, &[q, q + 4]), [EAX & !255, p as u32, EDX]);
        run(fc4p_0060c860, p, &[5, 0]);
        assert_eq!(run(fc4p_0060c8d0, p, &[q, q + 4]), [EAX & !255, p as u32, EDX]);
    }

    #[test]
    fn scan_keeps_zero_negative_wrapping_and_unknown_state_behavior() {
        for (state, x, z, step, max_x, max_z, want_x, want_z, done) in [
            (1, 5, 0, 0, 10, 10, 5, 0, false),
            (1, 5, 0, -2, 10, 10, 3, 0, false),
            (1, i32::MAX, 0, 1, i32::MAX, 10, i32::MIN, 0, false),
            (1, 10, i32::MAX, 1, 10, i32::MAX, 0, i32::MIN, false),
            (0x80, 5, 0, 1, 10, 10, 6, 0, false),
            (1, 10, 0x1234, 1, 10, 0x1234, 0, 0x1235, true),
        ] {
            let mut object = scan([0, 0, max_x, max_z], step, x, z, state);
            let p = object.as_mut_ptr().cast::<u8>();
            let mut out = [0xdead_beefu32; 2];
            let q = out.as_mut_ptr() as u32;
            let got = run(fc4p_0060c8d0, p, &[q, q + 4]);
            assert_eq!((object[1], object[2]), (want_x as u32, want_z as u32));
            if done {
                assert_eq!(got, [want_z as u32 & !255, p as u32, max_z as u32]);
                assert_eq!(object[0] as u8, 2);
                assert_eq!(out, [0xdead_beef; 2]);
            } else {
                assert_eq!(got, [(want_z as u32 & !255) | 1, q + 4, q]);
                assert_eq!(object[0] as u8, state);
                assert_eq!(out, [want_x as u32, want_z as u32]);
            }
        }
        let mut object = scan([0, 0, 10, 10], 1, 5, 9, 0);
        let p = object.as_mut_ptr().cast::<u8>();
        let mut z = 0u32;
        let q = &mut z as *mut u32 as u32;
        assert_eq!(run(fc4p_0060c8d0, p, &[p as u32 + 8, q]), [1, q, p as u32 + 8]);
        assert_eq!(z, 5);
        assert_eq!(object[2], 5);
    }

    #[test]
    fn nested_accessors_follow_the_nested_pointer_and_pending_query_retains_edx() {
        let mut object = [0xa5a5_a5a5u32; 256];
        let p = object.as_mut_ptr().cast::<u8>();
        for value in [0, 1, 0x8000_0000, u32::MAX] {
            object[0x20c / 4] = value;
            assert_eq!(run(fc4p_0062c8f0, p, &[]), [u32::from(value != 0), p as u32, value]);
        }
        let mut nested = [0u32; 8];
        for (i, v) in nested.iter_mut().enumerate() {
            *v = 0xdead_0000 + i as u32;
        }
        unsafe {
            store(p, 0x358, nested.as_ptr() as u32);
        }
        for (entry, offset) in [
            (fc4p_0062c910 as unsafe extern "C" fn(), 2),
            (fc4p_0062c920, 4),
            (fc4p_0062c930, 5),
        ] {
            assert_eq!(run(entry, p, &[]), [nested[offset], p as u32, EDX]);
        }
    }

    stub!(pair_assignment, 0x0061_1b00, 1, 4);
    stub!(lookup, 0x005b_9fc0, 1, 4);
    stub!(shutdown, 0x10, 0, 0);
    stub!(final_destructor, 0x68, 1, 4);
    stub!(release_reference_inner, 8, 0, 0);
    #[unsafe(naked)]
    unsafe extern "C" fn release_reference() {
        core::arch::naked_asm!("call {body}", "mov eax, 0xdeadbeef", "ret", body = sym release_reference_inner);
    }
    #[test]
    fn embedded_assignment_and_retained_lookup_make_the_original_calls_in_order() {
        let _assign = fake(0x0061_1b00, pair_assignment as *const () as usize);
        let _lookup = fake(0x005b_9fc0, lookup as *const () as usize);
        let table = [
            0,
            add_reference as *const () as usize,
            release_reference as *const () as usize,
        ];
        let mut object = [table.as_ptr() as u32; 32];
        let p = object.as_mut_ptr().cast::<u8>();
        assert_eq!(run(fc4p_0062ca30, p, &[0x1234_5678]), RESULT);
        assert_eq!(
            calls(),
            [Call {
                address: 0x0061_1b00,
                registers: [EAX, 0x1234_5678, EDX],
                args: vec![p as u32 + 0x60]
            }]
        );
        assert_eq!(run(fc4p_005e55b0, p, &[0xfedc_ba98]), RESULT);
        assert_eq!(
            calls(),
            [
                Call {
                    address: 4,
                    registers: [table.as_ptr() as u32, p as u32, EDX],
                    args: vec![]
                },
                Call {
                    address: 0x005b_9fc0,
                    registers: [RESULT[0], p as u32 + 8, RESULT[2]],
                    args: vec![0xfedc_ba98]
                },
                Call {
                    address: 8,
                    registers: [RESULT[0], p as u32, table.as_ptr() as u32],
                    args: vec![]
                },
            ]
        );
    }
    #[test]
    fn final_release_calls_shutdown_then_deleting_destructor() {
        let mut table = [0usize; 32];
        table[4] = shutdown as *const () as usize;
        table[0x68 / 4] = final_destructor as *const () as usize;
        let mut next_table = [0usize; 32];
        next_table[0x68 / 4] = final_destructor as *const () as usize;
        table[0x68 / 4] = 0;
        NEXT_TABLE.with(|n| n.set(next_table.as_ptr() as u32));
        let mut object = [table.as_ptr() as u32; 8];
        let p = object.as_mut_ptr().cast::<u8>();
        assert_eq!(run(fc4p_005fb0e0, p, &[]), RESULT);
        assert_eq!(
            calls(),
            [
                Call {
                    address: 0x10,
                    registers: [table.as_ptr() as u32, p as u32, EDX],
                    args: vec![]
                },
                Call {
                    address: 0x68,
                    registers: [RESULT[0], p as u32, next_table.as_ptr() as u32],
                    args: vec![1]
                },
            ]
        );
    }
    fn environment() -> (u16, u16, u32, u32) {
        let (mut cw, mut mxcsr) = (0u16, 0u32);
        let (sw, flags): (u16, u32);
        unsafe {
            core::arch::asm!(
                "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
                cw=in(reg) &mut cw, mxcsr=in(reg) &mut mxcsr, flags=lateout(reg) flags, out("ax") sw,
            );
        }
        (cw, sw & 0x3800, mxcsr, flags & 0x400)
    }

    stub!(array_free, 0x0090_cf63, 1, 0);
    stub!(format_size, 0x008d_0370, 1, 0);
    stub!(remove_resource, 0x60, 1, 4);
    #[unsafe(naked)]
    unsafe extern "C" fn remove_resource_failed() {
        core::arch::naked_asm!(
            "sub esp, 12", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
            "lea eax, [esp + 16]", "push 1", "push eax", "lea eax, [esp + 8]",
            "push eax", "push 0x60", "call {record}", "add esp, 28",
            "mov eax, 0xa1b2c300", "mov ecx, 0x10203040", "mov edx, 0x55667788", "ret 4",
            record = sym record,
        );
    }

    macro_rules! value_stub {
        ($name:ident, $address:literal, $count:literal, $pop:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
                core::arch::naked_asm!(
                    "sub esp, 12", "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "lea eax, [esp + 16]", "push {count}", "push eax", "lea eax, [esp + 8]",
                    "push eax", "push {address}", "call {record}", "add esp, 28",
                    "mov ecx, 0x10203040", "mov edx, 0x55667788", "ret {pop}",
                    address = const $address, count = const $count, pop = const $pop, record = sym record,
                );
            }
        };
    }
    value_stub!(get_resource, 0x0c, 0, 0);
    value_stub!(array_allocate, 0x0090_cf54, 1, 0);
    stub!(pair_resize, 0x0051_fc30, 1, 4);
    stub!(variant_destroy, 0x0091_5eb5, 0, 0);
    stub!(string_tree_child, 0x0062_2190, 1, 4);
    stub!(grid_copy, 0x005e_4fa0, 3, 0);
    stub!(grid_reduce, 0x005e_4d60, 3, 0);
    stub!(string_assign, 0x0044_d3a0, 2, 8);
    stub!(variant_query, 0x0091_4e56, 2, 8);
    value_stub!(object_allocate, 0x005e_55e0, 1, 0);
    value_stub!(proxy_construct, 0x0064_5d50, 0, 0);
    stub!(proxy_bind, 0x0064_5240, 2, 8);
    stub!(message_target_construct, 0x005e_a080, 0, 0);
    stub!(notify_close, 0x18, 11, 44);
    stub!(close_window, 0x44, 0, 0);

    unsafe extern "thiscall" fn insert_range(this: *mut u8, at: u32, first: u32, last: u32, tag: *mut u32) -> u64 {
        assert_eq!(unsafe { *tag }, last);
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x005c2470,
                registers: [0, this as u32, 0],
                args: vec![at, first, last, unsafe { *tag }],
            })
        });
        0x5566_7788_a1b2_c3d4
    }
    #[test]
    fn string_insertion_adapter_passes_original_last_word_as_allocator_tag() {
        let _insert = fake(0x005c_2470, insert_range as *const () as usize);
        let mut object = [0xfeed_faceu32; 4];
        let p = object.as_mut_ptr().cast();
        for args in [[0u32; 3], [0x1234, 0x5678, 0x9abc], [u32::MAX, 4, 2]] {
            let result = run(fc4p_005c2910, p, &args);
            assert_eq!((result[0], result[2]), (0xa1b2c3d4, 0x55667788));
            let log = calls();
            assert_eq!(log.len(), 1);
            assert_eq!(log[0].address, 0x005c2470);
            assert_eq!(log[0].registers[1], p as u32);
            assert_eq!(log[0].args, [args[0], args[1], args[2], args[2]]);
            assert_eq!(object, [0xfeed_face; 4]);
        }
    }

    fn center_component_reference(first: u32, second: u32, scale: f32) -> u32 {
        let mut result = 0u32;
        unsafe {
            core::arch::asm!(
                "fld dword ptr [{first}]", "fadd dword ptr [{second}]", "fmul dword ptr [{scale}]", "fstp dword ptr [{result}]",
                first = in(reg) &first, second = in(reg) &second, scale = in(reg) &scale,
                result = in(reg) &mut result, out("st(0)") _,
            );
        }
        result
    }
    fn float_status() -> u16 {
        let result: u16;
        unsafe {
            core::arch::asm!("fnstsw ax", out("ax") result, options(nomem, nostack));
        }
        result
    }
    #[test]
    fn bounds_center_keeps_extended_exponents_nan_bits_alias_order_and_float_environment() {
        let _fpu = GameFpu::new();
        let mut scale = 0.5f32;
        let _scale = crate::testing::fake_data(0x00a8_4d2c, (&mut scale as *mut f32).cast());
        for (first, second) in [
            (f32::MAX.to_bits(), f32::MAX.to_bits()),
            (0x80000000, 0x80000000),
            (0x00000001, 0x00000002),
            (0x7f800001, 0x7fc54321),
            (0xffc12345, 0x7f812345),
            (1.0f32.to_bits(), (-1.0f32).to_bits()),
        ] {
            for offset in [0usize, 0x34, 0x38, 0x40, 0x48] {
                let mut object = [0xcccc_ccccu32; 24];
                object[0x40 / 4] = first;
                object[0x34 / 4] = second;
                object[0x38 / 4] = 0xff812345;
                object[0x48 / 4] = second;
                object[0x3c / 4] = first;
                let mut expected = object;
                let environment_before = environment();
                unsafe {
                    core::arch::asm!("fnclex", options(nomem, nostack));
                }
                expected[offset / 4] = center_component_reference(expected[0x40 / 4], expected[0x34 / 4], scale);
                let y = expected[0x38 / 4];
                expected[offset / 4 + 1] = y;
                expected[offset / 4 + 2] = center_component_reference(expected[0x48 / 4], expected[0x3c / 4], scale);
                let expected_status = float_status();
                unsafe {
                    core::arch::asm!("fnclex", options(nomem, nostack));
                }
                let p = object.as_mut_ptr().cast::<u8>();
                let out = unsafe { p.add(offset) };
                let result = run(fc4p_005ed1f0, p, &[out as u32]);
                assert_eq!(float_status(), expected_status);
                assert_eq!(object, expected);
                assert_eq!(result, [out as u32, p as u32, y]);
                assert_eq!(environment(), environment_before);
            }
        }
        assert_eq!(
            center_component_reference(f32::MAX.to_bits(), f32::MAX.to_bits(), scale),
            f32::MAX.to_bits()
        );
    }

    #[test]
    fn string_count_adapter_wraps_end_and_property_query_adjusts_only_delegation() {
        let _assign = fake(0x0044_d3a0, string_assign as *const () as usize);
        let _query = fake(0x0091_4e56, variant_query as *const () as usize);
        let table = [0usize, add_reference as *const () as usize];
        let mut object = [table.as_ptr() as u32, 0xfeed_face];
        let p = object.as_mut_ptr().cast();
        for (first, count) in [(0u32, 0u32), (0x1234, 9), (u32::MAX - 3, 8)] {
            assert_eq!(run(fc4p_005fa540, p, &[first, count]), RESULT);
            assert_eq!(
                calls(),
                [Call {
                    address: 0x0044d3a0,
                    registers: [first, p as u32, first.wrapping_add(count)],
                    args: vec![first, first.wrapping_add(count)]
                }]
            );
        }
        for iid in [0, 1, 0x4521_60ec, 0x4521_60ed, u32::MAX] {
            let mut out = 0xdead_beefu32;
            let out_pointer = &mut out as *mut u32 as u32;
            let result = run(fc4p_005fd030, p, &[iid, out_pointer]);
            let log = calls();
            assert_eq!(log.len(), 1);
            if [1, 0x4521_60ec].contains(&iid) {
                assert_eq!(out, p as u32);
                assert_eq!(result, [RESULT[0] & !255 | 1, RESULT[1], RESULT[2]]);
                assert_eq!(
                    log[0],
                    Call {
                        address: 4,
                        registers: [table.as_ptr() as u32, p as u32, out_pointer],
                        args: vec![]
                    }
                );
            } else {
                assert_eq!(out, 0xdead_beef);
                assert_eq!(result, RESULT);
                assert_eq!(
                    log[0],
                    Call {
                        address: 0x00914e56,
                        registers: [iid.wrapping_sub(0x4521_60ec), p as u32 + 4, iid],
                        args: vec![iid, out_pointer]
                    }
                );
            }
        }
    }

    extern "thiscall" fn grid_height(this: *mut u8) -> u32 {
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x34,
                registers: [0, this as u32, 0],
                args: vec![],
            })
        });
        13
    }
    extern "thiscall" fn grid_width(this: *mut u8) -> u32 {
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x30,
                registers: [0, this as u32, 0],
                args: vec![],
            })
        });
        11
    }
    extern "thiscall" fn grid_storage(this: *mut u8) -> u32 {
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x60,
                registers: [0, this as u32, 0],
                args: vec![],
            })
        });
        0x12345678
    }
    #[test]
    fn grid_copy_adapter_skips_nulls_and_queries_height_width_storage_in_order() {
        let _copy = fake(0x005e_4fa0, grid_copy as *const () as usize);
        let _reduce = fake(0x005e_4d60, grid_reduce as *const () as usize);
        let mut table = [0usize; 25];
        table[0x34 / 4] = grid_height as *const () as usize;
        table[0x30 / 4] = grid_width as *const () as usize;
        table[0x60 / 4] = grid_storage as *const () as usize;
        let mut object = [table.as_ptr() as u32];
        let p = object.as_mut_ptr() as u32;
        for (destination, source) in [(0, 0), (p, 0), (0, 0x5678), (p, 0x5678)] {
            let result = run_cdecl(fc4p_005e5130, &[destination, source, 0xabcd]);
            let log = calls();
            if destination == 0 || source == 0 {
                assert_eq!(result, [source, 0x1122_3344, EDX]);
                assert!(log.is_empty());
            } else {
                assert_eq!(result, RESULT);
                assert_eq!(
                    log.iter().map(|c| c.address).collect::<Vec<_>>(),
                    [0x005e4fa0, 0x34, 0x30, 0x60, 0x005e4d60]
                );
                assert_eq!(log[0].args, [p, source, 0xabcd]);
                assert_eq!(log[0].registers, [source, 0xabcd, EDX]);
                assert_eq!(log[4].args, [0x12345678, 11, 13]);
            }
        }
    }

    #[test]
    fn window_proxy_allocation_constructs_retains_binds_then_publishes() {
        let _allocate = fake(0x005e_55e0, object_allocate as *const () as usize);
        let _construct = fake(0x0064_5d50, proxy_construct as *const () as usize);
        let _bind = fake(0x0064_5240, proxy_bind as *const () as usize);
        let table = [0usize, add_reference as *const () as usize];
        let mut proxy = [table.as_ptr() as u32];
        let proxy_pointer = proxy.as_mut_ptr() as u32;
        ALLOCATED.with(|v| v.set(proxy_pointer));
        let mut object = [0xfeed_faceu32; 0x360 / 4];
        object[0x35c / 4] = 0x12345678;
        let p = object.as_mut_ptr().cast();
        let mut out = 0u32;
        let result = run(fc4p_00623b20, p, &[&mut out as *mut u32 as u32]);
        assert_eq!(out, proxy_pointer);
        assert_eq!(result, [RESULT[0] & !255 | 1, RESULT[1], &mut out as *mut u32 as u32]);
        let log = calls();
        assert_eq!(
            log.iter().map(|c| c.address).collect::<Vec<_>>(),
            [0x005e55e0, 0x00645d50, 4, 0x00645240]
        );
        assert_eq!(log[0].args, [0x134]);
        assert_eq!(log[1].registers[1], proxy_pointer);
        assert_eq!(log[2].registers[1], proxy_pointer);
        assert_eq!(log[3].registers[1], proxy_pointer);
        assert_eq!(log[3].args, [p as u32, 0x12345678]);
    }

    #[test]
    fn closing_window_notifies_only_with_target_and_always_calls_close() {
        let _allocate = fake(0x005e_55e0, object_allocate as *const () as usize);
        let _construct = fake(0x005e_a080, message_target_construct as *const () as usize);
        let mut server_table = [0usize; 7];
        server_table[6] = notify_close as *const () as usize;
        let mut server = [server_table.as_ptr() as u32];
        let mut server_pointer = server.as_mut_ptr().cast::<u8>();
        let _server = crate::testing::fake_data(0x00b4_3d00, (&mut server_pointer as *mut *mut u8).cast());
        let mut window_table = [0usize; 18];
        window_table[17] = close_window as *const () as usize;
        for destination in [0u32, 0x12345678] {
            for allocate in [false, true] {
                let mut target = [0xcccc_ccccu32; 3];
                let target_pointer = if allocate { target.as_mut_ptr() as u32 } else { 0 };
                ALLOCATED.with(|v| v.set(target_pointer));
                let mut object = [0xfeed_faceu32; 0x214 / 4];
                object[0] = window_table.as_ptr() as u32;
                object[0x210 / 4] = destination;
                let before = object;
                assert_eq!(run(fc4p_00624920, object.as_mut_ptr().cast(), &[]), RESULT);
                assert_eq!(object, before);
                let log = calls();
                let expected = if destination == 0 {
                    vec![0x44]
                } else if allocate {
                    vec![0x005e55e0, 0x005ea080, 0x18, 0x44]
                } else {
                    vec![0x005e55e0, 0x18, 0x44]
                };
                assert_eq!(log.iter().map(|c| c.address).collect::<Vec<_>>(), expected);
                if destination != 0 {
                    assert_eq!(log[0].args, [12]);
                    let notify = &log[log.len() - 2];
                    assert_eq!(notify.registers[1], server_pointer as u32);
                    assert_eq!(
                        notify.args,
                        [1, destination, 1, 0x2001, 0, target_pointer, 0, 0, 0, 0, 0]
                    );
                    if allocate {
                        assert_eq!(target, [0xaa825c, 0xaa8244, 0xcccccccc]);
                    }
                }
                assert_eq!(log.last().unwrap().registers[1], object.as_mut_ptr() as u32);
            }
        }
    }

    unsafe fn transfer_plain_pairs(address: u32, first: u32, last: u32, out: u32, tag: *mut u32) -> u32 {
        assert_eq!(unsafe { *tag }, PAIR_TAG.with(Cell::get));
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address,
                registers: [0; 3],
                args: vec![first, last, out, unsafe { *tag }],
            })
        });
        let mut p = first;
        let mut q = out;
        while p != last {
            unsafe {
                store(q as *mut u8, 0, word(p as *mut u8, 0));
            }
            p += 4;
            q += 4;
        }
        q
    }
    unsafe extern "C" fn assign_plain_pairs(first: u32, last: u32, out: u32, tag: *mut u32) -> u32 {
        unsafe { transfer_plain_pairs(0x00463df0, first, last, out, tag) }
    }
    unsafe extern "C" fn construct_plain_pairs(first: u32, last: u32, out: u32, tag: *mut u32) -> u32 {
        unsafe { transfer_plain_pairs(0x0063bff0, first, last, out, tag) }
    }
    unsafe extern "thiscall" fn allocate_plain_pairs(this: *mut u8, count: u32, first: u32, last: u32) -> u32 {
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x007cec30,
                registers: [0, this as u32, 0],
                args: vec![count, first, last],
            })
        });
        let out = ALLOCATED.with(Cell::get);
        assert_eq!(last.wrapping_sub(first), count * 8);
        for offset in (0..count * 8).step_by(4) {
            unsafe {
                store(out as *mut u8, offset as usize, word(first as *mut u8, offset as usize));
            }
        }
        out
    }
    #[test]
    fn pair_vector_assignment_handles_self_capacity_allocation_and_tail_construction() {
        let _allocate = fake(0x007c_ec30, allocate_plain_pairs as *const () as usize);
        let _assign = fake(0x0046_3df0, assign_plain_pairs as *const () as usize);
        let _construct = fake(0x0063_bff0, construct_plain_pairs as *const () as usize);
        let _free = fake(0x0090_cf63, array_free as *const () as usize);
        for count in [0u32, 1, 3] {
            for capacity in [0u32, 2, 5] {
                for length in [0u32, capacity.min(2), capacity] {
                    let mut input = [1u32, 2, 3, 4, 5, 6];
                    let mut old = [0xcccc_ccccu32; 10];
                    let mut new = [0xdddd_ddddu32; 6];
                    let first = input.as_mut_ptr() as u32;
                    let mut source = [first, first + count * 8, first + 24];
                    let source_pointer = source.as_mut_ptr() as u32;
                    let out = if capacity == 0 { 0 } else { old.as_mut_ptr() as u32 };
                    let allocation = new.as_mut_ptr() as u32;
                    let mut vector = [out, out + length * 8, out + capacity * 8];
                    let p = vector.as_mut_ptr().cast();
                    ALLOCATED.with(|v| v.set(allocation));
                    PAIR_TAG.with(|v| v.set(source_pointer));
                    let result = run(fc4p_00611b00, p, &[source_pointer]);
                    let log = calls();
                    if count > capacity {
                        assert_eq!(
                            log.iter().map(|c| c.address).collect::<Vec<_>>(),
                            if out == 0 {
                                vec![0x007cec30]
                            } else {
                                vec![0x007cec30, 0x0090cf63]
                            }
                        );
                        assert_eq!(vector, [allocation, allocation + count * 8, allocation + count * 8]);
                        assert_eq!(&new[..count as usize * 2], &input[..count as usize * 2]);
                        assert_eq!(old, [0xcccccccc; 10]);
                        assert_eq!(result, [p as u32, allocation + count * 8, allocation + count * 8]);
                    } else {
                        assert_eq!(
                            log.iter().map(|c| c.address).collect::<Vec<_>>(),
                            if length >= count {
                                vec![0x00463df0]
                            } else {
                                vec![0x00463df0, 0x0063bff0]
                            }
                        );
                        assert_eq!(vector, [out, out + count * 8, out + capacity * 8]);
                        assert_eq!(&old[..count as usize * 2], &input[..count as usize * 2]);
                        assert!(old[count as usize * 2..].iter().all(|&v| v == 0xcccccccc));
                        assert_eq!(result[0..2], [p as u32, out + count * 8]);
                    }
                    let before = vector;
                    assert_eq!(run(fc4p_00611b00, p, &[p as u32]), [p as u32, p as u32, EDX]);
                    assert!(calls().is_empty());
                    assert_eq!(vector, before);
                }
            }
        }
    }

    #[test]
    fn record_pair_copy_resizes_only_for_different_signed_lengths() {
        let _resize = fake(0x0051_fc30, pair_resize as *const () as usize);
        for count in [-1i32, 0, 1, 3] {
            for same_length in [false, true] {
                let mut records = [0u32; 15];
                for (i, v) in records.iter_mut().enumerate() {
                    *v = 0x100 + i as u32;
                }
                let mut pairs = [0xfeed_faceu32; 8];
                let out = pairs.as_mut_ptr() as u32;
                let mut destination = [
                    out,
                    out.wrapping_add(if same_length { count as u32 } else { 7 }.wrapping_mul(8)),
                    out + 32,
                ];
                let mut source = [records.as_mut_ptr() as u32, 0xabc, 0xdef, count as u32];
                let before = destination;
                let result = run(
                    fc4p_005f4760,
                    source.as_mut_ptr().cast(),
                    &[destination.as_mut_ptr() as u32],
                );
                let log = calls();
                assert_eq!(log.len(), usize::from(!same_length));
                if !same_length {
                    assert_eq!(
                        (log[0].address, log[0].registers[1], log[0].args.as_slice()),
                        (0x0051fc30, destination.as_mut_ptr() as u32, &[count as u32][..])
                    );
                }
                for i in 0..count.max(0) as usize {
                    assert_eq!(&pairs[i * 2..i * 2 + 2], &records[i * 5..i * 5 + 2]);
                }
                assert!(pairs[count.max(0) as usize * 2..].iter().all(|&v| v == 0xfeed_face));
                assert_eq!(destination, before);
                assert_eq!(
                    result,
                    [
                        count as u32,
                        count.max(0) as u32,
                        if count > 0 {
                            count as u32 * 20
                        } else if same_length {
                            EDX
                        } else {
                            RESULT[2]
                        }
                    ]
                );
            }
        }
    }

    #[test]
    fn pooled_variant_deletion_restores_tables_and_handles_count_transitions() {
        let _destroy = fake(0x0091_5eb5, variant_destroy as *const () as usize);
        let _discard = fake(0x0090_d031, pool_discarded as *const () as usize);
        let _reopen = fake(0x0090_cff9, pool_reopened as *const () as usize);
        let mut manager = 0u8;
        let _manager = crate::testing::fake_data(0x00b0_a9e8, &mut manager);
        // This block has the alignment used by the game's size class.
        #[repr(C, align(2048))]
        struct Block([u32; 512]);
        let mut block = Box::new(Block([0xcccc_cccc; 512]));
        let base = block.0.as_mut_ptr() as u32;
        let this = (base + 0x100) as *mut u8;
        for flags in [0, 2, 1, 3, 0x100, u32::MAX] {
            for count in [0u32, 1, 2, 0x4000_0037] {
                block.0.fill(0xcccc_cccc);
                block.0[2] = 0x12345678;
                block.0[3] = count;
                let before = block.0;
                let result = run(fc4p_005fe980, this, &[flags]);
                let log = calls();
                assert_eq!(log[0].address, 0x00915eb5);
                assert_eq!(log[0].registers[1], this as u32 + 4);
                let mut expected = before;
                expected[0x100 / 4] = 0xaa6988;
                expected[0x104 / 4] = 0xaa6788;
                let mut registers = [this as u32, RESULT[1], RESULT[2]];
                if flags & 1 != 0 {
                    expected[0x100 / 4] = before[2];
                    expected[2] = this as u32;
                    expected[3] = count.wrapping_sub(1);
                    registers[1] = count.wrapping_sub(1);
                    if [1, 0x4000_0037].contains(&count) {
                        registers[1] = RESULT[1];
                        assert_eq!(log[1].address, if count == 1 { 0x0090d031 } else { 0x0090cff9 });
                        assert_eq!(log[1].args, [base]);
                        assert_eq!(log[1].registers[1], &mut manager as *mut u8 as u32);
                    }
                }
                assert_eq!(block.0, expected);
                assert_eq!(result, registers);
                assert_eq!(
                    log.len(),
                    1 + usize::from(flags & 1 != 0 && [1, 0x4000_0037].contains(&count))
                );
            }
        }
    }

    thread_local! {
        static PAIR_VECTOR: Cell<u32> = const { Cell::new(0) };
        static PAIR_TAG: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn construct_pairs(first: u32, last: u32, out: u32, tag: *mut u32) -> u32 {
        let vector = PAIR_VECTOR.with(Cell::get) as *mut u8;
        unsafe {
            assert_eq!(*tag, PAIR_TAG.with(Cell::get));
            assert_eq!((word(vector, 0), word(vector, 4)), (out, out));
            CALLS.with(|v| {
                v.borrow_mut().push(Call {
                    address: 0x0077ac20,
                    registers: [0; 3],
                    args: vec![first, last, out, *tag],
                })
            });
            let mut p = first;
            let mut q = out;
            while p != last {
                store(q as *mut u8, 0, word(p as *mut u8, 0));
                p += 4;
                q += 4;
            }
            q
        }
    }
    #[test]
    fn property_vector_copy_calls_allocator_and_constructor_even_for_empty_or_self_source() {
        let _allocate = fake(0x0090_cf54, array_allocate as *const () as usize);
        let _construct = fake(0x0077_ac20, construct_pairs as *const () as usize);
        for count in [0u32, 1, 3] {
            for same in [false, true] {
                let mut input = [1u32, 2, 3, 4, 5, 6];
                let mut output = [0xcccc_ccccu32; 8];
                let mut object = [0xfeed_faceu32; 5];
                let p = unsafe { object.as_mut_ptr().add(1).cast::<u8>() };
                let first = input.as_mut_ptr() as u32;
                let mut source = [first, first + count * 8, first + 24];
                if same {
                    object[1..4].copy_from_slice(&source);
                }
                let source_pointer = if same { p as u32 } else { source.as_mut_ptr() as u32 };
                let out = if count == 0 { 0 } else { output.as_mut_ptr() as u32 };
                ALLOCATED.with(|v| v.set(out));
                PAIR_VECTOR.with(|v| v.set(p as u32));
                PAIR_TAG.with(|v| v.set(source_pointer));
                let result = run(fc4p_005fea70, p, &[source_pointer]);
                let log = calls();
                assert_eq!(log.len(), 1 + usize::from(count != 0));
                if count != 0 {
                    assert_eq!(log[0].args, [count * 8]);
                }
                assert_eq!(
                    log.last().unwrap().args,
                    if same {
                        vec![out, out, out, source_pointer]
                    } else {
                        vec![first, first + count * 8, out, source_pointer]
                    }
                );
                assert_eq!(
                    object,
                    [
                        0xfeed_face,
                        out,
                        out + if same { 0 } else { count * 8 },
                        out + count * 8,
                        0xfeed_face
                    ]
                );
                assert_eq!(result[0], p as u32);
                if !same {
                    assert_eq!(&output[..count as usize * 2], &input[..count as usize * 2]);
                }
                assert!(output[if same { 0 } else { count as usize * 2 }..]
                    .iter()
                    .all(|&v| v == 0xcccc_cccc));
            }
        }
    }

    unsafe extern "C" fn shift_pairs(first: u32, last: u32, out: u32, tag: *mut u32, unused: u32) -> u32 {
        assert_eq!(unsafe { *tag }, out);
        assert_eq!(unused, 0);
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x0077a120,
                registers: [0; 3],
                args: vec![first, last, out, unused],
            })
        });
        let mut p = first;
        let mut q = out;
        while p != last {
            unsafe {
                store(q as *mut u8, 0, word(p as *mut u8, 0));
            }
            p += 4;
            q += 4;
        }
        0xdeadbeef
    }
    #[test]
    fn property_erase_shifts_before_releasing_last_pointer_and_handles_null() {
        let _shift = fake(0x0077_a120, shift_pairs as *const () as usize);
        let table = [0usize, 0, release_reference_inner as *const () as usize];
        let mut resource = [table.as_ptr() as u32];
        for position in 0..3 {
            for nonnull in [false, true] {
                let last = if nonnull { resource.as_mut_ptr() as u32 } else { 0 };
                let mut data = [1u32, 0x1111, 2, 0x2222, 3, last, 0xfeed_face, 0xfeed_face];
                let base = data.as_mut_ptr() as u32;
                let mut vector = [base, base + 24, base + 32];
                let erased = base + position * 8;
                let result = run(fc4p_005fead0, vector.as_mut_ptr().cast(), &[erased]);
                let log = calls();
                assert_eq!(vector, [base, base + 16, base + 32]);
                assert_eq!(result[0], erased);
                assert_eq!(log.len(), usize::from(position != 2) + usize::from(nonnull));
                if position != 2 {
                    assert_eq!(log[0].args, [erased + 8, base + 24, erased, 0]);
                }
                if nonnull {
                    assert_eq!(
                        (log.last().unwrap().address, log.last().unwrap().registers[1]),
                        (8, last)
                    );
                }
                let expected = match position {
                    0 => [2, 0x2222, 3, last, 3, last, 0xfeed_face, 0xfeed_face],
                    1 => [1, 0x1111, 3, last, 3, last, 0xfeed_face, 0xfeed_face],
                    _ => [1, 0x1111, 2, 0x2222, 3, last, 0xfeed_face, 0xfeed_face],
                };
                assert_eq!(data, expected);
            }
        }
    }

    #[test]
    fn string_tree_cleanup_calls_right_subtree_then_buffers_then_node_and_walks_left() {
        let _child = fake(0x0062_2190, string_tree_child as *const () as usize);
        let _free = fake(0x0090_cf63, array_free as *const () as usize);
        let mut owner = [0xfeed_faceu32; 4];
        let this = owner.as_mut_ptr().cast();
        assert_eq!(run(fc4p_00622190, this, &[0]), [EAX, this as u32, EDX]);
        assert!(calls().is_empty());
        for mask in 0..16 {
            let mut first = [0xcccc_ccccu32; 12];
            let mut second = [0xdddd_ddddu32; 12];
            let first_pointer = first.as_mut_ptr() as u32;
            let second_pointer = second.as_mut_ptr() as u32;
            first[2] = second_pointer;
            first[3] = 0x3333;
            second[2] = 0;
            second[3] = 0;
            first[9] = if mask & 1 != 0 { 0x1111 } else { 0 };
            first[6] = if mask & 2 != 0 { 0x2222 } else { 0 };
            second[9] = if mask & 4 != 0 { 0x4444 } else { 0 };
            second[6] = if mask & 8 != 0 { 0x5555 } else { 0 };
            let before = (first, second);
            assert_eq!(run(fc4p_00622190, this, &[first_pointer]), RESULT);
            let log = calls();
            let expected: Vec<_> = [
                (0x00622190, 0x3333),
                (0x0090cf63, first[9]),
                (0x0090cf63, first[6]),
                (0x0090cf63, first_pointer),
                (0x00622190, 0),
                (0x0090cf63, second[9]),
                (0x0090cf63, second[6]),
                (0x0090cf63, second_pointer),
            ]
            .into_iter()
            .filter(|&(address, value)| address == 0x00622190 || value != 0)
            .collect();
            assert_eq!(log.iter().map(|c| (c.address, c.args[0])).collect::<Vec<_>>(), expected);
            for c in log.iter().filter(|c| c.address == 0x00622190) {
                assert_eq!(c.registers[1], this as u32);
            }
            assert_eq!((first, second), before);
        }
    }

    #[test]
    fn quad_batch_initialization_and_configuration_only_change_owned_fields() {
        let _format = fake(0x008d_0370, format_size as *const () as usize);
        let mut object = [0xfeed_faceu32; 18];
        let p = unsafe { object.as_mut_ptr().add(1).cast::<u8>() };
        assert_eq!(run(fc4p_005ec690, p, &[]), [p as u32, 0, EDX]);
        assert_eq!(
            object,
            [
                0xfeed_face,
                0xaa44ac,
                6,
                4,
                0,
                10,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0xfeed_face
            ]
        );
        object[6] = 0x1111;
        object[7] = 0x2222;
        object[8] = 0x3333;
        object[12] = 0x4444;
        object[13] = 0x5555;
        object[14] = 0x6666;
        let before = object;
        let result = run(fc4p_005ec7c0, p, &[0x81, 0x82, 0x83, 0x84, 0x85, 0x86]);
        let log = calls();
        assert_eq!(
            log,
            [Call {
                address: 0x008d0370,
                registers: [0x84, 0x82, 0x83],
                args: vec![0x84]
            }]
        );
        let mut expected = before;
        for (offset, value) in [
            (4, 0x81),
            (8, 0x82),
            (12, 0x83),
            (16, 0x84),
            (0x20, RESULT[0]),
            (0x24, 0),
            (0x28, 0x85),
            (0x3c, 0x86),
            (0x38, 0),
            (0x30, 0x4444),
            (0x18, 0x1111),
        ] {
            expected[1 + offset / 4] = value;
        }
        assert_eq!(object, expected);
        assert_eq!(result, [0x2222, 0x1111, RESULT[2]]);
    }

    #[test]
    fn quad_indices_fill_only_requested_range_and_wrap_to_sixteen_bits() {
        for (first, quads) in [(0, 0), (0, 1), (1, 1), (6, 3), (98_304, 2)] {
            let mut indices = vec![0xbeefu16; first + quads * 6 + 2];
            let base = indices.as_mut_ptr() as u32;
            let last = first + quads * 6;
            let result = run_cdecl(fc4p_005ec430, &[base, first as u32, last as u32]);
            assert!(indices[..first].iter().all(|&v| v == 0xbeef));
            assert!(indices[last..].iter().all(|&v| v == 0xbeef));
            let start = first as u32 * 4 / 6;
            for q in 0..quads {
                let v = start.wrapping_add(q as u32 * 4) as u16;
                assert_eq!(
                    &indices[first + q * 6..first + q * 6 + 6],
                    &[
                        v,
                        v.wrapping_add(1),
                        v.wrapping_add(2),
                        v,
                        v.wrapping_add(2),
                        v.wrapping_add(3)
                    ]
                );
            }
            assert_eq!(
                result,
                [
                    base + last as u32 * 2,
                    if quads == 0 { base } else { start + quads as u32 * 4 - 1 },
                    start + quads as u32 * 4
                ]
            );
        }
    }

    #[test]
    fn array_cleanup_skips_nulls_keeps_fields_and_preserves_last_result() {
        let _free = fake(0x0090_cf63, array_free as *const () as usize);
        for mask in 0..32 {
            let mut object = [0x7777_7777u32; 44];
            let p = object.as_mut_ptr().cast();
            let offsets = [0xa0, 0x6c, 0x60, 0x54, 0x28];
            for (i, offset) in offsets.into_iter().enumerate() {
                object[offset / 4] = if mask & (1 << i) == 0 { 0 } else { 0x1000 + i as u32 };
            }
            let before = object;
            let result = run(fc4p_00624f70, p, &[]);
            let log = calls();
            assert_eq!(
                log.iter().map(|c| c.args[0]).collect::<Vec<_>>(),
                offsets
                    .into_iter()
                    .map(|o| object[o / 4])
                    .filter(|&p| p != 0)
                    .collect::<Vec<_>>()
            );
            assert!(log.iter().all(|c| c.address == 0x0090cf63));
            assert_eq!(object, before);
            assert_eq!(result[0], if mask & 16 != 0 || mask & 8 != 0 { RESULT[0] } else { 0 });
        }
        for indices in [0, 0x1234] {
            for vertices in [0, 0x5678] {
                let mut object = [0x7777_7777u32; 16];
                object[0x2c / 4] = indices;
                object[0x14 / 4] = vertices;
                let mut expected = object;
                expected[0] = 0xaa44ac;
                let result = run(fc4p_005ec6e0, object.as_mut_ptr().cast(), &[]);
                assert_eq!(
                    calls().iter().map(|c| c.args[0]).collect::<Vec<_>>(),
                    [indices, vertices].into_iter().filter(|&p| p != 0).collect::<Vec<_>>()
                );
                assert_eq!(object, expected);
                assert_eq!(
                    result,
                    if indices != 0 || vertices != 0 {
                        RESULT
                    } else {
                        [0, object.as_mut_ptr() as u32, EDX]
                    }
                );
            }
        }
    }

    thread_local! {
        static LOT_WATCH: Cell<u32> = const { Cell::new(0) };
        static LOT_OBSERVED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }
    extern "C" fn observe_lot() {
        let p = LOT_WATCH.with(Cell::get);
        LOT_OBSERVED.with(|v| v.borrow_mut().push(unsafe { word(p as *mut u8, 0x34) }));
    }
    #[unsafe(naked)]
    unsafe extern "C" fn lot_add() {
        core::arch::naked_asm!("pushad", "call {observe}", "popad", "jmp {body}", observe = sym observe_lot, body = sym add_reference);
    }
    #[unsafe(naked)]
    unsafe extern "C" fn lot_release() {
        core::arch::naked_asm!("pushad", "call {observe}", "popad", "jmp {body}", observe = sym observe_lot, body = sym release_reference_inner);
    }
    #[test]
    fn lot_configuration_retains_then_publishes_then_releases_and_skips_identity() {
        let table = [0usize, lot_add as *const () as usize, lot_release as *const () as usize];
        let mut a = [table.as_ptr() as u32];
        let mut b = [table.as_ptr() as u32];
        let a = a.as_mut_ptr() as u32;
        let b = b.as_mut_ptr() as u32;
        for (old, new) in [(0, 0), (a, a), (0, b), (a, 0), (a, b)] {
            let mut object = [0xcccc_ccccu32; 16];
            object[13] = old;
            let p = object.as_mut_ptr().cast();
            LOT_WATCH.with(|v| v.set(p as u32));
            let mut expected = object;
            expected[13] = new;
            let result = run(fc4p_0060cb80, p, &[new]);
            assert_eq!(object, expected);
            let log = calls();
            let wanted: Vec<_> = if old == new {
                vec![]
            } else {
                [(new, 4), (old, 8)].into_iter().filter(|&(p, _)| p != 0).collect()
            };
            assert_eq!(
                log.iter().map(|c| (c.registers[1], c.address)).collect::<Vec<_>>(),
                wanted
            );
            let observed = LOT_OBSERVED.with(|v| core::mem::take(&mut *v.borrow_mut()));
            let values: Vec<_> = wanted
                .iter()
                .map(|&(_, method)| if method == 4 { old } else { new })
                .collect();
            assert_eq!(observed, values);
            assert_eq!(result, if old == new { [EAX, p as u32, EDX] } else { RESULT });
        }
    }

    #[test]
    fn window_query_keeps_unknown_output_and_success_result_upper_bytes() {
        let table = [0usize, add_reference as *const () as usize];
        let mut object = [table.as_ptr() as u32, 0xfeed_face];
        let p = object.as_mut_ptr().cast();
        for iid in [0, 1, 0x0a74_ca15, 0x4522_94aa, 0x4522_94ab, u32::MAX] {
            let mut out = 0xdead_beefu32;
            let result = run(fc4p_00623290, p, &[iid, &mut out as *mut u32 as u32]);
            let log = calls();
            if [1, 0x0a74_ca15, 0x4522_94aa].contains(&iid) {
                assert_eq!(out, p as u32 + if iid == 0x4522_94aa { 4 } else { 0 });
                assert_eq!(result, [RESULT[0] & !255 | 1, RESULT[1], RESULT[2]]);
                assert_eq!(log.len(), 1);
                assert_eq!(
                    log[0].registers,
                    [table.as_ptr() as u32, p as u32, &mut out as *mut u32 as u32]
                );
            } else {
                assert_eq!(out, 0xdead_beef);
                assert_eq!(result, [iid.wrapping_sub(0x4522_94aa) & !255, p as u32, EDX]);
                assert!(log.is_empty());
            }
        }
    }

    #[test]
    fn registered_resource_cleanup_covers_missing_registry_and_failed_removal() {
        let resource_table = [0usize, 0, release_reference_inner as *const () as usize];
        let mut resource = [resource_table.as_ptr() as u32];
        let resource = resource.as_mut_ptr() as u32;
        ALLOCATED.with(|v| v.set(resource));
        let mut registry_table = [0usize; 25];
        registry_table[2] = release_reference_inner as *const () as usize;
        let mut registry = [registry_table.as_ptr() as u32];
        let secondary_table = [
            0usize,
            0,
            release_reference_inner as *const () as usize,
            get_resource as *const () as usize,
        ];
        let mut secondary = [secondary_table.as_ptr() as u32];
        let registry = registry.as_mut_ptr() as u32;
        let secondary = secondary.as_mut_ptr() as u32;
        for (owner, handle, success) in [
            (0, 0, false),
            (0, secondary, false),
            (registry, 0, false),
            (registry, secondary, false),
            (registry, secondary, true),
        ] {
            registry_table[24] = if success {
                remove_resource
            } else {
                remove_resource_failed
            } as *const () as usize;
            let mut object = [0xfeed_face, owner, handle, 0xdead_beef];
            let p = object.as_mut_ptr().cast();
            let result = run(fc4p_005d9430, p, &[]);
            assert_eq!(object, [0xfeed_face, 0, 0, 0xdead_beef]);
            let log = calls();
            let expected = if owner == 0 && handle == 0 {
                vec![]
            } else if owner == 0 {
                vec![(8, handle)]
            } else if handle == 0 {
                vec![(8, owner)]
            } else if success {
                vec![(12, handle), (0x60, owner), (8, resource), (8, owner)]
            } else {
                vec![(12, handle), (0x60, owner), (8, owner)]
            };
            assert_eq!(
                log.iter().map(|c| (c.address, c.registers[1])).collect::<Vec<_>>(),
                expected
            );
            if owner == 0 && handle == 0 {
                assert_eq!(result, [0, 0, EDX]);
            } else {
                assert_eq!(result, RESULT);
            }
            if owner != 0 && handle != 0 {
                assert_eq!(log[1].args, [resource]);
            }
        }
    }

    thread_local! { static NEW_MAP: Cell<u32> = const { Cell::new(0) }; }
    extern "thiscall" fn relocate_deque_map(this: *mut u8, front: u32, back: u32) -> u32 {
        assert_eq!((front, back), (1, 0));
        let next = NEW_MAP.with(Cell::get);
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x006db4a0,
                registers: [0, this as u32, 0],
                args: vec![front, back],
            })
        });
        unsafe {
            store(this, 0x20, next);
            store(this, 0x1c, next);
            store(this, 0x24, 10);
        }
        0
    }
    #[test]
    fn deque_append_grows_map_when_less_than_two_slots_and_keeps_saved_input() {
        let _allocate = fake(0x0090_cf54, array_allocate as *const () as usize);
        let _grow = fake(0x006db4a0, relocate_deque_map as *const () as usize);
        for free in [0u32, 1, 2, 3] {
            for has_slot in [false, true] {
                let mut object = [0xfeed_faceu32; 12];
                let p = object.as_mut_ptr().cast();
                let mut old_map = [0x1111u32; 4];
                let mut new_map = [0x2222u32; 4];
                let mut block = [0xa5a5_a5a5u32; 32];
                let mut slot = 0x1234u32;
                object[4] = if has_slot { &mut slot as *mut u32 as u32 } else { 0 };
                object[7] = unsafe { old_map.as_mut_ptr().add(1) as u32 };
                object[8] = old_map.as_mut_ptr() as u32;
                object[9] = free + 1;
                let before = object;
                let input = [0xdead_beefu32];
                let block_pointer = block.as_mut_ptr() as u32;
                ALLOCATED.with(|v| v.set(block_pointer));
                NEW_MAP.with(|v| v.set(new_map.as_mut_ptr() as u32));
                let result = run(fc4p_006266e0, p, &[input.as_ptr() as u32]);
                let log = calls();
                assert_eq!(
                    log.iter().map(|c| c.address).collect::<Vec<_>>(),
                    if free < 2 {
                        vec![0x006db4a0, 0x0090cf54]
                    } else {
                        vec![0x0090cf54]
                    }
                );
                assert_eq!(log.last().unwrap().args, [0x80]);
                let node = if free < 2 {
                    new_map.as_mut_ptr() as u32
                } else {
                    before[7]
                };
                assert_eq!(object[7], node + 4);
                assert_eq!(
                    (object[4], object[5], object[6]),
                    (block_pointer, block_pointer, block_pointer + 0x80)
                );
                assert_eq!(slot, if has_slot { input[0] } else { 0x1234 });
                assert_eq!(result, [block_pointer + 0x80, node, block_pointer]);
                assert_eq!(block, [0xa5a5_a5a5; 32]);
                for index in [0, 1, 2, 3, 10, 11] {
                    assert_eq!(object[index], before[index]);
                }
                assert_eq!(if free < 2 { new_map[1] } else { old_map[2] }, block_pointer);
            }
        }
    }
    extern "C" fn search_sequence(first: u32, last: u32, needle: u32, needle_end: u32, tag: u32) -> u32 {
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x004426c0,
                registers: [0; 3],
                args: vec![first, last, needle, needle_end, tag],
            })
        });
        let haystack = unsafe { core::slice::from_raw_parts(first as *const u8, (last - first) as usize) };
        let needle = unsafe { core::slice::from_raw_parts(needle as *const u8, (needle_end - needle) as usize) };
        haystack
            .windows(needle.len())
            .position(|s| s == needle)
            .map_or(last, |i| first + i as u32)
    }
    #[test]
    fn reverse_subsequence_search_keeps_overlapping_matches_and_empty_ranges() {
        let _search = fake(0x004426c0, search_sequence as *const () as usize);
        for (text, needle, wanted, searches) in [
            (&b"ababa"[..], &b"aba"[..], 2, 3),
            (&b"ababa"[..], &b"ba"[..], 3, 3),
            (&b"ababa"[..], &b"z"[..], 5, 1),
            (&b""[..], &b"z"[..], 0, 1),
            (&b"abc"[..], &b""[..], 3, 0),
        ] {
            let p = text.as_ptr() as u32;
            let n = needle.as_ptr() as u32;
            let result = run_cdecl(
                fc4p_005c2380,
                &[p, p + text.len() as u32, n, n + needle.len() as u32, 5, 6, 7],
            );
            assert_eq!(result[0], p + wanted);
            let log = calls();
            assert_eq!(log.len(), searches);
            for c in &log {
                assert_eq!(&c.args[1..], &[p + text.len() as u32, n, n + needle.len() as u32, 7]);
            }
            for pair in log.windows(2) {
                assert!(pair[1].args[0] > pair[0].args[0]);
            }
        }
    }
    extern "C" fn reverse_range_fake(
        first: u32,
        last: u32,
        needle: u32,
        needle_end: u32,
        a: u32,
        b: u32,
        tag: u32,
    ) -> u32 {
        assert_eq!(a, b);
        assert_eq!(unsafe { word(a as *mut u8, 0) }, tag);
        // The next argument is rewritten to the available prefix length before this call.
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x005c2380,
                registers: [0; 3],
                args: vec![first, last, needle, needle_end, tag, unsafe { word(a as *mut u8, 4) }],
            })
        });
        if tag == 7 {
            last
        } else {
            last - (needle_end - needle)
        }
    }
    #[test]
    fn string_reverse_find_clamps_position_rewrites_length_and_handles_empty_needles() {
        let _search = fake(0x005c2380, reverse_range_fake as *const () as usize);
        let mut string = [0x1000u32, 0x1009, 0x1100];
        let p = string.as_mut_ptr().cast();
        for (position, length, expected) in [
            (0, 3, 0),
            (4, 3, 4),
            (99, 3, 6),
            (7, 3, u32::MAX),
            (1, 10, u32::MAX),
            (3, 0, 3),
            (99, 0, 9),
        ] {
            let result = run(fc4p_005c23e0, p, &[0x2000, position, length]);
            assert_eq!((result[0], result[1]), (expected, 9));
            let log = calls();
            if length == 0 || length > 9 {
                assert!(log.is_empty());
            } else {
                assert_eq!(
                    log[0].args,
                    [
                        0x1000,
                        0x1000 + position.min(9 - length) + length,
                        0x2000,
                        0x2000 + length,
                        position,
                        9 - length
                    ]
                );
            }
            assert_eq!(string, [0x1000, 0x1009, 0x1100]);
        }
    }
    extern "C" fn outside_set_fake(first: u32, last: u32, set: u32, end: u32, tag: u32) -> u32 {
        assert_eq!(unsafe { word(tag as *mut u8, 0) }, set);
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address: 0x005c6060,
                registers: [0; 3],
                args: vec![first, last, set, end],
            })
        });
        let text = unsafe { core::slice::from_raw_parts(first as *const u8, (last - first) as usize) };
        let set = unsafe { core::slice::from_raw_parts(set as *const u8, (end - set) as usize) };
        text.iter()
            .position(|c| !set.contains(c))
            .map_or(last, |i| first + i as u32)
    }
    #[test]
    fn string_outside_set_scans_to_terminator_and_handles_position_at_end_or_beyond() {
        let _search = fake(0x005c6060, outside_set_fake as *const () as usize);
        let text = b"abbbc";
        let base = text.as_ptr() as u32;
        let mut string = [base, base + 5, base + 6];
        let p = string.as_mut_ptr().cast();
        for (set, position, expected) in [
            (&b"ab\0ignored"[..], 0, 4),
            (&b"ab\0"[..], 2, 4),
            (&b"abc\0"[..], 0, u32::MAX),
            (&b"\0"[..], 0, 0),
            (&b"ab\0"[..], 5, u32::MAX),
            (&b"ab\0"[..], 6, u32::MAX),
        ] {
            assert_eq!(run(fc4p_005c6610, p, &[set.as_ptr() as u32, position])[0], expected);
            let log = calls();
            assert_eq!(log.len(), usize::from(position <= 5));
            if position <= 5 {
                assert_eq!(
                    log[0].args,
                    [
                        base + position,
                        base + 5,
                        set.as_ptr() as u32,
                        set.as_ptr() as u32 + set.iter().position(|&c| c == 0).unwrap() as u32
                    ]
                );
            }
        }
    }
    #[test]
    fn point_bounds_cover_group_boundaries_nan_signed_zero_and_output_aliases() {
        let _fpu = GameFpu::new();
        for count in [-1i32, 0, 1, 2, 3, 4, 5, 6, 8, 9, 10] {
            let mut points = [
                [4.0f32, -2.0],
                [-1.0, 9.0],
                [8.0, -7.0],
                [f32::NAN, f32::NAN],
                [3.0, 5.0],
                [-9.0, 1.0],
                [20.0, -12.0],
                [0.0, 0.0],
                [-21.0, 30.0],
                [0.0, -40.0],
            ];
            let original = points.map(|p| p.map(f32::to_bits));
            let mut low = [0xaaaa_aaaau32; 4];
            let mut high = [0xbbbb_bbbbu32; 4];
            run_cdecl(
                fc4p_005d6ba0,
                &[
                    points.as_mut_ptr() as u32,
                    count as u32,
                    low[1..].as_mut_ptr() as u32,
                    high[1..].as_mut_ptr() as u32,
                ],
            );
            let mut expected_low = points[0];
            let mut expected_high = points[0];
            for p in points.iter().take(count.max(1) as usize).skip(1) {
                for axis in 0..2 {
                    if p[axis] < expected_low[axis] {
                        expected_low[axis] = p[axis];
                    }
                    if p[axis] > expected_high[axis] {
                        expected_high[axis] = p[axis];
                    }
                }
            }
            assert_eq!(
                low,
                [
                    0xaaaa_aaaa,
                    expected_low[0].to_bits(),
                    expected_low[1].to_bits(),
                    0xaaaa_aaaa
                ]
            );
            assert_eq!(
                high,
                [
                    0xbbbb_bbbb,
                    expected_high[0].to_bits(),
                    expected_high[1].to_bits(),
                    0xbbbb_bbbb
                ]
            );
            assert_eq!(points.map(|p| p.map(f32::to_bits)), original);
        }
        for bits in [0x7fc0_1234u32, 0x7f80_1234, 0, 0x8000_0000, 1, 0x7f80_0000, 0xff80_0000] {
            let mut points = [bits, bits, 0, 0x8000_0000];
            let p = points.as_mut_ptr() as u32;
            run_cdecl(fc4p_005d6ba0, &[p, 2, p, p]);
            assert_eq!(&points[2..], &[0, 0x8000_0000]);
            if f32::from_bits(bits).is_nan() || bits == 0 || bits == 0x8000_0000 {
                assert_eq!(&points[..2], &[bits, bits]);
            }
        }
    }

    thread_local! { static RECORD_STRIDE: Cell<u32> = const { Cell::new(12) }; }
    fn log_record_call(address: u32, args: Vec<u32>) {
        CALLS.with(|v| {
            v.borrow_mut().push(Call {
                address,
                registers: [0; 3],
                args,
            })
        });
    }
    unsafe fn copy_test_record(out: u32, source: u32) {
        match RECORD_STRIDE.with(Cell::get) {
            8 => unsafe {
                write_unaligned(
                    out.wrapping_add(4) as *mut u8,
                    read_unaligned(source.wrapping_add(4) as *const u8),
                );
                store(out as *mut u8, 0, word(source as *mut u8, 0));
                write_unaligned(
                    out.wrapping_add(5) as *mut u8,
                    read_unaligned(source.wrapping_add(5) as *const u8),
                );
            },
            13 => unsafe {
                for offset in [0, 4, 8] {
                    store(out as *mut u8, offset, word(source as *mut u8, offset));
                }
            },
            _ => unsafe {
                write_unaligned(out as *mut u8, read_unaligned(source as *const u8));
                for offset in [4, 8] {
                    store(out as *mut u8, offset, word(source as *mut u8, offset));
                }
            },
        }
    }
    extern "C" fn record_fill(out: u32, count: u32, value: u32, tag: u32) -> u32 {
        let stride = RECORD_STRIDE.with(Cell::get).min(12);
        log_record_call(1, vec![out, count, value, unsafe { word(tag as *mut u8, 0) }]);
        for i in 0..count {
            unsafe {
                copy_test_record(out + i * stride, value);
            }
        }
        out + count * stride
    }
    extern "C" fn record_copy(first: u32, last: u32, out: u32, tag: u32) -> u32 {
        let stride = RECORD_STRIDE.with(Cell::get).min(12);
        log_record_call(2, vec![first, last, out, unsafe { word(tag as *mut u8, 0) }]);
        let count = (last - first) / stride;
        for i in 0..count {
            unsafe {
                copy_test_record(out + i * stride, first + i * stride);
            }
        }
        out + count * stride
    }
    #[test]
    fn vector_growth_preserves_padding_inserts_at_each_position_and_copies_optional_tail() {
        let _allocate = fake(0x0090cf54, array_allocate as *const () as usize);
        let _free = fake(0x0090cf63, array_free as *const () as usize);
        for (entry, kind, fill, copy) in [
            (fc4p_00621a40 as unsafe extern "C" fn(), 12u32, 0x00621550, 0x00621580),
            (fc4p_00621b40, 8, 0x00621150, 0x00621110),
            (fc4p_00626240, 13, 0x00623fe0, 0x00623fa0),
        ] {
            RECORD_STRIDE.with(|v| v.set(kind));
            let stride = kind.min(12);
            let _fill = fake(fill, record_fill as *const () as usize);
            let _copy = fake(copy, record_copy as *const () as usize);
            for size in [0u32, 1, 3] {
                for index in 0..=size {
                    for count in [0u32, 1, 2, 5] {
                        for skip in [0u32, 1, 0x100] {
                            let mut old = [0x5152_5354u32; 12];
                            let begin = if size == 0 { 0 } else { old.as_mut_ptr() as u32 };
                            let mut destination = [0xa5a5_a5a5u32; 36];
                            let allocated = destination.as_mut_ptr() as u32;
                            ALLOCATED.with(|v| v.set(allocated));
                            let item = [0x1234_5678u32, 0x8192_a3b4, 0xc5d6_e7f8];
                            let value = item.as_ptr() as u32;
                            let mut vector = [begin, begin + size * stride, begin + size * stride, 0xfeed_face];
                            let p = vector.as_mut_ptr().cast();
                            let capacity = size + size.max(count);
                            let out = if capacity == 0 { 0 } else { allocated };
                            let result = run(entry, p, &[begin + index * stride, value, 0x1234, count, skip]);
                            let total = index + count + if skip as u8 == 0 { size - index } else { 0 };
                            assert_eq!(
                                vector,
                                [out, out + total * stride, out + capacity * stride, 0xfeed_face]
                            );
                            assert_eq!(result[1], size);
                            if kind == 13 {
                                assert_eq!((result[0], result[2]), (out + capacity * stride, capacity * 3));
                            } else {
                                assert_eq!(result[2], out + capacity * stride);
                            }
                            let log = calls();
                            let wanted: Vec<_> = [
                                (capacity != 0, 0x0090cf54),
                                (kind == 13, 2),
                                (count != 1, 1),
                                (skip as u8 == 0, 2),
                                (begin != 0, 0x0090cf63),
                            ]
                            .into_iter()
                            .filter_map(|(yes, address)| yes.then_some(address))
                            .collect();
                            assert_eq!(log.iter().map(|c| c.address).collect::<Vec<_>>(), wanted);
                            if capacity != 0 {
                                assert_eq!(log[0].args, [capacity * stride]);
                            }
                            let mut expected = [0xa5u8; 144];
                            let old_bytes = unsafe { core::slice::from_raw_parts(old.as_ptr().cast::<u8>(), 48) };
                            let item_bytes = unsafe { core::slice::from_raw_parts(item.as_ptr().cast::<u8>(), 12) };
                            for i in 0..total {
                                let source = if i < index {
                                    &old_bytes[(i * stride) as usize..]
                                } else if i < index + count {
                                    item_bytes
                                } else {
                                    &old_bytes[((i - count) * stride) as usize..]
                                };
                                let fields: &[usize] = if kind == 8 {
                                    &[0, 1, 2, 3, 4, 5]
                                } else if kind == 12 {
                                    &[0, 4, 5, 6, 7, 8, 9, 10, 11]
                                } else {
                                    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
                                };
                                for &byte in fields {
                                    expected[(i * stride) as usize + byte] = source[byte];
                                }
                            }
                            assert_eq!(
                                unsafe { core::slice::from_raw_parts(destination.as_ptr().cast::<u8>(), 144) },
                                expected
                            );
                            assert_eq!(old, [0x5152_5354; 12]);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn grid_relaxation_matches_manhattan_envelope_and_preserves_row_headers_and_guards() {
        for width in [-1i32, 0, 1, 2, 5] {
            for height in [-1i32, 0, 1, 3, 6] {
                for seed in [0u8, 1, 254, 255] {
                    let w = width.max(0) as usize;
                    let h = height.max(0) as usize;
                    let mut cells = vec![vec![255u8; h + 2]; w];
                    for (x, row) in cells.iter_mut().enumerate() {
                        row[0] = 0xa5;
                        row[h + 1] = 0x5a;
                        if h != 0 {
                            row[1 + (x * 3 % h)] = seed;
                        }
                    }
                    let before = cells.clone();
                    let mut rows: Vec<_> = cells
                        .iter_mut()
                        .map(|r| [r[1..].as_mut_ptr() as u32, 0xfeed_face, 0xdead_beef])
                        .collect();
                    let headers = rows.clone();
                    let mut storage = [rows.as_mut_ptr() as u32, 0x7654_3210, 0x1122_3344];
                    let original_storage = storage;
                    let result = run_cdecl(
                        fc4p_005e4d60,
                        &[storage.as_mut_ptr() as u32, width as u32, height as u32],
                    );
                    assert_eq!(result[1], if width > 0 { 0 } else { 0x11223344 });
                    for (x, row) in cells.iter().enumerate() {
                        assert_eq!((row[0], row[h + 1]), (0xa5, 0x5a));
                        for z in 0..h {
                            let expected = (0..w)
                                .flat_map(|sx| (0..h).map(move |sz| (sx, sz)))
                                .map(|(sx, sz)| {
                                    (usize::from(before[sx][sz + 1]) + x.abs_diff(sx) + z.abs_diff(sz)).min(255) as u8
                                })
                                .min()
                                .unwrap();
                            assert_eq!(row[z + 1], expected);
                        }
                    }
                    assert_eq!(rows, headers);
                    assert_eq!(storage, original_storage);
                    assert_eq!(
                        result[0],
                        if height > 0 && width <= 0 {
                            width.wrapping_sub(1) as u32
                        } else if width <= 0 {
                            width as u32
                        } else {
                            0
                        }
                    );
                }
            }
        }
    }
    extern "thiscall" fn mask_storage(this: *mut u8) -> u32 {
        log_record_call(0x60, vec![this as u32]);
        unsafe { word(this, 16) }
    }
    extern "thiscall" fn mask_const_storage(this: *mut u8) -> u32 {
        log_record_call(0x64, vec![this as u32]);
        unsafe { word(this, 16) }
    }
    extern "thiscall" fn mask_width(this: *mut u8) -> u32 {
        log_record_call(0x30, vec![this as u32]);
        unsafe { word(this, 4) }
    }
    extern "thiscall" fn mask_height(this: *mut u8) -> u32 {
        log_record_call(0x34, vec![this as u32]);
        unsafe { word(this, 8) }
    }
    extern "thiscall" fn mask_shift(this: *mut u8) -> u32 {
        log_record_call(0x2c, vec![this as u32]);
        unsafe { word(this, 12) }
    }
    #[test]
    fn grid_mask_queries_in_order_scales_coordinates_and_revisits_only_first_output_byte() {
        let mut table = [0usize; 26];
        for (slot, function) in [
            (0x60, mask_storage as *const () as usize),
            (0x64, mask_const_storage as *const () as usize),
            (0x30, mask_width as *const () as usize),
            (0x34, mask_height as *const () as usize),
            (0x2c, mask_shift as *const () as usize),
        ] {
            table[slot / 4] = function;
        }
        for difference in [-33i32, -2, -1, 0, 1, 2, 33] {
            for width in [-1i32, 0, 1, 3] {
                for height in [-1i32, 0, 1, 3] {
                    let mut input = vec![vec![0u8; 16]; 16];
                    for (x, row) in input.iter_mut().enumerate() {
                        for (z, value) in row.iter_mut().enumerate() {
                            *value = ((x + z * 3) % 5) as u8;
                        }
                    }
                    let mut source_rows: Vec<_> = input.iter_mut().map(|r| [r.as_mut_ptr() as u32, 16, 16]).collect();
                    let mut source_storage = [source_rows.as_mut_ptr() as u32];
                    let mut source = [table.as_ptr() as u32, 16, 16, 0, source_storage.as_mut_ptr() as u32];
                    let mut output = [[0xa5u8; 5]; 3];
                    let mut dest_rows: Vec<_> = output.iter_mut().map(|r| [r[1..].as_mut_ptr() as u32, 3, 4]).collect();
                    let mut dest_storage = [dest_rows.as_mut_ptr() as u32];
                    let mut dest = [
                        table.as_ptr() as u32,
                        width as u32,
                        height as u32,
                        difference as u32,
                        dest_storage.as_mut_ptr() as u32,
                    ];
                    let d = dest.as_mut_ptr() as u32;
                    let s = source.as_mut_ptr() as u32;
                    assert_eq!(run_cdecl(fc4p_005e4fa0, &[d, s, 0x1234_0002])[0], width as u32);
                    let log = calls();
                    assert_eq!(
                        log.iter().map(|c| (c.address, c.args[0])).collect::<Vec<_>>(),
                        [(0x60, d), (0x30, d), (0x34, d), (0x2c, d), (0x2c, s), (0x64, s)]
                    );
                    for (x, row) in output.iter().enumerate() {
                        let mut expected = [0xa5u8; 5];
                        if x < width.max(0) as usize && height > 0 {
                            let z = height - 1;
                            let matched = if difference == 0 {
                                input[x][0] == 2
                            } else if difference < 0 {
                                input[x >> ((-difference) & 31)][(z >> ((-difference) & 31)) as usize] == 2
                            } else {
                                let shift = difference as u32 & 31;
                                let xs = (x as i32).wrapping_shl(shift)..(x as i32 + 1).wrapping_shl(shift);
                                xs.into_iter().any(|sx| {
                                    (z.wrapping_shl(shift)..(z + 1).wrapping_shl(shift))
                                        .any(|sz| input[sx as usize][sz as usize] == 2)
                                })
                            };
                            expected[1] = if matched { 0 } else { 255 };
                        }
                        assert_eq!(*row, expected);
                    }
                }
            }
        }
        assert_eq!(run_cdecl(fc4p_005e4fa0, &[0, 0, 2]), [EAX, 0x11223344, EDX]);
        assert!(calls().is_empty());
    }

    value_stub!(occupant_message_allocate, 0x009133da, 1, 0);
    value_stub!(occupant_message_construct, 0x009134d6, 0, 0);
    stub!(notify_add, 0x1004, 0, 0);
    stub!(notify_id, 0x1014, 1, 4);
    stub!(notify_owner, 0x1030, 1, 4);
    stub!(notify_secondary_owner, 0x1084, 1, 4);
    stub!(notify_attribute, 0x1040, 1, 4);
    stub!(notify_type, 0x1054, 1, 4);
    stub!(notify_data, 0x1068, 1, 4);
    stub!(notify_release, 0x1008, 0, 0);
    stub!(occupant_attribute, 0x2080, 1, 4);
    stub!(occupant_type, 0x201c, 0, 0);
    stub!(send_message_now, 0x300c, 1, 4);
    stub!(post_message_later, 0x3010, 2, 8);
    #[test]
    fn occupant_notification_initializes_copies_fields_dispatches_then_releases() {
        let mut table = [0usize; 34];
        for (slot, function) in [
            (4, notify_add as *const () as usize),
            (0x14, notify_id as *const () as usize),
            (0x30, notify_owner as *const () as usize),
            (0x84, notify_secondary_owner as *const () as usize),
            (0x40, notify_attribute as *const () as usize),
            (0x54, notify_type as *const () as usize),
            (0x68, notify_data as *const () as usize),
            (8, notify_release as *const () as usize),
        ] {
            table[slot / 4] = function;
        }
        let mut message = [table.as_ptr() as u32];
        let message = message.as_mut_ptr() as u32;
        ALLOCATED.with(|v| v.set(message));
        let _allocate = fake(0x009133da, occupant_message_allocate as *const () as usize);
        let _construct = fake(0x009134d6, occupant_message_construct as *const () as usize);
        let mut owner_table = [0usize; 33];
        owner_table[0x80 / 4] = occupant_attribute as *const () as usize;
        owner_table[0x1c / 4] = occupant_type as *const () as usize;
        let server_table = [
            0usize,
            0,
            0,
            send_message_now as *const () as usize,
            post_message_later as *const () as usize,
        ];
        let mut server = [server_table.as_ptr() as u32];
        let server = server.as_mut_ptr() as u32;
        let mut default = server;
        let _default = crate::testing::fake_data(0x00b43ccc, (&mut default as *mut u32).cast());
        for initialized in [false, true] {
            for id in [0, 0x99ef1145, 0x99ef1146] {
                for special in [0u32, 0x8000_0000] {
                    let mut cached = if initialized { server } else { 0 };
                    let _cached = crate::testing::fake_data(0x00b23194, (&mut cached as *mut u32).cast());
                    let mut owner = [0xfeed_faceu32; 14];
                    owner[0] = owner_table.as_ptr() as u32;
                    owner[12] = special;
                    let before = owner;
                    let p = owner.as_mut_ptr().cast();
                    assert_eq!(run(fc4p_005e9010, p, &[id, 0xcafe_babe]), RESULT);
                    assert_eq!(cached, server);
                    assert_eq!(owner, before);
                    let log = calls();
                    let dispatch = if id == 0x99ef1146 && special != 0 {
                        0x300c
                    } else {
                        0x3010
                    };
                    assert_eq!(
                        log.iter().map(|c| c.address).collect::<Vec<_>>(),
                        [
                            0x009133da, 0x009134d6, 0x1004, 0x1014, 0x1030, 0x1084, 0x2080, 0x1040, 0x201c, 0x1054,
                            0x1068, dispatch, 0x1008
                        ]
                    );
                    assert_eq!(log[0].args, [0x2c]);
                    assert_eq!(log[1].registers[1], message);
                    for i in [2, 3, 4, 5, 7, 9, 10, 12] {
                        assert_eq!(log[i].registers[1], message);
                    }
                    for i in [6, 8] {
                        assert_eq!(log[i].registers[1], p as u32);
                    }
                    assert_eq!(log[3].args, [id]);
                    assert_eq!(log[4].args, [p as u32]);
                    assert_eq!(log[5].args, [p as u32]);
                    assert_eq!(log[6].args, [4]);
                    assert_eq!(log[7].args, [RESULT[0] & 255]);
                    assert_eq!(log[9].args, [RESULT[0]]);
                    assert_eq!(log[10].args, [0xcafe_babe]);
                    assert_eq!(log[11].registers[1], server);
                    assert_eq!(
                        log[11].args,
                        if dispatch == 0x300c {
                            vec![message]
                        } else {
                            vec![message, 0]
                        }
                    );
                }
            }
        }
    }

    thread_local! {
        static PROPERTY_OWNER: Cell<u32> = const { Cell::new(0) };
        static PROPERTY_OBSERVED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }
    extern "C" fn observe_property_end() {
        let end = PROPERTY_OWNER.with(|v| unsafe { word(v.get() as *mut u8, 4) });
        PROPERTY_OBSERVED.with(|v| v.borrow_mut().push(end));
    }
    #[unsafe(naked)]
    unsafe extern "C" fn property_add() {
        core::arch::naked_asm!("pushad", "call {observe}", "popad", "jmp {body}", observe = sym observe_property_end, body = sym add_reference);
    }
    #[unsafe(naked)]
    unsafe extern "C" fn property_release() {
        core::arch::naked_asm!("pushad", "call {observe}", "popad", "jmp {body}", observe = sym observe_property_end, body = sym release_reference_inner);
    }
    extern "C" fn shift_pairs_fake(first: u32, last: u32, out: u32, tag: u32, unused: u32) -> u32 {
        log_record_call(
            0x005fdd40,
            vec![first, last, out, unsafe { word(tag as *mut u8, 0) }, unused],
        );
        // A call may change the original input after the caller has retained its temporary.
        unsafe {
            store(word(tag as *mut u8, 0) as *mut u8, 0, 0xdead_beef);
        }
        RESULT[0]
    }
    extern "thiscall" fn assign_pair_fake(this: *mut u8, value: *mut u8) -> u32 {
        log_record_call(
            0x005fdcf0,
            vec![this as u32, unsafe { word(value, 0) }, unsafe { word(value, 4) }],
        );
        RESULT[0]
    }
    extern "thiscall" fn grow_pairs_fake(this: *mut u8, at: u32, value: u32, tag: u32, count: u32, skip: u32) -> u32 {
        assert_eq!(unsafe { word(tag as *mut u8, 0) }, value);
        log_record_call(0x0077d7e0, vec![this as u32, at, value, count, skip]);
        // Returning the iterator uses the current base, after a possible allocation.
        unsafe {
            store(this, 0, 0x2000);
        }
        RESULT[0]
    }
    #[test]
    fn property_pair_insertion_retains_before_publishing_end_and_uses_saved_temporary() {
        let _shift = fake(0x005fdd40, shift_pairs_fake as *const () as usize);
        let _assign = fake(0x005fdcf0, assign_pair_fake as *const () as usize);
        let _grow = fake(0x0077d7e0, grow_pairs_fake as *const () as usize);
        let table = [
            0usize,
            property_add as *const () as usize,
            property_release as *const () as usize,
        ];
        let mut resource = [table.as_ptr() as u32];
        let resource = resource.as_mut_ptr() as u32;
        for incoming in [0u32, resource] {
            for last_resource in [0u32, resource] {
                for index in [0u32, 1, 2] {
                    for full in [false, true] {
                        let mut buffer = [10u32, 0, 20, last_resource, 0xa5a5_a5a5, 0x5a5a_5a5a, 0xfeed_face];
                        let base = buffer.as_mut_ptr() as u32;
                        let mut vector = [base, base + 16, base + if full { 16 } else { 24 }];
                        let p = vector.as_mut_ptr().cast();
                        PROPERTY_OWNER.with(|v| v.set(p as u32));
                        let mut item = [30u32, incoming];
                        let value = item.as_mut_ptr() as u32;
                        let result = run(fc4p_005ff5d0, p, &[base + index * 8, value]);
                        let observed = PROPERTY_OBSERVED.with(|v| core::mem::take(&mut *v.borrow_mut()));
                        let log = calls();
                        assert_eq!(
                            (result[0], result[2]),
                            (
                                if full { 0x2000 } else { base } + index * 8,
                                if full { 0x2000 } else { base }
                            )
                        );
                        if full {
                            assert_eq!(log.len(), 1);
                            assert_eq!(log[0].args, [p as u32, base + index * 8, value, 1, 0]);
                            assert_eq!(log[0].address, 0x0077d7e0);
                            assert!(observed.is_empty());
                            assert_eq!(vector[1], base + 16);
                        } else if index == 2 {
                            assert_eq!(&buffer[4..], &[30, incoming, 0xfeed_face]);
                            assert_eq!(
                                log.iter().map(|c| c.address).collect::<Vec<_>>(),
                                if incoming == 0 { vec![] } else { vec![4] }
                            );
                            assert_eq!(observed, if incoming == 0 { vec![] } else { vec![base + 16] });
                            assert_eq!(vector[1], base + 24);
                        } else {
                            assert_eq!(&buffer[4..], &[20, last_resource, 0xfeed_face]);
                            let mut expected = vec![];
                            if last_resource != 0 {
                                expected.push(4);
                            }
                            if incoming != 0 {
                                expected.push(4);
                            }
                            expected.extend([0x005fdd40, 0x005fdcf0]);
                            if incoming != 0 {
                                expected.push(8);
                            }
                            assert_eq!(log.iter().map(|c| c.address).collect::<Vec<_>>(), expected);
                            let shift = log.iter().find(|c| c.address == 0x005fdd40).unwrap();
                            assert_eq!(shift.args, [base + index * 8, base + 8, base + 16, value, 0]);
                            let assign = log.iter().find(|c| c.address == 0x005fdcf0).unwrap();
                            assert_eq!(assign.args, [base + index * 8, 30, incoming]);
                            assert_eq!(item, [0xdead_beef, incoming]);
                            assert_eq!(vector[1], base + 24);
                            let wanted: Vec<_> = [
                                (last_resource != 0, base + 16),
                                (incoming != 0, base + 24),
                                (incoming != 0, base + 24),
                            ]
                            .into_iter()
                            .filter_map(|(yes, v)| yes.then_some(v))
                            .collect();
                            assert_eq!(observed, wanted);
                        }
                        assert_eq!(&buffer[..4], &[10, 0, 20, last_resource]);
                    }
                }
            }
        }
    }

    stub!(detach_window, 0x44, 2, 8);
    stub!(delete_window_nodes, 0x004676a0, 1, 4);
    #[test]
    fn window_cleanup_walks_tree_in_order_resets_header_and_frees_pool_entries_in_order() {
        let _tree = fake(0x004676a0, delete_window_nodes as *const () as usize);
        let _free = fake(0x005e5620, storage_delete as *const () as usize);
        let mut window_table = [0usize; 18];
        window_table[17] = detach_window as *const () as usize;
        let mut windows = [[window_table.as_ptr() as u32]; 5];
        for populated in [false, true] {
            for has_pool in [false, true] {
                for has_array in [false, true] {
                    for count in [0u32, 3] {
                        let mut header = [0xcccc_ccccu32; 5];
                        let h = header.as_mut_ptr() as u32;
                        let mut nodes = [[0u32; 5]; 5];
                        let pointers = core::array::from_fn::<_, 5, _>(|i| nodes[i].as_mut_ptr() as u32);
                        for i in 0..5 {
                            nodes[i][4] = windows[i].as_mut_ptr() as u32;
                        }
                        nodes[0][1] = pointers[1];
                        nodes[1][1] = pointers[2];
                        nodes[1][2] = pointers[0];
                        nodes[2][1] = h;
                        nodes[2][2] = pointers[1];
                        nodes[2][3] = pointers[4];
                        nodes[3][1] = pointers[4];
                        nodes[4][1] = pointers[2];
                        nodes[4][2] = pointers[3];
                        header[1] = if populated { pointers[2] } else { 0 };
                        header[2] = if populated { pointers[0] } else { h };
                        header[3] = if populated { pointers[4] } else { h };
                        let mut blocks = [0x1234u32, 0, 0x5678];
                        let blocks_pointer = blocks.as_mut_ptr() as u32;
                        let mut pool = [0xfeed_faceu32; 10];
                        pool[0x20 / 4] = if has_array { blocks_pointer } else { 0 };
                        pool[0x14 / 4] = count;
                        let pool_pointer = pool.as_mut_ptr() as u32;
                        let mut object = [0xa5a5_a5a5u32; 0x224 / 4];
                        object[0x214 / 4] = h;
                        object[0x218 / 4] = if populated { 5 } else { 0 };
                        object[0x210 / 4] = if has_pool { pool_pointer } else { 0 };
                        let before = object;
                        let before_nodes = nodes;
                        let p = object.as_mut_ptr().cast();
                        let result = run(fc4p_00625f10, p, &[]);
                        let log = calls();
                        let visits: Vec<_> = log.iter().filter(|c| c.address == 0x44).collect();
                        assert_eq!(
                            visits.iter().map(|c| c.registers[1]).collect::<Vec<_>>(),
                            if populated {
                                windows.iter_mut().map(|w| w.as_mut_ptr() as u32).collect()
                            } else {
                                vec![]
                            }
                        );
                        assert!(visits.iter().all(|c| c.args == [0, 0]));
                        let mut expected_addresses = if populated {
                            vec![0x44, 0x44, 0x44, 0x44, 0x44, 0x004676a0]
                        } else {
                            vec![]
                        };
                        let mut frees = vec![];
                        if has_pool {
                            if has_array {
                                frees.extend_from_slice(&blocks[..count as usize]);
                                frees.push(blocks_pointer);
                            }
                            frees.push(pool_pointer);
                        }
                        expected_addresses.extend(core::iter::repeat_n(0x005e5620, frees.len()));
                        assert_eq!(log.iter().map(|c| c.address).collect::<Vec<_>>(), expected_addresses);
                        assert_eq!(
                            log.iter()
                                .filter(|c| c.address == 0x005e5620)
                                .map(|c| c.args[0])
                                .collect::<Vec<_>>(),
                            frees
                        );
                        if populated {
                            assert_eq!(log[5].registers[1], p as u32 + 0x214);
                            assert_eq!(log[5].args, [pointers[2]]);
                        }
                        let mut expected_object = before;
                        expected_object[0x218 / 4] = 0;
                        expected_object[0x210 / 4] = 0;
                        assert_eq!(object, expected_object);
                        assert_eq!(header, [0xcccc_cccc, 0, h, h, 0xcccc_cccc]);
                        assert_eq!(nodes, before_nodes);
                        assert_eq!(pool[4], if has_pool { 0xa86364 } else { 0xfeed_face });
                        if has_pool {
                            assert_eq!(result, RESULT);
                        }
                    }
                }
            }
        }
    }

    thread_local! {
        static WINDOWS_MAJOR: Cell<u32> = const { Cell::new(10) };
        static HOOK_RESULT: Cell<u32> = const { Cell::new(0) };
    }
    extern "stdcall" fn windows_version_fake(info: *mut u32) -> u32 {
        let fields = unsafe { core::slice::from_raw_parts_mut(info, 37) };
        assert_eq!(fields[0], 0x94);
        assert!(fields[1..].iter().all(|&v| v == 0));
        log_record_call(0xf4, vec![0x94]);
        fields[1] = WINDOWS_MAJOR.with(Cell::get);
        RESULT[0]
    }
    extern "stdcall" fn windows_module_fake(name: u32) -> u32 {
        log_record_call(0xf0, vec![name]);
        0x1111_0000
    }
    extern "stdcall" fn windows_hook_fake(kind: u32, callback: u32, module: u32, thread: u32) -> u32 {
        log_record_call(0x500, vec![kind, callback, module, thread]);
        HOOK_RESULT.with(Cell::get)
    }
    extern "stdcall" fn windows_unhook_fake(hook: u32) -> u32 {
        log_record_call(0x504, vec![hook]);
        RESULT[0]
    }
    extern "stdcall" fn system_parameters_fake(action: u32, parameter: u32, previous: *mut u32, flags: u32) -> u32 {
        log_record_call(0x4c0, vec![action, parameter, flags]);
        unsafe {
            write_unaligned(previous, 0x1234);
        }
        RESULT[0]
    }
    #[test]
    fn application_switching_balances_nested_counts_and_selects_windows_api_by_major_version() {
        let mut version = windows_version_fake as *const () as u32;
        let mut module = windows_module_fake as *const () as u32;
        let mut install = windows_hook_fake as *const () as u32;
        let mut remove = windows_unhook_fake as *const () as u32;
        let mut parameters = system_parameters_fake as *const () as u32;
        let _version = crate::testing::fake_data(0x00a800f4, (&mut version as *mut u32).cast());
        let _module = crate::testing::fake_data(0x00a800f0, (&mut module as *mut u32).cast());
        let _install = crate::testing::fake_data(0x00a80500, (&mut install as *mut u32).cast());
        let _remove = crate::testing::fake_data(0x00a80504, (&mut remove as *mut u32).cast());
        let _parameters = crate::testing::fake_data(0x00a804c0, (&mut parameters as *mut u32).cast());
        for major in [0u32, 4, 5, 10] {
            WINDOWS_MAJOR.with(|v| v.set(major));
            for initial in [0u32, 1, 2, u32::MAX] {
                for disable in [0u32, 1, 0x80, 0x100] {
                    for old_hook in [0u32, 0x3333_0000] {
                        for new_hook in [0u32, 0x2222_0000] {
                            HOOK_RESULT.with(|v| v.set(new_hook));
                            let mut count = initial;
                            let mut hook = old_hook;
                            let _count = crate::testing::fake_data(0x00b43c24, (&mut count as *mut u32).cast());
                            let _hook = crate::testing::fake_data(0x00b43c20, (&mut hook as *mut u32).cast());
                            run_cdecl(fc4p_005f87d0, &[disable]);
                            let log = calls();
                            let enabling = disable as u8 != 0;
                            let expected_count = if enabling {
                                initial.wrapping_add(1)
                            } else {
                                initial.wrapping_sub(1)
                            };
                            assert_eq!(count, expected_count);
                            let transition = if enabling {
                                expected_count == 1
                            } else {
                                expected_count == 0
                            };
                            let mut expected = vec![(0xf4, vec![0x94])];
                            let mut expected_hook = old_hook;
                            if transition {
                                if major <= 4 {
                                    expected.push((0x4c0, vec![0x61, u32::from(enabling), 0]));
                                } else if enabling {
                                    expected.extend([(0xf0, vec![0]), (0x500, vec![13, 0x005f86e0, 0x11110000, 0])]);
                                    expected_hook = new_hook;
                                } else if old_hook != 0 {
                                    expected.push((0x504, vec![old_hook]));
                                    expected_hook = 0;
                                }
                            }
                            assert_eq!(hook, expected_hook);
                            assert_eq!(
                                log.into_iter().map(|c| (c.address, c.args)).collect::<Vec<_>>(),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn seeded_outside_set() {
        core::arch::naked_asm!(
            "mov dword ptr [esp - 12], 0x11223344", "mov dword ptr [esp - 8], 0x55667788",
            "mov dword ptr [esp - 4], 0x99aabbcc", "jmp {body}", body = sym fc4p_005c6060,
        );
    }
    extern "C" fn find_set_byte_fake(first: u32, last: u32, byte: u32, tag: u32) -> u32 {
        log_record_call(
            0x0044a4b0,
            vec![first, last, byte, unsafe { word(tag as *mut u8, 0) }, unsafe {
                word(tag as *mut u8, usize::MAX - 3)
            }],
        );
        let set = unsafe { core::slice::from_raw_parts(first as *const u8, (last - first) as usize) };
        set.iter()
            .position(|&v| v == byte as u8)
            .map_or(last, |i| first + i as u32)
    }
    #[test]
    fn outside_set_search_keeps_scratch_upper_bytes_groups_tails_and_early_exit_positions() {
        let _find = fake(0x0044a4b0, find_set_byte_fake as *const () as usize);
        let set = b"abcd";
        let set_base = set.as_ptr() as u32;
        for length in 0..=11usize {
            for missing in 0..=length {
                let mut text = *b"abcdabcdabcd";
                if missing < length {
                    text[missing] = b'X';
                }
                let base = text.as_ptr() as u32;
                let result = run_cdecl(
                    seeded_outside_set,
                    &[base, base + length as u32, set_base, set_base + 4, 0xfeed_face],
                );
                assert_eq!(result[0], base + missing as u32);
                let log = calls();
                let count = if missing < length { missing + 1 } else { length };
                assert_eq!(log.len(), count);
                for (i, call) in log.iter().enumerate() {
                    let upper = if i >= length / 4 * 4 || i & 3 == 0 {
                        base & !255
                    } else {
                        [0, 0x11223300, 0x55667700, 0x99aabb00][i & 3]
                    };
                    assert_eq!(
                        call.args,
                        [
                            set_base,
                            set_base + 4,
                            upper | u32::from(text[i]),
                            base + length as u32,
                            (base & !255) | u32::from(text[if i >= length / 4 * 4 { i } else { i / 4 * 4 }])
                        ]
                    );
                }
            }
        }
        let result = run_cdecl(seeded_outside_set, &[0x2000, 0x1fff, set_base, set_base + 4, 0]);
        assert_eq!(result, [0x1fff, 0x11223344, EDX]);
        assert!(calls().is_empty());
    }

    extern "C" fn byte_move_fake(out: u32, source: u32, count: u32) -> u64 {
        log_record_call(0x009eeb70, vec![out, source, count]);
        unsafe {
            core::ptr::copy(source as *const u8, out as *mut u8, count as usize);
        }
        (u64::from(RESULT[2]) << 32) | u64::from(out)
    }
    extern "C" fn byte_copy_fake(first: u32, last: u32, out: u32) -> u64 {
        log_record_call(0x006d6880, vec![first, last, out]);
        let count = last.wrapping_sub(first) as i32;
        for i in 0..count {
            unsafe {
                write_unaligned(
                    out.wrapping_add(i as u32) as *mut u8,
                    read_unaligned(first.wrapping_add(i as u32) as *const u8),
                );
            }
        }
        (u64::from(RESULT[2]) << 32) | u64::from(out.wrapping_add(count.max(0) as u32))
    }
    #[test]
    fn string_range_insertion_covers_empty_reuse_both_tail_lengths_and_new_allocation() {
        let _move = fake(0x009eeb70, byte_move_fake as *const () as usize);
        let _copy = fake(0x006d6880, byte_copy_fake as *const () as usize);
        let _allocate = fake(0x0090cf54, array_allocate as *const () as usize);
        let _free = fake(0x0090cf63, array_free as *const () as usize);
        for text in [&b""[..], &b"a"[..], &b"abcdef"[..]] {
            for insertion in [&b""[..], &b"X"[..], &b"123"[..], &b"12345678"[..]] {
                for position in 0..=text.len() {
                    for spare in [0usize, 1, 3, 9, 40] {
                        let mut buffer = [0xa5u8; 80];
                        buffer[1..text.len() + 1].copy_from_slice(text);
                        buffer[text.len() + 1] = 0;
                        let begin = buffer[1..].as_mut_ptr() as u32;
                        let before = buffer;
                        let mut allocated = [0x5au8; 80];
                        let next = allocated[1..].as_mut_ptr() as u32;
                        ALLOCATED.with(|v| v.set(next));
                        let mut string = [
                            begin,
                            begin + text.len() as u32,
                            begin + (text.len() + spare) as u32,
                            0xfeed_face,
                        ];
                        let p = string.as_mut_ptr().cast();
                        let source = insertion.as_ptr() as u32;
                        let result = run(
                            fc4p_005c2470,
                            p,
                            &[begin + position as u32, source, source + insertion.len() as u32, 0x1234],
                        );
                        let log = calls();
                        if insertion.is_empty() {
                            assert_eq!(result, [source, p as u32, source]);
                            assert!(log.is_empty());
                            assert_eq!(buffer, before);
                            continue;
                        }
                        let grow = spare < insertion.len() + 1;
                        let base = if grow { next } else { begin };
                        let mut expected = Vec::from(&text[..position]);
                        expected.extend_from_slice(insertion);
                        expected.extend_from_slice(&text[position..]);
                        expected.push(0);
                        let output = if grow { &allocated } else { &buffer };
                        assert_eq!(&output[1..expected.len() + 1], expected);
                        assert_eq!(output[0], if grow { 0x5a } else { 0xa5 });
                        assert!(output[expected.len() + 1..]
                            .iter()
                            .all(|&b| b == if grow { 0x5a } else { 0xa5 }));
                        assert_eq!(string[0], base);
                        assert_eq!(string[1], base + (text.len() + insertion.len()) as u32);
                        assert_eq!(string[3], 0xfeed_face);
                        if grow {
                            let capacity = text.len() + text.len().max(insertion.len()) + 1;
                            assert_eq!(string[2], next + capacity as u32);
                            assert_eq!(result, [next + capacity as u32, capacity as u32, RESULT[2]]);
                            assert_eq!(
                                log.iter().map(|c| c.address).collect::<Vec<_>>(),
                                [0x0090cf54, 0x009eeb70, 0x0090cf63]
                            );
                            assert_eq!(log[0].args, [capacity as u32]);
                            assert_eq!(log[1].args, [next + position as u32, source, insertion.len() as u32]);
                            assert_eq!(log[2].args, [begin]);
                            assert_eq!(buffer, before);
                        } else {
                            assert_eq!(string[2], begin + (text.len() + spare) as u32);
                            assert_eq!(result[1], p as u32);
                            if text.len() - position >= insertion.len() {
                                assert_eq!(result[0], insertion.len() as u32);
                                assert_eq!(result[2], RESULT[2]);
                                assert_eq!(
                                    log.iter().map(|c| c.address).collect::<Vec<_>>(),
                                    [0x006d6880, 0x009eeb70]
                                );
                                assert_eq!(
                                    log[0].args,
                                    [
                                        begin + (text.len() - insertion.len() + 1) as u32,
                                        begin + text.len() as u32 + 1,
                                        begin + text.len() as u32 + 1
                                    ]
                                );
                                assert_eq!(
                                    log[1].args,
                                    [
                                        begin + (position + insertion.len()) as u32,
                                        begin + position as u32,
                                        (text.len() - position - insertion.len() + 1) as u32
                                    ]
                                );
                            } else {
                                assert_eq!(result[0], (text.len() - position + 1) as u32);
                                assert_eq!(result[2], string[1]);
                                let move_needed = insertion.len() != text.len() - position + 1;
                                assert_eq!(
                                    log.iter().map(|c| c.address).collect::<Vec<_>>(),
                                    if move_needed {
                                        vec![0x009eeb70, 0x006d6880]
                                    } else {
                                        vec![0x006d6880]
                                    }
                                );
                                let copy = log.last().unwrap();
                                assert_eq!(
                                    copy.args,
                                    [
                                        begin + position as u32,
                                        begin + text.len() as u32 + 1,
                                        begin + (insertion.len() + position) as u32
                                    ]
                                );
                            }
                        }
                    }
                }
            }
        }
        // The empty string can also start with a null allocation.
        let mut allocated = [0xa5u8; 16];
        let next = allocated.as_mut_ptr() as u32;
        ALLOCATED.with(|v| v.set(next));
        let item = b"XY";
        let mut string = [0u32; 3];
        run(
            fc4p_005c2470,
            string.as_mut_ptr().cast(),
            &[0, item.as_ptr() as u32, item.as_ptr() as u32 + 2, 0],
        );
        assert_eq!(string, [next, next + 2, next + 3]);
        assert_eq!(&allocated[..4], b"XY\0\xa5");
        assert_eq!(
            calls().iter().map(|c| c.address).collect::<Vec<_>>(),
            [0x0090cf54, 0x009eeb70]
        );
    }
    #[test]
    fn string_insertion_with_aliased_source_observes_shifted_bytes_and_four_byte_copy_units() {
        let _move = fake(0x009eeb70, byte_move_fake as *const () as usize);
        let _copy = fake(0x006d6880, byte_copy_fake as *const () as usize);
        for (text, at, first, last, expected) in [
            (&b"abcdef"[..], 2u32, 4u32, 6u32, &b"abcdcdef"[..]),
            (&b"abcdefghijkl"[..], 1, 0, 8, &b"aabcddfghbcdefghijkl"[..]),
        ] {
            let mut buffer = [0xa5u8; 64];
            buffer[..text.len()].copy_from_slice(text);
            buffer[text.len()] = 0;
            let base = buffer.as_mut_ptr() as u32;
            let mut string = [base, base + text.len() as u32, base + 64];
            run(
                fc4p_005c2470,
                string.as_mut_ptr().cast(),
                &[base + at, base + first, base + last, 0],
            );
            assert_eq!(&buffer[..expected.len()], expected);
            assert_eq!(buffer[expected.len()], 0);
            assert_eq!(string[1], base + expected.len() as u32);
            assert_eq!(
                calls().iter().map(|c| c.address).collect::<Vec<_>>(),
                [0x006d6880, 0x009eeb70]
            );
        }
    }

    #[test]
    fn integer_ports_preserve_callee_saved_registers_and_float_environment() {
        let _fpu = GameFpu::new();
        let mut object = scan([0, 0, 2, 1], 1, 0, 0, 0);
        let mut out = [0u32; 2];
        let p = object.as_mut_ptr().cast::<u8>();
        let q = out.as_mut_ptr() as u32;
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]",p=in(reg) &sentinel,out("st(0)") _);
        }
        let before = environment();
        let args = [q, q + 4];
        let mut c = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            fc4p_0060c8d0 as *const () as usize,
            p as u32,
            &args,
        );
        let (ebx, esi, ebp): (u32, u32, u32);
        // EDI carries the descriptor across the call; its integrity also checks that register.
        unsafe {
            core::arch::asm!(
                "push ebx", "push esi", "push ebp", "push dword ptr [edi+16]",
                "mov eax,[edi+16]", "push dword ptr [eax+4]", "push dword ptr [eax]",
                "mov ecx,[edi+8]", "mov ebx,0x12345678", "mov esi,0x87654321", "mov ebp,0x11223344",
                "call dword ptr [edi]", "mov eax,ebx", "mov ecx,esi", "mov edx,ebp",
                "add esp,4", "pop ebp", "pop esi", "pop ebx",
                in("edi") &mut c, lateout("eax") ebx,lateout("ecx") esi,lateout("edx") ebp,clobber_abi("C"),
            );
        }
        let after = environment();
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]",p=in(reg) &mut restored,out("st(0)") _);
        }
        assert_eq!((ebx, esi, ebp), (0x12345678, 0x87654321, 0x11223344));
        assert_eq!(after, before);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
        assert_eq!(out, [0, 0]);
    }
}
