// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Message cases moved from the power core, with scripted game calls at the original layout.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

type Action = Box<dyn FnOnce(u32, &[u32])>;
struct Step {
    site: u32,
    this: Option<u32>,
    args: Vec<Option<u32>>,
    result: u64,
    action: Option<Action>,
    return_this: bool,
}
impl Step {
    fn new(site: u32, this: *mut u8, args: &[u32], eax: u32) -> Self {
        Self {
            site,
            this: Some(this as u32),
            args: args.iter().copied().map(Some).collect(),
            result: (0x7654_3210u64 << 32) | u64::from(eax),
            action: None,
            return_this: false,
        }
    }
    fn pointer(mut self, arg: usize) -> Self {
        self.args[arg] = None;
        self
    }
    fn scratch(mut self) -> Self {
        self.this = None;
        self
    }
    fn returns_this(mut self) -> Self {
        self.return_this = true;
        self
    }
    fn action(mut self, action: impl FnOnce(u32, &[u32]) + 'static) -> Self {
        self.action = Some(Box::new(action));
        self
    }
    fn output(self, arg: usize, words: &[u32]) -> Self {
        let words = words.to_vec();
        self.action(move |_, args| {
            for (i, value) in words.iter().enumerate() {
                unsafe { (args[arg] as *mut u32).add(i).write_unaligned(*value) };
            }
        })
    }
}
thread_local! {
    static STEPS: RefCell<VecDeque<Step>> = const { RefCell::new(VecDeque::new()) };
    static ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}
fn run(site: u32, this: u32, args: &[u32]) -> u64 {
    let mut step = STEPS.with(|s| s.borrow_mut().pop_front()).expect("unexpected call");
    if step.site != site
        || step.this.is_some_and(|p| p != this)
        || step.args.len() != args.len()
        || step
            .args
            .iter()
            .zip(args)
            .any(|(expected, value)| expected.is_some_and(|e| e != *value))
    {
        ERRORS.with(|e| {
            e.borrow_mut().push(format!(
                "expected {:x} {:?} {:?}, got {site:x} {this:x} {args:x?}",
                step.site, step.this, step.args
            ))
        });
    }
    if let Some(action) = step.action.take() {
        action(this, args);
    }
    if step.return_this {
        (step.result & 0xFFFF_FFFF_0000_0000) | u64::from(this)
    } else {
        step.result
    }
}
fn script(steps: Vec<Step>) {
    STEPS.with(|s| {
        assert!(s.borrow().is_empty());
        *s.borrow_mut() = steps.into();
    });
    ERRORS.with(|e| assert!(e.borrow().is_empty()));
}
fn finish() {
    let remaining = STEPS.with(|s| s.borrow().len());
    let errors = ERRORS.with(|e| std::mem::take(&mut *e.borrow_mut()));
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(remaining, 0, "unconsumed calls");
}
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
method!(qi, 0, iid, out);
method!(retain, 4);
method!(release_slot, 8);
method!(begin, 0x0C);
method!(kind, 0x10);
method!(tool_id, 0x14);
method!(manager_tool, 0x10014, network, create);
method!(properties, 0x18);
method!(lot_at, 0x10018, x, z, flags);
method!(demolish, 0x1C, mode, list, a, b, c, d, e, f);
method!(deal_type, 0x20);
method!(data1, 0x24);
method!(unknown1, 0x28);
method!(zone_tool, 0x10028, x0, z0, x1, z1, data);
method!(data2, 0x38);
method!(lot_bounds, 0x10038, rect);
method!(lot_for_occupant, 0x20038, occupant);
method!(rectangle, 0x3C);
method!(data3, 0x4C);
method!(data4, 0x60);
method!(cancel, 0x68, flags);
method!(unknown, 0x80);
method!(pole_flag, 0x10080, flag);
method!(lot_state, 0x84);
method!(powered, 0x94, x, z);
method!(lot_power, 0x10C, on);
method!(width, 0x174);
method!(height, 0x178);
method!(idle_fake, 0x006E_E960, budget);
method!(monthly_fake, 0x006E_C570);
method!(tuning_fake, 0x006E_D1C0);
method!(inserted_fake, 0x006E_CC50, occupant);
method!(removed_fake, 0x006E_BF60, occupant);
method!(costs_fake, 0x006E_B490);
method!(cell_fake, 0x006E_B350, x0, z0, x1, z1);
method!(terrain_fake, 0x006E_DDA0, low, high);
method!(erase_fake, 0x0064_5A60, position);
method!(list_new_fake, 0x004F_5A00, allocator);
method!(ref_new_fake, 0x004F_20C0, object);
method!(list_push_fake, 0x007E_98C0, reference);
method!(list_delete_fake, 0x0042_7670);
method!(ref_out_fake, 0x004F_20A0);
function!(manager_fake, 0x0060_2680);
function!(property_fake, 0x005F_D480, object, id, out);

struct World {
    sim: Object,
    message: Object,
    _table: Box<[usize; 0x17C / 4]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let mut table = Box::new([0usize; 0x17C / 4]);
        for (offset, entry) in [
            (0, qi as *const () as usize),
            (4, retain as *const () as usize),
            (8, release_slot as *const () as usize),
            (0x0C, begin as *const () as usize),
            (0x10, kind as *const () as usize),
            (0x14, tool_id as *const () as usize),
            (0x18, properties as *const () as usize),
            (0x20, deal_type as *const () as usize),
            (0x24, data1 as *const () as usize),
            (0x28, unknown1 as *const () as usize),
            (0x38, data2 as *const () as usize),
            (0x3C, rectangle as *const () as usize),
            (0x4C, data3 as *const () as usize),
            (0x60, data4 as *const () as usize),
            (0x68, cancel as *const () as usize),
            (0x80, unknown as *const () as usize),
            (0x84, lot_state as *const () as usize),
            (0x94, powered as *const () as usize),
            (0x10C, lot_power as *const () as usize),
            (0x174, width as *const () as usize),
            (0x178, height as *const () as usize),
        ] {
            table[offset / 4] = entry;
        }
        let mut sim = Object::new(0x400);
        let mut message = Object::new(16);
        unsafe {
            sim.ptr().cast::<*const usize>().write(table.as_ptr());
            message.ptr().cast::<*const usize>().write(table.as_ptr());
        }
        let fakes = [
            (0x006E_E960, idle_fake as *const () as usize),
            (0x006E_C570, monthly_fake as *const () as usize),
            (0x006E_D1C0, tuning_fake as *const () as usize),
            (0x006E_CC50, inserted_fake as *const () as usize),
            (0x006E_BF60, removed_fake as *const () as usize),
            (0x006E_B490, costs_fake as *const () as usize),
            (0x006E_B350, cell_fake as *const () as usize),
            (0x006E_DDA0, terrain_fake as *const () as usize),
            (0x0064_5A60, erase_fake as *const () as usize),
            (0x0060_2680, manager_fake as *const () as usize),
            (0x004F_5A00, list_new_fake as *const () as usize),
            (0x004F_20C0, ref_new_fake as *const () as usize),
            (0x007E_98C0, list_push_fake as *const () as usize),
            (0x0042_7670, list_delete_fake as *const () as usize),
            (0x004F_20A0, ref_out_fake as *const () as usize),
            (0x005F_D480, property_fake as *const () as usize),
        ]
        .map(|(address, entry)| fake(address, entry))
        .into();
        Self {
            sim,
            message,
            _table: table,
            _fakes: fakes,
        }
    }
    fn object(&self, overrides: &[(usize, usize)]) -> (Object, Box<[usize; 0x17C / 4]>) {
        let mut table = self._table.clone();
        for &(offset, entry) in overrides {
            table[offset / 4] = entry;
        }
        let mut object = Object::new(16);
        unsafe { object.ptr().cast::<*const usize>().write(table.as_ptr()) };
        (object, table)
    }
    fn invoke(&mut self) -> (u32, u32) {
        let args = [self.message.ptr() as u32];
        let mut c = RawCall::new(
            Conv::Thiscall,
            fc4p_006efb70 as *const () as usize,
            unsafe { self.sim.ptr().add(4) } as u32,
            &args,
        );
        c.edx = 0xAABB_CCDD;
        unsafe { call_raw(&mut c) };
        finish();
        (c.eax, c.edx)
    }
}

#[test]
fn connections_and_unknown_ids_keep_upper_return_bytes() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    for id in [
        0,
        u32::MAX,
        0x6695_6815,
        0x6695_6817,
        0x6695_6819,
        0x6695_681B,
        0x6AC2_84F2,
        0x6AC2_84F4,
    ] {
        script(vec![Step::new(0x10, m, &[], id)]);
        let expected = if id > 0x6695_6814 && id < 0x6AC2_84F3 {
            id.wrapping_sub(0x6695_681A)
        } else {
            id
        };
        let before = unsafe { std::slice::from_raw_parts(p, 0x400) }.to_vec();
        assert_eq!(w.invoke(), ((expected & !255) | 1, 0x7654_3210));
        assert_eq!(unsafe { std::slice::from_raw_parts(p, 0x400) }, before);
    }
    script(vec![Step::new(0x10, m, &[], 0x6AC2_84F3)]);
    assert_eq!(w.invoke(), (0x6AC2_8401, 0x7654_3210));
    assert_eq!(unsafe { MessageSim::from_ptr(p) }.enabled(), 1);
    assert_eq!(unsafe { p.add(0x13).read() }, 0);
    assert_eq!(unsafe { p.add(0x15).read() }, 0);
}

#[test]
fn scheduled_and_occupant_messages_call_the_original_entry_with_primary_this() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    for (id, site, query_slot, args) in [
        (0x6695_6814, 0x006E_E960, None, vec![5]),
        (0x6695_6816, 0x006E_C570, None, vec![]),
        (0x6695_681A, 0x006E_E960, Some(0x4C), vec![u32::MAX]),
        (0xCA1B_151C, 0x006E_D1C0, None, vec![]),
        (0x99EF_1142, 0x006E_CC50, Some(0x28), vec![0]),
        (0x99EF_1143, 0x006E_BF60, Some(0x28), vec![0x1234]),
    ] {
        let mut steps = vec![Step::new(0x10, m, &[], id)];
        if let Some(slot) = query_slot {
            steps.push(Step::new(slot, m, &[], args[0]));
        }
        steps.push(Step::new(site, p, &args, 0xABCD_EF80));
        script(steps);
        assert_eq!(w.invoke(), (0xABCD_EF01, 0x7654_3210));
    }
}

#[test]
fn terrain_ignores_modes_four_and_five_and_clamps_each_bound_in_its_own_direction() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let (mut city, _vt) = w.object(&[]);
    let city = city.ptr();
    unsafe { MessageSim::from_ptr(p) }.set_city(city);
    for mode in [4, 5] {
        script(vec![
            Step::new(0x10, m, &[], 0x278E_6003),
            Step::new(0x60, m, &[], mode),
        ]);
        assert_eq!(w.invoke().0, 1);
    }
    for (x, z, width, height, expected) in [
        (0x0000_FFFF, 0x0000_FFFF, 16u32, 32u32, [0, 0, 15, 31]),
        (0x0064_0002, 0x0032_0003, 16, 32, [99, 49, 2, 3]),
        (0x0000_0000, 0x0000_0000, 0, 0, [0, 0, u32::MAX, u32::MAX]),
    ] {
        script(vec![
            Step::new(0x10, m, &[], 0x278E_6003),
            Step::new(0x60, m, &[], 0),
            Step::new(0x24, m, &[], x),
            Step::new(0x38, m, &[], z),
            Step::new(0x174, city, &[], width),
            Step::new(0x178, city, &[], height),
            Step::new(0x006E_DDA0, p, &[0, 0], 0xABCD_EF00)
                .pointer(0)
                .pointer(1)
                .action(move |_, args| {
                    assert_eq!(args[1], args[0] + 8);
                    assert_eq!(
                        unsafe { std::slice::from_raw_parts(args[0] as *const u32, 4) },
                        expected
                    );
                }),
        ]);
        assert_eq!(w.invoke(), (0xABCD_EF01, 0x7654_3210));
    }
}

#[test]
fn zoning_orders_tool_lifetime_and_copies_the_rectangle_again_after_cell_changed() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let (mut manager, _manager_vt) = w.object(&[(0x14, manager_tool as *const () as usize)]);
    let manager = manager.ptr();
    let (mut tool, _tool_vt) = w.object(&[(0x28, zone_tool as *const () as usize)]);
    let tool = tool.ptr();
    let mut rect = [1u32, 2, 3, 4];
    let rectp = rect.as_mut_ptr() as usize;
    script(vec![
        Step::new(0x10, m, &[], 0x0769_9455),
        Step::new(0x3C, m, &[], rectp as u32),
        Step::new(0x0060_2680, core::ptr::null_mut(), &[], manager as u32),
        Step::new(0x10014, manager, &[5, 0], tool as u32),
        Step::new(4, tool, &[], 1),
        Step::new(0x0C, tool, &[], 0),
        Step::new(0x006E_B350, p, &[1, 2, 3, 4], 0).action(move |_, _| unsafe { (rectp as *mut u32).add(2).write(9) }),
        Step::new(0x4C, m, &[], 77),
        Step::new(0x10028, tool, &[1, 2, 9, 4, 77], 0),
        Step::new(0x10, tool, &[], 0),
        Step::new(8, tool, &[], 0xABCD_EF08),
    ]);
    assert_eq!(w.invoke(), (0xABCD_EF01, 0x7654_3210));
}

#[test]
fn pole_cleanup_queries_each_reference_erases_in_place_and_reloads_the_end() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let sim = unsafe { MessageSim::from_ptr(p) };
    let (mut pole, _pole_vt) = w.object(&[]);
    let pole = pole.ptr();
    let (mut interface, _interface_vt) = w.object(&[(0x80, pole_flag as *const () as usize)]);
    let interface = interface.ptr();
    let mut poles = [pole as u32; 3];
    let base = poles.as_mut_ptr() as usize;
    sim.set_poles_begin(base as *mut u8);
    sim.set_poles_end((base + 12) as *mut u8);
    sim.set_pole_count(0);
    script(vec![
        Step::new(0x10, m, &[], 0x26D3_1EC1),
        Step::new(0, pole, &[0x26BB_767E, 0], 0xDEAD_BF00).pointer(1),
        Step::new(0, pole, &[0x26BB_767E, 0], 1)
            .pointer(1)
            .output(1, &[interface as u32]),
        Step::new(0x10080, interface, &[1], 0xFF00),
        Step::new(8, pole, &[], 0),
        Step::new(
            0x0064_5A60,
            unsafe { p.add(0x29C) },
            &[(base + 4) as u32],
            (base + 4) as u32,
        )
        .action(move |this, _| unsafe { put(this as *mut u8, 4, (base + 8) as u32) }),
        Step::new(8, interface, &[], 0),
        Step::new(0, pole, &[0x26BB_767E, 0], 1)
            .pointer(1)
            .output(1, &[interface as u32]),
        Step::new(0x10080, interface, &[1], 0x10001),
        Step::new(8, interface, &[], 0),
        Step::new(0x006E_B490, p, &[], 0xABCD_EF01),
    ]);
    assert_eq!(w.invoke(), (0xABCD_EF01, 0x7654_3210));
    assert_eq!(sim.pole_count(), u32::MAX);
    assert_eq!(sim.recompute(), 1);
    for value in [0xABCD_EF00, 0xABCD_EF80] {
        sim.set_poles_end(sim.poles_begin());
        sim.set_recompute(0);
        script(vec![
            Step::new(0x10, m, &[], 0x26D3_1EC1),
            Step::new(0x006E_B490, p, &[], value),
        ]);
        assert_eq!(w.invoke().0, 0xABCD_EF01);
        assert_eq!(sim.recompute(), u8::from(value as u8 != 0));
    }
}

#[test]
fn deal_status_queries_types_in_order_and_releases_even_a_failed_query_output() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let sim = unsafe { MessageSim::from_ptr(p) };
    let (mut deal, _vt) = w.object(&[]);
    let deal = deal.ptr();
    for (types, queried, out, changed) in [
        (vec![0x4773_FFE1], 1, deal, true),
        (vec![0x1234, 0x4773_FFE0], 1, deal, true),
        (vec![0x4773_FFE0, 0x1234], 1, deal, false),
        (vec![], 0, deal, false),
        (vec![], 0, core::ptr::null_mut(), false),
    ] {
        sim.set_state(2);
        sim.set_recompute(0);
        let mut steps = vec![
            Step::new(0x10, m, &[], 0xEA41_8D8F),
            Step::new(0x24, m, &[], 0),
            Step::new(0x80, m, &[], deal as u32),
            Step::new(0x004F_20A0, core::ptr::null_mut(), &[], 0)
                .scratch()
                .returns_this(),
            Step::new(0, deal, &[0x0773_FEF1, 0], queried)
                .pointer(1)
                .output(1, &[out as u32]),
        ];
        for value in types {
            steps.push(Step::new(0x20, deal, &[], value));
        }
        if !out.is_null() {
            steps.push(Step::new(8, out, &[], 0xABCD_EF00));
        }
        script(steps);
        assert_eq!(w.invoke().0, if out.is_null() { 1 } else { 0xABCD_EF01 });
        assert_eq!((sim.state(), sim.recompute()), if changed { (4, 1) } else { (2, 0) });
    }
    script(vec![
        Step::new(0x10, m, &[], 0xEA41_8D8F),
        Step::new(0x24, m, &[], 0xABCD_EF02),
    ]);
    assert_eq!(w.invoke().0, 0xABCD_EF01);
    script(vec![
        Step::new(0x10, m, &[], 0xEA41_8D8F),
        Step::new(0x24, m, &[], 0),
        Step::new(0x80, m, &[], 0),
    ]);
    assert_eq!(w.invoke().0, 1);
}

#[test]
fn lot_state_uses_the_second_unknown_and_wrapping_signed_centres() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let (mut first, _first_vt) = w.object(&[]);
    let first = first.ptr();
    let (mut lot, _lot_vt) = w.object(&[(0x38, lot_bounds as *const () as usize)]);
    let lot = lot.ptr();
    let (mut manager, _manager_vt) = w.object(&[(0x18, lot_at as *const () as usize)]);
    let mut manager = manager.ptr();
    let _global = fake_data(0x00B4_3D08, (&mut manager as *mut *mut u8).cast());
    for (rect, centre) in [
        ([2u32, 4, 7, 11], [4, 7]),
        ([(-4i32) as u32, (-8i32) as u32, 1, 3], [(-1i32) as u32, (-2i32) as u32]),
        (
            [i32::MAX as u32, i32::MIN as u32, 1, u32::MAX],
            [(-1_073_741_824i32) as u32, 1_073_741_823],
        ),
    ] {
        script(vec![
            Step::new(0x10, m, &[], 0xE8BD_A2E7),
            Step::new(0x80, m, &[], first as u32),
            Step::new(0x80, m, &[], lot as u32),
            Step::new(0, lot, &[0xE818_FCED, 0], 1)
                .pointer(1)
                .output(1, &[lot as u32]),
            Step::new(0x84, lot, &[], 1),
            Step::new(0x10038, lot, &[0], 0).pointer(0).output(0, &rect),
            Step::new(0x94, p, &centre, 0x10001),
            Step::new(0x10018, manager, &[centre[0], centre[1], 0], first as u32),
            Step::new(0x10C, first, &[1], 0),
            Step::new(8, lot, &[], 0xABCD_EF00),
        ]);
        assert_eq!(w.invoke(), (0xABCD_EF01, 0x7654_3210));
    }
}

#[test]
fn lot_state_stops_on_null_failed_query_wrong_state_unpowered_and_missing_lot() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let (mut lot, _lot_vt) = w.object(&[(0x38, lot_bounds as *const () as usize)]);
    let lot = lot.ptr();
    let (mut manager, _manager_vt) = w.object(&[(0x18, lot_at as *const () as usize)]);
    let mut manager = manager.ptr();
    let _global = fake_data(0x00B4_3D08, (&mut manager as *mut *mut u8).cast());
    script(vec![Step::new(0x10, m, &[], 0xE8BD_A2E7), Step::new(0x80, m, &[], 0)]);
    assert_eq!(w.invoke().0, 1);
    script(vec![
        Step::new(0x10, m, &[], 0xE8BD_A2E7),
        Step::new(0x80, m, &[], lot as u32),
        Step::new(0x80, m, &[], lot as u32),
        Step::new(0, lot, &[0xE818_FCED, 0], 0xABCD_EF00).pointer(1),
    ]);
    assert_eq!(w.invoke().0, 0xABCD_EF01);
    for mode in [0, 1, 2] {
        let mut steps = vec![
            Step::new(0x10, m, &[], 0xE8BD_A2E7),
            Step::new(0x80, m, &[], lot as u32),
            Step::new(0x80, m, &[], lot as u32),
            Step::new(0, lot, &[0xE818_FCED, 0], 1)
                .pointer(1)
                .output(1, &[lot as u32]),
            Step::new(0x84, lot, &[], if mode == 0 { 2 } else { 1 }),
        ];
        if mode != 0 {
            steps.push(Step::new(0x10038, lot, &[0], 0).pointer(0).output(0, &[0; 4]));
            steps.push(Step::new(0x94, p, &[0, 0], if mode == 1 { 0xABCD_EF00 } else { 1 }));
            if mode == 2 {
                steps.push(Step::new(0x10018, manager, &[0, 0, 0], 0));
            }
        }
        steps.push(Step::new(8, lot, &[], 0xABCD_EF00));
        script(steps);
        assert_eq!(w.invoke().0, 0xABCD_EF01);
    }
}

#[test]
fn nuclear_free_zone_filters_properties_builds_the_lot_list_and_cancels_only_the_matching_tool() {
    let mut w = World::new();
    let p = w.sim.ptr();
    let m = w.message.ptr();
    let sim = unsafe { MessageSim::from_ptr(p) };
    let (mut occupant, _occupant_vt) = w.object(&[]);
    let occupant = occupant.ptr();
    let (mut lot, _lot_vt) = w.object(&[]);
    let lot = lot.ptr();
    let (mut manager, _manager_vt) = w.object(&[(0x38, lot_for_occupant as *const () as usize)]);
    let mut manager = manager.ptr();
    let (mut demolisher, _demolition_vt) = w.object(&[(0x1C, demolish as *const () as usize)]);
    let mut demolisher = demolisher.ptr();
    let (mut tool_manager, _tools_vt) = w.object(&[]);
    let mut tool_manager = tool_manager.ptr();
    let (mut tool, _tool_vt) = w.object(&[]);
    let tool = tool.ptr();
    let _a = fake_data(0x00B4_3D08, (&mut manager as *mut *mut u8).cast());
    let _b = fake_data(0x00B4_3D00, (&mut demolisher as *mut *mut u8).cast());
    let _c = fake_data(0x00B4_3DC4, (&mut tool_manager as *mut *mut u8).cast());
    let mut head = Object::new(16);
    let h = head.ptr();
    let mut nodes = [Object::new(16), Object::new(16), Object::new(16)];
    let ptrs = nodes.each_mut().map(Object::ptr);
    unsafe {
        put(h, 0, ptrs[0] as u32);
        for i in 0..3 {
            put(ptrs[i], 0, if i == 2 { h } else { ptrs[i + 1] } as u32);
            put(ptrs[i], 12, occupant as u32);
        }
    }
    sim.set_plants(h);
    for current_tool in [0, 0x1234_5678, 0x88F1_54FB] {
        let mut steps = vec![
            Step::new(0x10, m, &[], 0xC2C0_9554),
            Step::new(0x38, m, &[], 0xE0D0_722E),
            Step::new(0x4C, m, &[], 1),
            Step::new(0x004F_5A00, core::ptr::null_mut(), &[0], 0)
                .scratch()
                .pointer(0)
                .action(|this, args| assert_eq!(this, args[0])),
        ];
        for (present, value) in [(0, 5), (1, 4), (1, 5)] {
            steps.push(Step::new(0x18, occupant, &[], 0x1234));
            steps.push(
                Step::new(0x005F_D480, core::ptr::null_mut(), &[0x1234, 0x2781_2853, 0], present)
                    .pointer(2)
                    .output(2, &[value]),
            );
            if present != 0 && value == 5 {
                steps.extend([
                    Step::new(0x20038, manager, &[occupant as u32], lot as u32),
                    Step::new(0x004F_20C0, core::ptr::null_mut(), &[lot as u32], 0)
                        .scratch()
                        .action(|this, args| unsafe { put(this as *mut u8, 0, args[0]) }),
                    Step::new(0x007E_98C0, core::ptr::null_mut(), &[0], 0)
                        .scratch()
                        .pointer(0)
                        .action(move |_, args| assert_eq!(unsafe { word(args[0] as *mut u8, 0) }, lot as u32)),
                    Step::new(8, lot, &[], 0),
                ]);
            }
        }
        steps.push(Step::new(0x1C, demolisher, &[1, 0, 0, 1, 0, 0, 0, 0], 0).pointer(1));
        steps.push(Step::new(
            0x60,
            tool_manager,
            &[],
            if current_tool == 0 { 0 } else { tool as u32 },
        ));
        if current_tool != 0 {
            steps.push(Step::new(0x14, tool, &[], current_tool));
            if current_tool == 0x88F1_54FB {
                steps.push(Step::new(0x68, tool_manager, &[0], 0));
            }
        }
        steps.push(Step::new(0x0042_7670, core::ptr::null_mut(), &[], 0xABCD_EF00).scratch());
        script(steps);
        assert_eq!(w.invoke(), (0xABCD_EF01, 0x7654_3210));
    }
    for (factor, on) in [(0x1234, None), (0xE0D0_722E, Some(0))] {
        let mut steps = vec![Step::new(0x10, m, &[], 0xC2C0_9554), Step::new(0x38, m, &[], factor)];
        if let Some(value) = on {
            steps.push(Step::new(0x4C, m, &[], value));
        }
        script(steps);
        assert_eq!(w.invoke().0, if on.is_some() { 1 } else { 0x1201 });
    }
}
