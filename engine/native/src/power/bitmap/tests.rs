// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Inclusive rectangle cases moved from the core bitmap tests, with caller argument scratch words.
use super::*;
struct Frame {
    args: [u32; 4],
    eax: u32,
    ecx: u32,
}
fn run(entry: *const (), args: [u32; 4]) -> Frame {
    let mut f = [
        entry as u32,
        0xCAFE_F00D,
        args[0],
        args[1],
        args[2],
        args[3],
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    // SAFETY: push four cdecl words, capture the results and caller words, then reclaim all four words.
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 20]","push dword ptr [edi + 16]","push dword ptr [edi + 12]","push dword ptr [edi + 8]",
            "mov ecx, [edi + 4]","call dword ptr [edi]","mov [edi + 40], eax","mov [edi + 44], ecx",
            "mov eax, [esp]","mov [edi + 24], eax","mov eax, [esp + 4]","mov [edi + 28], eax",
            "mov eax, [esp + 8]","mov [edi + 32], eax","mov eax, [esp + 12]","mov [edi + 36], eax","add esp, 16",
            in("edi") f.as_mut_ptr(),clobber_abi("C"),
        );
    }
    Frame {
        args: [f[6], f[7], f[8], f[9]],
        eax: f[10],
        ecx: f[11],
    }
}
fn put(bits: &mut [u32], width: u32, r: &mut Rect, on: bool) -> Frame {
    run(
        if on {
            fc4p_00604180 as *const ()
        } else {
            fc4p_00604260 as *const ()
        },
        [bits.as_mut_ptr() as u32, width, r as *mut Rect as u32, 0xDEAD_BEEF],
    )
}
fn any(bits: &mut [u32], width: u32, r: &mut Rect) -> Frame {
    run(
        fc4p_00604350 as *const (),
        [bits.as_mut_ptr() as u32, width, r as *mut Rect as u32, 0xDEAD_BEEF],
    )
}
#[test]
fn inclusive_rectangles_set_clear_and_test_word_edges_and_caller_scratch() {
    let mut bits = [0u32; 32];
    let mut r = Rect {
        x0: 30,
        z0: 2,
        x1: 34,
        z1: 4,
    };
    let before = r;
    let result = put(&mut bits, 128, &mut r, true);
    let mut expected = [0u32; 32];
    for z in 2..=4 {
        expected[z * 4] = 0xC000_0000;
        expected[z * 4 + 1] = 7;
    }
    assert_eq!(bits, expected);
    assert_eq!(r, before);
    assert_eq!(result.args, [bits.as_ptr() as u32, 5, 21, 0xDEAD_BEEF]);
    let result = any(&mut bits, 128, &mut r);
    assert_eq!(result.eax, 1);
    assert_eq!(result.ecx, u32::MAX.wrapping_shl(r.x0));
    assert_eq!(
        result.args,
        [
            bits.as_ptr() as u32,
            4,
            unsafe { bits.as_ptr().add(8) } as u32,
            0xDEAD_BEEF
        ]
    );
    let result = put(&mut bits, 128, &mut r, false);
    assert_eq!(bits, [0u32; 32]);
    assert_eq!(result.args, [bits.as_ptr() as u32, 4, 21, 0xDEAD_BEEF]);
    let result = any(&mut bits, 128, &mut r);
    assert_eq!(result.eax, 0);
    assert_eq!(result.ecx, u32::MAX.wrapping_shl(r.x0));
    assert_eq!(result.args[2], unsafe { bits.as_ptr().add(20) } as u32);
}
#[test]
fn single_word_masks_include_both_ends_and_keep_argument_words() {
    for (x0, x1, mask) in [
        (0, 0, 1),
        (31, 31, 0x8000_0000),
        (0, 31, u32::MAX),
        (3, 8, 0x1F8),
        (12, 7, 0),
    ] {
        let mut bits = [0xAAAA_AAAAu32; 16];
        let mut r = Rect { x0, z0: 1, x1, z1: 2 };
        let rp = &mut r as *mut Rect as u32;
        let result = put(&mut bits, 64, &mut r, true);
        assert_eq!(bits[2], 0xAAAA_AAAA | mask);
        assert_eq!(bits[4], 0xAAAA_AAAA | mask);
        assert_eq!(result.args, [bits.as_ptr() as u32, 64, rp, 0xDEAD_BEEF]);
        let result = put(&mut bits, 64, &mut r, false);
        assert_eq!(bits[2], 0xAAAA_AAAA & !mask);
        assert_eq!(bits[4], 0xAAAA_AAAA & !mask);
        assert_eq!(result.args, [bits.as_ptr() as u32, 2, rp, 0xDEAD_BEEF]);
        assert_eq!(any(&mut bits, 64, &mut r).eax as u8, 0);
    }
}
#[test]
fn multiword_fill_changes_interiors_and_no_other_words() {
    let mut bits = [0x5555_5555u32; 32];
    let mut r = Rect {
        x0: 5,
        z0: 1,
        x1: 98,
        z1: 2,
    };
    put(&mut bits, 128, &mut r, true);
    let mut expected = [0x5555_5555u32; 32];
    for z in 1..=2 {
        expected[z * 4] |= 0xFFFF_FFE0;
        expected[z * 4 + 1] = u32::MAX;
        expected[z * 4 + 2] = u32::MAX;
        expected[z * 4 + 3] |= 7;
    }
    assert_eq!(bits, expected);
    put(&mut bits, 128, &mut r, false);
    for z in 1..=2 {
        expected[z * 4] &= 31;
        expected[z * 4 + 1] = 0;
        expected[z * 4 + 2] = 0;
        expected[z * 4 + 3] &= !7;
    }
    assert_eq!(bits, expected);
}
#[test]
fn any_keeps_upper_eax_loop_result_and_edge_mask_in_ecx_and_stops_on_each_word() {
    let mut bits = [0u32; 512];
    let mut r = Rect {
        x0: 0,
        z0: 300,
        x1: 0,
        z1: 300,
    };
    bits[300] = 1;
    assert_eq!(any(&mut bits, 32, &mut r).eax, 0x101);
    bits[300] = 0;
    assert_eq!(any(&mut bits, 32, &mut r).eax, 0x100);
    r = Rect {
        x0: 5,
        z0: 75,
        x1: 98,
        z1: 76,
    };
    for i in [300, 301, 302, 303, 304, 307] {
        bits.fill(0);
        bits[i] = u32::MAX;
        let result = any(&mut bits, 128, &mut r);
        assert_eq!(result.eax, 0x101);
        assert_eq!(result.ecx, u32::MAX.wrapping_shl(r.x0));
        assert_eq!(
            result.args[2],
            unsafe { bits.as_ptr().add(if i >= 304 { 304 } else { 300 }) } as u32
        );
    }
    bits.fill(0);
    bits[300] = 31;
    bits[303] = !7;
    assert_eq!(any(&mut bits, 128, &mut r).eax, 0x100);
}
#[test]
fn reversed_rows_skip_memory_but_keep_argument_changes() {
    let mut bits = [0xA5A5_A5A5u32; 32];
    let before = bits;
    for (x0, x1) in [(1, 4), (1, 63)] {
        let mut r = Rect { x0, z0: 3, x1, z1: 2 };
        let rp = &mut r as *mut Rect as u32;
        let set = put(&mut bits, 128, &mut r, true);
        assert_eq!(set.args[1], if x1 < 32 { 128 } else { 3 });
        assert_eq!(set.args[2], rp);
        let clear = put(&mut bits, 128, &mut r, false);
        assert_eq!(clear.args[1], 4);
        assert_eq!(clear.args[2], rp);
        let tested = any(&mut bits, 128, &mut r);
        assert_eq!(tested.eax, 0);
        assert_eq!(tested.ecx, u32::MAX.wrapping_shl(r.x0));
        assert_eq!(tested.args[1], if x1 < 32 { 128 } else { 2 });
        assert_eq!(tested.args[2], rp);
        assert_eq!(bits, before);
    }
}
#[test]
fn nonmultiple_width_uses_floor_stride_and_signed_start_index_without_clipping() {
    let mut bits = [0u32; 16];
    let mut r = Rect {
        x0: 1,
        z0: 1,
        x1: 2,
        z1: 3,
    };
    put(&mut bits, 40, &mut r, true);
    assert_eq!(&bits[..5], &[0, 6, 6, 6, 0]);
    r = Rect {
        x0: 63,
        z0: 0,
        x1: 0,
        z1: 0,
    };
    put(&mut bits, 64, &mut r, true);
    assert_eq!(bits[0], 1);
    assert_eq!(bits[1], 0x8000_0006);
    r = Rect {
        x0: u32::MAX,
        z0: 0,
        x1: u32::MAX,
        z1: 0,
    };
    let p = unsafe { bits.as_mut_ptr().add(4) };
    run(
        fc4p_00604180 as *const (),
        [p as u32, 64, &mut r as *mut Rect as u32, 0],
    );
    assert_eq!(bits[3], 0x8000_0006);
}
#[test]
fn rect_or_masks_edges_and_repeats_first_word_for_the_interior() {
    let mut src = [0u32; 24];
    let mut dst = [0x1000u32; 24];
    src[4] = u32::MAX;
    src[5] = 0x1234;
    src[6] = 0x5678;
    src[7] = u32::MAX;
    let mut r = Rect {
        x0: 5,
        z0: 1,
        x1: 98,
        z1: 1,
    };
    let result = run(
        fc4p_00604440 as *const (),
        [
            dst.as_mut_ptr() as u32,
            src.as_mut_ptr() as u32,
            128,
            &mut r as *mut Rect as u32,
        ],
    );
    assert_eq!(&dst[4..8], &[u32::MAX, 0x1000, 0x1000, 0x1007]);
    assert_eq!(
        result.args,
        [
            dst.as_ptr() as u32,
            src.as_ptr() as u32,
            9,
            (src.as_ptr() as u32).wrapping_sub(dst.as_ptr() as u32)
        ]
    );
    r = Rect {
        x0: 0,
        z0: 1,
        x1: 0,
        z1: 1,
    };
    let result = run(
        fc4p_00604440 as *const (),
        [
            dst.as_mut_ptr() as u32,
            src.as_mut_ptr() as u32,
            128,
            &mut r as *mut Rect as u32,
        ],
    );
    assert_eq!(result.args[2], 128);
    assert_eq!(result.args[3], &mut r as *mut Rect as u32);
    dst.fill(0);
    r = Rect {
        x0: 5,
        z0: 1,
        x1: 34,
        z1: 1,
    };
    run(
        fc4p_00604440 as *const (),
        [
            dst.as_mut_ptr() as u32,
            src.as_mut_ptr() as u32,
            128,
            &mut r as *mut Rect as u32,
        ],
    );
    assert_eq!(dst[4], 0xFFFF_FFE0);
    assert_eq!(dst[5], 4);
}

#[allow(clippy::too_many_arguments)]
fn grow_run(
    entry: *const (),
    bits: *mut u32,
    mask: *mut u32,
    width: i32,
    budget: i32,
    lo: *mut i32,
    hi: *mut i32,
) -> (u32, u32) {
    let mut f = [
        entry as u32,
        bits as u32,
        mask as u32,
        width as u32,
        budget as u32,
        lo as u32,
        hi as u32,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 24]","push dword ptr [edi + 20]","push dword ptr [edi + 16]",
            "push dword ptr [edi + 12]","push dword ptr [edi + 8]","push dword ptr [edi + 4]",
            "call dword ptr [edi]","mov [edi + 28], eax","mov eax, [esp + 8]","mov [edi + 32], eax","add esp, 24",
            in("edi") f.as_mut_ptr(),clobber_abi("C"),
        );
    }
    (f[7], f[8])
}
#[test]
fn horizontal_grows_across_word_edges_without_cascading_or_crossing_rows() {
    let mut bits = [0u32; 32];
    let mut mask = [u32::MAX; 32];
    bits[4] = 0x8000_0001;
    bits[5] = 0x8000_0000;
    bits[7] = 0x8000_0000;
    let (mut lo, mut hi) = (1i32, 2i32);
    let result = run_five(
        fc4p_00603e50 as *const (),
        [
            bits.as_mut_ptr() as u32,
            mask.as_mut_ptr() as u32,
            128,
            &mut lo as *mut i32 as u32,
            &mut hi as *mut i32 as u32,
        ],
    );
    assert_eq!(&bits[4..8], &[0xC000_0003, 0xC000_0001, 1, 0xC000_0000]);
    assert_eq!(&bits[..4], &[0; 4]);
    assert_eq!(&bits[8..12], &[0; 4]);
    assert_eq!((lo, hi), (1, 2));
    assert_eq!(result.0 as u8, 1);
    assert_eq!(result.1, 2);
}
fn run_five(entry: *const (), args: [u32; 5]) -> (u32, u32) {
    let mut f = [entry as u32, args[0], args[1], args[2], args[3], args[4], 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 20]","push dword ptr [edi + 16]","push dword ptr [edi + 12]","push dword ptr [edi + 8]","push dword ptr [edi + 4]",
            "call dword ptr [edi]","mov [edi + 24], eax","mov eax, [esp + 8]","mov [edi + 28], eax","add esp, 20",
            in("edi") f.as_mut_ptr(),clobber_abi("C"),
        );
    }
    (f[6], f[7])
}
#[test]
fn growth_respects_mask_and_reports_mismatches_even_when_old_bits_stay_set() {
    let mut bits = [0u32; 32];
    let mut mask = [0u32; 32];
    bits[4] = 4;
    mask[4] = 8;
    let (mut lo, mut hi) = (2i32, 3i32);
    let r = run_five(
        fc4p_00603e50 as *const (),
        [
            bits.as_mut_ptr() as u32,
            mask.as_mut_ptr() as u32,
            64,
            &mut lo as *mut i32 as u32,
            &mut hi as *mut i32 as u32,
        ],
    );
    assert_eq!(bits[4], 12);
    assert_eq!(r.0 as u8, 1);
    assert_eq!((lo, hi), (2, 3));
    mask.fill(0);
    lo = 2;
    hi = 3;
    let r = run_five(
        fc4p_00603e50 as *const (),
        [
            bits.as_mut_ptr() as u32,
            mask.as_mut_ptr() as u32,
            64,
            &mut lo as *mut i32 as u32,
            &mut hi as *mut i32 as u32,
        ],
    );
    assert_eq!(bits[4], 12);
    assert_eq!(r.0 as u8, 1);
    assert_eq!((lo, hi), (2, 3));
}
#[test]
fn vertical_is_banded_and_keeps_old_row_words_for_each_column() {
    let mut bits = [0u32; 32];
    let mut mask = [u32::MAX; 32];
    bits[4] = 4;
    bits[5] = 0x100;
    let (mut lo, mut hi) = (1i32, 5i32);
    let result = unsafe { fc4p_00603fd0(bits.as_mut_ptr(), mask.as_mut_ptr(), 64, &mut lo, &mut hi) };
    assert_eq!(result as u8, 1);
    assert_eq!((lo, hi), (1, 4));
    for row in 1..=3 {
        assert_eq!(&bits[row * 2..row * 2 + 2], &[4, 0x100]);
    }
    assert_eq!(&bits[8..10], &[0, 0]);
    bits.fill(0);
    bits[4] = 4;
    lo = 2;
    hi = 2;
    unsafe { fc4p_00603fd0(bits.as_mut_ptr(), mask.as_mut_ptr(), 64, &mut lo, &mut hi) };
    assert_eq!(bits[6], 4);
    assert_eq!((lo, hi), (3, 4));
}
#[test]
fn empty_growth_bands_reset_bounds_and_keep_width_scratch_rules() {
    let mut bits = [0u32; 16];
    let mut mask = [u32::MAX; 16];
    let (mut lo, mut hi) = (3i32, 2i32);
    let result = run_five(
        fc4p_00603e50 as *const (),
        [
            bits.as_mut_ptr() as u32,
            mask.as_mut_ptr() as u32,
            64,
            &mut lo as *mut i32 as u32,
            &mut hi as *mut i32 as u32,
        ],
    );
    assert_eq!((lo, hi), (64, 0));
    assert_eq!(result.0 as u8, 0);
    assert_eq!(result.1, 3);
    for width in [0, -1, i32::MIN] {
        lo = 1;
        hi = 3;
        assert_eq!(
            unsafe { fc4p_00603fd0(bits.as_mut_ptr(), mask.as_mut_ptr(), width, &mut lo, &mut hi) },
            0
        );
        assert_eq!((lo, hi), (width, 0));
    }
}
#[test]
fn selection_grow_connects_an_l_shape_but_keeps_a_diagonal_isolated() {
    let mut bits = [0u32; 128];
    let mut mask = [0u32; 128];
    for x in 1..=6 {
        mask[4] |= 1 << x;
    }
    for z in 2..=9 {
        mask[z * 2] |= 1 << 6;
    }
    mask[20] |= 1 << 7;
    bits[4] = 1 << 1;
    let (mut lo, mut hi) = (2i32, 3i32);
    let result = grow_run(
        fc4p_00604570 as *const (),
        bits.as_mut_ptr(),
        mask.as_mut_ptr(),
        64,
        600,
        &mut lo,
        &mut hi,
    );
    assert_eq!(bits[4] & 0x7E, 0x7E);
    for z in 2..=9 {
        assert_ne!(bits[z * 2] & (1 << 6), 0);
    }
    assert_eq!(bits[20], 0);
    assert!(lo <= 2 && hi >= 9);
    assert!(result.0 < 600);
    assert_eq!(result.0, result.1);
    bits.fill(0);
    bits[4] = 1 << 1;
    lo = 2;
    hi = 3;
    assert_eq!(
        grow_run(
            fc4p_00604570 as *const (),
            bits.as_mut_ptr(),
            mask.as_mut_ptr(),
            64,
            1,
            &mut lo,
            &mut hi
        ),
        (1, 1)
    );
    assert_ne!(bits[4] & (1 << 2), 0);
    assert_eq!(bits[4] & (1 << 3), 0);
    assert_eq!((lo, hi), (2, 3));
}
thread_local! {static GROW_CALLS:std::cell::RefCell<Vec<(u32,i32,i32,i32)>>=const {std::cell::RefCell::new(Vec::new())};}
extern "C" fn horizontal_fake(_bits: *mut u32, _mask: *mut u32, width: i32, lo: *mut i32, hi: *mut i32) -> u32 {
    unsafe {
        GROW_CALLS.with(|c| c.borrow_mut().push((0, width, lo.read(), hi.read())));
        lo.write(4);
        hi.write(7);
    }
    0xABCD_EF01
}
extern "C" fn vertical_fake(_bits: *mut u32, _mask: *mut u32, width: i32, lo: *mut i32, hi: *mut i32) -> u32 {
    unsafe {
        GROW_CALLS.with(|c| c.borrow_mut().push((1, width, lo.read(), hi.read())));
        lo.write(3);
        hi.write(9);
    }
    0xABCD_EF00
}
#[test]
fn selection_grow_calls_original_pass_addresses_and_publishes_at_next_iteration_start() {
    let _h = crate::testing::fake(0x0060_3E50, horizontal_fake as *const () as usize);
    let _v = crate::testing::fake(0x0060_3FD0, vertical_fake as *const () as usize);
    for (budget, expected_lo, expected_hi, expected) in [
        (0, 5, 6, vec![]),
        (-1, 5, 6, vec![]),
        (1, 5, 6, vec![(0, 64, 5, 6), (1, 64, 4, 7)]),
        (
            2,
            3,
            9,
            vec![(0, 64, 5, 6), (1, 64, 4, 7), (0, 64, 3, 9), (1, 64, 2, 10)],
        ),
    ] {
        let (mut lo, mut hi) = (5i32, 6i32);
        let r = grow_run(
            fc4p_00604570 as *const (),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            64,
            budget,
            &mut lo,
            &mut hi,
        );
        assert_eq!(r, (budget.max(0) as u32, budget.max(0) as u32));
        assert_eq!((lo, hi), (expected_lo, expected_hi));
        assert_eq!(GROW_CALLS.with(|c| std::mem::take(&mut *c.borrow_mut())), expected);
    }
}
