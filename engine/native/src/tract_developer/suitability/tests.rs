// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration-filter and road-bit cases from the core tract-developer checks, with boundary inputs.

use super::*;
use crate::testing::{fake, Object};
use crate::tract_developer::testing::{run, snapshot, ECX};
use std::cell::RefCell;

#[derive(Clone)]
struct Settings {
    purpose: u32,
    wealth: u32,
    purpose_ok: u32,
    wealth_ok: u32,
    stages: [u32; 2],
    stage_calls: usize,
    style: Option<u32>,
    buildings: u32,
    minimum: u32,
    maximum: u32,
    calls: Vec<(u32, u32, u32)>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            purpose: 1,
            wealth: 2,
            purpose_ok: 0x1234_5601,
            wealth_ok: 0x9876_5401,
            stages: [2, 2],
            stage_calls: 0,
            style: Some(0x2001),
            buildings: 1,
            minimum: 5,
            maximum: 100,
            calls: Vec::new(),
        }
    }
}
thread_local! {
    static SETTINGS: RefCell<Settings> = RefCell::new(Settings::default());
}
fn get(this: u32, slot: u32, arg: u32, value: impl FnOnce(&mut Settings) -> u32) -> u64 {
    SETTINGS.with(|settings| {
        let s = &mut *settings.borrow_mut();
        s.calls.push((slot, this, arg));
        (0xBADC_0FFEu64 << 32) | u64::from(value(s))
    })
}
extern "thiscall" fn purpose(this: u32) -> u64 {
    get(this, 0x1C, 0, |s| s.purpose)
}
extern "thiscall" fn wealth(this: u32) -> u64 {
    get(this, 0x20, 0, |s| s.wealth)
}
extern "thiscall" fn purpose_ok(this: u32, arg: u32) -> u64 {
    get(this, 0x44, arg, |s| s.purpose_ok)
}
extern "thiscall" fn wealth_ok(this: u32, arg: u32) -> u64 {
    get(this, 0x40, arg, |s| s.wealth_ok)
}
extern "thiscall" fn stage(this: u32) -> u64 {
    get(this, 0x2C, 0, |s| {
        let result = s.stages[s.stage_calls.min(1)];
        s.stage_calls += 1;
        result
    })
}
extern "thiscall" fn style_ok(this: u32, arg: u32) -> u64 {
    get(this, 0x48, arg, |s| u32::from(s.style == Some(arg)))
}
extern "thiscall" fn buildings(this: u32) -> u64 {
    get(this, 0x60, 0, |s| s.buildings)
}
extern "thiscall" fn minimum(this: u32) -> u64 {
    get(this, 0x30, 0, |s| s.minimum)
}
extern "thiscall" fn maximum(this: u32) -> u64 {
    get(this, 0x34, 0, |s| s.maximum)
}

struct World {
    td: Object,
    context: Object,
    histogram: [u32; 28],
    developer: [u32; 1],
    configuration: [u32; 1],
    styles: [u32; 3],
    style_count: u32,
    developer_table: Box<[usize; 9]>,
    configuration_table: Box<[usize; 25]>,
}
impl World {
    fn new(settings: Settings) -> Self {
        SETTINGS.with(|s| *s.borrow_mut() = settings);
        let mut w = Self {
            td: Object::new(0x180),
            context: Object::new(0x60),
            histogram: [u32::MAX; 28],
            developer: [0],
            configuration: [0],
            styles: [0x2000, 0x2001, 0x2002],
            style_count: 3,
            developer_table: Box::new([0; 9]),
            configuration_table: Box::new([0; 25]),
        };
        w.developer_table[7] = purpose as *const () as usize;
        w.developer_table[8] = wealth as *const () as usize;
        for (offset, target) in [
            (0x44, purpose_ok as *const () as usize),
            (0x40, wealth_ok as *const () as usize),
            (0x2C, stage as *const () as usize),
            (0x48, style_ok as *const () as usize),
            (0x60, buildings as *const () as usize),
            (0x30, minimum as *const () as usize),
            (0x34, maximum as *const () as usize),
        ] {
            w.configuration_table[offset / 4] = target;
        }
        w.developer[0] = w.developer_table.as_ptr() as u32;
        w.configuration[0] = w.configuration_table.as_ptr() as u32;
        let td = w.view();
        td.set_maximum_stage(3);
        td.set_existing(10);
        td.set_desired(20);
        td.set_desired_maximum(30);
        td.set_style_index(1);
        td.set_single_style(1);
        w
    }
    fn view(&mut self) -> TractDeveloper {
        unsafe { TractDeveloper::from_ptr(self.td.ptr()) }
    }
    fn check(&mut self, expected: u32, sequence: &[(u32, u32)]) {
        let developer = self.developer.as_mut_ptr() as u32;
        let configuration = self.configuration.as_mut_ptr() as u32;
        let context = self.context.ptr() as u32;
        unsafe {
            store(context, 0x54, self.histogram.as_mut_ptr() as u32);
        }
        let td = self.view();
        td.set_developer(developer);
        td.set_debug_context(context);
        td.set_styles_begin(self.styles.as_ptr() as u32);
        td.set_styles_end(self.styles.as_ptr() as u32 + self.style_count * 4);
        let before = snapshot(td.as_ptr(), 0x180);
        let result = run(0x00704C00, Conv::Thiscall, td.as_ptr() as u32, &[configuration]);
        assert_eq!(result.0, expected);
        assert_eq!(result.1, 0);
        assert_eq!(
            result.2,
            match expected {
                13 => td.desired_maximum().wrapping_add(td.existing()) as u32,
                27 => context,
                _ => 0xBADC_0FFE,
            }
        );
        assert_eq!(snapshot(td.as_ptr(), 0x180), before);
        let mut histogram = [u32::MAX; 28];
        histogram[expected as usize] = 0;
        assert_eq!(self.histogram, histogram);
        SETTINGS.with(|s| {
            assert_eq!(
                s.borrow().calls,
                sequence
                    .iter()
                    .map(|&(slot, arg)| (
                        slot,
                        if slot == 0x1C || slot == 0x20 {
                            developer
                        } else {
                            configuration
                        },
                        arg
                    ))
                    .collect::<Vec<_>>()
            )
        });
    }
}

const PREFIX: &[(u32, u32)] = &[(0x1C, 0), (0x44, 1), (0x20, 0), (0x40, 2), (0x2C, 0)];
fn sequence(tail: &[(u32, u32)]) -> Vec<(u32, u32)> {
    PREFIX.iter().chain(tail).copied().collect()
}

#[test]
fn configuration_filter_reports_each_early_rejection_and_increments_only_its_counter() {
    let mut w = World::new(Settings {
        purpose_ok: 0x1234_5600,
        ..Settings::default()
    });
    w.check(7, &[(0x1C, 0), (0x44, 1)]);
    let mut w = World::new(Settings {
        wealth_ok: 0x9876_5400,
        ..Settings::default()
    });
    w.check(8, &PREFIX[..4]);
    let mut w = World::new(Settings {
        stages: [0xAA00_00FF, 0],
        ..Settings::default()
    });
    w.check(14, PREFIX);
    let mut w = World::new(Settings {
        style: None,
        ..Settings::default()
    });
    w.check(9, &sequence(&[(0x48, 0x2001)]));
    let mut w = World::new(Settings {
        style: None,
        ..Settings::default()
    });
    w.view().set_single_style(0);
    w.check(9, &sequence(&[(0x48, 0x2000), (0x48, 0x2001), (0x48, 0x2002)]));
    let mut w = World::new(Settings {
        buildings: 0,
        ..Settings::default()
    });
    w.check(0, &sequence(&[(0x48, 0x2001), (0x60, 0)]));
}

#[test]
fn configuration_filter_capacity_edges_keep_signed_wrapping_and_low_width_semantics() {
    let full = sequence(&[(0x48, 0x2001), (0x60, 0), (0x30, 0), (0x34, 0)]);
    let mut w = World::new(Settings::default());
    w.check(27, &full);
    let mut w = World::new(Settings {
        minimum: 41,
        ..Settings::default()
    });
    w.check(13, &full);
    let mut w = World::new(Settings {
        minimum: 40,
        ..Settings::default()
    });
    w.check(27, &full);
    let mut w = World::new(Settings {
        minimum: 0xFFFF_0005,
        maximum: 0xAAAA_0014,
        buildings: 0x100,
        ..Settings::default()
    });
    w.check(27, &full);
    let mut w = World::new(Settings {
        maximum: 19,
        ..Settings::default()
    });
    let mut below = full.clone();
    below.push((0x2C, 0));
    w.check(11, &below);
    let mut w = World::new(Settings {
        maximum: 0,
        stages: [2, 3],
        ..Settings::default()
    });
    w.check(27, &below);
    let mut w = World::new(Settings {
        maximum: 0,
        ..Settings::default()
    });
    w.view().set_relaxed(0x80);
    w.check(27, &full);
    let mut w = World::new(Settings::default());
    w.view().set_desired(-1);
    w.check(27, &full);
    let mut w = World::new(Settings::default());
    w.view().set_desired_maximum(i32::MAX);
    w.view().set_existing(1);
    w.check(13, &full);
    let mut w = World::new(Settings {
        stages: [0x1234_0001, 1],
        minimum: u32::MAX,
        ..Settings::default()
    });
    w.check(27, &sequence(&[(0x48, 0x2001), (0x60, 0), (0x34, 0)]));
    let mut w = World::new(Settings::default());
    w.view().set_maximum_stage(-1);
    w.check(14, PREFIX);
}

#[test]
fn configuration_filter_tries_styles_in_order_only_for_residential_service_and_office() {
    let mut w = World::new(Settings::default());
    w.view().set_single_style(0);
    w.check(
        27,
        &sequence(&[(0x48, 0x2000), (0x48, 0x2001), (0x60, 0), (0x30, 0), (0x34, 0)]),
    );
    for p in [0u32, 4, u32::MAX] {
        let mut w = World::new(Settings {
            purpose: p,
            style: None,
            ..Settings::default()
        });
        let mut expected = PREFIX.to_vec();
        expected[1].1 = p;
        expected.extend([(0x60, 0), (0x30, 0), (0x34, 0)]);
        w.check(27, &expected);
    }
    for p in [1u32, 2, 3] {
        let mut w = World::new(Settings {
            purpose: p,
            ..Settings::default()
        });
        let mut expected = sequence(&[(0x48, 0x2001), (0x60, 0), (0x30, 0), (0x34, 0)]);
        expected[1].1 = p;
        w.check(27, &expected);
    }
    let mut w = World::new(Settings {
        style: None,
        ..Settings::default()
    });
    w.style_count = 0;
    w.check(27, &sequence(&[(0x60, 0), (0x30, 0), (0x34, 0)]));
}

struct Roads {
    storage: u32,
    offset: i32,
    calls: Vec<(u32, u32)>,
}
thread_local! {
    static ROADS: RefCell<Roads> = const { RefCell::new(Roads { storage: 0, offset: 0, calls: Vec::new() }) };
}
extern "thiscall" fn required_roads(this: u32, output: *mut u32) -> u64 {
    unsafe {
        assert_eq!(std::slice::from_raw_parts(output, 5), [0; 5]);
    }
    ROADS.with(|roads| {
        let r = &mut *roads.borrow_mut();
        r.calls.push((this, output as u32));
        unsafe {
            *output = r.storage;
            *output.add(1) = r.offset as u32;
        }
    });
    0x1234_5678_AABB_CCDD
}
extern "C" fn free_roads(storage: u32) -> u64 {
    ROADS.with(|roads| roads.borrow_mut().calls.push((0, storage)));
    0x5566_7788_AABB_CCDD
}

#[test]
fn corner_lot_rotates_bits_across_word_boundaries_and_accepts_each_network_type() {
    let _free = fake(0x0090CF63, free_roads as *const () as usize);
    let mut vt = [0usize; 22];
    vt[0x54 / 4] = required_roads as *const () as usize;
    let configuration = [vt.as_ptr() as u32];
    let cfg = configuration.as_ptr() as u32;
    let mut candidate = Object::new(0xA4);
    let p = candidate.ptr() as u32;
    for facing in [i32::MIN, -7, -1, 0, 1, 2, 3, 4, i32::MAX] {
        for offset in [-33i32, -1, 0, 30, 31, 32] {
            for mask in 0..16u32 {
                for network in [0x70u32, 0x7C, 0x88] {
                    let mut storage = [0u32; 8];
                    let center = storage.as_mut_ptr().wrapping_add(3) as u32;
                    for i in 0..4 {
                        let bit = offset + i;
                        let index = (3 + bit.div_euclid(32)) as usize;
                        if mask & (1 << i) != 0 {
                            storage[index] |= 1 << bit.rem_euclid(32);
                        }
                    }
                    let mut street = [0i32; 4];
                    let mut road = [0i32; 4];
                    let mut avenue = [0i32; 4];
                    for side in 0..4 {
                        match network {
                            0x70 => street[side] = -1,
                            0x7C => road[side] = 1,
                            _ => avenue[side] = i32::MIN,
                        }
                    }
                    unsafe {
                        store(p, 0x10, facing as u32);
                        store(p, 0x70, street.as_mut_ptr() as u32);
                        store(p, 0x7C, road.as_mut_ptr() as u32);
                        store(p, 0x88, avenue.as_mut_ptr() as u32);
                    }
                    ROADS.with(|r| {
                        *r.borrow_mut() = Roads {
                            storage: center,
                            offset,
                            calls: Vec::new(),
                        }
                    });
                    let before = snapshot(p as *mut u8, 0xA4);
                    let result = run(0x00705DA0, Conv::Thiscall, ECX, &[cfg, p]);
                    assert_eq!((result.0, result.2), (0xAABB_CC01, 0x5566_7788));
                    assert_eq!(snapshot(p as *mut u8, 0xA4), before);
                    ROADS.with(|r| {
                        let r = r.borrow();
                        assert_eq!(r.calls.len(), 2);
                        assert_eq!(r.calls[0].0, cfg);
                        assert_eq!(r.calls[1], (0, center));
                    });
                    if mask != 0 {
                        street.fill(0);
                        road.fill(0);
                        avenue.fill(0);
                        ROADS.with(|r| {
                            *r.borrow_mut() = Roads {
                                storage: center,
                                offset,
                                calls: Vec::new(),
                            }
                        });
                        assert_eq!(run(0x00705DA0, Conv::Thiscall, ECX, &[cfg, p]).0, 0xAABB_CC00);
                    }
                    for connected_side in 0..4usize {
                        street.fill(0);
                        road.fill(0);
                        avenue.fill(0);
                        match network {
                            0x70 => street[connected_side] = -1,
                            0x7C => road[connected_side] = 1,
                            _ => avenue[connected_side] = i32::MIN,
                        }
                        ROADS.with(|r| {
                            *r.borrow_mut() = Roads {
                                storage: center,
                                offset,
                                calls: Vec::new(),
                            }
                        });
                        let expected = (0..4).all(|side| {
                            mask & (1 << side) == 0
                                || facing.wrapping_add(side).wrapping_add(1).rem_euclid(4) as usize == connected_side
                        });
                        assert_eq!(
                            run(0x00705DA0, Conv::Thiscall, ECX, &[cfg, p]).0,
                            0xAABB_CC00 | u32::from(expected),
                            "facing={facing} mask={mask} connected={connected_side}"
                        );
                    }
                }
            }
        }
    }
}
