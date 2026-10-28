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
            let b = F80::from_f32(read_unaligned(r.eax as *const f32));
            let compared = a.compare(b);
            r.eax = u32::from(compared.status_word() & 0x4100 == 0);
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

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_0049caf0::PORT,
    fc4p_0049d9d0::PORT,
    fc4p_004abad0::PORT,
    fc4p_004ae420::PORT,
    fc4p_004b8860::PORT,
    fc4p_004b9e20::PORT,
    fc4p_004c3730::PORT,
    fc4p_004e13e0::PORT,
    fc4p_004e76a0::PORT,
    fc4p_004e82a0::PORT,
    fc4p_004ed910::PORT,
    fc4p_004f0240::PORT,
    fc4p_004f7ba0::PORT,
    fc4p_004fb340::PORT,
    fc4p_004fbbe0::PORT,
    fc4p_004fc8e0::PORT,
    fc4p_004fc900::PORT,
    fc4p_00504c40::PORT,
    fc4p_00504fa0::PORT,
    fc4p_00504fe0::PORT,
    fc4p_00504ff0::PORT,
    fc4p_00505000::PORT,
    fc4p_00505110::PORT,
    fc4p_00505140::PORT,
    fc4p_00505c70::PORT,
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
    }

    fn take_calls() -> Vec<Call> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }

    macro_rules! stub {
        ($name:ident, $pop:literal, $count:literal, $id:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
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
                    CALLS.with(|c| c.borrow_mut().push(Call { id: $id, registers: *r, args }));
                    *r = RESULTS.with(|v| v.borrow_mut().pop_front()).unwrap_or(RESULT);
                }
            }
        };
    }
    stub!(allocate_stub, 0, 1, 1);
    stub!(method_zero, 0, 0, 2);
    stub!(method_one, 4, 1, 3);

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
}
