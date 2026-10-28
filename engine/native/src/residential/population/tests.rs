// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Sample(usize, u32, u32, u32),
    Divide(i64, i64),
    History(usize, u32, u32),
    Curve(usize, u32),
    Allocate(u32),
    Construct(usize),
    Message(usize, u32, u32),
    Post(usize, usize),
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};
    static CONTEXT:RefCell<(usize,*mut u8)>=const{RefCell::new((0,core::ptr::null_mut()))};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn sample(p: *mut u8, wealth: u32, x: u32, z: u32) -> u32 {
    log(Call::Sample(p as usize, wealth, x, z));
    0xAABB0000 | x.wrapping_add(z)
}
#[test]
fn square_average_uses_low_words_unsigned_bounds_remainder_and_preserves_ecx() {
    let mut vt = [0usize; 7];
    vt[6] = sample as *const () as usize;
    let mut simulator = vt.as_ptr();
    let p = (&mut simulator as *mut *const usize).cast::<u8>();
    let mut global = p;
    let _g = fake_data(0x00B43D9C, (&mut global as *mut *mut u8).cast());
    let (eax, edx, ecx): (u32, u32, u32);
    unsafe {
        core::arch::asm!("push 2","push 7","push 3","push 1","call {entry}",entry=sym fc4p_006f7ff0,inlateout("ecx") 0x12345678u32=>ecx,lateout("eax") eax,lateout("edx") edx,clobber_abi("C"));
    }
    assert_eq!((eax, edx, ecx), (11, 0, 0x12345678));
    assert_eq!(
        take(),
        [
            Call::Sample(p as usize, 1, 3, 7),
            Call::Sample(p as usize, 1, 3, 8),
            Call::Sample(p as usize, 1, 4, 7),
            Call::Sample(p as usize, 1, 4, 8)
        ]
    );
    assert_eq!(unsafe { average_population(2, u32::MAX, 0, 2) }, 0);
    assert!(take().is_empty());
    assert_eq!(unsafe { average_population(2, 2, 0, u32::MAX) }, 0);
    assert!(take().is_empty());
}
extern "stdcall" fn divide(n: i64, d: i64) -> i64 {
    log(Call::Divide(n, d));
    n / d
}
extern "thiscall" fn history(p: *mut u8, id: u32, n: u32) -> u32 {
    log(Call::History(p as usize, id, n));
    0
}
extern "thiscall" fn curve(p: *mut u8, x: f32) -> f64 {
    log(Call::Curve(p as usize, x.to_bits()));
    CONTEXT.with(|c| {
        let base = c.borrow().0;
        let group = (p as usize - base - 0x264) / 16;
        f64::from([6, 24, 27, 20, 12, 7, 4, 0, 0][group])
    })
}
extern "C" fn allocate(n: u32) -> *mut u8 {
    log(Call::Allocate(n));
    CONTEXT.with(|c| c.borrow().1)
}
extern "thiscall" fn construct(p: *mut u8) -> *mut u8 {
    log(Call::Construct(p as usize));
    p
}
extern "thiscall" fn retain(p: *mut u8) -> u32 {
    log(Call::Message(p as usize, 4, 0));
    1
}
extern "thiscall" fn release(p: *mut u8) -> u32 {
    log(Call::Message(p as usize, 8, 0));
    0
}
extern "thiscall" fn id(p: *mut u8, n: u32) -> u32 {
    log(Call::Message(p as usize, 0x2C, n));
    0
}
extern "thiscall" fn message_type(p: *mut u8, n: u32) -> u32 {
    log(Call::Message(p as usize, 0x14, n));
    0
}
extern "thiscall" fn value(p: *mut u8, n: u32) -> u32 {
    log(Call::Message(p as usize, 0x40, n));
    0
}
extern "thiscall" fn post(p: *mut u8, message: *mut u8) -> u32 {
    log(Call::Post(p as usize, message as usize));
    0
}
#[test]
fn population_publication_matches_age_group_counts_eqs_and_life_history_order() {
    let _fpu = GameFpu::new();
    take();
    let _d = fake(0x009F0710, divide as *const () as usize);
    let _h = fake(0x006F7D00, history as *const () as usize);
    let _c = fake(0x005F4250, curve as *const () as usize);
    let _a = fake(0x009133DA, allocate as *const () as usize);
    let _m = fake(0x009134D6, construct as *const () as usize);
    let mut bias = 4294967296f32;
    let mut percent = 0.01f32;
    let _b = fake_data(0x00A80AA8, (&mut bias as *mut f32).cast());
    let _p = fake_data(0x00A823A4, (&mut percent as *mut f32).cast());
    let mut sim = Object::new(0x400);
    let p = sim.ptr();
    let view = unsafe { ResidentialSim::from_ptr(p) };
    view.set_running_population(400);
    view.set_life_sum_low(28600);
    let mut counts = [0i32; 20];
    counts[3] = 200;
    view.set_age_count(counts);
    let mut eq = [0i32; 20];
    eq[3] = 10000;
    view.set_age_eq_sum(eq);
    let mut mt = [0usize; 17];
    mt[1] = retain as *const () as usize;
    mt[2] = release as *const () as usize;
    mt[5] = message_type as *const () as usize;
    mt[11] = id as *const () as usize;
    mt[16] = value as *const () as usize;
    let mut st = [0usize; 4];
    st[3] = post as *const () as usize;
    let mut message = mt.as_ptr();
    let mp = (&mut message as *mut *const usize).cast::<u8>();
    let mut server = st.as_ptr();
    let sp = (&mut server as *mut *const usize).cast::<u8>();
    view.set_message_server(sp);
    CONTEXT.with(|c| *c.borrow_mut() = (p as usize, mp));
    let before = unsafe { core::slice::from_raw_parts(p, 0x400).to_vec() };
    unsafe {
        fc4p_006f8070(p);
    }
    assert_eq!(view.population(), 400);
    let mut expected = before;
    expected[0xA4..0xA8].copy_from_slice(&400i32.to_le_bytes());
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x400) }, expected);
    let mut expected = vec![Call::Divide(28600, 400), Call::History(p as usize, 0x2A5E7009, 71)];
    for (g, n) in [12, 48, 54, 40, 24, 14, 8, 0, 0].into_iter().enumerate() {
        expected.push(Call::Curve(unsafe { p.add(0x264 + g * 16) } as usize, 30f32.to_bits()));
        for (series, v) in [
            (0xCA545CAB + g as u32, n),
            (0xCA545CCA + g as u32, if n > 0 { 50 } else { 0 }),
        ] {
            expected.extend([
                Call::Allocate(0x2C),
                Call::Construct(mp as usize),
                Call::Message(mp as usize, 4, 0),
                Call::Message(mp as usize, 0x14, 0x89EFA536),
                Call::Message(mp as usize, 0x2C, series),
                Call::Message(mp as usize, 0x40, v),
                Call::Post(sp as usize, mp as usize),
                Call::Message(mp as usize, 8, 0),
            ]);
        }
    }
    assert_eq!(take(), expected);
    view.set_message_server(core::ptr::null_mut());
    view.set_age_count([0; 20]);
    view.set_running_population(0);
    unsafe {
        fc4p_006f8070(p);
    }
    assert_eq!(take(), [Call::History(p as usize, 0x2A5E7009, 0)]);
    assert_eq!(view.population(), 0);
}
