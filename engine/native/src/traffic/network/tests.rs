// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Network and pool cases from the core traffic-network tests, using the game's object layout.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Inner(usize, i32, i32), Allocate(usize), Delete(usize, u32, u32),
    Edge(usize, i32, i32), Occupant(usize, u32, i32, i32),
    Pool(usize, u16), Holder(usize), Vector(usize, u32), Free(usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>, direction: i32, inner: (i32, i32), allocation: usize,
    edge: usize, occupant: usize, pool_node: usize, holder: usize,
    vector: Vec<u32>, vector_ok: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn reset() { STATE.with(|s| *s.borrow_mut() = State::default()); }
fn calls() -> Vec<Call> { STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)) }

unsafe extern "thiscall" fn inner(this: *mut u8, x: i32, z: i32, ix: *mut i32, iz: *mut i32) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Inner(this as usize, x, z));
        unsafe { ix.write(s.inner.0); iz.write(s.inner.1); }
        s.direction
    })
}
unsafe extern "thiscall" fn allocate(this: *mut u8) -> *mut u8 {
    STATE.with(|s| { let mut s = s.borrow_mut(); s.calls.push(Call::Allocate(this as usize)); s.allocation as *mut u8 })
}
unsafe extern "thiscall" fn delete(this: *mut u8, flags: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Delete(this as usize, flags, unsafe { word(this, 0x14) })));
    0xCAFE_BABE
}
unsafe extern "thiscall" fn edge(this: *mut u8, x: i32, z: i32) -> *mut u8 {
    STATE.with(|s| { let mut s = s.borrow_mut(); s.calls.push(Call::Edge(this as usize, x, z)); s.edge as *mut u8 })
}
unsafe extern "thiscall" fn occupant(this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Occupant(this as usize, kind, x, z));
        if kind == 4 { s.occupant as *mut u8 } else { core::ptr::null_mut() }
    })
}
unsafe extern "thiscall" fn pool(this: *mut u8, id: *mut u16) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Pool(this as usize, unsafe { id.read_unaligned() }));
        (0xAABB_CCDDu64 << 32) | s.pool_node as u64
    })
}
unsafe extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    STATE.with(|s| { let mut s = s.borrow_mut(); s.calls.push(Call::Holder(this as usize)); s.holder as *mut u8 })
}
unsafe extern "C" fn vector(holder: *mut u8, id: u32, out: *mut u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Vector(holder as usize, id));
        unsafe {
            assert_eq!(std::slice::from_raw_parts(out, 3), &[0, 0, 0]);
            if !s.vector.is_empty() {
                out.write(s.vector.as_ptr() as u32);
                out.add(1).write(s.vector.as_ptr().wrapping_add(s.vector.len()) as u32);
                out.add(2).write(s.vector.as_ptr().wrapping_add(s.vector.capacity()) as u32);
            }
        }
        (0xDEAD_4321u64 << 32) | u64::from(s.vector_ok)
    })
}
unsafe extern "C" fn free(memory: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(memory as usize)));
    0x1234_5678_8765_4321
}

struct World {
    sim: Object, rows: Vec<u32>, cells: Vec<Vec<u32>>, records: Vec<Object>, edges: [Vec<u32>; 4],
}
impl World {
    fn new() -> Self {
        let mut world = Self {
            sim: Object::new(0xD00), rows: vec![0; 4 * 3], cells: vec![vec![0; 4]; 4],
            records: (0..5).map(|_| Object::new(24)).collect(), edges: core::array::from_fn(|_| vec![0; 8]),
        };
        for (x, cells) in world.cells.iter_mut().enumerate() { world.rows[x * 3] = cells.as_mut_ptr() as u32; }
        unsafe {
            NetworkSim::from_ptr(world.sim.ptr()).set_rows(world.rows.as_mut_ptr().cast());
            let sim = TrafficSim::from_ptr(world.sim.ptr());
            sim.set_max_cell_x(3); sim.set_max_cell_z(3);
            sim.set_west_connections(world.edges[0].as_mut_ptr().cast());
            sim.set_north_connections(world.edges[1].as_mut_ptr().cast());
            sim.set_east_connections(world.edges[2].as_mut_ptr().cast());
            sim.set_south_connections(world.edges[3].as_mut_ptr().cast());
        }
        world
    }
    fn record(&mut self, i: usize, kind: u8, next: Option<usize>) -> *mut u8 {
        let next = next.map(|j| self.records[j].ptr()).unwrap_or(core::ptr::null_mut());
        let memory = self.records[i].ptr();
        unsafe { let info = NetworkInfo::from_ptr(memory); info.set_kind(kind); info.set_next(next); }
        memory
    }
    unsafe fn query(&mut self, kind: u32, x: i32, z: i32) -> Registers {
        unsafe { invoke(fc4p_0070fb30 as *const () as usize, self.sim.ptr(), &[kind, x as u32, z as u32], 0xFEDC_BA98, 0x1234, false) }
    }
}

#[test]
fn network_lookup_empty_match_chain_and_full_type_word() {
    let mut w = World::new();
    unsafe {
        let empty = w.query(2, 2, 1);
        assert_eq!(empty, Registers { eax: 0, ecx: w.rows.as_ptr() as u32, edx: w.cells[2].as_ptr() as u32 });
        let tail = w.record(1, 2, None);
        let head = w.record(0, 1, Some(1));
        w.cells[2][1] = head as u32;
        let before = bytes(w.sim.ptr(), 0xD00);
        assert_eq!(w.query(1, 2, 1), Registers { eax: head as u32, ecx: 1, edx: 1 });
        assert_eq!(w.query(2, 2, 1), Registers { eax: tail as u32, ecx: 2, edx: 2 });
        assert_eq!(w.query(0x102, 2, 1), Registers { eax: 0, ecx: 0x102, edx: 2 });
        assert_eq!(bytes(w.sim.ptr(), 0xD00), before);
    }
}

#[test]
fn ensure_info_preserves_padding_and_clears_the_previous_tail_count() {
    reset();
    let mut w = World::new();
    let mut block = [0u32, 0, 0, 17];
    let mut block_ptr = block.as_mut_ptr().cast::<u8>();
    let _pool = fake_data(0x00B0_D484, block.as_mut_ptr().cast());
    let _block = fake_data(0x00B0_D488, (&mut block_ptr as *mut *mut u8).cast());
    let _alloc = fake(0x0090_CF9A, allocate as *const () as usize);
    unsafe {
        let memory = w.records[0].ptr();
        core::ptr::write_bytes(memory, 0xA5, 24);
        store(memory, 0, 0x1111_2222);
        block[2] = memory as u32;
        let result = fc4p_0070fb70(w.sim.ptr(), 0x102, 1, 2);
        assert_eq!(result, memory);
        assert_eq!((block[2], block[3]), (0x1111_2222, 18));
        assert_eq!(bytes(memory, 24), [0x22, 0x22, 0x11, 0x11, 0, 0xA5, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(calls().is_empty());
        assert_eq!(fc4p_0070fb70(w.sim.ptr(), 2, 1, 2), memory);
        assert_eq!(block[3], 18);
        NetworkInfo::from_ptr(memory).set_connection_count(9);
        let second = w.records[1].ptr();
        core::ptr::write_bytes(second, 0xA5, 24);
        store(second, 0, 0);
        block[2] = 0;
        STATE.with(|s| s.borrow_mut().allocation = second as usize);
        assert_eq!(fc4p_0070fb70(w.sim.ptr(), 3, 1, 2), second);
        assert_eq!(calls(), [Call::Allocate(block.as_ptr() as usize)]);
        assert_eq!(NetworkInfo::from_ptr(memory).next(), second);
        assert_eq!(NetworkInfo::from_ptr(memory).connection_count(), 0);
        assert_eq!(NetworkInfo::from_ptr(second).kind(), 3);
        assert_eq!((block[2], block[3]), (0, 19));
    }
}

#[test]
fn remove_info_unlinks_head_middle_tail_and_ignores_missing_type() {
    let _delete = fake(0x0070_F040, delete as *const () as usize);
    for kind in [1u32, 2, 3, 259, 99] {
        reset();
        let mut w = World::new();
        let tail = w.record(2, 3, None);
        let middle = w.record(1, 2, Some(2));
        let head = w.record(0, 1, Some(1));
        w.cells[1][2] = head as u32;
        unsafe {
            fc4p_0070fc70(w.sim.ptr(), kind, 1, 2);
            let deleted = match kind { 1 => Some((head, middle)), 2 => Some((middle, tail)), 3 => Some((tail, core::ptr::null_mut())), _ => None };
            if let Some((deleted, next)) = deleted {
                assert_eq!(calls(), [Call::Delete(deleted as usize, 1, next as u32)]);
                assert_eq!(w.cells[1][2], if kind == 1 { middle as u32 } else { head as u32 });
                if kind == 2 { assert_eq!(NetworkInfo::from_ptr(head).next(), tail); }
                if kind == 3 { assert!(NetworkInfo::from_ptr(middle).next().is_null()); }
            } else { assert!(calls().is_empty()); }
        }
    }
}

#[test]
fn clear_subnetwork_ids_visits_inclusive_bounds_and_keeps_ecx() {
    let mut w = World::new();
    unsafe {
        let head = w.record(0, 1, Some(1));
        let tail = w.record(1, 3, None);
        w.cells[3][3] = head as u32;
        for p in [head, tail] { store(p, 12, u32::MAX); store(p, 16, 0x1234); }
        let mut expected = bytes(w.sim.ptr(), 0xD00);
        expected[0x60..0x62].copy_from_slice(&1u16.to_le_bytes());
        let result = invoke(fc4p_0070fce0 as *const () as usize, w.sim.ptr(), &[], 0x1234, 0x5678, false);
        assert_eq!((result.eax, result.ecx), (3, w.sim.ptr() as u32));
        assert_eq!(bytes(w.sim.ptr(), 0xD00), expected);
        for p in [head, tail] { assert_eq!((word(p, 12), word(p, 16)), (0, 0)); }
        TrafficSim::from_ptr(w.sim.ptr()).set_max_cell_x(-1);
        store(head, 12, 17);
        invoke(fc4p_0070fce0 as *const () as usize, w.sim.ptr(), &[], 0, 0, false);
        assert_eq!(word(head, 12), 17);
    }
}

#[test]
fn map_exits_test_all_modes_rotations_and_ferry_fallbacks() {
    reset();
    let _inner = fake(0x0070_DDD0, inner as *const () as usize);
    let mut w = World::new();
    let mut connections = vec![0u16; 0x2E / 2];
    unsafe { NetworkSim::from_ptr(w.sim.ptr()).set_connections(connections.as_mut_ptr().cast()); }
    for direction in 0..4 {
        STATE.with(|s| { let mut s = s.borrow_mut(); s.direction = direction; s.inner = (0, 0); });
        for mode in 0..9 {
            for rotation in 0..4 {
                connections.fill(0);
                connections[mode] = 1 << (direction + rotation * 4);
                unsafe { assert_eq!(fc4p_0070fa40(w.sim.ptr(), -1, 1), 1); }
            }
        }
    }
    connections.fill(0);
    for (side, x, z, index) in [(0, -1, 1, 1), (1, 2, -1, 2), (2, 4, 2, 2), (3, 1, 4, 1)] {
        w.edges[side][index * 2] = 0x200;
        let ptr = unsafe { boundary(w.sim.ptr(), x, z) } as u32;
        unsafe { assert_eq!(fc4p_0070fa40(w.sim.ptr(), x, z), (ptr & !255) | 1); }
        w.edges[side][index * 2] = 0;
        unsafe { assert_eq!(fc4p_0070fa40(w.sim.ptr(), x, z), ptr & !255); }
    }
    STATE.with(|s| s.borrow_mut().direction = -1);
    unsafe { assert_eq!(fc4p_0070fa40(w.sim.ptr(), -1, -1), 0xFFFF_FF00); }
    assert!(calls().iter().all(|c| matches!(c, Call::Inner(_, _, _))));
}

struct Masks { _guards: Vec<Fake>, _travel: Box<[u32; 9]>, _masks: Box<[u32; 13]> }
impl Masks {
    fn new() -> Self {
        let mut travel = Box::new([0, 2, 2, 1, 1, 1, 1, 2, 2]);
        let mut masks = Box::new([0x10, 0x2, 0x4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let guards = vec![fake_data(0x00AB_31A8, travel.as_mut_ptr().cast()), fake_data(0x00AB_322C, masks.as_mut_ptr().cast())];
        Self { _guards: guards, _travel: travel, _masks: masks }
    }
}

#[test]
fn outside_occupants_skip_inside_cells_and_search_types_in_order() {
    reset();
    let _masks = Masks::new();
    let mut w = World::new();
    let vt = table(&[(0x2C, occupant as *const () as usize)]);
    let mut manager = object(16, &vt);
    let mut manager_ptr = manager.ptr();
    let _global = fake_data(0x00B4_3D8C, (&mut manager_ptr as *mut *mut u8).cast());
    let _edge = fake(0x0070_F970, edge as *const () as usize);
    let mut edge = [0u32; 2];
    STATE.with(|s| { let mut s = s.borrow_mut(); s.edge = edge.as_mut_ptr() as usize; s.occupant = 0x1357_2468; });
    unsafe {
        assert!(fc4p_007104a0(w.sim.ptr(), 0, 3, 3).is_null());
        assert!(calls().is_empty());
        assert!(fc4p_007104a0(w.sim.ptr(), 0, -1, 1).is_null());
        assert_eq!(calls(), [Call::Edge(w.sim.ptr() as usize, -1, 1)]);
        edge[0] = 0x10;
        assert_eq!(edge[0], 0x10);
        assert_eq!(fc4p_007104a0(w.sim.ptr(), 0, 4, 1) as u32, 0x1357_2468);
        assert_eq!(calls(), [Call::Edge(w.sim.ptr() as usize, 4, 1), Call::Occupant(manager_ptr as usize, 4, 4, 1)]);
    }
}

#[test]
fn pool_lookup_wanted_mask_ferry_existing_default_and_stale_ids() {
    reset();
    let _masks = Masks::new();
    let _find = fake(0x0070_EFC0, pool as *const () as usize);
    let mut w = World::new();
    let mut header = Object::new(24);
    let mut node = Object::new(24);
    unsafe {
        let sim = NetworkSim::from_ptr(w.sim.ptr());
        sim.set_pool_header(header.ptr()); sim.set_default_pool(0x1234_5678 as *mut u8);
        store(node.ptr(), 20, 0x8765_4321);
        w.edges[0][0] = 4;
        let query = |w: &mut World, kind| invoke(fc4p_007114e0 as *const () as usize, w.sim.ptr(), &[kind, (-1i32) as u32, 0], 0, 0, false);
        assert_eq!(query(&mut w, 3).eax, 0);
        assert_eq!(query(&mut w, 1).eax, 0x1234_5678);
        assert_eq!(query(&mut w, u32::MAX).eax, 0x1234_5678);
        assert!(calls().is_empty());
        w.edges[0][0] = (2 << 16) | 0x200;
        STATE.with(|s| s.borrow_mut().pool_node = node.ptr() as usize);
        let result = query(&mut w, 3);
        assert_eq!((result.eax, result.edx), (0x8765_4321, 0xAABB_CCDD));
        assert_eq!(calls(), [Call::Pool(w.sim.ptr().add(0xB8) as usize, 2)]);
        w.edges[0][0] = (7 << 16) | 4;
        STATE.with(|s| s.borrow_mut().pool_node = header.ptr() as usize);
        assert_eq!(query(&mut w, 1).eax, 0x1234_5678);
        assert_eq!(w.edges[0][0], 4);
        assert_eq!(calls(), [Call::Pool(w.sim.ptr().add(0xB8) as usize, 7)]);
    }
}

#[test]
fn density_key_uses_low_bytes_and_only_the_low_evening_byte() {
    let mut w = World::new();
    let mut buckets = vec![0u32; 7];
    let mut nodes = [Object::new(80), Object::new(80)];
    unsafe {
        let sim = NetworkSim::from_ptr(w.sim.ptr());
        sim.set_buckets(buckets.as_mut_ptr().cast()); sim.set_buckets_end(buckets.as_mut_ptr().add(7).cast());
        let key = 0x0103_0102u32;
        store(nodes[0].ptr(), 0, nodes[1].ptr() as u32); store(nodes[0].ptr(), 4, 123);
        store(nodes[1].ptr(), 0, 0); store(nodes[1].ptr(), 4, key);
        buckets[(key % 7) as usize] = nodes[0].ptr() as u32;
        let result = fc4p_00711460(w.sim.ptr(), 0x10001, 0x20002, 0x30003, 0x10001);
        assert_eq!(result as u32, nodes[1].ptr().add(8) as u32);
        assert_eq!((result >> 32) as u32, nodes[1].ptr() as u32);
        assert_eq!(fc4p_00711460(w.sim.ptr(), 1, 2, 3, 0x100), 0x00AB_31D0);
        store(nodes[1].ptr(), 4, 0);
        assert_eq!(fc4p_00711460(w.sim.ptr(), 1, 2, 3, 1), 0x00AB_31D0);
    }
}

#[test]
fn connected_destinations_take_the_maximum_ignore_invalid_ids_and_keep_negative_indices() {
    reset();
    let _vector = fake(0x005F_F510, vector as *const () as usize);
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let vt = table(&[(0x0C, holder as *const () as usize)]);
    let mut lot = object(16, &vt);
    let mut w = World::new();
    let mut subnetworks = vec![0u32; 3 * 7];
    subnetworks[3] = 5; subnetworks[7 + 3] = 7; subnetworks[14 + 3] = 6;
    subnetworks[7 + 4] = 9; subnetworks[7 + 2] = 77;
    unsafe {
        let sim = NetworkSim::from_ptr(w.sim.ptr());
        sim.set_subnetworks(subnetworks.as_mut_ptr().cast()); sim.set_subnetworks_end(subnetworks.as_mut_ptr().add(21).cast());
        STATE.with(|s| { let mut s = s.borrow_mut(); s.holder = 0x8765; s.vector = vec![0, 1, 2, 3, u32::MAX]; s.vector_ok = true; });
        for (category, expected) in [(0, 7), (1, 9), (-1, 77)] {
            let result = fc4p_007106c0(w.sim.ptr(), 0xBAD0, lot.ptr(), category);
            assert_eq!(result, (0x1234_5678u64 << 32) | expected);
            let address = STATE.with(|s| s.borrow().vector.as_ptr() as usize);
            assert_eq!(calls(), [Call::Holder(lot.ptr() as usize), Call::Vector(0x8765, 0x2A4F_9C74), Call::Free(address)]);
        }
        assert_eq!(fc4p_007106c0(w.sim.ptr(), 0xBAD0, lot.ptr(), 4), 0xBAD0u64 << 32);
        assert!(calls().is_empty());
        STATE.with(|s| s.borrow_mut().vector_ok = false);
        assert_eq!(fc4p_007106c0(w.sim.ptr(), 0, lot.ptr(), 1), 0x1234_5678u64 << 32);
        calls();
        STATE.with(|s| s.borrow_mut().vector.clear());
        assert_eq!(fc4p_007106c0(w.sim.ptr(), 0, lot.ptr(), 1), 0xDEAD_4321u64 << 32);
        assert_eq!(calls(), [Call::Holder(lot.ptr() as usize), Call::Vector(0x8765, 0x2A4F_9C74)]);
    }
}
