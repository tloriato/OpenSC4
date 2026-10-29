// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Neighbor simulator message dispatch and regional city reset notifications.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::abi::{call_raw, target, Conv, RawCall};
use crate::port::Port;

game_fn! { #[allow(dead_code)] fn clear_deals(this: *mut u8) = "thiscall" 0x0042_1660; }
game_fn! { #[allow(dead_code)] fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_reference(this: *mut u8) = "thiscall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn destroy_cities(this: *mut u8) = "thiscall" 0x0041_1FE0; }
game_fn! { #[allow(dead_code)] fn reference_output(this: *mut u8) -> *mut u8 = "thiscall" 0x004F_20A0; }
game_fn! { #[allow(dead_code)] fn terrain_changed(this: *mut u8, rect: *mut i32) = "thiscall" 0x006D_0BE0; }
game_fn! { #[allow(dead_code)] fn push_deals(this: *mut u8, deals: u32, stats: u32) = "thiscall" 0x006D_0A40; }
game_fn! { #[allow(dead_code)] fn simulate_deals(this: *mut u8) = "thiscall" 0x006D_0E50; }
game_data! { static city_allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static region_manager: *mut u8 = 0x00B4_3C94; }

unsafe fn call(address: u32, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> u64 {
    let mut c = RawCall::new(
        if address == 0x0087_93EC {
            Conv::Cdecl
        } else {
            Conv::Thiscall
        },
        target(address),
        this as u32,
        args,
    );
    c.eax = eax;
    c.edx = edx;
    unsafe {
        call_raw(&mut c);
    }
    pair(c.eax, c.edx)
}

unsafe fn allocator(mut result: u64) -> (*mut u8, u64) {
    unsafe {
        if city_allocator().read().is_null() {
            let mut local = 0u32;
            result = call(
                0x0087_93EC,
                core::ptr::null_mut(),
                &[],
                result as u32,
                (result >> 32) as u32,
            );
            let framework = result as u32 as *mut u8;
            if !framework.is_null() {
                result = virtual_call(
                    framework,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, &mut local as *mut u32 as u32],
                    result as u32,
                    word(framework, 0),
                );
            }
            city_allocator().write(local as *mut u8);
            result = call(0x0041_1870, (&mut local as *mut u32).cast(), &[], result as u32, local);
        }
        (city_allocator().read(), result)
    }
}

unsafe fn reset_cities(this: *mut u8, argument: *mut u32, mut result: u64) -> u64 {
    unsafe {
        result = call(0x0042_1660, this.add(0x20), &[], result as u32, (result >> 32) as u32);
        publish_cities(this, argument, result, false)
    }
}

/// Publishes the local list to adjacent cities in reverse order, then to the current city.
/// The two callers load virtual tables into different scratch registers before the update calls.
pub(super) unsafe fn publish_cities(this: *mut u8, argument: *mut u32, mut result: u64, table_in_eax: bool) -> u64 {
    unsafe {
        argument.write_unaligned(0);
        let (service, carried) = allocator(result);
        result = virtual_call(service, 0x0C, &[12], word(service, 0), (carried >> 32) as u32);
        let header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        argument.write_unaligned(header as u32);
        result = virtual_call(this, 0x50, &[argument as u32, 0], argument as u32, word(this, 0));
        let manager = region_manager().read();
        result = virtual_call(manager, 0x90, &[], result as u32, word(manager, 0));
        let current = result as u32 as *mut u8;
        loop {
            let header = argument.read_unaligned() as *mut u8;
            if pointer(header, 0) == header {
                break;
            }
            let city = pointer(pointer(header, 4), 8);
            if !city.is_null() {
                result = virtual_call(
                    city,
                    0x114,
                    &[this.add(0x20) as u32],
                    this.add(0x20) as u32,
                    word(city, 0),
                );
                result = if table_in_eax {
                    virtual_call(city, 0x11C, &[], word(city, 0), (result >> 32) as u32)
                } else {
                    virtual_call(city, 0x11C, &[], result as u32, word(city, 0))
                };
            }
            let header = argument.read_unaligned() as *mut u8;
            let node = pointer(header, 4);
            let previous = pointer(node, 4);
            let next = pointer(node, 0);
            previous.cast::<*mut u8>().write_unaligned(next);
            next.add(4).cast::<*mut u8>().write_unaligned(previous);
            let reference = pointer(node, 8);
            if !reference.is_null() {
                result = if table_in_eax {
                    virtual_call(reference, 8, &[], next as u32, word(reference, 0))
                } else {
                    virtual_call(reference, 8, &[], word(reference, 0), (result >> 32) as u32)
                };
            }
            let (service, carried) = allocator(result);
            result = virtual_call(service, 0x10, &[node as u32], word(service, 0), (carried >> 32) as u32);
        }
        result = virtual_call(
            current,
            0x114,
            &[this.add(0x20) as u32],
            this.add(0x20) as u32,
            word(current, 0),
        );
        result = if table_in_eax {
            virtual_call(current, 0x11C, &[], word(current, 0), (result >> 32) as u32)
        } else {
            virtual_call(current, 0x11C, &[], result as u32, word(current, 0))
        };
        let sim = NeighborsSim::from_ptr(this);
        sim.set_deals_dirty(0);
        sim.set_stats_dirty(0);
        call(0x0041_1FE0, argument.cast(), &[], result as u32, (result >> 32) as u32)
    }
}

port! {
    /// Handles simulation dates, dirty reports, terrain changes and occupant removals through the
    /// message-target interface at primary +4. AL is always one; the final operation supplies the
    /// remaining integer return bits. Reset and pole-query messages reuse the caller's message word.
    #[port(address = 0x006D_42B0, owner = "neighbors", name = "cSC4NeighborsSimulator::DoMessage", naked)]
    pub unsafe extern "C" fn fc4p_006d42b0() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 4", sym dispatch);
    }
}

unsafe extern "fastcall" fn dispatch(interface: *mut u8, argument: *mut u32, entry_edx: u32) -> u64 {
    // SAFETY: the entry passes the original message argument and the live simulator interface.
    unsafe {
        let this = interface.sub(4);
        let message = argument.read_unaligned() as *mut u8;
        let mut result = virtual_call(message, 0x10, &[], word(message, 0), entry_edx);
        let kind = result as u32;
        match kind {
            0x26D3_1EC4 | 0x26D3_1EC5 => result = reset_cities(this, argument, result),
            0x278E_6003 => {
                result = virtual_call(message, 0x24, &[], result as u32, word(message, 0));
                let packed_x = result as u32;
                result = virtual_call(message, 0x38, &[], word(message, 0), (result >> 32) as u32);
                let packed_z = result as u32;
                let mut rect = [
                    (packed_x >> 16) as i32 - 1,
                    (packed_z >> 16) as i32 - 1,
                    (packed_x & 0xFFFF) as i32,
                    (packed_z & 0xFFFF) as i32,
                ];
                let city = NeighborsSim::from_ptr(this).city();
                result = virtual_call(city, 0x174, &[], word(city, 0), packed_z & 0xFFFF);
                let xmax = (result as u32 as i32).wrapping_sub(1);
                let city = NeighborsSim::from_ptr(this).city();
                result = virtual_call(city, 0x178, &[], result as u32, word(city, 0));
                let zmax = (result as u32 as i32).wrapping_sub(1);
                rect[0] = rect[0].max(0);
                rect[2] = rect[2].min(xmax);
                rect[1] = rect[1].max(0);
                rect[3] = rect[3].min(zmax);
                result = call(
                    0x006D_0BE0,
                    this,
                    &[rect.as_mut_ptr() as u32],
                    rect.as_mut_ptr() as u32,
                    (result >> 32) as u32,
                );
            }
            0x6695_6814 | 0x6695_681A => {
                let sim = NeighborsSim::from_ptr(this);
                result = call(
                    0x006D_0A40,
                    this,
                    &[u32::from(sim.deals_dirty()), u32::from(sim.stats_dirty())],
                    u32::from(sim.stats_dirty()),
                    (result >> 32) as u32,
                );
            }
            0x6695_6816 => result = call(0x006D_0E50, this, &[], 0, (result >> 32) as u32),
            0x6AC2_84F3 => *this.add(0x0C) = 1,
            0x99EF_1143 => {
                result = virtual_call(message, 0x28, &[], result as u32, word(message, 0));
                let occupant = result as u32 as *mut u8;
                result = virtual_call(occupant, 0x1C, &[], word(occupant, 0), (result >> 32) as u32);
                if result as u32 == 0x2890_D4DE {
                    let table = word(occupant, 0);
                    argument.write_unaligned(0);
                    result = call(0x004F_20A0, argument.cast(), &[], result as u32, (result >> 32) as u32);
                    let query_args = [0xA8C8_6142, result as u32];
                    let mut query = RawCall::new(
                        Conv::Thiscall,
                        word(table as *mut u8, 0) as usize,
                        occupant as u32,
                        &query_args,
                    );
                    query.eax = result as u32;
                    query.edx = (result >> 32) as u32;
                    call_raw(&mut query);
                    let pole = argument.read_unaligned() as *mut u8;
                    result = virtual_call(pole, 0x34, &[], query.eax, word(pole, 0));
                    if result as u8 == 0 {
                        result = virtual_call(this, 0x20, &[occupant as u32], word(this, 0), (result >> 32) as u32);
                    }
                    let pole = argument.read_unaligned() as *mut u8;
                    if !pole.is_null() {
                        result = virtual_call(pole, 8, &[], result as u32, word(pole, 0));
                    }
                }
            }
            0xAA7A_014D => {
                result = virtual_call(message, 0x28, &[], word(message, 0), (result >> 32) as u32);
                result = virtual_call(this, 0x20, &[result as u32], result as u32, word(this, 0));
            }
            0xEA41_8D8F => {
                result = virtual_call(message, 0x24, &[], word(message, 0), (result >> 32) as u32);
                *this.add(if result as u32 == 0 || result as u32 == 8 {
                    0x3D
                } else {
                    0x3C
                }) = 1;
                result = call(0x006D_0E50, this, &[], result as u32, (result >> 32) as u32);
            }
            _ => {
                if kind > 0x6695_6810 && kind < 0x6695_681A {
                    result = pair(kind.wrapping_sub(0x6695_6816), (result >> 32) as u32);
                }
            }
        }
        (result & !0xFF) | 1
    }
}

pub const PORTS: &[Port] = &[fc4p_006d42b0::PORT];

#[cfg(test)]
mod tests;
