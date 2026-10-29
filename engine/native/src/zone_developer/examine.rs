// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Score a boundary cell from its networks, real lot, water and obstacle occupants.

use super::{
    network::{list_allocator, virtual_call},
    registers, Port, ZoneDeveloper,
};
use crate::abi::{Conv, RawCall};
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { #[allow(dead_code)] fn destroy_occupants(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3FE0; }

port! {
    /// Examine networks in priority order, then the real lot, water and each obstacle occupant.
    /// Update the condition before adding its score; finally add one when the condition is zero.
    /// Scores wrap as signed 32-bit values. Restore the final ECX, which boundary iteration reads.
    #[port(address = 0x0072_FAB0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::ExamineCell", naked)]
    pub unsafe extern "C" fn fc4p_0072fab0() {
        core::arch::naked_asm!("sub esp, 4", "mov edx, esp",
            "push dword ptr [esp + 20]", "push dword ptr [esp + 20]",
            "push dword ptr [esp + 20]", "push dword ptr [esp + 20]",
            "call {body}", "mov ecx, [esp]", "add esp, 4", "ret 16", body = sym examine_cell);
    }
}

unsafe extern "fastcall" fn examine_cell(
    this: *mut u8,
    out_ecx: *mut u32,
    x: i32,
    z: i32,
    condition: *mut i32,
    score: *mut i32,
) -> u64 {
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this);
        let table = read_unaligned(this.cast::<u32>());
        let mut call = virtual_call(this, 0x20, table, 0, &[x as u32, z as u32, 0x1F4F]);
        let mut add = None;
        if call.eax as u8 != 0 {
            for (flag, increment, blocked, eax_table) in [
                (0x104, 5, true, false),
                (0x808, 30, false, false),
                (0x401, 100, false, true),
                (0x40, 400, false, false),
                (2, 5, true, true),
            ] {
                let table = read_unaligned(this.cast::<u32>());
                call = virtual_call(
                    this,
                    0x20,
                    if eax_table { table } else { call.eax },
                    if eax_table { call.edx } else { table },
                    &[x as u32, z as u32, flag],
                );
                if call.eax as u8 != 0 {
                    if blocked {
                        write_unaligned(condition, 2);
                    } else if read_unaligned(condition) < 1 {
                        write_unaligned(condition, 1);
                    }
                    if flag == 2 {
                        call.ecx = condition as u32;
                    }
                    add = Some(increment);
                    break;
                }
            }
        } else {
            let lot_table = read_unaligned(this.add(4).cast::<u32>());
            call = virtual_call(this.add(4), 0x18, call.eax, lot_table, &[x as u32, z as u32, 0]);
            let mut blocked = call.eax != 0;
            if !blocked {
                call = virtual_call(
                    this,
                    0x24,
                    read_unaligned(this.cast::<u32>()),
                    call.edx,
                    &[x as u32, z as u32],
                );
                blocked = call.eax as u8 != 0;
            }
            if blocked {
                write_unaligned(condition, 2);
                call.ecx = condition as u32;
                add = Some(5);
            } else {
                let (service, _) = list_allocator(call.edx);
                let header = vcall!(service, 0x0C => fn(u32) -> *mut u8, 12);
                write_unaligned(header.cast::<*mut u8>(), header);
                write_unaligned(header.add(4).cast::<*mut u8>(), header);
                let mut list = header;
                let cell = [x, z];
                vcall!(dev.occupant_manager(), 0xA4 => fn(*mut *mut u8, *const i32, *const i32, u32, u32),
                    &mut list, cell.as_ptr(), cell.as_ptr(), 0, u32::MAX);
                let mut node = read_unaligned(list.cast::<*mut u8>());
                while node != list {
                    let occupant = read_unaligned(node.add(8).cast::<*mut u8>());
                    let kind = vcall!(occupant, 0x1C => fn() -> u32);
                    if kind == 0x2781_28A0 || kind == 0x2890_D4DE {
                        write_unaligned(condition, 2);
                        write_unaligned(score, read_unaligned(score).wrapping_add(5));
                    }
                    node = read_unaligned(node.cast::<*mut u8>());
                }
                vcall!(dev.occupant_manager(), 0xC0 => fn(*mut *mut u8), &mut list);
                call = RawCall::new(
                    Conv::Thiscall,
                    crate::abi::target(0x0041_3FE0),
                    &mut list as *mut *mut u8 as u32,
                    &[],
                );
                registers::call(&mut call);
            }
        }
        if let Some(increment) = add {
            call.eax = read_unaligned(score).wrapping_add(increment) as u32;
            write_unaligned(score, call.eax as i32);
        }
        if read_unaligned(condition) == 0 {
            write_unaligned(score, read_unaligned(score).wrapping_add(1));
        }
        write_unaligned(out_ecx, call.ecx);
        (u64::from(condition as u32) << 32) | u64::from(call.eax)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072fab0::PORT];

#[cfg(test)]
mod tests;
