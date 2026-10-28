// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native versions of the core's utility and garbage selection cases.

use super::*;
use crate::abi::Conv;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Framework,
    Service(u32, u32),
    ReleaseLocal(bool),
    Allocate(u32),
    Filter(u32, u32),
    Other(usize),
    Active(usize),
    Erase(usize),
    Destroy(Vec<usize>, u32),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static FILTERS: RefCell<Vec<(u32, Vec<usize>)>> = const { RefCell::new(Vec::new()) };
    #[allow(clippy::vec_box)]
    static NODES: RefCell<Vec<Box<[u32; 3]>>> = const { RefCell::new(Vec::new()) };
    static ALLOCATOR: RefCell<usize> = const { RefCell::new(0) };
    static FRAMEWORK: RefCell<usize> = const { RefCell::new(0) };
}

fn record(call: Call) { CALLS.with(|c| c.borrow_mut().push(call)); }
fn take() -> Vec<Call> { CALLS.with(|c| core::mem::take(&mut *c.borrow_mut())) }
fn node() -> *mut u8 {
    NODES.with(|n| { let mut n = n.borrow_mut(); n.push(Box::new([0xA5A5_A5A5; 3])); n.last_mut().unwrap().as_mut_ptr().cast() })
}

extern "C" fn framework() -> u64 { record(Call::Framework); pair(FRAMEWORK.with(|f| *f.borrow()) as u32, 0x1111_1111) }
extern "thiscall" fn service(_this: *mut u8, service: u32, iid: u32, out: *mut *mut u8) -> u64 {
    record(Call::Service(service, iid));
    unsafe { out.write(ALLOCATOR.with(|a| *a.borrow()) as *mut u8); }
    pair(1, 0x2222_2222)
}
extern "thiscall" fn release_local(this: *mut *mut u8) -> u64 {
    record(Call::ReleaseLocal(unsafe { !(*this).is_null() }));
    // The cached service must be published before its temporary reference is released.
    assert_eq!(unsafe { connection_allocator().read() }, unsafe { this.read() });
    unsafe { this.write(core::ptr::null_mut()); }
    pair(0x3333_3333, 0x4444_4444)
}
extern "thiscall" fn allocate(_this: *mut u8, bytes: u32) -> u64 {
    record(Call::Allocate(bytes));
    pair(node() as u32, 0x5555_5555)
}

extern "thiscall" fn filtered(_this: *mut u8, kind: u32, city: u32, list: *mut *mut u8) -> u64 {
    record(Call::Filter(kind, city));
    let header = unsafe { list.read() };
    assert_eq!(unsafe { pointer(header, 0) }, header);
    assert_eq!(unsafe { pointer(header, 4) }, header);
    assert_eq!(unsafe { word(header, 8) }, 0xA5A5_A5A5);
    FILTERS.with(|f| {
        for &object in f.borrow().iter().find(|(k, _)| *k == kind).map(|(_, v)| v.as_slice()).unwrap_or(&[]) {
            let n = node();
            unsafe {
                let prev = pointer(header, 4);
                n.cast::<*mut u8>().write(header);
                n.add(4).cast::<*mut u8>().write(prev);
                n.add(8).cast::<usize>().write(object);
                prev.cast::<*mut u8>().write(n);
                header.add(4).cast::<*mut u8>().write(n);
            }
        }
    });
    pair(0x6666_6666, 0x7777_7777)
}

#[repr(C)]
struct Connection { table: *const usize, active: u32, other: *mut Connection }
extern "thiscall" fn other(_this: *mut u8, connection: *mut Connection) -> u64 {
    record(Call::Other(connection as usize));
    pair(unsafe { (*connection).other } as u32, 0x8888_8888)
}
extern "thiscall" fn active(this: *mut Connection) -> u64 {
    record(Call::Active(this as usize));
    pair(unsafe { (*this).active }, 0x9999_9999)
}
extern "thiscall" fn erase(_this: *mut *mut u8, out: *mut u32, n: *mut u8) -> u64 {
    record(Call::Erase(unsafe { word(n, 8) } as usize));
    unsafe {
        let next = pointer(n, 0);
        let prev = pointer(n, 4);
        prev.cast::<*mut u8>().write(next);
        next.add(4).cast::<*mut u8>().write(prev);
        out.write(next as u32);
    }
    pair(out as u32, 0xAAAA_AAAA)
}
extern "fastcall" fn destroy(this: *mut *mut u8, edx: u32) -> u64 {
    let header = unsafe { this.read() };
    let mut n = unsafe { pointer(header, 0) };
    let mut objects = Vec::new();
    while n != header { objects.push(unsafe { word(n, 8) } as usize); n = unsafe { pointer(n, 0) }; }
    record(Call::Destroy(objects, edx));
    pair(0xBBBB_BBBB, 0xCCCC_CCCC)
}

struct World {
    sim: Box<[u32; 18]>,
    connections: [Connection; 3],
    _tables: [Box<[usize; 24]>; 4],
    _cache: Box<*mut u8>,
    _allocator: Box<usize>,
    _framework: Box<usize>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new(cached: bool) -> Self {
        let mut tables = [Box::new([0usize; 24]), Box::new([0; 24]), Box::new([0; 24]), Box::new([0; 24])];
        tables[0][0x48 / 4] = filtered as *const () as usize;
        tables[0][0x34 / 4] = other as *const () as usize;
        tables[1][0x34 / 4] = active as *const () as usize;
        tables[2][0x0C / 4] = allocate as *const () as usize;
        tables[3][0x14 / 4] = service as *const () as usize;
        let mut sim = Box::new([0xA5A5_A5A5; 18]); sim[0] = tables[0].as_ptr() as u32;
        let mut allocator = Box::new(tables[2].as_ptr() as usize);
        let mut framework_object = Box::new(tables[3].as_ptr() as usize);
        let allocator_p = (&mut *allocator as *mut usize).cast::<u8>();
        ALLOCATOR.with(|a| *a.borrow_mut() = allocator_p as usize);
        FRAMEWORK.with(|f| *f.borrow_mut() = &mut *framework_object as *mut usize as usize);
        let mut cache = Box::new(if cached { allocator_p } else { core::ptr::null_mut() });
        let fakes = vec![fake_data(0x00B2_07DC, (&mut *cache as *mut *mut u8).cast()), fake(0x0087_93EC, framework as *const () as usize), fake(0x0041_1870, release_local as *const () as usize), fake(0x0041_C000, erase as *const () as usize), fake(0x0041_1F80, destroy as *const () as usize)];
        FILTERS.with(|f| f.borrow_mut().clear()); NODES.with(|n| n.borrow_mut().clear());
        Self { sim, connections: core::array::from_fn(|_| Connection { table: tables[1].as_ptr(), active: 0xABCD_EF00, other: core::ptr::null_mut() }), _tables: tables, _cache: cache, _allocator: allocator, _framework: framework_object, _fakes: fakes }
    }
    fn connection(&mut self, i: usize) -> usize { &mut self.connections[i] as *mut _ as usize }
    fn filter(&mut self, kind: u32, indices: &[usize]) {
        let objects = indices.iter().map(|&i| self.connection(i)).collect();
        FILTERS.with(|f| f.borrow_mut().push((kind, objects)));
    }
    fn run(&mut self, kind: u32) -> (u64, u32) {
        let mut args = [kind, 0xDEAD_BEEF]; let before = *self.sim;
        let result = unsafe { checked_call(fc4p_006d1fc0 as *const () as usize, self.sim.as_mut_ptr().cast(), &mut args, 0x0123_4567, 0x0BAD_F00D, Conv::Thiscall) };
        assert_eq!(*self.sim, before); assert_eq!(args[0], kind);
        (result, args[1])
    }
}

#[test]
fn garbage_search_priority_skips_activity_and_stops_at_first_nonempty_type() {
    for cached in [false, true] {
        for first in 0..=5 {
            let mut w = World::new(cached);
            let priority = [3, 12, 8, 2, 10];
            for &kind in priority.iter().skip(first) { w.filter(kind, &[0, 1]); }
            for deal in [0x4773_FFE4, 0x4773_FFE5] {
                let (result, arg) = w.run(deal);
                let expected = if first == 5 { 0 } else { w.connection(0) as u32 };
                assert_eq!((result, arg), (pair(expected, 0xCCCC_CCCC), 0xDEAD_BEEF));
                let mut calls = Vec::new();
                if !cached && deal == 0x4773_FFE4 { calls.extend([Call::Framework, Call::Service(0x3AE4_BEA3, 0x3AE4_BEAB), Call::ReleaseLocal(true)]); }
                calls.push(Call::Allocate(12));
                calls.extend(priority.iter().take((first + 1).min(5)).map(|&k| Call::Filter(k, 0xDEAD_BEEF)));
                calls.push(Call::Destroy(if first == 5 { vec![] } else { vec![w.connection(0), w.connection(1)] }, 0x7777_7777));
                assert_eq!(take(), calls);
            }
        }
    }
}

#[test]
fn utility_buy_checks_remote_activity_and_sell_checks_local_after_remote_lookup() {
    for kind in 0x4773_FFE0..=0x4773_FFE3 {
        for active_remote in [false, true] {
            let mut w = World::new(true);
            w.filter(if kind < 0x4773_FFE2 { 4 } else { 5 }, &[0, 1]);
            w.connections[0].other = core::ptr::null_mut();
            w.connections[1].other = &mut w.connections[2];
            w.connections[1].active = if active_remote { 0xABCD_EF80 } else { 0xABCD_EF00 };
            w.connections[2].active = w.connections[1].active;
            let result = w.run(kind);
            let winner = if active_remote { w.connection(1) } else { 0 };
            assert_eq!(result.0, pair(winner as u32, 0xCCCC_CCCC));
            assert_ne!(result.1, 0xDEAD_BEEF);
            let mut calls = vec![Call::Allocate(12), Call::Filter(if kind < 0x4773_FFE2 { 4 } else { 5 }, 0xDEAD_BEEF), Call::Other(w.connection(0))];
            if kind & 1 != 0 { calls.push(Call::Active(w.connection(0))); }
            calls.extend([Call::Erase(w.connection(0)), Call::Other(w.connection(1)), Call::Active(w.connection(if kind & 1 != 0 { 1 } else { 2 }))]);
            if !active_remote { calls.push(Call::Erase(w.connection(1))); }
            calls.push(Call::Destroy(if active_remote { vec![w.connection(1)] } else { vec![] }, if active_remote { 0x9999_9999 } else { 0xAAAA_AAAA }));
            assert_eq!(take(), calls);
        }
    }
}

#[test]
fn immediate_local_winner_needs_no_remote_connection_and_keeps_the_city_argument() {
    let mut w = World::new(true);
    w.filter(4, &[0, 1]);
    w.connections[0].active = 0xAABB_CC80;
    assert_eq!(w.run(0x4773_FFE1), (pair(w.connection(0) as u32, 0xCCCC_CCCC), 0xDEAD_BEEF));
    assert_eq!(take(), [Call::Allocate(12), Call::Filter(4, 0xDEAD_BEEF), Call::Other(w.connection(0)), Call::Active(w.connection(0)), Call::Destroy(vec![w.connection(0), w.connection(1)], 0x9999_9999)]);
}

#[test]
fn unsupported_types_and_empty_utilities_still_allocate_and_destroy_the_list() {
    let mut w = World::new(true);
    for kind in [0, u32::MAX, 0x4773_FFDF, 0x4773_FFE6] {
        assert_eq!(w.run(kind), (pair(0, 0xCCCC_CCCC), 0xDEAD_BEEF));
        assert_eq!(take(), [Call::Allocate(12), Call::Destroy(vec![], 0x5555_5555)]);
    }
    assert_eq!(w.run(0x4773_FFE0), (pair(0, 0xCCCC_CCCC), 0xDEAD_BEEF));
    assert_eq!(take(), [Call::Allocate(12), Call::Filter(4, 0xDEAD_BEEF), Call::Destroy(vec![], 0x7777_7777)]);
}
