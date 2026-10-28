// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Publishing deal and commodity changes to regional cities.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::port::Port;

game_fn! { fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { fn release_local(local: *mut *mut u8, entry_edx: u32) -> u64 = "fastcall" 0x0041_1870; }
game_fn! { fn destroy_cities(list: *mut *mut u8, entry_edx: u32) -> u64 = "fastcall" 0x0041_1FE0; }
game_data! { static city_allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static region_manager: *mut u8 = 0x00B4_3C94; }

unsafe fn allocator(edx: &mut u32) -> *mut u8 {
    // SAFETY: the cached game service retains its own reference.
    unsafe {
        let mut allocator = city_allocator().read();
        if allocator.is_null() {
            let mut local = core::ptr::null_mut();
            let result = framework();
            let framework = result as u32 as *mut u8;
            if !framework.is_null() {
                virtual_call(
                    framework,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, &mut local as *mut *mut u8 as u32],
                    framework as u32,
                    word(framework, 0),
                );
            }
            city_allocator().write(local);
            let result = release_local(&mut local, local as u32);
            *edx = (result >> 32) as u32;
            allocator = city_allocator().read();
        }
        allocator
    }
}

port! {
    /// Publishes to adjacent cities from the back of the list, then to the current city. Each removed
    /// list entry releases its city and returns its node to the cached allocator. Only requested dirty
    /// bytes are cleared, after the callbacks and before the temporary list is destroyed.
    #[port(address = 0x006D_0A40, owner = "neighbors", name = "cSC4NeighborsSimulator::update_deals_in_regional_cities", naked)]
    pub unsafe extern "C" fn fc4p_006d0a40() {
        core::arch::naked_asm!("push eax", "push edx", "lea edx, [esp + 12]", "call {}", "ret 8", sym publish);
    }
}

unsafe extern "fastcall" fn publish(this: *mut u8, args: *mut u32, entry_edx: u32, entry_eax: u32) -> u64 {
    // SAFETY: the entry supplies the simulator and original flag words.
    unsafe {
        let deals = args.read_unaligned() as u8 != 0;
        let stats = args.add(1).read_unaligned() as u8 != 0;
        if !deals && !stats {
            return pair(entry_eax & !0xFF, entry_edx);
        }
        let sim = NeighborsSim::from_ptr(this);
        let mut edx = entry_edx;
        let service = allocator(&mut edx);
        let result = virtual_call(service, 0x0C, &[12], word(service, 0), edx);
        let mut header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        let result = virtual_call(
            this,
            0x50,
            &[&mut header as *mut *mut u8 as u32, 0],
            &mut header as *mut *mut u8 as u32,
            word(this, 0),
        );
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], result as u32, word(manager, 0));
        let current = result as u32 as *mut u8;
        edx = (result >> 32) as u32;
        while pointer(header, 0) != header {
            let city = pointer(pointer(header, 4), 8);
            if !city.is_null() {
                if deals {
                    let result = virtual_call(
                        city,
                        0x114,
                        &[this.add(0x20) as u32],
                        this.add(0x20) as u32,
                        word(city, 0),
                    );
                    edx = (result >> 32) as u32;
                }
                if stats {
                    let result = virtual_call(city, 0x11C, &[], header as u32, word(city, 0));
                    edx = (result >> 32) as u32;
                }
            }
            // Callbacks can replace the list's last entry, so reload it before unlinking.
            let node = pointer(header, 4);
            let previous = pointer(node, 4);
            let next = pointer(node, 0);
            previous.cast::<*mut u8>().write_unaligned(next);
            next.add(4).cast::<*mut u8>().write_unaligned(previous);
            let object = pointer(node, 8);
            if !object.is_null() {
                let result = virtual_call(object, 8, &[], word(object, 0), edx);
                edx = (result >> 32) as u32;
            }
            let service = allocator(&mut edx);
            let result = virtual_call(service, 0x10, &[node as u32], word(service, 0), edx);
            edx = (result >> 32) as u32;
        }
        let mut eax = header as u32;
        if deals {
            let result = virtual_call(
                current,
                0x114,
                &[this.add(0x20) as u32],
                this.add(0x20) as u32,
                word(current, 0),
            );
            eax = result as u32;
            edx = (result >> 32) as u32;
        }
        if stats {
            let result = virtual_call(current, 0x11C, &[], eax, word(current, 0));
            edx = (result >> 32) as u32;
        }
        if deals {
            sim.set_deals_dirty(0);
        }
        if stats {
            sim.set_stats_dirty(0);
        }
        destroy_cities(&mut header, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006d0a40::PORT];

#[cfg(test)]
mod tests;
