// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Insert(usize, usize, usize, u32, u32),
    Unique(usize, u32),
    Previous(usize),
    Next(usize),
    Allocate(u32),
    Copy,
    Hint(usize, usize, u32),
    Delete(usize),
    Free(usize),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    previous: usize,
    next: usize,
    result: usize,
    allocation: usize,
    copied: usize,
    populated: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(event: Event) {
    STATE.with(|s| s.borrow_mut().events.push(event));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn insert(
    this: *mut u8,
    out: *mut u32,
    child: *mut u8,
    parent: *mut u8,
    source: *mut u32,
    force: u32,
) -> u64 {
    log(Event::Insert(
        this as usize,
        child as usize,
        parent as usize,
        unsafe { read(source) },
        force,
    ));
    unsafe { write(out, STATE.with(|s| s.borrow().result as u32)) };
    0x1234_5678_0000_0000 | u64::from(out as u32)
}
extern "thiscall" fn unique(this: *mut u8, out: *mut u32, source: *mut u32) -> u64 {
    log(Event::Unique(this as usize, unsafe { read(source) }));
    unsafe {
        write(out, STATE.with(|s| s.borrow().result as u32));
        *out.cast::<u8>().add(4) = 1;
    }
    0x1234_5678_0000_0000 | u64::from(out as u32)
}
extern "thiscall" fn previous(slot: *mut usize) -> u64 {
    log(Event::Previous(unsafe { read(slot) }));
    unsafe { write(slot, STATE.with(|s| s.borrow().previous)) };
    0x1234_5678_AAAA_BBBB
}
extern "thiscall" fn next(slot: *mut usize) -> u64 {
    log(Event::Next(unsafe { read(slot) }));
    unsafe { write(slot, STATE.with(|s| s.borrow().next)) };
    0x1234_5678_AAAA_BBBB
}

struct World {
    map: Object,
    header: Object,
    nodes: Vec<Object>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            map: Object::new(8),
            header: Object::new(28),
            nodes: (0..3).map(|_| Object::new(28)).collect(),
        };
        let map = unsafe { Tree::from_ptr(w.map.ptr()) };
        let header = w.header.ptr();
        map.set_header(header);
        map.set_count(3);
        let first = w.nodes[0].ptr();
        let middle = w.nodes[1].ptr();
        let last = w.nodes[2].ptr();
        let h = unsafe { Node::from_ptr(header) };
        h.set_parent(middle);
        h.set_left(first);
        h.set_right(last);
        let a = unsafe { Node::from_ptr(first) };
        a.set_key(10);
        a.set_parent(middle);
        let b = unsafe { Node::from_ptr(middle) };
        b.set_key(20);
        b.set_parent(header);
        b.set_left(first);
        b.set_right(last);
        let c = unsafe { Node::from_ptr(last) };
        c.set_key(30);
        c.set_parent(middle);
        STATE.with(|s| s.borrow_mut().result = last as usize);
        w
    }
    fn run(&mut self, hint: *mut u8, key: u32) -> (u64, u32) {
        let mut value = [key, 0xDEAD_BEEF, 0];
        let mut out = 0u32;
        let result = unsafe { fc4p_0069a8f0(self.map.ptr(), 0x8765_4321, &mut out, hint, value.as_mut_ptr().cast()) };
        assert_eq!(result as u32, &mut out as *mut u32 as u32);
        (result >> 32, out)
    }
}

#[test]
fn hints_cover_empty_minimum_end_and_both_sides_of_interior_nodes() {
    let _a = fake(0x0069_A610, insert as *const () as usize);
    let _b = fake(0x0069_A6E0, unique as *const () as usize);
    let _c = fake(0x0068_09E0, previous as *const () as usize);
    let _d = fake(0x005D_17C0, next as *const () as usize);
    let mut w = World::new();
    let map = w.map.ptr() as usize;
    let header = w.header.ptr();
    let first = w.nodes[0].ptr();
    let middle = w.nodes[1].ptr();
    let last = w.nodes[2].ptr();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.previous = first as usize;
        s.next = last as usize;
    });
    assert_eq!(w.run(first, 5), (0x1234_5678, last as u32));
    assert_eq!(events(), [Event::Insert(map, first as usize, first as usize, 5, 0)]);
    assert_eq!(w.run(first, 10), (0x8765_4321, first as u32));
    assert!(events().is_empty());
    STATE.with(|s| s.borrow_mut().next = middle as usize);
    w.run(first, 15);
    assert_eq!(
        events(),
        [
            Event::Next(first as usize),
            Event::Insert(map, 0, first as usize, 15, first as u32)
        ]
    );
    unsafe { Node::from_ptr(first) }.set_right(middle);
    w.run(first, 15);
    assert_eq!(
        events(),
        [
            Event::Next(first as usize),
            Event::Insert(map, middle as usize, middle as usize, 15, 0)
        ]
    );
    w.run(first, 20);
    assert_eq!(events(), [Event::Next(first as usize), Event::Unique(map, 20)]);
    STATE.with(|s| s.borrow_mut().next = header as usize);
    w.run(first, 40);
    assert_eq!(
        events(),
        [
            Event::Next(first as usize),
            Event::Insert(map, 0, first as usize, 40, first as u32)
        ]
    );
    w.run(header, u32::MAX);
    assert_eq!(
        events(),
        [Event::Insert(map, 0, last as usize, u32::MAX, header as u32)]
    );
    w.run(header, 30);
    assert_eq!(events(), [Event::Unique(map, 30)]);
    unsafe { Node::from_ptr(first) }.set_right(null_mut());
    STATE.with(|s| s.borrow_mut().next = last as usize);
    w.run(middle, 15);
    assert_eq!(
        events(),
        [
            Event::Previous(middle as usize),
            Event::Insert(map, 0, first as usize, 15, first as u32)
        ]
    );
    unsafe { Node::from_ptr(first) }.set_right(middle);
    w.run(middle, 15);
    assert_eq!(
        events(),
        [
            Event::Previous(middle as usize),
            Event::Insert(map, middle as usize, middle as usize, 15, 0)
        ]
    );
    assert_eq!(w.run(middle, 20), (0x1234_5600, middle as u32));
    assert_eq!(
        events(),
        [Event::Previous(middle as usize), Event::Next(middle as usize)]
    );
    w.run(middle, 25);
    assert_eq!(
        events(),
        [
            Event::Previous(middle as usize),
            Event::Next(middle as usize),
            Event::Insert(map, last as usize, last as usize, 25, 0)
        ]
    );
    unsafe { Node::from_ptr(middle) }.set_right(null_mut());
    w.run(middle, 25);
    assert_eq!(
        events(),
        [
            Event::Previous(middle as usize),
            Event::Next(middle as usize),
            Event::Insert(map, 0, middle as usize, 25, middle as u32)
        ]
    );
    for key in [5, 30, 40] {
        w.run(middle, key);
        assert_eq!(
            events(),
            [
                Event::Previous(middle as usize),
                Event::Next(middle as usize),
                Event::Unique(map, key)
            ]
        );
    }
    let map_view = unsafe { Tree::from_ptr(w.map.ptr()) };
    map_view.set_count(0);
    unsafe { Node::from_ptr(header) }.set_left(header);
    assert_eq!(w.run(header, 0x8000_0000), (last as u32 as u64, last as u32));
    assert_eq!(events(), [Event::Unique(map, 0x8000_0000)]);
}

extern "C" fn allocate(size: u32) -> u64 {
    log(Event::Allocate(size));
    STATE.with(|s| s.borrow().allocation as u64)
}
extern "thiscall" fn copy(this: *mut u8, source: *mut u8) -> u64 {
    log(Event::Copy);
    unsafe {
        let original = Tree::from_ptr(source);
        let h = Node::from_ptr(original.header());
        assert_eq!(original.count(), 0);
        assert_eq!(h.color(), 0);
        assert!(h.parent().is_null());
        assert_eq!(h.left(), original.header());
        assert_eq!(h.right(), original.header());
        let bytes = core::slice::from_raw_parts(h.as_ptr(), 24);
        assert_eq!(&bytes[1..4], &[0xA5; 3]);
        assert_eq!(&bytes[16..24], &[0xA5; 8]);
        STATE.with(|s| {
            let s = s.borrow();
            let target = Tree::from_ptr(this);
            target.set_header(s.copied as *mut u8);
            target.set_count(u32::from(s.populated));
            if s.populated {
                original.set_count(1);
                h.set_parent(0x1110usize as *mut u8);
            }
        });
    }
    this as u64
}
extern "thiscall" fn hint(this: *mut u8, out: *mut u32, hint: *mut u8, value: *mut u32) -> u64 {
    log(Event::Hint(this as usize, hint as usize, unsafe { read(value) }));
    unsafe { write(out, STATE.with(|s| s.borrow().result as u32)) };
    0x7788_9900_1234_5678
}
extern "thiscall" fn delete(_this: *mut u8, root: *mut u8) -> u64 {
    log(Event::Delete(root as usize));
    0
}
extern "C" fn free(pointer: *mut u8) -> u64 {
    log(Event::Free(pointer as usize));
    let h = unsafe { Node::from_ptr(pointer) };
    assert!(h.parent().is_null());
    assert_eq!(h.left(), pointer);
    assert_eq!(h.right(), pointer);
    0xAABB_CCDD_1234_5678
}

#[test]
fn tried_tree_lookup_finds_existing_and_constructs_copies_disposes_missing_values() {
    let _a = fake(0x0090_CF54, allocate as *const () as usize);
    let _b = fake(0x0069_8A80, copy as *const () as usize);
    let _c = fake(0x0069_A8F0, hint as *const () as usize);
    let _d = fake(0x0069_5E10, delete as *const () as usize);
    let _e = fake(0x0090_CF63, free as *const () as usize);
    let mut w = World::new();
    let mp = w.map.ptr();
    let header = w.header.ptr();
    let middle = w.nodes[1].ptr();
    let last = w.nodes[2].ptr();
    let mut key = 20u32;
    assert_eq!(
        unsafe { fc4p_0069ab40(mp, 0xDEAD_BEEF, &mut key) },
        (20u64 << 32) | u64::from(middle as u32 + 20)
    );
    assert!(events().is_empty());
    let mut allocation = Object::new(24);
    let mut copied = Object::new(24);
    for populated in [false, true] {
        unsafe { core::ptr::write_bytes(allocation.ptr(), 0xA5, 24) };
        let h = unsafe { Node::from_ptr(copied.ptr()) };
        h.set_parent(if populated { 0x2220usize as *mut u8 } else { null_mut() });
        h.set_left(copied.ptr());
        h.set_right(copied.ptr());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.allocation = allocation.ptr() as usize;
            s.copied = copied.ptr() as usize;
            s.populated = populated;
        });
        for (value, candidate) in [(15u32, middle), (40, header)] {
            key = value;
            assert_eq!(
                unsafe { fc4p_0069ab40(mp, 0xDEAD_BEEF, &mut key) },
                0xAABB_CCDD_0000_0000 | u64::from(last as u32 + 20)
            );
            let mut expected = vec![
                Event::Allocate(24),
                Event::Copy,
                Event::Hint(mp as usize, candidate as usize, value),
            ];
            if populated {
                expected.push(Event::Delete(0x2220));
            }
            expected.push(Event::Free(copied.ptr() as usize));
            if populated {
                expected.push(Event::Delete(0x1110));
            }
            expected.push(Event::Free(allocation.ptr() as usize));
            assert_eq!(events(), expected);
            unsafe { core::ptr::write_bytes(allocation.ptr(), 0xA5, 24) };
            h.set_parent(if populated { 0x2220usize as *mut u8 } else { null_mut() });
        }
    }
}
