// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Supply(usize),
    Demand(usize),
    Change(usize, u32),
    Recompute(usize),
    Census(u32, u32),
    SetSupply(u32),
    Developer(usize),
}
thread_local! { static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) }; }
fn record(event: Event) {
    EVENTS.with(|events| events.borrow_mut().push(event));
}
fn events() -> Vec<Event> {
    EVENTS.with(|events| core::mem::take(&mut *events.borrow_mut()))
}

#[repr(C, packed(4))]
struct Node {
    table: *const usize,
    supply: f64,
    demand: f64,
}
impl Node {
    fn ptr(&mut self) -> *mut u8 {
        (self as *mut Self).cast()
    }
}

extern "thiscall" fn supply(this: *mut u8) -> f64 {
    record(Event::Supply(this as usize));
    // SAFETY: all test demand nodes have this layout.
    unsafe { read_unaligned(this.add(4).cast()) }
}
extern "thiscall" fn demand(this: *mut u8) -> f64 {
    record(Event::Demand(this as usize));
    // SAFETY: all test demand nodes have this layout.
    unsafe { read_unaligned(this.add(12).cast()) }
}
extern "thiscall" fn change(this: *mut u8, amount: f32) -> u64 {
    record(Event::Change(this as usize, amount.to_bits()));
    0x1234_5678_1111_1111
}
extern "thiscall" fn recompute(this: *mut u8) -> u64 {
    record(Event::Recompute(this as usize));
    0x5566_7788_AABB_CCDD
}
fn node_table() -> [usize; 31] {
    let mut t = [0; 31];
    t[8] = supply as *const () as usize;
    t[9] = demand as *const () as usize;
    t[17] = change as *const () as usize;
    t[30] = recompute as *const () as usize;
    t
}

#[test]
fn free_and_total_preserve_full_ftol_results_and_supply_rounding() {
    let _fpu = GameFpu::new();
    let table = node_table();
    let mut node = Node {
        table: table.as_ptr(),
        supply: 0.0,
        demand: 0.0,
    };
    let mut object = Object::new(0xE4);
    // SAFETY: complete simulator storage and the four installed demand slots.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_construction(node.ptr());
        for (s, d, expected) in [
            (100.75, 20.5, 80u64),
            (10.0, 10.9, 0),
            (10.0, 11.0, 0xFFFF_FFFF_0000_0000),
            (16_777_217.0, 16_777_215.0, 1),
            (4_294_967_297.0, 0.0, 0x0000_0001_0000_0000),
            (f64::NAN, 0.0, 0x8000_0000_0000_0000),
        ] {
            node.supply = s;
            node.demand = d;
            assert_eq!(fc4p_006958e0(sim.as_ptr()), expected, "{s} - {d}");
            assert_eq!(
                events(),
                [Event::Supply(node.ptr() as usize), Event::Demand(node.ptr() as usize)]
            );
        }
        for (s, expected) in [
            (100.75, 100),
            (-5.9, -5),
            (4_294_967_297.0, 4_294_967_297),
            (f64::NAN, i64::MIN),
        ] {
            node.supply = s;
            assert_eq!(fc4p_00695910(sim.as_ptr()), expected);
            assert_eq!(events(), [Event::Supply(node.ptr() as usize)]);
        }
    }
}

#[test]
fn claim_uses_uint16_checks_rounded_sum_and_overwrites_argument() {
    let _fpu = GameFpu::new();
    let table = node_table();
    let mut node = Node {
        table: table.as_ptr(),
        supply: 100.0,
        demand: 90.0,
    };
    let mut object = Object::new(0xE4);
    // SAFETY: the test supplies writable argument storage and a live demand fake.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_construction(node.ptr());
        for (s, d, count, succeeds) in [
            (100.0, 90.0, 10, true),
            (100.0, 90.0, 11, false),
            (100.0, 90.0, 0xFFFF_000A, true),
            (65_535.0, 0.0, 0x1234_FFFF, true),
            (16_777_216.0, 16_777_216.0, 1, true),
            (f64::NAN, 0.0, 0, false),
            (100.0, f64::NAN, 0, false),
        ] {
            node.supply = s;
            node.demand = d;
            let mut word = count;
            let result = claim(sim.as_ptr(), &mut word, count);
            let count_float = F80::from_i32((count & 0xFFFF) as i32).to_f32().to_bits();
            assert_eq!(word, count_float);
            assert_eq!(result as u8 != 0, succeeds);
            let mut expected = vec![Event::Demand(node.ptr() as usize), Event::Supply(node.ptr() as usize)];
            if succeeds {
                expected.extend([
                    Event::Change(node.ptr() as usize, count_float),
                    Event::Recompute(node.ptr() as usize),
                ]);
                assert_eq!(result, 0x5566_7788_AABB_CC01);
            }
            assert_eq!(events(), expected);
        }
        node.supply = 100.0;
        node.demand = 0.0;
        std::hint::black_box(&node);
        let (eax, edx) = call_thiscall_with_eax(fc4p_00695920 as *const () as usize, sim.as_ptr(), &[10], 0);
        assert_eq!((eax, edx), (0xAABB_CC01, 0x5566_7788));
        assert_eq!(events().len(), 4);
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn supply_registers(_this: *mut u8) -> f64 {
    core::arch::naked_asm!(
        "fld qword ptr [ecx + 4]",
        "mov eax, 0xaabbccdd",
        "mov edx, 0x11223344",
        "ret"
    );
}

#[test]
fn rejected_claim_retains_supply_registers_and_comparison_status() {
    let _fpu = GameFpu::new();
    let mut table = node_table();
    table[8] = supply_registers as *const () as usize;
    let mut node = Node {
        table: table.as_ptr(),
        supply: 0.0,
        demand: 0.0,
    };
    let mut object = Object::new(0xE4);
    // SAFETY: the raw supply fake returns its float in ST0 and distinctive integer registers.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_construction(node.ptr());
        let mut amount = 1;
        let result = claim(sim.as_ptr(), &mut amount, 1);
        let status = F80::from_f32(0.0).compare(F80::from_f32(1.0)).status_word();
        assert_eq!(result, 0x1122_3344_AABB_0000 | u64::from(status & 0xFF00));
        assert_eq!(events(), [Event::Demand(node.ptr() as usize)]);
    }
}

#[test]
fn release_negates_uint16_and_recomputes_demand() {
    let _fpu = GameFpu::new();
    let table = node_table();
    let mut node = Node {
        table: table.as_ptr(),
        supply: 100.0,
        demand: 90.0,
    };
    let mut object = Object::new(0xE4);
    // SAFETY: the test supplies writable argument storage and both demand slots.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_construction(node.ptr());
        for amount in [0u32, 1, 0x1234_FFFF] {
            let mut word = amount;
            let count = (amount & 0xFFFF).wrapping_neg();
            assert_eq!(
                release_resources(sim.as_ptr(), &mut word, amount),
                0x5566_7788_AABB_CCDD
            );
            assert_eq!(word, count);
            assert_eq!(
                events(),
                [
                    Event::Change(node.ptr() as usize, F80::from_i32(count as i32).to_f32().to_bits()),
                    Event::Recompute(node.ptr() as usize)
                ]
            );
        }
        assert_eq!(
            call_thiscall_with_eax(fc4p_00695980 as *const () as usize, sim.as_ptr(), &[10], 0),
            (0xAABB_CCDD, 0x5566_7788)
        );
        assert_eq!(events().len(), 2);
    }
}

extern "thiscall" fn census(this: *mut u8, kind: u32, zero: u32) -> *mut u8 {
    record(Event::Census(kind, zero));
    let offset = match kind {
        0x1001 => 4,
        0x3001 => 8,
        0x4001 => 12,
        _ => unreachable!(),
    };
    // SAFETY: the test census object holds its three node pointers after the table.
    unsafe { read_unaligned(this.add(offset).cast()) }
}
extern "thiscall" fn set_supply(_this: *mut u8, value: f32) {
    record(Event::SetSupply(value.to_bits()));
}
extern "thiscall" fn monthly_developer(this: *mut u8) {
    record(Event::Developer(this as usize));
}

#[test]
fn monthly_supply_core_vectors_unsigned_clamps_and_state_gate() {
    let _fpu = GameFpu::new();
    let mut table = node_table();
    table[18] = set_supply as *const () as usize;
    let mut nodes = [
        Node {
            table: table.as_ptr(),
            supply: 0.0,
            demand: 0.0,
        },
        Node {
            table: table.as_ptr(),
            supply: 0.0,
            demand: 0.0,
        },
        Node {
            table: table.as_ptr(),
            supply: 0.0,
            demand: 0.0,
        },
    ];
    let mut census_table = [0; 7];
    census_table[6] = census as *const () as usize;
    let mut census_object = [
        census_table.as_ptr() as usize,
        nodes[0].ptr() as usize,
        nodes[1].ptr() as usize,
        nodes[2].ptr() as usize,
    ];
    let mut census_global = census_object.as_mut_ptr().cast::<u8>();
    let _global = fake_data(0x00B4_3D74, (&mut census_global as *mut *mut u8).cast());
    let mut object = Object::new(0xE4);
    let mut header = [0usize; 6];
    let header_ptr = header.as_mut_ptr();
    header[2] = header_ptr as usize;
    std::hint::black_box(&header);
    // SAFETY: the map is empty and all census and demand methods used by this port are installed.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_construction(nodes[0].ptr());
        sim.set_developers(header_ptr.cast());
        sim.set_resource_min(8192);
        sim.set_resource_max(20480);
        sim.set_resource_fraction(0.5);
        for (r, c, i, expected) in [
            (30000.0, 12000.5, 9000.25, 20480.0f32),
            (100.0, 0.0, 0.0, 8192.0),
            (20001.0, 0.0, 0.0, 10000.0),
            (f64::NAN, 0.0, 0.0, 8192.0),
            (-100.0, 0.0, 0.0, 20480.0),
        ] {
            sim.set_state(0);
            nodes[0].supply = r;
            nodes[1].supply = c;
            nodes[2].supply = i;
            assert_eq!(fc4p_00696130(sim.as_ptr()), header_ptr as u32);
            assert_eq!(sim.state(), 1);
            assert_eq!(
                events(),
                [
                    Event::Census(0x1001, 0),
                    Event::Supply(nodes[0].ptr() as usize),
                    Event::Census(0x3001, 0),
                    Event::Supply(nodes[1].ptr() as usize),
                    Event::Census(0x4001, 0),
                    Event::Supply(nodes[2].ptr() as usize),
                    Event::SetSupply(expected.to_bits())
                ]
            );
            assert_eq!(fc4p_00696130(sim.as_ptr()), 1);
            assert!(events().is_empty());
        }
        sim.set_resource_min(0);
        sim.set_resource_max(u32::MAX);
        sim.set_resource_fraction(1.0);
        nodes[0].supply = 16_777_216.0;
        nodes[1].supply = -16_777_216.0;
        nodes[2].supply = 1.0;
        std::hint::black_box(&nodes);
        sim.set_state(0);
        fc4p_00696130(sim.as_ptr());
        assert_eq!(events().last(), Some(&Event::SetSupply(1.0f32.to_bits())));
    }
}

#[test]
fn monthly_developers_follow_tree_successors() {
    let _fpu = GameFpu::new();
    let mut table = node_table();
    table[18] = set_supply as *const () as usize;
    let mut node = Node {
        table: table.as_ptr(),
        supply: 0.0,
        demand: 0.0,
    };
    let mut census_table = [0; 7];
    census_table[6] = census as *const () as usize;
    let mut census_object = [
        census_table.as_ptr() as usize,
        node.ptr() as usize,
        node.ptr() as usize,
        node.ptr() as usize,
    ];
    let mut census_global = census_object.as_mut_ptr().cast::<u8>();
    let _global = fake_data(0x00B4_3D74, (&mut census_global as *mut *mut u8).cast());
    let mut developer_table = [0; 28];
    developer_table[27] = monthly_developer as *const () as usize;
    let mut developers = [[developer_table.as_ptr() as usize]; 4];
    let mut tree = [[0usize; 6]; 5];
    let base = tree.as_mut_ptr();
    let ptr = |i: usize| base.wrapping_add(i) as usize;
    // Header, root(2), left(1), right(3), and root.right.left(4): visit 1,2,4,3.
    tree[0][1] = ptr(2);
    tree[0][2] = ptr(1);
    tree[0][3] = ptr(3);
    tree[1][1] = ptr(2);
    tree[2][1] = ptr(0);
    tree[2][2] = ptr(1);
    tree[2][3] = ptr(3);
    tree[3][1] = ptr(2);
    tree[3][2] = ptr(4);
    tree[4][1] = ptr(3);
    for i in 1..5 {
        tree[i][5] = developers[i - 1].as_mut_ptr() as usize;
    }
    let mut object = Object::new(0xE4);
    // SAFETY: the map links form a valid four-node tree and each developer has its update slot.
    unsafe {
        let sim = BuildingSim::from_ptr(object.ptr());
        sim.set_construction(node.ptr());
        sim.set_developers(base.cast());
        fc4p_00696130(sim.as_ptr());
        let trace = events();
        let updates: Vec<_> = trace.into_iter().filter(|e| matches!(e, Event::Developer(_))).collect();
        assert_eq!(
            updates,
            [0, 1, 3, 2].map(|i| Event::Developer(developers[i].as_ptr() as usize))
        );
        assert_eq!(sim.state(), 1);
    }
}
