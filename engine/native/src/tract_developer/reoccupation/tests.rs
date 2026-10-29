// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Reoccupation eligibility, all cap failures, threshold arithmetic, and application order.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, u32, Vec<u32>);

struct State {
    calls: Vec<Call>,
    memory: Vec<Box<[u32]>>,
    lots: Vec<u32>,
    states: Vec<u32>,
    building: u32,
    wealth: u32,
    occupant_wealth: u32,
    capacity: u32,
    available: u32,
    cached: u32,
    suitability: u32,
    zone: u32,
    configuration: u32,
    profile: u32,
    threshold_developer: u32,
    grids: [u32; 3],
    stage: u32,
    limits: [u32; 4],
    cells: [u32; 4],
    ignore: [u32; 3],
    reverse: bool,
    score_clears: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            memory: Vec::new(),
            lots: Vec::new(),
            states: Vec::new(),
            building: 0x1234,
            wealth: 2,
            occupant_wealth: 1,
            capacity: 2,
            available: 2,
            cached: 2,
            suitability: 27,
            zone: 1,
            configuration: 0,
            profile: 0,
            threshold_developer: 0,
            grids: [0; 3],
            stage: 4,
            limits: [0; 4],
            cells: [2, 1, 1, 1],
            ignore: [0; 3],
            reverse: false,
            score_clears: false,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn answer(name: &'static str, this: u32, args: &[u32]) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call(name, this, args.to_vec()));
        let value = match name {
            "target" => 1,
            "type" => 7,
            "wealth" => s.wealth,
            "state" => s.states[s.lots.iter().position(|&p| p == this).unwrap()],
            "occupant" => s.occupant_wealth,
            "building" => {
                assert_eq!(args, [0]);
                s.building
            }
            "zone" => 1,
            "capacity" => {
                assert_eq!(args, [7, 1]);
                s.capacity
            }
            "available" => {
                assert_eq!(args, [7, 1]);
                s.available
            }
            "configuration" => s.configuration,
            "stage" => s.stage,
            "profile" => {
                assert_eq!(args, [s.building]);
                s.profile
            }
            "developer" => {
                assert_eq!(args, [1, 2]);
                s.threshold_developer
            }
            "ignore_caps" => s.ignore[0],
            "ignore_power" => s.ignore[1],
            "ignore_water" => s.ignore[2],
            "power_limit" => s.limits[0],
            "water_limit" => s.limits[1],
            "fire_limit" => s.limits[2],
            "park_limit" => s.limits[3],
            "power_grid" => s.grids[0],
            "fire_grid" => s.grids[1],
            "park_grid" => s.grids[2],
            "power_cell" => s.cells[0],
            "water_cell" => s.cells[1],
            "fire_cell" => s.cells[2],
            "park_cell" => s.cells[3],
            "location" => {
                unsafe {
                    store(args[0], 0, 9);
                    store(args[1], 0, 11);
                }
                0
            }
            "size" => {
                unsafe {
                    write_unaligned(args[0] as *mut u8, 2);
                    write_unaligned(args[1] as *mut u8, 3);
                }
                0
            }
            "facing" => 2,
            "abandon" | "reoccupy" => 0,
            _ => panic!("unknown slot {name}"),
        };
        0xAABB_CCDD_0000_0000 | u64::from(value)
    })
}
macro_rules! slot {
    ($fn:ident, $name:literal $(, $arg:ident)*) => {
        extern "thiscall" fn $fn(this: u32 $(, $arg: u32)*) -> u64 { answer($name, this, &[$($arg),*]) }
    };
}
slot!(target, "target");
slot!(dev_type, "type");
slot!(wealth, "wealth");
slot!(lot_state, "state");
slot!(occupant, "occupant");
slot!(building, "building", zero);
slot!(lot_zone, "zone");
slot!(lot_capacity, "capacity", kind, flag);
slot!(available, "available", kind, flag);
slot!(configuration, "configuration");
slot!(stage, "stage");
slot!(profile, "profile", building);
slot!(developer, "developer", purpose, wealth);
slot!(ignore_caps, "ignore_caps");
slot!(ignore_power, "ignore_power");
slot!(ignore_water, "ignore_water");
slot!(power_limit, "power_limit");
slot!(water_limit, "water_limit");
slot!(fire_limit, "fire_limit");
slot!(park_limit, "park_limit");
slot!(power_grid, "power_grid");
slot!(fire_grid, "fire_grid");
slot!(park_grid, "park_grid");
slot!(water_cell, "water_cell", x, z);
slot!(location, "location", x, z);
slot!(size, "size", w, d);
slot!(facing, "facing");
slot!(abandon, "abandon", lot, reason);
slot!(reoccupy, "reoccupy", lot, kind);

extern "thiscall" fn cell(this: u32, x: u32, z: u32) -> u64 {
    // Each grid has its requested service kind in its second word.
    let name = match unsafe { word(this, 4) } {
        0 => "power_cell",
        2 => "fire_cell",
        _ => "park_cell",
    };
    answer(name, this, &[x, z])
}

extern "C" fn zone(zone: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("convert", 0, vec![zone]));
        0xAABB_CCDD_0000_0000 | u64::from(s.zone)
    })
}
extern "thiscall" fn suitable(this: u32, lot: u32, check: u32) -> u64 {
    assert_eq!(check, 0);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("suitable", this, vec![lot, check]));
        0xAABB_CCDD_0000_0000 | u64::from(s.suitability)
    })
}
fn memory(s: &mut State, words: usize) -> u32 {
    let mut block = vec![0u32; words].into_boxed_slice();
    let p = block.as_mut_ptr() as u32;
    s.memory.push(block);
    p
}
extern "C" fn acquire() -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let p = memory(s, 41);
        s.memory.last_mut().unwrap().fill(0xCCCC_CCCC);
        s.calls.push(Call("acquire", 0, vec![p]));
        0xAABB_CCDD_0000_0000 | u64::from(p)
    })
}
extern "thiscall" fn capacity(this: u32, candidate: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("existing", this, vec![candidate]));
        unsafe { store(candidate, 0x28, s.cached) };
    });
    0xAABB_CCDD_1234_5678
}
extern "thiscall" fn insert(this: u32, at: u32, value: u32, _allocator: u32, count: u32, unit: u32) -> u64 {
    assert_eq!((count, unit), (1, 1));
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let p = unsafe { word(value, 0) };
        s.calls.push(Call("insert", this, vec![p]));
        unsafe {
            assert_eq!(at, word(this, 8));
            assert_eq!(word(this, 0), 0);
            let begin = memory(s, 32);
            store(begin, 0, p);
            store(this, 0, begin);
            store(this, 4, begin + 4);
            store(this, 8, begin + 128);
        }
    });
    0xAABB_CCDD_2345_6789
}
extern "thiscall" fn score(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("score", this, vec![]));
        if s.score_clears {
            unsafe { store(this, 0xE8, word(this, 0xE4)) };
        }
    });
    0xAABB_CCDD_3456_789A
}
extern "C" fn sort(begin: u32, end: u32, compare: u32) -> u64 {
    assert_eq!(compare, 0x0070_4770);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("sort", 0, vec![begin, end, compare]));
        if s.reverse {
            unsafe { std::slice::from_raw_parts_mut(begin as *mut u32, ((end - begin) / 4) as usize).reverse() };
        }
    });
    0xAABB_CCDD_4567_89AB
}
extern "C" fn clear(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call("clear", this, vec![])));
    unsafe { store(this, 4, word(this, 0)) };
    0xAABB_CCDD_5678_9ABC
}

struct Fixture {
    object: Object,
    tables: Vec<Box<[u32]>>,
    objects: Vec<Box<[u32]>>,
    nodes: Vec<Box<[u32]>>,
    header: Box<[u32; 5]>,
    row: Box<[u32; 28]>,
    context: Box<[u32; 4]>,
    profile: Box<[u32; 3]>,
    candidates: Box<[u32; 16]>,
    pool_buffer: Box<[u32; 16]>,
    pool: Box<[u32; 3]>,
    fire: Box<u32>,
    aura: Box<u32>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn service(&mut self, slots: &[(usize, u32)], tag: u32) -> u32 {
        let mut table = vec![0; 64].into_boxed_slice();
        for &(offset, function) in slots {
            table[offset / 4] = function;
        }
        let table_ptr = table.as_ptr() as u32;
        self.tables.push(table);
        let mut object = vec![table_ptr, tag].into_boxed_slice();
        let p = object.as_mut_ptr() as u32;
        self.objects.push(object);
        p
    }
    fn new(n: usize) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut f = Self {
            object: Object::new(0x188),
            tables: Vec::new(),
            objects: Vec::new(),
            nodes: Vec::new(),
            header: Box::new([0; 5]),
            row: Box::new([u32::MAX; 28]),
            context: Box::new([0; 4]),
            profile: Box::new([1, 2, 2]),
            candidates: Box::new([0; 16]),
            pool_buffer: Box::new([0; 16]),
            pool: Box::new([0; 3]),
            fire: Box::new(0),
            aura: Box::new(0),
            _fakes: Vec::new(),
        };
        let dev = f.service(
            &[
                (0x14, target as *const () as u32),
                (0x18, dev_type as *const () as u32),
                (0x20, wealth as *const () as u32),
                (0x54, fire_limit as *const () as u32),
                (0x58, park_limit as *const () as u32),
            ],
            0,
        );
        let actual = f.service(
            &[
                (0x4C, power_limit as *const () as u32),
                (0x50, water_limit as *const () as u32),
            ],
            0,
        );
        let bds = f.service(
            &[
                (0x9C, profile as *const () as u32),
                (0x84, developer as *const () as u32),
                (0xE0, ignore_caps as *const () as u32),
                (0xDC, ignore_power as *const () as u32),
                (0xD8, ignore_water as *const () as u32),
            ],
            0,
        );
        let ld = f.service(
            &[
                (0x20, abandon as *const () as u32),
                (0x24, reoccupy as *const () as u32),
            ],
            0,
        );
        let configuration = f.service(&[(0x2C, stage as *const () as u32)], 0);
        let grids = [
            f.service(&[(0x1C, cell as *const () as u32)], 0),
            f.service(&[(0x1C, cell as *const () as u32)], 2),
            f.service(&[(0x1C, cell as *const () as u32)], 3),
        ];
        let power = f.service(&[(0x8C, power_grid as *const () as u32)], 0);
        let water = f.service(&[(0x2C, water_cell as *const () as u32)], 0);
        let fire = f.service(&[(0x14, fire_grid as *const () as u32)], 0);
        let aura = f.service(&[(0x38, park_grid as *const () as u32)], 0);
        *f.fire = fire;
        *f.aura = aura;
        let header = f.header.as_mut_ptr() as u32;
        f.header[2] = header;
        f.header[3] = header;
        for _ in 0..n {
            let lot = f.service(
                &[
                    (0x84, lot_state as *const () as u32),
                    (0xA4, occupant as *const () as u32),
                    (0xB8, building as *const () as u32),
                    (0x7C, lot_zone as *const () as u32),
                    (0xC8, lot_capacity as *const () as u32),
                    (0xD0, available as *const () as u32),
                    (0x24, super::tests::configuration as *const () as u32),
                    (0x54, location as *const () as u32),
                    (0x58, size as *const () as u32),
                    (0x5C, facing as *const () as u32),
                ],
                0,
            );
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.lots.push(lot);
                s.states.push(4);
            });
            f.nodes.push(vec![0u32, header, 0, 0, lot].into_boxed_slice());
        }
        let node_pointers: Vec<_> = f.nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        for (i, &node) in node_pointers.iter().enumerate() {
            unsafe {
                store(node, 4, if i == 0 { header } else { node_pointers[i - 1] });
                store(node, 0xC, node_pointers.get(i + 1).copied().unwrap_or(0));
            }
        }
        if n != 0 {
            f.header[1] = node_pointers[0];
            f.header[2] = node_pointers[0];
            f.header[3] = node_pointers[n - 1];
        }
        f.context[3] = f.row.as_mut_ptr() as u32;
        let p = f.object.ptr() as u32;
        let begin = f.candidates.as_mut_ptr() as u32;
        let pool_begin = f.pool_buffer.as_mut_ptr() as u32;
        *f.pool = [pool_begin, pool_begin, pool_begin + 64];
        unsafe {
            store(p, 0x1C, bds);
            store(p, 0x28, ld);
            store(p, 0x80, dev);
            store(p, 0x30, water);
            store(p, 0x34, power);
            store(p, 0xCC, header);
            store(p, 0x170, f.context.as_mut_ptr() as u32);
            store(p, 0xB8, 1);
            store(p, 0xBC, 3);
            store(p, 0x44, 1.0f32.to_bits());
            store(p, 0x48, 1.0f32.to_bits());
            store(p, 0xE4, begin);
            store(p, 0xE8, begin);
            store(p, 0xEC, begin + 64);
            write_unaligned((p + 0x6C) as *mut u8, 1);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.profile = f.profile.as_mut_ptr() as u32;
            s.threshold_developer = actual;
            s.grids = grids;
            // Keep the configuration available for cap tests without enabling it by default.
            s.calls.push(Call("configuration_pointer", configuration, vec![]));
        });
        f._fakes = [
            (0x006B_3890, zone as *const () as usize),
            (0x0070_7A60, suitable as *const () as usize),
            (0x0070_7D90, acquire as *const () as usize),
            (0x0070_55B0, capacity as *const () as usize),
            (0x0051_CA60, insert as *const () as usize),
            (0x0070_5750, score as *const () as usize),
            (0x0070_9520, sort as *const () as usize),
            (0x0070_7370, clear as *const () as usize),
        ]
        .into_iter()
        .map(|(a, p)| fake(a, p))
        .collect();
        f._fakes.push(fake_data(0x00B4_AFCC, f.pool.as_mut_ptr().cast()));
        f._fakes.push(fake_data(0x00B4_3D78, (&mut *f.fire as *mut u32).cast()));
        f._fakes.push(fake_data(0x00B4_3D60, (&mut *f.aura as *mut u32).cast()));
        f
    }
    fn run(&mut self) -> (u32, u32, u32) {
        STATE.with(|s| s.borrow_mut().calls.retain(|c| c.0 != "configuration_pointer"));
        run(0x0070_A3D0, Conv::Thiscall, self.object.ptr() as u32, &[])
    }
    fn caps(&mut self) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.configuration = s.calls.iter().find(|c| c.0 == "configuration_pointer").unwrap().1;
        });
    }
}

fn names() -> Vec<&'static str> {
    STATE.with(|s| s.borrow().calls.iter().map(|c| c.0).collect())
}

#[test]
fn empty_lot_set_preserves_memory_and_returns_one_with_only_the_nothing_counter() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new(0);
    let p = f.object.ptr();
    let before = snapshot(p, 0x188);
    assert_eq!(f.run().0, 1);
    assert_eq!(snapshot(p, 0x188), before);
    assert_eq!(names(), ["target", "type"]);
    assert_eq!(f.row[1], 0);
    assert_eq!(f.row.iter().filter(|&&n| n == 0).count(), 1);
}

#[test]
fn every_lot_reads_wealth_before_eligibility_and_occupied_upgrades_use_signed_bounds() {
    let _fpu = GameFpu::new();
    for (state, kick, occupant, wealth, eligible) in [
        (4, 0, 3, 1, true),
        (3, 0, 1, 2, false),
        (3, 1, 1, 2, true),
        (3, 1, 2, 2, false),
        (3, 1, 0xFFFF_FFFF, 0, true),
        (3, 1, 1, 3, false),
        (3, 1, 0x8000_0000, 0x7FFF_FFFF, false),
        (2, 1, 1, 2, false),
    ] {
        let mut f = Fixture::new(1);
        let p = f.object.ptr() as u32;
        unsafe {
            write_unaligned((p + 0x6C) as *mut u8, kick);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.states[0] = state;
            s.occupant_wealth = occupant;
            s.wealth = wealth;
            s.suitability = 6;
        });
        assert_eq!(f.run().0, if eligible { 6 } else { 1 });
        let mut expected = vec!["target", "type", "state", "occupant", "building", "profile", "wealth"];
        if eligible {
            expected.extend(["zone", "convert", "type", "capacity", "suitable"]);
        }
        assert_eq!(names(), expected);
    }
}

#[test]
fn missing_building_zone_mismatch_zero_capacity_and_suitability_failures_keep_their_order() {
    let _fpu = GameFpu::new();
    for mode in 0..5 {
        let mut f = Fixture::new(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            match mode {
                0 => {
                    s.building = 0;
                    s.states[0] = 2;
                }
                1 => s.zone = 2,
                2 => s.capacity = 0x1234_0000,
                3 => s.suitability = 21,
                _ => s.suitability = u32::MAX,
            }
        });
        let expected = [1, 1, 15, 21, 1][mode];
        assert_eq!(f.run().0, expected);
        assert_eq!(f.row[15], if mode == 2 { 0 } else { u32::MAX });
        assert!(!names().contains(&"acquire"));
        if mode == 0 {
            assert!(!names().contains(&"profile"));
        }
    }
}

#[test]
fn all_four_caps_count_failures_in_order_and_recycle_without_scoring() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new(1);
    f.caps();
    STATE.with(|s| s.borrow_mut().cells = [1, 0, 0, 0]);
    assert_eq!(f.run().0, 26);
    let n = names();
    assert_eq!(
        &n[n.iter().position(|&n| n == "ignore_caps").unwrap()..],
        [
            "ignore_caps",
            "configuration",
            "stage",
            "building",
            "profile",
            "developer",
            "power_limit",
            "ignore_power",
            "power_grid",
            "power_cell",
            "water_limit",
            "ignore_water",
            "water_cell",
            "fire_limit",
            "fire_grid",
            "fire_cell",
            "park_limit",
            "park_grid",
            "park_cell",
        ]
    );
    assert_eq!([f.row[26], f.row[25], f.row[23], f.row[24]], [0; 4]);
    assert_eq!(f.pool[1] - f.pool[0], 4);
    assert!(!n.contains(&"score"));
    STATE.with(|s| {
        for call in s.borrow().calls.iter().filter(|c| c.0.ends_with("cell")) {
            assert_eq!(call.2, [9, 11]);
        }
    });
}

#[test]
fn cap_threshold_bytes_cheats_and_coverage_return_widths_are_preserved() {
    let _fpu = GameFpu::new();
    for (limits, ignore, cells, result) in [
        ([0x1004; 4], [0; 3], [0; 4], 27),
        ([0; 4], [0x1234_5601, 0, 0], [0; 4], 27),
        ([0; 4], [0, 1, 1], [0, 0, 1, 1], 27),
        ([0; 4], [0; 3], [0x1234_0002, 0x100, 1, 1], 25),
        ([0; 4], [0; 3], [2, 1, 0x100, 0x10000], 24),
    ] {
        let mut f = Fixture::new(1);
        f.caps();
        // Each service returns a different grid, with the original byte or word result width.
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.limits = limits;
            s.ignore = ignore;
            s.cells = cells;
        });
        assert_eq!(f.run().0, result);
    }
}

#[test]
fn capacity_fallbacks_historical_lots_and_zero_or_negative_capacities_use_the_pool() {
    let _fpu = GameFpu::new();
    for (state, cached, available, suitability, expected) in [
        (4, 2, 0, 18, 27),
        (4, 0, 2, 27, 27),
        (4, 0, 0, 27, 1),
        (3, 0, 0, 27, 27),
        (4, 0x8000_0000, 2, 27, 1),
        (4, 0xFFFF_FFFF, 2, 27, 1),
    ] {
        let mut f = Fixture::new(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.states[0] = state;
            s.cached = cached;
            s.available = available;
            s.suitability = suitability;
        });
        assert_eq!(f.run().0, expected);
        assert_eq!(
            names().iter().filter(|&&n| n == "available").count(),
            usize::from(cached == 0)
        );
        assert_eq!(
            names().iter().filter(|&&n| n == "abandon").count(),
            usize::from(state == 3 && expected == 27)
        );
    }
}

#[test]
fn candidates_follow_sort_order_and_stop_only_after_strictly_exceeding_the_maximum() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new(3);
    let p = f.object.ptr() as u32;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.reverse = true;
        s.states[2] = 3;
    });
    assert_eq!(f.run().0, 27);
    assert_eq!(unsafe { word(p, 0xC0) }, 4);
    STATE.with(|s| {
        let s = s.borrow();
        let applied: Vec<_> = s
            .calls
            .iter()
            .filter(|c| c.0 == "reoccupy")
            .map(|c| c.2.clone())
            .collect();
        assert_eq!(applied, [vec![s.lots[2], 7], vec![s.lots[1], 7]]);
        assert!(s
            .calls
            .contains(&Call("abandon", unsafe { word(p, 0x28) }, vec![s.lots[2], 5])));
    });
    assert_eq!(f.row[27], 0);
    assert_eq!(unsafe { word(p, 0xE4) }, unsafe { word(p, 0xE8) });
}

#[test]
fn minimum_equality_does_not_score_and_nan_or_infinite_thresholds_use_the_game_integer_helper() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for (minimum, expected) in [(2.0f32, 1), (1.99, 27), (-1.0, 27), (f32::NAN, 27), (f32::INFINITY, 27)] {
        let mut f = Fixture::new(1);
        let p = f.object.ptr() as u32;
        unsafe {
            store(p, 0x44, minimum.to_bits());
        }
        assert_eq!(f.run().0, expected);
        assert_eq!(names().contains(&"score"), expected == 27);
        assert!(names().contains(&"clear"));
    }
}

#[test]
fn full_candidate_and_pool_vectors_call_the_original_growth_helper() {
    let _fpu = GameFpu::new();
    for accepted in [false, true] {
        let mut f = Fixture::new(1);
        let p = f.object.ptr() as u32;
        unsafe {
            store(p, 0xE4, 0);
            store(p, 0xE8, 0);
            store(p, 0xEC, 0);
        }
        *f.pool = [0; 3];
        STATE.with(|s| s.borrow_mut().cached = if accepted { 2 } else { 0xFFFF_FFFF });
        assert_eq!(f.run().0, if accepted { 27 } else { 1 });
        STATE.with(|s| {
            let s = s.borrow();
            let call = s.calls.iter().find(|c| c.0 == "insert").unwrap();
            assert_eq!(call.1, if accepted { p + 0xE4 } else { f.pool.as_ptr() as u32 });
        });
    }
}

#[test]
fn scoring_callback_can_empty_the_candidate_vector_before_sorting_and_application() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new(1);
    STATE.with(|s| s.borrow_mut().score_clears = true);
    assert_eq!(f.run().0, 27);
    assert!(!names().contains(&"sort"));
    assert!(!names().contains(&"reoccupy"));
    assert_eq!(f.row[27], 0);
}

#[test]
fn candidate_fields_and_padding_and_wrapping_built_capacity_match_the_working_record() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new(1);
    let p = f.object.ptr() as u32;
    unsafe {
        store(p, 0xC0, i32::MAX as u32);
        store(p, 0xB8, i32::MIN as u32);
        store(p, 0xBC, 0x8000_0002);
    }
    assert_eq!(f.run().0, 27);
    assert_eq!(unsafe { word(p, 0xC0) }, 0x8000_0001);
    STATE.with(|s| {
        let s = s.borrow();
        let candidate = s.calls.iter().find(|c| c.0 == "acquire").unwrap().2[0];
        let mut expected = vec![0xCCu8; 0xA4];
        for (offset, value) in [(0, 1), (4, s.lots[0]), (8, 9), (0xC, 11), (0x10, 2), (0x28, 2)] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        expected[0x14] = 2;
        expected[0x15] = 3;
        assert_eq!(snapshot(candidate as *mut u8, 0xA4), expected);
    });
}

#[test]
fn maximum_threshold_is_strict_and_nonfinite_truncation_keeps_the_helper_result() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for (multiplier, taken) in [(0.0f32, 1), (1.0, 2), (-1.0, 1), (f32::NAN, 1), (f32::INFINITY, 1)] {
        let mut f = Fixture::new(3);
        let p = f.object.ptr() as u32;
        unsafe { store(p, 0x48, multiplier.to_bits()) };
        assert_eq!(f.run().0, 27);
        assert_eq!(unsafe { word(p, 0xC0) }, taken * 2);
    }
    for (multiplier, edx) in [
        (0.0f32, 0),
        (-1.0, u32::MAX),
        (f32::NAN, 0x8000_0000),
        (f32::INFINITY, 0x8000_0000),
    ] {
        let mut f = Fixture::new(0);
        let p = f.object.ptr() as u32;
        unsafe { store(p, 0x48, multiplier.to_bits()) };
        let result = f.run();
        assert_eq!((result.0, result.2), (1, edx));
    }
}
