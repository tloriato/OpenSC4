// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Deferred lighting transitions entered with the message handler's saved-register frame still live.

use super::queue::QueueIterator;
use super::*;

game_fn! { #[allow(dead_code)] fn queue_end(this: *mut u8, output: *mut QueueIterator) -> u64 = "thiscall" 0x0043_1AC0; }
game_fn! { #[allow(dead_code)] fn queue_begin(this: *mut u8, output: *mut QueueIterator) -> u64 = "thiscall" 0x006C_5FE0; }
game_fn! { #[allow(dead_code)] fn shuffle(first: QueueIterator, last: QueueIterator) -> u64 = "C" 0x006C_68C0; }

unsafe fn direct(address: u32, conv: Conv, this: u32, args: &[u32], previous: RawCall) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = previous.eax;
    call.edx = previous.edx;
    unsafe { call_raw(&mut call) };
    call
}

macro_rules! transition_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $mode:literal) => {
        port! {
            /// EDI is the message interface. ESP points at saved EDI, ESI, EBP and EBX, followed by
            /// 64 local bytes, the caller's return address and one callee-popped argument word.
            #[port(address = $address, owner = "lotdeveloper", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 64", "mov [esp], eax", "mov [esp + 4], edx",
                    "lea edx, [esp + 64]", "mov ecx, edi", "push esp", "call {}",
                    "add esp, 64", "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 64", "ret 4",
                    sym $body,
                );
            }
        }
        unsafe extern "fastcall" fn $body(interface: *mut u8, frame: *mut u8, registers: *const [u32; 2]) -> u64 {
            unsafe { transition(interface, frame, registers.read_unaligned(), $mode) }
        }
    };
}

transition_port!(0x006C_DA13, fc4p_006cda13, "BeginDeferredDayLighting", begin_day, 1);
transition_port!(0x006C_DAD7, fc4p_006cdad7, "BeginDeferredNightLighting", begin_night, 2);

unsafe fn transition(interface: *mut u8, frame: *mut u8, registers: [u32; 2], mode: u32) -> u64 {
    unsafe {
        let queue = interface.add(0x60);
        interface.add(0x98).cast::<u32>().write_unaligned(mode);
        let mut call = RawCall::new(Conv::Thiscall, 0, queue as u32, &[]);
        call.eax = registers[0];
        call.edx = registers[1];
        let end = frame.sub(16).cast::<u32>();
        call = direct(0x0043_1AC0, Conv::Thiscall, queue as u32, &[end as u32], call);
        let begin = frame.sub(32).cast::<u32>();
        call = direct(0x006C_5FE0, Conv::Thiscall, queue as u32, &[begin as u32], call);
        let mut range = [0u32; 8];
        for (i, word) in range.iter_mut().enumerate() {
            *word = begin.add(i).read_unaligned();
        }
        call = direct(0x006C_68C0, Conv::Cdecl, 0, &range, call);
        let output = frame.add(if mode == 1 { 0x30 } else { 0x40 }).cast::<u32>();
        if mode == 2 {
            call.eax = output as u32;
        }
        call = direct(0x006C_5FE0, Conv::Thiscall, queue as u32, &[output as u32], call);
        let returned = call.eax as *const u32;
        let cursor = interface.add(0x88).cast::<u32>();
        for i in 0..4 {
            let word = returned.add(i).read_unaligned();
            cursor.add(i).write_unaligned(word);
            if (mode == 1 && i == 2) || (mode == 2 && i == 3) {
                call.edx = word;
            }
            if mode == 1 && i == 3 {
                call.eax = word;
            }
        }
        call.eax = call.eax & !255 | 1;
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006cda13::PORT, fc4p_006cdad7::PORT];
