// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cycle and plant-budget cases from the core power tests, using original objects and calls.

use super::*;
use crate::abi::{call_raw, RawCall};
use crate::power::messages::tests::{finish, run, script, Step, World};
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};

macro_rules! method {
    ($name:ident, $site:literal $(, $arg:ident)*) => {
        extern "thiscall" fn $name(this: *mut u8 $(, $arg: u32)*) -> u64 {
            run($site, this as u32, &[$($arg),*])
        }
    };
}
macro_rules! function {
    ($name:ident, $site:literal $(, $arg:ident)*) => {
        extern "C" fn $name($($arg: u32),*) -> u64 { run($site, 0, &[$($arg),*]) }
    };
}
method!(reset, 0x006E_B020, rect);
method!(wire, 0x006E_D6E0, rect, skip);
method!(grid, 0x006E_E110, x, z);
method!(cell, 0x006E_B280, connection, x, z);
method!(powered_lots, 0x006E_BD00);
method!(age, 0x006E_C8F0, plant);
function!(framework_fake, 0x0087_93EC);
method!(release_ref, 0x0041_1870);
method!(clear_connections_fake, 0x0041_1D70);
method!(clear_deals_fake, 0x0042_1660);
method!(new_deals_fake, 0x0042_1A90);
method!(delete_deals_fake, 0x0042_1760);
function!(bitmap, 0x0060_3E00, bits, width);
function!(prop_float, 0x005F_D4F0, object, id, out);
function!(prop_cost, 0x005F_D4B0, object, id, out);
function!(prop_integer, 0x005F_D450, object, id, out);
method!(curve_construct, 0x007D_0410);
method!(curve_load, 0x005F_4620, property);
extern "thiscall" fn curve_value(this: *mut u8, input: f32) -> f64 {
    f64::from_bits(run(0x005F_4250, this as u32, &[input.to_bits()]))
}
function!(free_fake, 0x0090_CF63, object);
function!(allocate_message, 0x0091_33DA, size);
method!(construct_message, 0x0091_34D6);
extern "stdcall" fn divide(low: u32, high: u32, dl: u32, dh: u32) -> u64 {
    run(0x009E_FEB0, 0, &[low, high, dl, dh])
}
function!(notify_fake, 0x005F_98C0, id, flags, a, b, c, d);
method!(allocate_head, 0x0C, bytes);
method!(free_head, 0x10010, head);
method!(query_allocator, 0x10014, clsid, iid, out);
method!(deals, 0x78, kind, list);
method!(connections, 0x44, kind, list);
method!(set_used, 0x54, used);
method!(connection_visit, 0x30, visited);
method!(plant_position, 0x10020, out);
method!(convert_position, 0x10014A, x, z, cx, cz);
method!(plant_property, 0x10024, id);
method!(line_item, 0x50, index);
method!(set_cost, 0x10030, low, high);
method!(budget_item, 0xCC, props, purpose, out);
method!(department, 0x70, index);
method!(funded_cost, 0x64, props, low, high, purpose);
method!(budget_cost, 0xD4, props, index, low, high);
method!(deal_city, 0x1C);
method!(deal_used, 0x150);
method!(deal_max, 0x128);
method!(find_connection, 0x3C, kind, city);
method!(connection_neighbor, 0x34, connection);
method!(available, 0x134);
method!(set_message_type, 0x114, id);
method!(post, 0x110, message, immediate);
method!(statistic, 0x130, id, value);
method!(distress, 0x194, flag, on);
method!(update_advisor, 0x7C);
method!(department_cost, 0x8C, department);
method!(connection_position, 0x118, x, z);
method!(connection_power, 0x138, value);

struct Fixture {
    world: World,
    stats: Object,
    _stats_vt: Box<[usize; 0x17C / 4]>,
    globals: Box<[*mut u8; 8]>,
    _guards: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        let w = World::new();
        let (mut stats, vt) = w.object(&[(0x30, statistic as *const () as usize)]);
        let mut globals = Box::new([null_mut(); 8]);
        globals[0] = stats.ptr();
        let mut guards = Vec::new();
        for (i, address) in [
            0x00B4_3DC0,
            0x00B2_0D58,
            0x00B2_07DC,
            0x00B4_3D64,
            0x00B4_3CCC,
            0x00B4_3D8C,
            0x00B4_3CEC,
        ]
        .into_iter()
        .enumerate()
        {
            guards.push(fake_data(address, (&mut globals[i] as *mut *mut u8).cast()));
        }
        for (address, entry) in [
            (0x006E_B020, reset as *const ()),
            (0x006E_D6E0, wire as *const ()),
            (0x006E_E110, grid as *const ()),
            (0x006E_B280, cell as *const ()),
            (0x006E_BD00, powered_lots as *const ()),
            (0x006E_C8F0, age as *const ()),
            (0x0087_93EC, framework_fake as *const ()),
            (0x0041_1870, release_ref as *const ()),
            (0x0041_1D70, clear_connections_fake as *const ()),
            (0x0042_1660, clear_deals_fake as *const ()),
            (0x0042_1A90, new_deals_fake as *const ()),
            (0x0042_1760, delete_deals_fake as *const ()),
            (0x0060_3E00, bitmap as *const ()),
            (0x005F_D4F0, prop_float as *const ()),
            (0x005F_D4B0, prop_cost as *const ()),
            (0x005F_D450, prop_integer as *const ()),
            (0x007D_0410, curve_construct as *const ()),
            (0x005F_4620, curve_load as *const ()),
            (0x005F_4250, curve_value as *const ()),
            (0x0090_CF63, free_fake as *const ()),
            (0x0091_33DA, allocate_message as *const ()),
            (0x0091_34D6, construct_message as *const ()),
            (0x009E_FEB0, divide as *const ()),
            (0x005F_98C0, notify_fake as *const ()),
        ] {
            guards.push(fake(address, entry as usize));
        }
        Self {
            world: w,
            stats,
            _stats_vt: vt,
            globals,
            _guards: guards,
        }
    }
    fn sim(&mut self) -> IdleSim {
        unsafe { IdleSim::from_ptr(self.world.sim.ptr()) }
    }
    fn invoke(&mut self, budget: u32) -> (u32, u32) {
        let args = [budget];
        let mut c = RawCall::new(
            Conv::Thiscall,
            fc4p_006ee960 as *const () as usize,
            self.world.sim.ptr() as u32,
            &args,
        );
        c.eax = 0xABCD_1234;
        c.edx = 0xFEDC_BA98;
        unsafe { call_raw(&mut c) };
        finish();
        (c.eax, c.edx)
    }
}
fn scratch(site: u32, args: &[u32], eax: u32) -> Step {
    Step::new(site, null_mut(), args, eax).scratch()
}

fn environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]",
            "pushfd", "pop {flags:e}", cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
            flags = lateout(reg) flags, out("ax") sw);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn disabled_unknown_and_idle_states_ignore_budget_and_keep_return_bytes() {
    let mut f = Fixture::new();
    let sim = f.sim();
    sim.set_wire_dirty(1);
    sim.set_recompute(1);
    sim.set_state(3);
    let before = unsafe { std::slice::from_raw_parts(sim.as_ptr(), 0x400) }.to_vec();
    script(vec![]);
    assert_eq!(f.invoke(u32::MAX), (0xABCD_1200, 0xFEDC_BA98));
    assert_eq!(unsafe { std::slice::from_raw_parts(sim.as_ptr(), 0x400) }, before);
    sim.set_enabled(1);
    sim.set_wire_dirty(0);
    for state in [5, 0x8000_0000, u32::MAX] {
        sim.set_state(state);
        script(vec![]);
        assert_eq!(f.invoke(0).0, state);
        assert_eq!(sim.state(), state);
    }
    sim.set_state(4);
    script(vec![]);
    assert_eq!(f.invoke(0).0, 4);
    assert_eq!((sim.state(), sim.recompute()), (0, 0));
    sim.set_state(4);
    script(vec![]);
    assert_eq!(f.invoke(999).0, 4);
    assert_eq!(sim.state(), 4);
}

#[test]
fn dirty_wire_runs_before_state_dispatch_and_reads_the_state_after_callbacks() {
    let mut f = Fixture::new();
    let sim = f.sim();
    sim.set_enabled(0x80);
    sim.set_wire_dirty(1);
    sim.set_state(1);
    let p = sim.as_ptr();
    script(vec![
        Step::new(0x006E_B020, p, &[unsafe { p.add(0x33C) } as u32], 0),
        Step::new(0x006E_D6E0, p, &[unsafe { p.add(0x33C) } as u32, 0], 0)
            .action(move |_, _| unsafe { IdleSim::from_ptr(p) }.set_state(9)),
    ]);
    assert_eq!(f.invoke(5).0, 9);
    assert_eq!(sim.wire_dirty(), 0);
}

fn populate(holder: u32, head: *mut u8, nodes: &[*mut u8]) {
    unsafe {
        put(holder as *mut u8, 0, head as u32);
        put(head, 0, nodes.first().copied().unwrap_or(head) as u32);
        for (i, &node) in nodes.iter().enumerate() {
            put(node, 0, nodes.get(i + 1).copied().unwrap_or(head) as u32);
        }
    }
}

#[test]
fn state_zero_rotates_history_resets_deals_and_connections_and_clears_the_next_bitmap() {
    let mut f = Fixture::new();
    let sim = f.sim();
    let p = sim.as_ptr();
    let (mut allocator_obj, _avt) = f.world.object(&[
        (0x0C, allocate_head as *const () as usize),
        (0x10, free_head as *const () as usize),
    ]);
    let a = allocator_obj.ptr();
    let (mut framework_obj, _fvt) = f.world.object(&[(0x14, query_allocator as *const () as usize)]);
    let fw = framework_obj.ptr();
    let (mut neighbors_obj, _nvt) = f.world.object(&[
        (0x78, deals as *const () as usize),
        (0x44, connections as *const () as usize),
    ]);
    let neighbors = neighbors_obj.ptr();
    let (mut deal_obj, _dvt) = f.world.object(&[
        (0x54, set_used as *const () as usize),
        (0x30, connection_visit as *const () as usize),
    ]);
    let deal = deal_obj.ptr();
    let (mut city_obj, _cvt) = f.world.object(&[]);
    let city = city_obj.ptr();
    let mut heads = [Object::new(16), Object::new(16), Object::new(16)];
    let hs = heads.each_mut().map(Object::ptr);
    let mut nodes = [Object::new(16), Object::new(16), Object::new(16)];
    let ns = nodes.each_mut().map(Object::ptr);
    let mut plant_head = Object::new(16);
    let h = plant_head.ptr();
    let mut plant_node = Object::new(16);
    let node = plant_node.ptr();
    unsafe {
        put(h, 0, node as u32);
        put(node, 0, h as u32);
        put(node, 8, 2);
        for n in ns {
            put(n, 8, deal as u32);
        }
    }
    sim.set_enabled(1);
    sim.set_state(0);
    sim.set_plants(h);
    sim.set_neighbors(neighbors);
    sim.set_city(city);
    sim.set_next_powered(0x123400usize as *mut u8);
    sim.set_changed(0x80);
    for index in [0, 11, -1] {
        sim.set_ring_index(index);
        let ring = if index == 0 { 1 } else { 0 };
        for i in 0..10 {
            unsafe {
                put(p, 0x90 + ring * 40 + i * 4, (i as u32).wrapping_add(0x8000_0000));
                put(p, 0x270 + i * 4, i as u32);
            }
        }
        let mut steps = vec![];
        f.globals[1] = null_mut();
        f.globals[2] = null_mut();
        for i in 0..3 {
            if i == 0 || i == 2 {
                let cache = if i == 0 { &mut f.globals[1] } else { &mut f.globals[2] } as *mut *mut u8 as usize;
                steps.extend([
                    Step::new(0x0087_93EC, null_mut(), &[], fw as u32),
                    Step::new(0x10014, fw, &[0x3AE4_BEA3, 0x3AE4_BEAB, 0], 1)
                        .pointer(2)
                        .output(2, &[a as u32]),
                    scratch(0x0041_1870, &[], 0).action(move |_, _| assert_eq!(unsafe { *(cache as *mut *mut u8) }, a)),
                ]);
            }
            let slot = if i == 2 { 0x44 } else { 0x78 };
            let kind = if i == 2 { 4 } else { 0x4773_FFE0 + i as u32 };
            let (head, node) = (hs[i], ns[i]);
            steps.extend([
                Step::new(0x0C, a, &[12], head as u32),
                Step::new(slot, neighbors, &[kind, 0], 0)
                    .pointer(1)
                    .action(move |_, args| {
                        assert_eq!(unsafe { word(head, 0) }, head as u32);
                        assert_eq!(unsafe { word(head, 4) }, head as u32);
                        populate(args[1], head, &[node]);
                    }),
                Step::new(if i == 2 { 0x30 } else { 0x54 }, deal, &[0], 0),
            ]);
        }
        for (site, head) in [(0x0041_1D70, hs[2]), (0x0042_1660, hs[1]), (0x0042_1660, hs[0])] {
            steps.extend([scratch(site, &[], 0), Step::new(0x10010, a, &[head as u32], 0)]);
        }
        steps.extend([
            Step::new(0x174, city, &[], 64),
            Step::new(0x0060_3E00, null_mut(), &[0x123400, 64], 0xABCD_EF98).action(move |_, _| unsafe {
                for offset in [0x3C, 0x40, 0x44, 0x48, 0x4C, 0x6C, 0x70] {
                    assert_eq!(word(p, offset), 0);
                }
                assert_eq!(word(node, 8), 0);
            }),
        ]);
        sim.set_state(0);
        script(steps);
        assert_eq!(f.invoke(0), (0xABCD_EF98, 0x7654_3210));
        assert_eq!((sim.state(), sim.ring_index(), sim.changed()), (1, ring as i32, 0));
        for i in 0..10 {
            assert_eq!(unsafe { word(p, 0x270 + i * 4) }, 0x8000_0000);
            assert_eq!(unsafe { word(p, 0x90 + ring * 40 + i * 4) }, 0);
        }
    }
}

#[test]
fn state_one_empty_list_updates_the_full_cost_and_clears_age_requests_and_statistics() {
    let mut f = Fixture::new();
    let sim = f.sim();
    let mut head_obj = Object::new(16);
    let h = head_obj.ptr();
    unsafe { put(h, 0, h as u32) };
    let (mut dept_obj, _dvt) = f.world.object(&[(0x50, line_item as *const () as usize)]);
    let dept = dept_obj.ptr();
    let (mut line_obj, _lvt) = f.world.object(&[(0x30, set_cost as *const () as usize)]);
    let line = line_obj.ptr();
    sim.set_enabled(1);
    sim.set_state(1);
    sim.set_plants(h);
    sim.set_department(dept);
    sim.set_infrastructure_cost(0x89AB_CDEF_0123_4567);
    sim.set_age_plants(1);
    sim.set_overworked(4);
    sim.set_aged(5);
    sim.set_distressed(6);
    script(vec![
        Step::new(0x50, dept, &[1], line as u32),
        Step::new(0x10030, line, &[0x0123_4567, 0x89AB_CDEF], 0xABCD_EF12),
    ]);
    assert_eq!(f.invoke(5).0, 0xABCD_EF12);
    assert_eq!(
        (
            sim.state(),
            sim.age_plants(),
            sim.overworked(),
            sim.aged(),
            sim.distressed()
        ),
        (2, 0, 0, 0, 0)
    );
}

#[test]
fn state_one_seeds_unvisited_plants_defaults_missing_properties_and_ages_in_list_order() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    let sim = f.sim();
    let p = sim.as_ptr();
    let (mut plant_obj, _pvt) = f.world.object(&[
        (0x20, plant_position as *const () as usize),
        (0x24, plant_property as *const () as usize),
    ]);
    let plant = plant_obj.ptr();
    let mut sim_table = [0usize; 6];
    sim_table[5] = convert_position as *const () as usize;
    unsafe { p.cast::<*const usize>().write(sim_table.as_ptr()) };
    let (mut dept_obj, _dvt) = f.world.object(&[(0x50, line_item as *const () as usize)]);
    let dept = dept_obj.ptr();
    let (mut line_obj, _lvt) = f.world.object(&[(0x30, set_cost as *const () as usize)]);
    let line = line_obj.ptr();
    let mut head_obj = Object::new(16);
    let h = head_obj.ptr();
    let mut nodes = [Object::new(16), Object::new(16)];
    let ns = nodes.each_mut().map(Object::ptr);
    unsafe {
        put(h, 0, ns[0] as u32);
        put(ns[0], 0, ns[1] as u32);
        put(ns[1], 0, h as u32);
        for n in ns {
            put(n, 12, plant as u32);
        }
        put(ns[1], 8, 1);
    }
    sim.set_enabled(1);
    sim.set_state(1);
    sim.set_plants(h);
    sim.set_department(dept);
    sim.set_age_plants(1);
    sim.set_infrastructure_cost(u64::MAX);
    let mut steps = vec![
        Step::new(0x10020, plant, &[0], 0)
            .pointer(0)
            .output(0, &[32f32.to_bits(), 0, 48f32.to_bits()]),
        Step::new(0x10014A, p, &[32f32.to_bits(), 48f32.to_bits(), 0, 0], 0)
            .pointer(2)
            .pointer(3)
            .action(|_, args| unsafe {
                (args[2] as *mut u32).write(2);
                (args[3] as *mut u32).write(3);
            }),
        Step::new(0x006E_E110, p, &[2, 3], 0),
    ];
    for i in 0..2 {
        steps.push(Step::new(0x18, plant, &[], plant as u32));
        for (site, id) in [
            (0x005F_D4F0, 0xC910_B4CE),
            (0x005F_D4B0, 0x2A58_D350),
            (0x005F_D450, 0x08D9_763D),
            (0x005F_D4F0, 0xC910_B4CF),
        ] {
            let step = Step::new(site, null_mut(), &[plant as u32, id, 0], 0).pointer(2);
            steps.push(if i == 1 && id == 0xC910_B4CE {
                step.output(2, &[1.25f32.to_bits()])
            } else if i == 1 && id == 0x08D9_763D {
                step.output(2, &[(-3i32) as u32])
            } else {
                step
            });
        }
        steps.push(Step::new(0x10024, plant, &[0x0911_E2E7], 0));
    }
    steps.extend([
        Step::new(0x50, dept, &[1], line as u32),
        Step::new(0x10030, line, &[0xFFFF_FFFC, u32::MAX], 0),
        Step::new(0x006E_C8F0, p, &[plant as u32], 0).action(move |_, _| {
            let s = unsafe { IdleSim::from_ptr(p) };
            assert_eq!((s.age_plants(), s.overworked(), s.aged(), s.distressed()), (0, 0, 0, 0));
            s.set_aged(1);
        }),
        Step::new(0x006E_C8F0, p, &[plant as u32], 0xABCD_EF98),
    ]);
    script(steps);
    assert_eq!(f.invoke(1).0, 0xABCD_EF98);
    assert_eq!((sim.state(), sim.aged()), (2, 1));
}

#[test]
fn state_one_keeps_curve_output_in_st0_and_updates_budget_costs_only_when_needed() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    let sim = f.sim();
    let (mut plant_obj, _pvt) = f.world.object(&[(0x24, plant_property as *const () as usize)]);
    let plant = plant_obj.ptr();
    let (mut budget_obj, _bvt) = f.world.object(&[
        (0xCC, budget_item as *const () as usize),
        (0x70, department as *const () as usize),
        (0xD4, budget_cost as *const () as usize),
    ]);
    let budget = budget_obj.ptr();
    let (mut dept_obj, _dvt) = f.world.object(&[
        (0x50, line_item as *const () as usize),
        (0x64, funded_cost as *const () as usize),
    ]);
    let dept = dept_obj.ptr();
    let (mut line_obj, _lvt) = f.world.object(&[(0x30, set_cost as *const () as usize)]);
    let line = line_obj.ptr();
    let mut head_obj = Object::new(16);
    let h = head_obj.ptr();
    let mut node_obj = Object::new(16);
    let node = node_obj.ptr();
    unsafe {
        put(h, 0, node as u32);
        put(node, 0, h as u32);
        put(node, 8, 1);
        put(node, 12, plant as u32);
    }
    f.globals[3] = budget;
    sim.set_enabled(1);
    sim.set_plants(h);
    sim.set_department(dept);
    sim.set_infrastructure_cost(10);
    for mode in 0..4 {
        sim.set_state(1);
        let mut steps = vec![
            Step::new(0x18, plant, &[], plant as u32),
            Step::new(0x005F_D4F0, null_mut(), &[plant as u32, 0xC910_B4CE, 0], 1)
                .pointer(2)
                .output(2, &[0.75f32.to_bits()]),
            Step::new(0x005F_D4B0, null_mut(), &[plant as u32, 0x2A58_D350, 0], 1)
                .pointer(2)
                .output(2, &[16_777_215, 0]),
            Step::new(0x005F_D450, null_mut(), &[plant as u32, 0x08D9_763D, 0], 1)
                .pointer(2)
                .output(2, &[6]),
            Step::new(0x005F_D4F0, null_mut(), &[plant as u32, 0xC910_B4CF, 0], 1)
                .pointer(2)
                .output(2, &[0.25f32.to_bits()]),
            Step::new(0x10024, plant, &[0x0911_E2E7], 0x4455),
            scratch(0x007D_0410, &[], 0).action(|this, _| unsafe { core::ptr::write_bytes(this as *mut u8, 0, 16) }),
            scratch(0x005F_4620, &[0x4455], 0).action(|this, _| unsafe { put(this as *mut u8, 0, 0x123400) }),
            scratch(0x005F_4250, &[0.25f32.to_bits()], 0).value(1.000_000_059_604_644_8f64.to_bits()),
            Step::new(
                0xCC,
                budget,
                &[plant as u32, 0xCA58_E540, 0],
                if mode == 0 { 0 } else { 1 },
            )
            .pointer(2)
            .output(2, &[7, 0, 0, 0, if mode == 1 { 16_777_216 } else { 0 }, 0, 9, 0]),
        ];
        if mode >= 2 {
            steps.push(Step::new(0x70, budget, &[7], if mode == 2 { 0 } else { dept as u32 }));
        }
        if mode == 3 {
            steps.extend([
                Step::new(0x64, dept, &[plant as u32, 16_777_216, 0, 0xCA58_E540], 0),
                Step::new(0xD4, budget, &[plant as u32, 9, 16_777_216, 0], 0),
            ]);
        }
        steps.extend([
            Step::new(0x0090_CF63, null_mut(), &[0x123400], 0),
            Step::new(0x50, dept, &[1], line as u32),
            Step::new(0x10030, line, &[14, 0], 0xABCD_EF98),
        ]);
        script(steps);
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        unsafe { core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _) };
        let before = environment();
        let result = f.invoke(5);
        let after = environment();
        unsafe { core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _) };
        assert_eq!(result.0, 0xABCD_EF98);
        assert_eq!(before, after);
        assert_eq!(restored, sentinel);
        assert_eq!(sim.state(), 2);
    }
}

#[test]
fn state_two_skips_satisfied_deals_visits_connections_swaps_bitmaps_and_posts_changes() {
    let mut f = Fixture::new();
    let sim = f.sim();
    let p = sim.as_ptr();
    let (mut neighbor_obj, _nvt) = f.world.object(&[
        (0x78, deals as *const () as usize),
        (0x3C, find_connection as *const () as usize),
        (0x34, connection_neighbor as *const () as usize),
    ]);
    let neighbors = neighbor_obj.ptr();
    let (mut deal_obj, _dvt) = f.world.object(&[
        (0x1C, deal_city as *const () as usize),
        (0x50, deal_used as *const () as usize),
        (0x28, deal_max as *const () as usize),
    ]);
    let deal = deal_obj.ptr();
    let (mut connection_obj, _cvt) = f.world.object(&[(0x30, connection_visit as *const () as usize)]);
    let connection = connection_obj.ptr();
    let (mut provider_obj, _pvt) = f.world.object(&[(0x34, available as *const () as usize)]);
    let provider = provider_obj.ptr();
    let (mut message_obj, _mvt) = f.world.object(&[(0x14, set_message_type as *const () as usize)]);
    let message = message_obj.ptr();
    let (mut server_obj, _svt) = f.world.object(&[(0x10, post as *const () as usize)]);
    let server = server_obj.ptr();
    let mut head_obj = Object::new(16);
    let h = head_obj.ptr();
    let mut nodes = [Object::new(16), Object::new(16)];
    let ns = nodes.each_mut().map(Object::ptr);
    unsafe {
        for n in ns {
            put(n, 8, deal as u32);
        }
    }
    f.globals[4] = server;
    sim.set_enabled(1);
    sim.set_neighbors(neighbors);
    for changed in [0, 1] {
        sim.set_state(2);
        sim.set_next_powered(0x1234usize as *mut u8);
        sim.set_current_powered(0x5678usize as *mut u8);
        let mut steps = vec![
            scratch(0x0042_1A90, &[], 0).action(move |this, _| populate(this, h, &[])),
            scratch(0x0042_1660, &[], 0),
            Step::new(0x78, neighbors, &[0x4773_FFE0, 0], 0)
                .pointer(1)
                .action(move |_, args| populate(args[1], h, &ns)),
            Step::new(0x1C, deal, &[], 9),
            Step::new(0x150, deal, &[], 7),
            Step::new(0x128, deal, &[], 7),
            Step::new(0x1C, deal, &[], 11),
            Step::new(0x150, deal, &[], u32::MAX),
            Step::new(0x128, deal, &[], 0),
            Step::new(0x3C, neighbors, &[4, 11], connection as u32),
            Step::new(0x30, connection, &[1], 0),
            Step::new(0x34, neighbors, &[connection as u32], 0),
            Step::new(0x3C, neighbors, &[4, 11], connection as u32),
            Step::new(0x30, connection, &[1], 0),
            Step::new(0x34, neighbors, &[connection as u32], provider as u32),
            Step::new(0x134, provider, &[], 0x100),
            Step::new(0x3C, neighbors, &[4, 11], connection as u32),
            Step::new(0x30, connection, &[1], 0),
            Step::new(0x34, neighbors, &[connection as u32], provider as u32),
            Step::new(0x134, provider, &[], 0x10001),
            Step::new(0x006E_B280, p, &[connection as u32, 0, 0], 0)
                .pointer(1)
                .pointer(2)
                .action(|_, args| unsafe {
                    (args[1] as *mut u32).write(7);
                    (args[2] as *mut u32).write(8);
                }),
            Step::new(0x006E_E110, p, &[7, 8], 0),
            Step::new(0x3C, neighbors, &[4, 11], 0),
            scratch(0x0042_1760, &[], 0),
            Step::new(0x006E_BD00, p, &[], 0xABCD_EF98).action(move |_, _| {
                let s = unsafe { IdleSim::from_ptr(p) };
                assert_eq!(
                    (s.current_powered() as usize, s.next_powered() as usize),
                    (0x1234, 0x5678)
                );
                s.set_changed(changed);
            }),
        ];
        if changed != 0 {
            steps.extend([
                Step::new(0x0091_33DA, null_mut(), &[0x2C], message as u32),
                Step::new(0x0091_34D6, message, &[], message as u32),
                Step::new(4, message, &[], 1),
                Step::new(0x114, message, &[0x6A07_C2EE], 0),
                Step::new(0x110, server, &[message as u32, 0], 0),
                Step::new(8, message, &[], 0xABCD_EF78),
            ]);
        }
        script(steps);
        assert_eq!(f.invoke(5).0, if changed == 0 { 0xABCD_EF98 } else { 0xABCD_EF78 });
        assert_eq!(sim.state(), 3);
    }
}

#[test]
fn state_three_publishes_totals_marks_short_deals_and_reports_clamped_connection_power() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    let sim = f.sim();
    let p = sim.as_ptr();
    let stats = f.stats.ptr();
    let (mut neighbor_obj, _nvt) = f.world.object(&[
        (0x78, deals as *const () as usize),
        (0x44, connections as *const () as usize),
    ]);
    let neighbors = neighbor_obj.ptr();
    let (mut deal_obj, _dvt) = f.world.object(&[
        (0x50, deal_used as *const () as usize),
        (0x28, deal_max as *const () as usize),
        (0x94, distress as *const () as usize),
    ]);
    let deal = deal_obj.ptr();
    let (mut connection_obj, _cvt) = f.world.object(&[
        (0x18, connection_position as *const () as usize),
        (0x38, connection_power as *const () as usize),
    ]);
    let connection = connection_obj.ptr();
    let (mut allocator_obj, _avt) = f.world.object(&[
        (0x0C, allocate_head as *const () as usize),
        (0x10, free_head as *const () as usize),
    ]);
    let allocator = allocator_obj.ptr();
    let (mut advisor_obj, _advt) = f.world.object(&[(0x7C, update_advisor as *const () as usize)]);
    let adv = advisor_obj.ptr();
    let (mut budget_obj, _bvt) = f.world.object(&[(0x8C, department_cost as *const () as usize)]);
    let budget = budget_obj.ptr();
    let (mut city_obj, _city_vt) = f.world.object(&[]);
    let city = city_obj.ptr();
    let mut head_obj = Object::new(16);
    let h = head_obj.ptr();
    let mut conn_head = Object::new(16);
    let ch = conn_head.ptr();
    let mut nodes = [Object::new(16), Object::new(16)];
    let ns = nodes.each_mut().map(Object::ptr);
    unsafe {
        put(ns[0], 8, deal as u32);
        put(ns[1], 8, deal as u32);
    }
    let mut conn_node = Object::new(16);
    let cn = conn_node.ptr();
    unsafe {
        put(cn, 8, connection as u32);
    }
    f.globals[2] = allocator;
    f.globals[5] = adv;
    f.globals[6] = city;
    let mut zero = 0x8000_0000u32;
    let mut one_value = 1.0f64;
    let mut hundred_value = 100.0f64;
    let _a = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let _b = fake_data(0x00A8_0AB0, (&mut one_value as *mut f64).cast());
    let _c = fake_data(0x00A8_0AC0, (&mut hundred_value as *mut f64).cast());
    sim.set_enabled(1);
    sim.set_neighbors(neighbors);
    sim.set_budget(budget);
    sim.set_department(0xABCDusize as *mut u8);
    sim.set_factor(0.75);
    sim.set_ring_index(1);
    for (production, consumption, consumers, powered_count, fraction, expected_deficit, unpowered) in [
        (100, 75, 7, 3, 57, 0u32, 4),
        (-1, -2, 0, 7, 0, (-1.0f32).to_bits(), 0),
        (0, 0, -1, i32::MAX, 0, 0x8000_0000, i32::MIN),
    ] {
        sim.set_state(3);
        sim.set_production(production);
        sim.set_consumption(consumption);
        sim.set_imports(12);
        sim.set_exports(7);
        sim.set_demand(11);
        sim.set_consumers(consumers);
        sim.set_powered(powered_count);
        for i in 0..10 {
            unsafe {
                put(p, 0xB8 + i * 4, (i as u32).wrapping_mul(3));
                put(p, 0x270 + i * 4, u32::MAX);
            }
        }
        let mut steps = vec![
            Step::new(0x130, stats, &[0xAA11_F7CE, production as u32], 0).action(move |_, _| unsafe {
                assert_eq!(word(p, 0x50), production as u32);
                assert_eq!(word(p, 0x54), consumption as u32);
                assert_eq!(word(p, 0x58), 12);
                assert_eq!(word(p, 0x5C), 7);
                assert_eq!(word(p, 0x60), 11);
            }),
            Step::new(0x130, stats, &[0x0AB2_F224, 12], 0),
            Step::new(0x130, stats, &[0x4AB2_F236, 7], 0),
            Step::new(
                0x130,
                stats,
                &[
                    0xAA11_F7D2,
                    if consumption == 75 {
                        51
                    } else if consumption == -2 {
                        (-6i32) as u32
                    } else {
                        (-5i32) as u32
                    },
                ],
                0,
            ),
            Step::new(0x130, stats, &[0xAA11_F7D5, 8], 0),
            Step::new(
                0x130,
                stats,
                &[0xAA11_F7D8, production.wrapping_sub(consumption) as u32],
                0,
            ),
            scratch(0x0042_1A90, &[], 0).action(move |this, _| populate(this, h, &[])),
            Step::new(0x78, neighbors, &[0x4773_FFE1, 0], 0)
                .pointer(1)
                .action(move |_, args| populate(args[1], h, &ns)),
            Step::new(0x150, deal, &[], 5),
            Step::new(0x128, deal, &[], 6),
            Step::new(0x194, deal, &[0x20, 1], 0),
            Step::new(0x150, deal, &[], 5),
            Step::new(0x128, deal, &[], 4),
            scratch(0x0042_1760, &[], 0),
            Step::new(0x7C, adv, &[], 0),
            Step::new(0x8C, budget, &[0xABCD], 0).value((-9i64) as u64),
        ];
        if consumption > 0 {
            steps.extend([
                Step::new(0x009E_FEB0, null_mut(), &[(-9i32) as u32, u32::MAX, 75, 0], u32::MAX).value(u64::MAX),
                Step::new(0x130, stats, &[0xAA11_F7DC, u32::MAX], 0),
            ]);
        }
        steps.push(Step::new(0x130, stats, &[0xAA11_FB59, fraction], 0));
        for (i, id) in [
            0xAA12_0D5E,
            0xAA12_0D61,
            0xAA12_0D63,
            0xAA12_0D65,
            0xAA12_0D66,
            0xAA12_0D68,
            0xAA12_0D6A,
            0xAA12_0D6B,
            0xAA12_0D6D,
        ]
        .into_iter()
        .enumerate()
        {
            steps.push(Step::new(0x130, stats, &[id, (i as u32 + 1) * 3], 0));
        }
        steps.extend([
            Step::new(0x0C, allocator, &[12], ch as u32),
            Step::new(0x44, neighbors, &[4, 0], 0)
                .pointer(1)
                .action(move |_, args| populate(args[1], ch, &[cn])),
            Step::new(0x174, city, &[], 16),
            Step::new(0x178, city, &[], 32),
            Step::new(0x118, connection, &[0, 0], 0)
                .pointer(0)
                .pointer(1)
                .action(|_, args| unsafe {
                    (args[0] as *mut u32).write((-3i32) as u32);
                    (args[1] as *mut u32).write(800);
                }),
            Step::new(0x94, p, &[0, 31], 0xABCD_EF80),
            Step::new(0x138, connection, &[0xABCD_EF80], 0),
            scratch(0x0041_1D70, &[], 0),
            Step::new(0x10010, allocator, &[ch as u32], 0),
            Step::new(0x005F_98C0, null_mut(), &[0xEA41_8D8F, 8, 0, 0, 0, 0], 0xABCD_EF12)
                .action(move |_, _| assert_eq!(unsafe { IdleSim::from_ptr(p) }.state(), 4)),
        ]);
        script(steps);
        assert_eq!(f.invoke(5), (0xABCD_EF12, 0x7654_3210));
        assert_eq!(sim.deficit(), expected_deficit);
        assert_eq!(sim.unpowered(), unpowered.max(0));
        for i in 0..10 {
            assert_eq!(
                unsafe { word(p, 0x270 + i * 4) },
                (i as u32).wrapping_mul(3).wrapping_sub(1)
            );
        }
    }
}
