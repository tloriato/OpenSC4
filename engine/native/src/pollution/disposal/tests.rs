// SPDX-License-Identifier: GPL-3.0-or-later
//! Disposal vectors moved from core pollution, including the plant exhaustion and float-bias cases.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Usage,
    Tiles,
    Curve(i32),
    Supply(i32),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const {RefCell::new(Vec::new())}; }
fn log(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}
fn calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
#[repr(C)]
struct Landfill {
    table: *const usize,
    used: i32,
    capacity: i32,
    tiles: i32,
}
extern "thiscall" fn usage(this: *mut Landfill, used: *mut i32, capacity: *mut i32) {
    log(Call::Usage);
    unsafe {
        used.write((*this).used);
        capacity.write((*this).capacity)
    };
}
extern "thiscall" fn tiles(this: *mut Landfill) -> i32 {
    log(Call::Tiles);
    unsafe { (*this).tiles }
}
extern "thiscall" fn curve(_this: *mut u8, input: f32) -> f64 {
    log(Call::Curve(input as i32));
    f64::from(input) * 5000.0
}
extern "thiscall" fn supply(this: *mut u8, amount: i32) {
    log(Call::Supply(amount));
    unsafe {
        let sim = DisposalView::from_ptr(this);
        sim.set_supply(sim.supply().wrapping_add(amount));
    }
}
struct World {
    sim: Object,
    landfill: Box<Landfill>,
    curve_pointer: Box<*mut u8>,
    limit: Box<i32>,
    _landfill_table: Box<[usize; 12]>,
    _sim_table: Box<[usize; 30]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        calls();
        let mut lt = Box::new([0; 12]);
        lt[0x28 / 4] = usage as *const () as usize;
        lt[0x2C / 4] = tiles as *const () as usize;
        let mut st = Box::new([0; 30]);
        st[0x74 / 4] = supply as *const () as usize;
        let mut w = Self {
            sim: Object::new(0x240),
            landfill: Box::new(Landfill {
                table: lt.as_ptr(),
                used: 0,
                capacity: 0,
                tiles: 0,
            }),
            curve_pointer: Box::new(0x1234 as *mut u8),
            limit: Box::new(2000),
            _landfill_table: lt,
            _sim_table: st,
            _fakes: vec![],
        };
        unsafe { w.sim.ptr().cast::<u32>().write(w._sim_table.as_ptr() as u32) };
        let sim = w.view();
        sim.set_landfill((&mut *w.landfill as *mut Landfill).cast());
        sim.set_efficiency(1.0);
        sim.set_decomposed(2000);
        w._fakes = vec![
            fake(0x005F_4250, curve as *const () as usize),
            fake_data(0x00B4_9BC0, (&mut *w.curve_pointer as *mut *mut u8).cast()),
            fake_data(0x00B4_9B70, (&mut *w.limit as *mut i32).cast()),
        ];
        w
    }
    fn view(&mut self) -> DisposalView {
        unsafe { DisposalView::from_ptr(self.sim.ptr()) }
    }
    fn run(&mut self, amount: f32, incinerator: &mut u32, energy: &mut u32) -> F80 {
        let args = [
            amount.to_bits(),
            incinerator as *mut u32 as u32,
            energy as *mut u32 as u32,
        ];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_006e3820 as *const () as usize,
            self.sim.ptr() as u32,
            &args,
        );
        call.take_st0 = 1;
        unsafe { call_raw(&mut call) };
        call.st0
    }
}

#[test]
fn plant_exhaustion_consumes_nothing_and_energy_runs_before_incinerators() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let (mut inc, mut energy) = (0, 50);
    assert_eq!(w.run(100.0, &mut inc, &mut energy).to_f32(), 0.0);
    assert_eq!(energy, 0);
    assert_eq!(calls(), [Call::Usage, Call::Tiles, Call::Curve(0)]);
    energy = 150;
    assert_eq!(w.run(100.0, &mut inc, &mut energy).to_f32(), 100.0);
    assert_eq!(energy, 50);
    assert_eq!(w.view().energy(), 100);
    assert_eq!(calls(), []);
    inc = 150;
    energy = 100;
    assert_eq!(w.run(100.0, &mut inc, &mut energy).to_f32(), 100.0);
    assert_eq!((energy, inc), (0, 50));
    assert_eq!(w.view().incinerated(), 100);
    assert_eq!(calls(), []);
}

#[test]
fn disposal_floor_and_unsigned_remaining_capacity_vectors() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    for (amount, capacity, expected_remaining, expected_taken) in [
        (9.99, 10, 1, 9.99),
        (3.0, 10, 7, 3.0),
        (5_000_000.0, 8_000_000, 3_402_848, 5_000_000.0),
        (2_147_483_648.0, 2_147_483_649, 0, 0.0),
        (16_777_216.0, 16_777_217, 6_291_457, 16_777_216.0),
        (4_294_967_296.0, u32::MAX, 0, 0.0),
    ] {
        let (mut inc, mut energy) = (capacity, 0);
        assert_eq!(w.run(amount, &mut inc, &mut energy).to_f32(), expected_taken);
        assert_eq!(inc, expected_remaining);
    }
}

#[test]
fn export_and_decomposition_keep_signed_and_unsigned_counter_rules() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let (mut inc, mut energy) = (0, 0);
    w.view().set_export_capacity(50);
    assert_eq!(w.run(100.0, &mut inc, &mut energy).to_f32(), 50.0);
    assert_eq!(w.view().exported(), 50);
    assert_eq!(calls(), [Call::Usage, Call::Tiles, Call::Curve(0)]);
    w.view().set_decomposed(1990);
    assert_eq!(w.run(15.5, &mut inc, &mut energy).to_f32(), 10.0);
    assert_eq!(w.view().decomposed(), 2000);
    calls();
    w.view().set_decomposed(1990);
    assert_eq!(w.run(0.5, &mut inc, &mut energy).to_f32(), 0.5);
    assert_eq!(w.view().decomposed(), 1990);
    calls();
    w.view().set_exported(i64::MAX);
    w.view().set_export_capacity(1);
    assert_eq!(w.run(1.0, &mut inc, &mut energy).to_f32(), 1.0);
    assert_eq!(w.view().exported(), i64::MIN);
    assert_eq!(calls(), []);
    w.view().set_export_capacity(i32::MIN);
    w.view().set_exported(1);
    assert_eq!(w.run(2.0, &mut inc, &mut energy).to_f32(), 2.0);
    assert_eq!(w.view().exported(), 3);
    assert_eq!(calls(), []);
}

#[test]
fn landfill_limits_use_bias_product_signed_room_and_supply() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let (mut inc, mut energy) = (0, 0);
    w.landfill.capacity = 10_000;
    w.landfill.tiles = 1;
    assert_eq!(w.run(7000.0, &mut inc, &mut energy).to_f32(), 5000.0);
    assert_eq!(w.view().supply(), 5000);
    assert_eq!(calls(), [Call::Usage, Call::Tiles, Call::Curve(1), Call::Supply(5000)]);
    w.landfill.used = 20_000;
    w.view().set_supply(0);
    assert_eq!(w.run(10.0, &mut inc, &mut energy).to_f32(), 0.0);
    calls();
    w.landfill.used = 0;
    w.landfill.capacity = 10_000_000;
    for (tiles, supply, taken, total) in [
        (1000, 0, 4_597_152.0, 4_597_152),
        (839, 0, 4_194_652.0, 4_194_652),
        (838, 0, 4_190_000.0, 4_190_000),
        (1000, 100_000, 4_497_152.0, 4_597_152),
    ] {
        w.landfill.tiles = tiles;
        w.view().set_supply(supply);
        assert_eq!(w.run(6_000_000.0, &mut inc, &mut energy).to_f32(), taken);
        assert_eq!(w.view().supply(), total);
        calls();
    }
    w.view().set_supply(0);
    w.landfill.tiles = 1;
    assert_eq!(w.run(5.5, &mut inc, &mut energy).to_f32(), 5.5);
    assert_eq!(calls(), [Call::Usage, Call::Tiles, Call::Curve(1), Call::Supply(6)]);
}

#[test]
fn nonpositive_and_nan_offers_write_nothing_and_landfill_reloads_the_saved_total() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let (mut inc, mut energy) = (0x1234, 0x5678);
    for value in [0.0, -0.0, -1.0, f32::NEG_INFINITY, f32::NAN] {
        let before = unsafe { std::slice::from_raw_parts(w.sim.ptr(), 0x240) }.to_vec();
        assert_eq!(w.run(value, &mut inc, &mut energy), F80::ZERO);
        assert_eq!(unsafe { std::slice::from_raw_parts(w.sim.ptr(), 0x240) }, before);
        assert_eq!((inc, energy), (0x1234, 0x5678));
        assert_eq!(calls(), []);
    }
    // Export retains the exact integer load until the landfill path reloads the saved total.
    w.view().set_export_capacity(16_777_217);
    inc = 0;
    energy = 0;
    w.landfill.capacity = 0;
    // The landfill path reloads the saved single-precision total.
    assert_eq!(w.run(33_554_432.0, &mut inc, &mut energy).to_f64(), 16_777_216.0);
    assert_eq!(w.view().exported(), 16_777_216);
}

#[test]
fn return_wrapper_preserves_the_float_environment_and_an_existing_x87_value() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let (mut inc, mut energy) = (0, 10);
    let sentinel = 123.25f64;
    let (mut before_mxcsr, mut after_mxcsr) = (0u32, 0u32);
    // SAFETY: the sentinel stays below the port's own return value on the x87 stack.
    unsafe {
        core::arch::asm!("stmxcsr [{mxcsr}]", "fld qword ptr [{value}]",
            mxcsr = in(reg) &mut before_mxcsr, value = in(reg) &sentinel, out("st(0)") _);
    }
    let before_top = crate::x87::status_word() & 0x3800;
    let result = w.run(5.5, &mut inc, &mut energy);
    let after_top = crate::x87::status_word() & 0x3800;
    let mut restored = 0.0f64;
    // SAFETY: the port's return value was popped by call_raw; only the sentinel remains.
    unsafe {
        core::arch::asm!("fstp qword ptr [{value}]", "stmxcsr [{mxcsr}]",
            value = in(reg) &mut restored, mxcsr = in(reg) &mut after_mxcsr, out("st(0)") _);
    }
    assert_eq!(result.to_f32(), 5.5);
    assert_eq!(before_top, after_top);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert_eq!(before_mxcsr, after_mxcsr);
    assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
}
