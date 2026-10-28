// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Position(usize),
    Tract(usize, u32, u32),
    Get(usize, u32, u32),
    Curve(usize, u32),
}
thread_local! {static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};static CURVES:RefCell<Vec<(usize,f64)>>=const{RefCell::new(vec![])};}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn position(p: *mut u8, out: *mut f32) -> u32 {
    log(Call::Position(p as usize));
    unsafe {
        out.write(160.0);
        out.add(1).write(5.0);
        out.add(2).write(320.0);
    }
    0
}
extern "thiscall" fn to_tract(p: *mut u8, x: f32, z: f32, tx: *mut u32, tz: *mut u32) -> u32 {
    log(Call::Tract(p as usize, x.to_bits(), z.to_bits()));
    unsafe {
        *tx = 10;
        *tz = 20;
    }
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
extern "thiscall" fn curve(p: *mut u8, x: f32) -> f64 {
    log(Call::Curve(p as usize, x.to_bits()));
    CURVES.with(|c| {
        c.borrow()
            .iter()
            .find(|&&(a, _)| a == p as usize)
            .map_or(100.0, |&(_, v)| v)
    })
}

struct World {
    sim: Object,
    occupant: Object,
    grids: Box<[Grid; 5]>,
    vt: Box<[usize; 23]>,
    ot: Box<[usize; 9]>,
    constants: Box<[f32; 5]>,
    tracts: Box<[[u32; 4]; 2]>,
    effects: Box<[[u32; 12]; 2]>,
    curves: Box<[u32; 3]>,
    guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        take();
        CURVES.with(|v| v.borrow_mut().clear());
        let mut w = Self {
            sim: Object::new(0x400),
            occupant: Object::new(4),
            grids: Box::new(core::array::from_fn(|_| Grid {
                vt: core::ptr::null(),
                value: 0,
            })),
            vt: Box::new([0; 23]),
            ot: Box::new([0; 9]),
            constants: Box::new([0.0, 100.0, 0.01, 12582912.0, 4294967296.0]),
            tracts: Box::new([[0; 4]; 2]),
            effects: Box::new([[0; 12]; 2]),
            curves: Box::new([0; 3]),
            guards: vec![],
        };
        w.vt[0x58 / 4] = get as *const () as usize;
        w.vt[0x4C / 4] = to_tract as *const () as usize;
        w.ot[8] = position as *const () as usize;
        for (i, g) in w.grids.iter_mut().enumerate() {
            g.vt = w.vt.as_ptr();
            g.value = [0xAAAA0064, 0xBBBB00C8, 0xCCCC012C, 0xDDDD001E, 0xEEEE0002][i];
        }
        unsafe {
            w.occupant.ptr().cast::<*const usize>().write(w.ot.as_ptr());
            let sim = ServedSim::from_ptr(w.sim.ptr());
            sim.set_population_low((&mut w.grids[0] as *mut Grid).cast());
            sim.set_population_medium((&mut w.grids[1] as *mut Grid).cast());
            sim.set_population_high((&mut w.grids[2] as *mut Grid).cast());
            sim.set_age((&mut w.grids[3] as *mut Grid).cast());
        }
        let p = w.tracts.as_mut_ptr();
        w.tracts[0][0] = unsafe { p.add(1) } as u32;
        w.tracts[1][0] = p as u32;
        w.tracts[1][2] = 9;
        w.tracts[1][3] = 18;
        let e = w.effects.as_mut_ptr();
        w.effects[0][0] = e as u32;
        for (i, address) in [0x00A81054, 0x00A80AA0, 0x00A823A4, 0x00AB2BB0, 0x00A80AA8]
            .into_iter()
            .enumerate()
        {
            w.guards
                .push(fake_data(address, unsafe { w.constants.as_mut_ptr().add(i).cast() }));
        }
        w.guards.push(fake(0x005F4250, curve as *const () as usize));
        w
    }
    fn run(&mut self, radius: f32) -> u32 {
        let mut tracts = self.tracts.as_mut_ptr().cast::<u8>();
        let mut effects = self.effects.as_mut_ptr().cast::<u8>();
        unsafe {
            fc4p_006f8f60(
                self.sim.ptr(),
                self.occupant.ptr(),
                radius,
                50,
                self.curves.as_mut_ptr().cast(),
                self.curves.as_mut_ptr().add(1).cast(),
                self.curves.as_mut_ptr().add(2).cast(),
                &mut effects,
                (&mut self.grids[4] as *mut Grid).cast(),
                25,
                &mut tracts,
                0xDEADBEEF,
            )
        }
    }
}
#[test]
fn served_population_uses_all_wealths_low_width_values_and_exact_curve_call_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let age = w.curves.as_mut_ptr() as usize;
    let wealth = age + 4;
    let distance = age + 8;
    CURVES.with(|v| *v.borrow_mut() = vec![(age, 50.0), (distance, 50.0)]);
    assert_eq!(w.run(2.0), 150);
    let calls = take();
    let input = match calls[3] {
        Call::Curve(p, input) => {
            assert_eq!(p, distance);
            input
        }
        _ => panic!(),
    };
    assert_eq!(f32::from_bits(input), 111.8034);
    let g: Vec<_> = w.grids.iter().map(|g| g as *const Grid as usize).collect();
    assert_eq!(
        calls,
        [
            Call::Position(w.occupant.ptr() as usize),
            Call::Tract(g[0], 160f32.to_bits(), 320f32.to_bits()),
            Call::Get(g[4], 9, 18),
            Call::Curve(distance, input),
            Call::Get(g[3], 9, 18),
            Call::Curve(age, 30f32.to_bits()),
            Call::Get(g[0], 9, 18),
            Call::Get(g[4], 9, 18),
            Call::Curve(wealth, 0f32.to_bits()),
            Call::Get(g[1], 9, 18),
            Call::Get(g[4], 9, 18),
            Call::Curve(wealth, 1f32.to_bits()),
            Call::Get(g[2], 9, 18),
            Call::Get(g[4], 9, 18),
            Call::Curve(wealth, 2f32.to_bits())
        ]
    );
    let e = w.effects.as_mut_ptr();
    w.effects[0][0] = unsafe { e.add(1) } as u32;
    w.effects[1][0] = e as u32;
    CURVES.with(|v| {
        v.borrow_mut().extend([
            (unsafe { e.add(1).cast::<u8>().add(0x0C) } as usize, 50.0),
            (unsafe { e.add(1).cast::<u8>().add(0x24) } as usize, 50.0),
        ])
    });
    assert_eq!(w.run(2.0), 38);
    let calls = take();
    assert_eq!(calls.len(), 17);
    assert_eq!(
        calls[6],
        Call::Curve(unsafe { e.add(1).cast::<u8>().add(0x0C) } as usize, input)
    );
    assert_eq!(
        calls[7],
        Call::Curve(unsafe { e.add(1).cast::<u8>().add(0x24) } as usize, 30f32.to_bits())
    );
}
#[test]
fn served_population_handles_empty_uncovered_nonpositive_and_unsigned_distance_inputs() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    for radius in [0.0, -0.0, -1.0, f32::NEG_INFINITY] {
        assert_eq!(w.run(radius), 0);
        assert!(take().is_empty());
    }
    w.grids[4].value = 0;
    assert_eq!(w.run(2.0), 0);
    assert_eq!(take().len(), 3);
    w.tracts[0][0] = w.tracts.as_ptr() as u32;
    assert_eq!(w.run(2.0), 0);
    assert_eq!(take().len(), 2);
    w.tracts[0][0] = unsafe { w.tracts.as_ptr().add(1) } as u32;
    w.tracts[1][2] = 11;
    w.grids[4].value = 2;
    assert_eq!(w.run(2.0), 600);
    let calls = take();
    let Call::Curve(_, input) = calls[3] else { panic!() };
    assert!(f32::from_bits(input) > 1e11);
}

fn environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]","fnstsw ax","stmxcsr dword ptr [{mxcsr}]","pushfd","pop {flags:e}",cw=in(reg) &mut cw,mxcsr=in(reg) &mut mxcsr,flags=lateout(reg) flags,out("ax") sw);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}
#[test]
fn served_population_preserves_existing_x87_values_control_word_mxcsr_and_direction() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let sentinel = 123.25f64;
    let mut restored = 0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]",p=in(reg) &sentinel,out("st(0)") _);
    }
    let before = environment();
    let result = w.run(2.0);
    let after = environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]",p=in(reg) &mut restored,out("st(0)") _);
    }
    assert_eq!(result, 600);
    assert_eq!(restored, sentinel);
    assert_eq!(after, before);
    take();
}
