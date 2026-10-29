// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction and copying of a solution record, and clearing its chained bucket storage.

use super::*;
use crate::abi::{Conv, RawCall, call_raw};

game_fn! { #[allow(dead_code)] fn copy_small(this: *mut u8, source: *mut u8) = "thiscall" 0x0062_5190; }
game_fn! { #[allow(dead_code)] fn copy_words(this: *mut u8, source: *mut u8) = "thiscall" 0x007C_ED60; }
game_fn! { #[allow(dead_code)] fn copy_pairs(this: *mut u8, source: *mut u8) = "thiscall" 0x0062_4D60; }
game_fn! { #[allow(dead_code)] fn copy_first_table(this: *mut u8, source: *mut u8) = "thiscall" 0x0062_6430; }
game_fn! { #[allow(dead_code)] fn copy_second_table(this: *mut u8, source: *mut u8) = "thiscall" 0x0062_65B0; }
game_fn! { #[allow(dead_code)] fn copy_string(this: *mut u8, source: *mut u8, start: u32, length: u32, scratch: *mut *mut u8) = "thiscall" 0x0040_89B0; }
game_fn! { #[allow(dead_code)] fn copy_vector(this: *mut u8, source: *mut u8) = "thiscall" 0x0045_9E80; }
game_fn! { fn initialize_table(this: *mut u8, buckets: u32) -> u64 = "thiscall" 0x0082_9BC0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn destroy_value(this: *mut u8) = "thiscall" 0x0062_6160; }
game_fn! { #[allow(dead_code)] fn free(pointer: *mut u8) = "C" 0x0090_CF63; }

layout! {
    /// Bucket vector and live-node count; each node's first word links to the next node.
    struct BucketTable {
        0x04 => begin / set_begin: *mut u8,
        0x08 => end / set_end: *mut u8,
        0x10 => count / set_count: u32,
    }
}

unsafe fn copy_word(destination: *mut u8, source: *mut u8, offset: usize) {
    unsafe { write(destination, offset, read::<u32>(source, offset)) }
}

unsafe fn zero_table(table: *mut u8) {
    unsafe {
        for offset in [4, 8, 12, 16] {
            write(table, offset, 0u32);
        }
    }
}

port! {
    /// Copy scalars and owned members in order, retaining each member call's original stack depth.
    #[port(address = 0x0062_A1D0, owner = "networktool", name = "CopyNetworkSolutionRecord", naked)]
    pub unsafe extern "C" fn fc4p_0062a1d0() {
        core::arch::naked_asm!(
            "push ebx", "push esi", "push edi", "mov esi, ecx", "mov edi, [esp + 16]",
            "push 0", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 4]", "push ebx", "lea ecx, [esi + 4]", "call eax",
            "push 1", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 40]", "push ebx", "lea ecx, [esi + 40]", "call eax",
            "push 2", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 52]", "push ebx", "lea ecx, [esi + 52]", "call eax",
            "push 3", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 76]", "push ebx", "lea ecx, [esi + 76]", "call eax",
            "push 4", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 88]", "push ebx", "lea ecx, [esi + 88]", "call eax",
            "push 5", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 100]", "push ebx", "lea ecx, [esi + 100]", "call eax",
            "push 6", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [esp + 16]", "push ebx", "push -1", "push 0", "push edx", "lea ecx, [esi + 132]", "call eax",
            "push 7", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 156]", "push ebx", "lea ecx, [esi + 156]", "call eax",
            "push 8", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 184]", "push ebx", "lea ecx, [esi + 184]", "call eax",
            "push 9", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "lea ebx, [edi + 208]", "push ebx", "lea ecx, [esi + 208]", "call eax",
            "push 10", "mov ecx, esi", "mov edx, edi", "call {stage}",
            "pop edi", "pop esi", "pop ebx", "ret 4", stage = sym copy_stage,
        );
    }
}
unsafe extern "fastcall" fn copy_stage(this: *mut u8, source: *mut u8, stage: u32) -> u64 {
    unsafe {
        let target = match stage {
            0 => {
                copy_word(this, source, 0);
                0x0062_5190
            }
            1 => {
                for offset in (0x10..=0x24).step_by(4) {
                    copy_word(this, source, offset);
                }
                0x007C_ED60
            }
            2 => 0x007C_ED60,
            3 => {
                copy_word(this, source, 0x40);
                copy_word(this, source, 0x44);
                write(this, 0x48, read::<u8>(source, 0x48));
                0x007C_ED60
            }
            4 => 0x0062_4D60,
            5 => {
                zero_table(this.add(0x64));
                0x0062_6430
            }
            6 => {
                copy_word(this, source, 0x78);
                copy_word(this, source, 0x7C);
                let string = source.wrapping_add(0x80);
                let string = if string.is_null() {
                    string
                } else {
                    string.wrapping_add(4)
                };
                return registers(crate::abi::target(0x0040_89B0) as u32, string as u32);
            }
            7 => {
                write(this, 0x80, 0x00A8_0810u32);
                write(this, 0x90, 0u32);
                copy_word(this, source, 0x94);
                copy_word(this, source, 0x98);
                0x0045_9E80
            }
            8 => {
                for offset in (0xA8..=0xB4).step_by(4) {
                    copy_word(this, source, offset);
                }
                0x0045_9E80
            }
            9 => {
                for offset in [0xC4, 0xC8, 0xCC] {
                    copy_word(this, source, offset);
                }
                zero_table(this.add(0xD0));
                0x0062_65B0
            }
            _ => {
                copy_word(this, source, 0xE4);
                write(this, 0xE8, read::<u8>(source, 0xE8));
                write(this, 0xE9, read::<u8>(source, 0xE9));
                copy_word(this, source, 0xEC);
                copy_word(this, source, 0xF0);
                let edx = read::<u32>(source, 0xF4);
                write(this, 0xF4, edx);
                copy_word(this, source, 0xF8);
                return registers(this as u32, edx);
            }
        };
        registers(crate::abi::target(target) as u32, 0)
    }
}

register_port!(
    0x0062_A3C0,
    fc4p_0062a3c0,
    "ClearNetworkSolutionBuckets",
    clear_buckets,
    "ret"
);
unsafe extern "fastcall" fn clear_buckets(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let table = BucketTable::from_ptr(this);
        let mut count = (table.end() as u32).wrapping_sub(table.begin() as u32) as i32 >> 2;
        let mut index = 0u32;
        let mut edx = frame.read();
        while index < count as u32 {
            let mut node: *mut u8 = read(table.begin(), index.wrapping_mul(4) as usize);
            while !node.is_null() {
                let next: *mut u8 = read(node, 0);
                let mut c = RawCall::new(
                    Conv::Thiscall,
                    crate::abi::target(0x0062_6160),
                    node.wrapping_add(8) as u32,
                    &[],
                );
                c.eax = count as u32;
                c.edx = edx;
                call_raw(&mut c);
                let args = [node as u32];
                c.target = crate::abi::target(0x0090_CF63);
                c.args = args.as_ptr();
                c.count = 1;
                c.caller_pops = 1;
                call_raw(&mut c);
                edx = c.edx;
                // The next node sees the delete result in EAX.
                count = c.eax as i32;
                node = next;
            }
            edx = table.begin() as u32;
            write(table.begin(), index.wrapping_mul(4) as usize, 0u32);
            count = (table.end() as u32).wrapping_sub(table.begin() as u32) as i32 >> 2;
            index = index.wrapping_add(1);
        }
        table.set_count(0);
        registers(count as u32, edx)
    }
}

port! {
    /// Initialize owned vectors and two 100-bucket tables, allocate the empty string, and set the four
    /// terminal indices to -1. Fields without an initial value, including flag padding, stay untouched.
    #[port(address = 0x0062_A480, owner = "networktool", name = "ConstructNetworkSolutionRecord")]
    pub unsafe extern "thiscall" fn fc4p_0062a480(this: *mut u8) -> u64 {
        unsafe {
            for offset in [4, 8, 12, 16, 20, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C] { write(this, offset, 0u32); }
            write(this, 0x48, 0u8);
            for offset in [0x4C, 0x50, 0x54, 0x58, 0x5C, 0x60] { write(this, offset, 0u32); }
            zero_table(this.add(0x64));
            initialize_table(this.add(0x64), 100);
            write(this, 0x78, 0u32);
            for offset in [0x84, 0x88, 0x8C] { write(this, offset, 0u32); }
            let string = allocate(8);
            write(this, 0x84, string);
            write(this, 0x88, string);
            write(this, 0x8C, string.wrapping_add(8));
            write(string, 0, 0u8);
            write(this, 0x80, 0x00A8_0810u32);
            for offset in (0x90..=0xC0).step_by(4) { write(this, offset, 0u32); }
            write(this, 0xC8, 0u32);
            write(this, 0xCC, 0u32);
            zero_table(this.add(0xD0));
            let result = initialize_table(this.add(0xD0), 100);
            for offset in [0xEC, 0xF0, 0xF4, 0xF8] { write(this, offset, u32::MAX); }
            write(this, 0xE4, 0u32);
            write(this, 0xE8, 0u8);
            write(this, 0xE9, 0u8);
            registers(this as u32, (result >> 32) as u32)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0062a1d0::PORT, fc4p_0062a3c0::PORT, fc4p_0062a480::PORT];

#[cfg(test)]
mod tests;
