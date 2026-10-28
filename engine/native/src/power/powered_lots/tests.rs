// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Width(usize),
    Xor(usize, usize, u32),
    Any(usize, u32, Rect),
    Pixel(usize, u32, u32, u32),
    Grid(usize, u32, u32, u32),
    Allocate(u32),
    New(usize),
    Message(usize, usize, u32),
    Post(usize, usize, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    message: *mut u8,
    width: u32,
    any_override: Option<Rect>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn width(this: *mut u8) -> u32 {
    log(Call::Width(this as usize));
    STATE.with(|s| s.borrow().width)
}
extern "C" fn xor(out: *mut u32, source: *mut u32, width: u32) {
    log(Call::Xor(out as usize, source as usize, width));
    for i in 0..(width / 32 * width) as usize {
        unsafe {
            *out.add(i) ^= *source.add(i);
        }
    }
}
unsafe fn bit(bits: *mut u32, width: u32, x: u32, z: u32) -> bool {
    unsafe { bits.add((width / 32 * z + x / 32) as usize).read() & (1 << (x & 31)) != 0 }
}
extern "C" fn any(bits: *mut u32, width: u32, rect: *mut Rect) -> u32 {
    let r = unsafe { *rect };
    log(Call::Any(bits as usize, width, r));
    if let Some(replacement) = STATE.with(|s| s.borrow().any_override) {
        unsafe {
            *rect = replacement;
        }
        return 1;
    }
    if width == 0 {
        return 0x100;
    }
    for x in r.x0..=r.x1 {
        for z in r.z0..=r.z1 {
            if unsafe { bit(bits, width, x, z) } {
                return 0x101;
            }
        }
    }
    0x100
}
extern "C" fn pixel(bits: *mut u32, width: u32, x: u32, z: u32) -> u32 {
    log(Call::Pixel(bits as usize, width, x, z));
    0xA500 | u32::from(unsafe { bit(bits, width, x, z) })
}
extern "thiscall" fn grid(this: *mut u8, x: u32, z: u32, word: u32) {
    log(Call::Grid(this as usize, x, z, word));
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().message)
}
extern "thiscall" fn new(this: *mut u8) -> *mut u8 {
    log(Call::New(this as usize));
    this
}
extern "thiscall" fn add(this: *mut u8) {
    log(Call::Message(this as usize, 4, 0));
}
extern "thiscall" fn release(this: *mut u8) {
    log(Call::Message(this as usize, 8, 0));
}
macro_rules! setter {
    ($name:ident, $slot:literal) => {
        extern "thiscall" fn $name(this: *mut u8, word: u32) {
            log(Call::Message(this as usize, $slot, word));
        }
    };
}
setter!(kind, 0x14);
setter!(x0, 0x2C);
setter!(z0, 0x40);
setter!(x1, 0x54);
setter!(z1, 0x68);
extern "thiscall" fn post(this: *mut u8, message: *mut u8, flag: u32) {
    log(Call::Post(this as usize, message as usize, flag));
}

struct World {
    object: Object,
    changed: Box<[u32; 32]>,
    powered: Box<[u32; 32]>,
    wire: Box<[u32; 32]>,
    city: Box<*const usize>,
    grid: Box<*const usize>,
    server: Box<*const usize>,
    message: Box<*const usize>,
    _vt: Vec<Box<[usize]>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                width: 32,
                ..State::default()
            }
        });
        let table = |slots: &[(usize, usize)]| {
            let mut t = vec![0; slots.iter().map(|(s, _)| s / 4 + 1).max().unwrap()].into_boxed_slice();
            for &(slot, function) in slots {
                t[slot / 4] = function;
            }
            t
        };
        let cvt = table(&[(0x174, width as *const () as usize)]);
        let gvt = table(&[(0x68, grid as *const () as usize)]);
        let svt = table(&[(0x10, post as *const () as usize)]);
        let mvt = table(&[
            (4, add as *const () as usize),
            (8, release as *const () as usize),
            (0x14, kind as *const () as usize),
            (0x2C, x0 as *const () as usize),
            (0x40, z0 as *const () as usize),
            (0x54, x1 as *const () as usize),
            (0x68, z1 as *const () as usize),
        ]);
        let mut w = Self {
            object: Object::new(0x400),
            changed: Box::new([0; 32]),
            powered: Box::new([0; 32]),
            wire: Box::new([0; 32]),
            city: Box::new(cvt.as_ptr()),
            grid: Box::new(gvt.as_ptr()),
            server: Box::new(svt.as_ptr()),
            message: Box::new(mvt.as_ptr()),
            _vt: vec![cvt, gvt, svt, mvt],
            _fakes: vec![
                fake(0x0060_3E20, xor as *const () as usize),
                fake(0x0060_4350, any as *const () as usize),
                fake(0x0060_3DD0, pixel as *const () as usize),
                fake(0x0091_33DA, allocate as *const () as usize),
                fake(0x0091_34D6, new as *const () as usize),
            ],
        };
        let server = (&mut *w.server as *mut *const usize).cast::<u8>();
        // The global's stand-in must outlive every call; retain a separate word in the object fixture.
        unsafe {
            w.object.ptr().add(0x3F0).cast::<*mut u8>().write(server);
        }
        w._fakes
            .push(fake_data(0x00B4_3CCC, unsafe { w.object.ptr().add(0x3F0) }));
        let sim = w.sim();
        sim.set_city((&mut *w.city as *mut *const usize).cast());
        sim.set_grid((&mut *w.grid as *mut *const usize).cast());
        sim.set_changed_bits(w.changed.as_mut_ptr());
        sim.set_powered(w.powered.as_mut_ptr());
        sim.set_wire(w.wire.as_mut_ptr());
        STATE.with(|s| s.borrow_mut().message = (&mut *w.message as *mut *const usize).cast());
        w
    }
    fn sim(&mut self) -> PublishView {
        unsafe { PublishView::from_ptr(self.object.ptr()) }
    }
    fn run(&mut self) -> u64 {
        let mut frame = [0xA5u8; 0x4C];
        unsafe { publish_lots(self.object.ptr(), frame.as_mut_ptr()) }
    }
}
fn message_calls(message: usize, server: usize, r: Rect) -> [Call; 10] {
    [
        Call::Allocate(0x2C),
        Call::New(message),
        Call::Message(message, 4, 0),
        Call::Message(message, 0x14, 0x8A07_AE00),
        Call::Message(message, 0x2C, r.x0),
        Call::Message(message, 0x40, r.z0),
        Call::Message(message, 0x54, r.x1),
        Call::Message(message, 0x68, r.z1),
        Call::Post(server, message, 0),
        Call::Message(message, 8, 0),
    ]
}

#[test]
fn publishes_only_differing_cells_and_posts_one_message_per_changed_block() {
    let mut w = World::new();
    w.changed[0] = 1;
    w.powered[1] = 2;
    w.powered[8] = 1 << 8;
    w.wire[1] = 2;
    w.wire[8] = 1 << 8;
    let (c, p, wire) = (
        w.changed.as_ptr() as usize,
        w.powered.as_ptr() as usize,
        w.wire.as_ptr() as usize,
    );
    let (city, grid, server, message) = (
        &*w.city as *const _ as usize,
        &*w.grid as *const _ as usize,
        &*w.server as *const _ as usize,
        &*w.message as *const _ as usize,
    );
    assert_eq!(w.run(), 0x0000_0003_0000_0003);
    assert_eq!(w.sim().changed(), 1);
    assert_eq!(w.changed[0], 1);
    assert_eq!(w.changed[1], 2);
    assert_eq!(w.changed[8], 256);
    let mut expected = vec![Call::Width(city), Call::Width(city), Call::Xor(c, p, 32)];
    for bz in 0..4 {
        for bx in 0..4 {
            let rect = Rect {
                x0: bx * 8,
                z0: bz * 8,
                x1: bx * 8 + 7,
                z1: bz * 8 + 7,
            };
            expected.push(Call::Any(c, 32, rect));
            if (bx, bz) == (0, 0) || (bx, bz) == (1, 1) {
                for x in rect.x0..=rect.x1 {
                    for z in rect.z0..=rect.z1 {
                        expected.push(Call::Pixel(c, 32, x, z));
                        if [(0, 0), (1, 1), (8, 8)].contains(&(x, z)) {
                            expected.push(Call::Pixel(wire, 32, x, z));
                            expected.push(Call::Pixel(p, 32, x, z));
                            expected.push(Call::Grid(
                                grid,
                                x,
                                z,
                                0xA5A5_A500 | if (x, z) == (0, 0) { 0 } else { 2 },
                            ));
                        }
                    }
                }
                expected.extend(message_calls(message, server, rect));
            }
        }
    }
    assert_eq!(calls(), expected);
}

#[test]
fn equal_bitmaps_skip_cells_messages_and_changed_flag() {
    let mut w = World::new();
    w.changed[3] = 0xFFFF_0000;
    w.powered[3] = 0xFFFF_0000;
    w.sim().set_changed(0x7F);
    w.run();
    assert_eq!(w.sim().changed(), 0x7F);
    assert!(w.changed.iter().all(|v| *v == 0));
    assert_eq!(calls().len(), 19);
}

#[test]
fn zero_width_still_tests_the_origin_block_and_keeps_flag_bytes() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().width = 0);
    let before = unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x400).to_vec() };
    assert_eq!(w.run(), 0);
    assert_eq!(unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x400) }, before);
    assert!(matches!(
        calls().as_slice(),
        [
            Call::Width(_),
            Call::Width(_),
            Call::Xor(_, _, 0),
            Call::Any(
                _,
                0,
                Rect {
                    x0: 0,
                    z0: 0,
                    x1: 7,
                    z1: 7
                }
            )
        ]
    ));
}

#[test]
fn reversed_ranges_returned_by_block_test_suppress_notification_only_when_both_reverse() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.width = 0;
        s.any_override = Some(Rect {
            x0: 5,
            x1: 4,
            z0: 3,
            z1: 2,
        });
    });
    w.run();
    assert_eq!(w.sim().changed(), 1);
    assert_eq!(calls().len(), 4);
    STATE.with(|s| {
        s.borrow_mut().any_override = Some(Rect {
            x0: 5,
            x1: 4,
            z0: 2,
            z1: 3,
        })
    });
    w.run();
    assert_eq!(calls().len(), 14);
}

#[unsafe(naked)]
extern "thiscall" fn enter(_this: *mut u8) {
    core::arch::naked_asm!("mov dword ptr [esp - 0x3c], 0x123456a5", "jmp {entry}", entry = sym fc4p_006ebd00);
}
#[test]
fn exported_entry_preserves_scratch_upper_bytes_and_returns_block_limit_in_ecx() {
    let mut w = World::new();
    w.changed[0] = 1;
    w.wire[0] = 1;
    let (eax, ecx): (u32, u32);
    unsafe {
        core::arch::asm!("call {entry}", entry = sym enter, inlateout("ecx") w.object.ptr() => ecx,
        lateout("eax") eax, clobber_abi("C"));
    }
    assert_eq!((eax, ecx), (3, 3));
    assert!(calls().iter().any(|c| matches!(c, Call::Grid(_, 0, 0, 0x1234_5601))));
}
