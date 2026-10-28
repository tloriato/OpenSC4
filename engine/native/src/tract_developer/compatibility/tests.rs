// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Core eligibility outcomes at the native layout, with boundary inputs and ordered callees.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::tract_developer::testing::{run, snapshot};
use std::cell::RefCell;

const DEV: usize = 0;
const BDS: usize = 1;
const LOT: usize = 2;
const ZONE: usize = 3;
const LAND: usize = 4;
const LOT_DEV: usize = 5;
const TRAFFIC: usize = 6;
const GRID: usize = 7;
const MANAGER: usize = 8;
const RESOURCE: usize = 9;
const CFG: usize = 10;

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, Vec<u32>);
struct Settings {
    services: [u32; 11],
    calls: Vec<Call>,
    info: u32,
    purpose: u32,
    wealth: u32,
    group_ok: u32,
    resource_found: u32,
    monopoly: u8,
    mode: u32,
    x: u32,
    z: u32,
    rectangle: [u32; 4],
    target_zone: u32,
    zone: u32,
    fits: u32,
    age_flag: u32,
    age: u32,
    land_min: u8,
    land_max: u8,
    land: u32,
    state: u32,
    legal: u32,
    occupied: u32,
    building: u32,
    historical: u32,
    configuration: bool,
    stage: u32,
    access: u32,
    header: u32,
    node: u32,
    cached: bool,
    cell: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            services: [0; 11],
            calls: Vec::new(),
            info: 0,
            purpose: 2,
            wealth: 2,
            group_ok: 0x1234_0001,
            resource_found: 0x1234_0001,
            monopoly: 0,
            mode: 0,
            x: 10,
            z: 20,
            rectangle: [9, 18, 11, 22],
            target_zone: 1,
            zone: 0x1234_00FE,
            fits: 0x1234_0001,
            age_flag: 0,
            age: 180,
            land_min: 0,
            land_max: 0,
            land: 0x1234_0040,
            state: 0,
            legal: 7,
            occupied: 0,
            building: 0x1234,
            historical: 0,
            configuration: false,
            stage: 0,
            access: 0x1234_0001,
            header: 0,
            node: 0,
            cached: false,
            cell: 0,
        }
    }
}
thread_local! { static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default()); }
fn call(this: u32, service: usize, name: &'static str, args: &[u32], value: impl FnOnce(&mut Settings) -> u32) -> u64 {
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.services[service], "{name} this");
        s.calls.push(Call(name, args.to_vec()));
        0xBADC_0FFE_0000_0000 | u64::from(value(s))
    })
}
extern "thiscall" fn building_info(this: u32, id: u32) -> u64 {
    call(this, BDS, "info", &[id], |s| s.info)
}
extern "thiscall" fn purpose(this: u32) -> u64 {
    call(this, DEV, "purpose", &[], |s| s.purpose)
}
extern "thiscall" fn wealth(this: u32) -> u64 {
    call(this, DEV, "wealth", &[], |s| s.wealth)
}
extern "thiscall" fn key(this: u32, id: u32, output: u32) -> u64 {
    unsafe {
        assert_eq!([word(output, 0), word(output, 4), word(output, 8)], [0; 3]);
        store(output, 0, 0x1111);
        store(output, 4, 0x2222);
        store(output, 8, id);
    }
    call(this, BDS, "key", &[id], |_| 0)
}
extern "C" fn group(key: u32, group: u32) -> u64 {
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        let key = unsafe { [word(key, 0), word(key, 4), word(key, 8)] };
        s.calls.push(Call("group", [key.as_slice(), &[group]].concat()));
        0xBADC_0FFE_0000_0000 | u64::from(s.group_ok)
    })
}
extern "thiscall" fn resource(this: u32, key: u32, iid: u32, output: u32, a: u32, b: u32) -> u64 {
    let key = unsafe { [word(key, 0), word(key, 4), word(key, 8), iid, a, b] };
    call(this, MANAGER, "resource", &key, |s| {
        unsafe {
            store(output, 0, s.services[RESOURCE]);
        }
        s.resource_found
    })
}
extern "thiscall" fn holder(this: u32) -> u64 {
    call(this, RESOURCE, "holder", &[], |_| 0x1122_3344)
}
extern "C" fn property(holder: u32, id: u32, output: u32) -> u64 {
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("property", vec![holder, id]));
        unsafe {
            *(output as *mut u8) = s.monopoly;
        }
        0xBADC_0FFE_1234_5678
    })
}
extern "thiscall" fn release(this: u32) -> u64 {
    call(this, RESOURCE, "release", &[], |_| 0)
}
extern "thiscall" fn mode(this: u32) -> u64 {
    call(this, DEV, "mode", &[], |s| s.mode)
}
extern "thiscall" fn location(this: u32, x: u32, z: u32) -> u64 {
    call(this, LOT, "location", &[], |s| {
        unsafe {
            store(x, 0, s.x);
            store(z, 0, s.z);
        }
        0
    })
}
extern "thiscall" fn construction(this: u32, output: u32) -> u64 {
    call(this, DEV, "construction", &[], |s| {
        unsafe {
            store(output, 0, s.target_zone);
            *((output + 0x10) as *mut u8) = s.land_min;
            *((output + 0x11) as *mut u8) = s.land_max;
        }
        0
    })
}
extern "thiscall" fn zone(this: u32, x: u32, z: u32) -> u64 {
    call(this, ZONE, "zone", &[x, z], |s| s.zone)
}
extern "C" fn fits(target: u32, zone: i32) -> u64 {
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("fits", vec![target, zone as u32]));
        0xBADC_0FFE_0000_0000 | u64::from(s.fits)
    })
}
extern "thiscall" fn age_flag(this: u32) -> u64 {
    call(this, LOT, "age_flag", &[], |s| s.age_flag)
}
extern "thiscall" fn age(this: u32) -> u64 {
    call(this, LOT, "age", &[], |s| s.age)
}
extern "thiscall" fn land(this: u32, x: u32, z: u32) -> u64 {
    call(this, LAND, "land", &[x, z], |s| s.land)
}
extern "thiscall" fn state(this: u32) -> u64 {
    call(this, LOT, "state", &[], |s| s.state)
}
extern "thiscall" fn rectangle(this: u32, output: u32) -> u64 {
    call(this, LOT, "rectangle", &[], |s| {
        for i in 0..4 {
            unsafe {
                store(output, i * 4, s.rectangle[i as usize]);
            }
        }
        0
    })
}
extern "thiscall" fn legal(this: u32, x: u32, z: u32, width: u32, depth: u32, a: u32, b: u32, c: u32) -> u64 {
    call(this, LOT_DEV, "legal", &[x, z, width, depth, a, b, c], |s| s.legal)
}
extern "thiscall" fn occupied(this: u32) -> u64 {
    call(this, LOT, "occupied", &[], |s| s.occupied)
}
extern "thiscall" fn building(this: u32, flag: u32) -> u64 {
    call(this, LOT, "building", &[flag], |s| s.building)
}
extern "thiscall" fn historical(this: u32) -> u64 {
    call(this, LOT, "historical", &[], |s| s.historical)
}
extern "thiscall" fn configuration(this: u32) -> u64 {
    call(this, LOT, "configuration", &[], |s| {
        if s.configuration {
            s.services[CFG]
        } else {
            0
        }
    })
}
extern "thiscall" fn stage(this: u32) -> u64 {
    call(this, CFG, "stage", &[], |s| s.stage)
}
extern "thiscall" fn access(this: u32) -> u64 {
    call(this, LOT, "access", &[], |s| s.access)
}
extern "thiscall" fn purpose_access(this: u32, purpose: u32) -> u64 {
    call(this, LOT, "purpose_access", &[purpose], |s| s.access)
}
extern "thiscall" fn access_grid(this: u32) -> u64 {
    call(this, TRAFFIC, "access_grid", &[], |s| s.services[GRID])
}
extern "thiscall" fn access_cell(this: u32, x: u32, z: u32) -> u64 {
    call(this, GRID, "access_cell", &[x, z], |s| s.access)
}
extern "thiscall" fn find(_this: u32, output: u32, key: u32) -> u64 {
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("find", vec![unsafe { word(key, 0) }]));
        unsafe {
            store(output, 0, if s.cached { s.node } else { s.header });
        }
        0xBADC_0FFE_1234_5678
    })
}
extern "thiscall" fn cache(_this: u32, key: u32) -> u64 {
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call("cache", vec![unsafe { word(key, 0) }]));
        0xBADC_0FFE_0000_0000 | u64::from(s.cell)
    })
}

struct World {
    td: Object,
    context: Object,
    histograms: Box<[[u32; 28]; 2]>,
    services: Box<[u32; 11]>,
    tables: [Box<[usize; 128]>; 11],
    info: Box<[u32; 3]>,
    styles: Box<[u32; 2]>,
    manager: Box<u32>,
    header: Box<[u32; 6]>,
    cached_node: Box<[u32; 6]>,
    cell: Box<u32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        SETTINGS.with(|s| *s.borrow_mut() = Settings::default());
        let mut w = Self {
            td: Object::new(0x180),
            context: Object::new(0xA0),
            histograms: Box::new([[u32::MAX; 28]; 2]),
            services: Box::new([0; 11]),
            tables: core::array::from_fn(|_| Box::new([0; 128])),
            info: Box::new([2, 2, 40]),
            styles: Box::new([0x2000, 0x2001]),
            manager: Box::new(0),
            header: Box::new([0; 6]),
            cached_node: Box::new([0; 6]),
            cell: Box::new(0xDEAD_BEEF),
            _fakes: Vec::new(),
        };
        for (service, slot, target) in [
            (BDS, 0x9C, building_info as *const () as usize),
            (BDS, 0x38, key as *const () as usize),
            (DEV, 0x1C, purpose as *const () as usize),
            (DEV, 0x20, wealth as *const () as usize),
            (DEV, 0x64, mode as *const () as usize),
            (DEV, 0x24, construction as *const () as usize),
            (MANAGER, 0xC, resource as *const () as usize),
            (RESOURCE, 0x1C, holder as *const () as usize),
            (RESOURCE, 8, release as *const () as usize),
            (LOT, 0x54, location as *const () as usize),
            (ZONE, 0x30, zone as *const () as usize),
            (LOT, 0x94, age_flag as *const () as usize),
            (LOT, 0xA0, age as *const () as usize),
            (LAND, 0x18, land as *const () as usize),
            (LOT, 0x84, state as *const () as usize),
            (LOT, 0x38, rectangle as *const () as usize),
            (LOT_DEV, 0x50, legal as *const () as usize),
            (LOT, 0x98, occupied as *const () as usize),
            (LOT, 0xB8, building as *const () as usize),
            (LOT, 0xAC, historical as *const () as usize),
            (LOT, 0x24, configuration as *const () as usize),
            (CFG, 0x2C, stage as *const () as usize),
            (LOT, 0x70, access as *const () as usize),
            (LOT, 0x74, purpose_access as *const () as usize),
            (TRAFFIC, 0x34, access_grid as *const () as usize),
            (GRID, 0x1C, access_cell as *const () as usize),
        ] {
            w.tables[service][slot / 4] = target;
        }
        for i in 0..11 {
            w.services[i] = w.tables[i].as_ptr() as u32;
        }
        for (address, target) in [
            (0x005E_8F70, group as *const () as usize),
            (0x005F_D390, property as *const () as usize),
            (0x0069_5390, fits as *const () as usize),
            (0x0049_7520, find as *const () as usize),
            (0x0049_7A00, cache as *const () as usize),
        ] {
            w._fakes.push(fake(address, target));
        }
        *w.manager = w.service(MANAGER);
        w._fakes
            .push(fake_data(0x00B4_3CA8, (&mut *w.manager as *mut u32).cast()));
        SETTINGS.with(|s| {
            let s = &mut *s.borrow_mut();
            s.services = core::array::from_fn(|i| w.service(i));
            s.info = w.info.as_ptr() as u32;
            s.header = w.header.as_ptr() as u32;
            s.node = w.cached_node.as_ptr() as u32;
            s.cell = &*w.cell as *const u32 as u32;
        });
        let td = w.view();
        td.set_building_development(w.service(BDS));
        td.set_developer(w.service(DEV));
        td.set_debug_context(w.context.ptr() as u32);
        td.set_minimum_building_capacity(10);
        td.set_maximum_building_capacity(100);
        td.set_wealth(2);
        td.set_purpose(2);
        td.set_maximum_stage(4);
        unsafe {
            store(td.as_ptr() as u32, 0x3C, w.service(ZONE));
            store(td.as_ptr() as u32, 0x20, w.service(LAND));
            store(td.as_ptr() as u32, 0x2C, w.service(LOT_DEV));
            store(td.as_ptr() as u32, 0x38, w.service(TRAFFIC));
            store(td.as_ptr() as u32, 0x10C, w.header.as_ptr() as u32);
            store(w.context.ptr() as u32, 0x90, w.histograms[0].as_ptr() as u32);
            store(w.context.ptr() as u32, 0x60, w.histograms[1].as_ptr() as u32);
        }
        w
    }
    fn service(&self, i: usize) -> u32 {
        self.services.as_ptr().wrapping_add(i) as u32
    }
    fn view(&mut self) -> TractDeveloper {
        unsafe { TractDeveloper::from_ptr(self.td.ptr()) }
    }
    fn styles(&mut self) {
        let td = self.view();
        td.set_styles_begin(self.styles.as_ptr() as u32);
        td.set_styles_end(self.styles.as_ptr() as u32 + 8);
        td.set_style_index(1);
        td.set_single_style(0);
    }
    fn calls(&self) -> Vec<Call> {
        SETTINGS.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
    fn check(&mut self, building: bool, flag: u32, expected: u32, names: &[&str]) -> Vec<Call> {
        let td = self.td.ptr() as u32;
        let before = snapshot(td as *mut u8, 0x180);
        let args = if building {
            [0xABCD, 0xDEAD_BEEF]
        } else {
            [self.service(LOT), flag]
        };
        assert_eq!(
            run(
                if building { 0x0070_4E00 } else { 0x0070_7A60 },
                Conv::Thiscall,
                td,
                &args
            )
            .0,
            expected
        );
        let calls = self.calls();
        assert_eq!(calls.iter().map(|c| c.0).collect::<Vec<_>>(), names);
        let mut histograms = [[u32::MAX; 28]; 2];
        let hit = !building && flag as u8 != 0 && SETTINGS.with(|s| s.borrow().cached);
        if !hit {
            histograms[usize::from(!building)][expected as usize] = 0;
        }
        assert_eq!(*self.histograms, histograms);
        let mut after = snapshot(td as *mut u8, 0x180);
        if !building && expected == 22 {
            after[0xF8..0xFC].copy_from_slice(&before[0xF8..0xFC]);
        }
        assert_eq!(after, before);
        assert_eq!(
            *self.cell,
            if !building && flag as u8 != 0 && !hit {
                expected
            } else {
                0xDEAD_BEEF
            }
        );
        calls
    }
}

const BUILD_PASS: &[&str] = &[
    "info", "purpose", "wealth", "key", "resource", "holder", "property", "release", "mode",
];
const LOT_PASS: &[&str] = &[
    "location",
    "construction",
    "zone",
    "fits",
    "state",
    "rectangle",
    "legal",
    "occupied",
    "purpose",
    "access",
];

#[test]
fn building_rejections_are_ordered_and_increment_only_the_selected_counter() {
    let mut w = World::new();
    w.info[0] = 3;
    w.check(true, 0, 7, &["info", "purpose"]);
    let mut w = World::new();
    w.info[1] = 1;
    w.check(true, 0, 8, &["info", "purpose", "wealth"]);
    let mut w = World::new();
    w.styles();
    SETTINGS.with(|s| s.borrow_mut().group_ok = 0xFFFF_FF00);
    let calls = w.check(true, 0, 9, &["info", "purpose", "wealth", "key", "group"]);
    assert_eq!(calls.last(), Some(&Call("group", vec![0x1111, 0x2222, 0xABCD, 0x2001])));
    for (capacity, outcome) in [(9, 11), (101, 13)] {
        let mut w = World::new();
        w.info[2] = 0xFFFF_0000 | capacity;
        w.check(true, 0, outcome, &["info", "purpose", "wealth"]);
    }
}

#[test]
fn building_capacity_boundaries_styles_and_property_lifetime() {
    for capacity in [10, 100] {
        let mut w = World::new();
        w.info[2] = 0xFFFF_0000 | capacity;
        let calls = w.check(true, 0, 27, BUILD_PASS);
        assert_eq!(
            calls[4],
            Call("resource", vec![0x1111, 0x2222, 0xABCD, 0xA521_60F5, 0, 0])
        );
        assert_eq!(calls[6], Call("property", vec![0x1122_3344, 0x2781_283F]));
    }
    for purpose in [0, 1, 2, 3, 4, u32::MAX] {
        let mut w = World::new();
        w.styles();
        w.info[0] = purpose;
        SETTINGS.with(|s| s.borrow_mut().purpose = purpose);
        let mut names = BUILD_PASS.to_vec();
        if (1..=3).contains(&purpose) {
            names.splice(3..3, ["key", "group"]);
        }
        w.check(true, 0, 27, &names);
    }
}

#[test]
fn monopoly_modes_use_raw_property_byte_and_low_resource_success_byte() {
    for mode in [u32::MAX, 0, 1, 2, 3, 0x8000_0000] {
        for monopoly in [0, 1, 0x80] {
            let mut w = World::new();
            SETTINGS.with(|s| {
                s.borrow_mut().mode = mode;
                s.borrow_mut().monopoly = monopoly;
            });
            let rejected = ((mode == 0 || mode == 1) && monopoly != 0) || (mode == 2 && monopoly == 0);
            w.check(true, 0, if rejected { 2 } else { 27 }, BUILD_PASS);
        }
    }
    let mut w = World::new();
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.resource_found = 0xFFFF_0000;
        s.mode = 2;
        s.monopoly = 1;
    });
    w.check(true, 0, 2, &["info", "purpose", "wealth", "key", "resource", "mode"]);
}

#[test]
fn lot_success_uses_signed_zone_byte_and_original_rectangle_arguments() {
    let mut w = World::new();
    let calls = w.check(false, 0x100, 27, LOT_PASS);
    assert_eq!(calls[2], Call("zone", vec![10, 20]));
    assert_eq!(calls[3], Call("fits", vec![1, (-2i32) as u32]));
    assert_eq!(calls[6], Call("legal", vec![9, 18, 3, 5, 0, 0, 0]));
}

#[test]
fn lot_zone_age_and_land_value_rejections_and_cache_hit() {
    let mut w = World::new();
    SETTINGS.with(|s| s.borrow_mut().fits = 0xFFFF_0000);
    w.check(false, 0, 6, &LOT_PASS[..4]);
    let mut w = World::new();
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.age_flag = 0x1234_0001;
        s.age = 179;
    });
    w.check(
        false,
        0x101,
        19,
        &[
            "find",
            "location",
            "construction",
            "zone",
            "fits",
            "age_flag",
            "age",
            "cache",
        ],
    );
    for (land, outcome) in [(39, 22), (40, 27), (50, 27), (51, 22)] {
        let mut w = World::new();
        SETTINGS.with(|s| {
            let s = &mut *s.borrow_mut();
            s.land_min = 40;
            s.land_max = 50;
            s.land = 0xFFFF_0000 | land;
        });
        let mut names = LOT_PASS.to_vec();
        names.insert(4, "land");
        if outcome == 22 {
            names.truncate(5);
        }
        w.check(false, 0, outcome, &names);
        if outcome == 22 {
            assert_eq!(w.view().observed_land_value(), land as i32);
        }
    }
    let mut w = World::new();
    SETTINGS.with(|s| s.borrow_mut().cached = true);
    w.cached_node[5] = 0xABCD_5678;
    w.check(false, 1, 0xABCD_5678, &["find"]);
}

#[test]
fn lot_states_and_area_legality_return_each_outcome() {
    for state in [2, 5, 6, i32::MAX as u32] {
        let mut w = World::new();
        SETTINGS.with(|s| s.borrow_mut().state = state);
        w.check(false, 0, 4, &LOT_PASS[..5]);
    }
    for (legal, outcome) in [
        (0, 0),
        (1, 4),
        (2, 6),
        (3, 3),
        (4, 0),
        (6, 0),
        (7, 27),
        (8, 27),
        (u32::MAX, 0),
    ] {
        let mut w = World::new();
        SETTINGS.with(|s| s.borrow_mut().legal = legal);
        w.check(false, 0, outcome, if outcome == 27 { LOT_PASS } else { &LOT_PASS[..7] });
    }
}

#[test]
fn occupied_lot_rejects_missing_building_special_purposes_and_historical_flag() {
    let mut w = World::new();
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.occupied = 0xFFFF_0001;
        s.building = 0;
    });
    let mut names = LOT_PASS[..8].to_vec();
    names.push("building");
    w.check(false, 0, 4, &names);
    for (purpose, target) in [(5, 2), (7, 6)] {
        let mut w = World::new();
        w.info[0] = purpose;
        w.view().set_purpose(target);
        SETTINGS.with(|s| s.borrow_mut().occupied = 1);
        let mut names = LOT_PASS[..8].to_vec();
        names.extend(["building", "info"]);
        w.check(false, 0, 15, &names);
    }
    let mut w = World::new();
    SETTINGS.with(|s| {
        let s = &mut *s.borrow_mut();
        s.occupied = 1;
        s.historical = 0x1234_0080;
    });
    let mut names = LOT_PASS[..8].to_vec();
    names.extend(["building", "info", "historical"]);
    w.check(false, 0, 18, &names);
}

#[test]
fn cached_lot_wealth_checks_and_stage_thresholds() {
    for (kick_out, resident_wealth, resident_purpose, outcome) in [
        (0, 1, 2, 16),
        (1, 3, 2, 16),
        (1, 2, 3, 16),
        (1, 1, 3, 27),
        (1, 2, 2, 27),
    ] {
        let mut w = World::new();
        w.info[0] = resident_purpose;
        w.info[1] = resident_wealth;
        w.view().set_kick_out(kick_out);
        SETTINGS.with(|s| s.borrow_mut().occupied = 1);
        let mut names = vec![
            "find",
            "location",
            "construction",
            "zone",
            "fits",
            "age_flag",
            "state",
            "rectangle",
            "legal",
            "occupied",
            "building",
            "info",
        ];
        if outcome == 27 {
            names.extend(["historical", "configuration", "purpose", "access"]);
        }
        names.push("cache");
        w.check(false, 1, outcome, &names);
    }
    for (wealth, stage, outcome) in [(2, 3, 27), (2, 4, 17), (1, 4, 27), (1, 5, 17)] {
        let mut w = World::new();
        w.info[1] = wealth;
        SETTINGS.with(|s| {
            let s = &mut *s.borrow_mut();
            s.occupied = 1;
            s.configuration = true;
            s.stage = 0xFFFF_0000 | stage;
        });
        let mut names = LOT_PASS[..8].to_vec();
        names.extend(["building", "info", "historical", "configuration", "stage"]);
        if outcome == 27 {
            names.extend(["purpose", "access"]);
        }
        w.check(false, 0, outcome, &names);
    }
}

#[test]
fn road_access_selects_residential_grid_industrial_purpose_or_lot_access() {
    for purpose in [u32::MAX, 0, 1, 2, 5, 6, 7, 8, 9] {
        for access in [0xFFFF_0000, 0xFFFF_0001] {
            let mut w = World::new();
            SETTINGS.with(|s| {
                let s = &mut *s.borrow_mut();
                s.purpose = purpose;
                s.access = access;
            });
            let mut names = LOT_PASS[..9].to_vec();
            match purpose {
                1 => names.extend(["access_grid", "access_cell"]),
                6..=8 => names.push("purpose_access"),
                _ => names.push("access"),
            }
            let calls = w.check(false, 0, if access as u8 == 0 { 21 } else { 27 }, &names);
            if purpose == 1 {
                assert_eq!(calls.last(), Some(&Call("access_cell", vec![10, 20])));
            } else if (6..=8).contains(&purpose) {
                assert_eq!(calls.last(), Some(&Call("purpose_access", vec![purpose])));
            }
        }
    }
}
