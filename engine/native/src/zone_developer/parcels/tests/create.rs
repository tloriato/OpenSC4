// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Allocate(u32),
    Construct([u32; 5]),
    AddRef,
    Init,
    State(u32, u32, u32),
    Iterator,
    Insert(usize),
    AllocateNode(u32),
    City,
    Add(usize),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    lot: usize,
    iterator: usize,
    occupants: usize,
    nodes: Vec<Box<[u32]>>,
    clear_output: usize,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        s.lot as *mut u8
    })
}
extern "thiscall" fn construct_fake(this: *mut u8, x: i32, z: i32, w: i32, h: i32, f: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.lot);
        s.events
            .push(Event::Construct([x as u32, z as u32, w as u32, h as u32, f]));
    });
    this
}
extern "thiscall" fn add_ref_fake(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.lot);
        s.events.push(Event::AddRef);
    });
}
extern "thiscall" fn init_fake(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Init));
}
extern "thiscall" fn state_fake(_this: *mut u8, a: u32, b: u32, c: u32) {
    STATE.with(|s| s.borrow_mut().events.push(Event::State(a, b, c)));
}
extern "thiscall" fn iterator_fake(_this: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Iterator);
        s.iterator as *mut u8
    })
}
extern "thiscall" fn insert_fake(this: *mut u8, out: *mut *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.iterator);
        assert_eq!(unsafe { out.read() } as usize, s.lot);
        s.events.push(Event::Insert(this as usize));
    });
}
extern "C" fn node_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::AllocateNode(size));
        let mut node = vec![0xA5A5_A5A5u32; 3].into_boxed_slice();
        let p = node.as_mut_ptr().cast();
        s.nodes.push(node);
        p
    })
}
extern "thiscall" fn city_fake(_this: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::City);
        s.occupants as *mut u8
    })
}
extern "thiscall" fn add_fake(this: *mut u8, lot: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.occupants);
        assert_eq!(lot as usize, s.lot);
        s.events.push(Event::Add(lot as usize));
        if s.clear_output != 0 {
            unsafe { (s.clear_output as *mut *mut u8).write(null_mut()) };
        }
    });
    0xFEDC_BA98_AABB_CCDD
}

struct Fixture {
    developer: Object,
    lot: Object,
    _objects: Vec<Object>,
    _tables: Vec<Box<[usize]>>,
    rows: Vec<[u32; 3]>,
    columns: Vec<Vec<u32>>,
    header: [u32; 3],
    _globals: Box<[usize; 2]>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let definitions = [
            vec![
                (4, add_ref_fake as *const () as usize),
                (0x14, init_fake as *const () as usize),
                (0x88, state_fake as *const () as usize),
            ],
            vec![(0x58, iterator_fake as *const () as usize)],
            vec![(0x90, city_fake as *const () as usize)],
            vec![(0xAC, add_fake as *const () as usize)],
        ];
        let mut tables = vec![];
        let mut objects = vec![];
        for defs in definitions {
            let mut table = vec![0usize; 0xB0 / 4].into_boxed_slice();
            for (slot, f) in defs {
                table[slot / 4] = f;
            }
            let mut object = Object::new(0x90);
            unsafe { object.ptr().cast::<usize>().write(table.as_ptr() as usize) };
            tables.push(table);
            objects.push(object);
        }
        let lot = objects.remove(0);
        let mut globals = Box::new([objects[0].ptr() as usize, objects[1].ptr() as usize]);
        let fakes = vec![
            fake_data(0x00B4_3DA8, (&mut globals[0] as *mut usize).cast()),
            fake_data(0x00B4_3D68, (&mut globals[1] as *mut usize).cast()),
            fake(0x006B_F430, allocate_fake as *const () as usize),
            fake(0x006C_1E50, construct_fake as *const () as usize),
            fake(0x0090_CF54, node_fake as *const () as usize),
            fake(0x0072_C810, insert_fake as *const () as usize),
        ];
        let mut f = Self {
            developer: Object::new(0xF0),
            lot,
            _objects: objects,
            _tables: tables,
            rows: vec![[0; 3]; 8],
            columns: vec![vec![0xCCCC_CCCC; 8]; 8],
            header: [0, 0, 0xA5A5_A5A5],
            _globals: globals,
            _fakes: fakes,
        };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.lot = f.lot.ptr() as usize;
            s.occupants = f._objects[2].ptr() as usize;
        });
        for (row, column) in f.rows.iter_mut().zip(f.columns.iter_mut()) {
            *row = [column.as_mut_ptr() as u32, 0x1111_1111, 0x2222_2222];
        }
        let manager = unsafe { LotManager::from_ptr(f.developer.ptr().add(4)) };
        manager.set_rows(f.rows.as_mut_ptr().cast());
        f
    }
    fn setup_header(&mut self) {
        let p = self.header.as_mut_ptr() as u32;
        self.header[0] = p;
        self.header[1] = p;
        let pending = unsafe { PendingLots::from_ptr(self.developer.ptr().add(4)) };
        pending.set_header(p as *mut u8);
        pending.set_count(u32::MAX);
    }
}

#[test]
fn create_registers_exact_rectangle_references_and_local_list_writes() {
    let mut f = Fixture::new();
    f.setup_header();
    let mut out = null_mut();
    assert_eq!(
        unsafe { fc4p_0072e520(f.developer.ptr().add(4), 2, 3, 3, 2, 0x1234_5602, &mut out) },
        0xFEDC_BA98_0000_0001
    );
    assert_eq!(out, f.lot.ptr());
    for x in 0..8 {
        for z in 0..8 {
            assert_eq!(
                f.columns[x][z],
                if (2..=4).contains(&x) && (3..=4).contains(&z) {
                    out as u32
                } else {
                    0xCCCC_CCCC
                }
            );
        }
    }
    assert!(f.rows.iter().all(|r| r[1..] == [0x1111_1111, 0x2222_2222]));
    let node = f.header[0] as *const u32;
    assert_eq!(f.header[1], node as u32);
    assert_eq!(f.header[2], 0xA5A5_A5A5);
    assert_eq!(
        unsafe { node.cast::<[u32; 3]>().read() },
        [f.header.as_ptr() as u32, f.header.as_ptr() as u32, out as u32]
    );
    assert_eq!(unsafe { PendingLots::from_ptr(f.developer.ptr().add(4)) }.count(), 0);
    assert_eq!(
        events(),
        vec![
            Event::Allocate(0x8C),
            Event::Construct([2, 3, 3, 2, 0x1234_5602]),
            Event::AddRef,
            Event::Init,
            Event::State(1, 0, 0),
            Event::AddRef,
            Event::Iterator,
            Event::AllocateNode(12),
            Event::City,
            Event::Add(out as usize)
        ]
    );
}

#[test]
fn create_nonpositive_dimensions_cover_one_cell_and_iterator_receives_the_output_slot() {
    for (width, height) in [(0, 0), (-2, -3), (1, 1)] {
        let mut f = Fixture::new();
        f.setup_header();
        STATE.with(|s| s.borrow_mut().iterator = 0x1234_5678);
        let mut out = null_mut();
        let result = unsafe { fc4p_0072e520(f.developer.ptr().add(4), 2, 3, width, height, 3, &mut out) };
        assert_eq!(result, 0xFEDC_BA98_0000_0001);
        assert_eq!(f.columns.iter().flatten().filter(|&&v| v == out as u32).count(), 1);
        assert_eq!(f.columns[2][3], out as u32);
        assert_eq!(
            unsafe { PendingLots::from_ptr(f.developer.ptr().add(4)) }.count(),
            u32::MAX
        );
        assert_eq!(
            events(),
            vec![
                Event::Allocate(0x8C),
                Event::Construct([2, 3, width as u32, height as u32, 3]),
                Event::AddRef,
                Event::Init,
                Event::State(1, 0, 0),
                Event::AddRef,
                Event::Iterator,
                Event::Insert(0x1234_5678),
                Event::City,
                Event::Add(out as usize)
            ]
        );
    }
}

#[test]
fn create_returns_the_output_slots_final_nullness_after_occupant_registration() {
    let mut f = Fixture::new();
    f.setup_header();
    let mut out = null_mut();
    STATE.with(|s| s.borrow_mut().clear_output = &mut out as *mut *mut u8 as usize);
    let result = unsafe { fc4p_0072e520(f.developer.ptr().add(4), 0, 0, 1, 1, 0, &mut out) };
    assert_eq!(result, 0xFEDC_BA98_0000_0000);
    assert!(out.is_null());
    assert_eq!(f.columns[0][0], f.lot.ptr() as u32);
}
