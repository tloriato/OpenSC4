// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Edge removal cases from the core's occupant connection tests.

use super::*;
use crate::abi::Conv;
use crate::neighbors::tests::checked_call;
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Type,
    Query(u32),
    Network(u32),
    Release,
    Position,
    Cell(u32, u32),
    Width,
    Height,
    Delete(u32, i32, i32),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

fn record(call: Call) { CALLS.with(|c| c.borrow_mut().push(call)); }
fn take() -> Vec<Call> { CALLS.with(|c| core::mem::take(&mut *c.borrow_mut())) }

#[repr(C)]
struct Occupant {
    table: *const usize,
    kind: u32,
    mask: u32,
    query_result: u32,
    give_interface: bool,
}

extern "thiscall" fn kind(this: *mut Occupant) -> u64 {
    record(Call::Type);
    pair(unsafe { (*this).kind }, 0x1122_3344)
}

extern "thiscall" fn query(this: *mut Occupant, iid: u32, out: *mut *mut Occupant) -> u64 {
    record(Call::Query(iid));
    unsafe { if (*this).give_interface { out.write_unaligned(this); } }
    pair(unsafe { (*this).query_result }, 0x5566_7788)
}

extern "thiscall" fn network(this: *mut Occupant, kind: u32) -> u64 {
    record(Call::Network(kind));
    pair(0xABCD_EF00 | u32::from(unsafe { (*this).mask } & (1 << kind) != 0), 0x1234_5678)
}

extern "thiscall" fn release(_this: *mut Occupant) -> u64 {
    record(Call::Release);
    pair(0x8877_6655, 0x4433_2211)
}

extern "thiscall" fn position(_this: *mut Occupant, out: *mut u32) -> u64 {
    record(Call::Position);
    unsafe {
        out.write_unaligned(0x3F00_0000);
        out.add(1).write_unaligned(0x4321_0000);
        out.add(2).write_unaligned(0x4000_0000);
    }
    pair(0x2525_2525, 0x2626_2626)
}

#[repr(C)]
struct City { table: *const usize, x: i32, z: i32, width: i32, height: i32 }

extern "thiscall" fn cell(this: *mut City, x: u32, z: u32, cx: *mut i32, cz: *mut i32) -> u64 {
    record(Call::Cell(x, z));
    unsafe { cx.write_unaligned((*this).x); cz.write_unaligned((*this).z); }
    pair(0x1717_1717, 0x1818_1818)
}

extern "thiscall" fn width(this: *mut City) -> u64 {
    record(Call::Width);
    pair(unsafe { (*this).width } as u32, 0x7474_7474)
}

extern "thiscall" fn height(this: *mut City) -> u64 {
    record(Call::Height);
    pair(unsafe { (*this).height } as u32, 0x7878_7878)
}

extern "thiscall" fn delete(_this: *mut u8, kind: u32, x: i32, z: i32) -> u64 {
    record(Call::Delete(kind, x, z));
    pair(0xDEAD_BEEF, 0xCAFE_BABE)
}

struct World {
    sim: [u32; 18],
    occupant: Occupant,
    city: Box<City>,
    _tables: [Box<[usize; 96]>; 3],
}

impl World {
    fn new() -> Self {
        let mut tables = [Box::new([0usize; 96]), Box::new([0; 96]), Box::new([0; 96])];
        tables[0][0x1C / 4] = delete as *const () as usize;
        tables[1][0] = query as *const () as usize;
        tables[1][2] = release as *const () as usize;
        tables[1][0x1C / 4] = kind as *const () as usize;
        tables[1][0x20 / 4] = position as *const () as usize;
        tables[1][0x58 / 4] = network as *const () as usize;
        tables[2][0x174 / 4] = width as *const () as usize;
        tables[2][0x178 / 4] = height as *const () as usize;
        tables[2][0x17C / 4] = cell as *const () as usize;
        let mut city = Box::new(City { table: tables[2].as_ptr(), x: 0, z: 5, width: 64, height: 64 });
        let mut sim = [0xA5A5_A5A5; 18];
        sim[0] = tables[0].as_ptr() as u32;
        sim[0x14 / 4] = &mut *city as *mut _ as u32;
        Self { sim, occupant: Occupant { table: tables[1].as_ptr(), kind: 0xC772_BF98, mask: 3, query_result: 0xAA00_0080, give_interface: true }, city, _tables: tables }
    }

    fn run(&mut self) -> (u64, u32) {
        let original = &mut self.occupant as *mut _ as u32;
        let mut argument = [original];
        let before = self.sim;
        let result = unsafe { checked_call(fc4p_006cee40 as *const () as usize, self.sim.as_mut_ptr().cast(), &mut argument, 0xDEAD_BEEF, 0x0BAD_F00D, Conv::Thiscall) };
        assert_eq!(self.sim, before);
        (result, argument[0])
    }
}

fn prefix(mask: u32) -> Vec<Call> {
    let mut calls = vec![Call::Type, Call::Query(0xA821_EF94)];
    if mask != u32::MAX {
        calls.extend([4, 11, 0, 6, 12, 2, 7, 8, 9, 1].map(Call::Network));
    }
    calls.push(Call::Release);
    calls
}

#[test]
fn road_and_rail_select_rail_and_corner_pole_deletes_both_edges() {
    let mut w = World::new();
    assert_eq!(w.run(), (pair(5, 0x7878_7878), 0));
    let mut calls = prefix(3);
    calls.extend([Call::Position, Call::Cell(0x3F00_0000, 0x4000_0000), Call::Delete(1, -1, 5), Call::Height]);
    assert_eq!(take(), calls);
    w.occupant.kind = 0x2890_D4DE;
    w.city.z = 0;
    assert_eq!(w.run(), (pair(0xDEAD_BEEF, 0xCAFE_BABE), 0));
    assert_eq!(take(), [Call::Type, Call::Position, Call::Cell(0x3F00_0000, 0x4000_0000), Call::Delete(4, -1, 0), Call::Delete(4, 0, -1)]);
}

#[test]
fn each_network_and_later_priority_match_including_empty_mask() {
    let mut w = World::new();
    for (network, connection) in [(4, 5), (11, 10), (0, 2), (6, 8), (12, 12), (2, 3), (7, 6), (8, 7), (9, 11), (1, 1)] {
        w.occupant.mask = 1 << network;
        assert_eq!(w.run(), (pair(5, 0x7878_7878), 0));
        let mut calls = prefix(0);
        calls.extend([Call::Position, Call::Cell(0x3F00_0000, 0x4000_0000), Call::Delete(connection, -1, 5), Call::Height]);
        assert_eq!(take(), calls);
    }
    for mask in [0, 1 << 3] {
        w.occupant.mask = mask;
        let original = &mut w.occupant as *mut _ as u32;
        assert_eq!(w.run(), (pair(0x8877_6655, 0x4433_2211), original));
        assert_eq!(take(), prefix(0));
    }
}

#[test]
fn query_failure_releases_only_a_returned_interface_and_other_types_leave_argument_alone() {
    let mut w = World::new();
    w.occupant.query_result = 0xAABB_CC00;
    let original = &mut w.occupant as *mut _ as u32;
    assert_eq!(w.run(), (pair(0x8877_6655, 0x4433_2211), original));
    assert_eq!(take(), prefix(u32::MAX));
    w.occupant.give_interface = false;
    assert_eq!(w.run(), (pair(0xAABB_CC00, 0x5566_7788), 0));
    assert_eq!(take(), [Call::Type, Call::Query(0xA821_EF94)]);
    for kind in [0, 1, u32::MAX, 0xC772_BF97, 0x2890_D4DF] {
        w.occupant.kind = kind;
        assert_eq!(w.run(), (pair(kind, 0x1122_3344), original));
        assert_eq!(take(), [Call::Type]);
    }
}

#[test]
fn all_edges_interior_and_wrapping_dimensions() {
    let mut w = World::new();
    w.occupant.kind = 0x2890_D4DE;
    for (x, z, width, height, suffix, result) in [
        (63, 63, 64, 64, vec![Call::Width, Call::Delete(4, 64, 63), Call::Height, Call::Delete(4, 63, 64)], pair(0xDEAD_BEEF, 0xCAFE_BABE)),
        (5, 5, 64, 64, vec![Call::Width, Call::Height], pair(5, 0x7878_7878)),
        (5, 0, 64, 64, vec![Call::Width, Call::Delete(4, 5, -1)], pair(0xDEAD_BEEF, 0xCAFE_BABE)),
        (0, 0, 1, 1, vec![Call::Delete(4, -1, 0), Call::Delete(4, 0, -1)], pair(0xDEAD_BEEF, 0xCAFE_BABE)),
        (-1, -1, 0, 0, vec![Call::Width, Call::Delete(4, 0, -1), Call::Height, Call::Delete(4, -1, 0)], pair(0xDEAD_BEEF, 0xCAFE_BABE)),
        (i32::MAX, i32::MAX, i32::MIN, i32::MIN, vec![Call::Width, Call::Delete(4, i32::MIN, i32::MAX), Call::Height, Call::Delete(4, i32::MAX, i32::MIN)], pair(0xDEAD_BEEF, 0xCAFE_BABE)),
    ] {
        (w.city.x, w.city.z, w.city.width, w.city.height) = (x, z, width, height);
        assert_eq!(w.run(), (result, x as u32));
        let mut calls = vec![Call::Type, Call::Position, Call::Cell(0x3F00_0000, 0x4000_0000)];
        calls.extend(suffix);
        assert_eq!(take(), calls);
    }
}
