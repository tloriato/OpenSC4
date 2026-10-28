// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

thread_local! {
    static CALLS: RefCell<Vec<(usize, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
}

fn record(this: *mut u8, args: &[u32]) {
    CALLS.with(|calls| calls.borrow_mut().push((this as usize, args.to_vec())));
}
fn calls() -> Vec<(usize, Vec<u32>)> {
    CALLS.with(|calls| core::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn addref(this: *mut u8) -> u32 {
    record(this, &[]);
    // SAFETY: the test supplies a simulator at its original offsets.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        sim.set_references(sim.references().wrapping_add(1));
    }
    0xAABB_CC55
}

#[test]
fn identity_and_interface_offsets() {
    let mut object = Object::new(0xE4);
    let p = object.ptr();
    let mut table = [0usize; 60];
    table[1] = addref as *const () as usize;
    // SAFETY: all declared simulator fields are within the allocated buffer.
    unsafe {
        let sim = BuildingSim::from_ptr(p);
        sim.set_vtable(table.as_ptr());
        let mut id_call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            fc4p_00695280 as *const () as usize,
            p as u32,
            &[],
        );
        id_call.edx = 0x8765_4321;
        crate::abi::call_raw(&mut id_call);
        assert_eq!((id_call.eax, id_call.edx), (0x8990_BE7A, 0x8765_4321));
        for (iid, offset) in [
            (1, 0),
            (0x0781_3EAE, 0),
            (0xE98B_2F57, 4),
            (0x4522_94AA, 8),
            (0xE4FD_A3D4, 12),
        ] {
            let mut out = core::ptr::null_mut();
            assert_eq!(fc4p_00695290(p, iid, &mut out), 0xAABB_CC01);
            assert_eq!(out, p.add(offset));
        }
        assert_eq!(sim.references(), 5);
        assert_eq!(calls(), vec![(p as usize, vec![]); 5]);
        for iid in [0u32, 2, 0x0781_3EAD, 0x4522_94A9, 0x4522_94AB, 0xFFFF_FFFF] {
            let mut out = 0x1234usize as *mut u8;
            let expected = if iid < 0x4522_94AA {
                iid.wrapping_sub(0x0781_3EAE)
            } else {
                iid
            } & !255;
            assert_eq!(fc4p_00695290(p, iid, &mut out), expected);
            assert_eq!(out as usize, 0x1234);
        }
        assert!(calls().is_empty());
    }
}

#[test]
fn developer_types_and_wealth() {
    // The twelve categories and invalid combinations from the core helper tests.
    let types = [
        (1, 1, 0x1010),
        (1, 2, 0x1020),
        (1, 3, 0x1030),
        (2, 1, 0x3110),
        (2, 2, 0x3120),
        (2, 3, 0x3130),
        (3, 2, 0x3320),
        (3, 3, 0x3330),
        (5, 1, 0x4100),
        (6, 2, 0x4200),
        (7, 2, 0x4300),
        (8, 3, 0x4400),
    ];
    // SAFETY: both exports operate only on their integer arguments.
    unsafe {
        for (purpose, wealth, kind) in types {
            assert_eq!(fc4p_00695790(purpose, wealth), kind);
            assert_eq!(fc4p_00695840(kind as i32), wealth);
        }
        for p in [0, 4, 9, u32::MAX] {
            assert_eq!(fc4p_00695790(p, 2), 0);
        }
        for w in [0, 4, u32::MAX] {
            for p in 1..=3 {
                assert_eq!(fc4p_00695790(p, w), 0);
            }
            for p in 5..=8 {
                assert_eq!(fc4p_00695790(p, w), 0x4100 + (p - 5) * 0x100);
            }
        }
        assert_eq!(fc4p_00695790(3, 1), 0);
        for kind in [0, 0x1234, 0x1000, 0x1040, i32::MIN, i32::MAX, -1] {
            assert_eq!(fc4p_00695840(kind), 0);
        }
    }
}

#[test]
fn zone_fits_and_full_integer_results() {
    // SAFETY: the zoning helpers read only their integer arguments.
    unsafe {
        for stage in [i32::MIN, -1, 0, 3, 4, 6, 7, i32::MAX] {
            for zone in [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 256, i32::MIN] {
                let expected = match zone {
                    1 | 4 => u32::from(stage <= 3),
                    2 | 5 => u32::from(stage <= 6),
                    3 | 6..=9 => 1,
                    _ => (zone as u32).wrapping_sub(1) & !255,
                };
                assert_eq!(fc4p_00695340(stage, zone), expected);
            }
        }
        for building in -1..=16 {
            for cell in -1..=16 {
                let expected = building == cell
                    || matches!(
                        (building, cell),
                        (1, 2 | 3) | (2, 3) | (4, 5 | 6) | (5, 6) | (8, 9) | (0, 11 | 12 | 15)
                    );
                assert_eq!(fc4p_00695390(building, cell) as u8 != 0, expected);
            }
        }
        assert_eq!(fc4p_00695390(0x1234_5602, 0x1234_5602), 0x1234_5601);
        for cell in [2, 5, 9, 11, 12, 15, 7, i32::MIN] {
            assert_eq!(fc4p_00695390(0x1234_56AB, cell), 0x1234_5600);
        }
        assert_eq!(fc4p_00695390(0x1234_56AB, 3), 0);
        assert_eq!(fc4p_00695390(0x1234_56AB, 6), 0);
    }
}

extern "thiscall" fn pair(this: *mut u8, x: i32, z: i32) -> u32 {
    record(this, &[x as u32, z as u32]);
    // SAFETY: test service objects store their configured return value after the table.
    unsafe { read_unaligned(this.add(4).cast()) }
}
extern "thiscall" fn triple(this: *mut u8, x: i32, z: i32, exact: u32) -> *mut u8 {
    record(this, &[x as u32, z as u32, exact]);
    // SAFETY: the same test service layout.
    unsafe { read_unaligned(this.add(4).cast()) }
}
extern "thiscall" fn noargs(this: *mut u8) -> u32 {
    record(this, &[]);
    // SAFETY: the same test service layout.
    unsafe { read_unaligned(this.add(4).cast()) }
}

#[test]
fn construction_checks_short_circuit_in_order() {
    let mut object = Object::new(0xE4);
    let mut candidate = Object::new(0x24);
    let mut zone_table = [0usize; 13];
    zone_table[12] = pair as *const () as usize;
    let mut lot_table = [0usize; 34];
    lot_table[6] = triple as *const () as usize;
    lot_table[33] = noargs as *const () as usize;
    let mut power_table = [0usize; 20];
    power_table[19] = pair as *const () as usize;
    let mut lv_table = [0usize; 7];
    lv_table[5] = pair as *const () as usize;
    lv_table[6] = pair as *const () as usize;
    let mut zone = [zone_table.as_ptr() as usize, 3];
    let mut lot = [lot_table.as_ptr() as usize, 1];
    let mut lots = [lot_table.as_ptr() as usize, lot.as_mut_ptr() as usize];
    let mut power = [power_table.as_ptr() as usize, 1];
    let mut lv = [lv_table.as_ptr() as usize, 7];
    // SAFETY: the buffers and all virtual slots above have the signatures used by the port.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        let c = Candidate::from_ptr(candidate.ptr());
        sim.set_zones(zone.as_mut_ptr().cast());
        sim.set_lots(lots.as_mut_ptr().cast());
        sim.set_power(power.as_mut_ptr().cast());
        sim.set_land_value(lv.as_mut_ptr().cast());
        c.set_zone(1);
        c.set_x(-7);
        c.set_z(11);
        c.set_land_value(7);
        for mode in 0..11 {
            zone[1] = 3;
            lots[1] = lot.as_mut_ptr() as usize;
            power[1] = 0xABCD_1201;
            lv[1] = 7;
            lot[1] = 1;
            sim.set_ignore_power(0);
            c.set_zone(1);
            c.set_min_land_value(0);
            c.set_max_land_value(0);
            let (expected_reason, expected_stages, expected_result) = match mode {
                0 => (99, 5, 1),
                1 => {
                    zone[1] = 0x80;
                    (0, 1, 0)
                }
                2 => {
                    lots[1] = 0;
                    (3, 2, u32::MAX)
                }
                3 => {
                    power[1] = 0xAABB_CC00;
                    (5, 3, 0xAABB_CC00)
                }
                4 => {
                    lv[1] = 0xAABB_CC08;
                    (2, 4, 0xAABB_CC00)
                }
                5 => {
                    c.set_zone(8);
                    zone[1] = 9;
                    lv[1] = 0;
                    (99, 5, 1)
                }
                6 => {
                    c.set_min_land_value(8);
                    c.set_max_land_value(12);
                    (1, 4, 0)
                }
                7 => {
                    c.set_min_land_value(1);
                    c.set_max_land_value(6);
                    (1, 4, 0)
                }
                8 => {
                    c.set_min_land_value(7);
                    c.set_max_land_value(7);
                    lv[1] = 0xABCD_0007;
                    (99, 5, 1)
                }
                9 => {
                    lot[1] = 0xAABB_CC02;
                    (4, 5, 0xAABB_CC00)
                }
                _ => {
                    sim.set_ignore_power(1);
                    power[1] = 0;
                    (99, 4, 1)
                }
            };
            let mut reason = 99;
            let result = fc4p_00695430(sim.as_ptr(), c.as_ptr(), &mut reason);
            let expected_result = if expected_result == u32::MAX {
                &mut reason as *mut u32 as u32 & !255
            } else {
                expected_result
            };
            assert_eq!((reason, result), (expected_reason, expected_result), "mode {mode}");
            let trace = calls();
            assert_eq!(trace.len(), expected_stages, "mode {mode}");
            assert_eq!(trace[0], (zone.as_ptr() as usize, vec![(-7i32) as u32, 11]));
            if trace.len() > 1 {
                assert_eq!(trace[1], (lots.as_ptr() as usize, vec![(-7i32) as u32, 11, 1]));
            }
            if trace.len() > 2 && mode != 10 {
                assert_eq!(trace[2].0, power.as_ptr() as usize);
            }
            if reason == 99 {
                assert_eq!(trace.last().unwrap().0, lot.as_ptr() as usize);
            }
        }
    }
}

#[test]
fn rectangle_truncates_clamps_and_handles_overflow() {
    let _fpu = GameFpu::new();
    let mut scale = 0.0625f32;
    let _global = fake_data(0x00B0_D1DC, (&mut scale as *mut f32).cast());
    let mut object = Object::new(0xE4);
    // SAFETY: all pointers refer to four-element arrays and a complete simulator buffer.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_cells_x(64);
        sim.set_cells_z(32);
        for (input, expected) in [
            ([-1.0, -33.0, 2000.0, 1024.0], [0, 0, 63, 31]),
            ([17.0, 31.0, -17.0, 15.0], [1, 1, -1, 0]),
            ([f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX], [0, 0, 0, 0]),
        ] {
            let mut output = [123; 4];
            fc4p_00695610(sim.as_ptr(), input.as_ptr(), output.as_mut_ptr());
            assert_eq!(output, expected);
        }
        sim.set_cells_x(0);
        sim.set_cells_z(0);
        let mut output = [0; 4];
        assert_eq!(fc4p_00695610(sim.as_ptr(), [0.0; 4].as_ptr(), output.as_mut_ptr()), -1);
        assert_eq!(output, [0, 0, -1, -1]);
    }
}

extern "thiscall" fn category(this: *mut u8, p: u32, w: u32) -> u32 {
    record(this, &[p, w]);
    0x3320
}
extern "thiscall" fn developer(this: *mut u8, kind: u32) -> u64 {
    record(this, &[kind]);
    0x1234_5678_ABCD_EF01
}

#[test]
fn developer_lookup_forwards_full_return() {
    let mut object = Object::new(0xE4);
    let mut table = [0usize; 60];
    table[30] = category as *const () as usize;
    table[32] = developer as *const () as usize;
    // SAFETY: the test table provides the two methods used by the export.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_vtable(table.as_ptr());
        assert_eq!(fc4p_006958b0(sim.as_ptr(), 3, 2), 0x1234_5678_ABCD_EF01);
        assert_eq!(
            calls(),
            vec![
                (sim.as_ptr() as usize, vec![3, 2]),
                (sim.as_ptr() as usize, vec![0x3320])
            ]
        );
    }
}

extern "thiscall" fn shutdown(this: *mut u8) {
    record(this, &[]);
}
extern "thiscall" fn delete(this: *mut u8, flags: u32) {
    record(this, &[flags]);
}

#[test]
fn release_signed_reference_boundaries() {
    let mut object = Object::new(0xE4);
    let mut table = [0usize; 60];
    table[4] = shutdown as *const () as usize;
    table[59] = delete as *const () as usize;
    // SAFETY: the object and lifetime slots have the original layout and signatures.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_vtable(table.as_ptr());
        for count in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            sim.set_references(count);
            let result = fc4p_006959b0(sim.as_ptr());
            if count > 1 {
                assert_eq!(result, (count - 1) as u32);
                assert_eq!(sim.references(), count - 1);
                assert!(calls().is_empty());
            } else {
                assert_eq!(result, 0);
                assert_eq!(sim.references(), count);
                assert_eq!(
                    calls(),
                    vec![(sim.as_ptr() as usize, vec![]), (sim.as_ptr() as usize, vec![1])]
                );
            }
        }
        // Also exercises the exported thiscall shape through the register-aware harness.
        sim.set_references(3);
        assert_eq!(
            call_thiscall_with_eax(fc4p_006959b0 as *const () as usize, sim.as_ptr(), &[], 0xAA55).0,
            2
        );
    }
}

#[test]
fn suitability_calls_the_building_helper_address() {
    extern "C" fn reject(building: i32, zone: i32) -> u32 {
        assert_eq!((building, zone), (1, -1));
        0xABCD_1200
    }
    let _helper = fake(0x0069_5390, reject as *const () as usize);
    let mut object = Object::new(0xE4);
    let mut candidate = Object::new(0x24);
    let mut table = [0usize; 13];
    table[12] = pair as *const () as usize;
    let mut zone = [table.as_ptr() as usize, 255];
    // SAFETY: only the zone method and helper fake are reached on this failure.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_zones(zone.as_mut_ptr().cast());
        let c = Candidate::from_ptr(candidate.ptr());
        c.set_zone(1);
        let mut reason = 9;
        assert_eq!(fc4p_00695430(sim.as_ptr(), c.as_ptr(), &mut reason), 0xABCD_1200);
        assert_eq!(reason, 0);
        assert_eq!(calls().len(), 1);
    }
}
