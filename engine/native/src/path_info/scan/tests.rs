// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::path_info::tests::Map;
use crate::testing::{call_thiscall_with_eax, fake};
use std::cell::{Cell, RefCell};

#[derive(Debug, PartialEq)]
enum Call {
    Begin(usize, usize),
    Connection(usize, u32, u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static UNLINK: Cell<usize> = const { Cell::new(0) };
}
fn take_calls() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn map_begin(map: *mut u8, iterator: *mut *mut u8) -> u64 {
    CALLS.with(|calls| calls.borrow_mut().push(Call::Begin(map as usize, iterator as usize)));
    unsafe {
        let info = PathInfo::from_ptr(map.sub(0x1C));
        let mut first = core::ptr::null_mut();
        for i in 0..bucket_count(info) {
            first = read_unaligned(info.buckets().add(i as usize));
            if !first.is_null() {
                break;
            }
        }
        write_unaligned(iterator, first);
        write_unaligned(iterator.add(1), map);
    }
    0x7654_3210_0000_0000 | u64::from(iterator as u32)
}

extern "thiscall" fn connection(this: *mut u8, kind: u32, entry: u32, exit: u32) -> u64 {
    CALLS.with(|calls| {
        calls
            .borrow_mut()
            .push(Call::Connection(this as usize, kind, entry, exit))
    });
    UNLINK.with(|unlink| {
        let node = unlink.replace(0) as *mut u8;
        if !node.is_null() {
            unsafe { PathNode::from_ptr(node) }.set_next(core::ptr::null_mut());
        }
    });
    0xAABB_CCDD_EEFF_0011
}

#[test]
fn stop_count_matches_type_lane_and_both_sides_of_edge_wildcards() {
    let _begin = fake(0x004B_0390, map_begin as *const () as usize);
    let keys = [
        0x1101_0203,
        0x1102_0203,
        0x1101_FF03,
        0x1101_02FF,
        0x1201_0203,
        0x0101_0203,
        0x1101_0302,
        0x1101_0405,
    ];
    let mut map = Map::new(&keys, 7);
    let before = map.nodes.clone();
    for (kind, entry, exit, lane, expected) in [
        (1, 2, 3, 1, 3),
        (1, 2, 3, 2, 1),
        (2, 2, 3, 1, 1),
        (0, 2, 3, 1, 4),
        (1, 0xFF, 3, 1, 3),
        (1, 2, 0xFF, 1, 3),
        (1, 2, 3, 0xFF, 4),
        (0, 0xFF, 0xFF, 0xFF, 7),
        (1, 9, 3, 1, 1),
        (1, 2, 9, 1, 1),
        (9, 2, 3, 1, 0),
        (0x100, 0x102, 0x203, 0x301, 0),
    ] {
        let args = [kind, entry, exit, lane];
        let registers = unsafe {
            call_thiscall_with_eax(
                fc4p_0053e550 as *const () as usize,
                map.object.ptr(),
                &args,
                0xA5A5_A5A5,
            )
        };
        assert_eq!(registers, (expected, 7), "filters {args:x?}");
        let calls = take_calls();
        assert!(matches!(&calls[..], [Call::Begin(address, _)] if *address == map.object.ptr() as usize + 0x1C));
    }
    assert_eq!(map.nodes, before);
}

#[test]
fn empty_scans_retain_begin_return_registers_and_still_call_begin() {
    let _begin = fake(0x004B_0390, map_begin as *const () as usize);
    let mut map = Map::new(&[], 5);
    let this = map.object.ptr();
    let stop = unsafe { call_thiscall_with_eax(fc4p_0053e550 as *const () as usize, this, &[0, 255, 255, 255], 0) };
    assert_eq!(stop, (0, 0x7654_3210));
    assert_eq!(take_calls().len(), 1);
    let max = unsafe { call_thiscall_with_eax(fc4p_0053eb30 as *const () as usize, this, &[1, 2, 3], 0) };
    assert_eq!(max, (0, 0x7654_3210));
    assert_eq!(take_calls().len(), 1);
    let bitmap = unsafe { call_thiscall_with_eax(fc4p_0053ebb0 as *const () as usize, this, &[0], 0) };
    let calls = take_calls();
    let [Call::Begin(map_pointer, iterator)] = &calls[..] else {
        panic!("expected begin");
    };
    assert_eq!(*map_pointer, this as usize + 0x1C);
    assert_eq!(bitmap, (*iterator as u32, 0x7654_3210));
}

#[unsafe(naked)]
unsafe extern "C" fn maximum_with_ecx() {
    core::arch::naked_asm!("push [esp + 12]", "push [esp + 12]", "push [esp + 12]",
        "call {}", "mov edx, ecx", "ret 12", sym fc4p_0053eb30);
}

#[test]
fn maximum_lane_includes_stop_keys_full_byte_and_collision_chains() {
    let _begin = fake(0x004B_0390, map_begin as *const () as usize);
    let mut map = Map::new(&[0x0101_0203, 0x11FF_0000, 0x0180_0000, 0x1103_FFFF], 3);
    let this = map.object.ptr();
    let max = unsafe { call_thiscall_with_eax(fc4p_0053eb30 as *const () as usize, this, &[u32::MAX; 3], 0) };
    assert_eq!(max, (255, 3));
    take_calls();
    let ecx = unsafe { call_thiscall_with_eax(maximum_with_ecx as *const () as usize, this, &[7, 8, 9], 0) };
    assert_eq!(ecx, (255, this as u32 + 0x1C));
    assert_eq!(take_calls().len(), 1);
}

#[test]
fn bitmap_enumerates_bucket_order_skips_stop_keys_and_preserves_full_argument_words() {
    let _begin = fake(0x004B_0390, map_begin as *const () as usize);
    let keys = [0x0101_0203, 0x1205_0102, 0x0F80_FFFF, 0x0103_0002, 0x8001_0203];
    let mut map = Map::new(&keys, 5);
    let mut vtable = [0usize; 55];
    vtable[0xD8 / 4] = connection as *const () as usize;
    let mut bitmap = [vtable.as_ptr() as usize];
    let pointer = bitmap.as_mut_ptr().cast::<u8>();
    let before = map.nodes.clone();
    let result = unsafe {
        call_thiscall_with_eax(
            fc4p_0053ebb0 as *const () as usize,
            map.object.ptr(),
            &[pointer as u32],
            0,
        )
    };
    assert_eq!(result, (5, 5));
    let calls = take_calls();
    assert!(matches!(calls[0], Call::Begin(address, _) if address == map.object.ptr() as usize + 0x1C));
    let mut expected = Vec::new();
    for bucket in 0..5 {
        for &key in keys.iter().rev().filter(|&&key| key % 5 == bucket) {
            if key & 0xF000_0000 == 0 {
                expected.push(Call::Connection(pointer as usize, (key >> 24) & 0xF, key >> 8, key));
            }
        }
    }
    assert_eq!(&calls[1..], expected);
    assert_eq!(map.nodes, before);
}

#[test]
fn bitmap_reads_the_next_link_after_the_callback() {
    let _begin = fake(0x004B_0390, map_begin as *const () as usize);
    let mut map = Map::new(&[0x0101_0002, 0x0102_0003], 1);
    UNLINK.with(|p| p.set(map.nodes[1].as_mut_ptr() as usize));
    let mut vtable = [0usize; 55];
    vtable[0xD8 / 4] = connection as *const () as usize;
    let mut bitmap = [vtable.as_ptr() as usize];
    let pointer = bitmap.as_mut_ptr().cast::<u8>();
    let result = unsafe {
        call_thiscall_with_eax(
            fc4p_0053ebb0 as *const () as usize,
            map.object.ptr(),
            &[pointer as u32],
            0,
        )
    };
    assert_eq!(result, (1, 1));
    assert!(matches!(
        &take_calls()[..],
        [Call::Begin(_, _), Call::Connection(_, 1, 0x010200, 0x0102_0003)]
    ));
}
