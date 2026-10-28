// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::fake;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    allocation: usize,
    cells: Vec<(u32, usize)>,
    calls: Vec<(u32, Vec<u32>)>,
    replace: Option<(usize, usize, u32)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
extern "C" fn alloc(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push((1, vec![size]));
        s.allocation as *mut u8
    })
}
extern "C" fn copy(first: *mut u8, last: *mut u8, output: *mut u8, scratch: *mut u32) -> *mut u8 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push((2, vec![first as u32, last as u32, output as u32, unsafe { *scratch }]))
    });
    let len = (last as u32).wrapping_sub(first as u32) as usize;
    if !output.is_null() {
        unsafe {
            core::ptr::copy_nonoverlapping(first, output, len);
        }
    }
    output.wrapping_add(len)
}
extern "C" fn free(pointer: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push((3, vec![pointer as u32])));
}
extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push((4, vec![this as u32, key]));
        if let Some((location, value, new_end)) = s.replace.take() {
            unsafe {
                write(location as *mut u8, 0, value as u32);
                write(location as *mut u8, 4, new_end);
            }
        }
        s.cells.iter().find(|(k, _)| *k == key).unwrap().1 as *mut u8
    })
}

#[test]
fn tunnel_collection_appends_complete_valid_spans_and_sets_both_inclusive_endpoint_ranges() {
    let _f = fake(0x0063_3220, lookup as *const () as usize);
    let mut tool = [0u32; 0x368 / 4];
    let t = tool.as_mut_ptr().cast::<u8>();
    let mut spans = [
        [0x8000_0000u32, 0x7F80_0001, 0, 1],
        [0x1234, 0x5678, 2, 3],
        [9, 10, 0, 0],
    ];
    let begin = spans.as_mut_ptr().cast::<u8>();
    let mut steps = [[0u32, 1, 0xA5A5_A5A5], [2, 2, 0x5A5A_5A5A], [1, 0, 0], [2, 1, 0]];
    let mut points = [[0xFFFF_FFFFu32, 1], [8, 9], [10, 11]];
    let mut cells = [[0xA5A5_A5A5u32; 0x90 / 4]; 3];
    let cell_map = points
        .iter()
        .zip(cells.iter_mut())
        .map(|(p, c)| (p[1].wrapping_shl(16).wrapping_add(p[0]), c.as_mut_ptr() as usize))
        .collect();
    let mut output_data = [0xAAAA_AAAAu32; 20];
    let data = output_data.as_mut_ptr().cast::<u8>();
    let mut output = [data as u32, data as u32, data as u32 + 80];
    unsafe {
        write(t, 0xE4, begin);
        write(t, 0xE8, begin.add(48));
        write(t, 0x78, 3i32);
        write(t, 0x54, steps.as_mut_ptr());
        write(t, 0x60, points.as_mut_ptr());
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            cells: cell_map,
            ..State::default()
        }
    });
    assert_eq!(
        unsafe { fc4p_006274b0(t, output.as_mut_ptr().cast()) },
        begin as u32 + 48
    );
    assert_eq!(output, [data as u32, data as u32 + 16, data as u32 + 80]);
    assert_eq!(&output_data[..4], &[0, 1, 0x8000_0000, 0x7F80_0001]);
    assert_eq!(&output_data[4..], &[0xAAAA_AAAA; 16]);
    for (i, cell) in cells.iter().enumerate() {
        let mut expected = [0xA5A5_A5A5u32; 0x90 / 4];
        expected[0x58 / 4..0x88 / 4].fill(if i < 2 { 0x8000_0000 } else { 0x7F80_0001 });
        assert_eq!(*cell, expected);
    }
    STATE.with(|s| {
        assert_eq!(
            s.borrow().calls,
            [
                (4, vec![t as u32, 0xFFFF]),
                (4, vec![t as u32, 0x90008]),
                (4, vec![t as u32, 0xB000A]),
            ]
        )
    });
    // Exact equality to the valid-step limit rejects the entire span; a later valid span is not visited.
    unsafe {
        write(t, 0x78, 1i32);
    }
    let expected = output;
    STATE.with(|s| s.borrow_mut().calls.clear());
    unsafe {
        fc4p_006274b0(t, output.as_mut_ptr().cast());
    }
    assert_eq!(output, expected);
    STATE.with(|s| assert!(s.borrow().calls.is_empty()));
    unsafe {
        write(t, 0xE8, begin);
    }
    assert_eq!(unsafe { fc4p_006274b0(t, core::ptr::null_mut()) }, begin as u32);
}

#[test]
fn tunnel_append_doubles_full_storage_uses_game_copy_and_free_and_preserves_old_bytes() {
    let _f = [
        fake(0x0090_CF54, alloc as *const () as usize),
        fake(0x0063_7E20, copy as *const () as usize),
        fake(0x0090_CF63, free as *const () as usize),
    ];
    for old_count in [0usize, 1, 3] {
        let mut tool = [0u32; 0x368 / 4];
        let t = tool.as_mut_ptr().cast::<u8>();
        let mut span = [0xFF80_0000u32, 0x7FC0_0123, 0, 0];
        let sp = span.as_mut_ptr().cast::<u8>();
        let mut steps = [2u32, 1, 0xA5A5_A5A5];
        let mut previous = [0x1234_5678u32; 12];
        let previous_ptr = previous.as_mut_ptr().cast::<u8>();
        let mut allocated = [0xA5A5_A5A5u32; 24];
        let ap = allocated.as_mut_ptr().cast::<u8>();
        let old_begin = if old_count == 0 {
            core::ptr::null_mut()
        } else {
            previous_ptr
        };
        let old_end = old_begin.wrapping_add(old_count * 16);
        let mut output = [old_begin as u32, old_end as u32, old_end as u32];
        let out = output.as_mut_ptr().cast::<u8>();
        unsafe {
            write(t, 0xE4, sp);
            write(t, 0xE8, sp.add(16));
            write(t, 0x78, 1i32);
            write(t, 0x54, steps.as_mut_ptr());
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                allocation: ap as usize,
                ..State::default()
            }
        });
        unsafe {
            fc4p_006274b0(t, out);
        }
        let capacity = if old_count == 0 { 1 } else { 2 * old_count };
        assert_eq!(
            output,
            [
                ap as u32,
                ap as u32 + (old_count as u32 + 1) * 16,
                ap as u32 + capacity as u32 * 16
            ]
        );
        assert_eq!(&allocated[..old_count * 4], &previous[..old_count * 4]);
        assert_eq!(
            &allocated[old_count * 4..old_count * 4 + 4],
            &[0, 0, 0xFF80_0000, 0x7FC0_0123]
        );
        assert!(allocated[(old_count + 1) * 4..].iter().all(|&w| w == 0xA5A5_A5A5));
        assert_eq!(previous, [0x1234_5678; 12]);
        let mut expected = vec![
            (1, vec![capacity as u32 * 16]),
            (2, vec![old_begin as u32, old_end as u32, ap as u32, out as u32]),
        ];
        if old_count != 0 {
            expected.push((3, vec![old_begin as u32]));
        }
        STATE.with(|s| assert_eq!(s.borrow().calls, expected));
    }
}

#[test]
fn endpoint_ranges_reload_after_lookup_and_signed_span_limits_allow_negative_steps() {
    let _f = fake(0x0063_3220, lookup as *const () as usize);
    let mut tool = [0u32; 0x368 / 4];
    let t = tool.as_mut_ptr().cast::<u8>();
    // The negative last step passes a negative signed limit. Its address wraps back to the first row.
    let mut span = [123u32, 456, 1, u32::MAX];
    let sp = span.as_mut_ptr().cast::<u8>();
    let mut steps = [[2u32, 1, 0], [0, 3, 0], [0, 0, 0]];
    let steps_ptr = unsafe { steps.as_mut_ptr().cast::<u8>().add(12) };
    let mut points = [[1u32, 2], [3, 4]];
    let mut cell = [0xA5A5_A5A5u32; 0x90 / 4];
    let cp = cell.as_mut_ptr().cast::<u8>();
    let mut output_data = [0u32; 4];
    let op = output_data.as_mut_ptr().cast::<u8>();
    let mut output = [op as u32, op as u32, op as u32 + 16];
    unsafe {
        write(t, 0xE4, sp);
        write(t, 0xE8, sp.add(16));
        write(t, 0x78, 0i32);
        write(t, 0x54, steps_ptr);
        write(t, 0x60, points.as_mut_ptr());
    }
    // first_step=1 initially selects row 2 (one cell); callback changes that row's range to [0,1].
    let row = steps[2].as_mut_ptr() as usize;
    STATE.with(|s| {
        *s.borrow_mut() = State {
            cells: vec![(0x20001, cp as usize), (0x40003, cp as usize)],
            replace: Some((row, 0, 1)),
            ..State::default()
        }
    });
    unsafe {
        fc4p_006274b0(t, output.as_mut_ptr().cast());
    }
    STATE.with(|s| {
        assert_eq!(
            s.borrow().calls,
            [(4, vec![t as u32, 0x20001]), (4, vec![t as u32, 0x40003])]
        )
    });
    assert!(cell[0x58 / 4..0x88 / 4].iter().all(|&w| w == 123));
}
