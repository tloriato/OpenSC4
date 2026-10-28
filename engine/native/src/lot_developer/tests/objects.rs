// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::objects::*;

slot!(is_initialized);
slot!(delete_lot, lot);
slot!(bounding_rect, rect);
slot!(allocate_list, bytes);
slot!(free_list, pointer);
slot!(query_cells, list, min, max, kind, limit);
slot!(query_box, list, min, max, kind, limit);
slot!(query_interface, iid, output);
slot!(foundation_kind);
slot!(release_query, list);
slot!(query_props, list, lot);
slot!(remove_prop, prop);
slot!(get_prop_holder);
slot!(get_uint);
slot!(demolish, mode, occupant, a, b, c, d, e);
slot!(query_service, class, iid, output);
slot!(lot_building);
slot!(add_property, id, value, flags);
slot!(position_for_building, lot, position, facing);
slot!(occupant_bounds, min, max);
slot!(lot_center, position);
slot!(create_zot_prop, kind, position, facing);
slot!(removal_flags, flags);

unsafe extern "thiscall" fn clear_node_list(list: *mut *mut u8) -> u64 {
    unsafe {
        let head = list.read();
        let returned = run(core::ptr::null_mut(), "clear_node_list", &[head as u32]);
        head.cast::<*mut u8>().write(head);
        head.add(4).cast::<*mut u8>().write(head);
        returned
    }
}

unsafe extern "thiscall" fn destroy_node_list(list: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "destroy_node_list",
        &[unsafe { list.read() } as u32],
    )
}

unsafe extern "thiscall" fn clear_lot_direct(this: *mut u8, lot: u32) -> u64 {
    run(this, "clear_lot_direct", &[lot])
}

unsafe extern "thiscall" fn remove_prop_direct(this: *mut u8, lot: u32, kind: u32) -> u64 {
    run(this, "remove_prop_direct", &[lot, kind])
}

unsafe extern "C" fn post_zot_message(id: u32, a: u32, b: u32, c: u32, d: u32, lot: u32) -> u64 {
    run(core::ptr::null_mut(), "post_zot_message", &[id, a, b, c, d, lot])
}

unsafe extern "C" fn get_framework() -> u64 {
    run(core::ptr::null_mut(), "get_framework", &[])
}

unsafe extern "thiscall" fn release_service(this: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "release_service",
        &[unsafe { this.read() } as u32],
    )
}

struct List {
    head: Box<[u32; 3]>,
    nodes: Vec<[u32; 3]>,
}

impl List {
    fn new(occupants: &[*mut u8]) -> Self {
        let mut list = Self {
            head: Box::new([0xA5A5_A5A5; 3]),
            nodes: occupants.iter().map(|&p| [0, 0, p as u32]).collect(),
        };
        let head = list.ptr() as u32;
        let pointers: Vec<_> = list.nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        for (i, node) in list.nodes.iter_mut().enumerate() {
            node[0] = pointers.get(i + 1).copied().unwrap_or(head);
            node[1] = if i == 0 { head } else { pointers[i - 1] };
        }
        list
    }

    fn ptr(&mut self) -> *mut u8 {
        self.head.as_mut_ptr().cast()
    }

    fn allocate(&mut self, alloc: *mut u8) {
        let head = self.ptr();
        expect(alloc, "allocate_list", vec![w(12)], head as u64);
    }

    fn fill(&mut self) {
        let head = self.ptr();
        let next = self.nodes.first_mut().map_or(head as u32, |n| n.as_mut_ptr() as u32);
        let previous = self.nodes.last_mut().map_or(head as u32, |n| n.as_mut_ptr() as u32);
        edit_last(head.cast(), next);
        edit_last(unsafe { head.add(4) }.cast(), previous);
    }

    fn cleanup(&mut self, alloc: *mut u8) {
        let head = self.ptr();
        expect(core::ptr::null_mut(), "clear_node_list", vec![p(head)], RETURN);
        expect(alloc, "free_list", vec![p(head)], RETURN);
    }

    fn check_padding(&self) {
        assert_eq!(self.head[2], 0xA5A5_A5A5);
    }
}

#[test]
fn demolition_checks_only_the_low_initialized_byte_and_deletes_the_four_zones() {
    let _clear = fake(0x006C_8A20, clear_lot_direct as *const () as usize);
    unsafe {
        for initialized in [0u32, 0x100, 0xABCD_0001] {
            for zone in [0, 1, 6, 11, 12, 15, 16, u32::MAX] {
                let mut dev = Object::new(0x108);
                let this = dev.ptr();
                let mut lot = mock!(0x18 => is_initialized, 0x88 => set_state, 0x7C => get_zone);
                let lot = lot.ptr();
                let mut manager = mock!(0x60 => delete_lot);
                let manager = manager.ptr();
                LotDeveloper::from_ptr(this).set_lot_manager(core::ptr::null_mut());
                let initial = 0x1234_5678_0000_0000 | u64::from(initialized);
                expect(lot, "is_initialized", vec![], initial);
                let expected = if initialized as u8 != 0 {
                    expect(lot, "set_state", vec![w(6), w(0), w(0)], RETURN);
                    expect(this, "clear_lot_direct", vec![p(lot)], RETURN);
                    edit_last(this.add(0x24).cast(), manager as u32);
                    let zoned = 0x7654_3210_0000_0000 | u64::from(zone);
                    expect(lot, "get_zone", vec![], zoned);
                    if matches!(zone, 0 | 11 | 12 | 15) {
                        expect(manager, "delete_lot", vec![p(lot)], RETURN);
                        RETURN
                    } else {
                        zoned
                    }
                } else {
                    initial
                };
                assert_eq!(fc4p_006c90a0(this, 0xCAFEBABE, lot), expected);
                finished();
            }
        }
    }
}

#[test]
fn first_building_releases_the_interface_then_the_query_and_keeps_failed_query_output() {
    let _destroy = fake(0x0041_3FE0, destroy_node_list as *const () as usize);
    unsafe {
        for query in [0x100, 0x101] {
            for populated in [false, true] {
                for interface_success in [0x100, 0x101] {
                    let mut dev = Object::new(0x108);
                    let this = dev.ptr();
                    let mut lot = mock!(0x38 => bounding_rect);
                    let lot = lot.ptr();
                    let mut manager = mock!(0xA4 => query_cells, 0xC0 => release_query);
                    let manager = manager.ptr();
                    let mut occupant = mock!(0 => query_interface);
                    let occupant = occupant.ptr();
                    let mut building = mock!(8 => release);
                    let building = building.ptr();
                    let mut alloc = mock!(0xC => allocate_list);
                    let mut allocator = alloc.ptr();
                    let _data = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
                    LotDeveloper::from_ptr(this).set_occupant_manager(manager);
                    let occupants = [occupant];
                    let mut list = List::new(if populated { &occupants } else { &[] });
                    let head = list.ptr();
                    let rect = [2, 3, 4, 5];
                    expect(lot, "bounding_rect", vec![out(&rect)], RETURN);
                    list.allocate(allocator);
                    expect(
                        manager,
                        "query_cells",
                        vec![
                            Arg::In(vec![head as u32]),
                            Arg::In(vec![2, 3]),
                            Arg::In(vec![4, 5]),
                            w(0x2781_28A0),
                            w(1),
                        ],
                        query,
                    );
                    list.fill();
                    let found = query as u8 != 0 && populated;
                    if found {
                        expect(
                            occupant,
                            "query_interface",
                            vec![w(0x87DD_A3A9), out(&[building as u32])],
                            interface_success,
                        );
                        if interface_success as u8 != 0 {
                            expect(building, "release", vec![], RETURN);
                        }
                        expect(manager, "release_query", vec![Arg::In(vec![head as u32])], RETURN);
                    }
                    expect(core::ptr::null_mut(), "destroy_node_list", vec![p(head)], RETURN);
                    let returned = fc4p_006c9100(this, 0x1357_9BDF, lot);
                    assert_eq!(
                        returned,
                        (RETURN & 0xFFFF_FFFF_0000_0000) | if found { u64::from(building as u32) } else { 0 }
                    );
                    list.check_padding();
                    finished();
                }
            }
        }
    }
}

#[test]
fn foundation_search_releases_all_members_and_stops_querying_after_the_first_match() {
    let _clear = fake(0x0041_3E60, clear_node_list as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        for position in [false, true] {
            for queried in [0x100, 0x101] {
                for match_at in [None, Some(0), Some(1), Some(3)] {
                    let mut dev = Object::new(0x108);
                    let this = dev.ptr();
                    let mut manager = mock!(0xA4 => query_cells, 0x94 => query_box, 4 => add_ref);
                    let manager = manager.ptr();
                    LotDeveloper::from_ptr(this).set_occupant_manager(manager);
                    let mut alloc = mock!(0xC => allocate_list, 0x10 => free_list);
                    let mut allocator = alloc.ptr();
                    let _data = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
                    let mut inset = 0.05f32;
                    let _inset = fake_data(0x00A8_80D8, (&mut inset as *mut f32).cast());
                    let mut members: Vec<_> = (0..4).map(|_| mock!(0 => query_interface, 8 => release)).collect();
                    let members: Vec<_> = members.iter_mut().map(Mock::ptr).collect();
                    let mut interfaces: Vec<_> = (0..4).map(|_| mock!(0x24 => foundation_kind, 8 => release)).collect();
                    let interfaces: Vec<_> = interfaces.iter_mut().map(Mock::ptr).collect();
                    let mut list = List::new(&members);
                    let head = list.ptr();
                    list.allocate(allocator);
                    if position {
                        let x = terrain::subtract_float(F80::from_f32(7.0), inset).to_f32().to_bits();
                        let z = terrain::add_float(F80::from_f32(-3.0), inset).to_f32().to_bits();
                        expect(
                            manager,
                            "query_box",
                            vec![
                                Arg::In(vec![head as u32]),
                                Arg::In(vec![x, (-f32::MAX).to_bits(), z]),
                                Arg::In(vec![x, f32::MAX.to_bits(), z]),
                                w(0x68FD_0C69),
                                w(u32::MAX),
                            ],
                            queried,
                        );
                    } else {
                        expect(
                            manager,
                            "query_cells",
                            vec![
                                Arg::In(vec![head as u32]),
                                Arg::In(vec![7, (-3i32) as u32]),
                                Arg::In(vec![7, (-3i32) as u32]),
                                w(0x68FD_0C69),
                                w(u32::MAX),
                            ],
                            queried,
                        );
                    }
                    list.fill();
                    let mut expected = 0;
                    if queried as u8 != 0 {
                        for i in 0..4 {
                            if expected == 0 {
                                let success = i != 2;
                                expect(
                                    members[i],
                                    "query_interface",
                                    vec![w(0xC8FC_E101), out(&[interfaces[i] as u32])],
                                    if success { 0x101 } else { 0x100 },
                                );
                                if success {
                                    expect(
                                        interfaces[i],
                                        "foundation_kind",
                                        vec![],
                                        if match_at == Some(i) { 2 } else { 1 },
                                    );
                                    expect(interfaces[i], "release", vec![], RETURN);
                                    if match_at == Some(i) {
                                        expected = interfaces[i] as u32;
                                    }
                                }
                            }
                            expect(members[i], "release", vec![], RETURN);
                        }
                    }
                    list.cleanup(allocator);
                    let returned = if position {
                        fc4p_006c95e0(this, 0, 2, 7.0, -3.0)
                    } else {
                        fc4p_006c9480(this, 0, 2, 7, (-3i32) as u32)
                    };
                    assert_eq!(returned, (RETURN & 0xFFFF_FFFF_0000_0000) | u64::from(expected));
                    list.check_padding();
                    finished();
                }
            }
        }
    }
}

#[test]
fn position_search_caches_the_global_manager_and_rereads_it_after_addref() {
    let _clear = fake(0x0041_3E60, clear_node_list as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        let mut dev = Object::new(0x108);
        let this = dev.ptr();
        let mut manager = mock!(4 => add_ref);
        let mut manager = manager.ptr();
        let _manager = fake_data(0x00B4_3D0C, (&mut manager as *mut *mut u8).cast());
        let mut replacement = mock!(0x94 => query_box);
        let replacement = replacement.ptr();
        let mut alloc = mock!(0xC => allocate_list, 0x10 => free_list);
        let mut allocator = alloc.ptr();
        let _data = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
        let mut inset = 0.05f32;
        let _inset = fake_data(0x00A8_80D8, (&mut inset as *mut f32).cast());
        let mut list = List::new(&[]);
        list.allocate(allocator);
        expect(manager, "add_ref", vec![], RETURN);
        edit_last(this.add(0x28).cast(), replacement as u32);
        expect(
            replacement,
            "query_box",
            vec![
                Arg::In(vec![list.ptr() as u32]),
                Arg::In(vec![(-0.05f32).to_bits(), (-f32::MAX).to_bits(), 0.05f32.to_bits()]),
                Arg::In(vec![(-0.05f32).to_bits(), f32::MAX.to_bits(), 0.05f32.to_bits()]),
                w(0x68FD_0C69),
                w(u32::MAX),
            ],
            1,
        );
        list.cleanup(allocator);
        assert_eq!(fc4p_006c95e0(this, 0, 2, 0.0, 0.0), RETURN & 0xFFFF_FFFF_0000_0000);
        assert_eq!(LotDeveloper::from_ptr(this).occupant_manager(), replacement);
        finished();
    }
}

#[test]
fn allocator_initialization_publishes_before_releasing_and_rereads_after_list_clear() {
    let _framework = fake(0x0087_93EC, get_framework as *const () as usize);
    let _release = fake(0x0041_1870, release_service as *const () as usize);
    let _clear = fake(0x0041_3E60, clear_node_list as *const () as usize);
    unsafe {
        let mut dev = Object::new(0x108);
        let this = dev.ptr();
        let mut manager = mock!(0xA4 => query_cells);
        let manager = manager.ptr();
        LotDeveloper::from_ptr(this).set_occupant_manager(manager);
        let mut root = mock!(0x14 => query_service);
        let root = root.ptr();
        let mut alloc = mock!(0xC => allocate_list);
        let allocation_service = alloc.ptr();
        let mut other = mock!(0x10 => free_list);
        let other = other.ptr();
        let mut allocator = core::ptr::null_mut::<u8>();
        let global = (&mut allocator as *mut *mut u8).cast::<u32>();
        let _data = fake_data(0x00B2_07E4, global.cast());
        let mut list = List::new(&[]);
        expect(core::ptr::null_mut(), "get_framework", vec![], root as u64);
        expect(
            root,
            "query_service",
            vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[allocation_service as u32])],
            RETURN,
        );
        expect(
            core::ptr::null_mut(),
            "release_service",
            vec![p(allocation_service)],
            RETURN,
        );
        list.allocate(allocation_service);
        expect(
            manager,
            "query_cells",
            vec![
                Arg::In(vec![list.ptr() as u32]),
                Arg::In(vec![0, 0]),
                Arg::In(vec![0, 0]),
                w(0x68FD_0C69),
                w(u32::MAX),
            ],
            0,
        );
        expect(core::ptr::null_mut(), "clear_node_list", vec![p(list.ptr())], RETURN);
        edit_last(global, 0);
        expect(core::ptr::null_mut(), "get_framework", vec![], root as u64);
        expect(
            root,
            "query_service",
            vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[allocation_service as u32])],
            RETURN,
        );
        expect(
            core::ptr::null_mut(),
            "release_service",
            vec![p(allocation_service)],
            RETURN,
        );
        edit_last(global, other as u32);
        expect(other, "free_list", vec![p(list.ptr())], RETURN);
        assert_eq!(fc4p_006c9480(this, 0, 2, 0, 0), RETURN & 0xFFFF_FFFF_0000_0000);
        assert_eq!(allocator, other);
        finished();
    }
}

#[test]
fn prop_removal_ignores_nulls_removes_all_matching_numbers_and_ignores_remove_failure() {
    let _clear = fake(0x0041_3E60, clear_node_list as *const () as usize);
    unsafe {
        for matching in [false, true] {
            let mut dev = Object::new(0x108);
            let this = dev.ptr();
            let mut manager = mock!(0x60 => query_props, 0x74 => remove_prop);
            let manager = manager.ptr();
            LotDeveloper::from_ptr(this).set_prop_manager(manager);
            let mut alloc = mock!(0xC => allocate_list, 0x10 => free_list);
            let mut allocator = alloc.ptr();
            let _data = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
            let mut lot = Object::new(4);
            let lot = lot.ptr();
            let mut props: Vec<_> = (0..4).map(|_| mock!(0x18 => get_prop_holder, 8 => release)).collect();
            let props: Vec<_> = props.iter_mut().map(Mock::ptr).collect();
            let mut holder = mock!(0x24 => get_property);
            let holder = holder.ptr();
            let mut property = mock!(0x14 => get_variant);
            let property = property.ptr();
            let mut variant = mock!(0x120 => get_uint);
            let variant = variant.ptr();
            let mut occupants = vec![core::ptr::null_mut()];
            occupants.extend(&props);
            let mut list = List::new(&occupants);
            list.allocate(allocator);
            expect(
                manager,
                "query_props",
                vec![Arg::In(vec![list.ptr() as u32]), p(lot)],
                0,
            );
            list.fill();
            for (i, &prop) in props.iter().enumerate() {
                expect(prop, "get_prop_holder", vec![], holder as u64);
                expect(
                    holder,
                    "get_property",
                    vec![w(0x48E9_5539)],
                    if i == 0 { 0 } else { property as u64 },
                );
                if i != 0 {
                    expect(property, "get_variant", vec![], variant as u64);
                    expect(variant, "get_uint", vec![], if matching && i >= 2 { 6 } else { 5 });
                    if matching && i >= 2 {
                        expect(manager, "remove_prop", vec![p(prop)], 0);
                    }
                }
                expect(prop, "release", vec![], RETURN);
            }
            list.cleanup(allocator);
            assert_eq!(fc4p_006c9780(this, 0, lot, 6), (RETURN & !255) | u64::from(matching));
            list.check_padding();
            finished();
        }
    }
}

#[test]
fn zot_removal_requires_the_property_and_posts_road_message_before_testing_the_current_number() {
    let _remove = fake(0x006C_9780, remove_prop_direct as *const () as usize);
    let _post = fake(0x005F_98C0, post_zot_message as *const () as usize);
    unsafe {
        for present in [false, true] {
            for current in [0, 4, 6, u32::MAX] {
                for requested in [0, 4, 6, u32::MAX] {
                    let mut dev = Object::new(0x108);
                    let this = dev.ptr();
                    let mut lot = mock!(0xC => get_holder);
                    let lot = lot.ptr();
                    let mut holder = mock!(0x24 => get_property);
                    let holder = holder.ptr();
                    let mut property = mock!(0x14 => get_variant);
                    let property = property.ptr();
                    let mut variant = mock!(0x124 => get_data, 0x68 => set_uint);
                    let variant = variant.ptr();
                    let mut number = current;
                    expect(lot, "get_holder", vec![], holder as u64);
                    expect(
                        holder,
                        "get_property",
                        vec![w(0xC999_C45E)],
                        if present {
                            property as u64
                        } else {
                            0x1234_5678_0000_0000
                        },
                    );
                    let mut expected = 0x1234_5678_0000_0000;
                    if present {
                        expect(property, "get_variant", vec![], variant as u64);
                        expected = 0x1234_0000_0000_0000 | u64::from(&mut number as *mut u32 as u32);
                        expect(variant, "get_data", vec![], expected);
                        if requested == 6 {
                            expect(
                                core::ptr::null_mut(),
                                "post_zot_message",
                                vec![w(0x4C04_DE69), w(2), w(0), w(0), w(0), p(lot)],
                                RETURN,
                            );
                            expected = RETURN;
                        }
                        if requested == 0 || requested == current {
                            if current != 0 {
                                expect(this, "remove_prop_direct", vec![p(lot), w(current)], RETURN);
                            }
                            expect(variant, "set_uint", vec![w(0)], RETURN);
                            expected = RETURN;
                        }
                    }
                    assert_eq!(fc4p_006c9a40(this, 0, lot, requested), expected);
                    finished();
                }
            }
        }
    }
}

#[test]
fn blocking_query_uses_wrapping_inset_bounds_and_demolishes_every_result_even_on_query_failure() {
    let _clear = fake(0x0041_3E60, clear_node_list as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        for rect in [[2i32, -3, 4, 5], [i32::MAX, i32::MIN, i32::MAX, i32::MIN]] {
            for populated in [false, true] {
                let mut dev = mock!(0x44 => lowest_area_float);
                let this = dev.ptr();
                let mut lot = mock!(0x38 => bounding_rect);
                let lot = lot.ptr();
                let mut manager = mock!(0x90 => query_box);
                let manager = manager.ptr();
                let mut demolition = mock!(0x20 => demolish);
                let demolition = demolition.ptr();
                LotDeveloper::from_ptr(this).set_occupant_manager(manager);
                LotDeveloper::from_ptr(this).set_demolition(demolition);
                this.add(0x58).cast::<u32>().write_unaligned(0xBAD1_0000);
                let mut alloc = mock!(0xC => allocate_list, 0x10 => free_list);
                let mut allocator = alloc.ptr();
                let _data = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
                let mut half = 0.5f32;
                let mut margin = 100.0f32;
                let _half = fake_data(0x00A8_4D2C, (&mut half as *mut f32).cast());
                let _margin = fake_data(0x00A9_5690, (&mut margin as *mut f32).cast());
                let mut occupants: Vec<_> = (0..2).map(|_| mock!(8 => release)).collect();
                let occupants: Vec<_> = occupants.iter_mut().map(Mock::ptr).collect();
                let mut list = List::new(if populated { &occupants } else { &[] });
                expect(lot, "bounding_rect", vec![out(&rect.map(|v| v as u32))], RETURN);
                expect(
                    this,
                    "lowest_area_body",
                    vec![Arg::In(rect.map(|v| v as u32).to_vec()), w(0), w(1)],
                    RETURN,
                );
                FLOATS.with(|v| v.borrow_mut().push_back(F80::from_f32(200.0)));
                list.allocate(allocator);
                let min = [
                    terrain::add_float(F80::from_i32(rect[0].wrapping_shl(4)), half)
                        .to_f32()
                        .to_bits(),
                    0,
                    terrain::add_float(F80::from_i32(rect[1].wrapping_shl(4)), half)
                        .to_f32()
                        .to_bits(),
                ];
                let max = [
                    terrain::subtract_float(F80::from_i32(rect[2].wrapping_add(1).wrapping_shl(4)), half)
                        .to_f32()
                        .to_bits(),
                    300.0f32.to_bits(),
                    terrain::subtract_float(F80::from_i32(rect[3].wrapping_add(1).wrapping_shl(4)), half)
                        .to_f32()
                        .to_bits(),
                ];
                expect(
                    manager,
                    "query_box",
                    vec![
                        Arg::In(vec![list.ptr() as u32]),
                        Arg::In(min.to_vec()),
                        Arg::In(max.to_vec()),
                        w(0xBAD1_0000),
                        w(u32::MAX),
                    ],
                    0,
                );
                list.fill();
                if populated {
                    for occupant in occupants {
                        expect(
                            demolition,
                            "demolish",
                            vec![w(1), p(occupant), w(0), w(1), w(0), w(0), w(0)],
                            RETURN,
                        );
                        expect(occupant, "release", vec![], RETURN);
                    }
                }
                list.cleanup(allocator);
                assert_eq!(fc4p_006c9ad0(this, 0, lot, 0x1234, 0x5678, 0x9ABC), RETURN);
                finished();
            }
        }
    }
}

unsafe extern "fastcall" fn free_preserving_edx(this: *mut u8, edx: u32, pointer: u32) -> u64 {
    let returned = run(this, "free_preserving_edx", &[pointer]);
    (u64::from(edx) << 32) | u64::from(returned as u32)
}

#[test]
fn empty_queries_preserve_cleanup_edx_and_export_the_original_stack_conventions() {
    let _clear = fake(0x0041_3E60, clear_node_list as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        for address in [0x006C_9480, 0x006C_95E0, 0x006C_9780] {
            for queried in [0x100, 0x101] {
                let mut dev = Object::new(0x108);
                let this = dev.ptr();
                let mut manager = mock!(0xA4 => query_cells, 0x94 => query_box, 0x60 => query_props);
                let manager = manager.ptr();
                LotDeveloper::from_ptr(this).set_occupant_manager(manager);
                LotDeveloper::from_ptr(this).set_prop_manager(manager);
                let mut alloc = mock!(0xC => allocate_list, 0x10 => free_preserving_edx);
                let mut allocator = alloc.ptr();
                let _data = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
                let mut inset = 0.05f32;
                let _inset = fake_data(0x00A8_80D8, (&mut inset as *mut f32).cast());
                let mut list = List::new(&[]);
                list.allocate(allocator);
                let args = match address {
                    0x006C_9480 => {
                        expect(
                            manager,
                            "query_cells",
                            vec![
                                Arg::In(vec![list.ptr() as u32]),
                                Arg::In(vec![0, 0]),
                                Arg::In(vec![0, 0]),
                                w(0x68FD_0C69),
                                w(u32::MAX),
                            ],
                            queried,
                        );
                        vec![2, 0, 0]
                    }
                    0x006C_95E0 => {
                        expect(
                            manager,
                            "query_box",
                            vec![
                                Arg::In(vec![list.ptr() as u32]),
                                Arg::In(vec![(-0.05f32).to_bits(), (-f32::MAX).to_bits(), 0.05f32.to_bits()]),
                                Arg::In(vec![(-0.05f32).to_bits(), f32::MAX.to_bits(), 0.05f32.to_bits()]),
                                w(0x68FD_0C69),
                                w(u32::MAX),
                            ],
                            queried,
                        );
                        vec![2, 0, 0]
                    }
                    _ => {
                        expect(
                            manager,
                            "query_props",
                            vec![Arg::In(vec![list.ptr() as u32]), w(0xCAFEBABE)],
                            queried,
                        );
                        vec![0xCAFEBABE, 6]
                    }
                };
                expect(core::ptr::null_mut(), "clear_node_list", vec![p(list.ptr())], RETURN);
                expect(allocator, "free_preserving_edx", vec![p(list.ptr())], 0xABCD_1234);
                let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, &args);
                call.edx = 0xCAFE_F00D;
                call_raw(&mut call);
                assert_eq!(call.edx, (RETURN >> 32) as u32);
                assert_eq!(call.eax, if address == 0x006C_9780 { 0xABCD_1200 } else { 0 });
                list.check_padding();
                finished();
            }
        }
    }
}

#[test]
fn zot_addition_keeps_unsigned_priority_posts_road_notifications_and_selects_roof_or_center() {
    let _remove = fake(0x006C_9780, remove_prop_direct as *const () as usize);
    let _post = fake(0x005F_98C0, post_zot_message as *const () as usize);
    let _fpu = GameFpu::new();
    unsafe {
        for current in [0u32, 4, 7, 0x8000_0000] {
            for requested in [4, 5, 6, 7, u32::MAX] {
                for inputs in 0..8 {
                    let mut dev = mock!(0xA0 => position_for_building);
                    let this = dev.ptr();
                    let mut lot =
                        mock!(0xC0 => lot_building, 0x24 => get_config, 0xC => get_holder, 0x3C => lot_center);
                    let lot = lot.ptr();
                    let mut config = mock!(0x60 => get_primary);
                    let config = config.ptr();
                    let mut holder = mock!(0x24 => get_property, 0x34 => add_property);
                    let holder = holder.ptr();
                    let mut property = mock!(0x14 => get_variant);
                    let property = property.ptr();
                    let mut variant = mock!(0x124 => get_data, 0x68 => set_uint);
                    let variant = variant.ptr();
                    let mut manager = mock!(0x68 => create_zot_prop);
                    let manager = manager.ptr();
                    LotDeveloper::from_ptr(this).set_prop_manager(manager);
                    let mut building = mock!(0xC => get_occupant);
                    let building = building.ptr();
                    let mut occupant = mock!(0x28 => occupant_bounds);
                    let occupant = occupant.ptr();
                    let mut prop = mock!(0x30 => removal_flags, 0x18 => get_prop_holder);
                    let prop = prop.ptr();
                    let mut prop_holder = mock!(0x34 => add_property);
                    let prop_holder = prop_holder.ptr();
                    let mut view = Object::new(0x20);
                    let mut view = view.ptr();
                    view.add(0x10).cast::<u32>().write(0xABCD_0123);
                    let _view = fake_data(0x00B4_3DD8, (&mut view as *mut *mut u8).cast());
                    let mut height = 3.0f32;
                    let _height = fake_data(0x00A8_5074, (&mut height as *mut f32).cast());
                    let mut number = current;
                    expect(
                        lot,
                        "lot_building",
                        vec![],
                        if inputs & 1 != 0 { building as u64 } else { 0 },
                    );
                    expect(
                        lot,
                        "get_config",
                        vec![],
                        if inputs & 2 != 0 { config as u64 } else { 0 },
                    );
                    if inputs & 2 != 0 {
                        expect(
                            config,
                            "get_primary",
                            vec![],
                            if inputs & 4 != 0 { 0xFEED0000 } else { 0 },
                        );
                    }
                    if requested == 6 {
                        expect(
                            core::ptr::null_mut(),
                            "post_zot_message",
                            vec![w(0x4C04_DE69), w(1), w(0), w(0), w(0), p(lot)],
                            RETURN,
                        );
                    }
                    expect(lot, "get_holder", vec![], holder as u64);
                    let missing = inputs & 1 == 0;
                    expect(
                        holder,
                        "get_property",
                        vec![w(0xC999_C45E)],
                        if missing { 0 } else { property as u64 },
                    );
                    if missing {
                        expect(holder, "add_property", vec![w(0xC999_C45E), w(0), w(0)], RETURN);
                        expect(holder, "get_property", vec![w(0xC999_C45E)], property as u64);
                    }
                    expect(property, "get_variant", vec![], variant as u64);
                    expect(
                        variant,
                        "get_data",
                        vec![],
                        0x4321_5678_0000_0000 | u64::from(&mut number as *mut u32 as u32),
                    );
                    let rejected = current != 0 && requested >= current;
                    if !rejected {
                        if current != 0 {
                            expect(this, "remove_prop_direct", vec![p(lot), w(current)], RETURN);
                        }
                        expect(variant, "set_uint", vec![w(requested)], RETURN);
                        let expected_position = if inputs == 7 {
                            expect(
                                this,
                                "position_for_building",
                                vec![
                                    p(lot),
                                    out(&[2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits()]),
                                    out(&[0x1234_5678]),
                                ],
                                RETURN,
                            );
                            expect(lot, "lot_building", vec![], building as u64);
                            expect(building, "get_occupant", vec![], occupant as u64);
                            expect(
                                occupant,
                                "occupant_bounds",
                                vec![
                                    out(&[0; 3]),
                                    out(&[8.0f32.to_bits(), 90.0f32.to_bits(), 12.0f32.to_bits()]),
                                ],
                                RETURN,
                            );
                            [2.0f32, 93.0, 4.0]
                        } else {
                            expect(
                                lot,
                                "lot_center",
                                vec![out(&[10.0f32.to_bits(), 20.0f32.to_bits(), 30.0f32.to_bits()])],
                                RETURN,
                            );
                            [10.0f32, 23.0, 30.0]
                        };
                        expect(
                            manager,
                            "create_zot_prop",
                            vec![
                                w(0xDEAD_BEEF),
                                Arg::In(expected_position.map(f32::to_bits).to_vec()),
                                w(0xABCD_0123),
                            ],
                            prop as u64,
                        );
                        expect(prop, "removal_flags", vec![w(3)], RETURN);
                        expect(prop, "get_prop_holder", vec![], prop_holder as u64);
                        expect(
                            prop_holder,
                            "add_property",
                            vec![w(0x48E9_5539), w(requested), w(0)],
                            RETURN,
                        );
                    }
                    let args = [lot as u32, 0xDEAD_BEEF, requested];
                    let mut call =
                        RawCall::new(Conv::Thiscall, fc4p_006c98c0 as *const () as usize, this as u32, &args);
                    call.edx = 0xCAFE_F00D;
                    call_raw(&mut call);
                    assert_eq!(
                        result(call),
                        if rejected {
                            0x4321_5678_0000_0000 | u64::from(current)
                        } else {
                            RETURN
                        }
                    );
                    finished();
                }
            }
        }
    }
}
