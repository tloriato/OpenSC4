// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant connection removal at city edges.

use super::{pair, virtual_call, word, NeighborsSim};
use crate::port::Port;

port! {
    /// Selects the last carried network in priority order and removes its edge connections.
    /// The incoming occupant word becomes the queried network interface, then the cell's x coordinate.
    #[port(address = 0x006C_EE40, owner = "neighbors", name = "cSC4NeighborsSimulator::DeleteOccupantConnection", naked)]
    pub unsafe extern "C" fn fc4p_006cee40() {
        core::arch::naked_asm!(
            "push edx", "lea edx, [esp + 8]", "call {}", "ret 4", sym delete_occupant,
        );
    }
}

unsafe extern "fastcall" fn delete_occupant(this: *mut u8, argument: *mut u32, entry_edx: u32) -> u64 {
    // SAFETY: the entry supplies the original argument word and the game's live interfaces.
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        let occupant = argument.read_unaligned() as *mut u8;
        let mut result = virtual_call(occupant, 0x1C, &[], word(occupant, 0), entry_edx);
        let kind = match result as u32 {
            0xC772_BF98 => {
                argument.write_unaligned(0);
                result = virtual_call(occupant, 0, &[0xA821_EF94, argument as u32], argument as u32, word(occupant, 0));
                let mut kind = 0;
                if result as u8 != 0 {
                    for (index, (network, connection)) in [
                        (4, 5), (11, 10), (0, 2), (6, 8), (12, 12),
                        (2, 3), (7, 6), (8, 7), (9, 11), (1, 1),
                    ].into_iter().enumerate() {
                        let interface = argument.read_unaligned() as *mut u8;
                        let table = word(interface, 0);
                        let (eax, edx) = if index % 2 == 0 { (result as u32, table) } else { (table, (result >> 32) as u32) };
                        result = virtual_call(interface, 0x58, &[network], eax, edx);
                        if result as u8 != 0 { kind = connection; }
                    }
                }
                let interface = argument.read_unaligned() as *mut u8;
                if !interface.is_null() {
                    result = virtual_call(interface, 8, &[], result as u32, word(interface, 0));
                }
                if kind == 0 { return result; }
                kind
            }
            0x2890_D4DE => 4,
            _ => return result,
        };
        let mut position = [0u32; 3];
        virtual_call(occupant, 0x20, &[position.as_mut_ptr() as u32], word(occupant, 0), (result >> 32) as u32);
        let mut z = 0u32;
        let city = sim.city();
        virtual_call(city, 0x17C,
            &[position[0], position[2], argument as u32, &mut z as *mut u32 as u32],
            position[0], word(city, 0));
        let x = argument.read_unaligned();
        if x == 0 {
            argument.write_unaligned(u32::MAX);
            virtual_call(this, 0x1C, &[kind, u32::MAX, z], z, word(this, 0));
            argument.write_unaligned(argument.read_unaligned().wrapping_add(1));
        } else {
            let city = sim.city();
            result = virtual_call(city, 0x174, &[], x, word(city, 0));
            let x = argument.read_unaligned();
            if x == (result as u32).wrapping_sub(1) {
                let outside = x.wrapping_add(1);
                argument.write_unaligned(outside);
                virtual_call(this, 0x1C, &[kind, outside, z], outside, word(this, 0));
                argument.write_unaligned(argument.read_unaligned().wrapping_sub(1));
            }
        }
        if z == 0 {
            z = u32::MAX;
        } else {
            let city = sim.city();
            result = virtual_call(city, 0x178, &[], z, word(city, 0));
            if z != (result as u32).wrapping_sub(1) { return pair(z, (result >> 32) as u32); }
            z = z.wrapping_add(1);
        }
        let x = argument.read_unaligned();
        virtual_call(this, 0x1C, &[kind, x, z], x, word(this, 0))
    }
}

pub const PORTS: &[Port] = &[fc4p_006cee40::PORT];

#[cfg(test)]
mod tests;
