// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::rubble::*;

slot!(uniform, bound);
slot!(create, kind, output);
slot!(occupant);
slot!(initialize);
slot!(flags, mask);
slot!(orientation, facing);
slot!(position, input);
slot!(bounds, min, max);
slot!(insert, lot, occupant);
float_slot!(gaussian, gaussian_body, 4, min_lo, min_hi, max_lo, max_hi);

struct Globals {
    _constants: Box<[f32; 2]>,
    _density: Box<f64>,
    random: Box<u8>,
    _manager: Box<*mut u8>,
    _tables: Box<[u32; 18]>,
    _guards: Vec<crate::testing::Fake>,
}
impl Globals {
    fn new(manager: *mut u8) -> Self {
        let mut constants = Box::new([10f32, 12.]);
        let mut density = Box::new(0.0078125f64);
        let mut random = Box::new(0u8);
        let mut manager = Box::new(manager);
        let mut tables = Box::new([
            0x1DCF_0000,
            0x1DD0_0000,
            0x1DD1_0000,
            0x1DD2_0000,
            0x1DD3_0000,
            0x1DD3_0000,
            0x1C48_0000,
            0x1C46_0000,
            0x1C47_0000,
            0x1C49_0000,
            1,
            0,
            0,
            1,
            1,
            1,
            0,
            0,
        ]);
        let guards = vec![
            fake_data(0x00A9_4D50, constants.as_mut_ptr().cast()),
            fake_data(0x00AA_209C, unsafe { constants.as_mut_ptr().add(1).cast() }),
            fake_data(0x00AB_1B68, (&mut *density as *mut f64).cast()),
            fake_data(0x00B4_9AF0, &mut *random),
            fake_data(0x00B4_3D10, (&mut *manager as *mut *mut u8).cast()),
            fake_data(0x00AB_1AD4, tables.as_mut_ptr().cast()),
            fake_data(0x00AB_1AF4, unsafe { tables.as_mut_ptr().add(6).cast() }),
            fake_data(0x00AB_1AEC, unsafe { tables.as_mut_ptr().add(8).cast() }),
            fake_data(0x00AB_1B54, unsafe { tables.as_mut_ptr().add(10).cast() }),
            fake_data(0x00AB_1B44, unsafe { tables.as_mut_ptr().add(14).cast() }),
        ];
        Self {
            _constants: constants,
            _density: density,
            random,
            _manager: manager,
            _tables: tables,
            _guards: guards,
        }
    }
    fn rng(&mut self) -> *mut u8 {
        &mut *self.random
    }
}

fn uniform_draw(globals: &mut Globals, count: u32, returned: u32) {
    expect(globals.rng(), "uniform", vec![w(count)], u64::from(returned));
}
fn gaussian_draw(globals: &mut Globals, min: f64, max: f64, returned: f32) {
    let lo = min.to_bits();
    let hi = max.to_bits();
    expect_float(
        globals.rng(),
        "gaussian_body",
        vec![w(lo as u32), w((lo >> 32) as u32), w(hi as u32), w((hi >> 32) as u32)],
        RETURN,
        F80::from_f32(returned),
    );
}

unsafe fn invoke(min: &[f32; 3], max: &[f32; 3], burnt: u32) -> (u32, u32) {
    unsafe {
        let args = [0x1234, min.as_ptr() as u32, max.as_ptr() as u32, burnt];
        let mut call = RawCall::new(Conv::Stdcall, fc4p_006c4bd0 as *const () as usize, 0xDEAD_BEEF, &args);
        call_raw(&mut call);
        (call.eax, call.edx)
    }
}

#[test]
fn rubble_count_and_corner_thresholds_keep_float_to_integer_and_low_word_wrapping() {
    let _fpu = GameFpu::new();
    let _uniform = fake(0x0091_36B1, uniform as *const () as usize);
    let _gaussian = fake(0x0091_381F, gaussian as *const () as usize);
    let mut manager = mock!(0x3C => create);
    let mut globals = Globals::new(manager.ptr());
    for (max, count, corners, interiors) in [
        ([8f32, 0., 8.], 4i32, false, 0),
        ([11., 0., 11.], 4, true, 0),
        ([16., 0., 16.], 6, true, 2),
        ([f32::NAN, 0., 11.], 4, true, 0),
        ([f32::from_bits(0x7F80_0001), 0., 11.], 4, true, 0),
        ([524_288., 0., 1_048_576.], 4, true, 0),
        ([f32::INFINITY, 0., f32::INFINITY], 4, true, 0),
        ([-32., 0., 32.], -4, false, 0),
    ] {
        if corners {
            for _ in 0..4 {
                uniform_draw(&mut globals, 2, 0);
            }
        }
        for _ in 0..interiors {
            uniform_draw(&mut globals, 4, 1);
            uniform_draw(&mut globals, 6, 2);
            gaussian_draw(&mut globals, 0., f64::from(max[0]), 2.);
            gaussian_draw(&mut globals, 0., f64::from(max[2]), 3.);
            expect(
                manager.ptr(),
                "create",
                vec![w(0x1DD1_0000), out(&[0x9876])],
                RETURN & !255,
            );
        }
        let actual = unsafe { invoke(&[0.; 3], &max, 0x100) };
        assert_eq!(actual.0, count as u32);
        finished();
    }
}

#[test]
fn rubble_corners_select_tables_and_orientations_and_release_failed_initialization() {
    let _fpu = GameFpu::new();
    let _uniform = fake(0x0091_36B1, uniform as *const () as usize);
    let _gaussian = fake(0x0091_381F, gaussian as *const () as usize);
    let mut manager = mock!(0x3C => create);
    let mut prop = mock!(0x0C => occupant, 8 => release);
    let mut occ = mock!(0x0C => initialize);
    let mut globals = Globals::new(manager.ptr());
    for (width, burnt) in [(10f32, 0u32), (12., 0), (13., 0), (13., 0x80)] {
        // 13 by 10 is large in x only: its corners still use type zero without a second draw.
        let max = [width, 0., 10.];
        for _ in 0..4 {
            uniform_draw(&mut globals, 2, 1);
            expect(
                manager.ptr(),
                "create",
                vec![
                    w(if burnt == 0 { 0x1DCF_0000 } else { 0x1C48_0000 }),
                    out(&[prop.ptr() as u32]),
                ],
                RETURN | 1,
            );
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            expect(occ.ptr(), "initialize", vec![], RETURN & !255);
            expect(prop.ptr(), "release", vec![], RETURN);
        }
        if width == 13. {
            // Area 130 adds one interior prop, including both Gaussian draws before creation.
            uniform_draw(&mut globals, 4, 2);
            uniform_draw(&mut globals, if burnt != 0 { 2 } else { 6 }, 0);
            gaussian_draw(&mut globals, 0., f64::from(width), 3.);
            gaussian_draw(&mut globals, 0., 10., 4.);
            expect(
                manager.ptr(),
                "create",
                vec![
                    w(if burnt == 0 { 0x1DCF_0000 } else { 0x1C47_0000 }),
                    out(&[prop.ptr() as u32]),
                ],
                RETURN & !255,
            );
        }
        assert_eq!(
            unsafe { invoke(&[0.; 3], &max, burnt) }.0,
            if width == 13. { 5 } else { 4 }
        );
        finished();
    }
}

#[test]
fn rubble_success_fits_both_axes_then_inserts_and_releases_each_prop_in_order() {
    let _fpu = GameFpu::new();
    let _uniform = fake(0x0091_36B1, uniform as *const () as usize);
    let _gaussian = fake(0x0091_381F, gaussian as *const () as usize);
    let mut manager = mock!(0x3C => create, 0x64 => insert);
    let mut replacement = mock!(0x64 => insert);
    let mut prop = mock!(0x0C => occupant, 0x28 => orientation, 8 => release);
    let mut occ = mock!(0x0C => initialize, 0x74 => flags, 0x24 => position, 0x28 => bounds);
    let mut globals = Globals::new(manager.ptr());
    for burnt in [0u32, 0x1234_0080] {
        *globals._manager = manager.ptr();
        for i in 0..6 {
            let (kind, facing, initial) = if i < 4 {
                uniform_draw(&mut globals, 2, 1);
                let choice = i % 2;
                uniform_draw(&mut globals, if burnt != 0 { 2 } else { 6 }, choice);
                let kind = match (burnt != 0, choice) {
                    (false, 0) => 0x1DCF_0000,
                    (false, _) => 0x1DD0_0000,
                    (true, 0) => 0x1C48_0000,
                    (true, _) => 0x1C46_0000,
                };
                let facing = if choice == 0 { (i + 2) % 4 } else { i };
                let initial = [
                    if i == 0 || i == 3 { 16f32 } else { 0. },
                    5.,
                    if i < 2 { 16. } else { 0. },
                ];
                (kind, facing, initial)
            } else {
                uniform_draw(&mut globals, 4, 3);
                uniform_draw(&mut globals, if burnt != 0 { 2 } else { 6 }, 1);
                gaussian_draw(&mut globals, 0., 16., 8.);
                gaussian_draw(&mut globals, 0., 16., 9.);
                (if burnt != 0 { 0x1C49_0000 } else { 0x1DD0_0000 }, 3, [8., 5., 9.])
            };
            expect(
                manager.ptr(),
                "create",
                vec![w(kind), out(&[prop.ptr() as u32])],
                RETURN | 1,
            );
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            expect(occ.ptr(), "initialize", vec![], RETURN | 1);
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            expect(occ.ptr(), "flags", vec![w(0x40)], RETURN);
            expect(prop.ptr(), "orientation", vec![w(facing)], RETURN);
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            expect(
                occ.ptr(),
                "position",
                vec![Arg::In(initial.map(f32::to_bits).to_vec())],
                RETURN,
            );
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            let (lo, hi, correction) = match i {
                2 => ([1f32, -100., 1.], [15., 100., 15.], [0., 0.]),
                5 => ([f32::NAN; 3], [f32::NAN; 3], [0., 0.]),
                0 | 4 => ([-2., -100., -3.], [15., 100., 15.], [2., 3.]),
                _ => ([1., -100., 1.], [18., 100., 19.], [-2., -3.]),
            };
            expect(
                occ.ptr(),
                "bounds",
                vec![out(&lo.map(f32::to_bits)), out(&hi.map(f32::to_bits))],
                RETURN,
            );
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            let fitted = [initial[0] + correction[0], initial[1], initial[2] + correction[1]];
            expect(
                occ.ptr(),
                "position",
                vec![Arg::In(fitted.map(f32::to_bits).to_vec())],
                RETURN,
            );
            if i == 5 {
                edit_last(
                    (&mut *globals._manager as *mut *mut u8).cast(),
                    replacement.ptr() as u32,
                );
            }
            expect(prop.ptr(), "occupant", vec![], occ.ptr() as u64);
            expect(
                if i == 5 { replacement.ptr() } else { manager.ptr() },
                "insert",
                vec![w(0x1234), p(occ.ptr())],
                RETURN,
            );
            expect(prop.ptr(), "release", vec![], RETURN);
        }
        let returned = unsafe { invoke(&[0., 5., 0.], &[16., 0., 16.], burnt) };
        assert_eq!(returned, (6, (RETURN >> 32) as u32));
        finished();
    }
}

#[test]
fn zero_rubble_types_skip_creation_after_draws_and_small_lots_still_place_interiors() {
    let _fpu = GameFpu::new();
    let _uniform = fake(0x0091_36B1, uniform as *const () as usize);
    let _gaussian = fake(0x0091_381F, gaussian as *const () as usize);
    let mut globals = Globals::new(core::ptr::null_mut());
    globals._tables[0] = 0;
    for _ in 0..4 {
        uniform_draw(&mut globals, 2, 1);
    }
    assert_eq!(unsafe { invoke(&[0.; 3], &[11., 0., 11.], 0) }.0, 4);
    finished();
    for (x, z) in [(8f32, 32f32), (32., 8.)] {
        for _ in 0..2 {
            uniform_draw(&mut globals, 4, 2);
            uniform_draw(&mut globals, 6, 0);
            gaussian_draw(&mut globals, 0., f64::from(x), 2.);
            gaussian_draw(&mut globals, 0., f64::from(z), 3.);
        }
        assert_eq!(unsafe { invoke(&[0.; 3], &[x, 0., z], 0) }.0, 6);
        finished();
    }
}
