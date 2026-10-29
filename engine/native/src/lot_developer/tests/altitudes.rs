// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::altitudes::*;
use crate::lot_developer::maintenance::OccupantNode;

slot!(alt_allocate, size);
slot!(alt_free, node);
slot!(alt_objects, list);
slot!(alt_record, occupant);
slot!(alt_set_position, position);
slot!(alt_refresh, occupant, moved);
slot!(alt_foundation, occupant);
slot!(alt_remove_foundation, occupant);
slot!(alt_needs_foundation, occupant);
slot!(alt_add_type, building, kind);
slot!(alt_add_config, building, config);
slot!(alt_query, iid, output);
slot!(alt_foundation_kind);

unsafe extern "thiscall" fn alt_terrain(this: *mut u8, x: f32, z: f32) -> f32 {
    f32::from_bits(run(this, "alt_terrain", &[x.to_bits(), z.to_bits()]) as u32)
}

unsafe extern "thiscall" fn alt_clear(list: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "alt_clear",
        &[unsafe { list.read_unaligned() } as u32],
    )
}

struct World {
    developer: Mock,
    lot: Mock,
    allocator: Mock,
    head: Object,
    nodes: Vec<Object>,
    _allocator_pointer: Box<*mut u8>,
    _allocator_data: crate::testing::Fake,
    _clear: crate::testing::Fake,
}

impl World {
    fn new(occupants: &mut [Mock]) -> Self {
        let mut developer = mock!(0x9C => get_position, 0xC0 => alt_foundation, 0xF0 => alt_remove_foundation,
            0xE0 => alt_needs_foundation, 0xE8 => alt_add_config, 0xEC => alt_add_type);
        let lot = mock!(0xC4 => alt_objects, 0x24 => get_config, 0x2C => alt_record);
        let mut allocator = mock!(0x0C => alt_allocate, 0x10 => alt_free);
        let mut pointer = Box::new(allocator.ptr());
        let data = fake_data(0x00B2_07E4, (&mut *pointer as *mut *mut u8).cast());
        let clear = fake(0x0041_3E60, alt_clear as *const () as usize);
        let mut head = Object::new(12);
        let mut nodes = occupants.iter().map(|_| Object::new(12)).collect::<Vec<_>>();
        let addresses = nodes.iter_mut().map(Object::ptr).collect::<Vec<_>>();
        for (i, occupant) in occupants.iter_mut().enumerate() {
            let node = unsafe { OccupantNode::from_ptr(addresses[i]) };
            node.set_next(addresses.get(i + 1).copied().unwrap_or(head.ptr()));
            node.set_previous(if i == 0 { head.ptr() } else { addresses[i - 1] });
            node.set_occupant(occupant.ptr());
        }
        unsafe {
            LotDeveloper::from_ptr(developer.ptr()).set_occupant_manager(core::ptr::null_mut());
        }
        Self {
            developer,
            lot,
            allocator,
            head,
            nodes,
            _allocator_pointer: pointer,
            _allocator_data: data,
            _clear: clear,
        }
    }

    fn begin(&mut self) {
        expect(
            self.allocator.ptr(),
            "alt_allocate",
            vec![w(12)],
            self.head.ptr() as u64,
        );
        expect(
            self.lot.ptr(),
            "alt_objects",
            vec![Arg::In(vec![self.head.ptr() as u32])],
            0,
        );
        if !self.nodes.is_empty() {
            edit_last(self.head.ptr().cast(), self.nodes[0].ptr() as u32);
            edit_last(
                unsafe { self.head.ptr().add(4).cast() },
                self.nodes.last_mut().unwrap().ptr() as u32,
            );
        }
    }

    fn end_and_call(&mut self) {
        expect(core::ptr::null_mut(), "alt_clear", vec![p(self.head.ptr())], RETURN);
        expect(self.allocator.ptr(), "alt_free", vec![p(self.head.ptr())], RETURN);
        let returned = unsafe {
            call_thiscall_with_eax(
                fc4p_006c91f0 as *const () as usize,
                self.developer.ptr(),
                &[self.lot.ptr() as u32],
                0xABCD_1234,
            )
        };
        assert_eq!(returned, (RETURN as u32 & !255, (RETURN >> 32) as u32));
        finished();
    }
}

fn occupant() -> Mock {
    mock!(0x1C => get_type, 0x20 => get_world_position, 0x24 => alt_set_position, 0 => alt_query, 8 => release)
}

#[test]
fn altitude_updates_move_only_y_and_visit_building_prop_flora_network_and_other_in_order() {
    let _fpu = GameFpu::new();
    let mut tolerance = 0.1f32;
    let _tolerance = fake_data(0x00A8_C950, (&mut tolerance as *mut f32).cast());
    let mut occupants = (0..6).map(|_| occupant()).collect::<Vec<_>>();
    let mut world = World::new(&mut occupants);
    let mut terrain = mock!(0x34 => alt_terrain);
    let mut manager = mock!(0x60 => alt_refresh);
    let mut replacement = mock!(0x60 => alt_refresh);
    let mut network = mock!(8 => release);
    let mut config = Object::new(4);
    let mut record = Object::new(52);
    unsafe {
        let developer = LotDeveloper::from_ptr(world.developer.ptr());
        developer.set_terrain(terrain.ptr());
        developer.set_occupant_manager(manager.ptr());
    }
    world.begin();
    let kinds = [
        0x2781_28A0,
        0xA823_821E,
        0x7475_8926,
        0xA823_821E,
        0xC772_BF98,
        0x1234_5678,
    ];
    let positions = [
        [3.0f32, 105.0, 4.0],
        [5.0, 100.05, 6.0],
        [7.0, 99.8, 8.0],
        [1.0, 100.0, 2.0],
    ];
    for (i, occupant) in occupants.iter_mut().enumerate() {
        expect(occupant.ptr(), "get_type", vec![], kinds[i]);
        if i < 4 {
            expect(
                occupant.ptr(),
                "get_world_position",
                vec![out(&positions[i].map(f32::to_bits))],
                RETURN,
            );
            expect(world.lot.ptr(), "get_config", vec![], config.ptr() as u64);
            expect(
                world.lot.ptr(),
                "alt_record",
                vec![p(occupant.ptr())],
                if i == 3 { record.ptr() as u64 } else { 0 },
            );
            if i == 3 {
                expect(
                    world.developer.ptr(),
                    "get_position",
                    vec![
                        p(world.lot.ptr()),
                        p(config.ptr()),
                        p(record.ptr()),
                        out(&[(-1.0f32).to_bits(), 102.0f32.to_bits(), (-2.0f32).to_bits()]),
                        out(&[3]),
                    ],
                    RETURN,
                );
            } else {
                expect(
                    terrain.ptr(),
                    "alt_terrain",
                    vec![w(positions[i][0].to_bits()), w(positions[i][2].to_bits())],
                    u64::from(100.0f32.to_bits()),
                );
            }
            if i != 1 {
                let mut moved = positions[i];
                moved[1] = if i == 3 { 102.0 } else { 100.0 };
                expect(
                    occupant.ptr(),
                    "alt_set_position",
                    vec![Arg::In(moved.map(f32::to_bits).to_vec())],
                    RETURN,
                );
                if i == 0 {
                    edit_last(
                        unsafe { world.developer.ptr().add(0x28).cast() },
                        replacement.ptr() as u32,
                    );
                }
                expect(replacement.ptr(), "alt_refresh", vec![p(occupant.ptr()), w(1)], RETURN);
                if i == 0 {
                    expect(world.developer.ptr(), "alt_foundation", vec![p(occupant.ptr())], 0);
                    expect(
                        world.developer.ptr(),
                        "alt_needs_foundation",
                        vec![p(occupant.ptr())],
                        0x100,
                    );
                }
            }
        } else if i == 4 {
            expect(
                occupant.ptr(),
                "alt_query",
                vec![w(0xA821_EF94), out(&[network.ptr() as u32])],
                0x100,
            );
            expect(network.ptr(), "release", vec![], RETURN);
        }
        expect(occupant.ptr(), "release", vec![], RETURN);
    }
    world.end_and_call();
}

#[test]
fn altitude_difference_is_strict_and_nan_and_infinite_equal_heights_do_not_move() {
    let _fpu = GameFpu::new();
    let mut tolerance = 0.1f32;
    let _tolerance = fake_data(0x00A8_C950, (&mut tolerance as *mut f32).cast());
    for (old, target, moved) in [
        (0.1f32, 0.0f32, false),
        (-0.1, 0.0, false),
        (0.100_000_01, 0.0, true),
        (100.05, 100.0, false),
        (f32::NAN, 1.0, false),
        (1.0, f32::NAN, false),
        (f32::INFINITY, f32::INFINITY, false),
        (f32::INFINITY, 0.0, true),
        (-0.0, 0.0, false),
    ] {
        let mut occupants = [occupant()];
        let mut world = World::new(&mut occupants);
        let mut manager = mock!(0x60 => alt_refresh);
        unsafe {
            LotDeveloper::from_ptr(world.developer.ptr()).set_occupant_manager(manager.ptr());
        }
        world.begin();
        expect(occupants[0].ptr(), "get_type", vec![], 0x7475_8926);
        expect(
            occupants[0].ptr(),
            "get_world_position",
            vec![out(&[1.0f32.to_bits(), old.to_bits(), 2.0f32.to_bits()])],
            RETURN,
        );
        expect(world.lot.ptr(), "get_config", vec![], 0);
        expect(world.lot.ptr(), "alt_record", vec![p(occupants[0].ptr())], 0x1234);
        expect(
            world.developer.ptr(),
            "get_position",
            vec![
                p(world.lot.ptr()),
                w(0),
                w(0x1234),
                out(&[99.0f32.to_bits(), target.to_bits(), 98.0f32.to_bits()]),
                out(&[0]),
            ],
            RETURN,
        );
        if moved {
            expect(
                occupants[0].ptr(),
                "alt_set_position",
                vec![Arg::In(vec![1.0f32.to_bits(), target.to_bits(), 2.0f32.to_bits()])],
                RETURN,
            );
            expect(manager.ptr(), "alt_refresh", vec![p(occupants[0].ptr()), w(1)], RETURN);
        }
        expect(occupants[0].ptr(), "release", vec![], RETURN);
        world.end_and_call();
    }
}

#[test]
fn moved_building_foundations_use_saved_type_or_configuration_and_release_failed_queries() {
    let _fpu = GameFpu::new();
    let mut tolerance = 0.1f32;
    let _tolerance = fake_data(0x00A8_C950, (&mut tolerance as *mut f32).cast());
    for kind in [None, Some(0u32), Some(7)] {
        for (query_result, has_building) in [(0x100u32, false), (0x100, true), (0x101, true)] {
            let mut occupants = [occupant()];
            let mut world = World::new(&mut occupants);
            let mut manager = mock!(0x60 => alt_refresh);
            let mut foundation = mock!(0x10 => alt_foundation_kind);
            let mut building = mock!(8 => release);
            let building_pointer = if has_building {
                building.ptr()
            } else {
                core::ptr::null_mut()
            };
            unsafe {
                LotDeveloper::from_ptr(world.developer.ptr()).set_occupant_manager(manager.ptr());
            }
            world.begin();
            expect(occupants[0].ptr(), "get_type", vec![], 0x2781_28A0);
            expect(
                occupants[0].ptr(),
                "get_world_position",
                vec![out(&[0, 10.0f32.to_bits(), 0])],
                RETURN,
            );
            expect(world.lot.ptr(), "get_config", vec![], 0xABCD);
            expect(world.lot.ptr(), "alt_record", vec![p(occupants[0].ptr())], 0x1234);
            expect(
                world.developer.ptr(),
                "get_position",
                vec![p(world.lot.ptr()), w(0xABCD), w(0x1234), out(&[0, 0, 0]), out(&[0])],
                RETURN,
            );
            expect(
                occupants[0].ptr(),
                "alt_set_position",
                vec![Arg::In(vec![0, 0, 0])],
                RETURN,
            );
            expect(manager.ptr(), "alt_refresh", vec![p(occupants[0].ptr()), w(1)], RETURN);
            expect(
                world.developer.ptr(),
                "alt_foundation",
                vec![p(occupants[0].ptr())],
                if kind.is_some() { foundation.ptr() as u64 } else { 0 },
            );
            if let Some(kind) = kind {
                expect(foundation.ptr(), "alt_foundation_kind", vec![], u64::from(kind));
                expect(
                    world.developer.ptr(),
                    "alt_remove_foundation",
                    vec![p(occupants[0].ptr())],
                    RETURN,
                );
            }
            expect(
                world.developer.ptr(),
                "alt_needs_foundation",
                vec![p(occupants[0].ptr())],
                0x101,
            );
            expect(
                occupants[0].ptr(),
                "alt_query",
                vec![w(0x87DD_A3A9), out(&[building_pointer as u32])],
                u64::from(query_result),
            );
            if query_result as u8 != 0 {
                if let Some(kind @ 1..) = kind {
                    expect(
                        world.developer.ptr(),
                        "alt_add_type",
                        vec![p(building.ptr()), w(kind)],
                        RETURN,
                    );
                } else {
                    expect(
                        world.developer.ptr(),
                        "alt_add_config",
                        vec![p(building.ptr()), w(0xABCD)],
                        RETURN,
                    );
                }
            }
            if has_building {
                expect(building.ptr(), "release", vec![], RETURN);
            }
            expect(occupants[0].ptr(), "release", vec![], RETURN);
            world.end_and_call();
        }
    }
    let mut world = World::new(&mut []);
    world.begin();
    world.end_and_call();
}

#[test]
fn network_altitude_query_can_return_no_interface_without_moving_the_occupant() {
    let mut occupants = [occupant()];
    let mut world = World::new(&mut occupants);
    world.begin();
    expect(occupants[0].ptr(), "get_type", vec![], 0xC772_BF98);
    expect(occupants[0].ptr(), "alt_query", vec![w(0xA821_EF94), out(&[0])], 0x100);
    expect(occupants[0].ptr(), "release", vec![], RETURN);
    world.end_and_call();
}
