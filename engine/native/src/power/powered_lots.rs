// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Publish changed power cells and notify one eight-cell block at a time.

use super::*;
use core::ptr::read_unaligned;

layout! {
    /// Primary-interface fields used when publishing the power bitmap.
    struct PublishView {
        0x18 => city / set_city: *mut u8,
        0x80 => grid / set_grid: *mut u8,
        0x2C8 => wire / set_wire: *mut u32,
        0x2D0 => changed_bits / set_changed_bits: *mut u32,
        0x2D4 => powered / set_powered: *mut u32,
        0x2D9 => changed / set_changed: u8,
    }
}
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }
game_fn! { fn bitmap_xor(out: *mut u32, source: *mut u32, width: u32) = "C" 0x0060_3E20; }
game_fn! { fn bitmap_any(bits: *mut u32, width: u32, rect: *mut Rect) -> u32 = "C" 0x0060_4350; }
game_fn! { fn pixel_test(bits: *mut u32, width: u32, x: u32, z: u32) -> u32 = "C" 0x0060_3DD0; }
game_fn! { fn message_allocate(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn message_new(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }

port! {
    /// Preserve the original temporary word's upper bytes when passing a byte-valued grid state as a
    /// full stack word. The reserved frame keeps those incoming bytes intact for the Rust body.
    /// EAX and ECX both finish with the inclusive upper block index; the callee-saved registers survive.
    #[port(address = 0x006E_BD00, owner = "power", name = "cSC4PowerSimulator::UpdatePoweredLots", naked)]
    pub unsafe extern "C" fn fc4p_006ebd00() {
        core::arch::naked_asm!(
            "sub esp, 0x3c", "push ebx", "push ebp", "push esi", "push edi",
            "mov edx, esp", "call {body}", "mov ecx, edx",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0x3c", "ret",
            body = sym publish_lots,
        );
    }
}

unsafe extern "fastcall" fn publish_lots(this: *mut u8, frame: *mut u8) -> u64 {
    // SAFETY: the entry reserves the original frame, and the game supplies live services and bitmaps.
    unsafe {
        let sim = PublishView::from_ptr(this);
        let server = *message_server();
        let width = vcall!(sim.city(), 0x174 => fn() -> u32);
        let xor_width = vcall!(sim.city(), 0x174 => fn() -> u32);
        bitmap_xor(sim.changed_bits(), sim.powered(), xor_width);
        let max = ((width >> 3).wrapping_sub(1) as i32).max(0);
        let scratch = frame.add(0x10).cast::<u32>();
        let mut bz = 0i32;
        while bz <= max {
            let mut bx = 0i32;
            while bx <= max {
                let x0 = bx.wrapping_mul(8) as u32;
                let z0 = bz.wrapping_mul(8) as u32;
                let mut rect = Rect {
                    x0,
                    z0,
                    x1: x0.wrapping_add(7),
                    z1: z0.wrapping_add(7),
                };
                if bitmap_any(sim.changed_bits(), width, &mut rect) as u8 != 0 {
                    let mut x = rect.x0 as i32;
                    while x <= rect.x1 as i32 {
                        let mut z = rect.z0 as i32;
                        while z <= rect.z1 as i32 {
                            if pixel_test(sim.changed_bits(), width, x as u32, z as u32) as u8 != 0 {
                                let wired = pixel_test(sim.wire(), width, x as u32, z as u32) as u8;
                                let powered = pixel_test(sim.powered(), width, x as u32, z as u32) as u8;
                                let state = if wired == 0 { 0 } else { 1 + u8::from(powered != 0) };
                                scratch.cast::<u8>().write(state);
                                let word = read_unaligned(scratch);
                                vcall!(sim.grid(), 0x68 => fn(u32, u32, u32), x as u32, z as u32, word);
                            }
                            z = z.wrapping_add(1);
                        }
                        x = x.wrapping_add(1);
                    }
                    // The original notification is suppressed only if both ranges are reversed.
                    if rect.x0 as i32 <= rect.x1 as i32 || rect.z0 as i32 <= rect.z1 as i32 {
                        let storage = message_allocate(0x2C);
                        let message = if storage.is_null() {
                            core::ptr::null_mut()
                        } else {
                            message_new(storage)
                        };
                        vcall!(message, 4 => fn());
                        vcall!(message, 0x14 => fn(u32), 0x8A07_AE00);
                        for (slot, value) in [(0x2C, rect.x0), (0x40, rect.z0), (0x54, rect.x1), (0x68, rect.z1)] {
                            vcall!(message, slot => fn(u32), value);
                        }
                        vcall!(server, 0x10 => fn(*mut u8, u32), message, 0);
                        vcall!(message, 8 => fn());
                    }
                    sim.set_changed(1);
                }
                bx = bx.wrapping_add(1);
            }
            bz = bz.wrapping_add(1);
        }
        (u64::from(max as u32) << 32) | u64::from(max as u32)
    }
}

pub const PORTS: &[Port] = &[fc4p_006ebd00::PORT];

#[cfg(test)]
mod tests;
