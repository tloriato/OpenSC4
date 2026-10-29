// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    BaseConstruct(usize),
    BaseDestroy(usize),
    PoolGrow(usize),
    PoolEmpty(usize, usize),
    PoolReorder(usize, usize),
    Allocate(u32),
    Construct(usize, usize),
    AllocateCopy(usize, u32, usize, usize),
    Destroy(usize, usize),
    Free(usize),
    Assign(usize, usize, usize),
    ConstructRange(usize, usize, usize),
    Partition(usize, usize, u32, u32),
    Recurse(usize, usize, u32, u32, u32),
    Heap(usize, usize, usize, u32, u32),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    allocation: usize,
    new_page: usize,
    splits: Vec<usize>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(event: Event) {
    STATE.with(|s| s.borrow_mut().events.push(event));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}

extern "thiscall" fn construct_base(this: *mut u8) -> u64 {
    record(Event::BaseConstruct(this as usize));
    // SAFETY: the test supplies the allocated resource occupant's base at +4.
    unsafe {
        assert_eq!(read_unaligned(this.sub(4).cast::<u32>()), 0x00A8_7320);
        write_unaligned(this.add(0x10).cast::<u32>(), 0x9988_7766);
    }
    0x1122_3344_AAAA_BBBB
}
extern "thiscall" fn destroy_base(this: *mut u8) -> u64 {
    record(Event::BaseDestroy(this as usize));
    // SAFETY: the test supplies the same base interface.
    unsafe {
        let o = ResourceOccupant::from_ptr(this.sub(4));
        assert_eq!(
            (
                o.primary_table(),
                o.base_table(),
                o.target_table(),
                o.property_table(),
                o.serializable_table()
            ),
            (0x00A8_72C0, 0x00A8_7238, 0x00A8_721C, 0x00A8_71B8, 0x00A8_71A0)
        );
    }
    0x1122_3344_AAAA_BBBB
}
extern "thiscall" fn grow_pool(this: *mut u8) -> *mut u8 {
    record(Event::PoolGrow(this as usize));
    let (page, result) = STATE.with(|s| {
        let s = s.borrow();
        (s.new_page, s.allocation)
    });
    // SAFETY: active_page resolves to the test's writable global.
    unsafe {
        write_unaligned(active_page(), page as *mut u8);
    }
    result as *mut u8
}
extern "thiscall" fn empty_pool(this: *mut u8, page: *mut u8) -> u64 {
    record(Event::PoolEmpty(this as usize, page as usize));
    0x5566_7788_AAAA_BBBB
}
extern "thiscall" fn reorder_pool(this: *mut u8, page: *mut u8) -> u64 {
    record(Event::PoolReorder(this as usize, page as usize));
    0x99AA_BBCC_AAAA_BBBB
}

#[test]
fn constructor_writes_only_owned_fields_after_base_construction() {
    let _base = fake(0x005E_9220, construct_base as *const () as usize);
    let mut default = 0x7654_3210u32;
    let _default = fake_data(0x00B0_7F18, (&mut default as *mut u32).cast());
    let mut storage = [0xA5u8; 0x60];
    let p = storage.as_mut_ptr();
    let mut expected = storage;
    for (offset, value) in [
        (0, 0x00A8_72C0u32),
        (4, 0x00A8_7238),
        (8, 0x00A8_721C),
        (12, 0x00A8_71B8),
        (0x14, 0x9988_7766),
        (0x20, 0x00A8_71A0),
        (0x28, 0x2781_28A0),
        (0x38, 0),
        (0x3C, 0),
        (0x40, 0),
        (0x44, 0),
        (0x48, 0),
        (0x4C, 0),
        (0x50, 0),
        (0x58, default),
    ] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    expected[0x54] = 0;
    expected[0x55] = 0;
    // SAFETY: storage includes every field touched by the constructor and fake base.
    unsafe {
        assert_eq!(fc4p_00458e90(p), 0x1122_3344_0000_0000 | u64::from(p as u32));
    }
    assert_eq!(storage, expected);
    assert_eq!(events(), [Event::BaseConstruct(p.wrapping_add(4) as usize)]);
}

#[repr(align(8192))]
struct Page([u8; 8192]);

#[test]
fn pool_allocation_reloads_the_active_page_after_growth() {
    let _grow = fake(0x0090_CF9A, grow_pool as *const () as usize);
    let mut context = 0u8;
    let _context = fake_data(0x00B0_7F1C, &mut context);
    let mut page1 = Box::new(Page([0; 8192]));
    let p1 = page1.0.as_mut_ptr();
    let mut page2 = Box::new(Page([0; 8192]));
    let p2 = page2.0.as_mut_ptr();
    let mut current = p1;
    let _current = fake_data(0x00B0_7F20, (&mut current as *mut *mut u8).cast());
    // SAFETY: page headers and the free-list objects are inside aligned pool buffers.
    unsafe {
        let first = p1.add(64);
        let second = p1.add(160);
        write_unaligned(p1.add(8).cast::<*mut u8>(), first);
        write_unaligned(first.cast::<*mut u8>(), second);
        write_unaligned(p1.add(12).cast::<u32>(), u32::MAX);
        assert_eq!(fc4p_00458a60(), (u64::from(p1 as u32) << 32) | u64::from(first as u32));
        assert_eq!(read_unaligned(p1.add(8).cast::<*mut u8>()), second);
        assert_eq!(read_unaligned(p1.add(12).cast::<u32>()), 0);
        assert!(events().is_empty());
        write_unaligned(p1.add(8).cast::<*mut u8>(), core::ptr::null_mut());
        let grown = p2.add(64);
        write_unaligned(grown.cast::<*mut u8>(), p2.add(160));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.new_page = p2 as usize;
            s.allocation = grown as usize;
        });
        assert_eq!(fc4p_00458a60(), (u64::from(p2 as u32) << 32) | u64::from(grown as u32));
        assert_eq!(current, p2);
        assert_eq!(read_unaligned(p2.add(12).cast::<u32>()), 1);
        assert_eq!(read_unaligned(p2.add(8).cast::<*mut u8>()), p2.add(160));
        assert_eq!(events(), [Event::PoolGrow(&mut context as *mut u8 as usize)]);
    }
}

#[test]
fn deleting_destructor_pool_thresholds_and_flag_bit() {
    let _base = fake(0x005E_8C40, destroy_base as *const () as usize);
    let _empty = fake(0x0090_D031, empty_pool as *const () as usize);
    let _reorder = fake(0x0090_CFF9, reorder_pool as *const () as usize);
    let mut context = 0u8;
    let _context = fake_data(0x00B0_7F1C, &mut context);
    let mut page = Box::new(Page([0; 8192]));
    let p = page.0.as_mut_ptr();
    // SAFETY: the occupant and header are in this page, and all called methods are fakes.
    unsafe {
        let item = p.add(64);
        let next = p.add(160);
        for (count, flags, high) in [
            (2u32, 0u32, 0x1122_3344u32),
            (2, 2, 0x1122_3344),
            (2, 1, 0x1122_3344),
            (1, 0x101, 0x5566_7788),
            (0x4000_0043, 1, 0x99AA_BBCC),
            (0, 1, 0x1122_3344),
        ] {
            write_unaligned(p.add(8).cast::<*mut u8>(), next);
            write_unaligned(p.add(12).cast(), count);
            assert_eq!(
                fc4p_00458f80(item, flags),
                (u64::from(high) << 32) | u64::from(item as u32)
            );
            let mut expected = vec![Event::BaseDestroy(item.add(4) as usize)];
            if flags & 1 != 0 {
                assert_eq!(read_unaligned(p.add(8).cast::<*mut u8>()), item);
                assert_eq!(read_unaligned(item.cast::<*mut u8>()), next);
                assert_eq!(read_unaligned(p.add(12).cast::<u32>()), count.wrapping_sub(1));
                if count == 1 {
                    expected.push(Event::PoolEmpty(&mut context as *mut u8 as usize, p as usize));
                }
                if count == 0x4000_0043 {
                    expected.push(Event::PoolReorder(&mut context as *mut u8 as usize, p as usize));
                }
            } else {
                assert_eq!(read_unaligned(p.add(12).cast::<u32>()), count);
            }
            assert_eq!(events(), expected);
        }
    }
}

extern "thiscall" fn query_fake(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
    assert_eq!(iid, 0x87DD_A3A9);
    // SAFETY: the thunk test passes a writable output word.
    unsafe {
        write_unaligned(out, this);
    }
    0x1234_5678_ABCD_EF01
}
extern "thiscall" fn delete_fake(this: *mut u8, flags: u32) -> u64 {
    assert_eq!(flags, 3);
    (u64::from(this as u32) << 32) | 0xABCD_EF01
}

#[test]
fn interface_thunks_adjust_this_and_preserve_argument_pops() {
    let _query = fake(0x0045_87D0, query_fake as *const () as usize);
    let _delete = fake(0x0045_8F80, delete_fake as *const () as usize);
    let mut object = Object::new(0x60);
    let p = object.ptr();
    let mut out = core::ptr::null_mut();
    // SAFETY: the harness passes exactly the original two and one argument words to these thunks.
    unsafe {
        let result = call_thiscall_with_eax(
            fc4p_00458a10 as *const () as usize,
            p.add(12),
            &[0x87DD_A3A9, &mut out as *mut *mut u8 as u32],
            0,
        );
        assert_eq!(out, p);
        assert_eq!(result, (0xABCD_EF01, 0x1234_5678));
        assert_eq!(
            call_thiscall_with_eax(fc4p_00458a20 as *const () as usize, p.add(12), &[3], 0),
            (0xABCD_EF01, p as u32)
        );
    }
}

extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    record(Event::Allocate(size));
    STATE.with(|s| s.borrow().allocation as *mut u8)
}
extern "thiscall" fn element_fake(this: *mut u8, source: *mut u8) {
    record(Event::Construct(this as usize, source as usize));
}
extern "thiscall" fn allocate_copy_fake(this: *mut u8, count: u32, begin: *mut u8, end: *mut u8) -> *mut u8 {
    record(Event::AllocateCopy(this as usize, count, begin as usize, end as usize));
    STATE.with(|s| s.borrow().allocation as *mut u8)
}
extern "C" fn destroy_fake(begin: *mut u8, end: *mut u8, _cookie: *mut u8) {
    record(Event::Destroy(begin as usize, end as usize));
}
extern "C" fn free_fake(storage: *mut u8) {
    record(Event::Free(storage as usize));
}
extern "C" fn assign_fake(begin: *mut u8, end: *mut u8, dest: *mut u8, _cookie: *mut u8, tag: u32) -> *mut u8 {
    assert_eq!(tag, 0);
    record(Event::Assign(begin as usize, end as usize, dest as usize));
    dest.wrapping_add((end as usize).wrapping_sub(begin as usize))
}
extern "C" fn construct_range_fake(begin: *mut u8, end: *mut u8, dest: *mut u8, _cookie: *mut u8) -> *mut u8 {
    record(Event::ConstructRange(begin as usize, end as usize, dest as usize));
    dest.wrapping_add((end as usize).wrapping_sub(begin as usize))
}

#[test]
fn allocation_copy_handles_zero_count_and_null_first_destination() {
    let _allocate = fake(0x0090_CF54, allocate_fake as *const () as usize);
    let _element = fake(0x004C_9470, element_fake as *const () as usize);
    let mut source = [0u8; 36];
    let s = source.as_mut_ptr();
    let mut dest = [0u8; 36];
    let d = dest.as_mut_ptr();
    // SAFETY: source and destination have three twelve-byte elements; fakes do not dereference them.
    unsafe {
        STATE.with(|state| state.borrow_mut().allocation = d as usize);
        assert_eq!(fc4p_004e4040(core::ptr::null_mut(), 3, s, s.add(36)), d);
        assert_eq!(
            events(),
            [
                Event::Allocate(36),
                Event::Construct(d as usize, s as usize),
                Event::Construct(d.add(12) as usize, s.add(12) as usize),
                Event::Construct(d.add(24) as usize, s.add(24) as usize)
            ]
        );
        assert!(fc4p_004e4040(core::ptr::null_mut(), 0, s, s).is_null());
        assert!(events().is_empty());
        STATE.with(|state| state.borrow_mut().allocation = 0);
        assert!(fc4p_004e4040(core::ptr::null_mut(), 1, s, s.add(12)).is_null());
        assert_eq!(events(), [Event::Allocate(12)]);
    }
}

#[test]
fn vector_assignment_all_capacity_branches_and_return_edx() {
    let _a = fake(0x004E_4040, allocate_copy_fake as *const () as usize);
    let _b = fake(0x0045_9170, destroy_fake as *const () as usize);
    let _c = fake(0x0090_CF63, free_fake as *const () as usize);
    let _d = fake(0x006C_10A0, assign_fake as *const () as usize);
    let _e = fake(0x0069_9150, construct_range_fake as *const () as usize);
    let mut src_mem = [0u8; 60];
    let s = src_mem.as_mut_ptr();
    let mut old_mem = [0u8; 60];
    let old = old_mem.as_mut_ptr();
    let mut new_mem = [0u8; 60];
    let new = new_mem.as_mut_ptr();
    STATE.with(|state| state.borrow_mut().allocation = new as usize);
    let mut src = [0usize; 3];
    let src_ptr = src.as_mut_ptr().cast::<u8>();
    let mut dest = [0usize; 3];
    let dest_ptr = dest.as_mut_ptr().cast::<u8>();
    // SAFETY: the vectors are three-word headers and their helpers are isolated with fakes.
    unsafe {
        let source = ObjectVector::from_ptr(src_ptr);
        let target = ObjectVector::from_ptr(dest_ptr);
        source.set_begin(s);
        source.set_end(s.add(36));
        source.set_limit(s.add(60));
        for (length, capacity) in [(2usize, 2usize), (4, 5), (1, 5), (0, 0)] {
            target.set_begin(old);
            target.set_end(old.add(length * 12));
            target.set_limit(old.add(capacity * 12));
            let mut source_word = src_ptr as u32;
            let result = assign_vector(dest_ptr, 0xDEAD_BEEF, &mut source_word, src_ptr);
            assert_eq!(source_word, s.add(36) as u32);
            let storage = if capacity < 3 { new } else { old };
            assert_eq!(
                result,
                (u64::from(storage.add(36) as u32) << 32) | u64::from(dest_ptr as u32)
            );
            assert_eq!(
                (target.begin(), target.end(), target.limit()),
                (
                    storage,
                    storage.add(36),
                    if capacity < 3 {
                        new.add(36)
                    } else {
                        old.add(capacity * 12)
                    }
                )
            );
            let expected = if capacity < 3 {
                vec![
                    Event::AllocateCopy(dest_ptr as usize, 3, s as usize, s.add(36) as usize),
                    Event::Destroy(old as usize, old.add(length * 12) as usize),
                    Event::Free(old as usize),
                ]
            } else if length >= 3 {
                vec![
                    Event::Assign(s as usize, s.add(36) as usize, old as usize),
                    Event::Destroy(old.add(36) as usize, old.add(length * 12) as usize),
                ]
            } else {
                vec![
                    Event::Assign(s as usize, s.add(length * 12) as usize, old as usize),
                    Event::ConstructRange(
                        s.add(length * 12) as usize,
                        s.add(36) as usize,
                        old.add(length * 12) as usize,
                    ),
                ]
            };
            assert_eq!(events(), expected);
        }
        let before = dest;
        let args = [dest_ptr as u32];
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            fc4p_004e40a0 as *const () as usize,
            dest_ptr as u32,
            &args,
        );
        call.edx = 0xDEAD_BEEF;
        crate::abi::call_raw(&mut call);
        assert_eq!((call.eax, call.edx), (dest_ptr as u32, 0xDEAD_BEEF));
        assert_eq!(dest, before);
        assert!(events().is_empty());
        target.set_begin(core::ptr::null_mut());
        target.set_end(core::ptr::null_mut());
        target.set_limit(core::ptr::null_mut());
        source.set_end(s);
        let mut source_word = src_ptr as u32;
        assign_vector(dest_ptr, 0, &mut source_word, src_ptr);
        assert_eq!(target.end(), core::ptr::null_mut());
        assert_eq!(
            events(),
            [Event::Assign(s as usize, s as usize, 0), Event::Destroy(0, 0)]
        );
    }
}

extern "C" fn partition_fake(begin: *mut u32, end: *mut u32, pivot: u32, compare: u32) -> *mut u32 {
    record(Event::Partition(begin as usize, end as usize, pivot, compare));
    STATE.with(|s| s.borrow_mut().splits.remove(0) as *mut u32)
}
extern "C" fn recurse_fake(begin: *mut u32, end: *mut u32, unused: u32, depth: u32, compare: u32) -> u32 {
    record(Event::Recurse(begin as usize, end as usize, unused, depth, compare));
    0xABCD_1234
}
extern "C" fn heap_fake(begin: *mut u32, middle: *mut u32, end: *mut u32, unused: u32, compare: u32) -> u32 {
    record(Event::Heap(
        begin as usize,
        middle as usize,
        end as usize,
        unused,
        compare,
    ));
    0x9876_5432
}

#[test]
fn sort_small_ranges_depth_exhaustion_median_ties_and_left_iteration() {
    let _partition = fake(0x0061_1840, partition_fake as *const () as usize);
    let _recurse = fake(0x0047_EA40, recurse_fake as *const () as usize);
    let _heap = fake(0x0048_A4B0, heap_fake as *const () as usize);
    let mut data = [0u32; 40];
    let begin = data.as_mut_ptr();
    // SAFETY: every tested range and split is within the forty-word array.
    unsafe {
        for count in 0..=16 {
            assert_eq!(sort(begin, begin.add(count), 9, 0, 0x42), count as u32 * 4);
        }
        assert!(events().is_empty());
        assert_eq!(sort(begin, begin.add(17), 9, 0, 0x42), 0x9876_5432);
        assert_eq!(
            events(),
            [Event::Heap(
                begin as usize,
                begin.add(17) as usize,
                begin.add(17) as usize,
                0,
                0x42
            )]
        );
        for a in [0, 5, u32::MAX] {
            for b in [0, 5, u32::MAX] {
                for c in [0, 5, u32::MAX] {
                    data[0] = a;
                    data[16] = b;
                    data[31] = c;
                    STATE.with(|s| s.borrow_mut().splits = vec![begin.add(8) as usize]);
                    assert_eq!(sort(begin, begin.add(32), 7, 3, 0x44), 0xABCD_1234);
                    let mut three = [a, b, c];
                    three.sort();
                    assert_eq!(
                        events(),
                        [
                            Event::Partition(begin as usize, begin.add(32) as usize, three[1], 0x44),
                            Event::Recurse(begin.add(8) as usize, begin.add(32) as usize, 0, 2, 0x44)
                        ]
                    );
                }
            }
        }
        STATE.with(|s| s.borrow_mut().splits = vec![begin.add(20) as usize, begin.add(8) as usize]);
        assert_eq!(sort(begin, begin.add(40), 0, 2, 7), 0xABCD_1234);
        let trace = events();
        assert_eq!(trace.len(), 4);
        assert_eq!(
            trace[1],
            Event::Recurse(begin.add(20) as usize, begin.add(40) as usize, 0, 1, 7)
        );
        assert_eq!(
            trace[3],
            Event::Recurse(begin.add(8) as usize, begin.add(20) as usize, 0, 0, 7)
        );
    }
}

// Calls the cdecl export with its original five stack words.
unsafe fn sort(begin: *mut u32, end: *mut u32, unused: u32, depth: u32, compare: u32) -> u32 {
    // SAFETY: the naked entry has this cdecl signature.
    unsafe {
        let f: unsafe extern "C" fn(*mut u32, *mut u32, u32, u32, u32) -> u32 =
            core::mem::transmute(fc4p_0047ea40 as *const ());
        f(begin, end, unused, depth, compare)
    }
}
