// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Includes(usize, i32, i32),
    Place(usize, [u32; 8]),
    Last(usize, [u32; 5]),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    fail_first: usize,
    fail_all: bool,
    fail_cell: Option<(i32, i32)>,
    placed: usize,
    dev: usize,
    change_mode: bool,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}

extern "thiscall" fn includes_fake(this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Includes(this as usize, x, z)));
    0xBEEF_0000 | u32::from(unsafe { Region::from_ptr(this).includes(x, z) })
}

extern "thiscall" fn place_fake(
    this: *mut u8,
    x: i32,
    z: i32,
    x2: i32,
    z2: i32,
    mask: u32,
    callback: *mut u8,
    real: u32,
    last: u32,
) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Place(
            this as usize,
            [
                x as u32,
                z as u32,
                x2 as u32,
                z2 as u32,
                mask,
                callback as u32,
                real,
                last,
            ],
        ));
        let failure = s.fail_all
            || s.placed < s.fail_first
            || s.fail_cell
                .is_some_and(|(cx, cz)| cx >= x.min(x2) && cx <= x.max(x2) && cz >= z.min(z2) && cz <= z.max(z2));
        s.placed += 1;
        if s.change_mode {
            s.change_mode = false;
            unsafe { ZoneDeveloper::from_ptr(s.dev as *mut u8) }.set_real_mode(0);
        }
        0xABCD_EF00 | if failure { 0 } else { 0x80 }
    })
}

extern "thiscall" fn last_fake(this: *mut u8, x: u32, z: u32, x2: u32, z2: u32, mask: u32) {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Last(this as usize, [x, z, x2, z2, mask]))
    });
}

struct World {
    dev: Object,
    tool: Box<[usize; 1]>,
    _table: Box<[usize; 8]>,
    region: Box<[u32; 9]>,
    _columns: Box<[*mut u32]>,
    _rows: Vec<Box<[u32]>>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new(missing: &[(i32, i32)]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut table = Box::new([0usize; 8]);
        table[7] = place_fake as *const () as usize;
        let mut tool = Box::new([table.as_ptr() as usize]);
        let mut rows: Vec<_> = (0..6).map(|_| vec![1u32].into_boxed_slice()).collect();
        let mut columns = rows
            .iter_mut()
            .map(|r| r.as_mut_ptr())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut region = Box::new([0u32; 9]);
        let r = unsafe { Region::from_ptr(region.as_mut_ptr().cast()) };
        r.set_x1(10);
        r.set_z1(5);
        r.set_x2(15);
        r.set_z2(5);
        r.set_width(6);
        r.set_height(1);
        r.set_words(1);
        r.set_columns(columns.as_mut_ptr());
        for &(x, z) in missing {
            unsafe { r.set(x.wrapping_sub(10) as u32, z.wrapping_sub(5) as u32, false) };
        }
        let mut dev = Object::new(0xEC);
        let d = unsafe { ZoneDeveloper::from_ptr(dev.ptr()) };
        d.set_restriction(region.as_mut_ptr().cast());
        d.set_street_tool(tool.as_mut_ptr().cast());
        d.set_real_mode(1);
        STATE.with(|s| s.borrow_mut().dev = dev.ptr() as usize);
        Self {
            dev,
            tool,
            _table: table,
            region,
            _columns: columns,
            _rows: rows,
            _fakes: vec![
                fake(0x0046_6610, includes_fake as *const () as usize),
                fake(0x0072_A0D0, last_fake as *const () as usize),
            ],
        }
    }
    fn run(&mut self, a: (i32, i32), b: (i32, i32), per_cell: u32) -> Vec<Call> {
        let before = unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0xEC) }.to_vec();
        unsafe { fc4p_0072a2a0(self.dev.ptr(), a.0, a.1, b.0, b.1, per_cell) };
        if !STATE.with(|s| s.borrow().change_mode) && before[0x99] == 0 {
            assert_eq!(unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0xEC) }, before);
        }
        let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
        for call in &calls {
            match call {
                Call::Place(this, args) => {
                    assert_eq!(*this, self.tool.as_ptr() as usize);
                    assert_eq!(args[4], 0xC49);
                    assert_eq!(args[5], self.tool.as_ptr() as u32);
                    assert_eq!(args[7], u32::MAX);
                }
                Call::Last(this, args) => {
                    assert_eq!(*this, self.dev.ptr() as usize);
                    assert_eq!(args[4], 0xC49);
                }
                Call::Includes(this, ..) => assert_eq!(*this, self.region.as_ptr() as usize),
            }
        }
        calls
    }
}

fn segments(calls: &[Call]) -> Vec<[i32; 4]> {
    calls
        .iter()
        .filter_map(|c| match c {
            Call::Place(_, a) => Some([a[0] as i32, a[1] as i32, a[2] as i32, a[3] as i32]),
            _ => None,
        })
        .collect()
}

#[test]
fn clipped_runs_and_per_cell_placements_follow_input_direction() {
    for (a, b, per_cell, expected) in [
        (
            9,
            16,
            1,
            vec![
                [10, 5, 11, 5],
                [10, 5, 10, 5],
                [11, 5, 11, 5],
                [13, 5, 15, 5],
                [13, 5, 13, 5],
                [14, 5, 14, 5],
                [15, 5, 15, 5],
            ],
        ),
        (
            10,
            13,
            1,
            vec![[10, 5, 11, 5], [10, 5, 10, 5], [11, 5, 11, 5], [13, 5, 13, 5]],
        ),
        (13, 10, 0, vec![[13, 5, 13, 5], [11, 5, 10, 5]]),
        (10, 15, 0x100, vec![[10, 5, 11, 5], [13, 5, 15, 5]]),
    ] {
        let mut w = World::new(&[(12, 5)]);
        let c = w.run((a, 5), (b, 5), per_cell);
        assert_eq!(segments(&c), expected);
        assert!(!c.iter().any(|c| matches!(c, Call::Includes(..))));
    }
}

#[test]
fn single_cell_calls_the_region_helper_and_places_only_when_included() {
    for (x, included) in [(9, false), (10, true), (12, false), (15, true), (16, false)] {
        let mut w = World::new(&[(12, 5)]);
        let c = w.run((x, 5), (x, 5), 1);
        assert_eq!(c[0], Call::Includes(w.region.as_ptr() as usize, x, 5));
        assert_eq!(segments(&c), if included { vec![[x, 5, x, 5]] } else { vec![] });
    }
}

#[test]
fn segment_fallbacks_use_the_whole_line_end_and_short_circuit_on_success() {
    let mut w = World::new(&[]);
    STATE.with(|s| s.borrow_mut().fail_cell = Some((15, 5)));
    assert_eq!(
        segments(&w.run((10, 5), (15, 5), 0)),
        [[10, 5, 15, 5], [11, 5, 15, 5], [10, 5, 14, 5]]
    );
    let mut w = World::new(&[(12, 5)]);
    STATE.with(|s| s.borrow_mut().fail_first = 3);
    assert_eq!(
        segments(&w.run((10, 5), (15, 5), 0)),
        [
            [10, 5, 11, 5],
            [11, 5, 15, 5],
            [10, 5, 14, 5],
            [11, 5, 14, 5],
            [13, 5, 15, 5]
        ]
    );
}

#[test]
fn final_singleton_uses_the_developer_helper_for_the_fourth_failed_attempt() {
    let mut w = World::new(&[(12, 5)]);
    STATE.with(|s| s.borrow_mut().fail_all = true);
    let c = w.run((12, 5), (13, 5), 0);
    assert_eq!(segments(&c), [[13, 5, 13, 5], [14, 5, 13, 5], [13, 5, 12, 5]]);
    assert_eq!(c.last(), Some(&Call::Last(w.dev.ptr() as usize, [14, 5, 12, 5, 0xC49])));
}

#[test]
fn per_cell_mode_skips_single_run_segments_and_reloads_real_mode_after_callbacks() {
    let mut w = World::new(&[(11, 5), (12, 5), (14, 5)]);
    unsafe { ZoneDeveloper::from_ptr(w.dev.ptr()) }.set_real_mode(0x80);
    STATE.with(|s| s.borrow_mut().change_mode = true);
    let c = w.run((10, 5), (15, 5), 0x80);
    assert_eq!(segments(&c), [[10, 5, 10, 5], [13, 5, 13, 5], [15, 5, 15, 5]]);
    let modes: Vec<_> = c
        .iter()
        .filter_map(|c| match c {
            Call::Place(_, a) => Some(a[6]),
            _ => None,
        })
        .collect();
    assert_eq!(modes, [0x80, 0, 0]);
}
