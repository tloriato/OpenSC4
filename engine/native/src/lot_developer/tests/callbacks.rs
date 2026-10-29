// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::callbacks::*;
use crate::lot_developer::queue::QueueIterator;

unsafe fn invoke(entry: usize, this: *mut u8, args: &[u32]) -> u64 {
    let mut words = args.to_vec();
    unsafe { invoke_observing_arguments(entry, this, &mut words) }
}

unsafe fn invoke_observing_arguments(entry: usize, this: *mut u8, args: &mut [u32]) -> u64 {
    let mut record = [
        entry as u32,
        this as u32,
        args.as_mut_ptr() as u32,
        args.len() as u32,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push esi", "mov ecx, dword ptr [edi + 12]", "mov eax, dword ptr [edi + 8]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov ecx, dword ptr [edi + 4]", "call dword ptr [edi]",
            "mov dword ptr [edi + 16], eax", "mov dword ptr [edi + 20], edx",
            "mov ecx, dword ptr [edi + 12]", "mov eax, ecx", "shl eax, 2", "neg eax", "add eax, esp",
            "mov edx, dword ptr [edi + 8]", "test ecx, ecx", "jz 5f",
            "4:", "mov esi, dword ptr [eax + ecx*4 - 4]", "mov dword ptr [edx + ecx*4 - 4], esi",
            "dec ecx", "jnz 4b", "5:", "pop esi",
            in("edi") record.as_mut_ptr(), clobber_abi("C"),
        );
    }
    u64::from(record[5]) << 32 | u64::from(record[4])
}

slot!(set_lit, night);
slot!(occupant_cells, rect);
slot!(allocate_list, size);
slot!(query_cells, list, min, max, kind, limit);
slot!(get_bool);
slot!(post_light, id, data);
slot!(query_building, id, output);
slot!(grow_queue, value);
slot!(query_service, class, iid, output);

unsafe extern "C" fn framework() -> u64 {
    run(core::ptr::null_mut(), "framework", &[])
}

unsafe extern "thiscall" fn release_temp(this: *mut *mut u8) -> u64 {
    unsafe {
        let value = this.read_unaligned();
        let global = crate::abi::data_target(0x00B2_07E4) as *const *mut u8;
        assert_eq!(
            global.read(),
            value,
            "allocator is published before the temporary is released"
        );
        let returned = run(core::ptr::null_mut(), "release_temp", &[value as u32]);
        this.write_unaligned(core::ptr::null_mut());
        returned
    }
}

unsafe extern "thiscall" fn destroy_list(this: *mut *mut u8) -> u64 {
    run(
        core::ptr::null_mut(),
        "destroy_list",
        &[unsafe { this.read_unaligned() } as u32],
    )
}

unsafe extern "thiscall" fn remove_list(this: *mut u8, list: *mut *mut u8) -> u64 {
    run(this, "remove_list", &[unsafe { list.read_unaligned() } as u32])
}

unsafe extern "thiscall" fn queue_at(this: *mut u32, output: *mut u32, offset: u32) -> u64 {
    let returned = run(core::ptr::null_mut(), "queue_at", &[this as u32, output as u32, offset]);
    returned & 0xFFFF_FFFF_0000_0000 | u64::from(output as u32)
}

struct List {
    head: Box<[u32; 3]>,
    nodes: Vec<[u32; 3]>,
}

impl List {
    fn new(payloads: &[*mut u8]) -> Self {
        let mut list = Self {
            head: Box::new([0xA5A5_A5A5; 3]),
            nodes: payloads.iter().map(|&p| [0, 0, p as u32]).collect(),
        };
        let head = list.ptr() as u32;
        let links: Vec<_> = list.nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        for (i, node) in list.nodes.iter_mut().enumerate() {
            node[0] = links.get(i + 1).copied().unwrap_or(head);
            node[1] = if i == 0 { head } else { links[i - 1] };
        }
        list
    }
    fn ptr(&mut self) -> *mut u8 {
        self.head.as_mut_ptr().cast()
    }
    fn fill(&mut self) {
        let head = self.ptr();
        let next = self.nodes.first_mut().map_or(head as u32, |n| n.as_mut_ptr() as u32);
        let previous = self.nodes.last_mut().map_or(head as u32, |n| n.as_mut_ptr() as u32);
        edit_last(head.cast(), next);
        edit_last(unsafe { head.add(4) }.cast(), previous);
    }
}

#[test]
fn light_queries_the_lots_rectangle_and_not_the_building_rectangle() {
    let _destroy = fake(0x0041_3FE0, destroy_list as *const () as usize);
    unsafe {
        for query in [0u32, 0x100, 1] {
            let mut developer = Object::new(0x108);
            let this = developer.ptr();
            let mut building = mock!(0x44 => set_lit, 0x0C => get_occupant);
            let building = building.ptr();
            let mut occupant = mock!(0x2C => occupant_cells);
            let occupant = occupant.ptr();
            let mut manager = mock!(0x18 => get_lot);
            let manager = manager.ptr();
            let mut lot = mock!(0x38 => bounding_rect);
            let lot = lot.ptr();
            let mut occupants = mock!(0xA4 => query_cells);
            let occupants = occupants.ptr();
            let mut alloc = mock!(0x0C => allocate_list);
            let mut allocator = alloc.ptr();
            let _allocator = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
            let mut props: Vec<_> = (0..4)
                .map(|_| mock!(0x18 => get_holder, 0x3C => post_light, 8 => release))
                .collect();
            let props: Vec<_> = props.iter_mut().map(Mock::ptr).collect();
            let mut holder = mock!(0x24 => get_property);
            let holder = holder.ptr();
            let mut property = mock!(0x14 => get_variant);
            let property = property.ptr();
            let mut variant = mock!(0xFC => get_bool);
            let variant = variant.ptr();
            let mut flags = [0u32, 0x100, 0x80];
            let mut list = List::new(&props);
            let head = list.ptr();
            LotDeveloper::from_ptr(this).set_lot_manager(manager);
            LotDeveloper::from_ptr(this).set_occupant_manager(occupants);
            expect(building, "set_lit", vec![w(0x1234_5601)], RETURN);
            expect(building, "get_occupant", vec![], occupant as u64);
            expect(occupant, "occupant_cells", vec![out(&[1, 1, 2, 2])], RETURN);
            expect(manager, "get_lot", vec![w(1), w(1), w(0)], lot as u64);
            expect(lot, "bounding_rect", vec![out(&[0, 0, 3, 3])], RETURN);
            expect(allocator, "allocate_list", vec![w(12)], head as u64);
            expect(
                occupants,
                "query_cells",
                vec![
                    Arg::In(vec![head as u32]),
                    Arg::In(vec![0, 0]),
                    Arg::In(vec![3, 3]),
                    w(0xA823_821E),
                    w(u32::MAX),
                ],
                u64::from(query),
            );
            list.fill();
            if query as u8 != 0 {
                for (i, &prop) in props.iter().enumerate() {
                    expect(prop, "get_holder", vec![], holder as u64);
                    expect(
                        holder,
                        "get_property",
                        vec![w(0x4A9F_188B)],
                        if i == 0 { 0 } else { property as u64 },
                    );
                    if i != 0 {
                        expect(property, "get_variant", vec![], variant as u64);
                        expect(variant, "get_bool", vec![], flags.as_mut_ptr().add(i - 1) as u64);
                        if i == 3 {
                            expect(prop, "post_light", vec![w(0x09D8_E0B6), w(0)], RETURN);
                        }
                    }
                    expect(prop, "release", vec![], RETURN);
                }
            }
            expect(core::ptr::null_mut(), "destroy_list", vec![p(head)], RETURN);
            let mut args = [building as u32, 0x1234_5601];
            assert_eq!(
                invoke_observing_arguments(fc4p_006c8e10 as *const () as usize, this, &mut args),
                RETURN
            );
            assert_eq!(args, [head as u32, 0x1234_5601]);
            assert_eq!(list.head[2], 0xA5A5_A5A5);
            finished();
        }
    }
}

#[test]
fn light_without_a_lot_wraps_and_arithmetically_shifts_the_center_and_never_allocates() {
    unsafe {
        let mut developer = Object::new(0x108);
        let this = developer.ptr();
        let mut building = mock!(0x44 => set_lit, 0x0C => get_occupant);
        let building = building.ptr();
        let mut occupant = mock!(0x2C => occupant_cells);
        let occupant = occupant.ptr();
        let mut manager = mock!(0x18 => get_lot);
        let manager = manager.ptr();
        LotDeveloper::from_ptr(this).set_lot_manager(manager);
        expect(building, "set_lit", vec![w(0)], RETURN);
        expect(building, "get_occupant", vec![], occupant as u64);
        expect(
            occupant,
            "occupant_cells",
            vec![out(&[i32::MAX as u32, (-5i32) as u32, 1, 2])],
            RETURN,
        );
        expect(
            manager,
            "get_lot",
            vec![w((i32::MIN >> 1) as u32), w((-2i32) as u32), w(0)],
            RETURN & 0xFFFF_FFFF_0000_0000,
        );
        assert_eq!(
            invoke(fc4p_006c8e10 as *const () as usize, this, &[building as u32, 0]),
            RETURN & 0xFFFF_FFFF_0000_0000
        );
        finished();
    }
}

#[test]
fn light_lazy_allocator_uses_the_second_argument_and_reloads_the_published_service() {
    let _framework = fake(0x0087_93EC, framework as *const () as usize);
    let _release = fake(0x0041_1870, release_temp as *const () as usize);
    let _destroy = fake(0x0041_3FE0, destroy_list as *const () as usize);
    unsafe {
        let mut developer = Object::new(0x108);
        let this = developer.ptr();
        let mut building = mock!(0x44 => set_lit, 0x0C => get_occupant);
        let building = building.ptr();
        let mut occupant = mock!(0x2C => occupant_cells);
        let occupant = occupant.ptr();
        let mut manager = mock!(0x18 => get_lot);
        let manager = manager.ptr();
        let mut lot = mock!(0x38 => bounding_rect);
        let lot = lot.ptr();
        let mut occupants = mock!(0xA4 => query_cells);
        let occupants = occupants.ptr();
        LotDeveloper::from_ptr(this).set_lot_manager(manager);
        LotDeveloper::from_ptr(this).set_occupant_manager(occupants);
        let mut root = mock!(0x14 => query_service);
        let root = root.ptr();
        let mut first_service = mock!();
        let first_service = first_service.ptr();
        let mut allocator_object = mock!(0x0C => allocate_list);
        let allocator = allocator_object.ptr();
        let mut published = core::ptr::null_mut::<u8>();
        let _allocator = fake_data(0x00B2_07E4, (&mut published as *mut *mut u8).cast());
        let mut list = List::new(&[]);
        let head = list.ptr();
        expect(building, "set_lit", vec![w(1)], RETURN);
        expect(building, "get_occupant", vec![], occupant as u64);
        expect(occupant, "occupant_cells", vec![out(&[0; 4])], RETURN);
        expect(manager, "get_lot", vec![w(0), w(0), w(0)], lot as u64);
        expect(lot, "bounding_rect", vec![out(&[1, 2, 3, 4])], RETURN);
        expect(core::ptr::null_mut(), "framework", vec![], root as u64);
        expect(
            root,
            "query_service",
            vec![w(0x3AE4_BEA3), w(0x3AE4_BEAB), out(&[first_service as u32])],
            RETURN,
        );
        expect(core::ptr::null_mut(), "release_temp", vec![p(first_service)], RETURN);
        edit_last((&mut published as *mut *mut u8).cast(), allocator as u32);
        expect(allocator, "allocate_list", vec![w(12)], head as u64);
        expect(
            occupants,
            "query_cells",
            vec![
                Arg::In(vec![head as u32]),
                Arg::In(vec![1, 2]),
                Arg::In(vec![3, 4]),
                w(0xA823_821E),
                w(u32::MAX),
            ],
            1,
        );
        expect(core::ptr::null_mut(), "destroy_list", vec![p(head)], RETURN);
        let mut args = [building as u32, 1];
        assert_eq!(
            invoke_observing_arguments(fc4p_006c8e10 as *const () as usize, this, &mut args),
            RETURN
        );
        assert_eq!(args, [head as u32, 0]);
        assert_eq!(published, allocator);
        assert_eq!(*list.head, [head as u32, head as u32, 0xA5A5_A5A5]);
        finished();
    }
}

#[test]
fn texture_removal_allocates_before_null_checks_and_uses_query_low_byte() {
    let _destroy = fake(0x0041_3FE0, destroy_list as *const () as usize);
    let _remove = fake(0x006C_62F0, remove_list as *const () as usize);
    unsafe {
        for mode in 0..5 {
            let mut developer = Object::new(0x108);
            let this = developer.ptr();
            let mut lot = mock!(0x38 => bounding_rect);
            let lot = if mode == 0 { core::ptr::null_mut() } else { lot.ptr() };
            let mut manager = mock!(0xA4 => query_cells);
            let manager = if mode == 1 {
                core::ptr::null_mut()
            } else {
                manager.ptr()
            };
            let mut replacement = mock!(0xA4 => query_cells);
            let replacement = replacement.ptr();
            LotDeveloper::from_ptr(this).set_occupant_manager(manager);
            let mut alloc = mock!(0x0C => allocate_list);
            let mut allocator = alloc.ptr();
            let _allocator = fake_data(0x00B2_07E4, (&mut allocator as *mut *mut u8).cast());
            let mut list = List::new(&[]);
            let head = list.ptr();
            expect(allocator, "allocate_list", vec![w(12)], head as u64);
            if mode >= 2 {
                expect(lot, "bounding_rect", vec![out(&[10, 20, 30, 40])], RETURN);
                edit_last(this.add(0x28).cast(), replacement as u32);
                let query = if mode == 4 { 1 } else { 0x100 };
                expect(
                    replacement,
                    "query_cells",
                    vec![
                        Arg::In(vec![head as u32]),
                        Arg::In(vec![10, 20]),
                        Arg::In(vec![30, 40]),
                        w(0x497F_6D9D),
                        w(u32::MAX),
                    ],
                    query,
                );
                if mode == 4 {
                    expect(this, "remove_list", vec![p(head)], RETURN);
                }
            }
            expect(core::ptr::null_mut(), "destroy_list", vec![p(head)], RETURN);
            assert_eq!(fc4p_006caaf0(this, 0xDEAD_BEEF, lot), RETURN);
            assert_eq!(*list.head, [head as u32, head as u32, 0xA5A5_A5A5]);
            finished();
        }
    }
}

fn iterator(block: *mut u32, node: *mut *mut u32, index: usize) -> QueueIterator {
    QueueIterator {
        current: block.wrapping_add(index),
        first: block,
        last: block.wrapping_add(32),
        node,
    }
}

fn words(value: QueueIterator) -> Vec<u32> {
    vec![
        value.current as u32,
        value.first as u32,
        value.last as u32,
        value.node as u32,
    ]
}

#[test]
fn night_registration_retains_the_queue_reference_and_restores_cursor_after_lighting() {
    let _at = fake(0x006C_6860, queue_at as *const () as usize);
    let _grow = fake(0x0062_66E0, grow_queue as *const () as usize);
    unsafe {
        for boundary in [false, true] {
            for lit in [0u8, 1, 0x80, 255] {
                let mut developer = Object::new(0x108);
                let this = developer.ptr();
                let view = LotDeveloper::from_ptr(this);
                let mut object = mock!(0 => query_building);
                let object = object.ptr();
                let mut building = mock!(4 => add_ref, 0x44 => set_lit, 8 => release);
                let building = building.ptr();
                let mut first_block = [0xCCCC_CCCCu32; 32];
                let mut second_block = [0xDDDD_DDDDu32; 32];
                let mut map = [first_block.as_mut_ptr(), second_block.as_mut_ptr()];
                let begin = iterator(map[0], map.as_mut_ptr(), 7);
                let cursor = iterator(map[0], map.as_mut_ptr(), 19);
                let end = iterator(map[0], map.as_mut_ptr(), if boundary { 31 } else { 20 });
                view.set_night_begin(begin);
                view.set_night_cursor(cursor);
                view.set_night_end(end);
                view.set_lit(lit);
                expect(
                    object,
                    "query_building",
                    vec![w(0x87DD_A3A9), out(&[building as u32])],
                    0xAABB_0001,
                );
                expect(building, "add_ref", vec![], RETURN);
                if boundary {
                    expect(
                        this.add(0x64),
                        "grow_queue",
                        vec![Arg::In(vec![building as u32])],
                        RETURN,
                    );
                    edit_last(this.add(0x74).cast(), map[1].add(1) as u32);
                }
                expect(building, "set_lit", vec![w(u32::from(lit))], RETURN);
                let new_begin = iterator(map[1], map.as_mut_ptr().add(1), 3);
                for (i, value) in words(new_begin).into_iter().enumerate() {
                    edit_last(this.add(0x64 + i * 4).cast(), value);
                }
                let new_cursor = iterator(map[1], map.as_mut_ptr().add(1), 15);
                expect(
                    core::ptr::null_mut(),
                    "queue_at",
                    vec![Arg::In(words(new_begin)), out(&words(new_cursor)), w(12)],
                    RETURN,
                );
                expect(building, "release", vec![], RETURN);
                let mut args = [object as u32];
                assert_eq!(
                    invoke_observing_arguments(fc4p_006c8f80 as *const () as usize, this, &mut args),
                    RETURN
                );
                assert_eq!(args, [building as u32]);
                assert_eq!(view.night_cursor(), new_cursor);
                assert_eq!(view.night_begin(), new_begin);
                assert_eq!(
                    view.night_end().current,
                    if boundary { map[1].add(1) } else { map[0].add(21) }
                );
                assert_eq!(first_block[20], if boundary { 0xCCCC_CCCC } else { building as u32 });
                assert_eq!(view.lit(), lit);
                finished();
            }
        }
    }
}

#[test]
fn failed_night_query_still_releases_nonnull_output_and_retains_full_return_words() {
    unsafe {
        for output in [false, true] {
            let mut object = mock!(0 => query_building);
            let object = object.ptr();
            let mut building = mock!(8 => release);
            let building = building.ptr();
            let mut developer = Object::new(0x108);
            let this = developer.ptr();
            expect(
                object,
                "query_building",
                vec![w(0x87DD_A3A9), out(&[if output { building as u32 } else { 0 }])],
                RETURN & !255,
            );
            if output {
                expect(building, "release", vec![], RETURN);
            }
            assert_eq!(
                invoke(fc4p_006c8f80 as *const () as usize, this, &[object as u32]),
                if output { RETURN } else { RETURN & !255 }
            );
            finished();
        }
    }
}
