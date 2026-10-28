// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::building::BuildingSim;
use crate::testing::{fake, Object};
use std::cell::RefCell;

thread_local! { static CALLS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) }; }

fn invoke(function: usize, this: *mut u8, args: &[u32]) -> (u32, u32) {
    let mut c = RawCall::new(Conv::Thiscall, function, this as u32, args);
    c.eax = 0xABCD_1234;
    c.edx = 0x8765_4321;
    // SAFETY: each test supplies the entry's stack words and live object.
    unsafe { call_raw(&mut c) };
    (c.eax, c.edx)
}

#[test]
fn flags_keep_raw_bytes_upper_eax_and_edx_and_do_not_write() {
    let mut o = Object::new(0x100);
    let p = o.ptr();
    // SAFETY: the allocation covers the simulator layout.
    let b = unsafe { BuildingSim::from_ptr(p) };
    for byte in [0, 1, 0x80, 0xFF] {
        b.set_ignore_power(byte);
        b.set_ignore_water(byte);
        b.set_bar_stuck(byte);
        // SAFETY: the allocation is 0x100 bytes.
        let before = unsafe { std::slice::from_raw_parts(p, 0x100) }.to_vec();
        for f in [fc4p_00698d00 as *const () as usize, fc4p_00698d10 as *const () as usize, fc4p_00698d20 as *const () as usize] {
            assert_eq!(invoke(f, p, &[]), (0xABCD_1200 | u32::from(byte), 0x8765_4321));
        }
        assert_eq!(unsafe { std::slice::from_raw_parts(p, 0x100) }, before);
    }
}

#[test]
fn packed_profile_uses_only_argument_bytes_and_wraps() {
    for (a, b, result) in [(0, 0, 1), (1, 2, 7), (0xFFFF_FFFF, 0xFFFF_FFFF, 252), (64, 255, 0)] {
        assert_eq!(invoke(fc4p_00698d90 as *const () as usize, std::ptr::null_mut(), &[a, b]),
            (0xABCD_1200 | result, 0x8765_4321));
    }
}

extern "thiscall" fn query_fake(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((this as u32, vec![iid, out as u32])));
    unsafe { *out = this };
    0x1122_3344_5566_7788
}
extern "thiscall" fn release_fake(this: *mut u8) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((this as u32, vec![])));
    0x9988_7766_5544_3322
}

#[test]
fn adjusted_interfaces_forward_arguments_and_results() {
    let mut o = Object::new(0x100);
    let p = o.ptr();
    let _q = fake(0x0069_5290, query_fake as *const () as usize);
    let _r = fake(0x0069_59B0, release_fake as *const () as usize);
    let mut out = std::ptr::null_mut();
    let out_ptr = &mut out as *mut *mut u8;
    assert_eq!(invoke(fc4p_00698d50 as *const () as usize, p.wrapping_add(12), &[0xCAFE_BABE, out_ptr as u32]), (0x5566_7788, 0x1122_3344));
    assert_eq!(out, p);
    for (f, adjustment) in [(fc4p_00698dc0 as *const () as usize, 8), (fc4p_00698dd0 as *const () as usize, 12)] {
        assert_eq!(invoke(f, p.wrapping_add(adjustment), &[]), (0x5544_3322, 0x9988_7766));
    }
    assert_eq!(CALLS.with(|c| c.replace(vec![])), [(p as u32, vec![0xCAFE_BABE, out_ptr as u32]), (p as u32, vec![]), (p as u32, vec![])]);
}

extern "C" fn kind_fake(purpose: u32, wealth: u32) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((0x0069_5790, vec![purpose, wealth])));
    0x1111_2222_AAAA_BBBB
}
extern "C" fn wealth_fake(kind: u32) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((0x0069_5840, vec![kind])));
    0x3333_4444_CCCC_DDDD
}

#[test]
fn category_adapters_call_original_addresses_and_return_both_words() {
    let _k = fake(0x0069_5790, kind_fake as *const () as usize);
    let _w = fake(0x0069_5840, wealth_fake as *const () as usize);
    assert_eq!(invoke(fc4p_00698d60 as *const () as usize, std::ptr::null_mut(), &[0xFFFFFFFF, 0x80000000]), (0xAAAA_BBBB, 0x1111_2222));
    assert_eq!(invoke(fc4p_00698d80 as *const () as usize, std::ptr::null_mut(), &[0xFFFF_FFFF]), (0xCCCC_DDDD, 0x3333_4444));
    assert_eq!(CALLS.with(|c| c.replace(vec![])), [(0x0069_5790, vec![0xFFFFFFFF, 0x80000000]), (0x0069_5840, vec![0xFFFFFFFF])]);
}
