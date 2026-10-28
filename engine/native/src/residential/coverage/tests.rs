// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Properties(usize),
    Read(usize, u32),
    Position(usize),
    Grid(usize, u32),
    Allocate(u32),
    Clear,
    Tracts(usize, usize, u32, u32, u32, u32),
    Get(usize, u32, u32),
    Set(usize, u32, u32, u32),
    Destroy,
}
#[derive(Default)]
struct State {
    radius: f32,
    capacity: u32,
    missing: u32,
    sentinel: *mut u8,
    node: *mut u8,
}
thread_local! {static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};static STATE:RefCell<State>=RefCell::new(State::default());}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn properties(p: *mut u8) -> *mut u8 {
    log(Call::Properties(p as usize));
    p
}
extern "C" fn radius(p: *mut u8, id: u32, out: *mut f32) -> u32 {
    log(Call::Read(p as usize, id));
    STATE.with(|s| {
        let s = s.borrow();
        if s.missing == 1 {
            0
        } else {
            unsafe {
                *out = s.radius;
            }
            1
        }
    })
}
extern "C" fn capacity(p: *mut u8, id: u32, out: *mut u32) -> u32 {
    log(Call::Read(p as usize, id));
    STATE.with(|s| {
        let s = s.borrow();
        if s.missing == 2 {
            0
        } else {
            unsafe {
                *out = s.capacity;
            }
            1
        }
    })
}
extern "thiscall" fn position(p: *mut u8, out: *mut f32) -> u32 {
    log(Call::Position(p as usize));
    unsafe {
        out.write(1.0);
        out.add(1).write(0.0);
        out.add(2).write(2.0);
    }
    0
}
extern "thiscall" fn scale(p: *mut u8) -> f64 {
    log(Call::Grid(p as usize, 0x40));
    64.0
}
extern "thiscall" fn size_x(p: *mut u8) -> u32 {
    log(Call::Grid(p as usize, 0x30));
    16
}
extern "thiscall" fn size_z(p: *mut u8) -> u32 {
    log(Call::Grid(p as usize, 0x34));
    32
}
extern "thiscall" fn tract_size(p: *mut u8) -> f64 {
    log(Call::Grid(p as usize, 0x38));
    4.0
}
extern "C" fn allocate(n: u32) -> *mut u8 {
    log(Call::Allocate(n));
    STATE.with(|s| s.borrow().sentinel)
}
extern "thiscall" fn clear(head: *mut *mut u8) -> u32 {
    log(Call::Clear);
    unsafe {
        let h = *head;
        assert_eq!(*h.cast::<*mut u8>(), h);
        assert_eq!(*h.add(4).cast::<*mut u8>(), h);
    }
    0
}
extern "thiscall" fn tracts(
    sim: *mut u8,
    building: *mut u8,
    radius: f32,
    scale: f32,
    x: u32,
    z: u32,
    head: *mut *mut u8,
) -> u32 {
    log(Call::Tracts(
        sim as usize,
        building as usize,
        radius.to_bits(),
        scale.to_bits(),
        x,
        z,
    ));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            (*head).cast::<*mut u8>().write(s.node);
            s.node.cast::<*mut u8>().write(*head);
            s.node.add(8).cast::<u32>().write(3);
            s.node.add(12).cast::<u32>().write(4);
        }
    });
    0
}
extern "thiscall" fn destroy(_head: *mut *mut u8) -> u32 {
    log(Call::Destroy);
    0
}
#[repr(C)]
struct Grid {
    vt: *const usize,
    value: u32,
}
extern "thiscall" fn get(p: *mut Grid, x: u32, z: u32) -> u32 {
    log(Call::Get(p as usize, x, z));
    unsafe { (*p).value }
}
// The grid caller retains ECX's upper half when constructing its setter argument.
#[unsafe(naked)]
unsafe extern "thiscall" fn get_with_ecx(_p: *mut Grid, _x: u32, _z: u32) -> u32 {
    core::arch::naked_asm!("push dword ptr [esp+8]","push dword ptr [esp+8]","call {get}","mov ecx,0xabcd1234","ret 8",get=sym get);
}
extern "thiscall" fn set(p: *mut Grid, x: u32, z: u32, value: u32) -> u32 {
    log(Call::Set(p as usize, x, z, value));
    unsafe {
        (*p).value = value;
    }
    0
}

struct World {
    sim: Object,
    building: Object,
    grids: Box<[Grid; 5]>,
    vt: Box<[usize; 27]>,
    ot: Box<[usize; 9]>,
    heads: Box<[*mut u8; 2]>,
    sentinel: Object,
    node: Object,
    zero: Box<f32>,
    guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        take();
        let mut w = Self {
            sim: Object::new(0x400),
            building: Object::new(4),
            grids: Box::new(core::array::from_fn(|_| Grid {
                vt: core::ptr::null(),
                value: 0,
            })),
            vt: Box::new([0; 27]),
            ot: Box::new([0; 9]),
            heads: Box::new([core::ptr::null_mut(); 2]),
            sentinel: Object::new(16),
            node: Object::new(16),
            zero: Box::new(0.0),
            guards: vec![],
        };
        for (slot, f) in [
            (0x40, scale as *const () as usize),
            (0x30, size_x as *const () as usize),
            (0x34, size_z as *const () as usize),
            (0x38, tract_size as *const () as usize),
            (0x58, get_with_ecx as *const () as usize),
            (0x68, set as *const () as usize),
        ] {
            w.vt[slot / 4] = f;
        }
        w.ot[6] = properties as *const () as usize;
        w.ot[8] = position as *const () as usize;
        for (i, g) in w.grids.iter_mut().enumerate() {
            g.vt = w.vt.as_ptr();
            g.value = if i < 3 { 0xAABB0000 | ((i + 1) as u32 * 100) } else { 0 };
        }
        let gp = w.grids.as_mut_ptr();
        for i in 0..2 {
            w.heads[i] = unsafe { gp.add(i + 3) }.cast();
        }
        unsafe {
            w.building.ptr().cast::<*const usize>().write(w.ot.as_ptr());
            CoverageSim::from_ptr(w.sim.ptr()).set_populations([gp.cast(), gp.add(1).cast(), gp.add(2).cast()]);
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                radius: 200.0,
                capacity: 1000,
                missing: 0,
                sentinel: w.sentinel.ptr(),
                node: w.node.ptr(),
            }
        });
        for (a, f) in [
            (0x005FD4F0, radius as *const () as usize),
            (0x005FD480, capacity as *const () as usize),
            (0x0090CF54, allocate as *const () as usize),
            (0x0050A2A0, clear as *const () as usize),
            (0x005650A0, destroy as *const () as usize),
            (0x006F7E00, tracts as *const () as usize),
        ] {
            w.guards.push(fake(a, f));
        }
        w.guards.push(fake_data(0x00A81054, (&mut *w.zero as *mut f32).cast()));
        w
    }
    fn run(&mut self, index: u32, multiplier: f32, served: &mut u32) {
        let mut grids = self.heads.as_mut_ptr();
        unsafe {
            fc4p_006f9bf0(
                self.sim.ptr(),
                &mut grids,
                index,
                self.building.ptr(),
                11,
                multiplier,
                22,
                25,
                served,
            );
        }
    }
}
#[test]
fn coverage_counts_only_new_population_and_retains_ecx_upper_half_in_the_grid_write() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let mut served = 7;
    w.run(0, 0.5, &mut served);
    assert_eq!(served, 607);
    assert_eq!(w.grids[3].value, 0xABCD0028);
    let b = w.building.ptr() as usize;
    let sim = w.sim.ptr() as usize;
    let g: Vec<_> = w.grids.iter().map(|g| g as *const Grid as usize).collect();
    assert_eq!(
        take(),
        [
            Call::Properties(b),
            Call::Read(b, 11),
            Call::Read(b, 22),
            Call::Position(b),
            Call::Grid(g[3], 0x40),
            Call::Allocate(16),
            Call::Clear,
            Call::Grid(g[3], 0x34),
            Call::Grid(g[3], 0x30),
            Call::Grid(g[3], 0x38),
            Call::Tracts(sim, b, 100f32.to_bits(), 4f32.to_bits(), 16, 32),
            Call::Get(g[3], 3, 4),
            Call::Set(g[3], 3, 4, 0xABCD0028),
            Call::Get(g[0], 3, 4),
            Call::Get(g[1], 3, 4),
            Call::Get(g[2], 3, 4),
            Call::Destroy
        ]
    );
    w.run(1, 1.0, &mut served);
    assert_eq!(served, 607);
    let calls = take();
    assert_eq!(calls[calls.len() - 2], Call::Get(g[3], 3, 4));
    w.run(0, 1.0, &mut served);
    assert_eq!(served, 607);
    let calls = take();
    assert_eq!(calls.len(), 14);
    assert_eq!(w.grids[3].value, 0xABCD0050);
    STATE.with(|s| s.borrow_mut().capacity = 65535 * 25);
    w.run(0, 1.0, &mut served);
    assert_eq!(w.grids[3].value, 0xABCE004F);
    take();
    STATE.with(|s| s.borrow_mut().capacity = 300 * 25);
    w.run(0, 1.0, &mut served);
    assert_eq!(w.grids[3].value, 255);
    take();
}
#[test]
fn missing_properties_and_nonpositive_or_unordered_radius_skip_list_allocation() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let mut served = 9;
    for (missing, radius, count) in [
        (1, 200.0, 2),
        (2, 200.0, 3),
        (0, 0.0, 5),
        (0, -1.0, 5),
        (0, f32::NAN, 5),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.missing = missing;
            s.radius = radius;
        });
        w.run(0, 1.0, &mut served);
        assert_eq!(take().len(), count);
        assert_eq!(served, 9);
    }
}
