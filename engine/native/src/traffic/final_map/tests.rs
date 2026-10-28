// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, float_environment, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call { Grid(u32), Budget(u32), Curve(u32, u32), Allocate(u32), Construct(u32),
    AddRef, Type(u32), Sender(u32), Object(u32), Post(u32, u32), Release, Finish([u32; 5]) }
#[derive(Default)]
struct State { calls: Vec<Call>, checks: u32, expire: u32, message: u32, curve: u32 }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) { STATE.with(|s| s.borrow_mut().calls.push(c)); }
fn calls() -> Vec<Call> { STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)) }
unsafe extern "thiscall" fn grid(this: *mut u8) -> u32 { log(Call::Grid(unsafe { word(this, 8) })); unsafe { word(this, 4) } }
unsafe extern "thiscall" fn expired(this: *mut u32) -> u32 {
    log(Call::Budget(unsafe { this.add(2).read() }));
    STATE.with(|s| { let mut s = s.borrow_mut(); s.checks += 1; u32::from(s.expire != 0 && s.checks >= s.expire) })
}
unsafe extern "thiscall" fn finish(this: *mut u32) -> u32 {
    log(Call::Finish(unsafe { read_unaligned(this.cast::<[u32; 5]>()) }));
    0x12345678
}
#[unsafe(naked)]
unsafe extern "thiscall" fn finish_fake(_this: *mut u32) -> u32 {
    core::arch::naked_asm!("call {body}", "mov ecx, 0x87654321", "ret", body = sym finish);
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size)); STATE.with(|s| s.borrow().message as *mut u8)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 { log(Call::Construct(this as u32)); this }
extern "thiscall" fn add_ref(_this: *mut u8) { log(Call::AddRef); }
extern "thiscall" fn set_type(_this: *mut u8, id: u32) { log(Call::Type(id)); }
extern "thiscall" fn sender(_this: *mut u8, sender: u32) { log(Call::Sender(sender)); }
extern "thiscall" fn set_object(_this: *mut u8, object: u32) { log(Call::Object(object)); }
extern "thiscall" fn post(_this: *mut u8, message: u32, flag: u32) { log(Call::Post(message, flag)); }
extern "thiscall" fn release(_this: *mut u8) -> u32 { log(Call::Release); 0xFEDCBA98 }
extern "thiscall" fn curve(this: *mut u8, ratio: f32) -> f64 {
    log(Call::Curve(this as u32, ratio.to_bits()));
    let bits = STATE.with(|s| s.borrow().curve);
    if bits != 0 { return f64::from(f32::from_bits(bits)); }
    let value = match ratio.to_bits() {
        0x3F000000 | 0x3F800000 => 1.0,
        0x3FC00000 => f32::from_bits(0x3FA27628),
        0x40000000 => f32::from_bits(0x3FC4EC4F),
        0x40200000 => f32::from_bits(0x401BE5BE),
        _ => f32::from_bits(0x40555555),
    };
    f64::from(value)
}

struct Grid<T> { object: Object, vector: Box<[u32; 1]>, rows: Box<[u32; 3]>, cells: Vec<T> }
impl<T: Copy> Grid<T> {
    fn new(table: &[usize], id: u32, values: &[T]) -> Self {
        let mut object = object(12, table);
        let mut cells = values.to_vec();
        let mut rows = Box::new([cells.as_mut_ptr() as u32, 0, 0]);
        let mut vector = Box::new([rows.as_mut_ptr() as u32]);
        unsafe { store(object.ptr(), 4, vector.as_mut_ptr() as u32); store(object.ptr(), 8, id); }
        Self { object, vector, rows, cells }
    }
    fn stable(&self) { assert_ne!(self.vector[0], 0); assert_ne!(self.rows[0], 0); }
}
struct World {
    sim: Object, scan: Object, air: Grid<u8>, congestion: Grid<u8>, volume: Grid<f32>, ratio: Grid<f32>,
    pacing: Box<[u32; 2]>, flags: Box<u32>, _tables: Vec<Vec<usize>>, _objects: Vec<Object>, _constants: Box<[f32; 4]>, _guards: Vec<Fake>,
}
impl World {
    fn new(volumes: &[f32], ratios: &[f32]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![table(&[(0x60, grid as *const () as usize)]), table(&[(4, add_ref as *const () as usize),
            (8, release as *const () as usize), (0x14, set_type as *const () as usize),
            (0x30, sender as *const () as usize), (0x84, set_object as *const () as usize)]), table(&[(0x10, post as *const () as usize)])];
        let mut sim = Object::new(0xD10); let scan = Object::new(32);
        let mut air = Grid::new(&tables[0], 1, &vec![0xA5; volumes.len()]);
        let mut congestion = Grid::new(&tables[0], 2, &vec![0xA5; volumes.len()]);
        let mut volume = Grid::new(&tables[0], 3, volumes); let mut ratio = Grid::new(&tables[0], 4, ratios);
        let mut message = object(0x2C, &tables[1]); let mut server = object(8, &tables[2]);
        let mut curve = Object::new(16);
        STATE.with(|s| s.borrow_mut().message = message.ptr() as u32);
        let mut pacing = Box::new([1024u32, 0]); let mut flags = Box::new(1u32);
        let mut constants = Box::new([0.0f32, 1.0, 0.1, 12582912.0]);
        let mut guards = vec![fake(0x0068BF80, expired as *const () as usize), fake(0x0068B960, finish_fake as *const () as usize),
            fake(0x005F4250, self::curve as *const () as usize), fake(0x009133DA, allocate as *const () as usize),
            fake(0x009134D6, construct as *const () as usize), fake_data(0x00B4B30C, pacing.as_mut_ptr().cast()),
            fake_data(0x00B4B314, (&mut *flags as *mut u32).cast()), fake_data(0x00B4B2F0, curve.ptr())];
        for (i, address) in [0x00A81054, 0x00A81228, 0x00A8C950, 0x00AB32D8].into_iter().enumerate() {
            guards.push(fake_data(address, unsafe { constants.as_mut_ptr().add(i).cast() }));
        }
        unsafe {
            let v = FinalMapSim::from_ptr(sim.ptr()); v.set_pollution(air.object.ptr()); v.set_congestion(congestion.object.ptr());
            v.set_volume(volume.object.ptr()); v.set_ratio(ratio.object.ptr()); v.set_messages(server.ptr());
            v.set_scale(f32::from_bits(0x42DA924A));
        }
        let mut w = Self { sim, scan, air, congestion, volume, ratio, pacing, flags, _tables: tables, _objects: vec![message, server, curve], _constants: constants, _guards: guards };
        unsafe {
            FinalMapSim::from_ptr(w.sim.ptr()).set_cursor(w.scan.ptr());
            let scan = PassScan::from_ptr(w.scan.ptr()); scan.set_step(1); scan.set_maximum_x(0); scan.set_maximum_z(volumes.len() as i32 - 1);
        }
        w
    }
    fn run(&mut self) -> Registers {
        let mut budget = [11u32, 22];
        unsafe { invoke(fc4p_0070f460 as *const () as usize, self.sim.ptr(), &[budget.as_mut_ptr() as u32], 0, 0, false) }
    }
}

#[test]
fn final_map_air_and_congestion_examples_and_notification_call_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[150.0, 15.0, 25.0, 2.0, 3000.0, 0.0, 40.0, 1.0], &[0.5, 1.0, 1.5, 2.0, 2.5, 2.5, 3.0, 4.0]);
    let before = float_environment();
    let r = w.run();
    assert_eq!((r.eax, r.ecx), (0x12345601, 0x87654321));
    assert_eq!(w.air.cells, [15, 2, 2, 0, 255, 0, 4, 0]);
    assert_eq!(w.congestion.cells, [1, 1, 29, 59, 157, 0, 255, 255]);
    assert_eq!(float_environment(), before);
    let actual = calls();
    assert_eq!(&actual[..4], &[Call::Grid(1), Call::Grid(2), Call::Grid(3), Call::Grid(4)]);
    let curves: Vec<_> = actual.iter().filter(|c| matches!(c, Call::Curve(..))).collect();
    assert_eq!(curves.len(), 7);
    let message = STATE.with(|s| s.borrow().message);
    assert_eq!(&actual[actual.len() - 9..], &[
        Call::Allocate(44), Call::Construct(message), Call::AddRef, Call::Type(0x69247DC7),
        Call::Sender(w.sim.ptr() as u32), Call::Object(w.sim.ptr() as u32), Call::Post(message, 0), Call::Release,
        Call::Finish([11, 22, 9 * 1024, 1024, 0]),
    ]);
    w.volume.stable(); w.ratio.stable();
}

#[test]
fn final_map_timeout_keeps_cursor_pending_and_marks_budget_before_finishing() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[150.0, 15.0], &[1.0, 1.5]);
    *w.flags = 0xA0; *w.pacing = [3, 9];
    STATE.with(|s| s.borrow_mut().expire = 2);
    assert_eq!(w.run().eax, 0x12345600);
    assert_eq!(w.air.cells, [15, 0xA5]);
    assert_eq!(w.congestion.cells, [1, 0xA5]);
    assert_eq!(*w.flags, 0xA1); assert_eq!(*w.pacing, [1024, 0]);
    assert_eq!(calls().last(), Some(&Call::Finish([11, 22, 2048, 1024, 1])));
    STATE.with(|s| s.borrow_mut().expire = 0);
    assert_eq!(w.run().eax as u8, 1);
    assert_eq!(w.air.cells, [15, 2]); assert_eq!(w.congestion.cells, [1, 29]);
}

#[test]
fn final_map_signed_zero_nan_infinity_and_negative_volume_take_original_paths() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[-0.0, f32::NAN, f32::INFINITY, -10.0, 1.0], &[1.0; 5]);
    STATE.with(|s| s.borrow_mut().curve = f32::NAN.to_bits());
    w.run();
    assert_eq!(w.air.cells, [0, 255, 255, 255, 0]);
    assert_eq!(w.congestion.cells, [0, 1, 1, 1, 1]);
    assert_eq!(calls().iter().filter(|c| matches!(c, Call::Curve(..))).count(), 4);
}

#[test]
fn final_map_finished_cursor_still_queries_grids_and_posts_one_message() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[1.0], &[1.0]);
    unsafe { PassScan::from_ptr(w.scan.ptr()).set_state(2); }
    let before = unsafe { bytes(w.scan.ptr(), 32) };
    w.run();
    assert_eq!(w.air.cells, [0xA5]); assert_eq!(w.congestion.cells, [0xA5]);
    assert_eq!(unsafe { bytes(w.scan.ptr(), 32) }, before);
    assert_eq!(calls().iter().filter(|c| matches!(c, Call::Post(..))).count(), 1);
}
