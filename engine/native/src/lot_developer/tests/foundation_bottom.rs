// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;

#[test]
fn foundation_bottom_uses_strict_ordered_minimum_and_releases_all_neighbors() {
    let _fpu = GameFpu::new();
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    unsafe {
        for queried in [0u64, 0x100, 1] {
            for initial in [8.0f32, f32::NAN, f32::INFINITY] {
                let mut dev = mock!(0x44 => lowest_area_float);
                let this = dev.ptr();
                let mut lot = mock!(0x38 => bounding_rect);
                let lot = lot.ptr();
                let mut manager = mock!(0x24 => surrounding_lots);
                let manager = manager.ptr();
                LotDeveloper::from_ptr(this).set_lot_manager(manager);
                let mut members: Vec<_> = (0..5)
                    .map(|_| mock!(0x30 => get_flags, 0x40 => lot_altitude_float, 8 => release))
                    .collect();
                let members: Vec<_> = members.iter_mut().map(Mock::ptr).collect();
                let mut sentinel = [0xA5A5_A5A5u32; 3];
                let head = sentinel.as_mut_ptr().cast::<u8>();
                let mut nodes: Vec<_> = members.iter().map(|&member| [0u32, 0, member as u32]).collect();
                let links: Vec<_> = nodes.iter_mut().map(|node| node.as_mut_ptr() as u32).collect();
                for (i, node) in nodes.iter_mut().enumerate() {
                    node[0] = links.get(i + 1).copied().unwrap_or(head as u32);
                }
                let rect = [1, 2, 3, 4];
                expect(lot, "bounding_rect", vec![out(&rect)], RETURN);
                expect_float(
                    this,
                    "lowest_area_body",
                    vec![Arg::In(rect.to_vec()), w(1), w(1)],
                    RETURN,
                    F80::from_f32(initial),
                );
                expect(core::ptr::null_mut(), "operation_allocate", vec![w(12)], head as u64);
                expect(
                    manager,
                    "surrounding_lots",
                    vec![p(lot), Arg::In(vec![head as u32]), w(15)],
                    queried,
                );
                edit_last(sentinel.as_mut_ptr(), links[0]);
                edit_last(sentinel.as_mut_ptr().add(1), *links.last().unwrap());
                if queried != 0 {
                    for ((&member, flag), altitude) in
                        members
                            .iter()
                            .zip([0x100, 1, 2, 3, 1])
                            .zip([0.0, 8.0, f32::NAN, -3.0, -2.0])
                    {
                        expect(member, "get_flags", vec![], flag);
                        if flag & 3 != 0 {
                            expect_float(member, "lot_altitude_body", vec![], RETURN, F80::from_f32(altitude));
                        }
                    }
                }
                for (&member, &node) in members.iter().zip(&links) {
                    expect(member, "release", vec![], RETURN);
                    expect(core::ptr::null_mut(), "operation_free", vec![w(node)], RETURN);
                }
                expect(core::ptr::null_mut(), "operation_free", vec![p(head)], RETURN);
                let call = call_float(fc4p_006c7630 as *const () as usize, this, &[lot as u32]);
                let expected = if queried != 0 && !initial.is_nan() {
                    -3.0
                } else {
                    initial
                };
                assert_eq!(call.st0.to_f32().to_bits(), expected.to_bits());
                assert_eq!((call.eax, call.edx), (RETURN as u32, (RETURN >> 32) as u32));
                assert_eq!(sentinel, [head as u32, head as u32, 0xA5A5_A5A5]);
                finished();
            }
        }
    }
}

#[test]
fn foundation_bottom_rounds_the_initial_height_before_querying_and_returns_st0() {
    let _fpu = GameFpu::new();
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    unsafe {
        for value in [F80::from_f64(2f64.powi(200)), F80::from_f32(-0.0)] {
            let mut dev = mock!(0x44 => lowest_area_float);
            let this = dev.ptr();
            let mut lot = mock!(0x38 => bounding_rect);
            let lot = lot.ptr();
            let mut manager = mock!(0x24 => surrounding_lots);
            let manager = manager.ptr();
            LotDeveloper::from_ptr(this).set_lot_manager(manager);
            let mut sentinel = [0xA5A5_A5A5u32; 3];
            let head = sentinel.as_mut_ptr().cast::<u8>();
            expect(lot, "bounding_rect", vec![out(&[3, 2, 1, 0])], RETURN);
            expect_float(
                this,
                "lowest_area_body",
                vec![Arg::In(vec![3, 2, 1, 0]), w(1), w(1)],
                RETURN,
                value,
            );
            expect(core::ptr::null_mut(), "operation_allocate", vec![w(12)], head as u64);
            expect(
                manager,
                "surrounding_lots",
                vec![p(lot), Arg::In(vec![head as u32]), w(15)],
                1,
            );
            expect(core::ptr::null_mut(), "operation_free", vec![p(head)], RETURN);
            let call = call_float(fc4p_006c7630 as *const () as usize, this, &[lot as u32]);
            assert_eq!(call.st0.to_bytes(), F80::from_f32(value.to_f32()).to_bytes());
            assert_eq!((call.eax, call.edx), (RETURN as u32, (RETURN >> 32) as u32));
            finished();
        }
    }
}
