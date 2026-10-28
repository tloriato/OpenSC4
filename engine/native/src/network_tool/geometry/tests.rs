// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Altitude(usize, u32),
    Flag(usize, u32, u32),
    Collect(usize),
    Type(u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    altitudes: Vec<f64>,
    enabled: u32,
    swap: Option<(usize, usize, usize)>,
    counts: (u32, u32),
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn altitude(this: *mut u8, vertex: u32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Altitude(this as usize, vertex));
        if let Some((tool, terrain, cell)) = s.swap.take() {
            unsafe {
                write(tool as *mut u8, 0x44, terrain as u32);
                write(cell as *mut u8, 0x58, 5.0f32);
            }
        }
        s.altitudes.remove(0)
    })
}
extern "thiscall" fn enabled(this: *mut u8, flag: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Flag(this as usize, 0x20, flag));
        registers(s.enabled, 0x1234_5678)
    })
}
extern "thiscall" fn set(this: *mut u8, flag: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Flag(this as usize, 0x28, flag)));
    registers(0xAABB_CC01, 0xDEAD_BEEF)
}
extern "thiscall" fn clear(this: *mut u8, flag: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Flag(this as usize, 0x2C, flag)));
    registers(0xCCDD_EE02, 0x1234_ABCD)
}
extern "thiscall" fn collect(this: *mut u8, _two: *mut u32, _one: *mut u32, n_two: *mut u32, n_one: *mut u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Collect(this as usize));
        unsafe {
            *n_two = s.counts.0;
            *n_one = s.counts.1;
        }
    });
    registers(0xABCD_EF00, 0x1234_5678)
}
extern "C" fn find_type(flags: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Type(flags)));
    flags
}
fn cdecl(function: usize, arguments: [u32; 2], edx: u32) -> [u32; 4] {
    let mut frame = [function as u32, arguments[0], arguments[1], 0, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 8]", "push dword ptr [edi + 4]", "call dword ptr [edi]",
            "mov dword ptr [edi + 12], eax", "mov dword ptr [edi + 16], edx",
            "pop dword ptr [edi + 20]", "pop dword ptr [edi + 24]",
            in("edi") frame.as_mut_ptr(), inout("edx") edx => _, clobber_abi("C"),
        );
    }
    [frame[3], frame[4], frame[5], frame[6]]
}
#[test]
fn elevation_threshold_equalities_nan_and_maximum_of_all_four_corners() {
    let _fpu = GameFpu::new();
    let mut epsilon = 0.01f32;
    let _e = fake_data(0x00AA_8424, (&mut epsilon as *mut f32).cast());
    let mut table = [0usize; 13];
    table[12] = altitude as *const () as usize;
    let mut terrain = [table.as_ptr() as usize];
    let mut object = Object::new(0x360);
    let p = object.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    t.set_terrain(terrain.as_mut_ptr().cast());
    unsafe {
        write(p, 0x2C8, 10.0f32);
    }
    let mut cell = [0u32; 0xBC / 4];
    for i in 0..4 {
        cell[0xA8 / 4 + i] = 100 + i as u32;
    }
    for (heights, expected) in [
        ([-1., 0., 0.01, -10.], 0),
        ([1., 9., -1., 0.], 1),
        ([1., 10., 0., 0.], 2),
        ([f32::NAN, f32::INFINITY, 0., 0.], 2),
        ([f32::NAN; 4], 0),
    ] {
        for i in 0..4 {
            cell[0x58 / 4 + i] = heights[i].to_bits();
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                altitudes: vec![0.; 4],
                ..State::default()
            }
        });
        assert_eq!(unsafe { fc4p_00623bb0(p, cell.as_mut_ptr().cast()) }, expected);
        assert_eq!(
            calls(),
            (0..4)
                .map(|i| Call::Altitude(terrain.as_ptr() as usize, 100 + i))
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn elevation_reads_height_after_callback_reloads_terrain_and_retains_st0_precision() {
    let _fpu = GameFpu::new();
    let mut epsilon = 0.0f32;
    let _e = fake_data(0x00AA_8424, (&mut epsilon as *mut f32).cast());
    let mut table = [0usize; 13];
    table[12] = altitude as *const () as usize;
    let mut a = [table.as_ptr() as usize];
    let mut b = a;
    let mut obj = Object::new(0x360);
    let p = obj.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    t.set_terrain(a.as_mut_ptr().cast());
    unsafe {
        write(p, 0x2C8, 1.0f32);
    }
    let mut cell = [0u32; 0xBC / 4];
    STATE.with(|s| {
        *s.borrow_mut() = State {
            altitudes: vec![0., 0., 0., 0.],
            swap: Some((p as usize, b.as_mut_ptr() as usize, cell.as_mut_ptr() as usize)),
            ..State::default()
        }
    });
    assert_eq!(unsafe { fc4p_00623bb0(p, cell.as_mut_ptr().cast()) }, 2);
    assert_eq!(
        calls(),
        [
            Call::Altitude(a.as_ptr() as usize, 0),
            Call::Altitude(b.as_ptr() as usize, 0),
            Call::Altitude(b.as_ptr() as usize, 0),
            Call::Altitude(b.as_ptr() as usize, 0)
        ]
    );
    cell[0x58 / 4] = 1.0f32.to_bits();
    unsafe {
        write(p, 0x2C8, 0.0001f32);
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            altitudes: vec![0.999_999_99, 0., 0., 0.],
            ..State::default()
        }
    });
    assert_eq!(unsafe { fc4p_00623bb0(p, cell.as_mut_ptr().cast()) }, 1);
}
#[test]
fn padded_rectangle_clamps_unsigned_boundaries_wraps_sums_and_preserves_guards() {
    let mut object = Object::new(0x360);
    let p = object.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    for (w, h) in [(64, 128), (1, 1), (0, 0), (u32::MAX, u32::MAX)] {
        t.set_width(w);
        t.set_height(h);
        for (x, z, padding) in [
            (5u32, 7u32, 2u32),
            (0, 0, 0),
            (0, 0, 2),
            (63, 127, 2),
            (u32::MAX, u32::MAX, 2),
            (1, 1, u32::MAX),
        ] {
            let mut rectangle = [0xDEAD_BEEFu32; 6];
            let r = unsafe { rectangle.as_mut_ptr().add(1).cast() };
            assert_eq!(unsafe { fc4p_00623c20(p, x, z, r, padding) }, r);
            let max_x = x.wrapping_add(padding);
            let max_z = z.wrapping_add(padding);
            assert_eq!(
                rectangle,
                [
                    0xDEAD_BEEF,
                    x.saturating_sub(padding),
                    z.saturating_sub(padding),
                    if max_x < w { max_x } else { w.wrapping_sub(1) },
                    if max_z < h { max_z } else { h.wrapping_sub(1) },
                    0xDEAD_BEEF
                ]
            );
        }
    }
}
#[test]
fn simple_connections_overwrites_both_cdecl_arguments_and_keeps_collector_edx() {
    let _f = fake(0x0062_0DF0, collect as *const () as usize);
    let mut record = [0u32; 30];
    for n_two in 0..5 {
        for n_one in 0..5 {
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    counts: (n_two, n_one),
                    ..State::default()
                }
            });
            let result = cdecl(
                fc4p_00623c90 as *const () as usize,
                [record.as_mut_ptr() as u32, 7],
                0xAABB_CCDD,
            );
            assert_eq!(
                result,
                [
                    u32::from(n_two == 0 && (n_one == 1 || n_one == 2)),
                    0x1234_5678,
                    n_one,
                    n_two
                ]
            );
            assert_eq!(calls(), [Call::Collect(record.as_ptr() as usize + 7 * 4 + 0x18)]);
        }
    }
}
#[test]
fn quad_flag_operations_cover_all_patterns_and_disabled_path_skips_pattern_read() {
    let mut table = [0usize; 12];
    table[8] = enabled as *const () as usize;
    table[10] = set as *const () as usize;
    table[11] = clear as *const () as usize;
    let mut occupant = [table.as_ptr() as usize];
    let p = occupant.as_mut_ptr().cast::<u8>();
    type PatternGroup = (Vec<u32>, &'static [(u32, u32)]);
    let groups: &[PatternGroup] = &[
        (
            vec![9, 0x301, 0x309, 0xB00, 0xB01],
            &[(0x28, 0x1000000), (0x28, 0x2000000), (0x2C, 0x4000000)],
        ),
        (
            vec![0xB, 0x1000003, 0x100000B, 0x9000000, 0x9000003],
            &[(0x28, 0x2000000), (0x2C, 0x4000000)],
        ),
        (
            vec![0x900, 0x30100, 0x30900, 0xB0000, 0xB0100],
            &[(0x28, 0x4000000), (0x2C, 0x2000000)],
        ),
        (
            vec![0x90000, 0x3010000, 0x3090000, 0xB000000, 0xB010000],
            &[(0x28, 0x1000000), (0x28, 0x4000000), (0x2C, 0x2000000)],
        ),
        (
            vec![
                0,
                8,
                10,
                12,
                0x300,
                0x302,
                0x308,
                0x30A,
                0xAFF,
                0xB02,
                0x1000004,
                u32::MAX,
            ],
            &[(0x28, 0x2000000), (0x28, 0x4000000)],
        ),
    ];
    for enabled in [0u32, 0x100, 0xABCDEF01] {
        for (patterns, ops) in groups {
            for &pattern in patterns {
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        enabled,
                        ..State::default()
                    }
                });
                let mut pattern = pattern;
                let mut c = RawCall::new(Conv::Thiscall, fc4p_00623ce0 as *const () as usize, 0, &[]);
                let args = [
                    p as u32,
                    if enabled as u8 == 0 {
                        0
                    } else {
                        &mut pattern as *mut u32 as u32
                    },
                ];
                c.args = args.as_ptr();
                c.count = 2;
                c.edx = 0x9876_5432;
                unsafe {
                    call_raw(&mut c);
                }
                let ops = if enabled as u8 == 0 {
                    &[(0x28, 0x2000000), (0x28, 0x4000000)][..]
                } else {
                    ops
                };
                let mut expected = vec![Call::Flag(p as usize, 0x20, 0x40000000)];
                expected.extend(ops.iter().map(|&(slot, flag)| Call::Flag(p as usize, slot, flag)));
                assert_eq!(calls(), expected);
                assert_eq!(
                    (c.eax, c.edx),
                    if ops.last().unwrap().0 == 0x28 {
                        (0xAABB_CC01, 0xDEAD_BEEF)
                    } else {
                        (0xCCDD_EE02, 0x1234_ABCD)
                    }
                );
            }
        }
    }
}
#[test]
fn mask_lookup_uses_table_order_only_twelve_entries_and_keeps_edx() {
    let mut flags = [0u32; 13];
    for (i, f) in flags.iter_mut().enumerate() {
        *f = 1 << i;
    }
    flags[2] = 1 << 8;
    flags[8] = 1 << 2;
    let _f = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    for mask in [0u32, 1, 2, 4, 256, 1 << 11, 1 << 12, 1 << 13, u32::MAX, 0x804] {
        let expected = (0..12).find(|&i| flags[i] & mask != 0).unwrap_or(12);
        assert_eq!(
            cdecl(fc4p_00623e60 as *const () as usize, [mask, 0xAABB_CCDD], 0x1234_5678),
            [expected as u32, 0x1234_5678, mask, 0xAABB_CCDD]
        );
    }
}
#[test]
fn first_network_type_isolates_lowest_set_bit_overwrites_argument_and_calls_game_address() {
    let _f = fake(0x0062_3E60, find_type as *const () as usize);
    for flags in [0u32, 1, 7, 0x804, 0x80000000, u32::MAX] {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let first = flags & flags.wrapping_neg();
        assert_eq!(
            cdecl(fc4p_00623ec0 as *const () as usize, [flags, 0xAABB_CCDD], 0x1234_5678),
            [first, 0x1234_5678, first, 0xAABB_CCDD]
        );
        assert_eq!(calls(), [Call::Type(first)]);
    }
}
#[test]
fn support_lookup_checks_collisions_empty_buckets_missing_keys_and_unsigned_ids() {
    let mut object = Object::new(0x360);
    let p = object.ptr();
    let t = unsafe { NetworkTool::from_ptr(p) };
    let mut info = Object::new(0x50);
    t.set_type_info(info.ptr());
    let mut a = [0u32, 1, 0xABCD, 0xDCBA];
    let mut b = [0u32, 4, 0x1234, 0x4321];
    let mut c = [0u32, u32::MAX, 0x5678, 0x8765];
    a[0] = b.as_mut_ptr() as u32;
    let mut buckets = [c.as_mut_ptr() as u32, a.as_mut_ptr() as u32, 0];
    unsafe {
        write(info.ptr(), 0x40, buckets.as_mut_ptr() as u32);
        write(info.ptr(), 0x44, buckets.as_mut_ptr().add(3) as u32);
    }
    for (key, expected) in [
        (1, a.as_mut_ptr()),
        (4, b.as_mut_ptr()),
        (u32::MAX, c.as_mut_ptr()),
        (0, core::ptr::null_mut()),
        (2, core::ptr::null_mut()),
        (7, core::ptr::null_mut()),
    ] {
        assert_eq!(
            unsafe { fc4p_00625860(p, key) },
            if expected.is_null() {
                core::ptr::null_mut()
            } else {
                unsafe { expected.add(2).cast() }
            }
        );
    }
}
