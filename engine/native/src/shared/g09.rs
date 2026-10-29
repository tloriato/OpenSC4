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

//! Shared helpers, group 09: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

// These boundaries expose the incoming argument words to Rust and restore the three result registers.
// All object, iterator, and container operations are performed by the Rust bodies.
macro_rules! register_boundary {
    ($body:ident, $pop:literal) => {
        core::arch::naked_asm!(
            "sub esp, 12",
            "mov [esp], eax",
            "mov [esp + 4], ecx",
            "mov [esp + 8], edx",
            "lea edx, [esp + 12]",
            "mov ecx, esp",
            "call {body}",
            "mov eax, [esp]",
            "mov ecx, [esp + 4]",
            "mov edx, [esp + 8]",
            "add esp, 12",
            "ret {pop}",
            body = sym $body,
            pop = const $pop,
        );
    };
}

macro_rules! register_port {
    ($address:literal, $export:ident, $body:ident, $pop:literal, $name:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                register_boundary!($body, $pop);
            }
        }
    };
}

/// Four words used by a deque iterator over forty-byte records, three records per block.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordIterator {
    current: u32,
    first: u32,
    last: u32,
    node: u32,
}

layout! {
    /// Deque of forty-byte records, with two iterators followed by its block map.
    struct RecordDeque {
        0x00 => begin / set_begin: RecordIterator,
        0x10 => end / set_end: RecordIterator,
        0x20 => map / set_map: u32,
        0x24 => capacity / set_capacity: u32,
    }
}

game_fn! { fn advance_record_iterator(this: *mut RecordIterator, count: i32) = "thiscall" 0x008E_C1E0; }
game_fn! { fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn move_bytes(destination: u32, source: u32, bytes: u32) -> u32 = "C" 0x009E_EB70; }
game_fn! { #[allow(dead_code)] fn reserve_map(this: *mut u8, additional: u32, front: u32) -> u32 = "thiscall" 0x008E_C5B0; }
game_fn! { fn offset_iterator(this: *mut u8, out: *mut RecordIterator, count: i32) -> u64 = "thiscall" 0x008E_C550; }
game_fn! {
    fn move_records_backward(out: *mut RecordIterator, first: RecordIterator,
        last: RecordIterator, destination: RecordIterator) -> u32 = "C" 0x008E_C850;
}
game_fn! {
    fn move_records_forward(out: *mut RecordIterator, first: RecordIterator,
        last: RecordIterator, destination: RecordIterator) -> u32 = "C" 0x008E_C8D0;
}
// These calls use the register-preserving call boundary below, including ECX after cdecl calls.
game_fn! {
    #[allow(dead_code)]
    fn walk_records(first: RecordIterator, last: RecordIterator, tag: u32) = "C" 0x008E_C6F0;
}
game_fn! {
    #[allow(dead_code)]
    fn free_blocks(this: *mut u8, first: u32, last: u32) = "thiscall" 0x006D_A8D0;
}
game_fn! {
    #[allow(dead_code)]
    fn erase_record(this: *mut u8, out: *mut RecordIterator, position: RecordIterator) -> u64 = "thiscall" 0x008E_C9F0;
}
game_fn! {
    #[allow(dead_code)]
    fn flush_records(this: *mut u8) = "thiscall" 0x008E_CCA0;
}

layout! {
    /// Instruction emitter: output cursor, scratch record, and the pending-record deque.
    struct InstructionEmitter {
        0x04 => output / set_output: u32,
        0x08 => length / set_length: i32,
        0x0C => sequence / set_sequence: u32,
        0x20 => memory_access / set_memory_access: u32,
        0x24 => reads / set_reads: u32,
        0x28 => writes / set_writes: u32,
        0x2C => latency / set_latency: u32,
        0x30 => begin / set_begin: RecordIterator,
        0x40 => end / set_end: RecordIterator,
    }
}

/// A raw game call also retains ECX, which some of these shared helpers leave visible to their callers.
unsafe fn call_registers(address: u32, registers: Registers, args: &[u32], caller_pops: bool) -> Registers {
    #[repr(C)]
    struct Call {
        target: usize,
        registers: Registers,
        args: *const u32,
        count: u32,
        caller_pops: u32,
    }
    let mut call = Call {
        target: crate::abi::target(address),
        registers,
        args: args.as_ptr(),
        count: args.len() as u32,
        caller_pops: u32::from(caller_pops),
    };
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx",
            "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:",
            "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f",
            "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
            "4:",
            in("edi") &mut call,
            clobber_abi("C"),
        );
    }
    call.registers
}
game_fn! {
    #[allow(dead_code)]
    fn copy_records_backward(out: u32, first: RecordIterator, last: RecordIterator,
        destination: RecordIterator, context: *mut u32, tag: u32) -> u64 = "C" 0x008E_C340;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_records_forward(out: u32, first: RecordIterator, last: RecordIterator,
        destination: RecordIterator, context: *mut u32, tag: u32) -> u64 = "C" 0x008E_C440;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_records_backward_checked(out: u32, first: RecordIterator, last: RecordIterator,
        destination: RecordIterator, context: *mut u32) -> u64 = "C" 0x008E_C750;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_records_forward_checked(out: u32, first: RecordIterator, last: RecordIterator,
        destination: RecordIterator, context: *mut u32) -> u64 = "C" 0x008E_C7D0;
}

// Address arithmetic wraps at 32 bits, including on synthetic iterators near the address boundary.
unsafe fn load(address: u32) -> u32 {
    unsafe { read_unaligned(address as *const u32) }
}

unsafe fn store(address: u32, value: u32) {
    unsafe { write_unaligned(address as *mut u32, value) }
}

port! {
    /// Copy an iterator, advance the copy through the game's iterator helper, and write the result.
    #[port(address = 0x008E_C550, owner = "shared", name = "OffsetRecordIterator")]
    pub unsafe extern "thiscall" fn fc4p_008ec550(this: *mut u8, out: *mut RecordIterator, count: i32) -> u64 {
        unsafe {
            let mut iterator = read_unaligned(this.cast::<RecordIterator>());
            advance_record_iterator(&mut iterator, count);
            write_unaligned(out, iterator);
            (u64::from(iterator.node) << 32) | u64::from(out as u32)
        }
    }
}

register_port!(
    0x008E_C5B0,
    fc4p_008ec5b0,
    reserve_record_map,
    8,
    "ReserveRecordDequeMap"
);

unsafe extern "fastcall" fn reserve_record_map(registers: *mut Registers, stack: *mut u32) {
    unsafe {
        (*registers).eax =
            reserve_record_map_storage((*registers).ecx as *mut u8, stack.add(1).read(), stack.add(2).read());
    }
}

/// Recenter a deque's block map, growing it when fewer than twice the requested slots fit.
unsafe fn reserve_record_map_storage(this: *mut u8, additional: u32, front: u32) -> u32 {
    unsafe {
        let deque = RecordDeque::from_ptr(this);
        let begin = deque.begin();
        let end = deque.end();
        let nodes = (end.node.wrapping_sub(begin.node) as i32 >> 2).wrapping_add(1) as u32;
        let requested = nodes.wrapping_add(additional);
        let capacity = deque.capacity();
        let bias = if front as u8 != 0 { additional } else { 0 };
        let destination;
        if requested.wrapping_mul(2) < capacity {
            destination = deque
                .map()
                .wrapping_add(((capacity.wrapping_sub(requested) >> 1).wrapping_add(bias)).wrapping_mul(4));
            let bytes = end.node.wrapping_add(4).wrapping_sub(begin.node);
            if destination < begin.node {
                if end.node.wrapping_add(4) != begin.node {
                    move_bytes(destination, begin.node, bytes);
                }
            } else if bytes as i32 > 0 {
                move_bytes(
                    destination.wrapping_add(nodes.wrapping_mul(4)).wrapping_sub(bytes),
                    begin.node,
                    bytes,
                );
            }
        } else {
            let new_capacity = capacity.wrapping_add(capacity.max(additional)).wrapping_add(2);
            let map = if new_capacity == 0 {
                0
            } else {
                allocate(new_capacity.wrapping_mul(4))
            };
            destination =
                map.wrapping_add(((new_capacity.wrapping_sub(requested) >> 1).wrapping_add(bias)).wrapping_mul(4));
            // Read both iterators again after allocation, which can move the owner's storage.
            let source = deque.begin().node;
            let after = deque.end().node.wrapping_add(4);
            if after != source {
                move_bytes(destination, source, after.wrapping_sub(source));
            }
            if deque.map() != 0 {
                free(deque.map());
            }
            deque.set_map(map);
            deque.set_capacity(new_capacity);
        }
        // Only the first/last/node words change; the current element pointers are retained.
        store(this as u32 + 0x0C, destination);
        let first = load(destination);
        store(this as u32 + 4, first);
        store(this as u32 + 8, first.wrapping_add(0x78));
        let end_node = destination.wrapping_add(nodes.wrapping_mul(4)).wrapping_sub(4);
        store(this as u32 + 0x1C, end_node);
        let last = load(end_node);
        store(this as u32 + 0x14, last);
        store(this as u32 + 0x18, last.wrapping_add(0x78));
        last.wrapping_add(0x78)
    }
}

register_port!(0x008E_C6F0, fc4p_008ec6f0, walk_record_range, 0, "WalkRecordRange");

unsafe extern "fastcall" fn walk_record_range(registers: *mut Registers, stack: *mut u32) {
    unsafe {
        let mut current = stack.add(1).read();
        let first = stack.add(2).read();
        let mut last = stack.add(3).read();
        let mut node = stack.add(4).read();
        let end = stack.add(5).read();
        // The range helper stores the first block's base into the caller's sixth argument word.
        stack.add(6).write(first);
        while current != end {
            current = current.wrapping_add(0x28);
            if current == last {
                current = load(node.wrapping_add(4));
                node = node.wrapping_add(4);
                last = current.wrapping_add(0x78);
            }
        }
        *registers = Registers {
            eax: current,
            ecx: node,
            edx: last,
        };
    }
}

macro_rules! copy_range_port {
    ($address:literal, $export:ident, $body:ident, $callee:literal, $words:literal, $name:literal) => {
        register_port!($address, $export, $body, 0, $name);
        unsafe extern "fastcall" fn $body(registers: *mut Registers, stack: *mut u32) {
            unsafe {
                let out = stack.add(1).read();
                let first = read_unaligned(stack.add(2).cast::<RecordIterator>());
                let last = read_unaligned(stack.add(6).cast::<RecordIterator>());
                let destination = read_unaligned(stack.add(10).cast::<RecordIterator>());
                let arguments = [
                    out,
                    first.current,
                    first.first,
                    first.last,
                    first.node,
                    last.current,
                    last.first,
                    last.last,
                    last.node,
                    destination.current,
                    destination.first,
                    destination.last,
                    destination.node,
                    stack.add(1) as u32,
                    0,
                ];
                let mut result = call_registers(
                    $callee,
                    Registers {
                        eax: (*registers).eax,
                        ecx: first.last,
                        edx: first.node,
                    },
                    &arguments[..$words],
                    true,
                );
                result.eax = out;
                *registers = result;
            }
        }
    };
}

copy_range_port!(
    0x008E_C750,
    fc4p_008ec750,
    copy_record_range_backward,
    0x008E_C340,
    15,
    "CopyRecordRangeBackward"
);
copy_range_port!(
    0x008E_C7D0,
    fc4p_008ec7d0,
    copy_record_range_forward,
    0x008E_C440,
    15,
    "CopyRecordRangeForward"
);
copy_range_port!(
    0x008E_C850,
    fc4p_008ec850,
    move_record_range_backward,
    0x008E_C750,
    14,
    "MoveRecordRangeBackward"
);
copy_range_port!(
    0x008E_C8D0,
    fc4p_008ec8d0,
    move_record_range_forward,
    0x008E_C7D0,
    14,
    "MoveRecordRangeForward"
);

register_port!(
    0x008E_C950,
    fc4p_008ec950,
    destroy_record_deque,
    0,
    "DestroyRecordDeque"
);

unsafe extern "fastcall" fn destroy_record_deque(registers: *mut Registers, _stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        let deque = RecordDeque::from_ptr(this as *mut u8);
        let begin = deque.begin();
        let end = deque.end();
        let mut result = call_registers(
            0x008E_C6F0,
            Registers {
                eax: (*registers).eax,
                ecx: begin.node,
                edx: begin.first,
            },
            &[
                begin.current,
                begin.first,
                begin.last,
                begin.node,
                end.current,
                end.first,
                end.last,
                end.node,
                0,
            ],
            true,
        );
        result.eax = deque.map();
        if result.eax != 0 {
            result.ecx = this;
            result.eax = deque.begin().node;
            result.edx = deque.end().node.wrapping_add(4);
            result = call_registers(0x006D_A8D0, result, &[result.eax, result.edx], false);
            let map = deque.map();
            if map != 0 {
                result = call_registers(0x0090_CF63, result, &[map], true);
            }
        }
        *registers = result;
    }
}

port! {
    /// Erase one deque record, moving the nearer half and freeing a block only when its boundary is crossed.
    #[port(address = 0x008E_C9F0, owner = "shared", name = "EraseRecordDequeElement")]
    pub unsafe extern "thiscall" fn fc4p_008ec9f0(this: *mut u8, out: *mut RecordIterator, position: RecordIterator) -> u64 {
        unsafe {
            let deque = RecordDeque::from_ptr(this);
            let mut next = position;
            next.current = next.current.wrapping_add(0x28);
            if next.current == next.last {
                next.node = next.node.wrapping_add(4);
                next.current = load(next.node);
                next.first = next.current;
                next.last = next.current.wrapping_add(0x78);
            }
            let begin = deque.begin();
            let end = deque.end();
            let remaining = begin.last.wrapping_sub(begin.current) as i32 / 40;
            let index = (position.current.wrapping_sub(position.first) as i32 / 40)
                .wrapping_sub(3)
                .wrapping_add((position.node.wrapping_sub(begin.node) as i32 >> 2).wrapping_mul(3))
                .wrapping_add(remaining) as u32;
            let length = (end.node.wrapping_sub(begin.node) as i32 >> 2).wrapping_sub(1)
                .wrapping_mul(3)
                .wrapping_add(end.current.wrapping_sub(end.first) as i32 / 40)
                .wrapping_add(remaining) as u32;
            let mut temporary = position;
            if index < length >> 1 {
                move_records_backward(&mut temporary, begin, position, next);
                let begin = deque.begin();
                if begin.current == begin.last.wrapping_sub(0x28) {
                    if begin.first != 0 { free(begin.first); }
                    let node = deque.begin().node.wrapping_add(4);
                    store(this as u32 + 0x0C, node);
                    let first = load(node);
                    store(this as u32 + 4, first);
                    store(this as u32 + 8, first.wrapping_add(0x78));
                    store(this as u32, load(this as u32 + 4));
                } else {
                    store(this as u32, begin.current.wrapping_add(0x28));
                }
            } else {
                move_records_forward(&mut temporary, next, end, position);
                let end = deque.end();
                if end.current == end.first {
                    if end.first != 0 { free(end.first); }
                    let node = deque.end().node.wrapping_sub(4);
                    store(this as u32 + 0x1C, node);
                    let first = load(node);
                    store(this as u32 + 0x14, first);
                    store(this as u32 + 0x18, first.wrapping_add(0x78));
                    store(this as u32 + 0x10, load(this as u32 + 0x18).wrapping_sub(0x28));
                } else {
                    store(this as u32 + 0x10, end.current.wrapping_sub(0x28));
                }
            }
            let result = offset_iterator(this, out, index as i32);
            (result & 0xFFFF_FFFF_0000_0000) | u64::from(out as u32)
        }
    }
}

register_port!(
    0x008E_CC20,
    fc4p_008ecc20,
    append_record_block,
    4,
    "AppendRecordDequeBlock"
);

unsafe extern "fastcall" fn append_record_block(registers: *mut Registers, stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        let source = stack.add(1).read();
        let mut record = [0u32; 10];
        // Retain a copy before map growth and allocation, including when the input aliases the last block.
        for (i, value) in record.iter_mut().enumerate() {
            *value = load(source.wrapping_add(i as u32 * 4));
        }
        let deque = RecordDeque::from_ptr(this as *mut u8);
        let distance = deque.end().node.wrapping_sub(deque.map()) as i32 >> 2;
        let available = deque.capacity().wrapping_sub(distance as u32);
        let mut result = Registers {
            eax: distance as u32,
            ecx: available,
            edx: (*registers).edx,
        };
        if available < 2 {
            result.ecx = this;
            result = call_registers(0x008E_C5B0, result, &[1, 0], false);
        }
        result = call_registers(0x0090_CF54, result, &[0x78], true);
        result.edx = deque.end().node;
        store(result.edx.wrapping_add(4), result.eax);
        let destination = deque.end().current;
        if destination != 0 {
            for (i, value) in record.iter().enumerate() {
                store(destination.wrapping_add(i as u32 * 4), *value);
            }
            result.ecx = 0;
        }
        result.eax = deque.end().node.wrapping_add(4);
        store(this + 0x1C, result.eax);
        result.eax = load(result.eax);
        store(this + 0x14, result.eax);
        store(this + 0x18, result.eax.wrapping_add(0x78));
        result.eax = load(this + 0x14);
        store(this + 0x10, result.eax);
        *registers = result;
    }
}

register_port!(
    0x008E_CCA0,
    fc4p_008ecca0,
    flush_instruction_records,
    0,
    "FlushInstructionRecords"
);

unsafe extern "fastcall" fn flush_instruction_records(registers: *mut Registers, _stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        let emitter = InstructionEmitter::from_ptr(this as *mut u8);
        let mut result = *registers;
        result.eax = emitter.end().current;
        result.ecx = emitter.begin().current;
        while emitter.end().current != emitter.begin().current {
            let begin = emitter.begin();
            let mut index = 0i32;
            let mut output = emitter.output();
            while index < load(begin.current) as i32 {
                let byte = (begin.current.wrapping_add(8).wrapping_add(index as u32) as *const u8).read();
                (output as *mut u8).write(byte);
                output = output.wrapping_add(1);
                index = index.wrapping_add(1);
            }
            emitter.set_output(output);
            let mut iterator = MaybeUninit::<RecordIterator>::uninit();
            result.eax = begin.node;
            result.ecx = this.wrapping_add(0x30);
            result.edx = begin.last;
            result = call_registers(
                0x008E_C9F0,
                result,
                &[
                    iterator.as_mut_ptr() as u32,
                    begin.current,
                    begin.first,
                    begin.last,
                    begin.node,
                ],
                false,
            );
            result.edx = emitter.end().current;
        }
        *registers = result;
    }
}

register_port!(
    0x008E_CD50,
    fc4p_008ecd50,
    align_instruction_output,
    0,
    "AlignInstructionOutput"
);

unsafe extern "fastcall" fn align_instruction_output(registers: *mut Registers, _stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        let emitter = InstructionEmitter::from_ptr(this as *mut u8);
        let mut result = call_registers(0x008E_CCA0, *registers, &[], false);
        let mut remaining = emitter.output().wrapping_neg() & 15;
        if remaining >= 4 {
            result.ecx = remaining >> 2;
            result.eax = result.ecx.wrapping_neg();
            remaining = remaining.wrapping_add(result.eax.wrapping_mul(4));
            loop {
                result.edx = emitter.output();
                (result.edx as *mut u8).write(0x3E);
                result.eax = emitter.output().wrapping_add(1);
                emitter.set_output(result.eax);
                (result.eax as *mut u8).write(0x0F);
                result.edx = emitter.output().wrapping_add(1);
                emitter.set_output(result.edx);
                result.eax = result.edx;
                (result.eax as *mut u8).write(0x7F);
                result.eax = emitter.output().wrapping_add(1);
                emitter.set_output(result.eax);
                (result.eax as *mut u8).write(0xE4);
                result.edx = emitter.output().wrapping_add(1);
                result.ecx = result.ecx.wrapping_sub(1);
                emitter.set_output(result.edx);
                if result.ecx == 0 {
                    break;
                }
            }
        }
        if remaining >= 3 {
            result.edx = remaining / 3;
            result.eax = result.edx.wrapping_mul(0xFFFF_FFFD);
            remaining = remaining.wrapping_add(result.eax);
            loop {
                result.ecx = emitter.output();
                (result.ecx as *mut u8).write(0x0F);
                result.ecx = emitter.output().wrapping_add(1);
                emitter.set_output(result.ecx);
                result.eax = result.ecx;
                (result.eax as *mut u8).write(0x7F);
                result.eax = emitter.output().wrapping_add(1);
                emitter.set_output(result.eax);
                (result.eax as *mut u8).write(0xE4);
                result.ecx = emitter.output().wrapping_add(1);
                result.edx = result.edx.wrapping_sub(1);
                emitter.set_output(result.ecx);
                if result.edx == 0 {
                    break;
                }
            }
        }
        if remaining >= 2 {
            result.ecx = remaining >> 1;
            result.edx = result.ecx.wrapping_neg();
            remaining = remaining.wrapping_add(result.edx.wrapping_mul(2));
            loop {
                result.eax = emitter.output();
                (result.eax as *mut u8).write(0x8B);
                result.eax = emitter.output().wrapping_add(1);
                emitter.set_output(result.eax);
                (result.eax as *mut u8).write(0xC0);
                result.edx = emitter.output().wrapping_add(1);
                result.ecx = result.ecx.wrapping_sub(1);
                emitter.set_output(result.edx);
                if result.ecx == 0 {
                    break;
                }
            }
        }
        if remaining & 1 != 0 {
            result.ecx = emitter.output();
            (result.ecx as *mut u8).write(0x90);
            emitter.set_output(emitter.output().wrapping_add(1));
        }
        *registers = result;
    }
}

game_fn! {
    #[allow(dead_code)]
    fn encode_memory_operand(this: *mut u8, opcode: u32, register: u32, operand: u32) = "thiscall" 0x008E_C190;
}
game_fn! {
    #[allow(dead_code)]
    fn append_instruction_block(this: *mut u8, record: u32) = "thiscall" 0x008E_CC20;
}
game_fn! {
    #[allow(dead_code)]
    fn emit_long_conditional_jump(this: *mut u8, condition: u32, destination: u32) = "thiscall" 0x008E_E0A0;
}

/// Start a scratch instruction, retaining the unused bytes from the preceding record.
unsafe fn start_instruction(this: u32) {
    unsafe {
        let sequence = load(this.wrapping_add(0x0C));
        store(this.wrapping_add(8), 0);
        store(this.wrapping_add(0x0C), sequence.wrapping_add(1));
    }
}

unsafe fn instruction_byte(this: u32, byte: u8) {
    unsafe {
        let length = load(this.wrapping_add(8));
        (this.wrapping_add(0x10).wrapping_add(length) as *mut u8).write(byte);
        store(this.wrapping_add(8), load(this.wrapping_add(8)).wrapping_add(1));
    }
}

unsafe fn instruction_metadata(this: u32, latency: u32, memory: u32, reads: u32, writes: u32) {
    unsafe {
        store(this.wrapping_add(0x2C), latency);
        store(this.wrapping_add(0x20), memory);
        store(this.wrapping_add(0x24), reads);
        store(this.wrapping_add(0x28), writes);
    }
}

/// Append ten words in order, including overlapping scratch and deque storage. At the last slot,
/// the game helper owns block growth. The comparison word remains visible in EDX in some forms.
unsafe fn queue_instruction(this: u32, mut result: Registers, boundary_in_edx: bool) -> Registers {
    unsafe {
        result.eax = this.wrapping_add(0x30);
        let boundary = load(this.wrapping_add(0x48)).wrapping_sub(0x28);
        let destination = load(this.wrapping_add(0x40));
        if boundary_in_edx {
            result.edx = boundary;
        } else {
            result.ecx = boundary;
        }
        if destination == boundary {
            result.ecx = result.eax;
            return call_registers(0x008E_CC20, result, &[this.wrapping_add(8)], false);
        }
        if destination != 0 {
            for i in 0..10u32 {
                let value = load(this.wrapping_add(8).wrapping_add(i * 4));
                store(destination.wrapping_add(i * 4), value);
            }
        }
        result.ecx = load(this.wrapping_add(0x40)).wrapping_add(0x28);
        store(this.wrapping_add(0x40), result.ecx);
        result
    }
}

register_port!(0x008E_CE20, fc4p_008ece20, emit_register_move, 8, "EmitRegisterMove");

unsafe extern "fastcall" fn emit_register_move(registers: *mut Registers, stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        let destination = stack.add(1).read();
        let source = stack.add(2).read();
        start_instruction(this);
        instruction_byte(this, 0x8B);
        instruction_byte(
            this,
            (destination as u8)
                .wrapping_sub(8)
                .wrapping_mul(8)
                .wrapping_add(source as u8),
        );
        instruction_metadata(this, 1, 0, 1u32.wrapping_shl(source), 1u32.wrapping_shl(destination));
        *registers = queue_instruction(
            this,
            Registers {
                eax: this,
                ecx: destination,
                edx: destination,
            },
            true,
        );
    }
}

// Memory forms share the operand encoder, but retain each form's own dependency masks and register result.
#[derive(Clone, Copy)]
enum MemoryEffect {
    Load,
    Store,
    Arithmetic,
    Compare,
    Address,
    PackedArithmetic,
}

#[derive(Clone, Copy)]
struct MemoryInstruction {
    prefix: u8,
    opcode: u32,
    register_argument: usize,
    mask: u32,
    effect: MemoryEffect,
    boundary_in_edx: bool,
}

unsafe fn emit_memory_instruction(registers: *mut Registers, stack: *mut u32, instruction: MemoryInstruction) {
    unsafe {
        let this = (*registers).ecx;
        let register = stack.add(instruction.register_argument).read();
        let operand = stack.add(3 - instruction.register_argument).read();
        // Memory forms advance the sequence before resetting the scratch length.
        store(this.wrapping_add(0x0C), load(this.wrapping_add(0x0C)).wrapping_add(1));
        store(this.wrapping_add(8), 0);
        let mut result = *registers;
        result.eax = u32::from((register as u8).wrapping_mul(8));
        if instruction.prefix != 0 {
            instruction_byte(this, instruction.prefix);
            result.eax = 0;
            result.edx = 1;
        }
        result = call_registers(
            0x008E_C190,
            result,
            &[instruction.opcode, u32::from((register as u8).wrapping_mul(8)), operand],
            false,
        );
        let memory_reads = load(operand);
        let mask = instruction.mask.wrapping_shl(register);
        let (latency, memory, reads, writes) = match instruction.effect {
            MemoryEffect::Load => (1, 1, memory_reads, mask),
            MemoryEffect::Store => (2, 2, memory_reads | mask, 0),
            MemoryEffect::Arithmetic => (2, 1, memory_reads | mask, mask | 0x10000),
            MemoryEffect::Compare => (2, 1, memory_reads | mask, 0x10000),
            MemoryEffect::Address => (1, 0, memory_reads, mask),
            MemoryEffect::PackedArithmetic => (2, 1, memory_reads | mask, mask),
        };
        instruction_metadata(this, latency, memory, reads, writes);
        result.ecx = register;
        match instruction.effect {
            MemoryEffect::Load => {
                // The plain integer load leaves the operand encoder's EDX intact.
                if instruction.prefix != 0 || instruction.mask != 1 {
                    result.edx = mask;
                }
            }
            MemoryEffect::Store | MemoryEffect::Compare | MemoryEffect::PackedArithmetic => result.edx = reads,
            MemoryEffect::Address => result.edx = mask,
            MemoryEffect::Arithmetic => result.ecx = reads,
        }
        *registers = queue_instruction(this, result, instruction.boundary_in_edx);
    }
}

macro_rules! memory_instruction_port {
    ($address:literal, $export:ident, $body:ident, $name:literal,
        $prefix:literal, $opcode:literal, $argument:literal, $mask:literal, $effect:ident, $edx:literal) => {
        register_port!($address, $export, $body, 8, $name);
        unsafe extern "fastcall" fn $body(registers: *mut Registers, stack: *mut u32) {
            unsafe {
                emit_memory_instruction(
                    registers,
                    stack,
                    MemoryInstruction {
                        prefix: $prefix,
                        opcode: $opcode,
                        register_argument: $argument,
                        mask: $mask,
                        effect: MemoryEffect::$effect,
                        boundary_in_edx: $edx,
                    },
                );
            }
        }
    };
}

memory_instruction_port!(
    0x008E_CF80,
    fc4p_008ecf80,
    emit_memory_load,
    "EmitMemoryLoad",
    0,
    0x8B,
    1,
    1,
    Load,
    false
);
memory_instruction_port!(
    0x008E_D0B0,
    fc4p_008ed0b0,
    emit_zero_extended_word_load,
    "EmitZeroExtendedWordLoad",
    0x0F,
    0xB7,
    1,
    1,
    Load,
    true
);
memory_instruction_port!(
    0x008E_D150,
    fc4p_008ed150,
    emit_word_store,
    "EmitWordStore",
    0x66,
    0x89,
    2,
    1,
    Store,
    false
);
memory_instruction_port!(
    0x008E_D1F0,
    fc4p_008ed1f0,
    emit_memory_store,
    "EmitMemoryStore",
    0,
    0x89,
    2,
    1,
    Store,
    false
);
memory_instruction_port!(
    0x008E_D5E0,
    fc4p_008ed5e0,
    emit_memory_add,
    "EmitMemoryAdd",
    0,
    0x03,
    1,
    1,
    Arithmetic,
    true
);
memory_instruction_port!(
    0x008E_D860,
    fc4p_008ed860,
    emit_word_memory_compare,
    "EmitWordMemoryCompare",
    0x66,
    0x3B,
    1,
    1,
    Compare,
    false
);
memory_instruction_port!(
    0x008E_D900,
    fc4p_008ed900,
    emit_memory_compare,
    "EmitMemoryCompare",
    0,
    0x3B,
    1,
    1,
    Compare,
    false
);
memory_instruction_port!(
    0x008E_DA40,
    fc4p_008eda40,
    emit_memory_subtract,
    "EmitMemorySubtract",
    0,
    0x2B,
    1,
    1,
    Arithmetic,
    true
);
memory_instruction_port!(
    0x008E_DDB0,
    fc4p_008eddb0,
    emit_effective_address,
    "EmitEffectiveAddress",
    0,
    0x8D,
    1,
    1,
    Address,
    false
);
memory_instruction_port!(
    0x008E_DE40,
    fc4p_008ede40,
    emit_memory_multiply,
    "EmitMemoryMultiply",
    0x0F,
    0xAF,
    1,
    1,
    PackedArithmetic,
    false
);
memory_instruction_port!(
    0x008E_E3D0,
    fc4p_008ee3d0,
    emit_mmx_dword_load,
    "EmitMmxDwordLoad",
    0x0F,
    0x6E,
    1,
    0x100,
    Load,
    false
);
memory_instruction_port!(
    0x008E_E470,
    fc4p_008ee470,
    emit_mmx_dword_store,
    "EmitMmxDwordStore",
    0x0F,
    0x7E,
    2,
    0x100,
    Store,
    false
);
memory_instruction_port!(
    0x008E_E5C0,
    fc4p_008ee5c0,
    emit_mmx_qword_load,
    "EmitMmxQwordLoad",
    0x0F,
    0x6F,
    1,
    0x100,
    Load,
    false
);
memory_instruction_port!(
    0x008E_E660,
    fc4p_008ee660,
    emit_mmx_qword_store,
    "EmitMmxQwordStore",
    0x0F,
    0x7F,
    2,
    0x100,
    Store,
    false
);
memory_instruction_port!(
    0x008E_E7B0,
    fc4p_008ee7b0,
    emit_mmx_memory_and,
    "EmitMmxMemoryAnd",
    0x0F,
    0xDB,
    1,
    0x100,
    PackedArithmetic,
    false
);
memory_instruction_port!(
    0x008E_EB30,
    fc4p_008eeb30,
    emit_mmx_memory_maximum,
    "EmitMmxMemoryMaximum",
    0x0F,
    0xED,
    1,
    0x100,
    PackedArithmetic,
    false
);

#[derive(Clone, Copy)]
enum RegisterEffect {
    Arithmetic,
    WordCompare,
    FromMmx,
    ToMmx,
    MmxMove,
    MmxArithmetic,
    MmxXor,
}

unsafe fn emit_register_instruction(
    registers: *mut Registers,
    stack: *mut u32,
    prefix: u8,
    opcode: u8,
    effect: RegisterEffect,
) {
    unsafe {
        let this = (*registers).ecx;
        let destination = stack.add(1).read();
        let source = stack.add(2).read();
        start_instruction(this);
        if prefix != 0 {
            instruction_byte(this, prefix);
        }
        instruction_byte(this, opcode);
        let (high, low) = if matches!(effect, RegisterEffect::FromMmx) {
            (source, destination)
        } else {
            (destination, source)
        };
        instruction_byte(
            this,
            (high as u8).wrapping_sub(8).wrapping_mul(8).wrapping_add(low as u8),
        );
        let d = 1u32.wrapping_shl(destination);
        let s = 1u32.wrapping_shl(source);
        let md = 0x100u32.wrapping_shl(destination);
        let ms = 0x100u32.wrapping_shl(source);
        let (reads, writes, edx) = match effect {
            RegisterEffect::Arithmetic => (d | s, d | 0x10000, d | 0x10000),
            RegisterEffect::WordCompare => (d | s, 0x10000, source),
            RegisterEffect::FromMmx => (ms, d, destination),
            RegisterEffect::ToMmx => (s, md, destination),
            RegisterEffect::MmxMove => (ms, md, destination),
            RegisterEffect::MmxArithmetic => (md | ms, md, md),
            RegisterEffect::MmxXor => (if destination == source { 0 } else { md | ms }, md, md),
        };
        instruction_metadata(this, 1, 0, reads, writes);
        *registers = queue_instruction(
            this,
            Registers {
                eax: this,
                ecx: source,
                edx,
            },
            matches!(effect, RegisterEffect::MmxXor) && destination != source,
        );
    }
}

macro_rules! register_instruction_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $prefix:literal, $opcode:literal, $effect:ident) => {
        register_port!($address, $export, $body, 8, $name);
        unsafe extern "fastcall" fn $body(registers: *mut Registers, stack: *mut u32) {
            unsafe {
                emit_register_instruction(registers, stack, $prefix, $opcode, RegisterEffect::$effect);
            }
        }
    };
}

register_instruction_port!(
    0x008E_D530,
    fc4p_008ed530,
    emit_register_add,
    "EmitRegisterAdd",
    0,
    0x03,
    Arithmetic
);
register_instruction_port!(
    0x008E_D7B0,
    fc4p_008ed7b0,
    emit_word_register_compare,
    "EmitWordRegisterCompare",
    0x66,
    0x3B,
    WordCompare
);
register_instruction_port!(
    0x008E_D990,
    fc4p_008ed990,
    emit_register_subtract,
    "EmitRegisterSubtract",
    0,
    0x2B,
    Arithmetic
);
register_instruction_port!(
    0x008E_E270,
    fc4p_008ee270,
    emit_register_from_mmx,
    "EmitRegisterFromMmx",
    0x0F,
    0x7E,
    FromMmx
);
register_instruction_port!(
    0x008E_E320,
    fc4p_008ee320,
    emit_mmx_from_register,
    "EmitMmxFromRegister",
    0x0F,
    0x6E,
    ToMmx
);
register_instruction_port!(
    0x008E_E510,
    fc4p_008ee510,
    emit_mmx_register_move,
    "EmitMmxRegisterMove",
    0x0F,
    0x6F,
    MmxMove
);
register_instruction_port!(
    0x008E_E700,
    fc4p_008ee700,
    emit_mmx_register_and,
    "EmitMmxRegisterAnd",
    0x0F,
    0xDB,
    MmxArithmetic
);
register_instruction_port!(
    0x008E_E850,
    fc4p_008ee850,
    emit_mmx_register_and_not,
    "EmitMmxRegisterAndNot",
    0x0F,
    0xDF,
    MmxArithmetic
);
register_instruction_port!(
    0x008E_E900,
    fc4p_008ee900,
    emit_mmx_register_xor,
    "EmitMmxRegisterXor",
    0x0F,
    0xEF,
    MmxXor
);
register_instruction_port!(
    0x008E_E9D0,
    fc4p_008ee9d0,
    emit_mmx_register_add_words,
    "EmitMmxRegisterAddWords",
    0x0F,
    0xFD,
    MmxArithmetic
);

unsafe fn emit_immediate_instruction(registers: *mut Registers, stack: *mut u32, opcode: u8, bias: u8, width: u32) {
    unsafe {
        let this = (*registers).ecx;
        let register = stack.add(1).read();
        let immediate = stack.add(2).read();
        start_instruction(this);
        instruction_byte(this, opcode);
        instruction_byte(this, (register as u8).wrapping_add(bias));
        for i in 0..width {
            instruction_byte(this, (immediate >> (i * 8)) as u8);
        }
        let reads = 1u32.wrapping_shl(register);
        let writes = reads | 0x10000;
        instruction_metadata(this, 1, 0, reads, writes);
        *registers = queue_instruction(
            this,
            Registers {
                eax: this,
                ecx: register,
                edx: writes,
            },
            false,
        );
    }
}

macro_rules! immediate_instruction_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $opcode:literal, $bias:literal, $width:literal) => {
        register_port!($address, $export, $body, 8, $name);
        unsafe extern "fastcall" fn $body(registers: *mut Registers, stack: *mut u32) {
            unsafe {
                emit_immediate_instruction(registers, stack, $opcode, $bias, $width);
            }
        }
    };
}

immediate_instruction_port!(
    0x008E_D3C0,
    fc4p_008ed3c0,
    emit_register_and_immediate,
    "EmitRegisterAndImmediate",
    0x81,
    0xE0,
    4
);
immediate_instruction_port!(
    0x008E_D670,
    fc4p_008ed670,
    emit_register_add_immediate,
    "EmitRegisterAddImmediate",
    0x81,
    0xC0,
    4
);
immediate_instruction_port!(
    0x008E_DAD0,
    fc4p_008edad0,
    emit_register_shift_right,
    "EmitRegisterShiftRight",
    0xC1,
    0xE8,
    1
);
immediate_instruction_port!(
    0x008E_DB70,
    fc4p_008edb70,
    emit_register_arithmetic_shift_right,
    "EmitRegisterArithmeticShiftRight",
    0xC1,
    0xF8,
    1
);
immediate_instruction_port!(
    0x008E_DC10,
    fc4p_008edc10,
    emit_register_rotate_left,
    "EmitRegisterRotateLeft",
    0xC1,
    0xC0,
    1
);

register_port!(
    0x008E_D730,
    fc4p_008ed730,
    emit_accumulator_add_carry,
    0,
    "EmitAccumulatorAddCarry"
);
unsafe extern "fastcall" fn emit_accumulator_add_carry(registers: *mut Registers, _stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        start_instruction(this);
        for byte in [0x15, 0, 0, 0, 0] {
            instruction_byte(this, byte);
        }
        instruction_metadata(this, 2, 0, 0x10001, 0x10001);
        *registers = queue_instruction(
            this,
            Registers {
                eax: this,
                ecx: this,
                edx: 0,
            },
            false,
        );
    }
}

register_port!(
    0x008E_DD30,
    fc4p_008edd30,
    emit_register_decrement,
    4,
    "EmitRegisterDecrement"
);
unsafe extern "fastcall" fn emit_register_decrement(registers: *mut Registers, stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        let register = stack.add(1).read();
        start_instruction(this);
        instruction_byte(this, (register as u8).wrapping_add(0x48));
        let mask = 1u32.wrapping_shl(register);
        instruction_metadata(this, 1, 0, mask, mask | 0x10000);
        *registers = queue_instruction(
            this,
            Registers {
                eax: this,
                ecx: register,
                edx: mask | 0x10000,
            },
            false,
        );
    }
}

/// Write the scratch instruction directly after flushing pending records. Read the length again after
/// each byte store, so an output cursor that aliases the owner has the same effects.
unsafe fn write_instruction_bytes(this: u32, index_in_eax: bool) -> Registers {
    unsafe {
        let mut length = load(this.wrapping_add(8));
        let mut output = load(this.wrapping_add(4));
        let mut index = 0u32;
        while (index as i32) < length as i32 {
            let byte = (this.wrapping_add(0x10).wrapping_add(index) as *const u8).read();
            (output as *mut u8).write(byte);
            length = load(this.wrapping_add(8));
            output = output.wrapping_add(1);
            index = index.wrapping_add(1);
        }
        store(this.wrapping_add(4), output);
        if index_in_eax {
            Registers {
                eax: index,
                ecx: output,
                edx: length,
            }
        } else {
            Registers {
                eax: output,
                ecx: index,
                edx: length,
            }
        }
    }
}

#[derive(Clone, Copy)]
enum JumpPlaceholder {
    Unconditional,
    Conditional,
    ShortConditional,
    LongConditional,
}

unsafe fn emit_jump_placeholder(registers: *mut Registers, stack: *mut u32, form: JumpPlaceholder) {
    unsafe {
        let this = (*registers).ecx;
        call_registers(0x008E_CCA0, *registers, &[], false);
        start_instruction(this);
        let sequence = load(this.wrapping_add(0x0C));
        let long = matches!(form, JumpPlaceholder::Conditional | JumpPlaceholder::LongConditional);
        let short = matches!(form, JumpPlaceholder::ShortConditional);
        let condition_argument = if matches!(form, JumpPlaceholder::LongConditional) {
            1
        } else {
            2
        };
        let condition = if matches!(form, JumpPlaceholder::Unconditional) {
            0
        } else {
            stack.add(condition_argument).read()
        };
        if long {
            instruction_byte(this, 0x0F);
        }
        instruction_byte(
            this,
            if matches!(form, JumpPlaceholder::Unconditional) {
                0xE9
            } else {
                (condition as u8).wrapping_add(if short { 0x70 } else { 0x80 })
            },
        );
        let displacement_size = if short { 1 } else { 4 };
        for _ in 0..displacement_size {
            instruction_byte(this, 0);
        }
        instruction_metadata(this, 1, 3, 0, 0);
        let flush = Registers {
            eax: if short {
                (sequence & !0xFF) | u32::from((condition as u8).wrapping_add(0x70))
            } else if long {
                5
            } else {
                1
            },
            ecx: this,
            edx: if long { 4 } else { 1 },
        };
        call_registers(0x008E_CCA0, flush, &[], false);
        let mut result = write_instruction_bytes(this, false);
        if matches!(form, JumpPlaceholder::LongConditional) {
            result.eax = result.eax.wrapping_sub(4);
            if result.eax != 0 {
                result.ecx = stack.add(2).read().wrapping_sub(result.eax).wrapping_sub(4);
                store(result.eax, result.ecx);
            }
        } else {
            result.ecx = result.eax.wrapping_sub(displacement_size);
            result.eax = stack.add(1).read();
            store(result.eax, result.ecx);
        }
        *registers = result;
    }
}

macro_rules! jump_placeholder_port {
    ($address:literal, $export:ident, $body:ident, $pop:literal, $name:literal, $form:ident) => {
        register_port!($address, $export, $body, $pop, $name);
        unsafe extern "fastcall" fn $body(registers: *mut Registers, stack: *mut u32) {
            unsafe {
                emit_jump_placeholder(registers, stack, JumpPlaceholder::$form);
            }
        }
    };
}

jump_placeholder_port!(
    0x008E_DEE0,
    fc4p_008edee0,
    emit_jump_patch_location,
    4,
    "EmitJumpPatchLocation",
    Unconditional
);
jump_placeholder_port!(
    0x008E_DF70,
    fc4p_008edf70,
    emit_conditional_jump_patch_location,
    8,
    "EmitConditionalJumpPatchLocation",
    Conditional
);
jump_placeholder_port!(
    0x008E_E020,
    fc4p_008ee020,
    emit_short_jump_patch_location,
    8,
    "EmitShortJumpPatchLocation",
    ShortConditional
);
jump_placeholder_port!(
    0x008E_E0A0,
    fc4p_008ee0a0,
    emit_conditional_jump_long,
    8,
    "EmitConditionalJumpLong",
    LongConditional
);

register_port!(
    0x008E_E160,
    fc4p_008ee160,
    emit_conditional_jump,
    8,
    "EmitConditionalJump"
);
unsafe extern "fastcall" fn emit_conditional_jump(registers: *mut Registers, stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        call_registers(0x008E_CCA0, *registers, &[], false);
        start_instruction(this);
        let destination = stack.add(2).read();
        let displacement = destination.wrapping_sub(load(this.wrapping_add(4))).wrapping_sub(2);
        let signed_byte = displacement as i8 as i32 as u32;
        let condition = stack.add(1).read();
        if signed_byte != displacement {
            *registers = call_registers(
                0x008E_E0A0,
                Registers {
                    eax: condition,
                    ecx: this,
                    edx: signed_byte,
                },
                &[condition, destination],
                false,
            );
            return;
        }
        instruction_byte(this, (condition as u8).wrapping_add(0x70));
        instruction_byte(this, displacement as u8);
        instruction_metadata(this, 1, 3, 0, 0);
        call_registers(
            0x008E_CCA0,
            Registers {
                eax: displacement,
                ecx: this,
                edx: 0,
            },
            &[],
            false,
        );
        *registers = write_instruction_bytes(this, true);
    }
}

register_port!(
    0x008E_E200,
    fc4p_008ee200,
    emit_return_instruction,
    0,
    "EmitReturnInstruction"
);
unsafe extern "fastcall" fn emit_return_instruction(registers: *mut Registers, _stack: *mut u32) {
    unsafe {
        let this = (*registers).ecx;
        start_instruction(this);
        instruction_byte(this, 0xC3);
        instruction_metadata(this, 4, 3, 0, 0);
        call_registers(
            0x008E_CCA0,
            Registers {
                eax: 0,
                ecx: this,
                edx: (*registers).edx,
            },
            &[],
            false,
        );
        *registers = write_instruction_bytes(this, true);
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_008ec550::PORT,
    fc4p_008ec5b0::PORT,
    fc4p_008ec6f0::PORT,
    fc4p_008ec750::PORT,
    fc4p_008ec7d0::PORT,
    fc4p_008ec850::PORT,
    fc4p_008ec8d0::PORT,
    fc4p_008ec950::PORT,
    fc4p_008ec9f0::PORT,
    fc4p_008ecc20::PORT,
    fc4p_008ecca0::PORT,
    fc4p_008ecd50::PORT,
    fc4p_008ece20::PORT,
    fc4p_008ecf80::PORT,
    fc4p_008ed0b0::PORT,
    fc4p_008ed150::PORT,
    fc4p_008ed1f0::PORT,
    fc4p_008ed3c0::PORT,
    fc4p_008ed530::PORT,
    fc4p_008ed5e0::PORT,
    fc4p_008ed670::PORT,
    fc4p_008ed730::PORT,
    fc4p_008ed7b0::PORT,
    fc4p_008ed860::PORT,
    fc4p_008ed900::PORT,
    fc4p_008ed990::PORT,
    fc4p_008eda40::PORT,
    fc4p_008edad0::PORT,
    fc4p_008edb70::PORT,
    fc4p_008edc10::PORT,
    fc4p_008edd30::PORT,
    fc4p_008eddb0::PORT,
    fc4p_008ede40::PORT,
    fc4p_008edee0::PORT,
    fc4p_008edf70::PORT,
    fc4p_008ee020::PORT,
    fc4p_008ee0a0::PORT,
    fc4p_008ee160::PORT,
    fc4p_008ee200::PORT,
    fc4p_008ee270::PORT,
    fc4p_008ee320::PORT,
    fc4p_008ee3d0::PORT,
    fc4p_008ee470::PORT,
    fc4p_008ee510::PORT,
    fc4p_008ee5c0::PORT,
    fc4p_008ee660::PORT,
    fc4p_008ee700::PORT,
    fc4p_008ee7b0::PORT,
    fc4p_008ee850::PORT,
    fc4p_008ee900::PORT,
    fc4p_008ee9d0::PORT,
    fc4p_008eeb30::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Fake};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Advance(RecordIterator, i32),
        Allocate(u32),
        Move(u32, u32, u32),
        Free(u32),
        Copy(u32, u32, [RecordIterator; 3], u32, Option<u32>),
        Walk([RecordIterator; 2], u32),
        Blocks(u32, u32, u32),
        Reserve(u32, u32, u32),
        Shift(u32, [RecordIterator; 3]),
        Offset(u32, i32),
        Erase(u32, RecordIterator),
        Flush(u32),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static ALLOCATION: RefCell<u32> = const { RefCell::new(0) };
    }

    fn calls() -> Vec<Call> {
        CALLS.with(|s| core::mem::take(&mut *s.borrow_mut()))
    }

    fn record(call: Call) {
        CALLS.with(|s| s.borrow_mut().push(call));
    }

    // Like a game caller, this probe owns the argument words. Cdecl calls copy the words back after return,
    // making writes to those words observable. Register outputs and the callee's stack balance are checked.
    #[repr(C)]
    struct Probe {
        target: usize,
        registers: Registers,
        args: *mut u32,
        count: u32,
        caller_pops: u32,
        stack_delta: i32,
        saved: [u32; 3],
    }

    pub(super) unsafe fn invoke(function: usize, this: u32, args: &mut [u32], caller_pops: bool) -> Registers {
        let mut probe = Probe {
            target: function,
            registers: Registers {
                eax: 0x1234_5678,
                ecx: this,
                edx: 0x8765_4321,
            },
            args: args.as_mut_ptr(),
            count: args.len() as u32,
            caller_pops: u32::from(caller_pops),
            stack_delta: 0,
            saved: [0; 3],
        };
        unsafe {
            core::arch::asm!(
                "push ebp", "push ebx", "push esi",
                "mov [edi + 28], esp",
                "mov ebx, 0x11223344", "mov esi, 0x55667788", "mov ebp, 0x99AABBCC",
                "mov ecx, [edi + 20]",
                "mov eax, [edi + 16]",
                "2:", "test ecx, ecx", "jz 3f", "dec ecx",
                "push dword ptr [eax + ecx*4]", "jmp 2b",
                "3:",
                "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
                "call dword ptr [edi]",
                "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
                "cmp dword ptr [edi + 24], 0", "je 6f",
                "mov ecx, [edi + 20]", "mov edx, [edi + 16]", "mov eax, esp",
                "4:", "test ecx, ecx", "jz 5f",
                "push dword ptr [eax]", "pop dword ptr [edx]",
                "add eax, 4", "add edx, 4", "dec ecx", "jmp 4b",
                "5:", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
                "6:", "sub [edi + 28], esp",
                "mov [edi + 32], ebx", "mov [edi + 36], esi", "mov [edi + 40], ebp",
                "pop esi", "pop ebx", "pop ebp",
                in("edi") &mut probe,
                clobber_abi("C"),
            );
        }
        assert_eq!(probe.stack_delta, 0);
        assert_eq!(probe.saved, [0x1122_3344, 0x5566_7788, 0x99AA_BBCC]);
        probe.registers
    }

    extern "thiscall" fn advance(iterator: *mut RecordIterator, count: i32) {
        unsafe {
            record(Call::Advance(iterator.read(), count));
            assert_ne!(iterator as u32, 0);
            *iterator = RecordIterator {
                current: 0x1234,
                first: 0x4567,
                last: 0x89AB,
                node: 0xCDEF,
            };
        }
    }

    extern "C" fn allocation(bytes: u32) -> u32 {
        record(Call::Allocate(bytes));
        ALLOCATION.with(|p| *p.borrow())
    }

    extern "C" fn movement(destination: u32, source: u32, bytes: u32) -> u32 {
        record(Call::Move(destination, source, bytes));
        unsafe { core::ptr::copy(source as *const u8, destination as *mut u8, bytes as usize) };
        destination
    }

    extern "C" fn deletion(pointer: u32) -> u32 {
        record(Call::Free(pointer));
        0xDEAD_BEEF
    }

    fn map_fakes() -> [Fake; 3] {
        [
            fake(0x0090_CF54, allocation as *const () as usize),
            fake(0x009E_EB70, movement as *const () as usize),
            fake(0x0090_CF63, deletion as *const () as usize),
        ]
    }

    #[test]
    fn iterator_offset_calls_the_game_on_a_copy_and_supports_output_aliasing() {
        let _fake = fake(0x008E_C1E0, advance as *const () as usize);
        for count in [0, 1, -1, i32::MIN, i32::MAX] {
            for alias in [false, true] {
                let original = RecordIterator {
                    current: 11,
                    first: 22,
                    last: 33,
                    node: 44,
                };
                let mut source = original;
                let mut output = original;
                let out = if alias { &mut source } else { &mut output } as *mut RecordIterator;
                let result = unsafe {
                    invoke(
                        fc4p_008ec550 as *const () as usize,
                        &mut source as *mut RecordIterator as u32,
                        &mut [out as u32, count as u32],
                        false,
                    )
                };
                let expected = RecordIterator {
                    current: 0x1234,
                    first: 0x4567,
                    last: 0x89AB,
                    node: 0xCDEF,
                };
                assert_eq!(unsafe { out.read() }, expected);
                assert_eq!((result.eax, result.edx), (out as u32, 0xCDEF));
                assert_eq!(source, if alias { expected } else { original });
                assert_eq!(calls(), [Call::Advance(original, count)]);
            }
        }
    }

    fn deque(map: &mut [u32], begin: usize, end: usize) -> [u32; 12] {
        let map_ptr = map.as_mut_ptr() as u32;
        [
            0x1122_3344,
            map[begin],
            map[begin].wrapping_add(120),
            map_ptr + begin as u32 * 4,
            0x5566_7788,
            map[end],
            map[end].wrapping_add(120),
            map_ptr + end as u32 * 4,
            map_ptr,
            map.len() as u32,
            0xA5A5_A5A5,
            0x5A5A_5A5A,
        ]
    }

    #[test]
    fn deque_map_recenters_left_right_and_in_place_without_changing_current_elements() {
        let _fakes = map_fakes();
        for begin in [0, 6, 11] {
            for front in [0, 1, 0x100, 0xFF] {
                let mut map = [0xA5A5_A5A5u32; 16];
                map[begin..begin + 3].copy_from_slice(&[0x1000, 0x2000, 0x3000]);
                let map_ptr = map.as_mut_ptr() as u32;
                let mut object = deque(&mut map, begin, begin + 2);
                let before = object;
                let destination = 6 + usize::from(front as u8 != 0);
                let mut expected_map = map;
                expected_map.copy_within(begin..begin + 3, destination);
                let result = unsafe {
                    invoke(
                        fc4p_008ec5b0 as *const () as usize,
                        object.as_mut_ptr() as u32,
                        &mut [1, front],
                        false,
                    )
                };
                let mut expected = before;
                expected[1] = 0x1000;
                expected[2] = 0x1078;
                expected[3] = map_ptr + destination as u32 * 4;
                expected[5] = 0x3000;
                expected[6] = 0x3078;
                expected[7] = expected[3] + 8;
                assert_eq!(object, expected);
                assert_eq!(map, expected_map);
                assert_eq!(result.eax, 0x3078);
                assert_eq!(calls(), [Call::Move(expected[3], before[3], 12)]);
            }
        }
    }

    #[test]
    fn deque_map_growth_allocates_copies_frees_and_updates_only_the_map_fields() {
        let _fakes = map_fakes();
        for (capacity, additional) in [(4, 1), (4, 4), (4, 7), (6, 1), (0, 5)] {
            for front in [0, 1, 0x100, 0xFF] {
                let mut map = vec![0x1000, 0x2000, 0x3000, 0x4000];
                let mut allocation = [0xA5A5_A5A5u32; 24];
                let new_map = allocation.as_mut_ptr() as u32;
                ALLOCATION.with(|p| *p.borrow_mut() = new_map);
                let mut object = deque(&mut map, 1, 2);
                object[9] = capacity;
                let before = object;
                let new_capacity = capacity + capacity.max(additional) + 2;
                let index = (new_capacity - (2 + additional)) / 2 + if front as u8 != 0 { additional } else { 0 };
                let destination = new_map + index * 4;
                let result = unsafe {
                    invoke(
                        fc4p_008ec5b0 as *const () as usize,
                        object.as_mut_ptr() as u32,
                        &mut [additional, front],
                        false,
                    )
                };
                let mut expected = before;
                expected[3] = destination;
                expected[7] = destination + 4;
                expected[8] = new_map;
                expected[9] = new_capacity;
                assert_eq!(object, expected);
                let mut expected_allocation = [0xA5A5_A5A5u32; 24];
                expected_allocation[index as usize..index as usize + 2].copy_from_slice(&[0x2000, 0x3000]);
                assert_eq!(allocation, expected_allocation);
                assert_eq!((result.eax, result.ecx), (0x3078, object.as_mut_ptr() as u32));
                assert_eq!(
                    calls(),
                    [
                        Call::Allocate(new_capacity * 4),
                        Call::Move(destination, before[3], 8),
                        Call::Free(before[8])
                    ]
                );
            }
        }
    }

    #[test]
    fn deque_map_empty_node_span_skips_the_copy() {
        let _fakes = map_fakes();
        for (end, destination) in [(4, 7), (3, 8)] {
            let mut map = [0x2000; 16];
            let base = map.as_mut_ptr() as u32;
            let mut object = deque(&mut map, 5, end);
            let result = unsafe {
                invoke(
                    fc4p_008ec5b0 as *const () as usize,
                    object.as_mut_ptr() as u32,
                    &mut [1, 0],
                    false,
                )
            };
            assert_eq!(object[3], base + destination * 4);
            assert_eq!(object[7], base + 6 * 4);
            assert_eq!(result.eax, 0x2078);
            assert!(calls().is_empty());
        }
    }

    #[test]
    fn deque_map_wrapping_capacity_skips_zero_sized_allocation() {
        let _fakes = map_fakes();
        let mut source = [0x1000u32, 0x2000];
        let mut destination = [0xA5A5_A5A5u32; 4];
        let address = destination.as_mut_ptr() as u32;
        let requested = address.wrapping_neg() >> 1;
        let additional = requested.wrapping_sub(2);
        let mut object = deque(&mut source, 0, 1);
        object[9] = 0x7FFF_FFFF;
        let before = object;
        assert!(requested.wrapping_mul(2) >= object[9]);
        let result = unsafe {
            invoke(
                fc4p_008ec5b0 as *const () as usize,
                object.as_mut_ptr() as u32,
                &mut [additional, 0],
                false,
            )
        };
        let mut expected = before;
        expected[3] = address;
        expected[7] = address + 4;
        expected[8] = 0;
        expected[9] = 0;
        assert_eq!(object, expected);
        assert_eq!(destination, [0x1000, 0x2000, 0xA5A5_A5A5, 0xA5A5_A5A5]);
        assert_eq!(calls(), [Call::Move(address, before[3], 8), Call::Free(before[8])]);
        assert_eq!(result.eax, 0x2078);
    }

    #[test]
    fn deque_map_growth_without_an_owned_map_skips_free() {
        let _fakes = map_fakes();
        let mut source = [0x1000u32, 0x2000];
        let mut destination = [0xA5A5_A5A5u32; 8];
        let address = destination.as_mut_ptr() as u32;
        ALLOCATION.with(|p| *p.borrow_mut() = address);
        let mut object = deque(&mut source, 0, 1);
        object[8] = 0;
        object[9] = 0;
        let source_address = object[3];
        let result = unsafe {
            invoke(
                fc4p_008ec5b0 as *const () as usize,
                object.as_mut_ptr() as u32,
                &mut [1, 0],
                false,
            )
        };
        assert_eq!(calls(), [Call::Allocate(12), Call::Move(address, source_address, 8)]);
        assert_eq!(object[8], address);
        assert_eq!(object[9], 3);
        assert_eq!(
            destination,
            [
                0x1000,
                0x2000,
                0xA5A5_A5A5,
                0xA5A5_A5A5,
                0xA5A5_A5A5,
                0xA5A5_A5A5,
                0xA5A5_A5A5,
                0xA5A5_A5A5
            ]
        );
        assert_eq!(result.eax, 0x2078);
    }

    #[test]
    fn range_walk_preserves_objects_updates_its_argument_and_returns_the_final_iterator() {
        for (start, end, end_node) in [
            (0x1000, 0x1000, 0),
            (0x1000, 0x1050, 0),
            (0x1050, 0x2028, 1),
            (0x1028, 0x3000, 2),
        ] {
            let mut map = [0x1000u32, 0x2000, 0x3000];
            let node = map.as_mut_ptr() as u32;
            let before = map;
            let mut args = [
                start,
                0x1000,
                0x1078,
                node,
                end,
                0xDEAD_BEEF,
                0xEEEE_EEEE,
                0xFFFF_FFFF,
                0,
            ];
            let result = unsafe { invoke(fc4p_008ec6f0 as *const () as usize, 0xAABB_CCDD, &mut args, true) };
            assert_eq!(
                result,
                Registers {
                    eax: end,
                    ecx: node + end_node * 4,
                    edx: map[end_node as usize] + 120
                }
            );
            assert_eq!(
                args,
                [start, 0x1000, 0x1078, node, end, 0x1000, 0xEEEE_EEEE, 0xFFFF_FFFF, 0]
            );
            assert_eq!(map, before);
        }
    }

    #[test]
    fn range_walk_wraps_addresses_and_preserves_float_and_direction_state() {
        let _fpu = crate::testing::GameFpu::new();
        let mut map = [0xFFFF_FF88u32, 0x2000];
        let node = map.as_mut_ptr() as u32;
        let mut args = [0xFFFF_FFD8, 0xFFFF_FF88, 0, node, 0x2000, 9, 10, 11, 0];
        let value = 0x3FF1_2345_6789_ABCDu64;
        let mut recovered = 0u64;
        let mut saved_mxcsr = 0u32;
        unsafe {
            core::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut saved_mxcsr);
        }
        let chosen_mxcsr = (saved_mxcsr & !0x6000) | 0x2000;
        let mut after_mxcsr = 0u32;
        let flags: u32;
        let before_top = crate::x87::status_word() & 0x3800;
        let before_control = crate::x87::control_word();
        unsafe {
            core::arch::asm!("ldmxcsr [{p}]", p = in(reg) &chosen_mxcsr);
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &value, out("st(0)") _);
            let result = invoke(fc4p_008ec6f0 as *const () as usize, 0, &mut args, true);
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut recovered, out("st(0)") _);
            core::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut after_mxcsr);
            core::arch::asm!("ldmxcsr [{p}]", p = in(reg) &saved_mxcsr);
            core::arch::asm!("pushfd", "pop {flags}", flags = out(reg) flags);
            assert_eq!(
                result,
                Registers {
                    eax: 0x2000,
                    ecx: node + 4,
                    edx: 0x2078
                }
            );
        }
        assert_eq!(args[5], 0xFFFF_FF88);
        assert_eq!(recovered, value);
        assert_eq!(after_mxcsr, chosen_mxcsr);
        assert_eq!(crate::x87::control_word(), before_control);
        assert_eq!(crate::x87::status_word() & 0x3800, before_top);
        assert_eq!(flags & 0x400, 0);
    }

    unsafe fn copy_fake(
        kind: u32,
        out: u32,
        iterators: [RecordIterator; 3],
        context: *mut u32,
        tag: Option<u32>,
    ) -> u64 {
        unsafe {
            record(Call::Copy(kind, out, iterators, context.read(), tag));
            context.write(0xDECA_FBAD);
            let result = RecordIterator {
                current: 0x1010,
                first: 0x2020,
                last: 0x3030,
                node: 0x4040,
            };
            write_unaligned(out as *mut RecordIterator, result);
            0x4040_ABCD_EF01
        }
    }

    macro_rules! copy_fake_entry {
        ($entry:ident, $body:ident, $kind:literal, $tag:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $entry() {
                register_boundary!($body, 0);
            }
            unsafe extern "fastcall" fn $body(registers: *mut Registers, stack: *mut u32) {
                unsafe {
                    let first = read_unaligned(stack.add(2).cast::<RecordIterator>());
                    assert_eq!((*registers).ecx, first.last);
                    assert_eq!((*registers).edx, first.node);
                    let result = copy_fake(
                        $kind,
                        stack.add(1).read(),
                        [
                            first,
                            read_unaligned(stack.add(6).cast()),
                            read_unaligned(stack.add(10).cast()),
                        ],
                        stack.add(14).read() as *mut u32,
                        if $tag { Some(stack.add(15).read()) } else { None },
                    );
                    *registers = Registers {
                        eax: result as u32,
                        ecx: 0x3030,
                        edx: (result >> 32) as u32,
                    };
                }
            }
        };
    }
    copy_fake_entry!(backward, backward_body, 0, true);
    copy_fake_entry!(forward, forward_body, 1, true);
    copy_fake_entry!(backward_checked, backward_checked_body, 2, false);
    copy_fake_entry!(forward_checked, forward_checked_body, 3, false);

    #[test]
    fn copy_wrappers_forward_all_iterator_words_and_the_original_argument_address() {
        let fixtures = [
            (
                fc4p_008ec750 as *const () as usize,
                0x008E_C340,
                backward as *const () as usize,
                Some(0),
            ),
            (
                fc4p_008ec7d0 as *const () as usize,
                0x008E_C440,
                forward as *const () as usize,
                Some(0),
            ),
            (
                fc4p_008ec850 as *const () as usize,
                0x008E_C750,
                backward_checked as *const () as usize,
                None,
            ),
            (
                fc4p_008ec8d0 as *const () as usize,
                0x008E_C7D0,
                forward_checked as *const () as usize,
                None,
            ),
        ];
        for (kind, (entry, address, callee, tag)) in fixtures.into_iter().enumerate() {
            let _fake = fake(address, callee);
            let mut output = [0xA5A5_A5A5u32; 6];
            let out = unsafe { output.as_mut_ptr().add(1) } as u32;
            let mut args = [out, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let result = unsafe { invoke(entry, 0x8765_4321, &mut args, true) };
            assert_eq!(
                result,
                Registers {
                    eax: out,
                    ecx: 0x3030,
                    edx: 0x4040
                }
            );
            assert_eq!(args[0], 0xDECA_FBAD);
            assert_eq!(&args[1..], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
            assert_eq!(output, [0xA5A5_A5A5, 0x1010, 0x2020, 0x3030, 0x4040, 0xA5A5_A5A5]);
            assert_eq!(
                calls(),
                [Call::Copy(
                    kind as u32,
                    out,
                    [
                        RecordIterator {
                            current: 1,
                            first: 2,
                            last: 3,
                            node: 4
                        },
                        RecordIterator {
                            current: 5,
                            first: 6,
                            last: 7,
                            node: 8
                        },
                        RecordIterator {
                            current: 9,
                            first: 10,
                            last: 11,
                            node: 12
                        },
                    ],
                    out,
                    tag
                )]
            );
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn walk_fake() {
        register_boundary!(walk_fake_body, 0);
    }
    unsafe extern "fastcall" fn walk_fake_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            record(Call::Walk(
                [read_unaligned(stack.add(1).cast()), read_unaligned(stack.add(5).cast())],
                stack.add(9).read(),
            ));
            *registers = Registers {
                eax: 0xABAB_ABAB,
                ecx: 0xCDCD_CDCD,
                edx: 0xEFEF_EFEF,
            };
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn blocks_fake() {
        register_boundary!(blocks_fake_body, 8);
    }
    unsafe extern "fastcall" fn blocks_fake_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            let this = (*registers).ecx;
            record(Call::Blocks(this, stack.add(1).read(), stack.add(2).read()));
            // Exercise the second map read, which follows block destruction.
            if load(this + 0x28) != 0 {
                store(this + 0x20, 0);
            }
            *registers = Registers {
                eax: 0x1111_2222,
                ecx: 0x3333_4444,
                edx: 0x5555_6666,
            };
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn free_registers_fake() {
        register_boundary!(free_registers_body, 0);
    }
    unsafe extern "fastcall" fn free_registers_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            record(Call::Free(stack.add(1).read()));
            *registers = Registers {
                eax: 0xAAAA_BBBB,
                ecx: 0xCCCC_DDDD,
                edx: 0xEEEE_FFFF,
            };
        }
    }

    #[test]
    fn deque_destruction_walks_before_freeing_blocks_and_rechecks_the_map_pointer() {
        let _walk = fake(0x008E_C6F0, walk_fake as *const () as usize);
        let _blocks = fake(0x006D_A8D0, blocks_fake as *const () as usize);
        let _free = fake(0x0090_CF63, free_registers_fake as *const () as usize);
        for (map_present, cleared) in [(false, false), (true, false), (true, true)] {
            let mut map = [0x1000u32, 0x2000, 0x3000, 0x4000];
            let mut object = deque(&mut map, 1, 2);
            if !map_present {
                object[8] = 0;
            }
            object[10] = u32::from(cleared);
            let this = object.as_mut_ptr() as u32;
            let before = object;
            let begin = unsafe { read_unaligned(object.as_ptr().cast::<RecordIterator>()) };
            let end = unsafe { read_unaligned(object.as_ptr().add(4).cast::<RecordIterator>()) };
            let result = unsafe { invoke(fc4p_008ec950 as *const () as usize, this, &mut [], false) };
            let mut expected_calls = vec![Call::Walk([begin, end], 0)];
            let expected_result = if map_present {
                expected_calls.push(Call::Blocks(this, begin.node, end.node + 4));
                if !cleared {
                    expected_calls.push(Call::Free(before[8]));
                    Registers {
                        eax: 0xAAAA_BBBB,
                        ecx: 0xCCCC_DDDD,
                        edx: 0xEEEE_FFFF,
                    }
                } else {
                    Registers {
                        eax: 0x1111_2222,
                        ecx: 0x3333_4444,
                        edx: 0x5555_6666,
                    }
                }
            } else {
                Registers {
                    eax: 0,
                    ecx: 0xCDCD_CDCD,
                    edx: 0xEFEF_EFEF,
                }
            };
            assert_eq!(result, expected_result);
            assert_eq!(calls(), expected_calls);
            let mut expected_object = before;
            if cleared {
                expected_object[8] = 0;
            }
            assert_eq!(object, expected_object);
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn reserve_fake() {
        register_boundary!(reserve_fake_body, 8);
    }
    unsafe extern "fastcall" fn reserve_fake_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            record(Call::Reserve(
                (*registers).ecx,
                stack.add(1).read(),
                stack.add(2).read(),
            ));
            // Move the end node back to a slot with enough room.
            let deque = RecordDeque::from_ptr((*registers).ecx as *mut u8);
            store((*registers).ecx + 0x1C, deque.map() + 4);
            *registers = Registers {
                eax: 0x1111,
                ecx: 0x2222,
                edx: 0x3333,
            };
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn allocate_registers_fake() {
        register_boundary!(allocate_registers_body, 0);
    }
    unsafe extern "fastcall" fn allocate_registers_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            record(Call::Allocate(stack.add(1).read()));
            *registers = Registers {
                eax: ALLOCATION.with(|p| *p.borrow()),
                ecx: 0x4444,
                edx: 0x5555,
            };
        }
    }

    #[test]
    fn append_block_saves_an_aliased_record_before_reserve_and_keeps_null_destination_untouched() {
        let _reserve = fake(0x008E_C5B0, reserve_fake as *const () as usize);
        let _allocate = fake(0x0090_CF54, allocate_registers_fake as *const () as usize);
        for grow in [false, true] {
            for null_destination in [false, true] {
                let mut map = [0x1000u32, 0x2000, 0x3000, 0x4000];
                let mut object = deque(&mut map, 0, if grow { 3 } else { 1 });
                let this = object.as_mut_ptr() as u32;
                let mut old_block = [0xA5A5_A5A5u32; 32];
                for (i, word) in old_block.iter_mut().take(10).enumerate() {
                    *word = 0x1234_0000 + i as u32;
                }
                let input = old_block.as_mut_ptr() as u32;
                let record = old_block[..10].to_vec();
                object[4] = if null_destination { 0 } else { input + 4 };
                let before_object = object;
                let before_map = map;
                let mut new_block = [0xCAFE_BABEu32; 30];
                let new_pointer = new_block.as_mut_ptr() as u32;
                ALLOCATION.with(|p| *p.borrow_mut() = new_pointer);
                let result = unsafe { invoke(fc4p_008ecc20 as *const () as usize, this, &mut [input], false) };
                let end_node = before_object[8] + 4;
                assert_eq!(
                    result,
                    Registers {
                        eax: new_pointer,
                        ecx: if null_destination { 0x4444 } else { 0 },
                        edx: end_node
                    }
                );
                let mut expected_calls = Vec::new();
                if grow {
                    expected_calls.push(Call::Reserve(this, 1, 0));
                }
                expected_calls.push(Call::Allocate(120));
                assert_eq!(calls(), expected_calls);
                let mut expected = before_object;
                expected[4] = new_pointer;
                expected[5] = new_pointer;
                expected[6] = new_pointer + 120;
                expected[7] = end_node + 4;
                assert_eq!(object, expected);
                let mut expected_map = before_map;
                expected_map[2] = new_pointer;
                assert_eq!(map, expected_map);
                if !null_destination {
                    assert_eq!(&old_block[1..11], record);
                }
                assert_eq!(new_block, [0xCAFE_BABE; 30]);
            }
        }
    }

    extern "C" fn shift_backward(
        out: *mut RecordIterator,
        first: RecordIterator,
        last: RecordIterator,
        destination: RecordIterator,
    ) -> u32 {
        record(Call::Shift(0, [first, last, destination]));
        unsafe {
            out.write(destination);
        }
        0xDEAD_BEEF
    }
    extern "C" fn shift_forward(
        out: *mut RecordIterator,
        first: RecordIterator,
        last: RecordIterator,
        destination: RecordIterator,
    ) -> u32 {
        record(Call::Shift(1, [first, last, destination]));
        unsafe {
            out.write(destination);
        }
        0xDEAD_BEEF
    }
    extern "thiscall" fn offset_fake(this: *mut u8, out: *mut RecordIterator, count: i32) -> u64 {
        record(Call::Offset(this as u32, count));
        unsafe {
            *out = read_unaligned(this.cast());
        }
        (0xABCD_EF01u64 << 32) | 0xDEAD_BEEF
    }

    #[test]
    fn erase_moves_the_nearer_half_crosses_both_block_boundaries_and_returns_the_game_iterator() {
        let _backward = fake(0x008E_C850, shift_backward as *const () as usize);
        let _forward = fake(0x008E_C8D0, shift_forward as *const () as usize);
        let _free = fake(0x0090_CF63, deletion as *const () as usize);
        let _offset = fake(0x008E_C550, offset_fake as *const () as usize);
        for (front, crosses) in [(true, false), (true, true), (false, false), (false, true)] {
            let mut map = [0x1000u32, 0x2000, 0x3000, 0x4000, 0x5000];
            let mut object = deque(&mut map, 0, 3);
            object[0] = if front && crosses { 0x1050 } else { 0x1000 };
            object[4] = if !front && crosses { 0x4000 } else { 0x4050 };
            let this = object.as_mut_ptr() as u32;
            let before = object;
            let begin = unsafe { read_unaligned(object.as_ptr().cast::<RecordIterator>()) };
            let end = unsafe { read_unaligned(object.as_ptr().add(4).cast::<RecordIterator>()) };
            let position = if front {
                begin
            } else {
                RecordIterator {
                    current: 0x3050,
                    first: 0x3000,
                    last: 0x3078,
                    node: begin.node + 8,
                }
            };
            let next = if position.current == 0x3050 {
                RecordIterator {
                    current: 0x4000,
                    first: 0x4000,
                    last: 0x4078,
                    node: begin.node + 12,
                }
            } else if position.current == 0x1050 {
                RecordIterator {
                    current: 0x2000,
                    first: 0x2000,
                    last: 0x2078,
                    node: begin.node + 4,
                }
            } else {
                RecordIterator {
                    current: 0x1028,
                    ..position
                }
            };
            let index = if front { 0 } else { 8 };
            let mut output = position;
            let out = &mut output as *mut RecordIterator;
            let result = unsafe {
                invoke(
                    fc4p_008ec9f0 as *const () as usize,
                    this,
                    &mut [
                        out as u32,
                        position.current,
                        position.first,
                        position.last,
                        position.node,
                    ],
                    false,
                )
            };
            let mut expected_calls = vec![if front {
                Call::Shift(0, [begin, position, next])
            } else {
                Call::Shift(1, [next, end, position])
            }];
            let mut expected = before;
            if front {
                if crosses {
                    expected_calls.push(Call::Free(begin.first));
                    expected[..4].copy_from_slice(&[0x2000, 0x2000, 0x2078, begin.node + 4]);
                } else {
                    expected[0] += 40;
                }
            } else if crosses {
                expected_calls.push(Call::Free(end.first));
                expected[4..8].copy_from_slice(&[0x3050, 0x3000, 0x3078, end.node - 4]);
            } else {
                expected[4] -= 40;
            }
            expected_calls.push(Call::Offset(this, index));
            assert_eq!(calls(), expected_calls);
            assert_eq!(object, expected);
            assert_eq!((result.eax, result.edx), (out as u32, 0xABCD_EF01));
            assert_eq!(output, unsafe { read_unaligned(expected.as_ptr().cast()) });
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn erase_fake() {
        register_boundary!(erase_fake_body, 20);
    }
    unsafe extern "fastcall" fn erase_fake_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            let this = (*registers).ecx;
            let position = read_unaligned(stack.add(2).cast::<RecordIterator>());
            record(Call::Erase(this, position));
            // Move forward one record and change the block metadata to exercise fresh reads next time.
            store(this, position.current.wrapping_add(40));
            store(this + 4, position.first.wrapping_add(1));
            *registers = Registers {
                eax: 0x1122_3344,
                ecx: 0x5566_7788,
                edx: 0x99AA_BBCC,
            };
        }
    }

    #[test]
    fn flushing_copies_each_record_in_order_skips_nonpositive_lengths_and_erases_through_the_game() {
        let _erase = fake(0x008E_C9F0, erase_fake as *const () as usize);
        for lengths in [[0, -1, 0], [1, 3, 16], [i32::MIN, 0, 2]] {
            let mut emitter = [0xA5A5_A5A5u32; 24];
            let this = emitter.as_mut_ptr() as u32;
            let mut output = [0xCDu8; 64];
            let output_base = output.as_mut_ptr() as u32;
            let mut records = [0u32; 30];
            let record_base = records.as_mut_ptr() as u32;
            for (i, length) in lengths.into_iter().enumerate() {
                records[i * 10] = length as u32;
                records[i * 10 + 9] = [0, 1, 2][i];
                unsafe {
                    core::ptr::write_bytes(records.as_mut_ptr().add(i * 10 + 2).cast::<u8>(), 0x40 + i as u8, 16);
                }
            }
            let begin = RecordIterator {
                current: record_base,
                first: 0x1000,
                last: 0x2000,
                node: 0x3000,
            };
            emitter[1] = output_base;
            unsafe {
                write_unaligned(emitter.as_mut_ptr().add(12).cast(), begin);
                write_unaligned(
                    emitter.as_mut_ptr().add(16).cast(),
                    RecordIterator {
                        current: record_base + 120,
                        ..begin
                    },
                );
            }
            let before = emitter;
            let result = unsafe { invoke(fc4p_008ecca0 as *const () as usize, this, &mut [], false) };
            assert_eq!(
                result,
                Registers {
                    eax: 0x1122_3344,
                    ecx: 0x5566_7788,
                    edx: record_base + 120
                }
            );
            let mut expected_output = [0xCDu8; 64];
            let mut written = 0;
            let mut expected_calls = Vec::new();
            for (i, length) in lengths.into_iter().enumerate() {
                if length > 0 {
                    expected_output[written..written + length as usize].fill(0x40 + i as u8);
                    written += length as usize;
                }
                expected_calls.push(Call::Erase(
                    this + 0x30,
                    RecordIterator {
                        current: record_base + i as u32 * 40,
                        first: begin.first + i as u32,
                        ..begin
                    },
                ));
            }
            assert_eq!(output, expected_output);
            assert_eq!(calls(), expected_calls);
            let mut expected = before;
            expected[1] = output_base + written as u32;
            expected[12] = record_base + 120;
            expected[13] = begin.first + 3;
            assert_eq!(emitter, expected);
        }
    }

    #[test]
    fn flushing_an_empty_queue_keeps_the_entry_edx_and_writes_nothing() {
        let mut emitter = [0xA5A5_A5A5u32; 24];
        let this = emitter.as_mut_ptr() as u32;
        emitter[12] = 0x1234;
        emitter[16] = 0x1234;
        let before = emitter;
        let result = unsafe { invoke(fc4p_008ecca0 as *const () as usize, this, &mut [], false) };
        assert_eq!(
            result,
            Registers {
                eax: 0x1234,
                ecx: 0x1234,
                edx: 0x8765_4321
            }
        );
        assert_eq!(emitter, before);
        assert!(calls().is_empty());
    }

    #[unsafe(naked)]
    unsafe extern "C" fn flush_fake() {
        register_boundary!(flush_fake_body, 0);
    }
    unsafe extern "fastcall" fn flush_fake_body(registers: *mut Registers, _stack: *mut u32) {
        unsafe {
            record(Call::Flush((*registers).ecx));
            // Alignment must use the output cursor after the flush.
            let emitter = InstructionEmitter::from_ptr((*registers).ecx as *mut u8);
            emitter.set_output(emitter.output().wrapping_add(1));
            *registers = Registers {
                eax: 0x1111_2222,
                ecx: 0x3333_4444,
                edx: 0x5555_6666,
            };
        }
    }

    #[test]
    fn alignment_flushes_first_covers_all_sixteen_offsets_and_retains_the_matching_register_results() {
        let _flush = fake(0x008E_CCA0, flush_fake as *const () as usize);
        for residue in 0..16 {
            let mut emitter = [0xA5A5_A5A5u32; 24];
            let this = emitter.as_mut_ptr() as u32;
            let mut output = [0xCDu128; 6];
            unsafe {
                core::ptr::write_bytes(output.as_mut_ptr().cast::<u8>(), 0xCD, 96);
            }
            let output_base = output.as_mut_ptr() as u32;
            let start = output_base + 16 + residue;
            emitter[1] = start - 1;
            let before = emitter;
            let result = unsafe { invoke(fc4p_008ecd50 as *const () as usize, this, &mut [], false) };
            let padding = start.wrapping_neg() & 15;
            let mut expected_output = [0xCDu8; 96];
            let mut bytes = Vec::new();
            for _ in 0..padding / 4 {
                bytes.extend([0x3E, 0x0F, 0x7F, 0xE4]);
            }
            match padding % 4 {
                0 => (),
                1 => bytes.push(0x90),
                2 => bytes.extend([0x8B, 0xC0]),
                3 => bytes.extend([0x0F, 0x7F, 0xE4]),
                _ => unreachable!(),
            }
            let begin = (start - output_base) as usize;
            expected_output[begin..begin + bytes.len()].copy_from_slice(&bytes);
            assert_eq!(
                unsafe { core::slice::from_raw_parts(output.as_ptr().cast::<u8>(), 96) },
                expected_output
            );
            let mut expected = before;
            expected[1] = start + padding;
            assert_eq!(emitter, expected);
            assert_eq!(calls(), [Call::Flush(this)]);
            let mut expected_result = Registers {
                eax: 0x1111_2222,
                ecx: 0x3333_4444,
                edx: 0x5555_6666,
            };
            let four_end = start + padding / 4 * 4;
            if padding >= 4 {
                expected_result = Registers {
                    eax: four_end - 1,
                    ecx: 0,
                    edx: four_end,
                };
            }
            match padding % 4 {
                0 => (),
                1 => expected_result.ecx = four_end,
                2 => {
                    expected_result = Registers {
                        eax: four_end + 1,
                        ecx: 0,
                        edx: four_end + 2,
                    }
                }
                3 => {
                    expected_result = Registers {
                        eax: four_end + 2,
                        ecx: four_end + 3,
                        edx: 0,
                    }
                }
                _ => unreachable!(),
            }
            assert_eq!(result, expected_result);
        }
    }
}

#[cfg(test)]
mod instruction_tests {
    use super::tests::invoke;
    use super::*;
    use crate::testing::fake;
    use std::cell::RefCell;

    const CALLEE_RESULT: Registers = Registers {
        eax: 0xAABB_CCDD,
        ecx: 0x1122_3344,
        edx: 0xDDEE_1234,
    };
    const OPERAND_READS: u32 = 0x1357_9BDF;
    const VALUES: [u32; 10] = [0, 1, 7, 8, 31, 32, 0xFF, 0x100, 0x1234_5678, u32::MAX];

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Operand(Registers, [u32; 3], [u32; 10]),
        Append(Registers, u32, [u32; 10]),
        Flush(Registers, [u32; 10]),
        LongJump(Registers, [u32; 2], [u32; 10]),
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
        static FLUSH_LENGTH: RefCell<Option<u32>> = const { RefCell::new(None) };
        static FLUSH_OUTPUT: RefCell<Option<u32>> = const { RefCell::new(None) };
    }

    fn events() -> Vec<Event> {
        EVENTS.with(|e| core::mem::take(&mut *e.borrow_mut()))
    }
    fn log(event: Event) {
        EVENTS.with(|e| e.borrow_mut().push(event));
    }
    unsafe fn scratch(this: u32) -> [u32; 10] {
        unsafe { read_unaligned(this.wrapping_add(8) as *const [u32; 10]) }
    }
    fn scratch_expected(object: &[u32; 28]) -> [u32; 10] {
        object[2..12].try_into().unwrap()
    }
    fn byte(object: &mut [u32; 28], index: usize, value: u8) {
        let word = 4 + index / 4;
        let shift = (index % 4) * 8;
        object[word] = (object[word] & !(0xFF << shift)) | (u32::from(value) << shift);
    }
    fn expected_record(object: &mut [u32; 28], bytes: &[u8], metadata: [u32; 4]) {
        object[2] = bytes.len() as u32;
        object[3] = object[3].wrapping_add(1);
        for (i, &value) in bytes.iter().enumerate() {
            byte(object, i, value);
        }
        object[8..12].copy_from_slice(&metadata);
    }

    #[unsafe(naked)]
    unsafe extern "C" fn operand_fake() {
        register_boundary!(operand_body, 12);
    }
    unsafe extern "fastcall" fn operand_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            let this = (*registers).ecx;
            let args = [stack.add(1).read(), stack.add(2).read(), stack.add(3).read()];
            log(Event::Operand(*registers, args, scratch(this)));
            // The encoder can alter its descriptor before the owner reads the dependency mask.
            store(args[2], OPERAND_READS);
            for value in [args[0] as u8, args[1] as u8, 0x42] {
                instruction_byte(this, value);
            }
            *registers = CALLEE_RESULT;
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn append_fake() {
        register_boundary!(append_body, 4);
    }
    unsafe extern "fastcall" fn append_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            let source = stack.add(1).read();
            log(Event::Append(
                *registers,
                source,
                read_unaligned(source as *const [u32; 10]),
            ));
            store((*registers).ecx.wrapping_add(0x10), 0x1020_3040);
            *registers = CALLEE_RESULT;
        }
    }

    #[derive(Clone, Copy)]
    enum Destination {
        Slot,
        Null,
        Boundary,
    }
    const DESTINATIONS: [Destination; 3] = [Destination::Slot, Destination::Null, Destination::Boundary];

    fn object(block: &mut [u32; 30], destination: Destination, sequence: u32) -> [u32; 28] {
        let mut object = [0xA5A5_A5A5; 28];
        let first = block.as_mut_ptr() as u32;
        object[3] = sequence;
        object[16] = if matches!(destination, Destination::Null) {
            0
        } else {
            first + 8
        };
        object[18] = if matches!(destination, Destination::Boundary) {
            first + 48
        } else {
            first + 120
        };
        object
    }

    fn check_queue(
        actual: &[u32; 28],
        mut expected: [u32; 28],
        block: &[u32; 30],
        before_block: [u32; 30],
        result: Registers,
        edx: u32,
        destination: Destination,
    ) {
        let this = actual.as_ptr() as u32;
        let mut expected_block = before_block;
        if matches!(destination, Destination::Boundary) {
            assert_eq!(
                events(),
                [Event::Append(
                    Registers {
                        eax: this + 0x30,
                        ecx: this + 0x30,
                        edx
                    },
                    this + 8,
                    scratch_expected(&expected),
                )]
            );
            expected[16] = 0x1020_3040;
            assert_eq!(result, CALLEE_RESULT);
        } else {
            assert!(events().is_empty());
            if matches!(destination, Destination::Slot) {
                expected_block[2..12].copy_from_slice(&expected[2..12]);
            }
            expected[16] = expected[16].wrapping_add(40);
            assert_eq!(
                result,
                Registers {
                    eax: this + 0x30,
                    ecx: expected[16],
                    edx
                }
            );
        }
        assert_eq!(*actual, expected);
        assert_eq!(*block, expected_block);
    }

    #[derive(Clone, Copy)]
    enum MemoryEdx {
        Encoder,
        Mask,
        Reads,
        Boundary,
    }
    #[derive(Clone, Copy)]
    struct MemoryCase {
        entry: usize,
        prefix: u8,
        opcode: u8,
        argument: usize,
        mask: u32,
        latency: u32,
        memory: u32,
        reads_register: bool,
        writes_register: bool,
        flags: bool,
        edx: MemoryEdx,
    }

    fn check_memory(case: MemoryCase) {
        let _operand = fake(0x008E_C190, operand_fake as *const () as usize);
        let _append = fake(0x008E_CC20, append_fake as *const () as usize);
        for register in VALUES {
            for destination in DESTINATIONS {
                for aliased in [false, true] {
                    let mut block = [0xCCCC_CCCC; 30];
                    let before_block = block;
                    let mut object = object(
                        &mut block,
                        destination,
                        if register & 1 == 0 { u32::MAX } else { 0x1020_3040 },
                    );
                    let this = object.as_mut_ptr() as u32;
                    let mut descriptor = [0x0246_8ACE, 0xFFEEDDCC, 0xBA98_7654];
                    let operand = if aliased {
                        this + 0x24
                    } else {
                        descriptor.as_mut_ptr() as u32
                    };
                    let mut args = if case.argument == 1 {
                        [register, operand]
                    } else {
                        [operand, register]
                    };
                    let mut before_call = object;
                    before_call[2] = 0;
                    before_call[3] = before_call[3].wrapping_add(1);
                    if case.prefix != 0 {
                        before_call[2] = 1;
                        byte(&mut before_call, 0, case.prefix);
                    }
                    let modrm = u32::from((register as u8).wrapping_mul(8));
                    let mask = case.mask.wrapping_shl(register);
                    let reads = OPERAND_READS | if case.reads_register { mask } else { 0 };
                    let writes = if case.writes_register { mask } else { 0 } | if case.flags { 0x10000 } else { 0 };
                    let mut expected = object;
                    let mut bytes = Vec::new();
                    if case.prefix != 0 {
                        bytes.push(case.prefix);
                    }
                    bytes.extend([case.opcode, modrm as u8, 0x42]);
                    expected_record(&mut expected, &bytes, [case.memory, reads, writes, case.latency]);
                    let result = unsafe { invoke(case.entry, this, &mut args, false) };
                    let mut logged = events();
                    assert_eq!(
                        logged.remove(0),
                        Event::Operand(
                            Registers {
                                eax: if case.prefix != 0 { 0 } else { modrm },
                                ecx: this,
                                edx: if case.prefix != 0 { 1 } else { 0x8765_4321 }
                            },
                            [u32::from(case.opcode), modrm, operand],
                            scratch_expected(&before_call),
                        )
                    );
                    EVENTS.with(|e| *e.borrow_mut() = logged);
                    let edx = match case.edx {
                        MemoryEdx::Encoder => CALLEE_RESULT.edx,
                        MemoryEdx::Mask => mask,
                        MemoryEdx::Reads => reads,
                        MemoryEdx::Boundary => expected[18].wrapping_sub(40),
                    };
                    check_queue(&object, expected, &block, before_block, result, edx, destination);
                    if !aliased {
                        assert_eq!(descriptor, [OPERAND_READS, 0xFFEEDDCC, 0xBA98_7654]);
                    }
                }
            }
        }
    }

    macro_rules! memory_test {
        ($test:ident, $entry:ident, $prefix:literal, $opcode:literal, $argument:literal, $mask:literal,
            $latency:literal, $memory:literal, $reads:literal, $writes:literal, $flags:literal, $edx:ident) => {
            #[test]
            fn $test() {
                check_memory(MemoryCase {
                    entry: $entry as *const () as usize,
                    prefix: $prefix,
                    opcode: $opcode,
                    argument: $argument,
                    mask: $mask,
                    latency: $latency,
                    memory: $memory,
                    reads_register: $reads,
                    writes_register: $writes,
                    flags: $flags,
                    edx: MemoryEdx::$edx,
                });
            }
        };
    }

    memory_test!(
        memory_load_bytes_dependencies_calls_and_registers,
        fc4p_008ecf80,
        0,
        0x8B,
        1,
        1,
        1,
        1,
        false,
        true,
        false,
        Encoder
    );
    memory_test!(
        word_load_bytes_dependencies_calls_and_registers,
        fc4p_008ed0b0,
        0x0F,
        0xB7,
        1,
        1,
        1,
        1,
        false,
        true,
        false,
        Boundary
    );
    memory_test!(
        word_store_bytes_dependencies_calls_and_registers,
        fc4p_008ed150,
        0x66,
        0x89,
        2,
        1,
        2,
        2,
        true,
        false,
        false,
        Reads
    );
    memory_test!(
        memory_store_bytes_dependencies_calls_and_registers,
        fc4p_008ed1f0,
        0,
        0x89,
        2,
        1,
        2,
        2,
        true,
        false,
        false,
        Reads
    );
    memory_test!(
        memory_add_bytes_dependencies_calls_and_registers,
        fc4p_008ed5e0,
        0,
        0x03,
        1,
        1,
        2,
        1,
        true,
        true,
        true,
        Boundary
    );
    memory_test!(
        word_compare_bytes_dependencies_calls_and_registers,
        fc4p_008ed860,
        0x66,
        0x3B,
        1,
        1,
        2,
        1,
        true,
        false,
        true,
        Reads
    );
    memory_test!(
        memory_compare_bytes_dependencies_calls_and_registers,
        fc4p_008ed900,
        0,
        0x3B,
        1,
        1,
        2,
        1,
        true,
        false,
        true,
        Reads
    );
    memory_test!(
        memory_subtract_bytes_dependencies_calls_and_registers,
        fc4p_008eda40,
        0,
        0x2B,
        1,
        1,
        2,
        1,
        true,
        true,
        true,
        Boundary
    );
    memory_test!(
        effective_address_bytes_dependencies_calls_and_registers,
        fc4p_008eddb0,
        0,
        0x8D,
        1,
        1,
        1,
        0,
        false,
        true,
        false,
        Mask
    );
    memory_test!(
        memory_multiply_bytes_dependencies_calls_and_registers,
        fc4p_008ede40,
        0x0F,
        0xAF,
        1,
        1,
        2,
        1,
        true,
        true,
        false,
        Reads
    );
    memory_test!(
        mmx_dword_load_bytes_dependencies_calls_and_registers,
        fc4p_008ee3d0,
        0x0F,
        0x6E,
        1,
        0x100,
        1,
        1,
        false,
        true,
        false,
        Mask
    );
    memory_test!(
        mmx_dword_store_bytes_dependencies_calls_and_registers,
        fc4p_008ee470,
        0x0F,
        0x7E,
        2,
        0x100,
        2,
        2,
        true,
        false,
        false,
        Reads
    );
    memory_test!(
        mmx_qword_load_bytes_dependencies_calls_and_registers,
        fc4p_008ee5c0,
        0x0F,
        0x6F,
        1,
        0x100,
        1,
        1,
        false,
        true,
        false,
        Mask
    );
    memory_test!(
        mmx_qword_store_bytes_dependencies_calls_and_registers,
        fc4p_008ee660,
        0x0F,
        0x7F,
        2,
        0x100,
        2,
        2,
        true,
        false,
        false,
        Reads
    );
    memory_test!(
        mmx_memory_and_bytes_dependencies_calls_and_registers,
        fc4p_008ee7b0,
        0x0F,
        0xDB,
        1,
        0x100,
        2,
        1,
        true,
        true,
        false,
        Reads
    );
    memory_test!(
        mmx_memory_maximum_bytes_dependencies_calls_and_registers,
        fc4p_008eeb30,
        0x0F,
        0xED,
        1,
        0x100,
        2,
        1,
        true,
        true,
        false,
        Reads
    );

    #[derive(Clone, Copy)]
    enum RegisterEdx {
        Destination,
        Source,
        Writes,
        Boundary,
        Xor,
    }
    struct RegisterCase {
        entry: usize,
        prefix: u8,
        opcode: u8,
        reverse: bool,
        read_destination: u32,
        read_source: u32,
        write_destination: u32,
        flags: bool,
        edx: RegisterEdx,
    }

    fn check_register(case: RegisterCase) {
        let _append = fake(0x008E_CC20, append_fake as *const () as usize);
        for destination_register in VALUES {
            for source_register in VALUES {
                for destination in DESTINATIONS {
                    let mut block = [0xCCCC_CCCC; 30];
                    let before_block = block;
                    let mut object = object(&mut block, destination, u32::MAX);
                    let this = object.as_mut_ptr() as u32;
                    let mut expected = object;
                    let (high, low) = if case.reverse {
                        (source_register, destination_register)
                    } else {
                        (destination_register, source_register)
                    };
                    let mut bytes = Vec::new();
                    if case.prefix != 0 {
                        bytes.push(case.prefix);
                    }
                    bytes.extend([
                        case.opcode,
                        0xC0u8
                            .wrapping_add((high as u8).wrapping_mul(8))
                            .wrapping_add(low as u8),
                    ]);
                    let writes = case.write_destination.wrapping_shl(destination_register)
                        | if case.flags { 0x10000 } else { 0 };
                    let reads = if matches!(case.edx, RegisterEdx::Xor) && destination_register == source_register {
                        0
                    } else {
                        case.read_destination.wrapping_shl(destination_register)
                            | case.read_source.wrapping_shl(source_register)
                    };
                    expected_record(&mut expected, &bytes, [0, reads, writes, 1]);
                    let edx = match case.edx {
                        RegisterEdx::Destination => destination_register,
                        RegisterEdx::Source => source_register,
                        RegisterEdx::Writes => writes,
                        RegisterEdx::Boundary => expected[18].wrapping_sub(40),
                        RegisterEdx::Xor => {
                            if destination_register == source_register {
                                writes
                            } else {
                                expected[18].wrapping_sub(40)
                            }
                        }
                    };
                    let result =
                        unsafe { invoke(case.entry, this, &mut [destination_register, source_register], false) };
                    check_queue(&object, expected, &block, before_block, result, edx, destination);
                }
            }
        }
    }

    macro_rules! register_test {
        ($test:ident, $entry:ident, $prefix:literal, $opcode:literal, $reverse:literal,
            $destination:literal, $source:literal, $writes:literal, $flags:literal, $edx:ident) => {
            #[test]
            fn $test() {
                check_register(RegisterCase {
                    entry: $entry as *const () as usize,
                    prefix: $prefix,
                    opcode: $opcode,
                    reverse: $reverse,
                    read_destination: $destination,
                    read_source: $source,
                    write_destination: $writes,
                    flags: $flags,
                    edx: RegisterEdx::$edx,
                });
            }
        };
    }

    register_test!(
        register_move_bytes_masks_and_queue_branches,
        fc4p_008ece20,
        0,
        0x8B,
        false,
        0,
        1,
        1,
        false,
        Boundary
    );
    register_test!(
        register_add_bytes_masks_and_queue_branches,
        fc4p_008ed530,
        0,
        0x03,
        false,
        1,
        1,
        1,
        true,
        Writes
    );
    register_test!(
        register_word_compare_bytes_masks_and_queue_branches,
        fc4p_008ed7b0,
        0x66,
        0x3B,
        false,
        1,
        1,
        0,
        true,
        Source
    );
    register_test!(
        register_subtract_bytes_masks_and_queue_branches,
        fc4p_008ed990,
        0,
        0x2B,
        false,
        1,
        1,
        1,
        true,
        Writes
    );
    register_test!(
        register_from_mmx_bytes_masks_and_queue_branches,
        fc4p_008ee270,
        0x0F,
        0x7E,
        true,
        0,
        0x100,
        1,
        false,
        Destination
    );
    register_test!(
        mmx_from_register_bytes_masks_and_queue_branches,
        fc4p_008ee320,
        0x0F,
        0x6E,
        false,
        0,
        1,
        0x100,
        false,
        Destination
    );
    register_test!(
        mmx_move_bytes_masks_and_queue_branches,
        fc4p_008ee510,
        0x0F,
        0x6F,
        false,
        0,
        0x100,
        0x100,
        false,
        Destination
    );
    register_test!(
        mmx_and_bytes_masks_and_queue_branches,
        fc4p_008ee700,
        0x0F,
        0xDB,
        false,
        0x100,
        0x100,
        0x100,
        false,
        Writes
    );
    register_test!(
        mmx_and_not_bytes_masks_and_queue_branches,
        fc4p_008ee850,
        0x0F,
        0xDF,
        false,
        0x100,
        0x100,
        0x100,
        false,
        Writes
    );
    register_test!(
        mmx_xor_equal_full_words_and_queue_branches,
        fc4p_008ee900,
        0x0F,
        0xEF,
        false,
        0x100,
        0x100,
        0x100,
        false,
        Xor
    );
    register_test!(
        mmx_add_words_bytes_masks_and_queue_branches,
        fc4p_008ee9d0,
        0x0F,
        0xFD,
        false,
        0x100,
        0x100,
        0x100,
        false,
        Writes
    );

    fn check_immediate(entry: usize, opcode: u8, bias: u8, width: usize) {
        let _append = fake(0x008E_CC20, append_fake as *const () as usize);
        for register in VALUES {
            for immediate in VALUES {
                for destination in DESTINATIONS {
                    let mut block = [0xCCCC_CCCC; 30];
                    let before_block = block;
                    let mut object = object(&mut block, destination, u32::MAX);
                    let this = object.as_mut_ptr() as u32;
                    let mut expected = object;
                    let mut bytes = vec![opcode, (register as u8).wrapping_add(bias)];
                    bytes.extend_from_slice(&immediate.to_le_bytes()[..width]);
                    let mask = 1u32.wrapping_shl(register);
                    expected_record(&mut expected, &bytes, [0, mask, mask | 0x10000, 1]);
                    let result = unsafe { invoke(entry, this, &mut [register, immediate], false) };
                    check_queue(
                        &object,
                        expected,
                        &block,
                        before_block,
                        result,
                        mask | 0x10000,
                        destination,
                    );
                }
            }
        }
    }
    #[test]
    fn and_immediate_keeps_all_four_bytes() {
        check_immediate(fc4p_008ed3c0 as *const () as usize, 0x81, 0xE0, 4);
    }
    #[test]
    fn add_immediate_keeps_all_four_bytes() {
        check_immediate(fc4p_008ed670 as *const () as usize, 0x81, 0xC0, 4);
    }
    #[test]
    fn logical_shift_uses_the_low_count_byte() {
        check_immediate(fc4p_008edad0 as *const () as usize, 0xC1, 0xE8, 1);
    }
    #[test]
    fn arithmetic_shift_uses_the_low_count_byte() {
        check_immediate(fc4p_008edb70 as *const () as usize, 0xC1, 0xF8, 1);
    }
    #[test]
    fn rotate_uses_the_low_count_byte() {
        check_immediate(fc4p_008edc10 as *const () as usize, 0xC1, 0xC0, 1);
    }

    #[test]
    fn accumulator_carry_and_decrement_preserve_unused_bytes_and_append_at_the_boundary() {
        let _append = fake(0x008E_CC20, append_fake as *const () as usize);
        for destination in DESTINATIONS {
            for register in VALUES {
                for decrement in [false, true] {
                    let mut block = [0xCCCC_CCCC; 30];
                    let before_block = block;
                    let mut object = object(&mut block, destination, u32::MAX);
                    let this = object.as_mut_ptr() as u32;
                    let mut expected = object;
                    let (entry, mut args, bytes, metadata, edx) = if decrement {
                        let mask = 1u32.wrapping_shl(register);
                        (
                            fc4p_008edd30 as *const () as usize,
                            vec![register],
                            vec![(register as u8).wrapping_add(0x48)],
                            [0, mask, mask | 0x10000, 1],
                            mask | 0x10000,
                        )
                    } else {
                        (
                            fc4p_008ed730 as *const () as usize,
                            vec![],
                            vec![0x15, 0, 0, 0, 0],
                            [0, 0x10001, 0x10001, 2],
                            0,
                        )
                    };
                    expected_record(&mut expected, &bytes, metadata);
                    let result = unsafe { invoke(entry, this, &mut args, false) };
                    check_queue(&object, expected, &block, before_block, result, edx, destination);
                }
            }
        }
    }

    #[test]
    fn appending_copies_words_in_order_when_output_overlaps_scratch_or_the_end_cursor() {
        for offset in [0, 4, 8, 12, 16, 0x20, 0x28, 0x30, 0x38, 0x40] {
            let mut object = [0xA5A5_A5A5u32; 48];
            let this = object.as_mut_ptr() as u32;
            object[3] = 0xFFFF_FFFF;
            object[16] = this + offset;
            object[18] = this + 0x100;
            let mut expected = object;
            expected[2] = 2;
            expected[3] = 0;
            expected[4] = 0xA5A5_DA8B;
            expected[8] = 0;
            expected[9] = 4;
            expected[10] = 8;
            expected[11] = 1;
            for i in 0..10 {
                expected[offset as usize / 4 + i] = expected[2 + i];
            }
            expected[16] = expected[16].wrapping_add(40);
            let result = unsafe { invoke(fc4p_008ece20 as *const () as usize, this, &mut [3, 2], false) };
            assert_eq!(object, expected);
            assert_eq!(
                result,
                Registers {
                    eax: this + 0x30,
                    ecx: expected[16],
                    edx: this + 0xD8
                }
            );
            assert!(events().is_empty());
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn flush_fake() {
        register_boundary!(flush_body, 0);
    }
    unsafe extern "fastcall" fn flush_body(registers: *mut Registers, _stack: *mut u32) {
        unsafe {
            let this = (*registers).ecx;
            log(Event::Flush(*registers, scratch(this)));
            FLUSH_LENGTH.with(|v| {
                if let Some(length) = *v.borrow() {
                    store(this + 8, length);
                }
            });
            FLUSH_OUTPUT.with(|v| {
                if let Some(output) = v.borrow_mut().take() {
                    store(this + 4, output);
                }
            });
            *registers = CALLEE_RESULT;
        }
    }
    #[unsafe(naked)]
    unsafe extern "C" fn long_jump_fake() {
        register_boundary!(long_jump_body, 8);
    }
    unsafe extern "fastcall" fn long_jump_body(registers: *mut Registers, stack: *mut u32) {
        unsafe {
            log(Event::LongJump(
                *registers,
                [stack.add(1).read(), stack.add(2).read()],
                scratch((*registers).ecx),
            ));
            *registers = CALLEE_RESULT;
        }
    }
    fn entry_registers(this: u32) -> Registers {
        Registers {
            eax: 0x1234_5678,
            ecx: this,
            edx: 0x8765_4321,
        }
    }

    fn check_placeholder(entry: usize, form: u8) {
        let _flush = fake(0x008E_CCA0, flush_fake as *const () as usize);
        for condition in VALUES {
            for length_override in [None, Some(0), Some(1), Some(9), Some(0x8000_0000), Some(u32::MAX)] {
                let mut object = [0xA5A5_A5A5; 28];
                let this = object.as_mut_ptr() as u32;
                let mut output = [0xCCCC_CCCCu32; 32];
                let output_base = output.as_mut_ptr() as u32;
                object[1] = output_base + 16;
                object[3] = 0x12FF_FFFF;
                let before = object;
                let mut expected = object;
                // The initial flush changes the output cursor. All displacement work uses the new one.
                FLUSH_OUTPUT.with(|v| *v.borrow_mut() = Some(output_base + 24));
                FLUSH_LENGTH.with(|v| *v.borrow_mut() = length_override);
                let mut patch = 0xDEAD_BEEFu32;
                let patch_pointer = &mut patch as *mut u32 as u32;
                let target = output_base.wrapping_sub(0x1234);
                let mut args = match form {
                    0 => vec![patch_pointer],
                    3 => vec![condition, target],
                    _ => vec![patch_pointer, condition],
                };
                let bytes = match form {
                    0 => vec![0xE9, 0, 0, 0, 0],
                    2 => vec![(condition as u8).wrapping_add(0x70), 0],
                    _ => vec![0x0F, (condition as u8).wrapping_add(0x80), 0, 0, 0, 0],
                };
                expected_record(&mut expected, &bytes, [3, 0, 0, 1]);
                let scratch_at_flush = scratch_expected(&expected);
                let length = length_override.unwrap_or(bytes.len() as u32);
                let count = if (length as i32) > 0 { length } else { 0 };
                let mut expected_bytes = [0xCCu8; 128];
                for i in 0..count as usize {
                    let word = expected[4 + i / 4];
                    expected_bytes[24 + i] = (word >> ((i % 4) * 8)) as u8;
                }
                let end = output_base + 24 + count;
                expected[1] = end;
                expected[2] = length;
                let displacement_size = if form == 2 { 1 } else { 4 };
                let expected_result = if form == 3 {
                    let patch_location = end - 4;
                    let relative = target.wrapping_sub(end);
                    let start = (patch_location - output_base) as usize;
                    expected_bytes[start..start + 4].copy_from_slice(&relative.to_le_bytes());
                    Registers {
                        eax: patch_location,
                        ecx: relative,
                        edx: length,
                    }
                } else {
                    Registers {
                        eax: patch_pointer,
                        ecx: end - displacement_size,
                        edx: length,
                    }
                };
                let result = unsafe { invoke(entry, this, &mut args, false) };
                assert_eq!(object, expected);
                assert_eq!(
                    unsafe { core::slice::from_raw_parts(output.as_ptr().cast::<u8>(), 128) },
                    expected_bytes
                );
                assert_eq!(result, expected_result);
                assert_eq!(
                    patch,
                    if form == 3 {
                        0xDEAD_BEEF
                    } else {
                        end - displacement_size
                    }
                );
                let second_eax = match form {
                    0 => 1,
                    2 => 0x1300_0000 | u32::from((condition as u8).wrapping_add(0x70)),
                    _ => 5,
                };
                assert_eq!(
                    events(),
                    [
                        Event::Flush(entry_registers(this), scratch_expected(&before)),
                        Event::Flush(
                            Registers {
                                eax: second_eax,
                                ecx: this,
                                edx: if form == 0 || form == 2 { 1 } else { 4 }
                            },
                            scratch_at_flush
                        ),
                    ]
                );
            }
        }
        FLUSH_LENGTH.with(|v| *v.borrow_mut() = None);
    }
    #[test]
    fn unconditional_jump_flushes_twice_and_returns_the_patch_word_address() {
        check_placeholder(fc4p_008edee0 as *const () as usize, 0);
    }
    #[test]
    fn conditional_jump_placeholder_keeps_condition_wrapping_and_patch_location() {
        check_placeholder(fc4p_008edf70 as *const () as usize, 1);
    }
    #[test]
    fn short_jump_placeholder_keeps_the_partial_eax_at_the_second_flush() {
        check_placeholder(fc4p_008ee020 as *const () as usize, 2);
    }
    #[test]
    fn long_conditional_jump_patches_a_wrapping_relative_displacement() {
        check_placeholder(fc4p_008ee0a0 as *const () as usize, 3);
    }

    #[test]
    fn long_conditional_jump_skips_a_null_patch_location() {
        let _flush = fake(0x008E_CCA0, flush_fake as *const () as usize);
        let mut object = [0xA5A5_A5A5; 28];
        let this = object.as_mut_ptr() as u32;
        object[1] = 4;
        FLUSH_LENGTH.with(|v| *v.borrow_mut() = Some(0));
        let before = object;
        let result = unsafe { invoke(fc4p_008ee0a0 as *const () as usize, this, &mut [2, 0x1234], false) };
        let mut expected = before;
        expected_record(&mut expected, &[0x0F, 0x82, 0, 0, 0, 0], [3, 0, 0, 1]);
        let record = scratch_expected(&expected);
        expected[2] = 0;
        assert_eq!(object, expected);
        assert_eq!(result, Registers { eax: 0, ecx: 0, edx: 0 });
        assert_eq!(
            events(),
            [
                Event::Flush(entry_registers(this), scratch_expected(&before)),
                Event::Flush(
                    Registers {
                        eax: 5,
                        ecx: this,
                        edx: 4
                    },
                    record
                )
            ]
        );
        FLUSH_LENGTH.with(|v| *v.borrow_mut() = None);
    }

    #[test]
    fn conditional_jump_chooses_short_at_both_signed_byte_limits_and_calls_long_beyond_them() {
        let _flush = fake(0x008E_CCA0, flush_fake as *const () as usize);
        let _long = fake(0x008E_E0A0, long_jump_fake as *const () as usize);
        for displacement in [-129i32, -128, -127, -1, 0, 1, 126, 127, 128, i32::MIN, i32::MAX] {
            for condition in VALUES {
                let mut object = [0xA5A5_A5A5; 28];
                let this = object.as_mut_ptr() as u32;
                let mut output = [0xCCu8; 32];
                let base = output.as_mut_ptr() as u32;
                object[1] = base;
                object[3] = u32::MAX;
                let before = object;
                let target = (base + 7).wrapping_add(displacement as u32);
                FLUSH_OUTPUT.with(|v| *v.borrow_mut() = Some(base + 5));
                let mut expected = object;
                expected[1] = base + 5;
                let result = unsafe {
                    invoke(
                        fc4p_008ee160 as *const () as usize,
                        this,
                        &mut [condition, target],
                        false,
                    )
                };
                let mut expected_output = [0xCCu8; 32];
                if (-128..=127).contains(&displacement) {
                    let bytes = [(condition as u8).wrapping_add(0x70), displacement as u8];
                    expected_record(&mut expected, &bytes, [3, 0, 0, 1]);
                    let record = scratch_expected(&expected);
                    expected[1] += 2;
                    expected_output[5..7].copy_from_slice(&bytes);
                    assert_eq!(
                        result,
                        Registers {
                            eax: 2,
                            ecx: base + 7,
                            edx: 2
                        }
                    );
                    assert_eq!(
                        events(),
                        [
                            Event::Flush(entry_registers(this), scratch_expected(&before)),
                            Event::Flush(
                                Registers {
                                    eax: displacement as u32,
                                    ecx: this,
                                    edx: 0
                                },
                                record
                            )
                        ]
                    );
                } else {
                    expected[2] = 0;
                    expected[3] = 0;
                    assert_eq!(result, CALLEE_RESULT);
                    assert_eq!(
                        events(),
                        [
                            Event::Flush(entry_registers(this), scratch_expected(&before)),
                            Event::LongJump(
                                Registers {
                                    eax: condition,
                                    ecx: this,
                                    edx: displacement as i8 as i32 as u32
                                },
                                [condition, target],
                                scratch_expected(&expected)
                            )
                        ]
                    );
                }
                assert_eq!(object, expected);
                assert_eq!(output, expected_output);
            }
        }
    }

    #[test]
    fn return_flushes_once_then_copies_using_the_post_flush_length_and_cursor() {
        let _flush = fake(0x008E_CCA0, flush_fake as *const () as usize);
        for length_override in [None, Some(0), Some(3), Some(0x8000_0000), Some(u32::MAX)] {
            let mut object = [0xA5A5_A5A5; 28];
            let this = object.as_mut_ptr() as u32;
            let mut output = [0xCCu8; 32];
            let base = output.as_mut_ptr() as u32;
            object[1] = base;
            object[3] = u32::MAX;
            FLUSH_OUTPUT.with(|v| *v.borrow_mut() = Some(base + 5));
            FLUSH_LENGTH.with(|v| *v.borrow_mut() = length_override);
            let mut expected = object;
            expected_record(&mut expected, &[0xC3], [3, 0, 0, 4]);
            let record = scratch_expected(&expected);
            let length = length_override.unwrap_or(1);
            let count = if (length as i32) > 0 { length } else { 0 };
            expected[1] = base + 5 + count;
            expected[2] = length;
            let mut expected_output = [0xCCu8; 32];
            for i in 0..count as usize {
                expected_output[5 + i] = (expected[4 + i / 4] >> (i % 4 * 8)) as u8;
            }
            let result = unsafe { invoke(fc4p_008ee200 as *const () as usize, this, &mut [], false) };
            assert_eq!(object, expected);
            assert_eq!(output, expected_output);
            assert_eq!(
                result,
                Registers {
                    eax: count,
                    ecx: expected[1],
                    edx: length
                }
            );
            assert_eq!(
                events(),
                [Event::Flush(
                    Registers {
                        eax: 0,
                        ecx: this,
                        edx: 0x8765_4321
                    },
                    record
                )]
            );
        }
        FLUSH_LENGTH.with(|v| *v.borrow_mut() = None);
    }
}
