// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Holder(u32),
    Budget(u32),
    Local(u32),
    Float(u32, u32),
    Add(u32),
    Release(u32),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
fn log(c: Call) {
    CALLS.with(|s| s.borrow_mut().push(c));
}
fn calls() -> Vec<Call> {
    CALLS.with(|s| core::mem::take(&mut *s.borrow_mut()))
}
#[repr(C)]
struct Plant {
    vt: *const usize,
    id: u32,
    refs: u32,
    funding: f32,
    budget: u32,
    local: u32,
    wear: Option<f32>,
    t1: Option<f32>,
    t2: Option<f32>,
}
impl Plant {
    fn new(id: u32, funding: f32, wear: f32) -> Self {
        Self {
            vt: core::ptr::null(),
            id,
            refs: 1,
            funding,
            budget: 1,
            local: 1,
            wear: Some(wear),
            t1: Some(100.0),
            t2: Some(100.0),
        }
    }
}
extern "thiscall" fn holder(p: *mut Plant) -> *mut Plant {
    unsafe {
        log(Call::Holder((*p).id));
    }
    p
}
extern "thiscall" fn add(p: *mut Plant) -> u64 {
    unsafe {
        log(Call::Add((*p).id));
        (*p).refs += 1;
    }
    0xAAAA_1111_AABB_CCDD
}
extern "thiscall" fn release(p: *mut Plant) -> u64 {
    unsafe {
        log(Call::Release((*p).id));
        (*p).refs -= 1;
    }
    0xBBBB_2222_EEFF_0011
}
extern "thiscall" fn budget(_p: *mut u8, plant: *mut Plant, id: u32, out: *mut u32) -> u32 {
    assert_eq!(id, 0xCA58_E540);
    unsafe {
        log(Call::Budget((*plant).id));
        assert_eq!(out.add(7).read(), 0);
        out.add(7).write((*plant).funding.to_bits());
        (*plant).budget
    }
}
extern "thiscall" fn local(_p: *mut u8, plant: *mut Plant, id: u32) -> u32 {
    assert_eq!(id, 0xCA58_E540);
    unsafe {
        log(Call::Local((*plant).id));
        (*plant).local
    }
}
extern "C" fn float(p: *mut Plant, id: u32, out: *mut f32) -> u64 {
    unsafe {
        log(Call::Float((*p).id, id));
        let value = match id {
            0xC910_B4CF => (*p).wear,
            0xEA1C_F221 => (*p).t1,
            0xEA1C_F222 => (*p).t2,
            _ => panic!("{id:X}"),
        };
        if let Some(v) = value {
            out.write(v);
        }
    }
    0x8765_4321_ABCD_EF01
}
struct World {
    object: Object,
    plants: Vec<Plant>,
    nodes: Vec<[u32; 4]>,
    _pvt: Box<[usize; 7]>,
    _bvt: Box<[usize; 52]>,
    _dvt: Box<[usize; 29]>,
    _budget: Box<*const usize>,
    _department: Box<*const usize>,
    _fake: crate::testing::Fake,
    _fpu: GameFpu,
}
impl World {
    fn new(mut plants: Vec<Plant>) -> Self {
        calls();
        let mut pvt = Box::new([0; 7]);
        pvt[1] = add as *const () as usize;
        pvt[2] = release as *const () as usize;
        pvt[6] = holder as *const () as usize;
        for p in &mut plants {
            p.vt = pvt.as_ptr();
        }
        let mut bvt = Box::new([0; 52]);
        bvt[0xCC / 4] = budget as *const () as usize;
        let mut dvt = Box::new([0; 29]);
        dvt[0x70 / 4] = local as *const () as usize;
        let count = plants.len();
        let mut w = Self {
            object: Object::new(0x400),
            plants,
            nodes: vec![[0; 4]; count + 1],
            _budget: Box::new(bvt.as_ptr()),
            _department: Box::new(dvt.as_ptr()),
            _pvt: pvt,
            _bvt: bvt,
            _dvt: dvt,
            _fake: fake(0x005F_D4F0, float as *const () as usize),
            _fpu: GameFpu::new(),
        };
        for i in 0..=count {
            w.nodes[i][0] = w.nodes[(i + 1) % (count + 1)].as_ptr() as u32;
            if i > 0 {
                w.nodes[i][3] = (&mut w.plants[i - 1] as *mut Plant) as u32;
            }
        }
        unsafe {
            let sim = PowerSim::from_ptr(w.object.ptr());
            sim.set_plant_header(w.nodes.as_mut_ptr().cast());
            sim.set_budget((&mut *w._budget as *mut *const usize).cast());
            sim.set_department((&mut *w._department as *mut *const usize).cast());
        }
        w
    }
    fn run(&mut self, outputs: [u32; 4]) -> (u32, u32) {
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_006eba30 as *const () as usize,
            self.object.ptr() as u32,
            &outputs,
        );
        call.edx = 0x1357_2468;
        unsafe {
            call_raw(&mut call);
        }
        (call.eax, call.edx)
    }
}
fn floats(id: u32) -> [Call; 3] {
    [
        Call::Float(id, 0xC910_B4CF),
        Call::Float(id, 0xEA1C_F221),
        Call::Float(id, 0xEA1C_F222),
    ]
}

#[test]
fn searches_are_independent_and_own_department_filters_only_the_funding_result() {
    let mut w = World::new(vec![
        Plant::new(1, 1.5, 40.0),
        Plant::new(2, 0.7, 90.0),
        Plant::new(3, 0.5, 60.0),
    ]);
    w.plants[1].local = 0x100;
    let (mut low, mut fund, mut worn, mut percent) = (0u32, 0u32, 0u32, 0u32);
    let args = [
        &mut low as *mut u32 as u32,
        &mut fund as *mut u32 as u32,
        &mut worn as *mut u32 as u32,
        &mut percent as *mut u32 as u32,
    ];
    assert_eq!(w.run(args), (0xEEFF_0011, 0xBBBB_2222));
    assert_eq!(low, &w.plants[2] as *const Plant as u32);
    assert_eq!(fund, 0.5f32.to_bits());
    assert_eq!(worn, &w.plants[1] as *const Plant as u32);
    assert_eq!(percent, 90);
    assert_eq!(w.plants.iter().map(|p| p.refs).collect::<Vec<_>>(), [1, 2, 2]);
    let mut expected = vec![Call::Holder(1), Call::Budget(1), Call::Local(1), Call::Add(1)];
    expected.extend(floats(1));
    expected.push(Call::Add(1));
    expected.extend([Call::Holder(2), Call::Budget(2), Call::Local(2)]);
    expected.extend(floats(2));
    expected.extend([
        Call::Add(2),
        Call::Release(1),
        Call::Holder(3),
        Call::Budget(3),
        Call::Local(3),
        Call::Add(3),
        Call::Release(1),
    ]);
    expected.extend(floats(3));
    expected.extend([Call::Add(2), Call::Add(3), Call::Release(2), Call::Release(3)]);
    assert_eq!(calls(), expected);
}

#[test]
fn missing_budget_low_bool_bytes_ties_and_null_outputs_balance_temporary_references() {
    let mut w = World::new(vec![
        Plant::new(1, 0.0, 0.0),
        Plant::new(2, 1.0, 40.0),
        Plant::new(3, 1.0, 40.0),
    ]);
    w.plants[0].budget = 0x100;
    w.run([0; 4]);
    assert!(w.plants.iter().all(|p| p.refs == 1));
    let c = calls();
    assert!(!c.contains(&Call::Local(1)));
    assert!(!c.contains(&Call::Add(3)));
    assert_eq!(c.iter().filter(|c| **c == Call::Add(2)).count(), 2);
    assert_eq!(c.iter().filter(|c| **c == Call::Release(2)).count(), 2);
}

#[test]
fn empty_list_preserves_edx_header_eax_and_every_output_byte() {
    let mut w = World::new(vec![]);
    let mut out = [0xDEAD_BEEFu32; 4];
    let p = out.as_mut_ptr();
    assert_eq!(
        w.run(core::array::from_fn(|i| unsafe { p.add(i) as u32 })),
        (w.nodes.as_ptr() as u32, 0x1357_2468)
    );
    assert_eq!(out, [0xDEAD_BEEF; 4]);
    assert!(calls().is_empty());
}

#[test]
fn ratio_uses_defaults_maximum_ties_nan_and_full_ftol_semantics() {
    for (wear, t1, t2, expected) in [
        (Some(49.5), None, None, Some(50)),
        (Some(50.0), Some(50.0), Some(100.0), Some(50)),
        (Some(50.0), Some(100.0), Some(50.0), Some(50)),
        (Some(50.0), Some(f32::NAN), Some(100.0), Some(50)),
        (Some(50.0), Some(100.0), Some(f32::NAN), None),
        (Some(f32::NAN), Some(1.0), Some(1.0), None),
        (Some(f32::INFINITY), Some(1.0), Some(1.0), Some(0)),
        (Some(30_000_000.0), Some(1.0), Some(1.0), Some(3_000_000_000)),
        (Some(-1.0), Some(100.0), Some(100.0), None),
        (None, None, None, None),
        (Some(1.0), Some(0.0), Some(0.0), Some(0)),
    ] {
        let mut plant = Plant::new(1, f32::NAN, 0.0);
        plant.wear = wear;
        plant.t1 = t1;
        plant.t2 = t2;
        let mut w = World::new(vec![plant]);
        let mut worn = 0xCAFE_BABEu32;
        let mut percent = 0xA5A5_A5A5u32;
        w.run([0, 0, &mut worn as *mut u32 as u32, &mut percent as *mut u32 as u32]);
        match expected {
            Some(value) => {
                assert_eq!(worn, w.plants.as_ptr() as u32);
                assert_eq!(percent, value);
            }
            None => {
                assert_eq!(worn, 0xCAFE_BABE);
                assert_eq!(percent, 0xA5A5_A5A5);
            }
        }
    }
}

#[test]
fn aliased_outputs_are_written_worn_first_and_lowest_last() {
    let mut w = World::new(vec![Plant::new(1, 0.25, 10.0), Plant::new(2, 0.75, 50.0)]);
    let (mut plant, mut number) = (0u32, 0u32);
    let (p, n) = (&mut plant as *mut u32 as u32, &mut number as *mut u32 as u32);
    w.run([p, n, p, n]);
    assert_eq!(plant, w.plants.as_ptr() as u32);
    assert_eq!(number, 0.25f32.to_bits());
}

#[test]
fn no_winner_keeps_original_temporary_address_upper_half_and_property_edx() {
    let mut w = World::new(vec![Plant::new(1, 2.0, 0.0)]);
    let mut stack = [0u32; 5];
    let mut r = unsafe { find_plants(w.object.ptr(), 0x1234, stack.as_mut_ptr()) };
    assert_eq!((r >> 32) as u32, 0x8765_4321);
    assert_eq!(
        r as u32 & 0xFFFF_0000,
        (stack.as_ptr() as u32).wrapping_sub(0x38) & 0xFFFF_0000
    );
    assert_eq!(r as u32 & 0x4500, 0x4000);
    w.plants[0].t1 = Some(101.0);
    r = unsafe { find_plants(w.object.ptr(), 0x1234, stack.as_mut_ptr()) };
    assert_eq!(
        r as u32 & 0xFFFF_0000,
        (stack.as_ptr() as u32).wrapping_sub(0x3C) & 0xFFFF_0000
    );
}
