// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Date,
    Day,
    Demand(usize),
    Value,
    Allocate(u32),
    Decline(usize, [u32; 3], u8),
    Free(usize),
    Destroy(usize),
    TimerNew(u32),
    TimerStart,
    Grow(usize),
    Planned(usize),
    Execute(usize),
    TimerEnd,
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    date: usize,
    today: u32,
    allocation: usize,
    decline: i32,
    demand: f64,
    list_item: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(event: Event) {
    STATE.with(|s| s.borrow_mut().events.push(event));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn date(_this: *mut u8) -> *mut u8 {
    log(Event::Date);
    STATE.with(|s| s.borrow().date as *mut u8)
}
extern "thiscall" fn day(_this: *mut u8) -> u64 {
    log(Event::Day);
    STATE.with(|s| 0x1122_3344_0000_0000 | u64::from(s.borrow().today))
}
extern "thiscall" fn demand(this: *mut u8) -> *mut u8 {
    log(Event::Demand(this as usize));
    this
}
extern "thiscall" fn value(_this: *mut u8) -> f64 {
    log(Event::Value);
    STATE.with(|s| s.borrow().demand)
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Event::Allocate(size));
    STATE.with(|s| s.borrow().allocation as *mut u8)
}
extern "C" fn free(pointer: *mut u8) -> u64 {
    log(Event::Free(pointer as usize));
    0xAABB_CCDD_1234_5678
}
extern "thiscall" fn decline(this: *mut u8, profile: *mut u8) -> i32 {
    let words = unsafe {
        [
            read(profile.cast::<u32>()),
            read(profile.add(4).cast()),
            read(profile.add(8).cast()),
        ]
    };
    log(Event::Decline(this as usize, words, unsafe { *profile.add(12) }));
    STATE.with(|s| {
        let s = s.borrow();
        let list = s.allocation as *mut u8;
        // The temporary list is initialized before Decline and its payload is retained.
        unsafe {
            assert_eq!(read(list.cast::<usize>()), list as usize);
            assert_eq!(read(list.add(4).cast::<usize>()), list as usize);
            assert_eq!(read(list.add(8).cast::<u32>()), 0x5555_AAAA);
            if s.list_item != 0 {
                write(list.cast::<usize>(), s.list_item);
                write(list.add(4).cast::<usize>(), s.list_item);
            }
        }
        s.decline
    })
}
extern "thiscall" fn destroy(list: *mut *mut u8) -> u64 {
    log(Event::Destroy(unsafe { list.read() } as usize));
    0x8765_4321_FEDC_BA98
}

struct World {
    sim: Object,
    header: Object,
    nodes: Vec<Object>,
    clock: Object,
    date: Object,
    developer: Object,
    tract: Object,
    temporary: Object,
    _clock_table: Box<[usize]>,
    _date_table: Box<[usize]>,
    _developer_table: Box<[usize]>,
    _tract_table: Box<[usize]>,
}
impl World {
    fn new(days: &[u32]) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                today: 5000,
                demand: -20.99,
                ..State::default()
            }
        });
        let mut w = Self {
            sim: Object::new(0xE4),
            header: Object::new(28),
            nodes: days.iter().map(|_| Object::new(28)).collect(),
            clock: Object::new(4),
            date: Object::new(4),
            developer: Object::new(4),
            tract: Object::new(4),
            temporary: Object::new(12),
            _clock_table: vec![0; 28].into_boxed_slice(),
            _date_table: vec![0; 28].into_boxed_slice(),
            _developer_table: vec![0; 28].into_boxed_slice(),
            _tract_table: vec![0; 28].into_boxed_slice(),
        };
        w._clock_table[0x1C / 4] = date as *const () as usize;
        w._date_table[0x68 / 4] = day as *const () as usize;
        w._developer_table[0x48 / 4] = demand as *const () as usize;
        w._developer_table[0x30 / 4] = value as *const () as usize;
        w._tract_table[0x18 / 4] = decline as *const () as usize;
        unsafe {
            write(w.clock.ptr().cast(), w._clock_table.as_ptr());
            write(w.date.ptr().cast(), w._date_table.as_ptr());
            write(w.developer.ptr().cast(), w._developer_table.as_ptr());
            write(w.tract.ptr().cast(), w._tract_table.as_ptr());
            write(w.temporary.ptr().add(8).cast::<u32>(), 0x5555_AAAA);
        }
        let header = w.header.ptr();
        let mut pointers = vec![header];
        pointers.extend(w.nodes.iter_mut().map(|n| n.ptr()));
        for i in 0..pointers.len() {
            let node = unsafe { DeclineNode::from_ptr(pointers[i]) };
            node.set_next(pointers[(i + 1) % pointers.len()]);
            node.set_previous(pointers[(i + pointers.len() - 1) % pointers.len()]);
            if i != 0 {
                node.set_day(days[i - 1]);
                node.set_x(0xFFFF_FFFE);
                node.set_z(67);
                node.set_developer(w.developer.ptr());
                node.set_stage(0x80);
            }
        }
        let sim = unsafe { BuildingSim::from_ptr(w.sim.ptr()) };
        sim.set_decline_plan(header);
        sim.set_tract_developer(w.tract.ptr());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.date = w.date.ptr() as usize;
            s.allocation = w.temporary.ptr() as usize;
        });
        w
    }
    fn run(&mut self) -> u64 {
        let mut clock = self.clock.ptr();
        let _global = fake_data(0x00B4_3D5C, (&mut clock as *mut *mut u8).cast());
        let _allocate = fake(0x0090_CF54, allocate as *const () as usize);
        let _free = fake(0x0090_CF63, free as *const () as usize);
        let _destroy = fake(0x0056_50A0, destroy as *const () as usize);
        unsafe { fc4p_00696a50(self.sim.ptr(), 0xDEAD_BEEF) }
    }
    fn remaining(&mut self) -> Vec<usize> {
        let header = self.header.ptr();
        let mut out = vec![];
        let mut current = unsafe { DeclineNode::from_ptr(header) }.next();
        let mut previous = header;
        while current != header {
            let n = unsafe { DeclineNode::from_ptr(current) };
            assert_eq!(n.previous(), previous);
            out.push(current as usize);
            previous = current;
            current = n.next();
        }
        assert_eq!(unsafe { DeclineNode::from_ptr(header) }.previous(), previous);
        out
    }
}

#[test]
fn decline_empty_future_and_unsigned_days() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[]);
    let header = w.header.ptr() as u32;
    assert_eq!(w.run(), 0x1122_3344_0000_0000 | u64::from(header));
    assert_eq!(events(), [Event::Date, Event::Day]);
    let mut w = World::new(&[5001, u32::MAX]);
    assert_eq!(w.run(), 0x1122_3344_0000_1388);
    assert_eq!(events(), [Event::Date, Event::Day]);
    assert_eq!(w.remaining().len(), 2);
}

#[test]
fn decline_recovered_demand_truncates_and_removes_without_allocating() {
    let _fpu = GameFpu::new();
    for demand in [-19.99, -0.0, 1.0, f64::NAN, f64::INFINITY, 3_000_000_000.0] {
        let mut w = World::new(&[4999, 5000]);
        STATE.with(|s| s.borrow_mut().demand = demand);
        w.run();
        let calls = events();
        if demand == 3_000_000_000.0 {
            assert_eq!(calls.iter().filter(|e| matches!(e, Event::Allocate(12))).count(), 2);
        } else {
            assert!(!calls.iter().any(|e| matches!(e, Event::Allocate(_))));
        }
        assert_eq!(calls.iter().filter(|e| matches!(e, Event::Demand(_))).count(), 2);
        assert!(w.remaining().is_empty());
    }
}

#[test]
fn decline_failure_cleans_temporary_list_and_continues_success_stops_once() {
    let _fpu = GameFpu::new();
    for result in [i32::MIN, -1, 0, 1, i32::MAX] {
        let mut w = World::new(&[5001, 5000, 4999]);
        let pointers: Vec<_> = w.nodes.iter_mut().map(|n| n.ptr() as usize).collect();
        let temporary = w.temporary.ptr() as usize;
        STATE.with(|s| s.borrow_mut().decline = result);
        let value = w.run();
        let calls = events();
        let count = if result > 0 { 1 } else { 2 };
        assert_eq!(
            calls.iter().filter(|e| matches!(e, Event::Decline(_, _, _))).count(),
            count
        );
        assert!(calls.contains(&Event::Decline(
            w.tract.ptr() as usize,
            [w.developer.ptr() as u32, 0xFFFF_FFFE, 67],
            0x80
        )));
        if result > 0 {
            assert_eq!(value, 0x8765_4321_FEDC_BA98);
            assert_eq!(w.remaining(), [pointers[0], pointers[2]]);
            assert_eq!(
                &calls[calls.len() - 2..],
                [Event::Free(pointers[1]), Event::Destroy(temporary)]
            );
        } else {
            assert_eq!(w.remaining(), [pointers[0]]);
            assert_eq!(
                &calls[calls.len() - 2..],
                [Event::Free(pointers[2]), Event::Free(temporary)]
            );
        }
    }
    let mut w = World::new(&[5000]);
    let mut item = Object::new(12);
    unsafe { write(item.ptr().cast::<usize>(), w.temporary.ptr() as usize) };
    STATE.with(|s| s.borrow_mut().list_item = item.ptr() as usize);
    w.run();
    let calls = events();
    assert_eq!(
        &calls[calls.len() - 2..],
        [
            Event::Free(item.ptr() as usize),
            Event::Free(w.temporary.ptr() as usize)
        ]
    );
}

extern "thiscall" fn timer_new(this: *mut u8, category: u32) -> u64 {
    log(Event::TimerNew(category));
    unsafe { core::ptr::write_bytes(this, 0xA5, 24) };
    0
}
extern "thiscall" fn timer_start(_this: *mut u8) -> u64 {
    log(Event::TimerStart);
    0
}
extern "thiscall" fn grow(this: *mut u8) -> u64 {
    log(Event::Grow(this as usize));
    0x1111_2222_3333_4444
}
extern "thiscall" fn planned(this: *mut u8) -> u64 {
    log(Event::Planned(this as usize));
    0x5555_6666_7777_8888
}
extern "thiscall" fn execute(this: *mut u8) -> u64 {
    log(Event::Execute(this as usize));
    0x9999_AAAA_BBBB_CCCC
}
#[unsafe(naked)]
unsafe extern "thiscall" fn timer_end(_this: *mut u8) -> u64 {
    core::arch::naked_asm!("push eax", "push edx", "call {log}", "pop edx", "pop eax", "ret", log = sym end_log);
}
extern "C" fn end_log() {
    log(Event::TimerEnd);
}

#[test]
fn daily_dispatches_nonzero_growth_flag_then_decline_and_balances_timer() {
    let _a = fake(0x0088_FEDF, timer_new as *const () as usize);
    let _b = fake(0x0089_05C4, timer_start as *const () as usize);
    let _c = fake(0x0069_B0B0, grow as *const () as usize);
    let _d = fake(0x0069_9270, planned as *const () as usize);
    let _e = fake(0x0069_6A50, execute as *const () as usize);
    let _f = fake(0x00A6_D837, timer_end as *const () as usize);
    let mut object = Object::new(0xE4);
    let sim = unsafe { BuildingSim::from_ptr(object.ptr()) };
    for flag in [0, 1, 0x80, 0xFF] {
        sim.set_use_growth_plan(flag);
        assert_eq!(
            unsafe { fc4p_0069b6d0(object.ptr(), 0xDEAD_BEEF) },
            0x9999_AAAA_BBBB_CCCC
        );
        assert_eq!(
            events(),
            [
                Event::TimerNew(4),
                Event::TimerStart,
                if flag == 0 {
                    Event::Grow(object.ptr() as usize)
                } else {
                    Event::Planned(object.ptr() as usize)
                },
                Event::Execute(object.ptr() as usize),
                Event::TimerEnd
            ]
        );
    }
}
