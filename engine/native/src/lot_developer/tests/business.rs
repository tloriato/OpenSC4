// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::business::*;

slot!(road_access);
slot!(travel_designate);
slot!(set_travel_designate, lot);
slot!(join_unit, lot);
slot!(members);

unsafe extern "thiscall" fn reference_new(this: *mut u32, lot: u32) -> u64 {
    let returned = run(core::ptr::null_mut(), "reference_new", &[lot]);
    unsafe {
        this.write_unaligned(lot);
    }
    returned
}

unsafe extern "thiscall" fn push_candidate(vector: *mut u32, reference: *mut u32) -> u64 {
    unsafe {
        let value = reference.read_unaligned();
        let begin = vector.read_unaligned();
        let end = vector.add(1).read_unaligned();
        let returned = run(core::ptr::null_mut(), "push_candidate", &[value, begin, end]);
        let output = returned as u32;
        if begin == 0 {
            vector.write_unaligned(output);
            vector.add(1).write_unaligned(output);
            vector.add(2).write_unaligned(output.wrapping_add(64));
        }
        let end = vector.add(1).read_unaligned();
        (end as *mut u32).write_unaligned(value);
        vector.add(1).write_unaligned(end.wrapping_add(4));
        returned
    }
}

unsafe extern "thiscall" fn destroy_candidates(vector: *mut u32) -> u64 {
    unsafe {
        run(
            core::ptr::null_mut(),
            "destroy_candidates",
            &[
                vector.read_unaligned(),
                vector.add(1).read_unaligned(),
                vector.add(2).read_unaligned(),
            ],
        )
    }
}

unsafe extern "thiscall" fn choose_candidate(this: *mut u8, bound: u32) -> u64 {
    run(this, "choose_candidate", &[bound])
}
unsafe extern "thiscall" fn construct_unit(this: *mut u8) -> u64 {
    run(this, "construct_unit", &[])
}

unsafe fn pick(this: *mut u8, lot: *mut u8) -> [u32; 3] {
    let mut frame = [lot as u32, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi]", "call {function}", "mov [edi + 4], eax", "mov [edi + 8], edx",
            "mov eax, [esp - 4]", "mov [edi + 12], eax",
            function = sym fc4p_006cb260, in("edi") frame.as_mut_ptr(), in("ecx") this, clobber_abi("C"),
        );
    }
    frame[1..].try_into().unwrap()
}

#[test]
fn unit_selection_gets_location_even_when_already_joined_and_rejects_other_purposes() {
    let mut developer = Object::new(0x108);
    let mut lot_object = mock!(0x54 => get_location, 0x64 => business_unit, 0xB8 => get_building_type);
    let mut service = mock!(0x9C => get_profile);
    let (this, lot, service) = (developer.ptr(), lot_object.ptr(), service.ptr());
    unsafe {
        LotDeveloper::from_ptr(this).set_building_development(service);
    }
    for (unit, kind, purpose) in [
        (0x1234, 0, 5),
        (0, 0, 5),
        (0, 1, -1),
        (0, 1, 4),
        (0, 1, 9),
        (0, 1, 0x105),
    ] {
        let mut profile = purpose;
        expect(lot, "get_location", vec![out(&[7]), out(&[9])], RETURN);
        expect(lot, "business_unit", vec![], (RETURN & !0xFFFF_FFFF) | unit);
        if unit == 0 {
            expect(lot, "get_building_type", vec![w(1)], (RETURN & !0xFFFF_FFFF) | kind);
            if kind != 0 {
                expect(
                    service,
                    "get_profile",
                    vec![w(kind as u32)],
                    &mut profile as *mut i32 as u64,
                );
            }
        }
        let actual = unsafe { pick(this, lot) };
        assert_eq!(actual[2], lot as u32, "early exit keeps the lot argument");
        assert_eq!(unsafe { LotDeveloper::from_ptr(this).business_units_dirty() }, 0);
        finished();
    }
}

#[test]
fn unit_selection_checks_all_neighbor_filters_without_drawing_for_an_empty_vector() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _clear = fake(0x0047_3EC0, clear_list as *const () as usize);
    let _destroy = fake(0x006C_6E90, destroy_candidates as *const () as usize);
    unsafe {
        for purpose in 5..=8 {
            let mut developer = Object::new(0x108);
            let mut lot_object = mock!(0x54 => get_location, 0x64 => business_unit, 0xB8 => get_building_type);
            let mut first_manager = mock!(0x24 => surrounding_lots);
            let mut manager = mock!(0x24 => surrounding_lots);
            let mut service = mock!(0x9C => get_profile);
            let (this, lot, first, manager, service) = (
                developer.ptr(),
                lot_object.ptr(),
                first_manager.ptr(),
                manager.ptr(),
                service.ptr(),
            );
            let dev = LotDeveloper::from_ptr(this);
            dev.set_lot_manager(first);
            dev.set_building_development(service);
            dev.set_business_units_dirty(0xA5);
            let mut profile = purpose;
            let mut mismatch = purpose + 1;
            let mut neighbors: Vec<_> = (0..6)
                .map(|_| {
                    mock!(0xB8 => get_building_type, 0x84 => habitation_state,
                0x64 => business_unit, 0xC0 => building_occupant, 0x70 => road_access, 0xEC => travel_designate)
                })
                .collect();
            let lots: Vec<_> = neighbors.iter_mut().map(Mock::ptr).collect();
            let mut head = [0xA5A5_A5A5u32; 3];
            let sentinel = head.as_mut_ptr() as u32;
            let mut nodes: Vec<_> = lots.iter().map(|&lot| [0u32, 0, lot as u32]).collect();
            let links: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for (i, node) in nodes.iter_mut().enumerate() {
                node[0] = links.get(i + 1).copied().unwrap_or(sentinel);
            }
            expect(lot, "get_location", vec![out(&[7]), out(&[9])], RETURN);
            expect(lot, "business_unit", vec![], 0);
            expect(lot, "get_building_type", vec![w(1)], 0x22);
            expect(service, "get_profile", vec![w(0x22)], &mut profile as *mut i32 as u64);
            expect(
                core::ptr::null_mut(),
                "operation_allocate",
                vec![w(12)],
                u64::from(sentinel),
            );
            for (index, flags) in [1, 2, 4, 8].into_iter().enumerate() {
                expect(
                    if index == 0 { first } else { manager },
                    "surrounding_lots",
                    vec![p(lot), Arg::In(vec![sentinel]), w(flags)],
                    RETURN,
                );
                if index == 0 {
                    edit_last(this.add(0x24).cast(), manager as u32);
                }
                if flags == 8 {
                    edit_last(head.as_mut_ptr(), links[0]);
                }
            }
            expect(lots[0], "get_building_type", vec![w(1)], 0);
            expect(lots[1], "get_building_type", vec![w(1)], 1);
            expect(lots[1], "habitation_state", vec![], u64::from(u32::MAX));
            expect(lots[1], "business_unit", vec![], 0);
            expect(lots[2], "get_building_type", vec![w(1)], 2);
            expect(lots[2], "habitation_state", vec![], 2);
            expect(service, "get_profile", vec![w(2)], &mut mismatch as *mut i32 as u64);
            for &neighbor in &lots[3..] {
                expect(neighbor, "get_building_type", vec![w(1)], 3);
                expect(neighbor, "habitation_state", vec![], 1);
                expect(neighbor, "business_unit", vec![], 0x100);
                expect(service, "get_profile", vec![w(3)], &mut profile as *mut i32 as u64);
                if purpose == 5 {
                    expect(neighbor, "building_occupant", vec![], 0);
                } else {
                    expect(neighbor, "road_access", vec![], 0x100);
                    expect(
                        neighbor,
                        "travel_designate",
                        vec![],
                        if neighbor == lots[3] { 0 } else { lot as u64 },
                    );
                    if neighbor != lots[3] {
                        // Use the lot object's unit slot as a zero-low-byte road-access fake.
                        lot_object._vtable[0x70 / 4] = road_access as *const () as usize;
                        expect(lot, "road_access", vec![], 0x100);
                    }
                }
            }
            expect(
                core::ptr::null_mut(),
                "destroy_candidates",
                vec![w(0), w(0), w(0)],
                RETURN,
            );
            expect(core::ptr::null_mut(), "clear_list", vec![w(sentinel)], RETURN);
            expect(core::ptr::null_mut(), "operation_free", vec![w(sentinel)], RETURN);
            assert_eq!(pick(this, lot), [RETURN as u32, (RETURN >> 32) as u32, sentinel]);
            assert_eq!(dev.business_units_dirty(), 0xA5);
            assert_eq!(head[2], 0xA5A5_A5A5);
            finished();
        }
    }
}

#[test]
fn unit_selection_joins_existing_or_new_units_and_uses_industrial_first_member() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _unit_allocate = fake(0x005E_55E0, operation_allocate as *const () as usize);
    let _new = fake(0x006C_0800, construct_unit as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _clear = fake(0x0047_3EC0, clear_list as *const () as usize);
    let _destroy = fake(0x006C_6E90, destroy_candidates as *const () as usize);
    let _reference = fake(0x004F_20C0, reference_new as *const () as usize);
    let _push = fake(0x0044_2E40, push_candidate as *const () as usize);
    let _random = fake(0x0091_36B1, choose_candidate as *const () as usize);
    let mut random = 0u8;
    let _random_data = fake_data(0x00B4_9AF0, &mut random);
    unsafe {
        for purpose in 5..=8 {
            for existing in [false, true] {
                let mut developer = Object::new(0x108);
                let mut lot_object = mock!(0x54 => get_location, 0x64 => business_unit, 0xB8 => get_building_type, 0xF0 => set_travel_designate);
                let mut manager = mock!(0x24 => surrounding_lots);
                let mut service = mock!(0x9C => get_profile);
                let mut unit = mock!(0x0C => join_unit, 0x14 => members);
                let (this, lot, manager, service, unit) = (
                    developer.ptr(),
                    lot_object.ptr(),
                    manager.ptr(),
                    service.ptr(),
                    unit.ptr(),
                );
                let dev = LotDeveloper::from_ptr(this);
                dev.set_lot_manager(manager);
                dev.set_building_development(service);
                let mut profile = purpose;
                let mut neighbors: Vec<_> = (0..2)
                    .map(|_| {
                        mock!(0xB8 => get_building_type, 0x84 => habitation_state, 0x64 => business_unit,
                    0xC0 => building_occupant, 0x70 => road_access, 0xEC => travel_designate, 8 => release)
                    })
                    .collect();
                let lots: Vec<_> = neighbors.iter_mut().map(Mock::ptr).collect();
                let mut head = [0xA5A5_A5A5u32; 3];
                let sentinel = head.as_mut_ptr() as u32;
                let mut nodes = [[0, 0, lots[0] as u32], [sentinel, 0, lots[1] as u32]];
                nodes[0][0] = nodes[1].as_mut_ptr() as u32;
                let first = nodes[0].as_mut_ptr() as u32;
                let mut buffer = [0u32; 16];
                let storage = buffer.as_mut_ptr() as u32;
                expect(lot, "get_location", vec![out(&[7]), out(&[9])], RETURN);
                expect(lot, "business_unit", vec![], 0);
                expect(lot, "get_building_type", vec![w(1)], 0x22);
                expect(service, "get_profile", vec![w(0x22)], &mut profile as *mut i32 as u64);
                expect(
                    core::ptr::null_mut(),
                    "operation_allocate",
                    vec![w(12)],
                    u64::from(sentinel),
                );
                for flags in [1, 2, 4, 8] {
                    expect(
                        manager,
                        "surrounding_lots",
                        vec![p(lot), Arg::In(vec![sentinel]), w(flags)],
                        RETURN,
                    );
                    if flags == 8 {
                        edit_last(head.as_mut_ptr(), first);
                    }
                }
                for (index, &neighbor) in lots.iter().enumerate() {
                    expect(neighbor, "get_building_type", vec![w(1)], 3);
                    expect(neighbor, "habitation_state", vec![], 2);
                    expect(service, "get_profile", vec![w(3)], &mut profile as *mut i32 as u64);
                    if purpose == 5 {
                        expect(neighbor, "building_occupant", vec![], 0x100);
                    } else {
                        expect(neighbor, "road_access", vec![], if index == 0 { 0x100 } else { 1 });
                        if index == 0 {
                            expect(neighbor, "travel_designate", vec![], lots[1] as u64);
                            expect(lots[1], "road_access", vec![], 0x101);
                        }
                    }
                    expect(core::ptr::null_mut(), "reference_new", vec![p(neighbor)], RETURN);
                    expect(
                        core::ptr::null_mut(),
                        "push_candidate",
                        vec![
                            p(neighbor),
                            w(if index == 0 { 0 } else { storage }),
                            w(if index == 0 { 0 } else { storage + 4 }),
                        ],
                        u64::from(storage),
                    );
                    expect(neighbor, "release", vec![], RETURN);
                }
                expect(&mut random, "choose_candidate", vec![w(2)], 1);
                expect(lots[1], "business_unit", vec![], if existing { unit as u64 } else { 0 });
                if !existing {
                    expect(core::ptr::null_mut(), "operation_allocate", vec![w(32)], unit as u64);
                    expect(unit, "construct_unit", vec![], unit as u64);
                    expect(unit, "join_unit", vec![p(lots[1])], RETURN);
                }
                expect(unit, "join_unit", vec![p(lot)], RETURN);
                let mut member_head = [first, 0, 0];
                let mut list = member_head.as_mut_ptr();
                if purpose == 5 {
                    expect(unit, "members", vec![], &mut list as *mut *mut u32 as u64);
                    expect(lot, "set_travel_designate", vec![p(lots[0])], RETURN);
                }
                expect(
                    core::ptr::null_mut(),
                    "destroy_candidates",
                    vec![w(storage), w(storage + 8), w(storage + 64)],
                    RETURN,
                );
                expect(core::ptr::null_mut(), "clear_list", vec![w(sentinel)], RETURN);
                expect(core::ptr::null_mut(), "operation_free", vec![w(sentinel)], RETURN);
                assert_eq!(pick(this, lot), [RETURN as u32, (RETURN >> 32) as u32, sentinel]);
                assert_eq!(dev.business_units_dirty(), u8::from(purpose != 5));
                assert_eq!(&buffer[..2], &[lots[0] as u32, lots[1] as u32]);
                finished();
            }
        }
    }
}

slot!(leave_unit, lot);

unsafe extern "C" fn initialize_anchor(first: u32, last: u32, output: u32, allocator: *mut u32) -> u64 {
    assert!(!allocator.is_null());
    run(core::ptr::null_mut(), "initialize_anchor", &[first, last, output])
}

unsafe extern "thiscall" fn copy_members(this: *mut u32, head: u32, first: u32, last: u32, allocator: *mut u32) -> u64 {
    assert_eq!(this, allocator);
    run(core::ptr::null_mut(), "copy_members", &[head, first, last])
}

unsafe extern "thiscall" fn try_join_unit(this: *mut u8, lot: u32) -> u64 {
    run(this, "try_join_unit", &[lot])
}

unsafe fn designate(this: *mut u8, reference: *mut *mut u8) -> [u32; 3] {
    let mut frame = [reference as u32, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi]", "call {function}", "mov [edi + 4], eax", "mov [edi + 8], edx",
            "mov eax, [esp - 4]", "mov [edi + 12], eax",
            function = sym fc4p_006cb4d0, in("edi") frame.as_mut_ptr(), in("ecx") this, clobber_abi("C"),
        );
    }
    frame[1..].try_into().unwrap()
}

#[test]
fn travel_designates_follow_membership_and_road_table_with_balanced_temporary_references() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _initial = fake(0x0042_E4C0, initialize_anchor as *const () as usize);
    unsafe {
        let mut developer = Object::new(0x108);
        let this = developer.ptr();
        let mut unit_object = mock!(0x14 => members);
        let unit = unit_object.ptr();
        let mut reference = unit;
        let mut lot_objects: Vec<_> = (0..6)
            .map(|_| {
                mock!(0x70 => road_access, 0xC0 => building_occupant,
            0xEC => travel_designate, 0xF0 => set_travel_designate, 4 => add_ref, 8 => release)
            })
            .collect();
        let lots: Vec<_> = lot_objects.iter_mut().map(Mock::ptr).collect();
        let mut outsider_object = mock!(4 => add_ref, 8 => release);
        let outsider = outsider_object.ptr();
        let mut head = [0u32; 3];
        let sentinel = head.as_mut_ptr() as u32;
        let mut nodes: Vec<_> = lots.iter().map(|&lot| [0u32, 0, lot as u32]).collect();
        let links: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        for (i, node) in nodes.iter_mut().enumerate() {
            node[0] = links.get(i + 1).copied().unwrap_or(sentinel);
        }
        head[0] = links[0];
        let mut list = head.as_mut_ptr();
        let mut buffer = [0u32];
        let storage = buffer.as_mut_ptr() as u32;
        expect(unit, "members", vec![], &mut list as *mut *mut u32 as u64);
        expect(lots[0], "road_access", vec![], 0x100);
        expect(lots[1], "road_access", vec![], 0x101);
        expect(lots[1], "building_occupant", vec![], 0x100);
        expect(lots[1], "add_ref", vec![], RETURN);
        expect(
            core::ptr::null_mut(),
            "operation_allocate",
            vec![w(4)],
            u64::from(storage),
        );
        expect(
            core::ptr::null_mut(),
            "initialize_anchor",
            vec![w(0), w(0), w(storage)],
            u64::from(storage),
        );
        expect(lots[1], "add_ref", vec![], RETURN);
        expect(lots[1], "release", vec![], RETURN);
        let designates = [
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            lots[1],
            core::ptr::null_mut(),
            lots[3],
            outsider,
        ];
        for (index, (&lot, &target)) in lots.iter().zip(designates.iter()).enumerate() {
            expect(lot, "travel_designate", vec![], target as u64);
            if !target.is_null() {
                expect(target, "add_ref", vec![], RETURN);
                expect(target, "release", vec![], RETURN);
                if target != outsider {
                    expect(target, "road_access", vec![], if index == 2 { 0x101 } else { 0x100 });
                }
            }
            if index != 1 && index != 2 {
                if !target.is_null() {
                    expect(lot, "set_travel_designate", vec![w(0)], RETURN);
                }
                expect(lot, "set_travel_designate", vec![p(lots[1])], RETURN);
            }
        }
        expect(lots[1], "release", vec![], RETURN);
        expect(core::ptr::null_mut(), "operation_free", vec![w(storage)], RETURN);
        let original_argument = &mut reference as *mut *mut u8 as u32;
        assert_eq!(
            designate(this, &mut reference),
            [RETURN as u32, (RETURN >> 32) as u32, (original_argument & !255) | 1]
        );
        assert_eq!(reference, unit);
        assert_eq!(buffer[0], lots[1] as u32);
        finished();
    }
}

#[test]
fn travel_designates_without_anchor_dissolve_or_merge_every_snapshot_member_in_order() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _copy = fake(0x006C_6CA0, copy_members as *const () as usize);
    let _pick = fake(0x006C_B260, try_join_unit as *const () as usize);
    unsafe {
        for joining in [None, Some(0usize), Some(1usize)] {
            let mut developer = Object::new(0x108);
            let this = developer.ptr();
            let mut unit_object = mock!(0x14 => members, 0x10 => leave_unit);
            let mut new_object = mock!(0x0C => join_unit, 0x10 => leave_unit);
            let (unit, new_unit) = (unit_object.ptr(), new_object.ptr());
            let mut reference = unit;
            let mut lot_objects: Vec<_> = (0..3)
                .map(|_| {
                    mock!(0x70 => road_access, 0xC0 => building_occupant,
                0x64 => business_unit, 8 => release)
                })
                .collect();
            let lots: Vec<_> = lot_objects.iter_mut().map(Mock::ptr).collect();
            let mut head = [0u32; 3];
            let sentinel = head.as_mut_ptr() as u32;
            let mut nodes: Vec<_> = lots.iter().map(|&lot| [0u32, 0, lot as u32]).collect();
            let links: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for (i, node) in nodes.iter_mut().enumerate() {
                node[0] = links.get(i + 1).copied().unwrap_or(sentinel);
            }
            head[0] = links[0];
            let mut list = head.as_mut_ptr();
            let mut copy_head = [0xA5A5_A5A5u32; 3];
            let copy_sentinel = copy_head.as_mut_ptr() as u32;
            let mut copy_nodes: Vec<_> = lots.iter().map(|&lot| [0u32, 0, lot as u32]).collect();
            let copy_links: Vec<_> = copy_nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            for (i, node) in copy_nodes.iter_mut().enumerate() {
                node[0] = copy_links.get(i + 1).copied().unwrap_or(copy_sentinel);
            }
            expect(unit, "members", vec![], &mut list as *mut *mut u32 as u64);
            for (index, &lot) in lots.iter().enumerate() {
                expect(lot, "road_access", vec![], if index == 0 { 0x101 } else { 0x100 });
                if index == 0 {
                    expect(lot, "building_occupant", vec![], 0);
                }
            }
            expect(unit, "members", vec![], &mut list as *mut *mut u32 as u64);
            expect(
                core::ptr::null_mut(),
                "operation_allocate",
                vec![w(12)],
                u64::from(copy_sentinel),
            );
            expect(
                core::ptr::null_mut(),
                "copy_members",
                vec![w(copy_sentinel), w(links[0]), w(sentinel)],
                RETURN,
            );
            edit_last(copy_head.as_mut_ptr(), copy_links[0]);
            edit_last(copy_head.as_mut_ptr().add(1), copy_links[2]);
            for (index, &lot) in lots.iter().enumerate() {
                expect(unit, "leave_unit", vec![p(lot)], RETURN);
                expect(this, "try_join_unit", vec![p(lot)], RETURN);
                expect(
                    lot,
                    "business_unit",
                    vec![],
                    if joining == Some(index) { new_unit as u64 } else { 0 },
                );
                if joining == Some(index) {
                    break;
                }
            }
            if let Some(joined_index) = joining {
                for (index, &lot) in lots.iter().enumerate() {
                    let old = if index == joined_index {
                        new_unit
                    } else if index > joined_index {
                        unit
                    } else {
                        core::ptr::null_mut()
                    };
                    expect(lot, "business_unit", vec![], old as u64);
                    if !old.is_null() {
                        expect(old, "leave_unit", vec![p(lot)], RETURN);
                    }
                    expect(new_unit, "join_unit", vec![p(lot)], RETURN);
                }
            }
            for (index, &lot) in lots.iter().enumerate() {
                expect(lot, "release", vec![], RETURN);
                expect(
                    core::ptr::null_mut(),
                    "operation_free",
                    vec![w(copy_links[index])],
                    RETURN,
                );
            }
            expect(core::ptr::null_mut(), "operation_free", vec![w(copy_sentinel)], RETURN);
            let argument = &mut reference as *mut *mut u8 as u32;
            assert_eq!(
                designate(this, &mut reference),
                [RETURN as u32, (RETURN >> 32) as u32, argument]
            );
            assert_eq!(
                reference,
                if joining.is_some() {
                    new_unit
                } else {
                    core::ptr::null_mut()
                }
            );
            assert_eq!(copy_head, [copy_sentinel, copy_sentinel, 0xA5A5_A5A5]);
            finished();
        }
    }
}

#[test]
fn travel_designates_empty_unit_clears_reference_and_frees_only_the_snapshot_head() {
    let _allocate = fake(0x0090_CF54, operation_allocate as *const () as usize);
    let _free = fake(0x0090_CF63, operation_free as *const () as usize);
    let _copy = fake(0x006C_6CA0, copy_members as *const () as usize);
    unsafe {
        let mut developer = Object::new(0x108);
        let mut unit = mock!(0x14 => members);
        let unit = unit.ptr();
        let mut reference = unit;
        let mut head = [0u32; 3];
        let sentinel = head.as_mut_ptr() as u32;
        head[0] = sentinel;
        let mut list = head.as_mut_ptr();
        let mut copy_head = [0xA5A5_A5A5u32; 3];
        let copy_sentinel = copy_head.as_mut_ptr() as u32;
        expect(unit, "members", vec![], &mut list as *mut *mut u32 as u64);
        expect(unit, "members", vec![], &mut list as *mut *mut u32 as u64);
        expect(
            core::ptr::null_mut(),
            "operation_allocate",
            vec![w(12)],
            u64::from(copy_sentinel),
        );
        expect(
            core::ptr::null_mut(),
            "copy_members",
            vec![w(copy_sentinel), w(sentinel), w(sentinel)],
            RETURN,
        );
        expect(core::ptr::null_mut(), "operation_free", vec![w(copy_sentinel)], RETURN);
        let argument = &mut reference as *mut *mut u8 as u32;
        assert_eq!(
            designate(developer.ptr(), &mut reference),
            [RETURN as u32, (RETURN >> 32) as u32, argument]
        );
        assert!(reference.is_null());
        assert_eq!(copy_head, [copy_sentinel, copy_sentinel, 0xA5A5_A5A5]);
        finished();
    }
}
