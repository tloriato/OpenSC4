// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Developer(u32),
    Growth(u32),
    Decline(u32),
    Stage(u32, u32, u32),
    Lookup(u32, u32, u8),
}
thread_local! {
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static LOOKUP: RefCell<u32> = const { RefCell::new(0) };
}
fn events() -> Vec<Event> {
    EVENTS.with(|e| e.replace(vec![]))
}

#[repr(C)]
struct Developer {
    table: *const usize,
    id: u32,
    done: u32,
}
extern "thiscall" fn developer_idle(this: *mut Developer) -> u64 {
    let d = unsafe { &*this };
    EVENTS.with(|e| e.borrow_mut().push(Event::Developer(d.id)));
    (0x1234_5678u64 << 32) | u64::from(d.done)
}
extern "thiscall" fn growth(this: *mut u8) -> u64 {
    let sim = unsafe { BuildingSim::from_ptr(this) };
    EVENTS.with(|e| e.borrow_mut().push(Event::Growth(sim.state())));
    sim.set_state(99);
    0x1111_2222_AABB_CCDD
}
extern "thiscall" fn decline(this: *mut u8) -> u64 {
    let sim = unsafe { BuildingSim::from_ptr(this) };
    EVENTS.with(|e| e.borrow_mut().push(Event::Decline(sim.state())));
    sim.set_state(99);
    0x3333_4444_5566_7788
}
fn idle(p: *mut u8) -> (u32, u32) {
    let mut c = RawCall::new(
        Conv::Thiscall,
        fc4p_00699ea0 as *const () as usize,
        p as u32,
        &[0xFFFF_FFFF],
    );
    c.edx = 0xDEAD_BEEF;
    unsafe { call_raw(&mut c) };
    (c.eax, c.edx)
}

#[test]
fn idle_handles_every_state_and_keeps_unused_argument_untouched() {
    let _g = fake(0x0069_9440, growth as *const () as usize);
    let _d = fake(0x0069_9890, decline as *const () as usize);
    let mut o = Object::new(0x100);
    let mut h = Object::new(24);
    let p = o.ptr();
    let header = h.ptr();
    let sim = unsafe { BuildingSim::from_ptr(p) };
    sim.set_developers(header);
    sim.set_developer_iterator(header);
    unsafe { Node::from_ptr(header).set_left(header) };
    for state in [0, 5, u32::MAX, 0x8000_0000] {
        sim.set_state(state);
        assert_eq!(idle(p), (state.wrapping_sub(1), 0xDEAD_BEEF));
        assert_eq!(sim.state(), state);
        assert!(events().is_empty());
    }
    for state in [1, 2, 3] {
        for flag in [0, 1, 0x80] {
            sim.set_state(state);
            sim.set_use_growth_plan(flag);
            let result = idle(p);
            assert_eq!(sim.state(), 4);
            if flag == 0 {
                assert_eq!(result.0, if state == 1 { header as u32 & !0xFF } else { 0 });
                assert!(events().is_empty());
            } else {
                assert_eq!(result, (0xAABB_CCDD, 0x1111_2222));
                assert_eq!(events(), [Event::Growth(3)]);
            }
        }
    }
    sim.set_state(4);
    assert_eq!(idle(p), (0x5566_7788, 0x3333_4444));
    assert_eq!(sim.state(), 0);
    assert_eq!(events(), [Event::Decline(4)]);
}

#[test]
fn idle_resumes_yielded_developer_and_walks_right_children_and_parents() {
    let _g = fake(0x0069_9440, growth as *const () as usize);
    let _d = fake(0x0069_9890, decline as *const () as usize);
    let mut table = [0usize; 0x74 / 4];
    table[0x70 / 4] = developer_idle as *const () as usize;
    let mut devs: Vec<_> = (0..4)
        .map(|id| Developer {
            table: table.as_ptr(),
            id,
            done: 0xABCD_EF01,
        })
        .collect();
    devs[1].done = 0xABCD_EF00;
    let mut nodes: Vec<_> = (0..4).map(|_| Object::new(24)).collect();
    let p: Vec<_> = nodes.iter_mut().map(Object::ptr).collect();
    let mut o = Object::new(0x100);
    let mut header = Object::new(24);
    let h = header.ptr();
    let sim = unsafe { BuildingSim::from_ptr(o.ptr()) };
    sim.set_developers(h);
    sim.set_state(1);
    unsafe {
        let hn = Node::from_ptr(h);
        hn.set_parent(p[2]);
        hn.set_left(p[0]);
        hn.set_right(p[3]);
        for (i, &p) in p.iter().enumerate() {
            Node::from_ptr(p).set_value((&mut devs[i] as *mut Developer) as u32);
        }
        Node::from_ptr(p[2]).set_parent(h);
        Node::from_ptr(p[2]).set_left(p[0]);
        Node::from_ptr(p[2]).set_right(p[3]);
        Node::from_ptr(p[0]).set_parent(p[2]);
        Node::from_ptr(p[0]).set_right(p[1]);
        Node::from_ptr(p[1]).set_parent(p[0]);
        Node::from_ptr(p[3]).set_parent(p[2]);
    }
    assert_eq!(idle(o.ptr()), (0xABCD_EF00, 0x1234_5678));
    assert_eq!((sim.state(), sim.developer_iterator()), (2, p[1]));
    assert_eq!(events(), [Event::Developer(0), Event::Developer(1)]);
    devs[1].done = 0xABCD_EF01;
    let r = idle(o.ptr());
    assert_eq!(r.0, h as u32 & !0xFF);
    assert_eq!(sim.state(), 4);
    assert_eq!(sim.developer_iterator(), h);
    assert_eq!(
        events(),
        [Event::Developer(1), Event::Developer(2), Event::Developer(3)]
    );
    idle(o.ptr());
    assert_eq!(events(), [Event::Decline(4)]);
}

#[test]
fn family_lookup_handles_collision_chains_absence_zero_and_unsigned_keys() {
    let mut buckets = [core::ptr::null_mut::<u8>(); 3];
    let mut a = Object::new(32);
    let mut b = Object::new(32);
    let (ap, bp) = (a.ptr(), b.ptr());
    unsafe {
        core::ptr::write_unaligned(ap.cast::<*mut u8>(), bp);
        core::ptr::write_unaligned(ap.add(8).cast::<u32>(), 0);
        core::ptr::write_unaligned(bp.add(8).cast::<u32>(), u32::MAX);
    }
    buckets[0] = ap;
    let mut begin = buckets.as_mut_ptr();
    let mut end = begin.wrapping_add(3);
    let _b = fake_data(0x00B4_8588, (&mut begin as *mut *mut *mut u8).cast());
    let _e = fake_data(0x00B4_858C, (&mut end as *mut *mut *mut u8).cast());
    let before = unsafe { std::slice::from_raw_parts(ap, 32) }.to_vec();
    assert_eq!(
        unsafe { fc4p_006985c0(0) },
        (u64::from(ap as u32) << 32) | u64::from(ap as u32 + 16)
    );
    assert_eq!(
        unsafe { fc4p_006985c0(u32::MAX) },
        (u64::from(bp as u32) << 32) | u64::from(bp as u32 + 16)
    );
    for key in [1, 2, 3, 0x8000_0000] {
        assert_eq!(unsafe { fc4p_006985c0(key) }, 0);
    }
    assert_eq!(unsafe { std::slice::from_raw_parts(ap, 32) }, before);
}

extern "thiscall" fn stage(this: *mut u8, purpose: u32, wealth: u32) -> u64 {
    EVENTS.with(|e| e.borrow_mut().push(Event::Stage(this as u32, purpose, wealth)));
    0x1234_5678_AABB_CCFE
}
extern "thiscall" fn lookup(this: *mut u8, key: *mut u8) -> u64 {
    let (developer, stage) = unsafe { (read(key.cast::<u32>()), *key.add(4)) };
    EVENTS.with(|e| e.borrow_mut().push(Event::Lookup(this as u32, developer, stage)));
    (0xDEAD_BEEFu64 << 32) | u64::from(LOOKUP.with(|v| *v.borrow()))
}

fn environment() -> (u16, u16, u32, u32) {
    let mut mxcsr = 0u32;
    let flags: u32;
    unsafe {
        core::arch::asm!("stmxcsr [{mx}]", "pushfd", "pop eax", mx=in(reg)&mut mxcsr, lateout("eax")flags);
    }
    (
        crate::x87::control_word(),
        crate::x87::status_word() & 0x3800,
        mxcsr,
        flags & 0x400,
    )
}

#[test]
fn max_capacity_keeps_integer_helper_low_word_signed_load_and_both_result_registers() {
    let _fpu = GameFpu::new();
    let _l = fake(0x0069_5F10, lookup as *const () as usize);
    let mut map = [0x1234_5678u32, 0];
    let _m = fake_data(0x00B4_85AC, map.as_mut_ptr().cast());
    let mut table = [0usize; 0xAC / 4];
    table[0xA8 / 4] = stage as *const () as usize;
    let mut o = Object::new(0x100);
    let p = o.ptr();
    unsafe { BuildingSim::from_ptr(p).set_vtable(table.as_ptr()) };
    let mut profile = Object::new(32);
    let pp = profile.ptr();
    let sentinel = 1.234_567_890_123_456_7f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{value}]",value=in(reg)&sentinel,out("st(0)")_);
    }
    for (capacity, integer) in [
        (12.75, 12i64),
        (-12.75, -12),
        (3e9, 3_000_000_000),
        (f32::NAN, i64::MIN),
        (f32::INFINITY, i64::MIN),
    ] {
        unsafe { core::ptr::write_unaligned(pp.add(24).cast::<f32>(), capacity) };
        LOOKUP.with(|l| *l.borrow_mut() = pp as u32);
        let args = [0x8000_0000, 0xDEAD_0002, 0xBEEF_0003];
        let mut c = RawCall::new(Conv::Thiscall, fc4p_00696c20 as *const () as usize, p as u32, &args);
        c.take_st0 = 1;
        let before = environment();
        unsafe { call_raw(&mut c) };
        assert_eq!(environment(), before);
        assert_eq!((c.eax, c.edx), (integer as u32, ((integer as u64) >> 32) as u32));
        assert_eq!(c.st0, F80::from_i32(integer as i32));
        assert_eq!(
            events(),
            [
                Event::Stage(p as u32, args[1], args[2]),
                Event::Lookup(map.as_mut_ptr() as u32, args[0], 0xFE)
            ]
        );
    }
    LOOKUP.with(|l| *l.borrow_mut() = map[0]);
    let args = [1, 2, 3];
    let mut c = RawCall::new(Conv::Thiscall, fc4p_00696c20 as *const () as usize, p as u32, &args);
    c.take_st0 = 1;
    unsafe { call_raw(&mut c) };
    assert_eq!((c.eax, c.edx, c.st0), (map[0], 0xDEAD_BEEF, F80::ZERO));
    events();
    let mut restored = 0f64;
    unsafe {
        core::arch::asm!("fstp qword ptr [{value}]",value=in(reg)&mut restored,out("st(0)")_);
    }
    assert_eq!(restored.to_bits(), sentinel.to_bits());
}
