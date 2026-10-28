// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Road-facing lot placement and its caller-owned orientation list.

use super::{virtual_registers, Port, ZoneDeveloper};
use core::ptr::{read_unaligned, write_unaligned};

game_data! { static random: u8 = 0x00B4_B398; }
game_fn! { fn uniform(this: *mut u8, limit: u32) -> i32 = "thiscall" 0x0091_36B1; }
game_fn! { fn allocate_node(size: u32) -> *mut u8 = "C" 0x0090_CF54; }

layout! {
    /// Circular list links and the orientation payload; the sentinel has only links.
    struct OrientationNode {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => facing / set_facing: i32,
    }
}

port! {
    /// Fill an empty orientation list with four cyclic choices after one random draw. Try each side's
    /// neighbouring road cells in ascending order; create once, using the first choice if none has a road.
    /// The list remains populated for the caller. Forward the lot creator's complete EDX:EAX result.
    #[port(address = 0x0072_D200, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CreateLotToFaceRoad")]
    pub unsafe extern "thiscall" fn fc4p_0072d200(
        this: *mut u8, x: i32, z: i32, width: i32, height: i32,
        list: *mut *mut u8, out: *mut *mut u8,
    ) -> u64 {
        // SAFETY: the caller supplies the developer, circular list and the lot output destination.
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let header = read_unaligned(list);
            if read_unaligned(header.cast::<*mut u8>()) == header {
                let mut facing = uniform(random(), 4);
                for _ in 0..4 {
                    // The caller's list header is read again after each allocation.
                    let header = read_unaligned(list);
                    let node = allocate_node(12);
                    let value = node.wrapping_add(8).cast::<i32>();
                    if !value.is_null() {
                        write_unaligned(value, facing);
                    }
                    let previous = read_unaligned(header.add(4).cast::<*mut u8>());
                    write_unaligned(node.cast::<*mut u8>(), header);
                    write_unaligned(node.add(4).cast::<*mut u8>(), previous);
                    facing = facing.wrapping_add(1);
                    write_unaligned(previous.cast::<*mut u8>(), node);
                    write_unaligned(header.add(4).cast::<*mut u8>(), node);
                    if facing > 3 {
                        facing = 0;
                    }
                }
            }
            let z2 = z.wrapping_add(height).wrapping_sub(1);
            let mut node = read_unaligned(read_unaligned(list).cast::<*mut u8>());
            let mut selected = None;
            while node != read_unaligned(list) {
                let facing = OrientationNode::from_ptr(node).facing();
                let x2 = x.wrapping_add(width).wrapping_sub(1);
                let (mut a, end, fixed, horizontal, permitted) = match facing {
                    0 => (x, x2, z.wrapping_sub(1), true, z > 0),
                    1 => (z, z2, x2.wrapping_add(1), false, x2 < dev.cells_x().wrapping_sub(1)),
                    2 => (x, x2, z2.wrapping_add(1), true, z2 < dev.cells_z().wrapping_sub(1)),
                    3 => (z, z2, x.wrapping_sub(1), false, x > 0),
                    _ => (0, -1, 0, false, false),
                };
                if permitted {
                    while a <= end {
                        let (cx, cz) = if horizontal { (a, fixed) } else { (fixed, a) };
                        if vcall!(this, 0x20 => fn(i32, i32, u32) -> u32, cx, cz, 0xC49) as u8 != 0 {
                            selected = Some(facing);
                            break;
                        }
                        a = a.wrapping_add(1);
                    }
                }
                if selected.is_some() {
                    break;
                }
                node = read_unaligned(node.cast::<*mut u8>());
            }
            let facing = selected.unwrap_or_else(|| {
                let front = read_unaligned(read_unaligned(list).cast::<*mut u8>());
                read_unaligned(front.add(8).cast::<i32>())
            });
            let edx = if selected.is_some() { dev.lot_manager_vtable() } else { z as u32 };
            let eax = if selected.is_some() { z as u32 } else { width as u32 };
            virtual_registers(this.add(4), 0x58, eax, edx,
                &[x as u32, z as u32, width as u32, height as u32, facing as u32,
                  out as u32, 0, 0, 0])
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0072d200::PORT];

#[cfg(test)]
mod tests;
