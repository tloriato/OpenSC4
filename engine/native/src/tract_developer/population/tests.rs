// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Seeded population cases from the core tests, preserving game calls and raw population words.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Map,
    Shift,
    Allocate(u32),
    List([u32; 5]),
    Clear,
    Free(u32),
    Demand(u32, u32),
    Type,
    Wealth,
    Construct(u32),
    Occupied(u32),
    LotWealth(u32),
    Capacity(u32, u32, u32),
    Draw(u64, u64),
    Population(u32, u32, u32),
    Release(u32),
}
struct State {
    calls: Vec<Call>,
    demand: u32,
    shift: u32,
    wealth: u32,
    map: u32,
    lots: Vec<u32>,
    memory: Vec<Box<[u32]>>,
    draws: Vec<f64>,
    draw_index: usize,
    rng: u32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            demand: 100,
            shift: 2,
            wealth: 2,
            map: 0,
            lots: Vec::new(),
            memory: Vec::new(),
            draws: vec![0.2, 0.1, 0.2, 0.1, 0.2, 0.1],
            draw_index: 0,
            rng: 0,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
extern "thiscall" fn map(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Map);
        0xBADC_0FFE_0000_0000 | u64::from(s.map)
    })
}
extern "thiscall" fn shift(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Shift);
        0xBADC_0FFE_0000_0000 | u64::from(s.shift)
    })
}
extern "C" fn allocate(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        let mut memory = vec![0xCCCC_CCCC; bytes as usize / 4].into_boxed_slice();
        let p = memory.as_mut_ptr() as u32;
        s.memory.push(memory);
        0xBADC_0FFE_0000_0000 | u64::from(p)
    })
}
extern "C" fn free(p: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(p)));
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn clear(list: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Clear));
    unsafe {
        let h = word(list, 0);
        assert_eq!([word(h, 0), word(h, 4)], [h; 2]);
    }
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn list(_this: u32, list: u32, x0: u32, z0: u32, x1: u32, z1: u32, flag: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::List([x0, z0, x1, z1, flag]));
        let h = unsafe { word(list, 0) };
        assert_eq!(unsafe { word(h, 8) }, 0xCCCC_CCCC);
        let mut previous = h;
        for i in 0..s.lots.len() {
            let mut memory = vec![h, previous, s.lots[i]].into_boxed_slice();
            let node = memory.as_mut_ptr() as u32;
            s.memory.push(memory);
            unsafe {
                store(previous, 0, node);
                store(h, 4, node);
            }
            previous = node;
        }
    });
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn demand(_this: u32, x: u32, z: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Demand(x, z));
        0xBADC_0FFE_0000_0000 | u64::from(s.demand)
    })
}
extern "thiscall" fn kind(_this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Type));
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn wealth(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Wealth);
        0xBADC_0FFE_0000_0000 | u64::from(s.wealth)
    })
}
extern "thiscall" fn construct(this: u32, seed: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Construct(seed));
        s.draw_index = 0;
        s.rng = this;
    });
    unsafe {
        store(this, 0, seed);
        store(this, 4, 0xDEAD_BEEF);
        store(this, 8, 0);
    }
    0xBADC_0FFE_0000_0000 | u64::from(this)
}
extern "thiscall" fn draw(this: u32, low: f64, high: f64) -> f64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.rng);
        s.calls.push(Call::Draw(low.to_bits(), high.to_bits()));
        let value = s.draws[s.draw_index];
        s.draw_index += 1;
        unsafe {
            store(this, 8, s.draw_index as u32);
        }
        if low.is_nan() || high.is_nan() {
            f64::NAN
        } else {
            value
        }
    })
}
extern "thiscall" fn occupied(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Occupied(unsafe { word(this, 4) })));
    0xBADC_0FFE_0000_0000 | u64::from(unsafe { word(this, 8) })
}
extern "thiscall" fn lot_wealth(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::LotWealth(unsafe { word(this, 4) })));
    0xBADC_0FFE_0000_0000 | u64::from(unsafe { word(this, 12) })
}
extern "thiscall" fn capacity(this: u32, kind: u32, flag: u32) -> u64 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Capacity(unsafe { word(this, 4) }, kind, flag))
    });
    0xBADC_0FFE_0000_0000 | u64::from(unsafe { word(this, 16) })
}
extern "thiscall" fn population(this: u32, kind: u32, value: u32) -> u64 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Population(unsafe { word(this, 4) }, kind, value))
    });
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn release(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(unsafe { word(this, 4) })));
    0xBADC_0FFE_1234_5678
}

struct World {
    td: Object,
    objects: Box<[u32; 3]>,
    tables: [Box<[usize; 56]>; 4],
    lots: Box<[[u32; 5]; 3]>,
    constants: Box<[u64; 5]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(count: usize) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180),
            objects: Box::new([0; 3]),
            tables: core::array::from_fn(|_| Box::new([0; 56])),
            lots: Box::new([[0; 5]; 3]),
            constants: Box::new([0; 5]),
            _fakes: Vec::new(),
        };
        for (table, slot, target) in [
            (0, 0x34, map as *const () as usize),
            (0, 0x18, kind as *const () as usize),
            (0, 0x20, wealth as *const () as usize),
            (1, 0x2C, shift as *const () as usize),
            (1, 0x58, demand as *const () as usize),
            (2, 0x1C, list as *const () as usize),
            (3, 0x98, occupied as *const () as usize),
            (3, 0xA4, lot_wealth as *const () as usize),
            (3, 0xD0, capacity as *const () as usize),
            (3, 0xDC, population as *const () as usize),
            (3, 8, release as *const () as usize),
        ] {
            w.tables[table][slot / 4] = target;
        }
        for i in 0..3 {
            w.objects[i] = w.tables[i].as_ptr() as u32;
            w.lots[i] = [w.tables[3].as_ptr() as u32, i as u32, 0xABCD_0001, 2, 0xFFFF_0028];
        }
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.map = w.objects.as_ptr().wrapping_add(1) as u32;
            s.lots = (0..count).map(|i| w.lots[i].as_ptr() as u32).collect();
        });
        for (address, target) in [
            (0x0090_CF54, allocate as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
            (0x0047_3EC0, clear as *const () as usize),
            (0x0091_3A5C, construct as *const () as usize),
            (0x0091_3761, draw as *const () as usize),
        ] {
            w._fakes.push(fake(address, target));
        }
        w.constants[0] = u64::from(0.01f32.to_bits());
        w.constants[1] = 1.0f64.to_bits();
        w.constants[2] = u64::from(0.0f32.to_bits());
        w.constants[3] = u64::from(1.0f32.to_bits());
        w.constants[4] = u64::from(12_582_912.0f32.to_bits());
        for (i, address) in [0x00A8_23A4, 0x00A8_0AB0, 0x00A8_1054, 0x00A8_1228, 0x00AB_3048]
            .into_iter()
            .enumerate()
        {
            w._fakes
                .push(fake_data(address, w.constants.as_mut_ptr().wrapping_add(i).cast()));
        }
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_occupancy_variation(0.2);
        td.set_minimum_vacancy(0.1);
        td.set_maximum_vacancy(0.1);
        unsafe {
            store(td.as_ptr() as u32, 0x2C, w.objects.as_ptr().wrapping_add(2) as u32);
        }
        w
    }
    fn run(&mut self, x: u32, z: u32) -> Vec<Call> {
        let mut profile = [self.objects.as_ptr() as u32, x, z];
        let before = snapshot(self.td.ptr(), 0x180);
        let result = run(
            0x0070_6BC0,
            Conv::Thiscall,
            self.td.ptr() as u32,
            &[profile.as_mut_ptr() as u32],
        );
        assert_eq!(result.0, 0x1234_5678);
        assert_eq!(result.2, 0xBADC_0FFE);
        assert_eq!(snapshot(self.td.ptr(), 0x180), before);
        assert_eq!(profile, [self.objects.as_ptr() as u32, x, z]);
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}
fn populations(calls: &[Call]) -> Vec<(u32, u32)> {
    calls
        .iter()
        .filter_map(|c| {
            if let Call::Population(id, kind, value) = *c {
                assert_eq!(kind, 0x1234_5678);
                Some((id, value))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn zero_tract_seed_is_forwarded_to_game_constructor_and_population_is_repeatable() {
    let _fpu = GameFpu::new();
    let mut w = World::new(1);
    for _ in 0..2 {
        let calls = w.run(0, 0);
        assert!(calls.contains(&Call::Construct(0)));
        assert_eq!(populations(&calls), [(0, 0x4B40_0024)]);
        assert_eq!(calls.iter().filter(|c| matches!(c, Call::Draw(..))).count(), 2);
    }
}

#[test]
fn core_seeded_three_lot_case_and_cleanup_order() {
    let _fpu = GameFpu::new();
    let _machine = crate::tract_developer::testing::PreserveMachine::new();
    let mut w = World::new(3);
    w.lots[1][4] = 25;
    w.lots[2][4] = 7;
    STATE.with(|s| {
        s.borrow_mut().draws = vec![
            0.02620549499988556,
            0.0991901084780693,
            0.1932249218225479,
            0.045238249003887177,
            0.0026829540729522705,
            0.08752158284187317,
        ]
    });
    let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
    td.set_minimum_vacancy(0.01);
    td.set_maximum_vacancy(0.1);
    let calls = w.run(3, 5);
    assert_eq!(
        populations(&calls),
        [(0, 0x4B40_0024), (1, 0x4B40_0018), (2, 0x4B40_0006)]
    );
    assert_eq!(
        &calls[..9],
        [
            Call::Map,
            Call::Shift,
            Call::Allocate(12),
            Call::List([12, 20, 15, 23, 1]),
            Call::Demand(3, 5),
            Call::Type,
            Call::Wealth,
            Call::Construct(0x0030_0050),
            Call::Occupied(0)
        ]
    );
    let memory = STATE.with(|s| s.borrow().memory.iter().map(|m| m.as_ptr() as u32).collect::<Vec<_>>());
    assert_eq!(
        &calls[calls.len() - 7..],
        [
            Call::Release(0),
            Call::Free(memory[1]),
            Call::Release(1),
            Call::Free(memory[2]),
            Call::Release(2),
            Call::Free(memory[3]),
            Call::Free(memory[0])
        ]
    );
}

#[test]
fn occupancy_clamps_and_nan_consume_the_original_number_of_draws() {
    let _fpu = GameFpu::new();
    for (demand, variation, population, draws) in [
        (-50i16, 0.2, Some(0x4B40_0000), 1),
        (300, 0.2, Some(0x4B40_0024), 2),
        (80, 0.0, Some(0x4B40_0020), 1),
        (100, f32::NAN, None, 1),
    ] {
        let mut w = World::new(1);
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_occupancy_variation(variation);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.demand = 0xABCD_0000 | u32::from(demand as u16);
            if variation == 0.0 {
                s.draws[0] = 0.0;
            }
        });
        let calls = w.run(0, 0);
        let result = populations(&calls)[0].1;
        if let Some(expected) = population {
            assert_eq!(result, expected);
        } else {
            assert!(f32::from_bits(result).is_nan());
            assert_eq!(result as u16, 0);
        }
        assert_eq!(calls.iter().filter(|c| matches!(c, Call::Draw(..))).count(), draws);
    }
    assert_eq!(crate::x87::control_word(), 0x007F);
}

#[test]
fn zero_capacity_unoccupied_and_full_wealth_mismatch_skip_draws() {
    let _fpu = GameFpu::new();
    let mut w = World::new(3);
    w.lots[0][2] = 0xFFFF_0000;
    w.lots[1][3] = 0xFFFF_0002;
    w.lots[2][4] = 0xFFFF_0000;
    let calls = w.run(1, 1);
    assert!(populations(&calls).is_empty());
    assert!(!calls.iter().any(|c| matches!(c, Call::Draw(..))));
    assert_eq!(
        &calls[8..14],
        [
            Call::Occupied(0),
            Call::Occupied(1),
            Call::LotWealth(1),
            Call::Occupied(2),
            Call::LotWealth(2),
            Call::Capacity(2, 0x1234_5678, 0)
        ]
    );
}

#[test]
fn empty_list_uses_clear_helper_and_shift_and_seed_arithmetic_wraps() {
    let _fpu = GameFpu::new();
    let mut w = World::new(0);
    let calls = w.run(u32::MAX, 0x8000_0000);
    let header = STATE.with(|s| s.borrow().memory[0].as_ptr() as u32);
    assert_eq!(
        calls,
        [
            Call::Map,
            Call::Shift,
            Call::Allocate(12),
            Call::List([0xFFFF_FFFC, 0, u32::MAX, 3, 1]),
            Call::Clear,
            Call::Free(header)
        ]
    );
    let mut w = World::new(1);
    STATE.with(|s| s.borrow_mut().shift = 33);
    let calls = w.run(u32::MAX, u32::MAX);
    assert!(calls.contains(&Call::List([0xFFFF_FFFE, 0xFFFF_FFFE, u32::MAX, u32::MAX, 1])));
    assert!(calls.contains(&Call::Construct(0xFFEF_FFF0)));
    let bounds: Vec<_> = calls
        .iter()
        .filter_map(|c| {
            if let Call::Draw(low, high) = c {
                Some((*low, *high))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(bounds[0], ((-f64::from(0.2f32)).to_bits(), f64::from(0.2f32).to_bits()));
}
