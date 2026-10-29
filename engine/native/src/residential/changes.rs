// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population changes, building grades, ordinances and secondary interfaces.

use crate::port::Port;

game_fn! {
    #[allow(dead_code)]
    fn query_interface(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 = "thiscall" 0x006F_6E60;
}
game_fn! {
    #[allow(dead_code)]
    fn release(this: *mut u8) -> u32 = "thiscall" 0x006F_7DD0;
}

port! {
    /// Adjusts the serializable interface and forwards QueryInterface with the original registers and arguments.
    #[port(address = 0x006F_EAA0, owner = "residential", name = "SerializableQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_006feaa0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x006f6e60");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym tests::query_target);
    }
}

port! {
    /// Adjusts the message-target interface and forwards Release with the original registers.
    #[port(address = 0x006F_EAB0, owner = "residential", name = "MessageTargetRelease", naked)]
    pub unsafe extern "C" fn fc4p_006feab0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x006f7dd0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym tests::release_target);
    }
}

port! {
    /// Adjusts the serializable interface and forwards Release with the original registers.
    #[port(address = 0x006F_EAC0, owner = "residential", name = "SerializableRelease", naked)]
    pub unsafe extern "C" fn fc4p_006feac0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x006f7dd0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym tests::release_target);
    }
}

pub const PORTS: &[Port] = &[fc4p_006feaa0::PORT, fc4p_006feab0::PORT, fc4p_006feac0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};

    // Callees expose the adjusted pointer and incoming integer registers. The query target also
    // writes its arguments through the supplied output, then removes the two argument words.
    #[unsafe(naked)]
    pub unsafe extern "C" fn query_target() {
        core::arch::naked_asm!(
            "push edx",
            "mov edx, [esp + 12]",
            "mov [edx], eax",
            "mov eax, [esp + 8]",
            "mov [edx + 4], eax",
            "mov [edx + 8], ecx",
            "mov eax, ecx",
            "pop edx",
            "ret 8",
        );
    }

    #[unsafe(naked)]
    pub unsafe extern "C" fn release_target() {
        core::arch::naked_asm!("mov [ecx], eax", "mov eax, ecx", "ret");
    }

    #[test]
    fn serializable_query_forwards_registers_arguments_and_stack_cleanup() {
        let mut out = [0u32; 3];
        for interface in [0u32, 8, 0xFFFF_FFFF, 0x8000_0000] {
            let args = [0xE4FD_A3D4, out.as_mut_ptr() as u32];
            let mut c = RawCall::new(Conv::Thiscall, fc4p_006feaa0 as *const () as usize, interface, &args);
            c.eax = 0x1234_5678;
            c.edx = 0x90AB_CDEF;
            unsafe { call_raw(&mut c) };
            assert_eq!(out, [0x1234_5678, args[0], interface.wrapping_sub(8)]);
            assert_eq!((c.eax, c.edx), (interface.wrapping_sub(8), 0x90AB_CDEF));
        }
    }

    #[test]
    fn release_interfaces_forward_primary_pointer_and_registers() {
        let mut object = [0u32; 4];
        for (entry, offset) in [
            (fc4p_006feab0 as *const () as usize, 4),
            (fc4p_006feac0 as *const () as usize, 8),
        ] {
            let primary = object.as_mut_ptr() as u32;
            let mut c = RawCall::new(Conv::Thiscall, entry, primary + offset, &[]);
            c.eax = 0x1234_5678;
            c.edx = 0x90AB_CDEF;
            unsafe { call_raw(&mut c) };
            assert_eq!((c.eax, c.edx), (primary, 0x90AB_CDEF));
            assert_eq!(object, [0x1234_5678, 0, 0, 0]);
        }
    }
}
