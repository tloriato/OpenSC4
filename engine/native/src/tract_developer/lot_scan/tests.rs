// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Existing-lot enumeration and the scripted decline cases, using the game's field offsets.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Exit(u32), Map(u32), Shift(u32), Reset(u32, u32), Next(u32),
    Lot(u32, u32, u32, u32), Insert(u32, u32),
    State(u32), Days(u32), Building(u32, u32), Profile(u32, u32),
    Wealth(u32), Purpose(u32), DeveloperWealth(u32), Type(u32),
    Capacity(u32, u32, u32), Abandon(u32, u32, u32),
}
#[derive(Default)]
struct State { calls: Vec<Call>, cells: Vec<(u32, u32, u32)>, index: usize }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) { STATE.with(|s| s.borrow_mut().calls.push(call)); }
const RESULT: u64 = 0xDEAD_BEEF_AABB_CC00;
extern "C" fn exit(callback: u32) -> u64 { log(Call::Exit(callback)); RESULT }
extern "thiscall" fn map(this: u32) -> u64 {
    log(Call::Map(this)); RESULT & !0xFFFF_FFFF | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn shift(this: u32) -> u64 {
    log(Call::Shift(this)); RESULT & !0xFFFF_FFFF | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn reset(this: u32, flag: u32) -> u64 {
    log(Call::Reset(this, flag)); STATE.with(|s| s.borrow_mut().index = 0); RESULT
}
extern "thiscall" fn next(this: u32, x: *mut u32, z: *mut u32) -> u64 {
    log(Call::Next(this));
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        if s.index == s.cells.len() { return RESULT; }
        let (xx, zz, _) = s.cells[s.index]; s.index += 1;
        unsafe { *x = xx; *z = zz; }
        RESULT | 1
    })
}
extern "thiscall" fn lot(this: u32, x: u32, z: u32, flag: u32) -> u64 {
    log(Call::Lot(this, x, z, flag));
    STATE.with(|s| {
        let s = s.borrow();
        let cell = s.cells[s.index - 1]; assert_eq!((x, z), (cell.0, cell.1));
        RESULT & !0xFFFF_FFFF | u64::from(cell.2)
    })
}
extern "thiscall" fn insert(this: u32, result: *mut u32, lot: *mut u32) -> u64 {
    log(Call::Insert(this, unsafe { *lot }));
    unsafe { *result = 0x1122_3344; *result.add(1) = 1; }
    RESULT
}
macro_rules! getter {
    ($name:ident, $call:ident, $offset:literal) => {
        extern "thiscall" fn $name(this: u32) -> u64 {
            log(Call::$call(this));
            RESULT & !0xFFFF_FFFF | u64::from(unsafe { word(this, $offset) })
        }
    };
}
getter!(state, State, 4);
getter!(days, Days, 8);
getter!(wealth, Wealth, 16);
getter!(purpose, Purpose, 12);
getter!(developer_wealth, DeveloperWealth, 16);
getter!(kind, Type, 20);
extern "thiscall" fn building(this: u32, flag: u32) -> u64 {
    log(Call::Building(this, flag)); RESULT & !0xFFFF_FFFF | u64::from(unsafe { word(this, 12) })
}
extern "thiscall" fn profile(this: u32, building: u32) -> u64 {
    log(Call::Profile(this, building)); RESULT & !0xFFFF_FFFF | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn capacity(this: u32, kind: u32, flag: u32) -> u64 {
    log(Call::Capacity(this, kind, flag)); RESULT & !0xFFFF_FFFF | u64::from(unsafe { word(this, 20) })
}
extern "thiscall" fn abandon(this: u32, lot: u32, reason: u32) -> u64 {
    log(Call::Abandon(this, lot, reason)); unsafe { store(lot, 4, 4); } RESULT
}

struct World {
    td: Object, developer: Box<[u32; 6]>, map: Box<[u32; 2]>,
    manager: Box<[u32; 2]>, bds: Box<[u32; 2]>, lot_developer: Box<[u32; 2]>,
    building_profile: Box<[u32; 2]>, lots: Box<[[u32; 6]; 3]>,
    tables: [Box<[usize; 56]>; 6], global: Box<[u8; 0x34]>,
    profile: Box<[u32; 3]>, _fakes: Vec<Fake>,
}
impl World {
    fn new(decline: bool) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            td: Object::new(0x180), developer: Box::new([0; 6]), map: Box::new([0; 2]),
            manager: Box::new([0; 2]), bds: Box::new([0; 2]), lot_developer: Box::new([0; 2]),
            building_profile: Box::new([1, 0]), lots: Box::new([[0; 6]; 3]),
            tables: core::array::from_fn(|_| Box::new([0; 56])), global: Box::new([0xCC; 0x34]),
            profile: Box::new([0; 3]), _fakes: Vec::new(),
        };
        w.tables[0][0x34 / 4] = map as *const () as usize;
        w.tables[0][0x1C / 4] = purpose as *const () as usize;
        w.tables[0][0x20 / 4] = developer_wealth as *const () as usize;
        w.tables[0][0x18 / 4] = kind as *const () as usize;
        w.tables[1][0x2C / 4] = shift as *const () as usize;
        w.tables[2][0x18 / 4] = lot as *const () as usize;
        w.tables[3][0x9C / 4] = profile as *const () as usize;
        w.tables[4][0x20 / 4] = abandon as *const () as usize;
        for (offset, function) in [(0x84, state as *const () as usize), (0xA0, days as *const () as usize),
            (0xB8, building as *const () as usize), (0xA4, wealth as *const () as usize), (0xC8, capacity as *const () as usize)] {
            w.tables[5][offset / 4] = function;
        }
        w.map[0] = w.tables[1].as_ptr() as u32; w.map[1] = 2;
        *w.developer = [w.tables[0].as_ptr() as u32, w.map.as_ptr() as u32, 0, 1, 2, 0x1010];
        w.manager[0] = w.tables[2].as_ptr() as u32;
        *w.bds = [w.tables[3].as_ptr() as u32, w.building_profile.as_ptr() as u32];
        w.lot_developer[0] = w.tables[4].as_ptr() as u32;
        for lot in w.lots.iter_mut() { *lot = [w.tables[5].as_ptr() as u32, 3, 60, 0x300, 2, 40]; }
        *w.profile = [w.developer.as_ptr() as u32, 3, 4];
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_developer(w.developer.as_ptr() as u32); td.set_tract_x(3); td.set_tract_z(4);
        td.set_lot_manager(w.manager.as_ptr() as u32); td.set_building_development(w.bds.as_ptr() as u32);
        td.set_lot_developer(w.lot_developer.as_ptr() as u32);
        let base = if decline { 0x00B4_B0CC } else { 0x00B4_B098 };
        unsafe { write_unaligned(w.global.as_mut_ptr().add(0x30).cast::<u32>(), 0x8000_0000); }
        w._fakes.push(fake_data(base, w.global.as_mut_ptr()));
        w._fakes.push(fake_data(base + 0x30, unsafe { w.global.as_mut_ptr().add(0x30) }));
        for (address, f) in [(0x009E_F059, exit as *const () as usize), (0x0049_D1C0, reset as *const () as usize),
            (0x0049_CB10, next as *const () as usize), (0x0049_A050, insert as *const () as usize)] { w._fakes.push(fake(address, f)); }
        w
    }
    fn cells(&self, indexes: &[Option<usize>]) {
        STATE.with(|s| s.borrow_mut().cells = indexes.iter().enumerate().map(|(i, index)|
            (12 + i as u32, 16 + i as u32, index.map_or(0, |i| self.lots[i].as_ptr() as u32))).collect());
    }
    fn call(&mut self, decline: bool) -> (u32, u32, u32) {
        let args = [self.profile.as_ptr() as u32];
        run(if decline { 0x00709200 } else { 0x00707FB0 }, Conv::Thiscall, self.td.ptr() as u32,
            if decline { &args } else { &[] })
    }
    fn prefix(&self, decline: bool) -> Vec<Call> {
        vec![Call::Exit(if decline { 0x00A7_D270 } else { 0x00A7_D290 }),
            Call::Map(self.developer.as_ptr() as u32), Call::Shift(self.map.as_ptr() as u32),
            Call::Reset(self.global.as_ptr() as u32, 1), Call::Next(self.global.as_ptr() as u32)]
    }
}

#[test]
fn existing_visits_cells_and_inserts_duplicate_borrowed_lots_in_call_order() {
    let _machine = PreserveMachine::new();
    let mut w = World::new(false); w.cells(&[Some(0), None, Some(0), Some(1)]);
    let before = snapshot(w.td.ptr(), 0x180);
    let regs = w.call(false); assert_eq!((regs.0, regs.2), (RESULT as u32, (RESULT >> 32) as u32));
    let mut expected = w.prefix(false);
    for (i, index) in [Some(0), None, Some(0), Some(1)].into_iter().enumerate() {
        expected.push(Call::Lot(w.manager.as_ptr() as u32, 12 + i as u32, 16 + i as u32, 0));
        if let Some(index) = index { expected.push(Call::Insert(w.td.ptr() as u32 + 0xCC, w.lots[index].as_ptr() as u32)); }
        expected.push(Call::Next(w.global.as_ptr() as u32));
    }
    STATE.with(|s| assert_eq!(s.borrow().calls, expected));
    assert_eq!(snapshot(w.td.ptr(), 0x180), before);
    let mut global = [0u8; 0x34]; global[..3].fill(1); global[3] = 0xCC;
    for (offset, value) in [(4, 12u32), (8, 16), (12, 15), (16, 19), (0x2C, 1), (0x30, 0x8000_0001)] {
        global[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(*w.global, global);
    STATE.with(|s| s.borrow_mut().calls.clear()); w.call(false);
    STATE.with(|s| assert!(!s.borrow().calls.iter().any(|c| matches!(c, Call::Exit(_)))));
}

#[test]
fn iterator_masks_shift_counts_wraps_coordinates_and_clamps_signed_bounds() {
    for (x, z, shift_value) in [(i32::MAX, -1, 0), (i32::MIN, 0, 32), (0x3FFF_FFFF, -1, 34), (5, 7, 255)] {
        let mut w = World::new(false); w.map[1] = shift_value;
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) }; td.set_tract_x(x); td.set_tract_z(z);
        w.call(false);
        let lo_x = x.wrapping_shl(shift_value); let lo_z = z.wrapping_shl(shift_value);
        let hi_x = x.wrapping_add(1).wrapping_shl(shift_value).wrapping_sub(1).max(lo_x);
        let hi_z = z.wrapping_add(1).wrapping_shl(shift_value).wrapping_sub(1).max(lo_z);
        let p = w.global.as_ptr() as u32;
        assert_eq!(unsafe { [word(p, 4), word(p, 8), word(p, 12), word(p, 16)] },
            [lo_x as u32, lo_z as u32, hi_x as u32, hi_z as u32]);
    }
}

#[test]
fn decline_abandons_matching_lots_and_counts_capacity_once_after_state_change() {
    let _fpu = GameFpu::new(); let _machine = PreserveMachine::new();
    let mut w = World::new(true); w.cells(&[Some(0), Some(0)]);
    w.lots[0][5] = 0x1234_0028;
    let before = snapshot(w.td.ptr(), 0x180);
    assert_eq!(w.call(true).0, 40); assert_eq!(w.lots[0][1], 4);
    let l = w.lots[0].as_ptr() as u32; let d = w.developer.as_ptr() as u32;
    let mut expected = w.prefix(true);
    expected.extend([Call::Lot(w.manager.as_ptr() as u32, 12, 16, 0), Call::State(l), Call::Days(l),
        Call::Building(l, 0), Call::Profile(w.bds.as_ptr() as u32, 0x300), Call::Wealth(l),
        Call::Purpose(d), Call::DeveloperWealth(d), Call::Type(d), Call::Capacity(l, 0x1010, 0),
        Call::Abandon(w.lot_developer.as_ptr() as u32, l, 6), Call::Next(w.global.as_ptr() as u32),
        Call::Lot(w.manager.as_ptr() as u32, 13, 17, 0), Call::State(l), Call::Next(w.global.as_ptr() as u32)]);
    STATE.with(|s| assert_eq!(s.borrow().calls, expected));
    assert_eq!(snapshot(w.td.ptr(), 0x180), before);
    assert_eq!(w.call(true).0, 0);
}

#[test]
fn decline_checks_null_state_unsigned_age_building_purpose_and_wealth_in_order() {
    for case in 0..8 {
        let mut w = World::new(true); w.cells(&[if case == 0 { None } else { Some(0) }]);
        match case {
            1 => w.lots[0][1] = 4, 2 => w.lots[0][2] = 59, 3 => w.lots[0][3] = 0,
            4 => w.building_profile[0] = 2, 5 => w.lots[0][4] = 3,
            6 => w.lots[0][2] = u32::MAX, _ => {}
        }
        let success = case >= 6;
        assert_eq!(w.call(true).0, if success { 40 } else { 0 });
        let calls = STATE.with(|s| s.borrow().calls.clone());
        let checks = calls.iter().filter(|c| matches!(c, Call::State(_) | Call::Days(_) | Call::Building(..)
            | Call::Profile(..) | Call::Wealth(_) | Call::Purpose(_) | Call::DeveloperWealth(_)
            | Call::Type(_) | Call::Capacity(..) | Call::Abandon(..))).count();
        assert_eq!(checks, [0, 1, 2, 3, 6, 7, 10, 10][case]);
    }
}

#[test]
fn decline_iterator_overwrites_the_profile_argument_with_the_last_z() {
    let mut w = World::new(true); w.cells(&[None, None]);
    let mut entry = Entry { eax: 0x1234_5678, ecx: w.td.ptr() as u32, edx: 0x8765_4321,
        return_address: 0, args: [w.profile.as_ptr() as u32] };
    unsafe { decline(&mut entry); }
    assert_eq!(entry.args, [17]); assert_eq!(entry.eax, 0);
}
