// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Commute-map scan and slicing cases from the core's map-refresh tests.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call { Budget(u32, u32, u32, u32), Lot(i32, i32, u32), Holder(u32), Capacity(u32, u32) }
#[derive(Default)]
struct State { calls: Vec<Call>, checks: u32, expire: u32, primary: usize, secondary: usize, mutate: usize }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> { STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)) }
unsafe extern "thiscall" fn expired(scope: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe {
            s.calls.push(Call::Budget(scope.read(), scope.add(1).read(), scope.add(2).read(), scope.add(3).read()));
            assert_eq!(scope.add(4).cast::<u8>().read(), 0);
        }
        s.checks += 1;
        u32::from(s.expire != 0 && s.checks >= s.expire) | 0xAABB_CC00
    })
}
unsafe extern "thiscall" fn lot(_this: *mut u8, x: i32, z: i32, primary: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); s.calls.push(Call::Lot(x, z, primary));
        if x == 0 && primary != 0 { s.primary as *mut u8 }
        else if x == 1 && primary == 0 { s.secondary as *mut u8 }
        else { core::ptr::null_mut() }
    })
}
unsafe extern "thiscall" fn holder(this: *mut u8) -> u32 {
    let id = unsafe { word(this, 4) };
    STATE.with(|s| s.borrow_mut().calls.push(Call::Holder(id)));
    id
}
unsafe extern "thiscall" fn capacity(_this: *mut u8, holder: u32, category: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); s.calls.push(Call::Capacity(holder, category));
        if s.mutate != 0 { unsafe { store(s.mutate as *mut u8, 0, 77); } }
    });
    let mask = if holder == 0x10 { 0b1011 } else { 0b0100 };
    if mask & 1 << category != 0 { 0x100 } else { 0 }
}

struct World {
    sim: Object, scan: Object, rows: Vec<u32>, cells: Vec<Vec<u8>>, pacing: Box<[u32; 2]>,
    flags: Box<u32>, _objects: Vec<Object>, _tables: Vec<Vec<usize>>, _guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![table(&[(0x50, capacity as *const () as usize)]), table(&[(0x18, lot as *const () as usize)]), table(&[(0x0C, holder as *const () as usize)])];
        let sim = object(0xD00, &tables[0]);
        let mut manager = object(8, &tables[1]); let mut primary = object(8, &tables[2]); let mut secondary = object(8, &tables[2]);
        unsafe { store(primary.ptr(), 4, 0x10); store(secondary.ptr(), 4, 0x20); }
        STATE.with(|s| { let mut s = s.borrow_mut(); s.primary = primary.ptr() as usize; s.secondary = secondary.ptr() as usize; });
        let mut pacing = Box::new([1024u32, 0]); let mut flags = Box::new(1u32);
        // Store the pointer global separately so its allocation stays stable when the fixture moves.
        let mut pointer = Object::new(4);
        unsafe { store(pointer.ptr(), 0, manager.ptr() as u32); }
        let guards = vec![
            fake(0x0068_BF80, expired as *const () as usize), fake_data(0x00B4_B318, pacing.as_mut_ptr().cast()),
            fake_data(0x00B4_B320, (&mut *flags as *mut u32).cast()), fake_data(0x00B4_3D08, pointer.ptr()),
        ];
        let mut w = Self { sim, scan: Object::new(32), rows: vec![0; 15], cells: vec![vec![0xFF; 3]; 5], pacing, flags,
            _objects: vec![manager, primary, secondary, pointer], _tables: tables, _guards: guards };
        for (x, cells) in w.cells.iter_mut().enumerate() { w.rows[x * 3] = cells.as_mut_ptr() as u32; }
        unsafe {
            let sim = RefreshSim::from_ptr(w.sim.ptr()); sim.set_destination_rows(w.rows.as_mut_ptr().cast()); sim.set_cursor(w.scan.ptr());
            let scan = Scan::from_ptr(w.scan.ptr()); scan.set_maximum_x(2); scan.set_maximum_z(0); scan.set_step(1);
        }
        w
    }
    fn run(&mut self) -> Registers {
        let mut budget = [0x1234_5678u32, 0x9ABC_DEF0];
        unsafe { invoke(fc4p_0070f6e0 as *const () as usize, self.sim.ptr(), &[budget.as_mut_ptr() as u32], 0, 0, false) }
    }
}

#[test]
fn refresh_primary_secondary_missing_lots_preserves_high_bits_and_only_counts_primary() {
    let mut w = World::new();
    unsafe { RefreshSim::from_ptr(w.sim.ptr()).set_free_counts([u32::MAX, 10, 20, 30]); }
    let result = w.run();
    assert_eq!((result.eax as u8, result.ecx), (1, 0));
    assert_eq!([w.cells[1][1], w.cells[2][1], w.cells[3][1]], [0xFB, 0xFC, 0xF8]);
    unsafe {
        assert_eq!(RefreshSim::from_ptr(w.sim.ptr()).free_counts(), [0, 11, 20, 31]);
        let scan = Scan::from_ptr(w.scan.ptr()); assert_eq!((scan.state(), scan.x(), scan.z()), (2, 0, 1));
    }
    assert_eq!(calls(), [
        Call::Budget(0x1234_5678, 0x9ABC_DEF0, 1024, 1024), Call::Lot(0, 0, 1), Call::Holder(0x10),
        Call::Capacity(0x10, 0), Call::Capacity(0x10, 1), Call::Capacity(0x10, 2), Call::Capacity(0x10, 3),
        Call::Budget(0x1234_5678, 0x9ABC_DEF0, 2048, 1024), Call::Lot(1, 0, 1), Call::Lot(1, 0, 0), Call::Holder(0x20),
        Call::Capacity(0x20, 0), Call::Capacity(0x20, 1), Call::Capacity(0x20, 2), Call::Capacity(0x20, 3),
        Call::Budget(0x1234_5678, 0x9ABC_DEF0, 3072, 1024), Call::Lot(2, 0, 1), Call::Lot(2, 0, 0),
        Call::Budget(0x1234_5678, 0x9ABC_DEF0, 4096, 1024),
    ]);
    assert_eq!(*w.pacing, [1024, u32::MAX]);
}

#[test]
fn refresh_yield_resume_and_global_initialization_keep_pending_cell_unprocessed() {
    let mut w = World::new();
    *w.flags = 0xA0; *w.pacing = [7, 9];
    STATE.with(|s| s.borrow_mut().expire = 1);
    let before = unsafe { bytes(w.scan.ptr(), 32) };
    let result = w.run();
    assert_eq!((result.eax, result.ecx), (0xFFFF_FF00, 0));
    assert_eq!(unsafe { bytes(w.scan.ptr(), 32) }, before);
    assert_eq!(*w.flags, 0xA1); assert_eq!(*w.pacing, [1024, u32::MAX]);
    assert_eq!(calls(), [Call::Budget(0x1234_5678, 0x9ABC_DEF0, 1024, 1024)]);
    STATE.with(|s| { let mut s = s.borrow_mut(); s.checks = 0; s.expire = 2; });
    assert_eq!(w.run().eax as u8, 0);
    unsafe { assert_eq!(Scan::from_ptr(w.scan.ptr()).x(), 0); }
    calls();
    STATE.with(|s| s.borrow_mut().expire = 0);
    assert_eq!(w.run().eax as u8, 1);
    let resumed = calls(); assert!(resumed.contains(&Call::Lot(1, 0, 1)));
    assert!(!resumed.contains(&Call::Lot(0, 0, 1)));
}

#[test]
fn refresh_pacing_halves_doubles_and_keeps_original_eax_ecx_on_each_return_path() {
    for completed in [true, false] {
        for (step, history, expected_step, expected_history) in [
            (21 * 1024, 15, 21 * 512, 0), (1024, (-3i32) as u32, 1024, 0),
            (512, (-3i32) as u32, 512, (-3i32) as u32), (1024, 5, 1024, 4),
        ] {
            let mut w = World::new(); *w.pacing = [step, history];
            unsafe { Scan::from_ptr(w.scan.ptr()).set_state(2); }
            if !completed { STATE.with(|s| s.borrow_mut().expire = 1); }
            let result = w.run();
            // At step 512 the first clock check is at work 1024, after the already-finished scan exits.
            let did_finish = completed || step == 512;
            assert_eq!(result.eax as u8, u8::from(did_finish));
            assert_eq!(result.ecx, 0);
            assert_eq!(*w.pacing, [expected_step, expected_history]);
        }
    }
    // A check helper can leave a small step with a nonzero count already recorded in the scope.
    // Directly exercise doubling through a completed scan after two work units of 512.
    let mut w = World::new(); *w.pacing = [512, (-3i32) as u32];
    unsafe { Scan::from_ptr(w.scan.ptr()).set_maximum_x(-1); }
    w.run();
    assert_eq!(*w.pacing, [1024, 0]);
    let mut w = World::new(); *w.pacing = [512, (-3i32) as u32];
    STATE.with(|s| s.borrow_mut().expire = 1);
    let result = w.run();
    assert_eq!(*w.pacing, [1024, 0]);
    assert_eq!((result.eax, result.ecx), (512, 1024));
}
