// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout versions of the core's proposition dates, amount, term, and retained-list cases.

use super::*;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Propose(u32, u32),
    Flags(u32, u32),
    Node(bool, u32, u32),
    Ref,
    Amount(u32, bool),
    Tuning,
    Holder,
    Property(u32, u32),
    Seed(i32, u32),
    Range(u64, u64),
    Time,
    Today(u32),
    Day,
    Expiration(u32),
    Current(u32),
    Start(u32),
    End(u32),
    Destroy(bool),
    Release,
}
struct State {
    calls: Vec<Call>,
    deal: usize,
    tuning: usize,
    sim: usize,
    output: usize,
    low: Option<u32>,
    high: Option<u32>,
    random: F80,
    success: u32,
    give_deal: bool,
    references: i32,
    today: usize,
    expiration: usize,
    initialized: usize,
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 3]>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            deal: 0,
            tuning: 0,
            sim: 0,
            output: 0,
            low: None,
            high: None,
            random: F80::ZERO,
            success: 1,
            give_deal: true,
            references: 0,
            today: 0,
            expiration: 0,
            initialized: 0,
            nodes: vec![],
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn propose(_: *mut u8, kind: u32, city: u32, out: *mut u32) -> u64 {
    record(Call::Propose(kind, city));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe {
            assert_eq!(out.read_unaligned(), 0);
            if s.give_deal {
                out.write_unaligned(s.deal as u32);
                s.references += 1;
            }
        }
        pair(s.success, 0x1111_2222)
    })
}
extern "thiscall" fn flags(_: *mut u8, mask: u32, enabled: u32) {
    record(Call::Flags(mask, enabled));
}
extern "fastcall" fn allocate_node(list: *mut u8, _: u32, count: u32, hint: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let sim = list as usize == s.sim + 0x20;
        assert!(sim || list as usize == s.output);
        s.calls.push(Call::Node(sim, count, hint));
        s.nodes.push(Box::new([0xA5A5_A5A5; 3]));
        s.nodes.last_mut().unwrap().as_mut_ptr() as u32
    })
}
extern "thiscall" fn addref(_: *mut u8) -> u64 {
    record(Call::Ref);
    STATE.with(|s| s.borrow_mut().references += 1);
    0x2233_4455_6677_8899
}
extern "thiscall" fn release(_: *mut u8) -> u64 {
    record(Call::Release);
    STATE.with(|s| s.borrow_mut().references -= 1);
    0x3344_5566_AABB_CCDD
}
extern "thiscall" fn amount(_: *mut u8, amount: u32) {
    let listed = STATE.with(|s| unsafe {
        let s = s.borrow();
        let h = pointer(s.sim as *mut u8, 0x20);
        pointer(h, 0) != h
    });
    record(Call::Amount(amount, listed));
}
extern "thiscall" fn tuning(_: *mut u8) -> u32 {
    record(Call::Tuning);
    STATE.with(|s| s.borrow().tuning as u32)
}
extern "thiscall" fn holder(this: *mut u8) -> u32 {
    record(Call::Holder);
    this as u32
}
extern "C" fn property(_: *mut u8, id: u32, out: *mut u32) {
    record(Call::Property(id, unsafe { out.read_unaligned() }));
    let value = STATE.with(|s| {
        let s = s.borrow();
        match id {
            0xAA46_934B => s.low,
            0xEA46_934F => s.high,
            _ => panic!("unexpected property"),
        }
    });
    if let Some(value) = value {
        unsafe {
            out.write_unaligned(value);
        }
    }
}
extern "thiscall" fn seed(_: *mut u8, seed: i32) {
    let flags = STATE.with(|s| unsafe { (s.borrow().initialized as *const u32).read() });
    record(Call::Seed(seed, flags));
}
extern "thiscall" fn range_body(_: *mut u8, low: f64, high: f64) -> *const F80 {
    record(Call::Range(low.to_bits(), high.to_bits()));
    STATE.with(|s| &s.borrow().random as *const F80)
}
#[unsafe(naked)]
extern "thiscall" fn range() {
    core::arch::naked_asm!("push dword ptr [esp + 16]", "push dword ptr [esp + 16]", "push dword ptr [esp + 16]", "push dword ptr [esp + 16]", "call {}", "fld tbyte ptr [eax]", "ret 16", sym range_body);
}
extern "thiscall" fn time(_: *mut u8) -> u32 {
    record(Call::Time);
    1000
}
extern "fastcall" fn today(this: *mut u32, _: u32, value: u32) -> u64 {
    record(Call::Today(value));
    unsafe {
        this.cast::<[u32; 4]>()
            .write_unaligned([0x00AC_EAF0, value, 0x00AC_EA38, 0x1122_3344]);
    }
    STATE.with(|s| s.borrow_mut().today = this as usize);
    pair(this as u32, 0x4455_6677)
}
extern "fastcall" fn day(this: *mut u8, _: u32) -> u64 {
    record(Call::Day);
    assert_eq!(this as usize, STATE.with(|s| s.borrow().today) + 8);
    pair(unsafe { word(this.sub(8), 4) }, 0x5566_7788)
}
extern "fastcall" fn expiration(this: *mut u32, _: u32, value: u32) -> u64 {
    record(Call::Expiration(value));
    unsafe {
        this.cast::<[u32; 4]>()
            .write_unaligned([0x00AC_EAF0, value, 0x00AC_EA38, 0]);
    }
    STATE.with(|s| s.borrow_mut().expiration = this as usize);
    pair(this as u32, 0x6677_8899)
}
extern "thiscall" fn current(_: *mut u8, date: *mut u8) {
    record(Call::Current(unsafe { word(date.sub(8), 4) }));
}
extern "thiscall" fn start(_: *mut u8, date: *mut u8) {
    record(Call::Start(unsafe { word(date.sub(8), 4) }));
}
extern "thiscall" fn end(_: *mut u8, date: *mut u8) {
    record(Call::End(unsafe { word(date.sub(8), 4) }));
}
extern "thiscall" fn destroy_body(this: *mut u8) {
    let expiry = STATE.with(|s| this as usize == s.borrow().expiration);
    assert!(expiry || STATE.with(|s| this as usize == s.borrow().today));
    record(Call::Destroy(expiry));
}
#[unsafe(naked)]
extern "thiscall" fn destroy() {
    core::arch::naked_asm!("push eax", "push edx", "call {}", "pop edx", "pop eax", "ret", sym destroy_body);
}

// The tables remain at fixed addresses for all virtual calls.
#[allow(clippy::vec_box)]
struct World {
    sim: Object,
    deal: Object,
    tuning: Object,
    time: Object,
    tables: Vec<Box<[usize; 40]>>,
    output: Box<u32>,
    output_header: Box<[u32; 3]>,
    sim_header: Box<[u32; 3]>,
    initialized: Box<u32>,
    random: Box<[u8; 24]>,
    time_global: Box<usize>,
    epsilon: Box<f64>,
    half: Box<f32>,
    bias: Box<f32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(initialized: u32) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            sim: Object::new(0x48),
            deal: Object::new(0x80),
            tuning: Object::new(4),
            time: Object::new(4),
            tables: vec![],
            output: Box::new(0),
            output_header: Box::new([0; 3]),
            sim_header: Box::new([0; 3]),
            initialized: Box::new(initialized),
            random: Box::new([0xA5; 24]),
            time_global: Box::new(0),
            epsilon: Box::new(0.001),
            half: Box::new(0.5),
            bias: Box::new(12582912.0),
            _fakes: vec![],
        };
        for (object, slots) in [
            (
                w.sim.ptr(),
                vec![
                    (0x60, propose as *const () as usize),
                    (0x94, tuning as *const () as usize),
                ],
            ),
            (
                w.deal.ptr(),
                vec![
                    (4, addref as *const () as usize),
                    (8, release as *const () as usize),
                    (0x94, flags as *const () as usize),
                    (0x2C, amount as *const () as usize),
                    (0x74, current as *const () as usize),
                    (0x64, start as *const () as usize),
                    (0x6C, end as *const () as usize),
                ],
            ),
            (w.tuning.ptr(), vec![(0x18, holder as *const () as usize)]),
            (w.time.ptr(), vec![(0x1C, time as *const () as usize)]),
        ] {
            let mut table = Box::new([0; 40]);
            for (slot, function) in slots {
                table[slot / 4] = function;
            }
            unsafe {
                object.cast::<usize>().write(table.as_ptr() as usize);
            }
            w.tables.push(table);
        }
        for header in [&mut w.output_header, &mut w.sim_header] {
            let p = header.as_mut_ptr() as u32;
            **header = [p, p, 0xA5A5_A5A5];
        }
        *w.output = w.output_header.as_mut_ptr() as u32;
        unsafe {
            w.sim
                .ptr()
                .add(0x20)
                .cast::<u32>()
                .write(w.sim_header.as_mut_ptr() as u32);
        }
        *w.time_global = w.time.ptr() as usize;
        for (address, p) in [
            (0x00B4_9B30, (&mut *w.initialized as *mut u32).cast::<u8>()),
            (0x00B4_9B18, w.random.as_mut_ptr()),
            (0x00B4_3D5C, (&mut *w.time_global as *mut usize).cast()),
            (0x00AA_2270, (&mut *w.epsilon as *mut f64).cast()),
            (0x00A8_4D2C, (&mut *w.half as *mut f32).cast()),
            (0x00AB_1CAC, (&mut *w.bias as *mut f32).cast()),
        ] {
            w._fakes.push(fake_data(address, p));
        }
        for (address, f) in [
            (0x0042_0FD0, allocate_node as *const () as usize),
            (0x005F_D4F0, property as *const () as usize),
            (0x0091_3A5C, seed as *const () as usize),
            (0x0091_3761, range as *const () as usize),
            (0x0091_2DFA, today as *const () as usize),
            (0x0091_297E, day as *const () as usize),
            (0x0091_2E7B, expiration as *const () as usize),
            (0x0091_28BF, destroy as *const () as usize),
        ] {
            w._fakes.push(fake(address, f));
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.deal = w.deal.ptr() as usize;
            s.tuning = w.tuning.ptr() as usize;
            s.sim = w.sim.ptr() as usize;
            s.output = &mut *w.output as *mut u32 as usize;
            s.initialized = &mut *w.initialized as *mut u32 as usize;
            s.random = F80::from_f32(1.0);
        });
        w
    }
    fn run(&mut self, add: u32, amount: u32) -> (u64, [u32; 5]) {
        let mut args = [
            0x4773_FFE1,
            amount,
            0x1234_5678,
            &mut *self.output as *mut u32 as u32,
            add,
        ];
        let result = unsafe {
            checked_call(
                fc4p_006d0520 as *const () as usize,
                self.sim.ptr(),
                &mut args,
                0xFACE_CAFE,
                0x1122_3344,
                Conv::Thiscall,
            )
        };
        (result, args)
    }
    fn list_values(&self, header: *mut u8) -> Vec<usize> {
        unsafe {
            let mut p = pointer(header, 0);
            let mut values = vec![];
            while p != header {
                values.push(pointer(p, 8) as usize);
                p = pointer(p, 0);
            }
            values
        }
    }
}

#[test]
fn propositions_set_dates_and_amount_between_the_two_list_insertions() {
    let _fpu = GameFpu::new();
    for add in [0, 0x100, 1, 0x80] {
        let mut w = World::new(0xA4);
        STATE.with(|s| s.borrow_mut().random = F80::from_f32(2.75));
        let (result, args) = w.run(add, 0);
        assert_eq!(result, 0x3344_5566_AABB_CC01);
        assert_eq!(args[0], 3);
        assert_eq!(args[2], 5.0f32.to_bits());
        assert_eq!(args[4], 2.75f32.to_bits());
        let added = add as u8 != 0;
        assert_eq!(
            w.list_values(w.sim_header.as_ptr().cast_mut().cast()),
            if added { vec![w.deal.ptr() as usize] } else { vec![] }
        );
        assert_eq!(
            w.list_values(w.output_header.as_ptr().cast_mut().cast()),
            [w.deal.ptr() as usize]
        );
        assert_eq!(STATE.with(|s| s.borrow().references), if added { 2 } else { 1 });
        let mut expected = vec![Call::Propose(0x4773_FFE1, 0x1234_5678)];
        if added {
            expected.extend([Call::Flags(0x80, 0), Call::Node(true, 1, 0), Call::Ref]);
        }
        let high = (F80::from_f32(5.0) + F80::from_f64(0.001)).to_f64();
        expected.extend([
            Call::Amount(0, added),
            Call::Tuning,
            Call::Holder,
            Call::Property(0xAA46_934B, 1.0f32.to_bits()),
            Call::Property(0xEA46_934F, 5.0f32.to_bits()),
            Call::Seed(-1, 0xA5),
            Call::Range(1.0f64.to_bits(), high.to_bits()),
            Call::Time,
            Call::Today(1000),
            Call::Day,
            Call::Expiration(1730),
            Call::Current(1000),
            Call::Start(1000),
            Call::End(1730),
            Call::Node(false, 1, 0),
            Call::Ref,
            Call::Destroy(true),
            Call::Destroy(false),
            Call::Release,
        ]);
        assert_eq!(take(), expected);
        assert_eq!(*w.initialized, 0xA5);
    }
}

#[test]
fn failed_proposals_release_any_published_reference_and_skip_all_other_calls() {
    for give_deal in [false, true] {
        let mut w = World::new(0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.give_deal = give_deal;
            s.success = 0x9876_5400;
        });
        let (result, args) = w.run(1, 99);
        assert_eq!(
            result,
            if give_deal {
                0x3344_5566_AABB_CC00
            } else {
                0x1111_2222_9876_5400
            }
        );
        assert_eq!((args[0], args[1], args[2], args[4]), (0x4773_FFE1, 99, 0x1234_5678, 1));
        assert_eq!(
            take(),
            if give_deal {
                vec![Call::Propose(0x4773_FFE1, 0x1234_5678), Call::Release]
            } else {
                vec![Call::Propose(0x4773_FFE1, 0x1234_5678)]
            }
        );
        assert_eq!(STATE.with(|s| s.borrow().references), 0);
        assert_eq!(*w.initialized, 0);
    }
}

#[test]
fn tuning_clamps_the_upper_term_and_initialized_generators_are_reused() {
    let _fpu = GameFpu::new();
    for (low, high, expected_high) in [(3.0f32, 1.0f32, 3.0f32), (1.0, 3.0, 3.0), (-3.0, -1.0, -1.0)] {
        let mut w = World::new(3);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.low = Some(low.to_bits());
            s.high = Some(high.to_bits());
        });
        let (_, args) = w.run(0, 0x8000_0000);
        assert_eq!(args[2], expected_high.to_bits());
        let calls = take();
        assert!(!calls.iter().any(|c| matches!(c, Call::Seed(..))));
        assert!(calls.contains(&Call::Amount(0x8000_0000, false)));
        assert!(calls.contains(&Call::Range(
            (low as f64).to_bits(),
            (F80::from_f32(expected_high) + F80::from_f64(0.001)).to_f64().to_bits()
        )));
    }
}

#[test]
fn term_rounding_covers_exact_integers_neighbors_nan_and_large_values() {
    let _fpu = GameFpu::new();
    let _w = World::new(1);
    for (years, expected) in [
        (0.0, 0),
        (-0.0, 0),
        (1.0, 1),
        (f32::from_bits(1.0f32.to_bits() - 1), 0),
        (f32::from_bits(1.0f32.to_bits() + 1), 1),
        (2.75, 2),
        (-0.25, -1),
        (-1.0, -1),
        (-1.25, -2),
    ] {
        let mut scratch = 0;
        assert_eq!(unsafe { term_years(years, &mut scratch) }, expected, "{years}");
    }
    // These values exercise the bit arithmetic beyond its ordinary term range.
    for bits in [
        0x4AFF_FFFE,
        0x7F80_0000,
        0xFF80_0000,
        0x7FC0_1234,
        0x7F80_1234,
        0x4B80_0000,
        0xCB80_0000,
        1,
        0x8000_0001,
    ] {
        let years = f32::from_bits(bits);
        let mut scratch = 0;
        let result = unsafe { term_years(years, &mut scratch) };
        let (base, candidate, status): (u32, u32, u16);
        let mut operands = [bits, 0.5f32.to_bits(), 12582912.0f32.to_bits(), 0];
        unsafe {
            core::arch::asm!(
                "fld dword ptr [{p}]", "fsub dword ptr [{p} + 4]", "fadd dword ptr [{p} + 8]", "fstp dword ptr [{p} + 12]",
                "mov edx, [{p} + 12]", "add edx, 0xb4c00000", "lea ecx, [edx + 1]", "mov [{p} + 12], ecx",
                "fild dword ptr [{p} + 12]", "fld dword ptr [{p}]", "fucompp", "fnstsw ax",
                p = in(reg) operands.as_mut_ptr(),
                out("edx") base, out("ecx") candidate, out("ax") status, out("st(0)") _, out("st(1)") _,
            );
        }
        assert_eq!(result as u32, if status & 0x4400 == 0x4000 { candidate } else { base });
        assert_eq!(scratch, candidate);
    }
}

#[test]
fn term_is_stored_as_single_precision_before_dates_and_negative_days_wrap() {
    let _fpu = GameFpu::new();
    for (random, stored, expiration) in [
        (F80::from_f64(1.99999999), 2.0f32, 1730u32),
        (F80::from_f32(-3.25), -3.25, 1000u32.wrapping_sub(4 * 365)),
    ] {
        let mut w = World::new(1);
        STATE.with(|s| s.borrow_mut().random = random);
        let (_, args) = w.run(0, 0);
        assert_eq!(args[4], stored.to_bits());
        let calls = take();
        assert!(calls.contains(&Call::Expiration(expiration)));
        assert!(calls.contains(&Call::End(expiration)));
    }
}
