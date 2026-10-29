// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Farm growth cases on native objects, with ordered service calls and retained heap contents.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;
use std::collections::BTreeMap;

type Event = (&'static str, Vec<u32>);
const HI: u64 = 0xCAFE_BABE_0000_0000;
const FREE_RESULT: u64 = 0x7654_3210_1234_5678;

// The port retains node addresses while these vectors grow.
#[allow(clippy::vec_box)]
struct State {
    calls: Vec<Event>,
    allocations: Vec<Box<[u32; 3]>>,
    freed: Vec<u32>,
    rect: BTreeMap<u32, Vec<u32>>,
    surrounding: BTreeMap<(u32, u32), Vec<u32>>,
    cache: BTreeMap<u32, u32>,
    cache_nodes: Vec<Box<[u32; 6]>>,
    header: u32,
    this: u32,
    candidate: u32,
    services: [u32; 9],
    profile: u32,
    field_ids: u32,
    rng: u32,
    claim: u32,
    build: u32,
    fetch: u32,
    property: bool,
    configuration: u32,
    ids_count: u32,
    draw: u32,
    null_unlink: bool,
    mutate_facing: bool,
    evict_at_start: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            allocations: vec![],
            freed: vec![],
            rect: BTreeMap::new(),
            surrounding: BTreeMap::new(),
            cache: BTreeMap::new(),
            cache_nodes: vec![],
            header: 0,
            this: 0,
            candidate: 0,
            services: [0; 9],
            profile: 0,
            field_ids: 0,
            rng: 0,
            claim: 0x1234_0001,
            build: 27,
            fetch: 1,
            property: true,
            configuration: 0x1234_5600,
            ids_count: 3,
            draw: 1,
            null_unlink: false,
            mutate_facing: false,
            evict_at_start: false,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(name: &'static str, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push((name, args.to_vec())));
}
fn service(this: u32, index: usize) {
    STATE.with(|s| assert_eq!(this, s.borrow().services[index]));
}
fn allocate_node(s: &mut State) -> u32 {
    let node = Box::new([0xCCCC_CCCC; 3]);
    let p = node.as_ptr() as u32;
    s.allocations.push(node);
    p
}
extern "C" fn allocate(bytes: u32) -> u64 {
    assert_eq!(bytes, 12);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let p = allocate_node(s);
        s.calls.push(("allocate", vec![p]));
        HI | u64::from(p)
    })
}
extern "C" fn free(p: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert!(!s.freed.contains(&p), "double free");
        s.freed.push(p);
        s.calls.push(("free", vec![p]));
    });
    FREE_RESULT
}
fn append(list: u32, lots: &[u32]) {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let head = unsafe { word(list, 0) };
        for &lot in lots {
            let node = allocate_node(s);
            unsafe {
                let tail = word(head, 4);
                store(node, 0, head);
                store(node, 4, tail);
                store(node, 8, lot);
                store(tail, 0, node);
                store(head, 4, node);
            }
        }
    });
}
extern "thiscall" fn surrounding_rect(this: u32, rect: u32, list: u32, flag: u32) -> u64 {
    service(this, 0);
    STATE.with(|s| assert_eq!(rect, s.borrow().candidate + 0x38));
    record("rect", &[flag]);
    let lots = STATE.with(|s| s.borrow().rect.get(&flag).cloned().unwrap_or_default());
    append(list, &lots);
    HI | 0x1122_3300
}
extern "thiscall" fn surrounding_lot(this: u32, lot: u32, list: u32, flag: u32) -> u64 {
    service(this, 0);
    record("near", &[lot, flag]);
    let lots = STATE.with(|s| s.borrow().surrounding.get(&(lot, flag)).cloned().unwrap_or_default());
    append(list, &lots);
    HI | 0x2233_4400
}
extern "thiscall" fn rectangle(this: u32, output: *mut [u32; 4]) -> u64 {
    record("bounds", &[this]);
    unsafe {
        *output = [word(this, 4), word(this, 8), word(this, 12), word(this, 16)];
    }
    HI
}
extern "thiscall" fn release(this: u32) -> u64 {
    record("release", &[this]);
    HI
}
extern "thiscall" fn find(this: u32, output: *mut u32, key: *const u32) -> u64 {
    let lot = unsafe { *key };
    record("find", &[lot]);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this, s.this + 0x10C);
        unsafe {
            *output = s.cache.get(&lot).copied().unwrap_or(s.header);
        }
    });
    HI | output as u64
}
extern "C" fn unlink(node: u32, root: u32, left: u32, right: u32) -> u64 {
    record("unlink", &[node]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!([root, left, right], [s.header + 4, s.header + 8, s.header + 12]);
        s.cache.retain(|_, n| *n != node);
        HI | u64::from(if s.null_unlink { 0 } else { node })
    })
}
extern "thiscall" fn claim(this: u32, n: u32) -> u64 {
    service(this, 1);
    record("claim", &[n]);
    STATE.with(|s| HI | u64::from(s.borrow().claim))
}
extern "thiscall" fn release_resources(this: u32, n: u32) -> u64 {
    service(this, 1);
    record("resources", &[n]);
    HI
}
extern "thiscall" fn build(this: u32, candidate: u32) -> u64 {
    record("build", &[this, candidate]);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!((this, candidate), (s.this, s.candidate));
        // The enclosing farm routine must retain changes made by Build.
        unsafe {
            store(this, 0xC0, 20);
        }
        HI | u64::from(s.build)
    })
}
extern "thiscall" fn resource_key(this: u32, building: u32, output: *mut [u32; 3]) -> u64 {
    service(this, 1);
    record("key", &[building]);
    unsafe {
        assert_eq!(*output, [0; 3]);
        *output = [0x6534_28A4, 0x0123_4567, building];
    }
    HI
}
extern "thiscall" fn fetch(this: u32, key: *const [u32; 3], iid: u32, output: *mut u32, a: u32, b: u32) -> u64 {
    service(this, 4);
    assert_eq!([iid, a, b], [0xA521_60F5, 0, 0]);
    unsafe {
        assert_eq!(*output, 0);
        assert_eq!(*key, [0x6534_28A4, 0x0123_4567, 0x400]);
    }
    record("fetch", &[]);
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            *output = if s.fetch as u8 != 0 { s.services[5] } else { 0 };
        }
        HI | u64::from(s.fetch)
    })
}
extern "thiscall" fn properties(this: u32) -> u64 {
    service(this, 5);
    record("properties", &[]);
    STATE.with(|s| HI | u64::from(s.borrow().services[6]))
}
extern "thiscall" fn property(this: u32, id: u32) -> u64 {
    service(this, 6);
    assert_eq!(id, 0x29B5_5F73);
    record("property", &[id]);
    STATE.with(|s| {
        let s = s.borrow();
        HI | u64::from(if s.property { s.services[7] } else { 0 })
    })
}
extern "thiscall" fn variant(this: u32) -> u64 {
    service(this, 7);
    record("variant", &[]);
    STATE.with(|s| HI | u64::from(s.borrow().services[8]))
}
extern "thiscall" fn data(this: u32) -> u64 {
    service(this, 8);
    record("data", &[]);
    STATE.with(|s| HI | u64::from(s.borrow().field_ids))
}
extern "thiscall" fn count(this: u32) -> u64 {
    service(this, 8);
    record("count", &[]);
    STATE.with(|s| HI | u64::from(s.borrow().ids_count))
}
extern "thiscall" fn uniform(this: u32, n: u32) -> u64 {
    record("draw", &[n]);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this, s.rng);
        HI | u64::from(s.draw)
    })
}
extern "thiscall" fn profile(this: u32, id: u32) -> u64 {
    service(this, 1);
    record("profile", &[id]);
    STATE.with(|s| HI | u64::from(s.borrow().profile))
}
extern "thiscall" fn configuration(this: u32, id: u32) -> u64 {
    service(this, 3);
    record("configuration", &[id]);
    STATE.with(|s| HI | u64::from(s.borrow().configuration))
}
extern "thiscall" fn facing(this: u32, facing: u32) -> u64 {
    record("facing", &[this, facing]);
    HI
}
extern "thiscall" fn start(this: u32, lot: u32, cfg: u32, building: u32, flag: u32) -> u64 {
    service(this, 2);
    record("start", &[lot, cfg, building, flag]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        if s.mutate_facing {
            unsafe {
                store(s.candidate, 0x10, word(s.candidate, 0x10).wrapping_add(1));
            }
        }
        if s.evict_at_start {
            s.cache.remove(&lot);
            unsafe {
                store(s.this, 0x110, word(s.this, 0x110).wrapping_sub(1));
            }
        }
    });
    // Field state/population are still set when construction returns false in AL.
    HI | 0x1234_0000
}
extern "thiscall" fn set_state(this: u32, state: u32, a: u32, b: u32) -> u64 {
    record("state", &[this, state, a, b]);
    HI
}
extern "thiscall" fn population(this: u32, purpose: u32, capacity: u32) -> u64 {
    record("population", &[this, purpose, capacity]);
    HI
}

// Lists and cache keys retain lot addresses while the fixture adds more lots.
#[allow(clippy::vec_box)]
struct World {
    td: Object,
    candidate: Object,
    lots: Vec<Box<[u32; 5]>>,
    tables: [Box<[u32; 104]>; 10],
    objects: Box<[u32; 9]>,
    header: Box<[u32; 4]>,
    _profile: Box<[u32; 3]>,
    _ids: Box<[u32; 3]>,
    _global: Box<u32>,
    _rng: Box<u32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180),
            candidate: Object::new(0xA4),
            lots: vec![],
            tables: core::array::from_fn(|_| Box::new([0; 104])),
            objects: Box::new([0; 9]),
            header: Box::new([0; 4]),
            _profile: Box::new([0, 0, 0xABCD_0003]),
            _ids: Box::new([0x402, 0x401, 0x403]),
            _global: Box::new(0),
            _rng: Box::new(0),
            _fakes: vec![],
        };
        for (obj, slot, f) in [
            (0, 0x28, surrounding_rect as *const () as u32),
            (0, 0x24, surrounding_lot as *const () as u32),
            (1, 0xC8, claim as *const () as u32),
            (1, 0xCC, release_resources as *const () as u32),
            (1, 0x38, resource_key as *const () as u32),
            (1, 0x9C, profile as *const () as u32),
            (2, 0x18, start as *const () as u32),
            (3, 0x20, configuration as *const () as u32),
            (4, 0xC, fetch as *const () as u32),
            (5, 0x1C, properties as *const () as u32),
            (5, 8, release as *const () as u32),
            (6, 0x24, property as *const () as u32),
            (7, 0x14, variant as *const () as u32),
            (8, 0x19C, data as *const () as u32),
            (8, 0x10, count as *const () as u32),
            (9, 0x38, rectangle as *const () as u32),
            (9, 8, release as *const () as u32),
            (9, 0x60, facing as *const () as u32),
            (9, 0x88, set_state as *const () as u32),
            (9, 0xDC, population as *const () as u32),
        ] {
            w.tables[obj][slot / 4] = f;
        }
        for i in 0..9 {
            w.objects[i] = w.tables[i].as_ptr() as u32;
        }
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_lot_manager(w.object(0));
        td.set_building_development(w.object(1));
        td.set_lot_developer(w.object(2));
        td.set_lot_configuration_manager(w.object(3));
        td.set_suitability_header(w.header.as_ptr() as u32);
        td.set_chosen_building(0x400);
        td.set_suitability_count(0);
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        c.set_min_x(0);
        c.set_min_z(0);
        c.set_max_x(1);
        c.set_max_z(1);
        c.set_facing(0);
        *w._global = w.object(4);
        w._fakes = [
            (0x0090_CF54, allocate as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
            (0x0049_7520, find as *const () as usize),
            (0x0040_DAF0, unlink as *const () as usize),
            (0x0070_B440, build as *const () as usize),
            (0x0091_36B1, uniform as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        w._fakes
            .push(fake_data(0x00B4_3CA8, (&mut *w._global as *mut u32).cast()));
        w._fakes.push(fake_data(0x00B4_AFD8, (&mut *w._rng as *mut u32).cast()));
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.this = td.as_ptr() as u32;
            s.candidate = c.as_ptr() as u32;
            s.header = w.header.as_ptr() as u32;
            s.services = core::array::from_fn(|i| w.object(i));
            s.profile = w._profile.as_ptr() as u32;
            s.field_ids = w._ids.as_ptr() as u32;
            s.rng = &*w._rng as *const u32 as u32;
        });
        w
    }
    fn object(&self, i: usize) -> u32 {
        self.objects.as_ptr().wrapping_add(i) as u32
    }
    fn lot(&mut self, rect: [i32; 4], suitability: u32) -> u32 {
        let lot = Box::new([
            self.tables[9].as_ptr() as u32,
            rect[0] as u32,
            rect[1] as u32,
            rect[2] as u32,
            rect[3] as u32,
        ]);
        let p = lot.as_ptr() as u32;
        self.lots.push(lot);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            let node = Box::new([0, 0, 0, 0, p, suitability]);
            s.cache.insert(p, node.as_ptr() as u32);
            s.cache_nodes.push(node);
        });
        let td = unsafe { TractDeveloper::from_ptr(self.td.ptr()) };
        td.set_suitability_count(td.suitability_count() + 1);
        p
    }
    fn chain(&mut self, n: usize) -> Vec<u32> {
        let lots: Vec<_> = (0..n)
            .map(|i| self.lot([10 + i as i32, 10, 10 + i as i32, 10], if i == 10 { 27 } else { 21 }))
            .collect();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            if n > 0 {
                s.rect.insert(1, vec![lots[0]]);
            }
            for pair in lots.windows(2) {
                s.surrounding.insert((pair[0], 1), vec![pair[1]]);
            }
            if n > 1 {
                s.surrounding.insert((lots[0], 2), vec![lots[0]]);
            }
        });
        lots
    }
    fn run(&mut self, expected: u32) -> Vec<Event> {
        let _fpu = GameFpu::new();
        let _machine = PreserveMachine::new();
        let before = snapshot(self.td.ptr(), 0x180);
        let result = run(
            0x0070_B640,
            Conv::Thiscall,
            self.td.ptr() as u32,
            &[self.candidate.ptr() as u32],
        );
        assert_eq!(result.0, expected);
        assert_eq!(result.2, (FREE_RESULT >> 32) as u32);
        STATE.with(|s| {
            let s = s.borrow();
            for node in &s.allocations {
                assert!(s.freed.contains(&(node.as_ptr() as u32)), "leaked list node");
            }
            let headers: Vec<_> = s
                .calls
                .iter()
                .filter(|c| c.0 == "allocate")
                .take(2)
                .map(|c| c.1[0])
                .collect();
            for head in headers {
                assert_eq!(
                    unsafe { [word(head, 0), word(head, 4), word(head, 8)] },
                    [head, head, 0xCCCC_CCCC]
                );
            }
        });
        let mut after = snapshot(self.td.ptr(), 0x180);
        for field in [0xC0, 0x110] {
            after[field..field + 4].copy_from_slice(&before[field..field + 4]);
        }
        assert_eq!(after, before);
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}
fn only(calls: &[Event], names: &[&str]) -> Vec<Event> {
    calls.iter().filter(|c| names.contains(&c.0)).cloned().collect()
}

#[test]
fn core_farm_chain_requires_fifteen_distinct_fields_and_skips_overlapping_and_rejected_lots() {
    for n in [0, 14, 15] {
        let mut w = World::new();
        let lots = w.chain(n);
        let overlap = w.lot([1, 1, 2, 2], 27);
        let rejected = w.lot([5, 10, 5, 10], 4);
        STATE.with(|s| {
            s.borrow_mut().rect.insert(4, vec![overlap, rejected]);
        });
        let calls = w.run(if n < 15 { 0 } else { 27 });
        assert_eq!(only(&calls, &["rect"]), [1, 2, 4, 8].map(|f| ("rect", vec![f])));
        assert!(!calls.contains(&("find", vec![overlap])));
        assert_eq!(only(&calls, &["near"]).len(), 4 * n);
        if n < 15 {
            assert!(only(&calls, &["claim", "build", "draw", "start"]).is_empty());
        } else {
            assert_eq!(
                only(&calls, &["claim", "resources", "draw"]),
                [("claim", vec![45]), ("resources", vec![30]), ("draw", vec![3])]
            );
            let expected: Vec<_> = lots
                .iter()
                .flat_map(|&lot| {
                    [
                        ("facing", vec![lot, 0]),
                        ("start", vec![lot, 0x1234_5600, 0x401, 0]),
                        ("state", vec![lot, 3, 0, 0]),
                        ("population", vec![lot, 0x4100, 3]),
                    ]
                })
                .collect();
            assert_eq!(only(&calls, &["facing", "start", "state", "population"]), expected);
            assert_eq!(unsafe { word(w.td.ptr() as u32, 0xC0) }, 20);
            assert_eq!(unsafe { word(w.td.ptr() as u32, 0x110) }, 2);
        }
        let release_lots: Vec<_> = only(&calls, &["release"]).into_iter().map(|c| c.1[0]).collect();
        for (i, lot) in lots.into_iter().enumerate() {
            assert_eq!(
                release_lots.iter().filter(|&&l| l == lot).count(),
                if n > 1 && i == 0 { 2 } else { 1 }
            );
        }
    }
}

#[test]
fn resource_and_build_failures_keep_full_return_words_and_release_only_the_field_claim() {
    for (claim, build_result, expected, releases) in [
        (0x1234_0000, 27, 0, vec![]),
        (1, 0, 0, vec![30, 15]),
        (1, 20, 20, vec![30, 15]),
        (1, 0xABCD_001B, 0xABCD_001B, vec![30, 15]),
    ] {
        let mut w = World::new();
        w.chain(15);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.claim = claim;
            s.build = build_result;
        });
        let calls = w.run(expected);
        assert_eq!(
            only(&calls, &["resources"]),
            releases.into_iter().map(|n| ("resources", vec![n])).collect::<Vec<_>>()
        );
        assert!(only(&calls, &["fetch", "draw", "start"]).is_empty());
    }
}

#[test]
fn missing_field_configuration_releases_fields_and_releases_the_resource_after_lookup() {
    let mut w = World::new();
    w.chain(15);
    let resource = w.object(5);
    STATE.with(|s| s.borrow_mut().configuration = 0);
    let calls = w.run(27);
    assert_eq!(
        only(&calls, &["claim", "resources", "profile", "configuration", "release"])
            .into_iter()
            .take(6)
            .collect::<Vec<_>>(),
        [
            ("claim", vec![45]),
            ("resources", vec![30]),
            ("profile", vec![0x401]),
            ("configuration", vec![0x401]),
            ("resources", vec![15]),
            ("release", vec![resource]),
        ]
    );
    assert!(only(&calls, &["start"]).is_empty());
}

#[test]
fn absent_property_uses_id_zero_and_an_empty_array_still_draws_and_reads_element_zero() {
    for case in 0..3 {
        let mut w = World::new();
        w.chain(15);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            if case == 0 {
                s.fetch = 0x1234_0000;
            }
            if case == 1 {
                s.property = false;
            }
            if case == 2 {
                s.ids_count = 0;
                s.draw = 0;
            }
        });
        let calls = w.run(27);
        let id = if case == 2 { 0x402 } else { 0 };
        assert_eq!(
            only(&calls, &["profile", "configuration"]),
            [("profile", vec![id]), ("configuration", vec![id])]
        );
        assert_eq!(
            only(&calls, &["draw"]),
            if case == 2 { vec![("draw", vec![0])] } else { vec![] }
        );
    }
}

#[test]
fn fields_reread_candidate_facing_and_cache_after_construction_and_accept_null_unlink_output() {
    let mut w = World::new();
    let lots = w.chain(15);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.null_unlink = true;
        s.mutate_facing = true;
    });
    let calls = w.run(27);
    assert_eq!(
        only(&calls, &["facing"]),
        lots.iter()
            .enumerate()
            .map(|(i, &l)| ("facing", vec![l, i as u32]))
            .collect::<Vec<_>>()
    );
    assert_eq!(only(&calls, &["unlink"]).len(), 15);
    let first = calls.iter().position(|c| c.0 == "start").unwrap();
    assert_eq!(calls[first + 1], ("find", vec![lots[0]]));
    assert_eq!(calls[first + 2].0, "unlink");
    assert_eq!(calls[first + 3], ("state", vec![lots[0], 3, 0, 0]));
    assert_eq!(unsafe { word(w.td.ptr() as u32, 0x110) }, 0);
}

#[test]
fn construction_queries_resources_and_field_data_in_order_and_respects_cache_removal_by_the_callee() {
    let mut w = World::new();
    w.chain(15);
    STATE.with(|s| s.borrow_mut().evict_at_start = true);
    let td = w.td.ptr() as u32;
    let candidate = w.candidate.ptr() as u32;
    let calls = w.run(27);
    assert_eq!(
        only(
            &calls,
            &[
                "claim",
                "resources",
                "build",
                "key",
                "fetch",
                "properties",
                "property",
                "variant",
                "data",
                "count",
                "draw",
                "profile",
                "configuration"
            ]
        ),
        [
            ("claim", vec![45]),
            ("resources", vec![30]),
            ("build", vec![td, candidate]),
            ("key", vec![0x400]),
            ("fetch", vec![]),
            ("properties", vec![]),
            ("property", vec![0x29B5_5F73]),
            ("variant", vec![]),
            ("data", vec![]),
            ("count", vec![]),
            ("draw", vec![3]),
            ("profile", vec![0x401]),
            ("configuration", vec![0x401]),
        ]
    );
    assert!(only(&calls, &["unlink"]).is_empty());
    assert_eq!(unsafe { word(td, 0x110) }, 0);
}

#[test]
fn overlap_uses_inclusive_signed_edges_and_rejections_do_not_expand_neighbours() {
    let mut w = World::new();
    let cases = [
        ([-1, 0, 0, 0], true),
        ([0, -1, 0, 0], true),
        ([1, 1, 2, 2], true),
        ([-2, 0, -1, 0], false),
        ([2, 0, 2, 0], false),
        ([0, -2, 0, -1], false),
        ([0, 2, 0, 2], false),
        ([i32::MIN, 0, i32::MIN, 0], false),
    ];
    let lots: Vec<_> = cases.iter().map(|&(rect, _)| w.lot(rect, 22)).collect();
    STATE.with(|s| {
        s.borrow_mut().rect.insert(1, lots.clone());
    });
    let calls = w.run(0);
    let expected: Vec<_> = lots
        .into_iter()
        .zip(cases)
        .filter(|(_, (_, overlaps))| !overlaps)
        .map(|(l, _)| ("find", vec![l]))
        .collect();
    assert_eq!(only(&calls, &["find"]), expected);
    assert!(only(&calls, &["near", "claim"]).is_empty());
}

extern "thiscall" fn location(this: u32, x: *mut u32, z: *mut u32) -> u64 {
    record("location", &[this]);
    unsafe {
        *x = 10;
        *z = 20;
    }
    HI
}
extern "thiscall" fn construction_info(_this: u32, output: *mut [u32; 5]) -> u64 {
    record("info", &[]);
    unsafe {
        *output = [3, 0, 0, 0, 0];
    }
    HI
}
extern "thiscall" fn zone(_this: u32, x: u32, z: u32) -> u64 {
    assert_eq!([x, z], [10, 20]);
    record("zone", &[]);
    HI | 0x1234_00FE
}
extern "C" fn fits(target: u32, zone: u32) -> u64 {
    assert_eq!([target, zone], [3, 0xFFFF_FFFE]);
    record("fits", &[]);
    HI | 0x1234_0000
}
extern "thiscall" fn cache(this: u32, key: *const u32) -> u64 {
    let lot = unsafe { *key };
    record("cache", &[lot]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.this + 0x10C);
        let node = Box::new([0, 0, 0, 0, lot, 0xCCCC_CCCC]);
        let p = node.as_ptr() as u32;
        s.cache.insert(lot, p);
        s.cache_nodes.push(node);
        unsafe {
            store(this, 4, word(this, 4).wrapping_add(1));
        }
        HI | u64::from(p + 0x14)
    })
}

#[test]
fn cache_misses_run_the_farm_checks_once_and_update_the_histogram_before_reusing_the_result() {
    let mut w = World::new();
    let lot = w.lot([10, 10, 10, 10], 27);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.cache.clear();
        s.rect.insert(1, vec![lot, lot]);
    });
    let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
    td.set_suitability_count(0);
    td.set_developer(w.object(6));
    td.set_zone_manager(w.object(8));
    let mut context = Object::new(0x80);
    let mut histogram = Box::new([u32::MAX; 28]);
    td.set_debug_context(context.ptr() as u32);
    unsafe {
        store(context.ptr() as u32, 0x60, histogram.as_mut_ptr() as u32);
    }
    w.tables[9][0x54 / 4] = location as *const () as u32;
    w.tables[6][0x24 / 4] = construction_info as *const () as u32;
    w.tables[8][0x30 / 4] = zone as *const () as u32;
    let _fits = fake(0x0069_5390, fits as *const () as usize);
    let _cache = fake(0x0049_7A00, cache as *const () as usize);
    let calls = w.run(0);
    assert_eq!(
        only(&calls, &["find", "location", "info", "zone", "fits", "cache"]),
        [
            ("find", vec![lot]),
            ("location", vec![lot]),
            ("info", vec![]),
            ("zone", vec![]),
            ("fits", vec![]),
            ("cache", vec![lot]),
            ("find", vec![lot]),
        ]
    );
    let mut expected = [u32::MAX; 28];
    expected[6] = 0;
    assert_eq!(*histogram, expected);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(unsafe { word(s.cache[&lot], 0x14) }, 6);
    });
    assert_eq!(td.suitability_count(), 1);
}
