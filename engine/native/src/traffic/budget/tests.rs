// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Get(usize, u32),
    Create(usize, u32, u32),
    Description(u32, u32),
    Cost(u32, u32, u32),
    Funding,
    Random(usize),
    Allocate(u32),
    Construct(usize),
    Retain,
    Type(u32),
    Sender(usize),
    Target(usize),
    Post(usize, usize, u32),
    Release,
    City,
    Update(usize, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static MESSAGE: RefCell<*mut u8> = const { RefCell::new(core::ptr::null_mut()) };
    static RANDOM: RefCell<f64> = const { RefCell::new(0.0) };
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn get(this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Get(this as usize, id));
    unsafe {
        if id == 3 && word(this, 8) != 0 {
            return core::ptr::null_mut();
        }
        let mutation = word(this, 12) as *mut u8;
        if !mutation.is_null() && id == 4 {
            TrafficSim::from_ptr(mutation).set_subway_cells(20);
        }
        word(word(this, 4) as *mut u8, id as usize * 4) as *mut u8
    }
}
extern "thiscall" fn create(this: *mut u8, id: u32, flags: u32) -> *mut u8 {
    log(Call::Create(this as usize, id, flags));
    unsafe { word(word(this, 4) as *mut u8, id as usize * 4) as *mut u8 }
}
extern "thiscall" fn cost(this: *mut u8, low: u32, high: u32) -> u32 {
    log(Call::Cost(unsafe { word(this, 4) }, low, high));
    0xABCDEF
}
extern "thiscall" fn description(_this: *mut u8, a: u32, b: u32) -> u32 {
    log(Call::Description(a, b));
    0
}
extern "thiscall" fn funding(this: *mut u8) -> *mut f32 {
    log(Call::Funding);
    this.wrapping_add(16).cast()
}
extern "thiscall" fn random(this: *mut u8) -> f64 {
    log(Call::Random(this as usize));
    RANDOM.with(|r| *r.borrow())
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    MESSAGE.with(|m| *m.borrow())
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    log(Call::Construct(this as usize));
    this
}
extern "thiscall" fn retain(_this: *mut u8) -> u32 {
    log(Call::Retain);
    1
}
extern "thiscall" fn msg_type(_this: *mut u8, id: u32) -> u32 {
    log(Call::Type(id));
    1
}
extern "thiscall" fn sender(_this: *mut u8, value: *mut u8) -> u32 {
    log(Call::Sender(value as usize));
    1
}
extern "thiscall" fn target(_this: *mut u8, value: *mut u8) -> u32 {
    log(Call::Target(value as usize));
    1
}
extern "thiscall" fn post(this: *mut u8, msg: *mut u8, flags: u32) -> u32 {
    log(Call::Post(this as usize, msg as usize, flags));
    1
}
extern "thiscall" fn release(_this: *mut u8) -> u32 {
    log(Call::Release);
    0xAABB_CCDD
}
extern "thiscall" fn city_tick(_this: *mut u8) -> u32 {
    log(Call::City);
    0
}
extern "thiscall" fn update(this: *mut u8) -> u32 {
    log(Call::Update(
        this as usize,
        unsafe { TrafficSim::from_ptr(this) }.subway_cells(),
    ));
    0x1234
}

struct World {
    sim: Object,
    department: Object,
    items: Vec<Object>,
    _pointers: Vec<usize>,
    _tables: Vec<Vec<usize>>,
    message: Object,
    server: Object,
    city: Object,
    _constants: Box<[f32; 4]>,
    _city_pointer: Box<*mut u8>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[
                (0x50, get as *const () as usize),
                (0x44, create as *const () as usize),
                (0x28, funding as *const () as usize),
            ]),
            table(&[
                (0x30, cost as *const () as usize),
                (0x1C, description as *const () as usize),
            ]),
            table(&[
                (4, retain as *const () as usize),
                (0x14, msg_type as *const () as usize),
                (0x30, sender as *const () as usize),
                (0x84, target as *const () as usize),
                (8, release as *const () as usize),
            ]),
            table(&[(0x10, post as *const () as usize)]),
            table(&[(0x20, city_tick as *const () as usize)]),
        ];
        let mut items: Vec<_> = (0..7)
            .map(|id| {
                let mut o = object(8, &tables[1]);
                unsafe {
                    store(o.ptr(), 4, id);
                }
                o
            })
            .collect();
        let pointers: Vec<_> = items.iter_mut().map(|o| o.ptr() as usize).collect();
        let mut department = object(20, &tables[0]);
        unsafe {
            store(department.ptr(), 4, pointers.as_ptr() as u32);
            store(department.ptr(), 8, 0);
            store(department.ptr(), 12, 0);
            store(department.ptr(), 16, 1f32.to_bits());
        }
        let mut message = object(0x2C, &tables[2]);
        let mut server = object(4, &tables[3]);
        let mut city_obj = object(4, &tables[4]);
        let mut constants = Box::new([4294967296.0f32, 12582912.0, 255.0, 0.0]);
        let mut city_pointer = Box::new(city_obj.ptr());
        let fakes = vec![
            fake_data(0x00A8_0AA8, (&mut constants[0] as *mut f32).cast()),
            fake_data(0x00AB_32D8, (&mut constants[1] as *mut f32).cast()),
            fake_data(0x00A9_D8B0, (&mut constants[2] as *mut f32).cast()),
            fake_data(0x00A8_1054, (&mut constants[3] as *mut f32).cast()),
            fake_data(0x00B4_3D5C, (&mut *city_pointer as *mut *mut u8).cast()),
            fake(0x0091_33DA, allocate as *const () as usize),
            fake(0x0091_34D6, construct as *const () as usize),
            fake(0x0091_3A29, random as *const () as usize),
        ];
        MESSAGE.with(|m| *m.borrow_mut() = message.ptr());
        let mut sim = Object::new(0xD00);
        let s = unsafe { TrafficSim::from_ptr(sim.ptr()) };
        s.set_roads(department.ptr());
        s.set_transit(department.ptr());
        s.set_messages(server.ptr());
        s.set_random(0x1234usize as *mut u8);
        s.set_strike_keep(1.0);
        s.set_strike_start(0.7);
        s.set_strike_scale(1.0);
        s.set_strike_limit(24);
        take();
        Self {
            sim,
            department,
            items,
            _pointers: pointers,
            _tables: tables,
            message,
            server,
            city: city_obj,
            _constants: constants,
            _city_pointer: city_pointer,
            _fakes: fakes,
        }
    }
    fn s(&mut self) -> TrafficSim {
        unsafe { TrafficSim::from_ptr(self.sim.ptr()) }
    }
    fn pricing(&mut self, counts: [u32; 13], prices: [f32; 13]) {
        for i in 0..13 {
            unsafe {
                store(self.sim.ptr(), 0x740 + i * 4, counts[i]);
                store(self.sim.ptr(), 0xB54 + i * 4, prices[i].to_bits());
            }
        }
    }
    fn transition(&mut self, id: u32) -> Vec<Call> {
        vec![
            Call::Funding,
            Call::Allocate(0x2C),
            Call::Construct(self.message.ptr() as usize),
            Call::Retain,
            Call::Type(id),
            Call::Sender(self.sim.ptr() as usize),
            Call::Target(self.sim.ptr() as usize),
            Call::Post(self.server.ptr() as usize, self.message.ptr() as usize, 0),
            Call::Release,
        ]
    }
}

#[test]
fn infrastructure_costs_round_ties_to_even_and_preserve_call_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pricing(
        [10, 11, 12, 13, 0, 0, 14, 15, 16, 17, 18, 0, 19],
        [0.25, 0.5, 0.25, 0.5, 0.0, 0.0, 0.25, 0.5, 0.25, 0.5, 0.25, 0.0, 0.5],
    );
    let before = unsafe { bytes(w.sim.ptr(), 0xD00) };
    unsafe {
        fc4p_0070d8a0(w.sim.ptr());
    }
    let d = w.department.ptr() as usize;
    assert_eq!(
        take(),
        [
            Call::Get(d, 2),
            Call::Cost(2, 6, 0),
            Call::Get(d, 1),
            Call::Cost(1, 6, 0),
            Call::Get(d, 3),
            Call::Cost(3, 13, 0),
            Call::Get(d, 4),
            Call::Cost(4, 4, 0)
        ]
    );
    unsafe {
        fc4p_0070da40(w.sim.ptr());
    }
    assert_eq!(
        take(),
        [
            Call::Get(d, 3),
            Call::Cost(3, 6, 0),
            Call::Get(d, 4),
            Call::Cost(4, 8, 0),
            Call::Get(d, 5),
            Call::Cost(5, 4, 0),
            Call::Get(d, 6),
            Call::Cost(6, 8, 0)
        ]
    );
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD00) }, before);
    assert_eq!(w.items.len(), 7);
    assert!(!w.city.ptr().is_null());
}

#[test]
fn shipped_maintenance_prices_match_the_worked_city_costs() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pricing(
        [538, 143, 100, 458, 0, 0, 37, 60, 7, 3, 10, 0, 21],
        [0.1, 0.03, 0.5, 0.05, 0.0, 0.0, 0.15, 0.3, 0.2, 0.3, 0.1, 0.0, 0.5],
    );
    unsafe {
        fc4p_0070d8a0(w.sim.ptr());
        fc4p_0070da40(w.sim.ptr());
    }
    let costs: Vec<_> = take().into_iter().filter(|c| matches!(c, Call::Cost(..))).collect();
    assert_eq!(
        costs,
        [
            Call::Cost(2, 55, 0),
            Call::Cost(1, 23, 0),
            Call::Cost(3, 60, 0),
            Call::Cost(4, 6, 0),
            Call::Cost(3, 4, 0),
            Call::Cost(4, 18, 0),
            Call::Cost(5, 1, 0),
            Call::Cost(6, 1, 0)
        ]
    );
}

#[test]
fn rail_creation_and_post_lookup_cell_reload() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pricing([1; 13], [1.0; 13]);
    unsafe {
        store(w.department.ptr(), 8, 1);
        store(w.department.ptr(), 12, w.sim.ptr() as u32);
        fc4p_0070da40(w.sim.ptr());
    }
    let d = w.department.ptr() as usize;
    assert_eq!(
        take(),
        [
            Call::Get(d, 3),
            Call::Create(d, 3, 0),
            Call::Description(0x6A3F_F01C, 0x6A8B_0CFB),
            Call::Cost(3, 1, 0),
            Call::Get(d, 4),
            Call::Cost(4, 20, 0),
            Call::Get(d, 5),
            Call::Cost(5, 1, 0),
            Call::Get(d, 6),
            Call::Cost(6, 1, 0)
        ]
    );
}

#[test]
fn budget_nan_bit_patterns_and_combined_low_word_carry() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pricing([1; 13], [f32::from_bits(0xFFFF_FFFF); 13]);
    unsafe {
        fc4p_0070d8a0(w.sim.ptr());
    }
    let costs: Vec<_> = take().into_iter().filter(|c| matches!(c, Call::Cost(..))).collect();
    assert_eq!(
        costs,
        [
            Call::Cost(2, 0x697F_FFFE, 1),
            Call::Cost(1, 0xB4BF_FFFF, 0),
            Call::Cost(3, 0x697F_FFFE, 1),
            Call::Cost(4, 0xB4BF_FFFF, 0)
        ]
    );
    assert_eq!(unsafe { unsigned(u32::MAX) }.to_f64(), 4294967296.0);
    assert_eq!(unsafe { unsigned(0x8000_0001) }.to_f64(), 2147483648.0);
}

#[test]
fn core_strike_transitions_post_exact_message_sequence() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let s = w.s();
    s.set_budget_total(5);
    s.set_strike_state(2);
    unsafe {
        store(w.department.ptr(), 16, 0.5f32.to_bits());
    }
    RANDOM.with(|r| *r.borrow_mut() = 0.328);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(s.strike_state(), 2);
    assert_eq!(take(), [Call::Funding, Call::Random(0x1234)]);
    unsafe {
        store(w.department.ptr(), 16, 0f32.to_bits());
    }
    RANDOM.with(|r| *r.borrow_mut() = 0.434);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    let mut want = w.transition(0x6924_7DC6);
    want.insert(1, Call::Random(0x1234));
    assert_eq!(take(), want);
    assert_eq!(s.strike_state(), 0);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), w.transition(0x6924_7DC4));
    assert_eq!((s.strike_state(), s.strike_months(), s.striking()), (1, 0, 1));
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), [Call::Funding]);
    assert_eq!(s.strike_months(), 1);
    unsafe {
        store(w.department.ptr(), 16, 1f32.to_bits());
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), w.transition(0x6924_7DC5));
    assert_eq!((s.strike_state(), s.striking()), (2, 0));
}

#[test]
fn strikes_nan_funding_unsigned_month_limit_and_nonpositive_budget_edges() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let s = w.s();
    unsafe {
        store(w.department.ptr(), 16, f32::NAN.to_bits());
    }
    s.set_strike_state(2);
    s.set_budget_total(5);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), [Call::Funding]);
    assert_eq!(s.strike_state(), 2);
    s.set_strike_state(0);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), w.transition(0x6924_7DC4));
    assert_eq!(s.strike_state(), 1);
    s.set_strike_months(0xFFFF_FFFE);
    s.set_strike_limit(u32::MAX);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), [Call::Funding]);
    assert_eq!(s.strike_months(), u32::MAX);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), w.transition(0x6924_7DC5));
    for budget in [0, -1, i64::MIN] {
        s.set_strike_state(2);
        s.set_budget_total(budget);
        unsafe {
            store(w.department.ptr(), 16, 0);
            fc4p_0070dba0(w.sim.ptr());
        }
        assert_eq!(take(), [Call::Funding]);
    }
    s.set_budget_total(0);
    s.set_strike_state(0);
    s.set_striking(0xA5);
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), w.transition(0x6924_7DC3));
    assert_eq!(s.striking(), 0xA5);
    s.set_strike_state(3);
    let before = unsafe { bytes(w.sim.ptr(), 0xD00) };
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    assert_eq!(take(), [Call::Funding]);
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD00) }, before);
}

#[test]
fn subway_changes_wrap_increment_saturate_decrement_and_always_reprice() {
    let _fake = fake(0x0070_DA40, update as *const () as usize);
    let mut o = Object::new(0xD00);
    let s = unsafe { TrafficSim::from_ptr(o.ptr()) };
    for (n, flag, want) in [
        (0, 0, 0),
        (1, 0, 0),
        (u32::MAX, 0, u32::MAX - 1),
        (u32::MAX, 1, 0),
        (0, 0x100, 0),
        (0, 0x101, 1),
    ] {
        s.set_subway_cells(n);
        let mut before = unsafe { bytes(o.ptr(), 0xD00) };
        assert_eq!(unsafe { fc4p_0070ea70(o.ptr(), 0xDEAD_BEEF, flag) }, 0x1234);
        before[0x75C..0x760].copy_from_slice(&want.to_le_bytes());
        assert_eq!(unsafe { bytes(o.ptr(), 0xD00) }, before);
        assert_eq!(take(), [Call::Update(o.ptr() as usize, want)]);
    }
}

#[test]
fn ts61_final_statistics_only_publish_the_expected_fields_and_restore_ecx() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let s = w.s();
    s.set_transit_accumulator(1.25);
    s.set_toll_accumulator(9.5);
    s.set_commute_sum(10.0);
    s.set_commute_count(4);
    s.set_freight_sum(3.0);
    s.set_freight_count(2);
    s.set_commute_maximum(6.0);
    unsafe {
        *w.sim.ptr().add(0x654) = 0x1C;
    }
    let mut want = unsafe { bytes(w.sim.ptr(), 0xD00) };
    for (offset, value) in [(0x780, 1.25f32), (0x784, 9.5), (0x420, 2.5)] {
        want[offset..offset + 4].copy_from_slice(&value.to_bits().to_le_bytes());
    }
    want[0x41C] = 63;
    let r = unsafe {
        invoke(
            fc4p_0070e7d0 as *const () as usize,
            w.sim.ptr(),
            &[0xABCDEF],
            0,
            0,
            false,
        )
    };
    assert_eq!((r.eax as u8, r.ecx), (1, w.sim.ptr() as u32));
    assert_eq!(take(), [Call::City]);
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD00) }, want);
    s.set_freight_count(0);
    s.set_commute_count(0);
    s.set_commute_sum(0.0);
    unsafe {
        invoke(fc4p_0070e7d0 as *const () as usize, w.sim.ptr(), &[0], 0, 0, false);
    }
    assert_eq!(s.freight_byte(), 0);
    assert!(s.average_commute().is_nan());
    take();
    s.set_freight_count(1);
    s.set_freight_sum(f32::INFINITY);
    let r = unsafe { invoke(fc4p_0070e7d0 as *const () as usize, w.sim.ptr(), &[0], 0, 0, false) };
    assert_eq!((s.freight_byte(), r.edx), (0, 0x8000_0000));
    take();
}

#[test]
fn remaining_time_truncation_and_minimum_with_zero_and_nan_scale() {
    let _fpu = GameFpu::new();
    let mut scale = 0f32;
    let _fake = fake_data(0x00B0_A840, (&mut scale as *mut f32).cast());
    let mut deadline = 0u64;
    assert_eq!(unsafe { fc4p_0070e2b0((&mut deadline as *mut u64).cast()) }, 1);
    scale = f32::NAN;
    std::hint::black_box(scale);
    assert_eq!(
        unsafe { fc4p_0070e2b0((&mut deadline as *mut u64).cast()) },
        0x8000_0000_0000_0001
    );
}

#[test]
fn floating_ports_preserve_control_word_mxcsr_direction_and_existing_x87_value() {
    use crate::traffic::test_support::float_environment;
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pricing([10; 13], [0.5; 13]);
    let s = w.s();
    s.set_strike_state(2);
    s.set_budget_total(5);
    s.set_commute_sum(10.0);
    s.set_commute_count(2);
    s.set_freight_sum(3.0);
    s.set_freight_count(2);
    s.set_commute_maximum(6.0);
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = float_environment();
    unsafe {
        fc4p_0070d8a0(w.sim.ptr());
    }
    let after_roads = float_environment();
    unsafe {
        fc4p_0070da40(w.sim.ptr());
    }
    let after_transit = float_environment();
    unsafe {
        fc4p_0070dba0(w.sim.ptr());
    }
    let after_strikes = float_environment();
    unsafe {
        invoke(fc4p_0070e7d0 as *const () as usize, w.sim.ptr(), &[0], 0, 0, false);
    }
    let after_statistics = float_environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(
        [after_roads, after_transit, after_strikes, after_statistics],
        [before; 4]
    );
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    take();
}
