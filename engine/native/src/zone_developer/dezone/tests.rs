// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Allocate(u32),
    ConstructMessage,
    AddRef,
    MessageId(u32),
    MessageValue(u32),
    Post(u32, u32),
    Release,
    ConstructBits([i32; 4], i32, i32, u8),
    Bounds(u32),
    Facing(u32),
    Zone(u32),
    Eligible(u32),
    Delete(u32),
    Remnants([i32; 4], u32, u32),
    Network(i32, i32, u32),
    Demolish([i32; 4], Vec<(i32, i32)>, [u32; 9]),
    DestroyBits(u32),
}
#[derive(Clone)]
struct Lot {
    bounds: [i32; 4],
    facing: u32,
    zone: u32,
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    memory: Vec<Box<[u32]>>,
    message: usize,
    dev: usize,
    rows: usize,
    lots: Vec<Lot>,
    streets: Vec<(i32, i32)>,
    protected: Vec<(i32, i32)>,
    reject_zone: u32,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}
fn words(s: &mut State, values: &[u32]) -> *mut u32 {
    let mut p = values.to_vec().into_boxed_slice();
    let out = p.as_mut_ptr();
    s.memory.push(p);
    out
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        s.message as *mut u8
    })
}
extern "thiscall" fn message_fake(this: *mut u8) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().events.push(Event::ConstructMessage));
    this
}
extern "thiscall" fn addref_fake(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::AddRef));
}
extern "thiscall" fn id_fake(_this: *mut u8, id: u32) {
    STATE.with(|s| s.borrow_mut().events.push(Event::MessageId(id)));
}
extern "thiscall" fn value_fake(this: *mut u8, value: u32) {
    unsafe { this.add(4).cast::<u32>().write(value) };
    STATE.with(|s| s.borrow_mut().events.push(Event::MessageValue(value)));
}
extern "thiscall" fn post_fake(_this: *mut u8, message: *mut u8, mask: u32) {
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::Post(unsafe { message.add(4).cast::<u32>().read() }, mask))
    });
}
extern "thiscall" fn release_fake(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Release));
}

extern "thiscall" fn bits_fake(this: *mut u8, width: i32, height: i32, fill: *const u8) {
    let r = unsafe { Region::from_ptr(this.sub(16)) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::ConstructBits(
            [r.x1(), r.z1(), r.x2(), r.z2()],
            width,
            height,
            unsafe { fill.read() },
        ));
        let mut columns = vec![];
        for _ in 0..width.max(0) {
            columns.push(words(&mut s, &vec![0; height.max(0) as usize / 32 + 1]) as u32);
        }
        r.set_vtable(0x00A8_6364);
        r.set_width(width);
        r.set_height(height);
        r.set_columns(words(&mut s, &columns).cast());
        r.set_words((height as u32).div_ceil(32));
    });
}
extern "thiscall" fn destroy_fake(this: *mut u8) -> u64 {
    let r = unsafe { Region::from_ptr(this.sub(16)) };
    STATE.with(|s| s.borrow_mut().events.push(Event::DestroyBits(r.vtable())));
    0xABCD_EF01_7654_3210
}
fn id(this: *mut u8) -> u32 {
    unsafe { this.add(4).cast::<u32>().read() }
}
extern "thiscall" fn bounds_fake(this: *mut u8, out: *mut i32) {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Bounds(id));
        unsafe { out.cast::<[i32; 4]>().write(s.lots[id as usize].bounds) };
    });
}
extern "thiscall" fn facing_fake(this: *mut u8) -> u32 {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Facing(id));
        s.lots[id as usize].facing
    })
}
extern "thiscall" fn zone_fake(this: *mut u8) -> u32 {
    let id = id(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Zone(id));
        s.lots[id as usize].zone
    })
}
extern "thiscall" fn eligible_fake(_this: *mut u8, zone: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Eligible(zone));
        assert_eq!(
            unsafe { ZoneDeveloper::from_ptr(s.dev as *mut u8) }.current_zone(),
            zone
        );
        if zone == s.reject_zone {
            0x100
        } else if (1..=9).contains(&zone) {
            0x101
        } else {
            0
        }
    })
}
extern "thiscall" fn delete_fake(_this: *mut u8, lot: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Delete(id(lot)));
        for x in 0..8 {
            let row = unsafe { (s.rows as *mut u8).add(x * 12).cast::<*mut *mut u8>().read() };
            for z in 0..8 {
                let p = unsafe { row.add(z) };
                if unsafe { p.read() } == lot {
                    unsafe { p.write(core::ptr::null_mut()) };
                }
            }
        }
    });
}
extern "thiscall" fn remnants_fake(this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: u32) {
    STATE.with(|s| {
        s.borrow_mut().events.push(Event::Remnants(
            [x, z, width, height],
            facing,
            unsafe { ZoneDeveloper::from_ptr(this) }.current_zone(),
        ))
    });
}
extern "thiscall" fn network_fake(_this: *mut u8, x: i32, z: i32, flag: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Network(x, z, flag));
        u32::from(if flag == 0x808 {
            s.streets.contains(&(x, z))
        } else {
            s.protected.contains(&(x, z))
        })
    })
}
extern "thiscall" fn demolish_fake(
    _this: *mut u8,
    a: u32,
    region: *mut u8,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    f: u32,
    g: u32,
    h: u32,
    i: u32,
    j: u32,
) {
    let r = unsafe { Region::from_ptr(region) };
    let mut cells = vec![];
    for z in r.z1()..=r.z2() {
        for x in r.x1()..=r.x2() {
            if unsafe { r.includes(x, z) } {
                cells.push((x, z));
            }
        }
    }
    STATE.with(|s| {
        s.borrow_mut().events.push(Event::Demolish(
            [r.x1(), r.z1(), r.x2(), r.z2()],
            cells,
            [a, b, c, d, e, f, g, h, i],
        ))
    });
    assert_eq!(j, 0);
}

struct Fixture {
    dev: Object,
    source: Object,
    _objects: Vec<Object>,
    tables: Vec<Box<[usize]>>,
    columns: Vec<Vec<*mut u8>>,
    rows: Vec<[u32; 3]>,
    _source_columns: Vec<Box<[u32]>>,
    _source_rows: Box<[*mut u32]>,
    _global: Box<usize>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new(bounds: [i32; 4], cells: &[(i32, i32)]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let slots = [
            vec![(0x20, network_fake as *const () as usize)],
            vec![(0x60, delete_fake as *const () as usize)],
            vec![
                (4, addref_fake as *const () as usize),
                (8, release_fake as *const () as usize),
                (0x14, id_fake as *const () as usize),
                (0x2C, value_fake as *const () as usize),
            ],
            vec![(0x10, post_fake as *const () as usize)],
            vec![(0x34, eligible_fake as *const () as usize)],
            vec![(0x18, demolish_fake as *const () as usize)],
            vec![
                (0x38, bounds_fake as *const () as usize),
                (0x5C, facing_fake as *const () as usize),
                (0x7C, zone_fake as *const () as usize),
            ],
        ];
        let mut tables = vec![];
        let mut objects = vec![];
        for slots in slots {
            let mut table = vec![0usize; 0x80 / 4].into_boxed_slice();
            for (slot, f) in slots {
                table[slot / 4] = f;
            }
            let mut object = Object::new(8);
            unsafe { object.ptr().cast::<usize>().write(table.as_ptr() as usize) };
            tables.push(table);
            objects.push(object);
        }
        let mut dev = Object::new(0xF0);
        let view = unsafe { ZoneDeveloper::from_ptr(dev.ptr()) };
        view.set_primary_vtable(tables[0].as_ptr() as u32);
        view.set_lot_manager_vtable(tables[1].as_ptr() as u32);
        view.set_message_server(objects[3].ptr());
        view.set_zone_manager(objects[4].ptr());
        view.set_current_zone(0xCAFE_BABE);
        let mut source = Object::new(0x24);
        let r = unsafe { Region::from_ptr(source.ptr()) };
        r.set_x1(bounds[0]);
        r.set_z1(bounds[1]);
        r.set_x2(bounds[2]);
        r.set_z2(bounds[3]);
        let width = bounds[2] - bounds[0] + 1;
        let height = bounds[3] - bounds[1] + 1;
        let mut source_columns = vec![];
        for _ in 0..width {
            source_columns.push(vec![0u32; height as usize / 32 + 1].into_boxed_slice());
        }
        let mut source_rows = source_columns
            .iter_mut()
            .map(|p| p.as_mut_ptr())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        r.set_width(width);
        r.set_height(height);
        r.set_columns(source_rows.as_mut_ptr());
        for &(x, z) in cells {
            unsafe { r.set((x - bounds[0]) as u32, (z - bounds[1]) as u32, true) };
        }
        let columns = vec![vec![core::ptr::null_mut(); 8]; 8];
        let rows = vec![[0u32; 3]; 8];
        let mut global = Box::new(objects[5].ptr() as usize);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.message = objects[2].ptr() as usize;
            s.dev = dev.ptr() as usize;
            s.reject_zone = u32::MAX;
        });
        let fakes = vec![
            fake(0x0091_33DA, allocate_fake as *const () as usize),
            fake(0x0091_34D6, message_fake as *const () as usize),
            fake(0x0044_22A0, bits_fake as *const () as usize),
            fake(0x0044_1A60, destroy_fake as *const () as usize),
            fake(0x0072_D440, remnants_fake as *const () as usize),
            fake_data(0x00B4_3D00, (&mut *global as *mut usize).cast()),
        ];
        let mut result = Self {
            dev,
            source,
            _objects: objects,
            tables,
            columns,
            rows,
            _source_columns: source_columns,
            _source_rows: source_rows,
            _global: global,
            _fakes: fakes,
        };
        for (r, c) in result.rows.iter_mut().zip(result.columns.iter_mut()) {
            r[0] = c.as_mut_ptr() as u32;
        }
        unsafe {
            result
                .dev
                .ptr()
                .add(0x54)
                .cast::<*mut u8>()
                .write(result.rows.as_mut_ptr().cast())
        };
        STATE.with(|s| s.borrow_mut().rows = result.rows.as_mut_ptr() as usize);
        result
    }
    fn lot(&mut self, info: Lot, cells: &[(usize, usize)]) {
        let mut object = Object::new(8);
        let p = object.ptr();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            unsafe {
                p.cast::<usize>().write(self.tables[6].as_ptr() as usize);
                p.add(4).cast::<u32>().write(s.lots.len() as u32)
            };
            s.lots.push(info);
        });
        for &(x, z) in cells {
            self.columns[x][z] = p;
        }
        self._objects.push(object);
    }
    fn call(&mut self, flag: u32) -> Vec<Event> {
        assert_eq!(
            unsafe { fc4p_0072e6a0(self.dev.ptr(), self.source.ptr(), flag) },
            0xABCD_EF01_7654_3210
        );
        events()
    }
}
fn message_events(value: u32) -> Vec<Event> {
    vec![
        Event::Allocate(0x2C),
        Event::ConstructMessage,
        Event::AddRef,
        Event::MessageId(0x49D9_68C4),
        Event::MessageValue(value),
        Event::Post(value, 0),
        Event::Release,
    ]
}

#[test]
fn dezone_deletes_a_shared_lot_once_restores_remnants_and_interleaves_network_queries() {
    let mut f = Fixture::new([1, 1, 2, 2], &[(1, 1), (2, 1), (1, 2)]);
    f.lot(
        Lot {
            bounds: [1, 1, 2, 1],
            facing: 3,
            zone: 4,
        },
        &[(1, 1), (2, 1)],
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.streets = vec![(1, 1), (1, 2)];
        s.protected = vec![(1, 2)];
    });
    let actual = f.call(1);
    let mut expected = message_events(1);
    expected.extend([
        Event::ConstructBits([1, 1, 2, 2], 2, 2, 0),
        Event::Bounds(0),
        Event::Facing(0),
        Event::Zone(0),
        Event::Eligible(4),
        Event::Delete(0),
        Event::Remnants([1, 1, 2, 1], 3, 4),
        Event::Network(1, 1, 0x808),
        Event::Network(1, 1, 0x1747),
        Event::Network(2, 1, 0x808),
        Event::Network(1, 2, 0x808),
        Event::Network(1, 2, 0x1747),
        Event::Demolish([1, 1, 2, 2], vec![(1, 1)], [1, 1, 1, 0, 0, 0, 0, 0, 0]),
    ]);
    expected.extend(message_events(0));
    expected.push(Event::DestroyBits(0x00A8_6364));
    assert_eq!(actual, expected);
    assert_eq!(unsafe { ZoneDeveloper::from_ptr(f.dev.ptr()) }.current_zone(), 0);
}

#[test]
fn dezone_consults_zone_eligibility_before_special_zones_and_clears_scratch_even_when_rejected() {
    for (zone, eligible) in [
        (4, true),
        (11, true),
        (12, true),
        (10, false),
        (0, false),
        (u32::MAX, false),
    ] {
        let mut f = Fixture::new([2, 3, 2, 3], &[(2, 3)]);
        f.lot(
            Lot {
                bounds: [2, 3, 2, 3],
                facing: 0xAABB_CCFF,
                zone,
            },
            &[(2, 3)],
        );
        let log = f.call(0x100);
        let mut expected = message_events(1);
        expected.extend([
            Event::ConstructBits([2, 3, 2, 3], 1, 1, 0),
            Event::Bounds(0),
            Event::Facing(0),
            Event::Zone(0),
            Event::Eligible(zone),
        ]);
        if eligible {
            expected.extend([Event::Delete(0), Event::Remnants([2, 3, 1, 1], 0xAABB_CCFF, zone)]);
        }
        expected.extend(message_events(0));
        expected.push(Event::DestroyBits(0x00A8_6364));
        assert_eq!(log, expected);
        assert_eq!(unsafe { ZoneDeveloper::from_ptr(f.dev.ptr()) }.current_zone(), 0);
    }
    let mut f = Fixture::new([2, 3, 2, 3], &[(2, 3)]);
    f.lot(
        Lot {
            bounds: [2, 3, 2, 3],
            facing: 0,
            zone: 4,
        },
        &[(2, 3)],
    );
    STATE.with(|s| s.borrow_mut().reject_zone = 4);
    assert!(!f.call(0).iter().any(|e| matches!(e, Event::Delete(_))));
}

#[test]
fn clear_cells_outside_city_bounds_are_skipped_and_empty_lot_cells_keep_current_zone() {
    let mut f = Fixture::new([-1, -1, 2, 2], &[(0, 0), (2, 0), (0, 2)]);
    let log = f.call(0xFF);
    let mut expected = message_events(1);
    expected.push(Event::ConstructBits([-1, -1, 2, 2], 4, 4, 0));
    expected.extend([
        Event::Network(0, 0, 0x808),
        Event::Network(2, 0, 0x808),
        Event::Network(0, 2, 0x808),
    ]);
    expected.extend(message_events(0));
    expected.push(Event::DestroyBits(0x00A8_6364));
    assert_eq!(log, expected);
    assert_eq!(
        unsafe { ZoneDeveloper::from_ptr(f.dev.ptr()) }.current_zone(),
        0xCAFE_BABE
    );
}
