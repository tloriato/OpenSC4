// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Object};
use crate::traffic::test_support::{bytes, invoke, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Write(usize, u32),
    Bool(u32),
    Release(usize),
    Rows(usize),
    Free(usize),
    Remove(usize, usize),
    Reorder(usize, usize),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static OBSERVED: RefCell<*mut u8>=const{RefCell::new(core::ptr::null_mut())};
    static MUTATION: RefCell<Option<(usize,*mut u8,u32)>>=const{RefCell::new(None)};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
#[repr(C)]
struct Stream {
    table: *const usize,
    count: usize,
}
fn put(this: *mut Stream, slot: usize, value: u32) -> u64 {
    log(Call::Write(slot, value));
    unsafe {
        (*this).count += 1;
    }
    MUTATION.with(|m| {
        if let Some((n, p, value)) = *m.borrow() {
            if unsafe { (*this).count } == n {
                unsafe {
                    p.cast::<u32>().write_unaligned(value);
                }
            }
        }
    });
    0xCAFE_BABE_7654_3210
}
extern "thiscall" fn put10(this: *mut Stream, value: u32) -> u64 {
    put(this, 0x10, value)
}
extern "thiscall" fn put1c(this: *mut Stream, value: u32) -> u64 {
    put(this, 0x1C, value)
}
extern "thiscall" fn put20(this: *mut Stream, value: u32) -> u64 {
    put(this, 0x20, value)
}
extern "thiscall" fn put24(this: *mut Stream, value: u32) -> u64 {
    put(this, 0x24, value)
}
extern "thiscall" fn put30(this: *mut Stream, value: u32) -> u64 {
    put(this, 0x30, value)
}
extern "C" fn boolean(_this: *mut u8, value: u32) -> u32 {
    log(Call::Bool(value));
    0
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    log(Call::Release(this as usize));
    0xCAFE_BABE_1234_5678
}
extern "thiscall" fn rows(this: *mut u8) -> u64 {
    log(Call::Rows(this as usize));
    OBSERVED.with(|p| unsafe {
        let p = *p.borrow();
        assert_eq!(word(p, 0), 0x00A8_7D40);
        assert_eq!(word(p, 4), 0x00A8_7D24);
    });
    0x1122_3344_5566_7788
}
extern "C" fn heap_free(this: *mut u8) -> u64 {
    log(Call::Free(this as usize));
    0xAABB_CCDD_9876_5432
}
extern "C" fn free(this: *mut u8) -> u32 {
    log(Call::Free(this as usize));
    MUTATION.with(|m| {
        if let Some((when, p, value)) = *m.borrow() {
            if when == this as usize {
                unsafe {
                    p.cast::<u32>().write_unaligned(value);
                }
            }
        }
    });
    this as u32 ^ 0x1234_5678
}
extern "thiscall" fn remove(this: *mut u8, block: *mut u8) {
    log(Call::Remove(this as usize, block as usize));
}
extern "thiscall" fn reorder(this: *mut u8, block: *mut u8) {
    log(Call::Reorder(this as usize, block as usize));
}
fn stream_table() -> Vec<usize> {
    table(&[
        (0x10, put10 as *const () as usize),
        (0x1C, put1c as *const () as usize),
        (0x20, put20 as *const () as usize),
        (0x24, put24 as *const () as usize),
        (0x30, put30 as *const () as usize),
    ])
}

#[test]
fn grid_getter_uses_twelve_byte_rows_and_returns_row_in_edx() {
    let mut o = Object::new(0x50);
    let grid = unsafe { Grid32::from_ptr(o.ptr()) };
    let mut a = [0xFFFF_FFFFu32, 0x8000_0000, 123];
    let mut b = [1u32, 2, 3];
    let mut rows = [
        [a.as_mut_ptr() as u32, 0xABCD, 0xEF01],
        [b.as_mut_ptr() as u32, 0xABCD, 0xEF01],
    ];
    grid.set_rows(rows.as_mut_ptr().cast());
    let before = unsafe { bytes(o.ptr(), 0x50) };
    assert_eq!(
        unsafe { fc4p_0045ff50(o.ptr(), 0, 1) },
        (u64::from(a.as_mut_ptr() as u32) << 32) | 0x8000_0000
    );
    assert_eq!(
        unsafe { fc4p_0045ff50(o.ptr(), 1, 2) },
        (u64::from(b.as_mut_ptr() as u32) << 32) | 3
    );
    assert_eq!(unsafe { bytes(o.ptr(), 0x50) }, before);
}

#[test]
fn grid_release_thunk_adjusts_this_and_keeps_both_result_words() {
    let _f = fake(0x0045_E120, release as *const () as usize);
    let mut o = Object::new(0x50);
    take();
    let r = unsafe { invoke(fc4p_0045ff40 as *const () as usize, o.ptr().add(4), &[], 0, 0, false) };
    assert_eq!((r.eax, r.edx), (0x1234_5678, 0xCAFE_BABE));
    assert_eq!(take(), [Call::Release(o.ptr() as usize)]);
}

#[test]
fn deleting_grid_restores_tables_destroys_rows_then_frees_only_for_low_flag_bit() {
    let _a = fake(0x0046_38B0, rows as *const () as usize);
    let _b = fake(0x005E_5620, heap_free as *const () as usize);
    let mut o = Object::new(0x50);
    let p = o.ptr();
    OBSERVED.with(|v| *v.borrow_mut() = p);
    for flags in [0, 1, 2, 0x101, 0x100] {
        unsafe {
            core::ptr::write_bytes(p, 0xA5, 0x50);
        }
        let mut want = unsafe { bytes(p, 0x50) };
        want[0..4].copy_from_slice(&0x00A8_7D40u32.to_le_bytes());
        want[4..8].copy_from_slice(&0x00A8_7D24u32.to_le_bytes());
        let r = unsafe { fc4p_0045ff90(p, 0, flags) };
        let mut calls = vec![Call::Rows(p.wrapping_add(0x38) as usize)];
        if flags & 1 != 0 {
            calls.push(Call::Free(p as usize));
        }
        assert_eq!(take(), calls);
        assert_eq!(r as u32, p as u32);
        assert_eq!((r >> 32) as u32, if flags & 1 != 0 { 0xAABB_CCDD } else { 0x1122_3344 });
        assert_eq!(unsafe { bytes(p, 0x50) }, want);
    }
    OBSERVED.with(|v| *v.borrow_mut() = core::ptr::null_mut());
}

#[unsafe(naked)]
unsafe extern "thiscall" fn unchanged_edx() {
    core::arch::naked_asm!("ret");
}

#[test]
fn empty_grid_destruction_retains_entry_edx_when_callees_leave_it_untouched() {
    let _rows = fake(0x0046_38B0, unchanged_edx as *const () as usize);
    let _free = fake(0x005E_5620, unchanged_edx as *const () as usize);
    let mut o = Object::new(0x50);
    for flag in [0, 1] {
        let r = unsafe {
            invoke(
                fc4p_0045ff90 as *const () as usize,
                o.ptr(),
                &[flag],
                0,
                0x1234_ABCD,
                false,
            )
        };
        assert_eq!((r.eax, r.edx), (o.ptr() as u32, 0x1234_ABCD));
    }
}

#[test]
fn record_writer_emits_two_unsigned_bytes_then_seven_words_with_live_reads() {
    let vt = stream_table();
    let mut stream = Stream {
        table: vt.as_ptr(),
        count: 0,
    };
    let sp = (&mut stream as *mut Stream).cast();
    let mut record = Object::new(0x20);
    unsafe {
        record.ptr().write(0xFF);
        record.ptr().add(1).write(0x80);
        for i in 1..8 {
            store(record.ptr(), i * 4, 0x8000_0000 + i as u32);
        }
    }
    MUTATION.with(|m| *m.borrow_mut() = Some((1, record.ptr().wrapping_add(4), 0x1122_3344)));
    take();
    assert_eq!(unsafe { fc4p_005da390(sp, record.ptr()) }, sp);
    MUTATION.with(|m| *m.borrow_mut() = None);
    let mut want = vec![
        Call::Write(0x10, 255),
        Call::Write(0x10, 128),
        Call::Write(0x20, 0x1122_3344),
    ];
    want.extend((2..8).map(|i| Call::Write(0x20, 0x8000_0000 + i)));
    assert_eq!(take(), want);
}

fn header_calls() -> Vec<Call> {
    vec![
        Call::Write(0x1C, 1),
        Call::Bool(0xA5),
        Call::Write(0x24, 12),
        Call::Write(0x24, 16),
        Call::Write(0x20, 20),
        Call::Write(0x20, 24),
        Call::Write(0x20, 2),
        Call::Write(0x20, 2),
        Call::Write(0x30, 36),
        Call::Write(0x30, 40),
        Call::Write(0x30, 44),
        Call::Write(0x30, 48),
    ]
}
#[test]
fn grid_writer_header_cells_signed_dimensions_stack_counter_and_registers() {
    let _f = fake(0x005D_9FD0, boolean as *const () as usize);
    let vt = stream_table();
    let mut stream = Stream {
        table: vt.as_ptr(),
        count: 0,
    };
    let sp = (&mut stream as *mut Stream).cast::<u8>();
    let mut o = Object::new(0x50);
    let p = o.ptr();
    let grid = unsafe { Grid32Stream::from_ptr(p) };
    unsafe {
        for offset in (0x0C..=0x30).step_by(4) {
            store(p, offset, offset as u32);
        }
        p.add(4).write(0xA5);
    }
    let mut cells = [[10u32, 11], [20, 21]];
    let mut rows = [
        [cells[0].as_mut_ptr() as u32, 0, 0],
        [cells[1].as_mut_ptr() as u32, 0, 0],
    ];
    grid.set_serialized_rows(rows.as_mut_ptr().cast());
    grid.set_width(2);
    grid.set_height(2);
    let before = unsafe { bytes(p, 0x50) };
    take();
    let mut stack = [0u32, sp as u32];
    let result = unsafe { write_grid(p, stack.as_mut_ptr()) };
    let mut want = header_calls();
    want.extend([10, 11, 20, 21].into_iter().map(|v| Call::Write(0x24, v)));
    assert_eq!(take(), want);
    assert_eq!(stack[1], 2);
    assert_eq!(result, 0x0000_0002_0000_0001);
    assert_eq!(unsafe { bytes(p, 0x50) }, before);
    for (width, height, eax, edx, counter) in [
        (0, 99, 1, 0xCAFE_BABE, 0),
        (-257, 99, 0xFFFF_FE01, 0xCAFE_BABE, 0),
        (2, -257, 0xFFFF_FE01, 2, 2),
        (2, 0, 1, 2, 2),
    ] {
        grid.set_width(width);
        grid.set_height(height);
        stack[1] = sp as u32;
        let r = unsafe { write_grid(p, stack.as_mut_ptr()) };
        take();
        assert_eq!((r as u32, (r >> 32) as u32, stack[1]), (eax, edx, counter));
    }
    grid.set_width(2);
    grid.set_height(2);
    // The exported adapter removes exactly one stack word and returns EDX:EAX.
    let r = unsafe { invoke(fc4p_0045fe60 as *const () as usize, p, &[sp as u32], 0, 0, false) };
    assert_eq!((r.eax, r.edx), (1, 2));
    take();
}

#[test]
fn grid_writer_reloads_height_after_each_cell() {
    let _f = fake(0x005D_9FD0, boolean as *const () as usize);
    let vt = stream_table();
    let mut stream = Stream {
        table: vt.as_ptr(),
        count: 0,
    };
    let mut o = Object::new(0x50);
    let grid = unsafe { Grid32Stream::from_ptr(o.ptr()) };
    let mut cells = [10u32, 11];
    let mut rows = [cells.as_mut_ptr() as u32, 0, 0];
    grid.set_width(1);
    grid.set_height(2);
    grid.set_serialized_rows(rows.as_mut_ptr().cast());
    // Eleven header virtual calls precede the first cell.
    MUTATION.with(|m| *m.borrow_mut() = Some((12, o.ptr().wrapping_add(0x20), 1)));
    let mut stack = [0, (&mut stream as *mut Stream) as u32];
    take();
    unsafe {
        write_grid(o.ptr(), stack.as_mut_ptr());
    }
    MUTATION.with(|m| *m.borrow_mut() = None);
    let calls = take();
    assert_eq!(&calls[12..], &[Call::Write(0x24, 10)]);
}

#[test]
fn tree_erase_right_node_left_and_late_left_link_read() {
    let _f = fake(0x0090_CF63, free as *const () as usize);
    let mut map = Object::new(8);
    let mut left = Object::new(16);
    let mut replacement = Object::new(16);
    let mut right = Object::new(16);
    let mut root = Object::new(16);
    unsafe {
        store(root.ptr(), 8, left.ptr() as u32);
        store(root.ptr(), 12, right.ptr() as u32);
    }
    MUTATION.with(|m| {
        *m.borrow_mut() = Some((
            right.ptr() as usize,
            root.ptr().wrapping_add(8),
            replacement.ptr() as u32,
        ))
    });
    take();
    unsafe {
        assert_eq!(
            call_thiscall_with_eax(fc4p_0070ef50 as *const () as usize, map.ptr(), &[0], 0x1234_ABCD).0,
            0x1234_ABCD
        );
        assert_eq!(
            call_thiscall_with_eax(fc4p_0070ef50 as *const () as usize, map.ptr(), &[root.ptr() as u32], 0).0,
            replacement.ptr() as u32 ^ 0x1234_5678
        );
    }
    MUTATION.with(|m| *m.borrow_mut() = None);
    assert_eq!(
        take(),
        [
            Call::Free(right.ptr() as usize),
            Call::Free(root.ptr() as usize),
            Call::Free(replacement.ptr() as usize)
        ]
    );
}

#[test]
fn pooled_trip_free_list_counts_flags_and_pool_callbacks() {
    let _a = fake(0x0090_D031, remove as *const () as usize);
    let _b = fake(0x0090_CFF9, reorder as *const () as usize);
    let mut manager = 0u32;
    let mp = (&mut manager as *mut u32).cast();
    let _c = fake_data(0x00B0_D484, mp);
    let mut allocation = vec![0xA5u8; 8192];
    let block = ((allocation.as_mut_ptr() as usize + 2047) & !2047) as *mut u8;
    let p = block.wrapping_add(0x100);
    for (count, flags, kind) in [
        (1u32, 1, 1),
        (2, 1, 0),
        (0x4000_0040, 1, 2),
        (0, 1, 0),
        (2, 0, 0),
        (2, 2, 0),
        (2, 0x100, 0),
    ] {
        unsafe {
            store(block, 8, 0x1234_5678);
            store(block, 12, count);
            store(p, 0, 0xA5A5_A5A5);
        }
        take();
        let before = unsafe { bytes(block, 2048) };
        assert_eq!(unsafe { fc4p_0070f040(p, flags) }, p);
        let mut want = before;
        if flags & 1 != 0 {
            want[8..12].copy_from_slice(&(p as u32).to_le_bytes());
            want[12..16].copy_from_slice(&count.wrapping_sub(1).to_le_bytes());
            want[0x100..0x104].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        }
        assert_eq!(unsafe { bytes(block, 2048) }, want);
        assert_eq!(
            take(),
            match kind {
                1 => vec![Call::Remove(mp as usize, block as usize)],
                2 => vec![Call::Reorder(mp as usize, block as usize)],
                _ => vec![],
            }
        );
    }
    assert!(unsafe { fc4p_0070f040(core::ptr::null_mut(), 1) }.is_null());
}
