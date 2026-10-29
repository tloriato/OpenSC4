// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

slot!(water_compatible);
slot!(cell_wet, x, z);
slot!(world_center, x, z, world_x, world_z);
slot!(facing);
float_slot!(water_level_float, water_level_body, 2, x, z);

/// The two corner loads, comparison, difference and interpolation, kept together on the x87.
fn side_altitude(a: f32, b: f32, half: f32) -> (F80, u16) {
    let mut output = F80::ZERO;
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{a}]", "fld dword ptr [{b}]", "fld st(1)", "fcomp st(1)",
            "fnstsw ax", "test ah, 5", "jnp 2f", "fxch st(1)", "2:",
            "fsub st(0), st(1)", "fmul dword ptr [{half}]", "faddp st(1), st(0)", "fstp tbyte ptr [{output}]",
            a = in(reg) &a, b = in(reg) &b, half = in(reg) &half, output = in(reg) &mut output,
            out("ax") status, out("st(0)") _, out("st(1)") _, out("st(2)") _,
        );
    }
    (output, status)
}

#[test]
fn foundation_side_uses_the_unmasked_facing_and_signed_adjacent_remainder() {
    let _fpu = GameFpu::new();
    unsafe {
        for config_state in [0, 1, 2] {
            for direction in -5i32..=5 {
                let mut dev = Object::new(0x108);
                let this = dev.ptr();
                let mut lot = mock!(0x24 => get_config, 0x5C => facing);
                let lot = lot.ptr();
                let mut config = mock!(0x38 => water_compatible);
                let config = config.ptr();
                let mut half = 0.5f32;
                let _half = fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast());
                let mut terrain = mock!(0x88 => cell_wet);
                let mut terrain = terrain.ptr();
                let _terrain = fake_data(0x00B4_3CF4, (&mut terrain as *mut *mut u8).cast());
                let mut corners = [21.0f32, 9.0, 16.0, 10.0, 3.0, -2.0, 8.0, 12.0, 4.0, -7.0, 2.0];
                let middle = corners.as_mut_ptr().add(5);
                expect(
                    lot,
                    "get_config",
                    vec![],
                    if config_state == 0 { 0 } else { config as u64 },
                );
                if config_state != 0 {
                    expect(
                        config,
                        "water_compatible",
                        vec![],
                        if config_state == 1 { 0x100 } else { 0 },
                    );
                }
                expect(
                    lot,
                    "facing",
                    vec![],
                    0x1234_5678_0000_0000 | u64::from(direction as u32),
                );
                let a = middle.offset(direction as isize).read();
                let b = middle.offset((direction.wrapping_add(1) % 4) as isize).read();
                let (expected, sw) = side_altitude(a, b, half);
                let call = call_float(
                    fc4p_006c5360 as *const () as usize,
                    this,
                    &[lot as u32, 0xCAFEBABE, 0xBADF00D, middle as u32],
                );
                assert_eq!(call.st0.to_bytes(), expected.to_bytes());
                assert_eq!(call.edx, 0x1234_5678);
                assert_eq!(
                    call.eax & 0xFFFF_0000,
                    ((direction.wrapping_add(1) % 4) as u32) & 0xFFFF_0000
                );
                assert_eq!(call.eax as u16 & 0x7F00, sw & 0x7F00);
                finished();
            }
        }
    }
}

#[test]
fn foundation_side_preserves_nan_payloads_signed_zero_and_extended_exponents() {
    let _fpu = GameFpu::new();
    unsafe {
        let mut dev = Object::new(0x108);
        let this = dev.ptr();
        let mut lot = mock!(0x24 => get_config, 0x5C => facing);
        let lot = lot.ptr();
        let mut half = 0.5f32;
        let _half = fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast());
        for (a, b) in [
            (0.0, -0.0),
            (-0.0, 0.0),
            (-f32::MAX, f32::MAX),
            (f32::from_bits(0x7FC1_2345), f32::from_bits(0x7FC5_4321)),
            (f32::from_bits(0x7F81_2345), 5.0),
            (5.0, f32::from_bits(0xFF81_4321)),
            (f32::INFINITY, f32::NEG_INFINITY),
        ] {
            let corners = [a, b, 0.0, 0.0];
            expect(lot, "get_config", vec![], 0);
            expect(lot, "facing", vec![], 0);
            let (expected, _) = side_altitude(a, b, half);
            let call = call_float(
                fc4p_006c5360 as *const () as usize,
                this,
                &[lot as u32, 0, 0, corners.as_ptr() as u32],
            );
            assert_eq!(call.st0.to_bytes(), expected.to_bytes(), "{a:?}, {b:?}");
            finished();
        }
    }
}

#[test]
fn foundation_water_uses_the_first_wet_cell_x_before_z_and_keeps_the_cached_terrain() {
    let _fpu = GameFpu::new();
    unsafe {
        for wet_at in [None, Some((2, 4)), Some((2, 5)), Some((3, 4)), Some((3, 5))] {
            let mut dev = Object::new(0x108);
            let this = dev.ptr();
            let mut lot = mock!(0x24 => get_config, 0x38 => bounding_rect, 0x5C => facing);
            let lot = lot.ptr();
            let mut config = mock!(0x38 => water_compatible);
            let config = config.ptr();
            let mut terrain = mock!(0x88 => cell_wet, 0x78 => water_level_float);
            let cached = terrain.ptr();
            let mut terrain = cached;
            let terrain_location = &mut terrain as *mut *mut u8;
            let _terrain = fake_data(0x00B4_3CF4, terrain_location.cast());
            let mut city = mock!(0x184 => world_center);
            let city = city.ptr();
            LotDeveloper::from_ptr(this).set_city(city);
            let mut half = 0.5f32;
            let _half = fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast());
            expect(lot, "get_config", vec![], config as u64);
            expect(config, "water_compatible", vec![], 0x101);
            expect(lot, "bounding_rect", vec![out(&[2, 4, 3, 5])], RETURN);
            edit_last(terrain_location.cast(), 0xBAD0_0000);
            'cells: for x in 2..=3 {
                for z in 4..=5 {
                    expect(
                        cached,
                        "cell_wet",
                        vec![w(x), w(z)],
                        if wet_at == Some((x, z)) { 0x101 } else { 0x100 },
                    );
                    if wet_at == Some((x, z)) {
                        expect(
                            city,
                            "world_center",
                            vec![w(x), w(z), out(&[40.0f32.to_bits()]), out(&[72.0f32.to_bits()])],
                            RETURN,
                        );
                        expect_float(
                            cached,
                            "water_level_body",
                            vec![w(40.0f32.to_bits()), w(72.0f32.to_bits())],
                            RETURN,
                            F80::from_f32(100.25),
                        );
                        break 'cells;
                    }
                }
            }
            let corners = [10.0f32, 6.0, 3.0, 4.0];
            if wet_at.is_none() {
                expect(lot, "facing", vec![], RETURN & 0xFFFF_FFFF_0000_0000);
            }
            let call = call_float(
                fc4p_006c5360 as *const () as usize,
                this,
                &[lot as u32, 0xFFFFFFFF, 0xCAFEBABE, corners.as_ptr() as u32],
            );
            assert_eq!(call.st0.to_f32(), if wet_at.is_some() { 100.75 } else { 8.0 });
            if wet_at.is_some() {
                assert_eq!(result(call), RETURN);
            }
            finished();
        }
    }
}

#[test]
fn foundation_water_skips_reversed_rectangles() {
    let _fpu = GameFpu::new();
    unsafe {
        for rect in [[3, 4, 2, 5], [2, 6, 3, 5]] {
            let mut dev = Object::new(0x108);
            let this = dev.ptr();
            let mut lot = mock!(0x24 => get_config, 0x38 => bounding_rect, 0x5C => facing);
            let lot = lot.ptr();
            let mut config = mock!(0x38 => water_compatible);
            let config = config.ptr();
            let mut terrain = core::ptr::null_mut::<u8>();
            let _terrain = fake_data(0x00B4_3CF4, (&mut terrain as *mut *mut u8).cast());
            let mut half = 0.5f32;
            let _half = fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast());
            expect(lot, "get_config", vec![], config as u64);
            expect(config, "water_compatible", vec![], 1);
            expect(lot, "bounding_rect", vec![out(&rect)], RETURN);
            expect(lot, "facing", vec![], 3);
            let corners = [10.0f32, 6.0, 3.0, 4.0];
            let call = call_float(
                fc4p_006c5360 as *const () as usize,
                this,
                &[lot as u32, 0, 0, corners.as_ptr() as u32],
            );
            assert_eq!(call.st0.to_f32(), 7.0);
            finished();
        }
    }
}
