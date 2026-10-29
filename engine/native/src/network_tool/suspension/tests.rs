// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::network_tool::bridge_helpers::tests::invoke;
use crate::testing::{fake_data, GameFpu, Object};

struct Constants {
    _guards: Vec<crate::testing::Fake>,
    _unsigned_bias: Box<f32>,
    _zero: Box<f32>,
    _one: Box<f32>,
    _half: Box<f32>,
    _two: Box<f64>,
    _scale: Box<f64>,
    _offsets: Box<[f32; 8]>,
}
impl Constants {
    fn new() -> Self {
        let mut unsigned_bias = Box::new(4_294_967_296f32);
        let mut zero = Box::new(0f32);
        let mut one = Box::new(1f32);
        let mut half = Box::new(0.5f32);
        let mut two = Box::new(2f64);
        let mut scale = Box::new(f64::from_bits(0x3FED_7628_782D_3949));
        let mut offsets = Box::new([0., 0., 0., 1., 1., 1., 1., 0.]);
        let guards = vec![
            fake_data(0x00A8_0AA8, (&mut *unsigned_bias as *mut f32).cast()),
            fake_data(0x00A8_1054, (&mut *zero as *mut f32).cast()),
            fake_data(0x00A8_1228, (&mut *one as *mut f32).cast()),
            fake_data(0x00A8_4D2C, (&mut *half as *mut f32).cast()),
            fake_data(0x00A8_0AC8, (&mut *two as *mut f64).cast()),
            fake_data(0x00AA_8930, (&mut *scale as *mut f64).cast()),
            fake_data(0x00AA_8938, offsets.as_mut_ptr().cast()),
        ];
        Self {
            _guards: guards,
            _unsigned_bias: unsigned_bias,
            _zero: zero,
            _one: one,
            _half: half,
            _two: two,
            _scale: scale,
            _offsets: offsets,
        }
    }
}

// An independent continuous x87 evaluation checks the split F80 implementation's rounding and
// register exponent range. The position remains live while both exponentials are evaluated.
fn profile(position: F80, weight: f32) -> F80 {
    let complement = sub_float(F80::from_f32(1.), weight).to_f32();
    let two = 2f64;
    let scale = f64::from_bits(0x3FED_7628_782D_3949);
    let mut output = F80::ZERO;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{position}]", "fld st(0)", "fchs", "fldl2e", "fmulp st(1), st(0)",
            "fld st(0)", "frndint", "fxch st(1)", "fsub st(0), st(1)", "f2xm1",
            "fld1", "faddp st(1), st(0)", "fscale", "fstp st(1)",
            "fld st(1)", "fldl2e", "fmulp st(1), st(0)", "fld st(0)", "frndint",
            "fxch st(1)", "fsub st(0), st(1)", "f2xm1", "fld1", "faddp st(1), st(0)",
            "fscale", "fstp st(1)", "fstp st(2)", "fxch st(1)", "faddp st(1), st(0)",
            "fsub qword ptr [{two}]", "fmul qword ptr [{scale}]", "fmul dword ptr [{complement}]",
            "fadd dword ptr [{weight}]", "fstp tbyte ptr [{output}]",
            position = in(reg) &position, two = in(reg) &two, scale = in(reg) &scale,
            complement = in(reg) &complement, weight = in(reg) &weight, output = in(reg) &mut output,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
        );
    }
    output
}

fn records(coordinates: &[[u32; 2]]) -> Object {
    let mut records = Object::new(coordinates.len() * 0x50);
    unsafe {
        for (i, &[x, z]) in coordinates.iter().enumerate() {
            let p = records.ptr().add(i * 0x50);
            for o in (0..0x50).step_by(4) {
                write(p, o, 0xABCD_EF01u32);
            }
            write(p, 4, x);
            write(p, 8, z);
            for c in 0..4 {
                write(p, 0x2C + c * 4, 1f32);
            }
        }
    }
    records
}

#[test]
fn suspension_weights_match_continuous_x87_in_every_end_mode_and_direction() {
    let _constants = Constants::new();
    let _fpu = GameFpu::new();
    for mode in 0..3u32 {
        for [dx, dz] in [[1i32, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [-1, -1]] {
            let coordinates: Vec<_> = (0..9).map(|i| [(20 + i * dx) as u32, (20 + i * dz) as u32]).collect();
            let mut data = records(&coordinates);
            let p = data.ptr();
            let mut args = [p as u32, 9, 1, 0.25f32.to_bits(), mode];
            invoke(
                fc4p_0062e690 as *const () as usize,
                core::ptr::null_mut(),
                &mut args,
                true,
                false,
            );
            let x0 = 20 + i32::from(dx < 0);
            let z0 = 20 + i32::from(dz < 0);
            let x1 = 20 + dx * 8 + i32::from(dx > 0);
            let z1 = 20 + dz * 8 + i32::from(dz > 0);
            let start = (x0 * dx + z0 * dz) as f32;
            let end = (x1 * dx + z1 * dz) as f32;
            let base = match mode {
                0 => start,
                1 => (start + end) * 0.5,
                _ => end,
            };
            let upper = if mode == 2 { start + 0.5 } else { end - 0.5 };
            let denominator = upper - base;
            assert_eq!(args[2], (dz as f32).to_bits());
            assert_eq!(args[4], denominator.to_bits());
            assert_eq!(args[1], (coordinates[8][1] as f32).to_bits());
            for (i, &[x, z]) in coordinates.iter().enumerate() {
                let record = unsafe { p.add(i * 0x50) };
                for (c, [ox, oz]) in [[0u32, 0], [0, 1], [1, 1], [1, 0]].into_iter().enumerate() {
                    let numerator = (x + ox) as i32 * dx + (z + oz) as i32 * dz;
                    let position = (F80::from_i32(numerator) - F80::from_f32(base)) / F80::from_f32(denominator);
                    let expected = profile(position, 0.25);
                    let want = if less(compare_float(expected, 1.)) {
                        expected.to_f32().to_bits()
                    } else {
                        1f32.to_bits()
                    };
                    assert_eq!(
                        unsafe { read::<u32>(record, 0x2C + c * 4) },
                        want,
                        "mode {mode}, ({dx},{dz}), piece {i}, corner {c}"
                    );
                }
                for o in (0..0x50)
                    .step_by(4)
                    .filter(|&o| !matches!(o, 4 | 8 | 0x2C | 0x30 | 0x34 | 0x38))
                {
                    assert_eq!(unsafe { read::<u32>(record, o) }, 0xABCD_EF01, "unrelated word {o}");
                }
            }
        }
    }
}

#[test]
fn suspension_stride_selects_supports_but_updates_every_intervening_record() {
    let _constants = Constants::new();
    let _fpu = GameFpu::new();
    let coordinates: Vec<_> = (0..12).map(|i| [4 + i / 2, 8 + i % 2]).collect();
    let mut data = records(&coordinates);
    let p = data.ptr();
    let mut args = [p as u32, 6, 2, 0.5f32.to_bits(), 1];
    invoke(
        fc4p_0062e690 as *const () as usize,
        core::ptr::null_mut(),
        &mut args,
        true,
        false,
    );
    assert_eq!(args, [p as u32, 9f32.to_bits(), 0, 0.5f32.to_bits(), 2.5f32.to_bits()]);
    for i in 0..12 {
        assert_eq!(unsafe { read::<u32>(p.add(i * 0x50), 0x2C) }, unsafe {
            read::<u32>(p.add((i ^ 1) * 0x50), 0x2C)
        });
    }
}

#[test]
fn suspension_invalid_modes_degenerate_direction_and_nan_weights_keep_old_bits() {
    let _constants = Constants::new();
    let _fpu = GameFpu::new();
    for (coordinates, weight, mode) in [
        (vec![[4, 8], [5, 8], [6, 8]], 0.5f32.to_bits(), 3),
        (vec![[4, 8], [4, 8], [4, 8]], 0.5f32.to_bits(), 0),
        (vec![[4, 8], [5, 8], [6, 8]], 0x7F81_1234, 1),
    ] {
        let mut data = records(&coordinates);
        let p = data.ptr();
        for i in 0..3 {
            for c in 0..4 {
                unsafe {
                    write(p.add(i * 0x50), 0x2C + c * 4, 0x7F81_5678u32);
                }
            }
        }
        let mut args = [p as u32, 3, 1, weight, mode];
        invoke(
            fc4p_0062e690 as *const () as usize,
            core::ptr::null_mut(),
            &mut args,
            true,
            false,
        );
        for i in 0..3 {
            for c in 0..4 {
                assert_eq!(unsafe { read::<u32>(p.add(i * 0x50), 0x2C + c * 4) }, 0x7F81_5678);
            }
        }
        if mode == 3 {
            assert_eq!(args[4], 0);
        }
    }
}

#[test]
fn nonpositive_piece_counts_still_write_converted_endpoint_arguments() {
    let _constants = Constants::new();
    let _fpu = GameFpu::new();
    let coordinates = [[u32::MAX - 2, u32::MAX - 1], [u32::MAX - 1, u32::MAX]];
    // count zero and stride one: the far endpoint is the preceding record, while the direction reads
    // the following record. Reserve that following record explicitly for the direction load.
    let mut data2 = records(&[coordinates[0], coordinates[1], [u32::MAX, 0]]);
    let p2 = unsafe { data2.ptr().add(0x50) };
    let mut args = [p2 as u32, 0, 1, 0.25f32.to_bits(), 2];
    invoke(
        fc4p_0062e690 as *const () as usize,
        core::ptr::null_mut(),
        &mut args,
        true,
        false,
    );
    assert_eq!(args[1], 4_294_967_296f32.to_bits());
    assert_eq!(args[2], 1f32.to_bits()); // unsigned Z subtraction wraps to signed +1.
    assert_eq!(args[4], 2);
    for c in 0..4 {
        assert_eq!(unsafe { read::<u32>(p2, 0x2C + c * 4) }, 1f32.to_bits());
    }
}

#[test]
fn split_exponentials_keep_the_register_range_and_match_rounding_modes() {
    let _fpu = GameFpu::new();
    for control in [0x007F, 0x027F, 0x037F] {
        crate::x87::set_control_word(control);
        for value in [-120f32, -2., -0.5, 0., 0.5, 2., 120.] {
            let x = F80::from_f32(value);
            let candidate = exponential(-x) + exponential(x);
            let candidate = add_float(
                mul_float(
                    mul_double(sub_double(candidate, 2.), f64::from_bits(0x3FED_7628_782D_3949)),
                    0.75,
                ),
                0.25,
            );
            assert_eq!(
                candidate.to_bytes(),
                profile(x, 0.25).to_bytes(),
                "control {control:x}, x {value}"
            );
        }
    }
}
