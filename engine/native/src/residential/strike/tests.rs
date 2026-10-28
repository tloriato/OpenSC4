// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Member(usize, u32),
    Properties(usize),
    Read(usize, u32),
    Curve(usize, u32),
    Status(usize, bool),
    List(usize, usize, usize, u32, u32),
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};
    static SWAP:RefCell<Option<(*mut u8,*mut *mut u8,usize)>>=const{RefCell::new(None)};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}

#[repr(C)]
struct Building {
    table: *const usize,
    member: u32,
    grade: f32,
    present: bool,
}
extern "thiscall" fn member(p: *mut Building, group: u32) -> u32 {
    log(Call::Member(p as usize, group));
    unsafe { (*p).member }
}
extern "thiscall" fn properties(p: *mut Building) -> *mut u8 {
    log(Call::Properties(p as usize));
    p.cast()
}
extern "C" fn read(p: *mut Building, id: u32, out: *mut f32) -> u32 {
    log(Call::Read(p as usize, id));
    unsafe {
        if (*p).present {
            *out = (*p).grade;
            1
        } else {
            0
        }
    }
}
extern "thiscall" fn curve(p: *mut u8, x: f32) -> f64 {
    log(Call::Curve(p as usize, x.to_bits()));
    50.0
}
extern "thiscall" fn school_status(p: *mut u8) -> u64 {
    log(Call::Status(p as usize, true));
    (0x12345678u64 << 32) | unsafe { u64::from(*p.add(0x178)) }
}
extern "thiscall" fn health_status(p: *mut u8) -> u64 {
    log(Call::Status(p as usize, false));
    (0x12345678u64 << 32) | unsafe { u64::from(*p.add(0x118)) }
}

struct Constants {
    values: Box<[u32; 4]>,
    _guards: Vec<Fake>,
}
impl Constants {
    fn new() -> Self {
        take();
        let mut values = Box::new([0, 1f32.to_bits(), 100f32.to_bits(), 0.01f32.to_bits()]);
        let mut guards = vec![
            fake(0x005FD4F0, read as *const () as usize),
            fake(0x005F4250, curve as *const () as usize),
        ];
        for (i, address) in [0x00B4AF90, 0x00B0D440, 0x00A80AA0, 0x00A823A4].into_iter().enumerate() {
            guards.push(fake_data(address, unsafe { values.as_mut_ptr().add(i).cast() }));
        }
        Self {
            values,
            _guards: guards,
        }
    }
}
fn list(buildings: &mut [Building]) -> Vec<[u32; 3]> {
    let mut nodes = vec![[0; 3]; buildings.len() + 1];
    let len = nodes.len();
    let ptr = nodes.as_mut_ptr();
    for (i, node) in nodes.iter_mut().enumerate() {
        node[0] = unsafe { ptr.add((i + 1) % len) } as u32;
        if i > 0 {
            node[2] = &mut buildings[i - 1] as *mut Building as u32;
        }
    }
    nodes
}
#[test]
fn list_chance_reuses_previous_chance_and_missing_grade_and_keeps_call_order() {
    let _fpu = GameFpu::new();
    let _constants = Constants::new();
    let mut vt = [0usize; 23];
    vt[6] = properties as *const () as usize;
    vt[22] = member as *const () as usize;
    let mut buildings = [
        Building {
            table: vt.as_ptr(),
            member: 0,
            grade: 0.1,
            present: true,
        },
        Building {
            table: vt.as_ptr(),
            member: 1,
            grade: 0.4,
            present: true,
        },
        Building {
            table: vt.as_ptr(),
            member: 0,
            grade: 0.9,
            present: true,
        },
        Building {
            table: vt.as_ptr(),
            member: 1,
            grade: 0.0,
            present: false,
        },
    ];
    let mut nodes = list(&mut buildings);
    let mut head = nodes.as_mut_ptr().cast::<u8>();
    let mut block = Object::new(0x60);
    let bp = block.ptr();
    let mut out = -1f32;
    assert_eq!(
        unsafe { fc4p_006f9390(&mut out, bp, &mut head, 0x292453C5, 0x1A03) },
        &out as *const f32 as u32
    );
    assert_eq!(out, 0.875);
    let b: Vec<_> = buildings.iter().map(|b| b as *const Building as usize).collect();
    let cp = unsafe { bp.add(0x24) } as usize;
    assert_eq!(
        take(),
        [
            Call::Member(b[0], 0x1A03),
            Call::Member(b[1], 0x1A03),
            Call::Properties(b[1]),
            Call::Read(b[1], 0x292453C5),
            Call::Curve(cp, 40f32.to_bits()),
            Call::Member(b[2], 0x1A03),
            Call::Member(b[3], 0x1A03),
            Call::Properties(b[3]),
            Call::Read(b[3], 0x292453C5),
            Call::Curve(cp, 40f32.to_bits())
        ]
    );
    unsafe { Department::from_ptr(bp) }.set_strike(255);
    unsafe {
        fc4p_006f9390(&mut out, bp, core::ptr::null_mut(), 7, 8);
    }
    assert_eq!(out, 1.0);
    assert!(take().is_empty());
    unsafe { Department::from_ptr(bp) }.set_strike(0);
    let mut empty = list(&mut []);
    let mut head = empty.as_mut_ptr().cast();
    unsafe {
        fc4p_006f9390(&mut out, bp, &mut head, 7, 8);
    }
    assert_eq!(out, 0.0);
    assert!(take().is_empty());
}

extern "thiscall" fn list_probability(
    sim: *mut u8,
    out: *mut f32,
    block: *mut u8,
    list: *mut *mut u8,
    grade: u32,
    group: u32,
) -> u32 {
    log(Call::List(sim as usize, block as usize, list as usize, grade, group));
    unsafe {
        *out = 0.5;
    }
    SWAP.with(|s| {
        if let Some((sim, array, n)) = s.borrow_mut().take() {
            unsafe {
                let sim = StrikeSim::from_ptr(sim);
                sim.set_schools(array);
                sim.set_schools_end(array.add(n));
            }
        }
    });
    0
}
#[test]
fn department_probabilities_cover_active_empty_multiple_groups_and_return_registers() {
    let _fpu = GameFpu::new();
    let mut constants = Constants::new();
    let _f = fake(0x006F9390, list_probability as *const () as usize);
    let mut vt = [0usize; 8];
    vt[6] = school_status as *const () as usize;
    vt[7] = health_status as *const () as usize;
    let mut mem = Object::new(0x400);
    let p = mem.ptr();
    unsafe {
        p.cast::<*const usize>().write(vt.as_ptr());
    }
    let sim = unsafe { StrikeSim::from_ptr(p) };
    let mut arrays = [core::ptr::null_mut(); 2];
    let ap = arrays.as_mut_ptr();
    for school in [true, false] {
        if school {
            sim.set_schools(ap);
            sim.set_schools_end(unsafe { ap.add(2) });
        } else {
            sim.set_hospitals(ap);
            sim.set_hospitals_end(unsafe { ap.add(2) });
        }
        let function = if school { fc4p_006f9670 } else { fc4p_006f9730 };
        let mut out = -1f32;
        let result = unsafe { function(p, &mut out) };
        assert_eq!(out, 0.75);
        assert_eq!(result, (2u64 << 32) | u64::from(&out as *const f32 as u32));
        let (offset, grade, group) = if school {
            (0x134, 0x292453C5, 0x1A03)
        } else {
            (0xD4, 0x092453CB, 0x1A02)
        };
        assert_eq!(
            take(),
            [
                Call::Status(p as usize, school),
                Call::List(p as usize, unsafe { p.add(offset) } as usize, ap as usize, grade, group),
                Call::List(
                    p as usize,
                    unsafe { p.add(offset) } as usize,
                    unsafe { ap.add(1) } as usize,
                    grade,
                    group
                )
            ]
        );
        unsafe {
            *p.add(if school { 0x178 } else { 0x118 }) = 0xFF;
        }
        let result = unsafe { function(p, &mut out) };
        assert_eq!(out, 1.0);
        assert_eq!(result >> 32, 0x12345678);
        assert_eq!(take(), [Call::Status(p as usize, school)]);
        unsafe {
            *p.add(if school { 0x178 } else { 0x118 }) = 0;
        }
        if school {
            sim.set_schools_end(ap);
        } else {
            sim.set_hospitals_end(ap);
        }
        constants.values[0] = 0x80000000;
        let result = unsafe { function(p, &mut out) };
        assert_eq!(out.to_bits(), 0x80000000);
        assert_eq!(result >> 32, 0x12345678);
        assert_eq!(take(), [Call::Status(p as usize, school)]);
        constants.values[0] = 0;
    }
    sim.set_schools(ap);
    sim.set_schools_end(unsafe { ap.add(1) });
    let mut replacement = [core::ptr::null_mut(); 2];
    let rp = replacement.as_mut_ptr();
    SWAP.with(|s| *s.borrow_mut() = Some((p, rp, 2)));
    let mut out = 0f32;
    unsafe {
        fc4p_006f9670(p, &mut out);
    }
    assert_eq!(out, 0.75);
    let calls = take();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[2],
        Call::List(
            p as usize,
            unsafe { p.add(0x134) } as usize,
            unsafe { rp.add(1) } as usize,
            0x292453C5,
            0x1A03
        )
    );
}
