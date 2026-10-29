// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Join newly placed networks to adjacent dead ends and across a single rail cell.

use super::{Port, ZoneDeveloper};
use core::{mem::MaybeUninit, ptr::null_mut};

game_fn! { fn network_at(this: *mut u8, x: i32, z: i32, mask: u32) -> *mut u8 = "thiscall" 0x0072_FCA0; }
game_fn! { #[allow(dead_code)] fn matches_network() -> u32 = "C" 0x0072_9F90; }
game_data! { static broad_network_match: u8 = 0x00B4_B391; }

/// The predicate receives its object in ESI, with no stack arguments.
unsafe fn matches(network: *mut u8) -> u32 {
    let result: u32;
    unsafe {
        core::arch::asm!("push esi", "mov esi, ecx", "call edx", "pop esi",
            in("ecx") network, in("edx") crate::abi::target(0x0072_9F90),
            lateout("eax") result, clobber_abi("C"));
    }
    result
}

unsafe fn is_type(network: *mut u8, kind: u32) -> bool {
    unsafe { vcall!(network, 0x58 => fn(u32) -> u32, kind) as u8 != 0 }
}

unsafe fn connects(network: *mut u8, edge: u32, kind: u32) -> bool {
    unsafe { vcall!(network, 0xBC => fn(u32, u32) -> u32, edge, kind) as u8 != 0 }
}

unsafe fn draw(tool: *mut u8, from: (i32, i32), to: (i32, i32)) {
    unsafe {
        vcall!(tool, 0x1C => fn(i32, i32, i32, i32, u32, u32, u32, u32),
        from.0, from.1, to.0, to.1, 0, 0, 1, u32::MAX);
    }
}

unsafe fn release(network: *mut u8, result: u32) -> u32 {
    if network.is_null() {
        result
    } else {
        unsafe { vcall!(network, 8 => fn() -> u32) }
    }
}

port! {
    /// Select the new piece's road, street or dirt tool. Visit west, north, east and south neighbors.
    /// Join a matching neighbor only when exactly one of its edges connects, on the outward side.
    /// Street wins over dirt on each edge; the mode byte enables road and one-way checks after both fail.
    /// Release the queried network even on early exits, returning that release's full EAX.
    #[port(address = 0x0073_1F60, owner = "zonedeveloper", name = "JoinDeadEndStubs")]
    pub unsafe extern "thiscall" fn fc4p_00731f60(this: *mut u8, occupant: *mut u8) -> u32 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let mut network = null_mut();
            let result = vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0xA821_EF94, &mut network);
            if result as u8 == 0 { return release(network, result); }
            let tool = if is_type(network, 0) || is_type(network, 10) {
                dev.road_tool()
            } else if is_type(network, 3) {
                dev.street_tool()
            } else {
                let result = vcall!(network, 0x58 => fn(u32) -> u32, 11);
                if result as u8 == 0 { return release(network, result); }
                dev.dirt_tool()
            };
            if tool.is_null() { return release(network, 0); }
            let result = vcall!(network, 0x44 => fn() -> u32);
            if result as u8 != 0 { return release(network, result); }
            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
            vcall!(occupant, 0x2C => fn(*mut i32), bounds.as_mut_ptr().cast());
            let [x, z, _, _] = bounds.assume_init();
            for (expected, (nx, nz)) in [
                (x.wrapping_sub(1), z), (x, z.wrapping_sub(1)),
                (x.wrapping_add(1), z), (x, z.wrapping_add(1)),
            ].into_iter().enumerate() {
                let neighbor = network_at(this, nx, nz, 0xC09);
                if neighbor.is_null() || matches(neighbor) as u8 == 0 { continue; }
                let mut count = 0u32;
                let mut last = [u32::MAX; 3];
                for edge in 0..4 {
                    if connects(neighbor, edge, 3) {
                        count += 1;
                        last[0] = edge;
                    } else if connects(neighbor, edge, 11) {
                        count += 1;
                        last[1] = edge;
                    } else if *broad_network_match() != 0
                        && (connects(neighbor, edge, 0)
                            || (*broad_network_match() != 0 && connects(neighbor, edge, 10))) {
                        count += 1;
                        last[2] = edge;
                    }
                }
                if count == 1 && last.contains(&(expected as u32)) {
                    draw(tool, (x, z), (nx, nz));
                }
            }
            release(network, 0)
        }
    }
}

port! {
    /// Visit west, north, east and south rail cells. Require only rail flags and perpendicular
    /// connections, testing all four edges in order with early exits. Prefer a street two cells away,
    /// otherwise a dirt road; use that far cell's tool for a segment toward the new piece.
    /// Both street and dirt tools must exist before inspecting the new piece's bounds.
    #[port(address = 0x0073_2450, owner = "zonedeveloper", name = "JoinAcrossRail")]
    pub unsafe extern "thiscall" fn fc4p_00732450(this: *mut u8, occupant: *mut u8) -> u32 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let mut network = null_mut();
            let result = vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0xA821_EF94, &mut network);
            if result as u8 == 0 { return release(network, result); }
            if dev.dirt_tool().is_null() || dev.street_tool().is_null() { return release(network, 0); }
            let result = vcall!(network, 0x44 => fn() -> u32);
            if result as u8 != 0 { return release(network, result); }
            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
            vcall!(occupant, 0x2C => fn(*mut i32), bounds.as_mut_ptr().cast());
            let [x, z, _, _] = bounds.assume_init();
            for (dx, dz) in [(-1i32, 0i32), (0, -1), (1, 0), (0, 1)] {
                let rail = network_at(this, x.wrapping_add(dx), z.wrapping_add(dz), 2);
                if rail.is_null() { continue; }
                if vcall!(rail, 0x30 => fn() -> u32) & 0x1F4F != 2 { continue; }
                let mut crossing = true;
                for edge in 0..4 {
                    let wanted = if dx != 0 { edge & 1 != 0 } else { edge & 1 == 0 };
                    if connects(rail, edge, 1) != wanted {
                        crossing = false;
                        break;
                    }
                }
                if !crossing { continue; }
                let far = (x.wrapping_add(dx.wrapping_mul(2)), z.wrapping_add(dz.wrapping_mul(2)));
                let street = network_at(this, far.0, far.1, 8);
                let tool = if !street.is_null() && is_type(street, 3) {
                    dev.street_tool()
                } else {
                    let dirt = network_at(this, far.0, far.1, 0x800);
                    if dirt.is_null() || !is_type(dirt, 11) { continue; }
                    dev.dirt_tool()
                };
                draw(tool, (x, z), far);
            }
            release(network, 0)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00731f60::PORT, fc4p_00732450::PORT];

#[cfg(test)]
mod tests;
