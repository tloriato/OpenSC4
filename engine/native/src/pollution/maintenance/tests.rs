// SPDX-License-Identifier: GPL-3.0-or-later
//! Capacity and cost vectors run against live object layouts and recorded game calls.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Holder,
    Unsigned(u32),
    Base,
    Active,
    Condition,
    Property(u32),
    New,
    Set,
    Evaluate(u32),
    Free(u32),
    Cost(usize),
    Key(usize, u32),
    Item,
    Charge(i64),
    Remember(usize, u32, i64),
}
struct State {
    calls: Vec<Call>,
    nominal: Option<u32>,
    base: Option<i64>,
    active: bool,
    condition: Option<f32>,
    property: bool,
    gain: f64,
    storage: u32,
    has_key: bool,
    has_item: bool,
    item_previous: Option<i64>,
    previous: i64,
    cost: Option<i64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            nominal: Some(1000),
            base: Some(1000),
            active: true,
            condition: Some(50.0),
            property: true,
            gain: 0.75,
            storage: 0x1234,
            has_key: true,
            has_item: true,
            item_previous: None,
            previous: 200,
            cost: Some(600),
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

#[repr(C)]
struct Occupant {
    table: *const usize,
    holder: *mut u8,
}
#[repr(C)]
struct Budget {
    table: *const usize,
    item: *mut u8,
}
extern "thiscall" fn holder(this: *mut Occupant) -> *mut u8 {
    log(Call::Holder);
    unsafe { (*this).holder }
}
extern "thiscall" fn active(_this: *mut u8, _occupant: *mut u8) -> u32 {
    log(Call::Active);
    STATE.with(|s| u32::from(s.borrow().active))
}
extern "C" fn get_unsigned(_holder: *mut u8, id: u32, output: *mut u32) -> u64 {
    log(Call::Unsigned(id));
    STATE.with(|s| {
        if let Some(value) = s.borrow().nominal {
            unsafe { output.write_unaligned(value) };
            0xDEAD_BEEF_1122_3301
        } else {
            0xDEAD_BEEF_1122_3300
        }
    })
}
extern "C" fn get_base(_holder: *mut u8, id: u32, output: *mut i64) -> u64 {
    assert_eq!(id, 0x2A58_D350);
    assert_eq!(unsafe { output.read_unaligned() }, 0);
    log(Call::Base);
    STATE.with(|s| {
        if let Some(value) = s.borrow().base {
            unsafe { output.write_unaligned(value) };
            0x8765_4321_AABB_CC01
        } else {
            0x8765_4321_AABB_CC00
        }
    })
}
extern "C" fn get_condition(_holder: *mut u8, id: u32, output: *mut f32) -> u64 {
    assert_eq!(id, 0xC910_B4CF);
    assert_eq!(unsafe { output.read_unaligned() }.to_bits(), 0);
    log(Call::Condition);
    STATE.with(|s| {
        if let Some(value) = s.borrow().condition {
            unsafe { output.write_unaligned(value) };
            0x9876_5432_4433_2201
        } else {
            0x9876_5432_4433_2200
        }
    })
}
extern "thiscall" fn property(_this: *mut u8, id: u32) -> u64 {
    log(Call::Property(id));
    registers(
        STATE.with(|s| if s.borrow().property { 0x1234 } else { 0 }),
        0x6543_210F,
    )
}
extern "thiscall" fn new_curve(curve: *mut u32) -> u64 {
    log(Call::New);
    unsafe { core::ptr::write_bytes(curve, 0, 4) };
    0
}
extern "thiscall" fn set_curve(curve: *mut u32, property: u32) -> u64 {
    assert_eq!(property, 0x1234);
    log(Call::Set);
    STATE.with(|s| unsafe {
        let s = s.borrow();
        curve.write(s.storage);
        curve.add(2).cast::<f64>().write_unaligned(s.gain);
    });
    0xFACE_1234_1111_2222
}
extern "thiscall" fn output_body(curve: *mut u32, x: f32) -> f64 {
    log(Call::Evaluate(x.to_bits()));
    unsafe { curve.add(2).cast::<f64>().read_unaligned() }
}
#[unsafe(naked)]
unsafe extern "C" fn output() {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}",
        "mov edx, 0xcafe4321", "mov eax, 0xaabbccdd", "ret 4", body = sym output_body);
}
extern "C" fn free(data: u32) -> u64 {
    log(Call::Free(data));
    0xD00D_5678_1234_5600
}
extern "thiscall" fn cost(_this: *mut u8, occupant: *mut u8, output: *mut i64) -> u32 {
    log(Call::Cost(occupant as usize));
    assert_eq!(unsafe { output.read_unaligned() }, 0);
    STATE.with(|s| {
        if let Some(value) = s.borrow().cost {
            unsafe { output.write_unaligned(value) };
            1
        } else {
            0
        }
    })
}
extern "thiscall" fn key(_this: *mut u8, holder: *mut u8, id: u32, key: *mut u32) -> u32 {
    log(Call::Key(holder as usize, id));
    assert_eq!(unsafe { key.add(7).read_unaligned() }, 0);
    STATE.with(|s| {
        let s = s.borrow();
        if !s.has_key {
            return 0;
        }
        unsafe {
            key.add(4).cast::<i64>().write_unaligned(s.previous);
            key.add(6).write(42);
        }
        1
    })
}
extern "thiscall" fn item(this: *mut Budget, key: *mut u32) -> *mut u8 {
    log(Call::Item);
    assert_eq!(unsafe { key.add(6).read_unaligned() }, 42);
    STATE.with(|s| {
        if let Some(previous) = s.borrow().item_previous {
            unsafe { key.add(4).cast::<i64>().write_unaligned(previous) };
        }
        if s.borrow().has_item {
            unsafe { (*this).item }
        } else {
            core::ptr::null_mut()
        }
    })
}
extern "thiscall" fn charge(_this: *mut u8, amount: i64) {
    log(Call::Charge(amount));
}
extern "thiscall" fn remember(_this: *mut u8, holder: *mut u8, id: u32, amount: i64) {
    log(Call::Remember(holder as usize, id, amount));
}
fn table(slots: &[(usize, usize)]) -> Box<[usize; 70]> {
    let mut table = Box::new([0; 70]);
    for &(slot, function) in slots {
        table[slot / 4] = function;
    }
    table
}
struct World {
    sim: Object,
    occupant: Box<Occupant>,
    holder: Box<[usize; 1]>,
    manager: Box<[usize; 1]>,
    // Tables have stable addresses even when the owning vector moves.
    #[allow(clippy::vec_box)]
    _tables: Vec<Box<[usize; 70]>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![
            table(&[(0x18, holder as *const () as usize)]),
            table(&[(0x24, property as *const () as usize)]),
            table(&[(0x7C, active as *const () as usize)]),
            table(&[(0x104, cost as *const () as usize)]),
        ];
        let mut holder = Box::new([tables[1].as_ptr() as usize]);
        let mut manager = Box::new([tables[2].as_ptr() as usize]);
        let occupant = Box::new(Occupant {
            table: tables[0].as_ptr(),
            holder: holder.as_mut_ptr().cast(),
        });
        let mut sim = Object::new(0x240);
        let view = unsafe { CapacityView::from_ptr(sim.ptr()) };
        view.set_occupants(manager.as_mut_ptr().cast());
        view.set_efficiency(0.8);
        unsafe { sim.ptr().cast::<usize>().write(tables[3].as_ptr() as usize) };
        let fakes = [
            (0x005F_D480, get_unsigned as *const () as usize),
            (0x005F_D4B0, get_base as *const () as usize),
            (0x005F_D4F0, get_condition as *const () as usize),
            (0x007D_0410, new_curve as *const () as usize),
            (0x005F_4620, set_curve as *const () as usize),
            (0x005F_4250, output as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        Self {
            sim,
            occupant,
            holder,
            manager,
            _tables: tables,
            _fakes: fakes,
        }
    }
    fn capacity(&mut self) -> (u64, u32) {
        let mut output = 0xFFFF_FFFF;
        let result = unsafe {
            fc4p_006e4300(
                self.sim.ptr(),
                17,
                (&mut *self.occupant as *mut Occupant).cast(),
                &mut output,
            )
        };
        (result, output)
    }
    fn maintenance(&mut self) -> (u64, i64) {
        let mut output = i64::MIN;
        let result = unsafe { fc4p_006e4530((&mut *self.occupant as *mut Occupant).cast(), &mut output) };
        (result, output)
    }
}

#[test]
fn current_capacity_rounding_and_short_circuits() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    assert_eq!(w.capacity(), (0xD00D_5678_1234_5601, 600));
    assert_eq!(
        calls(),
        [
            Call::Holder,
            Call::Unsigned(17),
            Call::Active,
            Call::Holder,
            Call::Property(0x8A1E_07EE),
            Call::New,
            Call::Set,
            Call::Condition,
            Call::Evaluate(50.0f32.to_bits()),
            Call::Free(0x1234)
        ]
    );
    STATE.with(|s| s.borrow_mut().nominal = None);
    assert_eq!(w.capacity(), (0xDEAD_BEEF_1122_3300, 0));
    assert_eq!(calls(), [Call::Holder, Call::Unsigned(17)]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.nominal = Some(1000);
        s.active = false;
    });
    assert_eq!(w.capacity().1, 0);
    calls();
    STATE.with(|s| s.borrow_mut().property = false);
    assert_eq!(w.capacity(), (0x6543_210F_0000_0001, 0));
    assert_eq!(
        calls(),
        [
            Call::Holder,
            Call::Unsigned(17),
            Call::Active,
            Call::Holder,
            Call::Property(0x8A1E_07EE)
        ]
    );
}

#[test]
fn capacity_unsigned_scaling_bias_rounding_and_missing_condition() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let sim = unsafe { CapacityView::from_ptr(w.sim.ptr()) };
    sim.set_efficiency(1.0);
    for (nominal, gain, expected) in [(5, 0.5, 2), (7, 0.5, 4), (0xFFFF_FFFF, 0.0, 0), (17, -1.0, 0xFFFF_FFEF)] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.nominal = Some(nominal);
            s.gain = gain;
            s.storage = 0;
        });
        assert_eq!(w.capacity(), (0xCAFE_4321_0000_0001, expected));
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.nominal = Some(10);
        s.gain = 1.0;
        s.condition = None;
    });
    assert_eq!(w.capacity().1, 10);
    assert!(calls().contains(&Call::Evaluate(0)));
    STATE.with(|s| s.borrow_mut().property = false);
    sim.set_efficiency(f32::NAN);
    assert_eq!(w.capacity().1, 0);
    sim.set_efficiency(0.5);
    STATE.with(|s| s.borrow_mut().nominal = Some(0xFFFF_FFFF));
    // The integer is loaded exactly before the product rounds to 24 significant bits.
    assert_eq!(w.capacity().1, 0x8000_0000);
}

#[test]
fn maintenance_requires_all_three_properties_and_retains_failure_registers() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().base = None);
    assert_eq!(w.maintenance(), (0x8765_4321_AABB_CC00, 0));
    assert_eq!(calls(), [Call::Holder, Call::Base]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.base = Some(-1000);
        s.condition = None;
    });
    assert_eq!(w.maintenance(), (0x9876_5432_4433_2200, -1000));
    assert_eq!(calls(), [Call::Holder, Call::Base, Call::Condition]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.condition = Some(50.0);
        s.property = false;
    });
    assert_eq!(w.maintenance(), (0x6543_210F_0000_0000, -1000));
    assert_eq!(
        calls(),
        [Call::Holder, Call::Base, Call::Condition, Call::Property(0x0911_E2E7)]
    );
}

#[test]
fn maintenance_exact_integer_load_wide_curve_result_and_truncation() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    assert_eq!(w.maintenance(), (0xD00D_5678_1234_5601, 750));
    assert_eq!(
        calls(),
        [
            Call::Holder,
            Call::Base,
            Call::Condition,
            Call::Property(0x0911_E2E7),
            Call::New,
            Call::Set,
            Call::Evaluate(50.0f32.to_bits()),
            Call::Free(0x1234)
        ]
    );
    for (base, gain, expected) in [
        (5, 0.5, 2),
        (-5, 0.5, -2),
        (i64::MAX, 0.0, 0),
        (1, f64::NAN, i64::MIN),
        (i64::MAX, 2.0, i64::MIN),
        (16_777_217, 1.0, 16_777_216),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.base = Some(base);
            s.gain = gain;
            s.storage = 0;
        });
        let (result, cost) = w.maintenance();
        assert_eq!(cost, expected);
        assert_eq!(result, registers(1, (expected as u64 >> 32) as u32));
    }
    // A curve output is kept in its x87 register until after multiplication.
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.base = Some(16_777_217);
        s.gain = 1.0 + 2.0f64.powi(-25);
    });
    assert_eq!(w.maintenance().1, 16_777_218);
}

#[test]
fn budgets_book_only_changed_costs_and_use_wrapping_signed_differences() {
    let mut w = World::new();
    let item_table = table(&[(0x34, charge as *const () as usize)]);
    let mut item_object = [item_table.as_ptr() as usize];
    let budget_table = table(&[
        (0xCC, key as *const () as usize),
        (0xE4, item as *const () as usize),
        (0xD4, remember as *const () as usize),
    ]);
    let mut budget_object = Budget {
        table: budget_table.as_ptr(),
        item: item_object.as_mut_ptr().cast(),
    };
    let mut budget_pointer = (&mut budget_object as *mut Budget).cast::<u8>();
    let _budget = fake_data(0x00B4_3D64, (&mut budget_pointer as *mut *mut u8).cast());
    let mut sentinel = [0usize; 3];
    let mut node = [
        sentinel.as_mut_ptr() as usize,
        sentinel.as_mut_ptr() as usize,
        (&mut *w.occupant as *mut Occupant) as usize,
    ];
    sentinel[0] = node.as_mut_ptr() as usize;
    sentinel[1] = node.as_mut_ptr() as usize;
    let mut head = sentinel.as_mut_ptr().cast::<u8>();
    let occupant = &*w.occupant as *const Occupant as usize;
    let holder = w.holder.as_ptr() as usize;
    for (cost, previous, key, item) in [
        (Some(600), 200, true, true),
        (Some(200), 200, true, true),
        (None, 200, true, true),
        (Some(600), 200, false, true),
        (Some(600), 200, true, false),
        (Some(i64::MIN), i64::MAX, true, true),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.cost = cost;
            s.previous = previous;
            s.has_key = key;
            s.has_item = item;
        });
        unsafe { fc4p_006e4100(w.sim.ptr(), &mut head) };
        let mut expected = vec![Call::Holder, Call::Cost(occupant)];
        if let Some(cost) = cost {
            expected.push(Call::Key(holder, 0xCA58_EDC7));
            if key && cost != previous {
                expected.push(Call::Item);
                if item {
                    expected.extend([
                        Call::Charge(cost.wrapping_sub(previous)),
                        Call::Remember(holder, 42, cost),
                    ]);
                }
            }
        }
        assert_eq!(calls(), expected);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.cost = Some(600);
        s.previous = 200;
        s.item_previous = Some(700);
    });
    unsafe { fc4p_006e4100(w.sim.ptr(), &mut head) };
    assert_eq!(
        calls(),
        [
            Call::Holder,
            Call::Cost(occupant),
            Call::Key(holder, 0xCA58_EDC7),
            Call::Item,
            Call::Charge(-100),
            Call::Remember(holder, 42, 600),
        ]
    );
    sentinel[0] = sentinel.as_ptr() as usize;
    unsafe { fc4p_006e4100(w.sim.ptr(), &mut head) };
    assert_eq!(sentinel[0], sentinel.as_ptr() as usize);
    assert_eq!(calls(), []);
    // Keep the manager alive through every capacity query.
    assert_ne!(w.manager[0], 0);
}
