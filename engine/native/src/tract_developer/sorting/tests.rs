// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Sorting cases from the core STL helpers, with the game's callback and helper boundaries retained.

use super::*;
use crate::testing::fake;
use crate::tract_developer::testing::{run, ECX, EDX};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Less(u32, u32),
    Move(u32, u32, u32),
    Guarded(u32, u32, u32, u32, u32, u32),
    Tail(u32, u32, u32, u32, u32, u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}
fn log(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "C" fn less(a: u32, b: u32) -> u64 {
    log(Call::Less(a, b));
    0xDEAD_BEEF_AABB_CC00 | u64::from(a < b)
}
fn compare() -> u32 {
    less as *const () as u32
}

extern "C" fn move_bytes(destination: *mut u8, source: *const u8, bytes: u32) -> u64 {
    log(Call::Move(destination as u32, source as u32, bytes));
    unsafe {
        core::ptr::copy(source, destination, bytes as usize);
    }
    (0xCAFE_BABEu64 << 32) | u64::from(destination as u32)
}

#[test]
fn median_preserves_tie_rules_and_comparator_call_order() {
    for (values, calls, selected) in [
        ([1, 2, 3], vec![(1, 2), (2, 3)], 1),
        ([1, 3, 2], vec![(1, 3), (3, 2), (1, 2)], 2),
        ([2, 3, 1], vec![(2, 3), (3, 1), (2, 1)], 0),
        ([2, 1, 3], vec![(2, 1), (2, 3)], 0),
        ([3, 1, 2], vec![(3, 1), (3, 2), (1, 2)], 2),
        ([3, 2, 1], vec![(3, 2), (3, 1), (2, 1)], 1),
        ([1, 1, 1], vec![(1, 1), (1, 1), (1, 1)], 1),
    ] {
        let p = values.as_ptr() as u32;
        let result = run(0x00704790, Conv::Cdecl, ECX, &[p, p + 4, p + 8, compare()]);
        assert_eq!(result.0, p + 4 * selected);
        assert_eq!(result.2, 0xDEAD_BEEF);
        assert_eq!(
            take(),
            calls.into_iter().map(|(a, b)| Call::Less(a, b)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn unguarded_insert_and_tail_shift_only_strictly_greater_predecessors() {
    let mut values = [0u32, 2, 4, 4, 8, 0xFFFF];
    let p = values.as_mut_ptr() as u32;
    let result = run(0x00704810, Conv::Cdecl, ECX, &[p + 20, 4, compare()]);
    assert_eq!(values, [0, 2, 4, 4, 4, 8]);
    assert_eq!(result.0, 0xAABB_CC00);
    assert_eq!(result.2, 0xDEAD_BEEF);
    assert_eq!(take(), [Call::Less(4, 8), Call::Less(4, 4)]);
    let mut values = [0u32, 3, 5, 7, 4, 2, 6];
    let p = values.as_mut_ptr() as u32;
    assert_eq!(
        run(0x00704A90, Conv::Cdecl, ECX, &[p + 16, p + 28, 0xBAD0_CAFE, compare()]).0,
        p + 28
    );
    assert_eq!(values, [0, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        take(),
        [
            Call::Less(4, 7),
            Call::Less(4, 5),
            Call::Less(4, 3),
            Call::Less(2, 7),
            Call::Less(2, 5),
            Call::Less(2, 4),
            Call::Less(2, 3),
            Call::Less(2, 0),
            Call::Less(6, 7),
            Call::Less(6, 5)
        ]
    );
    assert_eq!(run(0x00704A90, Conv::Cdecl, ECX, &[p, p, 0, 0]), (p, ECX, EDX));
}

#[test]
fn partition_swaps_inward_and_calls_comparator_in_scan_order() {
    for (mut values, pivot, sorted, offset, comparisons) in [
        (
            vec![3u32, 1, 4, 2],
            2,
            vec![2, 1, 4, 3],
            2,
            vec![(3, 2), (2, 2), (1, 2), (4, 2), (2, 4), (2, 1)],
        ),
        (vec![1, 2, 3], 2, vec![1, 2, 3], 1, vec![(1, 2), (2, 2), (2, 3), (2, 2)]),
        (vec![5, 5, 5, 5], 5, vec![5, 5, 5, 5], 2, vec![(5, 5); 6]),
        (vec![0], 0, vec![0], 0, vec![(0, 0), (0, 0)]),
    ] {
        let p = values.as_mut_ptr() as u32;
        let result = run(
            0x007054E0,
            Conv::Cdecl,
            ECX,
            &[p, p + values.len() as u32 * 4, pivot, compare()],
        );
        assert_eq!(values, sorted);
        assert_eq!(result.0, p + offset * 4);
        assert_eq!(result.2, 0xDEAD_BEEF);
        assert_eq!(
            take(),
            comparisons
                .into_iter()
                .map(|(a, b)| Call::Less(a, b))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn guarded_insertion_uses_game_move_for_new_minima_and_original_insert_for_others() {
    let _move = fake(0x009EEB70, move_bytes as *const () as usize);
    let mut values = [4u32, 2, 3, 2, 1];
    let p = values.as_mut_ptr() as u32;
    let result = run(0x007062D0, Conv::Cdecl, ECX, &[p, p + 20, compare()]);
    assert_eq!(values, [1, 2, 2, 3, 4]);
    assert_eq!(result.0, p + 20);
    assert_eq!(result.2, 0xCAFE_BABE);
    assert_eq!(
        take(),
        [
            Call::Less(2, 4),
            Call::Move(p + 4, p, 4),
            Call::Less(3, 2),
            Call::Less(3, 4),
            Call::Less(3, 2),
            Call::Less(2, 2),
            Call::Less(2, 4),
            Call::Less(2, 3),
            Call::Less(2, 2),
            Call::Less(1, 2),
            Call::Move(p + 4, p, 16)
        ]
    );
    assert_eq!(run(0x007062D0, Conv::Cdecl, ECX, &[p, p, 0]), (p, ECX, EDX));
    assert_eq!(run(0x007062D0, Conv::Cdecl, ECX, &[p, p + 4, 0]), (p + 4, ECX, EDX));
    assert!(take().is_empty());
}

#[unsafe(naked)]
unsafe extern "C" fn guarded_stub() {
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push esp", "call {body}",
        "pop edx", "pop ecx", "pop eax", "ret", body = sym guarded_body);
}
unsafe extern "stdcall" fn guarded_body(p: *mut Entry<3>) {
    let r = unsafe { &mut *p };
    log(Call::Guarded(r.eax, r.ecx, r.edx, r.args[0], r.args[1], r.args[2]));
    r.eax = 0x1122_3344;
    r.ecx = 0x5566_7788;
    r.edx = 0x99AA_BBCC;
}
#[unsafe(naked)]
unsafe extern "C" fn tail_stub() {
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push esp", "call {body}",
        "pop edx", "pop ecx", "pop eax", "ret", body = sym tail_body);
}
unsafe extern "stdcall" fn tail_body(p: *mut Entry<4>) {
    let r = unsafe { &mut *p };
    log(Call::Tail(
        r.eax, r.ecx, r.edx, r.args[0], r.args[1], r.args[2], r.args[3],
    ));
    r.eax = 0x1234_5678;
    r.ecx = 0xABCD_EF01;
    r.edx = 0x9876_5432;
}

#[test]
fn final_pass_uses_sixteen_element_threshold_and_retains_helper_registers() {
    let _guarded = fake(0x007062D0, guarded_stub as *const () as usize);
    let _tail = fake(0x00704A90, tail_stub as *const () as usize);
    for (first, last) in [
        (0x1000u32, 0x1000u32),
        (0x1000, 0x1040),
        (0x1000, 0x1043),
        (0x1000, 0x1044),
        (0x1000, 0x1100),
        (0xFFFF_FFE0, 0x40),
        (0, 0x8000_0000),
    ] {
        let span = last.wrapping_sub(first) & !3;
        let result = run(0x00706EA0, Conv::Cdecl, ECX, &[first, last, 0xBEEF]);
        let calls = take();
        if span as i32 > 64 {
            let split = first.wrapping_add(64);
            assert_eq!(
                calls,
                [
                    Call::Guarded(first, span, EDX, first, split, 0xBEEF),
                    Call::Tail(0x1122_3344, 0x5566_7788, 0x99AA_BBCC, split, last, 0, 0xBEEF)
                ]
            );
            assert_eq!(result, (0x1234_5678, 0xABCD_EF01, 0x9876_5432));
        } else {
            assert_eq!(calls, [Call::Guarded(first, span, 0xBEEF, first, last, 0xBEEF)]);
            assert_eq!(result, (0x1122_3344, 0x5566_7788, 0x99AA_BBCC));
        }
    }
}

#[test]
fn final_pass_sorts_small_ranges_and_large_ranges_with_a_prefix_sentinel() {
    let _move = fake(0x009EEB70, move_bytes as *const () as usize);
    for n in [0usize, 1, 2, 15, 16, 17, 32, 100] {
        let mut values: Vec<_> = (0..n).map(|i| ((i * 37 + 11) % 23) as u32).collect();
        if n > 16 {
            values[0] = 0;
        }
        let mut want = values.clone();
        want.sort();
        let p = values.as_mut_ptr() as u32;
        run(0x00706EA0, Conv::Cdecl, ECX, &[p, p + n as u32 * 4, compare()]);
        assert_eq!(values, want);
        take();
    }
}
