// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    AddRef(usize),
    GetPath(usize, u32, usize),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

fn take_calls() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn add_ref(this: *mut u8) -> u32 {
    CALLS.with(|calls| calls.borrow_mut().push(Call::AddRef(this as usize)));
    0xABCD_EF99
}

extern "thiscall" fn get_path(this: *mut u8, key: u32, length: *mut u32) -> *mut u8 {
    CALLS.with(|calls| {
        calls
            .borrow_mut()
            .push(Call::GetPath(this as usize, key, length as usize))
    });
    if !length.is_null() {
        unsafe { write_unaligned(length, 0x7FC0_1234) };
    }
    0x1234_5678 as *mut u8
}

#[test]
fn query_interface_adjusts_resource_and_retains_primary() {
    let vtable = [0, add_ref as *const () as usize];
    let mut obj = Object::new(0x40);
    let this = obj.ptr();
    unsafe { PathInfo::from_ptr(this) }.set_vtable(vtable.as_ptr() as u32);
    for (iid, adjustment) in [(0x456B_8F1D, 4), (0xA966_7793, 0)] {
        let mut out = core::ptr::dangling_mut::<u8>();
        let result = unsafe { fc4p_0053cfa0(this, iid, &mut out) as u32 };
        assert_eq!(out, this.wrapping_add(adjustment));
        assert_eq!(result, 0xABCD_EF01);
        assert_eq!(take_calls(), [Call::AddRef(this as usize)]);
    }
}

#[test]
fn query_interface_failure_clears_output_and_keeps_pointer_upper_bytes() {
    for this in [core::ptr::null_mut(), core::ptr::dangling_mut::<u8>()] {
        for iid in [0, u32::MAX, 0x456B_8F1C] {
            let mut out = core::ptr::dangling_mut::<u8>();
            let result = unsafe { fc4p_0053cfa0(this, iid, &mut out) as u32 };
            assert!(out.is_null());
            assert_eq!(result, &mut out as *mut _ as u32 & !0xFF);
        }
    }
    for iid in [0x456B_8F1D, 0xA966_7793] {
        let mut out = core::ptr::dangling_mut::<u8>();
        let result = unsafe { fc4p_0053cfa0(core::ptr::null_mut(), iid, &mut out) as u32 };
        assert!(out.is_null());
        assert_eq!(result, &mut out as *mut _ as u32 & !0xFF);
    }
    assert!(take_calls().is_empty());
}

#[test]
fn edge_overload_packs_low_bytes_and_forwards_output_and_return() {
    let vtable = [0, 0, 0, get_path as *const () as usize];
    let mut obj = Object::new(0x40);
    let this = obj.ptr();
    unsafe { PathInfo::from_ptr(this) }.set_vtable(vtable.as_ptr() as u32);
    let mut length = 0;
    let result = unsafe { fc4p_0053cff0(this, 0x1234_56A1, 0xBB23, 0xCC45, 0xDD67, &mut length) };
    assert_eq!(result as usize, 0x1234_5678);
    assert_eq!(length, 0x7FC0_1234);
    assert_eq!(
        take_calls(),
        [Call::GetPath(
            this as usize,
            0xA167_2345,
            &mut length as *mut _ as usize
        )]
    );
    let result = unsafe { fc4p_0053cff0(this, 0, 0, 0, 0, core::ptr::null_mut()) };
    assert_eq!(result as usize, 0x1234_5678);
    assert_eq!(take_calls(), [Call::GetPath(this as usize, 0, 0)]);
}

#[test]
fn intersection_preserves_type_upper_bytes_and_returns_unmodified_type_byte() {
    let mut obj = Object::new(0x240);
    let this = obj.ptr();
    let info = unsafe { PathInfo::from_ptr(this) };
    for flags in 0..=255 {
        info.set_flags(flags);
        assert_eq!(
            unsafe { fc4p_0053d0a0(this, 0x1234_5678, 0) },
            0x1234_5678_0000_0000 | u64::from((flags >> 1) & 1)
        );
    }
    for (kind, value) in [(1, 0), (2, 1), (7, 0xFF), (0x101, 0x80)] {
        unsafe { write_unaligned(this.add(kind + 0x31), value) };
        assert_eq!(
            unsafe { fc4p_0053d0a0(this, 0, kind as u32) as u32 },
            (kind as u32 & !0xFF) | u32::from(value)
        );
    }
}

#[test]
fn elevation_uses_low_argument_byte_and_changes_only_bit_zero() {
    let mut obj = [0xA5u8; 0x40];
    let this = obj.as_mut_ptr();
    let info = unsafe { PathInfo::from_ptr(this) };
    for flags in 0..=255 {
        info.set_flags(flags);
        assert_eq!(
            unsafe { fc4p_005403a0(this, 0x1234_5678) },
            0x1234_5678_0000_0000 | u64::from(flags & 1)
        );
        for argument in [0, 1, 2, 0x80, 0x100, 0xFFFF_FF00, u32::MAX] {
            info.set_flags(flags);
            let mut want = obj;
            want[0x30] = (flags & !1) | u8::from(argument as u8 != 0);
            assert_eq!(
                unsafe { fc4p_005403b0(this, 0xABCD_EF01, argument) },
                0xABCD_EF01_0000_0000 | u64::from(want[0x30])
            );
            assert_eq!(obj, want);
        }
    }
}

pub(super) struct Map {
    pub(super) object: Object,
    pub(super) buckets: Vec<*mut u8>,
    pub(super) nodes: Vec<[u32; 8]>,
}

impl Map {
    pub(super) fn new(keys: &[u32], count: usize) -> Self {
        let mut result = Self {
            object: Object::new(0x40),
            buckets: vec![core::ptr::null_mut(); count],
            nodes: keys
                .iter()
                .enumerate()
                .map(|(i, &key)| [0, key, 0, 0, 0, 0x7F80_0001 + i as u32, 0xDEAD_BEEF, 0xA5A5_A5A5])
                .collect(),
        };
        for i in 0..result.nodes.len() {
            let bucket = result.nodes[i][1] as usize % count;
            result.nodes[i][0] = result.buckets[bucket] as u32;
            result.buckets[bucket] = result.nodes[i].as_mut_ptr().cast();
        }
        let info = unsafe { PathInfo::from_ptr(result.object.ptr()) };
        info.set_buckets(result.buckets.as_mut_ptr());
        info.set_buckets_end(result.buckets.as_mut_ptr().wrapping_add(count));
        info.set_buckets_capacity(result.buckets.as_mut_ptr().wrapping_add(count));
        info.set_path_count(keys.len() as u32);
        result
    }
}

#[test]
fn exact_lookup_walks_collisions_preserves_length_on_miss_and_copies_bits() {
    let keys = [0, 5, 10, 4, 0xFFFF_FFFF];
    let mut map = Map::new(&keys, 5);
    let before = map.nodes.clone();
    for (i, key) in keys.into_iter().enumerate() {
        let mut length = 0;
        let path = unsafe { fc4p_0053dd30(map.object.ptr(), key, &mut length) as u32 as *mut u8 };
        assert_eq!(path, map.nodes[i].as_mut_ptr().cast::<u8>().wrapping_add(8));
        assert_eq!(length, 0x7F80_0001 + i as u32);
        assert_eq!(
            unsafe { fc4p_0053dd30(map.object.ptr(), key, core::ptr::null_mut()) as u32 as *mut u8 },
            path
        );
    }
    for key in [15, 1, 2, 0xFFFF_FFFE] {
        let mut length = 0xA5A5_1234;
        assert!(unsafe { fc4p_0053dd30(map.object.ptr(), key, &mut length) as u32 as *mut u8 }.is_null());
        assert_eq!(length, 0xA5A5_1234);
    }
    assert_eq!(map.nodes, before);
}

#[test]
fn unique_key_skips_used_lanes_and_stops_at_signed_limit() {
    let keys: Vec<_> = [0xFF, 0, 1, 25, 26, 27, 0x80]
        .into_iter()
        .map(|lane| path_key(0xA1, 2, 3, lane))
        .collect();
    let mut map = Map::new(&keys, 7);
    let before = map.nodes.clone();
    for (lane, expected) in [
        (-1, 2),
        (0, 2),
        (1, 2),
        (2, 2),
        (25, 26),
        (26, 26),
        (27, 27),
        (0x100, 0),
        (i32::MIN, 2),
        (i32::MAX, 0xFF),
    ] {
        let result = unsafe { fc4p_0053dde0(map.object.ptr(), 0x1234_56A1, 0x102, 0x203, lane) };
        assert_eq!(result, path_key(0xA1, 2, 3, expected), "lane {lane}");
    }
    assert_eq!(map.nodes, before);
}

#[test]
fn exported_thiscall_signatures_accept_full_stack_words() {
    let mut obj = Object::new(0x40);
    let this = obj.ptr();
    unsafe { PathInfo::from_ptr(this) }.set_flags(0xF2);
    let read = unsafe { call_thiscall_with_eax(fc4p_005403a0 as *const () as usize, this, &[], u32::MAX) };
    assert_eq!(read.0, 0);
    let set = unsafe { call_thiscall_with_eax(fc4p_005403b0 as *const () as usize, this, &[0x100], u32::MAX) };
    assert_eq!(set.0, 0xF2);
    let intersection = unsafe { call_thiscall_with_eax(fc4p_0053d0a0 as *const () as usize, this, &[0], 0x1234_5678) };
    assert_eq!(intersection.0, 1);
}
