// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::test_support::{bytes, float_environment, object, table};
use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Elapsed(u32),
    Reset(u32, [u32; 8]),
    Next(Option<u32>),
    Draw(u32, u64, u64),
    Manager(u32),
    Tile(u32, i32, i32),
    Controller(u32, u32, u32),
    Create(u32, u32),
    Allocate(u32),
    Construct(u32),
    AddRef(u32),
    Type(u32),
    X(u32),
    Z(u32),
    Post(u32, u32, u32),
    Release(u32),
    Free(u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    time: u32,
    indices: Vec<u32>,
    cursor: usize,
    allocation: u32,
    draw: f64,
    manager: u32,
    holder: u32,
    message: u32,
    sim: u32,
    swap_entries: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn elapsed(this: *mut u8) -> u32 {
    log(Call::Elapsed(this as u32));
    STATE.with(|s| s.borrow().time)
}
extern "thiscall" fn reset(this: *mut u32, randomize: u32) {
    let fields = unsafe { this.cast::<[u32; 8]>().read() };
    log(Call::Reset(randomize, fields));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.cursor = 0;
        unsafe {
            this.add(1).write(s.allocation);
        }
    });
}
extern "thiscall" fn next(_this: *mut u8, index: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.indices.get(s.cursor).copied();
        s.cursor += 1;
        s.calls.push(Call::Next(i));
        if let Some(i) = i {
            unsafe {
                index.write(i);
            }
            0xABCD_1201
        } else {
            0xABCD_1200
        }
    })
}
extern "thiscall" fn random(this: *mut u8, low: f64, high: f64) -> f64 {
    log(Call::Draw(this as u32, low.to_bits(), high.to_bits()));
    STATE.with(|s| {
        let s = s.borrow();
        if s.swap_entries != 0 {
            unsafe { AccidentSim::from_ptr(s.sim as *mut u8) }.set_entries(s.swap_entries as *mut u8);
        }
        s.draw
    })
}
extern "thiscall" fn manager(this: *mut u8) -> *mut u8 {
    log(Call::Manager(this as u32));
    STATE.with(|s| s.borrow().manager as *mut u8)
}
extern "thiscall" fn tile(this: *mut u8, x: i32, z: i32) -> *mut u8 {
    log(Call::Tile(this as u32, x, z));
    STATE.with(|s| s.borrow().holder as *mut u8)
}
extern "thiscall" fn controller(this: *mut u8, kind: u32, duration: u32) -> *mut u8 {
    log(Call::Controller(this as u32, kind, duration));
    0x1234_5000 as *mut u8
}
extern "thiscall" fn create(this: *mut u8, controller: *mut u8) -> u32 {
    log(Call::Create(this as u32, controller as u32));
    0
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().message as *mut u8)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    log(Call::Construct(this as u32));
    this
}
extern "thiscall" fn addref(this: *mut u8) {
    log(Call::AddRef(this as u32));
}
extern "thiscall" fn release(this: *mut u8) {
    log(Call::Release(this as u32));
}
extern "thiscall" fn set_type(_this: *mut u8, value: u32) {
    log(Call::Type(value));
}
extern "thiscall" fn set_x(_this: *mut u8, value: u32) {
    log(Call::X(value));
}
extern "thiscall" fn set_z(_this: *mut u8, value: u32) {
    log(Call::Z(value));
}
extern "thiscall" fn post(this: *mut u8, msg: *mut u8, immediate: u32) {
    log(Call::Post(this as u32, msg as u32, immediate));
}
extern "C" fn free(p: *mut u8) {
    log(Call::Free(p as u32));
}

struct World {
    sim: Object,
    view: Object,
    networks: Object,
    city: Object,
    message: Object,
    server: Object,
    occupant: Object,
    holder: Box<u32>,
    entries: Vec<[u32; 3]>,
    _tables: Vec<Vec<usize>>,
    // Each global needs a stable address while more globals are registered.
    #[allow(clippy::vec_box)]
    _globals: Vec<Box<*mut u8>>,
    _floats: Box<[f32; 4]>,
    _doubles: Box<[f64; 2]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(entries: &[(i32, i32, f32)], indices: &[u32]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![
            table(&[(0x38, tile as *const () as usize)]),
            table(&[(0xAC, manager as *const () as usize)]),
            table(&[(0xC0, controller as *const () as usize)]),
            table(&[
                (4, addref as *const () as usize),
                (8, release as *const () as usize),
                (0x14, set_type as *const () as usize),
                (0x2C, set_x as *const () as usize),
                (0x40, set_z as *const () as usize),
            ]),
            table(&[(0x10, post as *const () as usize)]),
        ];
        let mut sim = object(0xD60, &[]);
        let view = Object::new(0x400);
        let mut networks = object(4, &tables[0]);
        let city = object(4, &tables[1]);
        let mut occupant = object(4, &tables[2]);
        let mut message = object(0x2C, &tables[3]);
        let server = object(4, &tables[4]);
        let holder = Box::new(occupant.ptr() as u32);
        let mut entries: Vec<_> = entries
            .iter()
            .map(|&(x, z, p)| [x as u32, z as u32, p.to_bits()])
            .collect();
        let asim = unsafe { AccidentSim::from_ptr(sim.ptr()) };
        asim.set_networks(networks.ptr());
        asim.set_entries(entries.as_mut_ptr().cast());
        asim.set_entries_end(entries.as_mut_ptr().wrapping_add(entries.len()).cast());
        asim.set_deadline(0);
        asim.set_period(60.0);
        asim.set_duration(20.0);
        unsafe { TrafficSim::from_ptr(sim.ptr()) }.set_random(0x7654_3000 as *mut u8);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.indices = indices.to_vec();
            s.time = 1;
            s.draw = 0.25;
            s.manager = 0x4567_8000;
            s.holder = &*holder as *const u32 as u32;
            s.message = message.ptr() as u32;
            s.sim = sim.ptr() as u32;
        });
        let mut w = Self {
            sim,
            view,
            networks,
            city,
            occupant,
            message,
            server,
            holder,
            entries,
            _tables: tables,
            _globals: Vec::new(),
            _floats: Box::new([0.0625, 0.5, 12_582_912.0, 1000.0]),
            _doubles: Box::new([0.0, 1.0]),
            _fakes: Vec::new(),
        };
        for (address, p) in [
            (0x00B4_3DD8, w.view.ptr()),
            (0x00B4_3CEC, w.city.ptr()),
            (0x00B4_3CCC, w.server.ptr()),
        ] {
            let mut g = Box::new(p);
            w._fakes.push(fake_data(address, (&mut *g as *mut *mut u8).cast()));
            w._globals.push(g);
        }
        for (i, address) in [0x00A8_E174, 0x00A8_4D2C, 0x00AB_32D8, 0x00A8_FAF0]
            .into_iter()
            .enumerate()
        {
            w._fakes
                .push(fake_data(address, (&mut w._floats[i] as *mut f32).cast()));
        }
        for (i, address) in [0x00A8_0990, 0x00A8_0AB0].into_iter().enumerate() {
            w._fakes
                .push(fake_data(address, (&mut w._doubles[i] as *mut f64).cast()));
        }
        for (address, function) in [
            (0x0089_0198, elapsed as *const () as usize),
            (0x005C_F610, reset as *const () as usize),
            (0x005C_BD90, next as *const () as usize),
            (0x0091_3A46, random as *const () as usize),
            (0x0055_4400, create as *const () as usize),
            (0x0091_33DA, allocate as *const () as usize),
            (0x0091_34D6, construct as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
        ] {
            w._fakes.push(fake(address, function));
        }
        w.rect([0.0, 0.0, 112.0, 112.0]);
        w
    }
    fn sim(&mut self) -> AccidentSim {
        unsafe { AccidentSim::from_ptr(self.sim.ptr()) }
    }
    fn rect(&mut self, rect: [f32; 4]) {
        unsafe {
            self.view.ptr().add(0x3EC).cast::<[f32; 4]>().write_unaligned(rect);
        }
    }
    fn run(&mut self) {
        unsafe {
            fc4p_007184e0(self.sim.ptr());
        }
    }
    fn prefix(&mut self) -> Vec<Call> {
        vec![
            Call::Elapsed(self.sim.ptr().wrapping_add(0x148) as u32),
            Call::Reset(1, [0x10101, 0, 0, 0, self.entries.len() as u32, 0, 0, 1]),
        ]
    }
}

#[test]
fn accident_deadline_uses_one_elapsed_read_strict_unsigned_comparison_and_raw_period() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[], &[]);
    for (now, deadline) in [(0, 0), (61, 61), (0, u32::MAX), (0x7FFF_FFFF, 0x8000_0000)] {
        STATE.with(|s| s.borrow_mut().time = now);
        w.sim().set_deadline(deadline);
        let before = unsafe { bytes(w.sim.ptr(), 0xD60) };
        w.run();
        assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD60) }, before);
        assert_eq!(calls(), [Call::Elapsed(w.sim.ptr().wrapping_add(0x148) as u32)]);
    }
    for (now, period, expected) in [
        (1, 60.75, 61),
        (u32::MAX, 60.0, 59),
        (2, -3.5, u32::MAX),
        (3, f32::NAN, 3),
        (4, f32::INFINITY, 4),
    ] {
        STATE.with(|s| s.borrow_mut().time = now);
        w.sim().set_deadline(0);
        w.sim().set_period(period);
        let mut expected_memory = unsafe { bytes(w.sim.ptr(), 0xD60) };
        expected_memory[0x160..0x164].copy_from_slice(&expected.to_le_bytes());
        w.run();
        assert_eq!(w.sim().deadline(), expected);
        assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD60) }, expected_memory);
        let mut expected_calls = w.prefix();
        expected_calls.push(Call::Next(None));
        assert_eq!(calls(), expected_calls);
    }
}

#[test]
fn accident_draws_before_rejection_and_frees_iterator_storage_on_exhaustion() {
    let _fpu = GameFpu::new();
    let mut w = World::new(
        &[(1, 1, 0.0), (20, 20, 1.0), (1, 1, f32::NAN), (2, 2, 1.0)],
        &[2, 0, 1, 3],
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.allocation = 0xDEAD_1000;
        s.holder = 0;
    });
    w.run();
    let mut expected = w.prefix();
    for i in [2, 0, 1, 3] {
        expected.extend([Call::Next(Some(i)), Call::Draw(0x7654_3000, 0, 1.0f64.to_bits())]);
        if i == 3 {
            expected.extend([
                Call::Manager(w.city.ptr() as u32),
                Call::Tile(w.networks.ptr() as u32, 2, 2),
            ]);
        }
    }
    expected.extend([Call::Next(None), Call::Free(0xDEAD_1000)]);
    assert_eq!(calls(), expected);
}

#[test]
fn accident_creates_once_then_posts_and_releases_even_when_creation_returns_zero() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(3, 4, 1.0), (5, 6, 1.0)], &[0, 1]);
    STATE.with(|s| s.borrow_mut().allocation = 0xDEAD_1000);
    w.run();
    let msg = w.message.ptr() as u32;
    let mut expected = w.prefix();
    expected.extend([
        Call::Next(Some(0)),
        Call::Draw(0x7654_3000, 0, 1.0f64.to_bits()),
        Call::Manager(w.city.ptr() as u32),
        Call::Tile(w.networks.ptr() as u32, 3, 4),
        Call::Controller(w.occupant.ptr() as u32, 2, 20_000),
        Call::Create(0x4567_8000, 0x1234_5000),
        Call::Allocate(0x2C),
        Call::Construct(msg),
        Call::AddRef(msg),
        Call::Type(0x0C05_E4C3),
        Call::X(3),
        Call::Z(4),
        Call::Post(w.server.ptr() as u32, msg, 0),
        Call::Release(msg),
        Call::Free(0xDEAD_1000),
    ]);
    assert_eq!(calls(), expected);
}

#[test]
fn accident_uses_unrounded_random_result_reloads_entries_and_allows_a_null_server() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(20, 20, 0.0)], &[0]);
    let mut replacement = [7u32, 0, 0.5f32.to_bits()];
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.draw = 0.5 - 2.0f64.powi(-30);
        s.swap_entries = replacement.as_mut_ptr() as u32;
    });
    *w._globals[2] = core::ptr::null_mut();
    w.sim().set_duration(-0.123_75);
    w.run();
    let mut expected = w.prefix();
    expected.extend([
        Call::Next(Some(0)),
        Call::Draw(0x7654_3000, 0, 1.0f64.to_bits()),
        Call::Manager(w.city.ptr() as u32),
        Call::Tile(w.networks.ptr() as u32, 7, 0),
        Call::Controller(w.occupant.ptr() as u32, 2, (-123i32) as u32),
        Call::Create(0x4567_8000, 0x1234_5000),
    ]);
    assert_eq!(calls(), expected);
}

#[test]
fn accident_visibility_is_inclusive_after_floor_and_ceil_in_all_four_directions() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(-1, -1, 1.0), (2, 2, 1.0), (-2, 0, 1.0), (0, 3, 1.0)], &[0, 1, 2, 3]);
    *w.holder = 0;
    w.rect([-8.0, -8.0, 24.0, 24.0]);
    w.run();
    let calls = calls();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Draw(..))).count(), 4);
    let tiles: Vec<_> = calls.into_iter().filter(|c| matches!(c, Call::Tile(..))).collect();
    assert_eq!(
        tiles,
        [
            Call::Tile(w.networks.ptr() as u32, -1, -1),
            Call::Tile(w.networks.ptr() as u32, 2, 2)
        ]
    );
    for (world, lower, upper) in [
        (-16.0, -1, -1),
        (-8.0, -1, 0),
        (0.0, 0, 0),
        (8.0, 0, 1),
        (16.0, 1, 1),
        (24.0, 1, 2),
    ] {
        assert_eq!(bound(world, false, 0.0625, 0.5, 12_582_912.0), lower);
        assert_eq!(bound(world, true, 0.0625, 0.5, 12_582_912.0), upper);
    }
}

#[test]
fn accident_preserves_float_stack_control_word_mxcsr_and_direction_flag() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(1, 1, 0.0)], &[0]);
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = float_environment();
    w.run();
    let after = float_environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(after, before);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
}
