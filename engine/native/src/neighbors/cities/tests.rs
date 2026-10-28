// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout versions of the core's adjacent and connected city cases.

use super::*;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    ClearOutput,
    AllocateTree(u32),
    Region,
    Current,
    Position,
    Size,
    At(i32, i32),
    Established(u32),
    Insert(u32),
    AddRef(u32),
    Release(u32),
    Framework,
    Service,
    ReleaseLocal,
    AllocateNode(u32),
    Subtree(u32),
    FreeTree(u32),
    AllConnections,
    Kind(u32),
    City(u32),
    CityNode(u32, u32),
    ClearConnections,
    FreeNode,
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
    position: [i32; 2],
    size: [i32; 2],
    cells: Vec<([i32; 2], usize)>,
    current: usize,
    allocator: usize,
    framework: usize,
    region: usize,
    connections: Vec<usize>,
    invalidate: bool,
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 5]>>,
    references: Vec<(u32, i32)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
fn allocate() -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.nodes.push(Box::new([0xA5A5_A5A5; 5]));
        s.nodes.last_mut().unwrap().as_mut_ptr().cast()
    })
}
fn id(p: *mut u8) -> u32 {
    if p.is_null() {
        0
    } else {
        unsafe { (*(p as *mut City)).id }
    }
}
fn tree_id(p: *mut u8) -> u32 {
    if p.is_null() {
        0
    } else if unsafe { word(p, 0x10) } == 0xA5A5_A5A5 {
        u32::MAX
    } else {
        id(unsafe { pointer(p, 0x10) })
    }
}

unsafe fn reset_list(list: *mut u8) {
    unsafe {
        let h = pointer(list, 0);
        h.cast::<*mut u8>().write(h);
        h.add(4).cast::<*mut u8>().write(h);
    }
}
unsafe fn append(list: *mut u8, object: usize) {
    let n = allocate();
    unsafe {
        let h = pointer(list, 0);
        let prev = pointer(h, 4);
        n.cast::<*mut u8>().write(h);
        n.add(4).cast::<*mut u8>().write(prev);
        n.add(8).cast::<usize>().write(object);
        prev.cast::<*mut u8>().write(n);
        h.add(4).cast::<*mut u8>().write(n);
    }
}
extern "thiscall" fn clear_output(list: *mut u8) -> u64 {
    record(Call::ClearOutput);
    unsafe {
        reset_list(list);
    }
    pair(1, 2)
}
extern "C" fn allocate_tree(bytes: u32) -> u64 {
    record(Call::AllocateTree(bytes));
    pair(allocate() as u32, 3)
}
extern "thiscall" fn region(_this: *mut u8) -> u64 {
    record(Call::Region);
    pair(STATE.with(|s| s.borrow().region) as u32, 4)
}
extern "thiscall" fn current(_this: *mut u8) -> u64 {
    record(Call::Current);
    pair(STATE.with(|s| s.borrow().current) as u32, 5)
}
extern "thiscall" fn position(_this: *mut u8, x: *mut i32, z: *mut i32) -> u64 {
    record(Call::Position);
    let p = STATE.with(|s| s.borrow().position);
    unsafe {
        x.write(p[0]);
        z.write(p[1]);
    }
    pair(6, 7)
}
extern "thiscall" fn size(_this: *mut u8, x: *mut i32, z: *mut i32) -> u64 {
    record(Call::Size);
    let p = STATE.with(|s| s.borrow().size);
    unsafe {
        x.write(p[0]);
        z.write(p[1]);
    }
    pair(8, 9)
}
extern "thiscall" fn at(_this: *mut u8, x: i32, z: i32) -> u64 {
    record(Call::At(x, z));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let r = s.cells.iter_mut().find(|(c, _)| *c == [x, z]).map_or(0, |(_, v)| {
            if *v == usize::MAX {
                0
            } else {
                v as *mut usize as u32
            }
        });
        pair(r, 10)
    })
}
extern "thiscall" fn established(this: *mut City) -> u64 {
    record(Call::Established(unsafe { (*this).id }));
    pair(unsafe { (*this).established }, 11)
}
extern "thiscall" fn addref(this: *mut City) -> u64 {
    let id = unsafe { (*this).id };
    record(Call::AddRef(id));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.references.iter_mut().find(|(i, _)| *i == id).unwrap().1 += 1;
    });
    pair(12, 13)
}
extern "thiscall" fn release(this: *mut City) -> u64 {
    let id = unsafe { (*this).id };
    record(Call::Release(id));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.references.iter_mut().find(|(i, _)| *i == id).unwrap().1 -= 1;
    });
    pair(14, 15)
}
extern "C" fn framework() -> u64 {
    record(Call::Framework);
    pair(STATE.with(|s| s.borrow().framework) as u32, 16)
}
extern "thiscall" fn service(_this: *mut u8, service: u32, iid: u32, out: *mut usize) -> u64 {
    assert_eq!((service, iid), (0x3AE4_BEA3, 0x3AE4_BEAB));
    record(Call::Service);
    unsafe {
        out.write(STATE.with(|s| s.borrow().allocator));
    }
    pair(1, 17)
}
extern "thiscall" fn release_local(this: *mut usize) -> u64 {
    record(Call::ReleaseLocal);
    unsafe {
        this.write(0);
    }
    pair(18, 19)
}
extern "thiscall" fn allocate_node(_this: *mut u8, bytes: u32) -> u64 {
    record(Call::AllocateNode(bytes));
    pair(allocate() as u32, 20)
}
extern "thiscall" fn free_node(_this: *mut u8, _node: *mut u8) -> u64 {
    record(Call::FreeNode);
    pair(0xAABB_CCDD, 0x1122_3344)
}
extern "thiscall" fn insert(tree: *mut CitySet, out: *mut u32, city: *mut *mut u8) -> u64 {
    let city = unsafe { city.read() };
    record(Call::Insert(id(city)));
    unsafe {
        let h = (*tree).header;
        let mut parent = h;
        let mut current = pointer(h, 4);
        let mut left = false;
        while !current.is_null() {
            parent = current;
            let stored = pointer(current, 0x10);
            if stored == city {
                out.write(current as u32);
                out.add(1).cast::<u8>().write(0);
                return pair(out as u32, 21);
            }
            left = (city as usize) < stored as usize;
            current = pointer(current, if left { 8 } else { 0x0C });
        }
        let n = allocate();
        n.cast::<u32>().write(0);
        n.add(4).cast::<*mut u8>().write(parent);
        n.add(8).cast::<u32>().write(0);
        n.add(12).cast::<u32>().write(0);
        n.add(16).cast::<*mut u8>().write(city);
        if parent == h {
            h.add(4).cast::<*mut u8>().write(n);
            h.add(8).cast::<*mut u8>().write(n);
            h.add(12).cast::<*mut u8>().write(n);
        } else {
            parent.add(if left { 8 } else { 12 }).cast::<*mut u8>().write(n);
            if left && pointer(h, 8) == parent {
                h.add(8).cast::<*mut u8>().write(n);
            }
            if !left && pointer(h, 12) == parent {
                h.add(12).cast::<*mut u8>().write(n);
            }
        }
        (*tree).count += 1;
        out.write(n as u32);
        out.add(1).cast::<u8>().write(1);
    }
    pair(out as u32, 21)
}
extern "thiscall" fn erase_subtree(_tree: *mut CitySet, node: *mut u8) -> u64 {
    record(Call::Subtree(tree_id(node)));
    pair(22, 23)
}
extern "C" fn free_tree(node: *mut u8) -> u64 {
    let id = tree_id(node);
    record(Call::FreeTree(id));
    if id == u32::MAX {
        unsafe {
            assert_eq!(word(node, 0), 0xA5A5_A500);
            assert_eq!(pointer(node, 4), core::ptr::null_mut());
            assert_eq!(pointer(node, 8), node);
            assert_eq!(pointer(node, 12), node);
        }
    }
    pair(0x5566_7788, 0x99AA_BBCC)
}
extern "thiscall" fn all_connections(_this: *mut u8, list: *mut u8) -> u64 {
    record(Call::AllConnections);
    let objects = STATE.with(|s| s.borrow().connections.clone());
    for p in objects {
        unsafe {
            append(list, p);
        }
    }
    pair(24, 25)
}
extern "thiscall" fn connection_kind(this: *mut Connection) -> u64 {
    let k = unsafe { (*this).kind };
    record(Call::Kind(k));
    pair(k, 26)
}
extern "thiscall" fn connection_city(this: *mut Connection) -> u64 {
    record(Call::City(unsafe { (*this).kind }));
    pair(unsafe { (*this).city } as u32, 27)
}
extern "thiscall" fn city_node(_this: *mut u8, count: u32, hint: u32) -> u64 {
    record(Call::CityNode(count, hint));
    pair(allocate() as u32, 28)
}
extern "thiscall" fn clear_connections(list: *mut u8) -> u64 {
    record(Call::ClearConnections);
    unsafe {
        reset_list(list);
    }
    if STATE.with(|s| s.borrow().invalidate) {
        unsafe {
            connection_allocator().write(core::ptr::null_mut());
        }
    }
    pair(29, 30)
}

struct World {
    sim: [u32; 18],
    output: Box<*mut u8>,
    _header: Box<[u32; 3]>,
    cities: Box<[City; 5]>,
    _current: Box<usize>,
    connections: Vec<Connection>,
    _allocator: Box<usize>,
    _region: Box<usize>,
    _manager: Box<usize>,
    _framework: Box<usize>,
    _tables: [Box<[usize; 73]>; 5],
    _caches: [Box<*mut u8>; 3],
    _fakes: Vec<Fake>,
}
impl World {
    fn new(cached: bool) -> Self {
        let mut tables = core::array::from_fn::<_, 5, _>(|_| Box::new([0usize; 73]));
        tables[0][1] = addref as *const () as usize;
        tables[0][2] = release as *const () as usize;
        tables[0][0xAC / 4] = established as *const () as usize;
        tables[0][0x14 / 4] = position as *const () as usize;
        tables[0][0x1C / 4] = size as *const () as usize;
        tables[1][0x14 / 4] = connection_kind as *const () as usize;
        tables[1][0x24 / 4] = connection_city as *const () as usize;
        tables[2][3] = allocate_node as *const () as usize;
        tables[2][4] = free_node as *const () as usize;
        tables[3][0x2C / 4] = at as *const () as usize;
        tables[3][0x8C / 4] = region as *const () as usize;
        tables[3][0x90 / 4] = current as *const () as usize;
        tables[3][0x14 / 4] = service as *const () as usize;
        tables[4][0x40 / 4] = all_connections as *const () as usize;
        let mut sim = [0xA5A5_A5A5; 18];
        sim[0] = tables[4].as_ptr() as u32;
        let cities = Box::new(core::array::from_fn(|i| City {
            table: tables[0].as_ptr(),
            id: i as u32 + 1,
            established: if i == 3 { 0xABCD_EF00 } else { 0xABCD_EF80 },
        }));
        let mut current = Box::new(tables[0].as_ptr() as usize);
        let mut allocator = Box::new(tables[2].as_ptr() as usize);
        let ap = (&mut *allocator as *mut usize).cast::<u8>();
        let mut region = Box::new(tables[3].as_ptr() as usize);
        let mut manager = Box::new(tables[3].as_ptr() as usize);
        let mut framework_object = Box::new(tables[3].as_ptr() as usize);
        let mut caches = [
            Box::new(if cached { ap } else { core::ptr::null_mut() }),
            Box::new(if cached { ap } else { core::ptr::null_mut() }),
            Box::new((&mut *manager as *mut usize).cast()),
        ];
        let mut header = Box::new([0xDEAD_BEEFu32; 3]);
        let h = header.as_mut_ptr() as u32;
        header[0] = h;
        header[1] = h;
        let output = Box::new(header.as_mut_ptr().cast::<u8>());
        STATE.with(|s| {
            *s.borrow_mut() = State {
                position: [3, 3],
                size: [2, 2],
                current: &mut *current as *mut usize as usize,
                allocator: ap as usize,
                framework: &mut *framework_object as *mut usize as usize,
                region: &mut *region as *mut usize as usize,
                references: (1..=5).map(|i| (i, 0)).collect(),
                ..State::default()
            }
        });
        let mut fakes = vec![
            fake_data(0x00B2_07E0, (&mut *caches[0] as *mut *mut u8).cast()),
            fake_data(0x00B2_07DC, (&mut *caches[1] as *mut *mut u8).cast()),
            fake_data(0x00B4_3C94, (&mut *caches[2] as *mut *mut u8).cast()),
        ];
        for (address, function) in [
            (0x0041_1DF0, clear_output as *const () as usize),
            (0x0090_CF54, allocate_tree as *const () as usize),
            (0x0049_A050, insert as *const () as usize),
            (0x004A_4660, erase_subtree as *const () as usize),
            (0x0090_CF63, free_tree as *const () as usize),
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_local as *const () as usize),
            (0x0041_1D70, clear_connections as *const () as usize),
            (0x0041_1B60, city_node as *const () as usize),
        ] {
            fakes.push(fake(address, function));
        }
        Self {
            sim,
            output,
            _header: header,
            cities,
            _current: current,
            connections: vec![],
            _allocator: allocator,
            _region: region,
            _manager: manager,
            _framework: framework_object,
            _tables: tables,
            _caches: caches,
            _fakes: fakes,
        }
    }
    fn p(&mut self, i: usize) -> usize {
        &mut self.cities[i] as *mut _ as usize
    }
    fn cell(&mut self, at: [i32; 2], city: Option<usize>) {
        let p = city.map_or(0, |i| self.p(i));
        STATE.with(|s| s.borrow_mut().cells.push((at, p)));
    }
    fn connection(&mut self, kind: u32, city: Option<usize>) {
        let city = city.map_or(0, |i| self.p(i)) as *mut City;
        self.connections.push(Connection {
            table: self._tables[1].as_ptr(),
            kind,
            city,
        });
    }
    fn run(&mut self, entry: usize, include: u32) -> (u64, u32, Vec<u32>) {
        let objects = self.connections.iter_mut().map(|c| c as *mut _ as usize).collect();
        STATE.with(|s| s.borrow_mut().connections = objects);
        let output = (&mut *self.output as *mut *mut u8).cast::<u8>();
        let mut args = [output as u32, include];
        let before = self.sim;
        let result = unsafe {
            checked_call(
                entry,
                self.sim.as_mut_ptr().cast(),
                &mut args,
                0xDEAD_BEEF,
                0xCAFE_BABE,
                Conv::Thiscall,
            )
        };
        assert_eq!(self.sim, before);
        assert_eq!(args[0], output as u32);
        let mut n = unsafe { pointer(*self.output, 0) };
        let mut ids = Vec::new();
        while n != *self.output {
            ids.push(id(unsafe { pointer(n, 8) }));
            n = unsafe { pointer(n, 0) };
            assert!(ids.len() < 20);
        }
        let counts = STATE.with(|s| s.borrow().references.clone());
        for (i, count) in counts {
            assert_eq!(count, i32::from(ids.contains(&i)));
        }
        (result, args[1], ids)
    }
}

#[test]
fn adjacent_scans_four_edges_filters_missing_and_unestablished_and_sorts_unique_city_pointers() {
    for (include, ids) in [
        (0, vec![1, 2, 3]),
        (0xABCD_EF80, vec![1, 2, 3, 4]),
        (0xFFFF_FF00, vec![1, 2, 3]),
    ] {
        let mut w = World::new(true);
        for (cell, city) in [
            ([3, 2], Some(2)),
            ([3, 5], Some(0)),
            ([4, 2], Some(2)),
            ([4, 5], Some(3)),
            ([2, 3], None),
            ([5, 3], Some(1)),
        ] {
            w.cell(cell, city);
        }
        STATE.with(|s| s.borrow_mut().cells.push(([2, 4], usize::MAX)));
        let (result, arg, actual) = w.run(fc4p_006d01e0 as *const () as usize, include);
        assert_eq!(result, pair(0x5566_7788, 0x99AA_BBCC));
        assert_eq!(arg, include);
        assert_eq!(actual, ids);
        let calls = take();
        assert_eq!(
            &calls[..6],
            &[
                Call::ClearOutput,
                Call::AllocateTree(20),
                Call::Region,
                Call::Current,
                Call::Position,
                Call::Size
            ]
        );
        assert_eq!(
            calls
                .iter()
                .filter_map(|c| if let Call::At(x, z) = c { Some((*x, *z)) } else { None })
                .collect::<Vec<_>>(),
            [(3, 2), (3, 5), (4, 2), (4, 5), (2, 3), (5, 3), (2, 4), (5, 4)]
        );
        assert_eq!(
            calls
                .iter()
                .filter_map(|c| if let Call::Insert(i) = c { Some(*i) } else { None })
                .collect::<Vec<_>>(),
            if include as u8 != 0 {
                vec![3, 1, 3, 4, 2]
            } else {
                vec![3, 1, 3, 2]
            }
        );
        assert_eq!(
            calls
                .iter()
                .filter_map(|c| if let Call::Established(i) = c { Some(*i) } else { None })
                .collect::<Vec<_>>(),
            if include as u8 != 0 {
                vec![]
            } else {
                vec![3, 1, 3, 4, 2]
            }
        );
        let mut tail = Vec::new();
        for id in ids {
            tail.extend([
                Call::AddRef(id),
                Call::AllocateNode(12),
                Call::AddRef(id),
                Call::Release(id),
            ]);
        }
        assert!(calls[6..].windows(tail.len()).any(|s| s == tail));
        assert_eq!(calls.last(), Some(&Call::FreeTree(u32::MAX)));
        assert!(calls.contains(&Call::Subtree(if include as u8 != 0 { 4 } else { 0 })));
        assert!(calls.contains(&Call::FreeTree(3)));
        assert!(calls.contains(&Call::FreeTree(1)));
    }
}

#[test]
fn empty_and_negative_sizes_skip_loops_and_wrapping_positions_are_kept() {
    for size in [[0, 0], [-1, -2], [0, 1], [1, 0]] {
        let mut w = World::new(false);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.size = size;
            s.position = [i32::MAX, i32::MIN];
        });
        assert_eq!(
            w.run(fc4p_006d01e0 as *const () as usize, 0),
            (pair(0x5566_7788, 0x99AA_BBCC), 0, vec![])
        );
        let calls = take();
        assert!(!calls.contains(&Call::Framework));
        let points: Vec<_> = calls
            .iter()
            .filter_map(|c| if let Call::At(x, z) = c { Some((*x, *z)) } else { None })
            .collect();
        assert_eq!(
            points,
            if size == [0, 1] {
                vec![(i32::MAX - 1, i32::MIN), (i32::MAX, i32::MIN)]
            } else if size == [1, 0] {
                vec![(i32::MAX, i32::MAX), (i32::MAX, i32::MIN)]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn adjacent_first_service_lookup_reuses_the_original_include_word() {
    let mut w = World::new(false);
    w.cell([3, 2], Some(0));
    assert_eq!(
        w.run(fc4p_006d01e0 as *const () as usize, 0xFFFF_FF80),
        (pair(0x5566_7788, 0x99AA_BBCC), 0, vec![1])
    );
    let calls = take();
    let index = calls.iter().position(|c| *c == Call::Framework).unwrap();
    assert_eq!(
        &calls[index..index + 6],
        [
            Call::Framework,
            Call::Service,
            Call::ReleaseLocal,
            Call::AllocateNode(12),
            Call::AddRef(1),
            Call::Release(1)
        ]
    );
}

#[test]
fn connected_filters_types_before_cities_and_establishment_before_deduplication() {
    for (include, ids) in [(0, vec![3, 1]), (0x80, vec![3, 1, 4])] {
        let mut w = World::new(false);
        for (kind, city) in [
            (4, Some(4)),
            (5, Some(4)),
            (0, Some(4)),
            (13, Some(4)),
            (u32::MAX, Some(4)),
            (2, Some(2)),
            (1, Some(0)),
            (3, Some(2)),
            (6, Some(3)),
            (12, None),
        ] {
            w.connection(kind, city);
        }
        let result = w.run(fc4p_006d1e00 as *const () as usize, include);
        assert_eq!(result, (pair(0xAABB_CCDD, 0x1122_3344), include, ids.clone()));
        let mut expected = vec![
            Call::ClearOutput,
            Call::Framework,
            Call::Service,
            Call::ReleaseLocal,
            Call::AllocateNode(12),
            Call::AllConnections,
        ];
        for (kind, city) in [
            (4, None),
            (5, None),
            (0, None),
            (13, None),
            (u32::MAX, None),
            (2, Some(3)),
            (1, Some(1)),
            (3, Some(3)),
            (6, Some(4)),
            (12, Some(0)),
        ] {
            expected.push(Call::Kind(kind));
            if matches!(kind, 1..=3 | 6..=12) {
                expected.push(Call::City(kind));
            }
            if let Some(id) = city {
                if id == 0 {
                    continue;
                }
                if include == 0 {
                    expected.push(Call::Established(id));
                }
                if id == 4 && include == 0 {
                    continue;
                }
                expected.extend([Call::AddRef(id), Call::Release(id)]);
                if kind != 3 {
                    expected.extend([
                        Call::AddRef(id),
                        Call::CityNode(1, 0),
                        Call::AddRef(id),
                        Call::Release(id),
                    ]);
                }
            }
        }
        expected.extend([Call::ClearConnections, Call::FreeNode]);
        assert_eq!(take(), expected);
    }
}

#[test]
fn every_transport_type_is_accepted_and_cleanup_reacquires_an_invalidated_allocator() {
    for kind in [1, 2, 3, 6, 7, 8, 9, 10, 11, 12] {
        let mut w = World::new(true);
        w.connection(kind, Some(0));
        STATE.with(|s| s.borrow_mut().invalidate = true);
        assert_eq!(
            w.run(fc4p_006d1e00 as *const () as usize, 0xFF),
            (pair(0xAABB_CCDD, 0x1122_3344), 0, vec![1])
        );
        let calls = take();
        assert_eq!(
            &calls[calls.len() - 5..],
            [
                Call::ClearConnections,
                Call::Framework,
                Call::Service,
                Call::ReleaseLocal,
                Call::FreeNode
            ]
        );
    }
    let mut w = World::new(true);
    assert_eq!(
        w.run(fc4p_006d1e00 as *const () as usize, 0xFFFF_FF00),
        (pair(0xAABB_CCDD, 0x1122_3344), 0xFFFF_FF00, vec![])
    );
    assert_eq!(
        take(),
        [
            Call::ClearOutput,
            Call::AllocateNode(12),
            Call::AllConnections,
            Call::ClearConnections,
            Call::FreeNode
        ]
    );
}
