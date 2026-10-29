// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, float_environment, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call { List, Rect, Grid, Connection(u32, i32, i32), Holder, Property(u32, u32, u32) }
#[derive(Default)]
struct State { calls: Vec<Call>, connection: u32, found_kind: u32 }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) { STATE.with(|s| s.borrow_mut().calls.push(c)); }
fn calls() -> Vec<Call> { STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)) }
unsafe extern "thiscall" fn list(this: *mut u8) -> *mut u8 { log(Call::List); unsafe { this.add(4) } }
unsafe extern "thiscall" fn rect(this: *mut u8, out: *mut i32) {
    log(Call::Rect); unsafe { core::ptr::copy(this.add(8).cast::<i32>(), out, 4); }
}
unsafe extern "thiscall" fn grid(this: *mut u8) -> *mut u8 { log(Call::Grid); unsafe { word(this, 4) as *mut u8 } }
extern "thiscall" fn connection(_this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
    log(Call::Connection(kind, x, z));
    STATE.with(|s| { let s = s.borrow(); if kind == s.found_kind { s.connection as *mut u8 } else { core::ptr::null_mut() } })
}
extern "thiscall" fn holder(_this: *mut u8) -> *mut u8 { log(Call::Holder); 0x12345678 as *mut u8 }
extern "C" fn property(holder: *mut u8, id: u32, value: f32) { log(Call::Property(holder as u32, id, value.to_bits())); }

struct World {
    sim: Object, source: Object, header: Object, trips: Vec<Object>, cells: Vec<Vec<u8>>, rows: Box<[u32]>,
    _vector: Box<[u32; 1]>, counts: Box<[u32; 9]>, _tables: Vec<Vec<usize>>, _objects: Vec<Object>, _constants: Box<[f32; 3]>, _guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![table(&[(0x0C, list as *const () as usize), (0x14, rect as *const () as usize)]),
            table(&[(0x60, grid as *const () as usize)]), table(&[(0x2C, connection as *const () as usize)]),
            table(&[(0x0C, holder as *const () as usize)])];
        let mut sim = Object::new(0xD10); let mut source = object(24, &tables[0]); let header = Object::new(8);
        let mut cells = vec![vec![77u8; 8]; 8];
        let mut rows = vec![0u32; 24].into_boxed_slice();
        for x in 0..8 { rows[x * 3] = cells[x].as_mut_ptr() as u32; }
        let mut vector = Box::new([rows.as_mut_ptr() as u32]);
        let mut grid = object(8, &tables[1]); let mut manager = object(4, &tables[2]); let mut connection = object(4, &tables[3]);
        let mut global = Object::new(4);
        unsafe { store(global.ptr(), 0, manager.ptr() as u32); }
        let mut constants = Box::new([4294967296.0f32, 255.0, 12582912.0]);
        let mut counts = Box::new([0u32; 9]);
        let mut guards = vec![fake(0x005FE4F0, property as *const () as usize), fake_data(0x00B43D8C, global.ptr())];
        for (i, address) in [0x00A80AA8, 0x00A9D8B0, 0x00AB32D8].into_iter().enumerate() {
            guards.push(fake_data(address, unsafe { constants.as_mut_ptr().add(i).cast() }));
        }
        unsafe {
            store(grid.ptr(), 4, vector.as_mut_ptr() as u32);
            let totals = SourceTotalsSim::from_ptr(sim.ptr()); totals.set_length_grid(grid.ptr()); totals.set_mode_counts(counts.as_mut_ptr().cast());
            let sim = TrafficSim::from_ptr(sim.ptr()); sim.set_min_cell_x(0); sim.set_min_cell_z(0); sim.set_max_cell_x(7); sim.set_max_cell_z(7); sim.set_commute_maximum(6.0);
            write_unaligned(source.ptr().add(8).cast::<[i32; 4]>(), [2, 2, 2, 2]);
        }
        STATE.with(|s| { let mut s = s.borrow_mut(); s.connection = connection.ptr() as u32; s.found_kind = 3; });
        let mut w = Self { sim, source, header, trips: Vec::new(), cells, rows, _vector: vector, counts,
            _tables: tables, _objects: vec![grid, manager, connection, global], _constants: constants, _guards: guards };
        w.link(); w
    }
    fn add(&mut self, count: u32, kind: u8, flags: u8, status: u8, length: f32, modes: u32) {
        let mut node = Object::new(0x38);
        unsafe {
            core::ptr::write_bytes(node.ptr(), 0xA5, 0x38);
            let t = CompletedTrip::from_ptr(node.ptr()); t.set_count(count); t.set_kind(kind); t.set_flags(flags);
            t.set_status(status); t.set_length(length); t.set_travel_types(modes);
        }
        self.trips.push(node); self.link();
    }
    fn link(&mut self) {
        unsafe {
            let header = self.header.ptr(); let mut next = header;
            for t in self.trips.iter_mut().rev() { store(t.ptr(), 0, next as u32); next = t.ptr(); }
            store(header, 0, next as u32); store(self.source.ptr(), 4, header as u32);
        }
    }
    fn run(&mut self) { unsafe { fc4p_0070f0a0(self.sim.ptr(), self.source.ptr()); } }
}

#[test]
fn source_commute_example_counts_modes_and_fills_inclusive_rectangle() {
    let _fpu = GameFpu::new(); let mut w = World::new();
    unsafe { write_unaligned(w.source.ptr().add(8).cast::<[i32; 4]>(), [2, 2, 3, 4]); }
    w.add(40, 0, 0, 3, 3.0, 2);
    let before = float_environment(); w.run(); assert_eq!(float_environment(), before);
    assert_eq!(calls(), [Call::List, Call::List, Call::Rect, Call::Grid]);
    for x in 0..8 { for z in 0..8 { assert_eq!(w.cells[x][z], if (2..=3).contains(&x) && (2..=4).contains(&z) { 128 } else { 77 }); } }
    unsafe {
        assert_eq!(CompletedTrip::from_ptr(w.trips[0].ptr()).flags(), 8);
        assert_eq!(SourceTotalsSim::from_ptr(w.sim.ptr()).pass_total(), 1);
        assert_eq!([word(w.sim.ptr(), 0x1CC), word(w.sim.ptr(), 0x1D8)], [40, 40]);
        let sim = TrafficSim::from_ptr(w.sim.ptr()); assert_eq!((sim.commute_sum(), sim.commute_count()), (120.0, 40));
    }
    assert_eq!(*w.counts, [0, 40, 0, 0, 0, 0, 0, 0, 0]); assert_ne!(w.rows[0], 0);
}

#[test]
fn failed_trip_clears_map_and_failed_freight_probe_still_counts_freight() {
    let _fpu = GameFpu::new(); let mut w = World::new();
    w.add(10, 0, 0, 3, 3.0, 0x1FF);
    w.add(5, 0, 0xFF, 5, 1.0, 0x1FF);
    w.add(7, 3, 2 | 8, 5, 1.0, 0x1FF);
    w.add(3, 3, 0, 3, 2.0, 0x100);
    w.add(11, 0, 2, 3, 4.0, 0x1FF);
    w.add(19, 2, 8, 1, 1.0, 0x1FF);
    w.run();
    assert_eq!(w.cells[2][2], 0);
    unsafe {
        let sim = TrafficSim::from_ptr(w.sim.ptr());
        assert_eq!((sim.commute_sum(), sim.commute_count()), (30.0, 10));
        assert_eq!((sim.freight_sum(), sim.freight_count()), (48.0, 10));
        assert_eq!([word(w.sim.ptr(), 0x1CC), word(w.sim.ptr(), 0x1D8)], [10, 10]);
        assert_eq!(SourceTotalsSim::from_ptr(w.sim.ptr()).pass_total(), 6);
        let flags: Vec<_> = w.trips.iter_mut().map(|t| CompletedTrip::from_ptr(t.ptr()).flags()).collect();
        assert_eq!(flags, [8, 0xF7, 2, 8, 10, 0]);
    }
    assert_eq!(*w.counts, [10, 10, 10, 10, 10, 10, 10, 10, 13]);
}

#[test]
fn source_byte_rounding_ties_clamps_and_nan() {
    let _fpu = GameFpu::new();
    for (length, expected) in [(3.0, 128), (0.6, 26), (1.5, 64), (6.0, 255), (7.0, 255), (0.0, 1), (-1.0, 255), (f32::NAN, 255), (f32::INFINITY, 255)] {
        let mut w = World::new(); w.add(1, 0, 0, 3, length, 0); w.run(); assert_eq!(w.cells[2][2], expected);
    }
}

#[test]
fn source_boundary_search_stops_at_first_connection_and_publishes_unscaled_mean() {
    let _fpu = GameFpu::new(); let mut w = World::new();
    unsafe { write_unaligned(w.source.ptr().add(8).cast::<[i32; 4]>(), [-1, 5, -1, 5]); }
    w.add(10, 0, 0, 3, 3.0, 0); w.add(5, 0, 0, 5, 0.0, 0);
    w.run();
    assert_eq!(calls(), [Call::List, Call::List, Call::Rect,
        Call::Connection(0, -1, 5), Call::Connection(1, -1, 5), Call::Connection(2, -1, 5), Call::Connection(3, -1, 5),
        Call::Holder, Call::Property(0x12345678, 0xCA947077, 4.0f32.to_bits())]);
    assert!(w.cells.iter().flatten().all(|&x| x == 77));
}

#[test]
fn source_empty_zero_count_and_missing_boundary_connection() {
    let _fpu = GameFpu::new(); let mut w = World::new();
    let before = unsafe { bytes(w.sim.ptr(), 0xD10) };
    w.run(); assert_eq!(calls(), [Call::List, Call::List]);
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD10) }, before);
    w.add(0, 0, 8, 1, 1.0, 0); w.run();
    assert_eq!(calls(), [Call::List, Call::List]);
    unsafe { assert_eq!(CompletedTrip::from_ptr(w.trips[0].ptr()).flags(), 0); }
    w.add(1, 0, 0, 3, 1.0, 0);
    unsafe { write_unaligned(w.source.ptr().add(8).cast::<[i32; 4]>(), [8, 1, 8, 1]); }
    STATE.with(|s| s.borrow_mut().connection = 0); w.run();
    let c = calls(); assert_eq!(c.len(), 16); assert_eq!(c.last(), Some(&Call::Connection(12, 8, 1)));
}

#[test]
fn source_unsigned_trip_count_and_signed_mean_denominator() {
    let _fpu = GameFpu::new(); let mut w = World::new();
    w.add(0x80000001, 0, 0, 3, 1.0, 1);
    w.run();
    assert_eq!(w.cells[2][2], 255);
    unsafe {
        let sim = TrafficSim::from_ptr(w.sim.ptr()); assert_eq!(sim.commute_sum().to_bits(), 2147483648.0f32.to_bits());
        assert_eq!(sim.commute_count(), 0x80000001);
    }
    assert_eq!(w.counts[0], 0x80000001);
}

#[test]
fn source_wrapped_zero_total_and_unrecognized_kind_keep_their_statistic_rules() {
    let _fpu = GameFpu::new(); let mut w = World::new();
    w.add(u32::MAX, 255, 0, 3, 1.0, 0x100);
    w.add(1, 255, 8, 5, 1.0, 0x1FF);
    w.run();
    assert_eq!(calls(), [Call::List, Call::List]);
    assert!(w.cells.iter().flatten().all(|&c| c == 77));
    assert_eq!(*w.counts, [0, 0, 0, 0, 0, 0, 0, 0, u32::MAX]);
    unsafe {
        let sim = TrafficSim::from_ptr(w.sim.ptr());
        assert_eq!((sim.commute_count(), sim.freight_count()), (0, 0));
        assert_eq!(SourceTotalsSim::from_ptr(w.sim.ptr()).pass_total(), 2);
        assert_eq!(CompletedTrip::from_ptr(w.trips[0].ptr()).flags(), 8);
        assert_eq!(CompletedTrip::from_ptr(w.trips[1].ptr()).flags(), 0);
    }
}
