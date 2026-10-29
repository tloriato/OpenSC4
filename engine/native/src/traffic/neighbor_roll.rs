// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Rollover of neighbor arrivals and trip-capacity histories.

use super::network::registers;
use super::*;
use crate::abi::{Conv, RawCall};

game_data! { static allocator: *mut u8 = 0x00B2_07DC; }
game_data! { static neighbors: *mut u8 = 0x00B4_3D8C; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_temporary(this: *mut *mut u8) -> u32 = "thiscall" 0x0041_1870; }
game_fn! { fn fill_vector(this: *mut u32, position: u32, count: u32, value: *const u32) -> u32 = "thiscall" 0x0042_3320; }
game_fn! { fn get_array(holder: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_F510; }
game_fn! { fn set_array(holder: *mut u8, id: u32, value: *const u32) -> u32 = "C" 0x005F_E6B0; }
game_fn! { fn clear_list(this: *mut u32) -> u32 = "thiscall" 0x0041_1D70; }
game_fn! { #[allow(dead_code)] fn free(pointer: u32) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn set_capacity(index: u32) -> u32 = "C" 0x0070_DE70; }

unsafe fn acquire_allocator() -> *mut u8 {
    // SAFETY: the game's cache stores a retained allocator interface.
    unsafe {
        if (*allocator()).is_null() {
            let mut temporary = core::ptr::null_mut();
            let fw = framework();
            if !fw.is_null() {
                vcall!(fw, 0x14 => fn(u32, u32, *mut *mut u8) -> u32,
                    0x3AE4_BEA3, 0x3AE4_BEAB, &mut temporary);
            }
            allocator().write(temporary);
            release_temporary(&mut temporary);
        }
        *allocator()
    }
}

/// Calls the capacity writer with its holder in EDI and its value in EBX.
unsafe fn write_capacity(holder: *mut u8, index: u32, value: u32, input: Registers) -> Registers {
    let target = crate::abi::target(0x0070_DE70);
    let mut frame = [
        target as u32,
        holder as u32,
        value,
        index,
        input.eax,
        input.ecx,
        input.edx,
    ];
    // SAFETY: the frame survives the call in ESI, and the caller owns the single stack argument.
    unsafe {
        core::arch::asm!("push esi", "push edi", "push ebx", "mov esi, ecx",
            "mov edi, [esi + 4]", "mov ebx, [esi + 8]", "push [esi + 12]",
            "mov eax, [esi + 16]", "mov ecx, [esi + 20]", "mov edx, [esi + 24]",
            "call dword ptr [esi]", "add esp, 4", "mov [esi + 16], eax", "mov [esi + 20], ecx", "mov [esi + 24], edx",
            "pop ebx", "pop edi", "pop esi", in("ecx") frame.as_mut_ptr(), clobber_abi("C"));
    }
    Registers {
        eax: frame[4],
        ecx: frame[5],
        edx: frame[6],
    }
}

port! {
    /// Copies existing arrival arrays to their preceding-cycle properties, clears three nine-word arrays,
    /// then moves capacity elements 4..7 to 8..11 and clears 4..7. Visits the retained connection list in order.
    /// The unused budget word is popped. AL becomes one; the last cleanup call supplies the other registers.
    #[port(address = 0x0071_4BE0, owner = "traffic", name = "cSC4TrafficSimulator::WorkOnUpdateNeighborConnections", naked)]
    pub unsafe extern "C" fn fc4p_00714be0() {
        core::arch::naked_asm!("sub esp, 12", "mov edx, esp", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 4", sym roll_neighbors);
    }
}

unsafe extern "fastcall" fn roll_neighbors(_this: *mut u8, out: *mut Registers) {
    // SAFETY: the game's services provide the list and property storage; all temporary containers use its heap.
    unsafe {
        let zero = 0u32;
        let mut previous = [0u32; 3];
        let mut empty = [0u32; 3];
        fill_vector(previous.as_mut_ptr(), 0, 9, &zero);
        let length = (empty[1].wrapping_sub(empty[0]) as i32 >> 2) as u32;
        if length > 9 {
            empty[1] = empty[0].wrapping_add(36);
        } else {
            fill_vector(empty.as_mut_ptr(), empty[1], 9u32.wrapping_sub(length), &zero);
        }
        let alloc = acquire_allocator();
        let sentinel = vcall!(alloc, 0x0C => fn(u32) -> *mut u8, 12);
        store(sentinel, 0, sentinel as u32);
        store(sentinel, 4, sentinel as u32);
        let mut list = [sentinel as u32, 0];
        vcall!(*neighbors(), 0x40 => fn(*mut u32) -> u32, list.as_mut_ptr());
        let head = list[0] as *mut u8;
        let mut node = word(head, 0) as *mut u8;
        while node != head {
            let conn = word(node, 8) as *mut u8;
            let holder = vcall!(conn, 0x0C => fn() -> *mut u8);
            for id in 0xAA9E_A81Au32..0xAA9E_A81D {
                if get_array(holder, id, previous.as_mut_ptr()) as u8 != 0 {
                    set_array(holder, id.wrapping_add(3), previous.as_ptr());
                }
                set_array(holder, id, empty.as_ptr());
            }
            for index in 4u32..8 {
                let mut result = virtual_registers(holder, 0x24, &[0x8A0B_4100], 0, word(holder, 0));
                let property = result.eax as *mut u8;
                let mut value = 0;
                if !property.is_null() {
                    result = virtual_registers(property, 0x14, &[], property as u32, word(property, 0));
                    let variant = result.eax as *mut u8;
                    result = virtual_registers(variant, 0x0C, &[], word(variant, 0), result.edx);
                    let narrow = result.eax == 0x8004;
                    result = virtual_registers(
                        variant,
                        if narrow { 0x18C } else { 0x19C },
                        &[],
                        result.eax,
                        word(variant, 0),
                    );
                    result.ecx = if narrow { index * 2 } else { index * 4 };
                    value = if narrow {
                        u32::from((result.eax as *mut u16).add(index as usize).read_unaligned())
                    } else {
                        (result.eax as *mut u32).add(index as usize).read_unaligned()
                    };
                }
                let result = write_capacity(holder, index + 4, value, result);
                write_capacity(holder, index, 0, result);
            }
            node = word(node, 0) as *mut u8;
        }
        clear_list(list.as_mut_ptr());
        let alloc = acquire_allocator();
        let mut result = virtual_registers(alloc, 0x10, &[list[0]], word(alloc, 0), 0);
        result.eax = empty[0];
        if result.eax != 0 {
            let args = [empty[0]];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), result.ecx, &args);
            call.eax = result.eax;
            call.edx = result.edx;
            result = registers(call);
        }
        result.eax = previous[0];
        if result.eax != 0 {
            let args = [previous[0]];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), result.ecx, &args);
            call.eax = result.eax;
            call.edx = result.edx;
            result = registers(call);
        }
        result.eax = (result.eax & !0xFF) | 1;
        out.write(result);
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_00714be0::PORT];

#[cfg(test)]
mod tests;
