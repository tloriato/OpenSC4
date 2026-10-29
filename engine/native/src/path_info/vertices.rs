// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Single-vertex insertion into the game's vector storage.

use super::*;

layout! {
    /// The three-word header shared by vertex vectors.
    pub struct VertexVector {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
        0x08 => capacity / set_capacity: *mut u32,
    }
}

game_fn! {
    #[allow(dead_code)]
    fn copy_backward(first: *mut u32, last: *mut u32, destination: *mut u32, out: *mut u32, marker: u32)
        = "C" 0x004F_A8B0;
}
game_fn! {
    fn grow_insert(this: *mut u8, position: *mut u32, value: *const u32, out: *mut u32, count: u32, marker: u32)
        = "thiscall" 0x0050_C3B0;
}

port! {
    /// Inserts one XYZ vertex, growing through the game's vector routine only when capacity is exhausted.
    /// The entry preserves the original temporary storage and the caller's argument slot passed to helpers.
    #[port(address = 0x0053_F940, owner = "pathinfo", name = "insert one XYZ vertex", naked)]
    pub unsafe extern "C" fn fc4p_0053f940() {
        core::arch::naked_asm!("sub esp, 24", "lea edx, [esp + 24]", "call {}", "add esp, 24", "ret 8",
            sym insert_vertex);
    }
}

unsafe extern "fastcall" fn insert_vertex(this: *mut u8, stack: *mut u32) -> *mut u32 {
    unsafe {
        let vector = VertexVector::from_ptr(this);
        let position = read_unaligned(stack.add(1)) as *mut u32;
        let index = (position as u32).wrapping_sub(vector.begin() as u32) as i32 / 12;
        let end = vector.end();
        if end == vector.capacity() {
            let value = read_unaligned(stack.add(2)) as *const u32;
            grow_insert(this, position, value, stack.add(2), 1, 0);
        } else if position == end {
            if !end.is_null() {
                let value = read_unaligned(stack.add(2)) as *const u32;
                write_unaligned(end, read_unaligned(value));
                write_unaligned(end.add(1), read_unaligned(value.add(1)));
                write_unaligned(end.add(2), read_unaligned(value.add(2)));
            }
            vector.set_end(vector.end().wrapping_add(3));
        } else {
            if !end.is_null() {
                write_unaligned(end, read_unaligned(end.sub(3)));
                write_unaligned(end.add(1), read_unaligned(end.sub(2)));
                write_unaligned(end.add(2), read_unaligned(end.sub(1)));
            }
            let end = vector.end().wrapping_add(3);
            vector.set_end(end);
            let value = read_unaligned(stack.add(2)) as *const u32;
            let temporary = stack.sub(3);
            write_unaligned(temporary, read_unaligned(value));
            write_unaligned(temporary.add(1), read_unaligned(value.add(1)));
            write_unaligned(temporary.add(2), read_unaligned(value.add(2)));
            let args = [
                position as u32,
                end.wrapping_sub(6) as u32,
                end.wrapping_sub(3) as u32,
                stack.add(2) as u32,
                0,
            ];
            let mut call = RawCall::new(
                Conv::Cdecl,
                crate::abi::target(0x004F_A8B0),
                end.wrapping_sub(3) as u32,
                &args,
            );
            call.eax = end.wrapping_sub(6) as u32;
            call.edx = stack.add(2) as u32;
            call_raw(&mut call);
            let x = read_unaligned(temporary);
            let y = read_unaligned(temporary.add(1));
            let z = read_unaligned(temporary.add(2));
            write_unaligned(position.add(1), y);
            write_unaligned(position.add(2), z);
            write_unaligned(position, x);
        }
        vector.begin().wrapping_offset(index.wrapping_mul(3) as isize)
    }
}

pub const PORTS: &[Port] = &[fc4p_0053f940::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, PartialEq)]
    enum Call {
        Backward(usize, usize, usize, u32, u32),
        Grow(usize, usize, usize, u32, u32, u32),
    }
    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static NEW_BEGIN: Cell<usize> = const { Cell::new(0) };
    }
    fn take_calls() -> Vec<Call> {
        CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
    }

    extern "C" fn backward(first: *mut u32, last: *mut u32, destination: *mut u32, out: *mut u32, marker: u32) {
        unsafe {
            CALLS.with(|calls| {
                calls.borrow_mut().push(Call::Backward(
                    first as usize,
                    last as usize,
                    destination as usize,
                    read_unaligned(out),
                    marker,
                ))
            });
            assert_eq!(
                [
                    read_unaligned(destination),
                    read_unaligned(destination.add(1)),
                    read_unaligned(destination.add(2))
                ],
                [7, 8, 9]
            );
            let mut last = last;
            let mut destination = destination;
            while first != last {
                last = last.sub(3);
                destination = destination.sub(3);
                for i in 0..3 {
                    write_unaligned(destination.add(i), read_unaligned(last.add(i)));
                }
            }
            write_unaligned(out, 0xFEED_FACE);
        }
    }

    extern "thiscall" fn grow(
        this: *mut u8,
        position: *mut u32,
        value: *const u32,
        out: *mut u32,
        count: u32,
        marker: u32,
    ) {
        unsafe {
            CALLS.with(|calls| {
                calls.borrow_mut().push(Call::Grow(
                    this as usize,
                    position as usize,
                    value as usize,
                    read_unaligned(out),
                    count,
                    marker,
                ))
            });
            write_unaligned(out, 0xFEED_FACE);
            let begin = NEW_BEGIN.with(|p| p.get() as *mut u32);
            let vector = VertexVector::from_ptr(this);
            vector.set_begin(begin);
            vector.set_end(begin.add(3));
            vector.set_capacity(begin.add(12));
        }
    }

    #[test]
    fn insertion_with_spare_capacity_copies_raw_words_and_uses_backward_helper_only_in_the_middle() {
        let _backward = fake(0x004F_A8B0, backward as *const () as usize);
        for insertion in 0..=3 {
            let mut data = [0xA5A5_A5A5u32; 18];
            data[..9].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
            let mut header = [0u32; 3];
            let vector = unsafe { VertexVector::from_ptr(header.as_mut_ptr().cast()) };
            vector.set_begin(data.as_mut_ptr());
            vector.set_end(data.as_mut_ptr().wrapping_add(9));
            vector.set_capacity(data.as_mut_ptr().wrapping_add(18));
            let value = [0x7F80_0001u32, 0xFFC0_1234, 0x8000_0000];
            let position = data.as_mut_ptr().wrapping_add(insertion * 3);
            let result = unsafe {
                call_thiscall_with_eax(
                    fc4p_0053f940 as *const () as usize,
                    vector.as_ptr(),
                    &[position as u32, value.as_ptr() as u32],
                    0xA5A5_A5A5,
                )
            };
            assert_eq!(result.0, position as u32);
            let mut expected = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
            expected.splice(insertion * 3..insertion * 3, value);
            assert_eq!(&data[..12], expected);
            assert_eq!(&data[12..], &[0xA5A5_A5A5; 6]);
            assert_eq!(vector.end(), data.as_mut_ptr().wrapping_add(12));
            let calls = take_calls();
            if insertion == 3 {
                assert!(calls.is_empty());
            } else {
                assert_eq!(
                    calls,
                    [Call::Backward(
                        position as usize,
                        data.as_ptr() as usize + 24,
                        data.as_ptr() as usize + 36,
                        value.as_ptr() as u32,
                        0
                    )]
                );
            }
        }
    }

    #[test]
    fn middle_insertion_copies_an_aliased_value_after_duplicating_the_tail() {
        let _backward = fake(0x004F_A8B0, backward as *const () as usize);
        let mut data = [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 0xA5, 0xA5, 0xA5, 0, 0, 0];
        let mut header = [0u32; 3];
        let vector = unsafe { VertexVector::from_ptr(header.as_mut_ptr().cast()) };
        vector.set_begin(data.as_mut_ptr());
        vector.set_end(data.as_mut_ptr().wrapping_add(9));
        vector.set_capacity(data.as_mut_ptr().wrapping_add(15));
        let position = data.as_mut_ptr().wrapping_add(3);
        let value = data.as_ptr().wrapping_add(9);
        unsafe {
            call_thiscall_with_eax(
                fc4p_0053f940 as *const () as usize,
                vector.as_ptr(),
                &[position as u32, value as u32],
                0,
            )
        };
        assert_eq!(&data[..12], &[1, 2, 3, 7, 8, 9, 4, 5, 6, 7, 8, 9]);
        assert_eq!(take_calls().len(), 1);
    }

    #[test]
    fn exhausted_capacity_calls_game_growth_and_recomputes_return_from_the_new_beginning() {
        let _grow = fake(0x0050_C3B0, grow as *const () as usize);
        for index in [-1i32, 0, 1, 2] {
            let mut data = [1u32, 2, 3, 4, 5, 6];
            let mut replacement = [0u32; 12];
            let mut header = [0u32; 3];
            let vector = unsafe { VertexVector::from_ptr(header.as_mut_ptr().cast()) };
            vector.set_begin(data.as_mut_ptr());
            vector.set_end(data.as_mut_ptr().wrapping_add(6));
            vector.set_capacity(vector.end());
            NEW_BEGIN.with(|p| p.set(replacement.as_mut_ptr() as usize));
            let value = [7u32, 8, 9];
            let position = data.as_mut_ptr().wrapping_offset(index.wrapping_mul(3) as isize);
            let result = unsafe {
                call_thiscall_with_eax(
                    fc4p_0053f940 as *const () as usize,
                    vector.as_ptr(),
                    &[position as u32, value.as_ptr() as u32],
                    0,
                )
            };
            assert_eq!(
                result.0,
                replacement.as_ptr().wrapping_offset(index.wrapping_mul(3) as isize) as u32
            );
            assert_eq!(
                take_calls(),
                [Call::Grow(
                    vector.as_ptr() as usize,
                    position as usize,
                    value.as_ptr() as usize,
                    value.as_ptr() as u32,
                    1,
                    0
                )]
            );
            assert_eq!(data, [1, 2, 3, 4, 5, 6]);
        }
    }
}
