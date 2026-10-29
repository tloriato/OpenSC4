// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, float_environment, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Holder,
    Bytes(usize, u32),
    Position,
    Lot(usize, u32, u32, u32),
    Reserve(usize, u32),
    Facing,
    Allocate(u32),
    Free(usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    raw: [u32; 3],
    holder: usize,
    lot: usize,
    facing: u32,
    allocations: Vec<usize>,
    switch: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn holder(_: *mut u8) -> *mut u8 {
    log(Call::Holder);
    STATE.with(|s| s.borrow().holder as *mut u8)
}
unsafe extern "C" fn property(h: *mut u8, id: u32, out: *mut u32) {
    log(Call::Bytes(h as usize, id));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            for (i, &v) in s.raw.iter().enumerate() {
                out.add(i).write(v);
            }
        }
    });
}
unsafe extern "thiscall" fn position(_: *mut u8, out: *mut u32) {
    log(Call::Position);
    STATE.with(|s| {
        assert_eq!(
            unsafe { TransitSwitch::from_ptr(s.borrow().switch as *mut u8) }.mask(),
            0
        )
    });
    unsafe {
        out.write(12);
        out.add(1).write(34);
    }
}
extern "thiscall" fn lot(this: *mut u8, x: u32, z: u32, zero: u32) -> *mut u8 {
    log(Call::Lot(this as usize, x, z, zero));
    STATE.with(|s| s.borrow().lot as *mut u8)
}
extern "thiscall" fn facing(_: *mut u8) -> u32 {
    log(Call::Facing);
    STATE.with(|s| s.borrow().facing)
}
extern "thiscall" fn reserve(this: *mut u8, count: u32) {
    log(Call::Reserve(this as usize, count));
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow_mut().allocations.remove(0) as *mut u8)
}
extern "C" fn free(p: *mut u8) {
    log(Call::Free(p as usize));
}

struct World {
    switch: Object,
    _occupant: Object,
    manager: Object,
    _lot: Object,
    raw: Object,
    rules: Object,
    _manager_global: Box<*mut u8>,
    _rotations: Box<[u32; 20]>,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(entries: &[u32], facing_value: u32) -> Self {
        let tables = vec![
            table(&[
                (0x18, holder as *const () as usize),
                (0x2C, position as *const () as usize),
            ]),
            table(&[(0x18, lot as *const () as usize)]),
            table(&[(0x5C, facing as *const () as usize)]),
        ];
        let mut occupant = object(0x20, &tables[0]);
        let mut manager = object(0x20, &tables[1]);
        let mut lot_object = object(0x20, &tables[2]);
        let mut manager_global = Box::new(manager.ptr());
        // The fifth row makes an out-of-range facing observable instead of masking it to two bits.
        let mut rotations = Box::new([2, 3, 0, 1, 1, 2, 3, 0, 0, 1, 2, 3, 3, 0, 1, 2, 0, 1, 2, 3]);
        let mut raw = Object::new(entries.len() * 4 + 16);
        unsafe {
            for (i, &v) in entries.iter().enumerate() {
                raw.ptr().cast::<u32>().add(i).write(v);
            }
        }
        let mut rules = Object::new(512);
        unsafe {
            core::ptr::write_bytes(rules.ptr(), 0xA5, 512);
        }
        let mut switch = Object::new(0x40);
        unsafe {
            core::ptr::write_bytes(switch.ptr(), 0xA5, 0x40);
        }
        let view = unsafe { TransitSwitch::from_ptr(switch.ptr()) };
        view.set_occupant(occupant.ptr());
        view.set_begin(rules.ptr());
        view.set_end(rules.ptr());
        view.set_capacity(rules.ptr().wrapping_add(500));
        let fakes = vec![
            fake(0x005F_FB40, property as *const () as usize),
            fake(0x0071_1CD0, reserve as *const () as usize),
            fake(0x0090_CF54, allocate as *const () as usize),
            fake(0x0090_CF63, free as *const () as usize),
            fake_data(0x00B4_3D08, (&mut *manager_global as *mut *mut u8).cast()),
            fake_data(0x00AB_3280, rotations.as_mut_ptr().cast()),
        ];
        STATE.with(|s| {
            *s.borrow_mut() = State {
                raw: [raw.ptr() as u32, raw.ptr().wrapping_add(entries.len() * 4) as u32, 0],
                holder: 0x1234,
                lot: lot_object.ptr() as usize,
                facing: facing_value,
                switch: switch.ptr() as usize,
                ..State::default()
            }
        });
        Self {
            switch,
            _occupant: occupant,
            manager,
            _lot: lot_object,
            raw,
            rules,
            _manager_global: manager_global,
            _rotations: rotations,
            _tables: tables,
            _fakes: fakes,
        }
    }
    fn view(&mut self) -> TransitSwitch {
        unsafe { TransitSwitch::from_ptr(self.switch.ptr()) }
    }
    fn run(&mut self, padding: u8) {
        let frame = [
            fc4p_00715cb0 as *const () as u32,
            self.switch.ptr() as u32,
            u32::from(padding),
        ];
        unsafe {
            core::arch::asm!("push [edi + 4]", "mov eax, [edi + 8]", "mov byte ptr [esp - 0x1b], al",
                "mov ecx, 0x87654321", "call dword ptr [edi]", in("edi") frame.as_ptr(), clobber_abi("C"));
        }
    }
    fn records(&mut self) -> Vec<[u8; 10]> {
        let v = self.view();
        let count = (v.end() as usize - v.begin() as usize) / 10;
        (0..count)
            .map(|i| unsafe { v.begin().add(i * 10).cast::<[u8; 10]>().read_unaligned() })
            .collect()
    }
    fn prefix(&mut self, n: u32) -> Vec<Call> {
        vec![
            Call::Holder,
            Call::Bytes(0x1234, 0xE90E_25A1),
            Call::Position,
            Call::Lot(self.manager.ptr() as usize, 12, 34, 0),
            Call::Reserve(self.switch.ptr() as usize + 0x1C, n),
            Call::Facing,
        ]
    }
}

#[test]
fn rule_rotation_and_kind_vectors_from_core() {
    let mut w = World::new(&[0x0101_8000], 1);
    let env = float_environment();
    let mut expected = unsafe { bytes(w.switch.ptr(), 0x40) };
    expected[0x10] = 0;
    expected[0x20..0x24].copy_from_slice(&(w.rules.ptr().wrapping_add(10) as u32).to_le_bytes());
    expected[0x28..0x2A].copy_from_slice(&2u16.to_le_bytes());
    w.run(0xD7);
    assert_eq!(w.records(), [[0, 0, 1, 0, 0, 0xD7, 1, 0, 1, 0]]);
    assert_eq!(unsafe { bytes(w.switch.ptr(), 0x40) }, expected);
    assert_eq!(float_environment(), env);
    let mut expected = w.prefix(1);
    expected.push(Call::Free(w.raw.ptr() as usize));
    assert_eq!(calls(), expected);
    let mut w = World::new(&[0x0302_8082, 0x0607_0000], 2);
    w.run(0xE1);
    assert_eq!(
        w.records(),
        [[1, 1, 0, 0, 0, 0xE1, 2, 0, 3, 0], [0, 0, 0, 0, 0, 0xE1, 7, 0, 6, 0]]
    );
    assert_eq!((w.view().mask(), w.view().flags()), (1 << 6, 0x46));
}

#[test]
fn station_flags_include_rules_of_both_kinds_for_every_facing() {
    for facing in 0..4 {
        let mut w = World::new(&[0x0707_F081, 0x0607_F082, 0x0706_F081, 0x0707_F082], facing);
        w.run(0x9B);
        let records = w.records();
        assert_eq!(records.iter().map(|r| r[0]).collect::<Vec<_>>(), [0, 1, 0, 1]);
        assert!(records.iter().all(|r| r[1..5] == [1; 4] && r[5] == 0x9B));
        assert_eq!((w.view().mask(), w.view().flags()), (0x80, 0x4C));
    }
}

#[test]
fn missing_lot_only_clears_mask_and_frees_property_bytes() {
    let mut w = World::new(&[0x0101_8000], 0);
    STATE.with(|s| s.borrow_mut().lot = 0);
    let mut expected = unsafe { bytes(w.switch.ptr(), 0x40) };
    expected[0x28..0x2A].fill(0);
    w.run(0);
    assert_eq!(unsafe { bytes(w.switch.ptr(), 0x40) }, expected);
    assert_eq!(
        calls(),
        [
            Call::Holder,
            Call::Bytes(0x1234, 0xE90E_25A1),
            Call::Position,
            Call::Lot(w.manager.ptr() as usize, 12, 34, 0),
            Call::Free(w.raw.ptr() as usize)
        ]
    );
}

#[test]
fn empty_bytes_clear_flags_but_retain_existing_rules_and_skip_null_free() {
    let mut w = World::new(&[], 0);
    let v = w.view();
    v.set_end(v.begin().wrapping_add(10));
    STATE.with(|s| s.borrow_mut().raw = [0; 3]);
    let before = unsafe { bytes(w.rules.ptr(), 20) };
    w.run(0x12);
    assert_eq!((w.view().mask(), w.view().flags()), (0, 0));
    assert_eq!(unsafe { bytes(w.rules.ptr(), 20) }, before);
    assert_eq!(calls(), w.prefix(0));
}

#[test]
fn append_retains_prior_records_and_flags_use_first_records() {
    let mut w = World::new(&[0x0301_8000], 0);
    unsafe {
        w.rules
            .ptr()
            .cast::<[u8; 10]>()
            .write([0, 0, 0, 0, 0, 0xAA, 6, 0, 7, 0]);
    }
    let v = w.view();
    v.set_end(v.begin().wrapping_add(10));
    w.run(0xBB);
    assert_eq!(
        w.records(),
        [[0, 0, 0, 0, 0, 0xAA, 6, 0, 7, 0], [0, 0, 0, 1, 0, 0xBB, 1, 0, 3, 0]]
    );
    assert_eq!((w.view().mask(), w.view().flags()), (8, 0x48));
}

#[test]
fn growth_copies_ten_bytes_and_frees_old_storage_before_replacing_fields() {
    let mut w = World::new(&[0x0301_8000, 0x0802_0082], 0);
    let v = w.view();
    v.set_begin(core::ptr::null_mut());
    v.set_end(core::ptr::null_mut());
    v.set_capacity(core::ptr::null_mut());
    let mut first = Object::new(16);
    let mut second = Object::new(32);
    unsafe {
        core::ptr::write_bytes(second.ptr(), 0xA5, 32);
    }
    STATE.with(|s| s.borrow_mut().allocations = vec![first.ptr() as usize, second.ptr() as usize]);
    w.run(0xDD);
    assert_eq!(
        w.records(),
        [[0, 0, 0, 1, 0, 0xDD, 1, 0, 3, 0], [1, 0, 0, 0, 0, 0xDD, 2, 0, 8, 0]]
    );
    assert_eq!(w.view().capacity(), second.ptr().wrapping_add(20));
    assert_eq!(unsafe { bytes(second.ptr().add(20), 12) }, [0xA5; 12]);
    let mut expected = w.prefix(2);
    expected.extend([
        Call::Allocate(10),
        Call::Allocate(20),
        Call::Free(first.ptr() as usize),
        Call::Free(w.raw.ptr() as usize),
    ]);
    assert_eq!(calls(), expected);
}

#[test]
fn destination_shift_masks_the_count_and_flags_ignore_type_five() {
    let mut w = World::new(&[0x2101_8000, 0x1001_0000, 0x0501_0000], 4);
    w.run(0x2A);
    assert_eq!(w.records()[0], [0, 1, 0, 0, 0, 0x2A, 1, 0, 33, 0]);
    assert_eq!((w.view().mask(), w.view().flags()), (2 | 32, 0));
}

#[test]
fn partial_property_entry_reads_four_bytes_but_flag_count_rounds_down() {
    let mut w = World::new(&[0x0706_0081], 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.raw[1] = s.raw[0] + 1;
    });
    w.run(0xCC);
    assert_eq!(w.records(), [[0, 0, 0, 0, 0, 0xCC, 6, 0, 7, 0]]);
    assert_eq!((w.view().mask(), w.view().flags()), (0x80, 0));
    let mut expected = w.prefix(0);
    expected.push(Call::Free(w.raw.ptr() as usize));
    assert_eq!(calls(), expected);
}
