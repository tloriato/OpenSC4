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

//! Shared helpers, group 10: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Instruction emitter with a 40-byte scratch record at +8 and a record queue at +0x30.
    struct Emitter {
        0x04 => output / set_output: *mut u8,
        0x08 => length / set_length: u32,
        0x0c => serial / set_serial: u32,
        0x20 => kind / set_kind: u32,
        0x24 => reads / set_reads: u32,
        0x28 => writes / set_writes: u32,
        0x2c => cost / set_cost: u32,
        0x40 => end / set_end: *mut u8,
        0x48 => limit / set_limit: *mut u8,
    }
}

game_fn! {
    /// Append an encoded memory operand. Register-sensitive calls retain all three integer results.
    #[allow(dead_code)]
    fn encode_memory(this: *mut u8, opcode: u32, register: u32, operand: *mut u8) -> u64 = "thiscall" 0x008e_c190;
}
game_fn! {
    /// Append a record at the boundary of a queue block.
    #[allow(dead_code)]
    fn append_record(this: *mut u8, record: *mut u8) -> u64 = "thiscall" 0x008e_cc20;
}
game_fn! {
    /// Flush queued instruction records to the output stream.
    #[allow(dead_code)]
    fn flush_records(this: *mut u8) -> u64 = "thiscall" 0x008e_cca0;
}

/// Integer results saved by the adapter; the original return address and argument words follow them.
#[repr(C)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

impl Registers {
    unsafe fn argument(&self, index: usize) -> u32 {
        unsafe { read_unaligned((self as *const Self).cast::<u32>().add(4 + index)) }
    }
}

// Only the calling-convention adapter is assembly. All encoding and queue logic lives in Rust.
macro_rules! register_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $pop:literal) => {
        port! {
            /// Emit the instruction and retain the original integer return registers.
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
                    "pop eax", "pop ecx", "pop edx", "ret {pop}",
                    body = sym $body, pop = const $pop,
                );
            }
        }
    };
}

/// Call an original game entry and retain ECX along with EDX:EAX.
unsafe fn invoke(r: &mut Registers, target: usize, args: &[u32]) {
    let mut c = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, target, r.ecx, args);
    c.eax = r.eax;
    c.edx = r.edx;
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            in("edi") &mut c, clobber_abi("C"),
        );
    }
    r.eax = c.eax;
    r.ecx = c.ecx;
    r.edx = c.edx;
}

fn register_mask(register: u32) -> u32 {
    0x100u32.wrapping_shl(register)
}

unsafe fn byte(emitter: Emitter, value: u8) {
    unsafe {
        emitter
            .as_ptr()
            .wrapping_add(0x10 + emitter.length() as usize)
            .write(value);
    }
    emitter.set_length(emitter.length().wrapping_add(1));
}

fn begin(emitter: Emitter) {
    emitter.set_length(0);
    emitter.set_serial(emitter.serial().wrapping_add(1));
    unsafe { byte(emitter, 0x0f) };
}

fn metadata(emitter: Emitter, cost: u32, kind: u32, reads: u32, writes: u32) {
    emitter.set_cost(cost);
    emitter.set_kind(kind);
    emitter.set_reads(reads);
    emitter.set_writes(writes);
}

/// Copy forwards one word at a time, preserving all unused bytes and any overlapping-copy effects.
unsafe fn enqueue(emitter: Emitter, r: &mut Registers, edx: u32) {
    unsafe {
        let queue = emitter.as_ptr().add(0x30);
        let end = emitter.end();
        r.eax = queue as u32;
        r.edx = edx;
        r.ecx = (emitter.limit() as u32).wrapping_sub(40);
        if end as u32 == r.ecx {
            r.ecx = queue as u32;
            invoke(r, crate::abi::target(0x008e_cc20), &[emitter.as_ptr().add(8) as u32]);
        } else {
            if !end.is_null() {
                for i in 0..10 {
                    let value = read_unaligned(emitter.as_ptr().add(8 + i * 4).cast::<u32>());
                    write_unaligned(end.add(i * 4).cast::<u32>(), value);
                }
            }
            let next = emitter.end().wrapping_add(40);
            emitter.set_end(next);
            r.ecx = next as u32;
        }
    }
}

/// Register-register encoding uses byte arithmetic; masks use the low five bits of the full words.
unsafe fn register_pair(r: &mut Registers, opcode: u8, separate_masks: bool) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        let destination = r.argument(0);
        let source = r.argument(1);
        begin(emitter);
        byte(emitter, opcode);
        byte(
            emitter,
            (destination as u8)
                .wrapping_sub(8)
                .wrapping_mul(8)
                .wrapping_add(source as u8),
        );
        let destination_mask = register_mask(destination);
        let source_mask = register_mask(source);
        metadata(
            emitter,
            1,
            0,
            if separate_masks {
                source_mask
            } else {
                source_mask | destination_mask
            },
            destination_mask,
        );
        enqueue(emitter, r, if separate_masks { destination } else { destination_mask });
    }
}

macro_rules! pair_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $opcode:literal, $separate:literal) => {
        register_port!($address, $export, $body, $name, 8);
        unsafe extern "thiscall" fn $body(r: &mut Registers) {
            unsafe { register_pair(r, $opcode, $separate) };
        }
    };
}

unsafe fn memory_pair(r: &mut Registers, opcode: u32) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        let destination = r.argument(0);
        let operand = r.argument(1) as *mut u32;
        begin(emitter);
        r.eax = 0;
        r.edx = 1;
        invoke(
            r,
            crate::abi::target(0x008e_c190),
            &[opcode, u32::from((destination as u8).wrapping_mul(8)), operand as u32],
        );
        let reads = read_unaligned(operand) | register_mask(destination);
        metadata(emitter, 2, 1, reads, register_mask(destination));
        enqueue(emitter, r, reads);
    }
}

macro_rules! memory_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $opcode:literal) => {
        register_port!($address, $export, $body, $name, 8);
        unsafe extern "thiscall" fn $body(r: &mut Registers) {
            unsafe { memory_pair(r, $opcode) };
        }
    };
}

unsafe fn immediate_shift(r: &mut Registers, opcode: u8, extension: u8) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        let destination = r.argument(0);
        let immediate = r.argument(1);
        begin(emitter);
        byte(emitter, opcode);
        byte(emitter, (destination as u8).wrapping_sub(extension));
        byte(emitter, immediate as u8);
        let mask = register_mask(destination);
        metadata(emitter, 1, 0, mask, mask);
        enqueue(emitter, r, mask);
    }
}

macro_rules! shift_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $opcode:literal, $extension:literal) => {
        register_port!($address, $export, $body, $name, 8);
        unsafe extern "thiscall" fn $body(r: &mut Registers) {
            unsafe { immediate_shift(r, $opcode, $extension) };
        }
    };
}

pair_port!(
    0x008e_ebd0,
    fc4p_008eebd0,
    emit_add_dwords,
    "EmitAddPackedDwords",
    0xfe,
    false
);
memory_port!(
    0x008e_ec80,
    fc4p_008eec80,
    emit_add_dwords_memory,
    "EmitAddPackedDwordsMemory",
    0xfe
);
memory_port!(
    0x008e_edd0,
    fc4p_008eedd0,
    emit_subtract_signed_saturated_words_memory,
    "EmitSubtractSignedSaturatedWordsMemory",
    0xe9
);
memory_port!(
    0x008e_ee70,
    fc4p_008eee70,
    emit_subtract_unsigned_saturated_words_memory,
    "EmitSubtractUnsignedSaturatedWordsMemory",
    0xd9
);
pair_port!(
    0x008e_ef10,
    fc4p_008eef10,
    emit_subtract_dwords,
    "EmitSubtractPackedDwords",
    0xfa,
    false
);
shift_port!(
    0x008e_efc0,
    fc4p_008eefc0,
    emit_shift_words_left,
    "EmitShiftWordsLeftImmediate",
    0x71,
    0x10
);
shift_port!(
    0x008e_f070,
    fc4p_008ef070,
    emit_shift_dwords_left,
    "EmitShiftDwordsLeftImmediate",
    0x72,
    0x10
);
shift_port!(
    0x008e_f120,
    fc4p_008ef120,
    emit_shift_words_right,
    "EmitShiftWordsRightImmediate",
    0x71,
    0x30
);
shift_port!(
    0x008e_f1d0,
    fc4p_008ef1d0,
    emit_shift_dwords_right,
    "EmitShiftDwordsRightImmediate",
    0x72,
    0x30
);
shift_port!(
    0x008e_f280,
    fc4p_008ef280,
    emit_shift_qword_right,
    "EmitShiftQwordRightImmediate",
    0x73,
    0x30
);
shift_port!(
    0x008e_f3e0,
    fc4p_008ef3e0,
    emit_shift_signed_dwords_right,
    "EmitShiftSignedDwordsRightImmediate",
    0x72,
    0x20
);
pair_port!(
    0x008e_f490,
    fc4p_008ef490,
    emit_multiply_words_low,
    "EmitMultiplyWordsLow",
    0xd5,
    false
);
pair_port!(
    0x008e_f540,
    fc4p_008ef540,
    emit_multiply_signed_words_high,
    "EmitMultiplySignedWordsHigh",
    0xe5,
    false
);
memory_port!(
    0x008e_f5f0,
    fc4p_008ef5f0,
    emit_multiply_signed_words_high_memory,
    "EmitMultiplySignedWordsHighMemory",
    0xe5
);
pair_port!(
    0x008e_f690,
    fc4p_008ef690,
    emit_multiply_add_words,
    "EmitMultiplyAddWords",
    0xf5,
    false
);
pair_port!(
    0x008e_f740,
    fc4p_008ef740,
    emit_pack_signed_dwords,
    "EmitPackSignedDwords",
    0x6b,
    false
);
pair_port!(
    0x008e_f7f0,
    fc4p_008ef7f0,
    emit_pack_unsigned_bytes,
    "EmitPackUnsignedBytes",
    0x67,
    false
);
pair_port!(
    0x008e_f8a0,
    fc4p_008ef8a0,
    emit_unpack_low_bytes,
    "EmitUnpackLowBytes",
    0x60,
    false
);
pair_port!(
    0x008e_fc10,
    fc4p_008efc10,
    emit_compare_equal_bytes,
    "EmitCompareEqualBytes",
    0x74,
    false
);
pair_port!(
    0x008e_fcc0,
    fc4p_008efcc0,
    emit_compare_equal_dwords,
    "EmitCompareEqualDwords",
    0x76,
    true
);

register_port!(0x008e_fd70, fc4p_008efd70, emit_shuffle_words, "EmitShuffleWords", 12);
unsafe extern "thiscall" fn emit_shuffle_words(r: &mut Registers) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        let destination = r.argument(0);
        let source = r.argument(1);
        let immediate = r.argument(2);
        begin(emitter);
        byte(emitter, 0x70);
        byte(
            emitter,
            (destination as u8)
                .wrapping_sub(8)
                .wrapping_mul(8)
                .wrapping_add(source as u8),
        );
        byte(emitter, immediate as u8);
        metadata(emitter, 1, 0, register_mask(source), register_mask(destination));
        enqueue(emitter, r, destination);
    }
}

register_port!(0x008e_fe30, fc4p_008efe30, emit_prefetch, "EmitPrefetch", 8);
unsafe extern "thiscall" fn emit_prefetch(r: &mut Registers) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        let operand = r.argument(0) as *mut u32;
        let hint = r.argument(1);
        begin(emitter);
        r.eax = 0;
        r.edx = 1;
        invoke(
            r,
            crate::abi::target(0x008e_c190),
            &[0x18, u32::from((hint as u8).wrapping_mul(8)), operand as u32],
        );
        metadata(emitter, 1, 1, read_unaligned(operand), 0);
        enqueue(emitter, r, (emitter.limit() as u32).wrapping_sub(40));
    }
}

register_port!(0x008e_fec0, fc4p_008efec0, emit_masked_store, "EmitMaskedStoreQword", 8);
unsafe extern "thiscall" fn emit_masked_store(r: &mut Registers) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        let source = r.argument(0);
        let mask = r.argument(1);
        begin(emitter);
        byte(emitter, 0xf7);
        byte(
            emitter,
            (source as u8).wrapping_sub(8).wrapping_mul(8).wrapping_add(mask as u8),
        );
        metadata(emitter, 1, 2, register_mask(source) | register_mask(mask) | 0x80, 0);
        enqueue(emitter, r, mask);
    }
}

register_port!(0x008e_ff70, fc4p_008eff70, emit_store_fence, "EmitStoreFence", 0);
unsafe extern "thiscall" fn emit_store_fence(r: &mut Registers) {
    unsafe {
        let emitter = Emitter::from_ptr(r.ecx as *mut u8);
        begin(emitter);
        byte(emitter, 0xae);
        byte(emitter, 0xf8);
        metadata(emitter, 1, 3, 0, 0);
        r.eax = 0;
        r.edx = 2;
        invoke(r, crate::abi::target(0x008e_cca0), &[]);
        let mut length = emitter.length();
        let mut output = emitter.output();
        let mut count = 0u32;
        if (length as i32) > 0 {
            loop {
                let value = emitter.as_ptr().wrapping_add(0x10 + count as usize).read();
                output.write(value);
                length = emitter.length();
                output = output.wrapping_add(1);
                count = count.wrapping_add(1);
                if (count as i32) >= length as i32 {
                    break;
                }
            }
        }
        emitter.set_output(output);
        r.eax = count;
        r.ecx = output as u32;
        r.edx = length;
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_008eebd0::PORT,
    fc4p_008eec80::PORT,
    fc4p_008eedd0::PORT,
    fc4p_008eee70::PORT,
    fc4p_008eef10::PORT,
    fc4p_008eefc0::PORT,
    fc4p_008ef070::PORT,
    fc4p_008ef120::PORT,
    fc4p_008ef1d0::PORT,
    fc4p_008ef280::PORT,
    fc4p_008ef3e0::PORT,
    fc4p_008ef490::PORT,
    fc4p_008ef540::PORT,
    fc4p_008ef5f0::PORT,
    fc4p_008ef690::PORT,
    fc4p_008ef740::PORT,
    fc4p_008ef7f0::PORT,
    fc4p_008ef8a0::PORT,
    fc4p_008efc10::PORT,
    fc4p_008efcc0::PORT,
    fc4p_008efd70::PORT,
    fc4p_008efe30::PORT,
    fc4p_008efec0::PORT,
    fc4p_008eff70::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Object};
    use std::cell::RefCell;

    #[derive(Clone, Copy, Debug)]
    enum Encoding {
        Pair(u8, bool),
        Memory(u8),
        Shift(u8, u8),
        Shuffle,
        Prefetch,
        MaskedStore,
    }

    // Independent instruction bytes and metadata expectations for every queued helper.
    const CASES: &[(u32, Encoding)] = &[
        (0x008e_ebd0, Encoding::Pair(0xfe, false)),
        (0x008e_ec80, Encoding::Memory(0xfe)),
        (0x008e_edd0, Encoding::Memory(0xe9)),
        (0x008e_ee70, Encoding::Memory(0xd9)),
        (0x008e_ef10, Encoding::Pair(0xfa, false)),
        (0x008e_efc0, Encoding::Shift(0x71, 0x10)),
        (0x008e_f070, Encoding::Shift(0x72, 0x10)),
        (0x008e_f120, Encoding::Shift(0x71, 0x30)),
        (0x008e_f1d0, Encoding::Shift(0x72, 0x30)),
        (0x008e_f280, Encoding::Shift(0x73, 0x30)),
        (0x008e_f3e0, Encoding::Shift(0x72, 0x20)),
        (0x008e_f490, Encoding::Pair(0xd5, false)),
        (0x008e_f540, Encoding::Pair(0xe5, false)),
        (0x008e_f5f0, Encoding::Memory(0xe5)),
        (0x008e_f690, Encoding::Pair(0xf5, false)),
        (0x008e_f740, Encoding::Pair(0x6b, false)),
        (0x008e_f7f0, Encoding::Pair(0x67, false)),
        (0x008e_f8a0, Encoding::Pair(0x60, false)),
        (0x008e_fc10, Encoding::Pair(0x74, false)),
        (0x008e_fcc0, Encoding::Pair(0x76, true)),
        (0x008e_fd70, Encoding::Shuffle),
        (0x008e_fe30, Encoding::Prefetch),
        (0x008e_fec0, Encoding::MaskedStore),
    ];

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Encode {
            this: u32,
            eax: u32,
            edx: u32,
            args: [u32; 3],
            scratch: Vec<u8>,
        },
        Append {
            this: u32,
            eax: u32,
            edx: u32,
            record: u32,
            scratch: Vec<u8>,
        },
        Flush {
            this: u32,
            eax: u32,
            edx: u32,
            scratch: Vec<u8>,
        },
    }

    #[derive(Default)]
    struct FlushChanges {
        length: Option<u32>,
        output: Option<u32>,
        first_byte: Option<u8>,
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static FLUSH: RefCell<FlushChanges> = RefCell::new(FlushChanges::default());
    }

    fn calls() -> Vec<Call> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }

    unsafe fn bytes(p: *mut u8, count: usize) -> Vec<u8> {
        unsafe { core::slice::from_raw_parts(p, count).to_vec() }
    }

    fn set_word(mem: &mut [u8], offset: usize, value: u32) {
        mem[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn word(mem: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(mem[offset..offset + 4].try_into().unwrap())
    }

    #[unsafe(naked)]
    unsafe extern "C" fn encode_fake() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "ret 12", body = sym encode_fake_body,
        );
    }

    unsafe extern "thiscall" fn encode_fake_body(r: &mut Registers) {
        unsafe {
            let p = r.ecx as *mut u8;
            let args = [r.argument(0), r.argument(1), r.argument(2)];
            CALLS.with(|c| {
                c.borrow_mut().push(Call::Encode {
                    this: r.ecx,
                    eax: r.eax,
                    edx: r.edx,
                    args,
                    scratch: bytes(p.add(8), 40),
                })
            });
            // The operand's mask changes during encoding, so callers must load it after this call.
            let operand = args[2] as *mut u32;
            write_unaligned(operand, read_unaligned(operand) ^ 0x80_0042);
            let emitter = Emitter::from_ptr(p);
            byte(emitter, args[0] as u8);
            byte(emitter, (args[1] as u8) | 5);
            byte(emitter, 0xa7);
            r.eax = 0x1234_5678;
            r.ecx = 0x8765_4321;
            r.edx = 0xfedc_ba98;
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn append_fake() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "ret 4", body = sym append_fake_body,
        );
    }

    unsafe extern "thiscall" fn append_fake_body(r: &mut Registers) {
        unsafe {
            let record = r.argument(0);
            CALLS.with(|c| {
                c.borrow_mut().push(Call::Append {
                    this: r.ecx,
                    eax: r.eax,
                    edx: r.edx,
                    record,
                    scratch: bytes(record as *mut u8, 40),
                })
            });
            // A visible queue change must survive the caller's return.
            write_unaligned((r.ecx as *mut u8).add(0x10).cast::<u32>(), 0x2468_ace0);
            r.eax = 0xa123_4567;
            r.ecx = 0xb234_5678;
            r.edx = 0xc345_6789;
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn flush_fake() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "ret", body = sym flush_fake_body,
        );
    }

    unsafe extern "thiscall" fn flush_fake_body(r: &mut Registers) {
        unsafe {
            let p = r.ecx as *mut u8;
            CALLS.with(|c| {
                c.borrow_mut().push(Call::Flush {
                    this: r.ecx,
                    eax: r.eax,
                    edx: r.edx,
                    scratch: bytes(p.add(8), 40),
                })
            });
            let emitter = Emitter::from_ptr(p);
            FLUSH.with(|f| {
                let f = f.borrow();
                if let Some(length) = f.length {
                    emitter.set_length(length);
                }
                if let Some(output) = f.output {
                    emitter.set_output(output as *mut u8);
                }
                if let Some(first_byte) = f.first_byte {
                    p.add(0x10).write(first_byte);
                }
            });
            r.eax = 0xa123_4567;
            r.ecx = 0xb234_5678;
            r.edx = 0xc345_6789;
        }
    }

    struct Fixture {
        object: Object,
        queue: Box<[u8; 128]>,
        operand: Box<[u32; 3]>,
    }

    impl Fixture {
        fn new() -> Self {
            let mut f = Self {
                object: Object::new(0x80),
                queue: Box::new([0xcc; 128]),
                operand: Box::new([0x1234_0080, 0x8877_6655, 0x4433_2211]),
            };
            unsafe {
                for i in 0..0x80 {
                    f.object.ptr().add(i).write((i as u8).wrapping_mul(13).wrapping_add(7));
                }
                let emitter = f.emitter();
                emitter.set_length(0xdead_beef);
                emitter.set_serial(u32::MAX);
                emitter.set_end(f.queue.as_mut_ptr().add(8));
                emitter.set_limit(f.queue.as_mut_ptr().add(128));
            }
            f
        }

        fn emitter(&mut self) -> Emitter {
            unsafe { Emitter::from_ptr(self.object.ptr()) }
        }

        fn run(&mut self, address: u32, args: &[u32]) -> [u32; 3] {
            let mut r = Registers {
                eax: 0x1122_3344,
                ecx: self.object.ptr() as u32,
                edx: 0x5566_7788,
            };
            unsafe { invoke(&mut r, crate::abi::target(address), args) };
            [r.eax, r.ecx, r.edx]
        }
    }

    /// Expected scratch bytes, metadata, arguments and EDX immediately before appending.
    fn expected(
        f: &mut Fixture,
        encoding: Encoding,
        a: u32,
        b: u32,
        immediate: u32,
    ) -> (Vec<u8>, Vec<u32>, u32, Option<Call>) {
        let p = f.object.ptr();
        let mut mem = unsafe { bytes(p, 0x80) };
        let mut at_encoder = mem.clone();
        set_word(&mut at_encoder, 8, 1);
        set_word(&mut at_encoder, 12, 0);
        at_encoder[0x10] = 0x0f;
        let am = 0x100u32 << (a & 31);
        let bm = 0x100u32 << (b & 31);
        let modrm = ((a as u8).wrapping_sub(8)).wrapping_mul(8).wrapping_add(b as u8);
        let (code, cost, kind, reads, writes, edx) = match encoding {
            Encoding::Pair(op, separate) => (
                vec![0x0f, op, modrm],
                1,
                0,
                if separate { bm } else { am | bm },
                am,
                if separate { a } else { am },
            ),
            Encoding::Shift(op, extension) => (
                vec![0x0f, op, (a as u8).wrapping_sub(extension), b as u8],
                1,
                0,
                am,
                am,
                am,
            ),
            Encoding::Shuffle => (vec![0x0f, 0x70, modrm, immediate as u8], 1, 0, bm, am, a),
            Encoding::MaskedStore => (vec![0x0f, 0xf7, modrm], 1, 2, am | bm | 0x80, 0, b),
            Encoding::Memory(op) => {
                let mask = f.operand[0] ^ 0x80_0042;
                (
                    vec![0x0f, op, ((a as u8).wrapping_mul(8)) | 5, 0xa7],
                    2,
                    1,
                    am | mask,
                    am,
                    am | mask,
                )
            }
            Encoding::Prefetch => (
                vec![0x0f, 0x18, ((b as u8).wrapping_mul(8)) | 5, 0xa7],
                1,
                1,
                f.operand[0] ^ 0x80_0042,
                0,
                word(&mem, 0x48).wrapping_sub(40),
            ),
        };
        set_word(&mut mem, 8, code.len() as u32);
        set_word(&mut mem, 12, 0);
        mem[0x10..0x10 + code.len()].copy_from_slice(&code);
        set_word(&mut mem, 0x20, kind);
        set_word(&mut mem, 0x24, reads);
        set_word(&mut mem, 0x28, writes);
        set_word(&mut mem, 0x2c, cost);
        let operand = f.operand.as_mut_ptr() as u32;
        let (args, encode) = match encoding {
            Encoding::Memory(op) => (
                vec![a, operand],
                Some((u32::from(op), u32::from((a as u8).wrapping_mul(8)))),
            ),
            Encoding::Prefetch => (vec![operand, b], Some((0x18, u32::from((b as u8).wrapping_mul(8))))),
            Encoding::Shuffle => (vec![a, b, immediate], None),
            _ => (vec![a, b], None),
        };
        let encode_call = encode.map(|(opcode, register)| Call::Encode {
            this: p as u32,
            eax: 0,
            edx: 1,
            args: [opcode, register, operand],
            scratch: at_encoder[8..48].to_vec(),
        });
        (mem, args, edx, encode_call)
    }

    #[test]
    fn every_queued_helper_preserves_record_bytes_masks_registers_and_call_order() {
        let _encode = fake(0x008e_c190, encode_fake as *const () as usize);
        let _append = fake(0x008e_cc20, append_fake as *const () as usize);
        for &(address, encoding) in CASES {
            for mode in 0..3 {
                for (a, b, immediate) in [
                    (0, 7, 0),
                    (7, 0, 255),
                    (8, 8, 256),
                    (23, 24, 0x1234_5678),
                    (31, 32, 0x80),
                    (255, 255, 0xffff_ffff),
                    (0xabcd_ef23, 0x9876_5436, 0xaaaa_aa55),
                ] {
                    let mut f = Fixture::new();
                    let p = f.object.ptr();
                    if mode == 1 {
                        f.emitter().set_end(core::ptr::null_mut());
                    }
                    if mode == 2 {
                        let end = f.emitter().limit().wrapping_sub(40);
                        f.emitter().set_end(end);
                    }
                    let old_queue = *f.queue;
                    let old_operand = *f.operand;
                    let (mut mem, args, edx, encode_call) = expected(&mut f, encoding, a, b, immediate);
                    let mut expected_calls: Vec<_> = encode_call.into_iter().collect();
                    let end = word(&mem, 0x40);
                    let result = f.run(address, &args);
                    if mode == 2 {
                        expected_calls.push(Call::Append {
                            this: (p as u32) + 0x30,
                            eax: (p as u32) + 0x30,
                            edx,
                            record: (p as u32) + 8,
                            scratch: mem[8..48].to_vec(),
                        });
                        set_word(&mut mem, 0x40, 0x2468_ace0);
                        assert_eq!(result, [0xa123_4567, 0xb234_5678, 0xc345_6789], "{address:08x}");
                        assert_eq!(*f.queue, old_queue);
                    } else {
                        if mode == 0 {
                            let mut queue = old_queue;
                            queue[8..48].copy_from_slice(&mem[8..48]);
                            assert_eq!(*f.queue, queue, "{address:08x}");
                        } else {
                            assert_eq!(*f.queue, old_queue);
                        }
                        set_word(&mut mem, 0x40, end.wrapping_add(40));
                        assert_eq!(result, [(p as u32) + 0x30, end.wrapping_add(40), edx], "{address:08x}");
                    }
                    assert_eq!(unsafe { bytes(p, 0x80) }, mem, "{address:08x}: {encoding:?}");
                    assert_eq!(calls(), expected_calls, "{address:08x}");
                    let mut operand = old_operand;
                    if matches!(encoding, Encoding::Memory(_) | Encoding::Prefetch) {
                        operand[0] ^= 0x80_0042;
                    }
                    assert_eq!(*f.operand, operand);
                }
            }
        }
    }

    #[test]
    fn register_and_immediate_bytes_cover_all_low_byte_values() {
        for &(address, encoding) in CASES {
            if matches!(encoding, Encoding::Memory(_) | Encoding::Prefetch) {
                continue;
            }
            let mut f = Fixture::new();
            for a in 0..256u32 {
                for b in 0..256u32 {
                    f.emitter().set_end(f.queue.as_mut_ptr());
                    f.emitter().set_serial(u32::MAX);
                    let (mem, args, edx, _) = expected(&mut f, encoding, a | 0xaabb_0000, b | 0xccdd_0000, a ^ b);
                    let result = f.run(address, &args);
                    assert_eq!(
                        unsafe { bytes(f.object.ptr().add(8), 40) },
                        mem[8..48],
                        "{address:08x} {a} {b}"
                    );
                    assert_eq!(result[2], edx);
                }
            }
        }
        assert!(calls().is_empty());
    }

    #[test]
    fn record_copy_runs_forwards_and_reloads_the_queue_end_after_copying() {
        for offset in [0x0c, 0x2c, 0x40] {
            let mut f = Fixture::new();
            let p = f.object.ptr();
            f.emitter().set_end(unsafe { p.add(offset) });
            let (mut mem, args, edx, _) = expected(&mut f, Encoding::Pair(0xfe, false), 1, 2, 0);
            for i in 0..10 {
                let value = word(&mem, 8 + i * 4);
                set_word(&mut mem, offset + i * 4, value);
            }
            let next = word(&mem, 0x40).wrapping_add(40);
            set_word(&mut mem, 0x40, next);
            assert_eq!(f.run(0x008e_ebd0, &args), [p as u32 + 0x30, next, edx]);
            assert_eq!(unsafe { bytes(p, 0x80) }, mem);
        }
        assert!(calls().is_empty());
    }

    #[test]
    fn store_fence_flushes_before_copying_and_uses_the_updated_output_and_signed_length() {
        let _flush = fake(0x008e_cca0, flush_fake as *const () as usize);
        for length in [None, Some(0), Some(1), Some(5), Some(0x8000_0000), Some(u32::MAX)] {
            let mut f = Fixture::new();
            let p = f.object.ptr();
            let output = unsafe { f.queue.as_mut_ptr().add(16) };
            let replacement = unsafe { f.queue.as_mut_ptr().add(64) };
            f.emitter().set_output(output);
            let mut expected = unsafe { bytes(p, 0x80) };
            set_word(&mut expected, 8, 3);
            set_word(&mut expected, 12, 0);
            expected[0x10..0x13].copy_from_slice(&[0x0f, 0xae, 0xf8]);
            set_word(&mut expected, 0x20, 3);
            set_word(&mut expected, 0x24, 0);
            set_word(&mut expected, 0x28, 0);
            set_word(&mut expected, 0x2c, 1);
            let expected_call = Call::Flush {
                this: p as u32,
                eax: 0,
                edx: 2,
                scratch: expected[8..48].to_vec(),
            };
            FLUSH.with(|f| {
                *f.borrow_mut() = FlushChanges {
                    length,
                    output: Some(replacement as u32),
                    first_byte: Some(0x66),
                }
            });
            let final_length = length.unwrap_or(3);
            set_word(&mut expected, 8, final_length);
            expected[0x10] = 0x66;
            let count = if (final_length as i32) > 0 { final_length } else { 0 };
            let mut queue = *f.queue;
            queue[64..64 + count as usize].copy_from_slice(&expected[0x10..0x10 + count as usize]);
            set_word(&mut expected, 4, (replacement as u32) + count);
            assert_eq!(
                f.run(0x008e_ff70, &[]),
                [count, replacement as u32 + count, final_length]
            );
            assert_eq!(*f.queue, queue);
            assert_eq!(unsafe { bytes(p, 0x80) }, expected);
            assert_eq!(calls(), [expected_call]);
        }
        FLUSH.with(|f| *f.borrow_mut() = FlushChanges::default());
    }

    #[test]
    fn store_fence_reloads_length_after_each_byte_written() {
        let _flush = fake(0x008e_cca0, flush_fake as *const () as usize);
        let mut f = Fixture::new();
        let p = f.object.ptr();
        // The flush changes the first byte to zero, and the output aliases the low byte of the length.
        f.emitter().set_output(unsafe { p.add(8) });
        let mut expected = unsafe { bytes(p, 0x80) };
        set_word(&mut expected, 8, 3);
        set_word(&mut expected, 12, 0);
        expected[0x10..0x13].copy_from_slice(&[0x0f, 0xae, 0xf8]);
        set_word(&mut expected, 0x20, 3);
        set_word(&mut expected, 0x24, 0);
        set_word(&mut expected, 0x28, 0);
        set_word(&mut expected, 0x2c, 1);
        let expected_call = Call::Flush {
            this: p as u32,
            eax: 0,
            edx: 2,
            scratch: expected[8..48].to_vec(),
        };
        FLUSH.with(|f| {
            *f.borrow_mut() = FlushChanges {
                length: None,
                output: None,
                first_byte: Some(0),
            }
        });
        // A zero prefix shortens the copy to one byte, demonstrating that the bound is reread.
        expected[0x10] = 0;
        expected[8] = 0;
        set_word(&mut expected, 4, p as u32 + 9);
        assert_eq!(f.run(0x008e_ff70, &[]), [1, p as u32 + 9, 0]);
        assert_eq!(unsafe { bytes(p, 0x80) }, expected);
        assert_eq!(calls(), [expected_call]);
        FLUSH.with(|f| *f.borrow_mut() = FlushChanges::default());
    }

    /// Probe words: entry, this, argument count, arguments, entry/exit ESP, then EBX/ESI/EDI/EBP.
    #[unsafe(naked)]
    unsafe extern "C" fn probe_machine(_probe: *mut u32) {
        core::arch::naked_asm!(
            "push ebx",
            "push esi",
            "push edi",
            "push ebp",
            "push [esp + 20]",
            "mov eax, [esp]",
            "mov [eax + 16], esp",
            "mov ecx, [eax + 8]",
            "mov edx, [eax + 12]",
            "2:",
            "test ecx, ecx",
            "jz 3f",
            "dec ecx",
            "push [edx + ecx*4]",
            "jmp 2b",
            "3:",
            "mov ecx, [eax + 4]",
            "mov edx, 0x55443322",
            "mov ebx, 0x12345678",
            "mov esi, 0x23456789",
            "mov edi, 0x3456789a",
            "mov ebp, 0x456789ab",
            "call [eax]",
            "mov eax, [esp]",
            "mov [eax + 20], esp",
            "mov [eax + 24], ebx",
            "mov [eax + 28], esi",
            "mov [eax + 32], edi",
            "mov [eax + 36], ebp",
            "pop eax",
            "pop ebp",
            "pop edi",
            "pop esi",
            "pop ebx",
            "ret",
        );
    }

    fn environment() -> (u16, u16, u32, u32) {
        let (mut cw, mut mxcsr) = (0u16, 0u32);
        let (sw, flags): (u16, u32);
        unsafe {
            core::arch::asm!(
                "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
                cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
                flags = lateout(reg) flags, out("ax") sw,
            );
        }
        (cw, sw, mxcsr, flags & 0x400)
    }

    #[test]
    fn every_export_preserves_saved_registers_stack_and_float_environment() {
        let _encode = fake(0x008e_c190, encode_fake as *const () as usize);
        let _append = fake(0x008e_cc20, append_fake as *const () as usize);
        let _flush = fake(0x008e_cca0, flush_fake as *const () as usize);
        let _fpu = crate::testing::GameFpu::new();
        for port in PORTS {
            for full in [false, true] {
                let mut f = Fixture::new();
                let p = f.object.ptr();
                let args = match CASES.iter().find(|&&(address, _)| address == port.address) {
                    Some(&(_, encoding)) => expected(&mut f, encoding, 3, 6, 0xa5).1,
                    None => {
                        f.emitter().set_output(f.queue.as_mut_ptr());
                        Vec::new()
                    }
                };
                if full {
                    let end = f.emitter().limit().wrapping_sub(40);
                    f.emitter().set_end(end);
                }
                let mut probe = [
                    port.entry as u32,
                    p as u32,
                    args.len() as u32,
                    args.as_ptr() as u32,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ];
                let sentinel = 123.25f64;
                let mut restored = 0.0f64;
                unsafe {
                    core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
                }
                let before = environment();
                unsafe {
                    probe_machine(probe.as_mut_ptr());
                }
                let after = environment();
                unsafe {
                    core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
                }
                assert_eq!(before, after, "{:08x}", port.address);
                assert_eq!(sentinel.to_bits(), restored.to_bits());
                assert_eq!(probe[4], probe[5], "{:08x}", port.address);
                assert_eq!(&probe[6..], &[0x1234_5678, 0x2345_6789, 0x3456_789a, 0x4567_89ab]);
                calls();
            }
        }
    }
}
