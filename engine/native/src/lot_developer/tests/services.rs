// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::services::*;

slot!(create_property, id, value, flags);
slot!(lot_unit);
slot!(needs_power);
slot!(has_power);
slot!(power_cheat);
slot!(water_cheat);
slot!(is_powered, x, z);
slot!(is_watered, x, z);
slot!(water_developer, purpose, wealth);
slot!(growth_stage);
slot!(water_cap);
slot!(road_access);
slot!(job_access);
slot!(organic_road, purpose);
slot!(trip_grid);
slot!(trip_length, x, z);
slot!(travel_jobs, map);
slot!(unit_occupant);
slot!(bounds, min, max);
slot!(set_power_word, value);

unsafe extern "thiscall" fn set_has_power(this: *mut u8, value: u32) -> u64 {
    // The three high bytes are deliberately retained from the caller's scratch word.
    run(this, "set_has_power", &[value & 255])
}

unsafe extern "thiscall" fn add_zot_call(this: *mut u8, lot: u32, kind: u32, zot: u32) -> u64 {
    run(this, "add_zot_call", &[lot, kind, zot])
}
unsafe extern "thiscall" fn remove_zot_call(this: *mut u8, lot: u32, zot: u32) -> u64 {
    run(this, "remove_zot_call", &[lot, zot])
}
unsafe extern "thiscall" fn remove_prop_call(this: *mut u8, lot: u32, zot: u32) -> u64 {
    run(this, "remove_prop_call", &[lot, zot])
}

float_slot!(cell_x_float, cell_x_body, 0);
float_slot!(cell_z_float, cell_z_body, 0);
float_slot!(jobs_float, jobs_body, 1, wealth);

struct World {
    _objects: Vec<Mock>,
    dev: *mut u8,
    lot: *mut u8,
    holder: *mut u8,
    property: *mut u8,
    variant: *mut u8,
    zot_property: *mut u8,
    zot_variant: *mut u8,
    building_dev: *mut u8,
    water: *mut u8,
    power: *mut u8,
    travel: *mut u8,
    grid: *mut u8,
    unit: *mut u8,
    occupant: *mut u8,
    config: *mut u8,
    growth: *mut u8,
    city: *mut u8,
    words: Box<[u32; 2]>,
    profile: Box<[u32; 2]>,
    _fakes: Vec<crate::testing::Fake>,
}

impl World {
    fn new() -> Self {
        let mut objects = vec![
            mock!(),
            mock!(0x0C => get_holder, 0xC0 => lot_unit, 0x104 => needs_power, 0x38 => bounding_rect,
                0x54 => get_location, 0x108 => has_power, 0x10C => set_has_power, 0x8C => get_type,
                0x90 => set_condition, 0x84 => habitation_state, 0xB8 => get_building_type,
                0x24 => get_config, 0x70 => road_access, 0x6C => job_access, 0x74 => organic_road,
                0x100 => travel_jobs, 0xFC => jobs_float),
            mock!(0x24 => get_property, 0x34 => create_property),
            mock!(0x14 => get_variant),
            mock!(0x124 => get_data, 0x68 => set_uint),
            mock!(0x14 => get_variant),
            mock!(0x124 => get_data, 0x68 => set_uint),
            mock!(0xDC => power_cheat, 0xD8 => water_cheat, 0x9C => get_profile, 0x84 => water_developer),
            mock!(0x2C => is_watered),
            mock!(0x94 => is_powered),
            mock!(0x34 => trip_grid),
            mock!(0x1C => trip_length),
            mock!(0x0C => unit_occupant),
            mock!(0x28 => bounds),
            mock!(0x2C => growth_stage),
            mock!(0x50 => water_cap),
            mock!(0x16C => cell_x_float, 0x170 => cell_z_float),
        ];
        let ptrs: Vec<_> = objects.iter_mut().map(Mock::ptr).collect();
        let dev = ptrs[0];
        unsafe {
            LotDeveloper::from_ptr(dev).set_building_development(ptrs[7]);
            LotDeveloper::from_ptr(dev).set_city(ptrs[16]);
            ServiceDeveloper::from_ptr(dev).set_water(ptrs[8]);
            ServiceDeveloper::from_ptr(dev).set_power(ptrs[9]);
            ServiceDeveloper::from_ptr(dev).set_travel(ptrs[10]);
        }
        Self {
            _objects: objects,
            dev,
            lot: ptrs[1],
            holder: ptrs[2],
            property: ptrs[3],
            variant: ptrs[4],
            zot_property: ptrs[5],
            zot_variant: ptrs[6],
            building_dev: ptrs[7],
            water: ptrs[8],
            power: ptrs[9],
            travel: ptrs[10],
            grid: ptrs[11],
            unit: ptrs[12],
            occupant: ptrs[13],
            config: ptrs[14],
            growth: ptrs[15],
            city: ptrs[16],
            words: Box::new([0; 2]),
            profile: Box::new([1, 2]),
            _fakes: vec![
                fake(0x006C_98C0, add_zot_call as *const () as usize),
                fake(0x006C_9A40, remove_zot_call as *const () as usize),
                fake(0x006C_9780, remove_prop_call as *const () as usize),
            ],
        }
    }

    fn counter(&mut self, id: u32, value: u32, exists: bool) {
        self.words[0] = value;
        expect(self.lot, "get_holder", vec![], self.holder as u64);
        expect(
            self.holder,
            "get_property",
            vec![w(id)],
            if exists { self.property as u64 } else { 0 },
        );
        if !exists {
            expect(self.holder, "create_property", vec![w(id), w(0), w(0)], RETURN);
            expect(self.holder, "get_property", vec![w(id)], self.property as u64);
        }
        expect(self.property, "get_variant", vec![], self.variant as u64);
        expect(self.variant, "get_data", vec![], self.words.as_mut_ptr() as u64);
    }

    fn zot_cleanup(&mut self, current: Option<u32>, wanted: u32) -> u64 {
        expect(self.lot, "get_holder", vec![], self.holder as u64);
        expect(
            self.holder,
            "get_property",
            vec![w(0xC999_C45E)],
            current.map_or(RETURN & 0xFFFF_FFFF_0000_0000, |_| self.zot_property as u64),
        );
        if let Some(value) = current {
            self.words[1] = value;
            expect(self.zot_property, "get_variant", vec![], self.zot_variant as u64);
            expect(
                self.zot_variant,
                "get_data",
                vec![],
                self.words.as_mut_ptr().wrapping_add(1) as u64,
            );
            if value == wanted {
                expect(self.dev, "remove_prop_call", vec![p(self.lot), w(wanted)], RETURN);
                expect(self.zot_variant, "set_uint", vec![w(0)], RETURN);
                RETURN
            } else {
                u64::from(value)
            }
        } else {
            RETURN & 0xFFFF_FFFF_0000_0000
        }
    }

    fn invoke(&mut self, entry: usize, args: &[u32]) -> u64 {
        unsafe {
            let mut call = RawCall::new(Conv::Thiscall, entry, self.dev as u32, args);
            call.eax = 0x1122_3344;
            call.edx = 0x5566_7788;
            call_raw(&mut call);
            finished();
            result(call)
        }
    }
}

#[test]
fn power_grace_wraps_and_visits_rectangle_before_cheat_and_location() {
    for old in [0, 1, 2, 3, u32::MAX - 1, u32::MAX] {
        let mut world = World::new();
        world.counter(0xE999_B11F, old, old != 0);
        expect(world.lot, "lot_unit", vec![], 0);
        expect(world.lot, "bounding_rect", vec![out(&[10, 20, 11, 21])], RETURN);
        expect(world.building_dev, "power_cheat", vec![], 0x100);
        expect(
            world.lot,
            "get_location",
            vec![out(&[u32::MAX]), out(&[0x8000_0000])],
            RETURN,
        );
        expect(world.power, "is_powered", vec![w(u32::MAX), w(0x8000_0000)], 0x100);
        expect(world.lot, "has_power", vec![], 0x100);
        let next = old.wrapping_add(1);
        expect(world.variant, "set_uint", vec![w(next)], RETURN);
        expect(
            world.dev,
            "add_zot_call",
            vec![p(world.lot), w(0x0FD1_0000), w(4)],
            RETURN,
        );
        let actual = world.invoke(fc4p_006cabb0 as *const () as usize, &[world.lot as u32, 123, 456]);
        assert_eq!(actual, RETURN & 0xFFFF_FFFF_0000_0000 | u64::from(next <= 2));
    }
}

#[test]
fn power_changes_condition_only_on_a_power_transition() {
    for powered in [false, true] {
        for prior in [false, true] {
            for condition in [0u32, 1, 2, u32::MAX] {
                let mut world = World::new();
                world.counter(0xE999_B11F, 20, true);
                expect(world.lot, "lot_unit", vec![], 0);
                expect(world.lot, "bounding_rect", vec![out(&[0; 4])], RETURN);
                expect(world.building_dev, "power_cheat", vec![], u64::from(powered));
                if !powered {
                    expect(world.lot, "get_location", vec![out(&[3]), out(&[4])], RETURN);
                    expect(world.power, "is_powered", vec![w(3), w(4)], 0);
                }
                expect(world.lot, "has_power", vec![], u64::from(prior));
                if powered != prior {
                    expect(world.lot, "set_has_power", vec![w(u32::from(powered))], RETURN);
                    expect(world.lot, "get_type", vec![], u64::from(condition));
                    if (!powered && condition == 0) || (powered && condition == 1) {
                        expect(world.lot, "set_condition", vec![w(u32::from(!powered))], RETURN);
                    }
                }
                expect(world.variant, "set_uint", vec![w(if powered { 0 } else { 21 })], RETURN);
                if !powered {
                    expect(
                        world.dev,
                        "add_zot_call",
                        vec![p(world.lot), w(0x0FD1_0000), w(4)],
                        RETURN,
                    );
                }
                let actual = world.invoke(fc4p_006cabb0 as *const () as usize, &[world.lot as u32, 0, 0]);
                assert_eq!(actual as u32, if powered { 2 } else { 0 });
            }
        }
    }
}

#[test]
fn power_unit_bounds_use_x87_centers_and_only_remove_the_power_zot() {
    let _fpu = GameFpu::new();
    for current in [None, Some(4), Some(5)] {
        let mut world = World::new();
        world.counter(0xE999_B11F, 2, true);
        expect(world.lot, "lot_unit", vec![], world.unit as u64);
        expect(world.lot, "needs_power", vec![], 0xAABB_0001);
        expect(world.lot, "bounding_rect", vec![out(&[0; 4])], RETURN);
        expect(world.building_dev, "power_cheat", vec![], 0);
        expect(world.unit, "unit_occupant", vec![], world.occupant as u64);
        expect(
            world.occupant,
            "bounds",
            vec![
                out(&[(-65.0f32).to_bits(), 2.0f32.to_bits(), 160.0f32.to_bits()]),
                out(&[(-32.0f32).to_bits(), 6.0f32.to_bits(), 192.0f32.to_bits()]),
            ],
            RETURN,
        );
        expect_float(world.city, "cell_x_body", vec![], RETURN, F80::from_f32(16.0));
        expect_float(world.city, "cell_z_body", vec![], RETURN, F80::from_f32(16.0));
        expect(world.power, "is_powered", vec![w((-3i32) as u32), w(11)], 0xAABB_0001);
        expect(world.lot, "has_power", vec![], 1);
        expect(world.variant, "set_uint", vec![w(0)], RETURN);
        world.zot_cleanup(current, 4);
        let actual = world.invoke(fc4p_006cabb0 as *const () as usize, &[world.lot as u32, 0, 0]);
        assert_eq!(actual as u32, 2);
    }
}

#[test]
fn power_not_needed_short_circuits_service_queries_but_cleans_existing_zot() {
    let mut world = World::new();
    world.counter(0xE999_B11F, 2, true);
    expect(world.lot, "lot_unit", vec![], world.unit as u64);
    expect(world.lot, "needs_power", vec![], 0x100);
    expect(world.lot, "bounding_rect", vec![out(&[0; 4])], RETURN);
    expect(world.building_dev, "power_cheat", vec![], 0);
    expect(world.lot, "has_power", vec![], 1);
    expect(world.variant, "set_uint", vec![w(0)], RETURN);
    world.zot_cleanup(Some(4), 4);
    assert_eq!(
        world.invoke(fc4p_006cabb0 as *const () as usize, &[world.lot as u32, 0, 0]) as u32,
        2
    );
}

#[test]
fn power_retains_scratch_high_bytes_and_overwrites_only_the_argument_low_byte() {
    let mut world = World::new();
    world._objects[1]._vtable[0x10C / 4] = set_power_word as *const () as usize;
    world.counter(0xE999_B11F, 0, true);
    expect(world.lot, "lot_unit", vec![], 0);
    expect(world.lot, "bounding_rect", vec![out(&[0; 4])], RETURN);
    expect(world.building_dev, "power_cheat", vec![], 1);
    expect(world.lot, "has_power", vec![], 0);
    expect(world.lot, "set_power_word", vec![w(0xA1B2_C301)], RETURN);
    expect(world.lot, "get_type", vec![], 1);
    expect(world.lot, "set_condition", vec![w(0)], RETURN);
    expect(world.variant, "set_uint", vec![w(0)], RETURN);
    let mut record = [
        fc4p_006cabb0 as *const () as u32,
        world.dev as u32,
        world.lot as u32,
        0x1234_5678,
        0x8765_4321,
        0,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "sub esp, 256", "mov dword ptr [esp + 52], 0xA1B2C3D4", "add esp, 128",
            "push dword ptr [edi + 16]", "push dword ptr [edi + 12]", "push dword ptr [edi + 8]",
            "mov ecx, dword ptr [edi + 4]", "call dword ptr [edi]",
            "mov dword ptr [edi + 20], eax", "mov dword ptr [edi + 24], edx",
            "mov eax, dword ptr [esp - 12]", "mov dword ptr [edi + 28], eax",
            "mov eax, dword ptr [esp - 76]", "mov dword ptr [edi + 32], eax", "add esp, 128",
            in("edi") record.as_mut_ptr(), clobber_abi("C"),
        );
    }
    assert_eq!(record[5], 2);
    assert_eq!(record[6], (RETURN >> 32) as u32);
    assert_eq!(record[7], world.lot as u32 & !255 | 1);
    assert_eq!(record[8], 0xA1B2_C301);
    finished();
}

#[test]
fn water_grace_and_counter_wrap_for_civic_buildings() {
    for old in [0u32, 1, 2, 3, u32::MAX] {
        let mut world = World::new();
        world.profile[0] = 0;
        world.counter(0x8999_B20B, old, old != 0);
        expect(world.lot, "habitation_state", vec![], 3);
        expect(world.lot, "get_building_type", vec![w(0)], 55);
        expect(
            world.building_dev,
            "get_profile",
            vec![w(55)],
            world.profile.as_mut_ptr() as u64,
        );
        expect(world.building_dev, "water_cheat", vec![], 0);
        expect(world.water, "is_watered", vec![w(0xFFFF_FFFE), w(30)], 0x100);
        let next = old.wrapping_add(1);
        expect(world.variant, "set_uint", vec![w(next)], RETURN);
        expect(
            world.dev,
            "add_zot_call",
            vec![p(world.lot), w(0x1C43_0000), w(5)],
            RETURN,
        );
        assert_eq!(
            world.invoke(
                fc4p_006cae30 as *const () as usize,
                &[world.lot as u32, 0xFFFF_FFFE, 30]
            ),
            RETURN & !255 | u64::from(next <= 2)
        );
    }
}

#[test]
fn water_stage_and_supply_cap_compare_unsigned_low_bytes_and_always_query_configuration() {
    for stage in [0u32, 2, 3, 255, 0x100] {
        for cap in [0u32, 2, 255, 0x100] {
            for missing in [0u32, 1, 2] {
                let mut world = World::new();
                world.counter(0x8999_B20B, 2, true);
                expect(world.lot, "habitation_state", vec![], 2);
                expect(world.lot, "get_building_type", vec![w(0)], 55);
                expect(
                    world.building_dev,
                    "get_profile",
                    vec![w(55)],
                    world.profile.as_mut_ptr() as u64,
                );
                expect(
                    world.building_dev,
                    "water_developer",
                    vec![w(1), w(2)],
                    if missing == 1 { 0 } else { world.growth as u64 },
                );
                expect(
                    world.lot,
                    "get_config",
                    vec![],
                    if missing == 2 { 0 } else { world.config as u64 },
                );
                if missing == 0 {
                    expect(world.config, "growth_stage", vec![], u64::from(stage));
                    expect(world.growth, "water_cap", vec![], u64::from(cap));
                }
                expect(world.building_dev, "water_cheat", vec![], 0);
                expect(world.water, "is_watered", vec![w(9), w(10)], 0);
                let fail = missing == 0 && stage as u8 > cap as u8;
                expect(world.variant, "set_uint", vec![w(if fail { 3 } else { 0 })], RETURN);
                let returned = if fail {
                    expect(
                        world.dev,
                        "add_zot_call",
                        vec![p(world.lot), w(0x1C43_0000), w(5)],
                        RETURN,
                    );
                    RETURN & !255
                } else {
                    world.zot_cleanup(Some(5), 5) & !255 | 1
                };
                assert_eq!(
                    world.invoke(fc4p_006cae30 as *const () as usize, &[world.lot as u32, 9, 10]),
                    returned
                );
            }
        }
    }
}

#[test]
fn water_ineligible_states_still_query_water_and_cheat_skips_that_query() {
    for state in [i32::MIN, -1, 0, 1, 4, 5, i32::MAX] {
        for cheat in [false, true] {
            let mut world = World::new();
            world.counter(0x8999_B20B, 99, true);
            expect(world.lot, "habitation_state", vec![], state as u32 as u64);
            if state == 4 {
                world.profile[0] = 0;
                expect(world.lot, "get_building_type", vec![w(0)], 0);
                expect(
                    world.building_dev,
                    "get_profile",
                    vec![w(0)],
                    world.profile.as_mut_ptr() as u64,
                );
            }
            expect(world.building_dev, "water_cheat", vec![], u64::from(cheat));
            if !cheat {
                expect(world.water, "is_watered", vec![w(1), w(2)], 1);
            }
            expect(world.variant, "set_uint", vec![w(0)], RETURN);
            let returned = world.zot_cleanup(Some(4), 5);
            assert_eq!(
                world.invoke(fc4p_006cae30 as *const () as usize, &[world.lot as u32, 1, 2]),
                returned & !255 | 1
            );
        }
    }
}

fn travel_pass(world: &mut World, reset: bool, property_exists: bool, zot: Option<u32>) -> u64 {
    if reset {
        expect(world.lot, "get_holder", vec![], world.holder as u64);
        expect(
            world.holder,
            "get_property",
            vec![w(0x8999_B10B)],
            if property_exists { world.property as u64 } else { 0 },
        );
        if property_exists {
            expect(world.property, "get_variant", vec![], world.variant as u64);
            expect(world.variant, "set_uint", vec![w(0)], RETURN);
        }
    }
    expect(world.dev, "remove_zot_call", vec![p(world.lot), w(6)], RETURN);
    world.zot_cleanup(zot, 7) & !255 | 1
}

#[test]
fn travel_commute_grace_matches_the_seven_visits_and_road_grace_matches_three() {
    for road in [false, true] {
        for old in 0..7u32 {
            let mut world = World::new();
            expect(world.lot, "habitation_state", vec![], 3);
            expect(world.lot, "road_access", vec![], u64::from(road));
            if road {
                expect(world.lot, "job_access", vec![], 1);
            }
            expect(world.travel, "trip_grid", vec![], world.grid as u64);
            expect(world.grid, "trip_length", vec![w(8), w(9)], 0);
            world.counter(0x8999_B10B, old, old != 0);
            let next = old + 1;
            expect(world.variant, "set_uint", vec![w(next)], RETURN);
            let returned = if !road || next > 2 {
                let (kind, zot) = if road { (0x1C44_0000, 7) } else { (0x107A_0000, 6) };
                expect(world.dev, "add_zot_call", vec![p(world.lot), w(kind), w(zot)], RETURN);
                if road {
                    RETURN & !255 | u64::from(next <= 6)
                } else {
                    RETURN & 0xFFFF_FFFF_0000_0000 | u64::from(next <= 2)
                }
            } else {
                travel_pass(&mut world, false, true, Some(7))
            };
            assert_eq!(
                world.invoke(
                    fc4p_006cafc0 as *const () as usize,
                    &[world.lot as u32, world.profile.as_ptr() as u32, 8, 9]
                ),
                returned
            );
        }
    }
}

#[test]
fn travel_residential_road_and_job_flags_use_low_bytes_and_trip_grid_runs_even_without_road() {
    for road in [0u32, 0x100, 1, 255] {
        for jobs in [0u32, 0x100, 1, 255] {
            let mut world = World::new();
            expect(world.lot, "habitation_state", vec![], 1);
            expect(world.lot, "road_access", vec![], u64::from(road));
            if road as u8 != 0 {
                expect(world.lot, "job_access", vec![], u64::from(jobs));
            }
            expect(world.travel, "trip_grid", vec![], world.grid as u64);
            expect(world.grid, "trip_length", vec![w(1), w(2)], 0xFF);
            let pass = road as u8 != 0 && jobs as u8 != 0;
            let returned = if pass {
                travel_pass(&mut world, true, false, None)
            } else {
                world.counter(0x8999_B10B, u32::MAX, true);
                expect(world.variant, "set_uint", vec![w(0)], RETURN);
                expect(
                    world.dev,
                    "add_zot_call",
                    vec![p(world.lot), w(0x107A_0000), w(6)],
                    RETURN,
                );
                RETURN & 0xFFFF_FFFF_0000_0000 | 1
            };
            assert_eq!(
                world.invoke(
                    fc4p_006cafc0 as *const () as usize,
                    &[world.lot as u32, world.profile.as_ptr() as u32, 1, 2]
                ),
                returned
            );
        }
    }
}

#[test]
fn travel_organic_purposes_and_ineligible_states_skip_trip_grid() {
    for state in [0u32, 1, 2, 3, 4, 5, u32::MAX] {
        for purpose in 5..=8 {
            let mut world = World::new();
            world.profile[0] = purpose;
            expect(world.lot, "habitation_state", vec![], u64::from(state));
            if matches!(state, 1 | 3 | 4) {
                expect(world.lot, "organic_road", vec![w(purpose)], 0x10001);
            }
            let returned = travel_pass(&mut world, true, true, Some(6));
            assert_eq!(
                world.invoke(
                    fc4p_006cafc0 as *const () as usize,
                    &[world.lot as u32, world.profile.as_ptr() as u32, 1, 2]
                ),
                returned
            );
        }
    }
}

#[test]
fn travel_job_scan_stops_at_first_positive_and_treats_nan_as_no_jobs() {
    let _fpu = GameFpu::new();
    let mut zero = 0.0f32;
    let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
    for purpose in [i32::MIN, -1, 0, 2, 3, 4, 9, i32::MAX] {
        for positive in 0..=3 {
            let mut world = World::new();
            world.profile[0] = purpose as u32;
            expect(world.lot, "habitation_state", vec![], 4);
            expect(world.lot, "travel_jobs", vec![out(&[0; 3])], RETURN);
            for wealth in 1..=if positive == 0 { 3 } else { positive } {
                let value = if wealth == positive {
                    1.0
                } else if wealth == 1 {
                    f32::NAN
                } else {
                    -1.0
                };
                expect_float(world.lot, "jobs_body", vec![w(wealth)], RETURN, F80::from_f32(value));
            }
            if positive != 0 {
                expect(world.lot, "road_access", vec![], 1);
            }
            let returned = travel_pass(&mut world, true, true, Some(7));
            let before = crate::x87::status_word() & 0x3800;
            assert_eq!(
                world.invoke(
                    fc4p_006cafc0 as *const () as usize,
                    &[world.lot as u32, world.profile.as_ptr() as u32, 1, 2]
                ),
                returned
            );
            assert_eq!(crate::x87::status_word() & 0x3800, before);
            assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
        }
    }
}
