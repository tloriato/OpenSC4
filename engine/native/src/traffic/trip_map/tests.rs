// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call { Grid, Budget(u32), Flags(u32), SetFlags(u32, u32), Location(u32), Zone(u32),
    Road(u32), Link(u32), Lot(i32, i32, u32), Rect(u32) }
#[derive(Default)]
struct State { calls: Vec<Call>, lots: Vec<u32>, checks: u32, expire: u32 }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) { STATE.with(|s| s.borrow_mut().calls.push(c)); }
fn calls() -> Vec<Call> { STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)) }
unsafe extern "thiscall" fn grid(this: *mut u8) -> u32 { log(Call::Grid); unsafe { word(this, 4) } }
unsafe extern "thiscall" fn expired(scope: *mut u32) -> u32 {
    log(Call::Budget(unsafe { scope.add(2).read() }));
    STATE.with(|s| { let mut s = s.borrow_mut(); s.checks += 1; u32::from(s.expire != 0 && s.checks >= s.expire) })
}
unsafe extern "thiscall" fn flags(this: *mut u8) -> u32 { log(Call::Flags(this as u32)); unsafe { word(this, 4) } }
unsafe extern "thiscall" fn set_flags(this: *mut u8, flags: u32) { log(Call::SetFlags(this as u32, flags)); unsafe { store(this, 4, flags); } }
unsafe extern "thiscall" fn location(this: *mut u8, x: *mut i32, z: *mut i32) {
    log(Call::Location(this as u32)); unsafe { x.write(word(this, 8) as i32); z.write(word(this, 12) as i32); }
}
unsafe extern "thiscall" fn zone(this: *mut u8) -> u32 { log(Call::Zone(this as u32)); unsafe { word(this, 16) } }
unsafe extern "thiscall" fn road(this: *mut u8) -> u32 { log(Call::Road(this as u32)); unsafe { word(this, 20) } }
unsafe extern "thiscall" fn link(this: *mut u8) -> u32 { log(Call::Link(this as u32)); unsafe { word(this, 24) } }
unsafe extern "thiscall" fn rect(this: *mut u8, out: *mut i32) {
    log(Call::Rect(this as u32)); unsafe { core::ptr::copy(this.add(28).cast::<i32>(), out, 4); }
}
unsafe extern "thiscall" fn lot(_this: *mut u8, x: i32, z: i32, primary: u32) -> *mut u8 {
    log(Call::Lot(x, z, primary));
    STATE.with(|s| {
        for &lot in &s.borrow().lots {
            let p = lot as *mut u8;
            unsafe { if x >= word(p, 28) as i32 && z >= word(p, 32) as i32 && x <= word(p, 36) as i32 && z <= word(p, 40) as i32 { return p; } }
        }
        core::ptr::null_mut()
    })
}

struct World {
    sim: Object, iterator: Box<[u32; 2]>, list: Box<[u32; 1]>, header: Object, lots: Vec<Object>, nodes: Vec<Object>,
    cells: Vec<Vec<u8>>, rows: Box<[u32]>, _vector: Box<[u32; 1]>, pacing: Box<[u32; 2]>, flags: Box<u32>,
    _tables: Vec<Vec<usize>>, _objects: Vec<Object>, _dx: Box<[i32; 4]>, _dz: Box<[i32; 4]>, _guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![table(&[(0x60, grid as *const () as usize)]), table(&[(0x18, lot as *const () as usize)]),
            table(&[(0x30, flags as *const () as usize), (0x34, set_flags as *const () as usize),
                (0x54, location as *const () as usize), (0x7C, zone as *const () as usize), (0x70, road as *const () as usize),
                (0xEC, link as *const () as usize), (0x38, rect as *const () as usize)])];
        let mut sim = Object::new(0xD10); let mut header = Object::new(12);
        let mut list = Box::new([header.ptr() as u32]);
        let mut iterator = Box::new([header.ptr() as u32, list.as_mut_ptr() as u32]);
        let mut cells = vec![vec![7u8; 32]; 32]; let mut rows = vec![0u32; 96].into_boxed_slice();
        for x in 0..32 { rows[x * 3] = cells[x].as_mut_ptr() as u32; }
        let mut vector = Box::new([rows.as_mut_ptr() as u32]);
        let mut grid = object(8, &tables[0]); let mut manager = object(4, &tables[1]); let mut global = Object::new(4);
        unsafe { store(grid.ptr(), 4, vector.as_mut_ptr() as u32); store(global.ptr(), 0, manager.ptr() as u32); }
        let mut dx = Box::new([-1, 0, 1, 0]); let mut dz = Box::new([0, -1, 0, 1]);
        let mut pacing = Box::new([1024u32, 0]); let mut flags = Box::new(1u32);
        let guards = vec![fake(0x0068BF80, expired as *const () as usize), fake_data(0x00B4B324, pacing.as_mut_ptr().cast()),
            fake_data(0x00B4B32C, (&mut *flags as *mut u32).cast()), fake_data(0x00B43D08, global.ptr()),
            fake_data(0x00AB325C, dx.as_mut_ptr().cast()), fake_data(0x00AB326C, dz.as_mut_ptr().cast())];
        unsafe {
            let v = TripMapSim::from_ptr(sim.ptr()); v.set_iterator(iterator.as_mut_ptr().cast()); v.set_length_grid(grid.ptr());
            let v = TrafficSim::from_ptr(sim.ptr()); v.set_min_cell_x(0); v.set_min_cell_z(0); v.set_max_cell_x(31); v.set_max_cell_z(31);
        }
        Self { sim, iterator, list, header, lots: Vec::new(), nodes: Vec::new(), cells, rows, _vector: vector, pacing, flags,
            _tables: tables, _objects: vec![grid, manager, global], _dx: dx, _dz: dz, _guards: guards }
    }
    fn add(&mut self, zone: u32, road: bool, rect: [i32; 4], visit: bool) -> *mut u8 {
        let mut lot = object(44, &self._tables[2]);
        unsafe {
            store(lot.ptr(), 4, 0x1234FFFF); store(lot.ptr(), 8, rect[0] as u32); store(lot.ptr(), 12, rect[1] as u32);
            store(lot.ptr(), 16, zone); store(lot.ptr(), 20, u32::from(road)); store(lot.ptr(), 24, 0);
            write_unaligned(lot.ptr().add(28).cast::<[i32; 4]>(), rect);
        }
        let pointer = lot.ptr(); STATE.with(|s| s.borrow_mut().lots.push(pointer as u32)); self.lots.push(lot);
        if visit {
            let mut node = Object::new(12); unsafe { store(node.ptr(), 8, pointer as u32); }
            self.nodes.push(node); self.rewind();
        }
        pointer
    }
    fn rewind(&mut self) {
        let mut next = self.header.ptr();
        for node in self.nodes.iter_mut().rev() { unsafe { store(node.ptr(), 0, next as u32); } next = node.ptr(); }
        self.iterator[0] = next as u32;
    }
    fn run(&mut self) -> Registers {
        let mut budget = [11u32, 22];
        unsafe { invoke(fc4p_0070fef0 as *const () as usize, self.sim.ptr(), &[budget.as_mut_ptr() as u32], 0, 0, false) }
    }
}

#[test]
fn trip_map_core_example_searches_open_directions_and_writes_zero_without_donor() {
    let mut w = World::new();
    let source = w.add(8, false, [20, 20, 20, 21], true);
    let other_zone = w.add(1, true, [19, 20, 19, 20], false);
    let no_road = w.add(8, false, [21, 20, 21, 20], false);
    let donor = w.add(8, true, [22, 19, 22, 20], false);
    let lone = w.add(9, false, [5, 5, 6, 5], true);
    w.cells[22][20] = 37; w.cells[22][19] = 99;
    let result = w.run();
    assert_eq!((result.eax as u8, result.ecx), (1, w.iterator.as_mut_ptr() as u32));
    assert_eq!((w.cells[20][20], w.cells[20][21]), (37, 37));
    assert_eq!((w.cells[5][5], w.cells[6][5]), (0, 0));
    unsafe {
        assert_eq!(word(source, 4), 0x1234FF7F); assert_eq!(word(lone, 4), 0x1234FF7F);
        assert_eq!(word(donor, 4), 0x1234FFFF);
        let sim = TripMapSim::from_ptr(w.sim.ptr()); assert_eq!((sim.phase_x(), sim.phase_z()), (5, 5));
    }
    let actual = calls();
    let start = [Call::Grid, Call::Budget(1024), Call::Flags(source as u32), Call::SetFlags(source as u32, 0x1234FF7F),
        Call::Location(source as u32), Call::Zone(source as u32), Call::Road(source as u32), Call::Link(source as u32), Call::Location(source as u32),
        Call::Lot(19, 20, 0), Call::Zone(other_zone as u32), Call::Lot(20, 19, 0),
        Call::Lot(21, 20, 0), Call::Zone(no_road as u32), Call::Road(no_road as u32),
        Call::Lot(20, 21, 0), Call::Zone(source as u32),
        Call::Lot(22, 20, 0), Call::Zone(donor as u32), Call::Road(donor as u32), Call::Rect(source as u32)];
    assert_eq!(&actual[..start.len()], &start);
    assert_eq!(actual.last(), Some(&Call::Budget(3072)));
    assert_eq!(w.iterator[0], w.list[0]); assert_ne!(w.rows[0], 0);
}

#[test]
fn trip_map_link_uses_linked_location_and_skips_neighbor_search() {
    let mut w = World::new(); let source = w.add(8, false, [2, 2, 3, 3], true);
    let link = w.add(1, false, [10, 11, 10, 11], false);
    unsafe { store(source, 24, link as u32); } w.cells[10][11] = 91;
    w.run();
    assert_eq!([w.cells[2][2], w.cells[2][3], w.cells[3][2], w.cells[3][3]], [91; 4]);
    let actual = calls(); assert!(!actual.iter().any(|c| matches!(c, Call::Lot(..))));
    assert!(actual.contains(&Call::Location(link as u32)));
}

#[test]
fn trip_map_other_zones_and_existing_road_access_only_clear_flags_and_save_location() {
    let mut w = World::new(); let a = w.add(1, false, [2, 3, 2, 3], true);
    let b = w.add(9, true, [6, 7, 6, 7], true);
    let before = w.cells.clone(); w.run(); assert_eq!(w.cells, before);
    assert_eq!(calls(), [Call::Grid, Call::Budget(1024), Call::Flags(a as u32), Call::SetFlags(a as u32, 0x1234FF7F),
        Call::Location(a as u32), Call::Zone(a as u32), Call::Budget(2048), Call::Flags(b as u32),
        Call::SetFlags(b as u32, 0x1234FF7F), Call::Location(b as u32), Call::Zone(b as u32), Call::Road(b as u32), Call::Budget(3072)]);
}

#[test]
fn trip_map_yields_before_next_lot_and_resumes_from_advanced_iterator() {
    let mut w = World::new(); let a = w.add(8, false, [2, 2, 2, 2], true); let b = w.add(9, false, [5, 5, 5, 5], true);
    *w.flags = 0xA0; *w.pacing = [3, 9]; STATE.with(|s| s.borrow_mut().expire = 2);
    let r = w.run(); assert_eq!((r.eax as u8, r.ecx), (0, 0));
    assert_eq!(*w.flags, 0xA1); assert_eq!(w.cells[2][2], 0); assert_eq!(w.cells[5][5], 7);
    assert_eq!(w.iterator[0], w.nodes[1].ptr() as u32); calls();
    STATE.with(|s| s.borrow_mut().expire = 0);
    assert_eq!(w.run().eax as u8, 1); assert_eq!(w.cells[5][5], 0);
    let actual = calls(); assert!(!actual.contains(&Call::Flags(a as u32))); assert!(actual.contains(&Call::Flags(b as u32)));
}

#[test]
fn trip_map_empty_null_iterator_and_pacing_doubling_keep_original_registers() {
    for null_list in [false, true] {
        let mut w = World::new(); if null_list { w.iterator[1] = 0; }
        let before = unsafe { bytes(w.sim.ptr(), 0xD10) }; let r = w.run();
        assert_eq!((r.eax as u8, r.ecx), (1, w.iterator.as_mut_ptr() as u32));
        assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD10) }, before);
    }
    for completed in [false, true] {
        let mut w = World::new(); w.add(1, true, [2, 2, 2, 2], true);
        *w.pacing = [512, (-3i32) as u32];
        if !completed { STATE.with(|s| s.borrow_mut().expire = 1); }
        let r = w.run();
        assert_eq!(r.eax, if completed { 513 } else { 1024 });
        assert_eq!(r.ecx, if completed { w.iterator.as_mut_ptr() as u32 } else { 0 });
        assert_eq!(*w.pacing, [1024, 0]);
    }
}

#[test]
fn trip_map_city_edge_closes_directions_without_asking_for_outside_lots() {
    let mut w = World::new(); w.add(8, false, [0, 0, 0, 0], true); w.run();
    let lookups: Vec<_> = calls().into_iter().filter(|c| matches!(c, Call::Lot(..))).collect();
    assert_eq!(lookups, [Call::Lot(1, 0, 0), Call::Lot(0, 1, 0)]); assert_eq!(w.cells[0][0], 0);
}
