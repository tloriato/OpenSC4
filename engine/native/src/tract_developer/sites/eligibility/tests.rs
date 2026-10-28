// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Expanded lot eligibility: branch outcomes and ordered calls at the original object offsets.

use super::*;
use crate::testing::{fake, Fake, Object};
use crate::tract_developer::testing::snapshot;
use std::cell::RefCell;

struct Settings {
    calls: Vec<(&'static str, Vec<u32>)>,
    fits: u32,
    live: u32,
    age: u32,
    range: [u8; 2],
    land: u32,
    state: u32,
    area: u32,
    occupied: u32,
    building: u32,
    resident: u32,
    historical: u32,
    configuration: u32,
    stage: u32,
    purpose: u32,
    access: u32,
    grid: u32,
    cell: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            calls: vec![],
            fits: 1,
            live: 0,
            age: 180,
            range: [0, 0],
            land: 50,
            state: 0,
            area: 7,
            occupied: 0,
            building: 0x300,
            resident: 0,
            historical: 0,
            configuration: 0,
            stage: 3,
            purpose: 2,
            access: 1,
            grid: 0,
            cell: 0,
        }
    }
}
thread_local! { static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default()); }
fn record(name: &'static str, args: &[u32]) {
    SETTINGS.with(|s| s.borrow_mut().calls.push((name, args.to_vec())));
}
const HI: u64 = 0xDEAD_BEEF_0000_0000;
macro_rules! getter {
    ($name:ident, $field:ident) => {
        extern "thiscall" fn $name(_this: u32) -> u64 {
            record(stringify!($name), &[]);
            SETTINGS.with(|s| HI | u64::from(s.borrow().$field))
        }
    };
}
getter!(live, live);
getter!(age, age);
getter!(state, state);
getter!(occupied, occupied);
getter!(historical, historical);
getter!(configuration, configuration);
getter!(stage, stage);
getter!(purpose, purpose);
getter!(access, access);
getter!(grid, grid);
extern "thiscall" fn location(_this: u32, x: *mut u32, z: *mut u32) -> u64 {
    record("location", &[]);
    unsafe {
        *x = 10;
        *z = 20;
    }
    HI | 0x1234
}
extern "thiscall" fn construction(_this: u32, output: *mut u8) -> u64 {
    record("construction", &[]);
    unsafe {
        core::ptr::write_bytes(output, 0, 20);
        write_unaligned(output.cast::<u32>(), 1);
    }
    SETTINGS.with(|s| unsafe {
        *output.add(16) = s.borrow().range[0];
        *output.add(17) = s.borrow().range[1];
    });
    HI
}
extern "thiscall" fn zone(_this: u32, x: u32, z: u32) -> u64 {
    record("zone", &[x, z]);
    HI | 0x1234_00FE
}
extern "C" fn fits(target: u32, zone: u32) -> u64 {
    record("fits", &[target, zone]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().fits))
}
extern "thiscall" fn land(_this: u32, x: u32, z: u32) -> u64 {
    record("land", &[x, z]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().land))
}
extern "thiscall" fn rectangle(_this: u32, output: *mut [u32; 4]) -> u64 {
    record("rectangle", &[]);
    unsafe {
        *output = [9, 18, 11, 22];
    }
    HI
}
extern "thiscall" fn area(_this: u32, x: u32, z: u32, w: u32, h: u32, a: u32, b: u32, c: u32) -> u64 {
    record("area", &[x, z, w, h, a, b, c]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().area))
}
extern "thiscall" fn building(_this: u32, flag: u32) -> u64 {
    record("building", &[flag]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().building))
}
extern "thiscall" fn profile(_this: u32, building: u32) -> u64 {
    record("profile", &[building]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().resident))
}
extern "thiscall" fn organic(_this: u32, purpose: u32) -> u64 {
    record("organic", &[purpose]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().access))
}
extern "thiscall" fn cell(_this: u32, x: u32, z: u32) -> u64 {
    record("cell", &[x, z]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().access))
}
extern "thiscall" fn cache(_this: u32, key: *const u32) -> u64 {
    record("cache", &[unsafe { *key }]);
    SETTINGS.with(|s| HI | u64::from(s.borrow().cell))
}
struct World {
    td: Object,
    context: Object,
    objects: Box<[u32; 8]>,
    tables: [Box<[usize; 64]>; 8],
    resident: Box<[u32; 2]>,
    histogram: Box<[u32; 28]>,
    value: Box<u32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        SETTINGS.with(|s| *s.borrow_mut() = Settings::default());
        let mut w = Self {
            td: Object::new(0x180),
            context: Object::new(0xA0),
            objects: Box::new([0; 8]),
            tables: core::array::from_fn(|_| Box::new([0; 64])),
            resident: Box::new([2, 2]),
            histogram: Box::new([u32::MAX; 28]),
            value: Box::new(0xCCCC_CCCC),
            _fakes: vec![],
        };
        for (object, offset, f) in [
            (0, 0x54, location as *const () as usize),
            (0, 0x94, live as *const () as usize),
            (0, 0xA0, age as *const () as usize),
            (0, 0x84, state as *const () as usize),
            (0, 0x38, rectangle as *const () as usize),
            (0, 0x98, occupied as *const () as usize),
            (0, 0xB8, building as *const () as usize),
            (0, 0xAC, historical as *const () as usize),
            (0, 0x24, configuration as *const () as usize),
            (0, 0x70, access as *const () as usize),
            (0, 0x74, organic as *const () as usize),
            (1, 0x24, construction as *const () as usize),
            (1, 0x1C, purpose as *const () as usize),
            (2, 0x30, zone as *const () as usize),
            (3, 0x18, land as *const () as usize),
            (4, 0x50, area as *const () as usize),
            (5, 0x9C, profile as *const () as usize),
            (6, 0x2C, stage as *const () as usize),
            (7, 0x34, grid as *const () as usize),
            (6, 0x1C, cell as *const () as usize),
        ] {
            w.tables[object][offset / 4] = f;
        }
        for i in 0..8 {
            w.objects[i] = w.tables[i].as_ptr() as u32;
        }
        let td = w.td.ptr() as u32;
        for (offset, object) in [(0x80, 1), (0x3C, 2), (0x20, 3), (0x2C, 4), (0x1C, 5), (0x38, 7)] {
            unsafe {
                store(td, offset, w.object(object));
            }
        }
        unsafe {
            store(td, 0x170, w.context.ptr() as u32);
            store(w.context.ptr() as u32, 0x60, w.histogram.as_ptr() as u32);
        }
        let view = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        view.set_wealth(2);
        view.set_purpose(2);
        view.set_maximum_stage(4);
        SETTINGS.with(|s| {
            let s = &mut *s.borrow_mut();
            s.resident = w.resident.as_ptr() as u32;
            s.grid = w.object(6);
            s.cell = w.value.as_mut() as *mut u32 as u32;
        });
        w._fakes.push(fake(0x0069_5390, fits as *const () as usize));
        w._fakes.push(fake(0x0049_7A00, cache as *const () as usize));
        w
    }
    fn object(&self, i: usize) -> u32 {
        self.objects.as_ptr().wrapping_add(i) as u32
    }
    fn check(&mut self, existing: bool, expected: u32) -> Vec<(&'static str, Vec<u32>)> {
        let td = self.td.ptr() as u32;
        let before = snapshot(self.td.ptr(), 0x180);
        let mut key = self.object(0);
        let mut r = Entry {
            eax: 0x1122_3344,
            ecx: td,
            edx: 0x5566_7788,
            return_address: 0,
            args: [],
        };
        assert_eq!(
            unsafe { super::check(&mut r, td, key, &mut key as *mut u32 as u32, existing) },
            expected
        );
        assert_eq!(*self.value, expected);
        assert_eq!(r.eax, self.histogram.as_ptr() as u32 + 4 * expected);
        let mut histogram = [u32::MAX; 28];
        histogram[expected as usize] = 0;
        assert_eq!(*self.histogram, histogram);
        let mut after = snapshot(self.td.ptr(), 0x180);
        if expected == 22 {
            assert_eq!(unsafe { word(td, 0xF8) }, SETTINGS.with(|s| s.borrow().land & 0xFF));
            after[0xF8..0xFC].copy_from_slice(&before[0xF8..0xFC]);
        }
        assert_eq!(after, before);
        SETTINGS.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn expanded_checks_use_signed_zone_byte_and_the_original_area_dimensions() {
    for existing in [false, true] {
        let mut w = World::new();
        let calls = w.check(existing, 27);
        assert_eq!(
            calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            [
                "location",
                "construction",
                "zone",
                "fits",
                "live",
                "state",
                "rectangle",
                "area",
                "occupied",
                "purpose",
                "access",
                "cache"
            ]
        );
        assert!(calls.contains(&("fits", vec![1, 0xFFFF_FFFE])));
        assert!(calls.contains(&("area", vec![9, 18, 14, 5, 0, 0, 0])));
    }
}

#[test]
fn expanded_zone_age_land_state_and_area_rejections_follow_the_first_failure() {
    for existing in [false, true] {
        for (case, code, last) in [
            (0, 6, "fits"),
            (1, 19, "age"),
            (2, 22, "land"),
            (3, 4, "state"),
            (4, 0, "area"),
            (5, 4, "area"),
            (6, 6, "area"),
            (7, 3, "area"),
        ] {
            let mut w = World::new();
            SETTINGS.with(|s| {
                let s = &mut *s.borrow_mut();
                match case {
                    0 => s.fits = 0,
                    1 => {
                        s.live = 1;
                        s.age = 179;
                    }
                    2 => {
                        s.range = [51, 60];
                        s.land = 0x1234_0032;
                    }
                    3 => s.state = 2,
                    _ => s.area = case - 4,
                }
            });
            let calls = w.check(existing, code);
            assert_eq!(calls[calls.len() - 2].0, last);
            assert_eq!(calls.last().unwrap().0, "cache");
        }
    }
}

#[test]
fn expanded_occupant_checks_reject_missing_buildings_protected_purposes_wealth_history_and_stage() {
    for existing in [false, true] {
        for (case, code, last) in [
            (0, 4, "building"),
            (1, 15, "profile"),
            (2, 15, "profile"),
            (3, 16, "profile"),
            (4, 16, "profile"),
            (5, 18, "historical"),
            (6, 17, "stage"),
        ] {
            let mut w = World::new();
            SETTINGS.with(|s| s.borrow_mut().occupied = 1);
            match case {
                0 => SETTINGS.with(|s| s.borrow_mut().building = 0),
                1 => w.resident[0] = 5,
                2 => {
                    w.resident[0] = 7;
                    unsafe {
                        store(w.td.ptr() as u32, 0x9C, 6);
                    }
                }
                3 => w.resident[1] = 3,
                4 => w.resident[0] = 3,
                5 => SETTINGS.with(|s| s.borrow_mut().historical = 1),
                _ => {
                    let cfg = w.object(6);
                    SETTINGS.with(|s| {
                        let s = &mut *s.borrow_mut();
                        s.configuration = cfg;
                        s.stage = 4;
                    });
                }
            }
            let calls = w.check(existing, code);
            assert_eq!(calls[calls.len() - 2].0, last);
        }
    }
}

#[test]
fn expanded_road_checks_select_grid_organic_and_general_access_and_keep_unsigned_age() {
    for existing in [false, true] {
        for (purpose, road) in [
            (1, "cell"),
            (2, "access"),
            (6, "organic"),
            (7, "organic"),
            (8, "organic"),
            (9, "access"),
        ] {
            for access in [0, 0x1234_0001] {
                let mut w = World::new();
                SETTINGS.with(|s| {
                    let s = &mut *s.borrow_mut();
                    s.purpose = purpose;
                    s.access = access;
                    s.live = 1;
                    s.age = u32::MAX;
                });
                let calls = w.check(existing, if access as u8 == 0 { 21 } else { 27 });
                assert_eq!(calls[calls.len() - 2].0, road);
                if purpose == 1 {
                    assert!(calls.contains(&("cell", vec![10, 20])));
                }
                if (6..=8).contains(&purpose) {
                    assert!(calls.contains(&("organic", vec![purpose])));
                }
            }
        }
    }
}
