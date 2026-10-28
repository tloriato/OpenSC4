// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Symmetric connection removal, edge markers, and deletion notifications.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::port::Port;

game_fn! { fn property_bool(holder: *mut u8, id: u32, out: *mut u8) -> u64 = "C" 0x005F_D390; }
game_fn! { fn allocate_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x0091_34D6; }
game_fn! { fn erase_connection(this: *mut u8, edx: u32, reference: *mut *mut u8) -> u64 = "fastcall" 0x006C_FD30; }
game_fn! { fn clear_occupants(list: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_3E60; }
game_data! { static occupant_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static prop_manager: *mut u8 = 0x00B4_3D10; }
game_data! { static network_manager: *mut u8 = 0x00B4_3D0C; }
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }

port! {
    /// Removes the regional city's matching connection, cleans markers in the inward cell, and posts
    /// the original type and coordinates. Optionally erases the simulator's reference afterwards.
    /// The entry adapter retains the final allocator call's ECX as well as EDX:EAX.
    #[port(address = 0x006D_17B0, owner = "neighbors", name = "cSC4NeighborsSimulator::DeleteConnection", naked)]
    pub unsafe extern "C" fn fc4p_006d17b0() {
        core::arch::naked_asm!(
            "push ecx", "mov eax, esp", "push eax", "push edx",
            "lea edx, [esp + 16]", "call {}", "pop ecx", "ret 8",
            sym delete_connection,
        );
    }
}

unsafe extern "fastcall" fn delete_connection(
    this: *mut u8,
    args: *mut u32,
    entry_edx: u32,
    exit_ecx: *mut u32,
) -> u64 {
    // SAFETY: all services, virtual methods, list links, and argument words belong to the game.
    unsafe {
        let connection = args.read_unaligned() as *mut u8;
        let result = virtual_call(connection, 0x24, &[], word(connection, 0), entry_edx);
        let city = result as u32 as *mut u8;
        if !city.is_null() {
            virtual_call(city, 0x58, &[connection as u32, 0], city as u32, word(city, 0));
        }
        let props = prop_manager().read();
        let mut cell = [0i32; 2];
        let result = virtual_call(
            connection,
            0x18,
            &[cell.as_mut_ptr() as u32, cell.as_mut_ptr().add(1) as u32],
            word(connection, 0),
            cell.as_mut_ptr() as u32,
        );
        let [mut x, mut z] = cell;
        let mut edx = (result >> 32) as u32;
        if x < 0 {
            x = x.wrapping_add(1);
        } else if z < 0 {
            z = z.wrapping_add(1);
        } else {
            let city = NeighborsSim::from_ptr(this).city();
            let result = virtual_call(city, 0x174, &[], this as u32, word(city, 0));
            edx = (result >> 32) as u32;
            if cell[0] >= result as u32 as i32 {
                x = x.wrapping_sub(1);
            } else {
                z = z.wrapping_sub(1);
            }
        }
        let mut scratch = core::ptr::null_mut();
        let allocator = super::cities::service(occupant_allocator(), &mut scratch, &mut edx);
        let result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
        let mut header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        let rectangle = [x, z, x, z];
        virtual_call(
            props,
            0x48,
            &[&mut header as *mut *mut u8 as u32, rectangle.as_ptr() as u32],
            rectangle.as_ptr() as u32,
            word(props, 0),
        );
        let mut node = pointer(header, 0);
        while node != header {
            let occupant = pointer(node, 8);
            let mut marker = 0u8;
            let result = virtual_call(occupant, 0x18, &[], word(occupant, 0), &mut marker as *mut u8 as u32);
            let result = property_bool(result as u32 as *mut u8, 0xCAA4_6595, &mut marker);
            if marker != 0 {
                virtual_call(
                    props,
                    0x74,
                    &[occupant as u32],
                    result as u32 & !0xFF | u32::from(marker),
                    word(props, 0),
                );
            }
            node = pointer(node, 0);
        }
        let networks = network_manager().read();
        virtual_call(
            networks,
            0xC0,
            &[&mut header as *mut *mut u8 as u32],
            word(networks, 0),
            &mut header as *mut *mut u8 as u32,
        );
        let result = allocate_message(0x2C);
        let storage = result as u32 as *mut u8;
        let result = if storage.is_null() {
            result
        } else {
            construct_message(storage, (result >> 32) as u32)
        };
        let message = result as u32 as *mut u8;
        let result = virtual_call(message, 4, &[], word(message, 0), (result >> 32) as u32);
        let result = virtual_call(message, 0x14, &[0xEA0D_163E], result as u32, word(message, 0));
        let message_table = word(message, 0);
        let result = virtual_call(connection, 0x14, &[], word(connection, 0), (result >> 32) as u32);
        // The message's table is fetched before querying the connection type.
        let arguments = [result as u32];
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            word(message_table as *mut u8, 0x2C) as usize,
            message as u32,
            &arguments,
        );
        call.eax = result as u32;
        call.edx = (result >> 32) as u32;
        crate::abi::call_raw(&mut call);
        virtual_call(message, 0x40, &[cell[0] as u32], cell[0] as u32, word(message, 0));
        let result = virtual_call(message, 0x54, &[cell[1] as u32], cell[1] as u32, word(message, 0));
        let server = message_server().read();
        let result = virtual_call(server, 0x10, &[message as u32, 0], result as u32, word(server, 0));
        let result = virtual_call(message, 8, &[], word(message, 0), (result >> 32) as u32);
        let mut result = pair(
            (result as u32 & !0xFF) | (args.add(1).read_unaligned() & 0xFF),
            (result >> 32) as u32,
        );
        if result as u8 != 0 {
            result = virtual_call(connection, 4, &[], result as u32, word(connection, 0));
            let mut reference = connection;
            erase_connection(this.add(0x1C), (result >> 32) as u32, &mut reference);
            result = virtual_call(
                connection,
                8,
                &[],
                &mut reference as *mut *mut u8 as u32,
                word(connection, 0),
            );
        }
        let result = clear_occupants(&mut header, (result >> 32) as u32);
        edx = (result >> 32) as u32;
        let saved_header = header;
        let allocator = super::cities::service(occupant_allocator(), &mut scratch, &mut edx);
        let eax;
        core::arch::asm!(
            "push {memory}", "call dword ptr [eax + 0x10]", "mov dword ptr [edi], ecx",
            memory = in(reg) saved_header,
            in("edi") exit_ecx,
            inlateout("ecx") allocator => _,
            inlateout("eax") word(allocator, 0) => eax,
            inlateout("edx") edx,
            clobber_abi("C"),
        );
        pair(eax, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006d17b0::PORT];

#[cfg(test)]
mod tests;
