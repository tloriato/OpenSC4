// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

struct Constants {
    _values: Box<[f32; 9]>,
    _fakes: Vec<crate::testing::Fake>,
}

impl Constants {
    fn new() -> Self {
        let mut values = Box::new([0.0, 6.0, 0.25, 5.0, 0.3, 4.0, 0.35, 3.0, 0.4]);
        let addresses = [
            0x00A8_1054,
            0x00A9_0900,
            0x00A8_821C,
            0x00A8_4D28,
            0x00A9_7D74,
            0x00A8_E17C,
            0x00A9_8FC0,
            0x00A8_5074,
            0x00A9_49E0,
        ];
        let fakes = addresses
            .into_iter()
            .enumerate()
            .map(|(i, address)| fake_data(address, values.as_mut_ptr().wrapping_add(i).cast()))
            .collect();
        Self {
            _values: values,
            _fakes: fakes,
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn length() {
    core::arch::naked_asm!(
        "sub esp, 12", "mov ecx, esp", "push dword ptr [esp + 20]", "push dword ptr [esp + 20]",
        "call {body}", "fld tbyte ptr [esp]", "add esp, 12", "ret", body = sym length_body,
    );
}

unsafe extern "fastcall" fn length_body(output: *mut F80, _edx: u32, first: u32, second: u32) -> u64 {
    let returned = run(core::ptr::null_mut(), "line_length", &[first, second]);
    let value = FLOATS.with(|f| f.borrow_mut().pop_front()).unwrap();
    unsafe {
        output.write_unaligned(value);
    }
    returned
}

unsafe fn invoke(this: *mut u8, rect: *const i32, water: u32) -> (RawCall, [u32; 2]) {
    let mut value = F80::ZERO;
    let mut record = [
        fc4p_006c5dc0 as *const () as u32,
        this as u32,
        rect as u32,
        water,
        0,
        0,
        0,
        0,
        &mut value as *mut F80 as u32,
    ];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 12]", "push dword ptr [edi + 8]", "mov ecx, dword ptr [edi + 4]",
            "call dword ptr [edi]", "mov dword ptr [edi + 16], eax", "mov dword ptr [edi + 20], edx",
            "mov ecx, dword ptr [edi + 32]", "fstp tbyte ptr [ecx]",
            "mov eax, dword ptr [esp - 8]", "mov dword ptr [edi + 24], eax",
            "mov eax, dword ptr [esp - 4]", "mov dword ptr [edi + 28], eax",
            in("edi") record.as_mut_ptr(), clobber_abi("C"), out("st(0)") _,
        );
    }
    let mut call = RawCall::new(Conv::Thiscall, 0, 0, &[]);
    call.eax = record[4];
    call.edx = record[5];
    call.st0 = value;
    (call, [record[6], record[7]])
}

fn vertices(this: *mut u8, water: u32, values: &[f32; 4]) {
    for (i, &value) in values.iter().enumerate() {
        expect_float(
            this,
            "vertex_body",
            vec![w((i / 2) as u32), w((i % 2) as u32), w(water)],
            RETURN,
            F80::from_f32(value),
        );
    }
}

#[test]
fn slope_of_one_cell_is_four_and_flat_area_returns_positive_zero() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let _length = fake(0x005D_76A0, length as *const () as usize);
    unsafe {
        for flat in [false, true] {
            let mut developer = mock!(0x3C => vertex_float);
            let this = developer.ptr();
            let rect = [0i32; 4];
            vertices(
                this,
                0xAABB_0100,
                &[100.0, 100.0, if flat { 100.0 } else { 104.0 }, 100.0],
            );
            expect_float(
                core::ptr::null_mut(),
                "line_length",
                vec![
                    Arg::In(vec![0, 0]),
                    Arg::In(vec![if flat { 0 } else { 1.0f32.to_bits() }, 0]),
                ],
                RETURN,
                F80::from_f32(if flat { 0.0 } else { 1.0 }),
            );
            let before = crate::x87::status_word() & 0x3800;
            let (call, args) = invoke(this, rect.as_ptr(), 0xAABB_0100);
            assert_eq!(call.st0.to_f32().to_bits(), if flat { 0 } else { 4.0f32.to_bits() });
            assert_eq!(call.eax & 0xFFFF_0000, RETURN as u32 & 0xFFFF_0000);
            assert_eq!(call.edx, (RETURN >> 32) as u32);
            assert_eq!(args, [2, 0xAABB_0100]);
            assert_eq!(crate::x87::status_word() & 0x3800, before);
            assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
            finished();
        }
    }
}

#[test]
fn slope_keeps_first_strict_extrema_and_ignores_nan_samples() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let _length = fake(0x005D_76A0, length as *const () as usize);
    unsafe {
        for values in [[100.0, 104.0, 104.0, 100.0], [100.0, 104.0, f32::NAN, 100.0]] {
            let mut developer = mock!(0x3C => vertex_float);
            let this = developer.ptr();
            let rect = [0i32; 4];
            vertices(this, 1, &values);
            expect_float(
                core::ptr::null_mut(),
                "line_length",
                vec![Arg::In(vec![0, 0]), Arg::In(vec![0, 1.0f32.to_bits()])],
                RETURN,
                F80::from_f32(1.0),
            );
            assert_eq!(invoke(this, rect.as_ptr(), 1).0.st0.to_f32(), 4.0);
            finished();
        }
    }
}

#[test]
fn slope_distance_thresholds_are_strict_and_scaled_in_cascade() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let _length = fake(0x005D_76A0, length as *const () as usize);
    let cases = [
        (1.0, 1.0),
        (3.0, 3.0),
        (3.5, 1.4),
        (4.0, 1.6),
        (4.5, f32::from_bits(0x3FC9_9999)),
        (5.0, 1.75),
        (5.5, 1.6500001),
        (6.0, 1.8000001),
        (6.5, 1.625),
        (20.0, 1.75),
        (24.0, 1.8000001),
        (32.0, 2.4),
        (64.0, 1.6800001),
    ];
    unsafe {
        for (distance, divisor) in cases {
            let mut developer = mock!(0x3C => vertex_float);
            let this = developer.ptr();
            let rect = [0i32; 4];
            vertices(this, 1, &[100.0, 100.0, 104.0, 100.0]);
            expect_float(
                core::ptr::null_mut(),
                "line_length",
                vec![Arg::In(vec![0, 0]), Arg::In(vec![1.0f32.to_bits(), 0])],
                RETURN,
                F80::from_f32(distance),
            );
            let answer = invoke(this, rect.as_ptr(), 1).0.st0;
            let expected = F80::from_f32(4.0) / F80::from_f32(divisor);
            assert_eq!(answer.to_bytes(), expected.to_bytes(), "distance {distance}");
            finished();
        }
    }
}

#[test]
fn slope_empty_or_wrapped_bounds_still_call_length_without_initializing_coordinates() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let _length = fake(0x005D_76A0, length as *const () as usize);
    unsafe {
        for rect in [[3i32, 0, 0, 0], [0, 4, 0, 0], [0, 0, i32::MAX, 0]] {
            let mut developer = mock!(0x3C => vertex_float);
            let this = developer.ptr();
            expect_float(
                core::ptr::null_mut(),
                "line_length",
                vec![Arg::In(vec![]), Arg::In(vec![])],
                RETURN,
                F80::ZERO,
            );
            let (answer, args) = invoke(this, rect.as_ptr(), 1);
            assert_eq!(answer.st0.to_f32().to_bits(), 0);
            assert_eq!(
                args[0],
                if rect[0] <= rect[2].wrapping_add(1) {
                    rect[1] as u32
                } else {
                    rect.as_ptr() as u32
                }
            );
            finished();
        }
    }
}

#[test]
fn slope_preserves_unrounded_length_and_handles_nonfinite_distances() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let _length = fake(0x005D_76A0, length as *const () as usize);
    for distance in [
        F80::from_f64(1.000_000_000_931_322_6),
        F80::from_f32(f32::INFINITY),
        F80::from_f32(f32::from_bits(0x7FC1_2345)),
        F80::from_f32(-0.0),
    ] {
        crate::x87::set_control_word(0x037F);
        unsafe {
            let mut developer = mock!(0x3C => vertex_float);
            let this = developer.ptr();
            let rect = [0i32; 4];
            vertices(this, 1, &[100.0, 100.0, 104.0, 100.0]);
            expect_float(
                core::ptr::null_mut(),
                "line_length",
                vec![Arg::In(vec![0, 0]), Arg::In(vec![1.0f32.to_bits(), 0])],
                RETURN,
                distance,
            );
            let answer = invoke(this, rect.as_ptr(), 1).0.st0;
            if distance.to_f32().is_nan() {
                assert!(answer.to_f32().is_nan());
            } else if distance.to_f32().is_infinite() || distance.to_f32() == 0.0 {
                assert_eq!(answer.to_f32().to_bits(), 0);
            } else {
                assert_eq!(answer.to_bytes(), (F80::from_f32(4.0) / distance).to_bytes());
            }
            assert_eq!(crate::x87::control_word(), 0x037F);
            finished();
        }
    }
}
