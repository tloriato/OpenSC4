// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate construction and selection with game-layout trees and scripted callees.

use super::*;
use crate::testing::{fake, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Call(&'static str, u32, Vec<u32>);
struct State {
    calls: Vec<Call>,
    candidates: Vec<Object>,
    next: usize,
    snapshots: Vec<Vec<u8>>,
    legal: Vec<u32>,
    compatible: Vec<u32>,
    legal_index: usize,
    compatible_index: usize,
    fits: u32,
    zone: u32,
    maximum: f64,
    lot_state: u32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            candidates: vec![],
            next: 0,
            snapshots: vec![],
            legal: vec![1; 16],
            compatible: vec![27; 16],
            legal_index: 0,
            compatible_index: 0,
            fits: 1,
            zone: 0x1234_0002,
            maximum: 1.0,
            lot_state: 0,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
const RESULT: u64 = 0xDEAD_BEEF_1234_5678;
fn record(name: &'static str, this: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(name, this, args.to_vec())));
}
extern "thiscall" fn size(this: u32, width: *mut u8, depth: *mut u8) -> u64 {
    record("size", this, &[]);
    unsafe {
        *width = word(this, 4) as u8;
        *depth = word(this, 8) as u8;
    }
    RESULT
}
extern "thiscall" fn dimensions(this: u32, width: *mut u32, depth: *mut u32) -> u64 {
    record("dimensions", this, &[]);
    unsafe {
        *width = word(this, 4);
        *depth = word(this, 8);
    }
    RESULT
}
extern "thiscall" fn location(this: u32, x: *mut u32, z: *mut u32) -> u64 {
    record("location", this, &[]);
    unsafe {
        *x = word(this, 12);
        *z = word(this, 16);
    }
    RESULT
}
extern "thiscall" fn slope(this: u32) -> f64 {
    record("slope", this, &[]);
    f64::from(f32::from_bits(unsafe { word(this, 20) }))
}
extern "thiscall" fn minimum(this: u32) -> f64 {
    record("minimum", this, &[]);
    f64::from(f32::from_bits(unsafe { word(this, 20) }))
}
extern "thiscall" fn maximum(this: u32) -> f64 {
    record("maximum", this, &[]);
    STATE.with(|s| s.borrow().maximum)
}
extern "thiscall" fn facing(this: u32) -> u64 {
    record("facing", this, &[]);
    (RESULT & !0xFFFF_FFFF) | u64::from(unsafe { word(this, 24) })
}
extern "thiscall" fn stage(this: u32) -> u64 {
    record("stage", this, &[]);
    0xDEAD_BEEF_1122_3304
}
extern "thiscall" fn zone(this: u32, x: u32, z: u32) -> u64 {
    record("zone", this, &[x, z]);
    STATE.with(|s| (RESULT & !0xFFFF_FFFF) | u64::from(s.borrow().zone))
}
extern "C" fn stage_fits(stage: u32, zone: u32) -> u64 {
    record("fits", 0, &[stage, zone]);
    STATE.with(|s| (RESULT & !0xFFFF_FFFF) | u64::from(s.borrow().fits))
}
extern "C" fn take() -> u64 {
    record("take", 0, &[]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let p = s.candidates[s.next].ptr();
        s.next += 1;
        (RESULT & !0xFFFF_FFFF) | u64::from(p as u32)
    })
}
extern "C" fn give_back(candidate: u32) -> u64 {
    record("return", 0, &[candidate]);
    RESULT
}
extern "thiscall" fn networks(this: u32, candidate: u32) -> u64 {
    record("networks", this, &[candidate]);
    STATE.with(|s| s.borrow_mut().snapshots.push(snapshot(candidate as *mut u8, 0xA4)));
    unsafe {
        store(candidate, 0x38, 10);
        store(candidate, 0x3C, 20);
        store(candidate, 0x40, 13);
        store(candidate, 0x44, 23);
    }
    RESULT
}
extern "thiscall" fn legal(this: u32, candidate: u32) -> u64 {
    record("legal", this, &[candidate]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let value = s.legal[s.legal_index];
        s.legal_index += 1;
        (RESULT & !0xFFFF_FFFF) | u64::from(value)
    })
}
extern "thiscall" fn compatible(this: u32, configuration: u32, candidate: u32, roads: u32) -> u64 {
    record("compatible", this, &[configuration, candidate, roads]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let value = s.compatible[s.compatible_index];
        s.compatible_index += 1;
        (RESULT & !0xFFFF_FFFF) | u64::from(value)
    })
}
extern "thiscall" fn append(this: u32, value: *const u32) -> u64 {
    record("append", this, &[unsafe { *value }]);
    RESULT
}
extern "thiscall" fn lot_state(this: u32) -> u64 {
    record("state", this, &[]);
    STATE.with(|s| (RESULT & !0xFFFF_FFFF) | u64::from(s.borrow().lot_state))
}
extern "thiscall" fn grow(this: u32, end: u32, value: *const u32, allocator: *const u8, count: u32, fill: u32) -> u64 {
    record(
        "grow",
        this,
        &[end, unsafe { *value }, u32::from(unsafe { *allocator }), count, fill],
    );
    RESULT
}

struct World {
    td: Object,
    lot: Box<[u32; 8]>,
    configuration: Box<[u32; 8]>,
    zone: Box<[u32; 1]>,
    tables: [Box<[usize; 56]>; 3],
    headers: Box<[[u32; 6]; 2]>,
    nodes: Box<[[u32; 6]; 2]>,
    debug: Object,
    rows: Box<[[u32; 28]; 3]>,
    cache: Box<[[u32; 6]; 2]>,
    vector: Box<[u32; 8]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180),
            lot: Box::new([0, 2, 2, 100, 200, 0.5f32.to_bits(), 0, 0]),
            configuration: Box::new([0, 4, 3, 0, 0, 0, 0, 0]),
            zone: Box::new([0]),
            tables: core::array::from_fn(|_| Box::new([0; 56])),
            headers: Box::new([[0; 6]; 2]),
            nodes: Box::new([[0; 6]; 2]),
            debug: Object::new(0xA0),
            rows: Box::new([[0; 28]; 3]),
            cache: Box::new([[0; 6]; 2]),
            vector: Box::new([0xCCCC_CCCC; 8]),
            _fakes: vec![],
        };
        for (offset, f) in [
            (0x58, size as *const () as usize),
            (0x54, location as *const () as usize),
            (0x48, slope as *const () as usize),
            (0x5C, facing as *const () as usize),
            (0x84, lot_state as *const () as usize),
        ] {
            w.tables[0][offset / 4] = f;
        }
        for (offset, f) in [
            (0x1C, dimensions as *const () as usize),
            (0x2C, stage as *const () as usize),
            (0x24, minimum as *const () as usize),
            (0x20, maximum as *const () as usize),
        ] {
            w.tables[1][offset / 4] = f;
        }
        w.tables[2][0x30 / 4] = zone as *const () as usize;
        w.lot[0] = w.tables[0].as_ptr() as u32;
        w.configuration[0] = w.tables[1].as_ptr() as u32;
        w.zone[0] = w.tables[2].as_ptr() as u32;
        for i in 0..2 {
            let h = w.headers[i].as_ptr() as u32;
            let n = w.nodes[i].as_ptr() as u32;
            w.headers[i] = [0, n, n, n, 0, 0];
            w.nodes[i] = [
                1,
                h,
                0,
                0,
                if i == 0 {
                    w.lot.as_ptr() as u32
                } else {
                    w.configuration.as_ptr() as u32
                },
                0,
            ];
        }
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_lots_header(w.headers[0].as_ptr() as u32);
        td.set_configurations_header(w.headers[1].as_ptr() as u32);
        td.set_zone_manager(w.zone.as_ptr() as u32);
        td.set_debug_context(w.debug.ptr() as u32);
        w.cache[0][1] = w.cache[1].as_ptr() as u32;
        w.cache[1][4] = w.lot.as_ptr() as u32;
        w.cache[1][5] = 27;
        td.set_suitability_header(w.cache[0].as_ptr() as u32);
        for (i, offset) in [0x24, 0x30, 0x3C].into_iter().enumerate() {
            unsafe {
                store(w.debug.ptr() as u32, offset, w.rows[i].as_ptr() as u32);
            }
        }
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            for _ in 0..16 {
                let mut candidate = Object::new(0xA4);
                unsafe {
                    core::ptr::write_bytes(candidate.ptr(), 0xCC, 0xA4);
                }
                let c = unsafe { Candidate::from_ptr(candidate.ptr()) };
                c.set_lot(core::ptr::null_mut());
                c.set_min_x(-1);
                c.set_min_z(-2);
                c.set_max_x(-3);
                c.set_max_z(-4);
                s.candidates.push(candidate);
            }
        });
        for (address, f) in [
            (0x0069_5340, stage_fits as *const () as usize),
            (0x0070_7D90, take as *const () as usize),
            (0x0070_7320, give_back as *const () as usize),
            (0x0070_5040, networks as *const () as usize),
            (0x0070_8700, legal as *const () as usize),
            (0x0070_6500, compatible as *const () as usize),
            (0x007E_BD30, append as *const () as usize),
            (0x0051_CA60, grow as *const () as usize),
        ] {
            w._fakes.push(fake(address, f));
        }
        w
    }
    fn call(&mut self) -> u32 {
        run(0x0070_97E0, Conv::Thiscall, self.td.ptr() as u32, &[]).0
    }
    fn candidate(&self, index: usize) -> u32 {
        STATE.with(|s| s.borrow_mut().candidates[index].ptr() as u32)
    }
    fn names(&self) -> Vec<&'static str> {
        STATE.with(|s| s.borrow().calls.iter().map(|c| c.0).collect())
    }
}

#[test]
fn existing_constructs_one_candidate_and_grows_or_writes_the_original_vector() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for grow in [false, true] {
        let mut w = World::new();
        w.configuration[1] = 2;
        w.configuration[2] = 2;
        let td = w.td.ptr() as u32;
        let begin = w.vector.as_ptr() as u32;
        if !grow {
            unsafe {
                store(td, 0xE4, begin);
                store(td, 0xE8, begin);
                store(td, 0xEC, begin + 32);
            }
        }
        let before = snapshot(w.td.ptr(), 0x180);
        assert_eq!(run(0x00708110, Conv::Thiscall, td, &[]).0, 27);
        assert_eq!(w.rows[0][27], 1);
        let mut expected = before;
        expected[0x108] = 1;
        if !grow {
            expected[0xE8..0xEC].copy_from_slice(&(begin + 4).to_le_bytes());
            assert_eq!(w.vector[0], w.candidate(0));
        }
        assert_eq!(snapshot(w.td.ptr(), 0x180), expected);
        let candidate = w.candidate(0);
        STATE.with(|s| {
            let s = s.borrow();
            let c = &s.snapshots[0];
            assert_eq!(u32::from_le_bytes(c[0..4].try_into().unwrap()), 2);
            assert_eq!(u32::from_le_bytes(c[4..8].try_into().unwrap()), w.lot.as_ptr() as u32);
            assert_eq!(&c[0x14..0x18], &[2, 2, 0xCC, 0xCC]);
            if grow {
                assert!(s.calls.contains(&Call("grow", td + 0xE4, vec![0, candidate, 2, 1, 1])));
            }
        });
        let names = w.names();
        assert_eq!(
            &names[..10],
            [
                "state",
                "location",
                "size",
                "slope",
                "dimensions",
                "stage",
                "zone",
                "fits",
                "minimum",
                "maximum"
            ]
        );
        assert_eq!(
            &names[10..],
            if grow {
                &["take", "location", "facing", "networks", "compatible", "grow"][..]
            } else {
                &["take", "location", "facing", "networks", "compatible"][..]
            }
        );
    }
}

#[test]
fn existing_out_flag_obeys_full_state_and_suitability_without_counting_cache_hits() {
    for (code, state, flag) in [(6, 0, 0), (19, 3, 0), (19, 0x10003, 1), (22, 0, 1), (u32::MAX, 0, 1)] {
        let mut w = World::new();
        w.cache[1][5] = code;
        STATE.with(|s| s.borrow_mut().lot_state = state);
        let td = w.td.ptr() as u32;
        assert_eq!(run(0x00708110, Conv::Thiscall, td, &[]).0, (code as i32).max(0) as u32);
        assert_eq!(unsafe { *(td.wrapping_add(0x108) as *const u8) }, flag);
        assert_eq!(w.names(), ["state", "location"]);
        assert_eq!(w.rows[0], [0; 28]);
    }
}

#[test]
fn existing_rejects_sizes_stages_and_all_slope_nans_and_keeps_compatibility_result() {
    for case in 0..8 {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.configuration[1] = 2;
        w.configuration[2] = 2;
        let expected = match case {
            0 => {
                w.configuration[1] = 3;
                5
            }
            1 => {
                STATE.with(|s| s.borrow_mut().fits = 0);
                6
            }
            2 => {
                w.lot[5] = f32::NAN.to_bits();
                10
            }
            3 => {
                w.configuration[5] = f32::NAN.to_bits();
                10
            }
            4 => {
                STATE.with(|s| s.borrow_mut().maximum = f64::NAN);
                10
            }
            5 => {
                w.lot[5] = (-0.5f32).to_bits();
                10
            }
            6 => {
                STATE.with(|s| s.borrow_mut().compatible[0] = 26);
                26
            }
            _ => {
                STATE.with(|s| s.borrow_mut().compatible[0] = 28);
                28
            }
        };
        assert_eq!(run(0x00708110, Conv::Thiscall, w.td.ptr() as u32, &[]).0, expected);
        if case < 6 {
            assert_eq!(w.rows[0][expected as usize], 1);
            assert!(!w.names().contains(&"take"));
        } else {
            assert_eq!(w.names().last(), Some(&"return"));
            assert_eq!(w.rows[0], [0; 28]);
        }
    }
}

#[test]
fn subdivision_keeps_piece_count_byte_and_ignores_failed_compatibility_result() {
    for compatible_result in [27, 26, 3, 0, u32::MAX] {
        let _fpu = GameFpu::new();
        let _machine = PreserveMachine::new();
        let mut w = World::new();
        w.lot[1] = 6;
        w.configuration[1] = 2;
        w.configuration[2] = 2;
        STATE.with(|s| s.borrow_mut().compatible[0] = compatible_result);
        let td = w.td.ptr() as u32;
        let before = snapshot(w.td.ptr(), 0x180);
        assert_eq!(
            run(0x00708C00, Conv::Thiscall, td, &[]).0,
            if compatible_result == 27 { 27 } else { 0 }
        );
        assert_eq!(snapshot(w.td.ptr(), 0x180), before);
        STATE.with(|s| {
            let s = s.borrow();
            let c = &s.snapshots[0];
            assert_eq!(u32::from_le_bytes(c[..4].try_into().unwrap()), 4);
            assert_eq!(u32::from_le_bytes(c[4..8].try_into().unwrap()), w.lot.as_ptr() as u32);
            assert_eq!(&c[0x14..0x18], &[2, 2, 0xCC, 0xCC]);
            assert_eq!(&c[0x6C..0x70], &[3, 0xCC, 0xCC, 0xCC]);
        });
        assert_eq!(
            w.names(),
            [
                "size",
                "location",
                "slope",
                "dimensions",
                "stage",
                "zone",
                "fits",
                "minimum",
                "maximum",
                "take",
                "facing",
                "location",
                "networks",
                "compatible",
                if compatible_result == 27 { "append" } else { "return" }
            ]
        );
        assert_eq!(w.rows[2][27], u32::from(compatible_result == 27));
    }
}

#[test]
fn subdivision_rejects_uneven_width_or_changed_depth_and_accepts_nan_slopes() {
    for (lot_width, lot_depth, width, depth, expected) in [
        (2, 2, 2, 2, 3),
        (2, 2, 3, 2, 3),
        (6, 2, 2, 3, 3),
        (6, 3, 2, 2, 3),
        (5, 2, 2, 2, 3),
        (6, 2, 2, 2, 27),
        (255, 2, 1, 2, 27),
    ] {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.lot[1] = lot_width;
        w.lot[2] = lot_depth;
        w.configuration[1] = width;
        w.configuration[2] = depth;
        w.lot[5] = f32::NAN.to_bits();
        assert_eq!(run(0x00708C00, Conv::Thiscall, w.td.ptr() as u32, &[]).0, expected);
        assert_eq!(w.rows[2][expected as usize], 1);
        if lot_width == 255 {
            STATE.with(|s| assert_eq!(s.borrow().snapshots[0][0x6C], 255));
        }
    }
}

#[test]
fn listings_empty_lot_sets_leave_the_expected_register_parts() {
    for address in [0x00708110, 0x00708C00] {
        let mut w = World::new();
        let h = w.headers[0].as_ptr() as u32;
        w.headers[0][2] = h;
        assert_eq!(run(address, Conv::Thiscall, w.td.ptr() as u32, &[]).0, 0);
        assert!(w.names().is_empty());
    }
}

#[test]
fn existing_stops_at_first_success_but_subdivision_scans_every_configuration() {
    for (address, answers, expected, attempts) in [
        (0x00708110, vec![27, 27], 27, 1),
        (0x00708110, vec![28, 27], 28, 2),
        (0x00708C00, vec![27, 27], 27, 2),
    ] {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.configuration[1] = 2;
        w.configuration[2] = 2;
        if address == 0x00708C00 {
            w.lot[1] = 6;
        }
        let second_configuration = Box::new(*w.configuration);
        let mut second_node = Box::new([
            1,
            w.nodes[1].as_ptr() as u32,
            0,
            0,
            second_configuration.as_ptr() as u32,
            0,
        ]);
        w.nodes[1][3] = second_node.as_mut_ptr() as u32;
        w.headers[1][3] = second_node.as_ptr() as u32;
        STATE.with(|s| s.borrow_mut().compatible = answers);
        assert_eq!(run(address, Conv::Thiscall, w.td.ptr() as u32, &[]).0, expected);
        assert_eq!(w.names().iter().filter(|&&n| n == "take").count(), attempts);
        assert_eq!(w.names().iter().filter(|&&n| n == "dimensions").count(), attempts);
        assert_eq!(
            w.rows[if address == 0x00708110 { 0 } else { 2 }][27],
            if address == 0x00708C00 { 2 } else { 1 }
        );
    }
}

#[test]
fn site_tree_iteration_descends_left_and_climbs_multiple_parents_without_writes() {
    let mut nodes = Box::new([[0u32; 6]; 8]);
    let pointer = |i: usize| nodes[i].as_ptr() as u32;
    let p: [u32; 8] = core::array::from_fn(pointer);
    // Header, then a seven-node balanced tree whose in-order visit is 4,2,5,1,6,3,7.
    nodes[0][1] = p[1];
    nodes[0][2] = p[4];
    nodes[0][3] = p[7];
    for (i, parent, left, right) in [
        (1, 0, 2, 3),
        (2, 1, 4, 5),
        (3, 1, 6, 7),
        (4, 2, 0, 0),
        (5, 2, 0, 0),
        (6, 3, 0, 0),
        (7, 3, 0, 0),
    ] {
        nodes[i][1] = p[parent];
        nodes[i][2] = if left == 0 { 0 } else { p[left] };
        nodes[i][3] = if right == 0 { 0 } else { p[right] };
    }
    let before = *nodes;
    let mut r = Entry {
        eax: 0,
        ecx: 0,
        edx: 0xDEAD_BEEF,
        return_address: 0,
        args: [],
    };
    let mut node = p[4];
    let mut order = vec![node];
    while node != p[0] {
        node = unsafe { successor(&mut r, node) };
        order.push(node);
    }
    assert_eq!(order, [p[4], p[2], p[5], p[1], p[6], p[3], p[7], p[0]]);
    assert_eq!(*nodes, before);
    assert_eq!(r.edx, 0xDEAD_BEEF);
}

#[test]
fn aggregation_builds_both_anchors_for_every_facing_and_writes_only_candidate_fields() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for facing in [0, 1, 2, 3, 4, u32::MAX] {
        let mut w = World::new();
        w.lot[6] = facing;
        let before = snapshot(w.td.ptr(), 0x180);
        assert_eq!(w.call(), 27);
        assert_eq!(snapshot(w.td.ptr(), 0x180), before);
        assert_eq!(w.rows[1][27], 2);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.snapshots.len(), 2);
            for (trial, actual) in s.snapshots.iter().enumerate() {
                let mut expected = vec![0xCC; 0xA4];
                let mut x = 100u32;
                let mut z = 200u32;
                if trial == 1 {
                    match facing {
                        0 => x += 2,
                        1 => z += 2,
                        2 => x -= 2,
                        3 => z -= 2,
                        _ => {}
                    }
                }
                for (offset, word) in [
                    (0, 3u32),
                    (4, 0),
                    (8, x),
                    (12, z),
                    (16, facing),
                    (0x38, u32::MAX),
                    (0x3C, if trial == 0 { u32::MAX - 1 } else { u32::MAX }),
                    (0x40, u32::MAX - 2),
                    (0x44, u32::MAX - 3),
                ] {
                    expected[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                }
                expected[0x14] = 4;
                expected[0x15] = 3;
                assert_eq!(*actual, expected);
            }
            let names: Vec<_> = s.calls.iter().map(|c| c.0).collect();
            assert_eq!(
                names,
                [
                    "size",
                    "location",
                    "slope",
                    "dimensions",
                    "stage",
                    "zone",
                    "fits",
                    "minimum",
                    "maximum",
                    "take",
                    "facing",
                    "location",
                    "networks",
                    "legal",
                    "compatible",
                    "append",
                    "take",
                    "facing",
                    "location",
                    "networks",
                    "legal",
                    "compatible",
                    "append"
                ]
            );
        });
        let td = w.td.ptr() as u32;
        let calls = STATE.with(|s| s.borrow().calls.clone());
        assert!(calls.contains(&Call("fits", 0, vec![4, 0x1234_0002])));
        for i in 0..2 {
            assert!(calls.contains(&Call(
                "compatible",
                td,
                vec![w.configuration.as_ptr() as u32, w.candidate(i), 0]
            )));
        }
    }
}

#[test]
fn aggregation_rejections_use_unsigned_dimensions_low_stage_byte_and_distinct_counters() {
    for (width, depth, fit, slope_value, counter) in [
        (1, 3, 1, 0.5, 5),
        (4, 1, 1, 0.5, 5),
        (2, 2, 1, 0.5, 5),
        (u32::MAX, 1, 1, 0.5, 5),
        (4, 3, 0, 0.5, 6),
        (4, 3, 1, -0.5, 10),
        (4, 3, 1, 1.5, 10),
    ] {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.configuration[1] = width;
        w.configuration[2] = depth;
        w.lot[5] = f32::to_bits(slope_value);
        STATE.with(|s| s.borrow_mut().fits = fit);
        assert_eq!(w.call(), 5);
        assert_eq!(w.rows[1][counter], 1);
        assert_eq!(w.rows[1].iter().sum::<u32>(), 1);
        assert!(!w.names().contains(&"take"));
    }
}

#[test]
fn aggregation_failed_trial_uses_configuration_width_even_after_success() {
    for width in [4, 5, 6, 0x100] {
        for (legal_answers, compatibility_answers, result, appended, returned) in [
            (vec![1, 0], vec![27], if width < 5 { 5 } else { 27 }, 1, 1),
            (vec![0, 1], vec![27], 27, 1, 1),
            (vec![0, 0], vec![], if width < 5 { 5 } else { 0 }, 0, 2),
            (vec![1, 1], vec![3, 10], if width < 5 { 5 } else { 0 }, 0, 2),
        ] {
            let _fpu = GameFpu::new();
            let mut w = World::new();
            w.configuration[1] = width;
            STATE.with(|s| {
                let s = &mut *s.borrow_mut();
                s.legal = legal_answers;
                s.compatible = compatibility_answers;
            });
            assert_eq!(w.call(), result, "width {width}");
            assert_eq!(w.names().iter().filter(|&&s| s == "append").count(), appended);
            assert_eq!(w.names().iter().filter(|&&s| s == "return").count(), returned);
        }
    }
}

#[test]
fn aggregation_slope_nans_pass_and_maximum_retains_st0_precision() {
    for (slope_value, minimum_value, maximum_value, success) in [
        (f32::NAN, 0.0, 1.0, true),
        (0.5, f32::NAN, 1.0, true),
        (0.5, 0.0, f64::NAN, true),
        (1.0, 0.0, f64::from_bits(1.0f64.to_bits() - 1), false),
        (0.0, 0.0, 0.0, true),
        (1.0, 0.0, 1.0, true),
    ] {
        let _fpu = GameFpu::new();
        let _machine = PreserveMachine::new();
        let mut w = World::new();
        w.lot[5] = f32::to_bits(slope_value);
        w.configuration[5] = f32::to_bits(minimum_value);
        STATE.with(|s| s.borrow_mut().maximum = maximum_value);
        assert_eq!(w.call(), if success { 27 } else { 5 });
    }
}

#[test]
fn aggregation_empty_trees_skip_all_or_only_configuration_calls() {
    for empty_lots in [true, false] {
        let mut w = World::new();
        let i = if empty_lots { 0 } else { 1 };
        let h = w.headers[i].as_ptr() as u32;
        w.headers[i][2] = h;
        assert_eq!(w.call(), 0);
        assert_eq!(
            w.names(),
            if empty_lots {
                vec![]
            } else {
                vec!["size", "location", "slope"]
            }
        );
    }
}
