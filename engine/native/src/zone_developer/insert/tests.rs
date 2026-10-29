// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Value(u32),
    Copy(u32, u32, u32, u32),
    Backward(u32, u32, u32, u32, u32),
    Construct(u32, u32, u32, u32),
    Fill(u32, u32, u32),
    Free(u32),
    Grow(u32, u32, u32, u32, u32, u32),
}

#[derive(Default)]
struct State {
    events: Vec<Event>,
    temporary: u32,
    count_pointer: u32,
    construct_count: Option<u32>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn value_fake(this: *mut u8, value: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.temporary = this as u32;
        s.events.push(Event::Value(value));
    });
    unsafe { core::ptr::copy_nonoverlapping(value as *const u32, this.cast::<u32>(), 3) };
    0xCCCC_DDDD_AAAA_BBBB
}

fn context(pointer: *mut u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.count_pointer == 0 {
            s.count_pointer = pointer as u32;
        } else {
            assert_eq!(pointer as u32, s.count_pointer);
        }
    });
    unsafe { pointer.read() }
}

extern "C" fn copy_fake(begin: u32, end: u32, out: u32, count: *mut u32) -> u64 {
    let scratch = context(count);
    STATE.with(|s| s.borrow_mut().events.push(Event::Copy(begin, end, out, scratch)));
    let words = (end - begin) as usize / 4;
    unsafe { core::ptr::copy(begin as *const u32, out as *mut u32, words) };
    0x1234_5678_9ABC_DEF0
}

extern "C" fn backward_fake(begin: u32, end: u32, out: u32, count: *mut u32, tag: u32) -> u64 {
    let scratch = context(count);
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::Backward(begin, end, out, scratch, tag))
    });
    let bytes = end - begin;
    unsafe { core::ptr::copy(begin as *const u32, (out - bytes) as *mut u32, bytes as usize / 4) };
    0x1357_2468_ABCD_1234
}

extern "C" fn construct_fake(out: u32, count: u32, value: u32, scratch: *mut u32) -> u64 {
    let before = context(scratch);
    assert_eq!(count, before);
    assert_eq!(value, STATE.with(|s| s.borrow().temporary));
    STATE.with(|s| s.borrow_mut().events.push(Event::Construct(out, count, value, before)));
    for i in 0..count {
        unsafe { core::ptr::copy_nonoverlapping(value as *const u32, (out + i * 12) as *mut u32, 3) };
    }
    if let Some(count) = STATE.with(|s| s.borrow().construct_count) {
        unsafe { scratch.write(count) };
    }
    0xDEAD_BEEF_1122_3344
}

extern "C" fn fill_record(begin: u32, end: u32, value: u32) -> u64 {
    assert_eq!(value, STATE.with(|s| s.borrow().temporary));
    STATE.with(|s| s.borrow_mut().events.push(Event::Fill(begin, end, value)));
    for offset in (0..end - begin).step_by(12) {
        unsafe { core::ptr::copy_nonoverlapping(value as *const u32, (begin + offset) as *mut u32, 3) };
    }
    0xEDDC_BBAA_1111_2222
}

#[unsafe(naked)]
unsafe extern "C" fn fill_fake(_begin: u32, _end: u32, _value: u32) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 12]", "push dword ptr [esp + 12]", "push dword ptr [esp + 12]",
        "call {record}", "add esp, 12", "mov ecx, 0x55556666", "ret", record = sym fill_record);
}

extern "C" fn free_record(value: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().events.push(Event::Free(value)));
    0xCCCC_DDDD_AAAA_BBBB
}

#[unsafe(naked)]
unsafe extern "C" fn free_fake(_value: u32) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {record}", "add esp, 4",
        "mov ecx, 0x10203040", "ret", record = sym free_record);
}

extern "thiscall" fn grow_record(
    this: *mut u8,
    position: u32,
    value: u32,
    scratch: *mut u32,
    count: u32,
    tag: u32,
) -> u64 {
    assert_eq!(context(scratch), count);
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::Grow(this as u32, position, value, scratch as u32, count, tag))
    });
    unsafe { scratch.write(0xFACE_FEED) };
    0x3030_4040_1010_2020
}

#[unsafe(naked)]
unsafe extern "thiscall" fn grow_fake(
    _this: *mut u8,
    _position: u32,
    _value: u32,
    _scratch: *mut u32,
    _count: u32,
    _tag: u32,
) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 20]", "push dword ptr [esp + 20]", "push dword ptr [esp + 20]",
        "push dword ptr [esp + 20]", "push dword ptr [esp + 20]", "call {record}",
        "mov ecx, 0x778899aa", "ret 20", record = sym grow_record);
}

fn fakes() -> Vec<Fake> {
    STATE.with(|s| *s.borrow_mut() = State::default());
    [
        (0x004B_03E0, value_fake as *const () as usize),
        (0x0071_2330, copy_fake as *const () as usize),
        (0x0072_C890, backward_fake as *const () as usize),
        (0x005C_E080, construct_fake as *const () as usize),
        (0x0072_C860, fill_fake as *const () as usize),
        (0x0090_CF63, free_fake as *const () as usize),
        (0x005D_0010, grow_fake as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect()
}

fn call(vector: &mut [u32; 4], position: u32, count: u32, value: u32) -> [u32; 10] {
    let mut c = [
        vector.as_mut_ptr() as u32,
        position,
        count,
        value,
        0x1122_3344,
        0x5566_7788,
        0,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!("push dword ptr [edi + 12]", "push dword ptr [edi + 8]", "push dword ptr [edi + 4]",
            "mov ecx, [edi]", "mov eax, [edi + 16]", "mov edx, [edi + 20]", "call {function}",
            "mov [edi + 24], eax", "mov [edi + 28], edx", "mov [edi + 32], ecx",
            "mov eax, [esp - 8]", "mov [edi + 36], eax", function = sym fc4p_00733450,
            in("edi") c.as_mut_ptr(), clobber_abi("C"));
    }
    c
}

#[test]
fn zero_count_retains_entry_registers_without_reading_vector_fields() {
    let _fakes = fakes();
    let mut vector = [0xA5A5_A5A5; 4];
    let before = vector;
    let result = call(&mut vector, 0xFFFF_FFFF, 0, 0);
    assert_eq!(&result[6..], &[0x1122_3344, 0x5566_7788, vector.as_mut_ptr() as u32, 0]);
    assert_eq!(vector, before);
    assert!(STATE.with(|s| s.borrow().events.is_empty()));
}

#[test]
fn insufficient_capacity_delegates_growth_and_keeps_its_registers_and_count_write() {
    let _fakes = fakes();
    for (end, limit, count) in [(100u32, 124u32, 3u32), (100, 100, 1)] {
        let mut vector = [76, end, limit, 0xA5A5_A5A5];
        let before = vector;
        let result = call(&mut vector, 88, count, 0x1234_5678);
        assert_eq!(&result[6..], &[0x1010_2020, 0x3030_4040, 0x7788_99AA, 0xFACE_FEED]);
        assert_eq!(vector, before);
        assert_eq!(
            STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)),
            [Event::Grow(
                vector.as_mut_ptr() as u32,
                88,
                0x1234_5678,
                STATE.with(|s| s.borrow().count_pointer),
                count,
                0
            ),]
        );
        STATE.with(|s| s.borrow_mut().count_pointer = 0);
    }
}

#[test]
fn insertion_before_long_equal_short_and_empty_tails_keeps_calls_and_storage() {
    let _fakes = fakes();
    for (position, count, empty_value) in [
        (1u32, 1u32, false),
        (2, 2, false),
        (3, 2, false),
        (4, 2, false),
        (1, 1, true),
    ] {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut records = [[0xA5A5_A5A5u32; 3]; 8];
        for (i, record) in records[..4].iter_mut().enumerate() {
            *record = [i as u32 + 1; 3];
        }
        let value = if empty_value { [0; 3] } else { [0x1111, 0x2222, 0x3333] };
        let base = records.as_mut_ptr() as u32;
        let old_end = base + 48;
        let position_ptr = base + position * 12;
        let value_ptr = value.as_ptr() as u32;
        let mut vector = [base, old_end, base + 96, 0xA5A5_A5A5];
        let result = call(&mut vector, position_ptr, count, value_ptr);
        let temporary = STATE.with(|s| s.borrow().temporary);
        assert_ne!(temporary, value_ptr);
        let mut expected = vec![Event::Value(value_ptr)];
        let tail = 4 - position;
        if tail > count {
            let middle = old_end - count * 12;
            assert_eq!(result[9], middle);
            expected.extend([
                Event::Copy(middle, old_end, old_end, middle),
                Event::Backward(position_ptr, middle, old_end, middle, 0),
                Event::Fill(position_ptr, position_ptr + count * 12, temporary),
            ]);
        } else {
            let remaining = count - tail;
            assert_eq!(result[9], remaining);
            expected.extend([
                Event::Construct(old_end, remaining, temporary, remaining),
                Event::Copy(position_ptr, old_end, old_end + remaining * 12, remaining),
                Event::Fill(position_ptr, old_end, temporary),
            ]);
        }
        if empty_value {
            assert_eq!(&result[6..9], &[0, 0xEDDC_BBAA, 0x5555_6666]);
        } else {
            expected.push(Event::Free(value[0]));
            assert_eq!(&result[6..9], &[0xAAAA_BBBB, 0xCCCC_DDDD, 0x1020_3040]);
        }
        assert_eq!(STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)), expected);
        assert_eq!(vector, [base, old_end + count * 12, base + 96, 0xA5A5_A5A5]);
        let mut expected = [[0xA5A5_A5A5u32; 3]; 8];
        for i in 0..4 + count {
            expected[i as usize] = if i < position {
                [i + 1; 3]
            } else if i < position + count {
                value
            } else {
                [i - count + 1; 3]
            };
        }
        assert_eq!(records, expected);
    }
}

#[test]
fn construction_helpers_can_update_the_original_count_word_before_end_is_advanced() {
    let _fakes = fakes();
    STATE.with(|s| s.borrow_mut().construct_count = Some(1));
    let mut records = [[0u32; 3]; 8];
    let base = records.as_mut_ptr() as u32;
    let mut vector = [base, base + 24, base + 96, 0xA5A5_A5A5];
    let value = [0u32; 3];
    let result = call(&mut vector, base + 12, 3, value.as_ptr() as u32);
    assert_eq!(vector[1], base + 48);
    assert_eq!(result[9], 1);
    let events = STATE.with(|s| std::mem::take(&mut s.borrow_mut().events));
    assert!(matches!(events[1], Event::Construct(_, 2, _, 2)));
    assert!(matches!(events[2], Event::Copy(_, _, out, 1) if out == base + 36));
}

#[test]
fn signed_record_division_truncates_toward_zero_before_unsigned_comparison() {
    for bytes in [
        i32::MIN,
        -37,
        -24,
        -23,
        -12,
        -11,
        -1,
        0,
        1,
        11,
        12,
        23,
        24,
        37,
        i32::MAX,
    ] {
        let (quotient, shifted) = divide_records(bytes as u32);
        assert_eq!(quotient as i32, bytes / 12);
        assert_eq!(shifted as i32, ((i64::from(bytes) * 0x2AAA_AAAB) >> 33) as i32);
    }
}
