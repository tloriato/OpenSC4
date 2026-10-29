// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The core's street-extension example, all four edges, and connection short circuits.

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Exists(usize, i32, i32, u32),
    Connect(usize, i32, i32, u32, i32, u32),
    Draw(usize, [i32; 4], u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    networks: Vec<(i32, i32)>,
    connections: Vec<(i32, i32, i32, u32)>,
    shorten: Option<usize>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn exists(this: *mut u8, x: i32, z: i32, flags: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Exists(this as usize, x, z, flags));
        0xAABB_CC00 | u32::from(s.networks.contains(&(x, z)))
    })
}

extern "thiscall" fn connects(this: *mut u8, x: i32, z: i32, flags: u32, side: i32, kind: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Connect(this as usize, x, z, flags, side, kind));
        0xAABB_CC00 | u32::from(s.connections.contains(&(x, z, side, kind)))
    })
}

extern "thiscall" fn draw(this: *mut u8, x: i32, z: i32, ex: i32, ez: i32, per_cell: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Draw(this as usize, [x, z, ex, ez], per_cell));
        if let Some(rect) = s.shorten {
            unsafe { *(rect as *mut i32).add(3) = z; }
        }
    });
}

fn run(rect: &mut [i32; 4], state: State) -> (Vec<Call>, usize) {
    STATE.with(|s| *s.borrow_mut() = state);
    let mut obj = Object::new(0x100);
    let mut table = [0usize; 9];
    table[8] = exists as *const () as usize;
    unsafe { *obj.ptr().cast::<usize>() = table.as_ptr() as usize; }
    let mut flags = [0u32; 12];
    flags[3] = 0x1234_5678;
    let _data = fake_data(0x00AB_3680, flags.as_mut_ptr().cast());
    let _connect = fake(0x0072_FE80, connects as *const () as usize);
    let _draw = fake(0x0072_A2A0, draw as *const () as usize);
    unsafe { fc4p_00730ba0(obj.ptr(), rect.as_ptr(), 3); }
    (STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)), obj.ptr() as usize)
}

#[test]
fn extends_the_core_example_and_reads_the_flag_table() {
    let (calls, p) = run(&mut [10, 10, 15, 13], State {
        networks: vec![(12, 10)], connections: vec![(12, 10, 0, 0)], ..State::default()
    });
    let mut expected = vec![];
    for x in 10..=15 {
        for z in 10..=13 {
            expected.push(Call::Exists(p, x, z, 0x1234_5678));
            if (x, z) == (12, 10) {
                for side in 0..4 {
                    for kind in if side == 0 { &[0][..] } else { &[0, 10, 3, 11][..] } {
                        expected.push(Call::Connect(p, x, z, 0x1234_5678, side, *kind));
                    }
                }
                expected.push(Call::Draw(p, [12, 10, 12, 13], 0));
            }
        }
    }
    assert_eq!(calls, expected);
}

#[test]
fn all_sides_draw_to_the_opposite_edge_and_skip_zero_length() {
    for (side, end) in [(0, [11, 11, 11, 12]), (1, [11, 11, 10, 11]),
        (2, [11, 11, 11, 10]), (3, [11, 11, 12, 11])] {
        let state = || State { networks: vec![(11, 11)], connections: vec![(11, 11, side, 11)], ..State::default() };
        let (calls, p) = run(&mut [10, 10, 12, 12], state());
        assert_eq!(calls.iter().filter(|c| matches!(c, Call::Draw(..))).collect::<Vec<_>>(), [&Call::Draw(p, end, 0)]);
        let (calls, _) = run(&mut [11, 11, 11, 11], state());
        assert!(!calls.iter().any(|c| matches!(c, Call::Draw(..))));
    }
}

#[test]
fn avenue_only_on_odd_cells_and_stops_after_two_sides() {
    let (calls, p) = run(&mut [10, 10, 10, 12], State {
        networks: vec![(10, 10), (10, 11)],
        connections: vec![(10, 10, 0, 6), (10, 11, 0, 6), (10, 11, 1, 10)],
        ..State::default()
    });
    assert!(!calls.iter().any(|c| matches!(c, Call::Connect(_, 10, 10, _, _, 6))));
    assert!(calls.contains(&Call::Connect(p, 10, 11, 0x1234_5678, 0, 6)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Connect(_, 10, 11, _, 2.., _))));
    assert!(!calls.iter().any(|c| matches!(c, Call::Draw(..))));
}

#[test]
fn empty_bounds_make_no_calls_and_draw_changes_are_visible() {
    for mut rect in [[5, 3, 4, 3], [3, 5, 3, 4]] {
        assert!(run(&mut rect, State::default()).0.is_empty());
    }
    let mut rect = [10, 10, 10, 13];
    let state = State { networks: vec![(10, 10)], connections: vec![(10, 10, 0, 3)],
        shorten: Some(rect.as_mut_ptr() as usize), ..State::default() };
    let (calls, _) = run(&mut rect, state);
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Exists(..))).count(), 1);
    assert_eq!(rect[3], 10);
}
