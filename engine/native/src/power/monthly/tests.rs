// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Original-layout versions of the core pole-decay and month-boundary cases.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Curve(usize, u32),
    Copy(usize),
    Shuffle(usize, usize),
    Free(usize),
    Coordinates(usize),
    Distressed(usize),
    Set(usize, u32),
    Factor(usize, u32, u32),
    Funding(usize),
    Apply(usize, usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    delta: f32,
    copy: Vec<*mut u8>,
    sim: *mut u8,
    change_on_set: Option<u32>,
    factor: f32,
    funding: f32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn curve(this: *mut u8, x: f32) -> f32 {
    log(Call::Curve(this as usize, x.to_bits()));
    STATE.with(|s| s.borrow().delta)
}
extern "thiscall" fn copy(out: *mut u32, source: *mut u8) -> u32 {
    log(Call::Copy(source as usize));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe {
            let begin = source.cast::<*mut *mut u8>().read_unaligned();
            let end = source.add(4).cast::<*mut *mut u8>().read_unaligned();
            s.copy.clear();
            let mut p = begin;
            while p != end {
                s.copy.push(p.read());
                p = p.add(1);
            }
            let begin = if s.copy.is_empty() {
                core::ptr::null_mut()
            } else {
                s.copy.as_mut_ptr()
            };
            out.write(begin as u32);
            out.add(1).write(begin.wrapping_add(s.copy.len()) as u32);
            out.add(2).write(begin.wrapping_add(s.copy.len()) as u32);
        }
    });
    out as u32
}
extern "C" fn shuffle(begin: *mut *mut u8, end: *mut *mut u8) {
    log(Call::Shuffle(begin as usize, end as usize));
    STATE.with(|s| s.borrow_mut().copy.reverse());
}
extern "C" fn free(begin: *mut *mut u8) -> u32 {
    log(Call::Free(begin as usize));
    0xFACE_4321
}
#[repr(C)]
struct Pole {
    vt: *const usize,
    distressed: u32,
}
extern "thiscall" fn coordinates(this: *mut Pole, x: *mut i32, z: *mut i32) {
    log(Call::Coordinates(this as usize));
    unsafe {
        x.write(12);
        z.write(-30);
    }
}
extern "thiscall" fn distressed(this: *mut Pole) -> u32 {
    log(Call::Distressed(this as usize));
    unsafe { (*this).distressed }
}
extern "thiscall" fn set(this: *mut Pole, on: u32) {
    log(Call::Set(this as usize, on));
    unsafe {
        (*this).distressed = on;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(count) = s.change_on_set.take() {
            unsafe {
                PowerSim::from_ptr(s.sim).set_broken_count(count);
            }
        }
    });
}
struct World {
    object: Object,
    poles: Vec<Pole>,
    pointers: Vec<*mut u8>,
    _vt: Box<[usize; 13]>,
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
}
impl World {
    fn new(n: usize, broken: usize) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut vt = Box::new([0; 13]);
        vt[8] = coordinates as *const () as usize;
        vt[11] = distressed as *const () as usize;
        vt[12] = set as *const () as usize;
        let mut poles: Vec<_> = (0..n)
            .map(|i| Pole {
                vt: vt.as_ptr(),
                distressed: u32::from(i < broken),
            })
            .collect();
        let pointers: Vec<_> = poles.iter_mut().map(|p| (p as *mut Pole).cast()).collect();
        let mut w = Self {
            object: Object::new(0x400),
            poles,
            pointers,
            _vt: vt,
            _fakes: vec![
                fake(0x005F_4250, curve as *const () as usize),
                fake(0x004B_03E0, copy as *const () as usize),
                fake(0x006D_B010, shuffle as *const () as usize),
                fake(0x0090_CF63, free as *const () as usize),
            ],
            _fpu: GameFpu::new(),
        };
        let sim = w.sim();
        sim.set_pole_count(n as u32);
        sim.set_broken_count(broken as u32);
        sim.set_poles_begin(w.pointers.as_mut_ptr());
        sim.set_poles_end(w.pointers.as_mut_ptr().wrapping_add(n));
        STATE.with(|s| s.borrow_mut().sim = sim.as_ptr());
        w
    }
    fn sim(&mut self) -> PowerSim {
        unsafe { PowerSim::from_ptr(self.object.ptr()) }
    }
    fn apply(&mut self, funding: f32, delta: f32) -> u32 {
        STATE.with(|s| s.borrow_mut().delta = delta);
        unsafe { fc4p_006ec3f0(self.object.ptr(), &funding) }
    }
}

#[test]
fn pole_decay_shuffles_then_repairs_and_stops_at_target() {
    let mut w = World::new(100, 0);
    let sim = w.sim();
    assert_eq!(w.apply(0.0, -0.02), 0xFACE_4321);
    assert_eq!(sim.broken_count(), 2);
    assert_eq!(sim.broken_fraction(), 0.02);
    assert_eq!(w.poles.iter().filter(|p| p.distressed != 0).count(), 2);
    let c = calls();
    let copy = STATE.with(|s| s.borrow().copy.as_ptr() as usize);
    let p99 = w.pointers[99] as usize;
    let p98 = w.pointers[98] as usize;
    assert_eq!(
        c,
        [
            Call::Curve(sim.as_ptr() as usize + 0x3B8, 0),
            Call::Copy(sim.as_ptr() as usize + 0x29C),
            Call::Shuffle(copy, copy + 400),
            Call::Coordinates(p99),
            Call::Distressed(p99),
            Call::Set(p99, 1),
            Call::Coordinates(p98),
            Call::Distressed(p98),
            Call::Set(p98, 1),
            Call::Free(copy)
        ]
    );
    w.apply(1.0, 0.025);
    assert_eq!(sim.broken_count(), 0);
    assert_eq!(sim.broken_fraction().to_bits(), 0);
    assert!(w.poles.iter().all(|p| p.distressed == 0));
}

#[test]
fn saved_broken_count_is_repaired_from_a_fresh_fraction() {
    for (funding, delta, target) in [(1.0, 0.025, 0), (0.0, -0.02, 20)] {
        let mut w = World::new(1000, 300);
        w.apply(funding, delta);
        assert_eq!(w.sim().broken_count(), target);
        assert_eq!(w.poles.iter().filter(|p| p.distressed != 0).count(), target as usize);
    }
}

#[test]
fn target_equality_skips_copy_and_nan_remains_stored() {
    for (count, fraction, delta, target) in [
        (100, 0.0, -2.0, 100),
        (100, 0.5, 2.0, 0),
        (100, 0.5, f32::NAN, 0),
        (100, f32::NAN, 0.0, 0),
        (0x8000_0000, 0.5, 0.0, 0x4000_0000),
        (u32::MAX, 1.0, 0.0, 0),
        (10, -0.0, 0.0, 0),
    ] {
        let mut w = World::new(0, 0);
        let sim = w.sim();
        sim.set_pole_count(count);
        sim.set_broken_fraction(fraction);
        sim.set_broken_count(target);
        assert_eq!(w.apply(0.25, delta), target);
        assert_eq!(calls(), [Call::Curve(sim.as_ptr() as usize + 0x3B8, 25.0f32.to_bits())]);
        if fraction.is_nan() || delta.is_nan() {
            assert!(sim.broken_fraction().is_nan());
        }
        if fraction.to_bits() == 0x8000_0000 {
            assert_eq!(sim.broken_fraction().to_bits(), 0x8000_0000);
        }
    }
}

#[test]
fn empty_copy_and_unreachable_target_visit_no_extra_poles() {
    let mut empty = World::new(0, 0);
    empty.sim().set_pole_count(10);
    assert_eq!(empty.apply(0.0, -1.0), 0);
    assert!(matches!(
        calls().as_slice(),
        [Call::Curve(..), Call::Copy(..), Call::Shuffle(0, 0)]
    ));
    let mut w = World::new(2, 2);
    w.sim().set_broken_count(0);
    w.sim().set_pole_count(10);
    w.apply(0.0, -1.0);
    assert_eq!(w.sim().broken_count(), 0);
    assert_eq!(calls().iter().filter(|c| matches!(c, Call::Coordinates(..))).count(), 2);
}

#[test]
fn distress_uses_low_byte_and_count_is_reloaded_after_set() {
    let mut w = World::new(1, 0);
    w.poles[0].distressed = 0x100;
    w.sim().set_pole_count(10);
    STATE.with(|s| s.borrow_mut().change_on_set = Some(7));
    w.apply(0.0, -1.0);
    assert_eq!(w.sim().broken_count(), 8);
    assert!(calls().iter().any(|c| matches!(c, Call::Set(_, 1))));
}

extern "thiscall" fn factor(this: *mut u8, id: u32, context: u32) -> f32 {
    log(Call::Factor(this as usize, id, context));
    STATE.with(|s| s.borrow().factor)
}
extern "thiscall" fn funding(this: *mut u8) -> *const f32 {
    log(Call::Funding(this as usize));
    unsafe {
        let sim = PowerSim::from_ptr(this);
        assert_eq!((sim.recompute(), sim.age_plants()), (1, 1));
    }
    STATE.with(|s| &s.borrow().funding as *const f32)
}
extern "thiscall" fn apply(this: *mut u8, funding: *const f32) -> u32 {
    log(Call::Apply(this as usize, funding as usize));
    0xCDEF_0123
}

#[test]
fn month_refreshes_factor_rotates_ten_types_and_calls_funding_in_order() {
    let mut w = World::new(0, 0);
    let mut ovt = [0; 26];
    ovt[25] = factor as *const () as usize;
    let mut ordinance = ovt.as_ptr();
    let mut op = (&mut ordinance as *mut *const usize).cast::<u8>();
    let _o = fake_data(0x00B4_3D90, (&mut op as *mut *mut u8).cast());
    let mut zero = 0x8000_0000u32;
    let _z = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let _a = fake(0x006E_C3F0, apply as *const () as usize);
    let mut vt = [0; 49];
    vt[48] = funding as *const () as usize;
    let sim = w.sim();
    sim.set_primary_vtable(vt.as_ptr());
    for (index, next) in [(11, 0), (5, 6), (12, 0), (-1, 0), (i32::MAX, i32::MIN)] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.factor = 0.9;
            s.funding = 0.75;
        });
        sim.set_ring_index(index);
        sim.set_production(100);
        sim.set_consumption(350);
        sim.set_recompute(0);
        sim.set_age_plants(0);
        let ring = core::array::from_fn(|i| (i as i32).wrapping_mul(0x102_0304));
        let annual = core::array::from_fn(|i| i32::MAX - i as i32);
        sim.set_usage_ring(ring);
        sim.set_annual_usage(annual);
        assert_eq!(unsafe { fc4p_006ec570(sim.as_ptr()) }, 0xCDEF_0123);
        assert_eq!(sim.ring_index(), next);
        assert_eq!(sim.ordinance_factor(), 0.9);
        assert_eq!(sim.deficit(), (-2.0f32).to_bits());
        let mut expected = ring;
        let row = next.wrapping_mul(40) as u32 as usize / 40;
        for i in 0..10 {
            expected[row * 10 + i] = 0;
        }
        assert_eq!(sim.usage_ring(), expected);
        assert_eq!(
            sim.annual_usage(),
            core::array::from_fn(|i| annual[i].wrapping_sub(ring[row * 10 + i]))
        );
        let funding_ptr = STATE.with(|s| &s.borrow().funding as *const f32 as usize);
        assert_eq!(
            calls(),
            [
                Call::Factor(op as usize, 0x0911_E117, 0),
                Call::Funding(sim.as_ptr() as usize),
                Call::Apply(sim.as_ptr() as usize, funding_ptr)
            ]
        );
    }
    for (p, c, bits) in [
        (100, 101, 0),
        (100, 99, zero),
        (0, 1, zero),
        (-1, 100, zero),
        (1, i32::MAX, (-2_147_483_646.0f32).to_bits()),
    ] {
        sim.set_production(p);
        sim.set_consumption(c);
        unsafe {
            fc4p_006ec570(sim.as_ptr());
        }
        assert_eq!(sim.deficit(), bits);
    }
}
