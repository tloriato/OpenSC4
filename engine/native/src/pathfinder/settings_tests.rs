// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration, result and reset expectations carried from the core pathfinder tests.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{call_thiscall_with_eax, GameFpu, Object};

fn object() -> Object {
    let mut o = Object::new(0x3160);
    unsafe { core::ptr::write_bytes(o.ptr(), 0xA5, 0x3160) };
    o
}

fn bytes(o: &mut Object) -> Vec<u8> {
    unsafe { core::slice::from_raw_parts(o.ptr(), 0x3160).to_vec() }
}

fn changed_only(o: &mut Object, before: &[u8], offset: usize, expected: &[u8]) {
    let mut want = before.to_vec();
    want[offset..offset + expected.len()].copy_from_slice(expected);
    assert_eq!(bytes(o), want);
}

#[test]
fn scalar_setters_store_only_their_field_and_return_the_supplied_word() {
    let functions = [
        (fc4p_006d6d30 as *const () as usize, 0x80),
        (fc4p_006d6de0 as *const () as usize, 0x90),
        (fc4p_006d6df0 as *const () as usize, 0x60),
        (fc4p_006d6e00 as *const () as usize, 0x68),
        (fc4p_006d6e70 as *const () as usize, 0xC0),
    ];
    for (f, offset) in functions {
        // Include null, out-of-range filter values, signed zero and a signaling NaN's raw bits.
        for value in [0, 3, u32::MAX, 0x8000_0000, 0x7F80_0123] {
            let mut o = object();
            let before = bytes(&mut o);
            let eax = unsafe { call_thiscall_with_eax(f, o.ptr(), &[value], 0x1234_5678).0 };
            assert_eq!(eax, value);
            changed_only(&mut o, &before, offset, &value.to_le_bytes());
        }
    }
}

#[test]
fn flags_accumulate_and_clears_preserve_entry_eax() {
    let mut o = object();
    let pf = unsafe { Pathfinder::from_ptr(o.ptr()) };
    pf.set_flags(0x8000_0001);
    let before = bytes(&mut o);
    assert_eq!(unsafe { fc4p_006d6cf0(o.ptr(), 0x402) }, 0x8000_0403_0000_0402);
    changed_only(&mut o, &before, 0x70, &0x8000_0403u32.to_le_bytes());
    let before = bytes(&mut o);
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006d6d20 as *const () as usize, o.ptr(), &[], 0x8765_4321).0 },
        0x8765_4321
    );
    changed_only(&mut o, &before, 0x70, &[0; 4]);
}

#[test]
fn destination_flags_are_one_byte_and_return_the_arguments_low_byte() {
    for mask in [0, 2, 0xFFFF_0100, 0x1234_5678, u32::MAX] {
        let mut o = object();
        let pf = unsafe { Pathfinder::from_ptr(o.ptr()) };
        pf.set_destinations(5);
        let before = bytes(&mut o);
        let eax =
            unsafe { call_thiscall_with_eax(fc4p_006d6d40 as *const () as usize, o.ptr(), &[mask], 0xABCD_1234).0 };
        assert_eq!(eax, 0xABCD_1200 | (mask & 255));
        changed_only(&mut o, &before, 0x84, &[5 | mask as u8]);
        let before = bytes(&mut o);
        assert_eq!(
            unsafe { call_thiscall_with_eax(fc4p_006d6d80 as *const () as usize, o.ptr(), &[], 0xABCD_1234).0 },
            0xABCD_1234
        );
        changed_only(&mut o, &before, 0x84, &[0]);
    }
}

#[test]
fn travel_masks_use_low_word_and_low_byte_without_normalizing_the_arguments() {
    for flag in [0, 1, 2, 0x80, 0x100, 0xFFFF_0000, u32::MAX] {
        for mask in [0, 2, 0x100, 0xFFFF_0001, u32::MAX] {
            let mut o = object();
            let pf = unsafe { Pathfinder::from_ptr(o.ptr()) };
            pf.set_allowed(0xA001);
            pf.set_starting(0x4000);
            let before = bytes(&mut o);
            assert_eq!(
                unsafe { call_thiscall_with_eax(fc4p_006d6d90 as *const () as usize, o.ptr(), &[mask, flag], 0).0 },
                mask
            );
            let starting = if flag as u8 != 0 { 0x4000 | mask as u16 } else { 0x4000 };
            let expected = [0xA001 | mask as u16, starting].map(u16::to_le_bytes).concat();
            changed_only(&mut o, &before, 0x64, &expected);
            let before = bytes(&mut o);
            assert_eq!(
                unsafe { call_thiscall_with_eax(fc4p_006d6dd0 as *const () as usize, o.ptr(), &[], 0).0 },
                0
            );
            changed_only(&mut o, &before, 0x64, &[0; 4]);
        }
    }
}

#[test]
fn rectangles_keep_inverted_and_signed_bounds_and_both_return_registers() {
    for (f, offset) in [
        (fc4p_006d6e10 as *const () as usize, 0x18),
        (fc4p_006d6e40 as *const () as usize, 0x28),
    ] {
        for rect in [[2, 2, 4, 4], [5, 6, -2, -3], [i32::MIN, i32::MAX, i32::MIN, i32::MAX]] {
            let mut o = object();
            let before = bytes(&mut o);
            let args = rect.map(|v| v as u32);
            let (eax, edx) = unsafe { call_thiscall_with_eax(f, o.ptr(), &args, 0) };
            assert_eq!((eax, edx), (args[2], args[3]));
            changed_only(&mut o, &before, offset, &args.map(u32::to_le_bytes).concat());
        }
    }
}

#[test]
fn reset_keeps_the_result_and_all_other_search_state() {
    let mut o = object();
    let before = bytes(&mut o);
    let eax = unsafe { call_thiscall_with_eax(fc4p_006d6e80 as *const () as usize, o.ptr(), &[], 0xABC0_1234).0 };
    assert_eq!(eax, 0xABC0_1234);
    changed_only(&mut o, &before, 0xC4, &[0]);
}

#[test]
fn result_getters_preserve_the_object_and_return_full_count_and_st0() {
    let _fpu = GameFpu::new();
    let mut o = object();
    let pf = unsafe { Pathfinder::from_ptr(o.ptr()) };
    for count in [0, 10, 0x8000_0000, u32::MAX] {
        pf.set_arrived(count);
        let before = bytes(&mut o);
        assert_eq!(
            unsafe { call_thiscall_with_eax(fc4p_00698ce0 as *const () as usize, o.ptr(), &[], 0).0 },
            count
        );
        assert_eq!(bytes(&mut o), before);
    }
    for bits in [0, 0x8000_0000, 0x4040_0000, 0x7F80_0000, 0x7FC0_0123] {
        pf.set_trip_length_bits(bits);
        let before = bytes(&mut o);
        let mut call = RawCall::new(Conv::Thiscall, fc4p_006d6e90 as *const () as usize, o.ptr() as u32, &[]);
        call.eax = 0x1234_ABCD;
        call.take_st0 = 1;
        let top = crate::x87::status_word() & 0x3800;
        unsafe { call_raw(&mut call) };
        assert_eq!(call.eax, 0x1234_ABCD);
        assert_eq!(call.st0.to_f32().to_bits(), bits);
        assert_eq!(crate::x87::status_word() & 0x3800, top);
        assert_eq!(bytes(&mut o), before);
    }
}

#[test]
fn end_cell_writes_in_order_even_when_outputs_alias() {
    let mut o = object();
    let pf = unsafe { Pathfinder::from_ptr(o.ptr()) };
    pf.set_end_x(4);
    pf.set_end_z(-1);
    let before = bytes(&mut o);
    let (mut x, mut z) = (0, 0);
    let result = unsafe { fc4p_006d6ea0(o.ptr(), &mut x, &mut z) };
    assert_eq!((x, z), (4, -1));
    assert_eq!(result as u32, u32::MAX);
    assert_eq!(bytes(&mut o), before);
    unsafe { fc4p_006d6ea0(o.ptr(), &mut x, &mut x) };
    assert_eq!(x, -1);
    // The x output replaces the stored z before the z input is fetched.
    unsafe { fc4p_006d6ea0(o.ptr(), o.ptr().add(0x5C).cast(), &mut z) };
    assert_eq!(z, 4);
    changed_only(&mut o, &before, 0x5C, &4i32.to_le_bytes());
}

#[test]
fn settings_keep_edx_including_partial_byte_updates() {
    let mut o = object();
    let pf = unsafe { Pathfinder::from_ptr(o.ptr()) };
    for (f, args, expected_eax, expected_edx) in [
        (fc4p_006d6d20 as *const () as usize, vec![], 0x1122_3344, 0xAABB_CCDD),
        (fc4p_006d6d30 as *const () as usize, vec![3], 3, 0xAABB_CCDD),
        (
            fc4p_006d6d40 as *const () as usize,
            vec![0x1234_5602],
            0x1122_3302,
            0xAABB_CC07,
        ),
        (fc4p_006d6d80 as *const () as usize, vec![], 0x1122_3344, 0xAABB_CCDD),
        (
            fc4p_006d6d90 as *const () as usize,
            vec![0x9876_0002, 0x1122_3380],
            0x9876_0002,
            0xAABB_CC80,
        ),
        (fc4p_006d6dd0 as *const () as usize, vec![], 0, 0xAABB_CCDD),
        (fc4p_006d6de0 as *const () as usize, vec![0], 0, 0xAABB_CCDD),
        (
            fc4p_006d6df0 as *const () as usize,
            vec![0x7F80_0001],
            0x7F80_0001,
            0xAABB_CCDD,
        ),
        (fc4p_006d6e00 as *const () as usize, vec![10], 10, 0xAABB_CCDD),
        (fc4p_006d6e70 as *const () as usize, vec![5], 5, 0xAABB_CCDD),
        (fc4p_006d6e80 as *const () as usize, vec![], 0x1122_3344, 0xAABB_CCDD),
    ] {
        pf.set_destinations(5);
        let mut call = RawCall::new(Conv::Thiscall, f, o.ptr() as u32, &args);
        call.eax = 0x1122_3344;
        call.edx = 0xAABB_CCDD;
        unsafe { call_raw(&mut call) };
        assert_eq!((call.eax, call.edx), (expected_eax, expected_edx));
    }
}

#[test]
fn layout_fields_cover_the_expected_offsets_without_overlap() {
    let mut fields: Vec<_> = Pathfinder::FIELDS
        .iter()
        .map(|&(name, offset, size)| (offset, offset + size, name))
        .collect();
    fields.sort();
    assert!(fields.windows(2).all(|w| w[0].1 <= w[1].0));
    assert!(fields.iter().all(|f| f.1 <= 0x3160));
    for (name, offset) in [
        ("arrived", 0x6C),
        ("suspended", 0xC4),
        ("pages", 0x3148),
        ("next_node", 0x3154),
    ] {
        assert!(Pathfinder::FIELDS.iter().any(|&(n, o, _)| n == name && o == offset));
    }
}
