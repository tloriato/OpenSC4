// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Fake hosts and the conformance tests of network-manager.md §16.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::grid::CellRect;
use crate::stream::{ByteReader, ByteWriter, RefResolver, RefValue, RegisteredObject, SaveObject, SaveRegistry};

use super::*;

const CITY: (i32, i32) = (64, 64);
const HOST_CLASS: u32 = 0x1234_5678;

/// (tool type, occupants) of one `DestroyOccupantsAndResolve` call.
type DestroyCall = (u32, Vec<u64>);
/// (tool type, cells, occupants) of one underground refit call.
type UndergroundCall = (u32, Vec<(i32, i32)>, Vec<u64>);

fn rect(x1: i32, z1: i32, x2: i32, z2: i32) -> CellRect {
    CellRect { x1, z1, x2, z2 }
}

fn bump(refs: &mut BTreeMap<u64, i64>, t: u64, d: i64) {
    *refs.entry(t).or_insert(0) += d;
}

/// Resolves the references a [`SaveRegistry`] handed out back to host tokens.
struct Resolver(Vec<RegisteredObject>);

impl RefResolver for Resolver {
    fn resolve(&mut self, class_id: u32, object_id: u32) -> Option<RefValue> {
        self.0
            .iter()
            .find(|e| e.1 == object_id)
            .map(|e| RefValue::Host { token: e.3, class_id })
    }
}

/// Writes a record with a registry; returns the bytes and the registered
/// objects.
fn save(obj: &dyn SaveObject) -> (Vec<u8>, Vec<RegisteredObject>) {
    let reg = Rc::new(RefCell::new(SaveRegistry::new()));
    let mut w = ByteWriter::with_registry(reg.clone());
    assert!(obj.write(&mut w));
    let bytes = w.into_bytes();
    let entries = std::mem::take(&mut reg.borrow_mut().entries);
    (bytes, entries)
}

fn tokens(entries: &[RegisteredObject]) -> Vec<u64> {
    entries.iter().map(|e| e.3).collect()
}

// =====================================================================
// The network manager's fake host
// =====================================================================

#[derive(Clone, Default)]
struct Occ {
    flags: u32,
    managed: bool,
    can_be_lit: bool,
    in_city: bool,
    cell: (i32, i32),
    /// Altitude above the terrain (the terrain is flat at 0).
    height: f32,
    creation: u64,
}

#[derive(Default)]
struct MState {
    occ: BTreeMap<u64, Occ>,
    refs: BTreeMap<u64, i64>,
    subs: Vec<u32>,
    unsubs: Vec<u32>,
    next_tool: u64,
    tool_type: BTreeMap<u64, u32>,
    destroy_calls: Vec<DestroyCall>,
    underground_calls: Vec<UndergroundCall>,
    rect_queries: Vec<(CellRect, u32)>,
    lit_calls: Vec<(u64, bool)>,
    removed: Vec<u64>,
    crew_calls: Vec<u64>,
    refreshes: Vec<u64>,
    night: bool,
}

#[derive(Default)]
struct MFake {
    s: RefCell<MState>,
    /// Set by crew tests: the manager a crew unregisters from.
    manager: RefCell<Option<Rc<NetworkManager>>>,
}

impl MFake {
    fn add(&self, t: u64, o: Occ) {
        self.s.borrow_mut().occ.insert(t, o);
    }
    fn refs(&self, t: u64) -> i64 {
        self.s.borrow().refs.get(&t).copied().unwrap_or(0)
    }
}

impl NetworkManagerHost for MFake {
    fn subscribe(&self, m: u32) {
        self.s.borrow_mut().subs.push(m);
    }
    fn unsubscribe(&self, m: u32) {
        self.s.borrow_mut().unsubs.push(m);
    }
    fn uk_paths(&self) -> bool {
        false
    }
    fn city_size(&self) -> (i32, i32) {
        CITY
    }
    fn is_night(&self) -> bool {
        self.s.borrow().night
    }
    fn view_rotation_changed(&self) {}
    fn add_ref(&self, t: u64) {
        bump(&mut self.s.borrow_mut().refs, t, 1);
    }
    fn release(&self, t: u64) {
        bump(&mut self.s.borrow_mut().refs, t, -1);
    }
    fn create_tool(&self, network_type: u32) -> Option<u64> {
        let mut s = self.s.borrow_mut();
        s.next_tool += 1;
        let t = 1000 + s.next_tool;
        s.tool_type.insert(t, network_type);
        bump(&mut s.refs, t, 1);
        Some(t)
    }
    fn tool_init(&self, _: u64) {}
    fn tool_shutdown(&self, _: u64) {}
    fn tool_destroy_occupants(&self, tool: u64, occupants: &[u64]) {
        let mut s = self.s.borrow_mut();
        let ty = s.tool_type[&tool];
        s.destroy_calls.push((ty, occupants.to_vec()));
    }
    fn tool_update_underground_heights(&self, tool: u64, cells: &[(i32, i32)], occupants: &[u64], network_type: u32) {
        let mut s = self.s.borrow_mut();
        assert_eq!(s.tool_type[&tool], network_type);
        s.underground_calls
            .push((network_type, cells.to_vec(), occupants.to_vec()));
    }
    fn tool_cell_info(&self, _: u64, packed: u32) -> u64 {
        u64::from(packed)
    }
    fn tool_reinsert_piece(&self, _: u64, _: u64) {}
    fn query_network_occupant(&self, o: u64) -> Option<u64> {
        if self.is_network_occupant(o) {
            self.add_ref(o);
            Some(o)
        } else {
            None
        }
    }
    fn is_network_occupant(&self, o: u64) -> bool {
        self.s
            .borrow()
            .occ
            .get(&o)
            .is_some_and(|x| x.flags & flag::ALL_TYPES != 0)
    }
    fn network_flags(&self, n: u64) -> u32 {
        self.s.borrow().occ.get(&n).map_or(0, |o| o.flags)
    }
    fn is_of_type(&self, n: u64, t: u32) -> bool {
        self.network_flags(n) & (1 << t) != 0
    }
    fn occupant_flags(&self, o: u64) -> u32 {
        u32::from(self.s.borrow().occ.get(&o).is_some_and(|x| x.in_city))
    }
    fn occupant_flag_set(&self, _: u64, _: u32) -> bool {
        false
    }
    fn occupied_cell(&self, n: u64) -> (i32, i32) {
        self.s.borrow().occ[&n].cell
    }
    fn occupant_altitude(&self, n: u64, _: f32, _: f32) -> f32 {
        self.s.borrow().occ[&n].height
    }
    fn is_managed_globally(&self, n: u64) -> bool {
        self.s.borrow().occ[&n].managed
    }
    fn can_be_lit(&self, n: u64) -> bool {
        self.s.borrow().occ[&n].can_be_lit
    }
    fn set_lit(&self, n: u64, lit: bool) {
        self.s.borrow_mut().lit_calls.push((n, lit));
    }
    fn check_for_retrofit(&self, _: u64) -> bool {
        false
    }
    fn retrofit(&self, _: u64) {}
    fn creation_number(&self, o: u64) -> u64 {
        self.s.borrow().occ[&o].creation
    }
    fn remove_occupant(&self, o: u64) -> bool {
        self.s.borrow_mut().removed.push(o);
        true
    }
    fn occupants_in_rect(&self, r: CellRect, mask: u32) -> Vec<u64> {
        let mut s = self.s.borrow_mut();
        s.rect_queries.push((r, mask));
        s.occ
            .iter()
            .filter(|(_, o)| {
                let (x, z) = o.cell;
                x >= r.x1 && x <= r.x2 && z >= r.z1 && z <= r.z2 && o.flags & mask != 0
            })
            .map(|(t, _)| *t)
            .collect()
    }
    fn network_occupants(&self) -> Vec<u64> {
        self.s.borrow().occ.keys().copied().collect()
    }
    fn post_refresh(&self, n: u64) {
        self.s.borrow_mut().refreshes.push(n);
    }
    fn terrain_altitude(&self, _: f32, _: f32) -> f32 {
        0.0
    }
    fn crew_shutdown(&self, crew: u64) {
        self.s.borrow_mut().crew_calls.push(crew);
        let m = self.manager.borrow().clone();
        if let Some(m) = m {
            m.unregister_construction_crew(crew);
        }
    }
    fn object_ref(&self, token: u64) -> RefValue {
        RefValue::Host {
            token,
            class_id: HOST_CLASS,
        }
    }
    fn resolve_ref(&self, r: &RefValue, _: u32) -> Option<u64> {
        match r {
            RefValue::Host { token, .. } => {
                self.add_ref(*token);
                Some(*token)
            }
            _ => None,
        }
    }
}

fn manager() -> (Rc<MFake>, Rc<NetworkManager>) {
    let f = Rc::new(MFake::default());
    let m = NetworkManager::new(f.clone(), 1);
    m.init();
    (f, m)
}

fn road(cell: (i32, i32), creation: u64) -> Occ {
    Occ {
        flags: flag::ROAD | flag::USABLE,
        in_city: true,
        cell,
        creation,
        ..Occ::default()
    }
}

fn bridge(creation: u64) -> Occ {
    Occ {
        managed: true,
        can_be_lit: true,
        ..road((1, 1), creation)
    }
}

fn insert_msg(o: u64) -> NetworkMessage {
    NetworkMessage {
        void1: Some(o),
        ..NetworkMessage::of_type(msg::OCCUPANT_INSERTED)
    }
}

fn remove_msg(o: u64) -> NetworkMessage {
    NetworkMessage {
        void1: Some(o),
        ..NetworkMessage::of_type(msg::OCCUPANT_REMOVED)
    }
}

fn terrain_msg(data1: u32, data2: u32, kind: u32) -> NetworkMessage {
    NetworkMessage {
        data1,
        data2,
        data4: kind,
        ..NetworkMessage::of_type(msg::TERRAIN_CHANGED)
    }
}

// ---- lifecycle ----

#[test]
fn init_subscribes_and_caches_tools_shutdown_releases() {
    let (f, m) = manager();
    assert_eq!(f.s.borrow().subs, msg::MANAGER_SUBSCRIPTIONS.to_vec());
    let tools = m.snapshot().tools;
    assert!(tools.iter().all(|t| t.is_some()));
    // A second Init does nothing.
    m.init();
    assert_eq!(f.s.borrow().subs.len(), 11);
    assert!(m.shutdown());
    assert_eq!(f.s.borrow().unsubs, msg::MANAGER_SUBSCRIPTIONS.to_vec());
    for t in tools.into_iter().flatten() {
        assert_eq!(f.refs(t), 0);
    }
    assert!(m.snapshot().tools.iter().all(|t| t.is_none()));
}

#[test]
fn release_destroys_at_one() {
    let (_, m) = manager();
    m.add_ref();
    m.add_ref();
    assert_eq!(m.release(), 1);
    assert_eq!(m.release(), 0);
    assert!(m.snapshot().destroyed);
}

// ---- §16 test 1 ----

#[test]
fn t01_component_53_empty() {
    let (_, m) = manager();
    let (bytes, _) = save(&*m);
    assert_eq!(bytes, [1, 0, 0, 0, 0, 0]);
    let mut r = ByteReader::new(&[2, 0, 0, 0, 0, 0]);
    assert!(!m.read(&mut r));
}

#[test]
fn component_53_read_null_and_resolved_refs() {
    let (f, m) = manager();
    f.add(7, bridge(1));
    let mut bytes = vec![1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
    bytes.extend_from_slice(&HOST_CLASS.to_le_bytes());
    let mut res = Resolver(vec![(HOST_CLASS, 1, None, 7)]);
    let mut r = ByteReader::with_resolver(&bytes, &mut res);
    assert!(m.read(&mut r));
    assert_eq!(m.snapshot().managed, vec![None, Some(7)]);
    // An unresolvable reference fails the read.
    let mut res = Resolver(vec![]);
    let mut r = ByteReader::with_resolver(&bytes, &mut res);
    assert!(!m.read(&mut r));
}

// ---- §16 tests 3–6 ----

#[test]
fn t03_managed_list_order() {
    let (f, m) = manager();
    for (t, c) in [(11, 1), (12, 2), (13, 3)] {
        f.add(t, bridge(c));
        m.do_message(&insert_msg(t));
    }
    m.do_message(&remove_msg(11));
    assert_eq!(m.snapshot().managed, vec![Some(13), Some(12)]);
    assert_eq!(f.refs(11), 0);
    let (bytes, entries) = save(&*m);
    let mut want = vec![1, 0, 2, 0, 0, 0, 1, 0, 0, 0];
    want.extend_from_slice(&HOST_CLASS.to_le_bytes());
    want.extend_from_slice(&[2, 0, 0, 0]);
    want.extend_from_slice(&HOST_CLASS.to_le_bytes());
    assert_eq!(bytes, want);
    assert_eq!(tokens(&entries), vec![13, 12]);
}

#[test]
fn t04_duplicate_insert() {
    let (f, m) = manager();
    f.add(11, bridge(1));
    m.do_message(&insert_msg(11));
    m.do_message(&insert_msg(11));
    assert_eq!(m.snapshot().managed, vec![Some(11), Some(11)]);
    m.do_message(&remove_msg(11));
    assert_eq!(m.snapshot().managed, vec![Some(11)]);
    assert_eq!(f.refs(11), 1);
}

#[test]
fn t05_not_managed() {
    let (f, m) = manager();
    f.add(11, road((3, 3), 1));
    m.do_message(&insert_msg(11));
    assert!(m.snapshot().managed.is_empty());
    assert_eq!(f.refs(11), 0);
}

#[test]
fn t06_lighting() {
    let (f, m) = manager();
    f.add(11, bridge(1));
    f.add(
        12,
        Occ {
            can_be_lit: false,
            ..bridge(2)
        },
    );
    m.do_message(&insert_msg(11));
    m.do_message(&insert_msg(12));
    m.do_message(&NetworkMessage::of_type(msg::DUSK));
    assert_eq!(f.s.borrow().lit_calls, vec![(11, true)]);
    m.do_message(&NetworkMessage::of_type(msg::DAWN));
    assert_eq!(f.s.borrow().lit_calls, vec![(11, true), (11, false)]);
}

// ---- §16 tests 7–9 ----

#[test]
fn t07_get_network_tool() {
    let (f, m) = manager();
    assert_eq!(m.get_network_tool(13, false), None);
    let a = m.get_network_tool(5, false).unwrap();
    let refs = f.refs(a);
    assert_eq!(m.get_network_tool(5, false), Some(a));
    assert_eq!(f.refs(a), refs);
    let b = m.get_network_tool(5, true).unwrap();
    assert_ne!(a, b);
    assert_eq!(f.s.borrow().tool_type[&b], 5);
}

#[test]
fn t08_queue_removals_dedupe() {
    let (f, m) = manager();
    f.add(21, road((1, 1), 2));
    f.add(22, road((2, 2), 1));
    m.queue_removals(true);
    m.destroy_network_occupants(&[21, 22]);
    m.destroy_network_occupants(&[21]);
    assert!(f.s.borrow().destroy_calls.is_empty());
    m.queue_removals(false);
    // Ordered by creation number.
    assert_eq!(f.s.borrow().destroy_calls, vec![(0, vec![22, 21])]);
    assert!(m.snapshot().pending.is_empty());
    // The collected references are kept *[quirk]*; the queue's are released.
    assert_eq!(f.refs(21), 1);
    assert_eq!(f.refs(22), 1);
}

#[test]
fn t09_groups() {
    let (f, m) = manager();
    f.add(31, road((1, 1), 1));
    f.add(
        32,
        Occ {
            flags: flag::PIPE,
            ..road((1, 1), 2)
        },
    );
    f.add(
        33,
        Occ {
            flags: flag::SUBWAY,
            ..road((1, 1), 3)
        },
    );
    m.destroy_network_occupants(&[33, 32, 31]);
    assert_eq!(
        f.s.borrow().destroy_calls,
        vec![(0, vec![31]), (4, vec![32]), (7, vec![33])]
    );
}

#[test]
fn null_tool_skips_group() {
    let (f, m) = manager();
    f.add(31, road((1, 1), 1));
    m.pre_city_shutdown();
    m.destroy_network_occupants(&[31]);
    assert!(f.s.borrow().destroy_calls.is_empty());
}

// ---- §16 tests 10–11 ----

#[test]
fn t10_terrain_rect() {
    assert_eq!(terrain_rect(0x0006_0009, 0x0000_000F, CITY), rect(5, 0, 9, 15));
    let (_, m) = manager();
    m.do_message(&NetworkMessage::of_type(msg::LONG_TERRAIN_BEGIN));
    m.with_state(|s| s.dirty = rect(8, 3, 20, 4));
    m.do_message(&terrain_msg(0x0006_0009, 0x0000_000F, terrain_kind::LEVELLING));
    let s = m.snapshot();
    assert_eq!(s.dirty, rect(5, 0, 20, 15));
    assert!(s.long_terrain_op);
    m.do_message(&NetworkMessage::of_type(msg::LONG_TERRAIN_END));
    let s = m.snapshot();
    assert!(rect_is_empty(&s.dirty));
    assert!(!s.long_terrain_op);
}

/// Runs one terrain change over the cell (5, 5) and returns the destroy
/// calls.
fn terrain_case(flags: u32, height: f32, kind: u32) -> (Vec<DestroyCall>, Vec<(CellRect, u32)>) {
    let (f, m) = manager();
    f.add(
        41,
        Occ {
            flags,
            height,
            ..road((5, 5), 1)
        },
    );
    m.do_message(&terrain_msg(0x0005_0006, 0x0005_0006, kind));
    let s = f.s.borrow();
    (s.destroy_calls.clone(), s.rect_queries.clone())
}

#[test]
fn t11_corner_rule() {
    let road_f = flag::ROAD;
    assert!(terrain_case(road_f, 0.005, 1).0.is_empty());
    assert_eq!(terrain_case(road_f, 0.02, 1).0, vec![(0, vec![41])]);
    assert_eq!(terrain_case(road_f, -0.001, 1).0, vec![(0, vec![41])]);
    assert!(terrain_case(road_f, 0.02, terrain_kind::KEEP_ALL).0.is_empty());
    assert!(terrain_case(flag::PIPE, 0.02, 1).0.is_empty());
    assert_eq!(
        terrain_case(flag::PIPE, 0.02, terrain_kind::UNDERGROUND).0,
        vec![(4, vec![41])]
    );
    let (calls, queries) = terrain_case(road_f, 0.02, terrain_kind::LEVELLING);
    assert!(calls.is_empty());
    assert!(queries.iter().all(|(_, mask)| *mask != flag::ALL_TYPES));
    // Kind 6 keeps pieces with 0x80000.
    assert!(
        terrain_case(road_f | flag::UNKNOWN_80000, 0.02, terrain_kind::SKIP_80000)
            .0
            .is_empty()
    );
    assert!(!corner_mismatch(f32::NAN));
}

#[test]
fn terrain_underground_refit_groups_pipes_and_subways() {
    let (f, m) = manager();
    f.add(
        51,
        Occ {
            flags: flag::PIPE,
            ..road((5, 5), 1)
        },
    );
    f.add(
        52,
        Occ {
            flags: flag::SUBWAY,
            ..road((6, 5), 2)
        },
    );
    m.do_message(&terrain_msg(0x0005_0006, 0x0005_0006, terrain_kind::KEEP_ALL));
    let s = f.s.borrow();
    assert_eq!(
        s.underground_calls,
        vec![(4, vec![(5, 5)], vec![51]), (7, vec![(6, 5)], vec![52])]
    );
    assert_eq!(s.refs.get(&51).copied().unwrap_or(0), 0);
    drop(s);
    assert!(rect_is_empty(&m.snapshot().dirty));
}

// ---- other manager slots ----

#[test]
fn remove_or_replace_intersection() {
    let (f, m) = manager();
    f.add(61, road((3, 4), 1));
    f.add(
        62,
        Occ {
            flags: flag::ROAD | flag::INTERSECTION,
            ..road((3, 4), 2)
        },
    );
    assert_eq!(m.remove_or_replace_occupant(1001, None), (false, false));
    assert_eq!(m.remove_or_replace_occupant(1001, Some(61)), (true, false));
    assert_eq!(m.remove_or_replace_occupant(1001, Some(62)), (true, true));
    assert_eq!(f.s.borrow().removed, vec![61, 62]);
}

#[test]
fn crews_unregister_during_pre_city_shutdown() {
    let (f, m) = manager();
    *f.manager.borrow_mut() = Some(m.clone());
    m.register_construction_crew(71);
    m.register_construction_crew(72);
    m.do_message(&NetworkMessage::of_type(msg::PRE_CITY_SHUTDOWN));
    assert_eq!(f.s.borrow().crew_calls, vec![71, 72]);
    assert!(m.snapshot().crews.is_empty());
    assert_eq!(f.refs(71), 0);
    assert!(m.snapshot().tools.iter().all(|t| t.is_none()));
    *f.manager.borrow_mut() = None;
}

#[test]
fn retrofit_list_is_emptied_and_released() {
    let (f, m) = manager();
    f.add(81, bridge(1));
    m.add_to_retrofit_list(81);
    assert_eq!(f.refs(81), 1);
    m.do_message(&NetworkMessage::of_type(msg::POST_CITY_INIT));
    assert!(m.snapshot().retrofit.is_empty());
    assert_eq!(f.refs(81), 0);
}

#[test]
fn english_names_and_interfaces() {
    assert_eq!(english_network_name(0), Some("Road"));
    assert_eq!(english_network_name(13), Some("road"));
    assert_eq!(english_network_name(14), None);
    assert!(manager_query_interface(IID_NETWORK_MANAGER));
    assert!(!manager_query_interface(IID_NETWORK_LOT_MANAGER));
    assert!(lot_manager_query_interface(IID_NETWORK_LOT_MANAGER));
    assert_eq!(lot_key(0x1AB, 0x7), 0xAB07);
    assert_eq!(key_cell(0xAB07), (0xAB, 7));
}

// =====================================================================
// The riot lock (§16 tests 12–13)
// =====================================================================

#[derive(Default)]
struct RFake {
    tiles: BTreeMap<(i32, i32), u64>,
    counters: RefCell<BTreeMap<u64, RemovalCounters>>,
    refs: RefCell<BTreeMap<u64, i64>>,
}

impl RFake {
    fn can_remove(&self, n: u64, level: u32) -> bool {
        self.counters
            .borrow()
            .get(&n)
            .copied()
            .unwrap_or_default()
            .can_remove(level)
    }
}

impl RiotHost for RFake {
    fn tile_occupant(&self, x: i32, z: i32) -> Option<u64> {
        self.tiles.get(&(x, z)).copied()
    }
    fn add_ref(&self, t: u64) {
        bump(&mut self.refs.borrow_mut(), t, 1);
    }
    fn release(&self, t: u64) {
        bump(&mut self.refs.borrow_mut(), t, -1);
    }
    fn unset_removal_flags(&self, o: u64, m: u32) {
        self.counters.borrow_mut().entry(o).or_default().unset_removal_flags(m);
    }
    fn set_removal_flags(&self, o: u64, m: u32) {
        self.counters.borrow_mut().entry(o).or_default().set_removal_flags(m);
    }
}

#[test]
fn t12_riot_lock() {
    let mut h = RFake::default();
    h.tiles.insert((10, 10), 90);
    let mut lock = RiotLock::new();
    lock.add_instance();
    lock.mark(&h, 12, 12);
    assert!(!h.can_remove(90, 2));
    assert!(h.can_remove(90, 1));
    lock.add_instance();
    lock.mark(&h, 11, 11);
    assert_eq!(lock.locked(), vec![90]);
    assert_eq!(h.refs.borrow()[&90], 1);
    lock.remove_instance(&h);
    assert!(!h.can_remove(90, 2));
    lock.remove_instance(&h);
    assert!(h.can_remove(90, 2));
    assert!(lock.locked().is_empty());
    assert_eq!(h.refs.borrow()[&90], 0);
}

#[test]
fn t13_riot_edge() {
    let mut h = RFake::default();
    h.tiles.insert((2, 10), 91);
    h.tiles.insert((4, 10), 92);
    let mut lock = RiotLock::new();
    lock.add_instance();
    lock.mark(&h, 2, 10);
    assert!(lock.locked().is_empty());
    assert!(h.can_remove(91, 2));
}

// =====================================================================
// Pure rules (§16 tests 14–17, 22)
// =====================================================================

#[test]
fn t14_zone_for_network() {
    let zones = |m: Vec<((i32, i32), i32)>| {
        let m: BTreeMap<(i32, i32), i32> = m.into_iter().collect();
        move |x: i32, z: i32| m.get(&(x, z)).copied().unwrap_or(0)
    };
    let z = zones(vec![((4, 5), 1), ((5, 4), 6), ((6, 5), 8)]);
    assert_eq!(zone_choice(5, 5, CITY, z), 8);
    let z = zones(vec![((4, 5), 7), ((4, 6), 2)]);
    assert_eq!(zone_choice(5, 5, CITY, z), 2);
    // Without the agriculture side the diagonals are still visited (best 0).
    let z = zones(vec![((4, 6), 2)]);
    assert_eq!(zone_choice(5, 5, CITY, z), 2);
    // A residential side stops the diagonals.
    let z = zones(vec![((4, 5), 1), ((4, 6), 9)]);
    assert_eq!(zone_choice(5, 5, CITY, z), 1);
    let z = zones(vec![((5, 5), 4), ((4, 5), 8)]);
    assert_eq!(zone_choice(5, 5, CITY, z), 4);
    assert_eq!(zone_priority(16), None);
}

#[test]
fn t15_wealth_for_network() {
    let lots = |m: Vec<((i32, i32), i32)>| {
        let m: BTreeMap<(i32, i32), i32> = m.into_iter().collect();
        move |x: i32, z: i32| m.get(&(x, z)).copied()
    };
    let w = lots(vec![((4, 5), 1), ((5, 4), 2), ((6, 5), 2), ((5, 6), 1)]);
    assert_eq!(wealth_choice(5, 5, CITY, w), 2);
    let w = lots(vec![((4, 5), 1), ((5, 4), 1), ((6, 5), 2)]);
    assert_eq!(wealth_choice(5, 5, CITY, w), 1);
    let w = lots(vec![((6, 6), 3)]);
    assert_eq!(wealth_choice(5, 5, CITY, w), 3);
    assert_eq!(wealth_choice(5, 5, CITY, lots(vec![])), 0);
    // A side lot stops the diagonals.
    let w = lots(vec![((4, 5), 1), ((6, 6), 3), ((4, 4), 3)]);
    assert_eq!(wealth_choice(5, 5, CITY, w), 1);
}

fn cfg(id: u32, piece: u32, zones: &[usize]) -> NetworkLotConfig {
    let mut c = NetworkLotConfig {
        token: u64::from(id),
        piece_id: piece,
        id,
        wealths: [true; 4],
        ..NetworkLotConfig::default()
    };
    for &z in zones {
        c.zones[z] = true;
    }
    c
}

#[test]
fn t16_compatibility_pattern() {
    let mut c = cfg(1, 1, &[1]);
    c.period = 2;
    c.mask = [0x01, 0x02, 0, 0];
    assert!(is_lot_compatible_pattern(&c, 4, 6, 1, 0, 0, 0));
    assert!(!is_lot_compatible_pattern(&c, 5, 6, 1, 0, 0, 0));
    assert!(is_lot_compatible_pattern(&c, 5, 7, 1, 0, 0, 0));
    let mut none = c.clone();
    none.zones = [false; 16];
    assert!(!is_lot_compatible_pattern(&none, 4, 6, 1, 0, 0, 0));
    c.min_slope = 0.0;
    c.max_slope = 2.0;
    assert!(!slope_fits(&c, 3.0));
    assert!(slope_fits(&c, 2.0));
    assert!(!slope_fits(&c, f32::NAN));
    // Flip rules and the zero period.
    let mut f = cfg(2, 1, &[1]);
    f.flip_rule = 1;
    assert!(!is_lot_compatible_pattern(&f, 0, 0, 1, 0, 0, 0));
    assert!(is_lot_compatible_pattern(&f, 0, 0, 1, 0, 0, 1));
    f.period = 0;
    assert!(!is_lot_compatible_pattern(&f, 0, 0, 1, 0, 0, 1));
}

#[test]
fn t17_object_position() {
    let alt = |x: f32, z: f32| x + z / 1000.0;
    let ([x, y, z], o) = object_position((10, 20), 1, false, 2, [3.0, 0.5, 5.0], alt);
    assert_eq!((x, z, o), (165.0, 333.0, 3));
    assert_eq!(y, alt(165.0, 333.0) + 0.5);
    let ([x, y2, _], o) = object_position((10, 20), 1, true, 2, [3.0, 0.5, 5.0], alt);
    assert_eq!((x, o), (171.0, 1));
    // y is taken before the mirror *[quirk]*.
    assert_eq!(y2, y);
}

#[test]
fn t22_usable_message() {
    let c = set_network_flag(flag::ROAD, flag::USABLE, true);
    assert_eq!(c.usable_post, Some(1));
    assert_eq!(set_network_flag(c.flags, flag::USABLE, true).usable_post, None);
    assert_eq!(clear_network_flag(c.flags, flag::USABLE).usable_post, Some(0));
    assert_eq!(set_network_flag(flag::ROAD, flag::USABLE, false).usable_post, None);
    let c = set_network_flag(flag::ROAD, flag::USABLE, false);
    assert_eq!(clear_network_flag(c.flags, flag::USABLE).usable_post, Some(0));
    assert!(set_network_flag(0, flag::TRANSLUCENT, true).translucency);
    assert_eq!(pipe_model(flag::PIPE_BURST | flag::PIPE_DISTRESSED), PipeModel::Burst);
}

#[test]
fn style_tables() {
    assert_eq!(road_texture(1, 2), 0x0820_0000);
    assert_eq!(road_variation(1, 2), 2);
    assert_eq!(road_texture(7, 0), 0x0840_0000);
    assert_eq!(road_variation(14, 3), 7);
    assert_eq!(road_texture(0, 0), 0);
    assert_eq!(road_texture(99, 3), 0x0830_0000);
    assert_eq!(pothole_offset(1), (-1, 0));
    assert_eq!(pothole_offset(16), (0, 0));
}

// =====================================================================
// The network lot manager's fake host
// =====================================================================

#[derive(Clone, Default)]
struct Piece {
    flags: u32,
    piece_id: u32,
    rot: i32,
    flip: i32,
    cell: (i32, i32),
    intersection: bool,
    edges: u32,
    active: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Effect {
    name: String,
    pos: Position,
    started: bool,
    stopped: bool,
    inserted: bool,
    removed: bool,
}

#[derive(Default)]
struct LState {
    pieces: BTreeMap<u64, Piece>,
    zones: BTreeMap<(i32, i32), i32>,
    lots: BTreeMap<(i32, i32), u64>,
    lot_wealth: BTreeMap<u64, i32>,
    lot_rect: BTreeMap<u64, CellRect>,
    configs: Vec<NetworkLotConfig>,
    object_ids: BTreeMap<(u64, usize), Vec<u32>>,
    refs: BTreeMap<u64, i64>,
    textures: Vec<(u64, u32)>,
    variations: Vec<(u64, u32)>,
    flag_sets: Vec<(u64, u32)>,
    prop_removals: Vec<(i32, i32)>,
    flora_removals: Vec<(i32, i32)>,
    street_props: Vec<(u32, Position, u32)>,
    floras: Vec<(u32, Position)>,
    tile_queries: Vec<(i32, i32)>,
    next: u64,
    effects: BTreeMap<u64, Effect>,
    slope: f32,
}

#[derive(Default)]
struct LFake {
    s: RefCell<LState>,
}

impl LFake {
    fn piece(&self, t: u64, p: Piece) {
        self.s.borrow_mut().pieces.insert(t, p);
    }
    fn zone(&self, x: i32, z: i32, zone: i32) {
        self.s.borrow_mut().zones.insert((x, z), zone);
    }
    fn refs(&self, t: u64) -> i64 {
        self.s.borrow().refs.get(&t).copied().unwrap_or(0)
    }
    fn new_token(&self) -> u64 {
        let mut s = self.s.borrow_mut();
        s.next += 1;
        5000 + s.next
    }
}

impl NetworkLotHost for LFake {
    fn subscribe(&self, _: u32) {}
    fn unsubscribe(&self, _: u32) {}
    fn load_configurations(&self) -> Vec<NetworkLotConfig> {
        self.s.borrow().configs.clone()
    }
    fn add_ref(&self, t: u64) {
        bump(&mut self.s.borrow_mut().refs, t, 1);
    }
    fn release(&self, t: u64) {
        bump(&mut self.s.borrow_mut().refs, t, -1);
    }
    fn query_network_occupant(&self, u: u64) -> Option<u64> {
        if self.s.borrow().pieces.contains_key(&u) {
            self.add_ref(u);
            Some(u)
        } else {
            None
        }
    }
    fn network_flags(&self, n: u64) -> u32 {
        self.s.borrow().pieces.get(&n).map_or(0, |p| p.flags)
    }
    fn piece_id(&self, n: u64) -> u32 {
        self.s.borrow().pieces[&n].piece_id
    }
    fn rotation(&self, n: u64) -> i32 {
        self.s.borrow().pieces[&n].rot
    }
    fn flip(&self, n: u64) -> i32 {
        self.s.borrow().pieces[&n].flip
    }
    fn position_cell(&self, n: u64) -> (i32, i32) {
        self.s.borrow().pieces[&n].cell
    }
    fn occupied_cell(&self, n: u64) -> (i32, i32) {
        self.s.borrow().pieces[&n].cell
    }
    fn occupant_altitude(&self, _: u64, _: f32, _: f32) -> f32 {
        2.0
    }
    fn is_intersection(&self, n: u64) -> bool {
        self.s.borrow().pieces[&n].intersection
    }
    fn edge_types_flag(&self, n: u64, kind: u32) -> u32 {
        assert_eq!(kind, AVENUE_EDGE_KIND);
        self.s.borrow().pieces[&n].edges
    }
    fn active_edges(&self, n: u64) -> u32 {
        self.s.borrow().pieces[&n].active
    }
    fn set_network_flag(&self, n: u64, bits: u32) {
        let mut s = self.s.borrow_mut();
        s.flag_sets.push((n, bits));
        if let Some(p) = s.pieces.get_mut(&n) {
            p.flags |= bits;
        }
    }
    fn set_under_texture(&self, n: u64, t: u32) {
        self.s.borrow_mut().textures.push((n, t));
    }
    fn set_variation(&self, n: u64, v: u32) {
        self.s.borrow_mut().variations.push((n, v));
    }
    fn traffic_map_exists(&self) -> bool {
        false
    }
    fn find_network_tile(&self, x: i32, z: i32, mask: u32) -> Option<u64> {
        let mut s = self.s.borrow_mut();
        s.tile_queries.push((x, z));
        s.pieces
            .iter()
            .find(|(_, p)| p.cell == (x, z) && p.flags & mask != 0)
            .map(|(t, _)| *t)
    }
    fn update_intersection(&self, _: u64) {}
    fn city_size(&self) -> (i32, i32) {
        CITY
    }
    fn cell_center(&self, x: i32, z: i32) -> (f32, f32) {
        ((x * 16 + 8) as f32, (z * 16 + 8) as f32)
    }
    fn terrain_altitude(&self, _: f32, _: f32) -> f32 {
        1.0
    }
    fn zone_type(&self, x: i32, z: i32) -> i32 {
        self.s.borrow().zones.get(&(x, z)).copied().unwrap_or(0)
    }
    fn lot_at(&self, x: i32, z: i32) -> Option<u64> {
        self.s.borrow().lots.get(&(x, z)).copied()
    }
    fn lot_wealth(&self, lot: u64) -> i32 {
        self.s.borrow().lot_wealth.get(&lot).copied().unwrap_or(0)
    }
    fn lot_is_livable(&self, _: u64) -> bool {
        false
    }
    fn lot_building_type(&self, _: u64) -> u32 {
        0
    }
    fn building_profile_wealth(&self, _: u32) -> Option<i32> {
        None
    }
    fn query_lot(&self, u: u64) -> Option<u64> {
        if self.s.borrow().lot_rect.contains_key(&u) {
            self.add_ref(u);
            Some(u)
        } else {
            None
        }
    }
    fn lot_bounding_rect(&self, lot: u64) -> CellRect {
        self.s.borrow().lot_rect[&lot]
    }
    fn area_slope(&self, _: i32, _: i32) -> f32 {
        self.s.borrow().slope
    }
    fn remove_street_props(&self, x: i32, z: i32) {
        self.s.borrow_mut().prop_removals.push((x, z));
    }
    fn add_street_prop(&self, id: u32, pos: Position, o: u32) -> Option<u64> {
        let t = self.new_token();
        self.s.borrow_mut().street_props.push((id, pos, o));
        Some(t)
    }
    fn street_prop_slot28(&self, _: u64, _: u32) {}
    fn street_prop_slot29(&self, _: u64, _: u32) {}
    fn demolish_flora(&self, x: i32, z: i32) {
        self.s.borrow_mut().flora_removals.push((x, z));
    }
    fn add_flora(&self, id: u32, pos: Position) -> Option<u64> {
        let t = self.new_token();
        self.s.borrow_mut().floras.push((id, pos));
        Some(t)
    }
    fn flora_slot28(&self, _: u64, _: u32) {}
    fn object_ids(&self, config: u64, index: usize) -> Vec<u32> {
        self.s
            .borrow()
            .object_ids
            .get(&(config, index))
            .cloned()
            .unwrap_or_default()
    }
    fn create_effect_prop(&self) -> Option<u64> {
        let t = self.new_token();
        let mut s = self.s.borrow_mut();
        bump(&mut s.refs, t, 1);
        s.effects.insert(t, Effect::default());
        Some(t)
    }
    fn effect_set_name(&self, p: u64, name: &str) {
        self.s.borrow_mut().effects.get_mut(&p).unwrap().name = name.to_string();
    }
    fn effect_start(&self, p: u64) {
        self.s.borrow_mut().effects.get_mut(&p).unwrap().started = true;
    }
    fn effect_stop(&self, p: u64) {
        self.s.borrow_mut().effects.get_mut(&p).unwrap().stopped = true;
    }
    fn set_position(&self, p: u64, pos: Position) {
        self.s.borrow_mut().effects.get_mut(&p).unwrap().pos = pos;
    }
    fn insert_occupant(&self, p: u64) {
        self.s.borrow_mut().effects.get_mut(&p).unwrap().inserted = true;
    }
    fn remove_occupant(&self, p: u64) {
        self.s.borrow_mut().effects.get_mut(&p).unwrap().removed = true;
    }
    fn first_network_occupant_at(&self, x: i32, z: i32, mask: u32) -> Option<u64> {
        self.s
            .borrow()
            .pieces
            .iter()
            .find(|(_, p)| p.cell == (x, z) && p.flags & mask != 0)
            .map(|(t, _)| *t)
    }
    fn object_ref(&self, token: u64) -> RefValue {
        RefValue::Host {
            token,
            class_id: HOST_CLASS,
        }
    }
    fn resolve_ref(&self, r: &RefValue, _: u32) -> Option<u64> {
        match r {
            RefValue::Host { token, .. } => {
                self.add_ref(*token);
                Some(*token)
            }
            _ => None,
        }
    }
}

/// Records each draw's `n` and answers 0.
struct Draws(Rc<RefCell<Vec<u32>>>);

impl NetworkLotRandom for Draws {
    fn uniform_u32(&mut self, n: u32) -> u32 {
        self.0.borrow_mut().push(n);
        0
    }
}

fn lot_manager(configs: Vec<NetworkLotConfig>) -> (Rc<LFake>, Rc<NetworkLotManager>, Rc<RefCell<Vec<u32>>>) {
    let f = Rc::new(LFake::default());
    f.s.borrow_mut().configs = configs;
    let m = NetworkLotManager::new(f.clone(), 2, 0);
    let draws = Rc::new(RefCell::new(Vec::new()));
    m.set_random(Box::new(Draws(draws.clone())));
    m.init();
    (f, m, draws)
}

fn street(piece_id: u32, cell: (i32, i32)) -> Piece {
    Piece {
        flags: flag::STREET | flag::USABLE,
        piece_id,
        cell,
        ..Piece::default()
    }
}

// ---- §16 test 2 and the record ----

#[test]
fn t02_component_11_empty() {
    let (_, m, _) = lot_manager(vec![]);
    let (bytes, _) = save(&*m);
    assert_eq!(bytes, [2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let data = [1u8, 0, 5, 0, 0, 0];
    let mut r = ByteReader::new(&data);
    assert!(!m.read(&mut r));
    assert_eq!(r.position(), 2);
}

#[test]
fn component_11_round_trip_and_init_resolution() {
    let (f, m, _) = lot_manager(vec![]);
    m.with_state(|s| {
        s.lots.insert(
            lot_key(3, 4),
            LotEntry {
                occupant: Some(50),
                config: None,
                config_id: 7,
                locked: false,
            },
        );
        s.lots.insert(
            lot_key(1, 2),
            LotEntry {
                occupant: Some(51),
                config: None,
                config_id: 8,
                locked: false,
            },
        );
        s.potholes.insert(lot_key(9, 9), Some(80));
    });
    let (bytes, entries) = save(&*m);
    assert_eq!(tokens(&entries), vec![50, 51, 80]);

    // A fresh manager reads the record (not initialised), then Init resolves
    // configuration 7 and drops the lot of the missing configuration 8.
    let g = Rc::new(LFake::default());
    g.s.borrow_mut().configs = vec![cfg(7, 0x100, &[1])];
    let n = NetworkLotManager::new(g.clone(), 3, 0);
    let mut res = Resolver(entries);
    let mut r = ByteReader::with_resolver(&bytes, &mut res);
    assert!(n.read(&mut r));
    let (again, _) = save(&*n);
    assert_eq!(again, bytes);
    n.init();
    let s = n.snapshot();
    assert_eq!(s.lots.keys(), vec![lot_key(3, 4)]);
    assert_eq!(s.lots.get(&lot_key(3, 4)).unwrap().config.as_ref().unwrap().id, 7);
    // The dropped lot's reference is not released *[quirk]*.
    assert_eq!(g.refs(51), 1);
    drop(f);
}

// ---- §16 tests 18–21, 23 ----

#[test]
fn t18_choose_and_keep() {
    let (f, m, draws) = lot_manager(vec![cfg(1, 0x100, &[1]), cfg(2, 0x100, &[1]), cfg(3, 0x100, &[2])]);
    f.piece(50, street(0x100, (10, 10)));
    f.zone(9, 10, 1);
    m.update_roads_in_cell(10, 10);
    assert_eq!(*draws.borrow(), vec![2]);
    let lot = m.get_network_lot(10, 10).unwrap();
    assert_eq!(lot.config.as_ref().unwrap().id, 1);
    assert_eq!(lot.occupant, Some(50));
    assert_eq!(f.refs(50), 1);

    m.update_roads_in_cell(10, 10);
    assert_eq!(*draws.borrow(), vec![2]);
    assert_eq!(m.get_network_lot(10, 10), Some(lot));

    f.zone(9, 10, 2);
    m.update_roads_in_cell(10, 10);
    assert_eq!(f.s.borrow().prop_removals, vec![(10, 10)]);
    assert_eq!(f.s.borrow().flora_removals, vec![(10, 10)]);
    assert_eq!(*draws.borrow(), vec![2, 1]);
    assert_eq!(m.get_network_lot(10, 10).unwrap().config.unwrap().id, 3);
    assert_eq!(f.refs(50), 1);
}

#[test]
fn t19_forced_configuration() {
    let (f, m, draws) = lot_manager(vec![]);
    f.piece(60, street(0x200, (20, 20)));
    m.set_lot_configuration_for_next_piece(Some(Rc::new(cfg(9, 0x999, &[]))));
    m.update_roads_in_cell(20, 20);
    let lot = m.get_network_lot(20, 20).unwrap();
    assert_eq!(lot.config.as_ref().unwrap().id, 9);
    assert!(lot.locked);
    assert!(m.snapshot().forced.is_none());
    assert!(draws.borrow().is_empty());
    f.zone(19, 20, 1);
    m.update_roads_in_cell(20, 20);
    assert_eq!(m.get_network_lot(20, 20), Some(lot));
    assert!(f.s.borrow().prop_removals.is_empty());
}

#[test]
fn t20_road_style() {
    let (f, m, _) = lot_manager(vec![]);
    f.piece(70, street(0x300, (10, 10)));
    f.zone(9, 10, 1);
    f.s.borrow_mut().lots.insert((9, 10), 700);
    f.s.borrow_mut().lot_wealth.insert(700, 2);
    m.update_road_style(10, 10, 70);
    {
        let s = f.s.borrow();
        assert_eq!(s.textures, vec![(70, 0x0820_0000)]);
        assert_eq!(s.variations, vec![(70, 2)]);
        assert_eq!(s.flag_sets, vec![(70, flag::UNDER_TEXTURE)]);
    }

    let (f, m, _) = lot_manager(vec![]);
    f.piece(
        71,
        Piece {
            flags: flag::RAIL | flag::USABLE,
            cell: (30, 30),
            ..Piece::default()
        },
    );
    m.update_road_style(30, 30, 71);
    {
        let s = f.s.borrow();
        assert_eq!(s.textures, vec![(71, 0)]);
        assert!(s.flag_sets.is_empty());
    }

    let (f, m, _) = lot_manager(vec![]);
    f.piece(72, street(0x300, (40, 40)));
    f.zone(41, 40, 7);
    m.update_road_style(40, 40, 72);
    let s = f.s.borrow();
    assert_eq!(s.textures, vec![(72, 0x0840_0000)]);
    assert_eq!(s.variations, vec![(72, 7)]);
}

fn damage(x: u32, z: u32, on: u32) -> NetworkLotMessage {
    NetworkLotMessage {
        data1: x,
        data2: z,
        data3: on,
        ..NetworkLotMessage::of_type(msg::ROAD_DAMAGE_CHANGED)
    }
}

#[test]
fn t21_pothole() {
    let (f, m, _) = lot_manager(vec![]);
    f.piece(
        80,
        Piece {
            flags: flag::ROAD | flag::USABLE,
            cell: (10, 10),
            active: 1,
            ..Piece::default()
        },
    );
    f.piece(
        81,
        Piece {
            flags: flag::RAIL | flag::USABLE,
            cell: (20, 20),
            ..Piece::default()
        },
    );
    m.do_message(&damage(10, 10, 1));
    let p = m.snapshot().potholes.get(&lot_key(10, 10)).copied().flatten().unwrap();
    let e = f.s.borrow().effects[&p].clone();
    assert_eq!(e.name, POTHOLE_NAME);
    assert_eq!(e.pos, [164.0, 2.0, 168.0]);
    assert!(e.started && e.inserted);

    m.do_message(&damage(10, 10, 1));
    assert_eq!(f.s.borrow().effects.len(), 1);

    m.do_message(&damage(20, 20, 1));
    let q = m.snapshot().potholes.get(&lot_key(20, 20)).copied().flatten().unwrap();
    assert_eq!(f.s.borrow().effects[&q].name, POTHOLE_RAIL_NAME);

    m.do_message(&damage(10, 10, 0));
    let e = f.s.borrow().effects[&p].clone();
    assert!(e.stopped && e.removed);
    assert_eq!(f.refs(p), 0);
    assert!(!m.snapshot().potholes.contains_key(&lot_key(10, 10)));

    let before = f.s.borrow().effects.clone();
    m.do_message(&damage(11, 11, 0));
    assert_eq!(f.s.borrow().effects, before);

    // A cell without a network: the terrain altitude at the centre.
    m.do_message(&damage(30, 30, 1));
    let r = m.snapshot().potholes.get(&lot_key(30, 30)).copied().flatten().unwrap();
    assert_eq!(f.s.borrow().effects[&r].pos, [488.0, 1.0, 488.0]);
}

#[test]
fn t23_lot_state_ring() {
    let (f, m, _) = lot_manager(vec![]);
    f.s.borrow_mut().lot_rect.insert(600, rect(10, 20, 11, 22));
    m.do_message(&NetworkLotMessage {
        unknown: Some(600),
        ..NetworkLotMessage::of_type(msg::LOT_STATE_CHANGED)
    });
    // §16 test 23: 61 cells, row 0 from x = 0, columns 9 and 12 from z = 1,
    // the full bottom row; not (10, 19), (11, 19) nor the lot's cells.
    let queried: BTreeSet<(i32, i32)> = f.s.borrow().tile_queries.iter().copied().collect();
    let mut want = BTreeSet::new();
    for x in 0..=12 {
        want.insert((x, 0));
    }
    for z in 1..=22 {
        want.insert((9, z));
        want.insert((12, z));
    }
    for x in 9..=12 {
        want.insert((x, 23));
    }
    assert_eq!(want.len(), 61);
    assert_eq!(queried, want);
    assert!(!queried.contains(&(10, 19)) && !queried.contains(&(11, 19)));
    assert!(!queried.contains(&(10, 20)) && !queried.contains(&(11, 22)));
    assert!(m.snapshot().pending.is_empty());
    assert_eq!(f.refs(600), 0);

    // Directly: the walk lands in `pending`, clipped to the city.
    m.add_border_to_pending(rect(-1, -1, 1, 1));
    let pending: Vec<(i32, i32)> = m.snapshot().pending.iter().map(|&k| key_cell(k)).collect();
    assert_eq!(pending, vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
}

#[test]
fn t24_lot_state_walk_top_edge() {
    // §16 test 24: lot (5, 0), grown {4, -1, 6, 1}: row 0 in full from x = 0
    // (the lot's own cell included), then row 1 from x = 4.
    let walk = lot_state_walk(rect(4, -1, 6, 1));
    let want: Vec<(i32, i32)> = (0..=6).map(|x| (x, 0)).chain((4..=6).map(|x| (x, 1))).collect();
    assert_eq!(walk, want);

    let (f, m, _) = lot_manager(vec![]);
    f.s.borrow_mut().lot_rect.insert(600, rect(5, 0, 5, 0));
    m.do_message(&NetworkLotMessage {
        unknown: Some(600),
        ..NetworkLotMessage::of_type(msg::LOT_STATE_CHANGED)
    });
    let queried: BTreeSet<(i32, i32)> = f.s.borrow().tile_queries.iter().copied().collect();
    assert_eq!(queried, want.into_iter().collect());
}

#[test]
fn lot_state_walk_order_and_edges() {
    // The §6.3 example: lot 15..16 x 94..96, grown {14, 93, 17, 97}.
    let w = lot_state_walk(rect(14, 93, 17, 97));
    assert_eq!(w[..18], (0..=17).map(|x| (x, 0)).collect::<Vec<_>>()[..]);
    assert_eq!(w[18..22], [(14, 1), (17, 1), (14, 2), (17, 2)]);
    assert_eq!(
        w[w.len() - 6..],
        [(14, 96), (17, 96), (14, 97), (15, 97), (16, 97), (17, 97)]
    );
    assert_eq!(w.len(), 18 + 2 * 96 + 4);
    assert!(w.contains(&(14, 40)) && w.contains(&(17, 40)));
    assert!(!w.contains(&(15, 93)) && !w.contains(&(16, 93)));
    let set: BTreeSet<_> = w.iter().copied().collect();
    assert_eq!(set.len(), w.len());

    // One column (x1 = x0): step 1, rows 1.. get (x0, z) once.
    assert_eq!(
        lot_state_walk(rect(3, 1, 3, 3)),
        vec![(0, 0), (1, 0), (2, 0), (3, 0), (3, 1), (3, 2), (3, 3)]
    );
    // Rect left of x = 0: (0, 0) still first, row 0 stops at once.
    assert_eq!(lot_state_walk(rect(-3, 0, -2, 1)), vec![(0, 0), (-3, 1), (-2, 1)]);
    // z1 = 0: row 0 only; z1 = 1: row 1 is the full bottom row.
    assert_eq!(lot_state_walk(rect(1, -1, 2, 0)), vec![(0, 0), (1, 0), (2, 0)]);
    assert_eq!(
        lot_state_walk(rect(1, 0, 2, 1)),
        vec![(0, 0), (1, 0), (2, 0), (1, 1), (2, 1)]
    );
    // Reversed rect: x2 := max(x2, x1), z2 := max(z2, z1).
    assert_eq!(lot_state_walk(rect(2, 1, 0, 0)), vec![(0, 0), (1, 0), (2, 0), (2, 1)]);
}

// ---- lot objects, removal and messages ----

#[test]
fn lot_objects_props_and_flora() {
    let mut c = cfg(1, 0x100, &[0]);
    c.objects = vec![
        NetworkLotObject {
            kind: OBJECT_PROP,
            orientation: 1,
            offset: [2.0, 0.0, 3.0],
            ..NetworkLotObject::default()
        },
        NetworkLotObject {
            kind: OBJECT_PROP,
            flags: 1,
            ..NetworkLotObject::default()
        },
        NetworkLotObject {
            kind: OBJECT_FLORA,
            offset: [8.0, 0.0, 8.0],
            ..NetworkLotObject::default()
        },
    ];
    let (f, m, draws) = lot_manager(vec![c]);
    {
        let mut s = f.s.borrow_mut();
        s.object_ids.insert((1, 0), vec![0xAA, 0xBB]);
        s.object_ids.insert((1, 1), vec![0xCC]);
        s.object_ids.insert((1, 2), vec![0xDD, 0xEE, 0xFF]);
    }
    f.piece(50, street(0x100, (2, 3)));
    m.update_roads_in_cell(2, 3);
    // Choice of the configuration, the prop, then the flora.
    assert_eq!(*draws.borrow(), vec![1, 2, 3]);
    let s = f.s.borrow();
    // Rotation 0: x = x0 + 16 − ox, z = z0 + 16 − oz.
    assert_eq!(s.street_props, vec![(0xAA, [46.0, 2.0, 61.0], 1)]);
    assert_eq!(s.floras, vec![(0xDD, [40.0, 2.0, 56.0])]);
}

#[test]
fn usable_and_removed_messages() {
    let (f, m, _) = lot_manager(vec![cfg(1, 0x100, &[0])]);
    f.piece(50, street(0x100, (5, 5)));
    m.do_message(&NetworkLotMessage {
        unknown: Some(50),
        data1: 1,
        ..NetworkLotMessage::of_type(msg::USABLE_CHANGED)
    });
    assert!(m.get_network_lot(5, 5).is_some());
    assert_eq!(f.refs(50), 1);
    m.do_message(&NetworkLotMessage {
        void1: Some(50),
        ..NetworkLotMessage::of_type(msg::OCCUPANT_REMOVED)
    });
    assert!(m.get_network_lot(5, 5).is_none());
    assert_eq!(f.refs(50), 0);
    // A connection node is ignored.
    f.piece(
        51,
        Piece {
            flags: flag::STREET | flag::USABLE | flag::CONNECTION_NODE,
            ..street(0x100, (6, 6))
        },
    );
    m.do_message(&NetworkLotMessage {
        unknown: Some(51),
        data1: 1,
        ..NetworkLotMessage::of_type(msg::USABLE_CHANGED)
    });
    assert!(m.get_network_lot(6, 6).is_none());
}

#[test]
fn avenue_style_is_one_block() {
    let (f, m, _) = lot_manager(vec![]);
    // Three avenue cells in a row, joined east-west (side 0 W, side 2 E).
    for (t, x, edges) in [
        (90u64, 10, 0x0001_0000u32),
        (91, 11, 0x0001_0001),
        (92, 12, 0x0000_0001),
    ] {
        f.piece(
            t,
            Piece {
                flags: flag::AVENUE | flag::USABLE,
                cell: (x, 10),
                edges,
                ..Piece::default()
            },
        );
    }
    f.zone(10, 9, 1);
    f.zone(11, 9, 1);
    f.zone(13, 10, 8);
    m.update_roads_in_cell(11, 10);
    let s = f.s.borrow();
    let mut tex: Vec<(u64, u32)> = s.textures.clone();
    tex.sort();
    // Zone 1 (cells 10 and 11) wins over zone 8 (cell 12), wealth 0.
    let t = road_texture(1, 0);
    assert_eq!(tex, vec![(90, t), (91, t), (92, t)]);
    let mut var: Vec<(u64, u32)> = s.variations.clone();
    var.sort();
    assert_eq!(var, vec![(90, 1), (91, 1), (92, 1)]);
}

#[test]
fn shutdown_releases_lots_and_potholes() {
    let (f, m, _) = lot_manager(vec![cfg(1, 0x100, &[0])]);
    f.piece(50, street(0x100, (5, 5)));
    f.piece(
        80,
        Piece {
            flags: flag::ROAD,
            cell: (7, 7),
            ..Piece::default()
        },
    );
    m.update_roads_in_cell(5, 5);
    m.do_message(&damage(7, 7, 1));
    let p = m.snapshot().potholes.get(&lot_key(7, 7)).copied().flatten().unwrap();
    m.shutdown();
    assert_eq!(f.refs(50), 0);
    assert_eq!(f.refs(p), 0);
    assert!(f.s.borrow().effects[&p].stopped);
    let s = m.snapshot();
    assert!(s.lots.is_empty() && s.potholes.is_empty() && s.configs.is_empty());
}

#[test]
fn ordered_map_keeps_insertion_order() {
    let mut o: OrderedMap<u16, u32> = OrderedMap::default();
    o.insert(5, 1);
    o.insert(2, 2);
    o.insert(5, 3);
    assert_eq!(o.keys(), vec![5, 2]);
    assert_eq!(o.get(&5), Some(&3));
    for k in 100..200 {
        o.insert(k, 0);
    }
    for k in 100..190 {
        o.remove(&k);
    }
    assert_eq!(o.len(), 12);
    assert_eq!(o.keys()[..3], [5, 2, 190]);
}

// ---- §16 tests 25-28: raw generator replay and the pending hash order ----

fn seeded_0x9e3d9501() -> crate::rng::RzRandomSuper {
    let seeds = crate::rng::FixedSeeds {
        wall_clock: 1,
        process_clock: 1,
    };
    crate::rng::RzRandomSuper::new(0x9E3D_9501, &seeds)
}

#[test]
fn t25_raw_state_replay() {
    let mut g = seeded_0x9e3d9501();
    assert_eq!(
        (g.state_word(0), g.state_word(623), g.raw().1),
        (0x3508_F7E2, 0x06CD_7FA3, 0)
    );
    assert_eq!([g.uniform_u32(3), g.uniform_u32(4), g.uniform_u32(2)], [2, 2, 1]);
    // Raw load at k = 622 (the seed's words after 622 draws).
    let mut s = seeded_0x9e3d9501();
    for _ in 0..622 {
        s.next_u32();
    }
    let (w, k) = s.raw();
    assert_eq!(k, 622);
    let words = *w;
    let (f, m, _) = lot_manager(vec![]);
    let _ = f;
    m.load_random_raw(&words, 622);
    let mut r = crate::rng::RzRandomSuper::from_raw(&words, 622);
    assert_eq!(r.next_u32(), 0x902A_FD71);
    assert_eq!(r.next_u32(), 0xC2CB_90BE);
    assert_eq!(r.next_u32(), 0xB7E8_22C6);
    let mut r = crate::rng::RzRandomSuper::from_raw(&words, 622);
    assert_eq!([r.uniform_u32(3), r.uniform_u32(4), r.uniform_u32(2)], [1, 3, 1]);
    assert_eq!(r.raw().1, 1);
    // The post-state equals the seeded generator after 625 draws.
    let mut t = seeded_0x9e3d9501();
    for _ in 0..625 {
        t.next_u32();
    }
    assert_eq!(r.raw(), t.raw());
    // Through the manager: the raw state round-trips.
    let raw = m.random_raw().unwrap();
    assert_eq!((&*raw.words, raw.k), (&words, 622));
}

#[test]
fn t26_pending_order() {
    let mut p = PendingSet::default();
    assert_eq!(p.bucket_count(), 193);
    for k in [0x0102u16, 0x0201, 0x0143, 0x00C3] {
        p.insert(k);
    }
    let order: Vec<u16> = p.iter().copied().collect();
    assert_eq!(order, vec![0x00C3, 0x0102, 0x0201, 0x0143]);
    let mut p = PendingSet::default();
    p.insert(2);
    p.insert(195);
    assert_eq!(p.first(), Some(195));
    let mut p = PendingSet::default();
    for k in 0..193u16 {
        p.insert(k);
        assert_eq!(p.bucket_count(), 193);
    }
    p.insert(193);
    assert_eq!((p.bucket_count(), p.len()), (389, 194));
}

#[test]
fn t27_pending_rehash_order() {
    // The pure set, keys in the zoning handler's order (x fastest).
    let mut p = PendingSet::default();
    let mut n = 0;
    for z in 9..=22 {
        for x in 9..=22 {
            p.insert(lot_key(x, z));
            n += 1;
            if n == 193 {
                assert_eq!(p.bucket_count(), 193);
            }
            if n == 194 {
                assert_eq!((lot_key(x, z), p.bucket_count()), (0x1416, 389));
            }
        }
    }
    assert_eq!(p.len(), 196);
    let order: Vec<u16> = p.iter().copied().collect();
    assert_eq!(&order[..4], &[0x1409, 0x140A, 0x140B, 0x140C]);
    let in_bucket = |b: u16| order.iter().copied().filter(|k| k % 389 == b).collect::<Vec<u16>>();
    assert_eq!(in_bucket(359), vec![0x0C0A, 0x0F14]);
    assert_eq!(in_bucket(368), vec![0x0909, 0x0C13]);

    // Through the zoning handler: the tile queries follow that order.
    let (f, m, _) = lot_manager(vec![]);
    m.do_message(&NetworkLotMessage {
        rect: Some(rect(10, 10, 21, 21)),
        ..NetworkLotMessage::of_type(msg::ZONING_CHANGED)
    });
    let queried: Vec<u16> = f.s.borrow().tile_queries.iter().map(|&(x, z)| lot_key(x, z)).collect();
    assert_eq!(queried, order);
    assert_eq!((m.pending_bucket_count(), m.snapshot().pending.len()), (389, 0));
    // A following one-cell zoning change keeps B = 389: its 9 keys by key mod 389.
    f.s.borrow_mut().tile_queries.clear();
    m.do_message(&NetworkLotMessage {
        rect: Some(rect(30, 30, 30, 30)),
        ..NetworkLotMessage::of_type(msg::ZONING_CHANGED)
    });
    let queried: Vec<u16> = f.s.borrow().tile_queries.iter().map(|&(x, z)| lot_key(x, z)).collect();
    let mut want: Vec<u16> = (29..=31).flat_map(|z| (29..=31).map(move |x| lot_key(x, z))).collect();
    want.sort_by_key(|k| k % 389);
    assert_eq!(queried, want);
    assert_eq!(m.pending_bucket_count(), 389);
    // Rule 6: a captured B for the next pass, refused while keys are pending.
    assert!(m.set_pending_bucket_count(193));
    assert_eq!(m.pending_bucket_count(), 193);
    m.add_pending(1, 1);
    assert!(!m.set_pending_bucket_count(389));
}

#[test]
fn t28_duplicate_insert_resizes() {
    let mut p = PendingSet::default();
    for k in 0..193u16 {
        p.insert(k);
    }
    assert!(!p.insert(5));
    assert_eq!((p.bucket_count(), p.len()), (389, 193));
    assert!(p.contains(&5));
    // Erase never shrinks B.
    for k in 0..193u16 {
        assert!(p.remove(&k));
    }
    assert!(p.is_empty());
    assert_eq!(p.bucket_count(), 389);
    assert_eq!(next_prime(100), 193);
    assert_eq!(next_prime(390), 769);
}
