// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Word(usize, u32, u32),
    Reference(usize, u32),
    List(usize, usize),
    Iterator(usize, usize),
    Department(usize, usize, usize),
    Wide(usize, u64),
    Boolean(usize, u32),
}
thread_local! {static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
extern "thiscall" fn version(p: *mut u8, n: u32) -> u32 {
    log(Call::Word(p as usize, 0x1C, n));
    0
}
extern "thiscall" fn int(p: *mut u8, n: u32) -> u32 {
    log(Call::Word(p as usize, 0x20, n));
    0
}
extern "thiscall" fn uint(p: *mut u8, n: u32) -> u32 {
    log(Call::Word(p as usize, 0x24, n));
    0
}
extern "thiscall" fn float(p: *mut u8, n: u32) -> u32 {
    log(Call::Word(p as usize, 0x30, n));
    0
}
extern "thiscall" fn byte(p: *mut u8, n: u32) -> u32 {
    log(Call::Word(p as usize, 0x14, n));
    0
}
extern "thiscall" fn wide(p: *mut u8, n: u64) -> u32 {
    log(Call::Wide(p as usize, n));
    0
}
extern "C" fn reference(p: *mut u8, n: u32) -> u32 {
    log(Call::Reference(p as usize, n));
    0
}
extern "C" fn list(p: *mut u8, l: *mut u8) -> u32 {
    log(Call::List(p as usize, l as usize));
    0
}
extern "C" fn iterator(p: *mut u8, i: *mut u8) -> u32 {
    log(Call::Iterator(p as usize, i as usize));
    0
}
extern "C" fn boolean(p: *mut u8, n: u32) -> u64 {
    log(Call::Boolean(p as usize, n));
    0x12345678ABCDEF00
}
extern "thiscall" fn department(p: *mut u8, s: *mut u8, d: *mut u8) -> u32 {
    log(Call::Department(p as usize, s as usize, d as usize));
    0
}
#[test]
fn version_seven_writer_keeps_all_field_bits_call_order_and_return_halves_despite_failures() {
    let _r = fake(0x005D99A0, reference as *const () as usize);
    let _l = fake(0x005DAFF0, list as *const () as usize);
    let _i = fake(0x005DA480, iterator as *const () as usize);
    let _b = fake(0x005D9FD0, boolean as *const () as usize);
    let _d = fake(0x006F7B00, department as *const () as usize);
    let mut vt = [0usize; 13];
    for (slot, f) in [
        (0x1C, version as *const () as usize),
        (0x20, int as *const () as usize),
        (0x24, uint as *const () as usize),
        (0x30, float as *const () as usize),
        (0x14, byte as *const () as usize),
        (0x2C, wide as *const () as usize),
    ] {
        vt[slot / 4] = f;
    }
    let mut stream = vt.as_ptr();
    let stream = (&mut stream as *mut *const usize).cast::<u8>();
    let s = stream as usize;
    let mut sim = Object::new(0x400);
    let p = sim.ptr();
    let v = unsafe { SaveView::from_ptr(p) };
    for offset in (0..0x400).step_by(4) {
        unsafe {
            p.add(offset).cast::<u32>().write(0xAB000000 | offset as u32);
        }
    }
    let mut a = [0x11u32, 0x22];
    let mut b = [0x33u32];
    let mut c = [0x44u32, 0x55];
    let mut d = [0x66u32];
    v.set_school_maps(a.as_mut_ptr());
    v.set_school_maps_end(unsafe { a.as_mut_ptr().add(2) });
    v.set_health_maps(b.as_mut_ptr());
    v.set_health_maps_end(unsafe { b.as_mut_ptr().add(1) });
    v.set_schools(c.as_mut_ptr());
    v.set_schools_end(unsafe { c.as_mut_ptr().add(2) });
    v.set_hospitals(d.as_mut_ptr());
    v.set_hospitals_end(unsafe { d.as_mut_ptr().add(1) });
    v.set_state(10);
    v.set_dirty(0);
    v.set_school_auto(0xFF);
    v.set_health_auto(0x80);
    let before = unsafe { core::slice::from_raw_parts(p, 0x400).to_vec() };
    assert_eq!(unsafe { fc4p_006f8c40(p, stream) }, 0x12345678ABCDEF01);
    let mut expected = vec![Call::Word(s, 0x1C, 7), Call::Word(s, 0x20, v.proximity_size())];
    for offset in [0x30, 0x34, 0x38] {
        expected.push(Call::Reference(s, unsafe { word(p, offset) }));
    }
    expected.push(Call::Word(s, 0x20, v.population_size()));
    for offset in [0x40, 0x44, 0x48, 0x4C] {
        expected.push(Call::Reference(s, unsafe { word(p, offset) }));
    }
    expected.extend([
        Call::Word(s, 0x24, 2),
        Call::Reference(s, 0x11),
        Call::Reference(s, 0x22),
        Call::Word(s, 0x24, 1),
        Call::Reference(s, 0x33),
    ]);
    for offset in [0x68, 0x6C] {
        expected.push(Call::Reference(s, unsafe { word(p, offset) }));
    }
    expected.push(Call::Word(s, 0x20, v.quotient_size()));
    for offset in [0x74, 0x78, 0x7C, 0x80, 0x84, 0x88] {
        expected.push(Call::Reference(s, unsafe { word(p, offset) }));
    }
    expected.extend([
        Call::Word(s, 0x30, unsafe { word(p, 0x90) }),
        Call::Word(s, 0x30, unsafe { word(p, 0x94) }),
        Call::Word(s, 0x24, 10),
        Call::Word(s, 0x14, s as u32 & !255),
        Call::Word(s, 0x20, v.population()),
        Call::Department(unsafe { p.sub(8) } as usize, s, unsafe { p.add(0xCC) } as usize),
        Call::Department(unsafe { p.sub(8) } as usize, s, unsafe { p.add(0x12C) } as usize),
    ]);
    for offset in [0x194, 0x198, 0x19C] {
        expected.push(Call::List(s, unsafe { p.add(offset) } as usize));
    }
    expected.extend([
        Call::Word(s, 0x24, 2),
        Call::List(s, c.as_ptr() as usize),
        Call::List(s, unsafe { c.as_ptr().add(1) } as usize),
        Call::Word(s, 0x24, 1),
        Call::List(s, d.as_ptr() as usize),
        Call::List(s, unsafe { p.add(0x1B8) } as usize),
        Call::Iterator(s, unsafe { p.add(0x320) } as usize),
        Call::Wide(s, v.life_sum()),
        Call::Word(s, 0x20, v.running_population()),
        Call::Boolean(s, 255),
        Call::Boolean(s, 128),
    ]);
    assert_eq!(take(), expected);
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x400) }, before);
    for (dirty, state) in [(1, 10), (0, 4), (255, 10)] {
        v.set_dirty(dirty);
        v.set_state(state);
        unsafe {
            fc4p_006f8c40(p, stream);
        }
        assert!(take().contains(&Call::Word(s, 0x14, (s as u32 & !255) | 1)));
    }
}
