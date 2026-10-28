// SPDX-License-Identifier: GPL-3.0-or-later
//! Completed-pass vectors moved from the core, with ordered calls and byte-level write checks.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Recycling,
    History(u32, u32),
    Post(u32, u32),
    New,
    Load([u32; 3], u32),
    Funding,
    Evaluate(u32),
    Free(u32),
}
struct State {
    calls: Vec<Call>,
    sim: *mut u8,
    curve: *mut u32,
    storage: u32,
    output: f64,
    mutate_history: bool,
    mutate_posts: bool,
    mutate_recycling: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            sim: core::ptr::null_mut(),
            curve: core::ptr::null_mut(),
            storage: 0x1234_5678,
            output: 100.0,
            mutate_history: false,
            mutate_posts: false,
            mutate_recycling: false,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn recycling(this: *mut u8) {
    log(Call::Recycling);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this, s.sim);
        if s.mutate_recycling {
            // SAFETY: the test simulator.
            unsafe {
                PollutionSim::from_ptr(this).set_uncollected(77);
                CycleView::from_ptr(this).set_developed(7);
            }
        }
    });
}
extern "thiscall" fn history(_this: *mut u8, id: u32, value: u32) {
    log(Call::History(id, value));
    STATE.with(|s| {
        let s = s.borrow();
        if s.mutate_history && id == 0x0A11_00E8 {
            // SAFETY: the test simulator.
            unsafe {
                let sim = PollutionSim::from_ptr(s.sim);
                sim.set_average_water(123);
                sim.set_average_garbage(456);
                sim.set_average_air(u32::MAX);
                CycleView::from_ptr(s.sim).set_radiation_sum(999);
            }
        }
    });
}
extern "C" fn post(id: u32, data1: u32, data2: u32, data3: u32, data4: u32, unknown: u32) {
    assert_eq!([data2, data3, data4, unknown], [0; 4]);
    log(Call::Post(id, data1));
    STATE.with(|s| {
        let s = s.borrow();
        // SAFETY: the test simulator.
        unsafe {
            let cycle = CycleView::from_ptr(s.sim);
            assert_eq!(
                [
                    cycle.air_sum(),
                    cycle.treated_water_sum(),
                    cycle.water_sum(),
                    cycle.treatment_sum(),
                    cycle.developed(),
                    cycle.radiation_sum()
                ],
                [0; 6]
            );
            if s.mutate_posts && id == 0xDA6D_D35D {
                let sim = PollutionSim::from_ptr(s.sim);
                sim.set_dirty_air(2);
                sim.set_dirty_water(0);
                sim.set_dirty_garbage(3);
            }
        }
    });
}
extern "thiscall" fn curve_new(this: *mut u32) {
    log(Call::New);
    STATE.with(|s| s.borrow_mut().curve = this);
    // SAFETY: the original constructor fills four words.
    unsafe {
        for i in 0..4 {
            this.add(i).write(0);
        }
    }
}
extern "thiscall" fn curve_load(this: *mut u32, resource: *const u32, property: u32) {
    // SAFETY: the supplied resource is a three-word key.
    log(Call::Load(
        unsafe { resource.cast::<[u32; 3]>().read_unaligned() },
        property,
    ));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.curve, this);
        // SAFETY: the test curve's storage word.
        unsafe { this.write(s.storage) };
    });
}
#[repr(C)]
struct Department {
    table: *const usize,
    funding: f32,
}
extern "thiscall" fn funding(this: *mut Department) -> *const f32 {
    log(Call::Funding);
    // SAFETY: the test department.
    unsafe { &raw const (*this).funding }
}
extern "thiscall" fn curve_output(this: *mut u32, input: f32) -> f64 {
    log(Call::Evaluate(input.to_bits()));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.curve, this);
        s.output
    })
}
extern "C" fn curve_free(storage: u32) {
    log(Call::Free(storage));
}

struct World {
    object: Object,
    department: Box<Department>,
    _history: Box<usize>,
    _history_global: Box<*mut u8>,
    _tables: [Box<[usize; 16]>; 2],
    _constants: [Box<f32>; 2],
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut history_table = Box::new([0; 16]);
        history_table[0x30 / 4] = history as *const () as usize;
        let mut department_table = Box::new([0; 16]);
        department_table[0x28 / 4] = funding as *const () as usize;
        let mut history_object = Box::new(history_table.as_ptr() as usize);
        let mut history_global = Box::new((&mut *history_object as *mut usize).cast());
        let mut department = Box::new(Department {
            table: department_table.as_ptr(),
            funding: 1.0,
        });
        let mut constants = [Box::new(100.0), Box::new(0.01)];
        let mut fakes: Vec<_> = [
            (0x006E_6DA0, recycling as *const () as usize),
            (0x005F_98C0, post as *const () as usize),
            (0x007D_0410, curve_new as *const () as usize),
            (0x005F_4700, curve_load as *const () as usize),
            (0x005F_4250, curve_output as *const () as usize),
            (0x0090_CF63, curve_free as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        fakes.push(fake_data(0x00B4_3DC0, (&mut *history_global as *mut *mut u8).cast()));
        fakes.push(fake_data(0x00A8_0AA0, (&mut *constants[0] as *mut f32).cast()));
        fakes.push(fake_data(0x00A8_23A4, (&mut *constants[1] as *mut f32).cast()));
        let mut object = Object::new(0x240);
        let base = object.ptr();
        STATE.with(|s| s.borrow_mut().sim = base);
        // SAFETY: the test object covers the complete layout.
        unsafe { CycleView::from_ptr(base) }.set_department((&mut *department as *mut Department).cast());
        Self {
            object,
            department,
            _history: history_object,
            _history_global: history_global,
            _tables: [history_table, department_table],
            _constants: constants,
            _fakes: fakes,
            _fpu: GameFpu::new(),
        }
    }
    fn sim(&mut self) -> PollutionSim {
        unsafe { PollutionSim::from_ptr(self.object.ptr()) }
    }
    fn cycle(&mut self) -> CycleView {
        unsafe { CycleView::from_ptr(self.object.ptr()) }
    }
    fn run(&mut self) {
        unsafe { fc4p_006e7930(self.object.ptr()) };
    }
    fn bytes(&mut self) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(self.object.ptr(), 0x240).to_vec() }
    }
}

fn tail(input: f32, storage: u32) -> Vec<Call> {
    let mut calls = vec![
        Call::New,
        Call::Load([0x6534_284A, 0xE7E2_C2DB, 0xC911_E35B], 0x6911_E2DF),
        Call::Funding,
        Call::Evaluate(input.to_bits()),
        Call::Post(0xEA41_8D8F, 8),
    ];
    if storage != 0 {
        calls.push(Call::Free(storage));
    }
    calls
}

#[test]
fn completed_pass_records_averages_and_water_totals_then_resets_only_its_fields() {
    let mut w = World::new();
    let sim = w.sim();
    let cycle = w.cycle();
    cycle.set_air_sum(50_000);
    cycle.set_treated_water_sum(2345);
    cycle.set_water_sum(3456);
    cycle.set_treatment_sum(1234);
    cycle.set_developed(100);
    cycle.set_radiation_sum(2);
    sim.set_uncollected(1000);
    cycle.set_dirty_radiation(1);
    sim.set_dirty_air(2);
    sim.set_dirty_water(128);
    sim.set_dirty_garbage(255);
    let before = w.bytes();
    w.run();
    let mut expected = before.clone();
    for (offset, data) in [
        (0x100, 500u32.to_le_bytes().to_vec()),
        (0x104, 23u32.to_le_bytes().to_vec()),
        (0x108, vec![0; 16]),
        (0x118, 3456u32.to_le_bytes().to_vec()),
        (0x11C, 1234u32.to_le_bytes().to_vec()),
        (0x120, vec![0; 8]),
        (0x128, 10u16.to_le_bytes().to_vec()),
        (0x138, vec![1]),
        (0x208, vec![0; 4]),
        (0x228, 1.0f32.to_bits().to_le_bytes().to_vec()),
    ] {
        expected[offset..offset + data.len()].copy_from_slice(&data);
    }
    assert_eq!(w.bytes(), expected);
    let mut calls = vec![
        Call::Recycling,
        Call::History(0x0A11_00E8, 500),
        Call::History(0xEA11_0130, 23),
        Call::History(0xEA11_0134, 10),
        Call::History(0xEA11_0137, 2),
        Call::History(0xEA11_0138, 502),
        Call::Post(0xDA6D_D35D, 0),
        Call::Post(0xFA6D_D0BB, 0),
        Call::Post(0x5A6D_D1AA, 0),
        Call::Post(0xDA6D_D2CB, 0),
    ];
    calls.extend(tail(100.0, 0x1234_5678));
    assert_eq!(take(), calls);
}

#[test]
fn averages_divide_unsigned_wrap_radiation_before_division_and_narrow_garbage() {
    let mut w = World::new();
    let sim = w.sim();
    let cycle = w.cycle();
    for (sum, count, radiation) in [
        (u32::MAX, 2, u32::MAX),
        (0x8000_0000, 1, 0x8000_0001),
        (10, u32::MAX, 2),
        (u32::MAX, 0, 99),
        (0, 1, 0),
    ] {
        cycle.set_air_sum(sum);
        cycle.set_treated_water_sum(sum);
        cycle.set_developed(count);
        cycle.set_radiation_sum(radiation);
        sim.set_uncollected(sum);
        w.run();
        let average = sum.checked_div(count).unwrap_or(0);
        let percent = radiation.wrapping_mul(100).checked_div(count).unwrap_or(0);
        assert_eq!(sim.average_air(), average);
        assert_eq!(sim.average_water() as u32, average);
        assert_eq!(sim.average_garbage(), average as u16);
        assert_eq!(cycle.garbage_uncollected(), u8::from(sum != 0));
        let mut expected = vec![
            Call::Recycling,
            Call::History(0x0A11_00E8, average),
            Call::History(0xEA11_0130, average),
            Call::History(0xEA11_0134, u32::from(average as u16)),
            Call::History(0xEA11_0137, radiation),
            Call::History(0xEA11_0138, average.wrapping_add(percent)),
        ];
        expected.extend(tail(100.0, 0x1234_5678));
        assert_eq!(take(), expected);
    }
}

#[test]
fn callbacks_are_observed_by_later_history_and_dirty_map_operations() {
    let mut w = World::new();
    let sim = w.sim();
    let cycle = w.cycle();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.mutate_history = true;
        s.mutate_posts = true;
        s.mutate_recycling = true;
    });
    cycle.set_developed(0);
    cycle.set_air_sum(700);
    cycle.set_radiation_sum(7);
    cycle.set_dirty_radiation(1);
    sim.set_dirty_water(1);
    w.run();
    let mut expected = vec![
        Call::Recycling,
        Call::History(0x0A11_00E8, 100),
        Call::History(0xEA11_0130, 123),
        Call::History(0xEA11_0134, 456),
        Call::History(0xEA11_0137, 999),
        Call::History(0xEA11_0138, 99),
        Call::Post(0xDA6D_D35D, 0),
        Call::Post(0xFA6D_D0BB, 0),
        Call::Post(0xDA6D_D2CB, 0),
    ];
    expected.extend(tail(100.0, 0x1234_5678));
    assert_eq!(take(), expected);
    assert_eq!(sim.average_garbage(), 456);
    assert_eq!(cycle.garbage_uncollected(), 1);
}

#[test]
fn funding_curve_uses_unrounded_st0_and_skips_free_for_null_storage() {
    let mut w = World::new();
    w.department.funding = 0.75;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.storage = 0;
        s.output = 1e40;
    });
    w.run();
    let expected = multiply_float(F80::from_f64(1e40), 0.01).to_f32();
    assert_eq!(w.cycle().efficiency().to_bits(), expected.to_bits());
    assert!(expected.is_finite());
    assert_eq!(&take()[6..], tail(75.0, 0));
}
