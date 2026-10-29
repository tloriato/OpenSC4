// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Parcellization dispatch and secondary-interface forwarding.

use super::{Port, ZoneDeveloper};
use crate::abi::{call_raw, Conv, RawCall};

game_fn! { #[allow(dead_code)] fn query_interface(this: *mut u8, id: u32, out: *mut *mut u8) -> u64 = "thiscall" 0x0072_9EF0; }
game_fn! { #[allow(dead_code)] fn release(this: *mut u8) -> u64 = "thiscall" 0x0072_A270; }
game_fn! { #[allow(dead_code)] fn clear_highlight(this: *mut u8) -> u64 = "thiscall" 0x0072_C0E0; }
game_fn! { #[allow(dead_code)] fn on_zone(this: *mut u8, region: *mut u8, out: *mut u8) -> u64 = "thiscall" 0x0073_3770; }
game_fn! { #[allow(dead_code)] fn on_dezone(this: *mut u8, region: *mut u8, flag: u32) -> u64 = "thiscall" 0x0072_E6A0; }

unsafe fn invoke(address: u32, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    call
}

port! {
    /// Set the current zone, dispatch RCI zones through highlight clearing and OnZone, and other zones
    /// through OnDezone with the complete flag word. Clear the current zone after the final call,
    /// retaining that call's EDX:EAX. The classification uses AL alone.
    #[port(address = 0x0073_3FA0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::DoParcellization")]
    pub unsafe extern "thiscall" fn fc4p_00733fa0(this: *mut u8, region: *mut u8, zone: u32, flag: u32) -> u64 {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let manager = dev.zone_manager();
            dev.set_current_zone(zone);
            let table = core::ptr::read_unaligned(manager.cast::<u32>());
            let result = super::virtual_registers(manager, 0x34, zone, table, &[zone]);
            let result = if result as u8 != 0 {
                let cleared = invoke(0x0072_C0E0, this, result as u32, (result >> 32) as u32, &[]);
                invoke(0x0073_3770, this, region as u32, cleared.edx, &[region as u32, 0])
            } else {
                invoke(0x0072_E6A0, this, result as u32, region as u32, &[region as u32, flag])
            };
            dev.set_current_zone(0);
            (u64::from(result.edx) << 32) | u64::from(result.eax)
        }
    }
}

port! {
    /// Forward the lot-manager interface's QueryInterface to the primary interface.
    #[port(address = 0x0073_47F0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::LotManagerQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_007347f0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00729ef0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym super::basic::fc4p_00729ef0);
    }
}

port! {
    /// Forward the serializable interface's QueryInterface to the primary interface.
    #[port(address = 0x0073_4810, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::SerializableQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_00734810() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 12", "jmp dword ptr [2f]", "2: .long 0x00729ef0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 12", "jmp {}", sym super::basic::fc4p_00729ef0);
    }
}

port! {
    /// Forward the lot-manager interface's Release to the shared reference count.
    #[port(address = 0x0073_4820, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::LotManagerRelease", naked)]
    pub unsafe extern "C" fn fc4p_00734820() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0072a270");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym super::basic::fc4p_0072a270);
    }
}

port! {
    /// Forward the message-target interface's Release to the shared reference count.
    #[port(address = 0x0073_4830, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::MessageTargetRelease", naked)]
    pub unsafe extern "C" fn fc4p_00734830() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x0072a270");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym super::basic::fc4p_0072a270);
    }
}

port! {
    /// Forward the serializable interface's Release to the shared reference count.
    #[port(address = 0x0073_4840, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::SerializableRelease", naked)]
    pub unsafe extern "C" fn fc4p_00734840() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 12", "jmp dword ptr [2f]", "2: .long 0x0072a270");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 12", "jmp {}", sym super::basic::fc4p_0072a270);
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_00733fa0::PORT,
    fc4p_007347f0::PORT,
    fc4p_00734810::PORT,
    fc4p_00734820::PORT,
    fc4p_00734830::PORT,
    fc4p_00734840::PORT,
];

#[cfg(test)]
mod tests;
