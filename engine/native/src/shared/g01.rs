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

//! Shared helpers, group 01: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

/// Integer entry and exit registers, followed on the entry's stack by its return address and arguments.
#[repr(C)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

/// Marshals the original registers to a Rust body and restores its three integer results.
macro_rules! register_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $pop:literal) => {
        port! {
            /// Runs the helper on the original arguments, retaining its integer return registers.
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov ecx, esp", "call {}",
                    "pop eax", "pop ecx", "pop edx", "ret {pop}", sym $body, pop = const $pop);
            }
        }
    };
}

impl Registers {
    unsafe fn argument_ptr(&self, index: usize) -> *mut u32 {
        // SAFETY: the entry's caller supplied this argument word.
        unsafe { (self as *const Self).cast::<u32>().add(4 + index).cast_mut() }
    }

    unsafe fn argument(&self, index: usize) -> u32 {
        // SAFETY: the entry's caller supplied this argument word.
        unsafe { read_unaligned((self as *const Self).cast::<u32>().add(4 + index)) }
    }
}

/// Calls with the register inputs used by the helper and captures ECX as well as EDX:EAX.
unsafe fn invoke(r: &mut Registers, target: usize, this: u32, args: &[u32], conv: crate::abi::Conv) {
    let mut c = crate::abi::RawCall::new(conv, target, this, args);
    c.eax = r.eax;
    c.edx = r.edx;
    // SAFETY: the enclosing port supplies the target's original convention and argument words.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
            "4:", in("edi") &mut c, clobber_abi("C"));
    }
    r.eax = c.eax;
    r.ecx = c.ecx;
    r.edx = c.edx;
}

macro_rules! game_entry {
    ($name:ident, $address:literal, $abi:literal, ($($arg:ident: $ty:ty),*)) => {
        game_fn! {
            /// Original call target; register-sensitive calls use its address and capture all integer results.
            #[allow(dead_code)]
            fn $name($($arg: $ty),*) -> u64 = $abi $address;
        }
    };
}

game_entry!(allocate_bytes, 0x0090cf54, "C", (size: u32));
game_entry!(release_bytes, 0x0090cf63, "C", (p: *mut u8));
game_entry!(copy_bytes, 0x009eeb70, "C", (dest: *mut u8, source: *mut u8, count: u32));
game_entry!(string_length_error, 0x009dd1d0, "C", (message: u32));
game_entry!(string_position_error, 0x009dd0a0, "C", (message: u32));
game_entry!(construct_string_range, 0x00407fa0, "thiscall", (this: *mut u8, first: u32, last: u32, marker: u32));
game_entry!(construct_string_subrange, 0x00408420, "thiscall", (this: *mut u8, first: u32, last: u32, marker: u32));
game_entry!(assign_string_range, 0x004081d0, "thiscall", (this: *mut u8, first: u32, last: u32, marker: u32));
game_entry!(append_string_range, 0x0040ebc0, "thiscall", (this: *mut u8, first: u32, last: u32, marker: u32));
game_entry!(write_string, 0x005d99a0, "C", (stream: *mut u8, string: *mut u8));

const STRING_ERROR_TEXT: u32 = 0x00a8_06f0;
game_data! {
    /// Text passed to the string length and position error helpers.
    #[allow(dead_code)]
    static string_error_text: u8 = 0x00a806f0;
}

register_port!(
    0x00407fa0,
    fc4p_00407fa0,
    initialize_string_from_range,
    "initialize_string_from_range",
    12
);
unsafe extern "fastcall" fn initialize_string_from_range(r: &mut Registers) {
    use crate::abi::{target, Conv};
    unsafe {
        let this = r.ecx;
        let first = r.argument(0);
        let len = r.argument(1).wrapping_sub(first);
        let capacity = len.wrapping_add(1);
        if capacity != 0 {
            invoke(r, target(0x0090cf54), r.ecx, &[capacity], Conv::Cdecl);
            put_word(this, 0, r.eax);
            put_word(this, 4, r.eax);
            r.eax = r.eax.wrapping_add(capacity);
            put_word(this, 8, r.eax);
        } else {
            invoke(r, target(0x009dd1d0), r.ecx, &[STRING_ERROR_TEXT], Conv::Cdecl);
        }
        r.eax = word(this, 0);
        if r.argument(1) != first {
            invoke(r, target(0x009eeb70), r.ecx, &[r.eax, first, len], Conv::Cdecl);
            r.eax = r.eax.wrapping_add(len);
        }
        put_word(this, 4, r.eax);
        write_unaligned(r.eax as *mut u8, 0);
    }
}

register_port!(
    0x00408000,
    fc4p_00408000,
    initialize_string_from_c_string,
    "initialize_string_from_c_string",
    8
);
unsafe extern "fastcall" fn initialize_string_from_c_string(r: &mut Registers) {
    unsafe {
        let this = r.ecx;
        let first = r.argument(0);
        r.edx = first;
        for offset in [0, 4, 8] {
            put_word(this, offset, 0);
        }
        let mut end = first;
        while read_unaligned(end as *const u8) != 0 {
            end = end.wrapping_add(1);
        }
        r.eax = end;
        invoke(
            r,
            crate::abi::target(0x00407fa0),
            this,
            &[first, end, r.argument_ptr(0) as u32],
            crate::abi::Conv::Thiscall,
        );
        r.eax = this;
    }
}

register_port!(
    0x00408170,
    fc4p_00408170,
    initialize_string_capacity,
    "initialize_string_capacity",
    12
);
unsafe extern "fastcall" fn initialize_string_capacity(r: &mut Registers) {
    unsafe {
        let this = r.ecx;
        let capacity = r.argument(1).wrapping_add(1);
        for offset in [0, 4, 8] {
            put_word(this, offset, 0);
        }
        if capacity != 0 {
            invoke(
                r,
                crate::abi::target(0x0090cf54),
                r.ecx,
                &[capacity],
                crate::abi::Conv::Cdecl,
            );
            put_word(this, 0, r.eax);
            put_word(this, 4, r.eax);
            r.eax = r.eax.wrapping_add(capacity);
            put_word(this, 8, r.eax);
        } else {
            invoke(
                r,
                crate::abi::target(0x009dd1d0),
                r.ecx,
                &[STRING_ERROR_TEXT],
                crate::abi::Conv::Cdecl,
            );
        }
        r.eax = word(this, 4);
        write_unaligned(r.eax as *mut u8, 0);
        r.eax = this;
    }
}

register_port!(
    0x004089b0,
    fc4p_004089b0,
    initialize_string_substring,
    "initialize_string_substring",
    16
);
unsafe extern "fastcall" fn initialize_string_substring(r: &mut Registers) {
    unsafe {
        let this = r.ecx;
        r.edx = r.argument(1);
        r.eax = 0;
        for offset in [0, 4, 8] {
            put_word(this, offset, 0);
        }
        r.eax = r.argument(0);
        r.ecx = word(r.eax, 0);
        r.eax = word(r.eax, 4).wrapping_sub(r.ecx);
        if r.edx > r.eax {
            invoke(
                r,
                crate::abi::target(0x009dd0a0),
                r.ecx,
                &[STRING_ERROR_TEXT],
                crate::abi::Conv::Cdecl,
            );
        } else {
            r.eax = r.eax.wrapping_sub(r.edx);
            write_unaligned(r.argument_ptr(0), r.eax);
            r.eax = r.eax.min(r.argument(2));
            r.eax = r.eax.wrapping_add(r.ecx).wrapping_add(r.edx);
            r.ecx = r.ecx.wrapping_add(r.edx);
            invoke(
                r,
                crate::abi::target(0x00408420),
                this,
                &[r.ecx, r.eax, r.argument_ptr(2) as u32],
                crate::abi::Conv::Thiscall,
            );
        }
        r.eax = this;
    }
}

register_port!(
    0x00408c20,
    fc4p_00408c20,
    assign_string_from_buffer,
    "assign_string_from_buffer",
    4
);
unsafe extern "fastcall" fn assign_string_from_buffer(r: &mut Registers) {
    unsafe {
        r.eax = r.argument(0);
        r.edx = word(r.eax, 4);
        r.eax = word(r.eax, 0);
        invoke(
            r,
            crate::abi::target(0x004081d0),
            r.ecx,
            &[r.eax, r.edx, r.argument_ptr(0) as u32],
            crate::abi::Conv::Thiscall,
        );
    }
}

register_port!(
    0x0040add0,
    fc4p_0040add0,
    initialize_string_from_count,
    "initialize_string_from_count",
    12
);
unsafe extern "fastcall" fn initialize_string_from_count(r: &mut Registers) {
    unsafe {
        let this = r.ecx;
        r.ecx = r.argument(1);
        for offset in [0, 4, 8] {
            put_word(this, offset, 0);
        }
        r.eax = r.argument(0);
        r.edx = r.eax.wrapping_add(r.ecx);
        invoke(
            r,
            crate::abi::target(0x00407fa0),
            this,
            &[r.eax, r.edx, r.argument_ptr(0) as u32],
            crate::abi::Conv::Thiscall,
        );
        r.eax = this;
    }
}

register_port!(0x00410f00, fc4p_00410f00, append_c_string, "append_c_string", 4);
unsafe extern "fastcall" fn append_c_string(r: &mut Registers) {
    unsafe {
        let first = r.argument(0);
        let mut end = first;
        while read_unaligned(end as *const u8) != 0 {
            end = end.wrapping_add(1);
        }
        r.edx = r.argument_ptr(0) as u32;
        r.eax = end;
        invoke(
            r,
            crate::abi::target(0x0040ebc0),
            r.ecx,
            &[first, end, r.edx],
            crate::abi::Conv::Thiscall,
        );
    }
}

/// Calls a virtual slot with the register inputs supplied by the enclosing helper.
unsafe fn virtual_call(r: &mut Registers, object: u32, slot: usize, args: &[u32]) {
    unsafe {
        invoke(
            r,
            crate::abi::vslot(object as *mut u8, slot),
            object,
            args,
            crate::abi::Conv::Thiscall,
        );
    }
}

register_port!(0x0042e440, fc4p_0042e440, write_version_one, "write_version_one", 4);
unsafe extern "fastcall" fn write_version_one(r: &mut Registers) {
    unsafe {
        let stream = r.argument(0);
        r.eax = word(stream, 0);
        virtual_call(r, stream, 0x1c, &[1]);
        r.edx = word(stream, 0);
        virtual_call(r, stream, 0x48, &[]);
        r.eax = u32::from(r.eax as u8 == 0);
    }
}

register_port!(0x00408090, fc4p_00408090, write_advice_record, "write_advice_record", 4);
unsafe extern "fastcall" fn write_advice_record(r: &mut Registers) {
    unsafe {
        let this = r.ecx;
        let stream = r.argument(0);
        r.eax = word(stream, 0);
        virtual_call(r, stream, 0x1c, &[4]);
        for offset in [0x7c, 0x80, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78] {
            r.eax = word(this, offset);
            r.edx = word(stream, 0);
            virtual_call(r, stream, 0x20, &[r.eax]);
        }
        r.ecx = word(this, 0xb0);
        r.eax = word(r.ecx, 0);
        r.edx = 0;
        while r.eax != r.ecx {
            r.eax = word(r.eax, 0);
            r.edx = r.edx.wrapping_add(1);
        }
        let count = r.edx;
        r.eax = word(stream, 0);
        virtual_call(r, stream, 0x20, &[count]);
        r.eax = word(this, 0xb0);
        let mut node = word(r.eax, 0);
        if node != r.eax {
            loop {
                r.edx = word(stream, 0);
                r.eax = word(node, 8);
                node = word(node, 0);
                virtual_call(r, stream, 0x20, &[r.eax]);
                if node == word(this, 0xb0) {
                    break;
                }
            }
        }
        r.ecx = word(this, 0xa0);
        invoke(
            r,
            crate::abi::target(0x005d99a0),
            r.ecx,
            &[stream, r.ecx],
            crate::abi::Conv::Cdecl,
        );
        r.edx = word(stream, 0);
        virtual_call(r, stream, 0x48, &[]);
        r.eax = u32::from(r.eax as u8 == 0);
    }
}

// All word accesses below use the live storage supplied to the enclosing entry.
unsafe fn word(p: u32, offset: usize) -> u32 {
    unsafe { read_unaligned((p as *const u8).wrapping_add(offset).cast()) }
}

unsafe fn put_word(p: u32, offset: usize, value: u32) {
    unsafe { write_unaligned((p as *mut u8).wrapping_add(offset).cast(), value) }
}

register_port!(0x00411d50, fc4p_00411d50, count_list_nodes, "count_list_nodes", 0);
unsafe extern "fastcall" fn count_list_nodes(r: &mut Registers) {
    unsafe {
        r.edx = word(r.ecx, 0);
        r.ecx = word(r.edx, 0);
        r.eax = 0;
        while r.ecx != r.edx {
            r.ecx = word(r.ecx, 0);
            r.eax = r.eax.wrapping_add(1);
        }
    }
}

register_port!(0x00431180, fc4p_00431180, equal_three_words, "equal_three_words", 4);
unsafe extern "fastcall" fn equal_three_words(r: &mut Registers) {
    unsafe {
        let other = r.argument(0);
        r.edx = word(r.ecx, 0);
        if r.edx != word(other, 0) {
            r.eax = 0;
            return;
        }
        r.edx = word(r.ecx, 4);
        if r.edx != word(other, 4) {
            r.eax = 0;
            return;
        }
        r.ecx = word(r.ecx, 8);
        r.eax = u32::from(r.ecx == word(other, 8));
    }
}

register_port!(
    0x00431ac0,
    fc4p_00431ac0,
    copy_member_four_words,
    "copy_member_four_words",
    4
);
unsafe extern "fastcall" fn copy_member_four_words(r: &mut Registers) {
    unsafe {
        r.eax = r.argument(0);
        let source = r.ecx.wrapping_add(0x10);
        r.edx = r.eax;
        for offset in [0, 4, 8] {
            put_word(r.edx, offset, word(source, offset));
        }
        r.ecx = word(source, 12);
        put_word(r.edx, 12, r.ecx);
    }
}

register_port!(0x00473080, fc4p_00473080, copy_four_words, "copy_four_words", 4);
unsafe extern "fastcall" fn copy_four_words(r: &mut Registers) {
    unsafe {
        r.eax = r.ecx;
        r.ecx = r.argument(0);
        for offset in [0, 4, 8] {
            r.edx = word(r.ecx, offset);
            put_word(r.eax, offset, r.edx);
        }
        r.ecx = word(r.ecx, 12);
        put_word(r.eax, 12, r.ecx);
    }
}

register_port!(
    0x00437ad0,
    fc4p_00437ad0,
    float_greater_or_equal,
    "float_greater_or_equal",
    4
);
unsafe extern "fastcall" fn float_greater_or_equal(r: &mut Registers) {
    unsafe {
        let other = r.argument(0);
        let left = F80::from_f32(f32::from_bits(word(r.ecx, 0)));
        let right = F80::from_f32(f32::from_bits(word(other, 0)));
        let cmp = left.compare(right);
        r.eax = u32::from(!cmp.less() && !cmp.unordered());
    }
}

register_port!(0x004622a0, fc4p_004622a0, copy_halfword_range, "copy_halfword_range", 0);
unsafe extern "fastcall" fn copy_halfword_range(r: &mut Registers) {
    unsafe {
        r.edx = r.argument(0);
        r.ecx = (r.argument(1).wrapping_sub(r.edx) as i32 >> 1) as u32;
        r.eax = r.argument(2);
        if r.ecx as i32 > 0 {
            r.edx = r.edx.wrapping_sub(r.eax);
            while r.ecx != 0 {
                let value = read_unaligned(r.edx.wrapping_add(r.eax) as *const u16);
                write_unaligned(r.eax as *mut u16, value);
                r.eax = r.eax.wrapping_add(2);
                r.ecx = r.ecx.wrapping_sub(1);
            }
        }
    }
}

register_port!(0x00465890, fc4p_00465890, fill_halfwords, "fill_halfwords", 0);
unsafe extern "fastcall" fn fill_halfwords(r: &mut Registers) {
    unsafe {
        r.ecx = r.argument(1);
        r.eax = r.argument(0);
        if r.ecx != 0 {
            r.edx = r.argument(2);
            while r.ecx != 0 {
                // Read on every iteration: the value can alias a destination halfword.
                let value = read_unaligned(r.edx as *const u16);
                write_unaligned(r.eax as *mut u16, value);
                r.eax = r.eax.wrapping_add(2);
                r.ecx = r.ecx.wrapping_sub(1);
            }
        }
    }
}

register_port!(0x0048c980, fc4p_0048c980, copy_word_range, "copy_word_range", 0);
unsafe extern "fastcall" fn copy_word_range(r: &mut Registers) {
    unsafe {
        r.ecx = r.argument(0);
        let count = r.argument(1).wrapping_sub(r.ecx) as i32 >> 2;
        r.eax = r.argument(2);
        if count >= 4 {
            r.edx = count as u32 / 4;
            while r.edx != 0 {
                for offset in [0, 4, 8, 12] {
                    put_word(r.eax, offset, word(r.ecx, offset));
                }
                r.ecx = r.ecx.wrapping_add(16);
                r.eax = r.eax.wrapping_add(16);
                r.edx -= 1;
            }
        }
        let remaining = if count >= 4 { count % 4 } else { count };
        if remaining > 0 {
            r.edx = remaining as u32;
            while r.edx != 0 {
                put_word(r.eax, 0, word(r.ecx, 0));
                r.ecx = r.ecx.wrapping_add(4);
                r.eax = r.eax.wrapping_add(4);
                r.edx -= 1;
            }
        }
    }
}

register_port!(
    0x00466660,
    fc4p_00466660,
    find_first_maximum_float,
    "find_first_maximum_float",
    0
);
unsafe extern "fastcall" fn find_first_maximum_float(r: &mut Registers) {
    unsafe {
        r.ecx = r.argument(0);
        let end = r.argument(1);
        if r.ecx == end {
            r.eax = r.ecx;
            return;
        }
        r.edx = r.ecx;
        r.ecx = r.ecx.wrapping_add(4);
        while r.ecx != end {
            let best = F80::from_f32(f32::from_bits(word(r.edx, 0)));
            let next = F80::from_f32(f32::from_bits(word(r.ecx, 0)));
            if best.compare(next).less() {
                r.edx = r.ecx;
            }
            r.ecx = r.ecx.wrapping_add(4);
        }
        r.eax = r.edx;
    }
}

register_port!(0x0040d500, fc4p_0040d500, previous_tree_node, "previous_tree_node", 0);
unsafe extern "fastcall" fn previous_tree_node(r: &mut Registers) {
    unsafe {
        r.ecx = r.argument(0);
        if read_unaligned(r.ecx as *const u8) == 0 {
            r.eax = word(r.ecx, 4);
            if word(r.eax, 4) == r.ecx {
                r.eax = word(r.ecx, 12);
                return;
            }
        }
        r.eax = word(r.ecx, 8);
        if r.eax != 0 {
            r.ecx = word(r.eax, 12);
            while r.ecx != 0 {
                r.eax = r.ecx;
                r.ecx = word(r.eax, 12);
            }
        } else {
            r.eax = word(r.ecx, 4);
            while r.ecx == word(r.eax, 8) {
                r.ecx = r.eax;
                r.eax = word(r.eax, 4);
            }
        }
    }
}

game_data! {
    /// Default scalar loaded while constructing the transform.
    static default_transform_scalar: u32 = 0x00b215e8;
}

register_port!(
    0x00466e10,
    fc4p_00466e10,
    initialize_unit_transform,
    "initialize_unit_transform",
    0
);
unsafe extern "fastcall" fn initialize_unit_transform(r: &mut Registers) {
    unsafe {
        r.eax = r.ecx;
        r.ecx = default_transform_scalar().read_unaligned();
        // These initial stores are observable when the object overlaps the default scalar.
        for offset in [12, 20, 16] {
            core::ptr::write_volatile((r.eax as *mut u8).add(offset).cast::<u32>(), r.ecx);
        }
        r.edx = r.ecx;
        for offset in [0, 4, 8] {
            put_word(r.eax, offset, 0x3f80_0000);
        }
        r.ecx = 0;
        for offset in [12, 16, 20] {
            put_word(r.eax, offset, 0);
        }
    }
}

register_port!(
    0x0045ddb0,
    fc4p_0045ddb0,
    initialize_byte_grid,
    "initialize_byte_grid",
    4
);
unsafe extern "fastcall" fn initialize_byte_grid(r: &mut Registers) {
    unsafe {
        r.eax = r.ecx;
        core::ptr::write_volatile((r.eax as *mut u8).add(4).cast::<u32>(), 0x00a8_0784);
        r.ecx = 0;
        put_word(r.eax, 0, 0x00a8_78c0);
        put_word(r.eax, 4, 0x00a8_78a4);
        write_unaligned((r.eax as *mut u8).add(8), 0);
        put_word(r.eax, 12, 0);
        put_word(r.eax, 16, 0x49b9_e602);
        for offset in [24, 56, 60, 64] {
            put_word(r.eax, offset, 0);
        }
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00407a90, owner = "shared", name = "return_identifier_8a09f5f4", naked)]
    pub unsafe extern "C" fn fc4p_00407a90() {
        core::arch::naked_asm!("mov eax, 0x8a09f5f4", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x0040ce00, owner = "shared", name = "return_identifier_6a26d049", naked)]
    pub unsafe extern "C" fn fc4p_0040ce00() {
        core::arch::naked_asm!("mov eax, 0x6a26d049", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x0040d380, owner = "shared", name = "return_identifier_eac937b4", naked)]
    pub unsafe extern "C" fn fc4p_0040d380() {
        core::arch::naked_asm!("mov eax, 0xeac937b4", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00420d40, owner = "shared", name = "return_identifier_6a3848db", naked)]
    pub unsafe extern "C" fn fc4p_00420d40() {
        core::arch::naked_asm!("mov eax, 0x6a3848db", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00423000, owner = "shared", name = "return_identifier_8be3753b", naked)]
    pub unsafe extern "C" fn fc4p_00423000() {
        core::arch::naked_asm!("mov eax, 0x8be3753b", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x0042f6d0, owner = "shared", name = "return_identifier_6b70efad", naked)]
    pub unsafe extern "C" fn fc4p_0042f6d0() {
        core::arch::naked_asm!("mov eax, 0x6b70efad", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00466490, owner = "shared", name = "return_identifier_0990c38a", naked)]
    pub unsafe extern "C" fn fc4p_00466490() {
        core::arch::naked_asm!("mov eax, 0x0990c38a", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x004695a0, owner = "shared", name = "return_identifier_4990bd93", naked)]
    pub unsafe extern "C" fn fc4p_004695a0() {
        core::arch::naked_asm!("mov eax, 0x4990bd93", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00477100, owner = "shared", name = "return_identifier_8a22bebe", naked)]
    pub unsafe extern "C" fn fc4p_00477100() {
        core::arch::naked_asm!("mov eax, 0x8a22bebe", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x78.
    #[port(address = 0x00407ab0, owner = "shared", name = "read_word_at_78", naked)]
    pub unsafe extern "C" fn fc4p_00407ab0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x78]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x74.
    #[port(address = 0x00407ac0, owner = "shared", name = "read_word_at_74", naked)]
    pub unsafe extern "C" fn fc4p_00407ac0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x74]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x7C.
    #[port(address = 0x00407ba0, owner = "shared", name = "read_word_at_7c", naked)]
    pub unsafe extern "C" fn fc4p_00407ba0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x7c]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x10.
    #[port(address = 0x0040ce60, owner = "shared", name = "read_word_at_10", naked)]
    pub unsafe extern "C" fn fc4p_0040ce60() {
        core::arch::naked_asm!("mov eax, [ecx + 0x10]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0xE0.
    #[port(address = 0x00423650, owner = "shared", name = "read_word_at_e0", naked)]
    pub unsafe extern "C" fn fc4p_00423650() {
        core::arch::naked_asm!("mov eax, [ecx + 0xe0]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x110.
    #[port(address = 0x00423700, owner = "shared", name = "read_word_at_110", naked)]
    pub unsafe extern "C" fn fc4p_00423700() {
        core::arch::naked_asm!("mov eax, [ecx + 0x110]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x12C.
    #[port(address = 0x00423710, owner = "shared", name = "read_word_at_12c", naked)]
    pub unsafe extern "C" fn fc4p_00423710() {
        core::arch::naked_asm!("mov eax, [ecx + 0x12c]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x14C.
    #[port(address = 0x00423750, owner = "shared", name = "read_word_at_14c", naked)]
    pub unsafe extern "C" fn fc4p_00423750() {
        core::arch::naked_asm!("mov eax, [ecx + 0x14c]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x198.
    #[port(address = 0x00423850, owner = "shared", name = "read_word_at_198", naked)]
    pub unsafe extern "C" fn fc4p_00423850() {
        core::arch::naked_asm!("mov eax, [ecx + 0x198]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x150.
    #[port(address = 0x0043d230, owner = "shared", name = "read_word_at_150", naked)]
    pub unsafe extern "C" fn fc4p_0043d230() {
        core::arch::naked_asm!("mov eax, [ecx + 0x150]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x1C0.
    #[port(address = 0x00459880, owner = "shared", name = "read_word_at_1c0", naked)]
    pub unsafe extern "C" fn fc4p_00459880() {
        core::arch::naked_asm!("mov eax, [ecx + 0x1c0]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x8.
    #[port(address = 0x004777e0, owner = "shared", name = "read_word_at_8", naked)]
    pub unsafe extern "C" fn fc4p_004777e0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x8]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x50, without reading it.
    #[port(address = 0x00407bb0, owner = "shared", name = "address_of_member_at_50", naked)]
    pub unsafe extern "C" fn fc4p_00407bb0() {
        core::arch::naked_asm!("lea eax, [ecx + 0x50]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x24, without reading it.
    #[port(address = 0x00477810, owner = "shared", name = "address_of_member_at_24", naked)]
    pub unsafe extern "C" fn fc4p_00477810() {
        core::arch::naked_asm!("lea eax, [ecx + 0x24]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x90, without reading it.
    #[port(address = 0x00477890, owner = "shared", name = "address_of_member_at_90", naked)]
    pub unsafe extern "C" fn fc4p_00477890() {
        core::arch::naked_asm!("lea eax, [ecx + 0x90]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x9C, without reading it.
    #[port(address = 0x004778a0, owner = "shared", name = "address_of_member_at_9c", naked)]
    pub unsafe extern "C" fn fc4p_004778a0() {
        core::arch::naked_asm!("lea eax, [ecx + 0x9c]", "ret");
    }
}

port! {
    /// Loads the float at offset 0x38 onto the caller's x87 stack.
    #[port(address = 0x00477860, owner = "shared", name = "load_float_at_38", naked)]
    pub unsafe extern "C" fn fc4p_00477860() {
        core::arch::naked_asm!("fld dword ptr [ecx + 0x38]", "ret");
    }
}

port! {
    /// Restores the value's vtable word and preserves the entry registers.
    #[port(address = 0x00489660, owner = "shared", name = "restore_value_vtable", naked)]
    pub unsafe extern "C" fn fc4p_00489660() {
        core::arch::naked_asm!("mov dword ptr [ecx], 0x00a8ec1c", "ret");
    }
}

port! {
    /// Forwards the original arguments to virtual slot 0x24.
    #[port(address = 0x00420130, owner = "shared", name = "forward_virtual_slot_24", naked)]
    pub unsafe extern "C" fn fc4p_00420130() {
        core::arch::naked_asm!("mov eax, [ecx]", "jmp dword ptr [eax + 0x24]");
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_advice_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x00407bc0;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_advice_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00407bc0",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x00407bc0.
    #[port(address = 0x00408720, owner = "shared", name = "query_advice_primary", naked)]
    pub unsafe extern "C" fn fc4p_00408720() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00407bc0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_advice_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn delete_advice_primary_target(this: *mut u8, arg0: u32) -> u64 = "thiscall" 0x0043ec10;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn delete_advice_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0043ec10",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0043ec10.
    #[port(address = 0x00408730, owner = "shared", name = "delete_advice_primary", naked)]
    pub unsafe extern "C" fn fc4p_00408730() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0043ec10");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym delete_advice_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_advisor_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0040d430;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_advisor_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0040d430",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -8 bytes and forwards to 0x0040d430.
    #[port(address = 0x0040f370, owner = "shared", name = "query_advisor_primary", naked)]
    pub unsafe extern "C" fn fc4p_0040f370() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x0040d430");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym query_advisor_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn retain_advisor_reference_target(this: *mut u8) -> u64 = "thiscall" 0x00968b60;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn retain_advisor_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00968b60",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 20 bytes and forwards to 0x00968b60.
    #[port(address = 0x004186e0, owner = "shared", name = "retain_advisor_reference", naked)]
    pub unsafe extern "C" fn fc4p_004186e0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 20", "jmp dword ptr [2f]", "2: .long 0x00968b60");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 20", "jmp {}", sym retain_advisor_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn retain_advisor_primary_target(this: *mut u8) -> u64 = "thiscall" 0x004186e0;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn retain_advisor_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x004186e0",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -16 bytes and forwards to 0x004186e0.
    #[port(address = 0x0041a260, owner = "shared", name = "retain_advisor_primary", naked)]
    pub unsafe extern "C" fn fc4p_0041a260() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 16", "jmp dword ptr [2f]", "2: .long 0x004186e0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 16", "jmp {}", sym retain_advisor_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn release_advisor_primary_target(this: *mut u8) -> u64 = "thiscall" 0x00449310;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn release_advisor_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00449310",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -16 bytes and forwards to 0x00449310.
    #[port(address = 0x0041a270, owner = "shared", name = "release_advisor_primary", naked)]
    pub unsafe extern "C" fn fc4p_0041a270() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 16", "jmp dword ptr [2f]", "2: .long 0x00449310");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 16", "jmp {}", sym release_advisor_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn retain_overlay_reference_target(this: *mut u8) -> u64 = "thiscall" 0x00968b60;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn retain_overlay_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00968b60",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 8 bytes and forwards to 0x00968b60.
    #[port(address = 0x004302a0, owner = "shared", name = "retain_overlay_reference", naked)]
    pub unsafe extern "C" fn fc4p_004302a0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 8", "jmp dword ptr [2f]", "2: .long 0x00968b60");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 8", "jmp {}", sym retain_overlay_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn release_overlay_reference_target(this: *mut u8) -> u64 = "thiscall" 0x0090d997;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn release_overlay_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0090d997",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 8 bytes and forwards to 0x0090d997.
    #[port(address = 0x004302b0, owner = "shared", name = "release_overlay_reference", naked)]
    pub unsafe extern "C" fn fc4p_004302b0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 8", "jmp dword ptr [2f]", "2: .long 0x0090d997");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 8", "jmp {}", sym release_overlay_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn release_advisor_reference_target(this: *mut u8) -> u64 = "thiscall" 0x0090d997;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn release_advisor_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0090d997",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 20 bytes and forwards to 0x0090d997.
    #[port(address = 0x00449310, owner = "shared", name = "release_advisor_reference", naked)]
    pub unsafe extern "C" fn fc4p_00449310() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 20", "jmp dword ptr [2f]", "2: .long 0x0090d997");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 20", "jmp {}", sym release_advisor_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_tool_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0045e760;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_tool_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0045e760",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0045e760.
    #[port(address = 0x0045fcd0, owner = "shared", name = "query_tool_primary", naked)]
    pub unsafe extern "C" fn fc4p_0045fcd0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0045e760");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_tool_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_terrain_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0092d091;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_terrain_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0092d091",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0092d091.
    #[port(address = 0x00472fd0, owner = "shared", name = "query_terrain_primary", naked)]
    pub unsafe extern "C" fn fc4p_00472fd0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0092d091");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_terrain_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn dispatch_terrain_primary_target(this: *mut u8, arg0: u32) -> u64 = "thiscall" 0x0092e040;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn dispatch_terrain_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0092e040",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0092e040.
    #[port(address = 0x00473000, owner = "shared", name = "dispatch_terrain_primary", naked)]
    pub unsafe extern "C" fn fc4p_00473000() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0092e040");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym dispatch_terrain_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_brush_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0087931b;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_brush_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0087931b",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0087931b.
    #[port(address = 0x00473050, owner = "shared", name = "query_brush_primary", naked)]
    pub unsafe extern "C" fn fc4p_00473050() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0087931b");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_brush_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn dispatch_brush_primary_target(this: *mut u8) -> u64 = "thiscall" 0x00879383;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn dispatch_brush_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00879383",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x00879383.
    #[port(address = 0x00473070, owner = "shared", name = "dispatch_brush_primary", naked)]
    pub unsafe extern "C" fn fc4p_00473070() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00879383");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym dispatch_brush_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_terrain_system_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x00477130;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_terrain_system_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00477130",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x00477130.
    #[port(address = 0x00477210, owner = "shared", name = "query_terrain_system_primary", naked)]
    pub unsafe extern "C" fn fc4p_00477210() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00477130");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_terrain_system_primary_test_target);
    }
}

#[cfg(test)]
extern "C" fn resolve_tail_target(address: u32) -> usize {
    crate::testing::resolve(address)
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00407fa0::PORT,
    fc4p_00408000::PORT,
    fc4p_00408090::PORT,
    fc4p_00408170::PORT,
    fc4p_004089b0::PORT,
    fc4p_00408c20::PORT,
    fc4p_0040add0::PORT,
    fc4p_00410f00::PORT,
    fc4p_0042e440::PORT,
    fc4p_0040d500::PORT,
    fc4p_00411d50::PORT,
    fc4p_00431180::PORT,
    fc4p_00431ac0::PORT,
    fc4p_00437ad0::PORT,
    fc4p_0045ddb0::PORT,
    fc4p_004622a0::PORT,
    fc4p_00465890::PORT,
    fc4p_00466660::PORT,
    fc4p_00466e10::PORT,
    fc4p_00473080::PORT,
    fc4p_0048c980::PORT,
    fc4p_00407a90::PORT,
    fc4p_00407ab0::PORT,
    fc4p_00407ac0::PORT,
    fc4p_00407ba0::PORT,
    fc4p_00407bb0::PORT,
    fc4p_00408720::PORT,
    fc4p_00408730::PORT,
    fc4p_0040ce00::PORT,
    fc4p_0040ce60::PORT,
    fc4p_0040d380::PORT,
    fc4p_0040f370::PORT,
    fc4p_004186e0::PORT,
    fc4p_0041a260::PORT,
    fc4p_0041a270::PORT,
    fc4p_00420130::PORT,
    fc4p_00420d40::PORT,
    fc4p_00423000::PORT,
    fc4p_00423650::PORT,
    fc4p_00423700::PORT,
    fc4p_00423710::PORT,
    fc4p_00423750::PORT,
    fc4p_00423850::PORT,
    fc4p_0042f6d0::PORT,
    fc4p_004302a0::PORT,
    fc4p_004302b0::PORT,
    fc4p_0043d230::PORT,
    fc4p_00449310::PORT,
    fc4p_00459880::PORT,
    fc4p_0045fcd0::PORT,
    fc4p_00466490::PORT,
    fc4p_004695a0::PORT,
    fc4p_00472fd0::PORT,
    fc4p_00473000::PORT,
    fc4p_00473050::PORT,
    fc4p_00473070::PORT,
    fc4p_00477100::PORT,
    fc4p_00477210::PORT,
    fc4p_004777e0::PORT,
    fc4p_00477810::PORT,
    fc4p_00477860::PORT,
    fc4p_00477890::PORT,
    fc4p_004778a0::PORT,
    fc4p_00489660::PORT,
];

#[cfg(test)]
mod value_tests {
    use super::*;
    use crate::abi::{Conv, RawCall};
    use crate::testing::{fake_data, GameFpu};

    pub(super) fn run(entry: unsafe extern "C" fn(), this: u32, args: &[u32]) -> (u32, u32, u32) {
        run_with(entry, this, args, 0xAABB_CCDD, 0x1234_5678)
    }

    fn run_with(entry: unsafe extern "C" fn(), this: u32, args: &[u32], eax: u32, edx: u32) -> (u32, u32, u32) {
        let mut c = RawCall::new(Conv::Thiscall, entry as usize, this, args);
        c.eax = eax;
        c.edx = edx;
        // The fixtures use cdecl for the range helpers and thiscall for object helpers.
        c.caller_pops = u32::from(matches!(entry as usize,
            x if x == fc4p_004622a0 as *const () as usize
                || x == fc4p_00465890 as *const () as usize
                || x == fc4p_0048c980 as *const () as usize
                || x == fc4p_00466660 as *const () as usize
                || x == fc4p_0040d500 as *const () as usize));
        // Capture ECX at the call as well as the ordinary EDX:EAX return channels.
        unsafe {
            core::arch::asm!(
                "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
                "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push [eax + ecx*4]", "jmp 2b",
                "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
                "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
                "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
                "4:", in("edi") &mut c, clobber_abi("C"));
        }
        (c.eax, c.ecx, c.edx)
    }

    #[test]
    fn list_count_handles_empty_single_and_multiple_nodes_without_writes() {
        let mut header = [0u32; 3];
        let mut first = [0u32; 3];
        let mut second = [0u32; 3];
        let h = header.as_mut_ptr() as u32;
        let a = first.as_mut_ptr() as u32;
        let b = second.as_mut_ptr() as u32;
        let mut list = [h];
        for (head, tail, expected) in [(h, h, 0), (a, h, 1), (a, b, 2)] {
            header[0] = head;
            first[0] = tail;
            second[0] = h;
            let before = (header, first, second, list);
            assert_eq!(run(fc4p_00411d50, list.as_mut_ptr() as u32, &[]), (expected, h, h));
            assert_eq!((header, first, second, list), before);
        }
    }

    #[test]
    fn three_word_equality_checks_each_mismatch_and_same_object() {
        let mut left = [0xAABB_CCDDu32, 0x1234_5678, 0x8000_0000];
        for mismatch in 0..=3 {
            let mut right = left;
            if mismatch < 3 {
                right[mismatch] ^= 1;
            }
            let before = (left, right);
            let result = run(fc4p_00431180, left.as_mut_ptr() as u32, &[right.as_mut_ptr() as u32]);
            assert_eq!(result.0, u32::from(mismatch == 3));
            assert_eq!((left, right), before);
        }
        let p = left.as_mut_ptr() as u32;
        assert_eq!(run(fc4p_00431180, p, &[p]), (1, left[2], left[1]));
    }

    #[test]
    fn four_word_copies_are_sequential_even_with_overlap() {
        for member in [false, true] {
            for destination in [0usize, 1, 2, 4, 5] {
                let mut words = [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
                let source = if member { 4 } else { 1 };
                let mut expected = words;
                for i in 0..4 {
                    expected[destination + i] = expected[source + i];
                }
                let base = words.as_mut_ptr() as u32;
                let result = if member {
                    run(fc4p_00431ac0, base, &[base + destination as u32 * 4])
                } else {
                    run(fc4p_00473080, base + destination as u32 * 4, &[base + 4])
                };
                assert_eq!(result.0, base + destination as u32 * 4);
                assert_eq!(words, expected);
            }
        }
    }

    #[test]
    fn halfword_copy_handles_signed_empty_lengths_odd_ends_and_overlap() {
        for delta in [-4i32, -1, 0, 1, 2, 3, 8] {
            let mut values = [1u16, 2, 3, 4, 5, 6, 7, 8];
            let mut expected = values;
            let count = (delta >> 1).max(0) as usize;
            for i in 0..count {
                expected[i + 2] = expected[i + 1];
            }
            let source = unsafe { values.as_mut_ptr().add(1) } as u32;
            let dest = source + 2;
            let result = run(fc4p_004622a0, 0, &[source, source.wrapping_add(delta as u32), dest]);
            assert_eq!(
                result,
                (
                    dest + count as u32 * 2,
                    if count == 0 { (delta >> 1) as u32 } else { 0 },
                    if count == 0 { source } else { source.wrapping_sub(dest) }
                )
            );
            assert_eq!(values, expected);
        }
    }

    #[test]
    fn halfword_fill_rereads_an_overlapping_value_and_zero_does_not_read_it() {
        let mut bytes = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let base = bytes.as_mut_ptr() as u32;
        assert_eq!(run(fc4p_00465890, 0, &[base, 0, 0]), (base, 0, 0x1234_5678));
        let mut expected = bytes;
        for i in 0..3 {
            let value = [expected[1], expected[2]];
            expected[i * 2..i * 2 + 2].copy_from_slice(&value);
        }
        assert_eq!(run(fc4p_00465890, 0, &[base, 3, base + 1]), (base + 6, 0, base + 1));
        assert_eq!(bytes, expected);
    }

    #[test]
    fn word_copy_handles_block_boundaries_negative_counts_and_overlap() {
        for count in -2i32..=9 {
            let mut words = [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let mut expected = words;
            for i in 0..count.max(0) as usize {
                expected[i + 2] = expected[i + 1];
            }
            let source = unsafe { words.as_mut_ptr().add(1) } as u32;
            let destination = source + 4;
            let result = run(
                fc4p_0048c980,
                0,
                &[source, source.wrapping_add((count * 4) as u32), destination],
            );
            assert_eq!(
                result,
                (
                    destination + count.max(0) as u32 * 4,
                    source + count.max(0) as u32 * 4,
                    if count > 0 { 0 } else { 0x1234_5678 }
                )
            );
            assert_eq!(words, expected);
        }
    }

    #[test]
    fn float_comparison_is_ordered_including_nan_infinities_and_signed_zero() {
        let _fpu = GameFpu::new();
        for (left, right, expected) in [
            (2.0f32, 1.0, 1),
            (1.0, 2.0, 0),
            (-0.0, 0.0, 1),
            (f32::INFINITY, 2.0, 1),
            (f32::NEG_INFINITY, f32::NEG_INFINITY, 1),
            (f32::NAN, 1.0, 0),
            (1.0, f32::NAN, 0),
        ] {
            let mut a = left.to_bits();
            let mut b = right.to_bits();
            assert_eq!(
                run(fc4p_00437ad0, &mut a as *mut u32 as u32, &[&mut b as *mut u32 as u32]).0,
                expected
            );
        }
    }

    #[test]
    fn maximum_selects_first_equal_value_and_keeps_nan_selection() {
        let _fpu = GameFpu::new();
        for (mut values, expected) in [
            (vec![], 0),
            (vec![5.0f32], 0),
            (vec![1.0, 5.0, 5.0, 2.0], 1),
            (vec![f32::NAN, 100.0, f32::INFINITY], 0),
            (vec![1.0, f32::NAN, 2.0], 2),
            (vec![-0.0, 0.0], 0),
        ] {
            let begin = values.as_mut_ptr() as u32;
            let end = begin + values.len() as u32 * 4;
            let before: Vec<u32> = values.iter().map(|v| v.to_bits()).collect();
            assert_eq!(
                run(fc4p_00466660, 0, &[begin, end]),
                (
                    begin + expected * 4,
                    end,
                    if values.is_empty() {
                        0x1234_5678
                    } else {
                        begin + expected * 4
                    }
                )
            );
            assert_eq!(values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), before);
        }
    }

    #[test]
    fn tree_predecessor_handles_header_left_subtree_and_parent_climb() {
        let mut nodes = [[1u32; 4]; 6];
        let base = nodes.as_mut_ptr() as u32;
        let p = |i: usize| base + i as u32 * 16;
        // Header, root, left child, left child's right child, parent and grandparent.
        nodes[0] = [0, p(1), p(2), p(1)];
        nodes[1] = [1, p(0), p(2), 0];
        nodes[2] = [1, p(1), 0, p(3)];
        nodes[3] = [1, p(2), 0, 0];
        nodes[4] = [1, p(5), p(1), 0];
        nodes[5] = [1, p(0), p(4), 0];
        assert_eq!(run(fc4p_0040d500, 0, &[p(0)]).0, p(1));
        assert_eq!(run(fc4p_0040d500, 0, &[p(1)]).0, p(3));
        assert_eq!(run(fc4p_0040d500, 0, &[p(3)]).0, p(2));
        nodes[1][1] = p(4);
        nodes[1][2] = 0;
        assert_eq!(run(fc4p_0040d500, 0, &[p(1)]).0, p(0));
        assert_eq!(nodes[1][2], 0);
    }

    #[test]
    fn unit_transform_writes_six_words_and_returns_loaded_scalar_in_edx() {
        let mut scalar = 0x7FC0_1234u32;
        let _global = fake_data(0x00b215e8, (&mut scalar as *mut u32).cast());
        let mut words = [0xAABB_CCDDu32; 8];
        let p = unsafe { words.as_mut_ptr().add(1) } as u32;
        assert_eq!(run(fc4p_00466e10, p, &[]), (p, 0, scalar));
        assert_eq!(
            words,
            [0xAABB_CCDD, 0x3F80_0000, 0x3F80_0000, 0x3F80_0000, 0, 0, 0, 0xAABB_CCDD]
        );
    }

    #[test]
    fn byte_grid_constructor_preserves_every_uninitialized_byte() {
        let mut bytes = [0xA5u8; 0x48];
        let mut expected = bytes;
        for (offset, value) in [
            (0, 0x00A8_78C0u32),
            (4, 0x00A8_78A4),
            (12, 0),
            (16, 0x49B9_E602),
            (24, 0),
            (56, 0),
            (60, 0),
            (64, 0),
        ] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        expected[8] = 0;
        let p = bytes.as_mut_ptr() as u32;
        assert_eq!(run(fc4p_0045ddb0, p, &[0x9876_5432]), (p, 0, 0x1234_5678));
        assert_eq!(bytes, expected);
    }
}

#[cfg(test)]
mod call_tests {
    use super::value_tests::run;
    use super::*;
    use crate::testing::fake;
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Allocate(u32, [u32; 3]),
        Copy(u32, u32, u32),
        Error(u32),
        Range(u32, u32, u32, u32, [u32; 3]),
        Version(u32, u32),
        Word(u32, u32),
        String(u32, u32),
        ErrorByte(u32),
    }

    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        object: u32,
        allocation: u32,
        copy_result: Option<u32>,
        error_repairs_object: bool,
        stream_error: u32,
        mutation: Option<(u32, u32)>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    fn reset(object: u32, allocation: u32) {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                object,
                allocation,
                ..State::default()
            }
        });
    }
    fn calls() -> Vec<Call> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }

    extern "C" fn allocate(count: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let fields = unsafe { [word(s.object, 0), word(s.object, 4), word(s.object, 8)] };
            s.calls.push(Call::Allocate(count, fields));
            (0x1234_5678u64 << 32) | u64::from(s.allocation)
        })
    }
    extern "C" fn copy(dest: u32, source: u32, count: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Copy(dest, source, count));
            let result = if let Some(result) = s.copy_result {
                result
            } else {
                unsafe {
                    core::ptr::copy(source as *const u8, dest as *mut u8, count as usize);
                }
                dest
            };
            (0x8765_4321u64 << 32) | u64::from(result)
        })
    }
    extern "C" fn error(message: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Error(message));
            if s.error_repairs_object {
                unsafe {
                    put_word(s.object, 4, s.allocation);
                }
            }
        });
        0xFEED_FACE_DEAD_BEEF
    }

    extern "C" fn record_range(this: u32, first: u32, last: u32, marker: u32) {
        let fields = unsafe { [word(this, 0), word(this, 4), word(this, 8)] };
        let marker_word = unsafe { read_unaligned(marker as *const u32) };
        STATE.with(|s| {
            s.borrow_mut()
                .calls
                .push(Call::Range(this, first, last, marker_word, fields))
        });
    }
    #[unsafe(naked)]
    unsafe extern "C" fn range() {
        core::arch::naked_asm!(
            "push [esp + 12]", "push [esp + 12]", "push [esp + 12]", "push ecx",
            "call {}", "add esp, 16", "mov eax, 0x12345678", "mov ecx, 0xABCDEF01", "mov edx, 0x87654321", "ret 12",
            sym record_range);
    }

    #[test]
    fn range_string_allocates_copies_terminates_and_returns_end_pointer() {
        let _allocate = fake(0x0090cf54, allocate as *const () as usize);
        let _copy = fake(0x009eeb70, copy as *const () as usize);
        for bytes in [b"".as_slice(), b"abc".as_slice(), b"a\0b".as_slice()] {
            let mut object = [0xA5A5_A5A5u32; 4];
            let mut buffer = [0xCDu8; 16];
            let this = object.as_mut_ptr() as u32;
            let dest = buffer.as_mut_ptr() as u32;
            let first = bytes.as_ptr() as u32;
            reset(this, dest);
            let result = run(fc4p_00407fa0, this, &[first, first + bytes.len() as u32, 0xDEAD_BEEF]);
            assert_eq!(result.0, dest + bytes.len() as u32);
            assert_eq!(
                object,
                [
                    dest,
                    dest + bytes.len() as u32,
                    dest + bytes.len() as u32 + 1,
                    0xA5A5_A5A5
                ]
            );
            assert_eq!(&buffer[..bytes.len()], bytes);
            assert_eq!(buffer[bytes.len()], 0);
            assert!(buffer[bytes.len() + 1..].iter().all(|b| *b == 0xCD));
            let mut expected = vec![Call::Allocate(bytes.len() as u32 + 1, [0xA5A5_A5A5; 3])];
            if !bytes.is_empty() {
                expected.push(Call::Copy(dest, first, bytes.len() as u32));
            }
            assert_eq!(calls(), expected);
        }
    }

    #[test]
    fn range_string_uses_copy_return_and_handles_wrapped_capacity_error() {
        let _allocate = fake(0x0090cf54, allocate as *const () as usize);
        let _copy = fake(0x009eeb70, copy as *const () as usize);
        let _error = fake(0x009dd1d0, error as *const () as usize);
        let mut buffer = [0xCDu8; 16];
        let dest = buffer.as_mut_ptr() as u32;
        let mut object = [dest, 99, 123, 456];
        let this = object.as_mut_ptr() as u32;
        reset(this, dest);
        STATE.with(|s| s.borrow_mut().copy_result = Some(dest + 1));
        assert_eq!(run(fc4p_00407fa0, this, &[0x1000, 0x0FFF, 0]).0, dest);
        assert_eq!(object, [dest, dest, 123, 456]);
        assert_eq!(buffer[0], 0);
        assert_eq!(
            calls(),
            [Call::Error(STRING_ERROR_TEXT), Call::Copy(dest, 0x1000, u32::MAX)]
        );
        reset(this, dest);
        STATE.with(|s| s.borrow_mut().copy_result = Some(dest + 4));
        assert_eq!(run(fc4p_00407fa0, this, &[0x1000, 0x1002, 0]).0, dest + 6);
        assert_eq!(object[1], dest + 6);
        assert_eq!(buffer[6], 0);
    }

    #[test]
    fn c_string_and_count_constructors_clear_fields_before_forwarding() {
        let _range = fake(0x00407fa0, range as *const () as usize);
        for bytes in [b"\0".as_slice(), b"abc\0tail".as_slice()] {
            let mut object = [u32::MAX; 4];
            let this = object.as_mut_ptr() as u32;
            let first = bytes.as_ptr() as u32;
            reset(this, 0);
            assert_eq!(
                run(fc4p_00408000, this, &[first, 0xA5]),
                (this, 0xABCD_EF01, 0x8765_4321)
            );
            let len = bytes.iter().position(|b| *b == 0).unwrap() as u32;
            assert_eq!(calls(), [Call::Range(this, first, first + len, first, [0; 3])]);
            assert_eq!(object, [0, 0, 0, u32::MAX]);
        }
        let mut object = [u32::MAX; 4];
        let this = object.as_mut_ptr() as u32;
        reset(this, 0);
        assert_eq!(
            run(fc4p_0040add0, this, &[0xFFFF_FFF0, 32, 17]),
            (this, 0xABCD_EF01, 0x8765_4321)
        );
        assert_eq!(calls(), [Call::Range(this, 0xFFFF_FFF0, 16, 0xFFFF_FFF0, [0; 3])]);
    }

    #[test]
    fn capacity_string_reserves_one_extra_byte_and_starts_empty() {
        let _allocate = fake(0x0090cf54, allocate as *const () as usize);
        let _error = fake(0x009dd1d0, error as *const () as usize);
        let mut object = [u32::MAX; 4];
        let mut buffer = [0xCDu8; 8];
        let this = object.as_mut_ptr() as u32;
        let dest = buffer.as_mut_ptr() as u32;
        for count in [0u32, 7, 0xFFFF_FFFE] {
            reset(this, dest);
            assert_eq!(run(fc4p_00408170, this, &[99, count, 55]).0, this);
            assert_eq!(object, [dest, dest, dest.wrapping_add(count).wrapping_add(1), u32::MAX]);
            assert_eq!(calls(), [Call::Allocate(count + 1, [0; 3])]);
            assert_eq!(buffer, [0, 0xCD, 0xCD, 0xCD, 0xCD, 0xCD, 0xCD, 0xCD]);
        }
        reset(this, dest);
        STATE.with(|s| s.borrow_mut().error_repairs_object = true);
        assert_eq!(run(fc4p_00408170, this, &[0, u32::MAX, 0]).0, this);
        assert_eq!(calls(), [Call::Error(STRING_ERROR_TEXT)]);
        assert_eq!(object, [0, dest, 0, u32::MAX]);
    }

    #[test]
    fn substring_clamps_count_accepts_end_and_reports_out_of_range() {
        let _range = fake(0x00408420, range as *const () as usize);
        let _error = fake(0x009dd0a0, error as *const () as usize);
        let mut source = [0x1000u32, 0x1005, 0x1010];
        for (position, count, end) in [(0, 2, 0x1002), (2, 99, 0x1005), (5, u32::MAX, 0x1005)] {
            let mut object = [u32::MAX; 4];
            let this = object.as_mut_ptr() as u32;
            reset(this, 0);
            assert_eq!(
                run(fc4p_004089b0, this, &[source.as_mut_ptr() as u32, position, count, 0]),
                (this, 0xABCD_EF01, 0x8765_4321)
            );
            assert_eq!(calls(), [Call::Range(this, 0x1000 + position, end, count, [0; 3])]);
            assert_eq!(object, [0, 0, 0, u32::MAX]);
        }
        let mut object = [u32::MAX; 3];
        let this = object.as_mut_ptr() as u32;
        reset(this, 0);
        assert_eq!(run(fc4p_004089b0, this, &[source.as_mut_ptr() as u32, 6, 1, 0]).0, this);
        assert_eq!(calls(), [Call::Error(STRING_ERROR_TEXT)]);
        assert_eq!(object, [0; 3]);
    }

    #[test]
    fn assignment_and_append_forward_range_marker_and_full_results() {
        let _assign = fake(0x004081d0, range as *const () as usize);
        let _append = fake(0x0040ebc0, range as *const () as usize);
        let mut object = [0x1234u32, 0x5678, 0x9ABC];
        let mut source = [0x1000u32, 0x1007, 0x1010];
        let this = object.as_mut_ptr() as u32;
        let other = source.as_mut_ptr() as u32;
        reset(this, 0);
        assert_eq!(
            run(fc4p_00408c20, this, &[other]),
            (0x1234_5678, 0xABCD_EF01, 0x8765_4321)
        );
        assert_eq!(calls(), [Call::Range(this, 0x1000, 0x1007, other, object)]);
        for bytes in [b"\0".as_slice(), b"abc\0".as_slice()] {
            let first = bytes.as_ptr() as u32;
            assert_eq!(
                run(fc4p_00410f00, this, &[first]),
                (0x1234_5678, 0xABCD_EF01, 0x8765_4321)
            );
            assert_eq!(
                calls(),
                [Call::Range(this, first, first + bytes.len() as u32 - 1, first, object)]
            );
        }
    }

    extern "thiscall" fn put_version(this: u32, value: u32) -> u32 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Version(this, value)));
        0
    }
    extern "thiscall" fn put_stream_word(this: u32, value: u32) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Word(this, value));
            if let Some((p, value)) = s.mutation.take() {
                unsafe {
                    put_word(p, 0, value);
                }
            }
        });
        0 // Transfer failures do not stop serialization.
    }
    extern "C" fn put_string(stream: u32, value: u32) -> u32 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::String(stream, value)));
        0
    }
    extern "thiscall" fn error_byte(this: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::ErrorByte(this));
            (0x8765_4321u64 << 32) | u64::from(s.stream_error)
        })
    }

    fn stream_table() -> [usize; 19] {
        let mut table = [0usize; 19];
        table[0x1C / 4] = put_version as *const () as usize;
        table[0x20 / 4] = put_stream_word as *const () as usize;
        table[0x48 / 4] = error_byte as *const () as usize;
        table
    }

    #[test]
    fn version_writer_uses_only_final_error_low_byte_and_retains_edx() {
        let table = stream_table();
        let mut stream = [table.as_ptr() as u32];
        let p = stream.as_mut_ptr() as u32;
        for error in [0u32, 0xAABB_CC00, 1, 0x1234_5680] {
            reset(0, 0);
            STATE.with(|s| s.borrow_mut().stream_error = error);
            let result = run(fc4p_0042e440, 0, &[p]);
            assert_eq!((result.0, result.2), (u32::from(error as u8 == 0), 0x8765_4321));
            assert_eq!(calls(), [Call::Version(p, 1), Call::ErrorByte(p)]);
        }
    }

    #[test]
    fn advice_writer_orders_fields_list_payloads_string_and_final_error() {
        let _string = fake(0x005d99a0, put_string as *const () as usize);
        let table = stream_table();
        let mut stream = [table.as_ptr() as u32];
        let p = stream.as_mut_ptr() as u32;
        let mut object = [0xA5A5_A5A5u32; 0xC0 / 4];
        let this = object.as_mut_ptr() as u32;
        let offsets = [0x7C, 0x80, 0x64, 0x68, 0x6C, 0x70, 0x74, 0x78];
        for (i, offset) in offsets.into_iter().enumerate() {
            object[offset / 4] = i as u32 + 10;
        }
        object[0xA0 / 4] = 0x1122_3344;
        let mut header = [0u32; 3];
        let mut node1 = [0u32, 0, 0x1234];
        let mut node2 = [0u32, 0, 0x5678];
        let h = header.as_mut_ptr() as u32;
        node1[0] = node2.as_mut_ptr() as u32;
        node2[0] = h;
        object[0xB0 / 4] = h;
        for empty in [true, false] {
            header[0] = if empty { h } else { node1.as_mut_ptr() as u32 };
            reset(this, 0);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.stream_error = 0xFFFF_FF01;
                s.mutation = Some((this + 0x80, 99));
            });
            let result = run(fc4p_00408090, this, &[p]);
            assert_eq!((result.0, result.2), (0, 0x8765_4321));
            let mut expected = vec![Call::Version(p, 4)];
            for i in 0..8 {
                expected.push(Call::Word(p, if i == 1 { 99 } else { i + 10 }));
            }
            expected.push(Call::Word(p, if empty { 0 } else { 2 }));
            if !empty {
                expected.extend([Call::Word(p, 0x1234), Call::Word(p, 0x5678)]);
            }
            expected.extend([Call::String(p, 0x1122_3344), Call::ErrorByte(p)]);
            assert_eq!(calls(), expected);
            assert_eq!(object[0xB0 / 4], h);
            assert_eq!(node2[0], h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{call_thiscall_with_eax, fake, GameFpu};

    #[test]
    fn constants_return_full_words_and_preserve_edx() {
        let entries: &[(unsafe extern "C" fn(), u32)] = &[
            (fc4p_00407a90, 0x8a09f5f4),
            (fc4p_0040ce00, 0x6a26d049),
            (fc4p_0040d380, 0xeac937b4),
            (fc4p_00420d40, 0x6a3848db),
            (fc4p_00423000, 0x8be3753b),
            (fc4p_0042f6d0, 0x6b70efad),
            (fc4p_00466490, 0x0990c38a),
            (fc4p_004695a0, 0x4990bd93),
            (fc4p_00477100, 0x8a22bebe),
        ];
        for &(entry, expected) in entries {
            let mut call = RawCall::new(Conv::Thiscall, entry as usize, 0, &[]);
            call.eax = 0xA5A5_5A5A;
            call.edx = 0x1234_5678;
            unsafe { call_raw(&mut call) };
            assert_eq!((call.eax, call.edx), (expected, 0x1234_5678));
        }
    }

    #[test]
    fn getters_read_full_words_without_writes() {
        let mut bytes = [0xA5u8; 0x200];
        let entries: &[(unsafe extern "C" fn(), usize)] = &[
            (fc4p_00407ab0, 0x78),
            (fc4p_00407ac0, 0x74),
            (fc4p_00407ba0, 0x7c),
            (fc4p_0040ce60, 0x10),
            (fc4p_00423650, 0xe0),
            (fc4p_00423700, 0x110),
            (fc4p_00423710, 0x12c),
            (fc4p_00423750, 0x14c),
            (fc4p_00423850, 0x198),
            (fc4p_0043d230, 0x150),
            (fc4p_00459880, 0x1c0),
            (fc4p_004777e0, 0x8),
        ];
        for &(entry, offset) in entries {
            for word in [0u32, 1, u32::MAX, 0x8040_FF81] {
                bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                let before = bytes;
                let result = unsafe { call_thiscall_with_eax(entry as usize, bytes.as_mut_ptr(), &[], 0x5A5A_A5A5) };
                assert_eq!(result, (word, 0));
                assert_eq!(bytes, before);
            }
        }
    }

    #[test]
    fn member_addresses_accept_null_and_wrapping_interfaces() {
        let entries: &[(unsafe extern "C" fn(), u32)] = &[
            (fc4p_00407bb0, 0x50),
            (fc4p_00477810, 0x24),
            (fc4p_00477890, 0x90),
            (fc4p_004778a0, 0x9c),
        ];
        for &(entry, offset) in entries {
            for base in [0u32, 0x1234_5678, u32::MAX - 16] {
                let result = unsafe { call_thiscall_with_eax(entry as usize, base as *mut u8, &[], 0) };
                assert_eq!(result, (base.wrapping_add(offset), 0));
            }
        }
    }

    #[test]
    fn float_getter_keeps_integer_registers_and_existing_x87_value() {
        let _fpu = GameFpu::new();
        let mut object = [0u32; 16];
        for bits in [
            0,
            0x8000_0000,
            0x3FC0_0000,
            0x7F80_0000,
            0xFF80_0000,
            0x7FC0_1234,
            0x7F80_1234,
        ] {
            object[0x38 / 4] = bits;
            let mut call = RawCall::new(
                Conv::Thiscall,
                fc4p_00477860 as *const () as usize,
                object.as_mut_ptr() as u32,
                &[],
            );
            call.eax = 0xABCD_1234;
            call.edx = 0x9876_5432;
            call.take_st0 = 1;
            let sentinel = 123.25f64;
            let mut restored = 0f64;
            unsafe {
                core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
            }
            unsafe { call_raw(&mut call) };
            unsafe {
                core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
            }
            assert_eq!(restored, sentinel);
            assert_eq!((call.eax, call.edx), (0xABCD_1234, 0x9876_5432));
            assert_eq!(
                call.st0.to_f32().to_bits(),
                if bits & 0x7FFF_FFFF > 0x7F80_0000 {
                    bits | 0x0040_0000
                } else {
                    bits
                }
            );
        }
    }

    #[test]
    fn vtable_restore_writes_only_the_first_word_and_keeps_eax() {
        let mut object = [0xDEAD_BEEFu32; 4];
        let result = unsafe {
            call_thiscall_with_eax(
                fc4p_00489660 as *const () as usize,
                object.as_mut_ptr().cast(),
                &[],
                0x1234_ABCD,
            )
        };
        assert_eq!(result, (0x1234_ABCD, 0));
        assert_eq!(object, [0x00A8_EC1C, 0xDEAD_BEEF, 0xDEAD_BEEF, 0xDEAD_BEEF]);
    }

    thread_local! {
        static FORWARDED: std::cell::RefCell<Vec<(usize, Vec<u32>)>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    extern "thiscall" fn target0(this: *mut u8) -> u64 {
        FORWARDED.with(|v| v.borrow_mut().push((this as usize, vec![])));
        0xABCD_1234_5678_90EF
    }
    extern "thiscall" fn target1(this: *mut u8, a: u32) -> u64 {
        FORWARDED.with(|v| v.borrow_mut().push((this as usize, vec![a])));
        0xABCD_1234_5678_90EF
    }
    extern "thiscall" fn target2(this: *mut u8, a: u32, b: u32) -> u64 {
        FORWARDED.with(|v| v.borrow_mut().push((this as usize, vec![a, b])));
        0xABCD_1234_5678_90EF
    }

    #[test]
    fn interface_thunks_forward_arguments_adjust_this_and_return_both_words() {
        let entries: &[(unsafe extern "C" fn(), u32, i32, usize)] = &[
            (fc4p_00408720, 0x00407bc0, -4, 2),
            (fc4p_00408730, 0x0043ec10, -4, 1),
            (fc4p_0040f370, 0x0040d430, -8, 2),
            (fc4p_004186e0, 0x00968b60, 20, 0),
            (fc4p_0041a260, 0x004186e0, -16, 0),
            (fc4p_0041a270, 0x00449310, -16, 0),
            (fc4p_004302a0, 0x00968b60, 8, 0),
            (fc4p_004302b0, 0x0090d997, 8, 0),
            (fc4p_00449310, 0x0090d997, 20, 0),
            (fc4p_0045fcd0, 0x0045e760, -4, 2),
            (fc4p_00472fd0, 0x0092d091, -4, 2),
            (fc4p_00473000, 0x0092e040, -4, 1),
            (fc4p_00473050, 0x0087931b, -4, 2),
            (fc4p_00473070, 0x00879383, -4, 0),
            (fc4p_00477210, 0x00477130, -4, 2),
        ];
        for &(entry, target, offset, count) in entries {
            let f = [
                target0 as *const () as usize,
                target1 as *const () as usize,
                target2 as *const () as usize,
            ][count];
            let _fake = fake(target, f);
            let args = [0x1122_3344, 0xAABB_CCDD];
            for base in [0u32, 0x1234_5678, u32::MAX - 4] {
                FORWARDED.with(|v| v.borrow_mut().clear());
                let result = unsafe { call_thiscall_with_eax(entry as usize, base as *mut u8, &args[..count], 0) };
                assert_eq!(result, (0x5678_90EF, 0xABCD_1234));
                FORWARDED.with(|v| {
                    assert_eq!(
                        *v.borrow(),
                        [(base.wrapping_add(offset as u32) as usize, args[..count].to_vec())]
                    )
                });
            }
        }
    }

    #[test]
    fn virtual_forwarding_keeps_all_stack_arguments() {
        let mut table = [0usize; 10];
        table[9] = target2 as *const () as usize;
        let mut object = [table.as_ptr() as usize, 0x1234_5678];
        FORWARDED.with(|v| v.borrow_mut().clear());
        let result = unsafe {
            call_thiscall_with_eax(
                fc4p_00420130 as *const () as usize,
                object.as_mut_ptr().cast(),
                &[17, 99],
                0,
            )
        };
        assert_eq!(result, (0x5678_90EF, 0xABCD_1234));
        FORWARDED.with(|v| assert_eq!(*v.borrow(), [(object.as_ptr() as usize, vec![17, 99])]));
        assert_eq!(object[1], 0x1234_5678);
    }
}
