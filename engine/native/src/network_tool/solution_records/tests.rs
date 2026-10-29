// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Copy(u32, usize, usize),
    String(usize, usize, u32, u32, usize),
    Initialize(usize, u32),
    Allocate(u32),
    Destroy(usize),
    Free(usize),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocation: usize,
    swap: Option<(usize, usize, usize)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

fn copied(address: u32, this: *mut u8, source: *mut u8) {
    record(Call::Copy(address, this as usize, source as usize));
    unsafe {
        if matches!(address, 0x0062_6430 | 0x0062_65B0) {
            for offset in [4, 8, 12, 16] {
                assert_eq!(read::<u32>(this, offset), 0);
            }
        }
        write(this, 0, address);
    }
}
macro_rules! copy_fake {
    ($name:ident, $address:literal) => {
        extern "thiscall" fn $name(this: *mut u8, source: *mut u8) {
            copied($address, this, source);
        }
    };
}
copy_fake!(small, 0x0062_5190);
copy_fake!(words, 0x007C_ED60);
copy_fake!(pairs, 0x0062_4D60);
copy_fake!(first_table, 0x0062_6430);
copy_fake!(second_table, 0x0062_65B0);
copy_fake!(vector, 0x0045_9E80);
extern "thiscall" fn string(this: *mut u8, source: *mut u8, start: u32, length: u32, scratch: *mut *mut u8) {
    unsafe {
        record(Call::String(
            this as usize,
            source as usize,
            start,
            length,
            *scratch as usize,
        ));
        *scratch = core::ptr::null_mut();
        write(this, 0, 0x0040_89B0u32);
    }
}
extern "thiscall" fn initialize(this: *mut u8, buckets: u32) -> u64 {
    record(Call::Initialize(this as usize, buckets));
    unsafe {
        for offset in [4, 8, 12, 16] {
            assert_eq!(read::<u32>(this, offset), 0);
        }
        write(this, 4, 0x1234_5678u32);
    }
    registers(0xAAAA_AAAA, 0xDEAD_BEEF)
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    record(Call::Allocate(size));
    STATE.with(|s| s.borrow().allocation as *mut u8)
}
extern "thiscall" fn destroy(this: *mut u8) {
    record(Call::Destroy(this as usize));
    // Changing the links and table storage here exercises the saved-next and reloaded-vector paths.
    unsafe {
        write(this.sub(8), 0, 0u32);
    }
    STATE.with(|s| {
        if let Some((table, begin, end)) = s.borrow_mut().swap.take() {
            unsafe {
                write(table as *mut u8, 4, begin as u32);
                write(table as *mut u8, 8, end as u32);
            }
        }
    });
}
extern "C" fn delete(pointer: *mut u8) {
    record(Call::Free(pointer as usize));
}

fn fakes() -> Vec<Fake> {
    STATE.with(|s| *s.borrow_mut() = State::default());
    [
        (0x0062_5190, small as *const ()),
        (0x007C_ED60, words as *const ()),
        (0x0062_4D60, pairs as *const ()),
        (0x0062_6430, first_table as *const ()),
        (0x0062_65B0, second_table as *const ()),
        (0x0045_9E80, vector as *const ()),
        (0x0040_89B0, string as *const ()),
        (0x0082_9BC0, initialize as *const ()),
        (0x0090_CF54, alloc as *const ()),
        (0x0062_6160, destroy as *const ()),
        (0x0090_CF63, delete as *const ()),
    ]
    .into_iter()
    .map(|(address, function)| fake(address, function as usize))
    .collect()
}

#[test]
fn copy_members_in_order_and_preserve_flag_padding_and_terminal_registers() {
    let _fakes = fakes();
    let mut source: [u8; 0x100] = core::array::from_fn(|i| i as u8);
    let mut output = [0xCDu8; 0x100];
    let (s, d) = (source.as_mut_ptr(), output.as_mut_ptr());
    let result = unsafe { fc4p_0062a1d0(d, s) };
    assert_eq!(result, registers(d as u32, 0xF7F6_F5F4));
    assert_eq!(
        calls(),
        [
            Call::Copy(0x0062_5190, d as usize + 4, s as usize + 4),
            Call::Copy(0x007C_ED60, d as usize + 0x28, s as usize + 0x28),
            Call::Copy(0x007C_ED60, d as usize + 0x34, s as usize + 0x34),
            Call::Copy(0x007C_ED60, d as usize + 0x4C, s as usize + 0x4C),
            Call::Copy(0x0062_4D60, d as usize + 0x58, s as usize + 0x58),
            Call::Copy(0x0062_6430, d as usize + 0x64, s as usize + 0x64),
            Call::String(d as usize + 0x84, s as usize + 0x84, 0, u32::MAX, s as usize),
            Call::Copy(0x0045_9E80, d as usize + 0x9C, s as usize + 0x9C),
            Call::Copy(0x0045_9E80, d as usize + 0xB8, s as usize + 0xB8),
            Call::Copy(0x0062_65B0, d as usize + 0xD0, s as usize + 0xD0),
        ]
    );
    let mut expected = [0xCDu8; 0x100];
    for offset in [
        0, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x40, 0x44, 0x78, 0x7C, 0x94, 0x98, 0xA8, 0xAC, 0xB0, 0xB4, 0xC4, 0xC8,
        0xCC, 0xE4, 0xEC, 0xF0, 0xF4, 0xF8,
    ] {
        expected[offset..offset + 4].copy_from_slice(&source[offset..offset + 4]);
    }
    for offset in [0x48, 0xE8, 0xE9] {
        expected[offset] = source[offset];
    }
    for offset in [0x68, 0x6C, 0x70, 0x74, 0x90, 0xD4, 0xD8, 0xDC, 0xE0] {
        expected[offset..offset + 4].fill(0);
    }
    for (offset, value) in [
        (4, 0x0062_5190u32),
        (0x28, 0x007C_ED60),
        (0x34, 0x007C_ED60),
        (0x4C, 0x007C_ED60),
        (0x58, 0x0062_4D60),
        (0x64, 0x0062_6430),
        (0x80, 0x00A8_0810),
        (0x84, 0x0040_89B0),
        (0x9C, 0x0045_9E80),
        (0xB8, 0x0045_9E80),
        (0xD0, 0x0062_65B0),
    ] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(output, expected);
}

#[test]
fn construction_initializes_only_owned_fields_and_uses_game_allocation() {
    let _fakes = fakes();
    let mut object = [0xCDu8; 0x100];
    let mut string = [0xABu8; 8];
    STATE.with(|s| s.borrow_mut().allocation = string.as_mut_ptr() as usize);
    let p = object.as_mut_ptr();
    assert_eq!(unsafe { fc4p_0062a480(p) }, registers(p as u32, 0xDEAD_BEEF));
    assert_eq!(
        calls(),
        [
            Call::Initialize(p as usize + 0x64, 100),
            Call::Allocate(8),
            Call::Initialize(p as usize + 0xD0, 100)
        ]
    );
    let mut expected = [0xCDu8; 0x100];
    for offset in [
        4, 8, 12, 16, 20, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x4C, 0x50, 0x54, 0x58, 0x5C, 0x60, 0x6C, 0x70, 0x74,
        0x78, 0x90, 0x94, 0x98, 0x9C, 0xA0, 0xA4, 0xA8, 0xAC, 0xB0, 0xB4, 0xB8, 0xBC, 0xC0, 0xC8, 0xCC, 0xD8, 0xDC,
        0xE0, 0xE4,
    ] {
        expected[offset..offset + 4].fill(0);
    }
    for offset in [0x48, 0xE8, 0xE9] {
        expected[offset] = 0;
    }
    let a = string.as_mut_ptr() as u32;
    for (offset, value) in [
        (0x68, 0x1234_5678),
        (0xD4, 0x1234_5678),
        (0x80, 0x00A8_0810),
        (0x84, a),
        (0x88, a),
        (0x8C, a + 8),
        (0xEC, u32::MAX),
        (0xF0, u32::MAX),
        (0xF4, u32::MAX),
        (0xF8, u32::MAX),
    ] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(object, expected);
    assert_eq!(string, [0, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB, 0xAB]);
}

#[test]
fn bucket_clear_walks_saved_links_and_reloads_vector_after_destruction() {
    let _fakes = fakes();
    let mut table = Object::new(0x20);
    let mut a = [0u32; 4];
    let mut b = [0u32; 4];
    let mut c = [0u32; 4];
    let (a, b, c) = (a.as_mut_ptr(), b.as_mut_ptr(), c.as_mut_ptr());
    unsafe {
        *a = b as u32;
    }
    let mut old = [a as u32, 0];
    let mut new = [a as u32, 0, c as u32, 0];
    let t = unsafe { BucketTable::from_ptr(table.ptr()) };
    t.set_begin(old.as_mut_ptr().cast());
    t.set_end(unsafe { old.as_mut_ptr().add(2).cast() });
    t.set_count(3);
    STATE.with(|s| {
        s.borrow_mut().swap = Some((
            t.as_ptr() as usize,
            new.as_mut_ptr() as usize,
            unsafe { new.as_mut_ptr().add(4) } as usize,
        ))
    });
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_0062a3c0 as *const () as usize,
        t.as_ptr() as u32,
        &[],
    );
    call.edx = 0xDEAD_BEEF;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (4, new.as_mut_ptr() as u32));
    assert_eq!(
        calls(),
        [
            Call::Destroy(a as usize + 8),
            Call::Free(a as usize),
            Call::Destroy(b as usize + 8),
            Call::Free(b as usize),
            Call::Destroy(c as usize + 8),
            Call::Free(c as usize)
        ]
    );
    assert_eq!(old, [a as u32, 0]);
    assert_eq!(new, [0; 4]);
    assert_eq!(t.count(), 0);
}

#[test]
fn empty_bucket_table_clears_count_and_keeps_entry_edx() {
    let _fakes = fakes();
    let mut table = [0xABCDu32, 0, 0, 0xBEEF, 42];
    let mut c = RawCall::new(
        Conv::Thiscall,
        fc4p_0062a3c0 as *const () as usize,
        table.as_mut_ptr() as u32,
        &[],
    );
    c.edx = 0x1234_5678;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!((c.eax, c.edx), (0, 0x1234_5678));
    assert_eq!(table, [0xABCD, 0, 0, 0xBEEF, 0]);
    assert!(calls().is_empty());
}
