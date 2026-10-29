// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lifetime and copying of intersection records and insertion into the cell-record hash table.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};

game_fn! { #[allow(dead_code)] fn free_storage(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn assign_records(first: *mut u8, last: *mut u8, output: *mut u8, scratch: *mut u32, flags: u32) -> u64 = "C" 0x0062_7220; }
game_fn! { #[allow(dead_code)] fn assign_piece_vector(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0062_C1E0; }
game_fn! { #[allow(dead_code)] fn resize_table(this: *mut u8, count: u32) -> u64 = "thiscall" 0x004F_0940; }
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn construct_cell(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0062_A1D0; }

layout! {
    /// Bucket range and live-node count of the cell-record hash table.
    struct CellTable {
        0x04 => buckets / set_buckets: *mut u8,
        0x08 => buckets_end / set_buckets_end: *mut u8,
        0x10 => count / set_count: u32,
    }
}

// Retain ECX as well as the ordinary return registers between helpers that leave it live.
pub(super) unsafe fn call_registers(c: &mut RawCall) {
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
            "4:", in("edi") c as *mut RawCall, clobber_abi("C"),
        );
    }
}

unsafe fn release(c: &mut RawCall, pointer: u32) {
    let args = [pointer];
    c.target = crate::abi::target(0x0090_CF63);
    c.args = args.as_ptr();
    c.count = 1;
    c.caller_pops = 1;
    unsafe { call_registers(c) };
}

unsafe fn destroy_vector(this: *mut u8, frame: *mut u32, stride: usize, member: usize) -> u64 {
    unsafe {
        let mut c = RawCall::new(Conv::Cdecl, 0, this as u32, &[]);
        c.eax = *frame.add(1);
        c.edx = *frame;
        let mut record: *mut u8 = read(this, 0);
        let end: *mut u8 = read(this, 4);
        while record != end {
            c.eax = read(record, member);
            if c.eax != 0 {
                let p = c.eax;
                release(&mut c, p);
            }
            record = record.wrapping_add(stride);
        }
        let storage: u32 = read(this, 0);
        if storage != 0 {
            release(&mut c, storage);
        }
        registers(c.eax, c.edx)
    }
}

register_port!(
    0x0062_BE40,
    fc4p_0062be40,
    "DestroyIntersectionRecordVector",
    destroy_intersections,
    "ret"
);
unsafe extern "fastcall" fn destroy_intersections(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe { destroy_vector(this, frame, 0x28, 0x1C) }
}

register_port!(
    0x0062_BF20,
    fc4p_0062bf20,
    "DestroyIntersectionPieceVector",
    destroy_pieces,
    "ret"
);
unsafe extern "fastcall" fn destroy_pieces(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe { destroy_vector(this, frame, 0x50, 0x44) }
}

register_port!(
    0x0062_BE80,
    fc4p_0062be80,
    "EraseIntersectionRecordRange",
    erase_intersections,
    "ret 8"
);
unsafe extern "fastcall" fn erase_intersections(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let first = *frame.add(3);
        let last = *frame.add(4);
        let end: u32 = read(this, 4);
        // The copy helper receives the caller's first argument word as its scratch destination.
        let args = [last, end, first, frame.add(3) as u32, 0];
        let mut c = RawCall::new(Conv::Cdecl, crate::abi::target(0x0062_7220), frame.add(3) as u32, &args);
        c.eax = end;
        c.edx = last;
        call_registers(&mut c);
        let new_end = c.eax;
        let end: u32 = read(this, 4);
        let mut cursor = new_end;
        if cursor != end {
            while cursor != end {
                c.eax = read(cursor as *mut u8, 0x1C);
                if c.eax != 0 {
                    let p = c.eax;
                    release(&mut c, p);
                }
                cursor = cursor.wrapping_add(0x28);
                c.eax = cursor;
            }
            c.eax = *frame.add(3);
        } else {
            c.eax = first;
        }
        write(this, 4, new_end);
        registers(c.eax, c.edx)
    }
}

register_port!(
    0x0062_C510,
    fc4p_0062c510,
    "AssignIntersectionRecordRange",
    copy_intersections,
    "ret"
);
unsafe extern "fastcall" fn copy_intersections(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let mut source = *frame.add(3) as *mut u8;
        let last = *frame.add(4);
        let mut destination = *frame.add(5) as *mut u8;
        let distance = last.wrapping_sub(source as u32) as i32;
        let high = (((i64::from(distance) * 0x6666_6667) >> 32) as i32) >> 4;
        let count = high.wrapping_add(((high as u32) >> 31) as i32);
        let mut edx = high as u32;
        for _ in 0..count {
            let mut eax = 0;
            for offset in (0..0x18).step_by(4) {
                let word: u32 = read(source, offset);
                if offset == 0xC {
                    eax = word;
                }
                if offset == 0x14 {
                    edx = word;
                }
                write(destination, offset, word);
            }
            eax = (eax & !0xFF) | u32::from(read::<u8>(source, 0x18));
            write(destination, 0x18, eax as u8);
            let args = [source.wrapping_add(0x1C) as u32];
            let mut c = RawCall::new(
                Conv::Thiscall,
                crate::abi::target(0x0062_C1E0),
                destination.wrapping_add(0x1C) as u32,
                &args,
            );
            c.eax = eax;
            c.edx = edx;
            call_raw(&mut c);
            edx = c.edx;
            source = source.wrapping_add(0x28);
            destination = destination.wrapping_add(0x28);
        }
        registers(destination as u32, edx)
    }
}

register_port!(
    0x0062_C590,
    fc4p_0062c590,
    "DestroyNestedIntersectionRecordRange",
    destroy_nested,
    "ret"
);
unsafe extern "fastcall" fn destroy_nested(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let mut c = RawCall::new(Conv::Cdecl, 0, _this as u32, &[]);
        c.eax = *frame.add(1);
        c.edx = *frame;
        let mut record = *frame.add(3) as *mut u8;
        let end = *frame.add(4) as *mut u8;
        while record != end {
            let mut piece: *mut u8 = read(record, 0x1C);
            let pieces_end: *mut u8 = read(record, 0x20);
            while piece != pieces_end {
                c.eax = read(piece, 0x44);
                if c.eax != 0 {
                    let p = c.eax;
                    release(&mut c, p);
                }
                piece = piece.wrapping_add(0x50);
            }
            c.eax = read(record, 0x1C);
            if c.eax != 0 {
                let p = c.eax;
                release(&mut c, p);
            }
            record = record.wrapping_add(0x28);
        }
        registers(c.eax, c.edx)
    }
}

port! {
    /// Resize for count plus one, prepend a copied cell record to its unsigned-key bucket, and return
    /// the node's value. ECX is the bucket's previous head; EDX is retained from the final helper.
    #[port(address = 0x0062_C160, owner = "networktool", name = "InsertCellRecordNode", naked)]
    pub unsafe extern "C" fn fc4p_0062c160() {
        core::arch::naked_asm!(
            "push eax", "push edx", "push ecx", "mov edx, esp", "call {body}",
            "pop ecx", "add esp, 8", "ret 4", body = sym insert_cell,
        )
    }
}
unsafe extern "fastcall" fn insert_cell(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let table = CellTable::from_ptr(this);
        let args = [table.count().wrapping_add(1)];
        let mut c = RawCall::new(Conv::Thiscall, crate::abi::target(0x004F_0940), this as u32, &args);
        c.eax = args[0];
        c.edx = *frame.add(1);
        call_raw(&mut c);
        let buckets = table.buckets();
        let bucket_count = ((table.buckets_end() as u32).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
        let source = *frame.add(4) as *mut u8;
        let key = read::<u32>(source, 0);
        let index: u32;
        // Keep unsigned division and its processor exception for a zero bucket count. This also
        // avoids introducing runtime panic support into the DLL.
        core::arch::asm!(
            "div {count:e}", count = in(reg) bucket_count,
            inout("eax") key => _, inout("edx") 0u32 => index,
            options(nomem, nostack),
        );
        let previous: u32 = read(buckets, index.wrapping_mul(4) as usize);
        let args = [0x104];
        c.target = crate::abi::target(0x0090_CF54);
        c.eax = buckets as u32;
        c.ecx = previous;
        c.edx = index;
        c.args = args.as_ptr();
        c.count = 1;
        c.caller_pops = 1;
        call_raw(&mut c);
        let node = c.eax as *mut u8;
        let value = node.wrapping_add(4);
        write(node, 0, 0u32);
        if !value.is_null() {
            let key: u32 = read(source, 0);
            write(value, 0, key);
            let args = [source.wrapping_add(4) as u32];
            c.target = crate::abi::target(0x0062_A1D0);
            c.eax = args[0];
            c.ecx = value.wrapping_add(4) as u32;
            c.edx = key;
            c.args = args.as_ptr();
            c.count = 1;
            c.caller_pops = 0;
            call_raw(&mut c);
        }
        write(node, 0, previous);
        write(table.buckets(), index.wrapping_mul(4) as usize, node);
        table.set_count(table.count().wrapping_add(1));
        *frame = previous;
        registers(value as u32, c.edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0062be40::PORT,
    fc4p_0062be80::PORT,
    fc4p_0062bf20::PORT,
    fc4p_0062c160::PORT,
    fc4p_0062c510::PORT,
    fc4p_0062c590::PORT,
];

#[cfg(test)]
mod tests;
