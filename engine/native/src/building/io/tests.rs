// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Error,
    U16(u32),
    U32(u32),
    I32(u32),
    U8(u32),
    Reference(u32),
    Bool(u32),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    errors: Vec<u64>,
    streams: Vec<usize>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn event(this: *mut u8, value: Event) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(value);
        s.streams.push(this as usize);
    });
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn error(this: *mut u8) -> u64 {
    event(this, Event::Error);
    STATE.with(|s| s.borrow_mut().errors.remove(0))
}
extern "thiscall" fn u16_word(this: *mut u8, word: u32) -> u64 {
    event(this, Event::U16(word));
    0
}
extern "thiscall" fn u32_word(this: *mut u8, word: u32) -> u64 {
    event(this, Event::U32(word));
    0xFFFF_FFFF
}
extern "thiscall" fn i32_word(this: *mut u8, word: u32) -> u64 {
    event(this, Event::I32(word));
    0
}
extern "thiscall" fn u8_word(this: *mut u8, word: u32) -> u64 {
    event(this, Event::U8(word));
    0
}
extern "C" fn reference(stream: *mut u8, object: *mut u8) -> u64 {
    event(stream, Event::Reference(object as u32));
    0
}
extern "C" fn boolean(stream: *mut u8, word: u32) -> u64 {
    event(stream, Event::Bool(word));
    0
}

struct World {
    sim: Object,
    memory: Vec<Object>,
    table: Box<[usize]>,
    stream: Object,
    marks: Box<(*mut u8, u32)>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                errors: vec![0x1234_5678_99AA_BB00, 0x8765_4321_CCDD_EE00],
                ..State::default()
            }
        });
        let mut w = Self {
            sim: Object::new(0xE4),
            memory: vec![],
            table: vec![0; 19].into_boxed_slice(),
            stream: Object::new(4),
            marks: Box::new((core::ptr::null_mut(), 0)),
        };
        w.table[0x48 / 4] = error as *const () as usize;
        w.table[0x1C / 4] = u16_word as *const () as usize;
        w.table[0x24 / 4] = u32_word as *const () as usize;
        w.table[0x20 / 4] = i32_word as *const () as usize;
        w.table[0x14 / 4] = u8_word as *const () as usize;
        unsafe { w.stream.ptr().cast::<*const usize>().write(w.table.as_ptr()) };
        let dev = w.tree(&[]);
        let stages = w.tree(&[]);
        let marks = w.tree(&[]);
        let special = w.list(&[]);
        w.marks.0 = marks;
        let sim = unsafe { SaveView::from_ptr(w.sim.ptr().wrapping_add(12)) };
        sim.set_developers(dev);
        sim.set_stages(stages);
        sim.set_special(special);
        w
    }
    fn object(&mut self, size: usize) -> *mut u8 {
        let mut object = Object::new(size);
        let ptr = object.ptr();
        self.memory.push(object);
        ptr
    }
    fn tree(&mut self, items: &[(u32, u32)]) -> *mut u8 {
        let header = self.object(28);
        let pointers: Vec<_> = items.iter().map(|_| self.object(28)).collect();
        fn build(pointers: &[*mut u8], items: &[(u32, u32)], parent: *mut u8) -> *mut u8 {
            if pointers.is_empty() {
                return core::ptr::null_mut();
            }
            let middle = pointers.len() / 2;
            let n = unsafe { SaveNode::from_ptr(pointers[middle]) };
            n.set_parent(parent);
            n.set_key(items[middle].0);
            n.set_value(items[middle].1);
            n.set_left(build(&pointers[..middle], &items[..middle], n.as_ptr()));
            n.set_right(build(&pointers[middle + 1..], &items[middle + 1..], n.as_ptr()));
            n.as_ptr()
        }
        let h = unsafe { SaveNode::from_ptr(header) };
        h.set_parent(build(&pointers, items, header));
        h.set_left(pointers.first().copied().unwrap_or(header));
        h.set_right(pointers.last().copied().unwrap_or(header));
        header
    }
    fn list(&mut self, objects: &[u32]) -> *mut u8 {
        let header = self.object(12);
        let mut previous = header;
        for &object in objects {
            let node = self.object(12);
            unsafe {
                previous.cast::<*mut u8>().write(node);
                node.add(4).cast::<*mut u8>().write(previous);
                node.add(8).cast::<u32>().write(object);
            }
            previous = node;
        }
        unsafe {
            previous.cast::<*mut u8>().write(header);
            header.add(4).cast::<*mut u8>().write(previous);
        }
        header
    }
    fn run(&mut self) -> u64 {
        let _refs = fake(0x005D_99A0, reference as *const () as usize);
        let _bool = fake(0x005D_9FD0, boolean as *const () as usize);
        let _header = fake_data(0x00B4_85E0, (&mut self.marks.0 as *mut *mut u8).cast());
        let _count = fake_data(0x00B4_85E4, (&mut self.marks.1 as *mut u32).cast());
        unsafe { fc4p_00697800(self.sim.ptr().add(12), 0xFACE_B00C, self.stream.ptr()) }
    }
}

#[test]
fn write_empty_record_and_initial_or_final_errors() {
    for (initial, final_error, expected) in [(0, 0, 1), (0, 0x80, 0), (0x80, 0, 0)] {
        let mut w = World::new();
        STATE.with(|s| {
            s.borrow_mut().errors = vec![0x1234_5678_AABB_CC00 | initial, 0x9876_5432_1122_3300 | final_error]
        });
        let result = w.run();
        if initial != 0 {
            assert_eq!(result, 0x1234_5678_AABB_CC00);
            assert_eq!(events(), [Event::Error]);
        } else {
            assert_eq!(result, 0x9876_5432_1122_3300 | expected);
            assert_eq!(
                events(),
                [
                    Event::Error,
                    Event::U16(4),
                    Event::Reference(0),
                    Event::Reference(0),
                    Event::U32(0),
                    Event::U32(0),
                    Event::U32(0),
                    Event::U32(0),
                    Event::U32(0),
                    Event::Bool(0),
                    Event::Bool(0),
                    Event::Bool(0),
                    Event::Error
                ]
            );
        }
        assert!(STATE.with(|s| s.borrow().streams.iter().all(|&p| p == w.stream.ptr() as usize)));
    }
}

#[test]
fn write_orders_tree_keys_list_references_marks_hash_chains_and_raw_cheats() {
    let mut w = World::new();
    let devs = w.tree(&[(0x1010, 0x1000), (0x3110, 0), (0x4400, 0x3000)]);
    let stages = w.tree(&[(0x1010, 3), (0x3110, 0x80), (0x4400, 0xFF)]);
    let special = w.list(&[0x4000, 0, 0x5000]);
    let cells = w.tree(&[(0x8000_0000, 0xFFFF_FFFF), (1, 0xFFFF_FFFE), (2, 67)]);
    let marks = w.tree(&[(0x1010, cells as u32), (0x4400, w.marks.0 as u32)]);
    let marks_first = unsafe { SaveNode::from_ptr(SaveNode::from_ptr(marks).left()) };
    marks_first.set_count(3);
    w.marks.0 = marks;
    w.marks.1 = 2;
    let plan_a = w.object(24);
    let plan_b = w.object(24);
    let plan_c = w.object(24);
    // Four buckets: the low two hash bits are the stage. Bucket 1 has a collision chain, bucket 3 another item.
    for (p, next, dev, x, z, stage, day) in [
        (plan_a, plan_b, 0x6000u32, 65u32, 0xFFFF_FFFEu32, 0x81u8, 5000u32),
        (plan_b, core::ptr::null_mut(), 0u32, 2u32, 3u32, 1u8, 5001u32),
        (plan_c, core::ptr::null_mut(), 0x7000u32, 4u32, 5u32, 3u8, u32::MAX),
    ] {
        unsafe {
            p.cast::<*mut u8>().write(next);
            p.add(4).cast::<u32>().write(dev);
            p.add(8).cast::<u32>().write(x);
            p.add(12).cast::<u32>().write(z);
            *p.add(16) = stage;
            p.add(20).cast::<u32>().write(day);
        }
    }
    let mut buckets = [core::ptr::null_mut(), plan_a, core::ptr::null_mut(), plan_c];
    let sim = unsafe { SaveView::from_ptr(w.sim.ptr().add(12)) };
    sim.set_grid(0x8000usize as *mut u8);
    sim.set_construction(0x9000usize as *mut u8);
    sim.set_developers(devs);
    sim.set_developer_count(3);
    sim.set_stages(stages);
    sim.set_stage_count(3);
    sim.set_special(special);
    sim.set_plans(buckets.as_mut_ptr());
    sim.set_plans_end(buckets.as_mut_ptr().wrapping_add(4));
    sim.set_plan_count(3);
    sim.set_ignore_plumbing(0x80);
    sim.set_ignore_power(2);
    sim.set_ignore_caps(0xFF);
    let before = unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0xE4) }.to_vec();
    assert_eq!(w.run(), 0x8765_4321_CCDD_EE01);
    let upper_stream = w.stream.ptr() as u32 & !255;
    assert_eq!(
        events(),
        [
            Event::Error,
            Event::U16(4),
            Event::Reference(0x8000),
            Event::Reference(0x9000),
            Event::U32(3),
            Event::U32(0x1010),
            Event::Reference(0x1000),
            Event::U32(0x3110),
            Event::Reference(0),
            Event::U32(0x4400),
            Event::Reference(0x3000),
            Event::U32(3),
            Event::Reference(0x4000),
            Event::Reference(0),
            Event::Reference(0x5000),
            Event::U32(3),
            Event::U32(0x1010),
            Event::U8(upper_stream | 3),
            Event::U32(0x3110),
            Event::U8(upper_stream | 0x80),
            Event::U32(0x4400),
            Event::U8(upper_stream | 0xFF),
            Event::U32(2),
            Event::U32(0x1010),
            Event::U32(3),
            Event::I32(0x8000_0000),
            Event::I32(u32::MAX),
            Event::I32(1),
            Event::I32(0xFFFF_FFFE),
            Event::I32(2),
            Event::I32(67),
            Event::U32(0x4400),
            Event::U32(0),
            Event::U32(3),
            Event::Reference(0x6000),
            Event::I32(65),
            Event::I32(0xFFFF_FFFE),
            Event::U8(0x81),
            Event::U32(5000),
            Event::Reference(0),
            Event::I32(2),
            Event::I32(3),
            Event::U8(1),
            Event::U32(5001),
            Event::Reference(0x7000),
            Event::I32(4),
            Event::I32(5),
            Event::U8(3),
            Event::U32(u32::MAX),
            Event::Bool(0x80),
            Event::Bool(2),
            Event::Bool(0xFF),
            Event::Error
        ]
    );
    assert_eq!(unsafe { core::slice::from_raw_parts(w.sim.ptr(), 0xE4) }, before);
}
