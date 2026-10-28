// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Search setup cases from the core's forced-start and trip-routing tests.

use super::*;
use crate::testing::{fake, fake_data, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Call(u32, usize, Vec<u32>);
#[derive(Default)]
struct State {
    calls: Vec<Call>, lot: usize, source: [i32; 4], rectangle: [i32; 4], swap: Option<(usize, usize)>,
    steps: Vec<[u8; 12]>, position: usize, iterator: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(this: *mut u8, slot: usize, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(unsafe { word(this, 4) }, slot, args.to_vec())));
}
macro_rules! no_args {
    ($name:ident, $slot:literal) => { unsafe extern "thiscall" fn $name(this: *mut u8) { record(this, $slot, &[]); } };
}
macro_rules! one_arg {
    ($name:ident, $slot:literal) => { unsafe extern "thiscall" fn $name(this: *mut u8, arg: u32) { record(this, $slot, &[arg]); } };
}
macro_rules! rect_arg {
    ($name:ident, $slot:literal) => { unsafe extern "thiscall" fn $name(this: *mut u8, x0: i32, z0: i32, x1: i32, z1: i32) { record(this, $slot, &[x0 as u32, z0 as u32, x1 as u32, z1 as u32]); } };
}
no_args!(clear_flags, 0x1C); no_args!(clear_destinations, 0x30); no_args!(clear_travel, 0x3C);
one_arg!(flag, 0x14); one_arg!(filter, 0x24); one_arg!(destination, 0x28);
one_arg!(costs, 0x40); one_arg!(length, 0x44); one_arg!(time, 0x54);
rect_arg!(start, 0x4C); rect_arg!(end, 0x50);
unsafe extern "thiscall" fn density(this: *mut u8, count: u32) {
    record(this, 0x48, &[count]);
    STATE.with(|s| { if let Some((context, pf)) = s.borrow_mut().swap.take() {
        unsafe { TripContext::from_ptr(context as *mut u8).set_outbound(pf as *mut u8); }
    }});
}
unsafe extern "thiscall" fn travel(this: *mut u8, mask: u32, value: u32) { record(this, 0x34, &[mask, value]); }
unsafe extern "thiscall" fn source(this: *mut u8, out: *mut i32) -> *mut i32 {
    record(this, 0x14, &[]);
    STATE.with(|s| { let mut s = s.borrow_mut(); unsafe { write_unaligned(out.cast::<[i32; 4]>(), s.source); } s.source.as_mut_ptr() })
}
unsafe extern "thiscall" fn get_lot(this: *mut u8, x: i32, z: i32, flag: u32) -> *mut u8 {
    record(this, 0x18, &[x as u32, z as u32, flag]); STATE.with(|s| s.borrow().lot as *mut u8)
}
unsafe extern "thiscall" fn rectangle(this: *mut u8, out: *mut i32) {
    record(this, 0x38, &[]); STATE.with(|s| unsafe { write_unaligned(out.cast::<[i32; 4]>(), s.borrow().rectangle); });
}
fn pf_table() -> Vec<usize> {
    table(&[
        (0x14, flag as *const () as usize), (0x1C, clear_flags as *const () as usize),
        (0x24, filter as *const () as usize), (0x28, destination as *const () as usize),
        (0x30, clear_destinations as *const () as usize), (0x34, travel as *const () as usize),
        (0x3C, clear_travel as *const () as usize), (0x40, costs as *const () as usize),
        (0x44, length as *const () as usize), (0x48, density as *const () as usize),
        (0x4C, start as *const () as usize), (0x50, end as *const () as usize), (0x54, time as *const () as usize),
    ])
}

unsafe extern "thiscall" fn begin_route(this: *mut u8, route: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); s.position = 0; s.iterator = this as usize;
        s.calls.push(Call(99, 0x006D_64B0, vec![route as u32]));
    });
    0x1357_2468_AABB_CCDD
}
unsafe extern "thiscall" fn next_route(this: *mut u8, step: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); assert_eq!(s.iterator, this as usize);
        let position = s.position; s.position += 1;
        s.calls.push(Call(99, 0x006D_6510, vec![]));
        let available = if let Some(value) = s.steps.get(position) {
            unsafe { write_unaligned(step.cast::<[u8; 12]>(), *value); } true
        } else { false };
        (0x89AB_CDEFu64 << 32) | (0xA1B2_C300 | u64::from(available))
    })
}
#[unsafe(naked)]
unsafe extern "C" fn next_route_entry() {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}", "mov ecx, 0x12345678", "ret 4", body = sym next_route);
}
fn step(x: i16, z: i16, from: u8, to: u8, entry: u8, exit: u16) -> [u8; 12] {
    let mut step = [0u8; 12]; step[..2].copy_from_slice(&x.to_le_bytes()); step[2..4].copy_from_slice(&z.to_le_bytes());
    step[4] = from; step[5] = to; step[7] = entry; step[8..10].copy_from_slice(&exit.to_le_bytes()); step
}
unsafe fn confirm(sim: *mut u8, steps: Vec<[u8; 12]>) -> Registers {
    STATE.with(|s| *s.borrow_mut() = State { steps, ..State::default() });
    unsafe { invoke(fc4p_0070e320 as *const () as usize, sim, &[0x1122_3344], 0, 0, false) }
}

#[test]
fn confirm_route_core_example_rotations_tunnels_ferries_strikes_and_switches() {
    let _begin = fake(0x006D_64B0, begin_route as *const () as usize);
    let _next = fake(0x006D_6510, next_route_entry as *const () as usize);
    let mut sim_mem = Object::new(0xD00); let mut connections = vec![0u8; 46 * 1028];
    unsafe {
        let sim = TrafficSim::from_ptr(sim_mem.ptr()); sim.set_max_cell_x(3); sim.set_max_cell_z(3);
        sim.set_intersections(connections.as_mut_ptr());
        let first = connections.as_mut_ptr().add(46 * ((2 << 8) | 3));
        let second = connections.as_mut_ptr().add(46 * ((3 << 8) | 3));
        let route = vec![step(2, 3, 1, 1, 255, 2), step(3, 3, 1, 1, 0, 2), step(4, 3, 1, 1, 0, 255)];
        assert_eq!(confirm(sim_mem.ptr(), vec![]), Registers { eax: 0xA1B2_C300, ecx: 0x1234_5678, edx: 0x89AB_CDEF });
        let failed = confirm(sim_mem.ptr(), route.clone());
        assert_eq!(failed, Registers { eax: 0x4000, ecx: 0, edx: 0x89AB_CD01 });
        write_unaligned(second.add(2).cast::<u16>(), 1 << 2);
        for bit in [2, 6, 10, 14] {
            write_unaligned(first.add(2).cast::<u16>(), 1 << bit);
            assert_eq!(confirm(sim_mem.ptr(), route.clone()), Registers { eax: 0xA1B2_C301, ecx: 0x1234_5678, edx: 0x89AB_CDEF });
        }
        write_unaligned(first.add(2).cast::<u16>(), 1 << 3);
        assert_eq!(confirm(sim_mem.ptr(), route.clone()).eax as u8, 0);
        write_unaligned(first.add(2).cast::<u16>(), 1 << 6);
        write_unaligned(second.add(2).cast::<u16>(), 1 << 6);
        assert_eq!(confirm(sim_mem.ptr(), route.clone()).eax as u8, 0);
        second.add(44).write(2);
        assert_eq!(confirm(sim_mem.ptr(), route.clone()).eax as u8, 1);
        second.add(44).write(4);
        assert_eq!(confirm(sim_mem.ptr(), route.clone()).eax as u8, 1);
        sim.set_strike_state(1);
        assert_eq!(confirm(sim_mem.ptr(), route.clone()).eax as u8, 0);
        write_unaligned(second.add(2).cast::<u16>(), 1 << 2);
        assert_eq!(confirm(sim_mem.ptr(), route).eax as u8, 1);
        let changing = vec![step(2, 3, 0, 0, 255, 2), step(3, 3, 0, 2, 0, 2)];
        write_unaligned(first.cast::<u16>(), 1 << 2); write_unaligned(second.add(18).cast::<u16>(), 0xF2FF);
        assert_eq!(confirm(sim_mem.ptr(), changing.clone()).eax as u8, 0);
        sim.set_strike_state(0);
        assert_eq!(confirm(sim_mem.ptr(), changing.clone()).eax as u8, 1);
        write_unaligned(second.add(18).cast::<u16>(), 0xFFFF);
        assert_eq!(confirm(sim_mem.ptr(), changing).eax as u8, 0);
        sim.set_strike_state(1);
        write_unaligned(first.add(18).cast::<u16>(), 0xF2FF);
        assert_eq!(confirm(sim_mem.ptr(), vec![step(2, 3, 0, 2, 255, 2)]).eax as u8, 1);
        write_unaligned(first.add(18).cast::<u16>(), 0xF3FF);
        assert_eq!(confirm(sim_mem.ptr(), vec![step(2, 3, 0, 2, 255, 2)]).eax as u8, 0);
    }
}

#[test]
fn confirm_route_skips_outside_and_no_exit_cells_and_keeps_call_order() {
    let _begin = fake(0x006D_64B0, begin_route as *const () as usize);
    let _next = fake(0x006D_6510, next_route_entry as *const () as usize);
    let mut sim_mem = Object::new(0xD00);
    unsafe {
        let sim = TrafficSim::from_ptr(sim_mem.ptr()); sim.set_max_cell_x(3); sim.set_max_cell_z(3);
        // No connection storage is needed for these skipped cells.
        let before = bytes(sim_mem.ptr(), 0xD00);
        let steps = vec![step(-1, 0, 255, 255, 255, 0), step(3, 4, 0, 1, 0, 0), step(2, 2, 255, 255, 0, 255)];
        assert_eq!(confirm(sim_mem.ptr(), steps).eax as u8, 1);
        let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
        assert_eq!(calls, [Call(99, 0x006D_64B0, vec![0x1122_3344]), Call(99, 0x006D_6510, vec![]), Call(99, 0x006D_6510, vec![]), Call(99, 0x006D_6510, vec![]), Call(99, 0x006D_6510, vec![])]);
        assert_eq!(bytes(sim_mem.ptr(), 0xD00), before);
    }
}

#[test]
fn setup_search_commuter_freight_probe_forced_start_and_wrapped_shifts() {
    let pf_vt = pf_table(); let source_vt = table(&[(0x14, source as *const () as usize)]);
    let mut pf = object(8, &pf_vt); let mut pf2 = object(8, &pf_vt); let mut src = object(8, &source_vt);
    let mut sim_mem = Object::new(0xD00); let mut cx_mem = Object::new(0x60); let mut trip_mem = Object::new(0x30);
    unsafe {
        store(pf.ptr(), 4, 1); store(pf2.ptr(), 4, 2); store(src.ptr(), 4, 3);
        let sim = TrafficSim::from_ptr(sim_mem.ptr()); let cx = TripContext::from_ptr(cx_mem.ptr()); let t = Trip::from_ptr(trip_mem.ptr());
        sim.set_commute_maximum(f32::from_bits(0x7FC1_2345));
        cx.set_outbound(pf.ptr()); cx.set_inbound(pf2.ptr()); cx.set_source(src.ptr()); cx.set_work_time(0xFFFF_FFFF);
        t.set_traffic(0x8000_0001);
        for (kind, strategy, flags, continuing) in [
            (0, 0, 0, 0), (1, 1, 0, 1), (2, 2, 0, 0), (0, 0, 2, 0),
            (2, 0, 4, 1), (1, 33, 4, 0), (3, 0, 0, 1), (4, 0, 0, 1), (32, 3, 0, 0), (0, 255, 0, 0),
        ] {
            STATE.with(|s| *s.borrow_mut() = State { source: [-2, 3, 7, 8], ..State::default() });
            cx.set_continue_search(continuing); cx.set_freight(0xABCD); cx.set_reversible(0xA5);
            t.set_kind(kind); t.set_strategy(strategy); t.set_flags(flags);
            let before_sim = bytes(sim_mem.ptr(), 0xD00); let before_trip = bytes(trip_mem.ptr(), 0x30);
            fc4p_0070ec40(sim_mem.ptr(), cx_mem.ptr(), trip_mem.ptr());
            let dest = if flags & 2 != 0 && kind == 0 { 7 } else { 1u32.wrapping_shl(u32::from(kind)) };
            let mut expected = vec![
                Call(1, 0x48, vec![0x8000_0001]), Call(1, 0x24, vec![0]), Call(1, 0x30, vec![]), Call(1, 0x28, vec![dest]),
                Call(1, 0x54, vec![u32::MAX]), Call(2, 0x54, vec![u32::MAX]), Call(3, 0x14, vec![]),
                Call(1, 0x4C, vec![(-2i32) as u32, 3, 7, 8]), Call(1, 0x1C, vec![]), Call(1, 0x3C, vec![]),
                Call(1, 0x40, vec![0]), Call(1, 0x44, vec![0x7FC1_2345]),
            ];
            if kind <= 2 {
                if continuing != 0 { expected.push(Call(1, 0x14, vec![4])); }
                expected.push(Call(1, 0x34, vec![0x1CF, 0]));
                expected.push(Call(1, 0x34, vec![if flags & 4 != 0 { 1u32.wrapping_shl(u32::from(strategy)) } else { 3 }, 1]));
                if flags & 4 == 0 {
                    let offset = match strategy { 0 => Some(0xC9C), 1 => Some(0xCC0), 2 => Some(0xC78), _ => None };
                    if let Some(offset) = offset { expected.push(Call(1, 0x40, vec![sim_mem.ptr().add(offset) as u32])); }
                }
                assert_eq!((cx.freight(), cx.reversible()), (0, 1));
            } else if kind == 3 {
                expected.push(Call(1, 0x14, vec![1])); expected.push(Call(1, 0x34, vec![0x30, 1]));
                assert_eq!((cx.freight(), cx.reversible()), (1, 0));
            } else { assert_eq!((cx.freight(), cx.reversible()), (0xABCD, 0xA5)); }
            assert_eq!(STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)), expected, "kind {kind}, strategy {strategy}, flags {flags}");
            assert_eq!(bytes(sim_mem.ptr(), 0xD00), before_sim); assert_eq!(bytes(trip_mem.ptr(), 0x30), before_trip);
        }
    }
}

#[test]
fn setup_reroute_uses_lot_or_single_cell_and_reloads_outbound_after_density_call() {
    let pf_vt = pf_table(); let source_vt = table(&[(0x14, source as *const () as usize)]);
    let manager_vt = table(&[(0x18, get_lot as *const () as usize)]); let lot_vt = table(&[(0x38, rectangle as *const () as usize)]);
    let mut pf = object(8, &pf_vt); let mut pf_new = object(8, &pf_vt); let mut pf2 = object(8, &pf_vt);
    let mut src = object(8, &source_vt); let mut manager = object(8, &manager_vt); let mut lot = object(8, &lot_vt);
    let mut manager_ptr = manager.ptr();
    let _global = fake_data(0x00B4_3D08, (&mut manager_ptr as *mut *mut u8).cast());
    let mut sim_mem = Object::new(0xD00); let mut cx_mem = Object::new(0x60); let mut trip_mem = Object::new(0x30);
    unsafe {
        store(pf.ptr(), 4, 1); store(pf_new.ptr(), 4, 9); store(pf2.ptr(), 4, 2);
        store(src.ptr(), 4, 3); store(manager.ptr(), 4, 4); store(lot.ptr(), 4, 5);
        let sim = TrafficSim::from_ptr(sim_mem.ptr()); sim.set_max_cell_x(3); sim.set_max_cell_z(3);
        let cx = TripContext::from_ptr(cx_mem.ptr()); cx.set_inbound(pf2.ptr()); cx.set_source(src.ptr());
        let trip = Trip::from_ptr(trip_mem.ptr()); trip.set_status(1); trip.set_kind(4);
        for (x, z, found) in [(2, 3, true), (2, 3, false), (-1, 2, true), (4, 2, true)] {
            cx.set_outbound(pf.ptr()); trip.set_destination_x(x); trip.set_destination_z(z);
            STATE.with(|s| *s.borrow_mut() = State {
                source: [1, 2, 3, 4], rectangle: [5, 6, 7, 8], lot: if found { lot.ptr() as usize } else { 0 },
                swap: Some((cx_mem.ptr() as usize, pf_new.ptr() as usize)), ..State::default()
            });
            fc4p_0070ec40(sim_mem.ptr(), cx_mem.ptr(), trip_mem.ptr());
            let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
            let mut prefix = vec![Call(1, 0x48, vec![0]), Call(9, 0x24, vec![3])];
            let inside = (0..=3).contains(&x) && (0..=3).contains(&z);
            if inside { prefix.push(Call(4, 0x18, vec![x as i32 as u32, z as i32 as u32, 0])); }
            if inside && found { prefix.push(Call(5, 0x38, vec![])); }
            prefix.push(Call(9, 0x50, if inside && found { vec![5, 6, 7, 8] } else { vec![x as i32 as u32, z as i32 as u32, x as i32 as u32, z as i32 as u32] }));
            assert_eq!(&calls[..prefix.len()], prefix);
            assert!(calls[prefix.len()..].iter().all(|c| c.0 != 1));
        }
    }
}
