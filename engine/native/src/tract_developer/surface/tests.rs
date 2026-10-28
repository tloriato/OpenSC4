// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Interface, style rotation and version-three save cases from the core tract-developer tests.

use super::*;
use crate::testing::{fake, Object};
use crate::tract_developer::testing::{run, snapshot, ECX, EDX};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    AddRef(u32, u32),
    Shutdown(u32),
    Delete(u32, u32),
    Version(u32),
    Signed(u32),
    Byte(u32),
    Unsigned(u32),
    Error,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

fn log(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    // SAFETY: the test supplies a primary interface with the output pointer stored at +4.
    unsafe {
        let output = word(this as u32, 4);
        log(Call::AddRef(this as u32, word(output, 0)));
    }
    0x7654_3210_AABB_CCDD
}

#[test]
fn identity_and_interfaces_keep_full_register_results() {
    assert_eq!(run(0x007045A0, Conv::Thiscall, ECX, &[]), (0x2990_C142, ECX, EDX));
    let vt = [0usize, add_ref as *const () as usize];
    let mut out = 0u32;
    let mut obj = [vt.as_ptr() as u32, &mut out as *mut u32 as u32, 0x9988_7766];
    let this = obj.as_mut_ptr() as u32;
    for (iid, delta) in [(1, 0), (0xA818_ED1B, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
        out = 0xDEAD_BEEF;
        let result = run(0x007045B0, Conv::Thiscall, this, &[iid, &mut out as *mut u32 as u32]);
        assert_eq!(out, this + delta);
        assert_eq!(result.0, 0xAABB_CC01);
        assert_eq!(result.2, 0x7654_3210);
        assert_eq!(take(), [Call::AddRef(this, this + delta)]);
    }
    for iid in [
        0,
        2,
        0x4522_94A9,
        0x4522_94AB,
        0xA818_ED1A,
        0xA818_ED1C,
        0x2990_C142,
        u32::MAX,
    ] {
        out = 0xDEAD_BEEF;
        let expected = if iid < 0xA818_ED1B {
            iid.wrapping_sub(0x4522_94AA)
        } else {
            iid
        } & !0xFF;
        assert_eq!(
            run(0x007045B0, Conv::Thiscall, this, &[iid, &mut out as *mut u32 as u32]),
            (expected, this, EDX)
        );
        assert_eq!(out, 0xDEAD_BEEF);
        assert!(take().is_empty());
    }
}

extern "thiscall" fn shutdown(this: *mut u8) -> u64 {
    log(Call::Shutdown(this as u32));
    // SAFETY: +4 contains the replacement table installed by this fake.
    unsafe {
        store(this as u32, 0, word(this as u32, 4));
    }
    0x1111_2222_3333_4444
}

extern "thiscall" fn delete(this: *mut u8, flags: u32) -> u64 {
    log(Call::Delete(this as u32, flags));
    0x5566_7788_AABB_CCDD
}

#[test]
fn release_uses_signed_count_and_reloads_table_after_shutdown() {
    let mut vt = [0usize; 16];
    vt[4] = shutdown as *const () as usize;
    let mut next_vt = [0usize; 16];
    next_vt[15] = delete as *const () as usize;
    let mut object = Object::new(0x180);
    let this = object.ptr() as u32;
    // SAFETY: the object covers the declared fields and the fake's extra table pointer.
    let td = unsafe { TractDeveloper::from_ptr(this as *mut u8) };
    unsafe {
        store(this, 4, next_vt.as_ptr() as u32);
    }
    for count in [2, 100, i32::MAX, 1, 0, -1, i32::MIN] {
        td.set_vtable(vt.as_ptr() as u32);
        td.set_references(count);
        let before = snapshot(this as *mut u8, 0x180);
        let result = run(0x007048B0, Conv::Thiscall, this, &[]);
        if count > 1 {
            assert_eq!(result, ((count - 1) as u32, this, EDX));
            let mut want = before;
            want[0x10..0x14].copy_from_slice(&(count - 1).to_le_bytes());
            assert_eq!(snapshot(this as *mut u8, 0x180), want);
            assert!(take().is_empty());
        } else {
            assert_eq!(result.0, 0);
            assert_eq!(result.2, 0x5566_7788);
            assert_eq!(td.references(), count);
            assert_eq!(take(), [Call::Shutdown(this), Call::Delete(this, 1)]);
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn message_type() {
    core::arch::naked_asm!(
        "mov eax, [ecx + 4]",
        "mov ecx, 0x11223344",
        "mov edx, 0x55667788",
        "ret"
    );
}

#[test]
fn styles_rotate_for_year_messages_including_empty_lists_and_overflow() {
    let mut vt = [0usize; 5];
    vt[4] = message_type as *const () as usize;
    let mut msg = [vt.as_ptr() as u32, 0];
    let mut obj = Object::new(0x180);
    let this = obj.ptr() as u32;
    // SAFETY: the object covers the declared fields.
    let td = unsafe { TractDeveloper::from_ptr(this as *mut u8) };
    for id in [0, 0x6695_6816, 0x6695_6817, 0x6695_6818, u32::MAX] {
        for n in [0u32, 1, 3] {
            for (index, years, threshold) in [
                (0i32, 0i32, 5i32),
                (2, 4, 5),
                (-2, 0, 0),
                (i32::MAX, 4, 5),
                (0, i32::MAX, i32::MIN),
                (0, 255, 1000),
            ] {
                td.set_styles_begin(0x1000);
                td.set_styles_end(0x1000 + 4 * n);
                td.set_style_index(index);
                td.set_years_since_change(years);
                td.set_years_between_changes(threshold);
                msg[1] = id;
                let before = snapshot(this as *mut u8, 0x180);
                let (mut i, mut y) = (index, years);
                let (eax, ecx, edx) = if id != 0x6695_6817 {
                    let value = id.wrapping_sub(0x6695_6816);
                    (
                        (if value == 0 { 0 } else { value.wrapping_sub(1) } & !0xFF) | 1,
                        0x1122_3344,
                        0x5566_7788,
                    )
                } else {
                    y = y.wrapping_add(1);
                    if y >= threshold {
                        i = i.wrapping_add(1);
                        let eax = (i as u32 & !0xFF) | 1;
                        y = 0;
                        if i as u32 >= n {
                            i = 0;
                        }
                        (eax, 0, n)
                    } else {
                        ((y as u32 & !0xFF) | 1, threshold as u32, y as u32)
                    }
                };
                assert_eq!(
                    run(0x007048E0, Conv::Thiscall, this + 4, &[msg.as_mut_ptr() as u32]),
                    (eax, ecx, edx)
                );
                let mut want = before;
                want[0x124..0x128].copy_from_slice(&i.to_le_bytes());
                want[0x128..0x12C].copy_from_slice(&y.to_le_bytes());
                assert_eq!(snapshot(this as *mut u8, 0x180), want);
            }
        }
    }
}

#[repr(C)]
struct Stream {
    table: *const usize,
    bytes: Vec<u8>,
    error: u32,
    developer: *mut u8,
    replacement_styles: *const u32,
    word_calls: u32,
}

extern "thiscall" fn version(p: *mut Stream, value: u32) -> u64 {
    log(Call::Version(value));
    unsafe {
        (*p).bytes.extend_from_slice(&(value as u16).to_le_bytes());
    }
    0x1111_2222_3333_4444
}
extern "thiscall" fn signed(p: *mut Stream, value: u32) -> u64 {
    log(Call::Signed(value));
    unsafe {
        (*p).bytes.extend_from_slice(&value.to_le_bytes());
    }
    0x1111_2222_3333_4444
}
extern "thiscall" fn unsigned(p: *mut Stream, value: u32) -> u64 {
    log(Call::Unsigned(value));
    unsafe {
        let s = &mut *p;
        s.bytes.extend_from_slice(&value.to_le_bytes());
        s.word_calls += 1;
        if s.word_calls == 1 && !s.replacement_styles.is_null() {
            let td = TractDeveloper::from_ptr(s.developer);
            td.set_styles_begin(s.replacement_styles as u32);
            td.set_styles_end(s.replacement_styles as u32);
            td.set_years_between_changes(-17);
        }
    }
    0x1111_2222_3333_4444
}
extern "C" fn byte(p: *mut Stream, value: u32) -> u64 {
    log(Call::Byte(value));
    unsafe {
        (*p).bytes.push(value as u8);
    }
    0x1111_2222_3333_4444
}
extern "thiscall" fn error(p: *mut Stream) -> u64 {
    log(Call::Error);
    unsafe { (0xFEED_FACEu64 << 32) | u64::from((*p).error) }
}

#[test]
fn save_record_writes_all_fields_and_uses_only_low_error_byte() {
    let _byte = fake(0x005D9FD0, byte as *const () as usize);
    let mut vt = [0usize; 19];
    vt[7] = version as *const () as usize;
    vt[8] = signed as *const () as usize;
    vt[9] = unsigned as *const () as usize;
    vt[18] = error as *const () as usize;
    let mut object = Object::new(0x180);
    let this = object.ptr();
    let td = unsafe { TractDeveloper::from_ptr(this) };
    let styles = [0x2000u32, 0, u32::MAX];
    let alternate = [7u32, 8, 9];
    for n in [0, 1, 3] {
        for err in [0u32, 1, 0xAABB_CC00, 0xAABB_CC80] {
            for replace in [false, true] {
                td.set_styles_begin(styles.as_ptr() as u32);
                td.set_styles_end((styles.as_ptr() as u32) + n * 4);
                td.set_style_index(-2);
                td.set_years_since_change(i32::MIN);
                td.set_single_style(0x80);
                td.set_years_between_changes(40);
                let mut stream = Stream {
                    table: vt.as_ptr(),
                    bytes: Vec::new(),
                    error: err,
                    developer: this,
                    replacement_styles: if replace { alternate.as_ptr() } else { core::ptr::null() },
                    word_calls: 0,
                };
                let result = run(
                    0x00704B60,
                    Conv::Thiscall,
                    this as u32 + 8,
                    &[&mut stream as *mut Stream as u32],
                );
                assert_eq!(result.0, u32::from(err as u8 == 0));
                assert_eq!(result.2, 0xFEED_FACE);
                let selected = if replace { &alternate } else { &styles };
                let years: i32 = if replace { -17 } else { 40 };
                let mut want = vec![
                    Call::Version(3),
                    Call::Signed((-2i32) as u32),
                    Call::Signed(i32::MIN as u32),
                    Call::Byte(0x80),
                    Call::Unsigned(n),
                ];
                want.extend(selected[..n as usize].iter().copied().map(Call::Unsigned));
                want.extend([Call::Signed(years as u32), Call::Error]);
                assert_eq!(take(), want);
                let mut bytes = 3u16.to_le_bytes().to_vec();
                bytes.extend_from_slice(&(-2i32).to_le_bytes());
                bytes.extend_from_slice(&i32::MIN.to_le_bytes());
                bytes.push(0x80);
                bytes.extend_from_slice(&n.to_le_bytes());
                for style in &selected[..n as usize] {
                    bytes.extend_from_slice(&style.to_le_bytes());
                }
                bytes.extend_from_slice(&years.to_le_bytes());
                assert_eq!(stream.bytes, bytes);
            }
        }
    }
}

#[test]
fn layout_matches_interface_adjustments() {
    assert!(TractDeveloper::FIELDS.contains(&("style_index", 0x124, 4)));
    assert!(TractDeveloper::FIELDS.contains(&("years_since_change", 0x128, 4)));
    assert!(TractDeveloper::FIELDS.contains(&("single_style", 0x12C, 1)));
}
