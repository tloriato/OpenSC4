// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::traffic::test_support::{invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Lookup(usize, u32),
    Allocate(u32),
    Construct(usize, u32),
    Retain,
    Variant,
    Type,
    Words,
    Dwords,
    Init(Vec<u16>),
    Promote(Vec<u32>),
    Id(u32),
    Attach(usize, usize, u32),
    Release,
}
thread_local! {
    static CALLS: RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static ALLOCATION: RefCell<*mut u8>=const{RefCell::new(core::ptr::null_mut())};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn lookup(this: *mut u8, id: u32) -> u64 {
    log(Call::Lookup(this as usize, id));
    (0x1122_3344u64 << 32) | u64::from(unsafe { word(this, 4) })
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    ALLOCATION.with(|v| *v.borrow())
}
extern "thiscall" fn construct(this: *mut u8, zero: u32) -> *mut u8 {
    log(Call::Construct(this as usize, zero));
    this
}
extern "thiscall" fn retain(_this: *mut u8) -> u32 {
    log(Call::Retain);
    1
}
extern "thiscall" fn variant(this: *mut u8) -> *mut u8 {
    log(Call::Variant);
    unsafe { word(this, 4) as *mut u8 }
}
extern "thiscall" fn kind(this: *mut u8) -> u64 {
    log(Call::Type);
    (0x4455_6677u64 << 32) | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn words(this: *mut u8) -> u64 {
    log(Call::Words);
    (0x1234_5678u64 << 32) | u64::from(unsafe { word(this, 8) })
}
extern "thiscall" fn dwords(this: *mut u8) -> u64 {
    log(Call::Dwords);
    (0x8765_4321u64 << 32) | u64::from(unsafe { word(this, 12) })
}
#[unsafe(naked)]
unsafe extern "thiscall" fn words_with_ecx() {
    core::arch::naked_asm!("call {}","mov ecx, 0xcafebabe","ret",sym words);
}
#[unsafe(naked)]
unsafe extern "thiscall" fn dwords_with_ecx() {
    core::arch::naked_asm!("call {}","mov ecx, 0xcafebabe","ret",sym dwords);
}
extern "thiscall" fn init(this: *mut u8, source: *const u16, count: u32) -> u32 {
    let values = unsafe { std::slice::from_raw_parts(source, count as usize) };
    log(Call::Init(values.to_vec()));
    unsafe {
        let dest = word(this, 8) as *mut u16;
        core::ptr::copy_nonoverlapping(source, dest, count as usize);
        store(this, 4, 0x8004);
    }
    0
}
extern "thiscall" fn promote(this: *mut u8, source: *const u32, count: u32) -> u32 {
    let values = unsafe { std::slice::from_raw_parts(source, count as usize) };
    log(Call::Promote(values.to_vec()));
    unsafe {
        let dest = word(this, 12) as *mut u32;
        core::ptr::copy_nonoverlapping(source, dest, count as usize);
        store(this, 4, 0x8006);
    }
    0xABCDEF
}
extern "thiscall" fn set_id(_this: *mut u8, id: u32) -> u32 {
    log(Call::Id(id));
    0
}
extern "thiscall" fn attach(this: *mut u8, property: *mut u8, flags: u32) -> u32 {
    log(Call::Attach(this as usize, property as usize, flags));
    unsafe {
        store(this, 4, property as u32);
    }
    0
}
extern "thiscall" fn release(_this: *mut u8) -> u32 {
    log(Call::Release);
    0
}

struct World {
    holder: Object,
    property: Object,
    variant: Object,
    short: Box<[u16; 12]>,
    wide: Box<[u32; 12]>,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<crate::testing::Fake>,
}
impl World {
    fn new(short: bool) -> Self {
        let tables = vec![
            table(&[
                (0x24, lookup as *const () as usize),
                (0x3C, attach as *const () as usize),
            ]),
            table(&[
                (4, retain as *const () as usize),
                (8, release as *const () as usize),
                (0x10, set_id as *const () as usize),
                (0x14, variant as *const () as usize),
            ]),
            table(&[
                (0x0C, kind as *const () as usize),
                (0x188, init as *const () as usize),
                (0x18C, words_with_ecx as *const () as usize),
                (0x198, promote as *const () as usize),
                (0x19C, dwords_with_ecx as *const () as usize),
            ]),
        ];
        let mut h = object(8, &tables[0]);
        let mut p = object(8, &tables[1]);
        let mut v = object(16, &tables[2]);
        let mut small = Box::new([1u16, 30, 3, 4, 5, 25, 7, 8, 9, 10, 11, 12]);
        let mut wide = Box::new([101u32, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, u32::MAX]);
        unsafe {
            store(h.ptr(), 4, p.ptr() as u32);
            store(p.ptr(), 4, v.ptr() as u32);
            store(v.ptr(), 4, if short { 0x8004 } else { 0x8006 });
            store(v.ptr(), 8, small.as_mut_ptr() as u32);
            store(v.ptr(), 12, wide.as_mut_ptr() as u32);
        }
        ALLOCATION.with(|a| *a.borrow_mut() = p.ptr());
        take();
        Self {
            holder: h,
            property: p,
            variant: v,
            short: small,
            wide,
            _tables: tables,
            _fakes: vec![
                fake(0x005F_D840, allocate as *const () as usize),
                fake(0x005F_DE50, construct as *const () as usize),
            ],
        }
    }
    fn prefix(&mut self) -> Vec<Call> {
        vec![
            Call::Lookup(self.holder.ptr() as usize, CAPACITY),
            Call::Variant,
            Call::Type,
        ]
    }
}
unsafe fn invoke_set(holder: *mut u8, index: u32, value: u32) -> Registers {
    unsafe { call_set(fc4p_0070de70 as *const () as usize, holder, index, value) }
}

#[test]
fn capacity_readers_support_short_wide_missing_and_nonstandard_type() {
    for short in [true, false] {
        let mut w = World::new(short);
        for (function, offset) in [
            (fc4p_0070df90 as *const () as usize, 0),
            (fc4p_0070dff0 as *const () as usize, 4),
            (fc4p_0070e050 as *const () as usize, 8),
        ] {
            for index in 0..4 {
                let args = [w.holder.ptr() as u32, index];
                let r = unsafe { invoke(function, 0xDEAD_BEEFusize as *mut u8, &args, 0, 0x9876_5432, false) };
                let want = if short {
                    u32::from(w.short[(index + offset) as usize])
                } else {
                    w.wide[(index + offset) as usize]
                };
                assert_eq!((r.eax, r.edx), (want, if short { 0x1234_5678 } else { 0x8765_4321 }));
                assert_eq!(r.ecx, if offset == 0 { index } else { 0xCAFE_BABE });
                let mut calls = w.prefix();
                calls.push(if short { Call::Words } else { Call::Dwords });
                assert_eq!(take(), calls);
            }
            unsafe {
                store(w.holder.ptr(), 4, 0);
            }
            let r = unsafe {
                invoke(
                    function,
                    core::ptr::null_mut(),
                    &[w.holder.ptr() as u32, 1],
                    0,
                    0x8765,
                    false,
                )
            };
            assert_eq!((r.eax, r.edx), (0, 0x1122_3344));
            assert_eq!(take(), [Call::Lookup(w.holder.ptr() as usize, CAPACITY)]);
            unsafe {
                store(w.holder.ptr(), 4, w.property.ptr() as u32);
            }
        }
        unsafe {
            store(w.variant.ptr(), 4, 0x1234);
        }
        let r = unsafe {
            invoke(
                fc4p_0070dff0 as *const () as usize,
                core::ptr::null_mut(),
                &[w.holder.ptr() as u32, 3],
                0,
                0,
                false,
            )
        };
        assert_eq!(r.eax, w.wide[7]);
        let mut calls = w.prefix();
        calls.push(Call::Dwords);
        assert_eq!(take(), calls);
    }
}

#[test]
fn capacity_short_write_and_promotion_at_exact_65535_boundary() {
    let mut w = World::new(true);
    let original = *w.short;
    for value in [0u32, 65534] {
        let r = unsafe { invoke_set(w.holder.ptr(), 5, value) };
        let mut calls = w.prefix();
        calls.push(Call::Words);
        assert_eq!(take(), calls);
        assert_eq!(
            (r.eax, r.ecx, r.edx),
            (w.short.as_mut_ptr() as u32, 0xCAFE_BABE, 0x1234_5678)
        );
        let mut want = original;
        want[5] = value as u16;
        assert_eq!(*w.short, want);
    }
    let before = *w.short;
    let r = unsafe { invoke_set(w.holder.ptr(), 5, 65535) };
    let mut want = before.map(u32::from);
    want[5] = 65535;
    assert_eq!(*w.wide, want);
    assert_eq!(*w.short, before);
    let mut calls = w.prefix();
    calls.extend([Call::Words, Call::Promote(before.map(u32::from).to_vec()), Call::Dwords]);
    assert_eq!(take(), calls);
    assert_eq!(
        (r.eax, r.ecx, r.edx),
        (w.wide.as_mut_ptr() as u32, 0xCAFE_BABE, 0x8765_4321)
    );
    let r = unsafe { invoke_set(w.holder.ptr(), 11, u32::MAX) };
    let mut calls = w.prefix();
    calls.push(Call::Dwords);
    assert_eq!(take(), calls);
    assert_eq!(w.wide[11], u32::MAX);
    assert_eq!(r.eax, w.wide.as_mut_ptr() as u32);
}

#[test]
fn missing_capacity_constructs_zero_word_array_and_attaches_before_setting() {
    let mut w = World::new(true);
    unsafe {
        store(w.holder.ptr(), 4, 0);
    }
    unsafe {
        invoke_set(w.holder.ptr(), 2, 30);
    }
    assert_eq!(*w.short, [0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        take(),
        [
            Call::Lookup(w.holder.ptr() as usize, CAPACITY),
            Call::Allocate(0x1C),
            Call::Construct(w.property.ptr() as usize, 0),
            Call::Retain,
            Call::Variant,
            Call::Init(vec![0; 12]),
            Call::Id(CAPACITY),
            Call::Attach(w.holder.ptr() as usize, w.property.ptr() as usize, 0),
            Call::Release,
            Call::Variant,
            Call::Type,
            Call::Words
        ]
    );
}

#[test]
fn maximum_capacity_wrapper_uses_stack_holder_and_preserves_helper_result() {
    let mut w = World::new(true);
    let r = unsafe {
        invoke(
            fc4p_0070df70 as *const () as usize,
            0x1234usize as *mut u8,
            &[w.holder.ptr() as u32, 1, 65536],
            0,
            0,
            false,
        )
    };
    assert_eq!(w.wide[1], 65536);
    assert_eq!((r.eax, r.edx), (w.wide.as_mut_ptr() as u32, 0x8765_4321));
    let mut calls = w.prefix();
    calls.extend([
        Call::Words,
        Call::Promote(w.short.map(u32::from).to_vec()),
        Call::Dwords,
    ]);
    assert_eq!(take(), calls);
}
