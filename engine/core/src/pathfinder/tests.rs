// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Unit tests for the pathfinder. Expected values are the ones
//! `docs/specs/pathfinder.md` states (section and §11 test numbers in the
//! comments), not values read back from the code. Where a test goes beyond
//! §11, the expected values are derived by hand from the cited section.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};

use super::*;
use crate::curve::ResponseCurve;
use crate::traffic_network::{
    net_type, travel, wormhole, CellConnection, FerryTerminal, NeighborLink, NeighbourEdge, NetworkState, TerminalLink,
    ARRAY_LEN,
};

// ---------------------------------------------------------------------------
// The stub traffic simulator and city (§11 preamble)
// ---------------------------------------------------------------------------

/// The trip capacities of a destination holder: (max[4], used[4]).
type Capacity = ([u32; 4], [u32; 4]);

#[derive(Default)]
struct World {
    cells: Cell<(i32, i32)>,
    tables: Cell<TravelTables>,
    max_distance: Cell<u32>,
    attractiveness: Cell<f32>,
    strike: Cell<u8>,
    trip_scale: Cell<f32>,
    carpool: Cell<bool>,
    volumes: RefCell<HashMap<(u32, i32, i32), f32>>,
    dest: RefCell<HashMap<(i32, i32), u8>>,
    free: Cell<[u32; 4]>,
    /// holder → (max[4], used[4]).
    caps: RefCell<HashMap<u64, Capacity>>,
    /// (lot token, bounding rect, holder) per lot.
    lots: RefCell<Vec<(u64, CellRect, u64)>>,
    pools: RefCell<HashMap<(i32, i32), u64>>,
    income: Cell<f64>,
    deadline_reached: Cell<bool>,
}

impl World {
    /// The §11 preamble: an 8 × 8 city, `affectedByTraffic[Car] =
    /// canReachDestination[Car] = generatesTraffic[Car] = true`,
    /// `incomePerTile[Car] = 0`, NearestDestinationAttractiveness 1.0,
    /// MaxDistanceOriginToNetwork 1, no strike, no carpool, volumes 0.
    fn preamble(w: i32, h: i32) -> World {
        let world = World::default();
        world.cells.set((w, h));
        let mut t = TravelTables::default();
        t.affected_by_traffic[travel::CAR] = true;
        t.can_reach_destination[travel::CAR] = true;
        t.generates_traffic[travel::CAR] = true;
        world.tables.set(t);
        world.max_distance.set(1);
        world.attractiveness.set(1.0);
        world.strike.set(2);
        world.trip_scale.set(1.0);
        world
    }

    fn add_lot(&self, token: u64, rect: CellRect, holder: u64) {
        self.lots.borrow_mut().push((token, rect, holder));
    }

    fn tables_mut(&self, f: impl FnOnce(&mut TravelTables)) {
        let mut t = self.tables.get();
        f(&mut t);
        self.tables.set(t);
    }
}

impl PathfinderHost for World {
    fn cell_counts(&self) -> (i32, i32) {
        self.cells.get()
    }
    fn work_deadline(&self, _ms: u32) -> u64 {
        0
    }
    fn work_deadline_reached(&self, _deadline: u64) -> bool {
        self.deadline_reached.get()
    }
    fn travel_tables(&self) -> TravelTables {
        self.tables.get()
    }
    fn max_distance_origin_to_network(&self) -> u32 {
        self.max_distance.get()
    }
    fn nearest_destination_attractiveness(&self) -> f32 {
        self.attractiveness.get()
    }
    fn strike_state(&self) -> u8 {
        self.strike.get()
    }
    fn trip_scale_for_display(&self) -> f32 {
        self.trip_scale.get()
    }
    fn volume(&self, n: u32, x: i32, z: i32) -> f32 {
        self.volumes.borrow().get(&(n, x, z)).copied().unwrap_or(0.0)
    }
    fn set_volume(&self, n: u32, x: i32, z: i32, v: f32) {
        self.volumes.borrow_mut().insert((n, x, z), v);
    }
    fn destination_byte(&self, x: i32, z: i32) -> u8 {
        self.dest.borrow().get(&(x, z)).copied().unwrap_or(0)
    }
    fn set_destination_byte(&self, x: i32, z: i32, bits: u8) {
        self.dest.borrow_mut().insert((x, z), bits);
    }
    fn free_destinations(&self, k: u32) -> u32 {
        self.free.get()[k as usize]
    }
    fn set_free_destinations(&self, k: u32, n: u32) {
        let mut f = self.free.get();
        f[k as usize] = n;
        self.free.set(f);
    }
    fn max_trip_capacity(&self, holder: u64, k: u32) -> u32 {
        self.caps.borrow().get(&holder).map_or(0, |c| c.0[k as usize])
    }
    fn used_trip_capacity(&self, holder: u64, k: u32) -> u32 {
        self.caps.borrow().get(&holder).map_or(0, |c| c.1[k as usize])
    }
    fn neighbor_pool_holder(&self, _t: u32, x: i32, z: i32) -> Option<u64> {
        self.pools.borrow().get(&(x, z)).copied()
    }
    fn income_accumulator(&self) -> f64 {
        self.income.get()
    }
    fn set_income_accumulator(&self, v: f64) {
        self.income.set(v);
    }
    fn lot_at(&self, x: i32, z: i32) -> Option<u64> {
        self.lots
            .borrow()
            .iter()
            .find(|(_, r, _)| rect_contains(*r, x, z))
            .map(|l| l.0)
    }
    fn lot_holder(&self, lot: u64) -> u64 {
        self.lots.borrow().iter().find(|l| l.0 == lot).map_or(0, |l| l.2)
    }
    fn lot_bounding_rect(&self, lot: u64) -> CellRect {
        self.lots
            .borrow()
            .iter()
            .find(|l| l.0 == lot)
            .map_or(CellRect::new(0, 0, -1, -1), |l| l.1)
    }
    fn occupants_in_cells(&self, _rect: CellRect) -> Vec<u64> {
        Vec::new()
    }
    fn occupant_filter_includes(&self, _filter: u64, _occupant: u64) -> bool {
        false
    }
    fn is_ordinance_on(&self, id: u32) -> bool {
        id == CARPOOL_ORDINANCE && self.carpool.get()
    }
}

/// An empty network of `w × h` cells: every cell connection record at the
/// Init values, no network info, no switches.
fn empty_net(w: i32, h: i32) -> NetworkState {
    NetworkState {
        count_x: w,
        count_z: h,
        conn: vec![CellConnection::default(); ARRAY_LEN],
        info: vec![Vec::new(); (w * h) as usize],
        ..NetworkState::default()
    }
}

/// A street cell of the §11 preamble: road-network info `{invCapacity
/// 0.0625, type 3 (Street), extraCost 0.0}`, `conn[Car] = 0x0104`,
/// `switch[Car] = 0xFFFF`, `priorCar[0…3] = 0xFFFF`, wormhole 0.
fn street_cell(net: &mut NetworkState, x: i32, z: i32) {
    let c = net.conn_mut(x, z).unwrap();
    c.conn[travel::CAR] = 0x0104;
    c.switch[travel::CAR] = 0xFFFF;
    c.prior_car = [0xFFFF; 4];
    c.wormhole = 0;
    let r = net.append_info(1, x, z).unwrap();
    r.inv_cap = 0.0625;
    r.net_type = net_type::STREET as u8;
    r.extra_cost = 0.0;
}

/// The §11 tests 2–7 city: 8 × 8, the street (0,3)…(4,3),
/// `invMaxSpeed[Car][Street] = 1.5`, `Curve(0.0) = 1.0`, a lot at (2,2).
fn street_city() -> (World, NetworkState) {
    let w = World::preamble(8, 8);
    w.add_lot(1, CellRect::new(2, 2, 2, 2), 101);
    let mut net = empty_net(8, 8);
    net.inv_max_speed[travel::CAR][net_type::STREET as usize] = 1.5;
    net.congestion_curve = ResponseCurve::from_points(&[(0.0, 1.0)]);
    for x in 0..=4 {
        street_cell(&mut net, x, 3);
    }
    (w, net)
}

/// The §11 test 2 configuration: start rect (2,2,2,2); end rect (4,4,4,4);
/// filter 3; `SetTravelTypeFlag(0x2, true)` only; starting cost array all
/// 0.0; route deque set; density 10; flags 1.
fn test2_pathfinder(w: &World) -> Pathfinder {
    let mut pf = Pathfinder::create(w, shared_work_check());
    pf.set_flag(flag::NO_BUDGET);
    pf.set_start_rect(CellRect::new(2, 2, 2, 2));
    pf.set_end_rect(CellRect::new(4, 4, 4, 4));
    pf.set_trip_destination_filter(filter::END_RECT);
    pf.clear_all_travel_type_flags();
    pf.set_travel_type_flag(0x2, true);
    pf.set_starting_cost_array(Some([0.0; 9]));
    pf.set_route_info(Some(VecDeque::new()));
    pf.set_traffic_density(10);
    pf
}

fn find(pf: &mut Pathfinder, w: &World, net: &mut NetworkState) -> u32 {
    pf.find_path(&mut PathCx { net, host: w })
}

fn route_of(pf: &Pathfinder) -> Vec<(u8, u8, u8)> {
    pf.route().unwrap().iter().map(|r| (r.x, r.z, r.travel_type)).collect()
}

fn start_key(side: u32, t: u32, x: i32, z: i32) -> u32 {
    ((0x10 | side) << 24) | (t << 16) | ((x as u32) << 8) | z as u32
}

// ---------------------------------------------------------------------------
// §11 conformance tests
// ---------------------------------------------------------------------------

/// §11 test 1: push f = a 5, b 3, c 3, d 1, e 3; the pop order is d, e, b,
/// c, a (§4.7: equal f is not FIFO).
#[test]
fn t01_open_set_tie_order() {
    let (a, b, c, d, e) = (0, 1, 2, 3, 4);
    assert_eq!(heap_order(&[5.0, 3.0, 3.0, 1.0, 3.0]), vec![d, e, b, c, a]);
}

/// §11 test 2: the straight street.
#[test]
fn t02_straight_street() {
    let (w, mut net) = street_city();
    // The ring-1 cells visited are (2,1), (1,2), (3,2), (2,3) (§4.6).
    let ring = crate::traffic_network::grow_clamp(CellRect::new(2, 2, 2, 2), 1, 8, 8);
    assert_eq!(
        start_ring_cells(CellRect::new(2, 2, 2, 2), ring),
        vec![(2, 1), (1, 2), (3, 2), (2, 3)]
    );
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    // Start nodes: only (2,3), sides 0 and 2, g = f = 0, pushed in that
    // order; then the adds (3,3) side 0 g 1.5, (1,3) side 2 g 1.5, (4,3)
    // side 0 g 3.0, (0,3) side 2 g 3.0, in the order the pops make them.
    let nodes = pf.nodes().nodes();
    let got: Vec<(i32, i32, u8, u32)> = nodes.iter().map(|n| (n.x, n.z, n.side, n.g.to_bits())).collect();
    assert_eq!(
        got,
        vec![
            (2, 3, 0, 0.0f32.to_bits()),
            (2, 3, 2, 0.0f32.to_bits()),
            (3, 3, 0, 1.5f32.to_bits()),
            (1, 3, 2, 1.5f32.to_bits()),
            (4, 3, 0, 3.0f32.to_bits()),
            (0, 3, 2, 3.0f32.to_bits()),
        ]
    );
    assert_eq!(nodes[0].key, start_key(0, 1, 2, 3));
    assert_eq!(nodes[1].key, start_key(2, 1, 2, 3));
    assert_eq!((nodes[0].f, nodes[1].f), (0.0, 0.0));
    // Popped and closed: both starts, (3,3), (1,3); (4,3) is the goal.
    let closed: Vec<bool> = nodes.iter().map(|n| n.closed).collect();
    assert_eq!(closed, vec![true, true, true, true, false, false]);
    assert_eq!(pf.result_node().map(|n| (n.x, n.z)), Some((4, 3)));
    // The results.
    assert_eq!(pf.get_trip_length().to_bits(), 0x4040_0000);
    assert_eq!(pf.get_end_cell(), (4, 4));
    assert_eq!(pf.get_traffic_arrived(), 10);
    assert_eq!(pf.get_last_travel_type(), 1);
    assert_eq!(route_of(&pf), vec![(2, 3, 1), (3, 3, 1), (4, 3, 1)]);
    assert!(pf.was_trip_reversible(&net));
}

/// §11 test 3: the compressed route of test 2.
#[test]
fn t03_compressed_route() {
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    let mut out = vec![0xEE; 3];
    let mask = pf.get_compressed_route(&net, &mut out);
    assert_eq!(out, vec![0x01, 0x02, 0x03, 0x41, 0x02, 0x0A]);
    assert_eq!(mask, 0x0000_0002);
}

/// The road (n = 1) edge-density counters of a cell, `current[4·e + o]`.
fn counters(net: &NetworkState, x: i32, z: i32, evening: bool) -> [u16; 16] {
    net.edge_density_at(x, z, 1, evening).current
}

fn one_counter(entry: usize, exit: usize, v: u16) -> [u16; 16] {
    let mut c = [0u16; 16];
    c[4 * entry + exit] = v;
    c
}

/// §11 test 4: ApplyTraffic(0), style 2 and the carpool ordinance.
#[test]
fn t04_apply_traffic() {
    for (style, carpool, vol, count) in [(0u32, false, 10.0f32, 10u16), (2, false, 20.0, 10), (0, true, 8.0, 8)] {
        let (w, mut net) = street_city();
        w.carpool.set(carpool);
        let mut pf = test2_pathfinder(&w);
        assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
        pf.apply_traffic(
            &mut PathCx {
                net: &mut net,
                host: &w,
            },
            style,
        );
        for x in 2..=4 {
            assert_eq!(
                w.volume(1, x, 3).to_bits(),
                vol.to_bits(),
                "volume ({x},3) style {style}"
            );
        }
        // Edge density, all in `current`, style bit 0, type 1: (4,3)
        // current[4·0 + 3], (3,3) current[4·0 + 2], (2,3) current[4·1 + 2]
        // (root: entry from the start rect = side 1).
        let want = [(4, (0, 3)), (3, (0, 2)), (2, (1, 2))];
        for (x, (entry, exit)) in want {
            assert_eq!(
                counters(&net, x, 3, false),
                one_counter(entry, exit, count),
                "({x},3) style {style}"
            );
            // Style 2: the key-bit-1 record gets the mirrored index
            // current[4·exit + entry] (PF2); otherwise no key-bit-1 record.
            let other = if style == 2 {
                one_counter(exit, entry, count)
            } else {
                [0; 16]
            };
            assert_eq!(counters(&net, x, 3, true), other, "({x},3) evening style {style}");
            let rec = net.edge_density_at(x, 3, 1, false);
            assert_eq!((rec.last, rec.usage), ([0; 16], 0), "last and usage stay 0");
        }
    }
}

/// §11 test 5: the starting cost array `[0, 0.4f, 0, …]`.
#[test]
fn t05_starting_cost() {
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    let mut costs = [0.0f32; 9];
    costs[1] = 0.4;
    pf.set_starting_cost_array(Some(costs));
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    let g = |x: i32| {
        pf.nodes()
            .nodes()
            .iter()
            .find(|n| (n.x, n.z) == (x, 3) && n.side == 0)
            .unwrap()
            .g
            .to_bits()
    };
    assert_eq!(g(2), 0x3ECC_CCCD); // start g = 0.4f
    assert_eq!(g(3), 0x3FF3_3333); // 1.9f
    assert_eq!(g(4), 0x4059_999A); // 3.4000001f
    assert_eq!(pf.get_trip_length().to_bits(), 0x4040_0000); // r24(3.4000001f − 0.4f) = 3.0
}

/// §11 test 6: the budget and the shared trip length.
#[test]
fn t06_budget_and_shared_trip_length() {
    // Without flag 1, SetMaxTripLength(2.0): the g = 3.0 nodes are rejected.
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    pf.clear_flag(flag::NO_BUDGET);
    pf.set_max_trip_length(2.0);
    assert_eq!(find(&mut pf, &w, &mut net), result::NOT_FOUND);
    assert_eq!(pf.over_budget()[travel::CAR], 2);
    assert_eq!(pf.get_trip_length().to_bits(), 2.0f32.to_bits());

    // With flag 2: the fallback to the stored node nearest the end rect,
    // (3,3) (√2 against √5, √5, √10).
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    pf.clear_flag(flag::NO_BUDGET);
    pf.set_flag(flag::NEAREST_ON_FAILURE);
    pf.set_max_trip_length(2.0);
    let end_before = pf.get_end_cell();
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    assert_eq!(pf.result_node().map(|n| (n.x, n.z)), Some((3, 3)));
    assert_eq!(pf.get_trip_length().to_bits(), 1.5f32.to_bits());
    assert_eq!(route_of(&pf), vec![(2, 3, 1), (3, 3, 1)]);
    assert_eq!(pf.get_traffic_arrived(), 0);
    assert_eq!(pf.get_end_cell(), end_before);

    // After test 2 (success), a second FindPath without SetMaxTripLength
    // uses maxCost = 3.0.
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    find(&mut pf, &w, &mut net);
    assert_eq!(pf.max_cost()[travel::CAR].to_bits(), 3.0f32.to_bits());
}

/// §11 test 7: the 65th over-budget rejection of a type disables it for the
/// call; later moves of that type are ignored even within budget. The 65
/// rejections come from the terminal links of a ferry terminal at the start
/// cell (§4.8.3 step 4), each `g = 100 > maxCost = 10`; the start cell is
/// made one-way (`conn[Car] = 0x0004`) so only one start node expands it.
#[test]
fn t07_over_budget_disable() {
    for links in [64usize, 65] {
        let (w, mut net) = street_city();
        net.conn_mut(2, 3).unwrap().conn[travel::CAR] = 0x0004;
        net.conn_mut(2, 3).unwrap().wormhole |= wormhole::FERRY;
        net.ferries.push(FerryTerminal {
            dep_x: 2,
            dep_z: 3,
            mask: 3,
            links: (0..links)
                .map(|_| TerminalLink {
                    arrival_x: 7,
                    arrival_z: 7,
                    cost: 100.0,
                    ..TerminalLink::default()
                })
                .collect(),
            ..FerryTerminal::default()
        });
        net.ferry_departures.entry((2 << 8) | 3, || 0);
        let mut pf = test2_pathfinder(&w);
        pf.clear_flag(flag::NO_BUDGET);
        pf.set_max_trip_length(10.0);
        let r = find(&mut pf, &w, &mut net);
        assert_eq!(pf.over_budget()[travel::CAR] as usize, links);
        if links == 64 {
            assert_eq!(r, result::FOUND);
            assert_eq!(pf.disabled_travel_types(), 0);
        } else {
            // (4,3) at g = 3.0 ≤ 10 is never created.
            assert_eq!(r, result::NOT_FOUND);
            assert_eq!(pf.disabled_travel_types(), 1 << travel::CAR);
            assert!(!pf.nodes().nodes().iter().any(|n| (n.x, n.z) == (4, 3)));
            assert_eq!(pf.nodes().len(), 2);
        }
    }
}

/// §11 test 8: the boundary order.
#[test]
fn t08_boundary_order() {
    assert_eq!(
        crate::traffic_network::boundary_cells(CellRect::new(1, 1, 3, 3)),
        vec![(1, 1), (2, 1), (3, 1), (1, 2), (3, 2), (1, 3), (2, 3), (3, 3)]
    );
    assert_eq!(
        crate::traffic_network::boundary_cells(CellRect::new(4, 0, 4, 2)),
        vec![(4, 0), (4, 1), (4, 2)]
    );
}

/// §11 test 9: the standard destination order.
#[test]
fn t09_standard_destination_order() {
    let w = World::preamble(16, 16);
    let mut net = empty_net(16, 16);
    w.free.set([1, 0, 0, 0]);
    for (i, (x, z)) in [(4, 6), (6, 4)].into_iter().enumerate() {
        w.add_lot(10 + i as u64, CellRect::new(x, z, x, z), 100 + i as u64);
        w.set_destination_byte(x, z, 1);
    }
    let mut pf = Pathfinder::create(&w, shared_work_check());
    pf.set_start_rect(CellRect::new(5, 5, 5, 5));
    pf.set_trip_destination_filter(filter::STANDARD);
    pf.set_destination_flag(1);
    // No network: the search itself finds no start node, but the end rect
    // has been chosen (and the caller's overwritten).
    assert_eq!(find(&mut pf, &w, &mut net), result::NOT_FOUND);
    assert_eq!(pf.end_rect(), CellRect::new(4, 6, 4, 6));
}

/// §11 test 10: the heuristic.
#[test]
fn t10_heuristic() {
    let w = World::preamble(16, 16);
    w.attractiveness.set(f32::from_bits(0x3DB8_51EC)); // 0.09f
    let mut net = empty_net(16, 16);
    let mut pf = Pathfinder::create(&w, shared_work_check());
    pf.set_start_rect(CellRect::new(2, 2, 2, 2));
    pf.set_end_rect(CellRect::new(12, 3, 12, 3));
    pf.set_trip_destination_filter(filter::END_RECT);
    find(&mut pf, &w, &mut net);
    assert_eq!(pf.search_rect(), CellRect::new(-2, -2, 6, 6));
    assert_eq!(pf.heuristic(10, 3), 2.0);
    assert_eq!(pf.score(10, 3, 0.0).to_bits(), 0x3E38_51EC); // r24(2 × 0.09f)
    assert_eq!(pf.heuristic(6, 3), 0.0);
}

fn nodes_of(chain: &[(i32, i32, u8)]) -> Vec<RouteNode> {
    chain.iter().map(|&(x, z, t)| RouteNode { x, z, t }).collect()
}

/// §11 test 11: the compressed route with jumps.
#[test]
fn t11_compressed_route_with_jumps() {
    let net = empty_net(16, 16);
    let chain = nodes_of(&[
        (5, 5, 0),
        (6, 5, 0),
        (6, 5, 6),
        (6, 6, 6),
        (6, 7, 6),
        (6, 9, 6),
        (7, 9, 6),
    ]);
    let (bytes, mask) = encode_route(&chain, &net);
    assert_eq!(
        bytes,
        vec![
            0x00, 0x05, 0x05, 0x40, 0x01, 0x02, 0x06, 0x06, 0x05, 0x46, 0x02, 0x0F, 0x06, 0x06, 0x09, 0x46, 0x01, 0x02
        ]
    );
    assert_eq!(mask, 0x41);
}

/// §11 test 12: the run length limit (300 unit steps of type 1, code 2).
#[test]
fn t12_run_length_limit() {
    let net = NetworkState {
        count_x: 1024,
        count_z: 16,
        ..NetworkState::default()
    };
    let chain: Vec<RouteNode> = (0..=300).map(|x| RouteNode { x, z: 0, t: 1 }).collect();
    let (bytes, _) = encode_route(&chain, &net);
    let mut want = vec![0x01, 0x00, 0x00, 0x41, 0xFF];
    want.extend(std::iter::repeat(0xAA).take(63));
    want.push(0x2A);
    want.extend([0x41, 0x2D]);
    want.extend(std::iter::repeat(0xAA).take(11));
    want.push(0x02);
    assert_eq!(bytes, want);
}

/// The test-13 long street: a 32 × 4 city, a street along z = 1, the start
/// lot at (15,0), filter 1 (never matches): the search expands both start
/// nodes, 16 cells east and 15 west, and the two exit nodes, 35 nodes.
fn long_street() -> (World, NetworkState) {
    let w = World::preamble(32, 4);
    w.add_lot(1, CellRect::new(15, 0, 15, 0), 101);
    let mut net = empty_net(32, 4);
    net.inv_max_speed[travel::CAR][net_type::STREET as usize] = 1.5;
    net.congestion_curve = ResponseCurve::from_points(&[(0.0, 1.0)]);
    for x in 0..32 {
        street_cell(&mut net, x, 1);
    }
    (w, net)
}

/// §11 test 13: the check-interval adaptation (§4.11).
#[test]
fn t13_check_interval_adaptation() {
    let shared = shared_work_check();
    let max_work_time = 1_000_000;
    // 16 calls expanding at least 21 nodes each (m = 1024, k ≥ 21).
    let (lw, mut lnet) = long_street();
    for i in 1..=16 {
        let mut pf = Pathfinder::create(&lw, shared.clone());
        pf.set_flag(flag::NO_BUDGET);
        pf.set_start_rect(CellRect::new(15, 0, 15, 0));
        pf.set_trip_destination_filter(filter::NEVER);
        pf.set_max_work_time(max_work_time);
        assert_eq!(find(&mut pf, &lw, &mut lnet), result::NOT_FOUND);
        assert!(pf.work_counter() >> 10 >= 21);
        let wc = shared.get();
        if i < 16 {
            assert_eq!((wc.multiplier, wc.adaptation), (1024, i));
        } else {
            assert_eq!((wc.multiplier, wc.adaptation), (512, 0));
        }
    }
    let (w, mut net) = street_city();
    // A call expanding 0 nodes (the first start node is a goal): k = 0.
    let mut pf = test2_pathfinder(&w);
    pf.set_max_work_time(max_work_time);
    pf.set_end_rect(CellRect::new(2, 3, 2, 3));
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    assert_eq!(pf.work_counter(), 0);
    assert_eq!((shared.get().multiplier, shared.get().adaptation), (512, 0));
    // A call expanding 1 node with m = 512: k = 0. The street is made one
    // way at (2,3) so (2,3) side 0 is the only start node; its expansion
    // adds (3,3), whose neighbour (4,3) is the end rect.
    {
        let (w1, mut net1) = street_city();
        net1.conn_mut(2, 3).unwrap().conn[travel::CAR] = 0x0004;
        let mut pf = Pathfinder::create(&w1, shared.clone());
        pf.set_flag(flag::NO_BUDGET);
        pf.set_start_rect(CellRect::new(2, 2, 2, 2));
        pf.set_end_rect(CellRect::new(4, 3, 4, 3));
        pf.set_max_work_time(max_work_time);
        assert_eq!(find(&mut pf, &w1, &mut net1), result::FOUND);
        assert_eq!(pf.work_counter(), 512);
        assert_eq!((shared.get().multiplier, shared.get().adaptation), (512, 0));
    }
    // Four calls expanding 2–9 nodes (m = 512, 1 ≤ k ≤ 4): test 2 expands
    // 4 nodes before its goal.
    for i in 1..=4 {
        let mut pf = Pathfinder::create(&w, shared.clone());
        pf.set_flag(flag::NO_BUDGET);
        pf.set_start_rect(CellRect::new(2, 2, 2, 2));
        pf.set_end_rect(CellRect::new(4, 4, 4, 4));
        pf.set_max_work_time(max_work_time);
        assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
        assert_eq!(pf.work_counter(), 4 * 512);
        let wc = shared.get();
        if i < 4 {
            assert_eq!((wc.multiplier, wc.adaptation), (512, -i));
        } else {
            assert_eq!((wc.multiplier, wc.adaptation), (1024, 0));
        }
    }
}

/// §11 test 14: lifetime (§1).
#[test]
fn t14_lifetime() {
    let w = World::preamble(8, 8);
    let mut pf = Pathfinder::create(&w, shared_work_check());
    assert_eq!(pf.ref_count(), 1);
    assert!(pf.is_initialised());
    assert_eq!(pf.city_size(), (8, 8));
    assert_eq!(pf.release(), 0);
    assert!(pf.is_destroyed());
    assert!(!pf.is_initialised()); // Shutdown ran first

    let mut pf = Pathfinder::create(&w, shared_work_check());
    assert_eq!(pf.add_ref(), 2);
    assert_eq!(pf.release(), 1);
    assert!(!pf.is_destroyed());
    assert!(pf.is_initialised());
}

// ---------------------------------------------------------------------------
// Further behaviour (§1–§4), values derived by hand from the cited sections
// ---------------------------------------------------------------------------

/// §1: the constructor values and the interface identity.
#[test]
fn constructor_values() {
    let mut pf = Pathfinder::new(shared_work_check());
    assert_eq!(pf.ref_count(), 0);
    assert!(!pf.is_initialised());
    assert_eq!(pf.weight(), 1.0);
    assert_eq!(pf.get_trip_length(), 0.0);
    assert_eq!((pf.allowed_travel_types(), pf.starting_travel_types()), (2, 2));
    assert_eq!(pf.trip_destination_filter(), 3);
    assert_eq!(pf.destination_flags(), 0);
    assert_eq!(pf.flags(), 0);
    assert!(pf.route().is_none());
    assert!(pf.starting_cost_array().is_none());
    assert_eq!(pf.max_work_time(), 0);
    assert_eq!(pf.get_last_travel_type(), 0);
    assert!(pf.query_interface(IID_PATH_FINDER));
    assert!(pf.query_interface(1));
    assert!(!pf.query_interface(0x1234));
    assert_eq!(pf.ref_count(), 2);
    // Init twice: the second returns false; Shutdown twice likewise.
    let w = World::preamble(8, 8);
    assert!(pf.init(&w));
    assert!(!pf.init(&w));
    assert_eq!(pf.distance_tables().0.len(), 10);
    assert!(pf.shutdown());
    assert!(!pf.shutdown());
}

/// §4.2: the travel-type and destination-flag slots.
#[test]
fn configuration_slots() {
    let mut pf = Pathfinder::new(shared_work_check());
    pf.set_travel_type_flag(0x30, false);
    assert_eq!((pf.allowed_travel_types(), pf.starting_travel_types()), (0x32, 0x02));
    pf.set_travel_type_flag(0x01, true);
    assert_eq!((pf.allowed_travel_types(), pf.starting_travel_types()), (0x33, 0x03));
    pf.clear_travel_type_flag(0x03);
    assert_eq!((pf.allowed_travel_types(), pf.starting_travel_types()), (0x30, 0x00));
    pf.clear_all_travel_type_flags();
    assert_eq!((pf.allowed_travel_types(), pf.starting_travel_types()), (0, 0));
    pf.set_destination_flag(0x5);
    pf.clear_destination_flag(0x1);
    assert_eq!(pf.destination_flags(), 0x4);
    pf.set_flag(0x9);
    pf.clear_flag(0x1);
    assert_eq!(pf.flags(), 0x8);
}

/// §4.3: the getters after a failure; §4.4 step 7: a zero starting mask
/// returns 0.
#[test]
fn starting_mask_zero_returns_not_found() {
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    pf.clear_all_travel_type_flags();
    pf.set_max_trip_length(7.0);
    assert_eq!(find(&mut pf, &w, &mut net), result::NOT_FOUND);
    assert_eq!(pf.get_last_travel_type(), 0);
    assert_eq!(pf.get_trip_length(), 7.0);
    assert!(pf.was_trip_reversible(&net));
    let mut out = vec![1, 2, 3];
    assert_eq!(pf.get_compressed_route(&net, &mut out), 0);
    assert!(out.is_empty());
}

/// §4.8.1: the goal node is pushed back onto the open set; §4.10.2 an off-map
/// node inside the end rect is a goal.
#[test]
fn goal_node_pushed_back() {
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    let res = pf.result_node().copied().unwrap();
    assert!(res.open);
    let in_heap: Vec<(i32, i32)> = pf
        .open_set()
        .entries()
        .iter()
        .map(|&id| {
            let n = pf.nodes().get(id);
            (n.x, n.z)
        })
        .collect();
    assert!(in_heap.contains(&(4, 3)));
}

/// §4.8.2 (a): leaving the map makes an exit node whose step cost is the
/// node's own heuristic term `r24(c.f − c.g)`; §4.10.2 an off-map node in the
/// end rect is a goal; §4.13 step 3 adds the neighbour connection cost for
/// filter 3 with the destination type reached ≠ 3 (0 on a fresh object, §10
/// Q1).
#[test]
fn exit_node_cost_and_connection_cost() {
    let (w, mut net) = street_city();
    w.attractiveness.set(0.5);
    // The edge record of (−1,3) (traffic-network.md §2.7.1): cost 0.25.
    net.set_neighbour_edge(
        -1,
        3,
        NeighbourEdge {
            mask: 1 << 2,
            pool: 0,
            cost: 0.25,
        },
    );
    let mut pf = test2_pathfinder(&w);
    // Start at (4,2) (no lot): ring 1 reaches the street at (4,3).
    pf.set_start_rect(CellRect::new(4, 2, 4, 2));
    pf.set_end_rect(CellRect::new(-1, 3, -1, 3));
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    // Search rect (0,−2)-(8,6): every street cell is inside, so H = 0 and
    // the exit cost at (0,3) is r24(f − g) = 0. g(0,3) = 4 × 1.5 = 6.0.
    let res = pf.result_node().copied().unwrap();
    assert_eq!((res.x, res.z), (-1, 3));
    assert_eq!(res.key & EXIT_KEY_BIT, EXIT_KEY_BIT);
    assert_eq!(res.g, 6.0);
    assert_eq!(pf.get_end_cell(), (-1, 3));
    // L = r24(6.0 − 0.0) = 6.0, plus the connection cost 0.25.
    assert_eq!(pf.get_trip_length(), 6.25);
    assert_eq!(
        route_of(&pf),
        vec![(4, 3, 1), (3, 3, 1), (2, 3, 1), (1, 3, 1), (0, 3, 1), (255, 3, 1)]
    );
    // The compressed route writes the off-map node as its connection's
    // in-city cell, but it is a unit step: header 01 04 03, then five −x
    // steps (code 0) of type 1.
    let mut out = Vec::new();
    pf.get_compressed_route(&net, &mut out);
    assert_eq!(out, vec![0x01, 0x04, 0x03, 0x41, 0x05, 0x00, 0x00]);
    // §4.16: an off-map node resets e.
    assert!(pf.was_trip_reversible(&net));
}

/// §4.8.2 (a): with the node outside the search rect, the exit cost is the
/// heuristic term `r24(r24(H × w))`.
#[test]
fn exit_cost_is_the_heuristic_term() {
    let w = World::preamble(16, 8);
    w.attractiveness.set(0.5);
    w.add_lot(1, CellRect::new(12, 2, 12, 2), 101);
    let mut net = empty_net(16, 8);
    net.inv_max_speed[travel::CAR][net_type::STREET as usize] = 1.5;
    net.congestion_curve = ResponseCurve::from_points(&[(0.0, 1.0)]);
    for x in 0..=12 {
        street_cell(&mut net, x, 3);
    }
    let mut pf = test2_pathfinder(&w);
    pf.set_start_rect(CellRect::new(12, 2, 12, 2));
    pf.set_end_rect(CellRect::new(-1, 3, -1, 3));
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    // Search rect (8,−2)-(16,6). (0,3) is outside: H = dX[0] + dZ[3] =
    // (−1 → 0: 1) + 0 = 1; g(0,3) = 12 × 1.5 = 18.0; f = r24(1 × 0.5) +
    // 18.0 = 18.5; exit cost = r24(18.5 − 18.0) = 0.5; exit g = 18.5.
    let res = pf.result_node().copied().unwrap();
    assert_eq!((res.x, res.z), (-1, 3));
    assert_eq!(res.g, 18.5);
}

/// A neighbour link of a ferry terminal (traffic-network.md §2.6): the
/// off-map cell, its in-city edge cell and the cost.
fn neighbour_link(off: (i16, i16), edge: (u16, u16), cost: f32) -> NeighborLink {
    NeighborLink {
        off_x: off.0,
        off_z: off.1,
        edge_x: edge.0,
        edge_z: edge.1,
        cost,
        ..NeighborLink::default()
    }
}

/// A ferry terminal arriving at `arrival` with the given neighbour links;
/// its departure cell (6,6) is off the street.
fn fallback_terminal(arrival: (u16, u16), mask: u16, links: Vec<NeighborLink>) -> FerryTerminal {
    FerryTerminal {
        dep_x: 6,
        dep_z: 6,
        arr_x: arrival.0,
        arr_z: arrival.1,
        mask,
        neighbour_links: links,
        ..FerryTerminal::default()
    }
}

/// §4.6 (PF5): the off-map start fallback runs over the ferry terminals and
/// their neighbour links; the node sits at the terminal's arrival cell.
#[test]
fn off_map_start_fallback() {
    let (w, mut net) = street_city();
    net.ferries.push(fallback_terminal(
        (0, 3),
        0x2,
        vec![
            neighbour_link((-1, 7), (0, 7), 9.0),
            neighbour_link((-1, 5), (0, 5), 2.0),
        ],
    ));
    let mut pf = test2_pathfinder(&w);
    pf.set_start_rect(CellRect::new(-1, 5, -1, 5));
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    let root = pf.nodes().nodes()[0];
    assert_eq!((root.x, root.z, root.side, root.g), (0, 3, 0, 2.0));
    assert_eq!(root.key, start_key(0, 1, 0, 3));
    // Search rect (−5,1)-(3,9); (4,3) outside: H = 1 with w = 1.0. g: 3.5,
    // 5.0, 6.5, 8.0 along the street; L = r24(8.0 − 2.0) = 6.0.
    assert_eq!(pf.get_trip_length(), 6.0);
    assert_eq!(
        route_of(&pf),
        vec![(0, 3, 1), (1, 3, 1), (2, 3, 1), (3, 3, 1), (4, 3, 1)]
    );
}

/// §4.6 (PF5): with two matching links the last match wins (its `g`, even
/// when higher) and the node is pushed once per match; a terminal whose mask
/// lacks the starting type is skipped; a later terminal's match overwrites
/// an earlier one's.
#[test]
fn off_map_start_fallback_last_match_wins() {
    for (costs, want_g) in [((0.5f32, 0.25f32), 0.25f32), ((0.25, 0.5), 0.5)] {
        let (w, mut net) = street_city();
        net.ferries.push(fallback_terminal(
            (0, 3),
            0x2,
            vec![
                neighbour_link((-1, 5), (0, 5), costs.0),
                neighbour_link((-1, 5), (0, 5), costs.1),
            ],
        ));
        // Mask 0x1 (Walk only): not a starting type, skipped.
        net.ferries.push(fallback_terminal(
            (1, 3),
            0x1,
            vec![neighbour_link((-1, 5), (0, 5), 0.0)],
        ));
        let mut pf = test2_pathfinder(&w);
        pf.set_start_rect(CellRect::new(-1, 5, -1, 5));
        // The end rect is the arrival cell: the first pop is the goal and is
        // pushed back (§4.8.1), beside the second copy of the start push.
        pf.set_end_rect(CellRect::new(0, 3, 0, 3));
        assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
        assert_eq!(pf.nodes().len(), 1);
        let root = pf.nodes().nodes()[0];
        assert_eq!((root.x, root.z, root.side), (0, 3, 0));
        assert_eq!(root.key, start_key(0, 1, 0, 3));
        assert_eq!(root.g.to_bits(), want_g.to_bits());
        assert_eq!(pf.open_set().entries(), &[0, 0]);
    }
    // Two terminals both matching: the later terminal's link wins.
    let (w, mut net) = street_city();
    for cost in [0.25f32, 0.75] {
        net.ferries.push(fallback_terminal(
            (0, 3),
            0x2,
            vec![neighbour_link((-1, 5), (0, 5), cost)],
        ));
    }
    let mut pf = test2_pathfinder(&w);
    pf.set_start_rect(CellRect::new(-1, 5, -1, 5));
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    assert_eq!(pf.nodes().nodes()[0].g, 0.75);
}

/// §4.8.2 (b), §4.13 step 1, §4.15, §4.16: a switch move Car → Walk records
/// the park-and-ride cell; the compressed route splits the runs by type.
#[test]
fn switch_move_and_park_and_ride() {
    let (w, mut net) = street_city();
    w.tables_mut(|t| t.can_reach_destination[travel::WALK] = true);
    net.inv_max_speed[travel::WALK][net_type::STREET as usize] = 1.0;
    // Leaving (3,3) through +x (nibble 2) a car becomes Walk (type 0).
    net.conn_mut(3, 3).unwrap().switch[travel::CAR] = 0xF0FF;
    let r = net.append_info(0, 4, 3).unwrap();
    r.inv_cap = 0.0625;
    r.net_type = net_type::STREET as u8;
    let mut pf = test2_pathfinder(&w);
    pf.set_travel_type_flag(1 << travel::WALK, false);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    // (4,3) Walk: g = 1.5 + r24(1.0 + 0.0) = 2.5 beats (4,3) Car at 3.0.
    assert_eq!(route_of(&pf), vec![(2, 3, 1), (3, 3, 1), (4, 3, 0)]);
    assert_eq!(pf.get_trip_length(), 2.5);
    assert_eq!(pf.get_last_travel_type(), 0);
    // The park-and-ride cell is the Walk node's cell, its side
    // EdgeTo((3,3) → (4,3)) = 0.
    assert_eq!(pf.park_and_ride(), ((4, 3), 0));
    let mut out = Vec::new();
    assert_eq!(pf.get_compressed_route(&net, &mut out), 0x3);
    assert_eq!(out, vec![0x01, 0x02, 0x03, 0x41, 0x01, 0x02, 0x40, 0x01, 0x02]);
    // Reversible: at (3,3), tn = Walk ≠ Car and switch[Car] nibble e = 2 is
    // 0, not 0xF.
    assert!(pf.was_trip_reversible(&net));
    net.conn_mut(3, 3).unwrap().switch[travel::CAR] = 0xFFFF;
    assert!(!pf.was_trip_reversible(&net));
    // During a strike the switch mask `allowed & 0x33` keeps Walk (§4.4
    // step 6), so the same trip still ends on foot.
    let (w2, mut net2) = street_city();
    w2.strike.set(STRIKE_ON);
    w2.tables_mut(|t| t.can_reach_destination[travel::WALK] = true);
    net2.inv_max_speed[travel::WALK][net_type::STREET as usize] = 1.0;
    net2.conn_mut(3, 3).unwrap().switch[travel::CAR] = 0xF0FF;
    net2.append_info(0, 4, 3).unwrap().net_type = net_type::STREET as u8;
    let mut pf2 = test2_pathfinder(&w2);
    pf2.set_travel_type_flag(1 << travel::WALK, false);
    assert_eq!(find(&mut pf2, &w2, &mut net2), result::FOUND);
    assert_eq!(pf2.get_last_travel_type(), 0);
}

/// §4.4 step 6: during a strike the switch transitions use `allowed & 0x33`,
/// so a switch to Bus (type 2) is not taken.
#[test]
fn strike_masks_switch_transitions() {
    for strike in [2u8, STRIKE_ON] {
        let (w, mut net) = street_city();
        w.strike.set(strike);
        w.tables_mut(|t| t.can_reach_destination[travel::BUS] = true);
        net.inv_max_speed[travel::BUS][net_type::STREET as usize] = 0.5;
        net.conn_mut(3, 3).unwrap().switch[travel::CAR] = 0xF2FF;
        let mut pf = test2_pathfinder(&w);
        pf.set_travel_type_flag(1 << travel::BUS, false);
        assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
        // Bus at (4,3): Bus is not affected by traffic here, so the step is
        // r24(0.5 + 0.0) and g = 1.5 + 0.5 = 2.0 < 3.0; without the strike
        // the Bus node wins.
        let want = if strike == STRIKE_ON { 1 } else { 2 };
        assert_eq!(pf.get_last_travel_type(), want, "strike state {strike}");
    }
}

/// §4.10.1: filter 0 ends next to a destination lot with capacity; traffic
/// arrived is `min(max − used, density)`; a full destination is cleared over
/// its lot and the free count decremented.
#[test]
fn standard_destination_goal_and_stale_destination() {
    for (used, found) in [(2u32, true), (5, false)] {
        let (w, mut net) = street_city();
        w.add_lot(2, CellRect::new(4, 4, 4, 4), 202);
        w.set_destination_byte(4, 4, 1);
        w.free.set([1, 0, 0, 0]);
        w.caps.borrow_mut().insert(202, ([5, 0, 0, 0], [used, 0, 0, 0]));
        let mut pf = test2_pathfinder(&w);
        pf.set_trip_destination_filter(filter::STANDARD);
        pf.set_destination_flag(1);
        pf.set_end_rect(CellRect::new(0, 0, 0, 0));
        let r = find(&mut pf, &w, &mut net);
        // §4.5: ring 2 finds (4,4); the end rect becomes its lot.
        assert_eq!(pf.end_rect(), CellRect::new(4, 4, 4, 4));
        if found {
            assert_eq!(r, result::FOUND);
            assert_eq!(pf.get_end_cell(), (4, 4));
            assert_eq!(pf.get_traffic_arrived(), 3);
            assert_eq!(pf.destination_type_reached(), 0);
            assert_eq!(pf.get_trip_length(), 3.0);
            assert_eq!(w.destination_byte(4, 4), 1);
        } else {
            assert_eq!(r, result::NOT_FOUND);
            assert_eq!(w.destination_byte(4, 4), 0);
            assert_eq!(w.free.get()[0], 0);
        }
    }
}

/// §4.5 step 1: filter 0 fails without a free destination of a wanted type.
#[test]
fn standard_destination_needs_free_count() {
    let (w, mut net) = street_city();
    w.set_destination_byte(4, 4, 1);
    let mut pf = test2_pathfinder(&w);
    pf.set_trip_destination_filter(filter::STANDARD);
    pf.set_destination_flag(1);
    assert_eq!(find(&mut pf, &w, &mut net), result::NOT_FOUND);
    assert!(pf.nodes().is_empty());
}

/// §4.17 step 4 and the final add: income with the uncarpooled count; the
/// accumulator add is rounded to 24 bits.
#[test]
fn apply_traffic_income() {
    let (w, mut net) = street_city();
    w.tables_mut(|t| t.income_per_tile[travel::CAR] = 0.5);
    w.income.set(1.0 / 3.0);
    w.carpool.set(true);
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    pf.apply_traffic(
        &mut PathCx {
            net: &mut net,
            host: &w,
        },
        0,
    );
    // I = 3 × r24(10 × 0.5) = 15.0; acc = r24(15.0 + 1/3).
    assert_eq!(w.income.get(), f64::from((15.0f64 + 1.0 / 3.0) as f32));
}

/// §4.17: nothing happens with traffic arrived 0 (the fallback of §4.12).
#[test]
fn apply_traffic_needs_arrived() {
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    pf.clear_flag(flag::NO_BUDGET);
    pf.set_flag(flag::NEAREST_ON_FAILURE);
    pf.set_max_trip_length(2.0);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    pf.apply_traffic(
        &mut PathCx {
            net: &mut net,
            host: &w,
        },
        0,
    );
    assert!(w.volumes.borrow().is_empty());
    assert!(net.edge_density.is_empty());
}

/// §4.14: `GetTripLengthScaledForDisplay` = `r24(scale × L)`.
#[test]
fn trip_length_scaled_for_display() {
    let (w, mut net) = street_city();
    w.trip_scale.set(25.0);
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    assert_eq!(pf.get_trip_length_scaled_for_display(&w), 75.0);
}

/// §4.11: with a work time set and the deadline reached at the first check,
/// the search suspends (2) once the counter reaches 1024; it resumes at the
/// next call with the open set kept.
#[test]
fn suspend_and_resume() {
    let (w, mut net) = street_city();
    w.deadline_reached.set(true);
    let mut pf = test2_pathfinder(&w);
    pf.set_max_work_time(5);
    // m = 1024: the timer is read after the first expansion.
    assert_eq!(find(&mut pf, &w, &mut net), result::SUSPENDED);
    assert!(pf.is_suspended());
    assert!(pf.result_node().is_none());
    assert_eq!(pf.get_last_travel_type(), 0);
    w.deadline_reached.set(false);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    assert_eq!(route_of(&pf), vec![(2, 3, 1), (3, 3, 1), (4, 3, 1)]);
    // Reset clears only the suspended state.
    pf.reset();
    assert!(pf.result_node().is_some());
}

/// A delivery-boundary sample cannot reconstruct a search that suspended
/// before the boundary. Section 4.4 keeps scored nodes on resume, but a new
/// unlimited search scores them against the current volumes. This is a
/// diagnostic witness for traffic TS68, not proof of train73's cause.
#[test]
fn resumed_search_and_atomic_replay_can_read_different_volume_histories() {
    let (w, mut net) = street_city();
    net.congestion_curve = ResponseCurve::from_points(&[(0.0, 1.0), (1.0, 2.0)]);
    let mut resumed = test2_pathfinder(&w);
    resumed.set_max_work_time(5);
    w.deadline_reached.set(true);
    assert_eq!(find(&mut resumed, &w, &mut net), result::SUSPENDED);

    // The first expansion has already scored the next street node. A
    // provider change between deliveries does not retroactively rescore it.
    for x in 0..=4 {
        w.volumes.borrow_mut().insert((1, x, 3), 16.0);
    }
    w.deadline_reached.set(false);
    assert_eq!(find(&mut resumed, &w, &mut net), result::FOUND);

    let mut atomic = test2_pathfinder(&w);
    assert_eq!(find(&mut atomic, &w, &mut net), result::FOUND);
    assert_eq!(route_of(&resumed), route_of(&atomic));
    assert_ne!(resumed.get_trip_length().to_bits(), atomic.get_trip_length().to_bits());
}

/// §4.12: `DistanceToRectangle`.
#[test]
fn distance_to_rectangle() {
    let r = CellRect::new(4, 4, 4, 4);
    assert_eq!(distance_to_rect(4, 4, r), 0.0);
    assert_eq!(distance_to_rect(4, 1, r), 3.0);
    assert_eq!(distance_to_rect(7, 4, r), 3.0);
    assert_eq!(distance_to_rect(3, 3, r), 2.0f32.sqrt());
    assert_eq!(distance_to_rect(2, 3, r), 5.0f32.sqrt());
    assert_eq!(distance_to_rect(1, 3, r), 10.0f32.sqrt());
    // PF12: the exact root rounded once to float32 (§4.12 examples).
    assert_eq!(distance_to_rect(3, 3, r).to_bits(), 0x3FB5_04F3);
    assert_eq!(distance_to_rect(2, 3, r).to_bits(), 0x400F_1BBD);
    assert_eq!(distance_to_rect(1, 3, r).to_bits(), 0x404A_62C2);
}

/// §4.9 step 5 (PF1): the replacement table; an equal `g` never replaces,
/// a NaN on either side always does.
#[test]
fn add_trip_node_replacement_table() {
    assert!(replaces_node_g(3.0, 2.5));
    assert!(!replaces_node_g(3.0, 3.0));
    assert!(replaces_node_g(3.0, f32::NAN));
    assert!(replaces_node_g(f32::NAN, 5.0));
    assert!(!replaces_node_g(3.0, 3.5));
}

/// §4.18 (PF8): RecalculateNodeScores evaluates `H` at the node's own cell;
/// for an exit node that is its off-map cell, not the in-city cell
/// AddTripNode used. The §4.18 example: end rect (10,5,10,5), the search
/// rect (2,0)-(10,8) holds neither cell, the exit node from (0,5) to (−1,5).
#[test]
fn recalculated_exit_node_uses_its_own_cell() {
    let w = World::preamble(16, 8);
    let mut net = empty_net(16, 8);
    net.inv_max_speed[travel::CAR][net_type::STREET as usize] = 1.5;
    net.congestion_curve = ResponseCurve::from_points(&[(0.0, 1.0)]);
    for x in 0..=6 {
        street_cell(&mut net, x, 5);
    }
    let mut pf = Pathfinder::create(&w, shared_work_check());
    pf.set_flag(flag::NO_BUDGET);
    pf.set_start_rect(CellRect::new(6, 4, 6, 4));
    pf.set_end_rect(CellRect::new(10, 5, 10, 5));
    pf.set_trip_destination_filter(filter::END_RECT);
    pf.clear_all_travel_type_flags();
    pf.set_travel_type_flag(0x2, true);
    pf.set_starting_cost_array(Some([0.0; 9]));
    // (10,5) has no street: the search runs out.
    assert_eq!(find(&mut pf, &w, &mut net), result::NOT_FOUND);
    let exit_id = pf
        .nodes()
        .nodes()
        .iter()
        .position(|n| n.key & EXIT_KEY_BIT != 0)
        .unwrap();
    let exit = pf.nodes().nodes()[exit_id];
    assert_eq!((exit.x, exit.z), (-1, 5));
    // g(0,5) = 6 × 1.5 = 9.0; exit cost r24(f − g) = H(0,5) = 10; exit g =
    // 19.0; AddTripNode's f = H(0,5) + g = 29.0.
    assert_eq!((exit.g, exit.f), (19.0, 29.0));
    pf.recalculate_node_scores();
    // The recomputation uses H(−1,5) = 11.
    assert_eq!(pf.nodes().nodes()[exit_id].f, 30.0);
}

/// §4.7 (PF8, PF15): a new node's cell comes from its key: the low bytes
/// zero-extended, plus `dir(side)` as a 16-bit value for an exit node.
#[test]
fn key_cell_decode() {
    assert_eq!(key_cell(node_key(0x10, 1, 2, 3)), (2, 3));
    // In-city keys of −1 decode as 255 (no exit bit).
    assert_eq!(key_cell(node_key(0, 1, -1, 3)), (255, 3));
    // Exit nodes: side 0 (−x) from (0,5) is (−1,5); side 2 (+x) from (255,5)
    // is (256,5); side 1 (−z) from (4,0) is (4,−1); side 3 (+z) from (4,7) is
    // (4,8).
    assert_eq!(key_cell(node_key(0, 1, 0, 5) | EXIT_KEY_BIT), (-1, 5));
    assert_eq!(key_cell(node_key(2, 1, 255, 5) | EXIT_KEY_BIT), (256, 5));
    assert_eq!(key_cell(node_key(1, 1, 4, 0) | EXIT_KEY_BIT), (4, -1));
    assert_eq!(key_cell(node_key(3, 1, 4, 7) | EXIT_KEY_BIT), (4, 8));
    // The pSide bits do not change the side read from the key.
    assert_eq!(key_cell(node_key(2 << 2, 1, 0, 5) | EXIT_KEY_BIT), (-1, 5));
}

/// §4.7: the node key and the hash bucket.
#[test]
fn node_keys_and_buckets() {
    // pSide 2, side 0 (prefix 2 << 2 | 0), Car at (3,3).
    assert_eq!(node_key(2 << 2, 1, 3, 3), 0x0801_0303);
    // Off-map −1 keeps its low byte.
    assert_eq!(node_key(0, 1, -1, 3) & 0xFFFF, 0xFF03);
    assert_eq!(NodeStore::bucket(1), (HASH_MULTIPLIER % 3079) as usize);
    let mut s = NodeStore::new();
    // Two keys in the same bucket: the newer is at the chain's head.
    let k1 = 5u32;
    let k2 = (0..u32::MAX)
        .find(|&k| k != k1 && NodeStore::bucket(k) == NodeStore::bucket(k1))
        .unwrap();
    s.find_or_create(k1, || Node::new(k1, (0, 0), 0, 0));
    s.find_or_create(k2, || Node::new(k2, (1, 0), 0, 0));
    assert_eq!(s.bucket_order(), vec![1, 0]);
    assert_eq!(s.find_or_create(k1, || unreachable!()), (0, false));
}

/// §4.4 steps 5 and 7, §4.8.1: with flag 4 the second call keeps the node
/// store and the open set and creates no start nodes. After test 2 the heap
/// holds (0,3) and the pushed-back goal (4,3), both f = 3.0, with (0,3) at
/// the root (the goal was pushed after it, strict `>` on the sift-up). So
/// the continued search first expands (0,3), whose only exit (direction 0)
/// leaves the map: an exit node at (−1,3), g = 3.0 + r24(f − g) = 3.0 (the
/// filter-3 score of a node inside the search rect is f = g). Then the goal
/// (4,3) is popped again (the exit node, pushed after it with the same f,
/// stays below it).
#[test]
fn continue_keeps_nodes() {
    let (w, mut net) = street_city();
    let mut pf = test2_pathfinder(&w);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    assert_eq!(pf.nodes().len(), 6);
    pf.set_flag(flag::CONTINUE);
    assert_eq!(find(&mut pf, &w, &mut net), result::FOUND);
    let nodes = pf.nodes().nodes();
    assert_eq!(nodes.len(), 7);
    let exit = nodes[6];
    assert_eq!((exit.x, exit.z, exit.g), (-1, 3, 3.0));
    assert_eq!(exit.key & EXIT_KEY_BIT, EXIT_KEY_BIT);
    assert!(nodes[5].closed); // (0,3)
    assert_eq!(pf.result_node().map(|n| (n.x, n.z)), Some((4, 3)));
    assert_eq!(pf.get_trip_length().to_bits(), 0x4040_0000);
}
