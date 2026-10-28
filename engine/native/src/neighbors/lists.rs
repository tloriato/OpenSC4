// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Filtered reference lists, including the game's allocator service cache.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::port::Port;

game_fn! {
    fn clear_connections(list: *mut u8, entry_edx: u32) -> u64 = "fastcall" 0x0041_1D70;
}
game_fn! {
    fn clear_deals(list: *mut u8, entry_edx: u32) -> u64 = "fastcall" 0x0042_1660;
}
game_fn! {
    fn framework() -> *mut u8 = "C" 0x0087_93EC;
}
game_fn! {
    fn release_local_reference(local: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870;
}
game_data! {
    static connection_allocator: *mut u8 = 0x00B2_07DC;
}
game_data! {
    static deal_allocator: *mut u8 = 0x00B2_0D58;
}

unsafe fn append(list: *mut u8, object: *mut u8, allocator_cache: *mut *mut u8) -> u64 {
    unsafe {
        let header = pointer(list, 0);
        let mut allocator = allocator_cache.read();
        if allocator.is_null() {
            let mut local = core::ptr::null_mut();
            let framework = framework();
            if !framework.is_null() {
                vcall!(framework, 0x14 => fn(u32, u32, *mut *mut u8) -> u32,
                    0x3AE4_BEA3, 0x3AE4_BEAB, &mut local);
            }
            allocator_cache.write(local);
            release_local_reference(&mut local);
            allocator = allocator_cache.read();
        }
        let node = vcall!(allocator, 0x0C => fn(u32) -> *mut u8, 12);
        let destination = node.wrapping_add(8);
        let mut edx = 0;
        if !destination.is_null() {
            destination.cast::<*mut u8>().write_unaligned(object);
            let result = virtual_call(object, 4, &[], destination as u32, word(object, 0));
            edx = (result >> 32) as u32;
        }
        let previous = pointer(header, 4);
        node.cast::<*mut u8>().write_unaligned(header);
        node.add(4).cast::<*mut u8>().write_unaligned(previous);
        previous.cast::<*mut u8>().write_unaligned(node);
        header.add(4).cast::<*mut u8>().write_unaligned(node);
        virtual_call(object, 8, &[], word(object, 0), edx)
    }
}

unsafe fn filtered(this: *mut u8, entry_edx: u32, kind: u32, city: Option<u32>, list: *mut u8, deals: bool) -> u64 {
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        let cleared = if deals {
            clear_deals(list, entry_edx)
        } else {
            clear_connections(list, entry_edx)
        };
        let header = if deals { sim.deals() } else { sim.connections() };
        let mut node = pointer(header, 0);
        let mut result = pair(header as u32, (cleared >> 32) as u32);
        while node != if deals { sim.deals() } else { sim.connections() } {
            let object = pointer(node, 8);
            result = virtual_call(
                object,
                if deals { 0x20 } else { 0x14 },
                &[],
                word(object, 0),
                (result >> 32) as u32,
            );
            let mut matches = result as u32 == kind;
            if let Some(city) = city {
                if matches {
                    result = virtual_call(
                        object,
                        if deals { 0x1C } else { 0x24 },
                        &[],
                        result as u32,
                        word(object, 0),
                    );
                    matches = result as u32 == city;
                }
            }
            if matches {
                let (eax, edx) = if city.is_some() {
                    (word(object, 0), (result >> 32) as u32)
                } else {
                    (result as u32, word(object, 0))
                };
                virtual_call(object, 4, &[], eax, edx);
                result = append(
                    list,
                    object,
                    if deals {
                        deal_allocator()
                    } else {
                        connection_allocator()
                    },
                );
            }
            node = pointer(node, 0);
        }
        result
    }
}

port! {
    /// Clears the output and appends every matching connection, retaining the resulting references.
    #[port(address = 0x006C_FF30, owner = "neighbors", name = "cSC4NeighborsSimulator::GetConnectionsOfType")]
    pub unsafe extern "fastcall" fn fc4p_006cff30(this: *mut u8, entry_edx: u32, kind: u32, list: *mut u8) -> u64 {
        unsafe { filtered(this, entry_edx, kind, None, list, false) }
    }
}

port! {
    /// Tests connection type before city and appends each match with the original temporary reference.
    #[port(address = 0x006D_0010, owner = "neighbors", name = "cSC4NeighborsSimulator::GetConnectionsOfTypeToCity")]
    pub unsafe extern "fastcall" fn fc4p_006d0010(this: *mut u8, entry_edx: u32, kind: u32, city: *mut u8, list: *mut u8) -> u64 {
        unsafe { filtered(this, entry_edx, kind, Some(city as u32), list, false) }
    }
}

port! {
    /// Clears the output deal list and appends matches without filtering their flags.
    #[port(address = 0x006D_0760, owner = "neighbors", name = "cSC4NeighborsSimulator::FindDeals")]
    pub unsafe extern "fastcall" fn fc4p_006d0760(this: *mut u8, entry_edx: u32, kind: u32, list: *mut u8) -> u64 {
        unsafe { filtered(this, entry_edx, kind, None, list, true) }
    }
}

port! {
    /// Tests deal type before other-side city and retains all matches in the output list.
    #[port(address = 0x006D_0840, owner = "neighbors", name = "cSC4NeighborsSimulator::FindDeals")]
    pub unsafe extern "fastcall" fn fc4p_006d0840(this: *mut u8, entry_edx: u32, kind: u32, city: *mut u8, list: *mut u8) -> u64 {
        unsafe { filtered(this, entry_edx, kind, Some(city as u32), list, true) }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006cff30::PORT,
    fc4p_006d0010::PORT,
    fc4p_006d0760::PORT,
    fc4p_006d0840::PORT,
];
