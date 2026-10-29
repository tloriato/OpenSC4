// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Segment clipping, path length and vertex copying.

use super::*;
use crate::x87::F80;

game_data! { static zero: f32 = 0x00A8_1054; }

// Stored operands must remain memory operands, including when they are signaling NaNs.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut result = F80::ZERO;
            unsafe {
                core::arch::asm!("fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{result}]",
                    value = in(reg) &value, operand = in(reg) &operand, result = in(reg) &mut result,
                    out("st(0)") _);
            }
            result
        }
    };
}
stored_op!(sub, "fsub dword ptr [{operand}]");
stored_op!(add, "fadd dword ptr [{operand}]");

port! {
    /// Chooses the greatest enabled edge parameter, with later edges winning ties, then interpolates XYZ.
    /// The small entry forwards the six cdecl arguments and restores the caller-visible start/end registers.
    #[port(address = 0x0053_D160, owner = "pathinfo", name = "clip segment to enabled rectangle edges", naked)]
    pub unsafe extern "C" fn fc4p_0053d160() {
        core::arch::naked_asm!(
            "push [esp + 20]", "push [esp + 20]", "lea eax, [esp + 8]", "push eax",
            "push [esp + 36]", "push [esp + 36]", "push [esp + 36]",
            "push [esp + 36]", "push [esp + 36]", "push [esp + 36]",
            "call {}", "add esp, 28", "pop ecx", "pop edx", "ret",
            sym clip_segment);
    }
}

unsafe extern "C" fn clip_segment(
    out: *mut f32,
    bounds: *const f32,
    mask: u32,
    start: *const f32,
    end: *const f32,
    side: *mut u8,
    original_stack: *mut u32,
) -> *mut f32 {
    unsafe {
        let mut t = F80::from_f32(read_unaligned(zero()));
        for edge in 0..4 {
            if mask & (1 << edge) == 0 {
                continue;
            }
            let axis = (edge & 1) * 2;
            let numerator = sub(
                F80::from_f32(read_unaligned(bounds.add(edge))),
                read_unaligned(start.add(axis)),
            );
            let denominator = sub(
                F80::from_f32(read_unaligned(end.add(axis))),
                read_unaligned(start.add(axis)),
            );
            let ratio = numerator / denominator;
            let stored = ratio.to_f32();
            if !original_stack.is_null() {
                write_unaligned(original_stack.add(3), stored.to_bits());
            }
            let comparison = ratio.compare(t);
            if comparison.status_word() & 0x0100 == 0 {
                write_unaligned(side, edge as u8);
                t = F80::from_f32(stored);
            }
        }
        let out = if original_stack.is_null() {
            out
        } else {
            read_unaligned(original_stack.add(1)) as *mut f32
        };
        let dx = sub(F80::from_f32(read_unaligned(end)), read_unaligned(start));
        let dy = sub(F80::from_f32(read_unaligned(end.add(1))), read_unaligned(start.add(1)));
        let dz = sub(F80::from_f32(read_unaligned(end.add(2))), read_unaligned(start.add(2))).to_f32();
        let scaled_x = (dx * t).to_f32();
        let scaled_y = (dy * t).to_f32();
        let scaled_z = (F80::from_f32(dz) * t).to_f32();
        let x = add(F80::from_f32(scaled_x), read_unaligned(start));
        let y = add(F80::from_f32(scaled_y), read_unaligned(start.add(1)));
        let z = add(F80::from_f32(scaled_z), read_unaligned(start.add(2)));
        write_unaligned(out, x.to_f32());
        write_unaligned(out.add(1), y.to_f32());
        write_unaligned(out.add(2), z.to_f32());
        out
    }
}

port! {
    /// Sums each segment's three-dimensional length and stores the total at path +0x0C.
    /// A single vertex stores zero; the supplied range must contain at least one vertex.
    #[port(address = 0x0053_D280, owner = "pathinfo", name = "recompute path length")]
    pub unsafe extern "C" fn fc4p_0053d280(path: *mut u8) -> *mut f32 {
        unsafe {
            let path = Path::from_ptr(path);
            let mut total = F80::from_f32(read_unaligned(zero()));
            let mut previous = path.begin();
            let end = path.end();
            let mut next = previous.wrapping_add(3);
            while next != end {
                let x = sub(F80::from_f32(read_unaligned(next)), read_unaligned(previous));
                let y = sub(F80::from_f32(read_unaligned(next.add(1))), read_unaligned(previous.add(1)));
                let z = sub(F80::from_f32(read_unaligned(next.add(2))), read_unaligned(previous.add(2)));
                previous = next;
                next = next.wrapping_add(3);
                total = total + ((z * z + x * x) + y * y).sqrt();
            }
            path.set_length(total.to_f32());
            next
        }
    }
}

port! {
    /// Copies vertices from the end of a range toward its beginning, writing XYZ words in order.
    /// Overlap follows the same individual-word reads and writes as a forward destination copy.
    #[port(address = 0x0053_D600, owner = "pathinfo", name = "copy vertices in reverse order")]
    pub unsafe extern "C" fn fc4p_0053d600(last: *const u32, first: *const u32, out: *mut u32) -> *mut u32 {
        unsafe {
            let count = (last as u32).wrapping_sub(first as u32) as i32 / 12;
            let mut last = last;
            let mut out = out;
            for _ in 0..count {
                last = last.wrapping_sub(3);
                write_unaligned(out, read_unaligned(last));
                write_unaligned(out.add(1), read_unaligned(last.add(1)));
                write_unaligned(out.add(2), read_unaligned(last.add(2)));
                out = out.wrapping_add(3);
            }
            out
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0053d160::PORT, fc4p_0053d280::PORT, fc4p_0053d600::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{fake_data, GameFpu};

    fn environment() -> (u16, u16, u32, u32) {
        let mut mxcsr = 0u32;
        let flags: u32;
        unsafe {
            core::arch::asm!("stmxcsr [{p}]", "pushfd", "pop {flags}",
                p = in(reg) &mut mxcsr, flags = out(reg) flags);
        }
        (
            crate::x87::control_word(),
            crate::x87::status_word() & 0x3800,
            mxcsr,
            flags & 0x400,
        )
    }

    #[test]
    fn geometry_keeps_existing_x87_values_control_word_mxcsr_and_direction_flag() {
        let _fpu = GameFpu::new();
        let mut zero = 0.0f32;
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let mut points = [0.0f32, 0.0, 0.0, 3.0, 4.0, 0.0];
        let mut record = [0u32; 5];
        let path = unsafe { Path::from_ptr(record.as_mut_ptr().cast()) };
        path.set_begin(points.as_mut_ptr());
        path.set_end(points.as_mut_ptr().wrapping_add(6));
        let start = [0.0f32, 2.0, 0.0];
        let end = [10.0f32, 12.0, 20.0];
        let bounds = [2.0f32, 4.0, 6.0, 16.0];
        let mut out = [0.0f32; 3];
        let mut side = 0u8;
        let args = [
            out.as_mut_ptr() as u32,
            bounds.as_ptr() as u32,
            15,
            start.as_ptr() as u32,
            end.as_ptr() as u32,
            &mut side as *mut u8 as u32,
        ];
        let mut call = RawCall::new(Conv::Cdecl, fc4p_0053d160 as *const () as usize, 0, &args);
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        let before = environment();
        unsafe {
            fc4p_0053d280(path.as_ptr());
        }
        let after_length = environment();
        unsafe {
            call_raw(&mut call);
        }
        let after_clip = environment();
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(after_length, before);
        assert_eq!(after_clip, before);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
    }

    #[unsafe(naked)]
    unsafe extern "C" fn clip_mask_slot(args: *const u32) -> u64 {
        core::arch::naked_asm!(
            "push esi", "mov esi, [esp + 8]",
            "push [esi + 20]", "push [esi + 16]", "push [esi + 12]",
            "push [esi + 8]", "push [esi + 4]", "push [esi]",
            "call {}", "mov eax, [esp + 8]", "mov edx, ecx",
            "add esp, 24", "pop esi", "ret", sym fc4p_0053d160);
    }

    #[test]
    fn clip_overwrites_the_callers_mask_slot_even_for_rejected_edges() {
        let _fpu = GameFpu::new();
        let mut zero = 0.0f32;
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let start = [0.0f32, 2.0, 0.0];
        let end = [10.0f32, 12.0, 20.0];
        let bounds = [2.0f32, 4.0, 6.0, -10.0];
        let mut out = [99.0f32; 3];
        let mut side = 0xA5u8;
        let args = [
            out.as_mut_ptr() as u32,
            bounds.as_ptr() as u32,
            15,
            start.as_ptr() as u32,
            end.as_ptr() as u32,
            &mut side as *mut u8 as u32,
        ];
        let registers = unsafe { clip_mask_slot(args.as_ptr()) };
        assert_eq!(registers as u32, (-0.5f32).to_bits());
        assert_eq!((registers >> 32) as u32, start.as_ptr() as u32);
        assert_eq!(out, [6.0, 8.0, 12.0]);
        assert_eq!(side, 2);
    }

    #[test]
    fn clip_tests_all_edges_ties_disabled_and_negative_parameters() {
        let _fpu = GameFpu::new();
        let mut zero = 0.0f32;
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let start = [0.0f32, 2.0, 0.0];
        let end = [10.0f32, 12.0, 20.0];
        let bounds = [2.0f32, 4.0, 6.0, 16.0];
        for (mask, expected, edge) in [
            (0, [0.0, 2.0, 0.0], 0xA5),
            (1, [2.0, 4.0, 4.0], 0),
            (2, [2.0, 4.0, 4.0], 1),
            (4, [6.0, 8.0, 12.0], 2),
            (8, [8.0, 10.0, 16.0], 3),
            (3, [2.0, 4.0, 4.0], 1),
            (15, [8.0, 10.0, 16.0], 3),
            (0xFFFF_FFF0, [0.0, 2.0, 0.0], 0xA5),
        ] {
            let mut out = [99.0f32; 3];
            let mut side = 0xA5u8;
            let args = [
                out.as_mut_ptr() as u32,
                bounds.as_ptr() as u32,
                mask,
                start.as_ptr() as u32,
                end.as_ptr() as u32,
                &mut side as *mut u8 as u32,
            ];
            let mut call = RawCall::new(Conv::Cdecl, fc4p_0053d160 as *const () as usize, 0, &args);
            unsafe { call_raw(&mut call) };
            assert_eq!(out, expected);
            assert_eq!(side, edge);
            assert_eq!(call.eax, out.as_mut_ptr() as u32);
            assert_eq!(call.edx, end.as_ptr() as u32);
        }
        let mut out = [99.0; 3];
        let mut side = 0xA5;
        unsafe {
            clip_segment(
                out.as_mut_ptr(),
                [-1.0, -2.0, -3.0, -4.0].as_ptr(),
                15,
                start.as_ptr(),
                end.as_ptr(),
                &mut side,
                core::ptr::null_mut(),
            )
        };
        assert_eq!(out, start);
        assert_eq!(side, 0xA5);
    }

    #[test]
    fn clip_ignores_unordered_candidates_and_supports_in_place_output() {
        let _fpu = GameFpu::new();
        let mut zero = 0.0f32;
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let mut start = [1.0f32, 2.0, 3.0];
        let end = [1.0f32, 6.0, 7.0];
        let mut side = 0xA5;
        unsafe {
            clip_segment(
                start.as_mut_ptr(),
                [1.0, 5.0, 1.0, 5.0].as_ptr(),
                15,
                start.as_ptr(),
                end.as_ptr(),
                &mut side,
                core::ptr::null_mut(),
            )
        };
        assert_eq!(start, [1.0, 4.0, 5.0]);
        assert_eq!(side, 3);
    }

    #[test]
    fn lengths_include_elevation_singletons_and_large_register_exponents() {
        let _fpu = GameFpu::new();
        let mut zero = 0.0f32;
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        for (mut points, expected) in [
            (vec![1.0f32, 2.0, 3.0], 0.0f32),
            (vec![0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 3.0, 4.0, 12.0], 17.0),
            (vec![0.0, 0.0, 0.0, 1.0e30, 0.0, 0.0], 1.0e30),
            (vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0], 0.0),
        ] {
            let mut object = [0xA5u8; 0x20];
            let path = unsafe { Path::from_ptr(object.as_mut_ptr()) };
            path.set_begin(points.as_mut_ptr());
            path.set_end(points.as_mut_ptr().wrapping_add(points.len()));
            let before = object;
            let points_before = points.clone();
            assert_eq!(unsafe { fc4p_0053d280(path.as_ptr()) }, path.end());
            assert_eq!(path.length().to_bits(), expected.to_bits());
            assert_eq!(&object[..12], &before[..12]);
            assert_eq!(&object[16..], &before[16..]);
            assert_eq!(points, points_before);
        }
    }

    #[test]
    fn reverse_copy_handles_unrolled_boundaries_raw_float_bits_and_overlap() {
        for count in 0..=9 {
            let points: Vec<u32> = (0..count * 3).map(|i| 0x7F80_0001 + i as u32).collect();
            let mut output = vec![0xA5A5_A5A5u32; count * 3 + 4];
            let begin = output.as_mut_ptr().wrapping_add(1);
            let result = unsafe { fc4p_0053d600(points.as_ptr().wrapping_add(points.len()), points.as_ptr(), begin) };
            assert_eq!(result, begin.wrapping_add(points.len()));
            let expected: Vec<_> = points.chunks(3).rev().flatten().copied().collect();
            assert_eq!(&output[1..1 + count * 3], expected);
            assert_eq!(output[0], 0xA5A5_A5A5);
            assert_eq!(&output[1 + count * 3..], &[0xA5A5_A5A5; 3]);
        }
        let mut points = [1u32, 2, 3, 4, 5, 6, 7, 8, 9];
        let first = points.as_mut_ptr();
        unsafe { fc4p_0053d600(first.add(9), first, first) };
        assert_eq!(points, [7, 8, 9, 4, 5, 6, 7, 8, 9]);
    }
}
