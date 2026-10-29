// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::traffic::test_support::{bytes, float_environment};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    cells: Vec<(i32, i32, u32, u32)>,
    calls: Vec<(usize, i32, i32, u32)>,
    change_limit: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
unsafe extern "thiscall" fn cell(this: *mut u8, x: i32, z: i32, evening: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push((this as usize, x, z, evening));
        if s.change_limit {
            unsafe { WindowSim::from_ptr(this) }.set_row_limit(-1);
        }
        s.cells
            .iter()
            .find(|&&(a, b, c, _)| (a, b, c) == (x, z, evening))
            .map_or(0xABCD_0000, |&(_, _, _, v)| v)
    })
}
fn run(sim: &mut Object, x: i32, z: i32) -> (u32, [u32; 2]) {
    let mut frame = [
        fc4p_00714aa0 as *const () as u32,
        sim.ptr() as u32,
        x as u32,
        z as u32,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!("push [edi + 12]", "push [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
            "mov [edi + 16], eax", "mov eax, [esp - 8]", "mov [edi + 20], eax",
            "mov eax, [esp - 4]", "mov [edi + 24], eax", in("edi") frame.as_mut_ptr(), clobber_abi("C"));
    }
    (frame[4], [frame[5], frame[6]])
}
fn reset(cells: &[(i32, i32, u32, u32)]) {
    STATE.with(|s| {
        *s.borrow_mut() = State {
            cells: cells.to_vec(),
            ..State::default()
        }
    });
}
fn calls() -> Vec<(usize, i32, i32, u32)> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn commercial_window_worked_example_and_complete_call_order() {
    let _fake = fake(0x0071_4940, cell as *const () as usize);
    let mut sim = Object::new(0xD50);
    unsafe { WindowSim::from_ptr(sim.ptr()) }.set_row_limit(32);
    // Cell totals from the core's worked example: road sums wrap before this helper receives them.
    reset(&[
        (13, 14, 0, 440),
        (9, 10, 0, 500),
        (6, 12, 1, 380),
        (11, 17, 1, 200),
        (6, 10, 0, 1000),
        (14, 13, 0, 1500),
        (9, 18, 0, 3000),
        (9, 9, 1, 3000),
    ]);
    let before = unsafe { bytes(sim.ptr(), 0xD50) };
    let env = float_environment();
    assert_eq!(run(&mut sim, 2, 3), (880, [380, 12]));
    assert_eq!(unsafe { bytes(sim.ptr(), 0xD50) }, before);
    assert_eq!(float_environment(), env);
    let p = sim.ptr() as usize;
    let mut expected = Vec::new();
    for z in 10..18 {
        for x in if (12..16).contains(&z) { 6..14 } else { 8..12 } {
            expected.extend([(p, x, z, 0), (p, x, z, 1)]);
        }
    }
    assert_eq!(calls(), expected);
}

#[test]
fn zero_origin_clips_negative_bounds_and_ignores_upper_return_halves() {
    let _fake = fake(0x0071_4940, cell as *const () as usize);
    let mut sim = Object::new(0x30);
    unsafe { WindowSim::from_ptr(sim.ptr()) }.set_row_limit(32);
    reset(&[(0, 0, 0, 0xCAFE_01D0), (5, 3, 1, 0xBEEF_000A), (6, 0, 0, 9999)]);
    assert_eq!(run(&mut sim, 0, 0), (474, [10, 4]));
    let seen = calls();
    assert_eq!(seen.len(), 64);
    assert!(seen.iter().all(|&(_, x, z, _)| x >= 0 && z >= 0));
}

#[test]
fn eastern_window_queries_columns_beyond_city_and_row_bound_is_read_once() {
    let _fake = fake(0x0071_4940, cell as *const () as usize);
    let mut sim = Object::new(0x30);
    unsafe { WindowSim::from_ptr(sim.ptr()) }.set_row_limit(256);
    reset(&[(257, 2, 0, 77)]);
    STATE.with(|s| s.borrow_mut().change_limit = true);
    assert_eq!(run(&mut sim, 63, 0), (77, [0, 256]));
    let c = calls();
    assert!(c.contains(&(sim.ptr() as usize, 257, 2, 0)));
    assert_eq!(c.iter().map(|c| c.2).max(), Some(5));
}

#[test]
fn unsigned_maxima_sum_above_sixteen_bits() {
    let _fake = fake(0x0071_4940, cell as *const () as usize);
    let mut sim = Object::new(0x30);
    unsafe { WindowSim::from_ptr(sim.ptr()) }.set_row_limit(6);
    reset(&[(0, 0, 0, 65535), (0, 0, 1, 65535), (1, 0, 0, 32768), (1, 0, 1, 1)]);
    assert_eq!(run(&mut sim, 0, 0), (131070, [65535, 4]));
}

#[test]
fn empty_and_wrapped_bounds_keep_the_original_argument_writes() {
    let _fake = fake(0x0071_4940, cell as *const () as usize);
    let mut sim = Object::new(0x30);
    for (limit, x, z, args) in [
        (-1, 0, 0, [0, (-2i32) as u32]),
        (2, 0, 1, [0, 2]),
        (6, 0x1FFF_FFFF, 0, [0, 0x8000_0000]),
        (6, 0, 0x2000_0000, [0, 0x7FFF_FFFE]),
    ] {
        unsafe { WindowSim::from_ptr(sim.ptr()) }.set_row_limit(limit);
        reset(&[]);
        assert_eq!(run(&mut sim, x, z), (0, args));
        assert_eq!(calls(), []);
    }
}
