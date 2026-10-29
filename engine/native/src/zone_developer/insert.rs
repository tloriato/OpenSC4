// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Insertion into a vector of twelve-byte integer-vector records.

use super::{registers, Port};
use crate::abi::{Conv, RawCall};
use core::{
    mem::MaybeUninit,
    ptr::{read_unaligned, write_unaligned},
};

layout! {
    /// Begin, initialized end and allocated end of the record storage.
    struct RecordVector {
        0x00 => begin / set_begin: u32,
        0x04 => end / set_end: u32,
        0x08 => limit / set_limit: u32,
    }
}

game_fn! { #[allow(dead_code)] fn copy_value(this: *mut u8, value: u32) -> u64 = "thiscall" 0x004B_03E0; }
game_fn! { #[allow(dead_code)] fn copy_records(begin: u32, end: u32, out: u32, context: *mut u32) -> u64 = "C" 0x0071_2330; }
game_fn! { #[allow(dead_code)] fn copy_backward(begin: u32, end: u32, out: u32, context: *mut u32, tag: u32) -> u64 = "C" 0x0072_C890; }
game_fn! { #[allow(dead_code)] fn fill_records(begin: u32, end: u32, value: u32) -> u64 = "C" 0x0072_C860; }
game_fn! { #[allow(dead_code)] fn construct_records(out: u32, count: u32, value: u32, context: *mut u32) -> u64 = "C" 0x005C_E080; }
game_fn! { #[allow(dead_code)] fn free_value(value: u32) -> u64 = "C" 0x0090_CF63; }
game_fn! {
    #[allow(dead_code)]
    fn grow_records(this: *mut u8, position: u32, value: u32, context: *mut u32, count: u32, tag: u32) -> u64
        = "thiscall" 0x005D_0010;
}

unsafe fn invoke(address: u32, convention: Conv, ecx: u32, eax: u32, edx: u32, args: &[u32]) -> RawCall {
    let mut call = RawCall::new(convention, crate::abi::target(address), ecx, args);
    call.eax = eax;
    call.edx = edx;
    unsafe { registers::call(&mut call) };
    call
}

/// Signed division by twelve, together with the shifted high product kept in EDX.
fn divide_records(bytes: u32) -> (u32, u32) {
    let shifted = ((i64::from(bytes as i32) * 0x2AAA_AAABi64) >> 33) as i32;
    let quotient = shifted.wrapping_add((shifted as u32 >> 31) as i32);
    (quotient as u32, shifted as u32)
}

port! {
    /// Insert count copies of one record, using the game's copy, fill and growth helpers.
    /// The caller's count word is scratch space for those helpers. Preserve it and the final ECX.
    #[port(address = 0x0073_3450, owner = "zonedeveloper", name = "InsertRecordCopies", naked)]
    pub unsafe extern "C" fn fc4p_00733450() {
        core::arch::naked_asm!(
            "push edx", "push eax", "push ecx", "mov edx, esp", "call {body}",
            "mov ecx, [esp]", "add esp, 12", "ret 12", body = sym insert_records,
        );
    }
}

unsafe extern "fastcall" fn insert_records(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let vector = RecordVector::from_ptr(this);
        let position = read_unaligned(frame.add(4));
        let count = frame.add(5);
        let value = read_unaligned(frame.add(6));
        let mut result = RawCall::new(Conv::Thiscall, 0, this as u32, &[]);
        result.eax = read_unaligned(frame.add(1));
        result.edx = read_unaligned(frame.add(2));
        if read_unaligned(count) != 0 {
            let (capacity, capacity_high) = divide_records(vector.limit().wrapping_sub(vector.end()));
            if capacity < read_unaligned(count) {
                result = invoke(
                    0x005D_0010,
                    Conv::Thiscall,
                    this as u32,
                    position,
                    value,
                    &[position, value, count as u32, read_unaligned(count), 0],
                );
            } else {
                let mut temporary = MaybeUninit::<[u32; 3]>::uninit();
                let temporary_ptr = temporary.as_mut_ptr() as u32;
                invoke(
                    0x004B_03E0,
                    Conv::Thiscall,
                    temporary_ptr,
                    capacity,
                    capacity_high,
                    &[value],
                );
                let old_end = vector.end();
                let distance = old_end.wrapping_sub(position);
                let (tail, _) = divide_records(distance);
                let n = read_unaligned(count);
                if tail > n {
                    let bytes = n.wrapping_mul(12);
                    let tail_begin = old_end.wrapping_sub(bytes);
                    write_unaligned(count, tail_begin);
                    result = invoke(
                        0x0071_2330,
                        Conv::Cdecl,
                        distance,
                        tail_begin,
                        count as u32,
                        &[tail_begin, old_end, old_end, count as u32],
                    );
                    vector.set_end(vector.end().wrapping_add(bytes));
                    let middle = read_unaligned(count);
                    result = invoke(
                        0x0072_C890,
                        Conv::Cdecl,
                        middle,
                        count as u32,
                        result.edx,
                        &[position, middle, old_end, count as u32, 0],
                    );
                    result = invoke(
                        0x0072_C860,
                        Conv::Cdecl,
                        result.ecx,
                        result.eax,
                        temporary_ptr,
                        &[position, position.wrapping_add(bytes), temporary_ptr],
                    );
                } else {
                    let remaining = n.wrapping_sub(tail);
                    write_unaligned(count, remaining);
                    result = invoke(
                        0x005C_E080,
                        Conv::Cdecl,
                        count as u32,
                        remaining,
                        temporary_ptr,
                        &[old_end, remaining, temporary_ptr, count as u32],
                    );
                    let new_end = vector.end().wrapping_add(read_unaligned(count).wrapping_mul(12));
                    vector.set_end(new_end);
                    result = invoke(
                        0x0071_2330,
                        Conv::Cdecl,
                        count as u32,
                        new_end,
                        result.edx,
                        &[position, old_end, new_end, count as u32],
                    );
                    let bytes = tail.wrapping_mul(12);
                    vector.set_end(vector.end().wrapping_add(bytes));
                    result = invoke(
                        0x0072_C860,
                        Conv::Cdecl,
                        result.ecx,
                        temporary_ptr,
                        bytes,
                        &[position, old_end, temporary_ptr],
                    );
                }
                result.eax = read_unaligned(temporary.as_ptr().cast::<u32>());
                if result.eax != 0 {
                    result = invoke(
                        0x0090_CF63,
                        Conv::Cdecl,
                        result.ecx,
                        result.eax,
                        result.edx,
                        &[result.eax],
                    );
                }
            }
        }
        write_unaligned(frame, result.ecx);
        (u64::from(result.edx) << 32) | u64::from(result.eax)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00733450::PORT];

#[cfg(test)]
mod tests;
