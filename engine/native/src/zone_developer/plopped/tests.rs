// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Plop constraints and network-record cases from the core, with ST0 fakes and complete call traces.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Water,
    Constraints([i32; 4], i32),
    Slope([i32; 4], u8),
    Maximum,
    Includes(i32, i32),
    CellWater(i32, i32),
    Network(i32, i32, u32),
    Zone(i32, i32),
    All(usize, u32),
    Any(usize, u32),
    Objects,
    Placement(usize, [i32; 2], i32),
    Flags(usize),
    Edges(usize, i32),
    Matches(u32, u32),
}

struct State {
    dev: usize,
    cfg: usize,
    lot_dev: usize,
    zones: usize,
    restriction: usize,
    vector: usize,
    records: usize,
    calls: Vec<Call>,
    water: u8,
    constraint: u32,
    slope: F80,
    maximum: F80,
    slope_word: u32,
    zone: u32,
    wet: Vec<(i32, i32)>,
    excluded: Vec<(i32, i32)>,
    networks: Vec<(i32, i32, u32, usize)>,
    occupants: Vec<Box<[u32]>>,
    flags: Vec<u32>,
    edges: Vec<u32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            dev: 0,
            cfg: 0,
            lot_dev: 0,
            zones: 0,
            restriction: 0,
            vector: 0,
            records: 0,
            calls: vec![],
            water: 0,
            constraint: 8,
            slope: F80::ZERO,
            maximum: F80::from_f32(5.0),
            slope_word: 0,
            zone: 0,
            wet: vec![],
            excluded: vec![],
            networks: vec![],
            occupants: vec![],
            flags: vec![],
            edges: vec![],
        }
    }
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn water(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.cfg);
        s.calls.push(Call::Water);
        (0x1234_5678u64 << 32) | 0xAABB_CC00 | u64::from(s.water)
    })
}

unsafe extern "thiscall" fn slope_data(this: *mut u8, rect: *const [i32; 4], water: u32) -> *const F80 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.lot_dev);
        s.calls.push(Call::Slope(unsafe { *rect }, water as u8));
        s.slope_word = water;
        &s.slope as *const F80
    })
}

#[unsafe(naked)]
unsafe extern "thiscall" fn slope() {
    core::arch::naked_asm!("push dword ptr [esp + 8]", "push dword ptr [esp + 8]",
        "call {log}", "fld tbyte ptr [eax]", "mov edx, 0x12121212", "ret 8", log = sym slope_data);
}

extern "thiscall" fn maximum_data(this: *mut u8) -> *const F80 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.cfg);
        s.calls.push(Call::Maximum);
        &s.maximum as *const F80
    })
}

#[unsafe(naked)]
unsafe extern "thiscall" fn maximum() {
    core::arch::naked_asm!("call {log}", "fld tbyte ptr [eax]", "mov edx, 0x34343434", "ret", log = sym maximum_data);
}

extern "thiscall" fn constraints(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, facing: i32, cfg: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        assert_eq!(cfg as usize, s.cfg);
        s.calls.push(Call::Constraints([x1, z1, x2, z2], facing));
        (0x5656_5656u64 << 32) | u64::from(s.constraint)
    })
}

extern "thiscall" fn includes(this: *mut u8, x: i32, z: i32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.restriction);
        s.calls.push(Call::Includes(x, z));
        (0x7878_7878u64 << 32) | 0xFFFF_FF00 | u64::from(!s.excluded.contains(&(x, z)))
    })
}

extern "thiscall" fn cell_water(this: *mut u8, x: i32, z: i32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::CellWater(x, z));
        (0x9090_9090u64 << 32) | 0xFFFF_FF00 | u64::from(s.wet.contains(&(x, z)))
    })
}

extern "thiscall" fn network(this: *mut u8, x: i32, z: i32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.dev);
        s.calls.push(Call::Network(x, z, flags));
        let p = s
            .networks
            .iter()
            .find(|&&(a, b, f, _)| (a, b, f) == (x, z, flags))
            .map_or(0, |&(_, _, _, id)| s.occupants[id].as_ptr() as u32);
        (0xABAB_ABABu64 << 32) | u64::from(p)
    })
}

extern "thiscall" fn zone(this: *mut u8, x: i32, z: i32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.zones);
        s.calls.push(Call::Zone(x, z));
        (0xCDCD_CDCDu64 << 32) | u64::from(s.zone)
    })
}

unsafe extern "thiscall" fn all(this: *mut u32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = unsafe { *this.add(1) } as usize;
        s.calls.push(Call::All(id, flags));
        (0xEFEF_EFEFu64 << 32) | 0xFFFF_FF00 | u64::from(s.flags[id] & flags == flags)
    })
}

unsafe extern "thiscall" fn any(this: *mut u32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = unsafe { *this.add(1) } as usize;
        s.calls.push(Call::Any(id, flags));
        (0x1313_1313u64 << 32) | 0xFFFF_FF00 | u64::from(s.flags[id] & flags != 0)
    })
}

unsafe extern "thiscall" fn flags(this: *mut u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = unsafe { *this.add(1) } as usize;
        s.calls.push(Call::Flags(id));
        (0x2424_2424u64 << 32) | u64::from(s.flags[id])
    })
}

unsafe extern "thiscall" fn edges(this: *mut u32, kind: i32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let id = unsafe { *this.add(1) } as usize;
        s.calls.push(Call::Edges(id, kind));
        (0x3535_3535u64 << 32) | u64::from(s.edges[id])
    })
}

extern "thiscall" fn objects(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.cfg);
        s.calls.push(Call::Objects);
        (0x4646_4646u64 << 32) | u64::from(s.vector as u32)
    })
}

unsafe extern "C" fn placement(
    cfg: *mut u8,
    record: *mut u32,
    origin: *const [i32; 2],
    facing: i32,
    x: *mut i32,
    z: *mut i32,
    a: *mut u32,
    b: *mut u32,
    kind: *mut i32,
) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(cfg as usize, s.cfg);
        let index = (record as usize - s.records) / 0x34;
        s.calls.push(Call::Placement(index, unsafe { *origin }, facing));
        unsafe {
            *x = *record.add(1) as i32;
            *z = *record.add(2) as i32;
            *a = *record.add(3);
            *b = *record.add(4);
            *kind = *record.add(5) as i32;
        }
        0x5757_5757u64 << 32
    })
}

extern "C" fn matches(mask: u32, piece: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Matches(mask, piece)));
    let active = ((mask.wrapping_add(0x0F0F_0F0F) >> 4) & 0x0101_0101) * 15;
    (u64::from(piece) << 32) | u64::from((mask ^ piece) & active == 0)
}

#[unsafe(naked)]
unsafe extern "C" fn seeded_entry() {
    core::arch::naked_asm!("mov dword ptr [esp - 0x1c], 0xCCBBAA77", "jmp {port}", port = sym fc4p_007306c0);
}

struct World {
    dev: Object,
    records: Vec<[u32; 13]>,
    occupant_table: Box<[usize; 48]>,
}

impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut dev = Object::new(0x100);
        unsafe {
            let view = ZoneDeveloper::from_ptr(dev.ptr());
            view.set_cells_x(64);
            view.set_cells_z(64);
            view.set_real_mode(1);
        }
        let mut occupant_table = Box::new([0; 48]);
        occupant_table[8] = all as *const () as usize;
        occupant_table[9] = any as *const () as usize;
        occupant_table[12] = flags as *const () as usize;
        occupant_table[38] = edges as *const () as usize;
        Self {
            dev,
            records: vec![],
            occupant_table,
        }
    }

    fn add_record(&mut self, type_: u8, x: i32, z: i32, a: u32, b: u32, kind: u32) {
        let mut record = [0; 13];
        record[..6].copy_from_slice(&[u32::from(type_), x as u32, z as u32, a, b, kind]);
        self.records.push(record);
    }

    fn add_network(&mut self, x: i32, z: i32, query: u32, flags: u32, edges: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let id = s.occupants.len();
            s.occupants
                .push(vec![self.occupant_table.as_ptr() as u32, id as u32].into_boxed_slice());
            s.flags.push(flags);
            s.edges.push(edges);
            s.networks.push((x, z, query, id));
        });
    }

    fn run(&mut self, rect: [i32; 4], restriction: bool) -> (u32, u32, Vec<Call>) {
        let _fpu = GameFpu::new();
        let mut cfg_vt = [0usize; 24];
        cfg_vt[8] = maximum as *const () as usize;
        cfg_vt[14] = water as *const () as usize;
        cfg_vt[23] = objects as *const () as usize;
        let mut cfg = [cfg_vt.as_ptr() as usize];
        let mut lot_vt = [0usize; 30];
        lot_vt[29] = slope as *const () as usize;
        let mut lot_dev = [lot_vt.as_ptr() as usize];
        let mut dev_vt = [0usize; 10];
        dev_vt[9] = cell_water as *const () as usize;
        let mut zones_vt = [0usize; 13];
        zones_vt[12] = zone as *const () as usize;
        let mut zones = [zones_vt.as_ptr() as usize];
        let mut restriction_mem = [0u32];
        let mut vector = [
            self.records.as_mut_ptr() as usize,
            self.records.as_mut_ptr().wrapping_add(self.records.len()) as usize,
        ];
        let mut network_flags = core::array::from_fn::<_, 13, _>(|i| 1u32 << i);
        let mut offsets = [0i32, -1, 0, 1, 0];
        unsafe {
            let dev = ZoneDeveloper::from_ptr(self.dev.ptr());
            dev.set_primary_vtable(dev_vt.as_ptr() as u32);
            dev.set_zone_manager(zones.as_mut_ptr().cast());
            dev.set_lot_configuration(lot_dev.as_mut_ptr().cast());
            dev.set_restriction(if restriction {
                restriction_mem.as_mut_ptr().cast()
            } else {
                core::ptr::null_mut()
            });
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.dev = self.dev.ptr() as usize;
            s.cfg = cfg.as_ptr() as usize;
            s.lot_dev = lot_dev.as_ptr() as usize;
            s.zones = zones.as_ptr() as usize;
            s.restriction = restriction_mem.as_ptr() as usize;
            s.vector = vector.as_mut_ptr() as usize;
            s.records = self.records.as_ptr() as usize;
            s.calls.clear();
        });
        let _flags = fake_data(0x00AB_3680, network_flags.as_mut_ptr().cast());
        let _offsets = fake_data(0x00AB_3780, offsets.as_mut_ptr().cast());
        let _constraints = fake(0x0072_B480, constraints as *const () as usize);
        let _includes = fake(0x0046_6610, includes as *const () as usize);
        let _network = fake(0x0072_FCA0, network as *const () as usize);
        let _placement = fake(0x0072_AD00, placement as *const () as usize);
        let _matches = fake(0x0072_9EC0, matches as *const () as usize);
        let args = [
            rect[0] as u32,
            rect[1] as u32,
            rect[2] as u32,
            rect[3] as u32,
            3,
            cfg.as_mut_ptr() as u32,
        ];
        let before = unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0x100).to_vec() };
        let top_before = crate::x87::status_word() & 0x3800;
        let (eax, edx) = unsafe {
            call_thiscall_with_eax(
                seeded_entry as *const () as usize,
                self.dev.ptr().add(4),
                &args,
                0x1234_5678,
            )
        };
        assert_eq!(top_before, crate::x87::status_word() & 0x3800);
        assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
        assert_eq!(before, unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0x100) });
        let calls = STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls));
        (eax, edx, calls)
    }
}

#[test]
fn dry_cells_scan_x_before_z_and_read_the_objects_after_all_cells() {
    let mut w = World::new();
    let rect = [5, 7, 6, 8];
    let (eax, edx, calls) = w.run(rect, false);
    assert_eq!((eax, edx), (8, 0x4646_4646));
    let mut expected = vec![Call::Water, Call::Slope(rect, 0), Call::Maximum];
    for (x, z) in [(5, 7), (5, 8), (6, 7), (6, 8)] {
        expected.extend([Call::CellWater(x, z), Call::Network(x, z, 0x1F4F), Call::Zone(x, z)]);
    }
    expected.push(Call::Objects);
    assert_eq!(calls, expected);
    STATE.with(|s| assert_eq!(s.borrow().slope_word, 0xCCBB_AA00));
}

#[test]
fn constraints_short_circuit_and_water_compatibility_skips_cell_water() {
    for code in [0, 5, 6, 7, 0xFFFF_FFFF] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.water = 0x80;
            s.constraint = code;
        });
        let (eax, edx, calls) = w.run([5, 7, 5, 7], false);
        assert_eq!((eax, edx), (code, 0x5656_5656));
        assert_eq!(calls, [Call::Water, Call::Constraints([5, 7, 5, 7], 3)]);
    }
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().water = 0x80);
    let (eax, _, calls) = w.run([5, 7, 5, 7], false);
    assert_eq!(eax, 8);
    assert!(!calls.contains(&Call::CellWater(5, 7)));
    STATE.with(|s| assert_eq!(s.borrow().slope_word, 0xCCBB_AA80));
}

#[test]
fn slope_is_rounded_before_comparing_the_unrounded_maximum_and_nan_passes() {
    for (slope, maximum, expected) in [
        (F80::from_f32(6.0), F80::from_f32(5.0), 4),
        (F80::from_f32(5.0), F80::from_f32(5.0), 8),
        (F80::from_f32(f32::NAN), F80::from_f32(5.0), 8),
        (F80::from_f32(5.0), F80::from_f32(f32::NAN), 8),
        (F80::from_f64(5.000000000931323), F80::from_f32(5.0), 8),
        (F80::from_f32(5.0), F80::from_f64(4.999999999068677), 4),
        (F80::from_f32(f32::INFINITY), F80::from_f32(f32::INFINITY), 8),
    ] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.slope = slope;
            s.maximum = maximum;
        });
        let (eax, edx, calls) = w.run([5, 7, 5, 7], false);
        assert_eq!(eax, expected);
        if expected == 4 {
            assert_eq!(edx, 0x3434_3434);
            assert_eq!(calls, [Call::Water, Call::Slope([5, 7, 5, 7], 0), Call::Maximum]);
        }
    }
}

#[test]
fn minimum_code_keeps_scanning_and_bounds_restriction_water_have_priority() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.excluded.push((0, 0));
        s.wet.push((0, 1));
        s.zone = 15;
    });
    let (eax, edx, calls) = w.run([-1, 0, 1, 1], true);
    assert_eq!((eax, edx), (0, 3));
    assert_eq!(
        calls,
        [
            Call::Water,
            Call::Slope([-1, 0, 1, 1], 0),
            Call::Maximum,
            Call::Includes(0, 0),
            Call::Includes(0, 1),
            Call::CellWater(0, 1),
            Call::Includes(1, 0),
            Call::CellWater(1, 0),
            Call::Network(1, 0, 0x1F4F),
            Call::Zone(1, 0),
            Call::Includes(1, 1),
            Call::CellWater(1, 1),
            Call::Network(1, 1, 0x1F4F),
            Call::Zone(1, 1)
        ]
    );
    for zone in [0, 1, 9, 10, 11, 12, 13, 14, 15, 16, 42, 47] {
        let mut w = World::new();
        STATE.with(|s| s.borrow_mut().zone = zone);
        let expected = match zone & 31 {
            15 => 3,
            10..=14 => 7,
            _ => 8,
        };
        assert_eq!(w.run([5, 7, 5, 7], false).0, expected);
    }
}

#[test]
fn network_nodes_block_and_matching_records_keep_success_until_a_flag_mismatch() {
    let mut w = World::new();
    w.add_network(10, 10, 0x1F4F, 0x20_0008, 0);
    let (eax, edx, calls) = w.run([10, 10, 10, 10], false);
    assert_eq!((eax, edx), (3, 0xEFEF_EFEF));
    assert_eq!(
        &calls[3..],
        &[
            Call::CellWater(10, 10),
            Call::Network(10, 10, 0x1F4F),
            Call::All(0, 0x20_0000)
        ]
    );

    let mut w = World::new();
    w.add_record(5, 0, 0, 0, 0, 0);
    w.add_record(7, 99, 99, 0, 0, 3);
    w.add_record(7, 10, 10, 0, 0, 3);
    w.add_record(7, 10, 10, 0, 0, 4);
    w.add_record(7, 10, 10, 0, 0, 3);
    w.add_network(10, 10, 0x1F4F, 8, 0);
    let (eax, _, calls) = w.run([10, 10, 10, 10], false);
    assert_eq!(eax, 8);
    assert_eq!(
        &calls[3..16],
        &[
            Call::CellWater(10, 10),
            Call::Network(10, 10, 0x1F4F),
            Call::All(0, 0x20_0000),
            Call::Objects,
            Call::Placement(1, [10, 10], 3),
            Call::Placement(2, [10, 10], 3),
            Call::Flags(0),
            Call::Edges(0, 3),
            Call::Matches(0, 0),
            Call::Matches(0, 0),
            Call::Placement(3, [10, 10], 3),
            Call::Flags(0),
            Call::Objects,
        ]
    );
}

#[test]
fn second_mask_comparison_is_reversed_and_first_failure_short_circuits() {
    for a in [0, 2] {
        let mut w = World::new();
        w.add_record(7, 10, 10, a, 0, 3);
        w.add_network(10, 10, 0x1F4F, 8, 1);
        let (eax, _, calls) = w.run([10, 10, 10, 10], false);
        assert_eq!(eax, 3);
        let compares = calls
            .iter()
            .filter(|c| matches!(c, Call::Matches(..)))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            compares,
            if a == 0 {
                vec![Call::Matches(0, 1), Call::Matches(1, 0)]
            } else {
                vec![Call::Matches(2, 1)]
            }
        );
    }
}

#[test]
fn outward_edges_use_city_bounds_masks_flags_and_stop_only_the_current_record() {
    let mut w = World::new();
    w.add_record(7, 10, 10, 0, 0x0101_0101, 3);
    w.add_record(7, 10, 10, 0, 0x0101_0101, 4);
    w.add_network(9, 10, 0x1F4F, 0x40_0008, 0);
    let (eax, edx, calls) = w.run([10, 10, 10, 10], false);
    assert_eq!((eax, edx), (3, 0xABAB_ABAB));
    assert_eq!(
        &calls[6..],
        &[
            Call::Objects,
            Call::Placement(0, [10, 10], 3),
            Call::Network(9, 10, 0x1F4F),
            Call::Any(0, 0x60_0000),
            Call::Placement(1, [10, 10], 3),
            Call::Network(9, 10, 0x10),
            Call::Network(10, 9, 0x10),
            Call::Network(11, 10, 0x10),
            Call::Network(10, 11, 0x10),
        ]
    );
    let mut w = World::new();
    w.add_record(7, 0, 0, 0, 0x0101_0101, 3);
    let (eax, _, calls) = w.run([0, 0, 1, 0], false);
    assert_eq!(eax, 8);
    assert_eq!(
        calls
            .iter()
            .filter(|c| matches!(c, Call::Network(..)))
            .cloned()
            .collect::<Vec<_>>(),
        [
            Call::Network(0, 0, 0x1F4F),
            Call::Network(1, 0, 0x1F4F),
            Call::Network(0, 1, 0x1F4F),
        ]
    );
}

#[test]
fn empty_rectangles_still_check_slope_and_objects_and_mode_does_not_change_validation() {
    let mut w = World::new();
    assert_eq!(
        w.run([2, 3, 1, 2], false),
        (
            8,
            0x4646_4646,
            vec![Call::Water, Call::Slope([2, 3, 1, 2], 0), Call::Maximum, Call::Objects]
        )
    );
    unsafe {
        ZoneDeveloper::from_ptr(w.dev.ptr()).set_real_mode(0);
    }
    let highlight = w.run([2, 3, 2, 3], false);
    unsafe {
        ZoneDeveloper::from_ptr(w.dev.ptr()).set_real_mode(0x80);
    }
    assert_eq!(highlight, w.run([2, 3, 2, 3], false));
}
