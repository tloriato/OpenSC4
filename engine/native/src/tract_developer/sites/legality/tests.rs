// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Aggregation legality with actual tree links, partial return bytes and caller argument writes.

use super::*;
use crate::testing::{fake, Fake, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Call(&'static str, u32, Vec<u32>);
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    cells: Vec<(u32, u32, u32)>,
    lots: [u32; 2],
    cache: [u32; 2],
    header: u32,
    members_header: u32,
    member_nodes: [u32; 2],
    members: Vec<u32>,
    candidate: u32,
    miss: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
const HI: u64 = 0xDEAD_BEEF_0000_0000;
fn log(name: &'static str, this: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(name, this, args.to_vec())));
}
extern "thiscall" fn bounds(this: u32, candidate: u32) -> u64 {
    log("bounds", this, &[candidate]);
    HI | 0x1234_5678
}
extern "thiscall" fn width(this: u32) -> u64 {
    log("width", this, &[]);
    HI | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn height(this: u32) -> u64 {
    log("height", this, &[]);
    HI | u64::from(unsafe { word(this, 8) })
}
extern "thiscall" fn lot(this: u32, x: u32, z: u32, flag: u32) -> u64 {
    log("lot", this, &[x, z, flag]);
    STATE.with(|s| {
        HI | u64::from(
            s.borrow()
                .cells
                .iter()
                .find(|c| c.0 == x && c.1 == z)
                .map_or(0, |c| c.2),
        )
    })
}
extern "thiscall" fn find(this: u32, output: *mut u32, key: *const u32) -> u64 {
    let key = unsafe { *key };
    log("find", this, &[key]);
    STATE.with(|s| {
        let s = s.borrow();
        let index = s.lots.iter().position(|&l| l == key).unwrap();
        unsafe {
            *output = if s.miss { s.header } else { s.cache[index] };
        }
    });
    HI | 0x1234_5678
}
extern "thiscall" fn face(this: u32) -> u64 {
    log("face", this, &[]);
    HI | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn zone(this: u32) -> u64 {
    log("zone", this, &[]);
    HI | u64::from(unsafe { word(this, 8) })
}
extern "thiscall" fn rectangle(this: u32, output: *mut [u32; 4]) -> u64 {
    log("rectangle", this, &[]);
    unsafe {
        *output = [word(this, 12), word(this, 16), word(this, 20), word(this, 24)];
    }
    HI | 0x1234_5678
}
extern "thiscall" fn insert(this: u32, result: *mut u32, lot: *const u32) -> u64 {
    let lot = unsafe { *lot };
    log("insert", this, &[lot]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.candidate + 0x18);
        if !s.members.contains(&lot) {
            s.members.push(lot);
            s.members.sort();
        }
        let h = s.members_header;
        for (i, &lot) in s.members.iter().enumerate() {
            let n = s.member_nodes[i];
            unsafe {
                store(n, 4, if i == 0 { h } else { s.member_nodes[i - 1] });
                store(n, 8, 0);
                store(
                    n,
                    0xC,
                    if i + 1 == s.members.len() {
                        0
                    } else {
                        s.member_nodes[i + 1]
                    },
                );
                store(n, 0x10, lot);
            }
        }
        unsafe {
            store(h, 4, s.member_nodes[0]);
            store(h, 8, s.member_nodes[0]);
            store(h, 0xC, s.member_nodes[s.members.len() - 1]);
            store(this, 4, s.members.len() as u32);
            *result = s.member_nodes[0];
            *result.add(1) = 1;
        }
    });
    HI | 0x1234_5678
}
extern "thiscall" fn next(iterator: *mut u32) -> u64 {
    let current = unsafe { *iterator };
    log("next", 0, &[current]);
    let right = unsafe { word(current, 0xC) };
    unsafe {
        *iterator = if right == 0 {
            STATE.with(|s| s.borrow().members_header)
        } else {
            right
        };
    }
    HI | 0x1234_5678
}
extern "thiscall" fn location(this: u32, x: *mut u32, z: *mut u32) -> u64 {
    log("location", this, &[]);
    unsafe {
        *x = word(this, 12);
        *z = word(this, 16);
    }
    HI
}
extern "thiscall" fn construction(this: u32, output: *mut u8) -> u64 {
    log("construction", this, &[]);
    unsafe {
        core::ptr::write_bytes(output, 0, 20);
        write_unaligned(output.cast::<u32>(), 1);
    }
    HI
}
extern "thiscall" fn zone_at(this: u32, x: u32, z: u32) -> u64 {
    log("zone_at", this, &[x, z]);
    HI | 2
}
extern "C" fn fits(target: u32, zone: u32) -> u64 {
    log("fits", 0, &[target, zone]);
    HI
}
extern "thiscall" fn cache(this: u32, key: *const u32) -> u64 {
    let key = unsafe { *key };
    log("cache", this, &[key]);
    STATE.with(|s| {
        let s = s.borrow();
        let i = s.lots.iter().position(|&l| l == key).unwrap();
        HI | u64::from(s.cache[i] + 0x14)
    })
}

struct World {
    td: Object,
    candidate: Object,
    city: Box<[u32; 3]>,
    manager: Box<u32>,
    developer: Box<u32>,
    zone: Box<u32>,
    tables: [Box<[usize; 128]>; 5],
    lots: Box<[[u32; 7]; 2]>,
    cache_header: Box<[u32; 6]>,
    cache_nodes: Box<[[u32; 6]; 2]>,
    membership: Box<[[u32; 6]; 3]>,
    context: Object,
    rows: Box<[[u32; 28]; 2]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180),
            candidate: Object::new(0xA4),
            city: Box::new([0, 64, 64]),
            manager: Box::new(0),
            developer: Box::new(0),
            zone: Box::new(0),
            tables: core::array::from_fn(|_| Box::new([0; 128])),
            lots: Box::new([[0, 0, 2, 1, 2, 1, 2], [0, 0, 2, 2, 2, 2, 2]]),
            cache_header: Box::new([0; 6]),
            cache_nodes: Box::new([[0; 6]; 2]),
            membership: Box::new([[0; 6]; 3]),
            context: Object::new(0xA0),
            rows: Box::new([[0; 28]; 2]),
            _fakes: vec![],
        };
        for (table, offset, f) in [
            (0, 0x174, width as *const () as usize),
            (0, 0x178, height as *const () as usize),
            (1, 0x18, lot as *const () as usize),
            (2, 0x5C, face as *const () as usize),
            (2, 0x7C, zone as *const () as usize),
            (2, 0x38, rectangle as *const () as usize),
            (2, 0x54, location as *const () as usize),
            (3, 0x24, construction as *const () as usize),
            (4, 0x30, zone_at as *const () as usize),
        ] {
            w.tables[table][offset / 4] = f;
        }
        w.city[0] = w.tables[0].as_ptr() as u32;
        *w.manager = w.tables[1].as_ptr() as u32;
        *w.developer = w.tables[3].as_ptr() as u32;
        *w.zone = w.tables[4].as_ptr() as u32;
        for i in 0..2 {
            w.lots[i][0] = w.tables[2].as_ptr() as u32;
            w.cache_nodes[i][5] = 27;
        }
        let h = w.membership[0].as_ptr() as u32;
        w.membership[0][2] = h;
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        c.set_min_x(1);
        c.set_min_z(2);
        c.set_max_x(2);
        c.set_max_z(2);
        c.set_facing(0);
        c.set_lot_header(h);
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_city(w.city.as_ptr() as u32);
        td.set_lot_manager(w.manager.as_ref() as *const u32 as u32);
        td.set_developer(w.developer.as_ref() as *const u32 as u32);
        td.set_zone_manager(w.zone.as_ref() as *const u32 as u32);
        td.set_debug_context(w.context.ptr() as u32);
        td.set_suitability_header(w.cache_header.as_ptr() as u32);
        unsafe {
            store(w.context.ptr() as u32, 0x60, w.rows[0].as_ptr() as u32);
            store(w.context.ptr() as u32, 0x6C, w.rows[1].as_ptr() as u32);
        }
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.lots = core::array::from_fn(|i| w.lots[i].as_ptr() as u32);
            s.cache = core::array::from_fn(|i| w.cache_nodes[i].as_ptr() as u32);
            s.header = w.cache_header.as_ptr() as u32;
            s.members_header = h;
            s.member_nodes = [w.membership[1].as_ptr() as u32, w.membership[2].as_ptr() as u32];
            s.candidate = w.candidate.ptr() as u32;
            s.cells = vec![(1, 2, s.lots[0]), (2, 2, s.lots[1])];
        });
        for (address, f) in [
            (0x0070_4630, bounds as *const () as usize),
            (0x0049_7520, find as *const () as usize),
            (0x0049_A050, insert as *const () as usize),
            (0x005D_17C0, next as *const () as usize),
            (0x0069_5390, fits as *const () as usize),
            (0x0049_7A00, cache as *const () as usize),
        ] {
            w._fakes.push(fake(address, f));
        }
        w
    }
    fn call(&mut self) -> u32 {
        run(
            0x00708700,
            Conv::Thiscall,
            self.td.ptr() as u32,
            &[self.candidate.ptr() as u32],
        )
        .0
    }
    fn names(&self) -> Vec<&'static str> {
        STATE.with(|s| s.borrow().calls.iter().map(|c| c.0).collect())
    }
    fn result(&self, success: bool) -> u32 {
        (self.rows[1].as_ptr() as u32 & !0xFF) | u32::from(success)
    }
}

#[test]
fn aggregation_legality_scans_x_before_z_keeps_distinct_lots_and_checks_complete_rectangles() {
    let _machine = PreserveMachine::new();
    let mut w = World::new();
    let before = snapshot(w.td.ptr(), 0x180);
    assert_eq!(w.call(), w.result(true));
    assert_eq!(w.rows[1][27], 1);
    assert_eq!(snapshot(w.td.ptr(), 0x180), before);
    assert_eq!(unsafe { word(w.candidate.ptr() as u32, 0x1C) }, 2);
    assert_eq!(
        w.names(),
        [
            "bounds",
            "width",
            "height",
            "lot",
            "find",
            "face",
            "zone",
            "zone",
            "insert",
            "lot",
            "find",
            "face",
            "zone",
            "insert",
            "rectangle",
            "next",
            "rectangle",
            "next"
        ]
    );
    STATE.with(|s| {
        let s = s.borrow();
        assert!(s
            .calls
            .contains(&Call("lot", w.manager.as_ref() as *const u32 as u32, vec![1, 2, 0])));
        assert!(s
            .calls
            .contains(&Call("lot", w.manager.as_ref() as *const u32 as u32, vec![2, 2, 0])));
    });
}

#[test]
fn aggregation_legality_bounds_and_missing_cells_fail_without_histogram_updates() {
    for case in 0..6 {
        let mut w = World::new();
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        match case {
            0 => c.set_min_x(-1),
            1 => c.set_min_z(-1),
            2 => c.set_max_x(64),
            3 => c.set_max_z(64),
            4 => STATE.with(|s| s.borrow_mut().cells.clear()),
            _ => w.cache_nodes[0][5] = 6,
        }
        assert_eq!(w.call() as u8, 0);
        assert_eq!(*w.rows, [[0; 28]; 2]);
        let names = w.names();
        assert_eq!(names.len(), [1, 1, 2, 3, 4, 5][case]);
    }
}

#[test]
fn aggregation_legality_allows_road_failure_but_requires_a_matching_facing_and_two_whole_lots() {
    for (case, code, success) in [
        (0, 27, true),
        (1, 21, false),
        (2, 3, false),
        (3, 6, false),
        (4, 3, false),
        (5, 3, false),
        (6, 3, false),
        (7, 3, false),
    ] {
        let mut w = World::new();
        match case {
            0 => w.cache_nodes[0][5] = 21,
            1 => {
                w.lots[0][1] = 1;
                w.lots[1][1] = 1;
            }
            2 => STATE.with(|s| {
                let s = &mut *s.borrow_mut();
                s.cells[1].2 = s.lots[0];
            }),
            3 => w.lots[1][2] = 3,
            4 => w.lots[0][4] = 1,
            5 => w.lots[0][6] = 3,
            6 => w.lots[0][3] = 0,
            _ => w.lots[0][5] = 3,
        }
        assert_eq!(w.call(), w.result(success), "case {case}");
        assert_eq!(w.rows[1][code], 1);
        assert_eq!(w.rows[1].iter().sum::<u32>(), 1);
    }
}

#[test]
fn aggregation_legality_zero_zone_repeats_the_first_query_and_empty_bounds_need_a_facing() {
    let mut w = World::new();
    w.lots[0][2] = 0;
    w.lots[1][2] = 0;
    assert_eq!(w.call() as u8, 1);
    assert_eq!(w.names().iter().filter(|&&n| n == "zone").count(), 4);
    let mut w = World::new();
    let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
    c.set_min_x(3);
    assert_eq!(w.call(), w.result(false));
    assert_eq!(w.names(), ["bounds", "width", "height"]);
}

#[test]
fn aggregation_legality_miss_runs_expanded_checks_and_records_only_suitability_failure() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().miss = true);
    assert_eq!(w.call() as u8, 0);
    assert_eq!(w.rows[0][6], 1);
    assert_eq!(w.rows[1], [0; 28]);
    assert_eq!(w.cache_nodes[0][5], 6);
    assert_eq!(
        w.names(),
        [
            "bounds",
            "width",
            "height",
            "lot",
            "find",
            "location",
            "construction",
            "zone_at",
            "fits",
            "cache"
        ]
    );
}

#[test]
fn containment_iterator_replaces_the_original_candidate_argument_until_the_header() {
    let mut w = World::new();
    let candidate = w.candidate.ptr() as u32;
    let mut entry = Entry {
        eax: 0x1122_3344,
        ecx: w.td.ptr() as u32,
        edx: 0x5566_7788,
        return_address: 0,
        args: [candidate],
    };
    unsafe {
        aggregation_legal(&mut entry);
    }
    assert_eq!(entry.args, [w.membership[0].as_ptr() as u32]);
    assert_eq!(entry.eax, w.result(true));
}
