// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::fake;
use std::cell::RefCell;

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
                    let expected = if offset < length { queue.at(start + offset) } else { last };
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
                let (source, destination) = if backward { (start, start + 3) } else { (start + 3, start) };
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

unsafe extern "C" fn fake_copy(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: *const u32, unused: u32) -> u64 {
    assert_eq!(unused, 0);
    unsafe { fake_dispatch(output, first, last, destination, tag) }
}

unsafe extern "C" fn fake_dispatch(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: *const u32) -> u64 {
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

unsafe extern "C" fn fake_find(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, value: *const u32, tag: *const u32) -> u64 {
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
            let tag = if address >= 0x006C_6ED0 { &mut output as *mut _ as u32 } else { 0x89AB_CDEF };
            EXPECTED.with(|e| *e.borrow_mut() = Some(Expected {
                output: &mut output, first, last,
                destination: if address == 0x006C_69D0 { first } else { destination }, tag, called: false,
            }));
            let returned = match address {
                0x006C_69D0 => fc4p_006c69d0(&mut output, first, last, &123, tag),
                0x006C_6BA0 => fc4p_006c6ba0(&mut output, first, last, destination, tag),
                0x006C_6C20 => fc4p_006c6c20(&mut output, first, last, destination, tag),
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
