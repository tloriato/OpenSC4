// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The core's jog and parallel-draft examples, with ordered calls on the original object layout.

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Exists(i32, i32, u32),
    Connection(i32, i32, u32, i32, u32),
    Network(i32, i32, u32),
    Edge(u32, u32),
    Draft(u32, i32, i32, u32),
    Draw([i32; 4], u32),
    Height,
    Width,
}

#[derive(Default)]
struct State {
    dev: usize,
    city: usize,
    occupant: usize,
    calls: Vec<Call>,
    exists: Vec<(i32, i32, u32)>,
    connections: Vec<(i32, i32, u32, i32, u32)>,
    real: Vec<(i32, i32, u32)>,
    edges: Vec<(u32, u32)>,
    drafts: Vec<(u32, i32, i32, u32)>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn exists(this: *mut u8, x: i32, z: i32, flags: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Exists(x, z, flags));
        0xFFFF_FF00 | u32::from(s.exists.contains(&(x, z, flags)))
    })
}

extern "thiscall" fn connection(this: *mut u8, x: i32, z: i32, flags: u32, side: i32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Connection(x, z, flags, side, kind));
        0xFFFF_FF00 | u32::from(s.connections.contains(&(x, z, flags, side, kind)))
    })
}

extern "thiscall" fn network(this: *mut u8, x: i32, z: i32, flags: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Network(x, z, flags));
        if s.real.contains(&(x, z, flags)) {
            s.occupant as *mut u8
        } else {
            core::ptr::null_mut()
        }
    })
}

extern "thiscall" fn edge(this: *mut u8, edge: u32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.occupant);
        s.calls.push(Call::Edge(edge, kind));
        0xFFFF_FF00 | u32::from(s.edges.contains(&(edge, kind)))
    })
}

unsafe extern "thiscall" fn draft(this: *mut u8, x: i32, z: i32, flags: u32, mask: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let kind = unsafe { *this.add(4).cast::<u32>() };
        assert_eq!(flags, if kind == 3 { 8 } else { 0x800 });
        s.calls.push(Call::Draft(kind, x, z, flags));
        if let Some((_, _, _, bits)) = s.drafts.iter().find(|&&(k, a, b, _)| (k, a, b) == (kind, x, z)) {
            unsafe {
                *mask = *bits;
            }
            0xFFFF_FF01
        } else {
            0xFFFF_FF00
        }
    })
}

extern "thiscall" fn draw(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, per_cell: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Draw([x1, z1, x2, z2], per_cell));
    });
}

extern "thiscall" fn height(this: *mut u8) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.city);
        s.calls.push(Call::Height);
    });
    64
}

extern "thiscall" fn width(this: *mut u8) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.city);
        s.calls.push(Call::Width);
    });
    64
}

fn run(rect: [i32; 4], real: u8, mut state: State) -> Vec<Call> {
    let mut dev = Object::new(0x100);
    let mut vt = [0usize; 9];
    vt[8] = exists as *const () as usize;
    let mut city_vt = [0usize; 95];
    city_vt[0x174 / 4] = width as *const () as usize;
    city_vt[0x178 / 4] = height as *const () as usize;
    let mut city = [city_vt.as_ptr() as usize];
    let mut occupant_vt = [0usize; 48];
    occupant_vt[0xBC / 4] = edge as *const () as usize;
    let mut occupant = [occupant_vt.as_ptr() as usize];
    let mut tool_vt = [0usize; 10];
    tool_vt[9] = draft as *const () as usize;
    let mut street = [tool_vt.as_ptr() as u32, 3];
    let mut dirt = [tool_vt.as_ptr() as u32, 11];
    unsafe {
        let view = ZoneDeveloper::from_ptr(dev.ptr());
        view.set_primary_vtable(vt.as_ptr() as u32);
        view.set_city(city.as_mut_ptr().cast());
        view.set_street_tool(street.as_mut_ptr().cast());
        view.set_dirt_tool(dirt.as_mut_ptr().cast());
        view.set_real_mode(real);
        // These cached dimensions must not replace the city's virtual calls.
        view.set_cells_x(1);
        view.set_cells_z(1);
    }
    state.dev = dev.ptr() as usize;
    state.city = city.as_ptr() as usize;
    state.occupant = occupant.as_mut_ptr() as usize;
    STATE.with(|s| *s.borrow_mut() = state);
    let mut flags = [0; 12];
    flags[3] = 8;
    let _flags = fake_data(0x00AB_3680, flags.as_mut_ptr().cast());
    let _connection = fake(0x0072_FE80, connection as *const () as usize);
    let _network = fake(0x0072_FCA0, network as *const () as usize);
    let _draw = fake(0x0072_A2A0, draw as *const () as usize);
    let before = unsafe { core::slice::from_raw_parts(dev.ptr(), 0x100).to_vec() };
    unsafe {
        fc4p_00730d30(dev.ptr(), rect.as_ptr(), 3);
    }
    assert_eq!(before, unsafe { core::slice::from_raw_parts(dev.ptr(), 0x100) });
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn core_jog_example_has_the_complete_call_order() {
    let calls = run(
        [10, 10, 15, 13],
        1,
        State {
            exists: vec![(12, 9, 8)],
            real: vec![(12, 9, 8)],
            connections: vec![(12, 9, 8, 0, 3)],
            ..State::default()
        },
    );
    let mut expected = vec![];
    for x in 10..=15 {
        expected.push(Call::Exists(x, 9, 8));
        if x == 12 {
            for side in 0..4 {
                expected.push(Call::Connection(x, 9, 8, side, 3));
            }
            for flags in [0x40, 1, 0x400, 8, 0x800] {
                expected.push(Call::Exists(x, 9, flags));
                if flags == 8 {
                    expected.extend([
                        Call::Network(x, 9, 8),
                        Call::Edge(0, 3),
                        Call::Network(x, 9, 8),
                        Call::Edge(2, 3),
                    ]);
                }
            }
        }
    }
    expected.push(Call::Draw([10, 10, 15, 10], 1));
    expected.push(Call::Height);
    for x in 10..=15 {
        expected.push(Call::Exists(x, 14, 8));
    }
    for z in 10..=13 {
        expected.push(Call::Exists(9, z, 8));
    }
    expected.push(Call::Width);
    for z in 10..=13 {
        expected.push(Call::Exists(16, z, 8));
    }
    assert_eq!(calls, expected);
}

#[test]
fn four_outside_lines_use_their_wanted_sides_and_draw_in_order() {
    let cells = [(11, 9, 0), (11, 13, 2), (9, 11, 3), (13, 11, 1)];
    let calls = run(
        [10, 10, 12, 12],
        1,
        State {
            exists: cells.iter().map(|&(x, z, _)| (x, z, 8)).collect(),
            connections: cells.iter().map(|&(x, z, side)| (x, z, 8, side, 3)).collect(),
            ..State::default()
        },
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| matches!(c, Call::Draw(..)))
            .cloned()
            .collect::<Vec<_>>(),
        [
            Call::Draw([10, 10, 12, 10], 1),
            Call::Draw([10, 12, 12, 12], 1),
            Call::Draw([10, 10, 10, 12], 1),
            Call::Draw([12, 10, 12, 12], 1),
        ]
    );
}

#[test]
fn each_parallel_kind_and_either_edge_blocks_and_stops_the_line() {
    for (flags, kind) in [(0x40, 6), (1, 0), (0x400, 10), (8, 3), (0x800, 11)] {
        for (x, z, side, edges) in [(11, 9, 0, [0, 2]), (9, 11, 3, [1, 3])] {
            for edge in edges {
                let calls = run(
                    [10, 10, 12, 12],
                    1,
                    State {
                        exists: vec![(x, z, 8), (x, z, flags)],
                        real: vec![(x, z, flags)],
                        connections: vec![(x, z, 8, side, 3)],
                        edges: vec![(edge, kind)],
                        ..State::default()
                    },
                );
                assert!(!calls.iter().any(|c| matches!(c, Call::Draw(..))));
                assert!(calls.contains(&Call::Edge(edge, kind)));
                assert!(!calls.contains(&Call::Exists(
                    if x == 11 { 12 } else { x },
                    if z == 11 { 12 } else { z },
                    8
                )));
                let gets = calls.iter().filter(|c| **c == Call::Network(x, z, flags)).count();
                assert_eq!(gets, if edge == edges[0] { 1 } else { 2 });
            }
        }
    }
}

#[test]
fn highlight_parallel_draft_uses_street_then_dirt_and_selected_mask_byte() {
    for (x, z, side, edge) in [(11, 9, 0, 0), (11, 9, 0, 2), (9, 11, 3, 1), (9, 11, 3, 3)] {
        for source in [3, 11] {
            let state = || State {
                exists: vec![(x, z, 8)],
                connections: vec![(x, z, 8, side, 3)],
                drafts: vec![(source, x, z, 1 << (edge * 8))],
                ..State::default()
            };
            let calls = run([10, 10, 12, 12], 0, state());
            assert!(!calls.iter().any(|c| matches!(c, Call::Draw(..))));
            let drafts = calls
                .iter()
                .filter(|c| matches!(c, Call::Draft(..)))
                .cloned()
                .collect::<Vec<_>>();
            let mut expected = vec![Call::Draft(3, x, z, 8)];
            if source == 11 {
                expected.push(Call::Draft(11, x, z, 0x800));
            }
            if edge >= 2 {
                expected.extend(expected.clone());
            }
            assert_eq!(drafts, expected);
            let real = run([10, 10, 12, 12], 1, state());
            assert!(real.iter().any(|c| matches!(c, Call::Draw(..))));
            assert!(!real.iter().any(|c| matches!(c, Call::Draft(..))));
        }
    }
}

#[test]
fn a_street_draft_with_no_parallel_edge_hides_the_dirt_draft() {
    let calls = run(
        [10, 10, 12, 12],
        0,
        State {
            exists: vec![(11, 9, 8)],
            connections: vec![(11, 9, 8, 0, 3)],
            drafts: vec![(3, 11, 9, 0), (11, 11, 9, 1)],
            ..State::default()
        },
    );
    assert!(calls.contains(&Call::Draw([10, 10, 12, 10], 1)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Draft(11, ..))));
}

#[test]
fn city_edges_empty_bounds_and_two_connections() {
    assert_eq!(run([0, 0, 63, 63], 1, State::default()), [Call::Height, Call::Width]);
    assert_eq!(run([2, 2, 1, 1], 1, State::default()), [Call::Height, Call::Width]);
    let calls = run(
        [10, 10, 12, 12],
        1,
        State {
            exists: vec![(11, 9, 8)],
            connections: vec![(11, 9, 8, 0, 3), (11, 9, 8, 1, 3)],
            ..State::default()
        },
    );
    assert!(!calls.contains(&Call::Connection(11, 9, 8, 2, 3)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Draw(..))));
}
