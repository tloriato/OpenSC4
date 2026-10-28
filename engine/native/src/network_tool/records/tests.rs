// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data};
use std::cell::RefCell;

thread_local! {
    static COPIES: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
    static CELL: RefCell<(usize, usize, u32, u32)> = const { RefCell::new((0, 0, 0, 0)) };
}
extern "thiscall" fn copy(this: *mut u8, source: *mut u8) -> u64 {
    COPIES.with(|s| s.borrow_mut().push((this as usize, source as usize)));
    unsafe {
        for offset in (0..12).step_by(4) {
            write(this, offset, read::<u32>(source, offset));
        }
    }
    registers(0xDEAD_1234, 0xABCD_9876)
}
extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    CELL.with(|s| {
        let s = *s.borrow();
        assert_eq!((this as usize, key), (s.0, s.2));
        unsafe {
            let context = CellContext::from_ptr(s.1 as *mut u8);
            assert_eq!(context.flags(), 0);
            assert_eq!((context.begin(), context.end(), context.capacity()), (0, 0, 0));
            assert_eq!(context.other_types(), s.3);
        }
        this
    })
}
extern "thiscall" fn missing(_this: *mut u8, _key: u32) -> *mut u8 {
    core::ptr::null_mut()
}

#[unsafe(naked)]
unsafe extern "C" fn keep_registers() {
    core::arch::naked_asm!("ret 4")
}

#[test]
fn self_assignment_keeps_the_registers_supplied_to_the_member_helpers() {
    let _f = fake(0x0062_4EA0, keep_registers as *const () as usize);
    let _g = fake(0x0050_DB90, keep_registers as *const () as usize);
    let mut parameters = [0xA5A5_A5A5u32; 20];
    parameters[0x3C / 4] = 0x1234_5678;
    let p = parameters.as_mut_ptr().cast::<u8>();
    let expected = parameters;
    assert_eq!(unsafe { fc4p_006267d0(p, p) }, registers(p as u32, 0x1234_5678));
    assert_eq!(parameters, expected);
    let mut records = [0xB5B5_B5B5u32; 20];
    records[5] = 0x8765_4321;
    records[15] = 0xDEAD_BEEF;
    let p = records.as_mut_ptr().cast::<u8>();
    let expected = records;
    assert_eq!(
        unsafe { fc4p_00627220(p, p.add(80), p) },
        registers(p as u32 + 80, 0xDEAD_BEEF)
    );
    assert_eq!(records, expected);
}

#[test]
fn parameter_assignment_keeps_padding_calls_member_and_supports_overlapping_storage() {
    let _f = fake(0x0062_4EA0, copy as *const () as usize);
    for displacement in [0, 4, 0x60] {
        let mut bytes = [0xA5u8; 0xC0];
        for (i, byte) in bytes[..0x50].iter_mut().enumerate() {
            *byte = i as u8;
        }
        let source = bytes.as_mut_ptr();
        let destination = unsafe { source.add(displacement) };
        let mut expected = bytes;
        for offset in [0, 4, 8, 12] {
            let value = u32::from_le_bytes(expected[offset..offset + 4].try_into().unwrap());
            expected[displacement + offset..displacement + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        expected[displacement + 0x10] = expected[0x10];
        for offset in (0x14..0x40).step_by(4) {
            let value = u32::from_le_bytes(expected[offset..offset + 4].try_into().unwrap());
            expected[displacement + offset..displacement + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        expected[displacement + 0x40] = expected[0x40];
        for offset in (0x44..0x50).step_by(4) {
            let value = u32::from_le_bytes(expected[offset..offset + 4].try_into().unwrap());
            expected[displacement + offset..displacement + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        COPIES.with(|s| s.borrow_mut().clear());
        let result = unsafe { fc4p_006267d0(destination, source) };
        assert_eq!(result, registers(destination as u32, 0xABCD_9876));
        assert_eq!(bytes, expected);
        COPIES.with(|s| assert_eq!(*s.borrow(), [(destination as usize + 0x44, source as usize + 0x44)]));
    }
}

#[test]
fn context_uses_wrapping_cell_key_low_boolean_byte_and_exact_fields() {
    let mut masks = [0u32; 13];
    masks[3] = 0x8123_4567;
    let _g = fake_data(0x00AA_835C, masks.as_mut_ptr().cast());
    let _f = fake(0x0063_3220, lookup as *const () as usize);
    for boolean in [0, 0x100, 1, 0xFFFF_0001] {
        let mut context = [0xA5u8; 0x30];
        let mut cell = [0u32; 20];
        cell[9] = 0x1234_5678;
        let cp = context.as_mut_ptr();
        let tool = cell.as_mut_ptr().cast::<u8>();
        let mask = if boolean as u8 == 0 { masks[3] ^ 0x1FFF } else { 0 };
        CELL.with(|s| *s.borrow_mut() = (tool as usize, cp as usize, 0xFFFE_FFFE, mask));
        assert_eq!(unsafe { fc4p_00627080(cp, 0xFFFF_FFFE, 0xFFFF, tool, 3, boolean) }, cp);
        assert_eq!(&context[0x28..], &[0xA5; 8]);
        let c = unsafe { CellContext::from_ptr(cp) };
        assert_eq!(
            (
                c.vtable(),
                c.flags(),
                c.x(),
                c.z(),
                c.tool(),
                c.network_type(),
                c.other_types()
            ),
            (0x00AA_8700, cell[9], 0xFFFF_FFFE, 0xFFFF, tool, 3, mask)
        );
    }
    let _f = fake(0x0063_3220, missing as *const () as usize);
    let mut context = [0xA5u8; 0x28];
    unsafe {
        fc4p_00627080(context.as_mut_ptr(), 0, 0, core::ptr::null_mut(), u32::MAX, 1);
    }
    assert_eq!(unsafe { read::<u32>(context.as_ptr(), 4) }, 0);
}

#[test]
fn range_assignment_truncates_signed_distances_and_keeps_last_helper_edx() {
    let _f = fake(0x0050_DB90, copy as *const () as usize);
    for distance in [-81i32, -40, -1, 0, 39, 40, 79, 80, 81] {
        let mut source = [0u32; 30];
        for (i, word) in source.iter_mut().enumerate() {
            *word = 0x5500_0000 | i as u32;
        }
        let mut destination = [0xAAAA_AAAAu32; 30];
        let first = source.as_mut_ptr().cast::<u8>();
        let out = destination.as_mut_ptr().cast::<u8>();
        COPIES.with(|s| s.borrow_mut().clear());
        let result = unsafe { fc4p_00627220(first, first.wrapping_offset(distance as isize), out) };
        let count = (distance / 40).max(0) as usize;
        assert_eq!(result as u32, out as u32 + count as u32 * 40);
        assert_eq!(
            (result >> 32) as u32,
            if count != 0 {
                0xABCD_9876
            } else {
                ((i64::from(distance) * 0x6666_6667) >> 36) as u32
            }
        );
        assert_eq!(&destination[..count * 10], &source[..count * 10]);
        assert!(destination[count * 10..].iter().all(|&w| w == 0xAAAA_AAAA));
        COPIES.with(|s| {
            assert_eq!(
                *s.borrow(),
                (0..count)
                    .map(|i| (out as usize + i * 40 + 28, first as usize + i * 40 + 28))
                    .collect::<Vec<_>>()
            )
        });
    }
}
