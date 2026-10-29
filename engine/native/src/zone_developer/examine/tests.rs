// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Network(i32, i32, u32),
    Lot(i32, i32, u32),
    Water(i32, i32),
    Allocate(u32),
    Collect([i32; 2], u32, u32),
    Type(u32),
    ReleaseList,
    Destroy,
}

#[derive(Default)]
struct State {
    events: Vec<Event>,
    networks: u32,
    lot: u32,
    water: u32,
    kinds: Vec<u32>,
    memory: Vec<Box<[u32]>>,
    occupant_table: usize,
}

thread_local! {static STATE: RefCell<State> = RefCell::new(State::default());}

fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}

fn allocate_words(s: &mut State, values: &[u32]) -> *mut u8 {
    let mut p = values.to_vec().into_boxed_slice();
    let out = p.as_mut_ptr().cast();
    s.memory.push(p);
    out
}

extern "thiscall" fn network_record(_this: *mut u8, x: i32, z: i32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Network(x, z, flags));
        0xAABB_CCDD_1234_5600 | u64::from(flags & s.networks != 0)
    })
}

#[unsafe(naked)]
unsafe extern "thiscall" fn network_fake(_this: *mut u8, _x: i32, _z: i32, _flags: u32) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 12]", "push dword ptr [esp + 12]", "push dword ptr [esp + 12]",
        "call {record}", "mov ecx, 0x11223344", "ret 12", record = sym network_record);
}

extern "thiscall" fn lot_fake(_this: *mut u8, x: i32, z: i32, virtual_lot: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Lot(x, z, virtual_lot));
        s.lot
    })
}

extern "thiscall" fn water_fake(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Water(x, z));
        s.water
    })
}

extern "thiscall" fn allocate_fake(_this: *mut u8, size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        allocate_words(&mut s, &[0xA5A5_A5A5; 3])
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
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events
            .push(Event::Collect(unsafe { a.cast::<[i32; 2]>().read() }, kind, flags));
        let header = unsafe { list.read() };
        assert_eq!(unsafe { header.add(8).cast::<u32>().read() }, 0xA5A5_A5A5);
        let mut previous = header;
        for kind in s.kinds.clone() {
            let table = s.occupant_table as u32;
            let occupant = allocate_words(&mut s, &[table, kind]);
            let node = allocate_words(&mut s, &[header as u32, previous as u32, occupant as u32]);
            unsafe { previous.cast::<*mut u8>().write(node) };
            previous = node;
        }
        unsafe { header.add(4).cast::<*mut u8>().write(previous) };
    });
}

extern "thiscall" fn type_fake(this: *mut u8) -> u32 {
    let kind = unsafe { this.add(4).cast::<u32>().read() };
    STATE.with(|s| s.borrow_mut().events.push(Event::Type(kind)));
    kind
}

extern "thiscall" fn release_list_fake(_this: *mut u8, _list: *mut *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::ReleaseList));
}

extern "thiscall" fn destroy_record(_list: *mut *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Destroy));
}

#[unsafe(naked)]
unsafe extern "thiscall" fn destroy_fake(_list: *mut *mut u8) -> u64 {
    core::arch::naked_asm!("call {record}","mov eax, 0x87654321", "mov edx, 0xAABBCCDD", "mov ecx, 0xCAFEBABE", "ret",record=sym destroy_record);
}

struct Fixture {
    dev: Object,
    _objects: Vec<Object>,
    _tables: Vec<Box<[usize]>>,
    _allocator: Box<usize>,
    _fakes: Vec<Fake>,
}

impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let slots = [
            vec![
                (0x20, network_fake as *const () as usize),
                (0x24, water_fake as *const () as usize),
            ],
            vec![(0x18, lot_fake as *const () as usize)],
            vec![(0x0C, allocate_fake as *const () as usize)],
            vec![
                (0xA4, collect_fake as *const () as usize),
                (0xC0, release_list_fake as *const () as usize),
            ],
            vec![(0x1C, type_fake as *const () as usize)],
        ];
        let mut tables = vec![];
        let mut objects = vec![];
        for slots in slots {
            let mut table = vec![0usize; 0xC4 / 4].into_boxed_slice();
            for (offset, f) in slots {
                table[offset / 4] = f;
            }
            let mut object = Object::new(8);
            unsafe { object.ptr().cast::<usize>().write(table.as_ptr() as usize) };
            tables.push(table);
            objects.push(object);
        }
        STATE.with(|s| s.borrow_mut().occupant_table = tables[4].as_ptr() as usize);
        let mut dev = Object::new(0xF0);
        let view = unsafe { ZoneDeveloper::from_ptr(dev.ptr()) };
        view.set_primary_vtable(tables[0].as_ptr() as u32);
        view.set_lot_manager_vtable(tables[1].as_ptr() as u32);
        view.set_occupant_manager(objects[3].ptr());
        let mut allocator = Box::new(objects[2].ptr() as usize);
        let fakes = vec![
            fake_data(0x00B2_07E4, (&mut *allocator as *mut usize).cast()),
            fake(0x0041_3FE0, destroy_fake as *const () as usize),
        ];
        Self {
            dev,
            _objects: objects,
            _tables: tables,
            _allocator: allocator,
            _fakes: fakes,
        }
    }

    fn examine(&mut self, x: i32, z: i32, condition: &mut i32, score: &mut i32) -> RawCall {
        let args = [
            x as u32,
            z as u32,
            condition as *mut i32 as u32,
            score as *mut i32 as u32,
        ];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_0072fab0 as *const () as usize,
            self.dev.ptr() as u32,
            &args,
        );
        unsafe { registers::call(&mut call) };
        assert_eq!(call.edx, condition as *mut i32 as u32);
        call
    }
}

#[test]
fn boundary_network_priority_scores_conditions_and_final_ecx() {
    let mut f = Fixture::new();
    let flags = [0x104, 0x808, 0x401, 0x40, 2];
    for (network, increment, blocked, index) in [
        (0x104, 5, true, 0),
        (0x808, 30, false, 1),
        (0x401, 100, false, 2),
        (0x40, 400, false, 3),
        (2, 5, true, 4),
    ] {
        for original_condition in [-7, 0, 1, 2, 9] {
            STATE.with(|s| s.borrow_mut().networks = network);
            let (mut condition, mut score) = (original_condition, i32::MAX - 2);
            let result = f.examine(10, 9, &mut condition, &mut score);
            assert_eq!(condition, if blocked { 2 } else { original_condition.max(1) });
            assert_eq!(score, (i32::MAX - 2).wrapping_add(increment));
            assert_eq!(result.eax, score as u32);
            assert_eq!(
                result.ecx,
                if index == 4 {
                    &mut condition as *mut i32 as u32
                } else {
                    0x1122_3344
                }
            );
            let mut expected = vec![Event::Network(10, 9, 0x1F4F)];
            expected.extend(flags[..=index].iter().map(|&flag| Event::Network(10, 9, flag)));
            assert_eq!(events(), expected);
        }
    }
    // Roads on three north-edge cells and streets on three east-edge cells from the boundary fixture.
    let mut conditions = [0i32; 4];
    let mut scores = [0i32; 4];
    STATE.with(|s| s.borrow_mut().networks = 1);
    for x in 10..=12 {
        f.examine(x, 9, &mut conditions[0], &mut scores[0]);
    }
    STATE.with(|s| s.borrow_mut().networks = 8);
    for z in 10..=12 {
        f.examine(13, z, &mut conditions[1], &mut scores[1]);
    }
    STATE.with(|s| s.borrow_mut().networks = 0);
    for x in 10..=12 {
        f.examine(x, 13, &mut conditions[2], &mut scores[2]);
    }
    for z in 10..=12 {
        f.examine(9, z, &mut conditions[3], &mut scores[3]);
    }
    assert_eq!(scores, [300, 90, 3, 3]);
    assert_eq!(conditions, [1, 1, 0, 0]);
}

#[test]
fn unclassified_network_only_adds_the_zero_condition_bonus() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().networks = 0x200);
    for initial in [-1, 0, 1, 2] {
        let (mut condition, mut score) = (initial, i32::MAX);
        let result = f.examine(-3, 6, &mut condition, &mut score);
        assert_eq!(condition, initial);
        assert_eq!(score, if initial == 0 { i32::MIN } else { i32::MAX });
        assert_eq!(result.eax, 0x1234_5600);
        assert_eq!(result.ecx, 0x1122_3344);
        assert_eq!(
            events(),
            [0x1F4F, 0x104, 0x808, 0x401, 0x40, 2].map(|flag| Event::Network(-3, 6, flag))
        );
    }
}

#[test]
fn real_lot_short_circuits_water_and_water_short_circuits_occupants() {
    let mut f = Fixture::new();
    for (lot, water) in [(0x100, 0), (0, 0x1234_5601)] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.lot = lot;
            s.water = water;
        });
        let (mut condition, mut score) = (-1, 7);
        let result = f.examine(3, 4, &mut condition, &mut score);
        assert_eq!((condition, score), (2, 12));
        assert_eq!(result.ecx, &mut condition as *mut i32 as u32);
        let mut expected = vec![Event::Network(3, 4, 0x1F4F), Event::Lot(3, 4, 0)];
        if lot == 0 {
            expected.push(Event::Water(3, 4));
        }
        assert_eq!(events(), expected);
    }
}

#[test]
fn occupant_scan_counts_every_building_and_power_pole_and_always_destroys_the_list() {
    let mut f = Fixture::new();
    for kinds in [
        vec![],
        vec![0x1234],
        vec![0x2781_28A0, 0x1234, 0x2890_D4DE, 0x2781_28A0],
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.kinds = kinds.clone();
            s.water = 0x100;
        });
        let (mut condition, mut score) = (0, i32::MAX - 4);
        let result = f.examine(i32::MIN, i32::MAX, &mut condition, &mut score);
        let count = kinds.iter().filter(|&&k| k == 0x2781_28A0 || k == 0x2890_D4DE).count() as i32;
        assert_eq!(condition, if count != 0 { 2 } else { 0 });
        assert_eq!(
            score,
            (i32::MAX - 4).wrapping_add(if count != 0 { 5 * count } else { 1 })
        );
        assert_eq!((result.eax, result.ecx), (0x8765_4321, 0xCAFE_BABE));
        let mut expected = vec![
            Event::Network(i32::MIN, i32::MAX, 0x1F4F),
            Event::Lot(i32::MIN, i32::MAX, 0),
            Event::Water(i32::MIN, i32::MAX),
            Event::Allocate(12),
            Event::Collect([i32::MIN, i32::MAX], 0, u32::MAX),
        ];
        expected.extend(kinds.into_iter().map(Event::Type));
        expected.extend([Event::ReleaseList, Event::Destroy]);
        assert_eq!(events(), expected);
    }
}

#[test]
fn aliased_condition_and_score_are_written_in_original_order() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().networks = 0x2004);
    let mut value = 90i32;
    let p = &mut value as *mut i32;
    let args = [0, 0, p as u32, p as u32];
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_0072fab0 as *const () as usize,
        f.dev.ptr() as u32,
        &args,
    );
    unsafe { registers::call(&mut call) };
    assert_eq!(value, 7);
    assert_eq!(
        events(),
        vec![Event::Network(0, 0, 0x1F4F), Event::Network(0, 0, 0x104)]
    );
}
