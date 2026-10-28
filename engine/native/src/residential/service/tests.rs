// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native counterparts of department persistence, history, and REH03 coverage-centre cases.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Write(usize, usize, u32),
    Reference(usize, usize),
    Allocate(u32),
    Construct(usize),
    Retain(usize),
    Type(usize, u32),
    Id(usize, u32),
    Value(usize, u32),
    Post(usize, usize),
    Release(usize),
    Lot(usize, usize),
    Centre(usize),
    Position(usize),
    Tracts([u32; 7]),
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static ALLOCATION:RefCell<(*mut u8,*mut u8)>=const{RefCell::new((core::ptr::null_mut(),core::ptr::null_mut()))};
    static SWAP:RefCell<Option<(*mut u8,*mut u8)>>=const{RefCell::new(None)};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}

extern "thiscall" fn float_word(stream: *mut u8, word: u32) -> u32 {
    log(Call::Write(stream as usize, 0x30, word));
    0
}
extern "thiscall" fn int_word(stream: *mut u8, word: u32) -> u32 {
    log(Call::Write(stream as usize, 0x20, word));
    0
}
extern "thiscall" fn byte_word(stream: *mut u8, word: u32) -> u32 {
    log(Call::Write(stream as usize, 0x14, word));
    0
}
extern "thiscall" fn final_word(stream: *mut u8, word: u32) -> u32 {
    log(Call::Write(stream as usize, 0x24, word));
    0xAABB_CCDD
}
extern "C" fn reference(stream: *mut u8, object: *mut u8) -> u32 {
    log(Call::Reference(stream as usize, object as usize));
    0
}

#[test]
fn department_write_keeps_field_bits_and_original_order_despite_failures() {
    let _reference = fake(0x005D_99A0, reference as *const () as usize);
    let mut table = [0usize; 13];
    table[0x30 / 4] = float_word as *const () as usize;
    table[0x20 / 4] = int_word as *const () as usize;
    table[0x14 / 4] = byte_word as *const () as usize;
    table[0x24 / 4] = final_word as *const () as usize;
    let mut stream = table.as_ptr();
    let stream = (&mut stream as *mut *const usize).cast::<u8>();
    let mut memory = Object::new(0x70);
    let p = memory.ptr();
    unsafe {
        p.write_bytes(0xA5, 0x70);
    }
    let block = unsafe { Department::from_ptr(p) };
    block.set_first(0x7F80_0100);
    block.set_second(0x8000_0000);
    block.set_third(0x7FC0_9999);
    block.set_count(23);
    block.set_fourth(0x3F80_0000);
    block.set_strike(0xFF);
    block.set_strike_day(u32::MAX);
    block.set_first_curve(0x1234usize as *mut u8);
    block.set_second_curve(core::ptr::null_mut());
    block.set_fifth(0x3E80_0000);
    block.set_sixth(0xFFC0_0001);
    block.set_final_word(0xFFFF_FFFE);
    let before = unsafe { core::slice::from_raw_parts(p, 0x70).to_vec() };
    assert_eq!(unsafe { fc4p_006f7b00(core::ptr::null_mut(), stream, p) }, 0xAABB_CC01);
    let s = stream as usize;
    assert_eq!(
        take(),
        [
            Call::Write(s, 0x30, 0x7F80_0100),
            Call::Write(s, 0x30, 0x8000_0000),
            Call::Write(s, 0x30, 0x7FC0_9999),
            Call::Write(s, 0x20, 23),
            Call::Write(s, 0x30, 0x3F80_0000),
            Call::Write(s, 0x14, 255),
            Call::Write(s, 0x20, u32::MAX),
            Call::Reference(s, 0x1234),
            Call::Reference(s, 0),
            Call::Write(s, 0x30, 0x3E80_0000),
            Call::Write(s, 0x30, 0xFFC0_0001),
            Call::Write(s, 0x24, 0xFFFF_FFFE),
        ]
    );
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x70) }, before);
}

extern "C" fn allocation(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    ALLOCATION.with(|v| v.borrow().0)
}
extern "thiscall" fn construct(p: *mut u8) -> *mut u8 {
    log(Call::Construct(p as usize));
    ALLOCATION.with(|v| v.borrow().1)
}
extern "thiscall" fn retain(p: *mut u8) -> u32 {
    log(Call::Retain(p as usize));
    1
}
extern "thiscall" fn release(p: *mut u8) -> u32 {
    log(Call::Release(p as usize));
    0xABCD_EF12
}
extern "thiscall" fn message_type(p: *mut u8, n: u32) -> u32 {
    log(Call::Type(p as usize, n));
    0
}
extern "thiscall" fn message_id(p: *mut u8, n: u32) -> u32 {
    log(Call::Id(p as usize, n));
    0
}
extern "thiscall" fn message_value(p: *mut u8, n: u32) -> u32 {
    log(Call::Value(p as usize, n));
    SWAP.with(|v| {
        if let Some((sim, next)) = v.borrow_mut().take() {
            unsafe { ResidentialSim::from_ptr(sim) }.set_message_server(next);
        }
    });
    0
}
extern "thiscall" fn post(p: *mut u8, m: *mut u8) -> u32 {
    log(Call::Post(p as usize, m as usize));
    0
}

#[test]
fn history_uses_game_allocation_retains_posts_and_releases_and_rereads_server() {
    let _a = fake(0x0091_33DA, allocation as *const () as usize);
    let _c = fake(0x0091_34D6, construct as *const () as usize);
    let mut table = [0usize; 17];
    table[1] = retain as *const () as usize;
    table[2] = release as *const () as usize;
    table[5] = message_type as *const () as usize;
    table[0x2C / 4] = message_id as *const () as usize;
    table[0x40 / 4] = message_value as *const () as usize;
    let mut message = Object::new(0x2C);
    let mp = message.ptr();
    unsafe {
        mp.cast::<*const usize>().write(table.as_ptr());
    }
    let mut allocation = Object::new(0x2C);
    let ap = allocation.ptr();
    ALLOCATION.with(|v| *v.borrow_mut() = (ap, mp));
    let mut server_table = [0usize; 4];
    server_table[3] = post as *const () as usize;
    let mut old_server = server_table.as_ptr();
    let mut new_server = server_table.as_ptr();
    let old = (&mut old_server as *mut *const usize).cast::<u8>();
    let new = (&mut new_server as *mut *const usize).cast::<u8>();
    let mut object = Object::new(0x350);
    let p = object.ptr();
    let sim = unsafe { ResidentialSim::from_ptr(p) };
    for value in [0, 1, u32::MAX, 0x8000_0000] {
        sim.set_message_server(old);
        SWAP.with(|v| *v.borrow_mut() = Some((p, new)));
        assert_eq!(unsafe { fc4p_006f7d00(p, 0xCA54_5CAB, value) }, 0xABCD_EF12);
        assert_eq!(
            take(),
            [
                Call::Allocate(44),
                Call::Construct(ap as usize),
                Call::Retain(mp as usize),
                Call::Type(mp as usize, 0x89EF_A536),
                Call::Id(mp as usize, 0xCA54_5CAB),
                Call::Value(mp as usize, value),
                Call::Post(new as usize, mp as usize),
                Call::Release(mp as usize)
            ]
        );
    }
    sim.set_message_server(core::ptr::null_mut());
    assert_eq!(unsafe { fc4p_006f7d00(p, 0, 0) }, 0);
    assert!(take().is_empty());
}

#[repr(C)]
struct PositionObject {
    vtable: *const usize,
    position: [f32; 3],
}
#[repr(C)]
struct LotManager {
    vtable: *const usize,
    lot: *mut u8,
    sim: *mut u8,
    slop: f32,
}
unsafe extern "thiscall" fn find_lot(p: *mut LotManager, b: *mut u8) -> *mut u8 {
    log(Call::Lot(p as usize, b as usize));
    unsafe { ResidentialSim::from_ptr((*p).sim) }.set_coverage_slop(unsafe { (*p).slop });
    unsafe { (*p).lot }
}
unsafe extern "thiscall" fn centre(p: *mut PositionObject, out: *mut f32) -> u32 {
    log(Call::Centre(p as usize));
    unsafe {
        out.copy_from_nonoverlapping((*p).position.as_ptr(), 3);
    }
    0
}
unsafe extern "thiscall" fn position(p: *mut PositionObject, out: *mut f32) -> u32 {
    log(Call::Position(p as usize));
    unsafe {
        out.copy_from_nonoverlapping((*p).position.as_ptr(), 3);
    }
    0
}
extern "C" fn radius(x: f32, z: f32, r: f32, a: u32, b: u32, c: u32, d: u32) -> u32 {
    log(Call::Tracts([x.to_bits(), z.to_bits(), r.to_bits(), a, b, c, d]));
    0xABCD_9876
}

#[test]
fn covered_tracts_prefer_lot_centre_and_scale_radius_after_the_lookup() {
    let _fpu = GameFpu::new();
    let _radius = fake(0x005E_5200, radius as *const () as usize);
    let mut object = Object::new(0x350);
    let sim = object.ptr();
    let mut pt = [0usize; 16];
    pt[0x3C / 4] = centre as *const () as usize;
    pt[0x20 / 4] = position as *const () as usize;
    let mut lot = PositionObject {
        vtable: pt.as_ptr(),
        position: [12.0, 40.0, 24.0],
    };
    let lp = (&mut lot as *mut PositionObject).cast::<u8>();
    let mut building = PositionObject {
        vtable: pt.as_ptr(),
        position: [-3.0, 100.0, -7.0],
    };
    let bp = (&mut building as *mut PositionObject).cast::<u8>();
    let mut mt = [0usize; 15];
    mt[0x38 / 4] = find_lot as *const () as usize;
    let mut manager = LotManager {
        vtable: mt.as_ptr(),
        lot: lp,
        sim,
        slop: 1.5,
    };
    let mp = (&mut manager as *mut LotManager).cast::<u8>();
    let mut global = mp;
    let _manager = fake_data(0x00B4_3D08, (&mut global as *mut *mut u8).cast());
    for (use_lot, slop, radius_bits, want) in [
        (true, 1.5, 10.0f32.to_bits(), 15.0f32.to_bits()),
        (false, -2.0, 0.0f32.to_bits(), (-0.0f32).to_bits()),
        (false, f32::from_bits(0x7F80_0200), 0x7FC0_0100, 0x7FC0_0100),
    ] {
        unsafe {
            (*mp.cast::<LotManager>()).lot = if use_lot { lp } else { core::ptr::null_mut() };
            (*mp.cast::<LotManager>()).slop = slop;
        }
        unsafe { ResidentialSim::from_ptr(sim) }.set_coverage_slop(99.0);
        assert_eq!(
            unsafe { fc4p_006f7e00(sim, bp, f32::from_bits(radius_bits), 4, 5, 6, 0xDEAD_BEEF) },
            0xABCD_9876
        );
        let (x, z) = if use_lot {
            (12.0f32, 24.0f32)
        } else {
            (-3.0f32, -7.0f32)
        };
        assert_eq!(
            take(),
            [
                Call::Lot(mp as usize, bp as usize),
                if use_lot {
                    Call::Centre(lp as usize)
                } else {
                    Call::Position(bp as usize)
                },
                Call::Tracts([x.to_bits(), z.to_bits(), want, 4, 5, 6, 0xDEAD_BEEF])
            ]
        );
    }
}
