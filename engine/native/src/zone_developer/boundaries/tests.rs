// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    owner: usize,
    calls: Vec<(usize, i32, i32)>,
    cells: Vec<(i32, i32, i32, i32)>,
    forced_scores: Option<[i32; 4]>,
    replacement: usize,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn cell(this: *mut u8, x: i32, z: i32, condition: *mut i32, score: *mut i32) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert_eq!(this as usize, state.owner);
        let side = (score as usize - this as usize - 0xAC) / 4;
        assert!(side < 4);
        assert_eq!(condition as usize, this as usize + 0xBC + 4 * side);
        let dev = unsafe { ZoneDeveloper::from_ptr(this) };
        if state.calls.is_empty() {
            assert_eq!(dev.scores(), [0; 4]);
            assert_eq!(dev.conditions(), [i32::from(dev.place_streets() == 0); 4]);
        }
        state.calls.push((side, x, z));
        if let Some(scores) = state.forced_scores {
            dev.set_scores(scores);
        } else {
            unsafe {
                if let Some(&(_, _, increment, value)) = state.cells.iter().find(|&&(cx, cz, _, _)| cx == x && cz == z)
                {
                    *condition = value;
                    *score = (*score).wrapping_add(increment);
                }
                if *condition == 0 {
                    *score = (*score).wrapping_add(1);
                }
            }
        }
        if state.replacement != 0 {
            dev.set_restriction(state.replacement as *mut u8);
        }
    });
}

struct Fixture {
    object: Object,
    region: Object,
    columns: Vec<Vec<u32>>,
    pointers: Vec<*mut u32>,
}

impl Fixture {
    fn new(rect: [i32; 4], streets: u8, alternate: u8, zone: u32) -> Self {
        let [x0, z0, x1, z1] = rect;
        let count = x1.wrapping_sub(x0).wrapping_add(1).max(1) as usize;
        let words = (z1.wrapping_sub(z0).max(0) as usize / 32) + 1;
        let mut columns = vec![vec![u32::MAX; words]; count];
        let mut pointers: Vec<_> = columns.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let mut region = Object::new(0x24);
        let r = unsafe { BoundaryRegion::from_ptr(region.ptr()) };
        r.set_x_min(x0);
        r.set_z_min(z0);
        r.set_x_max(x1);
        r.set_z_max(z1);
        r.set_columns(pointers.as_mut_ptr());
        let mut object = Object::new(0x100);
        unsafe { core::ptr::write_bytes(object.ptr(), 0xA5, 0x100) };
        let dev = unsafe { ZoneDeveloper::from_ptr(object.ptr()) };
        dev.set_cells_x(64);
        dev.set_cells_z(64);
        dev.set_restriction(region.ptr());
        dev.set_place_streets(streets);
        dev.set_alternate_orientation(alternate);
        dev.set_current_zone(zone);
        STATE.with(|s| {
            *s.borrow_mut() = State {
                owner: object.ptr() as usize,
                ..State::default()
            }
        });
        Self {
            object,
            region,
            columns,
            pointers,
        }
    }

    fn dev(&mut self) -> ZoneDeveloper {
        unsafe { ZoneDeveloper::from_ptr(self.object.ptr()) }
    }

    fn run(&mut self, scores: [i32; 4], conditions: [i32; 4], flag: u8, d: u32, q: u32, result: (u32, u32)) {
        let _fake = fake(0x0072_FAB0, cell as *const () as usize);
        let dev = self.dev();
        let mut expected = unsafe { core::slice::from_raw_parts(dev.as_ptr(), 0x100) }.to_vec();
        expected[0x98] = flag;
        for (offset, values) in [(0xAC, scores), (0xBC, conditions)] {
            for (i, value) in values.into_iter().enumerate() {
                expected[offset + 4 * i..offset + 4 * i + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        expected[0xCC..0xD0].copy_from_slice(&q.to_le_bytes());
        expected[0xD0..0xD4].copy_from_slice(&d.to_le_bytes());
        let actual =
            unsafe { call_thiscall_with_eax(fc4p_00731c80 as *const () as usize, dev.as_ptr(), &[], 0x1234_5678) };
        assert_eq!(actual, result);
        assert_eq!(dev.scores(), scores);
        assert_eq!(dev.conditions(), conditions);
        assert_eq!(
            (dev.orientation_flag(), dev.orientation_d(), dev.orientation_q()),
            (flag, d, q)
        );
        if STATE.with(|s| s.borrow().replacement != 0) {
            expected[0xA4..0xA8].copy_from_slice(&(dev.restriction() as u32).to_le_bytes());
        }
        assert_eq!(unsafe { core::slice::from_raw_parts(dev.as_ptr(), 0x100) }, expected);
        assert_eq!(
            unsafe { BoundaryRegion::from_ptr(self.region.ptr()) }.columns(),
            self.pointers.as_mut_ptr()
        );
    }
}

#[test]
fn road_and_street_boundaries_with_residential_and_alternate_swaps() {
    for (streets, alternate, zone, scores, conditions, flag, d, q, result) in [
        (1, 0, 4, [300, 90, 3, 3], [1, 1, 0, 0], 1, 0, 1, (0, 90)),
        (1, 0, 1, [300, 90, 3, 3], [1, 1, 0, 0], 0, 1, 0, (0, 3)),
        (1, 1, 4, [300, 90, 3, 3], [1, 1, 0, 0], 0, 1, 0, (1, 0)),
        (1, 1, 1, [300, 90, 3, 3], [1, 1, 0, 0], 1, 0, 1, (0, 1)),
        (0, 0, 4, [300, 90, 0, 0], [1; 4], 1, 0, 1, (0, 90)),
    ] {
        let mut fixture = Fixture::new([10, 10, 12, 12], streets, alternate, zone);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            for x in 10..=12 {
                s.cells.push((x, 9, 100, 1));
            }
            for z in 10..=12 {
                s.cells.push((13, z, 30, 1));
            }
        });
        fixture.run(scores, conditions, flag, d, q, result);
        assert_eq!(
            STATE.with(|s| s.borrow().calls.clone()),
            [
                (0, 10, 9),
                (2, 10, 13),
                (0, 11, 9),
                (2, 11, 13),
                (0, 12, 9),
                (2, 12, 13),
                (3, 9, 10),
                (1, 13, 10),
                (3, 9, 11),
                (1, 13, 11),
                (3, 9, 12),
                (1, 13, 12),
            ]
        );
    }
}

#[test]
fn city_borders_examine_the_regions_own_cells_and_skip_missing_bits() {
    for (rect, streets, zone, scores, conditions, flag, d, q, result) in [
        ([0, 0, 2, 2], 1, 4, [3, 300, 2, 2], [0, 1, 0, 0], 1, 1, 0, (0, 2)),
        ([0, 0, 2, 2], 0, 4, [0, 300, 0, 0], [1; 4], 1, 1, 2, (0, 0)),
        ([61, 61, 63, 63], 1, 4, [300, 3, 2, 2], [1, 0, 0, 0], 1, 0, 1, (0, 3)),
        ([61, 61, 63, 63], 1, 1, [300, 3, 2, 2], [1, 0, 0, 0], 0, 1, 0, (0, 3)),
    ] {
        let mut fixture = Fixture::new(rect, streets, 0, zone);
        fixture.columns[0][0] &= !4;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if rect[0] == 0 {
                for z in 0..=2 {
                    s.cells.push((3, z, 100, 1));
                }
            } else {
                for x in 61..=63 {
                    s.cells.push((x, 60, 100, 1));
                }
            }
        });
        fixture.run(scores, conditions, flag, d, q, result);
        let (x0, z0, x1, z1) = (rect[0], rect[1], rect[2], rect[3]);
        let outer_x0 = (x0 - 1).max(0);
        let outer_x1 = (x1 + 1).min(63);
        let outer_z0 = (z0 - 1).max(0);
        let outer_z1 = (z1 + 1).min(63);
        assert_eq!(
            STATE.with(|s| s.borrow().calls.clone()),
            [
                (0, x0, outer_z0),
                (0, x0 + 1, outer_z0),
                (2, x0 + 1, outer_z1),
                (0, x1, outer_z0),
                (2, x1, outer_z1),
                (3, outer_x0, z0),
                (1, outer_x1, z0),
                (3, outer_x0, z0 + 1),
                (1, outer_x1, z0 + 1),
                (1, outer_x1, z1),
            ]
        );
    }
}

#[test]
fn signed_scores_ties_and_minimum_score_sentinel_keep_register_bits() {
    for (scores, d, q, result) in [
        ([7; 4], 0, 3, (0, 7)),
        ([10, 3, 10, 3], 0, 3, (0, 3)),
        ([3, 10, 3, 10], 1, 2, (0, 3)),
        ([1, 2, 3, 0], 2, 1, (0, 2)),
        ([3, 2, 1, 10], 3, 0, (0, 1)),
        ([-9, -8, -7, -6], 3, 2, (0xFFFF_FF00, (-7i32) as u32)),
        ([i32::MIN; 4], u32::MAX, u32::MAX, (0x8000_0000, 0x8000_0000)),
    ] {
        let mut fixture = Fixture::new([10, 10, 10, 10], 0, 0, 0);
        STATE.with(|s| s.borrow_mut().forced_scores = Some(scores));
        fixture.run(scores, [1; 4], 1, d, q, result);
    }
}

#[test]
fn residential_swap_requires_both_dimensions_above_two_and_accepts_only_zones_one_and_two() {
    for (width, height, zone, swap, result) in [
        (2, 3, 1, false, (0, 2)),
        (3, 2, 2, false, (0, 3)),
        (3, 3, 2, true, (0, 3)),
        (3, 3, 0, false, (0, 1)),
        (3, 3, 3, false, (0, 1)),
        (3, 3, u32::MAX, false, (0xFFFF_FF00, 1)),
    ] {
        let mut fixture = Fixture::new([10, 10, 9 + width, 9 + height], 0x80, 0, zone);
        STATE.with(|s| s.borrow_mut().forced_scores = Some([10, 1, 0, 0]));
        fixture.run(
            [10, 1, 0, 0],
            [0; 4],
            u8::from(!swap),
            u32::from(swap),
            u32::from(!swap),
            result,
        );
    }
}

#[test]
fn multiword_columns_and_replaced_restriction_are_read_after_each_callback() {
    let mut fixture = Fixture::new([10, 10, 11, 43], 1, 0, 4);
    fixture.columns[0] = vec![1, 2];
    fixture.columns[1] = vec![0, 0];
    for (p, c) in fixture.pointers.iter_mut().zip(fixture.columns.iter_mut()) {
        *p = c.as_mut_ptr();
    }
    let mut replacement_column = [0u32, 2];
    let mut replacement_pointers = [replacement_column.as_mut_ptr(); 2];
    let mut replacement = Object::new(0x24);
    let r = unsafe { BoundaryRegion::from_ptr(replacement.ptr()) };
    r.set_columns(replacement_pointers.as_mut_ptr());
    STATE.with(|s| s.borrow_mut().replacement = replacement.ptr() as usize);
    fixture.run([1, 1, 2, 1], [0; 4], 1, 2, 3, (0, 1));
    assert_eq!(
        STATE.with(|s| s.borrow().calls.clone()),
        [(0, 10, 9), (2, 10, 44), (2, 11, 44), (3, 9, 43), (1, 12, 43),]
    );
}

#[test]
fn inverted_rectangle_does_not_read_columns_or_call_cells() {
    let mut fixture = Fixture::new([2, 2, 1, 1], 0, 0x80, 4);
    unsafe { BoundaryRegion::from_ptr(fixture.region.ptr()) }.set_columns(core::ptr::null_mut());
    fixture.pointers = Vec::new();
    let _fake = fake(0x0072_FAB0, cell as *const () as usize);
    let dev = fixture.dev();
    let result = unsafe { fc4p_00731c80(dev.as_ptr()) };
    assert_eq!((result as u32, (result >> 32) as u32), (3, 0));
    assert_eq!(
        (dev.orientation_flag(), dev.orientation_d(), dev.orientation_q()),
        (0, 3, 0)
    );
    assert_eq!(dev.scores(), [0; 4]);
    assert_eq!(dev.conditions(), [1; 4]);
    assert!(STATE.with(|s| s.borrow().calls.is_empty()));
}
