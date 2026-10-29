// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use crate::traffic::test_support::{bytes, float_environment, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Position(usize),
    Cell(usize, u32, u32),
    Budget(usize, u32),
    Insert(usize, u32),
    Find(usize, u32),
    Erase(usize, [u32; 2]),
    Allocate(u32),
    Construct(usize),
    Retain,
    Type(u32),
    X(u32),
    Z(u32),
    Zero(u32),
    Post(usize, usize, u32),
    Release,
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    sim: usize,
    kind: usize,
    x: u32,
    z: u32,
    intact: bool,
    damaged: bool,
    message: usize,
    cell_outputs: [usize; 2],
    change_cell_during_budget: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

unsafe extern "thiscall" fn position(this: *mut u8, out: *mut u32) -> u32 {
    log(Call::Position(this as usize));
    unsafe {
        out.write(0x3F80_0000);
        out.add(1).write(0x4000_0000);
        out.add(2).write(0x4040_0000);
    }
    0x1111
}
unsafe extern "thiscall" fn cell(this: *mut u8, x: u32, z: u32, out_x: *mut u32, out_z: *mut u32) -> u32 {
    log(Call::Cell(this as usize, x, z));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.cell_outputs = [out_x as usize, out_z as usize];
        unsafe {
            out_x.write(s.x);
            out_z.write(s.z);
        }
    });
    0x2222
}
unsafe extern "thiscall" fn budget(this: *mut u8) -> u32 {
    let count = STATE.with(|s| unsafe { word(this, 0x740 + s.borrow().kind * 4) });
    log(Call::Budget(this as usize, count));
    STATE.with(|s| {
        let s = s.borrow();
        if s.change_cell_during_budget {
            unsafe {
                (s.cell_outputs[0] as *mut u32).write(21);
                (s.cell_outputs[1] as *mut u32).write(22);
            }
        }
    });
    0x3333
}
unsafe extern "thiscall" fn insert(this: *mut u8, result: *mut u32, key: *const u32) -> u32 {
    log(Call::Insert(this as usize, unsafe { key.read() }));
    unsafe {
        result.write(0x1234);
        result.add(1).write(0x5678);
        result.add(2).write(1);
    }
    0x4444
}
unsafe extern "thiscall" fn find(this: *mut u8, result: *mut u32, key: *const u32) -> *mut u32 {
    log(Call::Find(this as usize, unsafe { key.read() }));
    let present = STATE.with(|s| {
        let s = s.borrow();
        if this as usize == s.sim + 0x7B8 {
            s.intact
        } else {
            s.damaged
        }
    });
    unsafe {
        result.write(if present { 0x9999 } else { 0 });
        result.add(1).write(0xABCD);
    }
    result
}
unsafe extern "thiscall" fn erase(this: *mut u8, iter: *const u32) -> u32 {
    log(Call::Erase(this as usize, unsafe { [iter.read(), iter.add(1).read()] }));
    0x5555
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().message as *mut u8)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    log(Call::Construct(this as usize));
    this
}
extern "thiscall" fn retain(_: *mut u8) -> u32 {
    log(Call::Retain);
    1
}
extern "thiscall" fn release(_: *mut u8) -> u32 {
    log(Call::Release);
    0x6666
}
extern "thiscall" fn message_type(_: *mut u8, x: u32) -> u32 {
    log(Call::Type(x));
    0
}
extern "thiscall" fn message_x(_: *mut u8, x: u32) -> u32 {
    log(Call::X(x));
    0
}
extern "thiscall" fn message_z(_: *mut u8, x: u32) -> u32 {
    log(Call::Z(x));
    0
}
extern "thiscall" fn message_zero(_: *mut u8, x: u32) -> u32 {
    log(Call::Zero(x));
    0
}
extern "thiscall" fn post(this: *mut u8, message: *mut u8, flag: u32) -> u32 {
    log(Call::Post(this as usize, message as usize, flag));
    0
}

struct World {
    sim: Object,
    occupant: Object,
    city: Object,
    server: Object,
    message: Object,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[(0x20, position as *const () as usize)]),
            table(&[(0x17C, cell as *const () as usize)]),
            table(&[(0x10, post as *const () as usize)]),
            table(&[
                (4, retain as *const () as usize),
                (8, release as *const () as usize),
                (0x14, message_type as *const () as usize),
                (0x2C, message_x as *const () as usize),
                (0x40, message_z as *const () as usize),
                (0x54, message_zero as *const () as usize),
            ]),
        ];
        let mut w = Self {
            sim: object(0xD00, &[]),
            occupant: object(0x20, &tables[0]),
            city: object(0x20, &tables[1]),
            server: object(0x20, &tables[2]),
            message: object(0x2C, &tables[3]),
            _tables: tables,
            _fakes: vec![
                fake(0x0070_D8A0, budget as *const () as usize),
                fake(0x0060_E210, insert as *const () as usize),
                fake(0x0060_D170, find as *const () as usize),
                fake(0x0060_D1F0, erase as *const () as usize),
                fake(0x0091_33DA, allocate as *const () as usize),
                fake(0x0091_34D6, construct as *const () as usize),
            ],
        };
        let sim = unsafe { NeighborSim::from_ptr(w.sim.ptr()) };
        sim.set_city(w.city.ptr());
        sim.set_server(w.server.ptr());
        sim.set_road_count(100);
        sim.set_damaged_count(3);
        sim.set_potholes(3);
        STATE.with(|s| {
            *s.borrow_mut() = State {
                sim: w.sim.ptr() as usize,
                x: 12,
                z: 34,
                message: w.message.ptr() as usize,
                ..State::default()
            }
        });
        w
    }
    fn run(&mut self, inserted: u32, kind: u32, count: u32) -> (u32, [u32; 3]) {
        unsafe {
            store(self.sim.ptr(), 0x740 + kind as usize * 4, count);
        }
        STATE.with(|s| s.borrow_mut().kind = kind as usize);
        let mut frame = [
            fc4p_007155d0 as *const () as u32,
            self.sim.ptr() as u32,
            self.occupant.ptr() as u32,
            inserted,
            kind,
            0,
            0,
            0,
            0,
        ];
        // The copied argument words are inspected immediately after the callee pops them.
        unsafe {
            core::arch::asm!(
            "push [edi + 16]", "push [edi + 12]", "push [edi + 8]", "mov ecx, [edi + 4]",
            "call dword ptr [edi]", "mov [edi + 20], eax",
            "mov eax, [esp - 12]", "mov [edi + 24], eax", "mov eax, [esp - 8]", "mov [edi + 28], eax",
            "mov eax, [esp - 4]", "mov [edi + 32], eax", in("edi") frame.as_mut_ptr(), clobber_abi("C"));
        }
        (frame[5], [frame[6], frame[7], frame[8]])
    }
    fn sim(&mut self) -> NeighborSim {
        unsafe { NeighborSim::from_ptr(self.sim.ptr()) }
    }
    fn prefix(&mut self, count: u32) -> Vec<Call> {
        vec![
            Call::Position(self.occupant.ptr() as usize),
            Call::Cell(self.city.ptr() as usize, 0x3F80_0000, 0x4040_0000),
            Call::Budget(self.sim.ptr() as usize, count),
        ]
    }
}

#[test]
fn road_insert_counts_calls_even_for_an_existing_key() {
    let mut w = World::new();
    let env = float_environment();
    for (count, old) in [(101, 100), (102, 101)] {
        let (eax, args) = w.run(0xABCD_0001, 0, old);
        assert_eq!((eax, args), (count, [12, 0x0C22, 0]));
        let mut expected = w.prefix(count);
        expected.push(Call::Insert(w.sim.ptr() as usize + 0x7B8, 0x0C22));
        assert_eq!(calls(), expected);
    }
    assert_eq!(float_environment(), env);
    assert_eq!(w.sim().road_count(), 102);
}

#[test]
fn road_removing_damage_posts_coordinates_and_preserves_unrelated_bytes() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().damaged = true);
    unsafe {
        store(w.sim.ptr(), 0x740, 100);
    }
    let mut expected_bytes = unsafe { bytes(w.sim.ptr(), 0xD00) };
    for (offset, value) in [(0x740, 99u32), (0x7AC, 99), (0x7B0, 2)] {
        expected_bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    expected_bytes[0x318..0x31A].copy_from_slice(&2u16.to_le_bytes());
    let (eax, args) = w.run(0xABCD_0000, 0, 100);
    assert_eq!((eax, args), (0x6666, [12, 0x0C22, 0]));
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD00) }, expected_bytes);
    let p = w.sim.ptr() as usize;
    let mut expected = w.prefix(99);
    expected.extend([
        Call::Find(p + 0x7B8, 0x0C22),
        Call::Find(p + 0x7CC, 0x0C22),
        Call::Erase(p + 0x7CC, [0x9999, 0xABCD]),
        Call::Allocate(0x2C),
        Call::Construct(w.message.ptr() as usize),
        Call::Retain,
        Call::Type(0x6924_7DC8),
        Call::X(12),
        Call::Z(34),
        Call::Zero(0),
        Call::Post(w.server.ptr() as usize, w.message.ptr() as usize, 0),
        Call::Release,
    ]);
    assert_eq!(calls(), expected);
}

#[test]
fn road_removing_an_intact_cell_skips_damage_and_message() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.intact = true;
        s.damaged = true;
    });
    assert_eq!(w.run(0, 6, 1).0, 0x5555);
    let p = w.sim.ptr() as usize;
    let mut expected = w.prefix(0);
    expected.extend([Call::Find(p + 0x7B8, 0x0C22), Call::Erase(p + 0x7B8, [0x9999, 0xABCD])]);
    assert_eq!(calls(), expected);
    assert_eq!(
        (w.sim().potholes(), w.sim().damaged_count(), w.sim().road_count()),
        (3, 3, 99)
    );
}

#[test]
fn road_absent_removal_saturates_network_count_and_wraps_road_count() {
    let mut w = World::new();
    w.sim().set_road_count(0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.x = 0xFFFF_FF12;
        s.z = 0xCAFE_3456;
    });
    let (_, args) = w.run(0xFF00, 10, 0);
    assert_eq!(args, [0xFFFF_FF12, 0xCAFE_3656, 10]);
    assert_eq!(w.sim().road_count(), u32::MAX);
    let p = w.sim.ptr() as usize;
    let mut expected = w.prefix(0);
    expected.extend([Call::Find(p + 0x7B8, 0xCAFE_3656), Call::Find(p + 0x7CC, 0xCAFE_3656)]);
    assert_eq!(calls(), expected);
}

#[test]
fn highways_only_change_network_count_and_budget() {
    let mut w = World::new();
    for kind in [2, 12] {
        assert_eq!(w.run(0xFF01, kind, u32::MAX), (0x3333, [12, 0xFF01, kind]));
        assert_eq!(calls(), w.prefix(0));
        assert_eq!(w.sim().road_count(), 100);
    }
}

#[test]
fn road_damage_counters_wrap_at_their_original_widths() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().damaged = true);
    w.sim().set_potholes(0);
    w.sim().set_damaged_count(0);
    w.run(0, 0, 0);
    assert_eq!((w.sim().potholes(), w.sim().damaged_count()), (u16::MAX, u32::MAX));
}

#[test]
fn road_cell_outputs_are_read_again_after_budget_callbacks() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.damaged = true;
        s.change_cell_during_budget = true;
    });
    let (_, args) = w.run(0, 0, 100);
    assert_eq!(args, [21, 0x1516, 0]);
    let actual = calls();
    assert!(actual.contains(&Call::Find(w.sim.ptr() as usize + 0x7B8, 0x1516)));
    assert!(actual.contains(&Call::X(21)));
    assert!(actual.contains(&Call::Z(22)));
}
