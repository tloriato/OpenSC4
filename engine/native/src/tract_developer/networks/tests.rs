// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Border enumeration, network fallbacks, and subdivision bounds.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::tract_developer::testing::{run, snapshot};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Grid,
    Bounds(u32, u32),
    LotBounds,
    Width,
    Height,
    Tile(i32, i32, u32, u32),
    Type(u32),
}
struct State {
    calls: Vec<Call>,
    grid: u32,
    tile: u32,
    rectangle: [u32; 4],
    lot_rectangle: [u32; 4],
    dimensions: [u32; 2],
    masks: BTreeMap<(i32, i32), u32>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            grid: 0,
            tile: 0,
            rectangle: [2, 3, 4, 5],
            lot_rectangle: [1, 2, 5, 6],
            dimensions: [8, 9],
            masks: BTreeMap::new(),
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
extern "thiscall" fn grid(_this: u32) -> u32 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Grid);
        s.grid
    })
}
extern "thiscall" fn bounds(this: u32, candidate: u32) {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Bounds(this, candidate));
        for i in 0..4 {
            unsafe {
                store(candidate, 0x38 + i * 4, s.rectangle[i as usize]);
            }
        }
    });
}
extern "thiscall" fn lot_bounds(_this: u32, rectangle: u32) {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::LotBounds);
        for i in 0..4 {
            unsafe {
                store(rectangle, i * 4, s.lot_rectangle[i as usize]);
            }
        }
    });
}
extern "thiscall" fn width(_this: u32) -> u32 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Width);
        s.dimensions[0]
    })
}
extern "thiscall" fn height(_this: u32) -> u32 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Height);
        s.dimensions[1]
    })
}
extern "thiscall" fn tile(_this: u32, x: i32, z: i32, kind: u32, flag: u32) -> u32 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Tile(x, z, kind, flag));
        match s.masks.get(&(x, z)) {
            Some(&mask) => {
                unsafe {
                    store(s.tile, 4, mask);
                }
                s.tile
            }
            None => 0,
        }
    })
}
extern "thiscall" fn is_type(this: u32, kind: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Type(kind)));
    0xABCD_0000 | u32::from(unsafe { word(this, 4) } & (1 << kind) != 0)
}

struct World {
    td: Object,
    candidate: Object,
    services: Box<[u32; 5]>,
    tile: Box<[u32; 2]>,
    counts: Box<[[u32; 4]; 4]>,
    tables: [Box<[usize; 96]>; 5],
    traffic: Box<u32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180),
            candidate: Object::new(0xA4),
            services: Box::new([0; 5]),
            tile: Box::new([0; 2]),
            counts: Box::new([[u32::MAX; 4]; 4]),
            tables: core::array::from_fn(|_| Box::new([0; 96])),
            traffic: Box::new(0),
            _fakes: Vec::new(),
        };
        for (i, slot, function) in [
            (0, 0xA0, grid as *const () as usize),
            (1, 0x20, tile as *const () as usize),
            (2, 0x174, width as *const () as usize),
            (2, 0x178, height as *const () as usize),
            (3, 0x38, lot_bounds as *const () as usize),
            (4, 0x58, is_type as *const () as usize),
        ] {
            w.tables[i][slot / 4] = function;
        }
        for i in 0..5 {
            w.services[i] = w.tables[i].as_ptr() as u32;
        }
        w.tile[0] = w.tables[4].as_ptr() as u32;
        *w.traffic = w.services.as_ptr() as u32;
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_city(w.services.as_mut_ptr().wrapping_add(2) as u32);
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        c.set_kind(2);
        c.set_lot(w.services.as_mut_ptr().wrapping_add(3).cast());
        for i in 0..4 {
            unsafe {
                store(c.as_ptr() as u32, 0x70 + i * 12, w.counts[i as usize].as_ptr() as u32);
            }
        }
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.grid = w.services.as_ptr().wrapping_add(1) as u32;
            s.tile = w.tile.as_ptr() as u32;
        });
        w._fakes
            .push(fake_data(0x00B4_3CEC, (&mut *w.traffic as *mut u32).cast()));
        w._fakes.push(fake(0x0070_4630, bounds as *const () as usize));
        w
    }
    fn check(&mut self, subdivision: bool) {
        let td = self.td.ptr() as u32;
        let p = self.candidate.ptr() as u32;
        unsafe {
            store(p, 0, if subdivision { 4 } else { 2 });
        }
        let before = snapshot(p as *mut u8, 0xA4);
        let td_before = snapshot(td as *mut u8, 0x180);
        let (rectangle, dimensions, masks) = STATE.with(|s| {
            let s = s.borrow();
            (
                if subdivision { s.lot_rectangle } else { s.rectangle },
                s.dimensions,
                s.masks.clone(),
            )
        });
        let [x0, z0, x1, z1] = rectangle.map(|v| v as i32);
        let [width, height] = dimensions.map(|v| v as i32);
        let mut calls = vec![Call::Grid, Call::Bounds(td, p)];
        if subdivision {
            calls.push(Call::LotBounds);
        }
        let mut expected = [[u32::MAX; 4]; 4];
        let mut visit = |side: usize, x, z, calls: &mut Vec<Call>| {
            calls.push(Call::Tile(x, z, 0xC4B, 1));
            if let Some(&mask) = masks.get(&(x, z)) {
                for (index, first, alternate) in [(0, 3, Some(11)), (1, 0, Some(10)), (2, 6, None), (3, 1, None)] {
                    calls.push(Call::Type(first));
                    let mut found = mask & (1 << first) != 0;
                    if !found {
                        if let Some(other) = alternate {
                            calls.push(Call::Type(other));
                            found = mask & (1 << other) != 0;
                        }
                    }
                    if found {
                        expected[index][side] = expected[index][side].wrapping_add(1);
                    }
                }
            }
        };
        if z0 > 0 {
            for x in x0..=x1 {
                visit(0, x, z0 - 1, &mut calls);
            }
        }
        calls.push(Call::Width);
        if x1 < width.wrapping_sub(1) {
            for z in z0..=z1 {
                visit(1, x1.wrapping_add(1), z, &mut calls);
            }
        }
        calls.push(Call::Height);
        if z1 < height.wrapping_sub(1) {
            for x in x0..=x1 {
                visit(2, x, z1.wrapping_add(1), &mut calls);
            }
        }
        if x0 > 0 {
            for z in z0..=z1 {
                visit(3, x0 - 1, z, &mut calls);
            }
        }
        let result = run(0x0070_5040, Conv::Thiscall, td, &[p]);
        assert_eq!(*self.counts, expected);
        assert_eq!(STATE.with(|s| s.borrow().calls.clone()), calls);
        let mut expected_candidate = before;
        STATE.with(|s| {
            for i in 0..4 {
                expected_candidate[0x38 + i * 4..0x3C + i * 4].copy_from_slice(&s.borrow().rectangle[i].to_le_bytes());
            }
        });
        assert_eq!(snapshot(p as *mut u8, 0xA4), expected_candidate);
        assert_eq!(snapshot(td as *mut u8, 0x180), td_before);
        let expected_eax = if x0 <= 0 {
            x0
        } else if z0 > z1 {
            z0
        } else {
            z1
        };
        assert_eq!(result.0, expected_eax as u32);
    }
}

#[test]
fn scans_all_sides_with_short_circuited_fallbacks_and_wrapping_counts() {
    let mut w = World::new();
    let masks = [
        0,
        1 << 3,
        (1 << 3) | (1 << 11),
        1 << 11,
        1,
        1 | (1 << 10),
        1 << 10,
        1 << 6,
        1 << 1,
        u32::MAX,
    ];
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        for x in 0..8 {
            for z in 0..9 {
                s.masks.insert((x, z), masks[(x * 9 + z) as usize % masks.len()]);
            }
        }
    });
    w.check(false);
}

#[test]
fn subdivision_uses_original_lot_bounds_after_caching_candidate_bounds() {
    let mut w = World::new();
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        for x in 0..8 {
            for z in 0..9 {
                s.masks.insert((x, z), u32::MAX);
            }
        }
    });
    w.check(true);
}

#[test]
fn city_edges_inverted_rectangles_and_negative_coordinates() {
    for rectangle in [
        [0, 0, 7, 8],
        [0, 3, 0, 5],
        [2, 0, 4, 0],
        [7, 8, 7, 8],
        [4, 5, 2, 3],
        [-2, -3, 1, 2],
        [3, 5, 2, 7],
        [2, 7, 4, 5],
    ] {
        let mut w = World::new();
        STATE.with(|s| s.borrow_mut().rectangle = rectangle.map(|v| v as u32));
        w.check(false);
    }
}
