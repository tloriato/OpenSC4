// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

fn invoke(function: usize, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> (u32, u32) {
    let mut c = RawCall::new(Conv::Thiscall, function, this as u32, args);
    c.eax = eax;
    c.edx = edx;
    unsafe {
        call_raw(&mut c);
    }
    (c.eax, c.edx)
}

#[test]
fn planes_horizontal_vertical_oblique_degenerate_and_output_guards() {
    let _fpu = GameFpu::new();
    for (a, b, c, expected) in [
        ([0., 2., 0.], [1., 2., 0.], [0., 2., 1.], [0., -1., 0., 2.]),
        ([3., 0., 0.], [3., 1., 0.], [3., 0., 1.], [1., 0., 0., -3.]),
        ([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 1., 1., -1.]),
        ([0., 0., 0.], [1., 1., 1.], [2., 2., 2.], [0., 0., 0., -0.]),
    ] {
        let mut out = [123.5f32; 6];
        let result = unsafe { fc4p_005d67e0(a.as_ptr(), b.as_ptr(), c.as_ptr(), out.as_mut_ptr().add(1)) };
        assert_eq!(result, c.as_ptr());
        assert_eq!(
            out.map(f32::to_bits),
            [123.5, expected[0], expected[1], expected[2], expected[3], 123.5].map(f32::to_bits)
        );
    }
}

#[test]
fn plane_reloads_aliased_inputs_and_keeps_large_x87_intermediates() {
    let _fpu = GameFpu::new();
    let mut a = [1., 0., 0., 77.];
    let b = [0., 1., 0.];
    let c = [0., 0., 1.];
    unsafe {
        fc4p_005d67e0(a.as_ptr(), b.as_ptr(), c.as_ptr(), a.as_mut_ptr());
    }
    // First coefficient overwrites a.x with 1; second changes a.y to 1 before z is evaluated.
    assert_eq!(a, [1., 1., 1., -1.]);
    let large = 2.0f32.powi(100);
    let a = [0., large, large];
    let b = [0., large, large];
    let c = [0., -large, -large];
    let mut out = [0.; 4];
    unsafe {
        fc4p_005d67e0(a.as_ptr(), b.as_ptr(), c.as_ptr(), out.as_mut_ptr());
    }
    assert_eq!(out[..3], [0., 0., 0.]);
    assert_eq!(out[3].to_bits(), (-0.0f32).to_bits());
}

thread_local! {
    static GROW: RefCell<(usize, usize, usize, Vec<usize>)> = const { RefCell::new((0, 0, 0, Vec::new())) };
    static CONSTRUCT: RefCell<(usize, usize, bool, Vec<usize>)> = const { RefCell::new((0, 0, false, Vec::new())) };
}
extern "thiscall" fn grow(handle: *mut u32) -> *mut u8 {
    GROW.with(|s| {
        let mut s = s.borrow_mut();
        s.3.push(handle as usize);
        unsafe {
            (s.0 as *mut *mut u8).write(s.1 as *mut u8);
        }
        s.2 as *mut u8
    })
}

#[test]
fn both_pools_pop_grow_reload_handle_and_wrap_the_live_count() {
    let _f = fake(0x0090_CF9A, grow as *const () as usize);
    for (global, handle_global, function) in [
        (
            0x00B0_B2D4,
            0x00B0_B2D0,
            fc4p_00605040 as unsafe extern "C" fn(u32) -> u64,
        ),
        (0x00B0_B3AC, 0x00B0_B3A8, fc4p_0061d200),
    ] {
        let mut old = Object::new(16);
        let mut new = Object::new(16);
        let mut block = [0xDEAD_BEEFu32, 0xAABB_CCDD];
        let mut location = old.ptr();
        let mut handle = 71u32;
        let _g = fake_data(global, (&mut location as *mut *mut u8).cast());
        let _h = fake_data(handle_global, (&mut handle as *mut u32).cast());
        let op = unsafe { Pool::from_ptr(old.ptr()) };
        op.set_free_head(block.as_mut_ptr().cast());
        op.set_live_count(u32::MAX);
        GROW.with(|s| *s.borrow_mut() = (0, 0, 0, vec![]));
        assert_eq!(
            unsafe { function(0xFFFF) },
            registers(block.as_ptr() as u32, old.ptr() as u32)
        );
        assert_eq!(op.free_head() as u32, 0xDEAD_BEEF);
        assert_eq!(op.live_count(), 0);
        assert_eq!(block, [0xDEAD_BEEF, 0xAABB_CCDD]);
        assert!(GROW.with(|s| s.borrow().3.is_empty()));
        op.set_free_head(core::ptr::null_mut());
        let np = unsafe { Pool::from_ptr(new.ptr()) };
        np.set_live_count(9);
        GROW.with(|s| {
            *s.borrow_mut() = (
                &mut location as *mut *mut u8 as usize,
                new.ptr() as usize,
                block.as_mut_ptr() as usize,
                vec![],
            )
        });
        assert_eq!(
            unsafe { function(0) },
            registers(block.as_ptr() as u32, new.ptr() as u32)
        );
        assert_eq!(np.live_count(), 10);
        assert_eq!(np.free_head() as u32, 0xDEAD_BEEF);
        assert_eq!(op.live_count(), 0);
        assert_eq!(
            GROW.with(|s| s.borrow().3.clone()),
            [(&mut handle as *mut u32) as usize]
        );
    }
}
extern "thiscall" fn base(this: *mut u8, arg: u32) {
    assert_eq!(arg, 0);
    CONSTRUCT.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.0);
        if s.2 {
            assert_eq!(unsafe { read::<u32>(this, 0x3C) }, 0x00AA_7448);
            assert_eq!(unsafe { read::<u32>(this, 0x1EC) }, 0x00AA_1F38);
        }
        unsafe {
            write(this, 0x3C, s.1 as *mut u8);
        }
        s.3.push(0);
    });
}
extern "thiscall" fn member(this: *mut u8) {
    CONSTRUCT.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.0 + 0x1D0);
        unsafe {
            let p = s.0 as *mut u8;
            assert_eq!(read::<u32>(p, 0x1EC), 0x00AA_7190);
            assert_eq!(read::<u32>(p, 0x1E8), 0);
            write(p, 0x13C, 0xAB00_0001u32);
        }
        s.3.push(1);
    });
}
#[test]
fn occupant_constructor_virtual_base_member_and_post_callback_flags() {
    let _a = fake(0x0061_E1A0, base as *const () as usize);
    let _b = fake(0x005E_8670, member as *const () as usize);
    for initialize in [0, 1, 0x8000_0000] {
        let mut obj = [0x5Au8; 0x200];
        let mut table = [0u32, 0x1B0];
        let p = obj.as_mut_ptr();
        CONSTRUCT.with(|s| *s.borrow_mut() = (p as usize, table.as_mut_ptr() as usize, initialize != 0, vec![]));
        assert_eq!(unsafe { fc4p_00605160(p, initialize) }, p);
        for (offset, value) in [
            (0, 0xAA7300),
            (4, 0xAA7278),
            (8, 0xAA725C),
            (0xC, 0xAA71F8),
            (0x20, 0xAA71E0),
            (0x38, 0xAA71DC),
            (0x13C, 0xABC00001),
        ] {
            assert_eq!(unsafe { read::<u32>(p, offset) }, value);
        }
        assert_eq!(&obj[0x24..0x38], &[0x5A; 0x14]);
        assert_eq!(CONSTRUCT.with(|s| s.borrow().3.clone()), [0, 1]);
    }
}
#[test]
fn construction_parameter_stores_touch_only_five_words_and_return_both_registers() {
    let mut object = [0xA5u8; 0x1C0];
    let mut expected = object;
    let values = [u32::MAX, 0x8000_0000, 0, 0x1234_5678, 0x8765_4321];
    for (i, v) in values.into_iter().enumerate() {
        expected[0x19C + i * 4..0x1A0 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    assert_eq!(
        unsafe {
            fc4p_0061d340(
                object.as_mut_ptr(),
                values[0],
                values[1],
                values[2],
                values[3],
                values[4],
            )
        },
        registers(values[4], values[3])
    );
    assert_eq!(object, expected);
}
#[test]
fn neighbor_offsets_sign_extend_wrap_and_are_read_before_output_writes() {
    let mut x = [0i8; 256];
    let mut z = [0i8; 256];
    for i in 0..256 {
        x[i] = i as i8;
        z[i] = !(i as i8);
    }
    let _x = fake_data(0x00B0_B3D8, x.as_mut_ptr().cast());
    let _z = fake_data(0x00B0_B3F4, z.as_mut_ptr().cast());
    for i in 0..256u32 {
        let mut obj = [0u32, i];
        let mut out_z = 7;
        let result = unsafe {
            fc4p_00620da0(
                obj.as_mut_ptr().cast(),
                0x7FFF_FFFF,
                0,
                obj.as_mut_ptr().add(1),
                &mut out_z,
            )
        };
        assert_eq!(obj[1], 0x7FFF_FFFFu32.wrapping_add(i32::from(x[i as usize]) as u32));
        assert_eq!(out_z, i32::from(z[i as usize]) as u32);
        assert_eq!(result, out_z);
    }
}
#[test]
fn packed_direction_indices_wrap_and_upper_value_bits_are_ignored() {
    for direction in [0u32, 1, 2, 3, 4, 31, 32, u32::MAX] {
        for value in [0u32, 1, 31, 32, 255, u32::MAX] {
            let mut word = 0xA7B6_C5D4u32;
            let shift = (direction & 3) * 8;
            let expected = ((word >> shift) & 7) | ((value & 31) << 3);
            let result = unsafe { fc4p_00621070((&mut word as *mut u32).cast(), direction, value) };
            assert_eq!(result, expected << shift);
            assert_eq!(word, (0xA7B6_C5D4 & !(255 << shift)) | (expected << shift));
            assert_eq!(
                invoke(
                    fc4p_00620de0 as *const () as usize,
                    (&mut word as *mut u32).cast(),
                    &[direction],
                    0x1234_5678,
                    0xAABB_CCDD
                ),
                (word >> shift, 0xAABB_CCDD)
            );
        }
    }
}
#[test]
fn connection_lists_cover_every_low_three_bit_combination_and_retain_output_tails() {
    for bits in 0..4096u32 {
        let mut word = 0u32;
        let mut expected_two = vec![];
        let mut expected_one = vec![];
        for i in 0..4u32 {
            let v = (bits >> (i * 3)) & 7;
            word |= (v | 0xD8) << (i * 8);
            if v == 2 {
                expected_two.push(i);
            }
            if v == 1 || v == 3 {
                expected_one.push(i);
            }
        }
        let mut two = [0xDEAD_BEEFu32; 6];
        let mut one = two;
        let mut n_two = 88u32;
        let mut n_one = 99u32;
        let result = invoke(
            fc4p_00620df0 as *const () as usize,
            (&mut word as *mut u32).cast(),
            &[
                two[1..].as_mut_ptr() as u32,
                one[1..].as_mut_ptr() as u32,
                &mut n_two as *mut u32 as u32,
                &mut n_one as *mut u32 as u32,
            ],
            0xABCD_EF98,
            0xAABB_CCDD,
        );
        assert_eq!(n_two as usize, expected_two.len());
        assert_eq!(n_one as usize, expected_one.len());
        assert_eq!(&two[1..1 + n_two as usize], expected_two);
        assert_eq!(&one[1..1 + n_one as usize], expected_one);
        assert!(two[1 + n_two as usize..]
            .iter()
            .chain(one[1 + n_one as usize..].iter())
            .all(|&x| x == 0xDEAD_BEEF));
        assert_eq!((two[0], one[0]), (0xDEAD_BEEF, 0xDEAD_BEEF));
        let last = (word >> 24) & 7;
        assert_eq!(
            result.1,
            if last == 1 || last == 3 {
                n_one - 1
            } else {
                &mut n_two as *mut u32 as u32
            }
        );
    }
}
