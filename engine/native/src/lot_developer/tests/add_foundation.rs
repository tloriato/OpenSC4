// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

slot!(foundation_center, center);
slot!(foundation_facing);
slot!(create_foundation, kind, purpose, position, width, depth, facing, bottom);
slot!(set_altitude, altitude);
slot!(internal_level, rect, target, water, force, lot);
slot!(border_level, rect, target, mode, lot);
float_slot!(bottom_float, bottom_body, 1, lot);
float_slot!(pick_float, pick_body, 4, lot, bottom, highest, corners);

unsafe extern "thiscall" fn get_corners(this: *mut u8, rect: u32, output: u32, water: u32) -> u64 {
    run(this, "get_corners", &[rect, output, water])
}

unsafe extern "C" fn add_wall(lot: u32, enabled: u32) -> u64 {
    run(core::ptr::null_mut(), "add_wall", &[lot, enabled])
}

unsafe extern "thiscall" fn update_foundations(this: *mut u8, lot: u32) -> u64 {
    run(this, "update_foundations", &[lot])
}

unsafe extern "C" fn update_walls(lot: u32) -> u64 {
    run(core::ptr::null_mut(), "update_walls", &[lot])
}

#[allow(clippy::too_many_arguments)]
unsafe extern "thiscall" fn create_and_adjust(
    this: *mut u8,
    kind: u32,
    purpose: u32,
    position: u32,
    width: u32,
    depth: u32,
    facing: u32,
    bottom: u32,
) -> u64 {
    let returned = run(
        this,
        "create_and_adjust",
        &[kind, purpose, position, width, depth, facing, bottom],
    );
    unsafe {
        (position as *mut f32).add(1).write(250.0);
    }
    returned
}

#[test]
fn lot_foundation_selects_altitude_levels_before_creation_and_always_updates_flags() {
    let _bottom = fake(0x006C_7630, bottom_float as *const () as usize);
    let _corners = fake(0x006C_4360, get_corners as *const () as usize);
    let _pick = fake(0x006C_5360, pick_float as *const () as usize);
    let _wall = fake(0x005E_3070, add_wall as *const () as usize);
    let _foundations = fake(0x006C_7720, update_foundations as *const () as usize);
    let _walls = fake(0x005E_3960, update_walls as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        for rect in [[2i32, 3, 2, 3], [2, 3, 4, 4], [2, 3, 4, 5]] {
            for use_lot in [0, 0x100, 0x101] {
                for level in [0, 0x101] {
                    for walls in [0, 0x100, 0x101] {
                        let mut dev = mock!(0x44 => lowest_area_float, 0x4C => internal_level, 0x50 => border_level);
                        let this = dev.ptr();
                        let mut lot = mock!(0x3C => foundation_center, 0x38 => bounding_rect, 0x5C => foundation_facing,
                            0x40 => lot_altitude_float, 0x44 => set_altitude, 0x30 => get_flags, 0x34 => set_flags);
                        let lot = lot.ptr();
                        let mut bds = mock!(0x5C => create_foundation);
                        let bds = bds.ptr();
                        LotDeveloper::from_ptr(this).set_building_development(bds);
                        let mut half = 0.5f32;
                        let _half = fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast());
                        let r = rect.map(|n| n as u32).to_vec();
                        expect(
                            lot,
                            "foundation_center",
                            vec![out(&[40.0f32.to_bits(), 999.0f32.to_bits(), 56.0f32.to_bits()])],
                            RETURN,
                        );
                        expect(lot, "bounding_rect", vec![out(&rect.map(|n| n as u32))], RETURN);
                        expect(lot, "foundation_facing", vec![], 3);
                        expect_float(this, "bottom_body", vec![p(lot)], RETURN, F80::from_f32(80.0));
                        expect_float(
                            this,
                            "lowest_area_body",
                            vec![Arg::In(r.clone()), w(0), w(1)],
                            RETURN,
                            F80::from_f32(150.0),
                        );
                        if use_lot as u8 != 0 {
                            expect_float(lot, "lot_altitude_body", vec![], RETURN, F80::from_f32(100.0));
                        } else {
                            expect(
                                this,
                                "get_corners",
                                vec![
                                    Arg::In(r.clone()),
                                    out(&[
                                        100.0f32.to_bits(),
                                        110.0f32.to_bits(),
                                        90.0f32.to_bits(),
                                        95.0f32.to_bits(),
                                    ]),
                                    w(1),
                                ],
                                RETURN,
                            );
                            expect_float(
                                this,
                                "pick_body",
                                vec![
                                    p(lot),
                                    w(80.0f32.to_bits()),
                                    w(150.0f32.to_bits()),
                                    Arg::In(vec![
                                        100.0f32.to_bits(),
                                        110.0f32.to_bits(),
                                        90.0f32.to_bits(),
                                        95.0f32.to_bits(),
                                    ]),
                                ],
                                RETURN,
                                F80::from_f32(100.0),
                            );
                        }
                        if level as u8 != 0 {
                            if rect[2] - rect[0] > 1 && rect[3] - rect[1] > 1 {
                                expect(
                                    this,
                                    "internal_level",
                                    vec![Arg::In(r.clone()), w(99.5f32.to_bits()), w(1), w(1), p(lot)],
                                    RETURN,
                                );
                            }
                            expect(
                                this,
                                "border_level",
                                vec![Arg::In(r), w(100.0f32.to_bits()), w(1), p(lot)],
                                RETURN,
                            );
                        }
                        let created = if use_lot as u8 != 0 { 0xAABB_0180 } else { 0xAABB_0100 };
                        let width = F80::from_i32((rect[2] - rect[0] + 1) * 16).to_f32().to_bits();
                        let depth = F80::from_i32((rect[3] - rect[1] + 1) * 16).to_f32().to_bits();
                        expect(
                            bds,
                            "create_foundation",
                            vec![
                                w(0x1234_5678),
                                w(2),
                                Arg::In(vec![40.0f32.to_bits(), 100.0f32.to_bits(), 56.0f32.to_bits()]),
                                w(width),
                                w(depth),
                                w(3),
                                w(80.0f32.to_bits()),
                            ],
                            created,
                        );
                        expect(lot, "set_altitude", vec![w(100.0f32.to_bits())], RETURN);
                        expect(lot, "get_flags", vec![], 0xFFFF_FFFC);
                        expect(lot, "set_flags", vec![w(0xFFFF_FFFB)], RETURN);
                        if walls as u8 != 0 {
                            expect(core::ptr::null_mut(), "add_wall", vec![p(lot), w(1)], RETURN);
                            expect(this, "update_foundations", vec![p(lot)], RETURN);
                            expect(core::ptr::null_mut(), "update_walls", vec![p(lot)], RETURN);
                        }
                        let call = fc4p_006c8ab0(this, 0xCAFE_F00D, lot, 0x1234_5678, use_lot, level, walls);
                        assert_eq!(call, (RETURN & !255) | (created & 255));
                        finished();
                    }
                }
            }
        }
    }
}

#[test]
fn lot_foundation_walls_require_a_strict_ordered_height_difference_and_return_creation_al() {
    let _bottom = fake(0x006C_7630, bottom_float as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        for (highest, top, adjust) in [
            (100.0, 100.0, false),
            (100.0, 120.0, false),
            (f32::NAN, 100.0, false),
            (100.0, f32::NAN, false),
            (150.0, 100.0, true),
        ] {
            let mut dev = mock!(0x44 => lowest_area_float);
            let this = dev.ptr();
            let mut lot = mock!(0x3C => foundation_center, 0x38 => bounding_rect, 0x5C => foundation_facing,
                0x40 => lot_altitude_float, 0x44 => set_altitude, 0x30 => get_flags, 0x34 => set_flags);
            let lot = lot.ptr();
            let mut bds = if adjust {
                mock!(0x5C => create_and_adjust)
            } else {
                mock!(0x5C => create_foundation)
            };
            let bds = bds.ptr();
            LotDeveloper::from_ptr(this).set_building_development(bds);
            expect(lot, "foundation_center", vec![out(&[0; 3])], RETURN);
            expect(
                lot,
                "bounding_rect",
                vec![out(&[i32::MAX as u32, 0, i32::MAX as u32, 0])],
                RETURN,
            );
            expect(lot, "foundation_facing", vec![], 7);
            expect_float(this, "bottom_body", vec![p(lot)], RETURN, F80::from_f32(-10.0));
            expect_float(
                this,
                "lowest_area_body",
                vec![Arg::In(vec![i32::MAX as u32, 0, i32::MAX as u32, 0]), w(0), w(1)],
                RETURN,
                F80::from_f32(highest),
            );
            expect_float(lot, "lot_altitude_body", vec![], RETURN, F80::from_f32(top));
            expect(
                bds,
                if adjust {
                    "create_and_adjust"
                } else {
                    "create_foundation"
                },
                vec![
                    w(0),
                    w(2),
                    Arg::In(vec![0, top.to_bits(), 0]),
                    w(16.0f32.to_bits()),
                    w(16.0f32.to_bits()),
                    w(7),
                    w((-10.0f32).to_bits()),
                ],
                0x80,
            );
            let final_height = if adjust { 250.0f32 } else { top };
            expect(lot, "set_altitude", vec![w(final_height.to_bits())], RETURN);
            expect(lot, "get_flags", vec![], 0);
            expect(lot, "set_flags", vec![w(3)], RETURN);
            let args = [lot as u32, 0, 1, 0, 1];
            let mut call = RawCall::new(Conv::Thiscall, fc4p_006c8ab0 as *const () as usize, this as u32, &args);
            call_raw(&mut call);
            assert_eq!(call.eax & 0xFFFF_00FF, (RETURN as u32 & 0xFFFF_0000) | 0x80);
            assert_eq!(call.edx, (RETURN >> 32) as u32);
            finished();
        }
    }
}
