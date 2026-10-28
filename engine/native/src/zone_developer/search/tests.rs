// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Clear,
    ConstructIterator,
    IteratorBounds([i32; 4]),
    Advance(i32, i32),
    End,
    Anchor(u32),
    Bounds(u32),
    AddRef(u32),
    Release(u32),
    Allocate(u32),
    Append(u32),
}
#[derive(Clone)]
struct Lot {
    anchor: [i32; 2],
    bounds: [i32; 4],
    refs: i32,
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    lots: Vec<Lot>,
    memory: Vec<Box<[u32]>>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}
fn id(lot: *mut u8) -> u32 {
    unsafe { lot.add(4).cast::<u32>().read() }
}
fn node(s: &mut State) -> *mut u8 {
    let mut memory = vec![0xA5A5_A5A5u32; 3].into_boxed_slice();
    let p = memory.as_mut_ptr().cast();
    s.memory.push(memory);
    p
}
unsafe fn link(header: *mut u8, node: *mut u8, lot: *mut u8) {
    unsafe {
        let previous = header.add(4).cast::<*mut u8>().read();
        node.cast::<*mut u8>().write(header);
        node.add(4).cast::<*mut u8>().write(previous);
        node.add(8).cast::<*mut u8>().write(lot);
        previous.cast::<*mut u8>().write(node);
        header.add(4).cast::<*mut u8>().write(node);
    }
}
extern "thiscall" fn clear_fake(out: *mut *mut u8) -> u64 {
    let header = unsafe { out.read() };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Clear);
        let mut n = unsafe { header.cast::<*mut u8>().read() };
        while n != header {
            let lot = unsafe { n.add(8).cast::<*mut u8>().read() };
            s.lots[id(lot) as usize].refs -= 1;
            n = unsafe { n.cast::<*mut u8>().read() };
        }
        unsafe {
            header.cast::<*mut u8>().write(header);
            header.add(4).cast::<*mut u8>().write(header);
        }
    });
    0xAABB_CCDD_1234_5678
}
extern "thiscall" fn constructor_fake(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::ConstructIterator));
    let r = unsafe { RectangleIterator::from_ptr(this) };
    r.set_state(0);
    r.set_step(1);
}
extern "thiscall" fn iterator_bounds_fake(this: *mut u8, bounds: *const i32) {
    let b = unsafe { bounds.cast::<[i32; 4]>().read() };
    STATE.with(|s| s.borrow_mut().events.push(Event::IteratorBounds(b)));
    let r = unsafe { RectangleIterator::from_ptr(this) };
    r.set_x1(b[0]);
    r.set_z1(b[1]);
    r.set_x2(b[2].max(b[0]));
    r.set_z2(b[3].max(b[1]));
}
extern "thiscall" fn advance_fake(this: *mut u8, out_x: *mut i32, out_z: *mut i32) -> u64 {
    let r = unsafe { RectangleIterator::from_ptr(this) };
    assert_eq!(r.repeat(), 0);
    if r.state() != 0 {
        let mut x = r.x().wrapping_add(r.step());
        let mut z = r.z();
        if x > r.x2() {
            if z >= r.z2() {
                STATE.with(|s| s.borrow_mut().events.push(Event::End));
                return 0x8765_4321_1234_5600;
            }
            x = r.x1();
            z = z.wrapping_add(1);
            r.set_step(if z < r.z2() {
                r.x2().wrapping_sub(r.x1()).max(1)
            } else {
                1
            });
        }
        r.set_x(x);
        r.set_z(z);
    } else {
        r.set_state(1);
    }
    STATE.with(|s| s.borrow_mut().events.push(Event::Advance(r.x(), r.z())));
    unsafe {
        out_x.write(r.x());
        out_z.write(r.z());
    }
    0x1234_5678_0000_0001
}
extern "thiscall" fn anchor_fake(this: *mut u8, x: *mut i32, z: *mut i32) -> u64 {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Anchor(id));
        unsafe {
            x.write(s.lots[id as usize].anchor[0]);
            z.write(s.lots[id as usize].anchor[1]);
        }
    });
    0x1122_3344_0000_0000
}
extern "thiscall" fn bounds_fake(this: *mut u8, out: *mut i32) {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Bounds(id));
        unsafe {
            out.cast::<[i32; 4]>().write(s.lots[id as usize].bounds);
        }
    });
}
extern "thiscall" fn addref_fake(this: *mut u8) {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::AddRef(id));
        s.lots[id as usize].refs += 1;
    });
}
extern "thiscall" fn release_fake(this: *mut u8) -> u64 {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Release(id));
        s.lots[id as usize].refs -= 1;
    });
    0xDEAD_BEEF_7654_3210
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        node(&mut s)
    })
}
extern "thiscall" fn append_fake(out: *mut *mut u8, value: *const *mut u8) {
    let lot = unsafe { value.read() };
    let id = id(lot);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Append(id));
        s.lots[id as usize].refs += 1;
        let n = node(&mut s);
        unsafe { link(out.read(), n, lot) };
    });
}

struct Fixture {
    manager: Object,
    _lots: Vec<Object>,
    table: Box<[usize]>,
    columns: Vec<Vec<*mut u8>>,
    rows: Vec<[u32; 3]>,
    header: Box<[u32; 3]>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut table = vec![0usize; 0x58 / 4].into_boxed_slice();
        for (slot, f) in [
            (4, addref_fake as *const () as usize),
            (8, release_fake as *const () as usize),
            (0x38, bounds_fake as *const () as usize),
            (0x54, anchor_fake as *const () as usize),
        ] {
            table[slot / 4] = f;
        }
        let mut columns = vec![vec![null_mut(); 8]; 8];
        let mut rows = columns
            .iter_mut()
            .map(|c| [c.as_mut_ptr() as u32, 0, 0])
            .collect::<Vec<_>>();
        let mut manager = Object::new(0x70);
        let m = unsafe { LotManager::from_ptr(manager.ptr()) };
        m.set_cells_x(8);
        m.set_cells_z(8);
        m.set_rows(rows.as_mut_ptr().cast());
        let mut header = Box::new([0u32, 0, 0xA5A5_A5A5]);
        let p = header.as_mut_ptr() as u32;
        header[0] = p;
        header[1] = p;
        let fakes = vec![
            fake(0x0047_3EC0, clear_fake as *const () as usize),
            fake(0x0060_CE90, constructor_fake as *const () as usize),
            fake(0x0060_C890, iterator_bounds_fake as *const () as usize),
            fake(0x0060_CEE0, advance_fake as *const () as usize),
            fake(0x007E_98C0, append_fake as *const () as usize),
            fake(0x0090_CF54, allocate_fake as *const () as usize),
        ];
        Self {
            manager,
            _lots: vec![],
            table,
            columns,
            rows,
            header,
            _fakes: fakes,
        }
    }
    fn lot(&mut self, anchor: [i32; 2], bounds: [i32; 4], cells: &[(usize, usize)]) -> *mut u8 {
        let mut lot = Object::new(8);
        let p = lot.ptr();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            unsafe {
                p.cast::<usize>().write(self.table.as_ptr() as usize);
                p.add(4).cast::<u32>().write(s.lots.len() as u32)
            };
            s.lots.push(Lot {
                anchor,
                bounds,
                refs: 0,
            });
        });
        for &(x, z) in cells {
            self.columns[x][z] = p;
        }
        self._lots.push(lot);
        p
    }
    fn list(&mut self) -> Vec<u32> {
        let header = self.header.as_mut_ptr().cast::<u8>();
        let mut p = unsafe { header.cast::<*mut u8>().read() };
        let mut previous = header;
        let mut result = vec![];
        while p != header {
            assert_eq!(unsafe { p.add(4).cast::<*mut u8>().read() }, previous);
            result.push(id(unsafe { p.add(8).cast::<*mut u8>().read() }));
            previous = p;
            p = unsafe { p.cast::<*mut u8>().read() };
        }
        assert_eq!(unsafe { header.add(4).cast::<*mut u8>().read() }, previous);
        assert_eq!(self.header[2], 0xA5A5_A5A5);
        result
    }
    fn inside(&mut self, bounds: [i32; 4], origin: u32) -> u64 {
        let mut list = self.header.as_mut_ptr().cast::<u8>();
        unsafe {
            fc4p_0072e080(
                self.manager.ptr(),
                0xFACE_CAFE,
                &mut list,
                bounds[0],
                bounds[1],
                bounds[2],
                bounds[3],
                origin,
            )
        }
    }
    fn surrounding(&mut self, bounds: [i32; 4], flags: u32) -> u64 {
        let mut list = self.header.as_mut_ptr().cast::<u8>();
        unsafe { fc4p_0072e2b0(self.manager.ptr(), bounds.as_ptr(), &mut list, flags) }
    }
    fn existing(&mut self, lot: *mut u8) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.lots[id(lot) as usize].refs += 1;
            let p = node(&mut s);
            unsafe { link(self.header.as_mut_ptr().cast(), p, lot) };
        });
    }
}

#[test]
fn inside_rectangle_matches_the_single_and_two_by_two_lot_fixture_in_both_modes() {
    let mut f = Fixture::new();
    f.lot([2, 2], [2, 2, 2, 2], &[(2, 2)]);
    f.lot([3, 2], [3, 2, 4, 3], &[(3, 2), (3, 3), (4, 2), (4, 3)]);
    for origins in [0u32, 1] {
        let result = f.inside([2, 2, 4, 3], origins);
        assert_eq!(result as u32, 2);
        assert_eq!(f.list(), vec![0, 1]);
        assert_eq!(
            (result >> 32) as u32,
            if origins == 0 {
                (unsafe { f.rows.as_mut_ptr().cast::<u8>().add(36) }) as u32
            } else {
                0x1122_3344
            }
        );
        STATE.with(|s| assert_eq!(s.borrow().lots.iter().map(|l| l.refs).collect::<Vec<_>>(), vec![1, 1]));
        let log = events();
        assert_eq!(log[0], Event::Clear);
        if origins == 0 {
            assert_eq!(
                &log[1..],
                &[
                    Event::AddRef(0),
                    Event::Allocate(12),
                    Event::AddRef(0),
                    Event::Release(0),
                    Event::AddRef(1),
                    Event::Allocate(12),
                    Event::AddRef(1),
                    Event::Release(1)
                ]
            );
        } else {
            assert_eq!(
                &log[1..],
                &[
                    Event::ConstructIterator,
                    Event::IteratorBounds([2, 2, 4, 3]),
                    Event::Anchor(0),
                    Event::AddRef(0),
                    Event::Append(0),
                    Event::Release(0),
                    Event::Anchor(1),
                    Event::AddRef(1),
                    Event::Append(1),
                    Event::Release(1),
                    Event::Anchor(1),
                    Event::Anchor(1),
                    Event::Anchor(1)
                ]
            );
        }
    }
}

#[test]
fn inside_uses_adjacent_cell_suppression_rather_than_global_uniqueness_and_clips_before_scanning() {
    let mut f = Fixture::new();
    f.lot([1, 1], [1, 1, 3, 3], &[(1, 1), (1, 3), (3, 1)]);
    assert_eq!(f.inside([-4, -2, 99, 99], 0) as u32, 3);
    assert_eq!(f.list(), vec![0, 0, 0]);
    events();
    assert_eq!(f.inside([0, 0, 7, 7], 1) as u32, 1);
    assert_eq!(f.list(), vec![0]);
    events();
    assert_eq!(f.inside([5, 0, 4, 0], 1), 0xAABB_CCDD_0000_0000);
    assert!(f.list().is_empty());
    assert_eq!(events(), vec![Event::Clear]);
    assert_eq!(f.inside([0, 2, 1, 1], 0), 0xAABB_CCDD_0000_0000);
    assert_eq!(events(), vec![Event::Clear]);
    unsafe { LotManager::from_ptr(f.manager.ptr()) }.set_cells_x(0);
    assert_eq!(f.inside([0, 0, 0, 0], 0), 0xAABB_CCDD_0000_0000);
    assert_eq!(events(), vec![Event::Clear]);
}

#[test]
fn low_origin_byte_selects_the_scan_order_and_an_origin_match_keeps_release_edx() {
    let mut f = Fixture::new();
    f.lot([1, 2], [1, 2, 1, 2], &[(1, 2)]);
    f.lot([2, 1], [2, 1, 2, 1], &[(2, 1)]);
    assert_eq!(f.inside([1, 1, 2, 2], 0x100) as u32, 2);
    assert_eq!(f.list(), vec![0, 1]);
    events();
    assert_eq!(f.inside([1, 1, 2, 2], 0x101) as u32, 2);
    assert_eq!(f.list(), vec![1, 0]);
    events();
    assert_eq!(f.inside([1, 2, 1, 2], 1), 0xDEAD_BEEF_0000_0001);
    assert_eq!(f.list(), vec![0]);
}

#[test]
fn surrounding_matches_side_masks_and_checks_existing_output_and_overlap_before_append() {
    for flags in [15u32, 2, 8] {
        let mut f = Fixture::new();
        f.lot([2, 2], [2, 2, 2, 2], &[(2, 2)]);
        f.lot([3, 2], [3, 2, 4, 3], &[(3, 2), (3, 3), (4, 2), (4, 3)]);
        assert_eq!(
            f.surrounding([3, 2, 4, 3], flags),
            0x8765_4321_0000_0000 | u64::from(flags != 2)
        );
        assert_eq!(f.list(), if flags == 2 { vec![] } else { vec![0] });
    }
    let mut f = Fixture::new();
    let existing = f.lot([2, 2], [2, 2, 2, 3], &[(2, 2), (2, 3)]);
    f.existing(existing);
    f.lot([3, 1], [3, 1, 3, 2], &[(3, 1)]);
    assert_eq!(f.surrounding([3, 2, 4, 3], 15), 0x8765_4321_0000_0000);
    assert_eq!(f.list(), vec![0]);
    let log = events();
    assert_eq!(log.iter().filter(|e| matches!(e, Event::AddRef(0))).count(), 2);
    assert_eq!(log.iter().filter(|e| matches!(e, Event::Release(0))).count(), 2);
    assert!(!log.iter().any(|e| matches!(e, Event::Allocate(_))));
}

#[test]
fn every_perimeter_corner_requires_both_side_bits_and_duplicate_lots_are_retained_only_once() {
    for flags in 0u32..32 {
        let mut f = Fixture::new();
        let mut ring = vec![];
        for z in 2..=5 {
            for x in 2..=5 {
                if z == 2 || z == 5 || x == 2 || x == 5 {
                    ring.push((x, z));
                    f.lot(
                        [x as i32, z as i32],
                        [x as i32, z as i32, x as i32, z as i32],
                        &[(x, z)],
                    );
                }
            }
        }
        let expected = ring
            .iter()
            .enumerate()
            .filter(|&(_, &(x, z))| {
                (z != 2 || flags & 1 != 0)
                    && (x != 5 || flags & 2 != 0)
                    && (z != 5 || flags & 4 != 0)
                    && (x != 2 || flags & 8 != 0)
            })
            .map(|(i, _)| i as u32)
            .collect::<Vec<_>>();
        assert_eq!(
            f.surrounding([3, 3, 4, 4], flags),
            0x8765_4321_0000_0000 | expected.len() as u64
        );
        assert_eq!(f.list(), expected);
        let positions = events()
            .into_iter()
            .filter_map(|e| {
                if let Event::Advance(x, z) = e {
                    Some((x as usize, z as usize))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(positions, ring);
    }
    let mut f = Fixture::new();
    f.lot([2, 2], [2, 2, 2, 5], &[(2, 2), (2, 3), (2, 4), (2, 5)]);
    assert_eq!(f.surrounding([3, 3, 4, 4], 15), 0x8765_4321_0000_0001);
    assert_eq!(f.list(), vec![0]);
    STATE.with(|s| assert_eq!(s.borrow().lots[0].refs, 1));
}

#[test]
fn surrounding_city_edge_clipping_disables_the_outside_sides_and_preserves_iterator_edx() {
    let mut f = Fixture::new();
    f.lot([0, 2], [0, 2, 0, 2], &[(0, 2)]);
    f.lot([2, 0], [2, 0, 2, 0], &[(2, 0)]);
    f.lot([2, 1], [2, 1, 2, 1], &[(2, 1)]);
    assert_eq!(f.surrounding([0, 0, 1, 1], 15), 0x8765_4321_0000_0003);
    assert_eq!(f.list(), vec![1, 2, 0]);
    assert_eq!(
        events()
            .into_iter()
            .filter_map(|e| if let Event::Bounds(id) = e { Some(id) } else { None })
            .collect::<Vec<_>>(),
        vec![1, 2, 0]
    );
}

#[test]
fn ordinary_scan_counts_columns_even_when_a_negative_city_size_clamps_to_int_max() {
    let mut f = Fixture::new();
    f.lot([0, 0], [0, 0, 0, 0], &[(0, 0)]);
    let manager = unsafe { LotManager::from_ptr(f.manager.ptr()) };
    manager.set_cells_x(i32::MIN as u32);
    // Multiplying MAX by the twelve-byte row stride wraps to -12.
    manager.set_rows(unsafe { f.rows.as_mut_ptr().cast::<u8>().add(12) });
    assert_eq!(f.inside([i32::MAX, 0, i32::MIN, 0], 0), 1);
    assert_eq!(f.list(), vec![0]);
    assert_eq!(
        events(),
        vec![
            Event::Clear,
            Event::AddRef(0),
            Event::Allocate(12),
            Event::AddRef(0),
            Event::Release(0)
        ]
    );
}

extern "fastcall" fn clear_preserving_edx(list: *mut *mut u8, edx: u32) -> u64 {
    clear_fake(list);
    u64::from(edx) << 32
}

#[test]
fn clipped_empty_rectangle_retains_incoming_edx_through_an_empty_list_clear() {
    let mut f = Fixture::new();
    let _clear = fake(0x0047_3EC0, clear_preserving_edx as *const () as usize);
    let mut list = f.header.as_mut_ptr().cast::<u8>();
    let args = [&mut list as *mut *mut u8 as u32, 4, 0, 3, 0, 0];
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        fc4p_0072e080 as *const () as usize,
        f.manager.ptr() as u32,
        &args,
    );
    call.edx = 0xDEAD_BEEF;
    unsafe { crate::abi::call_raw(&mut call) };
    assert_eq!((call.eax, call.edx), (0, 0xDEAD_BEEF));
    assert_eq!(events(), vec![Event::Clear]);
}
