// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use crate::traffic::test_support::{bytes, float_environment, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Prepare,
    Flags(u32),
    Cell,
    Grow(usize, u32),
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
    Budget(usize, u32, u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    sim: usize,
    x: u32,
    z: u32,
    crossing: u32,
    intact: bool,
    damaged: bool,
    message: usize,
    outputs: [usize; 2],
    mutate: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn prepare(_: *mut u8) {
    log(Call::Prepare);
}
extern "thiscall" fn flags(_: *mut u8, mask: u32) -> u32 {
    log(Call::Flags(mask));
    STATE.with(|s| s.borrow().crossing)
}
unsafe extern "thiscall" fn cell(_: *mut u8, x: *mut u32, z: *mut u32) {
    log(Call::Cell);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe {
            x.write(s.x);
            z.write(s.z);
        }
        s.outputs = [x as usize, z as usize];
    });
}
unsafe extern "thiscall" fn budget(this: *mut u8) -> u64 {
    let sim = unsafe { RailSim::from_ptr(this) };
    log(Call::Budget(
        this as usize,
        sim.rail_cells(),
        sim.cell_count(),
        sim.damaged_count(),
    ));
    0x7654_3210_FEDC_BA98
}
extern "thiscall" fn grow(this: *mut u8, count: u32) {
    log(Call::Grow(this as usize, count));
}
unsafe extern "thiscall" fn insert(this: *mut u8, out: *mut u32, key: *const u32) {
    log(Call::Insert(this as usize, unsafe { key.read() }));
    unsafe {
        out.write(0x9999);
        out.add(1).write(0xABCD);
        out.add(2).write(1);
    }
}
unsafe extern "thiscall" fn find(this: *mut u8, out: *mut u32, key: *const u32) -> *mut u32 {
    log(Call::Find(this as usize, unsafe { key.read() }));
    let present = STATE.with(|s| {
        let s = s.borrow();
        if this as usize == s.sim + 0x7EC {
            s.intact
        } else {
            s.damaged
        }
    });
    unsafe {
        out.write(if present { 0x9999 } else { 0 });
        out.add(1).write(0xABCD);
    }
    out
}
unsafe extern "thiscall" fn erase(this: *mut u8, iter: *const u32) {
    log(Call::Erase(this as usize, unsafe { [iter.read(), iter.add(1).read()] }));
    STATE.with(|s| {
        let s = s.borrow();
        if s.mutate {
            unsafe {
                (s.outputs[0] as *mut u32).write(21);
                (s.outputs[1] as *mut u32).write(22);
            }
        }
    });
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().message as *mut u8)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    log(Call::Construct(this as usize));
    this
}
extern "thiscall" fn retain(_: *mut u8) {
    log(Call::Retain);
}
extern "thiscall" fn release(_: *mut u8) {
    log(Call::Release);
}
extern "thiscall" fn message_type(_: *mut u8, v: u32) {
    log(Call::Type(v));
}
extern "thiscall" fn message_x(_: *mut u8, v: u32) {
    log(Call::X(v));
}
extern "thiscall" fn message_z(_: *mut u8, v: u32) {
    log(Call::Z(v));
}
extern "thiscall" fn message_zero(_: *mut u8, v: u32) {
    log(Call::Zero(v));
}
extern "thiscall" fn post(this: *mut u8, message: *mut u8, flag: u32) {
    log(Call::Post(this as usize, message as usize, flag));
}

struct World {
    sim: Object,
    occupant: Object,
    server: Object,
    message: Object,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[
                (0xC0, prepare as *const () as usize),
                (0x24, flags as *const () as usize),
                (0xEC, cell as *const () as usize),
            ]),
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
            sim: object(0xD50, &[]),
            occupant: object(0x20, &tables[0]),
            server: object(0x20, &tables[1]),
            message: object(0x2C, &tables[2]),
            _tables: tables,
            _fakes: vec![
                fake(0x0070_DA40, budget as *const () as usize),
                fake(0x0071_0DD0, grow as *const () as usize),
                fake(0x0060_D270, insert as *const () as usize),
                fake(0x0060_D170, find as *const () as usize),
                fake(0x0060_D1F0, erase as *const () as usize),
                fake(0x0091_33DA, allocate as *const () as usize),
                fake(0x0091_34D6, construct as *const () as usize),
            ],
        };
        let sim = w.sim();
        sim.set_server(w.server.ptr());
        sim.set_rail_cells(100);
        sim.set_cell_count(1);
        sim.set_damaged_count(1);
        sim.set_potholes(0);
        unsafe {
            store(w.sim.ptr(), 0x7FC, 7);
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                sim: w.sim.ptr() as usize,
                x: 6,
                z: 6,
                message: w.message.ptr() as usize,
                ..State::default()
            }
        });
        w
    }
    fn sim(&mut self) -> RailSim {
        unsafe { RailSim::from_ptr(self.sim.ptr()) }
    }
    fn run(&mut self, inserted: u32) -> (u64, [u32; 2]) {
        let mut frame = [
            fc4p_00715790 as *const () as u32,
            self.sim.ptr() as u32,
            self.occupant.ptr() as u32,
            inserted,
            0,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!("push [edi + 12]", "push [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
                "mov [edi + 16], eax", "mov [edi + 20], edx", "mov eax, [esp - 8]", "mov [edi + 24], eax",
                "mov eax, [esp - 4]", "mov [edi + 28], eax", in("edi") frame.as_mut_ptr(), clobber_abi("C"));
        }
        assert_eq!((frame[4], frame[5]), (0xFEDC_BA98, 0x7654_3210));
        ((u64::from(frame[5]) << 32) | u64::from(frame[4]), [frame[6], frame[7]])
    }
    fn budget(&mut self, rail: u32, count: u32, damaged: u32) -> Call {
        Call::Budget(self.sim.ptr() as usize, rail, count, damaged)
    }
}

#[test]
fn insertion_counts_calls_and_grows_before_inserting() {
    let mut w = World::new();
    let env = float_environment();
    for n in 1..=2 {
        assert_eq!(w.run(0xABCD_0001).1, [6, 0x606]);
        let p = w.sim.ptr() as usize + 0x7EC;
        assert_eq!(
            calls(),
            [
                Call::Prepare,
                Call::Flags(9),
                Call::Cell,
                Call::Grow(p, 8),
                Call::Insert(p, 0x606),
                w.budget(100 + n, 1 + n, 1)
            ]
        );
    }
    assert_eq!(float_environment(), env);
}

#[test]
fn damaged_rail_removal_wraps_road_potholes_and_posts_own_cell() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().damaged = true);
    let mut expected = unsafe { bytes(w.sim.ptr(), 0xD50) };
    for (off, v) in [(0x744, 99u32), (0x7E0, 0), (0x7E4, 0)] {
        expected[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    expected[0x318..0x31A].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(w.run(0xFF00).1, [6, 0x606]);
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD50) }, expected);
    let p = w.sim.ptr() as usize;
    assert_eq!(
        calls(),
        [
            Call::Prepare,
            Call::Flags(9),
            Call::Cell,
            Call::Find(p + 0x7EC, 0x606),
            Call::Find(p + 0x800, 0x606),
            Call::Erase(p + 0x800, [0x9999, 0xABCD]),
            Call::Allocate(0x2C),
            Call::Construct(w.message.ptr() as usize),
            Call::Retain,
            Call::Type(0x6924_7DC8),
            Call::X(6),
            Call::Z(6),
            Call::Zero(0),
            Call::Post(w.server.ptr() as usize, w.message.ptr() as usize, 0),
            Call::Release,
            w.budget(99, 0, 0)
        ]
    );
}

#[test]
fn intact_removal_stops_at_the_first_set_and_absent_removal_wraps_count() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.intact = true;
        s.damaged = true;
    });
    w.run(0);
    let p = w.sim.ptr() as usize;
    assert_eq!(
        calls(),
        [
            Call::Prepare,
            Call::Flags(9),
            Call::Cell,
            Call::Find(p + 0x7EC, 0x606),
            Call::Erase(p + 0x7EC, [0x9999, 0xABCD]),
            w.budget(99, 0, 1)
        ]
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.intact = false;
        s.damaged = false;
    });
    w.sim().set_rail_cells(0);
    w.run(0);
    assert_eq!(
        calls(),
        [
            Call::Prepare,
            Call::Flags(9),
            Call::Cell,
            Call::Find(p + 0x7EC, 0x606),
            Call::Find(p + 0x800, 0x606),
            w.budget(0, u32::MAX, 1)
        ]
    );
}

#[test]
fn crossing_checks_only_al_and_always_prices_rail() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().crossing = 0x1234_5601);
    let occupant = w.occupant.ptr() as u32;
    assert_eq!(w.run(1).1, [occupant, 1]);
    assert_eq!(calls(), [Call::Prepare, Call::Flags(9), w.budget(101, 1, 1)]);
    STATE.with(|s| s.borrow_mut().crossing = 0x1234_5600);
    w.run(1);
    assert!(calls().contains(&Call::Cell));
}

#[test]
fn packed_key_keeps_full_z_and_message_reloads_coordinate_outputs() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.x = 0xABCD_0106;
        s.z = 0x1234_0006;
        s.damaged = true;
        s.mutate = true;
    });
    assert_eq!(w.run(0).1, [21, 0x1234_0606]);
    let c = calls();
    assert!(c.contains(&Call::Find(w.sim.ptr() as usize + 0x7EC, 0x1234_0606)));
    assert!(c.contains(&Call::X(21)) && c.contains(&Call::Z(22)));
}

#[test]
fn insertion_and_damage_counters_wrap() {
    let mut w = World::new();
    w.sim().set_rail_cells(u32::MAX);
    w.sim().set_cell_count(u32::MAX);
    unsafe {
        store(w.sim.ptr(), 0x7FC, u32::MAX);
    }
    w.run(1);
    assert!(calls().contains(&Call::Grow(w.sim.ptr() as usize + 0x7EC, 0)));
    assert_eq!((w.sim().rail_cells(), w.sim().cell_count()), (0, 0));
    STATE.with(|s| s.borrow_mut().damaged = true);
    w.sim().set_damaged_count(0);
    w.run(0);
    assert_eq!(w.sim().damaged_count(), u32::MAX);
}
