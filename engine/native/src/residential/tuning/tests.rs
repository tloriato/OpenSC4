// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

const PROPERTIES: [(u32, usize, u32); 18] = [
    (0xC929_C433, 0, 0.2f32.to_bits()),
    (0x0929_C439, 4, 0.2f32.to_bits()),
    (0xC929_C652, 8, 25),
    (0x0929_CC30, 10, 30),
    (0x0929_CC36, 12, 35),
    (0xCA02_7307, 14, 40),
    (0xCA02_7325, 16, 80f32.to_bits()),
    (0xCA02_7328, 20, 80f32.to_bits()),
    (0xC929_CC58, 24, 20f32.to_bits()),
    (0x0929_CC65, 28, 40f32.to_bits()),
    (0x2929_CC6B, 32, 60f32.to_bits()),
    (0xC929_CC8D, 36, 20f32.to_bits()),
    (0x6929_CC92, 40, 40f32.to_bits()),
    (0x4929_CC97, 44, 60f32.to_bits()),
    (0x6AB7_0DA7, 48, 2f32.to_bits()),
    (0x6AB7_1654, 52, 1.5f32.to_bits()),
    (0x4AB7_1659, 56, 50f32.to_bits()),
    (0x4AB7_0DAC, 60, 10f32.to_bits()),
];
#[derive(Debug, PartialEq, Eq)]
enum Call {
    Resource,
    Holder,
    Read(u32, u32, usize),
    Insert([u32; 2]),
    Set(usize),
    Load(usize, u32),
    Release,
    Free(usize),
}
struct State {
    calls: Vec<Call>,
    available: bool,
    properties: bool,
    out: usize,
    allocations: Vec<Object>,
}
thread_local! {static STATE:RefCell<State>=const{RefCell::new(State{calls:Vec::new(),available:false,properties:false,out:0,allocations:Vec::new()})};}
fn record(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn resource(this: *mut u8, key: *const u32, iid: u32, out: *mut *mut u8, a: u32, b: u32) -> u32 {
    assert_eq!(unsafe { core::slice::from_raw_parts(key, 3) }, KEY);
    assert_eq!((iid, a, b), (0xA521_60F5, 0, 0));
    record(Call::Resource);
    if STATE.with(|s| s.borrow().available) {
        unsafe { *out = this };
        1
    } else {
        0
    }
}
extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    record(Call::Holder);
    this
}
extern "thiscall" fn release(_this: *mut u8) -> u32 {
    record(Call::Release);
    0
}
fn property(kind: u32, id: u32, out: *mut u8) -> u32 {
    let offset = STATE.with(|s| (out as usize) - s.borrow().out);
    record(Call::Read(kind, id, offset));
    if !STATE.with(|s| s.borrow().properties) {
        return 0;
    }
    let (_, expected, value) = PROPERTIES.iter().find(|p| p.0 == id).copied().unwrap();
    assert_eq!(offset, expected);
    unsafe {
        match kind {
            1 => *out = value as u8,
            2 => out.cast::<u16>().write_unaligned(value as u16),
            4 => out.cast::<u32>().write_unaligned(value),
            100 => out.cast::<u32>().write_unaligned(
                multiply_percent(F80::from_f32(f32::from_bits(value)), 0.01)
                    .to_f32()
                    .to_bits(),
            ),
            _ => unreachable!(),
        }
    };
    1
}
extern "C" fn float(_h: *mut u8, id: u32, out: *mut u8) -> u32 {
    property(4, id, out)
}
extern "C" fn u16_property(_h: *mut u8, id: u32, out: *mut u8) -> u32 {
    property(2, id, out)
}
extern "C" fn u8_property(_h: *mut u8, id: u32, out: *mut u8) -> u32 {
    property(1, id, out)
}
extern "C" fn percent(_h: *mut u8, id: u32, out: *mut u8) -> u32 {
    property(100, id, out)
}
extern "thiscall" fn insert(
    this: *mut u8,
    position: *mut u32,
    point: *const u32,
    _tag: *const u8,
    count: u32,
    stride: u32,
) -> u32 {
    assert_eq!((count, stride), (1, 1));
    let point = unsafe { [point.read_unaligned(), point.add(1).read_unaligned()] };
    record(Call::Insert(point));
    let p = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.allocations.push(Object::new(16));
        s.allocations.last_mut().unwrap().ptr().cast::<u32>()
    });
    unsafe {
        let points = Points::from_ptr(this);
        assert_eq!(position, points.end());
        let len = if points.begin().is_null() { 0 } else { 2 };
        if len > 0 {
            core::ptr::copy_nonoverlapping(points.begin(), p, len);
        }
        p.add(len).write(point[0]);
        p.add(len + 1).write(point[1]);
        points.set_begin(p);
        points.set_end(p.add(len + 2));
        points.set_capacity(p.add(len + 2));
    }
    0
}
extern "thiscall" fn set(this: *mut u8, vector: *mut u8) -> u32 {
    let offset = STATE.with(|s| this as usize - s.borrow().out);
    record(Call::Set(offset));
    unsafe {
        let points = Points::from_ptr(vector);
        assert_eq!(
            core::slice::from_raw_parts(points.begin(), 4),
            [0, 0, 32000f32.to_bits(), 100f32.to_bits()]
        );
        this.cast::<u32>().write_unaligned(0x1234_0000 | offset as u32);
    }
    1
}
extern "thiscall" fn load(this: *mut u8, key: *const u32, id: u32) -> u32 {
    assert_eq!(unsafe { core::slice::from_raw_parts(key, 3) }, KEY);
    let offset = STATE.with(|s| this as usize - s.borrow().out);
    record(Call::Load(offset, id));
    1
}
extern "C" fn free_storage(p: *mut u8) -> u64 {
    record(Call::Free(p as usize));
    0
}

struct Fixture {
    sim: Object,
    out: Object,
    heads: [Object; 2],
    _nodes: Vec<Object>,
    _resource: Object,
    _table: Box<[usize; 8]>,
    _guards: Vec<Fake>,
    _fpu: GameFpu,
}
impl Fixture {
    fn new(education: &[f32], health: &[f32]) -> Self {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.clear();
            s.available = false;
            s.properties = false;
            s.allocations.clear()
        });
        let mut table = Box::new([0usize; 8]);
        table[2] = release as *const () as usize;
        table[3] = resource as *const () as usize;
        table[7] = holder as *const () as usize;
        let mut resource = Object::new(0x20);
        unsafe {
            let p = resource.ptr();
            p.cast::<*const usize>().write(table.as_ptr());
            p.add(4).cast::<*mut u8>().write(p);
            p.add(8).cast::<f32>().write(1.0)
        };
        let guards = vec![
            fake(0x005F_D4F0, float as *const () as usize),
            fake(0x005F_D3F0, u16_property as *const () as usize),
            fake(0x005F_D3C0, u8_property as *const () as usize),
            fake(0x005F_DAC0, percent as *const () as usize),
            fake(0x007C_61A0, insert as *const () as usize),
            fake(0x005F_4480, set as *const () as usize),
            fake(0x005F_4700, load as *const () as usize),
            fake(0x0090_CF63, free_storage as *const () as usize),
            fake_data(0x00B4_3CA8, unsafe { resource.ptr().add(4) }),
            fake_data(0x00A8_1228, unsafe { resource.ptr().add(8) }),
        ];
        let mut f = Self {
            sim: Object::new(0x3D0),
            out: Object::new(0x80),
            heads: [Object::new(12), Object::new(12)],
            _nodes: Vec::new(),
            _resource: resource,
            _table: table,
            _guards: guards,
            _fpu: GameFpu::new(),
        };
        unsafe {
            core::ptr::write_bytes(f.out.ptr(), 0xCD, 0x80);
            core::ptr::write_bytes(f.sim.ptr(), 0xAB, 0x3D0)
        };
        for (i, values) in [education, health].into_iter().enumerate() {
            let head = f.heads[i].ptr();
            let mut previous = head;
            for &value in values {
                let mut n = Object::new(0x40);
                let p = n.ptr();
                unsafe {
                    Node::from_ptr(previous).set_next(p);
                    Ordinance::from_ptr(p).set_multiplier(value)
                };
                previous = p;
                f._nodes.push(n);
            }
            unsafe { Node::from_ptr(previous).set_next(head) };
        }
        unsafe {
            let sim = TuningSim::from_ptr(f.sim.ptr());
            sim.set_education(f.heads[0].ptr());
            sim.set_health(f.heads[1].ptr())
        };
        STATE.with(|s| s.borrow_mut().out = f.out.ptr() as usize);
        f
    }
    fn run(&mut self) {
        unsafe { fc4p_006fc0c0(self.sim.ptr(), self.out.ptr()) }
    }
    fn tuning(&mut self) -> PopulationTuning {
        unsafe { PopulationTuning::from_ptr(self.out.ptr()) }
    }
}

#[test]
fn missing_resource_defaults_preserve_padding_bonuses_and_curves() {
    let mut f = Fixture::new(&[], &[]);
    let before = unsafe { core::slice::from_raw_parts(f.out.ptr(), 0x80).to_vec() };
    let sim_before = unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x3D0).to_vec() };
    f.run();
    let t = f.tuning();
    assert_eq!((t.eq_decay(), t.hq_decay()), (1.0, 1.0));
    assert_eq!(
        (t.low_age(), t.medium_age(), t.high_age(), t.reset_age()),
        (25, 30, 35, 40)
    );
    assert_eq!(
        (t.eq_reset().to_bits(), t.hq_reset().to_bits()),
        (0x3F33_3333, 0x3F33_3333)
    );
    assert_eq!((t.low_eq(), t.medium_eq(), t.high_eq()), (4.0, 6.0, 8.0));
    assert_eq!(
        (t.door_hq(), t.effect_chance(), t.effect_height(), t.effect_delta()),
        (0.0, 0.0, 0.0, 0.0)
    );
    let after = unsafe { core::slice::from_raw_parts(f.out.ptr(), 0x80) };
    assert_eq!(after[15], before[15]);
    assert_eq!(&after[0x24..0x30], &before[0x24..0x30]);
    assert_eq!(&after[0x40..], &before[0x40..]);
    let mut expected = sim_before;
    expected[0x3C0] = 1;
    assert_eq!(unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x3D0) }, expected);
    assert_eq!(take(), [Call::Resource]);
}

#[test]
fn shipped_values_property_order_and_three_curve_rebuilds() {
    let mut f = Fixture::new(&[1.1], &[0.5]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.available = true;
        s.properties = true
    });
    f.run();
    let t = f.tuning();
    assert_eq!(t.eq_decay(), 0.2 * 1.1f32);
    assert_eq!(t.hq_decay(), 0.1);
    assert_eq!(t.eq_reset().to_bits(), 0x3F4C_CCCC);
    assert_eq!(t.hq_reset().to_bits(), 0x3F4C_CCCC);
    assert_eq!((t.low_eq(), t.medium_eq(), t.high_eq()), (20.0, 40.0, 60.0));
    assert_eq!((t.low_hq(), t.medium_hq(), t.high_hq()), (20.0, 40.0, 60.0));
    assert_eq!(
        (t.door_hq(), t.effect_chance(), t.effect_height(), t.effect_delta()),
        (2.0, 1.5, 50.0, 10.0)
    );
    let mut expected = vec![Call::Resource, Call::Holder];
    expected.extend(PROPERTIES.iter().enumerate().map(|(i, &(id, offset, _))| {
        Call::Read(
            match i {
                2..=4 => 2,
                5 => 1,
                6..=7 => 100,
                _ => 4,
            },
            id,
            offset,
        )
    }));
    expected.extend([
        Call::Insert([0, 0]),
        Call::Insert([32000f32.to_bits(), 100f32.to_bits()]),
        Call::Set(0x40),
        Call::Set(0x50),
        Call::Set(0x60),
        Call::Load(0x40, 0x6941_B022),
        Call::Load(0x50, 0xA941_B028),
        Call::Load(0x60, 0xC941_B02E),
        Call::Release,
    ]);
    let vector = STATE.with(|s| s.borrow_mut().allocations.last_mut().unwrap().ptr());
    expected.push(Call::Free(vector as usize));
    assert_eq!(take(), expected);
    assert_eq!(unsafe { *f.out.ptr().add(15) }, 0xCD);
}

#[test]
fn missing_properties_keep_defaults_and_existing_hq_bonus_words() {
    let mut f = Fixture::new(&[], &[]);
    STATE.with(|s| s.borrow_mut().available = true);
    f.run();
    let t = f.tuning();
    assert_eq!(t.eq_reset().to_bits(), 0x3F33_3333);
    assert_eq!(t.low_hq().to_bits(), 0xCDCD_CDCD);
    assert_eq!((t.eq_decay(), t.hq_decay()), (1.0, 1.0));
    assert_eq!(
        unsafe { f.out.ptr().add(0x40).cast::<u32>().read_unaligned() },
        0x1234_0040
    );
    assert_eq!(take().iter().filter(|c| matches!(c, Call::Read(..))).count(), 18);
}

#[test]
fn ordinance_products_keep_extended_range_until_the_final_store() {
    let large = f32::from_bits(0x7180_0000);
    let small = f32::from_bits(0x0D80_0000);
    let mut f = Fixture::new(&[large, large, small, small], &[small, small, large, large]);
    f.run();
    assert_eq!((f.tuning().eq_decay(), f.tuning().hq_decay()), (1.0, 1.0));
    assert_eq!(take(), [Call::Resource]);
}
