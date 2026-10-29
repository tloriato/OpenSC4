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
            let mut tag = source as u32;
            r.edx = &mut tag as *mut u32 as u32;
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
                let mut tag = erased;
                r.edx = &mut tag as *mut u32 as u32;
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

/// This module's complete replacements.
pub const PORTS: &[Port] = &[
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
        for (first, quads) in [(0, 0), (0, 1), (6, 3), (98_304, 2)] {
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
