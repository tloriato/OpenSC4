// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::*;
use crate::testing::{call_thiscall_with_eax, fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Street([i32; 4], u32),
    Allocate(u32),
    Face(i32, i32),
    Release,
    FreeNode(u32),
    FreeMemory(usize),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    memory: Vec<Box<[u32]>>,
    lot: usize,
    fill_list: bool,
    return_lot: bool,
    restriction: usize,
    dev: usize,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}
fn node(s: &mut State) -> *mut u8 {
    let mut p = vec![0xA5A5_A5A5u32; 3].into_boxed_slice();
    let out = p.as_mut_ptr().cast();
    s.memory.push(p);
    out
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        node(&mut s)
    })
}
extern "C" fn free_node_fake(memory: *mut u8) {
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::FreeNode(unsafe { memory.add(8).cast::<u32>().read() }))
    });
}
extern "thiscall" fn street_fake(_this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, per_cell: u32) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Street([x1, z1, x2, z2], per_cell)));
}
extern "thiscall" fn face_fake(_this: *mut u8, x: i32, z: i32, w: i32, h: i32, list: *mut *mut u8, out: *mut *mut u8) {
    assert_eq!((w, h), (1, 1));
    assert!(unsafe { out.read() }.is_null());
    let header = unsafe { list.read() };
    assert_eq!(unsafe { header.cast::<*mut u8>().read() }, header);
    assert_eq!(unsafe { header.add(4).cast::<*mut u8>().read() }, header);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Face(x, z));
        if s.fill_list {
            for facing in 0..4 {
                let p = node(&mut s);
                let prev = unsafe { header.add(4).cast::<*mut u8>().read() };
                unsafe {
                    p.cast::<*mut u8>().write(header);
                    p.add(4).cast::<*mut u8>().write(prev);
                    p.add(8).cast::<u32>().write(facing);
                    prev.cast::<*mut u8>().write(p);
                    header.add(4).cast::<*mut u8>().write(p);
                }
            }
        }
        if s.return_lot {
            unsafe { out.write(s.lot as *mut u8) };
        }
    });
}
extern "thiscall" fn release_fake(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Release));
}
extern "C" fn free_fake(memory: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(
            unsafe { Region::from_ptr(s.restriction as *mut u8) }.vtable(),
            0x00A8_6364
        );
        assert_eq!(
            unsafe { ZoneDeveloper::from_ptr(s.dev as *mut u8) }.restriction() as usize,
            s.restriction
        );
        s.events.push(Event::FreeMemory(memory as usize));
    });
}
struct Fixture {
    dev: Object,
    region: Object,
    _lot: Object,
    _table: Box<[usize]>,
    _columns: Vec<Vec<u32>>,
    rows: Vec<*mut u32>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new(bounds: [i32; 4], q: u32, d: u32, streets: u8) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut table = vec![0usize; 3].into_boxed_slice();
        table[2] = release_fake as *const () as usize;
        let mut lot = Object::new(4);
        unsafe { lot.ptr().cast::<usize>().write(table.as_ptr() as usize) };
        let mut columns = vec![vec![0u32; 1]; 2];
        let mut rows = columns.iter_mut().map(|c| c.as_mut_ptr()).collect::<Vec<_>>();
        let mut region = Object::new(0x24);
        let r = unsafe { Region::from_ptr(region.ptr()) };
        r.set_x1(bounds[0]);
        r.set_z1(bounds[1]);
        r.set_x2(bounds[2]);
        r.set_z2(bounds[3]);
        r.set_width(2);
        r.set_columns(rows.as_mut_ptr());
        let mut dev = Object::new(0xF0);
        let view = unsafe { ZoneDeveloper::from_ptr(dev.ptr()) };
        view.set_restriction(region.ptr());
        view.set_orientation_q(q);
        view.set_orientation_d(d);
        view.set_place_streets(streets);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.lot = lot.ptr() as usize;
            s.restriction = region.ptr() as usize;
            s.dev = dev.ptr() as usize;
        });
        let fakes = vec![
            fake(0x0090_CF54, allocate_fake as *const () as usize),
            fake(0x0090_CF63, free_node_fake as *const () as usize),
            fake(0x0072_D200, face_fake as *const () as usize),
            fake(0x0072_A2A0, street_fake as *const () as usize),
            fake(0x005E_5620, free_fake as *const () as usize),
        ];
        Self {
            dev,
            region,
            _lot: lot,
            _table: table,
            _columns: columns,
            rows,
            _fakes: fakes,
        }
    }
    fn call(&mut self, spacing: i32) {
        unsafe {
            call_thiscall_with_eax(
                fc4p_0072e9b0 as *const () as usize,
                self.dev.ptr(),
                &[spacing as u32],
                0,
            )
        };
        assert!(unsafe { ZoneDeveloper::from_ptr(self.dev.ptr()) }
            .restriction()
            .is_null());
    }
    fn free_events(&mut self) -> Vec<Event> {
        let mut out = self
            .rows
            .iter()
            .map(|&p| Event::FreeMemory(p as usize))
            .collect::<Vec<_>>();
        out.extend([
            Event::FreeMemory(self.rows.as_ptr() as usize),
            Event::FreeMemory(self.region.ptr() as usize),
        ]);
        out
    }
}

#[test]
fn all_orientation_table_entries_place_the_expected_street_lines_before_x_major_lots() {
    for q in 0..4 {
        for d in 0..4 {
            let mut f = Fixture::new([10, 10, 21, 21], q, d, 1);
            f.call(6);
            let log = events();
            let forward_x = match q {
                0 | 2 => d == 3,
                1 => false,
                _ => true,
            };
            let forward_z = match q {
                0 => true,
                2 => false,
                _ => d == 0,
            };
            assert_eq!(
                &log[..2],
                &[
                    Event::Street(
                        [if forward_x { 16 } else { 15 }, 10, if forward_x { 16 } else { 15 }, 21],
                        0
                    ),
                    Event::Street(
                        [10, if forward_z { 16 } else { 15 }, 21, if forward_z { 16 } else { 15 }],
                        0
                    )
                ]
            );
            let mut expected = vec![];
            for x in 10..=21 {
                for z in 10..=21 {
                    expected.extend([Event::Allocate(12), Event::Face(x, z), Event::FreeNode(0xA5A5_A5A5)]);
                }
            }
            expected.extend(f.free_events());
            assert_eq!(&log[2..], expected);
        }
    }
}

#[test]
fn every_cell_owns_a_fresh_list_releases_its_lot_then_frees_each_orientation_and_the_sentinel() {
    let mut f = Fixture::new([2, 3, 3, 4], 0, 1, 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.fill_list = true;
        s.return_lot = true;
    });
    f.call(16);
    let mut expected = vec![];
    for x in 2..=3 {
        for z in 3..=4 {
            expected.extend([
                Event::Allocate(12),
                Event::Face(x, z),
                Event::Release,
                Event::FreeNode(0),
                Event::FreeNode(1),
                Event::FreeNode(2),
                Event::FreeNode(3),
                Event::FreeNode(0xA5A5_A5A5),
            ]);
        }
    }
    expected.extend(f.free_events());
    assert_eq!(events(), expected);
}

#[test]
fn zero_and_negative_spacing_draw_each_interior_line_but_keep_two_cells_at_each_edge() {
    for spacing in [0, -1, i32::MIN] {
        let mut f = Fixture::new([0, 0, 5, 5], 3, 0, 0xFF);
        f.call(spacing);
        let log = events();
        assert_eq!(
            &log[..4],
            &[
                Event::Street([2, 0, 2, 5], 0),
                Event::Street([3, 0, 3, 5], 0),
                Event::Street([0, 2, 5, 2], 0),
                Event::Street([0, 3, 5, 3], 0)
            ]
        );
    }
    let mut f = Fixture::new([0, 0, 3, 3], 0, 0, 1);
    f.call(0);
    assert!(!events().iter().any(|e| matches!(e, Event::Street(..))));
    let mut f = Fixture::new([4, 3, 2, 2], 0, 0, 0);
    f.call(6);
    assert_eq!(events(), f.free_events());
}

#[unsafe(naked)]
unsafe extern "fastcall" fn call_with_directions(_this: *mut u8, _directions: u32, _spacing: i32) {
    core::arch::naked_asm!("mov word ptr [esp - 22], dx","push dword ptr [esp + 4]","call {port}","ret 4",port=sym fc4p_0072e9b0);
}

#[test]
fn invalid_q_uses_spacing_for_both_starts_and_retains_both_direction_bytes() {
    for directions in [0u32, 1, 0x100, 0x8080] {
        let mut f = Fixture::new([0, 0, 9, 9], u32::MAX, 0, 1);
        unsafe { call_with_directions(f.dev.ptr(), directions, 3) };
        let log = events();
        let mut expected = vec![Event::Street([6, 0, 6, 9], 0)];
        if directions as u8 == 0 {
            expected.clear();
        }
        if directions >> 8 != 0 {
            expected.push(Event::Street([0, 6, 9, 6], 0));
        }
        let actual = log
            .into_iter()
            .filter(|e| matches!(e, Event::Street(..)))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
