// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Connection traversal on the game's circular reference lists.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::port::Port;

game_fn! {
    fn clear_connections(list: *mut u8) -> u64 = "thiscall" 0x0041_1D70;
}
game_fn! {
    fn copy_connections(list: *mut u8, entry_edx: u32, position: u32, first: *mut u8, last: *mut u8, out: *mut u32) -> u64 = "fastcall" 0x004A_86F0;
}
game_fn! {
    fn erase_connection(list: *mut u8, out: *mut u32, iterator: u32) -> u64 = "thiscall" 0x0041_C000;
}
game_fn! {
    fn convert_cell(x: i32, z: i32, region_x: *mut i32, region_z: *mut i32, city: *mut *mut u8, cell_x: *mut i32, cell_z: *mut i32) -> u64 = "C" 0x006C_E920;
}

port! {
    /// Reads a connection's type and cell and dispatches the coordinate overload. The connection
    /// argument's stack slot is also the cell getter's second destination.
    #[port(address = 0x006C_DFA0, owner = "neighbors", name = "cSC4NeighborsSimulator::FindConnectionOnOtherSide", naked)]
    pub unsafe extern "C" fn fc4p_006cdfa0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {}", sym other_side_of_object);
    }
}

unsafe extern "fastcall" fn other_side_of_object(this: *mut u8, argument: *mut u32, connection: *mut u8) -> u64 {
    unsafe {
        let kind = virtual_call(connection, 0x14, &[], word(connection, 0), 0) as u32;
        let mut x = this as u32;
        virtual_call(
            connection,
            0x18,
            &[&mut x as *mut u32 as u32, argument as u32],
            argument as u32,
            word(connection, 0),
        );
        let z = argument.read_unaligned();
        virtual_call(this, 0x38, &[kind, x, z], z, word(this, 0))
    }
}

port! {
    /// Finds the first exact type/cell match. The first argument's stack word is used as the x
    /// destination on every visit. The entry shim also retains EDX for an empty list.
    /// Saved search values are read through the frame pointer before the cell getter can overwrite it.
    #[port(address = 0x006C_F6B0, owner = "neighbors", name = "cSC4NeighborsSimulator::FindConnection", naked)]
    pub unsafe extern "C" fn fc4p_006cf6b0() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 12", sym find_connection);
    }
}

unsafe extern "fastcall" fn find_connection(this: *mut u8, argument: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let kind = argument.read_unaligned();
        let x = argument.add(1).read_unaligned();
        let z = argument.add(2).read_unaligned();
        let sim = NeighborsSim::from_ptr(this);
        let header = sim.connections();
        let mut node = pointer(header, 0);
        let mut edx = entry_edx;
        let mut cell_z = this as u32;
        while node != sim.connections() {
            let connection = pointer(node, 8);
            let result = virtual_call(
                connection,
                0x18,
                &[argument as u32, &mut cell_z as *mut u32 as u32],
                word(connection, 0),
                argument as u32,
            );
            let result = virtual_call(connection, 0x14, &[], word(connection, 0), (result >> 32) as u32);
            edx = (result >> 32) as u32;
            if result as u32 == kind && argument.read_unaligned() == x {
                edx = z;
                if cell_z == z {
                    return pair(connection as u32, edx);
                }
            }
            node = pointer(node, 0);
        }
        pair(0, edx)
    }
}

port! {
    /// Finds the first connection of a type that is unvisited and names the requested city.
    /// Only the low visited byte controls whether the city getter is called.
    #[port(address = 0x006C_F720, owner = "neighbors", name = "cSC4NeighborsSimulator::FindUnvisitedConnection")]
    pub unsafe extern "fastcall" fn fc4p_006cf720(this: *mut u8, entry_edx: u32, kind: u32, city: *mut u8) -> u64 {
        unsafe {
            let sim = NeighborsSim::from_ptr(this);
            let header = sim.connections();
            let mut node = pointer(header, 0);
            let mut edx = entry_edx;
            while node != sim.connections() {
                let connection = pointer(node, 8);
                let result = virtual_call(connection, 0x14, &[], word(connection, 0), edx);
                edx = (result >> 32) as u32;
                if result as u32 == kind {
                    let result = virtual_call(connection, 0x2C, &[], result as u32, word(connection, 0));
                    edx = (result >> 32) as u32;
                    if result as u8 == 0 {
                        let result = virtual_call(connection, 0x24, &[], word(connection, 0), edx);
                        edx = (result >> 32) as u32;
                        if result as u32 == city as u32 {
                            return pair(connection as u32, edx);
                        }
                    }
                }
                node = pointer(node, 0);
            }
            pair(0, edx)
        }
    }
}

port! {
    /// Clears the caller's reference list and copies the full connection range. The range insertion
    /// writes its result iterator into the original output argument's stack word.
    #[port(address = 0x006C_FF00, owner = "neighbors", name = "cSC4NeighborsSimulator::GetAllConnections", naked)]
    pub unsafe extern "C" fn fc4p_006cff00() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {}", sym all_connections);
    }
}

unsafe extern "fastcall" fn all_connections(this: *mut u8, argument: *mut u32, list: *mut u8) -> u64 {
    unsafe {
        clear_connections(list);
        let header = NeighborsSim::from_ptr(this).connections();
        let first = pointer(header, 0);
        let destination = word(list, 0);
        copy_connections(list, destination, destination, first, header, argument)
    }
}

port! {
    /// Removes every equal pointer, retaining the next node before erasure and reloading the match
    /// value for every visit. Each erase stores its result in the incoming argument's stack word.
    #[port(address = 0x006C_FD30, owner = "neighbors", name = "remove_connection_references", naked)]
    pub unsafe extern "C" fn fc4p_006cfd30() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 4", sym remove_references);
    }
}

unsafe extern "fastcall" fn remove_references(list: *mut u8, argument: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let value = argument.read_unaligned() as *mut u32;
        let header = pointer(list, 0);
        let mut node = pointer(header, 0);
        let mut edx = entry_edx;
        while node != header {
            let matched = value.read_unaligned() == word(node, 8);
            let next = pointer(node, 0);
            if matched {
                edx = (erase_connection(list, argument, node as u32) >> 32) as u32;
            }
            node = next;
        }
        pair(header as u32, edx)
    }
}

port! {
    /// Converts into the neighboring city's cell before looking up that city's matching connection.
    /// Both coordinate argument slots are the conversion helper's output destinations.
    #[port(address = 0x006C_EDE0, owner = "neighbors", name = "cSC4NeighborsSimulator::FindConnectionOnOtherSide", naked)]
    pub unsafe extern "C" fn fc4p_006cede0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {}", sym other_side_of_cell);
    }
}

unsafe extern "fastcall" fn other_side_of_cell(_this: *mut u8, argument: *mut u32, kind: u32, x: i32, z: i32) -> u64 {
    unsafe {
        let mut city = core::ptr::null_mut();
        let mut rx = 0;
        let mut rz = 0;
        let result = convert_cell(
            x,
            z,
            &mut rx,
            &mut rz,
            &mut city,
            argument.add(1).cast(),
            argument.add(2).cast(),
        );
        if result as u8 == 0 {
            return pair(0, (result >> 32) as u32);
        }
        let x = argument.add(1).read_unaligned();
        let z = argument.add(2).read_unaligned();
        virtual_call(city, 0x50, &[kind, x, z], kind, word(city, 0))
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006cdfa0::PORT,
    fc4p_006cede0::PORT,
    fc4p_006cf6b0::PORT,
    fc4p_006cf720::PORT,
    fc4p_006cfd30::PORT,
    fc4p_006cff00::PORT,
];
