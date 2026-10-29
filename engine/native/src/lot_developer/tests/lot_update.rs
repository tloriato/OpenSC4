// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::daily::DailyState;
use crate::lot_developer::lot_update::{fc4p_006cd3a0, CommuteChecks};

slot!(location, x, z);
slot!(road_refresh);
slot!(building);
slot!(designate);
slot!(road_access);
slot!(zone);
slot!(building_type, include_pending);
slot!(profile, kind);
slot!(coffee_enabled);
slot!(state);
slot!(state_change, state, wealth, reason);
slot!(condition);
slot!(condition_change, condition);
slot!(days);
slot!(unit);
slot!(members);
slot!(designate_update, output);
slot!(population_update, unit);
slot!(population_reset);
slot!(population_add, lot);
slot!(livable);
slot!(desirability, lot, x, z);
slot!(power, lot, x, z);
slot!(water, lot, x, z);
slot!(travel, lot, profile, x, z);
slot!(next_condition, lot, x, z);
slot!(coffee, lot);
slot!(rotate, lot);
slot!(join, lot);

#[unsafe(naked)]
unsafe extern "C" fn suitable() {
    core::arch::naked_asm!("push eax", "call {}", "ret", sym suitable_body);
}
unsafe extern "fastcall" fn suitable_body(lot: *mut u8, x: u32, z: u32) -> u64 {
    run(lot, "suitable", &[x, z])
}
unsafe extern "thiscall" fn temporary_profile(this: *mut u8) -> u64 {
    run(core::ptr::null_mut(), "temporary_profile", &[this as u32])
}
unsafe extern "thiscall" fn destroy_profile(this: *mut u8) -> u64 {
    run(core::ptr::null_mut(), "destroy_profile", &[this as u32])
}
unsafe extern "thiscall" fn temporary_travel(this: *mut u8, lot: u32, profile: u32, x: u32, z: u32) -> u64 {
    run(this, "temporary_travel", &[lot, profile, x, z])
}

struct World {
    developer: Mock,
    lot: Mock,
    service: Mock,
    profile: Object,
    _fakes: Vec<crate::testing::Fake>,
}
impl World {
    fn new() -> Self {
        let mut world = Self {
            developer: mock!(0x88 => designate_update, 0x8C => population_update),
            lot: mock!(0x54 => location, 0x78 => road_refresh, 0xC0 => building, 0xEC => designate,
                0x70 => road_access, 0x7C => zone, 0xB8 => building_type, 0x84 => state,
                0x88 => state_change, 0x8C => condition, 0x90 => condition_change, 0xA0 => days,
                0x64 => unit, 0xE4 => population_reset, 0xE8 => population_add, 0x94 => livable),
            service: mock!(0x9C => profile, 0xE8 => coffee_enabled),
            profile: Object::new(64),
            _fakes: vec![
                fake(0x006C_47C0, desirability as *const () as usize),
                fake(0x006C_ABB0, power as *const () as usize),
                fake(0x006C_AE30, water as *const () as usize),
                fake(0x006C_AFC0, travel as *const () as usize),
                fake(0x006C_4630, next_condition as *const () as usize),
                fake(0x006C_78D0, coffee as *const () as usize),
                fake(0x006C_4910, rotate as *const () as usize),
                fake(0x006C_B260, join as *const () as usize),
                fake(0x006C_3B10, suitable as *const () as usize),
                fake(0x0045_91E0, temporary_profile as *const () as usize),
                fake(0x0045_9220, destroy_profile as *const () as usize),
            ],
        };
        unsafe { LotDeveloper::from_ptr(world.developer.ptr()).set_building_development(world.service.ptr()) };
        world
    }
    fn start(&mut self, kind: u32, purpose: u32) {
        expect(
            self.lot.ptr(),
            "location",
            vec![out(&[(-4i32) as u32]), out(&[9])],
            RETURN,
        );
        expect(self.lot.ptr(), "building_type", vec![w(0)], u64::from(kind));
        if kind != 0 {
            unsafe { self.profile.ptr().cast::<u32>().write_unaligned(purpose) };
            expect(self.service.ptr(), "profile", vec![w(kind)], self.profile.ptr() as u64);
        }
    }
    fn no_unit(&mut self, live: bool, road: bool) {
        expect(self.lot.ptr(), "unit", vec![], 0);
        expect(self.lot.ptr(), "unit", vec![], 0);
        expect(self.lot.ptr(), "population_reset", vec![], RETURN);
        expect(self.lot.ptr(), "population_add", vec![p(self.lot.ptr())], RETURN);
        expect(
            self.lot.ptr(),
            "livable",
            vec![],
            if live { RETURN | 1 } else { RETURN & !255 },
        );
        if live {
            expect(
                self.lot.ptr(),
                "road_access",
                vec![],
                if road { RETURN | 1 } else { RETURN & !255 },
            );
            if !road {
                expect(self.developer.ptr(), "join", vec![p(self.lot.ptr())], RETURN);
            }
        }
    }
    fn invoke(&mut self) -> [u32; 3] {
        let mut words = [0u32, 0, self.lot.ptr() as u32];
        unsafe {
            core::arch::asm!(
                "push [edi + 8]", "call {function}", "mov [edi], eax", "mov [edi + 4], edx",
                "mov eax, [esp - 4]", "mov [edi + 8], eax",
                function = sym fc4p_006cd3a0, in("edi") words.as_mut_ptr(), in("ecx") self.developer.ptr(), clobber_abi("C"),
            );
        }
        words
    }
}

#[test]
fn grown_lots_run_every_check_and_the_last_failure_sets_the_abandonment_reason() {
    let mut world = World::new();
    for mask in 0u32..16 {
        for previous in [3u32, 4] {
            world.start(0x3000, 2);
            expect(world.lot.ptr(), "state", vec![], u64::from(previous));
            expect(
                world.developer.ptr(),
                "desirability",
                vec![p(world.lot.ptr()), w((-4i32) as u32), w(9)],
                RETURN & !255 | u64::from(mask & 1 == 0),
            );
            expect(
                world.developer.ptr(),
                "power",
                vec![p(world.lot.ptr()), w((-4i32) as u32), w(9)],
                if mask & 2 == 0 { 2 } else { 0 },
            );
            expect(
                world.developer.ptr(),
                "water",
                vec![p(world.lot.ptr()), w((-4i32) as u32), w(9)],
                RETURN & !255 | u64::from(mask & 4 == 0),
            );
            expect(
                world.developer.ptr(),
                "travel",
                vec![p(world.lot.ptr()), p(world.profile.ptr()), w((-4i32) as u32), w(9)],
                RETURN & !255 | u64::from(mask & 8 == 0),
            );
            let changed = mask != 0 && previous != 4;
            if changed {
                let reason = 32 - mask.leading_zeros();
                expect(world.lot.ptr(), "state_change", vec![w(4), w(0), w(reason)], RETURN);
            }
            expect(
                world.developer.ptr(),
                "next_condition",
                vec![p(world.lot.ptr()), w((-4i32) as u32), w(9)],
                5,
            );
            expect(world.lot.ptr(), "condition", vec![], 1);
            expect(world.lot.ptr(), "condition_change", vec![w(5)], RETURN);
            expect(world.service.ptr(), "coffee_enabled", vec![], RETURN | 1);
            expect(world.developer.ptr(), "coffee", vec![p(world.lot.ptr())], RETURN);
            world.no_unit(true, false);
            let actual = world.invoke();
            assert_eq!(
                actual,
                [RETURN as u32 & !255 | u32::from(changed), (RETURN >> 32) as u32, 9]
            );
            finished();
        }
    }
}

#[test]
fn civic_power_conditions_cover_the_full_switch_and_test_travel_only_when_powered() {
    let mut world = World::new();
    for old in [0u32, 1, 2, 3, 4, 5, 6, 7, u32::MAX] {
        for powered in [false, true] {
            world.start(0x1234, 0);
            expect(
                world.developer.ptr(),
                "power",
                vec![p(world.lot.ptr()), w((-4i32) as u32), w(9)],
                if powered { 2 } else { 1 },
            );
            expect(world.lot.ptr(), "condition", vec![], u64::from(old));
            if old == 0 && !powered {
                expect(world.lot.ptr(), "condition_change", vec![w(1)], RETURN);
            } else if (old == 1 && powered) || matches!(old, 5 | 6) {
                expect(world.lot.ptr(), "condition_change", vec![w(0)], RETURN);
            }
            if powered {
                expect(
                    world.developer.ptr(),
                    "travel",
                    vec![p(world.lot.ptr()), p(world.profile.ptr()), w((-4i32) as u32), w(9)],
                    RETURN,
                );
            }
            world.no_unit(true, true);
            let actual = world.invoke();
            assert_eq!(actual, [RETURN as u32 & !255, (RETURN >> 32) as u32, 9]);
            finished();
        }
    }
}

#[test]
fn empty_lots_gate_commute_by_age_zone_and_pending_latch_then_test_zot_suitability() {
    let mut world = World::new();
    let _travel = fake(0x006C_AFC0, temporary_travel as *const () as usize);
    for allowed in [0u8, 0x80] {
        unsafe { DailyState::from_ptr(world.developer.ptr()).set_can_check_commute(allowed) };
        for age in [0u32, 30, 31, u32::MAX] {
            for zone in [0u32, 1, 3, 4, 6, 7, u32::MAX] {
                for pending in [0u8, 0x80] {
                    unsafe { CommuteChecks::from_ptr(world.developer.ptr()).set_enabled(pending) };
                    world.start(0, 0);
                    if allowed != 0 {
                        expect(world.lot.ptr(), "days", vec![], u64::from(age));
                        if age > 30 {
                            expect(world.lot.ptr(), "zone", vec![], u64::from(zone));
                            if (1..=6).contains(&zone) && pending != 0 {
                                expect(core::ptr::null_mut(), "temporary_profile", vec![out(&[0; 16])], RETURN);
                                expect(
                                    world.developer.ptr(),
                                    "temporary_travel",
                                    vec![
                                        p(world.lot.ptr()),
                                        Arg::In(vec![if zone <= 3 { 1 } else { 3 }]),
                                        w((-4i32) as u32),
                                        w(9),
                                    ],
                                    RETURN,
                                );
                                expect(
                                    core::ptr::null_mut(),
                                    "destroy_profile",
                                    vec![Arg::In(vec![if zone <= 3 { 1 } else { 3 }])],
                                    RETURN,
                                );
                            }
                        }
                        let suitable = zone & 1 != 0;
                        expect(
                            world.lot.ptr(),
                            "suitable",
                            vec![w((-4i32) as u32), w(9)],
                            RETURN & !255 | u64::from(suitable),
                        );
                        if suitable {
                            expect(
                                world.developer.ptr(),
                                "power",
                                vec![p(world.lot.ptr()), w((-4i32) as u32), w(9)],
                                RETURN,
                            );
                        }
                    }
                    world.no_unit(false, false);
                    let actual = world.invoke();
                    assert_eq!(actual, [RETURN as u32 & !255, (RETURN >> 32) as u32, 9]);
                    finished();
                }
            }
        }
    }
}

#[test]
fn unit_designation_can_replace_the_unit_before_the_population_pass() {
    let mut world = World::new();
    let mut first = mock!(0x14 => members);
    let mut replacement = mock!(0x14 => members);
    for initial_first in [false, true] {
        for dirty in [0u8, 0x80] {
            for still_member in [false, true] {
                unsafe { DailyState::from_ptr(world.developer.ptr()).set_units_changed(dirty) };
                let mut node = [0u32, 0, if initial_first { world.lot.ptr() as u32 } else { 0x1234 }];
                let mut head = [node.as_mut_ptr() as u32];
                let mut list = [head.as_mut_ptr() as u32];
                let mut new_node = [0u32, 0, if still_member { world.lot.ptr() as u32 } else { 0x5678 }];
                let mut new_head = [new_node.as_mut_ptr() as u32];
                let mut new_list = [new_head.as_mut_ptr() as u32];
                world.start(0, 0);
                expect(world.lot.ptr(), "unit", vec![], first.ptr() as u64);
                expect(first.ptr(), "members", vec![], list.as_mut_ptr() as u64);
                if initial_first && dirty != 0 {
                    expect(
                        world.developer.ptr(),
                        "designate_update",
                        vec![out(&[replacement.ptr() as u32])],
                        RETURN,
                    );
                }
                expect(world.lot.ptr(), "unit", vec![], replacement.ptr() as u64);
                expect(replacement.ptr(), "members", vec![], new_list.as_mut_ptr() as u64);
                if still_member {
                    expect(
                        world.developer.ptr(),
                        "population_update",
                        vec![p(replacement.ptr())],
                        RETURN,
                    );
                }
                let actual = world.invoke();
                assert_eq!(actual[0] as u8, 0);
                assert_eq!(actual[2], 9);
                if still_member {
                    assert_eq!(&actual[..2], &[RETURN as u32 & !255, (RETURN >> 32) as u32]);
                }
                finished();
            }
        }
    }
}

#[test]
fn network_refresh_short_circuits_each_rotation_gate_and_checks_zone_signed() {
    let mut world = World::new();
    let mut pending = 0u8;
    let _pending = fake_data(0x00B4_9AE9, &mut pending);
    for stop in 0..8 {
        unsafe { DailyState::from_ptr(world.developer.ptr()).set_roads_changed(if stop == 0 { 0 } else { 0x80 }) };
        pending = if stop == 1 { 0 } else { 0x80 };
        assert_eq!(pending, if stop == 1 { 0 } else { 0x80 });
        expect(
            world.lot.ptr(),
            "location",
            vec![out(&[(-4i32) as u32]), out(&[9])],
            RETURN,
        );
        if stop != 0 {
            expect(world.lot.ptr(), "road_refresh", vec![], RETURN);
            if stop != 1 {
                expect(world.lot.ptr(), "building", vec![], if stop == 2 { 0 } else { 0x1234 });
                if stop != 2 {
                    expect(world.lot.ptr(), "designate", vec![], if stop == 3 { 0x5678 } else { 0 });
                    if stop != 3 {
                        expect(
                            world.lot.ptr(),
                            "road_access",
                            vec![],
                            if stop == 4 { 0x101 } else { 0x100 },
                        );
                        if stop != 4 {
                            expect(
                                world.lot.ptr(),
                                "zone",
                                vec![],
                                match stop {
                                    5 => u64::from(u32::MAX),
                                    6 => 10,
                                    _ => 9,
                                },
                            );
                            if stop == 7 {
                                expect(world.developer.ptr(), "rotate", vec![p(world.lot.ptr())], RETURN);
                            }
                        }
                    }
                }
            }
        }
        expect(world.lot.ptr(), "building_type", vec![w(0)], 0);
        world.no_unit(false, false);
        assert_eq!(world.invoke(), [RETURN as u32 & !255, (RETURN >> 32) as u32, 9]);
        finished();
    }
}
