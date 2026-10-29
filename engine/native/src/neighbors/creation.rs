// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Creating an edge connection, its marker and its notification.

use super::{pointer, virtual_call, word, NeighborsSim};
use crate::port::Port;
use crate::x87::F80;

layout! {
    /// Connection fields initialized after insertion into the simulator's list.
    struct Connection {
        0x24 => kind / set_kind: u32,
        0x28 => x / set_x: u32,
        0x2C => z / set_z: u32,
        0x30 => region_x / set_region_x: u32,
        0x34 => region_z / set_region_z: u32,
    }
}

game_fn! { fn allocate_connection(size: u32) -> u64 = "C" 0x005E_55E0; }
game_fn! { fn construct_connection(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x006D_4680; }
game_fn! { fn allocate_node(this: *mut u8, edx: u32, count: u32, hint: u32) -> u64 = "fastcall" 0x0041_1B00; }
game_fn! {
    #[allow(dead_code)]
    fn convert_cell(x: u32, z: u32, rx: *mut u32, rz: *mut u32, city: u32) -> u64 = "C" 0x006C_E820;
}
game_fn! { fn allocate_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x0091_34D6; }
game_data! { static props: *mut u8 = 0x00B4_3D10; }
game_data! { static terrain: *mut u8 = 0x00B4_3CF4; }
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }
game_data! { static elevated_height: f32 = 0x00AB_1CA8; }

port! {
    /// Inserts a new connection unless the virtual existence query succeeds. Builds a marker except
    /// for ferries, updates the other city, and posts the created message. The first two incoming
    /// argument words also serve as output storage; the entry passes their actual addresses to Rust.
    #[port(address = 0x006D_4860, owner = "neighbors", name = "cSC4NeighborsSimulator::CreateConnection", naked)]
    pub unsafe extern "C" fn fc4p_006d4860() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {}", "ret 16", sym create);
    }
}

unsafe extern "fastcall" fn create(this: *mut u8, args: *mut u32) -> u64 {
    // SAFETY: the entry supplies four original argument words and a live simulator. All allocations,
    // constructors, reference operations and notifications use the game's own functions.
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        let kind = args.read_unaligned();
        let mut result = virtual_call(
            this,
            0x28,
            &[kind, args.add(1).read_unaligned(), args.add(2).read_unaligned()],
            word(this, 0),
            args.add(1).read_unaligned(),
        );
        if result as u8 != 0 {
            return result;
        }
        result = allocate_connection(0x44);
        let mut connection = result as u32 as *mut u8;
        if !connection.is_null() {
            result = construct_connection(connection, (result >> 32) as u32);
            connection = result as u32 as *mut u8;
            if !connection.is_null() {
                virtual_call(connection, 4, &[], word(connection, 0), (result >> 32) as u32);
            }
        }
        let header = sim.connections();
        args.write_unaligned(header as u32);
        result = allocate_node(this.add(0x1C), header as u32, 1, 0);
        let node = result as u32 as *mut u8;
        let element = node.wrapping_add(8);
        if !element.is_null() {
            element.cast::<*mut u8>().write_unaligned(connection);
            if !connection.is_null() {
                virtual_call(connection, 4, &[], word(connection, 0), (result >> 32) as u32);
            }
        }
        let header = args.read_unaligned() as *mut u8;
        let previous = pointer(header, 4);
        node.cast::<*mut u8>().write_unaligned(header);
        node.add(4).cast::<*mut u8>().write_unaligned(previous);
        previous.cast::<*mut u8>().write_unaligned(node);
        header.add(4).cast::<*mut u8>().write_unaligned(node);
        if !connection.is_null() {
            virtual_call(connection, 8, &[], previous as u32, word(connection, 0));
        }
        let x = args.add(1).read_unaligned();
        let conversion_args = [x, args.add(2).read_unaligned(), args.add(1) as u32, args as u32, 0];
        let mut conversion = crate::abi::RawCall::new(
            crate::abi::Conv::Cdecl,
            crate::abi::target(0x006C_E820),
            args.add(1) as u32,
            &conversion_args,
        );
        conversion.eax = args as u32;
        conversion.edx = args.add(2).read_unaligned();
        crate::abi::call_raw(&mut conversion);
        let c = Connection::from_ptr(connection);
        c.set_z(args.add(2).read_unaligned());
        c.set_region_x(args.add(1).read_unaligned());
        c.set_kind(kind);
        c.set_x(x);
        c.set_region_z(args.read_unaligned());
        result = virtual_call(connection, 0x24, &[], word(connection, 0), args.read_unaligned());
        let other = result as u32 as *mut u8;
        if !other.is_null() {
            virtual_call(other, 0x58, &[connection as u32, 1], result as u32, word(other, 0));
        }
        if kind != 9 {
            args.write_unaligned(props().read() as u32);
            let mut position = [0u32; 3];
            let flip = args.add(3).read_unaligned() as u8 != 0;
            let z = args.add(2).read_unaligned() as i32;
            let (inner_x, inner_z, orientation) = if (x as i32) < 0 {
                ((x as i32).wrapping_add(1), z, if flip { 1 } else { 3 })
            } else if z < 0 {
                (x as i32, z.wrapping_add(1), if flip { 2 } else { 0 })
            } else {
                let city = sim.city();
                let width = virtual_call(city, 0x174, &[], z as u32, word(city, 0)) as u32 as i32;
                if x as i32 >= width {
                    ((x as i32).wrapping_sub(1), z, if flip { 3 } else { 1 })
                } else {
                    (x as i32, z.wrapping_sub(1), if flip { 0 } else { 2 })
                }
            };
            let city = sim.city();
            virtual_call(
                city,
                0x184,
                &[
                    inner_x as u32,
                    inner_z as u32,
                    position.as_mut_ptr() as u32,
                    position.as_mut_ptr().add(2) as u32,
                ],
                inner_z as u32,
                word(city, 0),
            );
            position[1] = vcall_st0!(
                terrain().read(),
                0x34,
                f32::from_bits(position[0]),
                f32::from_bits(position[2])
            )
            .to_f32()
            .to_bits();
            if kind == 3 || kind == 7 || kind == 11 {
                position[1] = (F80::from_f32(f32::from_bits(position[1])) + F80::from_f32(elevated_height().read()))
                    .to_f32()
                    .to_bits();
            }
            let manager = args.read_unaligned() as *mut u8;
            args.add(1).write_unaligned(0);
            result = virtual_call(
                manager,
                0x3C,
                &[0x29F1_0000, args.add(1) as u32],
                args.add(1) as u32,
                word(manager, 0),
            );
            if result as u8 != 0 {
                let marker = args.add(1).read_unaligned() as *mut u8;
                result = virtual_call(marker, 0x0C, &[], result as u32, word(marker, 0));
                let occupant = result as u32 as *mut u8;
                virtual_call(
                    occupant,
                    0x24,
                    &[position.as_mut_ptr() as u32],
                    result as u32,
                    word(occupant, 0),
                );
                let marker = args.add(1).read_unaligned() as *mut u8;
                result = virtual_call(marker, 0x28, &[orientation], result as u32, word(marker, 0));
                let marker = args.add(1).read_unaligned() as *mut u8;
                result = virtual_call(marker, 0x0C, &[], word(marker, 0), (result >> 32) as u32);
                let occupant = result as u32 as *mut u8;
                result = virtual_call(occupant, 0x18, &[], result as u32, word(occupant, 0));
                let properties = result as u32 as *mut u8;
                result = virtual_call(
                    properties,
                    0x34,
                    &[0xCAA4_6595, 1, 0],
                    word(properties, 0),
                    (result >> 32) as u32,
                );
                if kind == 5 || kind == 6 {
                    result = virtual_call(
                        properties,
                        0x34,
                        &[0xCAA4_5670, 1, 0],
                        kind.wrapping_sub(6),
                        word(properties, 0),
                    );
                    virtual_call(
                        properties,
                        0x34,
                        &[if kind == 5 { 0xCAA4_5672 } else { 0xCAA4_5671 }, 1, 0],
                        word(properties, 0),
                        (result >> 32) as u32,
                    );
                }
                let marker = args.add(1).read_unaligned() as *mut u8;
                let manager_table = word(manager, 0);
                result = virtual_call(marker, 0x0C, &[], result as u32, word(marker, 0));
                let call_args = [result as u32];
                let mut call = crate::abi::RawCall::new(
                    crate::abi::Conv::Thiscall,
                    pointer(manager_table as *mut u8, 0x4C) as usize,
                    manager as u32,
                    &call_args,
                );
                call.eax = result as u32;
                call.edx = (result >> 32) as u32;
                crate::abi::call_raw(&mut call);
                let marker = args.add(1).read_unaligned() as *mut u8;
                virtual_call(marker, 8, &[], word(marker, 0), call.edx);
            }
        }
        result = allocate_message(0x2C);
        let mut message = result as u32 as *mut u8;
        if !message.is_null() {
            result = construct_message(message, (result >> 32) as u32);
            message = result as u32 as *mut u8;
        }
        result = virtual_call(message, 4, &[], result as u32, word(message, 0));
        result = virtual_call(message, 0x14, &[0x8A0D_15DA], word(message, 0), (result >> 32) as u32);
        result = virtual_call(message, 0x2C, &[kind], result as u32, word(message, 0));
        virtual_call(message, 0x40, &[x], word(message, 0), (result >> 32) as u32);
        result = virtual_call(
            message,
            0x54,
            &[args.add(2).read_unaligned()],
            args.add(2).read_unaligned(),
            word(message, 0),
        );
        let server = message_server().read();
        result = virtual_call(server, 0x10, &[message as u32, 0], result as u32, word(server, 0));
        virtual_call(message, 8, &[], word(message, 0), (result >> 32) as u32)
    }
}

pub const PORTS: &[Port] = &[fc4p_006d4860::PORT];

#[cfg(test)]
mod tests;
