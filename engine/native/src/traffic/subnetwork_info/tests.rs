// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, float_environment, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Alloc(u32),
    Copy(u32),
    Free,
    Deadline(u32),
    Lot(i32, i32),
    Rect,
    Boundary(Option<(i32, i32)>),
    Holder,
    Set(Vec<u32>),
    Remove,
    Populations,
    Population(u32, u32),
    Capacity(u32, u32),
    Framework,
    QueryAllocator,
    Release,
    ListNew,
    Neighbors,
    Position(i32, i32),
    NeighborHolder(i32, i32),
    ListClear,
    ListFree,
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    expire_at: u32,
    checks: u32,
    lots: Vec<(i32, i32, u32)>,
    boundary: Vec<(i32, i32)>,
    boundary_index: usize,
    changing_info: u32,
    expected_boundary: [i32; 4],
    working: u32,
    pops: Vec<(u32, u32, Box<u32>)>,
    neighbors: Vec<u32>,
    allocator: u32,
    neighbor_holder: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

unsafe extern "C" fn allocate_bytes(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Alloc(size));
        let mut b = vec![0xA5A5_A5A5; size as usize / 4].into_boxed_slice();
        let p = b.as_mut_ptr().cast();
        s.allocations.push(b);
        p
    })
}
extern "C" fn free_body(_p: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free));
}
unsafe extern "C" fn copy_bytes(dest: *mut u8, source: *const u8, size: u32) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Copy(size)));
    unsafe {
        core::ptr::copy_nonoverlapping(source, dest, size as usize);
    }
    dest
}
#[unsafe(naked)]
unsafe extern "C" fn free_bytes(_p: *mut u8) {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {}", "add esp, 4",
        "mov eax, 0x11223344", "mov ecx, 0x55667788", "mov edx, 0x99aabbcc", "ret", sym free_body);
}
unsafe extern "thiscall" fn deadline(this: *mut u8) -> u32 {
    unsafe {
        assert_eq!(word(this, 0), 0x1234);
        assert_eq!(word(this, 4), 0x5678);
        assert_eq!(word(this, 16), 0);
        assert_eq!(word(this, 20), stride() as u32);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Deadline(word(this, 8)));
            s.checks += 1;
            u32::from(s.expire_at != 0 && s.checks >= s.expire_at)
        })
    }
}
unsafe extern "thiscall" fn get_lot(_this: *mut u8, x: i32, z: i32, mode: u32) -> *mut u8 {
    assert_eq!(mode, 1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Lot(x, z));
        s.lots
            .iter()
            .find(|&&(a, b, _)| a == x && b == z)
            .map_or(0, |&(_, _, p)| p) as *mut u8
    })
}
unsafe extern "thiscall" fn get_rect(this: *mut u8, out: *mut i32) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Rect));
    unsafe {
        for i in 0..4 {
            out.add(i).write(word(this, 4 + i * 4) as i32);
        }
    }
}
unsafe extern "thiscall" fn get_holder(this: *mut u8) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Holder));
    unsafe { word(this, 28) as *mut u8 }
}
unsafe extern "thiscall" fn get_populations(this: *mut u8) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Populations));
    unsafe { word(this, 24) as *mut u8 }
}
unsafe extern "thiscall" fn boundary(this: *mut u8, x: *mut i32, z: *mut i32) -> u8 {
    unsafe {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if this.read() == 0 {
                assert_eq!(word(this, 12), 1);
                for i in 0..4 {
                    assert_eq!(word(this, 16 + i * 4) as i32, s.expected_boundary[i]);
                }
                this.write(1);
                s.boundary_index = 0;
            }
            let point = s.boundary.get(s.boundary_index).copied();
            s.calls.push(Call::Boundary(point));
            if let Some((a, b)) = point {
                if s.changing_info != 0 {
                    let first = s.boundary_index as u16 * 4 + 1;
                    (s.changing_info as *mut u8)
                        .add(12)
                        .cast::<[u16; 4]>()
                        .write_unaligned([first, first + 1, first + 2, first + 3]);
                }
                x.write(a);
                z.write(b);
                s.boundary_index += 1;
                1
            } else {
                0
            }
        })
    }
}
unsafe extern "C" fn set(_holder: *mut u8, id: u32, vector: *const u32) {
    assert_eq!(id, 0x2A4F_9C74);
    unsafe {
        let begin = vector.read();
        let end = vector.add(1).read();
        let values = std::slice::from_raw_parts(begin as *const u32, end.wrapping_sub(begin) as usize / 4).to_vec();
        STATE.with(|s| s.borrow_mut().calls.push(Call::Set(values)));
    }
}
unsafe extern "thiscall" fn remove(_this: *mut u8, id: u32, flags: u32) {
    assert_eq!(id, 0x2A4F_9C74);
    assert_eq!(flags, 0);
    STATE.with(|s| s.borrow_mut().calls.push(Call::Remove));
}
unsafe extern "thiscall" fn pop(this: *mut u8, key: *const u32) -> *mut u32 {
    unsafe {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let id = (this as u32).wrapping_sub(s.working) / 28;
            let key = key.read();
            s.calls.push(Call::Population(id, key));
            if let Some(i) = s.pops.iter().position(|&(a, b, _)| a == id && b == key) {
                return &mut *s.pops[i].2 as *mut u32;
            }
            s.pops.push((id, key, Box::new(0)));
            &mut *s.pops.last_mut().unwrap().2 as *mut u32
        })
    }
}
unsafe extern "thiscall" fn capacity(_this: *mut u8, holder: *mut u8, category: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Capacity(holder as u32, category)));
    category + 10
}
unsafe extern "C" fn framework_fake() -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Framework);
        s.neighbor_holder as *mut u8
    })
}
unsafe extern "thiscall" fn query_allocator(_this: *mut u8, a: u32, b: u32, result: *mut u32) {
    assert_eq!((a, b), (0x3AE4_BEA3, 0x3AE4_BEAB));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::QueryAllocator);
        unsafe {
            result.write(s.allocator);
        }
    });
}
unsafe extern "thiscall" fn release(_this: *mut u32) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release));
}
unsafe extern "thiscall" fn list_new(this: *mut u8, size: u32) -> *mut u8 {
    assert_eq!(size, 12);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as u32, s.allocator);
        s.calls.push(Call::ListNew);
        let mut b = vec![0xAAAA_AAAA; 3].into_boxed_slice();
        let p = b.as_mut_ptr().cast();
        s.allocations.push(b);
        p
    })
}
unsafe extern "thiscall" fn neighbors(_this: *mut u8, header: *mut u32) {
    unsafe {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Neighbors);
            let head = header.read() as *mut u32;
            assert_eq!(head.read(), head as u32);
            assert_eq!(head.add(1).read(), head as u32);
            let mut previous = head;
            for neighbor in s.neighbors.clone() {
                let mut b = vec![head as u32, previous as u32, neighbor].into_boxed_slice();
                let p = b.as_mut_ptr();
                previous.write(p as u32);
                previous = p;
                s.allocations.push(b);
            }
            head.add(1).write(previous as u32);
        });
    }
}
unsafe extern "thiscall" fn position(this: *mut u8, x: *mut i32, z: *mut i32) {
    unsafe {
        let a = word(this, 4) as i32;
        let b = word(this, 8) as i32;
        STATE.with(|s| s.borrow_mut().calls.push(Call::Position(a, b)));
        x.write(a);
        z.write(b);
    }
}
unsafe extern "thiscall" fn neighbor_property(_this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
    assert_eq!(kind, 1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::NeighborHolder(x, z));
        s.neighbor_holder as *mut u8
    })
}
unsafe extern "thiscall" fn list_clear(header: *mut u32) {
    unsafe {
        let p = header.read() as *mut u32;
        p.write(p as u32);
        p.add(1).write(p as u32);
    }
    STATE.with(|s| s.borrow_mut().calls.push(Call::ListClear));
}
unsafe extern "thiscall" fn list_free(_this: *mut u8, p: *mut u32) {
    unsafe {
        assert_eq!(p.read(), p as u32);
    }
    STATE.with(|s| s.borrow_mut().calls.push(Call::ListFree));
}

struct Fixture {
    sim_object: Object,
    sim: InfoSim,
    scan: Object,
    working: Object,
    cells: Vec<Vec<u32>>,
    _rows: Vec<[u32; 3]>,
    owned: Vec<Object>,
    tables: Vec<Vec<usize>>,
    arrays: Vec<Box<[u32]>>,
    init: Box<u32>,
    step: Box<u32>,
    bal: Box<i32>,
    alloc_global: Box<*mut u8>,
    _lot_global: Box<*mut u8>,
    _network_global: Box<*mut u8>,
    _guards: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let sim_table = table(&[(0x50, capacity as *const () as usize)]);
        let mut sim_object = object(0xD00, &sim_table);
        let sim = unsafe { InfoSim::from_ptr(sim_object.ptr()) };
        sim.set_edge_x(4);
        sim.set_edge_z(4);
        sim.set_mapped_x(3);
        sim.set_mapped_z(3);
        sim.set_max_x(3);
        sim.set_max_z(3);
        let mut working = Object::new(100 * 28);
        sim.set_working(working.ptr());
        let mut scan = Object::new(32);
        unsafe {
            for (i, value) in [0, 0, 0, 1, 0, 0, 0, 0].into_iter().enumerate() {
                store(scan.ptr(), i * 4, value);
            }
        }
        let mut cells: Vec<Vec<u32>> = (0..4).map(|_| vec![0; 4]).collect();
        let mut rows: Vec<_> = cells.iter_mut().map(|r| [r.as_mut_ptr() as u32, 0, 0]).collect();
        sim.set_rows(rows.as_mut_ptr().cast());
        let lot_table = table(&[(0x18, get_lot as *const () as usize)]);
        let mut lot_object = object(4, &lot_table);
        let network_table = table(&[(0x40, neighbors as *const () as usize)]);
        let mut network_object = object(4, &network_table);
        let alloc_table = table(&[
            (0x0C, list_new as *const () as usize),
            (0x10, list_free as *const () as usize),
        ]);
        let mut alloc_object = object(4, &alloc_table);
        let holder_table = table(&[
            (0x14, query_allocator as *const () as usize),
            (0x44, remove as *const () as usize),
        ]);
        let mut holder_object = object(4, &holder_table);
        let mut init = Box::new(1u32);
        let mut step = Box::new(1024u32);
        let mut bal = Box::new(0i32);
        let mut lot_global = Box::new(lot_object.ptr());
        let mut network_global = Box::new(network_object.ptr());
        let mut alloc_global = Box::new(alloc_object.ptr());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.working = working.ptr() as u32;
            s.allocator = alloc_object.ptr() as u32;
            s.neighbor_holder = holder_object.ptr() as u32;
            s.expected_boundary = [0, 0, 2, 2];
        });
        let guards = vec![
            fake(0x0090_CF54, allocate_bytes as *const () as usize),
            fake(0x0090_CF63, free_bytes as *const () as usize),
            fake(0x009E_EB70, copy_bytes as *const () as usize),
            fake(0x0068_BF80, deadline as *const () as usize),
            fake(0x0060_CEE0, boundary as *const () as usize),
            fake(0x005F_E6B0, set as *const () as usize),
            fake(0x0049_7A00, pop as *const () as usize),
            fake(0x0071_14E0, neighbor_property as *const () as usize),
            fake(0x0087_93EC, framework_fake as *const () as usize),
            fake(0x0041_1870, release as *const () as usize),
            fake(0x0041_1D70, list_clear as *const () as usize),
            fake_data(0x00B4_B344, (&mut *init as *mut u32).cast()),
            fake_data(0x00B4_B33C, (&mut *step as *mut u32).cast()),
            fake_data(0x00B4_B340, (&mut *bal as *mut i32).cast()),
            fake_data(0x00B4_3D08, (&mut *lot_global as *mut *mut u8).cast()),
            fake_data(0x00B4_3D8C, (&mut *network_global as *mut *mut u8).cast()),
            fake_data(0x00B2_07DC, (&mut *alloc_global as *mut *mut u8).cast()),
        ];
        Self {
            sim_object,
            sim,
            scan,
            working,
            cells,
            _rows: rows,
            owned: vec![lot_object, network_object, alloc_object, holder_object],
            tables: vec![sim_table, lot_table, network_table, alloc_table, holder_table],
            arrays: vec![],
            init,
            step,
            bal,
            alloc_global,
            _lot_global: lot_global,
            _network_global: network_global,
            _guards: guards,
        }
    }
    fn car(&mut self, x: usize, z: usize, labels: [u16; 4]) {
        let mut o = Object::new(24);
        unsafe {
            o.ptr().add(6).write(1);
            o.ptr().add(12).cast::<[u16; 4]>().write_unaligned(labels);
        }
        self.cells[x][z] = o.ptr() as u32;
        self.owned.push(o);
    }
    fn lot(&mut self, key: Option<(u32, u16)>) -> u32 {
        let vtable = table(&[
            (0x0C, get_holder as *const () as usize),
            (0x38, get_rect as *const () as usize),
            (0xD8, get_populations as *const () as usize),
        ]);
        let mut lot = object(32, &vtable);
        let mut header = Object::new(24);
        let h = header.ptr();
        unsafe {
            store(h, 4, h as u32);
            store(h, 8, h as u32);
            store(h, 12, h as u32);
        }
        if let Some((key, amount)) = key {
            let mut node = Object::new(24);
            let n = node.ptr();
            unsafe {
                store(n, 4, h as u32);
                store(n, 16, key);
                n.add(20).cast::<u16>().write_unaligned(amount);
                store(h, 4, n as u32);
                store(h, 8, n as u32);
                store(h, 12, n as u32);
            }
            self.owned.push(node);
        }
        let mut row = vec![h as u32, 0, 0].into_boxed_slice();
        let mut populations =
            vec![row.as_mut_ptr() as u32, row.as_mut_ptr().wrapping_add(3) as u32, 0].into_boxed_slice();
        let holder = STATE.with(|s| s.borrow().neighbor_holder);
        unsafe {
            for i in 0..4 {
                store(lot.ptr(), 4 + i * 4, 1);
            }
            store(lot.ptr(), 24, populations.as_mut_ptr() as u32);
            store(lot.ptr(), 28, holder);
        }
        let p = lot.ptr() as u32;
        self.arrays.push(row);
        self.arrays.push(populations);
        self.tables.push(vtable);
        self.owned.push(header);
        self.owned.push(lot);
        STATE.with(|s| s.borrow_mut().lots.push((0, 0, p)));
        p
    }
    fn neighbor(&mut self, x: i32, z: i32) {
        let vtable = table(&[(0x18, position as *const () as usize)]);
        let mut n = object(12, &vtable);
        unsafe {
            store(n.ptr(), 4, x as u32);
            store(n.ptr(), 8, z as u32);
        }
        STATE.with(|s| s.borrow_mut().neighbors.push(n.ptr() as u32));
        self.tables.push(vtable);
        self.owned.push(n);
    }
    fn run(&mut self) -> Registers {
        let before = float_environment();
        let time = [0x1234, 0x5678];
        let args = [time.as_ptr() as u32, self.scan.ptr() as u32];
        let r = unsafe {
            invoke(
                fc4p_00716180 as *const () as usize,
                self.sim.as_ptr(),
                &args,
                0x1122_3344,
                0x5566_7788,
                false,
            )
        };
        assert_eq!(before, float_environment());
        r
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
    fn capacities(&mut self, id: usize) -> [u32; 4] {
        unsafe { self.working.ptr().add(id * 28 + 12).cast::<[u32; 4]>().read_unaligned() }
    }
}

#[test]
fn finished_scan_still_checks_budget_and_collects_neighbors() {
    let mut f = Fixture::new();
    unsafe {
        f.scan.ptr().write(2);
    }
    let before = unsafe { bytes(f.sim_object.ptr(), 0xD00) };
    let r = f.run();
    assert_eq!(r.eax, 0xFFFF_FF01);
    assert_eq!(r.ecx, 0x5566_7788);
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::ListNew,
            Call::Neighbors,
            Call::ListClear,
            Call::ListFree,
            Call::Free
        ]
    );
    assert_eq!(unsafe { bytes(f.sim_object.ptr(), 0xD00) }, before);
}

#[test]
fn deadline_stop_retains_the_last_visited_cell_and_resumes_to_done() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().expire_at = 2);
    assert_eq!(f.run().eax as u8, 0);
    unsafe {
        assert_eq!(f.scan.ptr().read(), 1);
        assert_eq!(word(f.scan.ptr(), 4), 0);
        assert_eq!(word(f.scan.ptr(), 8), 0);
    }
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::Lot(0, 0),
            Call::Deadline(2048),
            Call::Free
        ]
    );
    STATE.with(|s| s.borrow_mut().expire_at = 0);
    assert_eq!(f.run().eax as u8, 1);
    unsafe {
        assert_eq!(f.scan.ptr().read(), 2);
        assert_eq!(word(f.scan.ptr(), 4), 0);
        assert_eq!(word(f.scan.ptr(), 8), 1);
    }
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::ListNew,
            Call::Neighbors,
            Call::ListClear,
            Call::ListFree,
            Call::Free
        ]
    );
}

#[test]
fn empty_ring_removes_the_old_property_without_reading_population() {
    let mut f = Fixture::new();
    f.lot(None);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.boundary = vec![(0, 0), (1, 0), (2, 2)];
    });
    f.run();
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::Lot(0, 0),
            Call::Rect,
            Call::Boundary(Some((0, 0))),
            Call::Boundary(Some((1, 0))),
            Call::Boundary(Some((2, 2))),
            Call::Boundary(None),
            Call::Holder,
            Call::Remove,
            Call::Deadline(2048),
            Call::ListNew,
            Call::Neighbors,
            Call::ListClear,
            Call::ListFree,
            Call::Free
        ]
    );
}

#[test]
fn ring_discovery_deduplicates_labels_excludes_corners_and_adds_population_and_capacity() {
    let mut f = Fixture::new();
    f.lot(Some((0x1010, 3)));
    f.car(1, 0, [2, 1, 2, 0]);
    f.car(0, 1, [1, 0, 0, 0]);
    f.car(0, 0, [3; 4]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.boundary = vec![(0, 0), (1, 0), (0, 1)];
        s.pops.push((2, 0x1010, Box::new(u32::MAX)));
    });
    f.run();
    assert_eq!(f.capacities(1), [10, 11, 12, 13]);
    assert_eq!(f.capacities(2), [10, 11, 12, 13]);
    assert_eq!(f.capacities(3), [0; 4]);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(*s.pops.iter().find(|&&(a, b, _)| a == 1 && b == 0x1010).unwrap().2, 3);
        assert_eq!(*s.pops.iter().find(|&&(a, b, _)| a == 2 && b == 0x1010).unwrap().2, 2);
    });
    let holder = STATE.with(|s| s.borrow().neighbor_holder);
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::Lot(0, 0),
            Call::Rect,
            Call::Boundary(Some((0, 0))),
            Call::Boundary(Some((1, 0))),
            Call::Boundary(Some((0, 1))),
            Call::Boundary(None),
            Call::Holder,
            Call::Set(vec![2, 1]),
            Call::Populations,
            Call::Population(2, 0x1010),
            Call::Population(1, 0x1010),
            Call::Holder,
            Call::Capacity(holder, 0),
            Call::Capacity(holder, 1),
            Call::Capacity(holder, 2),
            Call::Capacity(holder, 3),
            Call::Capacity(holder, 0),
            Call::Capacity(holder, 1),
            Call::Capacity(holder, 2),
            Call::Capacity(holder, 3),
            Call::Deadline(2048),
            Call::ListNew,
            Call::Neighbors,
            Call::ListClear,
            Call::ListFree,
            Call::Free
        ]
    );
}

#[test]
fn neighbor_labels_are_accumulated_with_duplicates_and_raw_edge_coordinates() {
    let mut f = Fixture::new();
    unsafe {
        f.scan.ptr().write(2);
    }
    f.neighbor(-1, 1);
    f.car(0, 1, [1, 1, 2, 0]);
    unsafe {
        store(f.working.ptr(), 28 + 12, u32::MAX);
    }
    f.run();
    assert_eq!(f.capacities(1), [19, 22, 24, 26]);
    assert_eq!(f.capacities(2), [10, 11, 12, 13]);
    let holder = STATE.with(|s| s.borrow().neighbor_holder);
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::ListNew,
            Call::Neighbors,
            Call::Position(-1, 1),
            Call::NeighborHolder(-1, 1),
            Call::Capacity(holder, 0),
            Call::Capacity(holder, 1),
            Call::Capacity(holder, 2),
            Call::Capacity(holder, 3),
            Call::ListClear,
            Call::ListFree,
            Call::Free
        ]
    );
}

#[test]
fn lazy_allocator_lookup_retains_and_releases_the_holder_before_list_creation() {
    let mut f = Fixture::new();
    *f.alloc_global = core::ptr::null_mut();
    unsafe {
        f.scan.ptr().write(2);
    }
    f.run();
    assert_eq!(
        f.calls(),
        [
            Call::Alloc(256),
            Call::Deadline(1024),
            Call::Framework,
            Call::QueryAllocator,
            Call::Release,
            Call::ListNew,
            Call::Neighbors,
            Call::ListClear,
            Call::ListFree,
            Call::Free
        ]
    );
}

#[test]
fn adaptive_stride_preserves_full_eax_and_the_completion_ecx_difference() {
    for complete in [false, true] {
        let mut f = Fixture::new();
        *f.step = 256;
        *f.bal = -3;
        unsafe {
            store(f.scan.ptr(), 24, 2);
        }
        if !complete {
            STATE.with(|s| s.borrow_mut().expire_at = 1);
            unsafe {
                f.scan.ptr().write(0);
                store(f.scan.ptr(), 24, 3);
            }
        }
        let r = f.run();
        assert_eq!(*f.step, 512);
        assert_eq!(*f.bal, 0);
        assert_eq!(r.eax, 0x100 | u32::from(complete));
        assert_eq!(r.ecx, if complete { 512 } else { 0x5566_7788 });
    }
}

#[test]
fn initialization_uses_the_low_flag_byte_and_keeps_other_flag_bits() {
    let mut f = Fixture::new();
    *f.init = 0xABCD_0200;
    *f.step = 4;
    *f.bal = 99;
    STATE.with(|s| s.borrow_mut().expire_at = 1);
    assert_eq!(f.run().eax, 0xFFFF_FF00);
    assert_eq!(*f.init, 0xABCD_0201);
    assert_eq!(*f.step, 1024);
    assert_eq!(*f.bal, -1);
    assert_eq!(f.calls(), [Call::Alloc(256), Call::Deadline(1024), Call::Free]);
}

#[test]
fn growing_the_label_vector_calls_the_game_allocator_copy_and_free_in_order() {
    let mut f = Fixture::new();
    f.lot(None);
    f.car(1, 0, [0; 4]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.boundary = vec![(1, 0); 17];
        s.changing_info = f.cells[1][0];
    });
    assert_eq!(f.run().eax as u8, 1);
    let calls = f.calls();
    let memory: Vec<_> = calls
        .iter()
        .filter(|c| matches!(c, Call::Alloc(_) | Call::Copy(_) | Call::Free))
        .collect();
    assert_eq!(
        memory,
        [
            &Call::Alloc(256),
            &Call::Alloc(512),
            &Call::Copy(256),
            &Call::Free,
            &Call::Free
        ]
    );
    assert!(calls.contains(&Call::Set((1..=68).collect())));
    for id in 1..=68 {
        assert_eq!(f.capacities(id), [10, 11, 12, 13]);
    }
}

#[test]
fn long_slices_halve_the_stride_only_after_sixteen_slow_results() {
    for complete in [false, true] {
        let mut f = Fixture::new();
        *f.bal = 15;
        unsafe {
            store(f.scan.ptr(), 24, 19);
        }
        if !complete {
            STATE.with(|s| s.borrow_mut().expire_at = 21);
        }
        let result = f.run();
        assert_eq!(result.eax, 0x200 | u32::from(complete));
        assert_eq!(result.ecx, 0x5566_7788);
        assert_eq!(*f.step, 512);
        assert_eq!(*f.bal, 0);
    }
}

#[test]
fn all_four_city_edges_map_to_their_inside_network_cells() {
    let mut f = Fixture::new();
    unsafe {
        f.scan.ptr().write(2);
    }
    for (x, z, cx, cz, id) in [(-1, 1, 0, 1, 1), (4, 1, 3, 1, 2), (1, -1, 1, 0, 3), (1, 4, 1, 3, 4)] {
        f.neighbor(x, z);
        f.car(cx, cz, [id, 0, 0, 0]);
    }
    assert_eq!(f.run().eax as u8, 1);
    for id in 1..=4 {
        assert_eq!(f.capacities(id), [10, 11, 12, 13]);
    }
    let calls = f.calls();
    let holders: Vec<_> = calls
        .iter()
        .filter(|c| matches!(c, Call::NeighborHolder(_, _)))
        .collect();
    assert_eq!(
        holders,
        [
            &Call::NeighborHolder(-1, 1),
            &Call::NeighborHolder(4, 1),
            &Call::NeighborHolder(1, -1),
            &Call::NeighborHolder(1, 4)
        ]
    );
}

#[test]
fn population_tree_successors_descend_left_and_climb_to_the_header() {
    let mut h = Object::new(24);
    let mut a = Object::new(24);
    let mut b = Object::new(24);
    let mut c = Object::new(24);
    let mut d = Object::new(24);
    let (h, a, b, c, d) = (h.ptr(), a.ptr(), b.ptr(), c.ptr(), d.ptr());
    unsafe {
        store(h, 4, b as u32);
        store(h, 8, a as u32);
        store(h, 12, d as u32);
        store(a, 4, b as u32);
        store(b, 4, h as u32);
        store(b, 8, a as u32);
        store(b, 12, d as u32);
        store(c, 4, d as u32);
        store(d, 4, b as u32);
        store(d, 8, c as u32);
        assert_eq!(next_tree(a), b);
        assert_eq!(next_tree(b), c);
        assert_eq!(next_tree(c), d);
        assert_eq!(next_tree(d), h);
    }
}
