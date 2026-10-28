// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Original-layout counterparts of the civic funding, efficiency, strike message, and ordinance cases.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Call(usize, u32, Vec<u32>);
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static STATE:RefCell<State>=RefCell::new(State::default());
}
#[derive(Default)]
struct State {
    message: *mut u8,
    clock: *mut u8,
    properties: *mut u8,
    department: *mut u8,
    property: *mut u8,
    values: *mut u8,
    groups: Vec<u32>,
    has: [bool; 2],
    local: [bool; 2],
    missing_department: Option<usize>,
    item_calls: usize,
    raw: [f32; 2],
    curve: f64,
    striking: u32,
    mutate_server: Option<(*mut *mut u8, *mut u8)>,
}
fn log(p: *mut u8, slot: u32, args: &[u32]) {
    CALLS.with(|c| c.borrow_mut().push(Call(p as usize, slot, args.to_vec())));
}
fn take() -> Vec<Call> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}
fn reset() {
    STATE.with(|s| {
        *s.borrow_mut() = State {
            has: [true; 2],
            local: [true; 2],
            raw: [0.5, 0.75],
            curve: 87.5,
            ..State::default()
        }
    });
    take();
}
unsafe fn table(p: *mut u8, t: &[usize]) {
    unsafe {
        p.cast::<*const usize>().write(t.as_ptr());
    }
}
fn event(p: *mut u8, slot: u32, args: &[u32]) -> Call {
    Call(p as usize, slot, args.to_vec())
}

extern "C" fn allocate(n: u32) -> *mut u8 {
    log(core::ptr::null_mut(), 0x009133DA, &[n]);
    STATE.with(|s| s.borrow().message)
}
extern "thiscall" fn construct(p: *mut u8) -> *mut u8 {
    log(p, 0x009134D6, &[]);
    p
}
extern "thiscall" fn retain(p: *mut u8) -> u32 {
    log(p, 4, &[]);
    0
}
extern "thiscall" fn release(p: *mut u8) -> u32 {
    log(p, 8, &[]);
    0xAABB_CCDD
}
extern "thiscall" fn message_type(p: *mut u8, n: u32) -> u32 {
    log(p, 0x14, &[n]);
    0
}
extern "thiscall" fn payload(p: *mut u8, n: u32) -> u32 {
    log(p, 0x30, &[n]);
    0
}
extern "thiscall" fn payload_two(p: *mut u8, n: u32) -> u32 {
    log(p, 0x84, &[n]);
    0
}
extern "thiscall" fn value(p: *mut u8, n: u32) -> u32 {
    log(p, 0x40, &[n]);
    STATE.with(|s| {
        if let Some((location, new)) = s.borrow_mut().mutate_server.take() {
            unsafe {
                *location = new;
            }
        }
    });
    0
}
extern "thiscall" fn days(p: *mut u8, n: u32) -> u32 {
    log(p, 0x54, &[n]);
    0
}
extern "thiscall" fn post(p: *mut u8, m: *mut u8, n: u32) -> u32 {
    log(p, 0x10, &[m as u32, n]);
    0
}
extern "thiscall" fn clock(p: *mut u8) -> *mut u8 {
    log(p, 0xC8, &[]);
    STATE.with(|s| s.borrow().clock)
}
extern "thiscall" fn calendar(p: *mut u8) -> *mut u8 {
    log(p, 0x1C, &[]);
    p
}
extern "thiscall" fn day(p: *mut u8) -> u32 {
    log(p, 0x68, &[]);
    0xFFFF_FFFE
}

#[test]
fn strike_start_and_end_keep_messages_order_state_and_partial_eax() {
    reset();
    let _a = fake(0x0091_33DA, allocate as *const () as usize);
    let _c = fake(0x0091_34D6, construct as *const () as usize);
    let mut msg_table = [0usize; 34];
    for (slot, f) in [
        (4, retain as *const () as usize),
        (8, release as *const () as usize),
        (0x14, message_type as *const () as usize),
        (0x30, payload as *const () as usize),
        (0x84, payload_two as *const () as usize),
        (0x40, value as *const () as usize),
        (0x54, days as *const () as usize),
    ] {
        msg_table[slot / 4] = f;
    }
    let mut server_table = [0usize; 5];
    server_table[4] = post as *const () as usize;
    let mut city_table = [0usize; 51];
    city_table[50] = clock as *const () as usize;
    let mut clock_table = [0usize; 27];
    clock_table[7] = calendar as *const () as usize;
    clock_table[26] = day as *const () as usize;
    let mut message = Object::new(0x2C);
    let mp = message.ptr();
    let mut server = Object::new(4);
    let sp = server.ptr();
    let mut city = Object::new(4);
    let cp = city.ptr();
    let mut clock_object = Object::new(4);
    let kp = clock_object.ptr();
    let mut sim = Object::new(0x400);
    let sim = sim.ptr();
    let mut department = Object::new(0x70);
    let dp = department.ptr();
    let mut global = sp;
    let _g = fake_data(0x00B4_3CCC, (&mut global as *mut *mut u8).cast());
    unsafe {
        table(mp, &msg_table);
        table(sp, &server_table);
        table(cp, &city_table);
        table(kp, &clock_table);
        CivicSim::from_ptr(sim).set_city(cp);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.message = mp;
        s.clock = kp;
    });
    let d = unsafe { Department::from_ptr(dp) };
    d.set_final_word(0x1234_5678);
    let occupant = 0xABCDEF10;
    let result = unsafe {
        call_thiscall_with_eax(
            fc4p_006f84c0 as *const () as usize,
            sim,
            &[dp as u32, occupant],
            0x11223344,
        )
    }
    .0;
    assert_eq!(result, 0xAABB_CC01);
    assert_eq!((d.strike(), d.strike_day()), (1, 0xFFFF_FFFE));
    assert_eq!(
        take(),
        [
            event(cp, 0xC8, &[]),
            event(kp, 0x1C, &[]),
            event(kp, 0x68, &[]),
            event(core::ptr::null_mut(), 0x009133DA, &[0x2C]),
            event(mp, 0x009134D6, &[]),
            event(mp, 4, &[]),
            event(mp, 0x14, &[0x12345678]),
            event(mp, 0x30, &[occupant]),
            event(mp, 0x84, &[occupant]),
            event(mp, 0x40, &[1]),
            event(sp, 0x10, &[mp as u32, 0]),
            event(mp, 8, &[])
        ]
    );
    assert_eq!(
        unsafe {
            call_thiscall_with_eax(
                fc4p_006f84c0 as *const () as usize,
                sim,
                &[dp as u32, occupant],
                0x11223344,
            )
        }
        .0,
        0x11223300
    );
    assert!(take().is_empty());
    STATE.with(|s| s.borrow_mut().mutate_server = Some((&mut global, core::ptr::null_mut())));
    assert_eq!(
        unsafe {
            call_thiscall_with_eax(
                fc4p_006f8570 as *const () as usize,
                sim,
                &[dp as u32, 0xFFFF_FFFD],
                0x66778899,
            )
        }
        .0,
        0xAABB_CC01
    );
    assert_eq!((d.strike(), d.strike_day()), (0, 0));
    assert!(global.is_null());
    assert_eq!(
        take(),
        [
            event(core::ptr::null_mut(), 0x009133DA, &[0x2C]),
            event(mp, 0x009134D6, &[]),
            event(mp, 4, &[]),
            event(mp, 0x14, &[0x12345678]),
            event(mp, 0x40, &[0]),
            event(mp, 0x54, &[0xFFFFFFFD]),
            event(sp, 0x10, &[mp as u32, 0]),
            event(mp, 8, &[])
        ]
    );
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006f8570 as *const () as usize, sim, &[dp as u32, 4], 0x66778899) }.0,
        0x66778800
    );
    assert!(take().is_empty());
}

#[allow(clippy::vec_box)] // Each node keeps its address while the owning vector moves.
fn effect_list(values: &[[f32; 3]]) -> (Vec<Box<[u32; 12]>>, *mut u8) {
    let mut nodes: Vec<_> = (0..=values.len()).map(|_| Box::new([0u32; 12])).collect();
    let pointers: Vec<_> = nodes.iter_mut().map(|p| p.as_mut_ptr().cast::<u8>()).collect();
    for i in 0..nodes.len() {
        nodes[i][0] = pointers[(i + 1) % nodes.len()] as u32;
    }
    for (n, v) in nodes[1..].iter_mut().zip(values) {
        n[2] = v[0].to_bits();
        n[8] = v[1].to_bits();
        n[7] = v[2].to_bits();
    }
    (nodes, pointers[0])
}
#[test]
fn ordinance_products_cover_empty_multiple_and_aliased_outputs() {
    let _fpu = GameFpu::new();
    for (values, expected) in [
        (vec![], [1.0, 1.0, 1.0]),
        (vec![[0.5, 2.0, 1.1], [2.0, 0.25, 0.5]], [1.0, 0.5, 0.55]),
    ] {
        let (_nodes, mut head) = effect_list(&values);
        let mut out = [-9f32; 3];
        assert_eq!(
            unsafe {
                fc4p_006f9340(
                    core::ptr::null_mut(),
                    &mut head,
                    out.as_mut_ptr(),
                    out.as_mut_ptr().add(1),
                    out.as_mut_ptr().add(2),
                )
            },
            head as u32
        );
        assert_eq!(out, expected);
    }
    let (_nodes, mut head) = effect_list(&[[2.0, 3.0, 5.0], [7.0, 11.0, 13.0]]);
    let mut out = 0f32;
    unsafe {
        fc4p_006f9340(core::ptr::null_mut(), &mut head, &mut out, &mut out, &mut out);
    }
    assert_eq!(out, 30030.0);
    let (_nodes, mut head) = effect_list(&[[f32::from_bits(0x7F800123), 1.0, 1.0]]);
    let mut out = [0f32; 3];
    unsafe {
        fc4p_006f9340(
            core::ptr::null_mut(),
            &mut head,
            out.as_mut_ptr(),
            out.as_mut_ptr().add(1),
            out.as_mut_ptr().add(2),
        );
    }
    assert_eq!(out[0].to_bits(), 0x7FC00123);
}

extern "thiscall" fn properties(p: *mut u8) -> *mut u8 {
    log(p, 0x18, &[]);
    STATE.with(|s| s.borrow().properties)
}
extern "thiscall" fn get_property(p: *mut u8, id: u32) -> *mut u8 {
    log(p, 0x24, &[id]);
    STATE.with(|s| s.borrow().property)
}
extern "thiscall" fn values(p: *mut u8) -> *mut u8 {
    log(p, 0x14, &[]);
    STATE.with(|s| s.borrow().values)
}
extern "thiscall" fn data(p: *mut u8) -> *const u32 {
    log(p, 0x1A4, &[]);
    STATE.with(|s| s.borrow().groups.as_ptr())
}
extern "thiscall" fn count(p: *mut u8) -> u32 {
    log(p, 0x10, &[]);
    STATE.with(|s| s.borrow().groups.len() as u32)
}

#[test]
fn ordinance_group_filter_calls_properties_even_for_empty_filter_and_searches_in_order() {
    reset();
    let mut ot = [0usize; 7];
    ot[6] = properties as *const () as usize;
    let mut pt = [0usize; 10];
    pt[9] = get_property as *const () as usize;
    let mut it = [0usize; 6];
    it[5] = values as *const () as usize;
    let mut vt = [0usize; 106];
    vt[0x1A4 / 4] = data as *const () as usize;
    vt[4] = count as *const () as usize;
    let mut o = ot.as_ptr();
    let op = (&mut o as *mut *const usize).cast::<u8>();
    let mut p = pt.as_ptr();
    let pp = (&mut p as *mut *const usize).cast::<u8>();
    let mut i = it.as_ptr();
    let ip = (&mut i as *mut *const usize).cast::<u8>();
    let mut v = vt.as_ptr();
    let vp = (&mut v as *mut *const usize).cast::<u8>();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.properties = pp;
        s.values = vp;
    });
    let mut effect = Object::new(0x40);
    let ep = effect.ptr();
    let (_nodes, head) = effect_list(&[]);
    unsafe { Effect::from_ptr(ep) }.set_groups(head);
    assert_eq!(unsafe { fc4p_006f94f0(op, ep, 77) }, (pp as u32 & !255) | 1);
    assert_eq!(take(), [event(op, 0x18, &[])]);
    let (mut nodes, head) = effect_list(&[[0.0; 3], [0.0; 3]]);
    nodes[1][2] = 20;
    nodes[2][2] = 40;
    unsafe { Effect::from_ptr(ep) }.set_groups(head);
    assert_eq!(unsafe { fc4p_006f94f0(op, ep, 77) }, 0);
    assert_eq!(take(), [event(op, 0x18, &[]), event(pp, 0x24, &[77])]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.property = ip;
        s.groups = vec![10, 40, 50];
    });
    assert_eq!(
        unsafe { fc4p_006f94f0(op, ep, 77) },
        (nodes[2].as_ptr() as u32 & !255) | 1
    );
    assert_eq!(
        take(),
        [
            event(op, 0x18, &[]),
            event(pp, 0x24, &[77]),
            event(ip, 0x14, &[]),
            event(vp, 0x1A4, &[]),
            event(vp, 0x10, &[]),
            event(vp, 0x10, &[])
        ]
    );
    STATE.with(|s| s.borrow_mut().groups = vec![10, 30]);
    assert_eq!(unsafe { fc4p_006f94f0(op, ep, 77) } as u8, 0);
    assert_eq!(take().len(), 7);
}

extern "thiscall" fn item(p: *mut u8, properties: *mut u8, purpose: u32, out: *mut BudgetItem) -> u32 {
    log(p, 0xCC, &[properties as u32, purpose]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.item_calls % 2;
        s.item_calls += 1;
        unsafe {
            (*out).words[0].write(0x100 + i as u32);
            (*out).words[7].write(s.raw[i].to_bits());
        }
        if s.has[i] {
            0xABCDEE01
        } else {
            0xABCDEE00
        }
    })
}
extern "thiscall" fn department(p: *mut u8, id: u32) -> *mut u8 {
    log(p, 0x70, &[id]);
    STATE.with(|s| {
        let s = s.borrow();
        if s.missing_department == Some((id - 0x100) as usize) {
            core::ptr::null_mut()
        } else {
            s.department
        }
    })
}
extern "thiscall" fn local(p: *mut u8, properties: *mut u8, purpose: u32) -> u32 {
    log(p, 0x70, &[properties as u32, purpose]);
    STATE.with(|s| {
        let s = s.borrow();
        u32::from(s.local[usize::from(purpose == 0x4A5654BA || purpose == 0xEA56549E)])
    })
}
extern "thiscall" fn funding(p: *mut u8) -> *const u32 {
    log(p, 0x28, &[]);
    unsafe { p.add(4).cast() }
}
extern "thiscall" fn raw(p: *mut u8, o: *mut u8, a: *mut f32, b: *mut f32, school: u32) -> u32 {
    log(p, 0x50, &[o as u32, school]);
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            *a = s.raw[0];
            *b = s.raw[1];
        }
    });
    0
}
extern "thiscall" fn curve(p: *mut u8, x: f32) -> f64 {
    log(p, 0x005F4250, &[x.to_bits()]);
    STATE.with(|s| s.borrow().curve)
}
extern "thiscall" fn on_strike(p: *mut u8, o: *mut u8) -> u32 {
    log(p, 0x006F6F30, &[o as u32]);
    STATE.with(|s| s.borrow().striking)
}

struct FundingWorld {
    sim: Object,
    occupant: Object,
    budget: Object,
    department: Object,
    st: Box<[usize; 21]>,
    ot: Box<[usize; 7]>,
    bt: Box<[usize; 52]>,
    dt: Box<[usize; 29]>,
    default: Box<u32>,
    hundred: Box<f32>,
    percent: Box<f32>,
    guards: Vec<Fake>,
}
impl FundingWorld {
    fn new() -> Self {
        reset();
        let mut w = Self {
            sim: Object::new(0x400),
            occupant: Object::new(4),
            budget: Object::new(4),
            department: Object::new(8),
            st: Box::new([0; 21]),
            ot: Box::new([0; 7]),
            bt: Box::new([0; 52]),
            dt: Box::new([0; 29]),
            default: Box::new(1f32.to_bits()),
            hundred: Box::new(100.0),
            percent: Box::new(0.01),
            guards: vec![],
        };
        w.st[20] = raw as *const () as usize;
        w.ot[6] = properties as *const () as usize;
        w.bt[0xCC / 4] = item as *const () as usize;
        w.bt[0x70 / 4] = department as *const () as usize;
        w.dt[0x70 / 4] = local as *const () as usize;
        w.dt[0x28 / 4] = funding as *const () as usize;
        unsafe {
            table(w.sim.ptr(), &*w.st);
            table(w.occupant.ptr(), &*w.ot);
            table(w.budget.ptr(), &*w.bt);
            table(w.department.ptr(), &*w.dt);
            w.department.ptr().add(4).cast::<f32>().write(0.625);
            CivicSim::from_ptr(w.sim.ptr()).set_budget(w.budget.ptr());
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.department = w.department.ptr();
            s.properties = 0xFEDCBA98usize as *mut u8;
        });
        w.guards
            .push(fake_data(0x00B0D440, (&mut *w.default as *mut u32).cast()));
        w.guards
            .push(fake_data(0x00A80AA0, (&mut *w.hundred as *mut f32).cast()));
        w.guards
            .push(fake_data(0x00A823A4, (&mut *w.percent as *mut f32).cast()));
        w.guards.push(fake(0x005F4250, curve as *const () as usize));
        w.guards.push(fake(0x006F6F30, on_strike as *const () as usize));
        w
    }
    fn raw(&mut self, school: u32) -> ([f32; 2], u32) {
        let mut out = [-7f32, -8f32];
        let result = unsafe {
            fc4p_006f8360(
                self.sim.ptr(),
                self.occupant.ptr(),
                out.as_mut_ptr().cast(),
                out.as_mut_ptr().add(1).cast(),
                school,
            )
        };
        (out, result)
    }
}
#[test]
fn funding_covers_local_global_missing_items_and_null_departments() {
    for school in [0, 0x100, 1, 0x100FF] {
        let mut w = FundingWorld::new();
        let (out, _) = w.raw(school);
        assert_eq!(out, [0.5, 0.75]);
        let purpose = if school as u8 == 0 {
            [0xCA565486, 0xEA56549E]
        } else {
            [0xEA5654B6, 0x4A5654BA]
        };
        assert_eq!(
            take(),
            [
                event(w.occupant.ptr(), 0x18, &[]),
                event(w.budget.ptr(), 0xCC, &[0xFEDCBA98, purpose[0]]),
                event(w.budget.ptr(), 0x70, &[0x100]),
                event(w.department.ptr(), 0x70, &[0xFEDCBA98, purpose[0]]),
                event(w.budget.ptr(), 0xCC, &[0xFEDCBA98, purpose[1]]),
                event(w.budget.ptr(), 0x70, &[0x101]),
                event(w.department.ptr(), 0x70, &[0xFEDCBA98, purpose[1]])
            ]
        );
    }
    for (has, missing, local, expected, calls) in [
        ([false, true], None, [true, true], [1.0, 1.0], 2),
        ([true, false], None, [true, true], [0.5, 1.0], 5),
        ([true, true], Some(0), [true, true], [-7.0, -8.0], 3),
        ([true, true], Some(1), [true, true], [0.5, -8.0], 6),
        ([true, true], None, [false, false], [0.625, 0.625], 9),
    ] {
        let mut w = FundingWorld::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.has = has;
            s.missing_department = missing;
            s.local = local;
        });
        let (out, result) = w.raw(1);
        assert_eq!(out, expected);
        assert_eq!(take().len(), calls);
        if missing.is_some() {
            assert_eq!(result, 0);
        } else if !has[1] {
            assert_eq!(result, 1f32.to_bits());
        } else if !local[1] {
            assert_eq!(result, 0.625f32.to_bits());
        }
    }
}
#[test]
fn efficiencies_cover_staff_item_absence_strikes_ignored_strikes_and_curve_order() {
    let _fpu = GameFpu::new();
    for school in [0u32, 1] {
        for (has, strike, ignore, expected) in [
            (false, 0, 0, [1.0, 1.0]),
            (true, 0, 0, [0.875, 0.875]),
            (true, 1, 0, [0.125, 0.875]),
            (true, 1, 1, [0.875, 0.875]),
            (true, 1, 0x100, [0.125, 0.875]),
        ] {
            let mut w = FundingWorld::new();
            let sp = w.sim.ptr();
            let op = w.occupant.ptr();
            let sim = unsafe { CivicSim::from_ptr(sp) };
            sim.set_health_strike_efficiency(0.25);
            sim.set_school_strike_efficiency(0.25);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.has = [has; 2];
                s.striking = strike;
            });
            let mut out = [-1f32; 2];
            unsafe {
                fc4p_006f8710(sp, op, out.as_mut_ptr(), out.as_mut_ptr().add(1), school, ignore);
            }
            assert_eq!(out, expected);
            let purpose = if school == 0 { 0xCA565486 } else { 0xEA5654B6 };
            let offset = if school == 0 { 0xE8 } else { 0x148 };
            let mut expected_calls = vec![
                event(sp, 0x50, &[op as u32, school]),
                event(op, 0x18, &[]),
                event(w.budget.ptr(), 0xCC, &[0xFEDCBA98, purpose]),
            ];
            if has {
                if ignore as u8 == 0 {
                    expected_calls.push(event(sp, 0x006F6F30, &[op as u32]));
                }
                if strike == 0 || ignore as u8 != 0 {
                    expected_calls.push(event(unsafe { sp.add(offset) }, 0x005F4250, &[50f32.to_bits()]));
                }
                expected_calls.push(event(unsafe { sp.add(offset + 0x20) }, 0x005F4250, &[75f32.to_bits()]));
            }
            assert_eq!(take(), expected_calls);
        }
        let mut w = FundingWorld::new();
        let mut out = -1f32;
        let sp = w.sim.ptr();
        let op = w.occupant.ptr();
        assert_eq!(
            unsafe { fc4p_006f85f0(sp, &mut out, op, school) },
            &out as *const f32 as u32
        );
        assert_eq!(out, 0.875);
        assert_eq!(take().len(), 4);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.has = [false; 2];
            s.raw[0] = f32::from_bits(0x7F800123);
        });
        unsafe {
            fc4p_006f85f0(sp, &mut out, op, school);
        }
        assert_eq!(out.to_bits(), 0x7F800123);
        assert_eq!(take().len(), 3);
    }
}
#[test]
fn base_efficiency_retains_unrounded_curve_result_and_the_x87_environment() {
    let _fpu = GameFpu::new();
    let mut w = FundingWorld::new();
    STATE.with(|s| s.borrow_mut().curve = 100.000_002_5);
    let mut out = 0f32;
    unsafe {
        fc4p_006f85f0(w.sim.ptr(), &mut out, w.occupant.ptr(), 1);
    }
    assert_eq!(out.to_bits(), 1f32.to_bits());
    assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
    assert_eq!(take().len(), 4);
}
