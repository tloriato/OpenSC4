// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction state and animation cases carried from the core lot developer, with terrain call ordering.

use super::*;
use crate::lot_developer::entrypoints::*;

slot!(lock_vertices, enabled);
slot!(water);
slot!(rectangle, output);
slot!(internal_level, rect, target, water, force, lot);
slot!(level, rect, slope, raise, water, area, lot);
slot!(position, lot, output, facing);
slot!(create_building, kind, position, facing);
slot!(occupant_rect, occupant, output);
slot!(building_foundation, building, config);
slot!(construction_query, class, interface, output);
slot!(construction_start, lot);
slot!(construction_occupant, index);
slot!(insert_occupant, occupant);

unsafe extern "thiscall" fn pick(this: *mut u8, lot: u32) -> u64 {
    run(this, "pick", &[lot])
}
unsafe extern "thiscall" fn blockers(this: *mut u8, lot: u32, config: u32, kind: u32, foundation: u32) -> u64 {
    run(this, "blockers", &[lot, config, kind, foundation])
}
unsafe extern "C" fn post(kind: u32, a: u32, b: u32, c: u32, d: u32, lot: u32) -> u64 {
    run(core::ptr::null_mut(), "post", &[kind, a, b, c, d, lot])
}
unsafe extern "thiscall" fn remove(this: *mut u8, lot: u32, kind: u32) -> u64 {
    run(this, "remove", &[lot, kind])
}
unsafe extern "C" fn get_com() -> u64 {
    run(core::ptr::null_mut(), "get_com", &[])
}
unsafe extern "C" fn alloc_construction(size: u32) -> u64 {
    run(core::ptr::null_mut(), "alloc_construction", &[size])
}
unsafe extern "thiscall" fn init_construction(this: *mut u8, enabled: u32) -> u64 {
    run(this, "init_construction", &[enabled])
}

unsafe extern "thiscall" fn config_slope(this: *mut u8) -> f64 {
    run(this, "config_slope", &[]);
    FLOATS.with(|s| s.borrow_mut().pop_front().unwrap().to_f64())
}
unsafe extern "thiscall" fn lot_slope(this: *mut u8, lot: u32, water: u32) -> f64 {
    run(this, "lot_slope", &[lot, water]);
    FLOATS.with(|s| s.borrow_mut().pop_front().unwrap().to_f64())
}
unsafe extern "thiscall" fn occupant_slope(this: *mut u8, occupant: u32, water: u32) -> f64 {
    run(this, "occupant_slope", &[occupant, water]);
    FLOATS.with(|s| s.borrow_mut().pop_front().unwrap().to_f64())
}

struct Construction {
    dev: Mock,
    lot: Mock,
    config: Mock,
    bds: Mock,
    holder: Mock,
    building: Mock,
    occupant: Mock,
    profile: [u32; 4],
    frame: [u32; 24],
    _zero: Box<f32>,
    _fakes: Vec<crate::testing::Fake>,
    _fpu: GameFpu,
}

impl Construction {
    fn new() -> Self {
        let mut zero = Box::new(0.0f32);
        let guards = vec![
            fake(0x006C_B260, pick as *const () as usize),
            fake(0x006C_9AD0, blockers as *const () as usize),
            fake(0x005F_98C0, post as *const () as usize),
            fake(0x006C_9780, remove as *const () as usize),
            fake(0x0090_DDF1, get_com as *const () as usize),
            fake(0x005C_A8A0, alloc_construction as *const () as usize),
            fake(0x005D_00F0, init_construction as *const () as usize),
            fake_data(0x00A8_1054, (&mut *zero as *mut f32).cast()),
        ];
        Self {
            dev: mock!(0x4C => internal_level, 0x54 => level, 0x58 => occupant_rect, 0x78 => lot_slope,
                0x7C => occupant_slope, 0xA0 => position, 0xD8 => add_lot_config, 0xE8 => building_foundation),
            lot: mock!(4 => add_ref, 8 => release, 0xC => get_holder, 0x28 => set_configuration,
                0x30 => get_flags, 0x34 => set_flags, 0x38 => rectangle, 0x88 => set_state,
                0x90 => set_condition, 0xBC => set_growth_type, 0x114 => lock_vertices),
            config: mock!(0x28 => config_slope, 0x38 => water),
            bds: mock!(0x9C => get_profile, 0x1C => create_building),
            holder: mock!(0x24 => get_property),
            building: mock!(0xC => get_occupant),
            occupant: mock!(),
            profile: [1, 0, 0, 0],
            frame: [0xA5A5_A5A5; 24],
            _zero: zero,
            _fakes: guards,
            _fpu: GameFpu::new(),
        }
    }

    fn prefix(&mut self, kind: u32, animate: u32, zot: Option<(&mut Mock, &mut Mock, &mut u32)>) {
        let (d, l, c, b, h) = (
            self.dev.ptr(),
            self.lot.ptr(),
            self.config.ptr(),
            self.bds.ptr(),
            self.holder.ptr(),
        );
        unsafe {
            LotDeveloper::from_ptr(d).set_building_development(b);
        }
        self.frame[18] = l as u32;
        self.frame[19] = c as u32;
        self.frame[20] = kind;
        self.frame[21] = animate;
        expect(l, "add_ref", vec![], RETURN);
        expect(l, "set_configuration", vec![p(c)], RETURN);
        expect(l, "set_growth_type", vec![w(kind)], RETURN);
        expect(l, "set_state", vec![w(2), w(0), w(0)], RETURN);
        expect(l, "set_condition", vec![w(1)], RETURN);
        expect(d, "pick", vec![p(l)], RETURN);
        expect(l, "get_flags", vec![], 0xFFFF_FFFF);
        expect(l, "set_flags", vec![w(0xFFFF_FFF8)], RETURN);
        expect(l, "lock_vertices", vec![w(1)], RETURN);
        expect(l, "get_holder", vec![], h as u64);
        if let Some((property, variant, value)) = zot {
            expect(h, "get_property", vec![w(0xC999_C45E)], property.ptr() as u64);
            expect(property.ptr(), "get_variant", vec![], variant.ptr() as u64);
            expect(variant.ptr(), "get_data", vec![], value as *mut u32 as u64);
            expect(
                core::ptr::null_mut(),
                "post",
                vec![w(0x4C04_DE69), w(2), w(0), w(0), w(0), p(l)],
                RETURN,
            );
            if *value == 6 {
                expect(d, "remove", vec![p(l), w(6)], RETURN);
                expect(variant.ptr(), "set_uint", vec![w(0)], RETURN);
            }
        } else {
            expect(h, "get_property", vec![w(0xC999_C45E)], 0);
        }
    }

    fn profile_water(&mut self, kind: u32, water_flag: u8) -> u32 {
        expect(
            self.bds.ptr(),
            "get_profile",
            vec![w(kind)],
            self.profile.as_mut_ptr() as u64,
        );
        expect(self.config.ptr(), "water", vec![], u64::from(water_flag));
        (self.lot.ptr() as u32 & !255) | u32::from(water_flag)
    }

    fn floating(&mut self, object: *mut u8, method: &'static str, args: Vec<Arg>, value: f64) {
        expect(object, method, args, RETURN);
        FLOATS.with(|s| s.borrow_mut().push_back(F80::from_f64(value)));
    }

    fn create(&mut self, kind: u32, foundation: u8, created: bool) {
        let (d, l, c, b) = (self.dev.ptr(), self.lot.ptr(), self.config.ptr(), self.bds.ptr());
        expect(
            d,
            "blockers",
            vec![p(l), p(c), w(kind), w(0xA5A5_A500 | u32::from(foundation))],
            RETURN,
        );
        expect(
            d,
            "position",
            vec![
                p(l),
                out(&[1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()]),
                out(&[2]),
            ],
            RETURN,
        );
        expect(
            b,
            "create_building",
            vec![
                w(kind),
                Arg::In(vec![1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()]),
                w(2),
            ],
            if created { self.building.ptr() as u64 } else { 0 },
        );
    }

    fn finish(&mut self, animate: bool) {
        if !animate {
            expect(self.lot.ptr(), "set_state", vec![w(3), w(0), w(0)], RETURN);
        }
        expect(self.lot.ptr(), "release", vec![], RETURN);
    }

    fn invoke(&mut self, ok: bool) {
        let returned = unsafe { start_construction(self.dev.ptr(), self.frame.as_mut_ptr().add(1).cast()) };
        assert_eq!(returned, RETURN & !255 | u64::from(ok));
        finished();
        FLOATS.with(|s| assert!(s.borrow().is_empty()));
    }
}

#[test]
fn construction_type_zero_succeeds_and_releases_the_lot() {
    for animate in [0, 1, 0xABCD_0100] {
        let mut world = Construction::new();
        world.prefix(0, animate, None);
        world.finish(animate as u8 != 0);
        world.invoke(true);
    }
}

#[test]
fn construction_without_animation_sets_occupied_even_if_building_creation_fails() {
    for created in [false, true] {
        let mut world = Construction::new();
        world.prefix(0x3000, 0, None);
        let water = world.profile_water(0x3000, 0);
        let dev = world.dev.ptr();
        let lot = world.lot.ptr();
        world.floating(dev, "lot_slope", vec![p(lot), w(water)], 0.0);
        world.create(0x3000, 0, created);
        if created {
            expect(
                world.building.ptr(),
                "get_occupant",
                vec![],
                world.occupant.ptr() as u64,
            );
            let occupant = world.occupant.ptr();
            world.floating(dev, "occupant_slope", vec![p(occupant), w(water)], 0.0);
        }
        world.finish(false);
        world.invoke(created);
        assert_eq!(world.frame[18], water);
        assert_eq!(&world.frame[22..], &[0xA5A5_A5A5; 2]);
    }
}

#[test]
fn construction_export_balances_its_four_argument_words_and_returns_both_registers() {
    let mut world = Construction::new();
    world.prefix(0, 0, None);
    world.finish(false);
    let returned = unsafe {
        call_thiscall_with_eax(
            fc4p_006cc490 as *const () as usize,
            world.dev.ptr(),
            &[world.lot.ptr() as u32, world.config.ptr() as u32, 0, 0],
            0x1234_5678,
        )
    };
    assert_eq!(returned, ((RETURN as u32 & !255) | 1, (RETURN >> 32) as u32));
    finished();
}

#[test]
fn construction_notifies_for_each_zot_and_removes_only_the_road_zot() {
    for value in [6, 7, 0x100] {
        let mut world = Construction::new();
        let mut property = mock!(0x14 => get_variant);
        let mut variant = mock!(0x124 => get_data, 0x68 => set_uint);
        let mut zot = value;
        world.prefix(0, 0, Some((&mut property, &mut variant, &mut zot)));
        world.finish(false);
        world.invoke(true);
        assert_eq!(world.frame[18], value);
    }
}

#[test]
fn water_foundation_skips_slopes_and_animation_uses_factory_or_game_allocation() {
    for factory_success in [false, true] {
        let mut world = Construction::new();
        world.prefix(0x3000, 0x101, None);
        world.profile_water(0x3000, 2);
        let (dev, lot, config) = (world.dev.ptr(), world.lot.ptr(), world.config.ptr());
        world.floating(config, "config_slope", vec![], -0.0);
        expect(dev, "add_lot_config", vec![p(lot), p(config), w(0), w(1), w(1)], 2);
        world.create(0x3000, 2, true);
        let mut factory = mock!(0xC => construction_query);
        let mut construction =
            mock!(4 => add_ref, 8 => release, 0x10 => construction_start, 0xC => construction_occupant);
        let mut manager = mock!(0x50 => insert_occupant);
        unsafe {
            LotDeveloper::from_ptr(dev).set_occupant_manager(manager.ptr());
        }
        expect(core::ptr::null_mut(), "get_com", vec![], factory.ptr() as u64);
        expect(
            factory.ptr(),
            "construction_query",
            vec![w(0xA97F_909E), w(0x496E_7636), out(&[construction.ptr() as u32])],
            u64::from(factory_success),
        );
        if !factory_success {
            expect(
                core::ptr::null_mut(),
                "alloc_construction",
                vec![w(0x1A0)],
                construction.ptr() as u64,
            );
            expect(
                construction.ptr(),
                "init_construction",
                vec![w(1)],
                construction.ptr() as u64,
            );
            expect(construction.ptr(), "add_ref", vec![], RETURN);
        }
        expect(construction.ptr(), "construction_start", vec![p(lot)], RETURN);
        expect(construction.ptr(), "construction_occupant", vec![w(0)], 0x1234);
        expect(manager.ptr(), "insert_occupant", vec![w(0x1234)], RETURN);
        expect(construction.ptr(), "release", vec![], RETURN);
        world.finish(true);
        world.invoke(true);
        assert_eq!(world.frame[20], construction.ptr() as u32);
    }
}

#[test]
fn construction_levels_internal_vertices_then_lot_slope_and_tests_the_full_level_result() {
    for level_result in [0, 0x100] {
        let mut world = Construction::new();
        world.prefix(0x3000, 0, None);
        let water = world.profile_water(0x3000, 0);
        let (dev, lot, config) = (world.dev.ptr(), world.lot.ptr(), world.config.ptr());
        unsafe {
            let layout = LotDeveloper::from_ptr(dev);
            layout.set_foundation_slope(0.1);
            layout.set_level_max_slope(0.5);
            dev.add(0xE0).write(1);
            dev.add(0xE1).write(1);
        }
        expect(lot, "rectangle", vec![out(&[4, 5, 7, 9])], RETURN);
        expect(
            dev,
            "internal_level",
            vec![
                Arg::In(vec![4, 5, 7, 9]),
                w((-1.0f32).to_bits()),
                w(water),
                w(0),
                p(lot),
            ],
            RETURN,
        );
        world.floating(dev, "lot_slope", vec![p(lot), w(water)], 0.75);
        world.floating(config, "config_slope", vec![], 0.25);
        expect(lot, "rectangle", vec![out(&[4, 5, 7, 9])], RETURN);
        expect(
            dev,
            "level",
            vec![
                Arg::In(vec![4, 5, 7, 9]),
                w(0.25f32.to_bits()),
                w(1),
                w(water),
                w(0.5f32.to_bits()),
                p(lot),
            ],
            level_result,
        );
        if level_result != 0 {
            world.floating(dev, "lot_slope", vec![p(lot), w(water)], 0.0);
        } else {
            expect(dev, "add_lot_config", vec![p(lot), p(config), w(0), w(1), w(1)], 1);
        }
        world.create(0x3000, u8::from(level_result == 0), false);
        world.finish(false);
        world.invoke(false);
    }
}

#[test]
fn construction_water_requires_exact_zero_and_unordered_slopes_skip_foundations() {
    for max_slope in [0.25, f64::NAN] {
        let mut world = Construction::new();
        world.prefix(0x3000, 0, None);
        let water = world.profile_water(0x3000, 1);
        let (dev, lot, config) = (world.dev.ptr(), world.lot.ptr(), world.config.ptr());
        world.floating(config, "config_slope", vec![], max_slope);
        world.floating(dev, "lot_slope", vec![p(lot), w(water)], f64::NAN);
        world.create(0x3000, 0, false);
        world.finish(false);
        world.invoke(false);
    }
}

#[test]
fn construction_building_slope_compares_the_unrounded_register_then_the_stored_single() {
    for level_result in [0, 1] {
        let mut world = Construction::new();
        world.prefix(0x3000, 0, None);
        let water = world.profile_water(0x3000, 0);
        let (dev, lot, occupant) = (world.dev.ptr(), world.lot.ptr(), world.occupant.ptr());
        unsafe {
            LotDeveloper::from_ptr(dev).set_foundation_slope(1.0);
        }
        world.floating(dev, "lot_slope", vec![p(lot), w(water)], 0.0);
        world.create(0x3000, 0, true);
        expect(world.building.ptr(), "get_occupant", vec![], occupant as u64);
        world.floating(
            dev,
            "occupant_slope",
            vec![p(occupant), w(water)],
            1.0 + 2.0f64.powi(-30),
        );
        expect(world.building.ptr(), "get_occupant", vec![], occupant as u64);
        expect(dev, "occupant_rect", vec![p(occupant), out(&[1, 2, 3, 4])], RETURN);
        expect(
            dev,
            "level",
            vec![
                Arg::In(vec![1, 2, 3, 4]),
                w(1.0f32.to_bits()),
                w(1),
                w(water),
                w(0),
                p(lot),
            ],
            level_result,
        );
        if level_result != 0 {
            expect(world.building.ptr(), "get_occupant", vec![], occupant as u64);
            world.floating(
                dev,
                "occupant_slope",
                vec![p(occupant), w(water)],
                1.0 + 2.0f64.powi(-30),
            );
            expect(
                dev,
                "building_foundation",
                vec![p(world.building.ptr()), p(world.config.ptr())],
                RETURN,
            );
        }
        world.finish(false);
        world.invoke(true);
    }
}

slot!(message_kind);
slot!(data1);
slot!(void1);
slot!(data2);
slot!(data3);
slot!(data4);
slot!(unknown);
slot!(lighting_mode);
slot!(notification_type, kind);
slot!(notification_post, message, immediate);
slot!(clock_day);
slot!(demand_get, kind, mask);
slot!(query_lot, id, output);
slot!(lot_location, x, z);
slot!(lot_size, width, height);
slot!(message_textures, lot, config, occupied);

macro_rules! direct_slot {
    ($name:ident $(, $arg:ident)*) => {
        unsafe extern "thiscall" fn $name(this: *mut u8 $(, $arg: u32)*) -> u64 {
            run(this, stringify!($name), &[$($arg),*])
        }
    };
}
direct_slot!(message_daily);
direct_slot!(message_removed, occupant);
direct_slot!(message_inserted, occupant);
direct_slot!(message_tick, millis);
direct_slot!(message_lighting, mode);
direct_slot!(message_power, rectangle);
direct_slot!(message_props, lot, config, occupied);
direct_slot!(message_base, lot, config, occupied);
direct_slot!(message_flora, lot, config, occupied);

unsafe extern "thiscall" fn message_new(this: *mut u8) -> u64 {
    run(this, "message_new", &[])
}
unsafe extern "thiscall" fn demand_value(this: *mut u8) -> f64 {
    run(this, "demand_value", &[]);
    FLOATS.with(|s| s.borrow_mut().pop_front().unwrap().to_f64())
}
unsafe extern "thiscall" fn queue_first(this: *mut u8, output: *mut u32) -> u64 {
    run(this, "queue_first", &[output as u32]) & !0xFFFF_FFFF | output as u64
}
unsafe extern "thiscall" fn queue_last(this: *mut u8, output: *mut u32) -> u64 {
    run(this, "queue_last", &[output as u32]) & !0xFFFF_FFFF | output as u64
}
unsafe extern "C" fn message_shuffle(first: queue::QueueIterator, last: queue::QueueIterator) -> u64 {
    run(
        core::ptr::null_mut(),
        "message_shuffle",
        &[
            first.current as u32,
            first.first as u32,
            first.last as u32,
            first.node as u32,
            last.current as u32,
            last.first as u32,
            last.last as u32,
            last.node as u32,
        ],
    )
}

struct Messages {
    dev: Mock,
    message: Mock,
    frame: [u32; 32],
    _fakes: Vec<crate::testing::Fake>,
}
impl Messages {
    fn new(kind: u32) -> Self {
        let guards = vec![
            fake(0x006C_D770, message_daily as *const () as usize),
            fake(0x006C_8F80, message_removed as *const () as usize),
            fake(0x006C_8830, message_inserted as *const () as usize),
            fake(0x006C_BD60, message_tick as *const () as usize),
            fake(0x006C_BE50, message_lighting as *const () as usize),
            fake(0x006C_BF40, message_power as *const () as usize),
            fake(0x006C_7860, message_props as *const () as usize),
            fake(0x006C_D2B0, message_base as *const () as usize),
            fake(0x006C_9C80, message_flora as *const () as usize),
            fake(0x0091_33DA, operation_allocate as *const () as usize),
            fake(0x0091_34D6, message_new as *const () as usize),
            fake(0x006C_5FE0, queue_first as *const () as usize),
            fake(0x0043_1AC0, queue_last as *const () as usize),
            fake(0x006C_68C0, message_shuffle as *const () as usize),
        ];
        let mut world = Self {
            dev: mock!(0xB0 => message_textures),
            message: mock!(0x10 => message_kind, 0x24 => data1, 0x28 => void1, 0x38 => data2,
                0x4C => data3, 0x60 => data4, 0x80 => unknown),
            frame: [0xA5A5_A5A5; 32],
            _fakes: guards,
        };
        world.frame[30] = world.message.ptr() as u32;
        expect(
            world.message.ptr(),
            "message_kind",
            vec![],
            RETURN & !0xFFFF_FFFF | u64::from(kind),
        );
        world
    }
    fn invoke(&mut self) -> u64 {
        let returned = unsafe { do_message(self.dev.ptr().add(4), self.frame.as_mut_ptr().add(9).cast()) };
        finished();
        returned
    }
}

#[test]
fn messages_ignored_types_keep_upper_eax_bytes_and_daily_returns_its_callee_result() {
    for kind in [0, 0x1234_5678, 0x6695_6810, 0x6695_6811, 0x7000_0000, u32::MAX] {
        let mut world = Messages::new(kind);
        let eax = if kind > 0x6695_6810 && kind < 0x6924_7DC7 {
            kind.wrapping_sub(0x6695_6814)
        } else {
            kind
        };
        assert_eq!(world.invoke(), RETURN & !0xFFFF_FFFF | u64::from(eax & !255 | 1));
    }
    let mut world = Messages::new(0x6695_6814);
    expect(world.dev.ptr(), "message_daily", vec![], RETURN);
    assert_eq!(world.invoke(), RETURN);
    let mut world = Messages::new(0x278E_6003);
    for name in ["data1", "data2", "data4"] {
        expect(world.message.ptr(), name, vec![], RETURN);
    }
    assert_eq!(world.invoke(), RETURN & !255 | 1);
}

#[test]
fn messages_animation_idle_returns_false_and_tick_divides_unsigned_microseconds() {
    let mut world = Messages::new(0x86AD_10EE);
    assert_eq!(world.invoke(), RETURN & !0xFFFF_FFFF);
    for micros in [0, 1, 999, 1000, 2000, 500_000, u32::MAX] {
        let mut world = Messages::new(0x86AD_10EE);
        unsafe {
            world.dev.ptr().add(0x9C).cast::<u32>().write_unaligned(0x0100);
        }
        expect(world.message.ptr(), "data2", vec![], u64::from(micros));
        expect(world.dev.ptr(), "message_tick", vec![w(micros / 1000)], RETURN);
        assert_eq!(world.invoke(), RETURN & !255 | 1);
    }
}

#[test]
fn messages_building_routes_and_network_flags_use_primary_interface_offsets() {
    for (kind, name) in [(0x99EF_1142, "message_removed"), (0x99EF_1143, "message_inserted")] {
        for occupant_kind in [0x2781_28A0u32, 0xC772_BF98, 0x1122_3300] {
            let mut world = Messages::new(kind);
            expect(world.message.ptr(), "data3", vec![], u64::from(occupant_kind));
            if occupant_kind == 0x2781_28A0 {
                expect(world.message.ptr(), "void1", vec![], 0x1234);
                expect(world.dev.ptr(), name, vec![w(0x1234)], RETURN);
            }
            let returned = world.invoke();
            assert_eq!(returned as u8, 1);
            unsafe {
                assert_eq!(*world.dev.ptr().add(0x5D), u8::from(occupant_kind == 0xC772_BF98));
                assert_eq!(*world.dev.ptr().add(0x5E), u8::from(occupant_kind == 0xC772_BF98));
                assert_eq!(*world.dev.ptr().add(0x5C), 0);
                assert_eq!(*world.dev.ptr().add(0x5F), 0);
            }
        }
    }
}

#[test]
fn messages_city_grace_period_wraps_and_commute_check_requires_positive_ordered_demand() {
    for date in [0, u32::MAX, 0xFFFF_FFE1] {
        let mut world = Messages::new(0x26D3_1EC4);
        let mut clock = mock!(0x20 => clock_day);
        let mut pointer = clock.ptr();
        let _global = fake_data(0x00B4_3D5C, (&mut pointer as *mut *mut u8).cast());
        expect(
            clock.ptr(),
            "clock_day",
            vec![],
            RETURN & !0xFFFF_FFFF | u64::from(date),
        );
        let eax = date.wrapping_add(31);
        assert_eq!(world.invoke(), RETURN & !0xFFFF_FFFF | u64::from(eax & !255 | 1));
        assert_eq!(unsafe { world.dev.ptr().add(0x60).cast::<u32>().read_unaligned() }, eax);
    }
    let _fpu = GameFpu::new();
    for value in [f64::NEG_INFINITY, -1.0, -0.0, 0.0, 1e-300, 1.0, f64::INFINITY, f64::NAN] {
        let mut world = Messages::new(0x6924_7DC7);
        let mut manager = mock!(0x18 => demand_get);
        let mut demand = mock!(0x24 => demand_value);
        let mut pointer = manager.ptr();
        let mut zero = 0.0f32;
        let _manager = fake_data(0x00B4_3D74, (&mut pointer as *mut *mut u8).cast());
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        expect(
            manager.ptr(),
            "demand_get",
            vec![w(0x2000), w(0x20000)],
            demand.ptr() as u64,
        );
        expect(demand.ptr(), "demand_value", vec![], RETURN);
        FLOATS.with(|s| s.borrow_mut().push_back(F80::from_f64(value)));
        assert_eq!(world.invoke() as u32, 1);
        assert_eq!(unsafe { *world.dev.ptr().add(0x5F) }, u8::from(value > 0.0));
    }
}

#[test]
fn messages_power_queries_reverse_order_and_overwrites_the_message_argument() {
    let mut world = Messages::new(0x8A07_AE00);
    for (name, value) in [("data4", 40), ("data3", 30), ("data2", 20), ("data1", 10)] {
        expect(world.message.ptr(), name, vec![], value);
    }
    expect(
        world.dev.ptr(),
        "message_power",
        vec![Arg::In(vec![10, 20, 30, 40])],
        RETURN,
    );
    assert_eq!(world.invoke(), RETURN & !255 | 1);
    assert_eq!(world.frame[30], 20);
}

#[test]
fn messages_lighting_immediate_posts_and_deferred_shuffles_and_stores_all_cursor_words() {
    for mode in [1, 2] {
        for immediate in [false, true] {
            let mut world = Messages::new(0x4477_8234);
            let mut manager = mock!(0x18 => lighting_mode);
            let mut notification = mock!(4 => add_ref, 8 => release, 0x14 => notification_type);
            let mut server = mock!(0x10 => notification_post);
            let mut pointer = manager.ptr();
            let _global = fake_data(0x00B4_3DBC, (&mut pointer as *mut *mut u8).cast());
            expect(world.message.ptr(), "data2", vec![], u64::from(u32::from(mode == 1)));
            expect(
                manager.ptr(),
                "lighting_mode",
                vec![],
                if immediate { mode.into() } else { 3 },
            );
            if immediate {
                unsafe {
                    world
                        .dev
                        .ptr()
                        .add(0x14)
                        .cast::<u32>()
                        .write_unaligned(server.ptr() as u32);
                }
                expect(world.dev.ptr(), "message_lighting", vec![w(mode)], RETURN);
                expect(
                    core::ptr::null_mut(),
                    "operation_allocate",
                    vec![w(0x2C)],
                    notification.ptr() as u64,
                );
                expect(notification.ptr(), "message_new", vec![], notification.ptr() as u64);
                expect(notification.ptr(), "add_ref", vec![], RETURN);
                expect(notification.ptr(), "notification_type", vec![w(0xA6B7_9621)], RETURN);
                expect(
                    server.ptr(),
                    "notification_post",
                    vec![p(notification.ptr()), w(0)],
                    RETURN,
                );
                expect(notification.ptr(), "release", vec![], RETURN);
                assert_eq!(world.invoke(), RETURN & !255 | 1);
            } else {
                let queue = unsafe { world.dev.ptr().add(0x64) };
                let end = [0x4000, 0x5000, 0x6000, 0x7000];
                let begin = [0x1000, 0x2000, 0x3000, 0x4000];
                let next = [0x8000, 0x9000, 0xA000, 0xB000];
                expect(queue, "queue_last", vec![out(&end)], RETURN);
                expect(queue, "queue_first", vec![out(&begin)], RETURN);
                expect(
                    core::ptr::null_mut(),
                    "message_shuffle",
                    begin.iter().chain(&end).map(|v| w(*v)).collect(),
                    RETURN,
                );
                expect(queue, "queue_first", vec![out(&next)], RETURN);
                let (eax, edx) = unsafe {
                    call_thiscall_with_eax(
                        fc4p_006cd940 as *const () as usize,
                        world.dev.ptr().add(4),
                        &[world.message.ptr() as u32],
                        0x1234_5678,
                    )
                };
                finished();
                let returned = u64::from(eax) | u64::from(edx) << 32;
                assert_eq!(returned as u8, 1);
                assert_eq!(returned >> 32, u64::from(next[if mode == 1 { 2 } else { 3 }]));
                unsafe {
                    assert_eq!(world.dev.ptr().add(0x9C).cast::<u32>().read_unaligned(), mode);
                    assert_eq!(
                        std::slice::from_raw_parts(world.dev.ptr().add(0x8C).cast::<u32>(), 4),
                        next
                    );
                }
            }
        }
    }
    let mut world = Messages::new(0x4477_8234);
    expect(world.message.ptr(), "data2", vec![], 0x1234_5602);
    assert_eq!(world.invoke() as u32, 0x1234_5601);
}

#[test]
fn messages_condition_queries_lot_interface_checks_state_and_releases_the_interface() {
    for (query_ok, found, state) in [
        (false, false, 0),
        (true, false, 0),
        (true, true, 2),
        (true, true, 3),
        (true, true, 4),
    ] {
        let mut world = Messages::new(0xE99B_3D4D);
        let mut object = mock!(0 => query_lot);
        let mut source = mock!(0x54 => lot_location, 8 => release);
        let mut manager = mock!(0x18 => get_lot);
        let mut lot = mock!(0x84 => get_type, 0x24 => get_config);
        unsafe {
            world
                .dev
                .ptr()
                .add(0x24)
                .cast::<u32>()
                .write_unaligned(manager.ptr() as u32);
        }
        expect(world.message.ptr(), "unknown", vec![], object.ptr() as u64);
        expect(
            object.ptr(),
            "query_lot",
            vec![w(0xE818_FCED), out(&[source.ptr() as u32])],
            u64::from(query_ok),
        );
        if query_ok {
            expect(source.ptr(), "lot_location", vec![out(&[10]), out(&[20])], RETURN);
            expect(
                manager.ptr(),
                "get_lot",
                vec![w(10), w(20), w(1)],
                if found { lot.ptr() as u64 } else { 0 },
            );
            if found {
                expect(lot.ptr(), "get_type", vec![], state);
                if matches!(state, 3 | 4) {
                    expect(lot.ptr(), "get_config", vec![], 0x1234);
                    expect(
                        world.dev.ptr(),
                        "message_textures",
                        vec![p(lot.ptr()), w(0x1234), w(1)],
                        RETURN,
                    );
                }
            }
            expect(source.ptr(), "release", vec![], RETURN);
        }
        let returned = world.invoke();
        assert_eq!(returned as u8, 1);
        assert_eq!(world.frame[30], source.ptr() as u32);
    }
}

#[test]
fn messages_state_only_refreshes_matching_lot_size_and_reoccupation_adds_props_then_flora() {
    for (old, new, query_ok, found, config_present, same_width, same_height) in [
        (2, 2, false, false, false, false, false),
        (2, 3, false, false, false, false, false),
        (2, 3, true, false, false, false, false),
        (2, 3, true, true, false, false, false),
        (2, 3, true, true, true, false, true),
        (2, 3, true, true, true, true, false),
        (2, 3, true, true, true, true, true),
        (1, 3, true, true, true, true, true),
        (2, 4, true, true, true, true, true),
    ] {
        let mut world = Messages::new(0xE8BD_A2E7);
        let mut object = mock!(0 => query_lot);
        let mut source = mock!(0x54 => lot_location, 0x58 => lot_size, 8 => release);
        let mut manager = mock!(0x18 => get_lot);
        let mut lot = mock!(0x24 => get_config, 0x58 => lot_size);
        unsafe {
            world
                .dev
                .ptr()
                .add(0x24)
                .cast::<u32>()
                .write_unaligned(manager.ptr() as u32);
        }
        expect(world.message.ptr(), "data1", vec![], old);
        expect(world.message.ptr(), "data2", vec![], new);
        if matches!(new, 3 | 4) {
            expect(world.message.ptr(), "unknown", vec![], object.ptr() as u64);
            expect(
                object.ptr(),
                "query_lot",
                vec![w(0xE818_FCED), out(&[source.ptr() as u32])],
                u64::from(query_ok),
            );
            if query_ok {
                expect(source.ptr(), "lot_location", vec![out(&[10]), out(&[20])], RETURN);
                expect(
                    source.ptr(),
                    "lot_size",
                    vec![Arg::Bytes(vec![3]), Arg::Bytes(vec![4])],
                    RETURN,
                );
                expect(
                    manager.ptr(),
                    "get_lot",
                    vec![w(10), w(20), w(1)],
                    if found { lot.ptr() as u64 } else { 0 },
                );
                if found {
                    expect(lot.ptr(), "get_config", vec![], if config_present { 0x1234 } else { 0 });
                    if config_present {
                        expect(
                            lot.ptr(),
                            "lot_size",
                            vec![
                                Arg::Bytes(vec![if same_width { 3 } else { 1 }]),
                                Arg::Bytes(vec![if same_height { 4 } else { 2 }]),
                            ],
                            RETURN,
                        );
                        if same_width && same_height {
                            expect(
                                world.dev.ptr(),
                                "message_textures",
                                vec![p(lot.ptr()), w(0x1234), w(1)],
                                RETURN,
                            );
                            if old == 2 && new == 3 {
                                for name in ["message_props", "message_base", "message_flora"] {
                                    expect(world.dev.ptr(), name, vec![p(lot.ptr()), w(0x1234), w(1)], RETURN);
                                }
                            }
                        }
                    }
                }
                expect(source.ptr(), "release", vec![], RETURN);
            }
        }
        let returned = world.invoke();
        assert_eq!(returned as u8, 1);
        assert_eq!(world.frame[30] & 255, u32::from(matches!(new, 3 | 4)));
    }
}

#[test]
fn message_export_uses_the_secondary_interface_and_pops_one_word() {
    let mut world = Messages::new(0x1234_5678);
    let returned = unsafe {
        call_thiscall_with_eax(
            fc4p_006cd940 as *const () as usize,
            world.dev.ptr().add(4),
            &[world.message.ptr() as u32],
            0x1234,
        )
    };
    assert_eq!(returned, (0x1234_5601, (RETURN >> 32) as u32));
    finished();
}

slot!(zone_color, zone);
slot!(zone_alpha);
slot!(city_depth);
slot!(city_width);
slot!(facing);
slot!(boundary, sides, texture, facing, flip);
slot!(empty_texture, lot, output, create);
slot!(texture_specification, cells, count, unused);
slot!(occupant_flag, flag);
slot!(occupant_changed, occupant, unused);
slot!(occupant_inserted, occupant, unused);
slot!(occupant_post, kind, unused);
direct_slot!(clear_empty, lot);

struct EmptyTextures {
    dev: Mock,
    lot: Mock,
    zones: Mock,
    city: Mock,
    lots: Mock,
    texture: Mock,
    occupant: Mock,
    occupants: Mock,
    frame: [u32; 36],
    _globals: Box<[*mut u8; 2]>,
    _fakes: Vec<crate::testing::Fake>,
}
impl EmptyTextures {
    fn new() -> Self {
        let mut world = Self {
            dev: mock!(0xA8 => boundary, 0xA4 => empty_texture),
            lot: mock!(0x7C => get_zone, 0x38 => rectangle, 0x58 => lot_size, 0x5C => facing),
            zones: mock!(0x3C => zone_color, 0x40 => zone_alpha),
            city: mock!(0x178 => city_depth, 0x174 => city_width),
            lots: mock!(0x18 => get_lot),
            texture: mock!(0x20 => texture_specification, 0xC => get_occupant, 8 => release),
            occupant: mock!(0x80 => occupant_flag, 0x3C => occupant_post),
            occupants: mock!(0x50 => occupant_inserted, 0x60 => occupant_changed),
            frame: [0xA5A5_A5A5; 36],
            _globals: Box::new([core::ptr::null_mut(); 2]),
            _fakes: vec![],
        };
        world._globals[0] = world.zones.ptr();
        world._globals[1] = world.city.ptr();
        world._fakes = vec![
            fake_data(0x00B4_3D14, world._globals.as_mut_ptr().cast()),
            fake_data(0x00B4_3CEC, unsafe { world._globals.as_mut_ptr().add(1).cast() }),
            fake(0x006C_AAF0, clear_empty as *const () as usize),
            fake(0x0090_CF54, operation_allocate as *const () as usize),
            fake(0x0090_CF63, operation_free as *const () as usize),
        ];
        let developer = unsafe { LotDeveloper::from_ptr(world.dev.ptr()) };
        developer.set_lot_manager(world.lots.ptr());
        developer.set_occupant_manager(world.occupants.ptr());
        unsafe {
            world.dev.ptr().add(0x4C).cast::<u32>().write_unaligned(8);
            world.dev.ptr().add(0x50).cast::<u32>().write_unaligned(8);
        }
        world.frame[34] = world.lot.ptr() as u32;
        world
    }
    fn prefix(&mut self, zone: u32, rect: [u32; 4], size: [u8; 2]) {
        expect(self.lot.ptr(), "get_zone", vec![], u64::from(zone));
        expect(self.zones.ptr(), "zone_color", vec![w(zone)], 0xAABB_CCDD);
        expect(self.zones.ptr(), "zone_alpha", vec![], 0x1234_567F);
        expect(self.city.ptr(), "city_depth", vec![], 8);
        expect(self.city.ptr(), "city_width", vec![], 8);
        expect(self.lot.ptr(), "rectangle", vec![out(&rect)], RETURN);
        if (7..=9).contains(&zone) {
            expect(
                self.lot.ptr(),
                "lot_size",
                vec![Arg::Bytes(vec![size[0]]), Arg::Bytes(vec![size[1]])],
                RETURN,
            );
        }
    }
    fn cell(&mut self, sides: [u32; 4], texture: u32, angle: u8, flip: u8) {
        expect(
            self.dev.ptr(),
            "boundary",
            vec![
                Arg::In(sides.to_vec()),
                out(&[texture]),
                Arg::Bytes(vec![angle]),
                Arg::Bytes(vec![flip]),
            ],
            RETURN,
        );
    }
    fn finish(&mut self, records: Option<&[u32]>, inserted: bool) {
        expect(
            self.dev.ptr(),
            "empty_texture",
            vec![p(self.lot.ptr()), out(&[self.texture.ptr() as u32]), w(1)],
            u64::from(records.is_some()),
        );
        if let Some(records) = records {
            expect(
                self.texture.ptr(),
                "texture_specification",
                vec![
                    if records.is_empty() {
                        w(0)
                    } else {
                        Arg::In(records.to_vec())
                    },
                    w(records.len() as u32 / 4),
                    w(0),
                ],
                RETURN,
            );
            expect(self.texture.ptr(), "get_occupant", vec![], self.occupant.ptr() as u64);
            expect(
                self.occupant.ptr(),
                "occupant_flag",
                vec![w(1)],
                if inserted { 1 } else { 0x100 },
            );
            if inserted {
                expect(
                    self.occupants.ptr(),
                    "occupant_changed",
                    vec![p(self.occupant.ptr()), w(0)],
                    RETURN,
                );
                expect(self.occupant.ptr(), "occupant_post", vec![w(0x99EF_1145), w(0)], RETURN);
            } else {
                expect(
                    self.occupants.ptr(),
                    "occupant_inserted",
                    vec![p(self.occupant.ptr()), w(0)],
                    RETURN,
                );
            }
            expect(self.texture.ptr(), "release", vec![], RETURN);
        }
    }
    fn invoke(&mut self) -> u64 {
        let returned = unsafe { empty_textures(self.dev.ptr(), self.frame.as_mut_ptr().add(1).cast()) };
        finished();
        returned
    }
}

#[test]
fn empty_textures_clear_unzoned_and_special_zones_through_the_game_function() {
    for zone in [0, 11, 12, 15] {
        let mut world = EmptyTextures::new();
        expect(world.lot.ptr(), "get_zone", vec![], zone);
        expect(world.dev.ptr(), "clear_empty", vec![p(world.lot.ptr())], RETURN);
        assert_eq!(world.invoke(), RETURN);
    }
}

#[test]
fn empty_textures_still_updates_the_occupant_with_no_cells_and_checks_only_the_flag_low_byte() {
    for (rect, inserted) in [
        ([5, 2, 4, 3], false),
        ([2, 5, 3, 4], true),
        ([8, 8, 9, 9], false),
        ([u32::MAX, 0, u32::MAX, 0], true),
    ] {
        let mut world = EmptyTextures::new();
        world.prefix(1, rect, [1, 1]);
        world.finish(Some(&[]), inserted);
        assert_eq!(world.invoke(), RETURN);
    }
}

#[test]
fn empty_industrial_one_by_one_suppresses_boundaries_and_cell_buffer_grows_one_two_four() {
    for inserted in [false, true] {
        let mut world = EmptyTextures::new();
        world.prefix(7, [2, 3, 3, 4], [1, 1]);
        let mut first = Box::new([0xCCCC_CCCCu32; 4]);
        let mut second = Box::new([0xCCCC_CCCCu32; 8]);
        let mut third = Box::new([0xCCCC_CCCCu32; 16]);
        let mut records = vec![];
        for (index, (x, z)) in [(2, 3), (2, 4), (3, 3), (3, 4)].into_iter().enumerate() {
            world.cell([0; 4], 0x1234 + index as u32, 2, 3);
            records.extend([
                0x1234 + index as u32,
                x | z << 8 | 2 << 16 | 1 << 24,
                0x7FCC_BBAA,
                0xA5A5_FF03,
            ]);
            match index {
                0 => expect(
                    core::ptr::null_mut(),
                    "operation_allocate",
                    vec![w(16)],
                    first.as_mut_ptr() as u64,
                ),
                1 => {
                    expect(
                        core::ptr::null_mut(),
                        "operation_allocate",
                        vec![w(32)],
                        second.as_mut_ptr() as u64,
                    );
                    expect(
                        core::ptr::null_mut(),
                        "operation_free",
                        vec![w(first.as_mut_ptr() as u32)],
                        RETURN,
                    );
                }
                2 => {
                    expect(
                        core::ptr::null_mut(),
                        "operation_allocate",
                        vec![w(64)],
                        third.as_mut_ptr() as u64,
                    );
                    expect(
                        core::ptr::null_mut(),
                        "operation_free",
                        vec![w(second.as_mut_ptr() as u32)],
                        RETURN,
                    );
                }
                _ => {}
            }
        }
        world.finish(Some(&records), inserted);
        expect(
            core::ptr::null_mut(),
            "operation_free",
            vec![w(third.as_mut_ptr() as u32)],
            RETURN,
        );
        assert_eq!(world.invoke(), RETURN);
        assert_eq!(third.as_slice(), records);
        assert_eq!(world.frame[34], world.lot.ptr() as u32);
    }
}

#[test]
fn empty_texture_edges_query_north_south_west_east_and_use_the_current_lot_facing() {
    for (zone, x, z, facing_value) in [(1, 2u32, 3u32, 2u32), (16, 0, 0, 0), (8, 7, 7, 1)] {
        let mut world = EmptyTextures::new();
        world.prefix(zone, [x, z, x, z], [2, 2]);
        let mut sides = [1u32; 4];
        for (side, nx, nz) in [
            (0, x, z.wrapping_sub(1)),
            (2, x, z.wrapping_add(1)),
            (3, x.wrapping_sub(1), z),
            (1, x.wrapping_add(1), z),
        ] {
            let same_lot = side == 3 && nx < 8 && nz < 8;
            if nx < 8 && nz < 8 {
                expect(
                    world.lots.ptr(),
                    "get_lot",
                    vec![w(nx), w(nz), w(0)],
                    if same_lot { world.lot.ptr() as u64 } else { 0 },
                );
            }
            if same_lot {
                sides[side] = 0;
            } else {
                expect(world.lot.ptr(), "facing", vec![], u64::from(facing_value));
                if side as u32 == facing_value {
                    sides[side] = 2;
                }
            }
        }
        world.cell(sides, 0x5678, 3, 0);
        let mut storage = Box::new([0u32; 4]);
        expect(
            core::ptr::null_mut(),
            "operation_allocate",
            vec![w(16)],
            storage.as_mut_ptr() as u64,
        );
        world.finish(None, false);
        expect(
            core::ptr::null_mut(),
            "operation_free",
            vec![w(storage.as_mut_ptr() as u32)],
            RETURN,
        );
        assert_eq!(world.invoke(), RETURN);
        assert_eq!(*storage, [0x5678, x | z << 8 | 3 << 16, 0x7FCC_BBAA, 0xA5A5_FF03]);
    }
}

#[test]
fn empty_texture_export_pops_one_argument_and_preserves_the_delegate_result() {
    let mut world = EmptyTextures::new();
    expect(world.lot.ptr(), "get_zone", vec![], 0);
    expect(world.dev.ptr(), "clear_empty", vec![p(world.lot.ptr())], RETURN);
    let returned = unsafe {
        call_thiscall_with_eax(
            fc4p_006cc970 as *const () as usize,
            world.dev.ptr(),
            &[world.lot.ptr() as u32],
            0x1234,
        )
    };
    assert_eq!(returned, (RETURN as u32, (RETURN >> 32) as u32));
    finished();
}
