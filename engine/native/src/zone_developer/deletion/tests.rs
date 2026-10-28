// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Bounds,
    Facing,
    DeletedCell(i32, i32),
    Configuration(usize),
    City,
    Building(usize, u32),
    BuildingOccupant(u32, u32),
    RemoveOccupant(usize, usize),
    ReleaseBuilding,
    RemoveList,
    Attached,
    Detach(usize),
    Shutdown,
    ReleaseLot,
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    bounds: [i32; 4],
    facing: u32,
    records: Vec<([i32; 2], Box<[u32; 3]>)>,
    manager: usize,
    lot: usize,
    building: usize,
    occupants: usize,
    new_occupants: usize,
    has_building: u32,
    attached: u32,
    rows: usize,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn bounds_fake(_this: *mut u8, out: *mut i32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Bounds);
        unsafe { out.cast::<[i32; 4]>().write(s.bounds) };
    });
}
extern "thiscall" fn facing_fake(_this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Facing);
        s.facing
    })
}
extern "thiscall" fn cell_fake(this: *mut u8, cell: *const i32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.manager + 0xE0);
        let cell = unsafe { cell.cast::<[i32; 2]>().read() };
        s.events.push(Event::DeletedCell(cell[0], cell[1]));
        let mut record = Box::new([0xA5A5_A5A5u32; 3]);
        let p = record.as_mut_ptr().cast();
        s.records.push((cell, record));
        p
    })
}
extern "thiscall" fn configuration_fake(_this: *mut u8, lot: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Configuration(lot as usize)));
}
extern "thiscall" fn city_fake(_this: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::City);
        s.occupants as *mut u8
    })
}
extern "thiscall" fn building_fake(_this: *mut u8, lot: *mut u8, out: *mut *mut u8, flag: u32) -> u32 {
    assert!(unsafe { out.read() }.is_null());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Building(lot as usize, flag));
        unsafe { out.write(s.building as *mut u8) };
        s.has_building
    })
}
extern "thiscall" fn occupant_fake(_this: *mut u8, a: u32, b: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::BuildingOccupant(a, b));
        if s.new_occupants != 0 {
            unsafe { DeletionManager::from_ptr(s.manager as *mut u8) }.set_occupants(s.new_occupants as *mut u8);
        }
    });
    0x1122_3344usize as *mut u8
}
extern "thiscall" fn remove_fake(this: *mut u8, occupant: *mut u8) {
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::RemoveOccupant(this as usize, occupant as usize))
    });
}
extern "thiscall" fn release_building_fake(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::ReleaseBuilding));
}
extern "thiscall" fn list_fake(this: *mut u8, lot: *const *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.manager + 0x5C);
        assert_eq!(unsafe { lot.read() } as usize, s.lot);
        let [x1, z1, x2, z2] = s.bounds;
        for x in x1..=x2.max(x1) {
            let row = unsafe { (s.rows as *mut u8).add(x as usize * 12).cast::<*const u32>().read() };
            for z in z1..=z2.max(z1) {
                assert_eq!(unsafe { row.add(z as usize).read() }, 0);
            }
        }
        s.events.push(Event::RemoveList);
    });
}
extern "thiscall" fn attached_fake(_this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Attached);
        s.attached
    })
}
extern "thiscall" fn detach_fake(_this: *mut u8, lot: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Detach(lot as usize)));
}
extern "thiscall" fn shutdown_fake(_this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().events.push(Event::Shutdown));
    0xAABB_CCDD
}
extern "thiscall" fn release_lot_fake(_this: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().events.push(Event::ReleaseLot));
    0xFEDC_BA98_7654_3210
}

struct Fixture {
    manager: Object,
    lot: Object,
    _objects: Vec<Object>,
    _tables: Vec<Box<[usize]>>,
    columns: Vec<Vec<u32>>,
    rows: Vec<[u32; 3]>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new(bounds: [i32; 4], real: u8) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let slots = [
            vec![
                (0x38, bounds_fake as *const () as usize),
                (0x5C, facing_fake as *const () as usize),
                (0x64, attached_fake as *const () as usize),
                (0x1C, shutdown_fake as *const () as usize),
                (8, release_lot_fake as *const () as usize),
            ],
            vec![(0x2C, configuration_fake as *const () as usize)],
            vec![(0x90, city_fake as *const () as usize)],
            vec![
                (0xA4, building_fake as *const () as usize),
                (0x54, remove_fake as *const () as usize),
                (0x84, detach_fake as *const () as usize),
            ],
            vec![
                (0x0C, occupant_fake as *const () as usize),
                (8, release_building_fake as *const () as usize),
            ],
            vec![],
        ];
        let mut objects = vec![];
        let mut tables = vec![];
        for slots in slots {
            let mut table = vec![0usize; 0xA8 / 4].into_boxed_slice();
            for (slot, f) in slots {
                table[slot / 4] = f;
            }
            let mut object = Object::new(8);
            unsafe { object.ptr().cast::<usize>().write(table.as_ptr() as usize) };
            objects.push(object);
            tables.push(table);
        }
        let mut lot = objects.remove(0);
        let mut manager = Object::new(0xF0);
        let view = unsafe { DeletionManager::from_ptr(manager.ptr()) };
        view.set_city(objects[1].ptr());
        view.set_occupants(objects[2].ptr());
        view.set_configuration(objects[0].ptr());
        view.set_real_mode(real);
        let mut columns = vec![vec![0xA5A5_A5A5u32; 8]; 8];
        let mut rows = columns
            .iter_mut()
            .map(|c| [c.as_mut_ptr() as u32, 0x1234_5678, 0x8765_4321])
            .collect::<Vec<_>>();
        view.set_rows(rows.as_mut_ptr().cast());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.bounds = bounds;
            s.facing = 0xABCD_EF03;
            s.manager = manager.ptr() as usize;
            s.lot = lot.ptr() as usize;
            s.building = objects[3].ptr() as usize;
            s.occupants = objects[2].ptr() as usize;
            s.rows = rows.as_mut_ptr() as usize;
        });
        let fakes = vec![
            fake(0x0072_F7D0, cell_fake as *const () as usize),
            fake(0x0072_B990, list_fake as *const () as usize),
        ];
        Self {
            manager,
            lot,
            _objects: objects,
            _tables: tables,
            columns,
            rows,
            _fakes: fakes,
        }
    }
}

#[test]
fn preview_records_anchor_and_full_facing_for_every_cell_in_z_major_order_without_real_side_effects() {
    for bounds in [[2, 3, 4, 4], [2, 3, 1, 1], [-2, -3, -1, -2]] {
        let mut f = Fixture::new(bounds, 0);
        let [x1, z1, x2, z2] = bounds;
        let result = unsafe { fc4p_0072f8e0(f.manager.ptr(), f.lot.ptr()) };
        assert_eq!(result, (u64::from(z1 as u32) << 32) | u64::from(z2.max(z1) as u32));
        let mut expected = vec![Event::Bounds, Event::Facing];
        for z in z1..=z2.max(z1) {
            for x in x1..=x2.max(x1) {
                expected.push(Event::DeletedCell(x, z));
            }
        }
        assert_eq!(events(), expected);
        STATE.with(|s| {
            for (_, record) in &s.borrow().records {
                assert_eq!(**record, [x1 as u32, z1 as u32, 0xABCD_EF03]);
            }
        });
        assert!(f.columns.iter().flatten().all(|&v| v == 0xA5A5_A5A5));
    }
}

#[test]
fn real_deletion_orders_services_clears_only_the_bounds_and_preserves_release_registers() {
    for has_building in [0u32, 0x100, 0x101] {
        for attached in [0u32, 0x100] {
            let mut f = Fixture::new([2, 3, 4, 4], 0xFF);
            let lot = f.lot.ptr() as usize;
            let occupants = f._objects[2].ptr() as usize;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.has_building = has_building;
                s.attached = attached;
            });
            assert_eq!(
                unsafe { fc4p_0072f8e0(f.manager.ptr(), f.lot.ptr()) },
                0xFEDC_BA98_7654_3210
            );
            let mut expected = vec![
                Event::Bounds,
                Event::Configuration(lot),
                Event::City,
                Event::Building(lot, 0),
            ];
            if has_building as u8 != 0 {
                expected.extend([
                    Event::BuildingOccupant(1, 0),
                    Event::RemoveOccupant(occupants, 0x1122_3344),
                    Event::ReleaseBuilding,
                ]);
            }
            expected.extend([Event::RemoveList, Event::Attached]);
            if attached != 0 {
                expected.extend([Event::City, Event::Detach(lot)]);
            }
            expected.extend([Event::Shutdown, Event::ReleaseLot]);
            assert_eq!(events(), expected);
            for x in 0..8 {
                for z in 0..8 {
                    assert_eq!(
                        f.columns[x][z],
                        if (2..=4).contains(&x) && (3..=4).contains(&z) {
                            0
                        } else {
                            0xA5A5_A5A5
                        }
                    );
                }
            }
            assert!(f.rows.iter().all(|r| r[1..] == [0x1234_5678, 0x8765_4321]));
        }
    }
}

#[test]
fn building_call_keeps_the_selected_method_but_reloads_its_destination_object() {
    let mut f = Fixture::new([1, 1, 1, 1], 1);
    let destination = f._objects[4].ptr() as usize;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.has_building = 1;
        s.new_occupants = destination;
    });
    unsafe { fc4p_0072f8e0(f.manager.ptr(), f.lot.ptr()) };
    assert!(events().contains(&Event::RemoveOccupant(destination, 0x1122_3344)));
}
