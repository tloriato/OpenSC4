// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Flag access, interface adjustment, and developer category adapters.

use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;

game_fn! { #[allow(dead_code)] fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 = "thiscall" 0x0069_5290; }
game_fn! { #[allow(dead_code)] fn release(this: *mut u8) -> u64 = "thiscall" 0x0069_59B0; }
game_fn! { #[allow(dead_code)] fn developer_type(purpose: u32, wealth: u32) -> u64 = "C" 0x0069_5790; }
game_fn! { #[allow(dead_code)] fn developer_wealth(kind: u32) -> u64 = "C" 0x0069_5840; }

#[cfg(test)]
unsafe extern "thiscall" fn query_primary(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
    // SAFETY: the entry has adjusted the interface and forwards both arguments.
    unsafe { query(this, iid, out) }
}

#[cfg(test)]
unsafe extern "thiscall" fn release_primary(this: *mut u8) -> u64 {
    // SAFETY: the entry has adjusted the interface.
    unsafe { release(this) }
}

port! {
    /// Returns the power flag in AL, preserving the other EAX bytes and EDX.
    #[port(address = 0x0069_8D00, owner = "building", name = "GetIgnorePower", naked)]
    pub unsafe extern "C" fn fc4p_00698d00() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0xde]", "ret");
    }
}
port! {
    /// Returns the water flag in AL, preserving the other EAX bytes and EDX.
    #[port(address = 0x0069_8D10, owner = "building", name = "GetIgnoreWater", naked)]
    pub unsafe extern "C" fn fc4p_00698d10() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0xdf]", "ret");
    }
}
port! {
    /// Returns the bar flag in AL, preserving the other EAX bytes and EDX.
    #[port(address = 0x0069_8D20, owner = "building", name = "GetBarStuck", naked)]
    pub unsafe extern "C" fn fc4p_00698d20() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0xe1]", "ret");
    }
}
port! {
    /// Queries the primary interface from the serializable interface.
    #[port(address = 0x0069_8D50, owner = "building", name = "SerializableQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_00698d50() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 12", "push 0x00695290", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 12", "jmp {target}", target = sym query_primary);
    }
}

port! {
    /// Forwards purpose and wealth to the static conversion, preserving its EDX result.
    #[port(address = 0x0069_8D60, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetDeveloperType")]
    pub unsafe extern "fastcall" fn fc4p_00698d60(_this: *mut u8, entry_edx: u32, purpose: u32, wealth: u32) -> u64 {
        let args = [purpose, wealth];
        let mut c = RawCall::new(Conv::Cdecl, crate::abi::target(0x0069_5790), purpose, &args);
        c.eax = wealth;
        c.edx = entry_edx;
        // SAFETY: the game's cdecl conversion takes two words.
        unsafe { call_raw(&mut c) };
        (u64::from(c.edx) << 32) | u64::from(c.eax)
    }
}
port! {
    /// Forwards the developer type to the static wealth conversion.
    #[port(address = 0x0069_8D80, owner = "building", name = "GetDeveloperWealth")]
    pub unsafe extern "fastcall" fn fc4p_00698d80(this: *mut u8, entry_edx: u32, kind: u32) -> u64 {
        let args = [kind];
        let mut c = RawCall::new(Conv::Cdecl, crate::abi::target(0x0069_5840), this as u32, &args);
        c.eax = kind;
        c.edx = entry_edx;
        // SAFETY: the game's cdecl conversion takes one word.
        unsafe { call_raw(&mut c) };
        (u64::from(c.edx) << 32) | u64::from(c.eax)
    }
}
port! {
    /// Combines two byte categories, wrapping AL while retaining EAX's upper bytes.
    #[port(address = 0x0069_8D90, owner = "building", name = "PackCapacityProfile", naked)]
    pub unsafe extern "C" fn fc4p_00698d90() {
        core::arch::naked_asm!("mov al, byte ptr [esp + 4]", "mov cl, byte ptr [esp + 8]", "shl al, 2", "add al, cl", "inc al", "ret 8");
    }
}
port! {
    /// Releases through the primary interface from the message-target interface.
    #[port(address = 0x0069_8DC0, owner = "building", name = "MessageTargetRelease", naked)]
    pub unsafe extern "C" fn fc4p_00698dc0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "push 0x006959b0", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {target}", target = sym release_primary);
    }
}
port! {
    /// Releases through the primary interface from the serializable interface.
    #[port(address = 0x0069_8DD0, owner = "building", name = "SerializableRelease", naked)]
    pub unsafe extern "C" fn fc4p_00698dd0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 12", "push 0x006959b0", "ret");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 12", "jmp {target}", target = sym release_primary);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00698d00::PORT,
    fc4p_00698d10::PORT,
    fc4p_00698d20::PORT,
    fc4p_00698d50::PORT,
    fc4p_00698d60::PORT,
    fc4p_00698d80::PORT,
    fc4p_00698d90::PORT,
    fc4p_00698dc0::PORT,
    fc4p_00698dd0::PORT,
];

#[cfg(test)]
mod tests;
