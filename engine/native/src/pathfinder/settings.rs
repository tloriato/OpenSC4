// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Configuration and result access at the game's original field offsets.

use super::*;

port! {
    /// Return the full accepted-trip count.
    #[port(address = 0x0069_8CE0, owner = "pathfinder", name = "cSC4PathFinder::GetTrafficArrived", naked)]
    pub unsafe extern "C" fn fc4p_00698ce0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x6c]", "ret")
    }
}

port! {
    /// Add flags and return the supplied mask.
    #[port(address = 0x006D_6CF0, owner = "pathfinder", name = "cSC4PathFinder::SetFlag")]
    pub unsafe extern "thiscall" fn fc4p_006d6cf0(this: *mut u8, mask: u32) -> u64 {
        let pf = unsafe { Pathfinder::from_ptr(this) };
        let flags = pf.flags() | mask;
        pf.set_flags(flags);
        (u64::from(flags) << 32) | u64::from(mask)
    }
}

port! {
    /// Clear just the flags word, retaining the incoming EAX.
    #[port(address = 0x006D_6D20, owner = "pathfinder", name = "cSC4PathFinder::ClearAllFlags", naked)]
    pub unsafe extern "C" fn fc4p_006d6d20() {
        core::arch::naked_asm!("mov dword ptr [ecx + 0x70], 0", "ret")
    }
}

port! {
    /// Set the destination filter without restricting its value.
    #[port(address = 0x006D_6D30, owner = "pathfinder", name = "cSC4PathFinder::SetTripDestinationFilter", naked)]
    pub unsafe extern "C" fn fc4p_006d6d30() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0x80], eax", "ret 4")
    }
}

port! {
    /// Add the argument's low byte to the destination flags. EAX's upper bytes survive.
    #[port(address = 0x006D_6D40, owner = "pathfinder", name = "cSC4PathFinder::SetDestinationFlag", naked)]
    pub unsafe extern "C" fn fc4p_006d6d40() {
        core::arch::naked_asm!(
            "push dword ptr [esp + 4]", "push edx", "mov edx, eax", "call {body}", "ret 4",
            body = sym destination_flag,
        )
    }
}

unsafe extern "fastcall" fn destination_flag(this: *mut u8, entry: u32, entry_edx: u32, mask: u32) -> u64 {
    let pf = unsafe { Pathfinder::from_ptr(this) };
    let flags = pf.destinations() | mask as u8;
    pf.set_destinations(flags);
    (u64::from((entry_edx & !0xFF) | u32::from(flags)) << 32) | u64::from((entry & !0xFF) | (mask & 0xFF))
}

port! {
    /// Clear only the destination byte, retaining EAX.
    #[port(address = 0x006D_6D80, owner = "pathfinder", name = "cSC4PathFinder::ClearAllDestinationFlags", naked)]
    pub unsafe extern "C" fn fc4p_006d6d80() {
        core::arch::naked_asm!("mov byte ptr [ecx + 0x84], 0", "ret")
    }
}

port! {
    /// Add the low word to allowed types, and to starting types when the second argument's low byte is nonzero.
    #[port(address = 0x006D_6D90, owner = "pathfinder", name = "cSC4PathFinder::SetTravelTypeFlag", naked)]
    pub unsafe extern "C" fn fc4p_006d6d90() {
        core::arch::naked_asm!("jmp {body}", body = sym travel_flag)
    }
}

unsafe extern "fastcall" fn travel_flag(this: *mut u8, entry_edx: u32, mask: u32, also_starting: u32) -> u64 {
    let pf = unsafe { Pathfinder::from_ptr(this) };
    pf.set_allowed(pf.allowed() | mask as u16);
    if also_starting as u8 != 0 {
        pf.set_starting(pf.starting() | mask as u16);
    }
    (u64::from((entry_edx & !0xFF) | (also_starting & 0xFF)) << 32) | u64::from(mask)
}

port! {
    /// Clear the starting mask before the allowed mask, returning zero.
    #[port(address = 0x006D_6DD0, owner = "pathfinder", name = "cSC4PathFinder::ClearAllTravelTypeFlags", naked)]
    pub unsafe extern "C" fn fc4p_006d6dd0() {
        core::arch::naked_asm!("xor eax, eax", "mov [ecx + 0x66], ax", "mov [ecx + 0x64], ax", "ret")
    }
}

port! {
    /// Retain the caller's starting-cost array address without reading the array.
    #[port(address = 0x006D_6DE0, owner = "pathfinder", name = "cSC4PathFinder::SetStartingCostArray", naked)]
    pub unsafe extern "C" fn fc4p_006d6de0() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0x90], eax", "ret 4")
    }
}

port! {
    /// Store the float's bits in the shared maximum/result trip-length word, without loading the x87.
    #[port(address = 0x006D_6DF0, owner = "pathfinder", name = "cSC4PathFinder::SetMaxTripLength", naked)]
    pub unsafe extern "C" fn fc4p_006d6df0() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0x60], eax", "ret 4")
    }
}

port! {
    /// Store and return the full traffic-density word.
    #[port(address = 0x006D_6E00, owner = "pathfinder", name = "cSC4PathFinder::SetTrafficDensity", naked)]
    pub unsafe extern "C" fn fc4p_006d6e00() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0x68], eax", "ret 4")
    }
}

port! {
    /// Set the four start bounds without normalizing them; return x1 in EAX and z1 in EDX.
    #[port(address = 0x006D_6E10, owner = "pathfinder", name = "cSC4PathFinder::SetStartRect")]
    pub unsafe extern "thiscall" fn fc4p_006d6e10(this: *mut u8, x0: i32, z0: i32, x1: i32, z1: i32) -> u64 {
        let pf = unsafe { Pathfinder::from_ptr(this) };
        pf.set_start_x0(x0);
        pf.set_start_z0(z0);
        pf.set_start_x1(x1);
        pf.set_start_z1(z1);
        (u64::from(z1 as u32) << 32) | u64::from(x1 as u32)
    }
}

port! {
    /// Set the four end bounds without normalizing them; return x1 in EAX and z1 in EDX.
    #[port(address = 0x006D_6E40, owner = "pathfinder", name = "cSC4PathFinder::SetEndRect")]
    pub unsafe extern "thiscall" fn fc4p_006d6e40(this: *mut u8, x0: i32, z0: i32, x1: i32, z1: i32) -> u64 {
        let pf = unsafe { Pathfinder::from_ptr(this) };
        pf.set_end_x0(x0);
        pf.set_end_z0(z0);
        pf.set_end_x1(x1);
        pf.set_end_z1(z1);
        (u64::from(z1 as u32) << 32) | u64::from(x1 as u32)
    }
}

port! {
    /// Set the work budget without changing suspension or search state.
    #[port(address = 0x006D_6E70, owner = "pathfinder", name = "cSC4PathFinder::SetMaxWorkTime", naked)]
    pub unsafe extern "C" fn fc4p_006d6e70() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0xc0], eax", "ret 4")
    }
}

port! {
    /// Clear only suspension, retaining EAX and the previous result.
    #[port(address = 0x006D_6E80, owner = "pathfinder", name = "cSC4PathFinder::Reset", naked)]
    pub unsafe extern "C" fn fc4p_006d6e80() {
        core::arch::naked_asm!("mov byte ptr [ecx + 0xc4], 0", "ret")
    }
}

port! {
    /// Load the stored trip length into ST0, leaving EAX alone.
    #[port(address = 0x006D_6E90, owner = "pathfinder", name = "cSC4PathFinder::GetTripLength", naked)]
    pub unsafe extern "C" fn fc4p_006d6e90() {
        core::arch::naked_asm!("fld dword ptr [ecx + 0x60]", "ret")
    }
}

port! {
    /// Write x, then read and write z. Output pointers may alias the pathfinder or one another.
    #[port(address = 0x006D_6EA0, owner = "pathfinder", name = "cSC4PathFinder::GetEndCell")]
    pub unsafe extern "thiscall" fn fc4p_006d6ea0(this: *mut u8, x: *mut i32, z: *mut i32) -> u64 {
        let pf = unsafe { Pathfinder::from_ptr(this) };
        unsafe { core::ptr::write_unaligned(x, pf.end_x()) };
        let value = pf.end_z();
        unsafe { core::ptr::write_unaligned(z, value) };
        (u64::from(x as u32) << 32) | u64::from(value as u32)
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_00698ce0::PORT,
    fc4p_006d6cf0::PORT,
    fc4p_006d6d20::PORT,
    fc4p_006d6d30::PORT,
    fc4p_006d6d40::PORT,
    fc4p_006d6d80::PORT,
    fc4p_006d6d90::PORT,
    fc4p_006d6dd0::PORT,
    fc4p_006d6de0::PORT,
    fc4p_006d6df0::PORT,
    fc4p_006d6e00::PORT,
    fc4p_006d6e10::PORT,
    fc4p_006d6e40::PORT,
    fc4p_006d6e70::PORT,
    fc4p_006d6e80::PORT,
    fc4p_006d6e90::PORT,
    fc4p_006d6ea0::PORT,
];

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
