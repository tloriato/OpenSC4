// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, GameFpu, Object};
use std::cell::RefCell;

#[repr(C)]
struct Capture {
    call: RawCall,
    stack_delta: i32,
}

// Copy back the original argument words before discarding the call frame. This also checks the
// callee's pop count, including functions that receive ECX but leave their arguments to the caller.
#[unsafe(naked)]
unsafe extern "thiscall" fn capture_call(_capture: *mut Capture) {
    core::arch::naked_asm!(
        "push ebp",
        "mov ebp, esp",
        "push ebx",
        "push esi",
        "push edi",
        "mov edi, ecx",
        "mov ecx, [edi + 20]",
        "mov eax, [edi + 16]",
        "2:",
        "test ecx, ecx",
        "jz 3f",
        "dec ecx",
        "push dword ptr [eax + ecx*4]",
        "jmp 2b",
        "3:",
        "mov eax, [edi + 4]",
        "mov ecx, [edi + 8]",
        "mov edx, [edi + 12]",
        "call dword ptr [edi]",
        "mov [edi + 4], eax",
        "mov [edi + 12], edx",
        "cmp dword ptr [edi + 28], 0",
        "je 4f",
        "fstp tbyte ptr [edi + 32]",
        "4:",
        "lea eax, [ebp - 12]",
        "cmp dword ptr [edi + 24], 0",
        "je 5f",
        "mov ecx, [edi + 20]",
        "shl ecx, 2",
        "sub eax, ecx",
        "5:",
        "mov edx, esp",
        "sub edx, eax",
        "mov [edi + 44], edx",
        "lea esi, [ebp - 12]",
        "mov ecx, [edi + 20]",
        "shl ecx, 2",
        "sub esi, ecx",
        "mov ebx, [edi + 16]",
        "xor ecx, ecx",
        "6:",
        "cmp ecx, [edi + 20]",
        "jae 7f",
        "mov eax, [esi + ecx*4]",
        "mov [ebx + ecx*4], eax",
        "inc ecx",
        "jmp 6b",
        "7:",
        "lea esp, [ebp - 12]",
        "pop edi",
        "pop esi",
        "pop ebx",
        "pop ebp",
        "ret",
    );
}

pub(crate) fn invoke(function: usize, this: *mut u8, args: &mut [u32], cdecl: bool, st0: bool) -> RawCall {
    let control = crate::x87::control_word();
    let before_top: u16;
    let mut before_mxcsr = 0u32;
    unsafe {
        core::arch::asm!("fnstsw ax", out("ax") before_top);
        core::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut before_mxcsr);
    }
    let mut capture = Capture {
        call: RawCall::new(
            if cdecl { Conv::Cdecl } else { Conv::Thiscall },
            function,
            this as u32,
            args,
        ),
        stack_delta: i32::MIN,
    };
    capture.call.eax = 0xA1B2_C3D4;
    capture.call.edx = 0xD4C3_B2A1;
    capture.call.take_st0 = u32::from(st0);
    unsafe { capture_call(&mut capture) };
    assert_eq!(capture.stack_delta, 0, "callee pop count");
    let after_top: u16;
    let mut after_mxcsr = 0u32;
    unsafe {
        core::arch::asm!("fnstsw ax", out("ax") after_top);
        core::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut after_mxcsr);
    }
    assert_eq!(after_top & 0x3800, before_top & 0x3800, "x87 stack balance");
    assert_eq!(crate::x87::control_word(), control, "x87 control word");
    assert_eq!(after_mxcsr, before_mxcsr, "MXCSR");
    capture.call
}

#[derive(Debug, PartialEq)]
enum Call {
    Destroy(u32),
    Free(u32),
    Corners(u32, u32, u32),
    Backward(u32, u32, u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static HEIGHTS: RefCell<[u32; 4]> = const { RefCell::new([0; 4]) };
}
unsafe extern "thiscall" fn destroy(this: *mut u8) -> u64 {
    CALLS.with(|c| c.borrow_mut().push(Call::Destroy(this as u32)));
    unsafe { write(this, 4, 0x1234_5678u32) };
    registers(0x1020_3040, 0x5060_7080)
}
extern "C" fn free(p: *mut u8) -> u64 {
    CALLS.with(|c| c.borrow_mut().push(Call::Free(p as u32)));
    registers(0x1122_3344, 0x5566_7788)
}
unsafe extern "thiscall" fn corners(this: *mut u8, x: u32, z: u32, output: *mut u32) {
    CALLS.with(|c| c.borrow_mut().push(Call::Corners(this as u32, x, z)));
    HEIGHTS.with(|h| unsafe { core::ptr::copy_nonoverlapping(h.borrow().as_ptr(), output, 4) });
}
#[unsafe(naked)]
unsafe extern "C" fn backward() {
    core::arch::naked_asm!(
        "push eax", "push ecx", "push edx", "mov ecx, esp", "call {body}", "add esp, 12", "ret",
        body = sym backward_body,
    );
}
unsafe extern "thiscall" fn backward_body(frame: *mut u32) -> u64 {
    unsafe {
        assert_eq!(*frame, 0xD4C3_B2A1, "copy helper entry EDX");
        assert_eq!(*frame.add(1), *frame.add(6), "copy helper entry ECX");
        assert_eq!(*frame.add(2), *frame.add(5), "copy helper entry EAX");
        backward_fields(
            *frame.add(4) as *mut u32,
            *frame.add(5) as *mut u32,
            *frame.add(6) as *mut u32,
            *frame.add(7) as *mut u32,
            *frame.add(8),
        )
    }
}
unsafe fn backward_fields(first: *mut u32, last: *mut u32, end: *mut u32, value: *mut u32, flags: u32) -> u64 {
    CALLS.with(|c| {
        c.borrow_mut()
            .push(Call::Backward(first as u32, last as u32, end as u32, flags))
    });
    assert_eq!(unsafe { *value }, 1f32.to_bits());
    let n = (last as usize - first as usize) / 4;
    for i in (0..n).rev() {
        unsafe { *end.sub(n).add(i) = *first.add(i) };
    }
    // The insertion must read this same argument word after the helper returns.
    unsafe { *value = (-2f32).to_bits() };
    registers(0xDEAD_BEEF, 0xCAFE_BABE)
}
fn take_calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

#[test]
fn scalar_delete_always_destroys_then_frees_only_for_the_low_bit() {
    let _destroy = fake(0x0062_CAF0, destroy as *const () as usize);
    let _free = fake(0x005E_5620, free as *const () as usize);
    let mut object = Object::new(16);
    let p = object.ptr();
    for flags in [0, 2, 0x100, 1, 3, u32::MAX] {
        let mut args = [flags];
        let result = invoke(fc4p_0062cd30 as *const () as usize, p, &mut args, false, false);
        assert_eq!(result.eax, p as u32);
        assert_eq!(args, [flags]);
        assert_eq!(unsafe { read::<u32>(p, 4) }, 0x1234_5678);
        let mut expected = vec![Call::Destroy(p as u32)];
        if flags & 1 != 0 {
            expected.push(Call::Free(p as u32));
        }
        assert_eq!(take_calls(), expected);
        assert_eq!(result.edx, if flags & 1 != 0 { 0x5566_7788 } else { 0x5060_7080 });
    }
}

#[test]
fn first_minimum_handles_empty_single_equal_signed_zero_and_nan_ranges() {
    let _fpu = GameFpu::new();
    for (bits, index) in [
        (vec![], 0),
        (vec![5f32.to_bits()], 0),
        (
            vec![5f32.to_bits(), (-3f32).to_bits(), (-3f32).to_bits(), 7f32.to_bits()],
            1,
        ),
        (vec![0, 0x8000_0000], 0),
        (vec![0x7FC1_2345, (-10f32).to_bits()], 0),
        (vec![5f32.to_bits(), 0x7F81_2345, (-10f32).to_bits()], 2),
        (vec![f32::INFINITY.to_bits(), f32::NEG_INFINITY.to_bits()], 1),
    ] {
        let mut bits = bits;
        let original = bits.clone();
        let first = bits.as_mut_ptr();
        let mut args = [first as u32, first.wrapping_add(bits.len()) as u32];
        let result = invoke(
            fc4p_0062db90 as *const () as usize,
            core::ptr::null_mut(),
            &mut args,
            true,
            false,
        );
        assert_eq!(result.eax, first.wrapping_add(index) as u32);
        assert_eq!(bits, original);
    }
}

#[test]
fn terrain_minimum_queries_once_and_selects_the_original_pair_order() {
    let _fpu = GameFpu::new();
    let mut table = [0usize; 18];
    table[0x44 / 4] = corners as *const () as usize;
    let mut terrain = Object::new(16);
    let p = terrain.ptr();
    unsafe { write(p, 0, table.as_ptr()) };
    for (heights, want) in [
        (
            [7f32.to_bits(), 5f32.to_bits(), 3f32.to_bits(), 1f32.to_bits()],
            1f32.to_bits(),
        ),
        ([0, 0x8000_0000, 0x8000_0000, 0], 0),
        (
            [7f32.to_bits(), 0x7FC1_2345, 5f32.to_bits(), 0x7FC5_4321],
            5f32.to_bits(),
        ),
        ([0x7F81_2345, 2f32.to_bits(), 1f32.to_bits(), 0], 0x7FC1_2345),
    ] {
        HEIGHTS.with(|h| *h.borrow_mut() = heights);
        let mut args = [0xFFFF_FFFF, 0x1234_5678];
        let result = invoke(fc4p_0062e630 as *const () as usize, p, &mut args, true, true);
        assert_eq!(result.st0.to_f32().to_bits(), want);
        assert_eq!(take_calls(), [Call::Corners(p as u32, args[0], args[1])]);
    }
}

#[test]
fn bridge_record_copy_returns_the_end_and_keeps_forward_word_aliasing() {
    let mut words: Vec<u32> = (0..24).map(|i| 0x0100_0000 + i).collect();
    let first = words.as_mut_ptr().cast::<u8>();
    let mut output = [0xABCD_EF01u32; 24];
    let target = output.as_mut_ptr().cast::<u8>();
    let mut args = [first as u32, first.wrapping_add(64) as u32, target as u32];
    let result = invoke(
        fc4p_0062e970 as *const () as usize,
        core::ptr::null_mut(),
        &mut args,
        true,
        false,
    );
    assert_eq!(result.eax, target.wrapping_add(64) as u32);
    assert_eq!(&output[..16], &words[..16]);
    assert_eq!(&output[16..], &[0xABCD_EF01; 8]);
    let mut empty = [first as u32, first as u32, 0x1234_5678];
    assert_eq!(
        invoke(
            fc4p_0062e970 as *const () as usize,
            core::ptr::null_mut(),
            &mut empty,
            true,
            false
        )
        .eax,
        0x1234_5678
    );
    let mut null = [first as u32, first.wrapping_add(32) as u32, 0];
    assert_eq!(
        invoke(
            fc4p_0062e970 as *const () as usize,
            core::ptr::null_mut(),
            &mut null,
            true,
            false
        )
        .eax,
        32
    );
    let mut alias = [
        first as u32,
        first.wrapping_add(64) as u32,
        first.wrapping_add(4) as u32,
    ];
    invoke(
        fc4p_0062e970 as *const () as usize,
        core::ptr::null_mut(),
        &mut alias,
        true,
        false,
    );
    assert_eq!(words[..17], [0x0100_0000; 17]);
}

#[test]
fn ordered_insertion_uses_the_helper_and_the_live_value_argument() {
    let _fpu = GameFpu::new();
    let _copy = fake(0x0062_DC30, backward as *const () as usize);
    let mut values = [3f32.to_bits(), 5f32.to_bits(), 7f32.to_bits(), 0];
    let p = values.as_mut_ptr();
    let mut args = [p as u32, p.wrapping_add(3) as u32, 1f32.to_bits()];
    invoke(
        fc4p_0062edf0 as *const () as usize,
        core::ptr::null_mut(),
        &mut args,
        true,
        false,
    );
    assert_eq!(
        values,
        [(-2f32).to_bits(), 3f32.to_bits(), 5f32.to_bits(), 7f32.to_bits()]
    );
    assert_eq!(args[2], (-2f32).to_bits());
    assert_eq!(
        take_calls(),
        [Call::Backward(
            p as u32,
            p.wrapping_add(3) as u32,
            p.wrapping_add(4) as u32,
            0
        )]
    );
}

#[test]
fn ordered_insertion_shifts_strictly_greater_values_and_preserves_raw_bits() {
    let _fpu = GameFpu::new();
    for (initial, value, want) in [
        (
            [0, 3f32.to_bits(), 5f32.to_bits(), 0],
            2f32.to_bits(),
            [0, 2f32.to_bits(), 3f32.to_bits(), 5f32.to_bits()],
        ),
        ([0, 0x8000_0000, 0, 0], 0x8000_0000, [0, 0x8000_0000, 0, 0x8000_0000]),
        (
            [0, 3f32.to_bits(), 5f32.to_bits(), 0],
            0x7F81_2345,
            [0, 3f32.to_bits(), 5f32.to_bits(), 0x7F81_2345],
        ),
        (
            [0, 0x7F81_2345, 5f32.to_bits(), 0],
            2f32.to_bits(),
            [0, 0x7F81_2345, 2f32.to_bits(), 5f32.to_bits()],
        ),
    ] {
        let mut values = initial;
        let p = values.as_mut_ptr();
        let mut args = [p as u32, p.wrapping_add(3) as u32, value];
        invoke(
            fc4p_0062edf0 as *const () as usize,
            core::ptr::null_mut(),
            &mut args,
            true,
            false,
        );
        assert_eq!(values, want);
        assert_eq!(args[2], value);
        assert!(take_calls().is_empty());
    }
}
