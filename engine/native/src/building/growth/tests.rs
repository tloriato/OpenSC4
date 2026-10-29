// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Budget(u32),
    Copy(usize),
    Expired(u32),
    Demand(u32),
    Value(u32),
    Random(u32),
    Type(u32),
    CanGrow(u32),
    List(u32),
    Find(u32),
    Tried(u32),
    Mark([u32; 2]),
    Grow([u32; 4]),
    Allocate(u32),
    Free(usize),
    Date,
    Day,
    DayEntry(usize, u32),
    Erase(u32, u32),
    DeleteTried(usize),
    DeleteDeveloper(usize),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    allocations: Vec<Object>,
    draws: Vec<u32>,
    expiration: Vec<u32>,
    results: Vec<i32>,
    budget: usize,
    globals: usize,
    tried: Vec<(u32, usize)>,
    missed: bool,
    flag: usize,
    list_items: usize,
    date: usize,
    day: u32,
    days: usize,
    mutate: usize,
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
        let mut o = Object::new(size);
        let p = o.ptr();
        unsafe { core::ptr::write_bytes(p, 0xA5, size) };
        s.allocations.push(o);
        p
    })
}
#[repr(C)]
struct Globals {
    pace: u32,
    adjustment: i32,
    initialized: u32,
    rng: u8,
    active: u32,
    tried: u8,
    clock: *mut u8,
}
extern "thiscall" fn budget_new(this: *mut u8, category: u32) {
    log(Event::Budget(category));
    unsafe { core::ptr::write_bytes(this, 0xA5, 24) };
    STATE.with(|s| s.borrow_mut().budget = this as usize);
}
extern "C" {
    static fc4r_0069b149: u8;
}

#[unsafe(naked)]
unsafe extern "thiscall" fn expired(_this: *mut u8) -> u32 {
    core::arch::naked_asm!("mov edx, [esp]", "jmp {}", sym expired_at_site);
}

extern "fastcall" fn expired_at_site(this: *mut u8, return_address: usize) -> u32 {
    assert_eq!(return_address, core::ptr::addr_of!(fc4r_0069b149) as usize);
    log(Event::Expired(unsafe { read(this.add(8).cast()) }));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.expiration.is_empty() {
            1
        } else {
            s.expiration.remove(0)
        }
    })
}
#[repr(C)]
struct Demand {
    table: *const usize,
    key: u32,
    value: f64,
}
#[repr(C)]
struct Developer {
    table: *const usize,
    key: u32,
    can_grow: u32,
    demand: *mut Demand,
    list: *mut u32,
}
extern "thiscall" fn demand(this: *mut Developer) -> *mut Demand {
    let d = unsafe { &*this };
    log(Event::Demand(d.key));
    d.demand
}
extern "thiscall" fn value(this: *mut Demand) -> f64 {
    let d = unsafe { &*this };
    log(Event::Value(d.key));
    d.value
}
extern "thiscall" fn kind(this: *mut Developer) -> u32 {
    let d = unsafe { &*this };
    log(Event::Type(d.key));
    d.key
}
extern "thiscall" fn can_grow(this: *mut Developer) -> u32 {
    let d = unsafe { &*this };
    log(Event::CanGrow(d.key));
    d.can_grow
}
extern "thiscall" fn list(this: *mut Developer) -> *mut u32 {
    let d = unsafe { &*this };
    log(Event::List(d.key));
    d.list
}

unsafe fn rebuild(tree: *mut u8, nodes: &[*mut u8]) {
    unsafe {
        let map = Tree::from_ptr(tree);
        let h = Node::from_ptr(map.header());
        h.set_parent(nodes.first().copied().unwrap_or(core::ptr::null_mut()));
        h.set_left(nodes.first().copied().unwrap_or(map.header()));
        h.set_right(nodes.last().copied().unwrap_or(map.header()));
        map.set_count(nodes.len() as u32);
        for (i, &p) in nodes.iter().enumerate() {
            let n = Node::from_ptr(p);
            n.set_parent(if i == 0 { map.header() } else { nodes[i - 1] });
            n.set_left(core::ptr::null_mut());
            n.set_right(nodes.get(i + 1).copied().unwrap_or(core::ptr::null_mut()));
        }
    }
}
extern "thiscall" fn copy(this: *mut u8, source: *mut u8) {
    log(Event::Copy(source as usize));
    unsafe {
        STATE.with(|s| {
            let s = s.borrow();
            let budget = s.budget as *mut u8;
            let g = &*(s.globals as *mut Globals);
            assert_eq!(read(budget.add(12).cast::<u32>()), g.pace);
            assert_eq!(*budget.add(16), 0);
            assert_eq!(read(budget.add(20).cast::<*const u32>()), &g.pace as *const u32);
        });
        let src = Tree::from_ptr(source);
        let header = memory(24);
        let dst = Tree::from_ptr(this);
        dst.set_header(header);
        let mut current = Node::from_ptr(src.header()).left();
        let mut nodes = vec![];
        while current != src.header() {
            let n = Node::from_ptr(current);
            let p = memory(24);
            let copy = Node::from_ptr(p);
            copy.set_key(n.key());
            copy.set_value(n.value());
            nodes.push(p);
            current = successor(current);
        }
        rebuild(this, &nodes);
    }
}
extern "thiscall" fn random(this: *mut u8, total: u32) -> u32 {
    assert_eq!(
        this,
        STATE.with(|s| unsafe { &mut (*(s.borrow().globals as *mut Globals)).rng } as *mut u8)
    );
    log(Event::Random(total));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.draws.is_empty() {
            0
        } else {
            s.draws.remove(0)
        }
    })
}
extern "C" fn find(begin: *mut u8, end: *mut u8, predicate: u32, flag: *mut u8) -> *mut u8 {
    assert_eq!(predicate, 0x0069_B070);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.flag = flag as usize;
        let active = unsafe { (*(s.globals as *mut Globals)).active };
        s.events.push(Event::Find(active));
        if s.missed {
            end
        } else {
            begin
        }
    })
}
extern "thiscall" fn tried(this: *mut u8, key: *mut u32) -> *mut u8 {
    let key = unsafe { read(key) };
    log(Event::Tried(key));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this, unsafe { &mut (*(s.globals as *mut Globals)).tried } as *mut u8);
        s.tried.iter().find(|&&(k, _)| k == key).unwrap().1 as *mut u8
    })
}
extern "thiscall" fn mark(_this: *mut u8, _out: *mut u8, key: *mut u32) {
    log(Event::Mark(unsafe { [read(key), read(key.add(1))] }));
    STATE.with(|s| {
        let p = s.borrow().mutate as *mut u32;
        if !p.is_null() {
            unsafe {
                write(p, 9);
                write(p.add(1), 10);
                write(p.add(2), 0x80);
            }
        }
    });
}
extern "thiscall" fn grow(_this: *mut u8, profile: *mut u32, list: *mut *mut u8, flag: *mut u8) -> i32 {
    log(Event::Grow(unsafe {
        [
            read(profile),
            read(profile.add(1)),
            read(profile.add(2)),
            read(profile.add(3)),
        ]
    }));
    assert_eq!(flag as usize, STATE.with(|s| s.borrow().flag));
    assert_eq!(unsafe { read(flag) }, 0);
    unsafe {
        write(flag, 0x80);
    }
    let count = STATE.with(|s| s.borrow().list_items);
    unsafe {
        let header = read(list);
        let mut previous = header;
        for _ in 0..count {
            let node = memory(12);
            write(previous.cast::<*mut u8>(), node);
            write(node.add(4).cast::<*mut u8>(), previous);
            previous = node;
        }
        write(previous.cast::<*mut u8>(), header);
        write(header.add(4).cast::<*mut u8>(), previous);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.results.is_empty() {
            1
        } else {
            s.results.remove(0)
        }
    })
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Event::Allocate(size));
    memory(size as usize)
}
extern "C" fn free(pointer: *mut u8) {
    log(Event::Free(pointer as usize));
}
extern "thiscall" fn date(_this: *mut u8) -> *mut u8 {
    log(Event::Date);
    STATE.with(|s| s.borrow().date as *mut u8)
}
extern "thiscall" fn day(_this: *mut u8) -> u32 {
    log(Event::Day);
    STATE.with(|s| s.borrow().day)
}
extern "thiscall" fn day_entry(this: *mut u8, key: *mut u32) -> *mut u32 {
    let key = unsafe { read(key) };
    log(Event::DayEntry(this as usize, key));
    STATE.with(|s| s.borrow().days as *mut u32)
}
extern "thiscall" fn delete_tried(_this: *mut u8, node: *mut u8) {
    log(Event::DeleteTried(node as usize));
}
extern "thiscall" fn delete_developer(_this: *mut u8, node: *mut u8) {
    log(Event::DeleteDeveloper(node as usize));
}
extern "thiscall" fn erase(this: *mut u8, first: *mut u8, last: *mut u8) {
    unsafe {
        let map = Tree::from_ptr(this);
        let header = map.header();
        let key = |p| if p == header { u32::MAX } else { Node::from_ptr(p).key() };
        log(Event::Erase(key(first), key(last)));
        let mut current = Node::from_ptr(header).left();
        let mut nodes = vec![];
        let mut remove = false;
        while current != header {
            if current == first {
                remove = true;
            }
            if current == last {
                remove = false;
            }
            if !remove {
                nodes.push(current);
            }
            current = successor(current);
        }
        rebuild(this, &nodes);
    }
}

struct World {
    sim: Object,
    _developers: Vec<Developer>,
    _demands: Vec<Demand>,
    candidates: Vec<Vec<[u32; 3]>>,
    _lists: Vec<[u32; 3]>,
    _tables: Vec<Box<[usize]>>,
    _objects: Vec<Object>,
    globals: Box<Globals>,
    days: Box<u32>,
    _guards: Vec<Fake>,
}
impl World {
    fn new(config: &[(f64, u32, usize)]) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                day: 5000,
                ..State::default()
            }
        });
        let mut dt = vec![0usize; 30].into_boxed_slice();
        dt[0x48 / 4] = demand as *const () as usize;
        dt[0x18 / 4] = kind as *const () as usize;
        dt[0x28 / 4] = can_grow as *const () as usize;
        dt[0x2C / 4] = list as *const () as usize;
        let mut nt = vec![0usize; 13].into_boxed_slice();
        nt[0x30 / 4] = value as *const () as usize;
        let mut tt = vec![0usize; 6].into_boxed_slice();
        tt[0x14 / 4] = grow as *const () as usize;
        let mut ct = vec![0usize; 8].into_boxed_slice();
        ct[0x1C / 4] = date as *const () as usize;
        let mut date_table = vec![0usize; 27].into_boxed_slice();
        date_table[0x68 / 4] = day as *const () as usize;
        let candidates: Vec<_> = config
            .iter()
            .enumerate()
            .map(|(i, &(_, _, n))| (0..n).map(|j| [i as u32 * 10 + j as u32, 67, 3]).collect::<Vec<_>>())
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
            .map(|(i, &(value, _, _))| Demand {
                table: nt.as_ptr(),
                key: 0x1010 + i as u32,
                value,
            })
            .collect();
        let developers = config
            .iter()
            .enumerate()
            .map(|(i, &(_, can_grow, _))| Developer {
                table: dt.as_ptr(),
                key: 0x1010 + i as u32,
                can_grow,
                demand: &mut demands[i],
                list: lists[i].as_mut_ptr(),
            })
            .collect();
        let mut w = Self {
            sim: Object::new(0xE4),
            _developers: developers,
            _demands: demands,
            candidates,
            _lists: lists,
            _tables: vec![dt, nt, tt, ct, date_table],
            _objects: vec![],
            globals: Box::new(Globals {
                pace: 1,
                adjustment: 0,
                initialized: 1,
                rng: 0,
                active: 0,
                tried: 0,
                clock: core::ptr::null_mut(),
            }),
            days: Box::new(1234),
            _guards: vec![],
        };
        let hp = w.object(24);
        let tree = w.sim.ptr().wrapping_add(0x5C);
        unsafe { Tree::from_ptr(tree) }.set_header(hp);
        let mut nodes = vec![];
        for i in 0..config.len() {
            let p = w.object(24);
            let n = unsafe { Node::from_ptr(p) };
            n.set_key(0x1010 + i as u32);
            n.set_value(&mut w._developers[i] as *mut Developer as u32);
            nodes.push(p);
            let tried = w.object(8);
            let th = w.object(24);
            let t = unsafe { Tree::from_ptr(tried) };
            t.set_header(th);
            t.set_count(0);
            let h = unsafe { Node::from_ptr(th) };
            h.set_parent(core::ptr::null_mut());
            h.set_left(th);
            h.set_right(th);
            STATE.with(|s| s.borrow_mut().tried.push((0x1010 + i as u32, tried as usize)));
        }
        unsafe { rebuild(tree, &nodes) };
        let tract = w.object(4);
        let clock = w.object(4);
        let date = w.object(4);
        unsafe {
            write(tract.cast::<*const usize>(), w._tables[2].as_ptr());
            write(clock.cast::<*const usize>(), w._tables[3].as_ptr());
            write(date.cast::<*const usize>(), w._tables[4].as_ptr());
        }
        let sim = unsafe { BuildingSim::from_ptr(w.sim.ptr()) };
        sim.set_demand_bias(10);
        sim.set_tract_developer(tract);
        w.globals.clock = clock;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.globals = &mut *w.globals as *mut Globals as usize;
            s.date = date as usize;
            s.days = &mut *w.days as *mut u32 as usize;
        });
        w._guards.extend([
            fake(0x0068_BF50, budget_new as *const () as usize),
            fake(0x0068_BF80, expired as *const () as usize),
            fake(0x0069_89D0, copy as *const () as usize),
            fake(0x0091_36B1, random as *const () as usize),
            fake(0x0069_5E50, find as *const () as usize),
            fake(0x0069_AB40, tried as *const () as usize),
            fake(0x0069_6300, mark as *const () as usize),
            fake(0x0069_5E10, delete_tried as *const () as usize),
            fake(0x0069_5CD0, delete_developer as *const () as usize),
            fake(0x0069_6240, erase as *const () as usize),
            fake(0x0042_9760, day_entry as *const () as usize),
            fake(0x0090_CF54, allocate as *const () as usize),
            fake(0x0090_CF63, free as *const () as usize),
        ]);
        for (addr, pointer) in [
            (0x00B4_8608, (&mut w.globals.pace as *mut u32).cast()),
            (0x00B4_860C, (&mut w.globals.adjustment as *mut i32).cast()),
            (0x00B4_8610, (&mut w.globals.initialized as *mut u32).cast()),
            (0x00B4_85F0, &mut w.globals.rng as *mut u8),
            (0x00B4_8544, (&mut w.globals.active as *mut u32).cast()),
            (0x00B4_85E0, &mut w.globals.tried as *mut u8),
            (0x00B4_3D5C, (&mut w.globals.clock as *mut *mut u8).cast()),
        ] {
            w._guards.push(fake_data(addr, pointer));
        }
        w
    }
    fn object(&mut self, size: usize) -> *mut u8 {
        let mut o = Object::new(size);
        let p = o.ptr();
        self._objects.push(o);
        p
    }
    fn run(&mut self) -> u32 {
        unsafe { super::grow(self.sim.ptr(), 0, 0xAABB_CCDD) }
    }
}

#[test]
fn growth_initializes_only_the_game_pacing_globals_and_empty_copy_skips_random() {
    let mut w = World::new(&[]);
    w.globals.initialized = 0x1234_5600;
    w.globals.pace = 7;
    w.globals.adjustment = 99;
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        fc4p_0069b0b0 as *const () as usize,
        w.sim.ptr() as u32,
        &[],
    );
    unsafe { crate::abi::call_raw(&mut call) };
    assert_eq!(call.eax, 0);
    assert_eq!(
        (w.globals.initialized, w.globals.pace, w.globals.adjustment),
        (0x1234_5601, 1024, 0)
    );
    let calls = events();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        &calls[..2],
        [Event::Budget(3), Event::Copy(w.sim.ptr().wrapping_add(0x5C) as usize)]
    );
}

#[test]
fn weighted_daily_attempts_recompute_weights_and_remove_each_selected_developer() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(10.0, 1, 1), (30.0, 0x80, 1)]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.draws = vec![20, 0];
        s.results = vec![1, 0];
        s.list_items = 2;
    });
    w.run();
    let calls = events();
    assert_eq!(
        calls
            .iter()
            .filter_map(|e| if let Event::Random(n) = e { Some(*n) } else { None })
            .collect::<Vec<_>>(),
        [60, 20]
    );
    assert_eq!(
        calls
            .iter()
            .filter_map(|e| if let Event::Type(n) = e { Some(*n) } else { None })
            .collect::<Vec<_>>(),
        [0x1011, 0x1010]
    );
    let profile: Vec<_> = calls
        .iter()
        .filter_map(|e| if let Event::Grow(p) = e { Some(*p) } else { None })
        .collect();
    assert_eq!(
        profile,
        [
            [&mut w._developers[1] as *mut Developer as u32, 10, 67, 0xAABB_CC03],
            [&mut w._developers[0] as *mut Developer as u32, 0, 67, 0xAABB_CC03]
        ]
    );
    assert_eq!(calls.iter().filter(|e| matches!(e, Event::Day)).count(), 1);
    assert_eq!(*w.days, 5000);
    assert_eq!(
        calls
            .iter()
            .filter_map(|e| if let Event::Erase(a, b) = e {
                Some((*a, *b))
            } else {
                None
            })
            .collect::<Vec<_>>(),
        [(0x1011, u32::MAX), (0x1010, u32::MAX)]
    );
    assert_eq!(calls.iter().filter(|e| matches!(e, Event::Free(_))).count(), 7);
}

#[test]
fn failed_growth_conditions_and_success_codes_keep_cleanup_and_date_call_order() {
    let _fpu = GameFpu::new();
    for (can, count, result, attempt, date) in [
        (0x100, 1, 1, false, false),
        (1, 0, 1, false, false),
        (1, 1, -1, true, false),
        (1, 1, 0, true, false),
        (1, 1, i32::MAX, true, true),
    ] {
        let mut w = World::new(&[(6.0, can, count)]);
        STATE.with(|s| s.borrow_mut().results = vec![result]);
        w.run();
        let calls = events();
        assert_eq!(calls.iter().any(|e| matches!(e, Event::Grow(_))), attempt);
        assert_eq!(calls.contains(&Event::Day), date);
        assert!(calls.contains(&Event::Erase(0x1010, u32::MAX)));
        assert_eq!(*w.days, if date { 5000 } else { 1234 });
        if can as u8 == 0 {
            assert!(!calls.iter().any(|e| matches!(e, Event::Find(_))));
        }
    }
}

#[test]
fn tried_exhaustion_clears_the_tree_restarts_at_begin_and_reads_profile_after_marking() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(6.0, 1, 2)]);
    let nested = STATE.with(|s| s.borrow().tried[0].1 as *mut u8);
    let tree = unsafe { Tree::from_ptr(nested) };
    let root = w.object(24);
    tree.set_count(1);
    unsafe { Node::from_ptr(tree.header()) }.set_parent(root);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.missed = true;
        s.mutate = w.candidates[0].as_mut_ptr() as usize;
    });
    w.run();
    let calls = events();
    assert_eq!(tree.count(), 0);
    assert!(unsafe { Node::from_ptr(tree.header()) }.parent().is_null());
    assert!(calls.contains(&Event::DeleteTried(0)));
    assert!(calls.contains(&Event::Mark([0, 67])));
    assert!(calls.contains(&Event::Grow([
        &mut w._developers[0] as *mut Developer as u32,
        9,
        10,
        0xAABB_CC80
    ])));
    assert_eq!(calls.iter().filter(|e| matches!(e, Event::Tried(0x1010))).count(), 2);
    let clear = calls.iter().position(|e| *e == Event::DeleteTried(0)).unwrap();
    let mark = calls.iter().position(|e| matches!(e, Event::Mark(_))).unwrap();
    assert!(clear < mark);
}

#[test]
fn budget_yields_zero_demand_without_erasing_and_cleans_remaining_copy() {
    let _fpu = GameFpu::new();
    for demand in [5.999, -1.0, f64::NAN, f64::INFINITY] {
        let mut w = World::new(&[(demand, 1, 1)]);
        w.globals.pace = 1024;
        STATE.with(|s| s.borrow_mut().expiration = vec![0x100, 1]);
        w.run();
        let calls = events();
        assert_eq!(
            calls
                .iter()
                .filter_map(|e| if let Event::Expired(n) = e { Some(*n) } else { None })
                .collect::<Vec<_>>(),
            [1024, 2048]
        );
        assert!(calls.contains(&Event::Random(0)));
        assert!(!calls.iter().any(|e| matches!(e, Event::Erase(_, _) | Event::Grow(_))));
        assert!(calls.contains(&Event::DeleteDeveloper(0)));
    }
}

#[test]
fn adaptive_pacing_retains_signed_counter_boundaries_and_original_return_word() {
    for (pace, counter, next_pace, next_counter, eax) in [
        (21 * 1024, 15, 21 * 512, 0, 21 * 512),
        (21 * 1024, 0, 21 * 1024, 1, 1),
        (21 * 1024, i32::MAX, 21 * 1024, i32::MIN, i32::MIN as u32),
        (20 * 1024, 4, 20 * 1024, 4, 20),
        (5 * 1024, 4, 5 * 1024, 4, 5),
        (1024, -3, 1024, 0, 1024),
        (512, -3, 1024, 0, 512),
        (1024, i32::MIN, 1024, i32::MAX, i32::MAX as u32),
        (0, -3, 0, -3, 0),
    ] {
        let mut w = World::new(&[(0.0, 1, 0)]);
        w.globals.pace = pace;
        w.globals.adjustment = counter;
        assert_eq!(w.run(), eax);
        assert_eq!((w.globals.pace, w.globals.adjustment), (next_pace, next_counter));
        events();
    }
}

#[test]
fn growth_export_keeps_aligned_stage_padding_and_existing_x87_value() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(6.0, 1, 1)]);
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    // SAFETY: the aligned seed matches the original local profile word and the entry has no stack arguments.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{sentinel}]", "lea eax, [esp - 8]", "and eax, -8",
            "mov dword ptr [eax - 28], 0x11223344", "call {entry}", "fstp qword ptr [edi]",
            sentinel = in(reg) &sentinel, in("edi") &mut restored,
            entry = sym fc4p_0069b0b0, in("ecx") w.sim.ptr(), clobber_abi("C"), out("st(0)") _,
        );
    }
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert!(events().contains(&Event::Grow([
        &mut w._developers[0] as *mut Developer as u32,
        0,
        67,
        0x1122_3303
    ])));
}
