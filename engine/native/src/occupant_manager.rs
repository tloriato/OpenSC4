// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! The occupant manager's interface query and standard-cell search, plus pair-deque storage cleanup.
//! The search keeps the embedded lock held through filtering, geometry calls and retaining the result.

use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// The manager's primary interface and shared state.
    pub struct OccupantManager {
        0x00 => vtable / set_vtable: u32,
        0x04 => serializable_vtable / set_serializable_vtable: u32,
        0x08 => message_vtable / set_message_vtable: u32,
        0x0C => initialized / set_initialized: u8,
        0x10 => references / set_references: u32,
        0x14 => lock_vtable / set_lock_vtable: u32,
        0x1C => width / set_width: f32,
        0x20 => depth / set_depth: f32,
        0x24 => cells_x / set_cells_x: i32,
        0x28 => cells_z / set_cells_z: i32,
        0x2C => standard_cells_x / set_standard_cells_x: i32,
        0x30 => standard_cells_z / set_standard_cells_z: i32,
        0x34 => filter / set_filter: *mut u8,
        0x38 => rows / set_rows: *mut u8,
    }
}

port! {
    /// Query the primary, serializable or message interface and retain the manager on success.
    /// A failed query leaves the output untouched and clears only AL of the adjusted IID.
    #[port(address = 0x0049_D910, owner = "occupantmanager", name = "cSC4OccupantManager::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_0049d910(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        unsafe {
            let offset = match iid {
                1 | 0xC6BB_8334 => 0,
                0x4522_94AA => 8,
                0xE4FD_A3D4 => 4,
                _ => return (if iid < 0xC6BB_8334 { iid.wrapping_sub(0x4522_94AA) } else { iid }) & !255,
            };
            write_unaligned(out, if this.is_null() { this } else { this.add(offset) });
            let result = vcall!(this, 4 => fn() -> u32);
            (result & !255) | 1
        }
    }
}

// Supply the caller's argument words to Rust; the body has no stack arguments of its own.
macro_rules! frame_port {
    ($address:literal, $export:ident, $body:ident, $pop:literal, $name:literal) => {
        port! {
            /// Preserve the original argument slots used as coordinate scratch storage.
            #[port(address = $address, owner = "occupantmanager", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "mov edx, esp", "push eax", "call {body}", concat!("ret ", stringify!($pop)),
                    body = sym $body,
                )
            }
        }
    };
}

game_data! { static query_inset: f32 = 0x00A8_F768; }

/// The stored float comparison retains the x87 condition codes and NaN handling.
fn compare_float(a: f32, b: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!("fld dword ptr [{a}]", "fcomp dword ptr [{b}]", "fnstsw ax",
            a = in(reg) &a, b = in(reg) &b, out("ax") status, out("st(0)") _);
    }
    status
}

// Null corners cover the city's standard-cell extent. Other corners are clipped to the city counts.
// The query box is inset on x and z, and candidates are visited in cell and list order. The caller's
// min, max and filter argument words become the upper grid coordinates and the inner-loop cursor.
frame_port!(
    0x0049_E800,
    fc4p_0049e800,
    first_in_standard_cells,
    16,
    "cSC4OccupantManager::GetFirstOccupantByStandardCityCells"
);

unsafe extern "fastcall" fn first_in_standard_cells(this: *mut u8, stack: *mut u32, _eax: u32) -> u32 {
    unsafe {
        let manager = OccupantManager::from_ptr(this);
        let lock = this.add(0x14);
        vcall!(lock, 4 => fn() -> u32);
        if manager.initialized() != 0 {
            let min = *stack.add(2) as *const i32;
            let (mut x0, mut z0) = (0i32, 0i32);
            if !min.is_null() {
                x0 = read_unaligned(min);
                z0 = read_unaligned(min.add(1));
                if x0 < 0 {
                    x0 = 0;
                }
                if z0 < 0 {
                    z0 = 0;
                }
                if x0 >= manager.standard_cells_x() {
                    x0 = manager.standard_cells_x().wrapping_sub(1);
                }
                if z0 >= manager.standard_cells_z() {
                    z0 = manager.standard_cells_z().wrapping_sub(1);
                }
            }
            let max = *stack.add(3) as *const i32;
            let (x1, z1) = if !max.is_null() {
                let (mut x, mut z) = (read_unaligned(max), read_unaligned(max.add(1)));
                *stack.add(3) = x as u32;
                *stack.add(2) = z as u32;
                if x < 0 {
                    x = 0;
                    *stack.add(3) = 0;
                }
                if z < 0 {
                    z = 0;
                    *stack.add(2) = 0;
                }
                if x >= manager.standard_cells_x() {
                    x = manager.standard_cells_x().wrapping_sub(1);
                    *stack.add(3) = x as u32;
                }
                if z >= manager.standard_cells_z() {
                    z = manager.standard_cells_z().wrapping_sub(1);
                    *stack.add(2) = z as u32;
                }
                (x, z)
            } else {
                let pair = (manager.standard_cells_x(), manager.standard_cells_z());
                *stack.add(3) = pair.0 as u32;
                *stack.add(2) = pair.1 as u32;
                pair
            };
            let inset = F80::from_f32(*query_inset());
            let high_x = F80::from_i32(x1.wrapping_add(1).wrapping_shl(4)).to_f32();
            let high_z = F80::from_i32(z1.wrapping_add(1).wrapping_shl(4)).to_f32();
            let low_x = (F80::from_i32(x0.wrapping_shl(4)) + inset).to_f32();
            let low_z = (F80::from_i32(z0.wrapping_shl(4)) + inset).to_f32();
            let high_x = (F80::from_f32(high_x) - inset).to_f32();
            let high_z = (F80::from_f32(high_z) - inset).to_f32();
            vcall!(this, 0x3C => fn(f32, f32, *mut i32, *mut i32) -> u32,
                low_x, low_z, &mut x0, &mut z0);
            vcall!(this, 0x3C => fn(f32, f32, *mut u32, *mut u32) -> u32,
                high_x, high_z, stack.add(3), stack.add(2));
            let mut x = x0;
            if x <= *stack.add(3) as i32 {
                let filter = *stack.add(4) as *mut u8;
                loop {
                    let mut z = z0;
                    *stack.add(4) = z as u32;
                    while z <= *stack.add(2) as i32 {
                        let row = read_unaligned(
                            manager
                                .rows()
                                .wrapping_offset(x.wrapping_mul(12) as isize)
                                .cast::<*mut u8>(),
                        );
                        let holder = read_unaligned(row.wrapping_offset(z.wrapping_mul(4) as isize).cast::<*mut u8>());
                        if !holder.is_null() {
                            let head = read_unaligned(holder.cast::<*mut u8>());
                            let mut node = read_unaligned(head.cast::<*mut u8>());
                            while node != head {
                                let occupant = (read_unaligned(node.add(8).cast::<u32>()) & !1) as *mut u8;
                                if filter.is_null() || vcall!(filter, 0x0C => fn(*mut u8) -> u32, occupant) as u8 != 0 {
                                    let mut min = [0.0f32; 3];
                                    let mut max = [0.0f32; 3];
                                    vcall!(occupant, 0x28 => fn(*mut f32, *mut f32) -> u32, min.as_mut_ptr(), max.as_mut_ptr());
                                    if compare_float(min[0], high_x) & 0x100 != 0
                                        && compare_float(low_x, max[0]) & 0x100 != 0
                                        && compare_float(min[2], high_z) & 0x100 != 0
                                        && compare_float(low_z, max[2]) & 0x100 != 0
                                    {
                                        write_unaligned(*stack.add(1) as *mut *mut u8, occupant);
                                        vcall!(occupant, 4 => fn() -> u32);
                                        let result = vcall!(lock, 8 => fn() -> u32);
                                        return (result & !255) | 1;
                                    }
                                }
                                node = read_unaligned(node.cast::<*mut u8>());
                            }
                        }
                        z = z.wrapping_add(1);
                        *stack.add(4) = z as u32;
                    }
                    x = x.wrapping_add(1);
                    if x > *stack.add(3) as i32 {
                        break;
                    }
                }
            }
        }
        vcall!(lock, 8 => fn() -> u32) & !255
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DequeIterator {
    current: u32,
    begin: u32,
    end: u32,
    map: u32,
}

game_fn! { fn deque_destroy_range(first: DequeIterator, last: DequeIterator, allocator: *mut u8) = "C" 0x0049_ECC0; }
game_fn! { fn deque_free_blocks(this: *mut u8, first: u32, last: u32) -> u32 = "thiscall" 0x006D_A8D0; }
game_fn! { fn delete_storage(storage: *mut u8) -> u32 = "C" 0x0090_CF63; }

port! {
    /// Destroy the deque's entries, release its blocks, then release its map allocation.
    #[port(address = 0x0049_F8A0, owner = "occupantmanager", name = "DequePairStorageDestructor")]
    pub unsafe extern "thiscall" fn fc4p_0049f8a0(this: *mut u8) {
        unsafe {
            let last = read_unaligned(this.add(16).cast::<DequeIterator>());
            let first = read_unaligned(this.cast::<DequeIterator>());
            let mut allocator = core::mem::MaybeUninit::<u8>::uninit();
            deque_destroy_range(first, last, allocator.as_mut_ptr());
            if read_unaligned(this.add(32).cast::<u32>()) != 0 {
                let first = read_unaligned(this.add(12).cast::<u32>());
                let last = read_unaligned(this.add(28).cast::<u32>()).wrapping_add(4);
                deque_free_blocks(this, first, last);
                let storage = read_unaligned(this.add(32).cast::<*mut u8>());
                if !storage.is_null() { delete_storage(storage); }
            }
        }
    }
}

/// Replacements verified against the available captured calls and generated variants.
pub const PORTS: &[Port] = &[fc4p_0049d910::PORT, fc4p_0049e800::PORT, fc4p_0049f8a0::PORT];
