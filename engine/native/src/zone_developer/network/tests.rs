// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

const CALL_EDX: u32 = 0xFEDC_BA98;
const FREE_ECX: u32 = 0x1234_5678;

#[derive(Debug, PartialEq)]
enum Event {
    Framework,
    Service(u32, u32),
    DropTemporary(usize),
    Allocate(u32),
    Collect([i32; 2], u32, u32),
    Query(u32, u32),
    Flags(u32, u32),
    Release(u32),
    ReleaseList,
    Clear,
    Free,
    Get(i32, i32, u32),
    Draft(u32, i32, i32, u32),
    Connect(u32, u32),
}

#[derive(Default)]
struct State {
    events: Vec<Event>,
    allocations: Vec<Box<[u32]>>,
    occupants: Vec<usize>,
    networks: Vec<usize>,
    query_results: Vec<u32>,
    flag_results: Vec<u32>,
    allocator: usize,
    framework: usize,
    cache: usize,
    reset_cache: bool,
    expected_free_edx: u32,
    get_result: u64,
    connection: u32,
    street: Option<u32>,
    dirt: Option<u32>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}

fn words(s: &mut State, values: &[u32]) -> *mut u8 {
    let mut allocation = values.to_vec().into_boxed_slice();
    let p = allocation.as_mut_ptr().cast();
    s.allocations.push(allocation);
    p
}

extern "C" fn framework_fake() -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Framework);
        s.framework as *mut u8
    })
}

extern "thiscall" fn service_fake(_this: *mut u8, cls: u32, iid: u32, out: *mut *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Service(cls, iid));
        unsafe { out.write(s.allocator as *mut u8) };
    });
    1
}

extern "fastcall" fn temporary_fake(slot: *mut *mut u8, edx: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let reference = unsafe { slot.read() };
        assert_eq!(edx, reference as u32);
        assert_eq!(unsafe { (s.cache as *const usize).read() }, reference as usize);
        s.events.push(Event::DropTemporary(reference as usize));
    });
    0x1122_3344_5566_7788
}

extern "thiscall" fn allocate_fake(_this: *mut u8, size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        words(&mut s, &[0xA5A5_A5A5; 3])
    })
}

extern "thiscall" fn collect_fake(
    _this: *mut u8,
    list: *mut *mut u8,
    a: *const i32,
    b: *const i32,
    kind: u32,
    flags: u32,
) {
    assert_eq!(a, b);
    let cell = unsafe { a.cast::<[i32; 2]>().read_unaligned() };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Collect(cell, kind, flags));
        let header = unsafe { list.read() };
        assert_eq!(unsafe { header.cast::<*mut u8>().read() }, header);
        assert_eq!(unsafe { header.add(4).cast::<*mut u8>().read() }, header);
        assert_eq!(unsafe { header.add(8).cast::<u32>().read() }, 0xA5A5_A5A5);
        let mut previous = header;
        for occupant in s.occupants.clone() {
            let node = words(&mut s, &[header as u32, previous as u32, occupant as u32]);
            unsafe { previous.cast::<*mut u8>().write(node) };
            previous = node;
        }
        unsafe { header.add(4).cast::<*mut u8>().write(previous) };
    });
}

fn id(this: *mut u8) -> u32 {
    unsafe { this.add(4).cast::<u32>().read() }
}

extern "thiscall" fn query_fake(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
    assert!(unsafe { out.read() }.is_null());
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Query(id, iid));
        unsafe { out.write(s.networks[id as usize] as *mut u8) };
        s.query_results[id as usize]
    })
}

extern "thiscall" fn flags_fake(this: *mut u8, flags: u32) -> u32 {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Flags(id, flags));
        s.flag_results[id as usize]
    })
}

extern "thiscall" fn release_fake(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Release(id(this))));
}

extern "thiscall" fn release_list_fake(_this: *mut u8, _list: *mut *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::ReleaseList));
}

extern "thiscall" fn clear_fake(list: *mut *mut u8) -> u64 {
    let header = unsafe { list.read() };
    unsafe {
        header.cast::<*mut u8>().write(header);
        header.add(4).cast::<*mut u8>().write(header);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Clear);
        if s.reset_cache {
            unsafe { (s.cache as *mut usize).write(0) };
        }
    });
    0x1122_3344_0000_0000
}

extern "fastcall" fn free_record(_this: *mut u8, edx: u32, node: *mut u8) {
    assert_eq!(unsafe { node.cast::<*mut u8>().read() }, node);
    assert_eq!(unsafe { node.add(4).cast::<*mut u8>().read() }, node);
    assert_eq!(unsafe { node.add(8).cast::<u32>().read() }, 0xA5A5_A5A5);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(edx, s.expected_free_edx);
        s.events.push(Event::Free);
    });
}

#[unsafe(naked)]
unsafe extern "thiscall" fn free_fake(_this: *mut u8, _node: *mut u8) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {record}",
        "mov eax, 0x76543210", "mov edx, 0xFEDCBA98", "mov ecx, 0x12345678", "ret 4", record = sym free_record);
}

extern "thiscall" fn get_fake(_this: *mut u8, x: i32, z: i32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Get(x, z, flags));
        s.get_result
    })
}

extern "thiscall" fn draft_fake(this: *mut u8, x: i32, z: i32, flag: u32, out: *mut u32) -> u64 {
    let tool = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Draft(tool, x, z, flag));
        let mask = if tool == 3 { s.street } else { s.dirt };
        if let Some(mask) = mask {
            unsafe { out.write(mask) };
        }
        (u64::from(CALL_EDX) << 32) | 0xAABB_CC00 | u64::from(mask.is_some())
    })
}

extern "thiscall" fn connect_fake(_this: *mut u8, edge: u32, kind: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Connect(edge, kind));
        (u64::from(CALL_EDX) << 32) | u64::from(s.connection)
    })
}

struct Fixture {
    developer: Object,
    _objects: Vec<Object>,
    _tables: Vec<Box<[usize]>>,
    _cache: Box<usize>,
    _fakes: Vec<Fake>,
}

impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut tables = vec![];
        let mut objects = vec![];
        for (tag, slots) in [
            (
                0,
                vec![
                    (0x0C, allocate_fake as *const () as usize),
                    (0x10, free_fake as *const () as usize),
                ],
            ),
            (0, vec![(0x14, service_fake as *const () as usize)]),
            (
                0,
                vec![
                    (0xA4, collect_fake as *const () as usize),
                    (0xC0, release_list_fake as *const () as usize),
                ],
            ),
            (3, vec![(0x24, draft_fake as *const () as usize)]),
            (11, vec![(0x24, draft_fake as *const () as usize)]),
            (0, vec![(0xBC, connect_fake as *const () as usize)]),
        ] {
            let mut table = vec![0usize; 0xC4 / 4].into_boxed_slice();
            for (offset, f) in slots {
                table[offset / 4] = f;
            }
            let mut object = Object::new(8);
            unsafe {
                object.ptr().cast::<usize>().write(table.as_ptr() as usize);
                object.ptr().add(4).cast::<u32>().write(tag);
            }
            objects.push(object);
            tables.push(table);
        }
        let mut cache = Box::new(objects[0].ptr() as usize);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.expected_free_edx = 0x1122_3344;
            s.allocator = objects[0].ptr() as usize;
            s.framework = objects[1].ptr() as usize;
            s.cache = &mut *cache as *mut usize as usize;
        });
        let mut developer = Object::new(0xF0);
        let dev = unsafe { ZoneDeveloper::from_ptr(developer.ptr()) };
        dev.set_occupant_manager(objects[2].ptr());
        dev.set_street_tool(objects[3].ptr());
        dev.set_dirt_tool(objects[4].ptr());
        let fakes = vec![
            fake_data(0x00B2_07E4, (&mut *cache as *mut usize).cast()),
            fake(0x0087_93EC, framework_fake as *const () as usize),
            fake(0x0041_1870, temporary_fake as *const () as usize),
            fake(0x0041_3E60, clear_fake as *const () as usize),
        ];
        Self {
            developer,
            _objects: objects,
            _tables: tables,
            _cache: cache,
            _fakes: fakes,
        }
    }

    fn occupants(&mut self, query: &[u32], flags: &[u32], null: &[bool]) {
        let mut occupant_table = vec![0usize; 1].into_boxed_slice();
        occupant_table[0] = query_fake as *const () as usize;
        let mut network_table = vec![0usize; 10].into_boxed_slice();
        network_table[2] = release_fake as *const () as usize;
        network_table[9] = flags_fake as *const () as usize;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.query_results = query.to_vec();
            s.flag_results = flags.to_vec();
            for (i, &null) in null.iter().enumerate() {
                let o = words(&mut s, &[occupant_table.as_ptr() as u32, i as u32]);
                let n = words(&mut s, &[network_table.as_ptr() as u32, i as u32]);
                s.occupants.push(o as usize);
                s.networks.push(if null { 0 } else { n as usize });
            }
        });
        self._tables.extend([occupant_table, network_table]);
    }
}

#[test]
fn lookup_queries_in_order_releases_the_match_and_keeps_last_free_registers() {
    let mut f = Fixture::new();
    f.occupants(
        &[0x100, 0x101, 1, 1],
        &[0, 0x100, 0x101, 1],
        &[true, false, false, false],
    );
    let expected = STATE.with(|s| s.borrow().networks[2] as u32);
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_0072fca0 as *const () as usize,
        f.developer.ptr() as u32,
        &[5, 7, 0xC09],
    );
    unsafe { registers::call(&mut call) };
    assert_eq!((call.eax, call.edx, call.ecx), (expected, CALL_EDX, FREE_ECX));
    assert_eq!(
        events(),
        vec![
            Event::Allocate(12),
            Event::Collect([5, 7], 0xC772_BF98, u32::MAX),
            Event::Query(0, 0xA821_EF94),
            Event::Query(1, 0xA821_EF94),
            Event::Flags(1, 0xC09),
            Event::Release(1),
            Event::Query(2, 0xA821_EF94),
            Event::Flags(2, 0xC09),
            Event::Release(2),
            Event::ReleaseList,
            Event::Clear,
            Event::Free,
        ]
    );
}

#[test]
fn lookup_empty_and_failed_interfaces_still_release_the_list_and_initialize_cache() {
    for populated in [false, true] {
        let mut f = Fixture::new();
        *f._cache = 0;
        if populated {
            f.occupants(&[0x100, 0x101], &[0, 0x100], &[false, false]);
        }
        STATE.with(|s| s.borrow_mut().reset_cache = true);
        let allocator = STATE.with(|s| s.borrow().allocator);
        let result = unsafe {
            crate::testing::call_thiscall_with_eax(
                fc4p_0072fca0 as *const () as usize,
                f.developer.ptr(),
                &[u32::MAX, 0, 0],
                0xFFFF_FFFF,
            )
        };
        assert_eq!(result, (0, CALL_EDX));
        let log = events();
        assert_eq!(
            &log[..4],
            &[
                Event::Framework,
                Event::Service(0x3AE4_BEA3, 0x3AE4_BEAB),
                Event::DropTemporary(allocator),
                Event::Allocate(12)
            ]
        );
        assert_eq!(
            &log[log.len() - 6..],
            &[
                Event::ReleaseList,
                Event::Clear,
                Event::Framework,
                Event::Service(0x3AE4_BEA3, 0x3AE4_BEAB),
                Event::DropTemporary(allocator),
                Event::Free
            ]
        );
        if populated {
            assert_eq!(
                &log[5..10],
                &[
                    Event::Query(0, 0xA821_EF94),
                    Event::Release(0),
                    Event::Query(1, 0xA821_EF94),
                    Event::Flags(1, 0),
                    Event::Release(1)
                ]
            );
        }
    }
}

#[test]
fn draft_network_queries_preserve_street_precedence_and_register_bytes() {
    let mut f = Fixture::new();
    let _get = fake(0x0072_FCA0, get_fake as *const () as usize);
    let dev = unsafe { ZoneDeveloper::from_ptr(f.developer.ptr()) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.street = Some(0x0001_0001);
        s.dirt = Some(0x0100_0100);
    });
    for (flags, tool) in [(0x808, 3), (0x800, 11), (0x1F4F, 3)] {
        let result = unsafe { fc4p_0072fe10(dev.as_ptr(), 5, 5, flags) };
        assert_eq!(result, (u64::from(CALL_EDX) << 32) | 0xAABB_CC01);
        assert_eq!(
            events(),
            vec![
                Event::Get(5, 5, flags),
                Event::Draft(tool, 5, 5, if tool == 3 { 8 } else { 0x800 })
            ]
        );
    }
    STATE.with(|s| s.borrow_mut().street = None);
    assert_eq!(unsafe { fc4p_0072fe10(dev.as_ptr(), 6, 6, 0x808) } as u32, 0xAABB_CC00);
    assert_eq!(events(), vec![Event::Get(6, 6, 0x808), Event::Draft(3, 6, 6, 8)]);
    for real in [0, 1, 0x80] {
        dev.set_real_mode(real);
        assert_eq!(unsafe { fc4p_0072fe10(dev.as_ptr(), 5, 5, 1) }, 0);
        assert_eq!(events(), vec![Event::Get(5, 5, 1)]);
        if real != 0 {
            assert_eq!(unsafe { fc4p_0072fe10(dev.as_ptr(), 5, 5, 0x808) }, 0);
            assert_eq!(events(), vec![Event::Get(5, 5, 0x808)]);
        }
    }
    STATE.with(|s| s.borrow_mut().get_result = 0xDEAD_BEEF_1234_56F0);
    assert_eq!(
        unsafe { fc4p_0072fe10(dev.as_ptr(), 1, 2, 0x808) },
        0xDEAD_BEEF_1234_5601
    );
    assert_eq!(events(), vec![Event::Get(1, 2, 0x808)]);
}

#[test]
fn draft_connections_rotate_sides_and_test_street_then_dirt_even_with_only_one_flag() {
    let mut f = Fixture::new();
    let _get = fake(0x0072_FCA0, get_fake as *const () as usize);
    let p = f.developer.ptr();
    for (street, dirt, flags) in [
        (Some(0x0001_0001), Some(0x0100_0100), 0x800),
        (None, Some(0x0100_0100), 8),
        (Some(0), Some(u32::MAX), 0x808),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.street = street;
            s.dirt = dirt;
        });
        for side in [0u32, 1, 2, 3, u32::MAX, 4, 0x8000_0000] {
            let shift = if side <= 3 { ((side + 1) & 3) * 8 } else { 24 };
            let shifted = street.or(dirt).unwrap() >> shift;
            let result = unsafe { fc4p_0072fe80(p, 6, 6, flags, side, 11) };
            let (eax, edx) = if street.is_some() {
                (0xAABB_CC00 | u32::from(shifted as u8 != 0), shifted)
            } else {
                ((shifted & !0xFF) | u32::from(shifted as u8 != 0), CALL_EDX)
            };
            assert_eq!(result, (u64::from(edx) << 32) | u64::from(eax));
            let mut expected = vec![Event::Get(6, 6, flags), Event::Draft(3, 6, 6, 8)];
            if street.is_none() {
                expected.push(Event::Draft(11, 6, 6, 0x800));
            }
            assert_eq!(events(), expected);
        }
    }
}

#[test]
fn real_connections_short_circuit_drafts_and_preserve_edx_on_every_exit() {
    let mut f = Fixture::new();
    let _get = fake(0x0072_FCA0, get_fake as *const () as usize);
    let p = f.developer.ptr();
    let network = f._objects[5].ptr();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.get_result = (0x1234_5678u64 << 32) | network as u64;
        s.connection = 0xAABB_CC7F;
    });
    for side in [0u32, 1, 2, 3, 4, u32::MAX] {
        assert_eq!(
            unsafe { fc4p_0072fe80(p, 1, 2, 0x808, side, 99) },
            (u64::from(CALL_EDX) << 32) | 0xAABB_CC01
        );
        assert_eq!(
            events(),
            vec![
                Event::Get(1, 2, 0x808),
                Event::Connect(if side < 4 { (side + 1) & 3 } else { u32::MAX }, 99)
            ]
        );
    }
    STATE.with(|s| s.borrow_mut().connection = 0xAABB_CC00);
    let dev = unsafe { ZoneDeveloper::from_ptr(p) };
    for mode in [1, 0xFF] {
        dev.set_real_mode(mode);
        assert_eq!(
            unsafe { fc4p_0072fe80(p, 1, 2, 0x808, 0, 3) },
            (u64::from(CALL_EDX) << 32) | 0xAABB_CC00
        );
        assert_eq!(events(), vec![Event::Get(1, 2, 0x808), Event::Connect(1, 3)]);
    }
    dev.set_real_mode(0);
    STATE.with(|s| s.borrow_mut().get_result = 0x1234_5678_0000_0000);
    assert_eq!(unsafe { fc4p_0072fe80(p, 1, 2, 1, 0, 3) }, 0x1234_5678_0000_0000);
    assert_eq!(events(), vec![Event::Get(1, 2, 1)]);
    assert_eq!(
        unsafe { fc4p_0072fe80(p, 1, 2, 8, 0, 3) },
        (u64::from(CALL_EDX) << 32) | 0xAABB_CC00
    );
    assert_eq!(
        events(),
        vec![
            Event::Get(1, 2, 8),
            Event::Draft(3, 1, 2, 8),
            Event::Draft(11, 1, 2, 0x800)
        ]
    );
}

extern "thiscall" fn release_list_tagged(this: *mut u8, list: *mut *mut u8) -> u64 {
    release_list_fake(this, list);
    0xAABB_CCDD_1234_5678
}

extern "fastcall" fn clear_preserving_edx(list: *mut *mut u8, edx: u32) -> u64 {
    clear_fake(list);
    u64::from(edx) << 32
}

#[test]
fn empty_network_list_keeps_release_list_edx_through_node_cleanup_into_allocator_free() {
    let mut f = Fixture::new();
    f._tables[2][0xC0 / 4] = release_list_tagged as *const () as usize;
    let _clear = fake(0x0041_3E60, clear_preserving_edx as *const () as usize);
    STATE.with(|s| s.borrow_mut().expected_free_edx = 0xAABB_CCDD);
    assert_eq!(
        unsafe {
            crate::testing::call_thiscall_with_eax(
                fc4p_0072fca0 as *const () as usize,
                f.developer.ptr(),
                &[0, 0, 0],
                0,
            )
        },
        (0, CALL_EDX)
    );
    assert_eq!(
        events(),
        vec![
            Event::Allocate(12),
            Event::Collect([0, 0], 0xC772_BF98, u32::MAX),
            Event::ReleaseList,
            Event::Clear,
            Event::Free
        ]
    );
}
