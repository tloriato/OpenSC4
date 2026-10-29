// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::construction::*;

slot!(refresh_building, lot);
slot!(refresh_lot, lot);
slot!(refresh_texture, lot, refresh);
slot!(finish_elements);
slot!(foundation_choice, purpose);
float_slot!(lot_slope_float, lot_slope_body, 2, lot, water);
float_slot!(maximum_slope_float, maximum_slope_body, 0);

unsafe extern "thiscall" fn altitude_update(this: *mut u8, lot: u32) -> u64 {
    run(this, "altitude_update", &[lot])
}

/// Capture the return registers, restored ECX, and the two argument words after the callee pops them.
unsafe fn elements(this: *mut u8, lot: *mut u8, kind: u32) -> [u32; 5] {
    let mut words = [0, 0, 0, lot as u32, kind];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 16]", "push dword ptr [edi + 12]", "call {function}",
            "mov [edi], eax", "mov [edi + 4], edx", "mov [edi + 8], ecx",
            "mov eax, [esp - 8]", "mov [edi + 12], eax",
            "mov eax, [esp - 4]", "mov [edi + 16], eax",
            function = sym fc4p_006cc880, in("edi") words.as_mut_ptr(),
            in("ecx") this, in("edx") 0x5566_7788u32, clobber_abi("C"),
        );
    }
    words
}

#[test]
fn elements_early_states_keep_arguments_and_return_high_bytes() {
    let mut developer = mock!();
    let mut lot_object = mock!(0x84 => habitation_state);
    let (this, lot) = (developer.ptr(), lot_object.ptr());
    for state in [i32::MIN, -1, 0, 1] {
        expect(
            lot,
            "habitation_state",
            vec![],
            (RETURN & !0xFFFF_FFFF) | state as u32 as u64,
        );
        let actual = unsafe { elements(this, lot, 0xFFFF_0005) };
        assert_eq!(
            actual,
            [
                (state as u32 & !255) | 1,
                (RETURN >> 32) as u32,
                this as u32,
                lot as u32,
                0xFFFF_0005
            ]
        );
        finished();
    }
}

#[test]
fn elements_existing_foundations_refresh_both_interfaces_before_texture_and_lot() {
    let mut developer = mock!(0xF4 => refresh_building, 0xF8 => refresh_lot, 0xB4 => refresh_texture);
    let mut lot_object = mock!(0x84 => habitation_state, 0x30 => get_flags, 0x50 => finish_elements);
    let (this, lot) = (developer.ptr(), lot_object.ptr());
    for flags in [1, 2, 3, 0x8000_0001] {
        expect(lot, "habitation_state", vec![], 2);
        expect(lot, "get_flags", vec![], flags);
        expect(this, "refresh_building", vec![p(lot)], RETURN);
        expect(this, "refresh_lot", vec![p(lot)], RETURN);
        expect(this, "refresh_texture", vec![p(lot), w(this as u32 & !255)], RETURN);
        expect(lot, "finish_elements", vec![], RETURN);
        assert_eq!(
            unsafe { elements(this, lot, 5) },
            [0xA1B2_C301, 0x9876_5432, this as u32 & !255, lot as u32, 5]
        );
        finished();
    }
}

#[test]
fn elements_kind_comparison_uses_all_bits_and_missing_configuration_skips_slope() {
    let mut developer = mock!(0xB4 => refresh_texture);
    let mut lot_object =
        mock!(0x84 => habitation_state, 0x30 => get_flags, 0x24 => get_config, 0x50 => finish_elements);
    let (this, lot) = (developer.ptr(), lot_object.ptr());
    for kind in [5, 0x105, 0x100, u32::MAX] {
        expect(lot, "habitation_state", vec![], 3);
        expect(lot, "get_flags", vec![], 0x100);
        if kind != 5 {
            expect(lot, "get_config", vec![], 0);
        }
        expect(this, "refresh_texture", vec![p(lot), w(this as u32 & !255)], RETURN);
        expect(lot, "finish_elements", vec![], RETURN);
        assert_eq!(unsafe { elements(this, lot, kind) }[3..], [lot as u32, kind]);
        finished();
    }
}

#[test]
fn elements_slope_branches_and_caller_scratch_words() {
    let _fpu = GameFpu::new();
    let mut quarter = 0.25f32;
    let _threshold = fake_data(0x00A8_821C, (&mut quarter as *mut f32).cast());
    let _altitude = fake(0x006C_91F0, altitude_update as *const () as usize);
    let mut developer = mock!(0x78 => lot_slope_float, 0xD8 => add_lot_config, 0xB4 => refresh_texture);
    let mut lot_object =
        mock!(0x84 => habitation_state, 0x30 => get_flags, 0x24 => get_config, 0x50 => finish_elements);
    let mut configuration = mock!(0x38 => get_flags, 0x28 => maximum_slope_float, 0x6C => foundation_choice);
    let (this, lot, config) = (developer.ptr(), lot_object.ptr(), configuration.ptr());
    // The full kind compare and foundation case also carry the core's kind 0x105 / 0x100 vectors.
    for (slope, maximum, kind, foundation, altitude) in [
        (4.0, 0.0, 0x105, true, true),
        (4.0, 0.0, 0x100, true, true),
        (0.25, 0.25, 7, false, false),
        (-0.25, 0.0, 7, false, false),
        (-0.5, 0.0, 7, false, true),
        (0.0, -1.0, 7, true, false),
        (f32::NAN, 0.0, 7, false, false),
        (4.0, f32::NAN, 7, false, true),
        (f32::INFINITY, 0.0, 7, true, true),
    ] {
        expect(lot, "habitation_state", vec![], 3);
        expect(lot, "get_flags", vec![], 0);
        expect(lot, "get_config", vec![], config as u64);
        expect(config, "get_flags", vec![], 0x1234_5601);
        expect_float(
            this,
            "lot_slope_body",
            vec![p(lot), w((lot as u32 & !255) | 1)],
            RETURN,
            F80::from_f32(slope),
        );
        expect_float(config, "maximum_slope_body", vec![], RETURN, F80::from_f32(maximum));
        if foundation {
            expect(config, "foundation_choice", vec![w(7)], 0x1122_3380);
            expect(
                this,
                "add_lot_config",
                vec![p(lot), p(config), w((kind & !255) | 0x80), w(0), w(1)],
                RETURN,
            );
        }
        if altitude {
            expect(this, "altitude_update", vec![p(lot)], RETURN);
        }
        let flag = (this as u32 & !255) | u32::from(foundation);
        expect(this, "refresh_texture", vec![p(lot), w(flag)], RETURN);
        expect(lot, "finish_elements", vec![], RETURN);
        let actual = unsafe { elements(this, lot, kind) };
        assert_eq!(&actual[..3], &[0xA1B2_C301, 0x9876_5432, flag]);
        assert_eq!(actual[3], slope.to_bits());
        assert_eq!(actual[4], if foundation { (kind & !255) | 0x80 } else { kind });
        finished();
    }
}
