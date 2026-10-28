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

/// This module's complete replacements.
pub const PORTS: &[Port] = &[
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
    fc4p_005fd720::PORT,
    fc4p_0060ce80::PORT,
    fc4p_0060fb50::PORT,
    fc4p_0060fb80::PORT,
    fc4p_0062c8e0::PORT,
    fc4p_0062c980::PORT,
    fc4p_0062c990::PORT,
    fc4p_0062c9b0::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, GameFpu};
    use std::cell::RefCell;
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
    extern "C" fn record(address: u32, r: *const Registers, args: *const u32, count: u32) {
        unsafe {
            CALLS.with(|calls| {
                calls.borrow_mut().push(Call {
                    address,
                    registers: [(*r).eax, (*r).ecx, (*r).edx],
                    args: core::slice::from_raw_parts(args, count as usize).to_vec(),
                })
            });
        }
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
}
