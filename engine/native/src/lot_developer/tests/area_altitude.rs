// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

float_slot!(position_altitude, position_altitude_body, 2, x, z);
float_slot!(position_water, position_water_body, 2, x, z);

struct Globals {
    _values: Box<[f32; 5]>,
    _guards: Vec<crate::testing::Fake>,
}
impl Globals {
    fn new() -> Self {
        let mut values = Box::new([0.0, 0.1, 16.0, 0.0625, 4.0]);
        let guards = [0x00A8_1054, 0x00A8_C950, 0x00A8_D45C, 0x00AB_1B04, 0x00A8_E17C]
            .iter()
            .enumerate()
            .map(|(i, &a)| fake_data(a, unsafe { values.as_mut_ptr().add(i).cast() }))
            .collect();
        Self {
            _values: values,
            _guards: guards,
        }
    }
}

unsafe fn invoke(this: *mut u8, rect: &[f32; 4], lowest: u32, water: u32) -> (F80, [u32; 5]) {
    let mut value = F80::ZERO;
    let mut words = [
        0u32,
        0,
        rect.as_ptr() as u32,
        lowest,
        water,
        &mut value as *mut F80 as u32,
    ];
    unsafe {
        core::arch::asm!(
            "push [edi + 16]", "push [edi + 12]", "push [edi + 8]", "call {function}",
            "mov [edi], eax", "mov [edi + 4], edx", "mov eax, [esp - 12]", "mov [edi + 8], eax",
            "mov eax, [esp - 8]", "mov [edi + 12], eax", "mov eax, [esp - 4]", "mov [edi + 16], eax",
            "mov ecx, [edi + 20]", "fstp tbyte ptr [ecx]",
            function = sym fc4p_006c56c0, in("edi") words.as_mut_ptr(),
            in("ecx") this, clobber_abi("C"), out("st(0)") _,
        );
    }
    (value, words[..5].try_into().unwrap())
}

fn edge_points(bounds: [f32; 4]) -> Vec<(f32, f32)> {
    let sweep = |start: f32, end: f32| {
        let mut points = Vec::new();
        if start < end {
            let mut p = start;
            loop {
                points.push(p);
                if p == end {
                    break;
                }
                p = (p + 4.).min(end);
            }
        }
        points
    };
    let [x0, z0, x1, z1] = bounds;
    sweep(x0, x1)
        .iter()
        .map(|&x| (x, z0))
        .chain(sweep(z0, z1).iter().map(|&z| (x1, z)))
        .chain(sweep(x0, x1).iter().map(|&x| (x, z1)))
        .chain(sweep(z0, z1).iter().map(|&z| (x0, z)))
        .collect()
}

#[test]
fn metre_altitude_clamps_lower_edges_visits_endpoints_and_selects_water_before_extrema() {
    let _fpu = GameFpu::new();
    let _globals = Globals::new();
    let mut dev = mock!(0x3C => vertex_float);
    let mut terrain = mock!(0x34 => position_altitude, 0x78 => position_water);
    let developer = unsafe { LotDeveloper::from_ptr(dev.ptr()) };
    developer.set_terrain(terrain.ptr());
    developer.set_extent_x_bits(64f32.to_bits());
    developer.set_extent_z_bits(64f32.to_bits());
    let rect = [-2., -1., 10., 6.];
    let points = edge_points([0., 0., 10., 6.]);
    assert_eq!(points.len(), 14);
    for lowest in [0u32, 1, 0x1234_0100, 0xFFFF_FF80] {
        for water in [0u32, 1, 0x1234_0100, 0xFFFF_FF80] {
            for (i, &(x, z)) in points.iter().enumerate() {
                expect_float(
                    terrain.ptr(),
                    "position_altitude_body",
                    vec![w(x.to_bits()), w(z.to_bits())],
                    RETURN,
                    F80::from_f32(100. + i as f32),
                );
                if water as u8 != 0 {
                    expect_float(
                        terrain.ptr(),
                        "position_water_body",
                        vec![w(x.to_bits()), w(z.to_bits())],
                        RETURN,
                        F80::from_f32(if i == 1 { 50. } else { 150. + i as f32 }),
                    );
                }
            }
            let expected = match (lowest as u8 != 0, water as u8 != 0) {
                (true, false) => 100.,
                (false, false) => 113.,
                (true, true) => 101.,
                (false, true) => 163.,
            };
            let (value, words) = unsafe { invoke(dev.ptr(), &rect, lowest, water) };
            assert_eq!(value.to_f32(), expected);
            assert_eq!(&words[2..], &[expected.to_bits(), lowest, 10f32.to_bits()]);
            assert_eq!(words[1], (RETURN >> 32) as u32);
            assert_eq!(words[0] & 0xFFFF_0000, RETURN as u32 & 0xFFFF_0000);
            assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
            finished();
        }
    }
}

#[test]
fn metre_altitude_samples_interior_vertices_before_edges_and_ignores_nan_samples() {
    let _fpu = GameFpu::new();
    let _globals = Globals::new();
    let mut dev = mock!(0x3C => vertex_float);
    let mut terrain = mock!(0x34 => position_altitude, 0x78 => position_water);
    let developer = unsafe { LotDeveloper::from_ptr(dev.ptr()) };
    developer.set_terrain(terrain.ptr());
    developer.set_extent_x_bits(128f32.to_bits());
    developer.set_extent_z_bits(128f32.to_bits());
    let rect = [1., 2., 32., 32.];
    for lowest in [false, true] {
        for (i, (x, z)) in [(1, 1), (1, 2), (2, 1), (2, 2)].into_iter().enumerate() {
            expect_float(
                dev.ptr(),
                "vertex_body",
                vec![w(x), w(z), w(0x1234_0080)],
                RETURN,
                F80::from_f32([150., f32::NAN, 50., 100.][i]),
            );
        }
        for (i, (x, z)) in edge_points(rect).into_iter().enumerate() {
            expect_float(
                terrain.ptr(),
                "position_altitude_body",
                vec![w(x.to_bits()), w(z.to_bits())],
                RETURN,
                F80::from_f32(if i == 0 { f32::NAN } else { 100. }),
            );
            expect_float(
                terrain.ptr(),
                "position_water_body",
                vec![w(x.to_bits()), w(z.to_bits())],
                RETURN,
                F80::from_f32(if i == 0 { 1000. } else { f32::NAN }),
            );
        }
        let expected: f32 = if lowest { 50. } else { 150. };
        let (value, words) = unsafe { invoke(dev.ptr(), &rect, u32::from(lowest), 0x1234_0080) };
        assert_eq!(value.to_f32(), expected);
        assert_eq!(words[2], expected.to_bits());
        assert_eq!(words[4], 36f32.to_bits());
        finished();
    }
}

#[test]
fn metre_altitude_reversed_degenerate_and_nan_rectangles_keep_empty_extrema() {
    let _fpu = GameFpu::new();
    let _globals = Globals::new();
    let mut dev = mock!(0x3C => vertex_float);
    let developer = unsafe { LotDeveloper::from_ptr(dev.ptr()) };
    developer.set_extent_x_bits(128f32.to_bits());
    developer.set_extent_z_bits(128f32.to_bits());
    for rect in [[10., 10., 5., 5.], [5., 5., 5., 5.], [f32::NAN; 4]] {
        for lowest in [false, true] {
            if rect[0].is_nan() {
                expect_float(
                    dev.ptr(),
                    "vertex_body",
                    vec![w(0), w(0), w(1)],
                    RETURN,
                    F80::from_f32(f32::NAN),
                );
            }
            let (value, words) = unsafe { invoke(dev.ptr(), &rect, u32::from(lowest), 1) };
            let expected = if lowest { f32::MAX } else { -f32::MAX };
            assert_eq!(value.to_f32(), expected);
            assert_eq!(words[2], expected.to_bits());
            assert_eq!(words[4], rect[1].to_bits());
            finished();
        }
    }
}

#[test]
fn metre_altitude_clamps_upper_extent_and_keeps_the_rectangle_storage_unchanged() {
    let _fpu = GameFpu::new();
    let _globals = Globals::new();
    let mut dev = mock!();
    let mut terrain = mock!(0x34 => position_altitude);
    let developer = unsafe { LotDeveloper::from_ptr(dev.ptr()) };
    developer.set_terrain(terrain.ptr());
    developer.set_extent_x_bits(4f32.to_bits());
    developer.set_extent_z_bits(4f32.to_bits());
    let rect = [-1., -1., 4., f32::INFINITY];
    for (x, z) in edge_points([0., 0., 3.9, 3.9]) {
        expect_float(
            terrain.ptr(),
            "position_altitude_body",
            vec![w(x.to_bits()), w(z.to_bits())],
            RETURN,
            F80::from_f32(75.),
        );
    }
    let (value, _) = unsafe { invoke(dev.ptr(), &rect, 1, 0) };
    assert_eq!(value.to_f32(), 75.);
    assert_eq!(
        rect.map(f32::to_bits),
        [-1f32, -1., 4., f32::INFINITY].map(f32::to_bits)
    );
    finished();
}
