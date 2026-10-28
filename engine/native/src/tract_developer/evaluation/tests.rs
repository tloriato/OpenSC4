// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Capacity caching and the core scoring cases, with call order and x87 edge cases.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, EDX};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Call(u32, u32, u32);
#[derive(Default)]
struct Settings {
    calls: Vec<Call>,
    zone: u32,
    score_from_capacity: f32,
    draw: f64,
}
thread_local! { static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default()); }
fn log(slot: u32, this: u32, argument: u32, value: u32) -> u64 {
    SETTINGS.with(|s| s.borrow_mut().calls.push(Call(slot, this, argument)));
    0xBADC_0FFE_0000_0000 | u64::from(value)
}
extern "thiscall" fn dev_type(this: u32, flag: u32) -> u64 {
    log(0x18, this, flag, 0x1234_5678)
}
extern "thiscall" fn capacity(this: u32, kind: u32) -> u64 {
    log(0xC8, this, kind, unsafe { word(this, 4) })
}
extern "thiscall" fn state(this: u32) -> u64 {
    log(0x84, this, 0, unsafe { word(this, 8) })
}
extern "thiscall" fn configuration(this: u32) -> u64 {
    log(0x24, this, 0, unsafe { word(this, 12) })
}
extern "thiscall" fn stage(this: u32) -> u64 {
    log(0x2C, this, 0, unsafe { word(this, 4) })
}
extern "thiscall" fn building(this: u32, flag: u32) -> u64 {
    log(0xB8, this, flag, unsafe { word(this, 16) })
}
extern "thiscall" fn building_info(this: u32, building: u32) -> u64 {
    log(0x9C, this, building, building)
}
extern "thiscall" fn advance(this: u32) -> u64 {
    unsafe {
        store(this, 0, word(word(this, 0), 0));
    }
    log(0x005D_17C0, 0, 0, 0x7654_3210)
}
extern "thiscall" fn zone(this: u32) -> u64 {
    let zone = SETTINGS.with(|s| s.borrow().zone);
    log(0x14, this, 0, zone)
}
extern "thiscall" fn capacity_for_score(this: u32, candidate: u32) -> u64 {
    let score = SETTINGS.with(|s| s.borrow().score_from_capacity);
    unsafe {
        store(candidate, 0x34, score.to_bits());
    }
    log(0x0070_55B0, this, candidate, 0x7654_3210)
}
extern "thiscall" fn uniform(this: u32) -> f64 {
    log(0x0091_372E, this, 0, 0);
    SETTINGS.with(|s| s.borrow().draw)
}

struct World {
    td: Object,
    developer: [u32; 1],
    bds: [u32; 1],
    dev_table: Box<[usize; 10]>,
    bds_table: Box<[usize; 40]>,
    lot_table: Box<[usize; 52]>,
    configuration_table: Box<[usize; 12]>,
    zero: Box<f32>,
    bias: Box<f32>,
    random: Box<u8>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        SETTINGS.with(|s| *s.borrow_mut() = Settings::default());
        let mut w = Self {
            td: Object::new(0x180),
            developer: [0],
            bds: [0],
            dev_table: Box::new([0; 10]),
            bds_table: Box::new([0; 40]),
            lot_table: Box::new([0; 52]),
            configuration_table: Box::new([0; 12]),
            zero: Box::new(0.0),
            bias: Box::new(4_294_967_296.0),
            random: Box::new(0),
            _fakes: Vec::new(),
        };
        w.dev_table[0x18 / 4] = dev_type as *const () as usize;
        w.dev_table[0x14 / 4] = zone as *const () as usize;
        w.bds_table[0x9C / 4] = building_info as *const () as usize;
        for (slot, target) in [
            (0xC8, capacity as *const () as usize),
            (0x84, state as *const () as usize),
            (0x24, configuration as *const () as usize),
            (0xB8, building as *const () as usize),
        ] {
            w.lot_table[slot / 4] = target;
        }
        w.configuration_table[0x2C / 4] = stage as *const () as usize;
        w._fakes.push(fake(0x005D_17C0, advance as *const () as usize));
        w._fakes.push(fake_data(0x00A8_1054, (&mut *w.zero as *mut f32).cast()));
        w._fakes.push(fake_data(0x00A8_0AA8, (&mut *w.bias as *mut f32).cast()));
        w._fakes.push(fake_data(0x00B4_AFD8, &mut *w.random));
        w
    }
    fn view(&mut self) -> TractDeveloper {
        self.developer[0] = self.dev_table.as_ptr() as u32;
        self.bds[0] = self.bds_table.as_ptr() as u32;
        let td = unsafe { TractDeveloper::from_ptr(self.td.ptr()) };
        td.set_developer(self.developer.as_mut_ptr() as u32);
        td.set_building_development(self.bds.as_mut_ptr() as u32);
        td
    }
    fn lot(&self, capacity: u32, state: u32, configuration: u32, building: u32) -> [u32; 5] {
        [self.lot_table.as_ptr() as u32, capacity, state, configuration, building]
    }
    fn calls(&self) -> Vec<Call> {
        SETTINGS.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn cached_capacity_and_unknown_kinds_have_no_calls() {
    let mut w = World::new();
    let td = w.view();
    let mut memory = [0xA5u8; 0xA4];
    let p = memory.as_mut_ptr();
    let c = unsafe { Candidate::from_ptr(p) };
    c.set_occupied(0);
    c.set_abandoned(i32::MAX);
    let before = memory;
    assert_eq!(
        run(0x0070_55B0, Conv::Thiscall, td.as_ptr() as u32, &[p as u32]),
        (0, i32::MAX as u32, EDX)
    );
    assert_eq!(memory, before);
    for kind in [0, -1, 5, i32::MIN, i32::MAX] {
        c.set_kind(kind);
        c.set_occupied(-1);
        c.set_abandoned(1);
        c.set_stage(77);
        c.set_wealth(88);
        let mut expected = memory;
        expected[0x24..0x34].fill(0);
        assert_eq!(
            run(0x0070_55B0, Conv::Thiscall, td.as_ptr() as u32, &[p as u32]),
            ((kind as u32).wrapping_sub(1), 1, EDX)
        );
        assert_eq!(memory, expected);
    }
    assert!(w.calls().is_empty());
}

#[test]
fn single_lot_capacity_uses_low_word_and_stage_byte() {
    for kind in [1, 2, 4] {
        for lot_state in [0, 4, 0xFFFF_0004] {
            let mut w = World::new();
            let td = w.view();
            let mut info = [0, 3u32, 0];
            let mut cfg = [w.configuration_table.as_ptr() as u32, 0xDEAD_FF09];
            let mut lot = w.lot(
                0xBEEF_FFFE,
                lot_state,
                cfg.as_mut_ptr() as u32,
                info.as_mut_ptr() as u32,
            );
            let l = lot.as_mut_ptr() as u32;
            let mut memory = [0xA5u8; 0xA4];
            let c = unsafe { Candidate::from_ptr(memory.as_mut_ptr()) };
            c.set_kind(kind);
            c.set_lot(l as *mut u8);
            c.set_occupied(-1);
            c.set_abandoned(9);
            let mut expected = memory;
            expected[0x24..0x34].fill(0);
            let offset = if lot_state == 4 { 0x28 } else { 0x24 };
            expected[offset..offset + 4].copy_from_slice(&65534u32.to_le_bytes());
            expected[0x2C..0x30].copy_from_slice(&9u32.to_le_bytes());
            expected[0x30..0x34].copy_from_slice(&3u32.to_le_bytes());
            let result = run(0x0070_55B0, Conv::Thiscall, td.as_ptr() as u32, &[c.as_ptr() as u32]);
            assert_eq!(memory, expected);
            assert_eq!(result.0, 3);
            assert_eq!(
                w.calls(),
                [
                    Call(0x18, td.developer(), 1),
                    Call(0xC8, l, 0x1234_5678),
                    Call(0x84, l, 0),
                    Call(0x24, l, 0),
                    Call(0x2C, cfg.as_ptr() as u32, 0),
                    Call(0xB8, l, 0),
                    Call(0x9C, td.building_development(), info.as_ptr() as u32)
                ]
            );
        }
    }
}

#[test]
fn missing_configuration_or_building_contributes_zero() {
    let mut w = World::new();
    let td = w.view();
    let mut lot = w.lot(0x1234_0000, 4, 0, 0);
    let mut memory = [0u8; 0xA4];
    let c = unsafe { Candidate::from_ptr(memory.as_mut_ptr()) };
    c.set_kind(2);
    c.set_lot(lot.as_mut_ptr().cast());
    c.set_abandoned(-1);
    run(0x0070_55B0, Conv::Thiscall, td.as_ptr() as u32, &[c.as_ptr() as u32]);
    assert_eq!((c.occupied(), c.abandoned(), c.stage(), c.wealth()), (0, 0, 0, 0));
    assert_eq!(
        w.calls().iter().map(|c| c.0).collect::<Vec<_>>(),
        [0x18, 0xC8, 0x84, 0x24, 0xB8]
    );
}

#[test]
fn aggregation_sums_in_tree_order_and_keeps_maximum_stage_and_signed_wealth() {
    let mut w = World::new();
    let td = w.view();
    let mut info = [[0, 2u32], [0, u32::MAX], [0, 3]];
    let mut cfg = [
        [w.configuration_table.as_ptr() as u32, 0xFE],
        [w.configuration_table.as_ptr() as u32, 7],
    ];
    let mut lots = [
        w.lot(0xAAAA_1234, 4, cfg[0].as_mut_ptr() as u32, info[0].as_mut_ptr() as u32),
        w.lot(0xBBBB_FFFF, 0, cfg[1].as_mut_ptr() as u32, info[1].as_mut_ptr() as u32),
        w.lot(0xCCCC_FFFF, 0, 0, info[2].as_mut_ptr() as u32),
    ];
    let mut nodes = [[0u32; 5]; 4];
    let header = nodes[3].as_mut_ptr() as u32;
    for i in 0..3 {
        nodes[i][0] = if i == 2 {
            header
        } else {
            nodes[i + 1].as_mut_ptr() as u32
        };
        nodes[i][4] = lots[i].as_mut_ptr() as u32;
    }
    nodes[3][2] = nodes[0].as_mut_ptr() as u32;
    let mut memory = [0u8; 0xA4];
    let c = unsafe { Candidate::from_ptr(memory.as_mut_ptr()) };
    c.set_kind(3);
    c.set_lot_header(header);
    c.set_occupied(-1);
    assert_eq!(
        run(0x0070_55B0, Conv::Thiscall, td.as_ptr() as u32, &[c.as_ptr() as u32]).0,
        header
    );
    assert_eq!(
        (c.occupied(), c.abandoned(), c.stage(), c.wealth()),
        (131070, 0x1234, 254, 3)
    );
    let calls = w.calls();
    assert_eq!(
        calls.iter().filter(|c| c.0 == 0xC8).map(|c| c.1).collect::<Vec<_>>(),
        lots.iter().map(|l| l.as_ptr() as u32).collect::<Vec<_>>()
    );
    assert_eq!(calls.iter().filter(|c| c.0 == 0x005D_17C0).count(), 3);
    assert_eq!(calls.last(), Some(&Call(0x005D_17C0, 0, 0)));
    nodes[3][2] = header;
    c.set_abandoned(-1);
    assert_eq!(
        run(0x0070_55B0, Conv::Thiscall, td.as_ptr() as u32, &[c.as_ptr() as u32]).0,
        header
    );
    assert_eq!((c.occupied(), c.abandoned(), c.stage(), c.wealth()), (0, 0, 0, 0));
    assert!(w.calls().is_empty());
    assert_eq!(nodes[3][2], header);
}

#[test]
fn scoring_weights_and_lot_costs_match_core_cases() {
    let _fpu = GameFpu::new();
    let _machine = crate::tract_developer::testing::PreserveMachine::new();
    for (zone, expected) in [
        (1, [218.0f32, 262.0, 277.0, 273.0]),
        (2, [0.0, 44.0, 59.0, 55.0]),
        (3, [-109.0, -65.0, -65.0, -65.0]),
        (0, [0.0, 44.0, 59.0, 55.0]),
    ] {
        let mut w = World::new();
        w._fakes
            .push(fake(0x0070_55B0, capacity_for_score as *const () as usize));
        w._fakes.push(fake(0x0091_372E, uniform as *const () as usize));
        SETTINGS.with(|s| {
            let s = &mut *s.borrow_mut();
            s.zone = zone;
            s.draw = 0.25;
        });
        let td = w.view();
        td.set_occupied_cost(3.0);
        td.set_abandoned_cost(7.0);
        td.set_aggregation_cost(5.0);
        td.set_subdivision_cost(11.0);
        let mut road = [1i32, 2, 3, 4];
        let mut avenue = [2i32, 3, 4, 5];
        let mut rail = [3i32, 4, 5, 6];
        let mut candidates = [[0xA5u8; 0xA4]; 4];
        let mut pointers = [0u32; 4];
        for (i, memory) in candidates.iter_mut().enumerate() {
            pointers[i] = memory.as_mut_ptr() as u32;
            let c = unsafe { Candidate::from_ptr(memory.as_mut_ptr()) };
            c.set_kind(i as i32 + 1);
            c.set_occupied(10);
            c.set_abandoned(2);
            c.set_lot_count(4);
            unsafe {
                store(pointers[i], 0x7C, road.as_mut_ptr() as u32);
                store(pointers[i], 0x88, avenue.as_mut_ptr() as u32);
                store(pointers[i], 0x94, rail.as_mut_ptr() as u32);
            }
        }
        td.set_candidates_begin(pointers.as_ptr() as u32);
        td.set_candidates_end(pointers.as_ptr() as u32 + 16);
        let before = candidates;
        assert_eq!(
            run(0x0070_5750, Conv::Thiscall, td.as_ptr() as u32, &[]).0,
            td.candidates_end()
        );
        for i in 0..4 {
            let mut wanted = before[i];
            wanted[0x34..0x38].copy_from_slice(&expected[i].to_bits().to_le_bytes());
            assert_eq!(candidates[i], wanted, "zone {zone}, kind {}", i + 1);
        }
        let mut expected_calls = vec![Call(0x14, td.developer(), 0)];
        for (i, p) in pointers.into_iter().enumerate() {
            if i != 0 {
                expected_calls.push(Call(0x0070_55B0, td.as_ptr() as u32, p));
            }
            if zone == 3 {
                expected_calls.push(Call(0x0091_372E, &*w.random as *const u8 as u32, 0));
            }
        }
        assert_eq!(w.calls(), expected_calls);
    }
}

#[test]
fn scoring_keeps_register_sum_across_stores_and_loads_large_integers_exactly() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    SETTINGS.with(|s| s.borrow_mut().zone = 1);
    let td = w.view();
    let mut road = [0i32; 4];
    let mut avenue = [0i32; 4];
    let mut rail = [0i32; 4];
    let mut memory = [0u8; 0xA4];
    let c = unsafe { Candidate::from_ptr(memory.as_mut_ptr()) };
    c.set_kind(1);
    let mut pointers = [memory.as_mut_ptr() as u32];
    unsafe {
        store(pointers[0], 0x7C, road.as_mut_ptr() as u32);
        store(pointers[0], 0x88, avenue.as_mut_ptr() as u32);
        store(pointers[0], 0x94, rail.as_mut_ptr() as u32);
    }
    td.set_candidates_begin(pointers.as_mut_ptr() as u32);
    td.set_candidates_end(td.candidates_begin() + 4);
    for (roads, avenues, rails) in [
        ([i32::MAX, 0, 0, 0], [-1_073_741_824, 0, 0, 0], [0; 4]),
        ([16_777_217, -16_777_216, 1, -1], [0; 4], [0; 4]),
        ([0; 4], [0; 4], [i32::MAX, i32::MIN, 1, -1]),
    ] {
        road.copy_from_slice(&roads);
        avenue.copy_from_slice(&avenues);
        rail.copy_from_slice(&rails);
        let mut expected = 0f32;
        // A separate x87 expression retains the accumulator through all four side stores.
        unsafe {
            core::arch::asm!(
                "fild dword ptr [{a}]", "fmul dword ptr [{weights}+4]",
                "fild dword ptr [{l}]", "fmul dword ptr [{weights}+8]", "faddp",
                "fild dword ptr [{r}]", "fmul dword ptr [{weights}]", "faddp", "fldz", "faddp",
                "fild dword ptr [{r}+4]", "fmul dword ptr [{weights}]", "fild dword ptr [{a}+4]", "fmul dword ptr [{weights}+4]", "faddp",
                "fild dword ptr [{l}+4]", "fmul dword ptr [{weights}+8]", "faddp", "faddp",
                "fild dword ptr [{r}+8]", "fmul dword ptr [{weights}]", "fild dword ptr [{a}+8]", "fmul dword ptr [{weights}+4]", "faddp",
                "fild dword ptr [{l}+8]", "fmul dword ptr [{weights}+8]", "faddp", "faddp",
                "fild dword ptr [{r}+12]", "fmul dword ptr [{weights}]", "fild dword ptr [{a}+12]", "fmul dword ptr [{weights}+4]", "faddp",
                "fild dword ptr [{l}+12]", "fmul dword ptr [{weights}+8]", "faddp", "fadd st(0), st(1)",
                "fstp dword ptr [{o}]", "fstp st(0)",
                r = in(reg) road.as_ptr(), a = in(reg) avenue.as_ptr(), l = in(reg) rail.as_ptr(),
                weights = in(reg) [1.0f32, 2.0, 10.0].as_ptr(), o = in(reg) &mut expected,
                out("st(0)") _, out("st(1)") _, out("st(2)") _,
            );
        }
        run(0x0070_5750, Conv::Thiscall, td.as_ptr() as u32, &[]);
        assert_eq!(c.score().to_bits(), expected.to_bits());
    }
    assert_eq!(crate::x87::control_word(), 0x007F);
}

#[test]
fn scoring_unsigned_lot_count_and_nonfinite_costs() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w._fakes
        .push(fake(0x0070_55B0, capacity_for_score as *const () as usize));
    SETTINGS.with(|s| s.borrow_mut().zone = 2);
    let td = w.view();
    td.set_aggregation_cost(1.0);
    let mut memory = [0u8; 0xA4];
    let c = unsafe { Candidate::from_ptr(memory.as_mut_ptr()) };
    c.set_kind(3);
    let mut pointers = [memory.as_mut_ptr() as u32];
    td.set_candidates_begin(pointers.as_mut_ptr() as u32);
    td.set_candidates_end(td.candidates_begin() + 4);
    for count in [0u32, 1, 0x8000_0000, u32::MAX] {
        c.set_lot_count(count);
        run(0x0070_5750, Conv::Thiscall, td.as_ptr() as u32, &[]);
        let integer = count.wrapping_sub(1);
        let mut expected = F80::from_i32(integer as i32);
        if (integer as i32) < 0 {
            expected = expected + F80::from_f32(4_294_967_296.0);
        }
        assert_eq!(c.score().to_bits(), expected.to_f32().to_bits());
    }
    c.set_kind(2);
    c.set_occupied(i32::MAX);
    c.set_abandoned(i32::MAX);
    td.set_occupied_cost(1.0e30);
    td.set_abandoned_cost(-1.0e30);
    run(0x0070_5750, Conv::Thiscall, td.as_ptr() as u32, &[]);
    assert!(c.score().is_finite());
    SETTINGS.with(|s| s.borrow_mut().score_from_capacity = f32::NAN);
    run(0x0070_5750, Conv::Thiscall, td.as_ptr() as u32, &[]);
    assert!(c.score().is_nan());
    let before = snapshot(td.as_ptr(), 0x180);
    td.set_candidates_end(td.candidates_begin());
    assert_eq!(run(0x0070_5750, Conv::Thiscall, td.as_ptr() as u32, &[]).0, 0);
    td.set_candidates_end(td.candidates_begin() + 4);
    assert_eq!(snapshot(td.as_ptr(), 0x180), before);
}
