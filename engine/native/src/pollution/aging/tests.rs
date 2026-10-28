// SPDX-License-Identifier: GPL-3.0-or-later
//! Monthly facility aging vectors with original property and service calls.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Holder,
    Funding,
    Used(u32),
    Capacity(u32),
    Property(u32),
    Curve(usize, u32),
    Set(u32),
    Position,
    Cell(u32, u32),
    Lot(i32, i32, u32),
    Condition(u32),
}
struct State {
    calls: Vec<Call>,
    used: i32,
    capacity: u32,
    properties: Vec<(u32, f32)>,
    usage_factor: f64,
    funding_factor: f64,
    base: usize,
    lot: *mut u8,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        calls: Vec::new(), used: 0, capacity: 0, properties: Vec::new(),
        usage_factor: 0.0, funding_factor: 0.0, base: 0, lot: core::ptr::null_mut(),
    }) };
}
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take_calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    log(Call::Holder);
    this
}
extern "thiscall" fn funding(this: *mut u8) -> *const f32 {
    log(Call::Funding);
    unsafe { this.add(4).cast() }
}
extern "C" fn used(_holder: *mut u8, id: u32, output: *mut i32) {
    log(Call::Used(id));
    unsafe { output.write_unaligned(STATE.with(|s| s.borrow().used)) };
}
extern "thiscall" fn capacity(_this: *mut u8, id: u32, _occupant: *mut u8, output: *mut u32) {
    log(Call::Capacity(id));
    unsafe { output.write_unaligned(STATE.with(|s| s.borrow().capacity)) };
}
extern "C" fn property(_holder: *mut u8, id: u32, output: *mut f32) {
    log(Call::Property(id));
    STATE.with(|s| {
        if let Some(&(_, value)) = s.borrow().properties.iter().find(|&&(key, _)| key == id) {
            unsafe { output.write_unaligned(value) };
        }
    });
}
extern "C" fn set(_holder: *mut u8, id: u32, value: f32) {
    assert_eq!(id, 0xC910_B4CF);
    log(Call::Set(value.to_bits()));
}
extern "thiscall" fn curve(this: *mut u8, value: f32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let offset = this as usize - s.base;
        s.calls.push(Call::Curve(offset, value.to_bits()));
        match offset {
            0xDC => s.funding_factor,
            0xEC => s.usage_factor,
            _ => panic!("unexpected curve"),
        }
    })
}
extern "thiscall" fn position(_this: *mut u8, output: *mut f32) {
    log(Call::Position);
    unsafe {
        output.write(16.0);
        output.add(1).write(7.0);
        output.add(2).write(-32.0);
    }
}
extern "thiscall" fn cell(_this: *mut u8, x: f32, z: f32, out_x: *mut i32, out_z: *mut i32) {
    log(Call::Cell(x.to_bits(), z.to_bits()));
    unsafe {
        out_x.write(1);
        out_z.write(-2);
    }
}
extern "thiscall" fn lot(_this: *mut u8, x: i32, z: i32, flags: u32) -> *mut u8 {
    log(Call::Lot(x, z, flags));
    STATE.with(|s| s.borrow().lot)
}
extern "thiscall" fn condition(_this: *mut u8, value: u32) {
    log(Call::Condition(value));
}
fn table(slots: &[(usize, usize)]) -> Box<[usize; 100]> {
    let mut t = Box::new([0; 100]);
    for &(slot, f) in slots {
        t[slot / 4] = f;
    }
    t
}
struct World {
    sim: Object,
    occupant: Box<[u32; 1]>,
    funding: Box<[u32; 2]>,
    city: Box<[*mut u8; 1]>,
    lots: Box<[*mut u8; 1]>,
    city_pointer: Box<*mut u8>,
    lots_pointer: Box<*mut u8>,
    lot: Box<[u32; 1]>,
    sentinel: Box<[u32; 3]>,
    node: Box<[u32; 3]>,
    default: Box<f32>,
    // Each vtable must keep its address while other tables are allocated.
    #[allow(clippy::vec_box)]
    _tables: Vec<Box<[usize; 100]>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[(0xD4, capacity as *const () as usize)]),
            table(&[
                (0x18, holder as *const () as usize),
                (0x20, position as *const () as usize),
            ]),
            table(&[(0x28, funding as *const () as usize)]),
            table(&[(0x17C, cell as *const () as usize)]),
            table(&[(0x18, lot as *const () as usize)]),
            table(&[(0x90, condition as *const () as usize)]),
        ];
        let mut w = Self {
            sim: Object::new(0x240),
            occupant: Box::new([tables[1].as_ptr() as u32]),
            funding: Box::new([tables[2].as_ptr() as u32, 0.75f32.to_bits()]),
            city: Box::new([tables[3].as_ptr() as *mut u8]),
            lots: Box::new([tables[4].as_ptr() as *mut u8]),
            city_pointer: Box::new(core::ptr::null_mut()),
            lots_pointer: Box::new(core::ptr::null_mut()),
            lot: Box::new([tables[5].as_ptr() as u32]),
            sentinel: Box::new([0; 3]),
            node: Box::new([0; 3]),
            default: Box::new(0.25),
            _tables: tables,
            _fakes: vec![],
        };
        unsafe { w.sim.ptr().cast::<u32>().write(w._tables[0].as_ptr() as u32) };
        let sim = unsafe { AgingView::from_ptr(w.sim.ptr()) };
        sim.set_department(w.funding.as_mut_ptr().cast());
        let sentinel = w.sentinel.as_mut_ptr() as u32;
        let node = w.node.as_mut_ptr() as u32;
        *w.sentinel = [node, node, 0];
        *w.node = [sentinel, sentinel, w.occupant.as_mut_ptr() as u32];
        STATE.with(|s| {
            *s.borrow_mut() = State {
                calls: vec![],
                used: 50,
                capacity: 100,
                properties: vec![
                    (0xC910_B4CF, 10.0),
                    (0xEA1C_F220, 2.0),
                    (0xEA1C_F221, 20.0),
                    (0xEA1C_F222, 30.0),
                ],
                usage_factor: 3.0,
                funding_factor: 4.0,
                base: w.sim.ptr() as usize,
                lot: w.lot.as_mut_ptr().cast(),
            }
        });
        *w.city_pointer = w.city.as_mut_ptr().cast();
        *w.lots_pointer = w.lots.as_mut_ptr().cast();
        w._fakes = vec![
            fake(0x005F_D450, used as *const () as usize),
            fake(0x005F_D4F0, property as *const () as usize),
            fake(0x005F_E4F0, set as *const () as usize),
            fake(0x005F_4250, curve as *const () as usize),
            fake_data(0x00B4_9BC4, (&mut *w.default as *mut f32).cast()),
            fake_data(0x00B4_3CEC, (&mut *w.city_pointer as *mut *mut u8).cast()),
            fake_data(0x00B4_3D08, (&mut *w.lots_pointer as *mut *mut u8).cast()),
        ];
        w
    }
    fn run(&mut self, usage: u32) {
        let mut head = self.sentinel.as_mut_ptr().cast();
        unsafe { fc4p_006e3d00(self.sim.ptr(), &mut head, usage, 22) };
    }
}

#[test]
fn aging_applies_both_curves_and_prioritizes_condition_four() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.run(11);
    assert_eq!(
        take_calls(),
        [
            Call::Holder,
            Call::Funding,
            Call::Used(11),
            Call::Capacity(22),
            Call::Property(0xC910_B4CF),
            Call::Property(0xEA1C_F220),
            Call::Property(0xEA1C_F221),
            Call::Property(0xEA1C_F222),
            Call::Curve(0xEC, 0.5f32.to_bits()),
            Call::Curve(0xDC, 0.75f32.to_bits()),
            Call::Set(34.0f32.to_bits()),
            Call::Position,
            Call::Cell(16.0f32.to_bits(), (-32.0f32).to_bits()),
            Call::Lot(1, -2, 0),
            Call::Condition(4),
        ]
    );
}

#[test]
fn usage_defaults_unsigned_capacity_and_recycling_ratio() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    for (used, capacity, ratio) in [
        (0, 100, 0.25f32),
        (-1, 100, 0.25),
        (5, 0, 0.25),
        (i32::MAX, u32::MAX, 0.5),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.used = used;
            s.capacity = capacity;
        });
        w.run(11);
        assert!(take_calls().contains(&Call::Curve(0xEC, ratio.to_bits())));
    }
    let sim = unsafe { AgingView::from_ptr(w.sim.ptr()) };
    for (count, total, ratio) in [(0, 500.0, 0.25f32), (2, 1.0, 0.5), (u32::MAX, 2_147_483_648.0, 0.5)] {
        sim.set_recycling_count(count);
        sim.set_recycled(total);
        w.run(0);
        let calls = take_calls();
        assert!(calls.contains(&Call::Curve(0xEC, ratio.to_bits())));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, Call::Used(_) | Call::Capacity(_))));
    }
}

#[test]
fn threshold_boundaries_missing_properties_nan_and_missing_lots() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    for (age, expected) in [
        (19.0, None),
        (20.0, Some(2)),
        (29.0, Some(2)),
        (30.0, Some(4)),
        (f32::NAN, None),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.properties[0].1 = age;
            s.properties[1].1 = 0.0;
        });
        w.run(11);
        let calls = take_calls();
        assert_eq!(
            calls
                .iter()
                .find_map(|c| if let Call::Condition(v) = c { Some(*v) } else { None }),
            expected
        );
        assert_eq!(calls.contains(&Call::Position), expected.is_some());
    }
    STATE.with(|s| s.borrow_mut().properties.clear());
    w.run(11);
    let calls = take_calls();
    assert!(calls.contains(&Call::Set(0)));
    assert!(!calls.contains(&Call::Position));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.properties = vec![(0xC910_B4CF, 99.0)];
        s.lot = core::ptr::null_mut();
    });
    w.run(11);
    let calls = take_calls();
    assert!(calls.contains(&Call::Lot(1, -2, 0)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Condition(_))));
    w.sentinel[0] = w.sentinel.as_ptr() as u32;
    w.run(11);
    assert_eq!(take_calls(), []);
}
