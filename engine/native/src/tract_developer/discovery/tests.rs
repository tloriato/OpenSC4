// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration discovery and sorting contracts exercised through native entries.

use super::*;
use crate::testing::{fake, Fake, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine, EAX, ECX, EDX};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Sort([u32; 5]), Finish([u32; 3]), Construct(u32), Wealth(u32), Purpose(u32),
    Entry(u32, [u32; 2]), Allocate(u32), Register(u32, u32), Query(u32, u32),
    Begin, Suitable(u32, u32), Insert(u32, u32), Release(u32), Next, Destroy, Free(u32),
}
struct State {
    calls: Vec<Call>, slot: u32, allocation: u32, query: u32,
    nodes: [u32; 3], extra_bucket: bool, storage: u32, buckets: u32,
}
impl Default for State {
    fn default() -> Self {
        Self { calls: vec![], slot: 0, allocation: 0, query: 1, nodes: [0; 3],
            extra_bucket: true, storage: 0, buckets: 0 }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
const RESULT: u64 = 0xDEAD_BEEF_1234_5678;
fn log(c: Call) { STATE.with(|s| s.borrow_mut().calls.push(c)); }
extern "C" fn sort(a: u32, b: u32, c: u32, d: u32, e: u32) -> u64 {
    log(Call::Sort([a, b, c, d, e])); RESULT
}
extern "C" fn finish(a: u32, b: u32, c: u32) -> u64 {
    log(Call::Finish([a, b, c])); RESULT
}

#[test]
fn sort_retains_empty_registers_and_calls_both_passes_with_logarithmic_depth() {
    let _machine = PreserveMachine::new();
    let _sort = fake(0x0070_7F20, sort as *const () as usize);
    let _finish = fake(0x0070_6EA0, finish as *const () as usize);
    for (n, depth) in [(0, 0), (1, 0), (2, 2), (3, 2), (4, 4), (15, 6), (16, 8), (17, 8), (1024, 20)] {
        STATE.with(|s| s.borrow_mut().calls.clear());
        let (first, last, compare) = (0x1234_0000, 0x1234_0000 + n * 4, 0x0070_4770);
        let result = run(0x0070_9520, Conv::Cdecl, ECX, &[first, last, compare]);
        STATE.with(|s| {
            let s = s.borrow();
            if n == 0 { assert!(s.calls.is_empty()); assert_eq!(result, (EAX, ECX, EDX)); }
            else {
                assert_eq!(s.calls, [Call::Sort([first, last, 0, depth, compare]), Call::Finish([first, last, compare])]);
                assert_eq!((result.0, result.2), (RESULT as u32, (RESULT >> 32) as u32));
            }
        });
    }
}

extern "thiscall" fn construct(this: u32, hint: u32) -> u64 {
    log(Call::Construct(hint));
    for offset in [4, 8, 12, 16] { assert_eq!(unsafe { word(this, offset) }, 0); }
    STATE.with(|s| {
        let s = &mut *s.borrow_mut(); s.storage = this;
        unsafe { store(this, 0, 0xCCCC_CCCC); store(this, 4, s.buckets); }
    }); RESULT
}
extern "thiscall" fn wealth(this: u32) -> u64 {
    log(Call::Wealth(this)); (RESULT & !0xFFFF_FFFF) | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn purpose(this: u32) -> u64 {
    log(Call::Purpose(this)); (RESULT & !0xFFFF_FFFF) | u64::from(unsafe { word(this, 8) })
}
extern "thiscall" fn entry(this: u32, key: *const [u32; 2]) -> u64 {
    log(Call::Entry(this, unsafe { *key }));
    STATE.with(|s| (RESULT & !0xFFFF_FFFF) | u64::from(s.borrow().slot))
}
extern "C" fn allocate(bytes: u32) -> u64 {
    log(Call::Allocate(bytes)); STATE.with(|s| (RESULT & !0xFFFF_FFFF) | u64::from(s.borrow().allocation))
}
extern "thiscall" fn register(this: u32, filter: u32) -> u64 { log(Call::Register(this, filter)); RESULT }
extern "thiscall" fn query(this: u32, set: u32, filter: u32) -> u64 {
    log(Call::Query(this, filter));
    STATE.with(|s| { let s = s.borrow(); assert_eq!(set, s.storage); (RESULT & !0xFFFF_FFFF) | u64::from(s.query) })
}
extern "thiscall" fn begin(this: u32, iterator: *mut [u32; 2]) -> u64 {
    log(Call::Begin);
    STATE.with(|s| { let s = s.borrow(); assert_eq!(this, s.storage); unsafe { *iterator = [s.nodes[0], 0x1234_5678]; } });
    RESULT
}
extern "thiscall" fn suitable(this: u32, configuration: u32) -> u64 {
    log(Call::Suitable(this, configuration));
    (RESULT & !0xFFFF_FFFF) | u64::from(unsafe { word(configuration, 4) })
}
extern "thiscall" fn insert(this: u32, result: *mut u32, configuration: *mut u32) -> u64 {
    log(Call::Insert(this, unsafe { *configuration }));
    unsafe { *result = 0xDEAD_1111; *result.add(1) = 1; *configuration = 0; } RESULT
}
extern "thiscall" fn release(this: u32) -> u64 { log(Call::Release(this)); RESULT }
extern "thiscall" fn next(this: *mut u32) -> u64 {
    log(Call::Next);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut(); assert_eq!(unsafe { *this.add(1) }, 0x1234_5678);
        let value = if s.extra_bucket { s.nodes[2] } else { 0 }; s.extra_bucket = false;
        (RESULT & !0xFFFF_FFFF) | u64::from(value)
    })
}
extern "thiscall" fn destroy(this: u32) -> u64 {
    log(Call::Destroy); STATE.with(|s| assert_eq!(this, s.borrow().storage)); RESULT
}
extern "C" fn free(pointer: u32) -> u64 { log(Call::Free(pointer)); RESULT }

struct World {
    td: Object, developer: Box<[u32; 3]>, manager: Box<[u32; 1]>,
    tables: [Box<[usize; 20]>; 3], slot: Box<u32>, filter: Box<[u32; 3]>,
    buckets: Box<[u32; 4]>, nodes: Box<[[u32; 2]; 3]>, configurations: Box<[[u32; 2]; 3]>,
    debug: Object, histogram: Box<[u32; 28]>, _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180), developer: Box::new([0, 2, 3]), manager: Box::new([0]),
            tables: core::array::from_fn(|_| Box::new([0; 20])), slot: Box::new(0), filter: Box::new([0xCCCC_CCCC; 3]),
            buckets: Box::new([0; 4]), nodes: Box::new([[0; 2]; 3]), configurations: Box::new([[0; 2]; 3]),
            debug: Object::new(0x90), histogram: Box::new([u32::MAX; 28]), _fakes: vec![],
        };
        w.tables[0][0x20 / 4] = wealth as *const () as usize;
        w.tables[0][0x1C / 4] = purpose as *const () as usize;
        w.tables[1][0x48 / 4] = register as *const () as usize;
        w.tables[1][0x24 / 4] = query as *const () as usize;
        w.tables[2][2] = release as *const () as usize;
        w.developer[0] = w.tables[0].as_ptr() as u32; w.manager[0] = w.tables[1].as_ptr() as u32;
        for (i, code) in [5, 27, 10].into_iter().enumerate() {
            w.configurations[i] = [w.tables[2].as_ptr() as u32, code];
            w.nodes[i][1] = w.configurations[i].as_ptr() as u32;
        }
        w.nodes[0][0] = w.nodes[1].as_ptr() as u32;
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_developer(w.developer.as_ptr() as u32); td.set_debug_context(w.debug.ptr() as u32);
        unsafe { store(w.td.ptr() as u32, 0x24, w.manager.as_ptr() as u32); store(w.debug.ptr() as u32, 0x18, w.histogram.as_ptr() as u32); }
        STATE.with(|s| {
            let s = &mut *s.borrow_mut(); s.slot = w.slot.as_mut() as *mut u32 as u32;
            s.allocation = w.filter.as_mut_ptr() as u32; s.buckets = w.buckets.as_ptr() as u32;
            s.nodes = core::array::from_fn(|i| w.nodes[i].as_ptr() as u32);
        });
        for (address, f) in [(0x0082_9BC0, construct as *const () as usize), (0x0070_74D0, entry as *const () as usize),
            (0x005E_55E0, allocate as *const () as usize), (0x008E_BA60, destroy as *const () as usize),
            (0x0090_CF63, free as *const () as usize), (0x004B_0390, begin as *const () as usize),
            (0x0070_4C00, suitable as *const () as usize), (0x0049_A050, insert as *const () as usize),
            (0x0047_C1A0, next as *const () as usize)] { w._fakes.push(fake(address, f)); }
        w
    }
    fn call(&mut self) -> (u32, u32, u32) { run(0x0070_9650, Conv::Thiscall, self.td.ptr() as u32, &[]) }
    fn prefix(&mut self, cached: bool) -> Vec<Call> {
        let d = self.developer.as_ptr() as u32; let m = self.manager.as_ptr() as u32;
        let f = self.filter.as_ptr() as u32;
        let mut v = vec![Call::Construct(100), Call::Wealth(d), Call::Purpose(d), Call::Entry(self.td.ptr() as u32 + 0x130, [2, 3])];
        if !cached { v.extend([Call::Allocate(12), Call::Wealth(d), Call::Purpose(d), Call::Register(m, f)]); }
        v.push(Call::Query(m, f)); v
    }
}

#[test]
fn configurations_cache_filter_keep_successes_release_in_hash_order_and_count_all_results() {
    let _machine = PreserveMachine::new(); let mut w = World::new();
    let before = snapshot(w.td.ptr(), 0x180);
    let regs = w.call(); assert_eq!((regs.0, regs.2), (27, (RESULT >> 32) as u32));
    assert_eq!(*w.filter, [0x00AB_30DC, 2, 3]); assert_eq!(*w.slot, w.filter.as_ptr() as u32);
    for code in 0..28 { assert_eq!(w.histogram[code], if [5, 10, 27].contains(&code) { 0 } else { u32::MAX }); }
    assert_eq!(snapshot(w.td.ptr(), 0x180), before);
    let mut expected = w.prefix(false); expected.push(Call::Begin);
    for i in 0..3 {
        let c = w.configurations[i].as_ptr() as u32;
        expected.push(Call::Suitable(w.td.ptr() as u32, c));
        if i == 1 { expected.push(Call::Insert(w.td.ptr() as u32 + 0xD8, c)); }
        expected.push(Call::Release(c));
        if i >= 1 { expected.push(Call::Next); }
    }
    expected.extend([Call::Destroy, Call::Free(w.buckets.as_ptr() as u32)]);
    STATE.with(|s| assert_eq!(s.borrow().calls, expected));
    STATE.with(|s| { let s = &mut *s.borrow_mut(); s.calls.clear(); s.nodes[0] = 0; });
    assert_eq!(w.call().0, 0);
    let mut expected = w.prefix(true); expected.extend([Call::Begin, Call::Destroy, Call::Free(w.buckets.as_ptr() as u32)]);
    STATE.with(|s| assert_eq!(s.borrow().calls, expected));
}

#[test]
fn query_failure_cleans_up_and_returns_five_without_visiting_or_counting() {
    for buckets_present in [false, true] {
        let mut w = World::new();
        STATE.with(|s| { let s = &mut *s.borrow_mut(); s.query = 0; if !buckets_present { s.buckets = 0; } });
        assert_eq!(w.call().0, 5); assert_eq!(*w.histogram, [u32::MAX; 28]);
        let mut expected = w.prefix(false); expected.push(Call::Destroy);
        if buckets_present { expected.push(Call::Free(w.buckets.as_ptr() as u32)); }
        STATE.with(|s| assert_eq!(s.borrow().calls, expected));
    }
}

#[test]
fn failed_filter_allocation_still_registers_and_queries_null_and_retries_later() {
    let mut w = World::new();
    STATE.with(|s| { let s = &mut *s.borrow_mut(); s.allocation = 0; s.query = 0; });
    for _ in 0..2 {
        STATE.with(|s| s.borrow_mut().calls.clear()); assert_eq!(w.call().0, 5); assert_eq!(*w.slot, 0);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Wealth(_) | Call::Purpose(_))).count(), 2);
            assert!(s.calls.contains(&Call::Register(w.manager.as_ptr() as u32, 0)));
            assert!(s.calls.contains(&Call::Query(w.manager.as_ptr() as u32, 0)));
        });
    }
}
