// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Breadth-first transport connectivity across the regional cities.

use super::{pair, pointer, virtual_call, word};
use crate::port::Port;

game_fn! { fn clear_cities(list: *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1DF0; }
game_fn! { fn city_node(list: *mut u8, edx: u32, count: u32, hint: u32) -> u64 = "fastcall" 0x0041_1B60; }
game_fn! { fn clear_connections(list: *mut u32, edx: u32) -> u64 = "fastcall" 0x0041_1D70; }
game_fn! { fn erase_city(list: *mut u8, edx: u32, out: *mut u32, node: *mut u8) -> u64 = "fastcall" 0x004A_4A50; }
game_data! { static region_manager: *mut u8 = 0x00B4_3C94; }
game_data! { static connection_allocator: *mut u8 = 0x00B2_07DC; }
game_data! { static city_allocator: *mut u8 = 0x00B2_07E0; }

unsafe fn link(header: *mut u8, node: *mut u8) -> *mut u8 {
    unsafe {
        let previous = pointer(header, 4);
        node.cast::<*mut u8>().write_unaligned(header);
        node.add(4).cast::<*mut u8>().write_unaligned(previous);
        previous.cast::<*mut u8>().write_unaligned(node);
        header.add(4).cast::<*mut u8>().write_unaligned(node);
        previous
    }
}

port! {
    /// Walks transport connections breadth first, deduplicating by city address. The current city is
    /// removed after traversal; unestablished cities are removed last, so their links still participate.
    /// The first stack word doubles as the temporary connection-list header and final erase iterator.
    #[port(address = 0x006D_1AB0, owner = "neighbors", name = "cSC4NeighborsSimulator::GetAllConnectedCities", naked)]
    pub unsafe extern "C" fn fc4p_006d1ab0() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 8", sym connected_cities);
    }
}

unsafe extern "fastcall" fn connected_cities(this: *mut u8, args: *mut u32, entry_edx: u32) -> u64 {
    // SAFETY: the entry supplies live game objects, list nodes, service caches, and argument words.
    unsafe {
        let list = args.read_unaligned() as *mut u8;
        let result = clear_cities(list, entry_edx);
        let manager = region_manager().read();
        let mut result = virtual_call(manager, 0x90, &[], word(manager, 0), (result >> 32) as u32);
        let current = result as u32 as *mut u8;
        if !current.is_null() {
            result = virtual_call(current, 4, &[], current as u32, word(current, 0));
        }
        let header = pointer(list, 0);
        result = city_node(list, (result >> 32) as u32, 1, 0);
        let node = result as u32 as *mut u8;
        let destination = node.wrapping_add(8);
        if !destination.is_null() {
            destination.cast::<*mut u8>().write_unaligned(current);
            if !current.is_null() {
                result = virtual_call(current, 4, &[], word(current, 0), (result >> 32) as u32);
            }
        }
        let previous = link(header, node);
        if !current.is_null() {
            result = virtual_call(current, 8, &[], previous as u32, word(current, 0));
        }
        let header = pointer(list, 0);
        let mut node = pointer(header, 0);
        let mut edx = (result >> 32) as u32;
        while node != pointer(list, 0) {
            let city = pointer(node, 8);
            args.write_unaligned(0);
            let mut scratch = core::ptr::null_mut();
            let allocator = super::cities::service(connection_allocator(), &mut scratch, &mut edx);
            let result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
            let header = result as u32 as *mut u8;
            header.cast::<*mut u8>().write_unaligned(header);
            header.add(4).cast::<*mut u8>().write_unaligned(header);
            args.write_unaligned(header as u32);
            let source = if city == current { this } else { city };
            let result = virtual_call(
                source,
                if city == current { 0x40 } else { 0x54 },
                &[args as u32],
                args as u32,
                word(source, 0),
            );
            edx = (result >> 32) as u32;
            let mut connection_node = pointer(args.read_unaligned() as *mut u8, 0);
            let mut kind_eax = args.read_unaligned();
            while connection_node != args.read_unaligned() as *mut u8 {
                let connection = pointer(connection_node, 8);
                let mut result = virtual_call(connection, 0x14, &[], kind_eax, word(connection, 0));
                if matches!(result as u32, 1..=3 | 6..=12) {
                    result = virtual_call(connection, 0x24, &[], 0, word(connection, 0));
                    let next_city = result as u32 as *mut u8;
                    if !next_city.is_null() {
                        virtual_call(next_city, 4, &[], word(next_city, 0), (result >> 32) as u32);
                        let output_header = pointer(list, 0);
                        let mut existing = pointer(output_header, 0);
                        while existing != output_header && pointer(existing, 8) != next_city {
                            existing = pointer(existing, 0);
                        }
                        result = virtual_call(next_city, 8, &[], output_header as u32, word(next_city, 0));
                        if existing == pointer(list, 0) {
                            result = virtual_call(next_city, 4, &[], word(next_city, 0), (result >> 32) as u32);
                            let output_header = pointer(list, 0);
                            edx = (result >> 32) as u32;
                            let allocator = super::cities::service(city_allocator(), &mut scratch, &mut edx);
                            result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
                            let added = result as u32 as *mut u8;
                            let destination = added.wrapping_add(8);
                            if !destination.is_null() {
                                destination.cast::<*mut u8>().write_unaligned(next_city);
                                result = virtual_call(next_city, 4, &[], destination as u32, word(next_city, 0));
                            }
                            link(output_header, added);
                            result = virtual_call(next_city, 8, &[], word(next_city, 0), (result >> 32) as u32);
                        }
                    }
                }
                edx = (result >> 32) as u32;
                connection_node = pointer(connection_node, 0);
                kind_eax = connection_node as u32;
            }
            // The next output node is captured before clearing the temporary connection references.
            node = pointer(node, 0);
            let result = clear_connections(args, edx);
            edx = (result >> 32) as u32;
            let header = args.read_unaligned();
            let allocator = super::cities::service(connection_allocator(), &mut scratch, &mut edx);
            let result = virtual_call(allocator, 0x10, &[header], word(allocator, 0), edx);
            edx = (result >> 32) as u32;
        }
        let header = pointer(list, 0);
        let first = pointer(header, 0);
        let mut result = erase_city(list, first as u32, args, first);
        let include = args.add(1).read_unaligned() as u8;
        result = (result & !0xFF) | u64::from(include);
        if include != 0 {
            return result;
        }
        let header = pointer(list, 0);
        result = pair(header as u32, (result >> 32) as u32);
        let mut node = pointer(header, 0);
        while node != pointer(list, 0) {
            let city = pointer(node, 8);
            result = virtual_call(city, 0xAC, &[], result as u32, word(city, 0));
            if result as u8 == 0 {
                let next = pointer(node, 0);
                let previous = pointer(node, 4);
                previous.cast::<*mut u8>().write_unaligned(next);
                next.add(4).cast::<*mut u8>().write_unaligned(previous);
                let city = pointer(node, 8);
                let mut edx = (result >> 32) as u32;
                if !city.is_null() {
                    let result = virtual_call(city, 8, &[], word(city, 0), edx);
                    edx = (result >> 32) as u32;
                }
                let allocator = super::cities::service(city_allocator(), args.add(1).cast(), &mut edx);
                result = virtual_call(allocator, 0x10, &[node as u32], word(allocator, 0), edx);
                node = next;
            } else {
                node = pointer(node, 0);
            }
        }
        result
    }
}

pub const PORTS: &[Port] = &[fc4p_006d1ab0::PORT];

#[cfg(test)]
mod tests;
