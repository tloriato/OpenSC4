// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Member(usize, u32),
    Properties(usize),
    Read(usize, u32),
    Curve(usize, u32),
    Status(usize, bool),
    List(usize, usize, usize, u32, u32),
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};
    static SWAP:RefCell<Option<(*mut u8,*mut *mut u8,usize)>>=const{RefCell::new(None)};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}

#[repr(C)]
struct Building {
    table: *const usize,
    member: u32,
    grade: f32,
    present: bool,
}
extern "thiscall" fn member(p: *mut Building, group: u32) -> u32 {
    log(Call::Member(p as usize, group));
    unsafe { (*p).member }
}
extern "thiscall" fn properties(p: *mut Building) -> *mut u8 {
    log(Call::Properties(p as usize));
    p.cast()
}
extern "C" fn read(p: *mut Building, id: u32, out: *mut f32) -> u32 {
    log(Call::Read(p as usize, id));
    unsafe {
        if (*p).present {
            *out = (*p).grade;
            1
        } else {
            0
        }
    }
}
extern "thiscall" fn curve(p: *mut u8, x: f32) -> f64 {
    log(Call::Curve(p as usize, x.to_bits()));
    50.0
}
extern "thiscall" fn school_status(p: *mut u8) -> u64 {
    log(Call::Status(p as usize, true));
    (0x12345678u64 << 32) | unsafe { u64::from(*p.add(0x178)) }
}
extern "thiscall" fn health_status(p: *mut u8) -> u64 {
    log(Call::Status(p as usize, false));
    (0x12345678u64 << 32) | unsafe { u64::from(*p.add(0x118)) }
}

struct Constants {
    values: Box<[u32; 4]>,
    _guards: Vec<Fake>,
}
impl Constants {
    fn new() -> Self {
        take();
        let mut values = Box::new([0, 1f32.to_bits(), 100f32.to_bits(), 0.01f32.to_bits()]);
        let mut guards = vec![
            fake(0x005FD4F0, read as *const () as usize),
            fake(0x005F4250, curve as *const () as usize),
        ];
        for (i, address) in [0x00B4AF90, 0x00B0D440, 0x00A80AA0, 0x00A823A4].into_iter().enumerate() {
            guards.push(fake_data(address, unsafe { values.as_mut_ptr().add(i).cast() }));
        }
        Self {
            values,
            _guards: guards,
        }
    }
}
fn list(buildings: &mut [Building]) -> Vec<[u32; 3]> {
    let mut nodes = vec![[0; 3]; buildings.len() + 1];
    let len = nodes.len();
    let ptr = nodes.as_mut_ptr();
    for (i, node) in nodes.iter_mut().enumerate() {
        node[0] = unsafe { ptr.add((i + 1) % len) } as u32;
        if i > 0 {
            node[2] = &mut buildings[i - 1] as *mut Building as u32;
        }
    }
    nodes
}
#[test]
fn list_chance_reuses_previous_chance_and_missing_grade_and_keeps_call_order() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let mut vt = [0usize; 23];
    vt[6] = properties as *const () as usize;
    vt[22] = member as *const () as usize;
    let mut buildings = [
        Building {
            table: vt.as_ptr(),
            member: 0,
            grade: 0.1,
            present: true,
        },
        Building {
            table: vt.as_ptr(),
            member: 1,
            grade: 0.4,
            present: true,
        },
        Building {
            table: vt.as_ptr(),
            member: 0,
            grade: 0.9,
            present: true,
        },
        Building {
            table: vt.as_ptr(),
            member: 1,
            grade: 0.0,
            present: false,
        },
    ];
    let mut nodes = list(&mut buildings);
    let mut head = nodes.as_mut_ptr().cast::<u8>();
    let mut block = Object::new(0x60);
    let bp = block.ptr();
    let mut out = -1f32;
    assert_eq!(
        unsafe { fc4p_006f9390(&mut out, bp, &mut head, 0x292453C5, 0x1A03) },
        &out as *const f32 as u32
    );
    assert_eq!(out, 0.875);
    let b: Vec<_> = buildings.iter().map(|b| b as *const Building as usize).collect();
    let cp = unsafe { bp.add(0x24) } as usize;
    assert_eq!(
        take(),
        [
            Call::Member(b[0], 0x1A03),
            Call::Member(b[1], 0x1A03),
            Call::Properties(b[1]),
            Call::Read(b[1], 0x292453C5),
            Call::Curve(cp, 40f32.to_bits()),
            Call::Member(b[2], 0x1A03),
            Call::Member(b[3], 0x1A03),
            Call::Properties(b[3]),
            Call::Read(b[3], 0x292453C5),
            Call::Curve(cp, 40f32.to_bits())
        ]
    );
    unsafe { Department::from_ptr(bp) }.set_strike(255);
    unsafe {
        fc4p_006f9390(&mut out, bp, core::ptr::null_mut(), 7, 8);
    }
    assert_eq!(out, 1.0);
    assert!(take().is_empty());
    unsafe { Department::from_ptr(bp) }.set_strike(0);
    let mut empty = list(&mut []);
    let mut head = empty.as_mut_ptr().cast();
    unsafe {
        fc4p_006f9390(&mut out, bp, &mut head, 7, 8);
    }
    assert_eq!(out, 0.0);
    assert!(take().is_empty());
}

extern "thiscall" fn list_probability(
    sim: *mut u8,
    out: *mut f32,
    block: *mut u8,
    list: *mut *mut u8,
    grade: u32,
    group: u32,
) -> u32 {
    log(Call::List(sim as usize, block as usize, list as usize, grade, group));
    unsafe {
        *out = 0.5;
    }
    SWAP.with(|s| {
        if let Some((sim, array, n)) = s.borrow_mut().take() {
            unsafe {
                let sim = StrikeSim::from_ptr(sim);
                sim.set_schools(array);
                sim.set_schools_end(array.add(n));
            }
        }
    });
    0
}
#[test]
fn department_probabilities_cover_active_empty_multiple_groups_and_return_registers() {
    let _fpu = GameFpu::new();
    let mut constants = Constants::new();
    let _f = fake(0x006F9390, list_probability as *const () as usize);
    let mut vt = [0usize; 8];
    vt[6] = school_status as *const () as usize;
    vt[7] = health_status as *const () as usize;
    let mut mem = Object::new(0x400);
    let p = mem.ptr();
    unsafe {
        p.cast::<*const usize>().write(vt.as_ptr());
    }
    let sim = unsafe { StrikeSim::from_ptr(p) };
    let mut arrays = [core::ptr::null_mut(); 2];
    let ap = arrays.as_mut_ptr();
    for school in [true, false] {
        if school {
            sim.set_schools(ap);
            sim.set_schools_end(unsafe { ap.add(2) });
        } else {
            sim.set_hospitals(ap);
            sim.set_hospitals_end(unsafe { ap.add(2) });
        }
        let function = if school { fc4p_006f9670 } else { fc4p_006f9730 };
        let mut out = -1f32;
        let result = unsafe { function(p, &mut out) };
        assert_eq!(out, 0.75);
        assert_eq!(result, (2u64 << 32) | u64::from(&out as *const f32 as u32));
        let (offset, grade, group) = if school {
            (0x134, 0x292453C5, 0x1A03)
        } else {
            (0xD4, 0x092453CB, 0x1A02)
        };
        assert_eq!(
            take(),
            [
                Call::Status(p as usize, school),
                Call::List(p as usize, unsafe { p.add(offset) } as usize, ap as usize, grade, group),
                Call::List(
                    p as usize,
                    unsafe { p.add(offset) } as usize,
                    unsafe { ap.add(1) } as usize,
                    grade,
                    group
                )
            ]
        );
        unsafe {
            *p.add(if school { 0x178 } else { 0x118 }) = 0xFF;
        }
        let result = unsafe { function(p, &mut out) };
        assert_eq!(out, 1.0);
        assert_eq!(result >> 32, 0x12345678);
        assert_eq!(take(), [Call::Status(p as usize, school)]);
        unsafe {
            *p.add(if school { 0x178 } else { 0x118 }) = 0;
        }
        if school {
            sim.set_schools_end(ap);
        } else {
            sim.set_hospitals_end(ap);
        }
        constants.values[0] = 0x80000000;
        let result = unsafe { function(p, &mut out) };
        assert_eq!(out.to_bits(), 0x80000000);
        assert_eq!(result >> 32, 0x12345678);
        assert_eq!(take(), [Call::Status(p as usize, school)]);
        constants.values[0] = 0;
    }
    sim.set_schools(ap);
    sim.set_schools_end(unsafe { ap.add(1) });
    let mut replacement = [core::ptr::null_mut(); 2];
    let rp = replacement.as_mut_ptr();
    SWAP.with(|s| *s.borrow_mut() = Some((p, rp, 2)));
    let mut out = 0f32;
    unsafe {
        fc4p_006f9670(p, &mut out);
    }
    assert_eq!(out, 0.75);
    let calls = take();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[2],
        Call::List(
            p as usize,
            unsafe { p.add(0x134) } as usize,
            unsafe { rp.add(1) } as usize,
            0x292453C5,
            0x1A03
        )
    );
}

#[derive(Debug, PartialEq, Eq)]
enum TransitionCall {
    Clock(usize),
    Calendar(usize),
    Day(usize),
    Properties(usize),
    UInt(usize, u32),
    Float(usize, u32),
    Curve(usize, u32),
    Random(usize, u64, u64),
    Write(usize, u32, u32),
    Controller(usize),
    String(usize),
    Add(usize, usize, u32),
    Remove(usize, usize),
    Free(usize),
}
#[derive(Default)]
struct TransitionState {
    clock: *mut u8,
    day: u32,
    old: u32,
    value: f32,
    present: bool,
    curve: f64,
    draw: f64,
    controller: *mut u8,
    buffer: *mut u8,
}
thread_local! {static TCALLS:RefCell<Vec<TransitionCall>>=const{RefCell::new(vec![])};static TRANSITION:RefCell<TransitionState>=RefCell::new(TransitionState::default());}
fn tlog(c: TransitionCall) {
    TCALLS.with(|v| v.borrow_mut().push(c));
}
fn ttake() -> Vec<TransitionCall> {
    TCALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn tclock(p: *mut u8) -> *mut u8 {
    tlog(TransitionCall::Clock(p as usize));
    TRANSITION.with(|s| s.borrow().clock)
}
extern "thiscall" fn tcalendar(p: *mut u8) -> *mut u8 {
    tlog(TransitionCall::Calendar(p as usize));
    p
}
extern "thiscall" fn tday(p: *mut u8) -> u32 {
    tlog(TransitionCall::Day(p as usize));
    TRANSITION.with(|s| s.borrow().day)
}
extern "thiscall" fn tproperties(p: *mut u8) -> *mut u8 {
    tlog(TransitionCall::Properties(p as usize));
    p
}
extern "C" fn tuint(p: *mut u8, id: u32, out: *mut u32) -> u32 {
    tlog(TransitionCall::UInt(p as usize, id));
    unsafe {
        *out = TRANSITION.with(|s| s.borrow().old);
    }
    1
}
extern "C" fn tfloat(p: *mut u8, id: u32, out: *mut f32) -> u32 {
    tlog(TransitionCall::Float(p as usize, id));
    TRANSITION.with(|s| {
        let s = s.borrow();
        if s.present {
            unsafe {
                *out = s.value;
            }
            1
        } else {
            0
        }
    })
}
extern "thiscall" fn tcurve(p: *mut u8, x: f32) -> f64 {
    tlog(TransitionCall::Curve(p as usize, x.to_bits()));
    TRANSITION.with(|s| s.borrow().curve)
}
extern "thiscall" fn trandom(p: *mut u8, min: f64, max: f64) -> f64 {
    tlog(TransitionCall::Random(p as usize, min.to_bits(), max.to_bits()));
    TRANSITION.with(|s| s.borrow().draw)
}
extern "C" fn twrite(p: *mut u8, id: u32, value: u32) -> u32 {
    tlog(TransitionCall::Write(p as usize, id, value));
    1
}
extern "thiscall" fn tcontroller(p: *mut u8) -> *mut u8 {
    tlog(TransitionCall::Controller(p as usize));
    TRANSITION.with(|s| s.borrow().controller)
}
extern "thiscall" fn tstring(p: *mut u8, text: *const u8) -> *mut u8 {
    tlog(TransitionCall::String(text as usize));
    unsafe {
        p.add(4).cast::<*mut u8>().write(TRANSITION.with(|s| s.borrow().buffer));
    }
    p
}
extern "thiscall" fn tadd(p: *mut u8, _name: *mut u8, building: *mut u8, zero: u32) -> u32 {
    tlog(TransitionCall::Add(p as usize, building as usize, zero));
    0
}
extern "thiscall" fn tremove(p: *mut u8, _name: *mut u8, building: *mut u8) -> u32 {
    tlog(TransitionCall::Remove(p as usize, building as usize));
    0
}
extern "C" fn tfree(p: *mut u8) -> u32 {
    tlog(TransitionCall::Free(p as usize));
    0
}

#[test]
fn strike_transition_covers_roll_boundaries_unsigned_duration_staff_threshold_nan_and_controller_order() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let mut min = 0f64;
    let mut max = 100f64;
    let mut health = 0x1111usize as *const u8;
    let mut school = 0x2222usize as *const u8;
    let mut city_vt = [0usize; 51];
    city_vt[50] = tclock as *const () as usize;
    let mut clock_vt = [0usize; 27];
    clock_vt[7] = tcalendar as *const () as usize;
    clock_vt[26] = tday as *const () as usize;
    let mut building_vt = [0usize; 7];
    building_vt[6] = tproperties as *const () as usize;
    let mut simulator_vt = [0usize; 48];
    simulator_vt[47] = tcontroller as *const () as usize;
    let mut controller_vt = [0usize; 13];
    controller_vt[10] = tadd as *const () as usize;
    controller_vt[12] = tremove as *const () as usize;
    let mut city = city_vt.as_ptr();
    let cp = (&mut city as *mut *const usize).cast::<u8>();
    let mut clock = clock_vt.as_ptr();
    let kp = (&mut clock as *mut *const usize).cast::<u8>();
    let mut building = building_vt.as_ptr();
    let bp = (&mut building as *mut *const usize).cast::<u8>();
    let mut simulator = simulator_vt.as_ptr();
    let sp = (&mut simulator as *mut *const usize).cast::<u8>();
    let mut controller = controller_vt.as_ptr();
    let ap = (&mut controller as *mut *const usize).cast::<u8>();
    let mut global = sp;
    let _min = fake_data(0x00A80990, (&mut min as *mut f64).cast());
    let _max = fake_data(0x00A80AC0, (&mut max as *mut f64).cast());
    let _health = fake_data(0x00B0D438, (&mut health as *mut *const u8).cast());
    let _school = fake_data(0x00B0D43C, (&mut school as *mut *const u8).cast());
    let _global = fake_data(0x00B43CEC, (&mut global as *mut *mut u8).cast());
    let _fakes: Vec<_> = [
        (0x005FD480, tuint as *const () as usize),
        (0x005FD4F0, tfloat as *const () as usize),
        (0x005F4250, tcurve as *const () as usize),
        (0x00913761, trandom as *const () as usize),
        (0x005FE3C0, twrite as *const () as usize),
        (0x00408480, tstring as *const () as usize),
        (0x0090CF63, tfree as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect();
    let mut sim = Object::new(0x400);
    let p = sim.ptr();
    unsafe { TransitionSim::from_ptr(p) }.set_city(cp);
    let mut department = Object::new(0x60);
    let dp = department.ptr();
    let d = unsafe { Department::from_ptr(dp) };
    d.set_count(50);
    d.set_fourth(1f32.to_bits());
    for (old, today, value, present, draw, chance, changed) in [
        (0, 100, 0.4, true, 49.9, 50.0, true),
        (0, 100, 0.4, true, 50.0, 50.0, false),
        (0, 100, 0.4, true, f64::NAN, 50.0, false),
        (0, 100, 0.4, true, 0.0, f64::NAN, false),
        (50, 100, 0.9, true, 0.0, 50.0, false),
        (50, 101, 0.9, true, 0.0, 50.0, true),
        (50, 100, 1.0, true, 0.0, 50.0, true),
        (50, 100, f32::NAN, true, 0.0, 50.0, false),
        (u32::MAX, 1, 0.1, true, 0.0, 50.0, false),
        (50, 100, 0.0, false, 0.0, 50.0, true),
        (0, 0, 0.4, true, 0.0, 50.0, false),
    ] {
        ttake();
        TRANSITION.with(|s| {
            *s.borrow_mut() = TransitionState {
                clock: kp,
                day: today,
                old,
                value,
                present,
                curve: chance,
                draw,
                controller: ap,
                buffer: 0x9999usize as *mut u8,
            }
        });
        d.set_final_word(if old == 0 { 0x077ACCD2 } else { 0x12345678 });
        assert_eq!(unsafe { fc4p_006fa280(p, bp, dp, 77, 88) } as u8, u8::from(changed));
        let mut expected = vec![
            TransitionCall::Clock(cp as usize),
            TransitionCall::Calendar(kp as usize),
            TransitionCall::Day(kp as usize),
            TransitionCall::Properties(bp as usize),
            TransitionCall::UInt(bp as usize, 0xEAA69F83),
            TransitionCall::Properties(bp as usize),
            TransitionCall::Float(bp as usize, if old == 0 { 88 } else { 77 }),
        ];
        if old == 0 {
            expected.push(TransitionCall::Curve(
                unsafe { dp.add(0x24) } as usize,
                (value * 100.0).to_bits(),
            ));
            expected.push(TransitionCall::Random(
                unsafe { p.add(0xB8) } as usize,
                0f64.to_bits(),
                100f64.to_bits(),
            ));
        }
        if changed {
            expected.extend([
                TransitionCall::Properties(bp as usize),
                TransitionCall::Write(bp as usize, 0xEAA69F83, if old == 0 { today } else { 0 }),
                TransitionCall::Controller(sp as usize),
                TransitionCall::String(if old == 0 { 0x1111 } else { 0x2222 }),
            ]);
            expected.push(if old == 0 {
                TransitionCall::Add(ap as usize, bp as usize, 0)
            } else {
                TransitionCall::Remove(ap as usize, bp as usize)
            });
            expected.push(TransitionCall::Free(0x9999));
        }
        assert_eq!(ttake(), expected);
    }
}
