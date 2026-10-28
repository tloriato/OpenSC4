// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate defaults and recycling, using the game's byte layout and allocator boundaries.

use super::*;
use crate::testing::{fake, fake_data, Fake};
use crate::tract_developer::testing::{run, snapshot, ECX, EDX};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Candidate(u32),
    Free(u32),
    Move(u32, u32, u32),
    Fill(u32, u32, u32),
    Insert(u32, u32, u32, u32, u32),
    Construct(u32),
    Assign(u32, u32),
    AtExit(u32),
    Dispose(u32, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    memory: Vec<Box<[u32]>>,
    // Stable addresses are stored in the candidates while this collection grows.
    #[allow(clippy::vec_box)]
    vector_memory: Vec<Box<[u32; 4]>>,
    null_allocation: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn allocate_words(s: &mut State, bytes: u32) -> u32 {
    let mut memory = vec![0xCCCC_CCCCu32; (bytes as usize).div_ceil(4)].into_boxed_slice();
    let p = memory.as_mut_ptr() as u32;
    s.memory.push(memory);
    p
}
extern "C" fn allocation(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        (0xBADC_0FFEu64 << 32) | u64::from(allocate_words(s, bytes))
    })
}
extern "C" fn candidate_allocation(bytes: u32) -> u32 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Candidate(bytes));
        if s.null_allocation {
            0
        } else {
            allocate_words(s, bytes)
        }
    })
}
extern "C" fn release(p: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(p)));
    0x1111_2222
}
extern "C" fn move_memory(to: u32, from: u32, bytes: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Move(to, from, bytes)));
    unsafe {
        core::ptr::copy(from as *const u8, to as *mut u8, bytes as usize);
    }
    to
}
extern "thiscall" fn fill(this: u32, count: u32, value: u32) -> u64 {
    let value = unsafe { word(value, 0) };
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Fill(this, count, value));
        assert_eq!(count, 4);
        let mut memory = Box::new([value; 4]);
        let p = memory.as_mut_ptr() as u32;
        s.vector_memory.push(memory);
        unsafe {
            store(this, 0, p);
            store(this, 4, p + 16);
            store(this, 8, p + 16);
        }
    });
    0x8765_4321_1234_5678
}
extern "thiscall" fn insert(this: u32, at: u32, value: u32, _temporary: u32, count: u32, flag: u32) -> u64 {
    let value = unsafe { word(value, 0) };
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Insert(this, at, value, count, flag));
        unsafe {
            assert_eq!(at, word(this, 4));
            let begin = word(this, 0);
            let used = at.wrapping_sub(begin);
            let p = allocate_words(s, used.max(4) * 2);
            if used != 0 {
                core::ptr::copy_nonoverlapping(begin as *const u8, p as *mut u8, used as usize);
            }
            store(p, used, value);
            store(this, 0, p);
            store(this, 4, p + used + 4);
            store(this, 8, p + used.max(4) * 2);
        }
    });
    0x9876_5432_1234_5678
}
extern "thiscall" fn construct(this: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Construct(this)));
    unsafe {
        store(this, 0, 0xABC0_1234);
    }
    this
}
extern "thiscall" fn assign(this: u32, template: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Assign(this, template)));
    if this != 0 {
        unsafe {
            store(this, 0, word(template, 0));
        }
    }
    0xBADC_0FFE_FFFF_FFFF
}
extern "C" fn register_exit(callback: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::AtExit(callback)));
    0
}
extern "thiscall" fn dispose(this: u32, child: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Dispose(this, child)));
    0
}

struct World {
    pool: Box<[u32; 3]>,
    template: Box<[u32; 41]>,
    flags: Box<u32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            pool: Box::new([0; 3]),
            template: Box::new([0x7777_7777; 41]),
            flags: Box::new(0),
            _fakes: Vec::new(),
        };
        w._fakes.extend([
            fake_data(0x00B4_AFCC, w.pool.as_mut_ptr().cast()),
            fake_data(0x00B4_AFF0, w.template.as_mut_ptr().cast()),
            fake_data(0x00B4_B094, (&mut *w.flags as *mut u32).cast()),
        ]);
        for (address, target) in [
            (0x0090_CF54, allocation as *const () as usize),
            (0x005E_55E0, candidate_allocation as *const () as usize),
            (0x0090_CF63, release as *const () as usize),
            (0x009E_EB70, move_memory as *const () as usize),
            (0x0051_CA60, insert as *const () as usize),
            (0x006B_DF10, fill as *const () as usize),
            (0x0070_7240, assign as *const () as usize),
            (0x009E_F059, register_exit as *const () as usize),
            (0x0070_49D0, dispose as *const () as usize),
        ] {
            w._fakes.push(fake(address, target));
        }
        w
    }
    fn use_fake_constructor(&mut self) {
        self._fakes.push(fake(0x0070_7680, construct as *const () as usize));
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn candidate_defaults_keep_unassigned_bytes() {
    let w = World::new();
    let mut candidate = [0xA5u8; 0xA4];
    let p = candidate.as_mut_ptr() as u32;
    let result = run(0x0070_7680, Conv::Thiscall, p, &[]);
    assert_eq!(result, (p, p, 0x8765_4321));
    let header = unsafe { word(p, 0x18) };
    let mut expected = [0xA5u8; 0xA4];
    for (offset, value) in [
        (0, 0),
        (4, 0),
        (8, u32::MAX),
        (0xC, u32::MAX),
        (0x10, u32::MAX),
        (0x18, header),
        (0x1C, 0),
        (0x24, u32::MAX),
        (0x28, u32::MAX),
        (0x34, 0xBF80_0000),
        (0x38, u32::MAX),
        (0x3C, u32::MAX),
        (0x40, u32::MAX),
        (0x44, u32::MAX),
    ] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for offset in [0x14, 0x15, 0x6C, 0xA0] {
        expected[offset] = 0;
    }
    for offset in [0x70, 0x7C, 0x88, 0x94] {
        expected[offset..offset + 12].copy_from_slice(&candidate[offset..offset + 12]);
        let begin = unsafe { word(p, offset as u32) };
        assert_eq!(unsafe { word(p, offset as u32 + 4) }, begin + 16);
        assert_eq!(unsafe { word(p, offset as u32 + 8) }, begin + 16);
        assert_eq!(snapshot(begin as *mut u8, 16), [0; 16]);
    }
    assert_eq!(candidate, expected);
    let mut expected_header = [0xCCu8; 20];
    expected_header[0] = 0;
    expected_header[4..8].copy_from_slice(&0u32.to_le_bytes());
    expected_header[8..12].copy_from_slice(&header.to_le_bytes());
    expected_header[12..16].copy_from_slice(&header.to_le_bytes());
    assert_eq!(snapshot(header as *mut u8, 20), expected_header);
    assert_eq!(
        w.calls(),
        [
            Call::Allocate(20),
            Call::Fill(p + 0x70, 4, 0),
            Call::Fill(p + 0x7C, 4, 0),
            Call::Fill(p + 0x88, 4, 0),
            Call::Fill(p + 0x94, 4, 0)
        ]
    );
}

#[test]
fn recycle_fast_and_full_pool_and_null_cursor() {
    let mut w = World::new();
    let mut storage = [0x5555_5555; 2];
    let p = storage.as_mut_ptr() as u32;
    *w.pool = [p, p, p + 8];
    for value in [0x1234, 0] {
        let end = w.pool[1];
        assert_eq!(run(0x0070_7320, Conv::Cdecl, ECX, &[value]), (end + 4, ECX, EDX));
    }
    assert_eq!(storage, [0x1234, 0]);
    assert!(w.calls().is_empty());
    let result = run(0x0070_7320, Conv::Cdecl, ECX, &[0x9876]);
    assert_eq!(result, (0x1234_5678, ECX, 0x9876_5432));
    assert_eq!(w.calls(), [Call::Insert(w.pool.as_ptr() as u32, p + 8, 0x9876, 1, 1)]);
    assert_eq!(unsafe { word(w.pool[0], 8) }, 0x9876);
    *w.pool = [0, 0, 4];
    assert_eq!(run(0x0070_7320, Conv::Cdecl, ECX, &[123]), (4, ECX, EDX));
    assert_eq!(*w.pool, [0, 4, 4]);
}

#[test]
fn clear_candidates_reverses_into_pool_and_keeps_source_allocation() {
    let mut w = World::new();
    let mut source = [11u32, 22, 33];
    let p = source.as_mut_ptr() as u32;
    let mut list = [p, p + 12, p + 12];
    let mut destination = [9u32; 4];
    let d = destination.as_mut_ptr() as u32;
    *w.pool = [d, d, d + 16];
    let result = run(0x0070_7370, Conv::Cdecl, ECX, &[list.as_mut_ptr() as u32]);
    assert_eq!(result, (p, list.as_ptr() as u32, p + 12));
    assert_eq!(list, [p, p, p + 12]);
    assert_eq!(source, [11, 22, 33]);
    assert_eq!(destination, [33, 22, 11, 9]);
    assert_eq!(*w.pool, [d, d + 12, d + 16]);
    assert!(w.calls().is_empty());
    assert_eq!(
        run(0x0070_7370, Conv::Cdecl, ECX, &[list.as_mut_ptr() as u32]),
        (p, list.as_ptr() as u32, p)
    );
}

#[test]
fn clear_candidates_grows_empty_then_full_storage() {
    let w = World::new();
    let mut source = [11u32, 22, 33];
    let p = source.as_mut_ptr() as u32;
    let mut list = [p, p + 12, p + 20];
    run(0x0070_7370, Conv::Cdecl, ECX, &[list.as_mut_ptr() as u32]);
    assert_eq!(list, [p, p, p + 20]);
    let begin = w.pool[0];
    assert_eq!(w.pool[1..], [begin + 12, begin + 16]);
    assert_eq!(
        unsafe { std::slice::from_raw_parts(begin as *const u32, 4) },
        [33, 22, 11, 0xCCCC_CCCC]
    );
    let calls = w.calls();
    assert_eq!(calls.len(), 7);
    assert_eq!(calls[0], Call::Allocate(4));
    assert_eq!(calls[1], Call::Allocate(8));
    let Call::Move(second, first, 4) = calls[2] else {
        panic!("{calls:?}")
    };
    assert_eq!(calls[3], Call::Free(first));
    assert_eq!(calls[4], Call::Allocate(16));
    assert_eq!(calls[5], Call::Move(begin, second, 8));
    assert_eq!(calls[6], Call::Free(second));
}

#[test]
fn acquire_initializes_once_and_reuses_last_pointer() {
    let mut w = World::new();
    w.use_fake_constructor();
    *w.flags = 0xCC00_0020;
    let mut candidates = [0xAAu32, 0xBB];
    let mut storage = [candidates.as_mut_ptr() as u32, candidates.as_mut_ptr() as u32 + 4];
    let p = storage.as_mut_ptr() as u32;
    *w.pool = [p, p + 8, p + 8];
    let template = w.template.as_ptr() as u32;
    for index in [1, 0] {
        let candidate = storage[index];
        assert_eq!(run(0x0070_7D90, Conv::Cdecl, ECX, &[]).0, candidate);
    }
    assert_eq!(
        w.calls(),
        [
            Call::Construct(template),
            Call::AtExit(0x00A7_D260),
            Call::Assign(storage[1], template),
            Call::Assign(storage[0], template)
        ]
    );
    assert_eq!(*w.flags, 0xCC00_0021);
    assert_eq!(candidates, [0xABC0_1234; 2]);
    assert_eq!(*w.pool, [p, p, p + 8]);
}

#[test]
fn acquire_refills_ten_records_and_handles_failed_allocations() {
    for fail in [false, true] {
        let mut w = World::new();
        w.use_fake_constructor();
        *w.flags = 1;
        STATE.with(|s| s.borrow_mut().null_allocation = fail);
        let mut storage = [0u32; 10];
        let p = storage.as_mut_ptr() as u32;
        *w.pool = [p, p, p + 40];
        let result = run(0x0070_7D90, Conv::Cdecl, ECX, &[]);
        assert_eq!(result.0, storage[9]);
        assert_eq!(*w.pool, [p, p + 36, p + 40]);
        let mut calls = w.calls().into_iter();
        for pointer in storage {
            assert_eq!(calls.next(), Some(Call::Candidate(0xA4)));
            if !fail {
                assert_eq!(calls.next(), Some(Call::Construct(pointer)));
            }
            assert_eq!(pointer == 0, fail);
        }
        assert_eq!(calls.next(), Some(Call::Assign(storage[9], w.template.as_ptr() as u32)));
        assert_eq!(calls.next(), None);
    }
}

#[test]
fn acquire_grows_the_pool_through_original_insert_helper() {
    let mut w = World::new();
    w.use_fake_constructor();
    *w.flags = 1;
    let result = run(0x0070_7D90, Conv::Cdecl, ECX, &[]);
    let calls = w.calls();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Candidate(0xA4))).count(), 10);
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Insert(..))).count(), 4);
    assert_eq!(unsafe { word(w.pool[0], 36) }, result.0);
    assert_eq!(w.pool[1] - w.pool[0], 36);
    assert_eq!(calls.last(), Some(&Call::Assign(result.0, w.template.as_ptr() as u32)));
}

#[test]
fn filter_list_disposes_owned_objects_in_order_and_retains_header() {
    let w = World::new();
    let mut header = [0xABCDu32; 2];
    let h = header.as_mut_ptr() as u32;
    let mut first = [0xCCCC_CCCCu32; 7];
    let mut second = [0xDDDD_DDDDu32; 7];
    let a = first.as_mut_ptr() as u32;
    let b = second.as_mut_ptr() as u32;
    let mut tree_header = [0x1234u32; 5];
    let th = tree_header.as_mut_ptr() as u32;
    let mut child = [0u32, 0, 0, 0x7777, 0];
    let c = child.as_mut_ptr() as u32;
    tree_header[1] = c;
    first[0] = b;
    first[3] = th;
    first[4] = 1;
    first[6] = 0x4444;
    second[0] = h;
    second[3] = 0;
    second[4] = 0;
    second[6] = 0;
    let before_first = first;
    let before_second = second;
    header = [a, b];
    let mut list = [h, 0x1357];
    let result = run(0x0070_75C0, Conv::Thiscall, list.as_mut_ptr() as u32, &[]);
    assert_eq!(result.0, h);
    assert_eq!(header, [h, h]);
    assert_eq!(list[1], 0x1357);
    let mut expected = before_first;
    expected[4] = 0;
    assert_eq!(first, expected);
    assert_eq!(second, before_second);
    assert_eq!(tree_header, [0x1234, 0, th, th, 0x1234]);
    assert_eq!(
        w.calls(),
        [
            Call::Free(0x4444),
            Call::Dispose(a + 0xC, 0x7777),
            Call::Free(c),
            Call::Free(th),
            Call::Free(a),
            Call::Free(b)
        ]
    );
    assert_eq!(run(0x0070_75C0, Conv::Thiscall, list.as_mut_ptr() as u32, &[]).0, h);
    assert!(w.calls().is_empty());
}
