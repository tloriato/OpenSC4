// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::route_map_support::Grid;
use crate::traffic::test_support::{bytes, float_environment, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Begin(u32, u32),
    Next(u32, usize),
    Grid(u32),
    Ordinance(u32),
    Ceiling(u64),
    Lot(i32, i32, u32),
    Rect,
    Direction(u32, u32, i32, i32),
    Arrival(u32, u32),
    Neighbor(u32, u32, u32, u32),
    Link(u32, u32, u32),
    Switch(u32, u32),
    Edge(u32, u32),
    Destination(u32, [u32; 5]),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    steps: Vec<[u8; 10]>,
    position: usize,
    iterator: u32,
    carpool: bool,
    lot: u32,
    rect: [i32; 4],
    switches: Vec<(u32, u32)>,
    edges: Vec<(u32, Object)>,
    arrivals: Vec<(u16, u32)>,
    neighbors: Vec<(u32, u32, u32, u32)>,
    links: Vec<(u32, u32, u32)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
unsafe extern "thiscall" fn begin(this: *mut u8, route: u32) {
    // Seed the three bytes retained by the caller's byte-only travel scratch assignment.
    unsafe {
        store(this.sub(68), 0, 0xC0FFEE00);
    }
    log(Call::Begin(this as u32, route));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.iterator = this as u32;
        s.position = 0;
    });
}
unsafe extern "thiscall" fn next(this: *mut u8, step: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as u32, s.iterator);
        let i = s.position;
        s.position += 1;
        s.calls.push(Call::Next(this as u32, i));
        let available = if let Some(value) = s.steps.get(i) {
            unsafe {
                write_unaligned(step.cast(), *value);
            }
            true
        } else {
            false
        };
        (0xDEAD_BEEFu64 << 32) | (0x1234_5600 | u64::from(available))
    })
}
unsafe extern "thiscall" fn grid(this: *mut u8) -> u32 {
    log(Call::Grid(unsafe { word(this, 8) }));
    unsafe { word(this, 4) }
}
extern "thiscall" fn ordinance(_this: *mut u8, id: u32) -> u32 {
    log(Call::Ordinance(id));
    STATE.with(|s| u32::from(s.borrow().carpool))
}
extern "C" fn ceiling(low: u32, high: u32) -> f64 {
    let bits = u64::from(low) | (u64::from(high) << 32);
    log(Call::Ceiling(bits));
    // The fixture exercises 37 commuters: the rounded product with 0.8 is 29.600000381469727.
    assert_eq!(bits, 29.600000381469727f64.to_bits());
    30.0
}
extern "thiscall" fn lot(_this: *mut u8, x: i32, z: i32, flag: u32) -> u32 {
    log(Call::Lot(x, z, flag));
    STATE.with(|s| s.borrow().lot)
}
unsafe extern "thiscall" fn rect(_this: *mut u8, out: *mut i32) {
    log(Call::Rect);
    STATE.with(|s| unsafe {
        write_unaligned(out.cast(), s.borrow().rect);
    });
}
extern "C" fn direction(x_word: u32, z_word: u32, x: i32, z: i32) -> u64 {
    log(Call::Direction(x_word, z_word, x, z));
    let dx = x.wrapping_sub(i32::from(x_word as i16));
    let dz = z.wrapping_sub(i32::from(z_word as i16));
    let d = match (dx, dz) {
        (-1, 0) => 0,
        (0, -1) => 1,
        (1, 0) => 2,
        (0, 1) => 3,
        _ => 255,
    };
    (0xABCD_9876u64 << 32) | d
}
unsafe extern "thiscall" fn arrival(this: *mut u8, out: *mut u32, key: *mut u32) -> *mut u32 {
    let key = unsafe { key.read() };
    log(Call::Arrival(this as u32, key));
    let node = STATE.with(|s| {
        s.borrow()
            .arrivals
            .iter()
            .find(|&&(k, _)| k == key as u16)
            .map_or(0, |&(_, v)| v)
    });
    unsafe {
        out.write(node);
        out.add(1).write(this as u32);
    }
    out
}
extern "thiscall" fn neighbor(this: *mut u8, terminal: u32, x: u32, z: u32) -> u32 {
    log(Call::Neighbor(this as u32, terminal, x, z));
    STATE.with(|s| {
        s.borrow()
            .neighbors
            .iter()
            .find(|&&(t, xx, zz, _)| t == terminal && xx == x && zz == z)
            .map_or(0, |&(_, _, _, link)| link)
    })
}
extern "thiscall" fn link(this: *mut u8, terminal: u32, key: u32) -> u32 {
    log(Call::Link(this as u32, terminal, key));
    STATE.with(|s| {
        s.borrow()
            .links
            .iter()
            .find(|&&(t, k, _)| t == terminal && k == key)
            .map_or(0, |&(_, _, link)| link)
    })
}
unsafe extern "thiscall" fn switch(this: *mut u8, key: *mut u32) -> u32 {
    let key = unsafe { key.read() };
    log(Call::Switch(this as u32, key));
    STATE.with(|s| {
        s.borrow()
            .switches
            .iter()
            .find(|&&(id, _)| id == key)
            .expect("switch fixture")
            .1
    })
}
unsafe extern "thiscall" fn edge(this: *mut u8, key: *mut u32) -> *mut u8 {
    let key = unsafe { key.read() };
    log(Call::Edge(this as u32, key));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some((_, o)) = s.edges.iter_mut().find(|(k, _)| *k == key) {
            return o.ptr();
        }
        let mut o = Object::new(0x44);
        let p = o.ptr();
        s.edges.push((key, o));
        p
    })
}
extern "thiscall" fn destination(this: *mut u8, count: u32, x: u32, z: u32, travel: u32, kind: u32) -> u64 {
    log(Call::Destination(this as u32, [count, x, z, travel, kind]));
    0x8765_4321_1234_5678
}
fn step(x: i16, z: i16, travel: u8, entry: u8, exit: u8, jump: bool) -> [u8; 10] {
    let mut out = [0; 10];
    out[..2].copy_from_slice(&x.to_le_bytes());
    out[2..4].copy_from_slice(&z.to_le_bytes());
    out[4] = travel;
    out[5] = travel;
    out[7] = entry;
    out[8] = exit;
    out[9] = u8::from(jump);
    out
}
fn density(x: u32, z: u32, travel: u32, evening: bool) -> [u16; 16] {
    let key = (u32::from(evening) << 24) | (travel << 16) | (x << 8) | z;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.edges
            .iter_mut()
            .find(|(k, _)| *k == key)
            .map_or([0; 16], |(_, o)| unsafe { read_unaligned(o.ptr().add(0x22).cast()) })
    })
}
fn one(index: usize, value: u16) -> [u16; 16] {
    let mut out = [0; 16];
    out[index] = value;
    out
}

struct World {
    sim: Object,
    switch_grid: Grid<u32>,
    info: Grid<u32>,
    volumes: Vec<Grid<f32>>,
    objects: Vec<Object>,
    buckets: Box<[u32; 1]>,
    _volume_objects: Box<[u32; 9]>,
    _constants: Box<[f32; 3]>,
    _services: Box<[u32; 2]>,
    _network: Box<[u32; 64]>,
    _guards: Vec<Fake>,
    _tables: Vec<Vec<usize>>,
}
impl World {
    fn new(steps: Vec<[u8; 10]>) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                steps,
                ..State::default()
            }
        });
        let tables = vec![
            table(&[(0x60, grid as *const () as usize)]),
            table(&[(0x20, ordinance as *const () as usize)]),
            table(&[(0x18, lot as *const () as usize)]),
            table(&[(0x38, rect as *const () as usize)]),
        ];
        let mut sim = Object::new(0xD20);
        let mut switch_grid = Grid::new(&tables[0], 90, 8, 8, 0u32);
        let mut info = Grid::new(&tables[0], 91, 8, 8, 0u32);
        let mut volumes: Vec<_> = (0..9).map(|n| Grid::new(&tables[0], n + 1, 8, 8, 0.0f32)).collect();
        let mut volume_objects = Box::new(core::array::from_fn(|i| volumes[i].object.ptr() as u32));
        let mut ordinance = object(4, &tables[1]);
        let mut manager = object(4, &tables[2]);
        let mut lot = object(4, &tables[3]);
        let mut services = Box::new([ordinance.ptr() as u32, manager.ptr() as u32]);
        let mut network = Box::new([0u32; 64]);
        network[..9].copy_from_slice(&[0, 1, 1, 2, 1, 2, 3, 4, 5]);
        network[63] = 8;
        let mut constants = Box::new([4_294_967_296.0, 0.8, 12_582_912.0]);
        let mut buckets = Box::new([0u32; 1]);
        let mut guards = vec![
            fake(0x006D64B0, begin as *const () as usize),
            fake(0x006D6510, next as *const () as usize),
            fake(0x0070D780, direction as *const () as usize),
            fake(0x0060D170, arrival as *const () as usize),
            fake(0x0070F9F0, neighbor as *const () as usize),
            fake(0x0070F9C0, link as *const () as usize),
            fake(0x006D90F0, switch as *const () as usize),
            fake(0x006D7820, edge as *const () as usize),
            fake(0x00714740, destination as *const () as usize),
            fake(0x009F0A20, ceiling as *const () as usize),
            fake_data(0x00B43D90, services.as_mut_ptr().cast()),
            fake_data(0x00B43D08, unsafe { services.as_mut_ptr().add(1).cast() }),
            fake_data(0x00AB31A8, network.as_mut_ptr().cast()),
        ];
        for (i, address) in [0x00A80AA8, 0x00AB3158, 0x00AB32D8].into_iter().enumerate() {
            guards.push(fake_data(address, unsafe { constants.as_mut_ptr().add(i).cast() }));
        }
        unsafe {
            let s = ApplyRouteSim::from_ptr(sim.ptr());
            s.set_network_rows(info.rows());
            s.set_switches(switch_grid.object.ptr());
            s.set_volumes(volume_objects.as_mut_ptr().cast());
            s.set_departures(buckets.as_mut_ptr().cast());
            s.set_departures_end(buckets.as_mut_ptr().add(1).cast());
            s.set_fare(0.01);
            let s = TrafficSim::from_ptr(sim.ptr());
            s.set_max_cell_x(7);
            s.set_max_cell_z(7);
            for i in 0..64 {
                sim.ptr().add(0xC34 + i).write(1);
            }
        }
        STATE.with(|s| s.borrow_mut().lot = lot.ptr() as u32);
        Self {
            sim,
            switch_grid,
            info,
            volumes,
            objects: vec![ordinance, manager, lot],
            buckets,
            _volume_objects: volume_objects,
            _constants: constants,
            _services: services,
            _network: network,
            _guards: guards,
            _tables: tables,
        }
    }
    fn add_info(&mut self, x: usize, z: usize, network: u8) {
        let mut o = Object::new(24);
        unsafe {
            o.ptr().add(6).write(network);
            store(o.ptr(), 20, self.info.cells[x][z]);
        }
        self.info.cells[x][z] = o.ptr() as u32;
        self.objects.push(o);
    }
    fn switch(&mut self, id: u32, toll: f32) -> *mut u8 {
        let mut o = Object::new(48);
        let p = o.ptr();
        unsafe {
            store(p, 0x2C, toll.to_bits());
        }
        STATE.with(|s| s.borrow_mut().switches.push((id, p as u32)));
        self.objects.push(o);
        p
    }
    fn terminal(&mut self, departure: (u8, u8), arrival: (u8, u8)) -> *mut u8 {
        let mut o = Object::new(0x40);
        let p = o.ptr();
        unsafe {
            p.add(8).write(departure.0);
            p.add(9).write(departure.1);
        }
        let mut node = Object::new(12);
        unsafe {
            store(node.ptr(), 4, (u32::from(departure.0) << 8) | u32::from(departure.1));
            store(node.ptr(), 8, p as u32);
        }
        if self.buckets[0] == 0 {
            self.buckets[0] = node.ptr() as u32;
        } else {
            unsafe {
                store(node.ptr(), 0, self.buckets[0]);
            }
            self.buckets[0] = node.ptr() as u32;
        }
        let mut arrival_node = Object::new(12);
        let ap = arrival_node.ptr();
        unsafe {
            store(ap, 8, p as u32);
        }
        STATE.with(|s| {
            s.borrow_mut()
                .arrivals
                .push(((u16::from(arrival.0) << 8) | u16::from(arrival.1), ap as u32))
        });
        self.objects.extend([o, node, arrival_node]);
        unsafe { p.add(8) }
    }
    fn neighbor(&mut self, terminal: *mut u8, x: u32, z: u32) -> *mut u8 {
        let mut o = Object::new(0x30);
        let p = o.ptr();
        STATE.with(|s| s.borrow_mut().neighbors.push((terminal as u32, x, z, p as u32)));
        self.objects.push(o);
        p
    }
    fn link(&mut self, terminal: *mut u8, key: u32) -> *mut u8 {
        let mut o = Object::new(0x24);
        let p = o.ptr();
        STATE.with(|s| s.borrow_mut().links.push((terminal as u32, key, p as u32)));
        self.objects.push(o);
        p
    }
    fn run(&mut self, count: u32, source: [i32; 4], dest: (u32, u32), mode: u32) -> Registers {
        let args = [
            0x11223344,
            count,
            source[0] as u32,
            source[1] as u32,
            source[2] as u32,
            source[3] as u32,
            dest.0,
            dest.1,
            mode,
            0x12340002,
        ];
        let r = unsafe {
            invoke(
                fc4p_00719910 as *const () as usize,
                self.sim.ptr(),
                &args,
                0xABCDEF98,
                0x13572468,
                false,
            )
        };
        self.switch_grid.padding_untouched();
        self.info.padding_untouched();
        for v in &self.volumes {
            v.padding_untouched();
        }
        r
    }
    fn income(&mut self) -> f64 {
        unsafe { TrafficSim::from_ptr(self.sim.ptr()).transit_accumulator() }
    }
    fn fees(&mut self) -> f64 {
        unsafe { TrafficSim::from_ptr(self.sim.ptr()).toll_accumulator() }
    }
}

#[test]
fn toll_and_volume_example_counts_once_in_both_directions_and_preserves_call_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![
        step(2, 3, 1, 255, 2, false),
        step(3, 3, 1, 0, 2, false),
        step(4, 3, 1, 0, 255, false),
    ]);
    for x in 2..=4 {
        w.add_info(x, 3, 1);
    }
    w.switch_grid.cells[3][3] = 5;
    let sw = w.switch(5, 0.1);
    let environment = float_environment();
    let r = w.run(37, [2, 2, 2, 2], (4, 4), 2);
    assert_eq!(r.edx, 0x87654321);
    assert_eq!(float_environment(), environment);
    assert_eq!(w.fees(), f64::from(f32::from_bits(0x406CCCCD)));
    assert_eq!(unsafe { float(sw, 4) }, 37.0);
    for x in 2..=4 {
        assert_eq!(w.volumes[1].cells[x][3], 74.0);
    }
    assert_eq!(density(2, 3, 1, false), one(6, 37));
    assert_eq!(density(2, 3, 1, true), one(9, 37));
    let actual = calls();
    let it = STATE.with(|s| s.borrow().iterator);
    let p = w.sim.ptr() as u32;
    assert_eq!(
        actual,
        [
            Call::Begin(it, 0x11223344),
            Call::Grid(90),
            Call::Ordinance(0xA2BF1DDC),
            Call::Next(it, 0),
            Call::Direction(0x00030002, 0x01010003, 2, 2),
            Call::Next(it, 1),
            Call::Grid(2),
            Call::Edge(p + 0x178, 0x010203),
            Call::Edge(p + 0x178, 0x01010203),
            Call::Next(it, 2),
            Call::Grid(2),
            Call::Switch(p + 0x70C, 5),
            Call::Edge(p + 0x178, 0x010303),
            Call::Edge(p + 0x178, 0x01010303),
            Call::Next(it, 3),
            Call::Grid(2),
            Call::Edge(p + 0x178, 0x010403),
            Call::Edge(p + 0x178, 0x01010403),
            Call::Destination(p, [37, 4, 4, 0xC0FFEE01, 0x12340002])
        ]
    );
}

#[test]
fn switch_ids_follow_every_in_city_cell_and_subway_lookup_still_occurs_before_suppression() {
    let _fpu = GameFpu::new();
    let steps = (1..=5).map(|x| step(x, 2, 2, 0, 2, false)).collect();
    let mut w = World::new(steps);
    let sw = w.switch(7, 0.5);
    for (x, id) in [(1, 0), (2, 7), (3, 7), (4, 0), (5, 7)] {
        w.switch_grid.cells[x][2] = id;
    }
    w.run(10, [0, 2, 0, 2], (6, 2), 0);
    assert_eq!(unsafe { float(sw, 4) }, 20.0);
    assert_eq!(w.fees(), 10.0);
    let mut w = World::new(vec![step(1, 3, 6, 0, 2, false), step(2, 3, 6, 0, 2, false)]);
    let sw = w.switch(11, 0.5);
    w.switch_grid.cells[1][3] = 11;
    w.run(10, [0, 3, 0, 3], (3, 3), 0);
    assert_eq!(unsafe { float(sw, 4) }, 0.0);
    assert!(calls().contains(&Call::Switch(w.sim.ptr() as u32 + 0x70C, 11)));
}

#[test]
fn return_route_uses_destination_rectangle_and_invalid_last_side_keeps_volume_without_density() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![
        step(4, 3, 1, 255, 0, false),
        step(3, 3, 1, 2, 0, false),
        step(2, 3, 1, 2, 255, false),
    ]);
    STATE.with(|s| s.borrow_mut().rect = [3, 4, 5, 5]);
    w.run(40, [2, 2, 2, 2], (5, 5), 1);
    assert_eq!(density(4, 3, 1, true), one(12, 40));
    assert_eq!(density(3, 3, 1, true), one(8, 40));
    assert_eq!(density(2, 3, 1, true), one(9, 40));
    let actual = calls();
    assert!(actual.contains(&Call::Lot(5, 5, 0)));
    assert!(actual.contains(&Call::Rect));
    assert!(!actual.iter().any(|c| matches!(c, Call::Destination(..))));
    let mut w = World::new(vec![
        step(2, 3, 1, 255, 2, false),
        step(3, 3, 1, 0, 2, false),
        step(4, 3, 1, 0, 255, false),
    ]);
    w.add_info(4, 3, 1);
    w.run(40, [2, 2, 2, 2], (4, 5), 0);
    assert_eq!(density(4, 3, 1, false), [0; 16]);
    assert_eq!(w.volumes[1].cells[4][3], 40.0);
}

#[test]
fn invalid_intermediate_side_leaves_partial_volume_and_switch_load_and_discards_income() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![
        step(2, 3, 1, 255, 2, false),
        step(3, 3, 1, 0, 255, false),
        step(4, 3, 1, 255, 3, false),
    ]);
    for x in 2..=4 {
        w.add_info(x, 3, 1);
    }
    let sw = w.switch(5, 0.5);
    w.switch_grid.cells[3][3] = 5;
    unsafe {
        store(w.sim.ptr(), 0xBA8, 1.0f32.to_bits());
    }
    w.run(40, [2, 2, 2, 2], (4, 4), 0);
    assert_eq!(
        (
            w.volumes[1].cells[2][3],
            w.volumes[1].cells[3][3],
            w.volumes[1].cells[4][3]
        ),
        (40.0, 40.0, 0.0)
    );
    assert_eq!(unsafe { float(sw, 4) }, 40.0);
    assert_eq!(density(3, 3, 1, false), [0; 16]);
    assert_eq!(w.income(), 0.0);
    assert_eq!(w.fees(), 0.0);
    assert!(!calls().iter().any(|c| matches!(c, Call::Destination(..))));
}

#[test]
fn ferry_crossings_wrap_riders_and_links_and_charge_one_fare_for_all_modes() {
    let _fpu = GameFpu::new();
    for (mode, riders, a, b) in [(0, 30, 30, 0), (1, 30, 0, 0), (2, 60, 30, 30)] {
        let mut w = World::new(vec![
            step(1, 2, 0, 255, 2, false),
            step(2, 2, 0, 0, 2, false),
            step(6, 3, 0, 0, 2, true),
            step(7, 3, 0, 0, 255, false),
        ]);
        STATE.with(|s| s.borrow_mut().lot = 0);
        let t = w.terminal((2, 2), (2, 1));
        let u = w.terminal((6, 2), (6, 3));
        let ab = w.link(t, 0x0602);
        let ba = w.link(u, 0x0202);
        unsafe {
            write_unaligned(t.add(0x1A).cast::<u16>(), 65530);
        }
        w.run(30, [0, 2, 0, 2], (7, 4), mode);
        assert_eq!(
            unsafe { read_unaligned(t.add(0x1A).cast::<u16>()) },
            65530u16.wrapping_add(riders)
        );
        assert_eq!(
            unsafe { read_unaligned(ab.add(if mode == 1 { 0x1E } else { 0x1A }).cast::<u16>()) },
            if mode == 1 { 30 } else { a }
        );
        assert_eq!(unsafe { read_unaligned(ba.add(0x1E).cast::<u16>()) }, b);
        assert_eq!(w.income(), f64::from(f32::from_bits(0x3E999999)));
    }
}

#[test]
fn ferry_neighbor_call_receives_packed_step_words_and_first_cell_lookup_uses_equal_adjacency_flags() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![step(3, 6, 0, 255, 2, false), step(0, 6, 0, 0, 255, true)]);
    let t = w.terminal((3, 6), (3, 7));
    let packed_x = word_from_step(step(0, 6, 0, 0, 255, true), 0);
    let packed_z = word_from_step(step(0, 6, 0, 0, 255, true), 2);
    let n = w.neighbor(t, packed_x, packed_z);
    w.run(30, [2, 6, 2, 6], (u32::MAX, 6), 2);
    assert_eq!(unsafe { read_unaligned(n.add(0x26).cast::<u16>()) }, 30);
    assert_eq!(unsafe { read_unaligned(n.add(0x2A).cast::<u16>()) }, 30);
    assert!(calls().contains(&Call::Neighbor(w.sim.ptr() as u32, t as u32, packed_x, packed_z)));
    for (source, lookup) in [([-1, 5, -1, 5], false), ([-1, 4, -1, 4], true), ([0, 5, 0, 5], true)] {
        let mut w = World::new(vec![step(0, 5, 0, 255, 2, false)]);
        let t = w.terminal((3, 6), (0, 5));
        let n = w.neighbor(t, source[0] as u32, source[1] as u32);
        w.run(30, source, (2, 5), 2);
        assert_eq!(
            unsafe { read_unaligned(n.add(0x26).cast::<u16>()) },
            if lookup { 30 } else { 0 }
        );
        assert_eq!(
            w.income(),
            if lookup {
                f64::from(f32::from_bits(0x3E999999))
            } else {
                0.0
            }
        );
    }
}
fn word_from_step(step: [u8; 10], offset: usize) -> u32 {
    u32::from_le_bytes(step[offset..offset + 4].try_into().unwrap())
}

#[test]
fn carpool_ceiling_saturation_off_map_income_empty_routes_and_full_type_indexing() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![step(1, 1, 1, 255, 2, false)]);
    STATE.with(|s| s.borrow_mut().carpool = true);
    w.add_info(1, 1, 1);
    w.run(37, [0, 1, 0, 1], (2, 1), 2);
    assert_eq!(w.volumes[1].cells[1][1], 60.0);
    assert_eq!(density(1, 1, 1, false), one(2, 30));
    let mut w = World::new(vec![step(1, 1, 1, 255, 2, false)]);
    w.run(70000, [0, 1, 0, 1], (2, 1), 2);
    assert_eq!(density(1, 1, 1, false), one(2, 65535));
    let mut w = World::new(vec![
        step(1, 3, 1, 255, 0, false),
        step(0, 3, 1, 2, 0, false),
        step(-1, 3, 1, 2, 255, false),
    ]);
    STATE.with(|s| s.borrow_mut().lot = 0);
    unsafe {
        store(w.sim.ptr(), 0xBA8, 1.0f32.to_bits());
    }
    w.run(40, [2, 3, 2, 3], (u32::MAX, 3), 1);
    assert_eq!(w.income(), 120.0);
    let mut w = World::new(vec![]);
    let before = unsafe { bytes(w.sim.ptr(), 0xD20) };
    let r = w.run(40, [0; 4], (0, 0), 0);
    assert_eq!(r.edx, 0xDEADBEEF);
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD20) }, before);
    assert_eq!(calls().len(), 4);
    let mut w = World::new(vec![step(1, 1, 63, 255, 2, false)]);
    w.add_info(1, 1, 8);
    unsafe {
        store(w.sim.ptr(), 0xBA4 + 63 * 4, 2.0f32.to_bits());
    }
    w.run(5, [0, 1, 0, 1], (2, 1), 0);
    assert_eq!(w.volumes[8].cells[1][1], 5.0);
    assert_eq!(w.income(), 10.0);
}

#[test]
fn unsigned_count_conversion_and_fractional_density_rounding_follow_the_x87_control_word() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![step(1, 1, 0, 255, 2, false)]);
    w.add_info(1, 1, 0);
    w.run(u32::MAX, [0, 1, 0, 1], (2, 1), 0);
    assert_eq!(w.volumes[0].cells[1][1].to_bits(), 0x4F800000);
    assert_eq!(density(1, 1, 0, false), one(2, 65535));
    // Use a nonstandard precision mode to retain integer 16777217 until its explicit f32 store.
    crate::x87::set_control_word(0x037F);
    let mut w = World::new(vec![step(1, 1, 0, 255, 2, false)]);
    w.add_info(1, 1, 0);
    w.volumes[0].cells[1][1] = 16_777_216.0;
    w.run(1, [0, 1, 0, 1], (2, 1), 2);
    // The first store rounds 16777217 down, but the second add uses the retained register value.
    assert_eq!(w.volumes[0].cells[1][1], 16_777_218.0);
}

#[test]
fn route_preserves_an_existing_x87_value_control_word_mxcsr_and_direction_flag() {
    let _fpu = GameFpu::new();
    let mut w = World::new(vec![step(1, 1, 1, 255, 2, false)]);
    w.add_info(1, 1, 1);
    let saved = 123.25f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{value}]", value=in(reg) &saved, out("st(0)") _);
    }
    let before = float_environment();
    w.run(37, [0, 1, 0, 1], (2, 1), 2);
    assert_eq!(float_environment(), before);
    let mut after = 0.0f64;
    unsafe {
        core::arch::asm!("fstp qword ptr [{value}]", value=in(reg) &mut after, out("st(0)") _);
    }
    assert_eq!(saved.to_bits(), after.to_bits());
}
