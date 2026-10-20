// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The city detail manager through the C ABI (docs/specs/city-detail.md):
//! a fake game behind `fc4_city_detail_host`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::c_void;

use fc4_core::rng::{derive_seed, FixedSeeds, RzRandom};

use crate::city_detail::*;
use crate::sinks::{fc4_lot_cfg_object, fc4_lot_cfg_objects_sink, fc4_u32s_sink, fc4_u64s_sink};
use crate::test_streams::{istream, ostream, In, Out};

const PROP: u32 = 0xA823_821E;
const FLORA: u32 = 0x7475_8926;

#[derive(Clone)]
struct Obj {
    object_type: u32,
    flags: u8,
    tag: u32,
    ids: Vec<u32>,
}

/// occupant → (lot, type, tag)
type Occupants = BTreeMap<u64, (u64, u32, Option<u32>)>;

#[derive(Default)]
struct Game {
    setting: Cell<u32>,
    lots: RefCell<Vec<u64>>,
    configs: RefCell<BTreeMap<u64, Vec<Obj>>>,
    occs: RefCell<Occupants>,
    network: RefCell<Vec<(i32, i32, u64)>>,
    log: RefCell<Vec<String>>,
    next: Cell<u64>,
}

impl Game {
    fn new() -> Box<Game> {
        let g = Box::new(Game::default());
        g.setting.set(3);
        g.next.set(0x9000);
        g
    }
    fn user(&self) -> *mut c_void {
        self as *const Game as *mut c_void
    }
    fn log(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    fn on_lot(&self, lot: u64) -> Vec<u64> {
        self.occs
            .borrow()
            .iter()
            .filter(|(_, o)| o.0 == lot)
            .map(|(k, _)| *k)
            .collect()
    }
}

fn game(u: *mut c_void) -> &'static Game {
    unsafe { &*(u as *const Game) }
}

unsafe fn give(sink: fc4_u64s_sink, ctx: *mut c_void, v: &[u64]) {
    if let Some(f) = sink {
        f(ctx, v.as_ptr(), v.len());
    }
}

unsafe extern "C" fn detail_setting(u: *mut c_void) -> u32 {
    game(u).setting.get()
}
unsafe extern "C" fn lots(u: *mut c_void, sink: fc4_u64s_sink, ctx: *mut c_void) {
    give(sink, ctx, &game(u).lots.borrow());
}
unsafe extern "C" fn lot_config_objects(
    u: *mut c_void,
    lot: u64,
    sink: fc4_lot_cfg_objects_sink,
    ctx: *mut c_void,
) -> bool {
    let cfgs = game(u).configs.borrow();
    let Some(objs) = cfgs.get(&lot) else { return false };
    let c: Vec<fc4_lot_cfg_object> = objs
        .iter()
        .map(|o| fc4_lot_cfg_object {
            object_type: o.object_type,
            tag: o.tag,
            flags: o.flags,
            reserved: [0; 3],
            id_count: o.ids.len() as u32,
            ids: o.ids.as_ptr(),
        })
        .collect();
    // Two calls: the sink appends.
    let f = sink.unwrap();
    f(ctx, c.as_ptr(), 1.min(c.len()));
    if c.len() > 1 {
        f(ctx, c.as_ptr().add(1), c.len() - 1);
    }
    true
}
unsafe extern "C" fn lot_occupants(u: *mut c_void, lot: u64, sink: fc4_u64s_sink, ctx: *mut c_void) {
    give(sink, ctx, &game(u).on_lot(lot));
}
unsafe extern "C" fn occupant_type(u: *mut c_void, occ: u64) -> u32 {
    game(u).occs.borrow().get(&occ).map_or(0, |o| o.1)
}
unsafe extern "C" fn occupant_lot_tag(u: *mut c_void, occ: u64, out: *mut u32) -> bool {
    match game(u).occs.borrow().get(&occ).and_then(|o| o.2) {
        Some(t) => {
            *out = t;
            true
        }
        None => false,
    }
}
unsafe extern "C" fn demolish_flora(u: *mut c_void, occ: u64) {
    game(u).log(format!("demolish {occ:X}"));
}
unsafe extern "C" fn remove_prop(u: *mut c_void, occ: u64) {
    game(u).log(format!("remove {occ:X}"));
}
unsafe extern "C" fn object_placement(u: *mut c_void, lot: u64, i: u32, out: *mut fc4_object_placement) {
    game(u).log(format!("placement {lot:X} {i}"));
    *out = fc4_object_placement {
        x: i as f32,
        y: 2.0,
        z: 3.0,
        orientation: 1,
    };
}
unsafe extern "C" fn object_id_array(u: *mut c_void, lot: u64, i: u32, sink: fc4_u32s_sink, ctx: *mut c_void) {
    let ids = game(u).configs.borrow()[&lot][i as usize].ids.clone();
    if let Some(f) = sink {
        f(ctx, ids.as_ptr(), ids.len());
    }
}
unsafe extern "C" fn add_lot_prop(u: *mut c_void, id: u32, p: *const fc4_object_placement) -> u64 {
    let g = game(u);
    let t = g.next.get();
    g.next.set(t + 1);
    g.log(format!("prop {id:X} {} {}", (*p).x, (*p).orientation));
    t
}
unsafe extern "C" fn add_flora(u: *mut c_void, id: u32, p: *const fc4_object_placement) -> u64 {
    game(u).log(format!("flora {id:X} {}", (*p).x));
    0
}
unsafe extern "C" fn set_lot_tag(u: *mut c_void, occ: u64, tag: u32) {
    game(u).log(format!("tag {occ:X} {tag:X}"));
}
unsafe extern "C" fn network_lots(u: *mut c_void, sink: fc4_network_lot_sink, ctx: *mut c_void) {
    for &(x, z, l) in game(u).network.borrow().iter() {
        sink.unwrap()(ctx, x, z, l);
    }
}
unsafe extern "C" fn redevelop(u: *mut c_void, x: i32, z: i32, l: u64) {
    game(u).log(format!("redevelop {x} {z} {l:X}"));
}

fn host(g: &Game) -> fc4_city_detail_host {
    fc4_city_detail_host {
        struct_size: std::mem::size_of::<fc4_city_detail_host>() as u32,
        user: g.user(),
        detail_setting: Some(detail_setting),
        lots: Some(lots),
        lot_config_objects: Some(lot_config_objects),
        lot_occupants: Some(lot_occupants),
        occupants_in_lot_rect: Some(lot_occupants),
        occupant_type: Some(occupant_type),
        occupant_lot_tag: Some(occupant_lot_tag),
        demolish_flora: Some(demolish_flora),
        remove_prop: Some(remove_prop),
        object_placement: Some(object_placement),
        object_id_array: Some(object_id_array),
        add_lot_prop: Some(add_lot_prop),
        add_flora: Some(add_flora),
        set_lot_tag: Some(set_lot_tag),
        network_lots: Some(network_lots),
        redevelop_network_lot: Some(redevelop),
    }
}

fn state(p: *const fc4_city_detail) -> fc4_city_detail_state {
    let mut s = fc4_city_detail_state::default();
    assert!(unsafe { fc4_city_detail_get_state(p, &mut s) });
    s
}

fn write(p: *const fc4_city_detail) -> Vec<u8> {
    let mut o = Out::default();
    let os = ostream(&mut o);
    assert!(unsafe { fc4_city_detail_write(p, &os) });
    o.bytes
}

fn read(p: *mut fc4_city_detail, bytes: &[u8]) -> bool {
    let mut i = In::new(bytes);
    let is = istream(&mut i);
    unsafe { fc4_city_detail_read(p, &is) }
}

#[test]
fn create_rejects_bad_tables_and_identity() {
    let g = Game::new();
    unsafe {
        assert!(fc4_city_detail_create(std::ptr::null(), 0, 0).is_null());
        let mut h = host(&g);
        h.struct_size -= 1;
        assert!(fc4_city_detail_create(&h, 0, 0).is_null());
        assert_eq!(fc4_city_detail_get_level(std::ptr::null()), 0);
    }
    for iid in [1, 0x6AD4_B9F1, 0xE4FD_A3D4] {
        assert!(fc4_city_detail_query_interface(iid));
    }
    assert!(!fc4_city_detail_query_interface(0x4522_94AA));
}

/// §8 tests 1, 2 and 6 (a new city).
#[test]
fn cd8_new_city_and_record() {
    let g = Game::new();
    g.network.borrow_mut().push((1, 2, 0x50));
    unsafe {
        let p = fc4_city_detail_create(&host(&g), 0x55, 0x77);
        assert_eq!(state(p).level, 0xFFFF_FFFF);
        assert!(fc4_city_detail_init(p));
        assert!(g.take().is_empty());
        assert_eq!(write(p), [0, 0, 0, 0, 3, 0, 0, 0]);
        assert!(!read(p, &[1, 0, 0, 0, 1, 0, 0, 0]));
        assert_eq!(fc4_city_detail_get_level(p), 3);
        assert!(read(p, &[0, 0, 0, 0, 1, 0, 0, 0]));
        assert_eq!(fc4_city_detail_get_level(p), 1);
        assert_eq!(fc4_city_detail_add_ref(p), 1);
        assert_eq!(fc4_city_detail_release(p), 0);
        assert_eq!(state(p).destroyed, 1);
        assert!(fc4_city_detail_shutdown(p));
        fc4_city_detail_destroy(p);
    }
}

/// §8 tests 4 and 5 through the ABI, with the session generator.
#[test]
fn cd8_reduce_and_increase() {
    let g = Game::new();
    g.lots.borrow_mut().push(0x10);
    g.configs.borrow_mut().insert(
        0x10,
        vec![
            Obj {
                object_type: 1,
                flags: 0,
                tag: 0xA,
                ids: vec![0xA0],
            },
            Obj {
                object_type: 1,
                flags: 0,
                tag: 0xB,
                ids: vec![0xB0, 0xB1, 0xB2, 0xB3],
            },
            Obj {
                object_type: 1,
                flags: 1,
                tag: 0xC,
                ids: vec![0xC0],
            },
            Obj {
                object_type: 4,
                flags: 0,
                tag: 0xD,
                ids: vec![0xD0],
            },
        ],
    );
    g.occs.borrow_mut().insert(0x11, (0x10, PROP, Some(0xA)));
    g.occs.borrow_mut().insert(0x12, (0x10, PROP, Some(0x7)));
    g.occs.borrow_mut().insert(0x13, (0x10, FLORA, Some(0x8)));
    g.occs.borrow_mut().insert(0x14, (0x10, FLORA, Some(0xD)));
    g.network.borrow_mut().push((4, 5, 0x50));
    unsafe {
        // The pass count n belongs to the handle (CD5); any seed will do.
        let seed = 0x00C1_7D37;
        let p = fc4_city_detail_create(&host(&g), seed, 0x77);
        assert!(read(p, &[0, 0, 0, 0, 3, 0, 0, 0]));
        g.setting.set(1);
        fc4_city_detail_init(p);
        assert_eq!(g.take(), ["remove 12", "demolish 13", "redevelop 4 5 50"]);
        let s = state(p);
        assert_eq!(
            (s.level, s.last_pass, s.last_removed, s.last_network_lots),
            (1, 1, 2, 1)
        );

        // Increase: B is missing (four IDs, one draw), C is optional, D
        // exists; the flora callback answers no occupant.
        g.occs.borrow_mut().remove(&0x14);
        g.setting.set(2);
        let before = state(p).session_passes;
        fc4_city_detail_init(p);
        let mut r = RzRandom::new(
            derive_seed(seed, 0xEAD4_BA36 ^ before),
            &FixedSeeds {
                wall_clock: 0,
                process_clock: 0,
            },
        );
        let pick = [0xB0, 0xB1, 0xB2, 0xB3][r.uniform_u32(4) as usize];
        let d = [0xD0][r.uniform_u32(1) as usize];
        assert_eq!(
            g.take(),
            [
                "placement 10 1".to_string(),
                format!("prop {pick:X} 1 1"),
                "tag 9000 B".to_string(),
                "placement 10 3".to_string(),
                format!("flora {d:X} 3"),
                "redevelop 4 5 50".to_string(),
            ]
        );
        let s = state(p);
        assert_eq!((s.level, s.last_pass, s.last_created, s.last_draws), (2, 2, 1, 2));
        assert_eq!(s.session_passes, before + 1);
        fc4_city_detail_destroy(p);
    }
}

/// CD5: the pass count is per handle. Two handles created with the same
/// session seed both run pass n = 0 first and draw the same.
#[test]
fn cd8_pass_count_per_handle() {
    let run = || {
        let g = Game::new();
        g.lots.borrow_mut().push(0x10);
        g.configs.borrow_mut().insert(
            0x10,
            vec![Obj {
                object_type: 1,
                flags: 0,
                tag: 0xA,
                ids: (0..97).map(|i| 0x100 + i).collect(),
            }],
        );
        unsafe {
            let p = fc4_city_detail_create(&host(&g), 0x4242_4242, 0x77);
            assert_eq!(state(p).session_passes, 0);
            assert!(read(p, &[0, 0, 0, 0, 1, 0, 0, 0]));
            fc4_city_detail_init(p);
            assert_eq!(state(p).session_passes, 1);
            fc4_city_detail_destroy(p);
        }
        g.take()
    };
    let first = run();
    assert!(first.iter().any(|l| l.starts_with("prop ")));
    assert_eq!(run(), first);
}
