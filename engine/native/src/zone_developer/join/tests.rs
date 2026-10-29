// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The core's dead-end and rail-crossing scenarios, observed through the game's object interfaces.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Query(u32),
    Type(u32, u32),
    Blocked,
    Bounds,
    Lookup(i32, i32, u32),
    Matches(u32),
    Edge(u32, u32, u32),
    Flags(u32),
    Draw(u32, [u32; 8]),
    Release,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static CELLS: RefCell<Vec<(i32, i32, u32, *mut u8)>> = const { RefCell::new(Vec::new()) };
    static MODE: RefCell<*mut u8> = const { RefCell::new(core::ptr::null_mut()) };
}

fn log(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

#[repr(C)]
struct Network {
    table: *const usize,
    id: u32,
    types: u32,
    edges: [u32; 4],
    flags: u32,
    blocked: bool,
    matches: bool,
    clear_mode: Option<(u32, u32)>,
}

#[repr(C)]
struct Occupant {
    table: *const usize,
    network: *mut u8,
    result: u32,
    bounds: [i32; 4],
}

extern "thiscall" fn query(this: *mut Occupant, id: u32, out: *mut *mut u8) -> u32 {
    log(Call::Query(id));
    unsafe {
        assert!(out.read().is_null());
        out.write((*this).network);
        (*this).result
    }
}

extern "thiscall" fn bounds(this: *mut Occupant, out: *mut i32) {
    log(Call::Bounds);
    unsafe {
        core::ptr::copy_nonoverlapping((*this).bounds.as_ptr(), out, 4);
    }
}

extern "thiscall" fn kind(this: *mut Network, kind: u32) -> u32 {
    unsafe {
        log(Call::Type((*this).id, kind));
        0xFACE_0000 | if (*this).types & (1 << kind) != 0 { 0x80 } else { 0 }
    }
}

extern "thiscall" fn blocked(this: *mut Network) -> u32 {
    log(Call::Blocked);
    0xFACE_0000 | u32::from(unsafe { (*this).blocked })
}

extern "thiscall" fn edge(this: *mut Network, edge: u32, kind: u32) -> u32 {
    unsafe {
        log(Call::Edge((*this).id, edge, kind));
        if (*this).clear_mode == Some((edge, kind)) {
            MODE.with(|mode| **mode.borrow() = 0);
        }
        0xFACE_0000
            | if (*this).edges[edge as usize] & (1 << kind) != 0 {
                0x80
            } else {
                0
            }
    }
}

extern "thiscall" fn flags(this: *mut Network) -> u32 {
    unsafe {
        log(Call::Flags((*this).id));
        (*this).flags
    }
}

extern "thiscall" fn release(_this: *mut Network) -> u32 {
    log(Call::Release);
    0xA1B2_C3D4
}

#[allow(clippy::too_many_arguments)]
extern "thiscall" fn draw(this: *mut Network, x: u32, z: u32, tx: u32, tz: u32, a: u32, b: u32, c: u32, d: u32) {
    log(Call::Draw(unsafe { (*this).id }, [x, z, tx, tz, a, b, c, d]));
}

extern "thiscall" fn lookup(_this: *mut u8, x: i32, z: i32, mask: u32) -> *mut u8 {
    log(Call::Lookup(x, z, mask));
    CELLS.with(|cells| {
        cells
            .borrow()
            .iter()
            .find(|&&(cx, cz, cm, _)| (cx, cz, cm) == (x, z, mask))
            .map_or(null_mut(), |&(_, _, _, p)| p)
    })
}

#[unsafe(naked)]
unsafe extern "C" fn match_stub() {
    core::arch::naked_asm!("mov ecx, esi", "jmp {}", sym match_body);
}

extern "thiscall" fn match_body(this: *mut Network) -> u32 {
    unsafe {
        log(Call::Matches((*this).id));
        u32::from((*this).matches)
    }
}

struct World {
    memory: Object,
    table: Box<[usize; 48]>,
    occupant_table: Box<[usize; 12]>,
    occupant: Box<Occupant>,
    networks: Vec<Network>,
    mode: Box<u8>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new(types: u32) -> Self {
        take();
        CELLS.with(|cells| cells.borrow_mut().clear());
        let mut table = Box::new([0; 48]);
        for (offset, function) in [
            (8, release as *const () as usize),
            (0x58, kind as *const () as usize),
            (0x44, blocked as *const () as usize),
            (0xBC, edge as *const () as usize),
            (0x30, flags as *const () as usize),
            (0x1C, draw as *const () as usize),
        ] {
            table[offset / 4] = function;
        }
        let mut occupant_table = Box::new([0; 12]);
        occupant_table[0] = query as *const () as usize;
        occupant_table[0x2C / 4] = bounds as *const () as usize;
        // Reserve once: pointers published to the game's interfaces must stay fixed.
        let mut networks = Vec::with_capacity(32);
        for id in 0..16 {
            networks.push(Network {
                table: table.as_ptr(),
                id,
                types: 0,
                edges: [0; 4],
                flags: 0,
                blocked: false,
                matches: true,
                clear_mode: None,
            });
        }
        networks[0].types = types;
        let occupant = Box::new(Occupant {
            table: occupant_table.as_ptr(),
            network: networks.as_mut_ptr().cast(),
            result: 0xFACE_0080,
            bounds: [20, 20, 30, 40],
        });
        let mut mode = Box::new(0u8);
        let mode_ptr = (&mut *mode) as *mut u8;
        MODE.with(|slot| *slot.borrow_mut() = mode_ptr);
        let fakes = vec![
            fake(0x0072_FCA0, lookup as *const () as usize),
            fake(0x0072_9F90, match_stub as *const () as usize),
            fake_data(0x00B4_B391, mode_ptr),
        ];
        let mut world = Self {
            memory: Object::new(0x100),
            table,
            occupant_table,
            occupant,
            networks,
            mode,
            _fakes: fakes,
        };
        let dev = world.dev();
        dev.set_road_tool(world.pointer(10));
        dev.set_street_tool(world.pointer(11));
        dev.set_dirt_tool(world.pointer(12));
        world
    }

    fn dev(&mut self) -> ZoneDeveloper {
        unsafe { ZoneDeveloper::from_ptr(self.memory.ptr()) }
    }
    fn pointer(&mut self, id: usize) -> *mut u8 {
        (&mut self.networks[id] as *mut Network).cast()
    }
    fn cell(&mut self, x: i32, z: i32, mask: u32, id: usize) {
        let p = self.pointer(id);
        CELLS.with(|cells| cells.borrow_mut().push((x, z, mask, p)));
    }
    fn call(&mut self, rail: bool) -> u32 {
        // Keep the owned tables live throughout every virtual call.
        assert_ne!(self.table[2], 0);
        assert_ne!(self.occupant_table[0], 0);
        unsafe {
            let before = core::slice::from_raw_parts(self.memory.ptr(), 0x100).to_vec();
            let result = if rail {
                fc4p_00732450(self.memory.ptr(), (&mut *self.occupant as *mut Occupant).cast())
            } else {
                fc4p_00731f60(self.memory.ptr(), (&mut *self.occupant as *mut Occupant).cast())
            };
            assert_eq!(core::slice::from_raw_parts(self.memory.ptr(), 0x100), before);
            result
        }
    }
}

fn drawn(calls: &[Call]) -> Vec<Call> {
    calls
        .iter()
        .filter(|call| matches!(call, Call::Draw(..)))
        .cloned()
        .collect()
}

#[test]
fn dead_end_core_scenarios_all_directions_tools_and_connection_counts() {
    // Core vector zn15_50: each outward stub, each tool, wrong-facing and multi-edge neighbors.
    for (types, tool, selected_queries) in [
        (1, 10, vec![0]),
        (1 << 10, 10, vec![0, 10]),
        (1 << 3, 11, vec![0, 10, 3]),
        (1 << 11, 12, vec![0, 10, 3, 11]),
    ] {
        for direction in 0..4 {
            for (connection_count, facing) in [(0, direction), (1, direction), (1, (direction + 1) % 4), (2, direction)]
            {
                let mut world = World::new(types);
                let neighbors = [(19, 20), (20, 19), (21, 20), (20, 21)];
                let (nx, nz) = neighbors[direction];
                world.cell(nx, nz, 0xC09, 1);
                if connection_count > 0 {
                    world.networks[1].edges[facing] = 1 << 3;
                }
                if connection_count > 1 {
                    world.networks[1].edges[(facing + 1) % 4] = 1 << 11;
                }
                assert_eq!(world.call(false), 0xA1B2_C3D4);
                let mut expected = vec![Call::Query(0xA821_EF94)];
                expected.extend(selected_queries.iter().map(|&kind| Call::Type(0, kind)));
                expected.extend([Call::Blocked, Call::Bounds]);
                for (d, (x, z)) in neighbors.into_iter().enumerate() {
                    expected.push(Call::Lookup(x, z, 0xC09));
                    if d != direction {
                        continue;
                    }
                    expected.push(Call::Matches(1));
                    for edge in 0..4 {
                        expected.push(Call::Edge(1, edge, 3));
                        if world.networks[1].edges[edge as usize] & (1 << 3) == 0 {
                            expected.push(Call::Edge(1, edge, 11));
                        }
                    }
                    if connection_count == 1 && facing == direction {
                        expected.push(Call::Draw(tool, [20, 20, nx as u32, nz as u32, 0, 0, 1, u32::MAX]));
                    }
                }
                expected.push(Call::Release);
                assert_eq!(take(), expected);
            }
        }
    }
}

#[test]
fn dead_end_mode_byte_is_read_between_the_road_and_one_way_tests() {
    let mut world = World::new(1 << 3);
    *world.mode = 1;
    world.cell(19, 20, 0xC09, 1);
    world.networks[1].edges[0] = 1 << 10;
    assert_eq!(world.call(false), 0xA1B2_C3D4);
    let calls = take();
    assert_eq!(drawn(&calls), [Call::Draw(11, [20, 20, 19, 20, 0, 0, 1, u32::MAX])]);
    assert!(calls.windows(4).any(|calls| calls
        == [
            Call::Edge(1, 0, 3),
            Call::Edge(1, 0, 11),
            Call::Edge(1, 0, 0),
            Call::Edge(1, 0, 10)
        ]));
    world.networks[1].clear_mode = Some((0, 0));
    assert_eq!(world.call(false), 0xA1B2_C3D4);
    let calls = take();
    assert!(!calls.contains(&Call::Edge(1, 0, 10)));
    assert!(!calls.contains(&Call::Edge(1, 1, 0)));
    assert!(drawn(&calls).is_empty());
}

#[test]
fn dead_end_street_precedes_dirt_on_the_same_edge_and_predicate_can_reject() {
    let mut world = World::new(1 << 3);
    world.cell(19, 20, 0xC09, 1);
    world.networks[1].edges[0] = (1 << 3) | (1 << 11);
    assert_eq!(world.call(false), 0xA1B2_C3D4);
    assert_eq!(drawn(&take()).len(), 1);
    world.networks[1].matches = false;
    assert_eq!(world.call(false), 0xA1B2_C3D4);
    let calls = take();
    assert!(calls.contains(&Call::Matches(1)));
    assert!(!calls.iter().any(|call| matches!(call, Call::Edge(..))));
}

#[test]
fn network_queries_release_on_all_early_exits_and_keep_failure_eax() {
    for rail in [false, true] {
        let mut world = World::new(1 << 3);
        world.occupant.result = 0xAABB_CC00;
        world.occupant.network = null_mut();
        assert_eq!(world.call(rail), 0xAABB_CC00);
        assert_eq!(take(), [Call::Query(0xA821_EF94)]);
        world.occupant.network = world.pointer(0);
        assert_eq!(world.call(rail), 0xA1B2_C3D4);
        assert_eq!(take(), [Call::Query(0xA821_EF94), Call::Release]);
        world.occupant.result = 1;
        world.networks[0].blocked = true;
        assert_eq!(world.call(rail), 0xA1B2_C3D4);
        let calls = take();
        assert_eq!(calls.last(), Some(&Call::Release));
        assert!(calls.contains(&Call::Blocked));
        assert!(!calls.contains(&Call::Bounds));
        world.networks[0].blocked = false;
        if rail {
            world.dev().set_dirt_tool(null_mut());
        } else {
            world.dev().set_street_tool(null_mut());
        }
        assert_eq!(world.call(rail), 0xA1B2_C3D4);
        let calls = take();
        assert!(!calls.contains(&Call::Blocked));
        assert_eq!(calls.last(), Some(&Call::Release));
    }
    let mut world = World::new(0);
    assert_eq!(world.call(false), 0xA1B2_C3D4);
    assert_eq!(
        take(),
        [
            Call::Query(0xA821_EF94),
            Call::Type(0, 0),
            Call::Type(0, 10),
            Call::Type(0, 3),
            Call::Type(0, 11),
            Call::Release
        ]
    );
}

#[test]
fn rail_core_scenarios_each_direction_and_tool_preference() {
    // Core vector zn15_51: perpendicular rail, street preferred, dirt fallback, reversed drawing direction.
    for (direction, (dx, dz)) in [(-1, 0), (0, -1), (1, 0), (0, 1)].into_iter().enumerate() {
        for street_matches in [true, false] {
            let mut world = World::new(1 << 3);
            let (nx, nz) = (20 + dx, 20 + dz);
            let (fx, fz) = (20 + 2 * dx, 20 + 2 * dz);
            world.cell(nx, nz, 2, 1);
            world.cell(fx, fz, 8, 2);
            world.cell(fx, fz, 0x800, 3);
            world.networks[1].flags = 0x8000_0002;
            for edge in 0..4 {
                if (edge & 1 != 0) == (dx != 0) {
                    world.networks[1].edges[edge] = 1 << 1;
                }
            }
            world.networks[2].types = if street_matches { 1 << 3 } else { 0 };
            world.networks[3].types = 1 << 11;
            assert_eq!(world.call(true), 0xA1B2_C3D4);
            let mut expected = vec![Call::Query(0xA821_EF94), Call::Blocked, Call::Bounds];
            for (d, (x, z)) in [(19, 20), (20, 19), (21, 20), (20, 21)].into_iter().enumerate() {
                expected.push(Call::Lookup(x, z, 2));
                if d != direction {
                    continue;
                }
                expected.push(Call::Flags(1));
                expected.extend((0..4).map(|edge| Call::Edge(1, edge, 1)));
                expected.extend([Call::Lookup(fx, fz, 8), Call::Type(2, 3)]);
                if !street_matches {
                    expected.extend([Call::Lookup(fx, fz, 0x800), Call::Type(3, 11)]);
                }
                expected.push(Call::Draw(
                    if street_matches { 11 } else { 12 },
                    [20, 20, fx as u32, fz as u32, 0, 0, 1, u32::MAX],
                ));
            }
            expected.push(Call::Release);
            assert_eq!(take(), expected);
        }
    }
}

#[test]
fn rail_flags_and_each_edge_short_circuit_before_far_cell_lookups() {
    for (dx, dz) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
        for failure in 0..5 {
            let mut world = World::new(0);
            world.cell(20 + dx, 20 + dz, 2, 1);
            world.networks[1].flags = if failure == 4 { 3 } else { 2 };
            for edge in 0..4 {
                let wanted = (edge & 1 != 0) == (dx != 0);
                if wanted ^ (edge == failure) {
                    world.networks[1].edges[edge] = 1 << 1;
                }
            }
            assert_eq!(world.call(true), 0xA1B2_C3D4);
            let calls = take();
            let edges: Vec<_> = calls
                .iter()
                .filter(|call| matches!(call, Call::Edge(..)))
                .cloned()
                .collect();
            assert_eq!(
                edges,
                if failure == 4 {
                    vec![]
                } else {
                    (0..=failure).map(|edge| Call::Edge(1, edge as u32, 1)).collect()
                }
            );
            assert_eq!(calls.iter().filter(|call| matches!(call, Call::Lookup(..))).count(), 4);
            assert!(drawn(&calls).is_empty());
        }
    }
}

#[test]
fn coordinate_arithmetic_wraps_at_both_signed_limits() {
    let mut world = World::new(1);
    world.occupant.bounds = [i32::MIN, i32::MAX, 0, 0];
    for rail in [false, true] {
        assert_eq!(world.call(rail), 0xA1B2_C3D4);
        let calls = take();
        let lookups: Vec<_> = calls
            .into_iter()
            .filter(|call| matches!(call, Call::Lookup(..)))
            .collect();
        let mask = if rail { 2 } else { 0xC09 };
        assert_eq!(
            lookups,
            [
                Call::Lookup(i32::MAX, i32::MAX, mask),
                Call::Lookup(i32::MIN, i32::MAX - 1, mask),
                Call::Lookup(i32::MIN + 1, i32::MAX, mask),
                Call::Lookup(i32::MIN, i32::MIN, mask)
            ]
        );
    }
}
