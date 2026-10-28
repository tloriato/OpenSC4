// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout versions of the core's breadth-first connected-city cases.

use super::*;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Clear,
    Current,
    Ref(u32),
    Release(u32),
    Node(u32, u32),
    Allocate(u32),
    Free,
    Connections(u32),
    Kind(u32),
    City(u32),
    ClearConnections,
    Erase,
    Established(u32),
    Framework,
    Service,
    ReleaseLocal,
}
#[repr(C)]
struct City {
    table: *const usize,
    id: u32,
    established: u32,
}
#[repr(C)]
struct Connection {
    table: *const usize,
    kind: u32,
    city: *mut City,
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    current: usize,
    allocator: usize,
    edges: Vec<(u32, Vec<usize>)>,
    references: Vec<(u32, i32)>,
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 3]>>,
    invalidate: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
fn node() -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.nodes.push(Box::new([0xA5A5_A5A5; 3]));
        s.nodes.last_mut().unwrap().as_mut_ptr().cast()
    })
}
unsafe fn append(list: *mut u8, object: *mut u8) {
    unsafe {
        let n = node();
        n.add(8).cast::<*mut u8>().write_unaligned(object);
        link(pointer(list, 0), n);
    }
}
fn reference(city: *mut City, delta: i32) {
    if city.is_null() {
        return;
    }
    let id = unsafe { (*city).id };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(r) = s.references.iter_mut().find(|r| r.0 == id) {
            r.1 += delta;
        } else {
            s.references.push((id, delta));
        }
    });
}
extern "thiscall" fn addref(city: *mut City) -> u64 {
    record(Call::Ref(unsafe { (*city).id }));
    reference(city, 1);
    0x1122_3344_5566_7788
}
extern "thiscall" fn release(city: *mut City) -> u64 {
    record(Call::Release(unsafe { (*city).id }));
    reference(city, -1);
    0x2233_4455_6677_8899
}
extern "thiscall" fn established(city: *mut City) -> u64 {
    record(Call::Established(unsafe { (*city).id }));
    pair(0xAABB_CC00 | unsafe { (*city).established }, 0x1234_5678)
}
extern "fastcall" fn clear_cities(list: *mut u8, _: u32) -> u64 {
    record(Call::Clear);
    unsafe {
        let header = pointer(list, 0);
        let mut p = pointer(header, 0);
        while p != header {
            reference(pointer(p, 8).cast(), -1);
            p = pointer(p, 0);
        }
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
    }
    0x1111_2222_3333_4444
}
extern "thiscall" fn current(_: *mut u8) -> u32 {
    record(Call::Current);
    STATE.with(|s| s.borrow().current as u32)
}
extern "fastcall" fn allocate_city(_: *mut u8, _: u32, count: u32, hint: u32) -> u32 {
    record(Call::Node(count, hint));
    node() as u32
}
extern "thiscall" fn allocate(_: *mut u8, size: u32) -> u32 {
    record(Call::Allocate(size));
    assert_eq!(size, 12);
    node() as u32
}
extern "thiscall" fn free(_: *mut u8, _: *mut u8) -> u64 {
    record(Call::Free);
    0x3344_5566_7788_99AA
}
extern "C" fn framework() -> u32 {
    record(Call::Framework);
    STATE.with(|s| s.borrow().allocator as u32)
}
extern "thiscall" fn service(_: *mut u8, a: u32, b: u32, out: *mut u32) {
    assert_eq!((a, b), (0x3AE4_BEA3, 0x3AE4_BEAB));
    record(Call::Service);
    unsafe {
        assert_eq!(out.read_unaligned(), 0);
        out.write_unaligned(STATE.with(|s| s.borrow().allocator as u32));
    }
}
extern "fastcall" fn release_local(local: *mut u32, edx: u32) -> u64 {
    record(Call::ReleaseLocal);
    assert_eq!(unsafe { local.read_unaligned() }, edx);
    unsafe {
        local.write_unaligned(0);
    }
    0x4455_6677_8899_AABB
}
extern "thiscall" fn connections(city: *mut City, list: *mut u8) {
    let id = unsafe { (*city).id };
    record(Call::Connections(id));
    let edges = STATE.with(|s| {
        s.borrow()
            .edges
            .iter()
            .find(|e| e.0 == id)
            .map_or(vec![], |e| e.1.clone())
    });
    for connection in edges {
        unsafe {
            append(list, connection as *mut u8);
        }
    }
}
extern "thiscall" fn kind(connection: *mut Connection) -> u32 {
    let kind = unsafe { (*connection).kind };
    record(Call::Kind(kind));
    kind
}
extern "thiscall" fn other_city(connection: *mut Connection) -> u32 {
    let city = unsafe { (*connection).city };
    record(Call::City(if city.is_null() { 0 } else { unsafe { (*city).id } }));
    city as u32
}
extern "fastcall" fn clear_connections(list: *mut u32, _: u32) -> u64 {
    record(Call::ClearConnections);
    unsafe {
        let header = list.read_unaligned() as *mut u8;
        assert_eq!(word(header, 8), 0xA5A5_A5A5);
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
    }
    0x5566_7788_99AA_BBCC
}
extern "fastcall" fn erase(list: *mut u8, _: u32, out: *mut u32, first: *mut u8) -> u64 {
    record(Call::Erase);
    unsafe {
        assert_eq!(pointer(first, 8) as usize, STATE.with(|s| s.borrow().current));
        assert_eq!(pointer(pointer(list, 0), 0), first);
        let next = pointer(first, 0);
        let previous = pointer(first, 4);
        previous.cast::<*mut u8>().write_unaligned(next);
        next.add(4).cast::<*mut u8>().write_unaligned(previous);
        reference(pointer(first, 8).cast(), -1);
        out.write_unaligned(next as u32);
        STATE.with(|s| {
            let cache = s.borrow().invalidate;
            if cache != 0 {
                (cache as *mut u32).write(0);
            }
        });
    }
    0x6677_8899_AABB_CCDD
}

// All boxed objects are kept at stable addresses while their ownership vectors grow.
#[allow(clippy::vec_box)]
struct World {
    cities: Vec<Box<City>>,
    connections: Vec<Box<Connection>>,
    table: Box<[usize; 70]>,
    connection_table: Box<[usize; 10]>,
    allocator_table: Box<[usize; 6]>,
    manager: Box<usize>,
    allocator: Box<usize>,
    globals: Vec<Box<usize>>,
    output_header: Box<[u32; 3]>,
    output: Box<usize>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(cached: bool) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            cities: vec![],
            connections: vec![],
            table: Box::new([0; 70]),
            connection_table: Box::new([0; 10]),
            allocator_table: Box::new([0; 6]),
            manager: Box::new(0),
            allocator: Box::new(0),
            globals: vec![],
            output_header: Box::new([0; 3]),
            output: Box::new(0),
            _fakes: vec![],
        };
        w.table[1] = addref as *const () as usize;
        w.table[2] = release as *const () as usize;
        w.table[0x40 / 4] = connections as *const () as usize;
        w.table[0x54 / 4] = connections as *const () as usize;
        w.table[0x90 / 4] = current as *const () as usize;
        w.table[0xAC / 4] = established as *const () as usize;
        w.connection_table[0x14 / 4] = kind as *const () as usize;
        w.connection_table[0x24 / 4] = other_city as *const () as usize;
        w.allocator_table[0x0C / 4] = allocate as *const () as usize;
        w.allocator_table[0x10 / 4] = free as *const () as usize;
        w.allocator_table[0x14 / 4] = service as *const () as usize;
        *w.manager = w.table.as_ptr() as usize;
        *w.allocator = w.allocator_table.as_ptr() as usize;
        let a = &mut *w.allocator as *mut usize as usize;
        for (address, value) in [
            (0x00B4_3C94, &mut *w.manager as *mut usize as usize),
            (0x00B2_07DC, if cached { a } else { 0 }),
            (0x00B2_07E0, if cached { a } else { 0 }),
        ] {
            let mut value = Box::new(value);
            w._fakes.push(fake_data(address, (&mut *value as *mut usize).cast()));
            w.globals.push(value);
        }
        for (address, f) in [
            (0x0041_1DF0, clear_cities as *const () as usize),
            (0x0041_1B60, allocate_city as *const () as usize),
            (0x0041_1D70, clear_connections as *const () as usize),
            (0x004A_4A50, erase as *const () as usize),
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_local as *const () as usize),
        ] {
            w._fakes.push(fake(address, f));
        }
        let header = w.output_header.as_mut_ptr() as u32;
        *w.output_header = [header, header, 0xDEAD_BEEF];
        *w.output = header as usize;
        STATE.with(|s| s.borrow_mut().allocator = a);
        w.city(1, 1);
        STATE.with(|s| s.borrow_mut().current = &mut *w.cities[0] as *mut City as usize);
        w
    }
    fn city(&mut self, id: u32, established: u32) {
        self.cities.push(Box::new(City {
            table: self.table.as_ptr(),
            id,
            established,
        }));
    }
    fn edge(&mut self, from: usize, to: Option<usize>, kind: u32) {
        let city = to.map_or(core::ptr::null_mut(), |i| &mut *self.cities[i] as *mut City);
        self.connections.push(Box::new(Connection {
            table: self.connection_table.as_ptr(),
            kind,
            city,
        }));
        let connection = &mut **self.connections.last_mut().unwrap() as *mut Connection as usize;
        let id = self.cities[from].id;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(e) = s.edges.iter_mut().find(|e| e.0 == id) {
                e.1.push(connection);
            } else {
                s.edges.push((id, vec![connection]));
            }
        });
    }
    fn run(&mut self, include: u32) -> (u64, [u32; 2]) {
        let mut args = [&mut *self.output as *mut usize as u32, include];
        let result = unsafe {
            checked_call(
                fc4p_006d1ab0 as *const () as usize,
                (&mut *self.cities[0] as *mut City).cast(),
                &mut args,
                0xFACE_CAFE,
                0x1122_3344,
                crate::abi::Conv::Thiscall,
            )
        };
        (result, args)
    }
    fn values(&self) -> Vec<u32> {
        let mut values = vec![];
        unsafe {
            let h = *self.output as *mut u8;
            let mut p = pointer(h, 0);
            while p != h {
                values.push((*(pointer(p, 8).cast::<City>())).id);
                p = pointer(p, 0);
            }
        }
        values
    }
    fn check_references(&self, ids: &[u32]) {
        for city in &self.cities {
            let r = STATE.with(|s| s.borrow().references.iter().find(|r| r.0 == city.id).map_or(0, |r| r.1));
            assert_eq!(r, i32::from(ids.contains(&city.id)), "city {}", city.id);
        }
    }
}

#[test]
fn breadth_first_order_includes_links_through_unestablished_cities() {
    for include in [0, 1, 0x80, 0x100] {
        let mut w = World::new(true);
        for (id, established) in [(2, 1), (3, 1), (4, 0), (9, 1)] {
            w.city(id, established);
        }
        for (from, to, kind) in [
            (0, 1, 2),
            (0, 2, 3),
            (0, 3, 1),
            (1, 0, 2),
            (1, 2, 2),
            (2, 4, 2),
            (3, 4, 2),
            (4, 0, 1),
        ] {
            w.edge(from, Some(to), kind);
        }
        w.edge(0, Some(1), 4);
        w.edge(0, Some(1), 5);
        w.edge(0, None, 2);
        let (_, args) = w.run(include);
        let expected = if include as u8 != 0 {
            vec![2, 3, 4, 9]
        } else {
            vec![2, 3, 9]
        };
        assert_eq!(w.values(), expected);
        w.check_references(&expected);
        assert_eq!(args[0], unsafe { pointer(*w.output as *mut u8, 0) } as u32);
        let calls = take();
        assert_eq!(
            calls
                .iter()
                .filter_map(|c| if let Call::Connections(id) = c { Some(*id) } else { None })
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 9]
        );
        let erase = calls.iter().position(|c| *c == Call::Erase).unwrap();
        assert!(!calls[..erase].iter().any(|c| matches!(c, Call::Established(_))));
        assert_eq!(
            calls[erase + 1..]
                .iter()
                .filter_map(|c| if let Call::Established(id) = c { Some(*id) } else { None })
                .collect::<Vec<_>>(),
            if include as u8 == 0 { vec![2, 3, 4, 9] } else { vec![] }
        );
    }
}

#[test]
fn every_transport_kind_and_unknown_kinds_take_the_same_branches() {
    for kind in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, u32::MAX] {
        let mut w = World::new(true);
        w.city(2, 1);
        w.edge(0, Some(1), kind);
        let (result, _) = w.run(1);
        let transport = matches!(kind, 1..=3 | 6..=12);
        assert_eq!(w.values(), if transport { vec![2] } else { vec![] });
        assert_eq!(result, 0x6677_8899_AABB_CC01);
        let calls = take();
        assert_eq!(
            calls.iter().filter(|c| matches!(c, Call::City(_))).count(),
            usize::from(transport)
        );
        assert!(!calls.iter().any(|c| matches!(c, Call::Established(_))));
    }
}

#[test]
fn empty_graph_and_establishment_results_keep_return_registers() {
    let mut empty = World::new(true);
    let (result, args) = empty.run(0);
    assert_eq!(result, pair(*empty.output as u32, 0x6677_8899));
    assert_eq!(args, [*empty.output as u32, 0]);
    empty.check_references(&[]);
    assert_eq!(
        take(),
        [
            Call::Clear,
            Call::Current,
            Call::Ref(1),
            Call::Node(1, 0),
            Call::Ref(1),
            Call::Release(1),
            Call::Allocate(12),
            Call::Connections(1),
            Call::ClearConnections,
            Call::Free,
            Call::Erase
        ]
    );
    for flag in [0, 1, 0x80] {
        let mut w = World::new(true);
        w.city(2, flag);
        w.edge(0, Some(1), 2);
        let (result, _) = w.run(0);
        assert_eq!(
            result,
            if flag == 0 {
                0x3344_5566_7788_99AA
            } else {
                pair(0xAABB_CC00 | flag, 0x1234_5678)
            }
        );
        w.check_references(if flag == 0 { &[] } else { &[2] });
    }
}

#[test]
fn cold_allocators_and_filter_cleanup_reacquire_the_service() {
    let mut w = World::new(false);
    w.city(2, 0);
    w.edge(0, Some(1), 2);
    STATE.with(|s| s.borrow_mut().invalidate = &mut *w.globals[2] as *mut usize as usize);
    let (_, args) = w.run(0);
    let calls = take();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Framework)).count(), 3);
    assert_eq!(args[1], 0); // The temporary reference destructor clears this reused argument word.
    assert_eq!(w.values(), []);
    w.check_references(&[]);
}

#[test]
fn absent_current_city_still_walks_the_simulators_connections() {
    let mut w = World::new(true);
    w.city(2, 1);
    w.edge(0, Some(1), 2);
    STATE.with(|s| s.borrow_mut().current = 0);
    let (_, _) = w.run(0);
    assert_eq!(w.values(), [2]);
    w.check_references(&[2]);
    let calls = take();
    assert_eq!(
        calls
            .iter()
            .filter_map(|c| if let Call::Connections(id) = c { Some(*id) } else { None })
            .collect::<Vec<_>>(),
        [1, 2]
    );
}
