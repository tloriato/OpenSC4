// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Interface identity, reference counting, and virtual forwarding.

use super::{pair, virtual_call, word, NeighborsSim};
use crate::port::Port;

game_fn! {
    #[allow(dead_code)]
    fn release(this: *mut u8) -> u32 = "thiscall" 0x006C_E770;
}

port! {
    /// Returns the simulator's class ID without changing the other registers.
    #[port(address = 0x006C_DEF0, owner = "neighbors", name = "cSC4NeighborsSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006cdef0() {
        core::arch::naked_asm!("mov eax, 0xc990c0a4", "ret");
    }
}

port! {
    /// Publishes the requested interface before AddRef. Unsupported IDs leave the output alone.
    /// AL contains success; upper EAX bytes retain AddRef's result or the ID comparison's subtraction.
    #[port(address = 0x006C_DF00, owner = "neighbors", name = "cSC4NeighborsSimulator::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_006cdf00(this: *mut u8, entry_edx: u32, iid: u32, out: *mut *mut u8) -> u64 {
        unsafe {
            let offset = match iid {
                1 | 0x0773_FEF0 => 0,
                0x4522_94AA => 4,
                0xE4FD_A3D4 => 8,
                _ => {
                    let eax = if iid > 0x4522_94AA { iid } else { iid.wrapping_sub(0x0773_FEF0) };
                    return pair(eax & !0xFF, entry_edx);
                }
            };
            out.write_unaligned(if this.is_null() { this } else { this.add(offset) });
            let table = word(this, 0);
            let (eax, edx) = if offset == 0 { (out as u32, table) } else { (table, out as u32) };
            let result = virtual_call(this, 4, &[], eax, edx);
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Tests the full pointer returned by the simulator's FindConnection slot.
    #[port(address = 0x006C_DF80, owner = "neighbors", name = "cSC4NeighborsSimulator::HasConnection")]
    pub unsafe extern "thiscall" fn fc4p_006cdf80(this: *mut u8, kind: u32, x: i32, z: i32) -> u64 {
        unsafe {
            let result = virtual_call(this, 0x2C, &[kind, x as u32, z as u32], word(this, 0), kind);
            pair(u32::from(result as u32 != 0), (result >> 32) as u32)
        }
    }
}

port! {
    /// Compares the serializable subobjects; a zero comparison result means equal.
    #[port(address = 0x006C_E6A0, owner = "neighbors", name = "serializable_objects_equal")]
    pub unsafe extern "C" fn fc4p_006ce6a0(left: *mut u8, right: *mut u8) -> u64 {
        unsafe {
            let right = if right.is_null() { right } else { right.add(8) };
            let result = virtual_call(left.add(8), 0x6C, &[right as u32], right as u32, word(left, 8));
            pair(u32::from(result as u32 == 0), (result >> 32) as u32)
        }
    }
}

port! {
    /// Decrements a signed count above one. Otherwise shuts down, reloads the table, and destroys.
    #[port(address = 0x006C_E770, owner = "neighbors", name = "cSC4NeighborsSimulator::Release")]
    pub unsafe extern "fastcall" fn fc4p_006ce770(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe {
            let sim = NeighborsSim::from_ptr(this);
            let count = sim.references();
            if count > 1 {
                sim.set_references(count - 1);
                return pair((count - 1) as u32, entry_edx);
            }
            let result = virtual_call(this, 0x10, &[], word(this, 0), entry_edx);
            let result = virtual_call(this, 0x98, &[1], result as u32, word(this, 0));
            pair(0, (result >> 32) as u32)
        }
    }
}

port! {
    /// Marks pending regional deal updates, preserving the caller's integer return registers.
    #[port(address = 0x006C_FD70, owner = "neighbors", name = "cSC4NeighborsSimulator::MarkDealsDirty", naked)]
    pub unsafe extern "C" fn fc4p_006cfd70() {
        core::arch::naked_asm!("mov byte ptr [ecx + 0x3c], 1", "ret");
    }
}

port! {
    /// Releases through the primary interface from the message-target interface.
    #[port(address = 0x006C_FDA0, owner = "neighbors", name = "release_message_interface", naked)]
    pub unsafe extern "C" fn fc4p_006cfda0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "push 0x006ce770", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym fc4p_006ce770);
    }
}

port! {
    /// Releases through the primary interface from the serializable interface.
    #[port(address = 0x006C_FDB0, owner = "neighbors", name = "release_serializable_interface", naked)]
    pub unsafe extern "C" fn fc4p_006cfdb0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "push 0x006ce770", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym fc4p_006ce770);
    }
}

port! {
    /// Looks up a property and forwards its value object to the output operation when present.
    #[port(address = 0x005F_D530, owner = "neighbors", name = "write_property_value")]
    pub unsafe extern "C" fn fc4p_005fd530(holder: *mut u8, id: u32, output: *mut u8) -> u64 {
        unsafe {
            let result = virtual_call(holder, 0x24, &[id], word(holder, 0), id);
            let property = result as u32 as *mut u8;
            if property.is_null() {
                return result & !0xFF;
            }
            let result = virtual_call(property, 0x14, &[], property as u32, word(property, 0));
            let value = result as u32 as *mut u8;
            virtual_call(value, 0xA0, &[output as u32], value as u32, word(value, 0))
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_005fd530::PORT,
    fc4p_006cdef0::PORT,
    fc4p_006cdf00::PORT,
    fc4p_006cdf80::PORT,
    fc4p_006ce6a0::PORT,
    fc4p_006ce770::PORT,
    fc4p_006cfd70::PORT,
    fc4p_006cfda0::PORT,
    fc4p_006cfdb0::PORT,
];
