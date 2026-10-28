// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Budget(u32, u32, u32, u32), Lot(i32, i32, u32), Holder(u32), Lookup(u32),
    Variant, Kind, Short, Wide, Set(u32, u32, u32), Capacity(u32, u32), Switches,
}
#[derive(Default)]
struct State { calls: Vec<Call>, checks: u32, expire: u32, mutate_work: Option<u32> }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) { STATE.with(|s| s.borrow_mut().calls.push(call)); }
fn calls() -> Vec<Call> { STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)) }
unsafe extern "thiscall" fn expired_body(scope: *mut u32) -> u32 {
    STATE.with(|s| { let mut s = s.borrow_mut(); unsafe {
        s.calls.push(Call::Budget(scope.read(), scope.add(1).read(), scope.add(2).read(), scope.add(3).read()));
        if let Some(work) = s.mutate_work { scope.add(2).write(work); }
    }
        s.checks += 1;
        u32::from(s.expire != 0 && s.checks >= s.expire) | 0xABCDEF00
    })
}
#[unsafe(naked)]
unsafe extern "thiscall" fn expired(_scope: *mut u32) -> u32 {
    core::arch::naked_asm!("call {body}", "mov ecx, 0x12345678", "ret", body = sym expired_body);
}
unsafe extern "thiscall" fn lot(this: *mut u8, x: i32, z: i32, primary: u32) -> *mut u8 {
    log(Call::Lot(x, z, primary));
    unsafe { (word(this, 4) as *mut u32).wrapping_offset(x as isize).read() as *mut u8 }
}
unsafe extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    log(Call::Holder(this as u32));
    unsafe { word(this, 4) as *mut u8 }
}
unsafe extern "thiscall" fn lookup(this: *mut u8, id: u32) -> *mut u8 {
    assert_eq!(id, 0x8A0B4100);
    log(Call::Lookup(this as u32));
    unsafe { if word(this, 12) == 0 { core::ptr::null_mut() } else { word(this, 4) as *mut u8 } }
}
unsafe extern "thiscall" fn variant(this: *mut u8) -> *mut u8 { log(Call::Variant); unsafe { word(this, 4) as *mut u8 } }
unsafe extern "thiscall" fn kind(this: *mut u8) -> u32 { log(Call::Kind); unsafe { word(this, 4) } }
unsafe extern "thiscall" fn short(this: *mut u8) -> *mut u8 { log(Call::Short); unsafe { word(this, 8) as *mut u8 } }
unsafe extern "thiscall" fn wide(this: *mut u8) -> *mut u8 { log(Call::Wide); unsafe { word(this, 8) as *mut u8 } }
unsafe extern "thiscall" fn capacity(_this: *mut u8, holder: *mut u8, category: u32) -> u32 {
    log(Call::Capacity(holder as u32, category));
    if category == 0 { 4 } else { category }
}
unsafe extern "fastcall" fn set_body(holder: *mut u8, value: u32, index: u32) {
    log(Call::Set(holder as u32, index, value));
    unsafe {
        store(holder, 12, 1);
        let variant = word(word(holder, 4) as *mut u8, 4) as *mut u8;
        let data = word(variant, 8) as *mut u8;
        if word(variant, 4) == 0x8004 { write_unaligned(data.add(index as usize * 2).cast::<u16>(), value as u16); }
        else { store(data, index as usize * 4, value); }
    }
}
#[unsafe(naked)]
unsafe extern "C" fn set_fake() {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "mov ecx, edi", "mov edx, ebx", "call {body}", "ret", body = sym set_body);
}
unsafe extern "thiscall" fn switches(this: *mut u8) -> u32 { log(Call::Switches); unsafe { word(this, 4) } }

struct World {
    sim: Object, scan: Object, pacing: Box<[u32; 2]>, flags: Box<u32>,
    _manager: Object, lots: Box<[u32; 4]>, _tables: Vec<Vec<usize>>, _objects: Vec<Object>, _guards: Vec<Fake>,
}
impl World {
    fn new(edge: bool) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![table(&[(0x50, capacity as *const () as usize)]), table(&[(0x18, lot as *const () as usize)]),
            table(&[(0x0C, holder as *const () as usize)]), table(&[(0x24, lookup as *const () as usize)]),
            table(&[(0x14, variant as *const () as usize)]), table(&[(0x0C, kind as *const () as usize),
                (0x18C, short as *const () as usize), (0x19C, wide as *const () as usize)]),
            table(&[(0x60, switches as *const () as usize)])];
        let sim = object(0xD10, &tables[0]);
        let scan = Object::new(32);
        let mut manager = object(8, &tables[1]);
        let mut lots = Box::new([0u32; 4]);
        unsafe { store(manager.ptr(), 4, lots.as_mut_ptr() as u32); }
        let mut global = Object::new(4);
        unsafe { store(global.ptr(), 0, manager.ptr() as u32); }
        let mut pacing = Box::new([1024u32, 0]);
        let mut flags = Box::new(1u32);
        let (p, f) = if edge { (0x00B4B330, 0x00B4B338) } else { (0x00B4B300, 0x00B4B308) };
        let guards = vec![fake(0x0068BF80, expired as *const () as usize), fake(0x0070DE70, set_fake as *const () as usize),
            fake_data(p, pacing.as_mut_ptr().cast()), fake_data(f, (&mut *flags as *mut u32).cast()),
            fake_data(0x00B43D08, global.ptr())];
        let mut world = Self { sim, scan, pacing, flags, _manager: manager, lots, _tables: tables, _objects: vec![global], _guards: guards };
        unsafe {
            let sim = PassSim::from_ptr(world.sim.ptr()); sim.set_cursor(world.scan.ptr());
            for offset in [0x1C0, 0x1C4, 0x1C8] { store(world.sim.ptr(), offset, 0); }
            let scan = PassScan::from_ptr(world.scan.ptr()); scan.set_maximum_x(3); scan.set_maximum_z(0); scan.set_step(1);
        }
        world
    }
    fn add_holder(&mut self, x: usize, kind: u32, present: bool) -> (*mut u8, *mut u8) {
        let mut lot = object(8, &self._tables[2]);
        let mut holder = object(16, &self._tables[3]);
        let mut property = object(8, &self._tables[4]);
        let mut variant = object(12, &self._tables[5]);
        let mut values = Object::new(48);
        unsafe {
            store(lot.ptr(), 4, holder.ptr() as u32);
            store(holder.ptr(), 4, property.ptr() as u32); store(holder.ptr(), 12, u32::from(present));
            store(property.ptr(), 4, variant.ptr() as u32); store(variant.ptr(), 4, kind); store(variant.ptr(), 8, values.ptr() as u32);
            if present { for i in 4..8usize {
                if kind == 0x8004 { write_unaligned(values.ptr().add(i * 2).cast::<u16>(), (i - 1) as u16); }
                else { store(values.ptr(), i * 4, 0x10000 + i as u32); }
            } }
        }
        self.lots[x] = lot.ptr() as u32;
        let answer = (holder.ptr(), values.ptr());
        self._objects.extend([lot, holder, property, variant, values]);
        answer
    }
    fn run(&mut self, function: usize) -> Registers {
        let mut budget = [11u32, 22];
        unsafe { invoke(function, self.sim.ptr(), &[budget.as_mut_ptr() as u32], 0, 0, false) }
    }
}

#[test]
fn capacity_rollover_creates_missing_property_and_handles_short_and_wide_values() {
    let mut w = World::new(false);
    let (missing, zero) = w.add_holder(0, 0x8004, false);
    let (small, short) = w.add_holder(1, 0x8004, true);
    let (large, wide) = w.add_holder(2, 0x8006, true);
    unsafe { store(w.sim.ptr(), 0x1C0, u32::MAX); }
    let r = w.run(fc4p_0070e4f0 as *const () as usize);
    assert_eq!(r.eax as u8, 1);
    unsafe {
        assert_eq!(std::slice::from_raw_parts(zero.cast::<u16>(), 12), &[0; 12]);
        assert_eq!(std::slice::from_raw_parts(short.cast::<u16>(), 12), &[0, 0, 0, 0, 0, 0, 0, 0, 3, 4, 5, 6]);
        assert_eq!(std::slice::from_raw_parts(wide.cast::<u32>(), 12), &[0, 0, 0, 0, 0, 0, 0, 0, 0x10004, 0x10005, 0x10006, 0x10007]);
        assert_eq!([word(w.sim.ptr(), 0x1C0), word(w.sim.ptr(), 0x1C4), word(w.sim.ptr(), 0x1C8)], [11, 3, 6]);
    }
    let actual = calls();
    let writes: Vec<_> = actual.iter().filter(|c| matches!(c, Call::Set(..))).collect();
    let mut expected = Vec::new();
    for (h, values) in [(missing, [0, 0, 0, 0]), (small, [3, 4, 5, 6]), (large, [0x10004, 0x10005, 0x10006, 0x10007])] {
        for (k, value) in values.into_iter().enumerate() {
            expected.push(Call::Set(h as u32, k as u32 + 8, value)); expected.push(Call::Set(h as u32, k as u32 + 4, 0));
        }
    }
    assert_eq!(writes, expected.iter().collect::<Vec<_>>());
    let mut position = 0usize;
    for (x, h, is_missing, is_short) in [(0, missing, true, true), (1, small, false, true), (2, large, false, false)] {
        assert_eq!(actual[position], Call::Budget(11, 22, (x as u32 + 1) * 1024, 1024)); position += 1;
        assert_eq!(actual[position], Call::Lot(x, 0, 1)); position += 1;
        assert_eq!(actual[position], Call::Holder(w.lots[x as usize])); position += 1;
        for k in 4..8u32 {
            assert_eq!(actual[position], Call::Lookup(h as u32)); position += 1;
            if !(is_missing && k == 4) {
                assert_eq!(&actual[position..position + 3], &[Call::Variant, Call::Kind, if is_short { Call::Short } else { Call::Wide }]);
                position += 3;
            }
            position += 2;
        }
        assert_eq!(&actual[position..position + 3], &[Call::Capacity(h as u32, 0), Call::Capacity(h as u32, 1), Call::Capacity(h as u32, 2)]);
        position += 3;
    }
    assert_eq!(&actual[position..], &[Call::Budget(11, 22, 4096, 1024), Call::Lot(3, 0, 1), Call::Budget(11, 22, 5120, 1024)]);
}

#[test]
fn capacity_scan_yields_before_a_cell_and_resumes_without_repeating_it() {
    let mut w = World::new(false);
    let (holder, _) = w.add_holder(0, 0x8004, false);
    *w.flags = 0xA0; *w.pacing = [3, 9];
    STATE.with(|s| s.borrow_mut().expire = 2);
    assert_eq!(w.run(fc4p_0070e4f0 as *const () as usize).eax as u8, 0);
    unsafe { assert_eq!(PassScan::from_ptr(w.scan.ptr()).x(), 0); }
    assert_eq!(*w.flags, 0xA1);
    let first = calls();
    assert!(first.contains(&Call::Capacity(holder as u32, 2)));
    STATE.with(|s| s.borrow_mut().expire = 0);
    assert_eq!(w.run(fc4p_0070e4f0 as *const () as usize).eax as u8, 1);
    assert!(!calls().contains(&Call::Lot(0, 0, 1)));
}

#[test]
fn capacity_pacing_and_ecx_match_completion_and_timeout_paths() {
    for completed in [true, false] {
        for (step, history, expected_step, expected_history) in [
            (21 * 1024, 15u32, 21 * 512, 0), (1024, (-3i32) as u32, 1024, 0),
            (512, (-3i32) as u32, 512, (-3i32) as u32), (1024, 5, 1024, 4),
        ] {
            let mut w = World::new(false); *w.pacing = [step, history];
            unsafe { PassScan::from_ptr(w.scan.ptr()).set_state(2); }
            if !completed { STATE.with(|s| s.borrow_mut().expire = 1); }
            let r = w.run(fc4p_0070e4f0 as *const () as usize);
            let did_finish = completed || step == 512;
            assert_eq!(r.eax as u8, u8::from(did_finish));
            assert_eq!(r.ecx, if did_finish { byte(w.sim.ptr() as u32, 2) } else { 0x12345678 });
            assert_eq!(*w.pacing, [expected_step, expected_history]);
        }
    }
}

struct Edges { world: World, nodes: Vec<Object>, buckets: Box<[u32; 1]>, _storage: Vec<Box<[u32]>> }
impl Edges {
    fn new() -> Self {
        let mut w = World::new(true);
        let mut switch = object(8, &w._tables[6]);
        let mut netrows = vec![0u32; 12].into_boxed_slice();
        let mut switchrows = vec![0u32; 12].into_boxed_slice();
        let mut n = vec![0u32; 4].into_boxed_slice(); n[0] = 1;
        let mut s = vec![0u32; 4].into_boxed_slice(); s[1] = 1;
        for x in 0..4 { netrows[x * 3] = unsafe { n.as_mut_ptr().add(x) as u32 }; switchrows[x * 3] = unsafe { s.as_mut_ptr().add(x) as u32 }; }
        let mut switchvec = vec![switchrows.as_mut_ptr() as u32].into_boxed_slice();
        let mut header = Object::new(12); let mut ferry = Object::new(12);
        unsafe {
            store(header.ptr(), 0, ferry.ptr() as u32); store(ferry.ptr(), 0, header.ptr() as u32);
            ferry.ptr().add(8).write(0); ferry.ptr().add(9).write(0); // Departure is a different cell.
            ferry.ptr().add(10).write(3); ferry.ptr().add(11).write(0);
            store(switch.ptr(), 4, switchvec.as_mut_ptr() as u32);
            let sim = PassSim::from_ptr(w.sim.ptr()); sim.set_network_rows(netrows.as_mut_ptr().cast());
            sim.set_switches(switch.ptr()); sim.set_ferry_header(header.ptr());
            PassScan::from_ptr(w.scan.ptr()).set_maximum_x(2);
        }
        let mut nodes = Vec::new();
        for x in 0..5u32 { for mode in 0..9u32 { for evening in 0..2u32 {
            let mut node = Object::new(80);
            unsafe {
                core::ptr::write_bytes(node.ptr(), 0xA5, 80);
                store(node.ptr(), 0, nodes.last_mut().map_or(0, |n: &mut Object| n.ptr() as u32));
                store(node.ptr(), 4, evening << 24 | mode << 16 | x << 8);
            }
            nodes.push(node);
        } } }
        let mut buckets = Box::new([nodes.last_mut().unwrap().ptr() as u32]);
        unsafe {
            let sim = PassSim::from_ptr(w.sim.ptr()); sim.set_edge_buckets(buckets.as_mut_ptr().cast());
            sim.set_edge_buckets_end(buckets.as_mut_ptr().add(1).cast());
        }
        w._objects.extend([switch, header, ferry]);
        Self { world: w, nodes, buckets, _storage: vec![netrows, switchrows, n, s, switchvec] }
    }
    fn assert_cleared(&mut self, cells: &[u32]) {
        for node in &mut self.nodes {
            let key = unsafe { word(node.ptr(), 4) }; let x = (key >> 8) & 255;
            let b = unsafe { bytes(node.ptr(), 80) };
            assert!(b[8..42].iter().all(|&v| v == 0xA5));
            assert_eq!(&b[42..74], &[if cells.contains(&x) { 0 } else { 0xA5 }; 32], "key {key:08x}");
            assert!(b[74..].iter().all(|&v| v == 0xA5));
        }
    }
}

#[test]
fn edge_clear_visits_network_switch_then_ferry_arrival_and_retains_other_bytes() {
    let mut e = Edges::new();
    assert_eq!(e.world.run(fc4p_007110b0 as *const () as usize).eax as u8, 1);
    e.assert_cleared(&[0, 1, 3]);
    assert_eq!(calls(), [Call::Switches, Call::Budget(11, 22, 1024, 1024), Call::Budget(11, 22, 2048, 1024)]);
    assert_ne!(e.buckets[0], 0);
}

#[test]
fn edge_clear_budget_is_after_active_cell_and_defers_ferries_until_completion() {
    let mut e = Edges::new();
    STATE.with(|s| s.borrow_mut().expire = 1);
    let r = e.world.run(fc4p_007110b0 as *const () as usize);
    assert_eq!((r.eax, r.ecx), (0xFFFFFF00, 0x12345678));
    e.assert_cleared(&[0]);
    STATE.with(|s| s.borrow_mut().expire = 0);
    assert_eq!(e.world.run(fc4p_007110b0 as *const () as usize).eax as u8, 1);
    e.assert_cleared(&[0, 1, 3]);
}
