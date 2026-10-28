// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration dimensions, stage, slope, utilities, and road requirements at their native widths.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, Vec<u32>);
struct State {
    calls: Vec<Call>,
    objects: [u32; 13],
    td: u32,
    candidate: u32,
    size: [u32; 2],
    zone: u32,
    zone_ok: u32,
    stage: u32,
    wealth_ok: u32,
    slope: f64,
    minimum: f64,
    maximum: f64,
    all_cheat: u32,
    power_cheat: u32,
    water_cheat: u32,
    thresholds: [u32; 4],
    power: u32,
    water: u32,
    fire: u32,
    park: u32,
    target: u32,
    roads: u32,
    storage: u32,
    corner: u32,
    histogram: u32,
    null_storage: bool,
}
impl Default for State {
    fn default() -> Self { Self { calls: Vec::new(), objects: [0; 13], td: 0, candidate: 0, size: [3, 2], zone: 0xBEEF_0002,
        zone_ok: 0xFFFF_0001, stage: 0xDEAD_0004, wealth_ok: 0xFFFF_0001, slope: 0.5, minimum: 0.0, maximum: 1.0,
        all_cheat: 0, power_cheat: 0, water_cheat: 0, thresholds: [255; 4], power: 2, water: 1, fire: 1, park: 1,
        target: 1, roads: 0b0001, storage: 0, corner: 0xFFFF_0001, histogram: 0, null_storage: false } }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn call(this: u32, object: usize, name: &'static str, args: &[u32], f: impl FnOnce(&mut State) -> u32) -> u64 {
    STATE.with(|s| { let s = &mut *s.borrow_mut(); assert_eq!(this, s.objects[object], "{name}"); s.calls.push(Call(name, args.to_vec()));
        0xBADC_0FFE_0000_0000 | u64::from(f(s)) })
}
macro_rules! getter {
    ($name:ident, $object:expr, $value:expr) => {
        extern "thiscall" fn $name(this: u32) -> u64 { call(this, $object, stringify!($name), &[], $value) }
    };
}
getter!(all_cheat, 2, |s| s.all_cheat);
getter!(power_cheat, 2, |s| s.power_cheat);
getter!(water_cheat, 2, |s| s.water_cheat);
getter!(power_threshold, 1, |s| s.thresholds[0]);
getter!(water_threshold, 1, |s| s.thresholds[1]);
getter!(fire_threshold, 1, |s| s.thresholds[2]);
getter!(park_threshold, 1, |s| s.thresholds[3]);
getter!(target, 1, |s| s.target);
getter!(stage, 0, |s| s.stage);
getter!(power_grid, 6, |s| s.objects[10]);
getter!(fire_grid, 8, |s| s.objects[11]);
getter!(park_grid, 9, |s| s.objects[12]);

extern "thiscall" fn capacity(this: u32, candidate: u32) -> u64 {
    STATE.with(|s| { let s = &mut *s.borrow_mut(); assert_eq!((this, candidate), (s.td, s.candidate)); s.calls.push(Call("capacity", vec![])); });
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn size(this: u32, width: u32, depth: u32) -> u64 {
    call(this, 0, "size", &[], |s| { unsafe { store(width, 0, s.size[0]); store(depth, 0, s.size[1]); } 0 })
}
extern "thiscall" fn zone(this: u32, x: u32, z: u32) -> u64 { call(this, 3, "zone", &[x, z], |s| s.zone) }
extern "thiscall" fn zone_ok(this: u32, zone: u32) -> u64 { call(this, 0, "zone_ok", &[zone], |s| s.zone_ok) }
extern "thiscall" fn wealth_ok(this: u32, wealth: u32) -> u64 { call(this, 0, "wealth_ok", &[wealth], |s| s.wealth_ok) }
fn floating(this: u32, object: usize, name: &'static str, f: impl FnOnce(&State) -> f64) -> f64 {
    STATE.with(|s| { let s = &mut *s.borrow_mut(); assert_eq!(this, s.objects[object]); s.calls.push(Call(name, vec![])); f(s) })
}
extern "thiscall" fn lot_slope(this: u32) -> f64 { floating(this, 4, "lot_slope", |s| s.slope) }
extern "thiscall" fn area_slope(this: u32, rectangle: u32, flag: u32) -> f64 {
    assert_eq!(flag, 0); STATE.with(|s| assert_eq!(rectangle, s.borrow().candidate + 0x38));
    floating(this, 5, "area_slope", |s| s.slope)
}
extern "thiscall" fn minimum(this: u32) -> f64 { floating(this, 0, "minimum", |s| s.minimum) }
extern "thiscall" fn maximum(this: u32) -> f64 { floating(this, 0, "maximum", |s| s.maximum) }
extern "thiscall" fn power(this: u32, x: u32, z: u32) -> u64 { call(this, 10, "power", &[x, z], |s| s.power) }
extern "thiscall" fn water(this: u32, x: u32, z: u32) -> u64 { call(this, 7, "water", &[x, z], |s| s.water) }
extern "thiscall" fn fire(this: u32, x: u32, z: u32) -> u64 { call(this, 11, "fire", &[x, z], |s| s.fire) }
extern "thiscall" fn park(this: u32, x: u32, z: u32) -> u64 { call(this, 12, "park", &[x, z], |s| s.park) }
extern "thiscall" fn required(this: u32, output: u32) -> u64 { call(this, 0, "required", &[], |s| {
    unsafe { store(output, 0, if s.null_storage { 0 } else { s.storage }); } 0 }) }
extern "thiscall" fn bit(_this: u32, output: u32, index: u32) -> u64 {
    STATE.with(|s| { let s = &mut *s.borrow_mut(); s.calls.push(Call("bit", vec![index]));
        unsafe { store(output, 0, s.storage); store(output, 4, 1u32 << index); }
    }); 0xBADC_0FFE_0000_0000 | u64::from(output)
}
extern "thiscall" fn corner(this: u32, configuration: u32, candidate: u32) -> u64 {
    STATE.with(|s| { let s = &mut *s.borrow_mut(); assert_eq!((this, configuration, candidate), (s.td, s.objects[0], s.candidate));
        s.calls.push(Call("corner", vec![])); 0xBADC_0FFE_0000_0000 | u64::from(s.corner) })
}
extern "C" fn free(p: u32) -> u64 {
    STATE.with(|s| { let s = &mut *s.borrow_mut(); assert_eq!(p, s.storage); let count = unsafe { word(s.histogram, 21 * 4) };
        s.calls.push(Call("free", vec![count, unsafe { *((s.candidate + 0xA0) as *const u8) } as u32])); });
    0xBADC_0FFE_1234_5678
}

struct World {
    td: Object,
    context: Object,
    candidate: Object,
    histogram: Box<[u32; 28]>,
    objects: Box<[u32; 13]>,
    tables: [Box<[usize; 64]>; 13],
    storage: Box<u32>,
    globals: Box<[u32; 2]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(lot: bool) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self { td: Object::new(0x180), context: Object::new(0xA0), candidate: Object::new(0xA4), histogram: Box::new([u32::MAX; 28]),
            objects: Box::new([0; 13]), tables: core::array::from_fn(|_| Box::new([0; 64])), storage: Box::new(1), globals: Box::new([0; 2]), _fakes: Vec::new() };
        for (object, slot, function) in [(0, 0x1C, size as *const () as usize), (0, 0x3C, zone_ok as *const () as usize),
            (0, 0x2C, stage as *const () as usize), (0, 0x40, wealth_ok as *const () as usize), (0, 0x24, minimum as *const () as usize),
            (0, 0x20, maximum as *const () as usize), (0, 0x54, required as *const () as usize), (1, 0x14, target as *const () as usize),
            (1, 0x4C, power_threshold as *const () as usize), (1, 0x50, water_threshold as *const () as usize),
            (1, 0x54, fire_threshold as *const () as usize), (1, 0x58, park_threshold as *const () as usize),
            (2, 0xE0, all_cheat as *const () as usize), (2, 0xDC, power_cheat as *const () as usize), (2, 0xD8, water_cheat as *const () as usize),
            (3, 0x30, zone as *const () as usize), (4, 0x48, lot_slope as *const () as usize), (5, 0x74, area_slope as *const () as usize),
            (6, 0x8C, power_grid as *const () as usize), (7, 0x2C, water as *const () as usize), (8, 0x14, fire_grid as *const () as usize),
            (9, 0x38, park_grid as *const () as usize), (10, 0x1C, power as *const () as usize), (11, 0x1C, fire as *const () as usize), (12, 0x1C, park as *const () as usize)] {
            w.tables[object][slot / 4] = function;
        }
        for i in 0..13 { w.objects[i] = w.tables[i].as_ptr() as u32; }
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) }; td.set_developer(w.object(1)); td.set_building_development(w.object(2)); td.set_debug_context(w.context.ptr() as u32);
        unsafe { for (offset, object) in [(0x3C, 3), (0x28, 5), (0x34, 6), (0x30, 7)] { store(td.as_ptr() as u32, offset, w.object(object)); }
            store(w.context.ptr() as u32, 0x84, w.histogram.as_ptr() as u32); }
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) }; c.set_x(10); c.set_z(20); c.set_width(3); c.set_depth(2);
        c.set_stage(3); c.set_wealth(2); if lot { c.set_lot(w.object(4) as *mut u8); }
        unsafe { *((c.as_ptr() as u32 + 0xA0) as *mut u8) = 0x7F; }
        STATE.with(|s| { let s = &mut *s.borrow_mut(); s.objects = core::array::from_fn(|i| w.object(i)); s.td = td.as_ptr() as u32;
            s.candidate = c.as_ptr() as u32; s.storage = &*w.storage as *const u32 as u32; s.histogram = w.histogram.as_ptr() as u32; });
        *w.globals = [w.object(8), w.object(9)];
        w._fakes.push(fake_data(0x00B4_3D78, w.globals.as_mut_ptr().cast())); w._fakes.push(fake_data(0x00B4_3D60, w.globals.as_mut_ptr().wrapping_add(1).cast()));
        for (address, function) in [(0x0070_55B0, capacity as *const () as usize), (0x0047_7A40, bit as *const () as usize),
            (0x0070_5DA0, corner as *const () as usize), (0x0090_CF63, free as *const () as usize)] { w._fakes.push(fake(address, function)); }
        w
    }
    fn object(&self, i: usize) -> u32 { self.objects.as_ptr().wrapping_add(i) as u32 }
    fn check(&mut self, flag: u32, expected: u32, names: &[&str]) -> Vec<Call> {
        *self.storage = STATE.with(|s| s.borrow().roads);
        let before = snapshot(self.candidate.ptr(), 0xA4); let td_before = snapshot(self.td.ptr(), 0x180);
        assert_eq!(run(0x0070_6500, Conv::Thiscall, self.td.ptr() as u32, &[self.object(0), self.candidate.ptr() as u32, flag]).0, expected);
        let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
        assert_eq!(calls.iter().map(|c| c.0).collect::<Vec<_>>(), names);
        let mut histogram = [u32::MAX; 28]; histogram[if expected == 0 { 5 } else { expected as usize }] = 0;
        assert_eq!(*self.histogram, histogram);
        let mut expected_bytes = before;
        let corner_written = STATE.with(|s| { let s = s.borrow(); expected == 27 && s.target != 3 && s.roads & 7 != 0 });
        if corner_written { expected_bytes[0xA0] = 1; }
        assert_eq!(snapshot(self.candidate.ptr(), 0xA4), expected_bytes); assert_eq!(snapshot(self.td.ptr(), 0x180), td_before);
        calls
    }
}
const PREFIX: &[&str] = &["capacity", "size", "zone", "zone_ok", "stage", "wealth_ok", "lot_slope", "minimum", "maximum", "all_cheat"];
const PASS: &[&str] = &["capacity", "size", "zone", "zone_ok", "stage", "wealth_ok", "lot_slope", "minimum", "maximum", "all_cheat",
    "power_threshold", "water_threshold", "fire_threshold", "park_threshold", "target", "required", "bit", "corner", "free"];

#[test]
fn dimensions_zone_and_stage_rejections_use_original_counter_rows() {
    let _fpu = GameFpu::new();
    for size in [[4, 2], [3, 3], [259, 2]] { let mut w = World::new(true); STATE.with(|s| s.borrow_mut().size = size); w.check(0, 0, &PREFIX[..2]); }
    let mut w = World::new(true); STATE.with(|s| s.borrow_mut().zone_ok = 0xFFFF_0000); w.check(0, 6, &PREFIX[..4]);
    for (matching, stage, expected) in [(true, 3, 12), (true, 4, 27), (false, 3, 27), (false, 2, 12)] {
        let mut w = World::new(true); STATE.with(|s| { let s = &mut *s.borrow_mut(); s.wealth_ok = u32::from(matching); s.stage = 0xFFFF_0000 | stage; });
        w.check(0, expected, if expected == 12 { &PREFIX[..6] } else { PASS });
    }
}

#[test]
fn slope_sources_stores_boundaries_and_nan_behave_like_x87() {
    let _fpu = GameFpu::new();
    let _machine = crate::tract_developer::testing::PreserveMachine::new();
    for (slope, minimum, maximum, expected) in [(-0.1, 0.0, 1.0, 10), (1.1, 0.0, 1.0, 10),
        (0.0, 0.0, 1.0, 27), (1.0, 0.0, 1.0, 27), (f64::NAN, 0.0, 1.0, 27), (0.5, f64::NAN, f64::NAN, 27),
        (1.0 + 1.0e-8, 0.0, 1.0, 27)] {
        let mut w = World::new(true); STATE.with(|s| { let s = &mut *s.borrow_mut(); s.slope = slope; s.minimum = minimum; s.maximum = maximum; });
        w.check(0, expected, if expected == 10 { &PREFIX[..9] } else { PASS });
    }
    let mut w = World::new(false); let mut names = PASS.to_vec(); names[6] = "area_slope"; w.check(0, 27, &names);
}

#[test]
fn utility_coverage_failure_order_low_widths_and_cheat_bypasses() {
    let _fpu = GameFpu::new();
    let utility_names = ["power_threshold", "power_cheat", "power_grid", "power", "water_threshold", "water_cheat", "water",
        "fire_threshold", "fire_grid", "fire", "park_threshold", "park_grid", "park"];
    for (which, value, outcome, through) in [(0, 0xFFFF_0001, 26, 4), (1, 0xFFFF_0000, 25, 7),
        (2, 0xFFFF_0000, 23, 10), (3, 0xFFFF_0000, 24, 13), (3, 0xFFFF_0100, 27, 13)] {
        let mut w = World::new(true); STATE.with(|s| { let s = &mut *s.borrow_mut(); s.thresholds = [0xFFFF_0003; 4];
            match which { 0 => s.power = value, 1 => s.water = value, 2 => s.fire = value, _ => s.park = value } });
        let mut names = PREFIX.to_vec(); names.extend(&utility_names[..through]);
        if outcome == 27 { names.extend(&PASS[14..]); }
        let calls = w.check(0, outcome, &names);
        for c in calls.iter().filter(|c| ["zone", "power", "water", "fire", "park"].contains(&c.0)) { assert_eq!(c.1, [10, 20]); }
    }
    let mut w = World::new(true); STATE.with(|s| { let s = &mut *s.borrow_mut(); s.all_cheat = 0xFFFF_0001; s.thresholds = [0; 4]; });
    let mut names = PREFIX.to_vec(); names.extend(&PASS[14..]); w.check(0, 27, &names);
    let mut w = World::new(true); STATE.with(|s| { let s = &mut *s.borrow_mut(); s.thresholds = [0, 0, 255, 255]; s.power_cheat = 1; s.water_cheat = 1; });
    let mut names = PREFIX.to_vec(); names.extend(["power_threshold", "power_cheat", "water_threshold", "water_cheat", "fire_threshold", "park_threshold"]); names.extend(&PASS[14..]);
    w.check(0, 27, &names);
}

#[test]
fn required_roads_look_at_only_three_bits_and_free_after_failure_counter() {
    let _fpu = GameFpu::new();
    for (roads, flag, corner, outcome) in [(0, 1u32, 1, 21), (8, 1, 1, 21), (8, 0x100, 1, 27),
        (0, 0, 0, 21), (4, 1, 1, 27), (1, 0, 0xFFFF_0000, 21)] {
        let mut w = World::new(true); STATE.with(|s| { let s = &mut *s.borrow_mut(); s.roads = roads; s.corner = corner; });
        let mut names = PASS[..16].to_vec(); let bits = if roads & 1 != 0 { 1 } else if roads & 2 != 0 { 2 } else { 3 };
        names.extend(core::iter::repeat_n("bit", bits)); if flag as u8 == 0 || roads & 7 != 0 { names.push("corner"); } names.push("free");
        let calls = w.check(flag, outcome, &names);
        assert_eq!(calls.last(), Some(&Call("free", vec![if outcome == 21 { 0 } else { u32::MAX }, if outcome == 27 && roads & 7 != 0 { 1 } else { 0x7F }])));
    }
    let mut w = World::new(true); STATE.with(|s| s.borrow_mut().target = 3); let mut names = PREFIX.to_vec(); names.extend(&PASS[10..15]); w.check(1, 27, &names);
    let mut w = World::new(true); STATE.with(|s| s.borrow_mut().null_storage = true); w.check(0, 27, &PASS[..PASS.len() - 1]);
}
