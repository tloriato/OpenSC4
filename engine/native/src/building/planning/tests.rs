// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Allocate(u32),
    Free(usize),
    Purpose(u32),
    Demand(u32),
    Value(u32),
    List(u32),
    Cursor(u32),
    Date,
    Day,
    Random(u32),
    Delete(usize),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    allocations: Vec<Object>,
    cursors: Vec<(u32, usize)>,
    draws: Vec<u32>,
    today: u32,
    date: usize,
    rng: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(event: Event) {
    STATE.with(|s| s.borrow_mut().events.push(event));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
fn memory(size: usize) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut object = Object::new(size);
        let p = object.ptr();
        unsafe {
            core::ptr::write_bytes(p, 0xA5, size);
        }
        s.allocations.push(object);
        p
    })
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Event::Allocate(size));
    memory(size as usize)
}
extern "C" fn free(pointer: *mut u8) -> u64 {
    log(Event::Free(pointer as usize));
    0xAABB_CCDD_1234_5678
}
#[repr(C)]
struct Demand {
    table: *const usize,
    id: u32,
    value: f64,
}
#[repr(C)]
struct Developer {
    table: *const usize,
    id: u32,
    purpose: u32,
    demand: *mut Demand,
    list: *mut u32,
}
extern "thiscall" fn purpose(this: *mut Developer) -> u32 {
    let d = unsafe { &*this };
    log(Event::Purpose(d.id));
    d.purpose
}
extern "thiscall" fn demand(this: *mut Developer) -> *mut Demand {
    let d = unsafe { &*this };
    log(Event::Demand(d.id));
    d.demand
}
extern "thiscall" fn value(this: *mut Demand) -> f64 {
    let d = unsafe { &*this };
    log(Event::Value(d.id));
    d.value
}
extern "thiscall" fn list(this: *mut Developer) -> *mut u32 {
    let d = unsafe { &*this };
    log(Event::List(d.id));
    d.list
}
extern "thiscall" fn cursor(this: *mut u8, key: *mut *mut Developer) -> *mut u32 {
    let developer = unsafe { read(key) };
    let d = unsafe { &*developer };
    log(Event::Cursor(d.id));
    if let Some(p) = STATE.with(|s| {
        s.borrow()
            .cursors
            .iter()
            .find(|&&(key, _)| key == developer as u32)
            .map(|&(_, p)| p)
    }) {
        return (p as *mut u8).wrapping_add(20).cast();
    }
    let node = memory(24);
    unsafe {
        let map = Tree::from_ptr(this);
        let header = Node::from_ptr(map.header());
        let n = Node::from_ptr(node);
        n.set_key(developer as u32);
        n.set_left(header.parent());
        n.set_right(core::ptr::null_mut());
        n.set_parent(map.header());
        header.set_parent(node);
        map.set_count(map.count().wrapping_add(1));
    }
    STATE.with(|s| s.borrow_mut().cursors.push((developer as u32, node as usize)));
    node.wrapping_add(20).cast()
}
extern "thiscall" fn random(this: *mut u8, limit: u32) -> u32 {
    assert_eq!(this as usize, STATE.with(|s| s.borrow().rng));
    log(Event::Random(limit));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.draws.is_empty() {
            0
        } else {
            s.draws.remove(0)
        }
    })
}
extern "thiscall" fn date(_this: *mut u8) -> *mut u8 {
    log(Event::Date);
    STATE.with(|s| s.borrow().date as *mut u8)
}
extern "thiscall" fn day(_this: *mut u8) -> u32 {
    log(Event::Day);
    STATE.with(|s| s.borrow().today)
}
extern "thiscall" fn delete(_this: *mut u8, root: *mut u8) -> u64 {
    log(Event::Delete(root as usize));
    0
}

struct World {
    sim: Object,
    header: Object,
    plan: Object,
    nodes: Vec<Object>,
    developers: Vec<Developer>,
    _demands: Vec<Demand>,
    _candidates: Vec<Vec<[u32; 3]>>,
    _lists: Vec<[u32; 3]>,
    _tables: Vec<Box<[usize]>>,
    _clock: Object,
    _date: Object,
    globals: Box<(*mut u8, f32, u8)>,
    _guards: Vec<Fake>,
}
impl World {
    fn new(config: &[(u32, f64, usize)]) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                today: 5000,
                ..State::default()
            }
        });
        let mut developer_table = vec![0usize; 30].into_boxed_slice();
        developer_table[0x1C / 4] = purpose as *const () as usize;
        developer_table[0x48 / 4] = demand as *const () as usize;
        developer_table[0x30 / 4] = list as *const () as usize;
        let mut demand_table = vec![0usize; 13].into_boxed_slice();
        demand_table[0x30 / 4] = value as *const () as usize;
        let mut clock_table = vec![0usize; 8].into_boxed_slice();
        clock_table[0x1C / 4] = date as *const () as usize;
        let mut date_table = vec![0usize; 27].into_boxed_slice();
        date_table[0x68 / 4] = day as *const () as usize;
        let candidates: Vec<_> = config
            .iter()
            .enumerate()
            .map(|(i, &(_, _, n))| {
                (0..n)
                    .map(|j| [i as u32 * 10 + j as u32, 67 + j as u32, 0xDEAD_BEEF])
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut lists: Vec<_> = candidates
            .iter()
            .map(|c| {
                [
                    c.as_ptr() as u32,
                    c.as_ptr().wrapping_add(c.len()) as u32,
                    c.as_ptr().wrapping_add(c.len()) as u32,
                ]
            })
            .collect();
        let mut demands: Vec<_> = config
            .iter()
            .enumerate()
            .map(|(i, &(_, v, _))| Demand {
                table: demand_table.as_ptr(),
                id: i as u32,
                value: v,
            })
            .collect();
        let developers = config
            .iter()
            .enumerate()
            .map(|(i, &(p, _, _))| Developer {
                table: developer_table.as_ptr(),
                id: i as u32,
                purpose: p,
                demand: &mut demands[i],
                list: lists[i].as_mut_ptr(),
            })
            .collect();
        let mut w = Self {
            sim: Object::new(0xE4),
            header: Object::new(24),
            plan: Object::new(8),
            nodes: config.iter().map(|_| Object::new(24)).collect(),
            developers,
            _demands: demands,
            _candidates: candidates,
            _lists: lists,
            _tables: vec![developer_table, demand_table, clock_table, date_table],
            _clock: Object::new(4),
            _date: Object::new(4),
            globals: Box::new((core::ptr::null_mut(), 4294967296.0, 0)),
            _guards: vec![],
        };
        let hp = w.header.ptr();
        let h = unsafe { Node::from_ptr(hp) };
        let pointers: Vec<_> = w.nodes.iter_mut().map(Object::ptr).collect();
        h.set_parent(pointers.first().copied().unwrap_or(core::ptr::null_mut()));
        h.set_left(pointers.first().copied().unwrap_or(hp));
        h.set_right(pointers.last().copied().unwrap_or(hp));
        for (i, &p) in pointers.iter().enumerate() {
            let n = unsafe { Node::from_ptr(p) };
            n.set_key(0x1010 + i as u32);
            n.set_value(&mut w.developers[i] as *mut Developer as u32);
            n.set_parent(if i == 0 { hp } else { pointers[i - 1] });
            n.set_right(pointers.get(i + 1).copied().unwrap_or(core::ptr::null_mut()));
        }
        let pp = w.plan.ptr();
        unsafe {
            write(pp.cast::<*mut u8>(), pp);
            write(pp.add(4).cast::<*mut u8>(), pp);
        }
        let sim = unsafe { BuildingSim::from_ptr(w.sim.ptr()) };
        sim.set_developers(hp);
        sim.set_decline_plan(pp);
        sim.set_demand_bias(10);
        unsafe {
            write(w._clock.ptr().cast::<*const usize>(), w._tables[2].as_ptr());
            write(w._date.ptr().cast::<*const usize>(), w._tables[3].as_ptr());
        }
        w.globals.0 = w._clock.ptr();
        STATE.with(|s| {
            s.borrow_mut().date = w._date.ptr() as usize;
            s.borrow_mut().rng = &mut w.globals.2 as *mut u8 as usize;
        });
        w._guards.extend([
            fake(0x0090_CF54, allocate as *const () as usize),
            fake(0x0090_CF63, free as *const () as usize),
            fake(0x0049_7A00, cursor as *const () as usize),
            fake(0x0091_36B1, random as *const () as usize),
            fake(0x0069_5D90, delete as *const () as usize),
            fake_data(0x00B4_3D5C, (&mut w.globals.0 as *mut *mut u8).cast()),
            fake_data(0x00A8_0AA8, (&mut w.globals.1 as *mut f32).cast()),
            fake_data(0x00B4_85F0, &mut w.globals.2),
        ]);
        w
    }
    fn run(&mut self) -> u64 {
        unsafe { plan_decline(self.sim.ptr(), 0, 0xAABB_CCDD) }
    }
    fn entries(&mut self) -> Vec<[u32; 5]> {
        let mut entries = vec![];
        let hp = self.plan.ptr();
        let mut p = unsafe { read(hp.cast::<*mut u8>()) };
        let mut previous = hp;
        while p != hp {
            assert_eq!(unsafe { read(p.add(4).cast::<*mut u8>()) }, previous);
            entries.push(unsafe {
                [
                    read(p.add(8).cast()),
                    read(p.add(12).cast()),
                    read(p.add(16).cast()),
                    read(p.add(20).cast()),
                    read(p.add(24).cast()),
                ]
            });
            previous = p;
            p = unsafe { read(p.cast()) };
        }
        assert_eq!(unsafe { read(hp.add(4).cast::<*mut u8>()) }, previous);
        entries
    }
}

#[test]
fn one_developer_plan_has_exact_call_order_replaces_old_nodes_and_keeps_stage_padding() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(1, -30.0, 1)]);
    let mut old = Object::new(28);
    let hp = w.plan.ptr();
    let op = old.ptr();
    unsafe {
        write(hp.cast::<*mut u8>(), op);
        write(hp.add(4).cast::<*mut u8>(), op);
        write(op.cast::<*mut u8>(), hp);
        write(op.add(4).cast::<*mut u8>(), hp);
    }
    assert_eq!(w.run(), 0xAABB_CCDD_1234_5678);
    assert_eq!(
        w.entries(),
        [[5000, 0, 67, &mut w.developers[0] as *mut Developer as u32, 0xAABB_CC00]]
    );
    let calls = events();
    let frees: Vec<_> = calls
        .iter()
        .filter_map(|e| if let Event::Free(p) = e { Some(*p) } else { None })
        .collect();
    let cursor_node = STATE.with(|s| s.borrow().cursors[0].1);
    assert_eq!(frees[0], op as usize);
    assert_eq!(frees[1], cursor_node);
    assert_eq!(
        calls,
        [
            Event::Free(op as usize),
            Event::Allocate(24),
            Event::Purpose(0),
            Event::Demand(0),
            Event::Value(0),
            Event::List(0),
            Event::Cursor(0),
            Event::Date,
            Event::Day,
            Event::Random(40),
            Event::Purpose(0),
            Event::Demand(0),
            Event::Value(0),
            Event::List(0),
            Event::Cursor(0),
            Event::Allocate(28),
            Event::Purpose(0),
            Event::Demand(0),
            Event::Value(0),
            Event::List(0),
            Event::Cursor(0),
            Event::Delete(0),
            Event::Free(cursor_node),
            Event::Free(frees[2])
        ]
    );
}

#[test]
fn weighted_draws_retain_exhausted_weights_and_all_candidates_are_planned_in_draw_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(1, -30.0, 2), (2, -50.0, 1)]);
    STATE.with(|s| s.borrow_mut().draws = vec![40, 60, 0, 0]);
    w.run();
    let a = &mut w.developers[0] as *mut Developer as u32;
    let b = &mut w.developers[1] as *mut Developer as u32;
    assert_eq!(
        w.entries(),
        [
            [5000, 10, 67, b, 0xAABB_CC00],
            [5001, 0, 67, a, 0xAABB_CC00],
            [5002, 1, 68, a, 0xAABB_CC00]
        ]
    );
    let calls = events();
    assert_eq!(calls.iter().filter(|e| matches!(e, Event::Random(100))).count(), 4);
    assert_eq!(calls.iter().filter(|e| matches!(e, Event::Allocate(28))).count(), 3);
    assert_eq!(calls.iter().filter(|e| matches!(e, Event::Delete(0))).count(), 2);
}

#[test]
fn agriculture_thresholds_zero_weight_and_empty_candidates_keep_call_conditions() {
    let _fpu = GameFpu::new();
    for demand in [-20.99, -20.0, 0.0, f64::NAN, f64::INFINITY] {
        let mut w = World::new(&[(5, -1000.0, 5), (1, demand, 3)]);
        w.run();
        assert!(w.entries().is_empty());
        let calls = events();
        assert!(calls.contains(&Event::Random(0)));
        assert!(!calls.contains(&Event::Demand(0)));
        assert!(calls.contains(&Event::List(1)));
        assert!(!calls.contains(&Event::Allocate(28)));
    }
    let mut w = World::new(&[(1, -21.0, 0)]);
    w.run();
    assert!(w.entries().is_empty());
    assert!(events().contains(&Event::Random(31)));
    let mut w = World::new(&[]);
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_00699890 as *const () as usize,
        w.sim.ptr() as u32,
        &[],
    );
    unsafe { call_raw(&mut call) };
    assert_eq!(call.eax, 0x1234_5678);
    assert!(events().contains(&Event::Random(0)));
}

#[test]
fn planned_days_follow_float_addition_and_unsigned_conversion() {
    let _fpu = GameFpu::new();
    for (today, expected) in [
        ((1u32 << 24) + 2, [(1u32 << 24) + 2, (1u32 << 24) + 4, (1u32 << 24) + 4]),
        (u32::MAX, [0, 0, 0]),
    ] {
        let mut w = World::new(&[(1, -30.0, 3)]);
        STATE.with(|s| s.borrow_mut().today = today);
        w.run();
        assert_eq!(w.entries().iter().map(|e| e[0]).collect::<Vec<_>>(), expected);
        events();
    }
}

#[test]
fn plan_export_keeps_the_original_stage_padding_and_existing_x87_value() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(1, -30.0, 1)]);
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    // SAFETY: the seed occupies the original entry's local stage word; the call balances its own stack.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{sentinel}]", "mov dword ptr [esp - 8], 0x11223344", "call {entry}",
            "fstp qword ptr [edi]",
            sentinel = in(reg) &sentinel, in("edi") &mut restored,
            entry = sym fc4p_00699890, in("ecx") w.sim.ptr(), clobber_abi("C"), out("st(0)") _,
        );
    }
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert_eq!(w.entries()[0][4], 0x1122_3300);
    events();
}
