// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

struct Queue {
    // Separate allocations ensure tests also exercise nonadjacent queue blocks.
    #[allow(clippy::vec_box)]
    blocks: Vec<Box<[u32; 32]>>,
    map: Vec<*mut u32>,
}

impl Queue {
    fn new() -> Self {
        let mut blocks: Vec<_> = (0..4)
            .map(|block| Box::new(core::array::from_fn(|i| (block * 32 + i) as u32)))
            .collect();
        let map = blocks.iter_mut().map(|block| block.as_mut_ptr()).collect();
        Self { blocks, map }
    }

    fn at(&mut self, index: usize) -> QueueIterator {
        let first = self.map[index / 32];
        QueueIterator {
            current: first.wrapping_add(index % 32),
            first,
            last: first.wrapping_add(32),
            node: self.map.as_mut_ptr().wrapping_add(index / 32),
        }
    }

    fn values(&self) -> Vec<u32> {
        self.blocks.iter().flat_map(|b| b.iter().copied()).collect()
    }
}

#[test]
fn search_all_unrolled_positions_tails_and_block_edges() {
    let mut queue = Queue::new();
    unsafe {
        for start in [0, 29, 30, 31, 32, 60] {
            for length in 0..=37 {
                let first = queue.at(start);
                let last = queue.at(start + length);
                for offset in 0..=length {
                    let key = if offset < length { (start + offset) as u32 } else { 999 };
                    let mut output = first;
                    let returned = fc4p_006c63c0(&mut output, first, last, &key, core::ptr::null());
                    let expected = if offset < length {
                        queue.at(start + offset)
                    } else {
                        last
                    };
                    assert_eq!(output, expected);
                    assert_eq!(returned as u32, &mut output as *mut _ as u32);
                    assert_eq!((returned >> 32) as u32, expected.node as u32);
                }
            }
        }
    }
    assert_eq!(queue.values(), (0..128).collect::<Vec<_>>());
}

#[test]
fn search_returns_first_duplicate_and_empty_reversed_range() {
    let mut queue = Queue::new();
    queue.blocks[1][0] = 31;
    let first = queue.at(29);
    let last = queue.at(35);
    let mut output = last;
    unsafe {
        fc4p_006c63c0(&mut output, first, last, &31, core::ptr::null());
        assert_eq!(output, queue.at(31));
        fc4p_006c63c0(&mut output, last, first, &31, core::ptr::null());
        assert_eq!(output, first);
    }
}

#[test]
fn forward_and_backward_copy_preserve_overlap_and_cross_blocks() {
    for backward in [false, true] {
        for start in [0, 28, 31, 32] {
            for count in [0, 1, 2, 31, 32, 33, 63] {
                let mut queue = Queue::new();
                let before = queue.values();
                let (source, destination) = if backward {
                    (start, start + 3)
                } else {
                    (start + 3, start)
                };
                let first = queue.at(source);
                let last = queue.at(source + count);
                let destination_iterator = queue.at(destination + if backward { count } else { 0 });
                let expected_iterator = queue.at(destination + if backward { 0 } else { count });
                let mut output = first;
                let mut expected = before.clone();
                expected[destination..destination + count].copy_from_slice(&before[source..source + count]);
                let returned = unsafe {
                    if backward {
                        fc4p_006c6570(&mut output, first, last, destination_iterator, core::ptr::null(), 0)
                    } else {
                        fc4p_006c6630(&mut output, first, last, destination_iterator, core::ptr::null(), 0)
                    }
                };
                assert_eq!(output, expected_iterator);
                assert_eq!(queue.values(), expected);
                assert_eq!(returned as u32, &mut output as *mut _ as u32);
                assert_eq!((returned >> 32) as u32, expected_iterator.first as u32);
            }
        }
    }
}

#[test]
fn copy_negative_distance_leaves_destination_and_memory_alone() {
    let mut queue = Queue::new();
    let first = queue.at(40);
    let last = queue.at(30);
    let destination = queue.at(60);
    let mut output = first;
    unsafe {
        fc4p_006c6570(&mut output, first, last, destination, core::ptr::null(), 0);
        assert_eq!(output, destination);
        fc4p_006c6630(&mut output, first, last, destination, core::ptr::null(), 0);
        assert_eq!(output, destination);
    }
    assert_eq!(queue.values(), (0..128).collect::<Vec<_>>());
}

#[derive(Clone, Copy)]
struct Expected {
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    destination: QueueIterator,
    tag: u32,
    called: bool,
}

thread_local! {
    static EXPECTED: RefCell<Option<Expected>> = const { RefCell::new(None) };
}

unsafe extern "C" fn fake_copy(
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    destination: QueueIterator,
    tag: *const u32,
    unused: u32,
) -> u64 {
    assert_eq!(unused, 0);
    unsafe { fake_dispatch(output, first, last, destination, tag) }
}

unsafe extern "C" fn fake_dispatch(
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    destination: QueueIterator,
    tag: *const u32,
) -> u64 {
    EXPECTED.with(|e| {
        let mut state = e.borrow_mut();
        let expected = state.as_mut().unwrap();
        assert!(!expected.called);
        assert_eq!(output, expected.output);
        assert_eq!(first, expected.first);
        assert_eq!(last, expected.last);
        assert_eq!(destination, expected.destination);
        assert_eq!(unsafe { tag.read_unaligned() }, expected.tag);
        expected.called = true;
    });
    unsafe { output.write_unaligned(last) };
    0xA1B2_C3D4_9988_7766
}

unsafe extern "C" fn fake_find(
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    value: *const u32,
    tag: *const u32,
) -> u64 {
    assert_eq!(unsafe { value.read_unaligned() }, 123);
    unsafe { fake_dispatch(output, first, last, first, tag) }
}

#[test]
fn wrappers_call_original_addresses_with_all_stack_words_and_keep_edx() {
    let mut queue = Queue::new();
    let first = queue.at(3);
    let last = queue.at(36);
    let destination = queue.at(40);
    let mut output = first;
    unsafe {
        for address in [0x006C_69D0, 0x006C_6BA0, 0x006C_6C20, 0x006C_6ED0, 0x006C_6F50] {
            let callee = match address {
                0x006C_69D0 => 0x006C_63C0,
                0x006C_6BA0 => 0x006C_6570,
                0x006C_6C20 => 0x006C_6630,
                0x006C_6ED0 => 0x006C_6BA0,
                _ => 0x006C_6C20,
            };
            let fake_function = match address {
                0x006C_69D0 => fake_find as *const () as usize,
                0x006C_6BA0 | 0x006C_6C20 => fake_copy as *const () as usize,
                _ => fake_dispatch as *const () as usize,
            };
            let _fake = fake(callee, fake_function);
            let key = 123u32;
            let tag = if address == 0x006C_69D0 {
                &key as *const _ as u32
            } else {
                &mut output as *mut _ as u32
            };
            EXPECTED.with(|e| {
                *e.borrow_mut() = Some(Expected {
                    output: &mut output,
                    first,
                    last,
                    destination: if address == 0x006C_69D0 { first } else { destination },
                    tag,
                    called: false,
                })
            });
            let returned = match address {
                0x006C_69D0 => fc4p_006c69d0(&mut output, first, last, &key),
                0x006C_6BA0 => fc4p_006c6ba0(&mut output, first, last, destination, core::ptr::null()),
                0x006C_6C20 => fc4p_006c6c20(&mut output, first, last, destination, core::ptr::null()),
                0x006C_6ED0 => fc4p_006c6ed0(&mut output, first, last, destination),
                _ => fc4p_006c6f50(&mut output, first, last, destination),
            };
            EXPECTED.with(|e| assert!(e.borrow_mut().take().unwrap().called));
            assert_eq!(output, last);
            assert_eq!(returned as u32, &mut output as *mut _ as u32);
            assert_eq!(returned >> 32, 0xA1B2_C3D4);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ShuffleCall {
    At(i32),
    Random,
    Advance(i32),
}
thread_local! {
    static SHUFFLE_CALLS: RefCell<Vec<ShuffleCall>> = const { RefCell::new(Vec::new()) };
    static DRAWS: RefCell<VecDeque<i32>> = const { RefCell::new(VecDeque::new()) };
}

unsafe extern "thiscall" fn at(this: *const QueueIterator, output: *mut QueueIterator, offset: i32) -> u64 {
    SHUFFLE_CALLS.with(|c| c.borrow_mut().push(ShuffleCall::At(offset)));
    let mut iterator = unsafe { this.read_unaligned() };
    for _ in 0..offset {
        unsafe { iterator.advance() };
    }
    unsafe { output.write_unaligned(iterator) };
    (u64::from(iterator.node as u32) << 32) | u64::from(output as u32)
}
unsafe extern "thiscall" fn advance(this: *mut QueueIterator, offset: i32) -> u64 {
    SHUFFLE_CALLS.with(|c| c.borrow_mut().push(ShuffleCall::Advance(offset)));
    let mut iterator = unsafe { this.read_unaligned() };
    for _ in 0..offset {
        unsafe { iterator.advance() };
    }
    unsafe { this.write_unaligned(iterator) };
    0x1234_5678_9ABC_DEF0
}
unsafe extern "C" fn random() -> i32 {
    SHUFFLE_CALLS.with(|c| c.borrow_mut().push(ShuffleCall::Random));
    DRAWS.with(|d| d.borrow_mut().pop_front().expect("unexpected random draw"))
}

#[test]
fn shuffle_uses_one_crt_draw_per_prefix_and_keeps_empty_entry_registers() {
    let _at = fake(0x006C_6860, at as *const () as usize);
    let _advance = fake(0x006C_6010, advance as *const () as usize);
    let _random = fake(0x009E_FB9F, random as *const () as usize);
    for start in [0, 29, 31, 32] {
        for length in [0, 1, 2, 4, 33, 61] {
            let mut queue = Queue::new();
            let first = queue.at(start);
            let last = queue.at(start + length);
            let mut expected = queue.values();
            let draws: Vec<i32> = (1..length)
                .map(|i| if i % 2 == 0 { 32767 } else { -((i + 1) as i32) })
                .collect();
            let mut expected_calls = Vec::new();
            if length != 0 {
                expected_calls.push(ShuffleCall::At(1));
            }
            for (i, &draw) in (1..length).zip(&draws) {
                let index = draw % (i + 1) as i32;
                expected.swap(start + i, start + index as usize);
                expected_calls.push(ShuffleCall::Random);
                expected_calls.push(ShuffleCall::Advance(index));
            }
            DRAWS.with(|d| *d.borrow_mut() = draws.into());
            let words = [
                first.current as u32,
                first.first as u32,
                first.last as u32,
                first.node as u32,
                last.current as u32,
                last.first as u32,
                last.last as u32,
                last.node as u32,
            ];
            let mut call = RawCall::new(Conv::Cdecl, fc4p_006c68c0 as *const () as usize, 0, &words);
            call.eax = 0xABCD_1234;
            call.edx = 0xDEAD_BEEF;
            unsafe { call_raw(&mut call) };
            if length == 0 {
                assert_eq!((call.eax, call.edx), (0xABCD_1234, 0xDEAD_BEEF));
            }
            assert_eq!(queue.values(), expected);
            SHUFFLE_CALLS.with(|c| assert_eq!(core::mem::take(&mut *c.borrow_mut()), expected_calls));
            DRAWS.with(|d| assert!(d.borrow().is_empty()));
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum QueueCall {
    Backward(QueueIterator, QueueIterator, QueueIterator),
    Forward(QueueIterator, QueueIterator, QueueIterator),
    Free(u32),
    At(i32),
    Query,
    Find,
    Erase,
    Release,
}
thread_local! {
    static QUEUE_CALLS: RefCell<Vec<QueueCall>> = const { RefCell::new(Vec::new()) };
    static REPLACE_MAP_ON_FREE: RefCell<Option<(*mut *mut u32, *mut u32)>> = const { RefCell::new(None) };
}

unsafe extern "C" fn backward(
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    destination: QueueIterator,
) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Backward(first, last, destination)));
    unsafe { fc4p_006c6570(output, first, last, destination, core::ptr::null(), 0) }
}
unsafe extern "C" fn forward(
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    destination: QueueIterator,
) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Forward(first, last, destination)));
    unsafe { fc4p_006c6630(output, first, last, destination, core::ptr::null(), 0) }
}
unsafe extern "C" fn free(block: *mut u32) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Free(block as u32)));
    REPLACE_MAP_ON_FREE.with(|c| {
        if let Some((address, replacement)) = c.borrow_mut().take() {
            unsafe { address.write_unaligned(replacement) };
        }
    });
    0xA1B2_C3D4_5566_7788
}
unsafe extern "thiscall" fn position(this: *const QueueIterator, output: *mut QueueIterator, offset: i32) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::At(offset)));
    let mut iterator = unsafe { this.read_unaligned() };
    for _ in 0..offset {
        unsafe { iterator.advance() };
    }
    unsafe { output.write_unaligned(iterator) };
    (u64::from(iterator.node as u32) << 32) | u64::from(output as u32)
}

fn values_between(mut begin: QueueIterator, end: QueueIterator) -> Vec<u32> {
    let mut result = Vec::new();
    unsafe {
        while begin.current != end.current {
            result.push(begin.current.read_unaligned());
            begin.advance();
        }
    }
    result
}

#[test]
fn erase_shifts_the_shorter_side_and_frees_only_an_emptied_edge_block() {
    let _backward = fake(0x006C_6ED0, backward as *const () as usize);
    let _forward = fake(0x006C_6F50, forward as *const () as usize);
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let _position = fake(0x006C_6860, position as *const () as usize);
    for (start, length) in [(0, 1), (0, 32), (29, 7), (31, 2), (31, 33), (32, 64), (0, 63)] {
        for index in 0..length {
            let mut queue = Queue::new();
            let begin = queue.at(start);
            let end = queue.at(start + length);
            let selected = queue.at(start + index);
            let after = queue.at(start + index + 1);
            let mut range = [begin, end];
            let mut result = begin;
            let returned = unsafe { fc4p_006c7c50(range.as_mut_ptr().cast(), &mut result, selected) };
            let mut expected: Vec<_> = (start as u32..(start + length) as u32).collect();
            expected.remove(index);
            assert_eq!(values_between(range[0], range[1]), expected);
            let mut successor = range[0];
            for _ in 0..index {
                unsafe { successor.advance() };
            }
            assert_eq!(result, successor);
            assert_eq!(returned as u32, &mut result as *mut _ as u32);
            assert_eq!((returned >> 32) as u32, successor.node as u32);
            let front = index < length / 2;
            let mut calls = vec![if front {
                QueueCall::Backward(begin, selected, after)
            } else {
                QueueCall::Forward(after, end, selected)
            }];
            if front && begin.current == begin.last.wrapping_sub(1) {
                calls.push(QueueCall::Free(begin.first as u32));
            } else if !front && end.current == end.first {
                calls.push(QueueCall::Free(end.first as u32));
            }
            calls.push(QueueCall::At(index as i32));
            QUEUE_CALLS.with(|c| assert_eq!(core::mem::take(&mut *c.borrow_mut()), calls));
        }
    }
}

#[test]
fn erase_reads_replacement_map_entry_after_the_old_block_is_freed() {
    let _backward = fake(0x006C_6ED0, backward as *const () as usize);
    let _forward = fake(0x006C_6F50, forward as *const () as usize);
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let _position = fake(0x006C_6860, position as *const () as usize);
    for front in [false, true] {
        let mut queue = Queue::new();
        let begin = queue.at(if front { 31 } else { 0 });
        let end = queue.at(if front { 35 } else { 32 });
        let selected = if front { begin } else { queue.at(31) };
        let mut replacement = [77u32; 32];
        let address = queue.map.as_mut_ptr().wrapping_add(usize::from(front));
        REPLACE_MAP_ON_FREE.with(|e| *e.borrow_mut() = Some((address, replacement.as_mut_ptr())));
        let mut range = [begin, end];
        let mut output = begin;
        unsafe { fc4p_006c7c50(range.as_mut_ptr().cast(), &mut output, selected) };
        let changed = range[usize::from(!front)];
        assert_eq!(changed.first, replacement.as_mut_ptr());
        assert_eq!(changed.last, replacement.as_mut_ptr().wrapping_add(32));
        assert_eq!(
            changed.current,
            replacement.as_mut_ptr().wrapping_add(if front { 0 } else { 31 })
        );
        QUEUE_CALLS.with(|c| c.borrow_mut().clear());
        REPLACE_MAP_ON_FREE.with(|e| assert!(e.borrow().is_none()));
    }
}

#[repr(C)]
struct Occupant {
    table: *const usize,
    building: *mut u8,
    success: u32,
}
#[repr(C)]
struct Building {
    table: *const usize,
    releases: u32,
}
unsafe extern "thiscall" fn query(this: *mut Occupant, iid: u32, output: *mut *mut u8) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Query));
    assert_eq!(iid, 0x87DD_A3A9);
    unsafe {
        output.write_unaligned((*this).building);
        (0x5566_7788u64 << 32) | u64::from((*this).success)
    }
}
unsafe extern "thiscall" fn release(this: *mut Building) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Release));
    unsafe { (*this).releases += 1 };
    0x1234_5678_9ABC_DEF0
}
unsafe extern "C" fn find(
    output: *mut QueueIterator,
    first: QueueIterator,
    last: QueueIterator,
    value: *const u32,
) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Find));
    unsafe { fc4p_006c63c0(output, first, last, value, core::ptr::null()) }
}
unsafe extern "thiscall" fn erase(this: *mut u8, output: *mut QueueIterator, selected: QueueIterator) -> u64 {
    QUEUE_CALLS.with(|c| c.borrow_mut().push(QueueCall::Erase));
    unsafe { fc4p_006c7c50(this, output, selected) }
}

#[test]
fn removing_a_building_preserves_the_cursor_element_and_releases_twice() {
    let _backward = fake(0x006C_6ED0, backward as *const () as usize);
    let _forward = fake(0x006C_6F50, forward as *const () as usize);
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let _position = fake(0x006C_6860, position as *const () as usize);
    let _find = fake(0x006C_69D0, find as *const () as usize);
    let _erase = fake(0x006C_7C50, erase as *const () as usize);
    for removed in 0..7 {
        for cursor in 0..=7 {
            let mut queue = Queue::new();
            let begin = queue.at(29);
            let end = queue.at(36);
            let mut object = Object::new(0x108);
            let this = object.ptr();
            let developer = unsafe { LotDeveloper::from_ptr(this) };
            developer.set_night_begin(begin);
            developer.set_night_end(end);
            developer.set_night_cursor(queue.at(29 + cursor));
            let building_table = [0usize, 0, release as *const () as usize];
            let mut building = Building {
                table: building_table.as_ptr(),
                releases: 0,
            };
            let bp = (&mut building as *mut Building).cast::<u8>();
            let occupant_table = [query as *const () as usize];
            let mut occupant = Occupant {
                table: occupant_table.as_ptr(),
                building: bp,
                success: 0xABC1,
            };
            let selected = queue.at(29 + removed);
            unsafe { selected.current.write(bp as u32) };
            let mut expected = values_between(begin, end);
            expected.remove(removed);
            let returned = unsafe { fc4p_006c8830(this, (&mut occupant as *mut Occupant).cast()) };
            assert_eq!(returned, 0x1234_5678_9ABC_DEF0);
            assert_eq!(building.releases, 2);
            assert_eq!(values_between(developer.night_begin(), developer.night_end()), expected);
            let index = if cursor > removed { cursor - 1 } else { cursor };
            let mut expected_cursor = developer.night_begin();
            for _ in 0..index {
                unsafe { expected_cursor.advance() };
            }
            assert_eq!(developer.night_cursor(), expected_cursor);
            let mut expected_calls = vec![QueueCall::Query, QueueCall::Find, QueueCall::Erase];
            let mut after = selected;
            unsafe { after.advance() };
            expected_calls.push(if removed < 3 {
                QueueCall::Backward(begin, selected, after)
            } else {
                QueueCall::Forward(after, end, selected)
            });
            expected_calls.push(QueueCall::At(removed as i32));
            if cursor != removed {
                expected_calls.push(QueueCall::At(index as i32));
            }
            expected_calls.extend([QueueCall::Release, QueueCall::Release]);
            QUEUE_CALLS.with(|c| assert_eq!(core::mem::take(&mut *c.borrow_mut()), expected_calls));
        }
    }
}

#[test]
fn absent_or_failed_building_queries_release_only_a_nonnull_interface() {
    let _find = fake(0x006C_69D0, find as *const () as usize);
    for (success, nonnull) in [(0, false), (0x100, true), (1, false), (1, true)] {
        let mut queue = Queue::new();
        let begin = queue.at(3);
        let end = queue.at(7);
        let mut object = Object::new(0x108);
        let this = object.ptr();
        let developer = unsafe { LotDeveloper::from_ptr(this) };
        developer.set_night_begin(begin);
        developer.set_night_end(end);
        developer.set_night_cursor(begin);
        let building_table = [0usize, 0, release as *const () as usize];
        let mut building = Building {
            table: building_table.as_ptr(),
            releases: 0,
        };
        let bp = if nonnull {
            (&mut building as *mut Building).cast()
        } else {
            core::ptr::null_mut()
        };
        let occupant_table = [query as *const () as usize];
        let mut occupant = Occupant {
            table: occupant_table.as_ptr(),
            building: bp,
            success,
        };
        let returned = unsafe { fc4p_006c8830(this, (&mut occupant as *mut Occupant).cast()) };
        if nonnull {
            assert_eq!(returned, 0x1234_5678_9ABC_DEF0);
        } else if success == 1 {
            assert_eq!(
                returned,
                (u64::from(end.last as u32) << 32) | u64::from(end.current as u32)
            );
        } else {
            assert_eq!(returned, 0x5566_7788_0000_0000);
        }
        assert_eq!(building.releases, u32::from(nonnull));
        assert_eq!(developer.night_cursor(), begin);
        assert_eq!(queue.values(), (0..128).collect::<Vec<_>>());
        let mut expected = vec![QueueCall::Query];
        if success as u8 != 0 {
            expected.push(QueueCall::Find);
        }
        if nonnull {
            expected.push(QueueCall::Release);
        }
        QUEUE_CALLS.with(|c| assert_eq!(core::mem::take(&mut *c.borrow_mut()), expected));
    }
}
