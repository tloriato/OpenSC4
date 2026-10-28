// SPDX-License-Identifier: GPL-3.0-or-later
//! Native-layout versions of the core's message and visibility cases.

use super::*;
use crate::testing::{fake, Object};
use std::cell::RefCell;

const RETURN: u64 = 0xDDEE_FF00_AABB_CC42;
const MESSAGE_EDX: u32 = 0x1234_5678;
const MESSAGE_ECX: u32 = 0x9876_5432;

#[derive(Debug, PartialEq)]
enum Call {
    Type,
    Occupant,
    View,
    Dispatch(u32, u32, u32, Vec<u32>),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
fn log(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

#[repr(C)]
struct Message {
    vtable: *const usize,
    id: u32,
    occupant: u32,
    level: *mut u32,
}

extern "thiscall" fn message_type(this: *mut Message) -> u64 {
    log(Call::Type);
    // SAFETY: the test message.
    registers(unsafe { (*this).id }, MESSAGE_EDX)
}
extern "thiscall" fn occupant_body(this: *mut Message) -> u64 {
    log(Call::Occupant);
    // SAFETY: the test message.
    registers(unsafe { (*this).occupant }, MESSAGE_EDX)
}
#[unsafe(naked)]
unsafe extern "thiscall" fn message_occupant(_: *mut Message) -> u64 {
    core::arch::naked_asm!("call {body}", "mov ecx, {ecx}", "ret",
        body = sym occupant_body, ecx = const MESSAGE_ECX);
}
extern "thiscall" fn message_view(this: *mut Message) -> u32 {
    log(Call::View);
    // SAFETY: the test message.
    unsafe { (*this).level as u32 }
}

macro_rules! handler {
    ($name:ident, $address:literal) => {
        extern "fastcall" fn $name(this: *mut u8, edx: u32) -> u64 {
            log(Call::Dispatch($address, this as u32, edx, vec![]));
            RETURN
        }
    };
}
handler!(day, 0x006E_7FD0);
handler!(month, 0x006E_7B60);
handler!(begin, 0x006E_44B0);
handler!(visuals, 0x006E_63C0);
handler!(tuning, 0x006E_5B10);
extern "fastcall" fn changed(this: *mut u8, edx: u32, occupant: u32, insert: u32) -> u64 {
    log(Call::Dispatch(0x006E_4680, this as u32, edx, vec![occupant, insert]));
    RETURN
}

struct World {
    object: Object,
    message: Box<Message>,
    level: Box<u32>,
    _table: Box<[usize; 16]>,
    _fakes: Vec<crate::testing::Fake>,
}
impl World {
    fn new() -> Self {
        let mut table = Box::new([0; 16]);
        table[0x10 / 4] = message_type as *const () as usize;
        table[0x28 / 4] = message_occupant as *const () as usize;
        table[0x3C / 4] = message_view as *const () as usize;
        let mut level = Box::new(1);
        let message = Box::new(Message {
            vtable: table.as_ptr(),
            id: 0,
            occupant: 0xFFFF_8765,
            level: &mut *level,
        });
        let fakes = [
            (0x006E_7FD0, day as *const () as usize),
            (0x006E_7B60, month as *const () as usize),
            (0x006E_44B0, begin as *const () as usize),
            (0x006E_63C0, visuals as *const () as usize),
            (0x006E_5B10, tuning as *const () as usize),
            (0x006E_4680, changed as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        take();
        Self {
            object: Object::new(0x240),
            message,
            level,
            _table: table,
            _fakes: fakes,
        }
    }
    fn run(&mut self, id: u32) -> u64 {
        self.message.id = id;
        // SAFETY: the test's secondary interface and complete message.
        unsafe {
            fc4p_006e8e20(
                self.object.ptr().add(4),
                0xDEAD_BEEF,
                (&mut *self.message as *mut Message).cast(),
            )
        }
    }
    fn bytes(&mut self) -> Vec<u8> {
        // SAFETY: the object's complete allocation.
        unsafe { std::slice::from_raw_parts(self.object.ptr(), 0x240).to_vec() }
    }
}

#[test]
fn messages_dispatch_in_order_with_the_primary_interface_and_keep_result_words() {
    let mut w = World::new();
    let base = w.object.ptr() as u32;
    for (id, address) in [
        (0x6695_6814, 0x006E_7FD0),
        (0x6695_6816, 0x006E_7B60),
        (0x6695_6810, 0x006E_44B0),
        (0x26D3_1EC1, 0x006E_63C0),
        (0xCA1B_151C, 0x006E_5B10),
    ] {
        let before = w.bytes();
        assert_eq!(w.run(id), (RETURN & !0xFF) | 1);
        assert_eq!(take(), [Call::Type, Call::Dispatch(address, base, MESSAGE_EDX, vec![])]);
        assert_eq!(w.bytes(), before);
    }
}

#[test]
fn inserted_and_removed_messages_keep_the_getters_ecx_upper_bytes_in_the_boolean_word() {
    let mut w = World::new();
    let base = w.object.ptr() as u32;
    for (id, low) in [(0x99EF_1142, 1), (0x99EF_1143, 0)] {
        assert_eq!(w.run(id), (RETURN & !0xFF) | 1);
        assert_eq!(
            take(),
            [
                Call::Type,
                Call::Occupant,
                Call::Dispatch(
                    0x006E_4680,
                    base,
                    MESSAGE_EDX,
                    vec![w.message.occupant, (MESSAGE_ECX & !0xFF) | low]
                )
            ]
        );
    }
}

#[test]
fn view_visibility_uses_void2_and_only_changes_the_visibility_byte_when_needed() {
    let mut w = World::new();
    let base = w.object.ptr() as u32;
    for (stored, level) in [(0, 1), (1, 1), (1, 3), (0, 3), (2, 1), (255, 0), (0, u32::MAX)] {
        // SAFETY: the test's native object.
        let sim = unsafe { PollutionSim::from_ptr(w.object.ptr()) };
        sim.set_effects_visible(stored);
        *w.level = level;
        let mut expected_bytes = w.bytes();
        let visible = u8::from(level == 1);
        let changed = stored != visible;
        let want = if changed {
            (RETURN & !0xFF) | 1
        } else {
            registers((w.message.level as u32 & !0xFF) | 1, level)
        };
        assert_eq!(w.run(0xC6CD_BF1A), want);
        let mut calls = vec![Call::Type, Call::View];
        if changed {
            calls.push(Call::Dispatch(0x006E_63C0, base, level, vec![]));
        }
        assert_eq!(take(), calls);
        expected_bytes[0x20C] = visible;
        assert_eq!(w.bytes(), expected_bytes);
    }
}

#[test]
fn unhandled_message_boundaries_succeed_with_the_type_getters_upper_eax_and_edx() {
    let mut w = World::new();
    for id in [
        0,
        u32::MAX,
        0x26D3_1EC0,
        0x26D3_1EC2,
        0x6695_680F,
        0x6695_6811,
        0x6695_6813,
        0x6695_6815,
        0x6695_6817,
        0x99EF_1141,
        0x99EF_1144,
        0xC6CD_BF19,
        0xC6CD_BF1B,
        0xCA1B_151B,
        0xCA1B_151D,
    ] {
        let before = w.bytes();
        assert_eq!(w.run(id), registers((id & !0xFF) | 1, MESSAGE_EDX));
        assert_eq!(take(), [Call::Type]);
        assert_eq!(w.bytes(), before);
    }
}

#[test]
fn average_air_getter_reads_the_full_word_preserves_edx_and_writes_nothing() {
    let mut object = Object::new(0x240);
    // SAFETY: the allocation contains the simulator's average-air word.
    unsafe {
        let sim = PollutionSim::from_ptr(object.ptr());
        for value in [0, 500, 0x8000_0000, u32::MAX] {
            sim.set_average_air(value);
            let before = std::slice::from_raw_parts(object.ptr(), 0x240).to_vec();
            let mut call = RawCall::new(
                Conv::Thiscall,
                fc4p_007624c0 as *const () as usize,
                object.ptr() as u32,
                &[],
            );
            call.eax = 0x1357_2468;
            call.edx = MESSAGE_EDX;
            call_raw(&mut call);
            assert_eq!(registers(call.eax, call.edx), registers(value, MESSAGE_EDX));
            assert_eq!(std::slice::from_raw_parts(object.ptr(), 0x240), before);
        }
    }
}
