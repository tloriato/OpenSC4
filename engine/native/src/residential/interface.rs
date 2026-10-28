// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Identity, reference counting, strike predicates, and retained grid access.

use super::ResidentialSim;
use crate::port::Port;

game_fn! {
    fn property_u32(holder: *mut u8, id: u32, value: *mut u32) -> u64 = "C" 0x005F_D480;
}

port! {
    /// Class identity; the remaining registers are untouched.
    #[port(address = 0x006F_6E50, owner = "residential", name = "cSC4ResidentialSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006f6e50() {
        core::arch::naked_asm!("mov eax, 0x4990c013", "ret")
    }
}

port! {
    /// Known interfaces share the primary AddRef. An unknown ID leaves the output untouched.
    /// The upper result bytes come from AddRef, or the adjusted unknown ID.
    #[port(address = 0x006F_6E60, owner = "residential", name = "cSC4ResidentialSimulator::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_006f6e60(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        unsafe {
            let offset = match iid {
                1 | 0x077A_C1EE => 0,
                0x4522_94AA => 4,
                0xE4FD_A3D4 => 8,
                _ => {
                    let result = if iid < 0x4522_94AA { iid.wrapping_sub(0x077A_C1EE) } else { iid };
                    return result & !0xFF;
                }
            };
            out.write_unaligned(if this.is_null() { this } else { this.add(offset) });
            (vcall!(this, 4 => fn() -> u32) & !0xFF) | 1
        }
    }
}

port! {
    /// Reset both age histograms, the life-expectancy sum, and the running population.
    #[port(address = 0x006F_6EE0, owner = "residential", name = "ResetPopulationStatistics")]
    pub unsafe extern "thiscall" fn fc4p_006f6ee0(this: *mut u8) -> u64 {
        unsafe {
            // Interleave the aligned word stores in age-bin order, without a runtime memory helper.
            for i in 0..20 {
                this.add(0x1C4 + i * 4).cast::<u32>().write_volatile(0);
                this.add(0x214 + i * 4).cast::<u32>().write_volatile(0);
            }
            let sim = ResidentialSim::from_ptr(this);
            sim.set_life_sum_low(0);
            sim.set_life_sum_high(0);
            sim.set_running_population(0);
            this.wrapping_add(0x264) as u32 as u64
        }
    }
}

port! {
    /// Normalize the school strike byte, preserving the caller's upper EAX and EDX.
    #[port(address = 0x006F_6F10, owner = "residential", name = "cSC4ResidentialSimulator::SchoolIsOnStrike", naked)]
    pub unsafe extern "C" fn fc4p_006f6f10() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0x178]", "test al, al", "setnz al", "ret")
    }
}

port! {
    /// Normalize the health strike byte, preserving the caller's upper EAX and EDX.
    #[port(address = 0x006F_6F20, owner = "residential", name = "cSC4ResidentialSimulator::HealthIsOnStrike", naked)]
    pub unsafe extern "C" fn fc4p_006f6f20() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0x118]", "test al, al", "setnz al", "ret")
    }
}

port! {
    /// A building strikes when its strike-day property exists and is nonzero. ECX returns the day word;
    /// EDX is zero on success, otherwise it is retained from the property helper.
    #[port(address = 0x006F_6F30, owner = "residential", name = "cSC4ResidentialSimulator::IsBuildingOnStrike", naked)]
    pub unsafe extern "C" fn fc4p_006f6f30() {
        core::arch::naked_asm!(
            "sub esp, 12", "mov edx, esp", "push dword ptr [esp + 16]", "call {body}",
            "mov eax, dword ptr [esp]", "mov edx, dword ptr [esp + 4]",
            "mov ecx, dword ptr [esp + 8]", "add esp, 12", "ret 4",body=sym building_on_strike,
        )
    }
}

#[repr(C)]
struct StrikeResult {
    eax: u32,
    edx: u32,
    day: u32,
}

unsafe extern "fastcall" fn building_on_strike(_this: *mut u8, out: *mut StrikeResult, building: *mut u8) {
    unsafe {
        let mut day = 0;
        let holder = vcall!(building,0x18 => fn()->*mut u8);
        let result = property_u32(holder, 0xEAA6_9F83, &mut day);
        let (eax, edx) = if result as u8 == 0 {
            (result as u32 & !0xFF, (result >> 32) as u32)
        } else {
            (u32::from(day != 0), 0)
        };
        out.write(StrikeResult { eax, edx, day });
    }
}

port! {
    /// Published population; EDX is unchanged.
    #[port(address = 0x006F_6F70, owner = "residential", name = "cSC4ResidentialSimulator::GetPopulation", naked)]
    pub unsafe extern "C" fn fc4p_006f6f70() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0xa4]", "ret")
    }
}

port! {
    /// Cdecl byte assignment callback; upper EAX and EDX are unchanged.
    #[port(address = 0x006F_6F80, owner = "residential", name = "AssignGridByte", naked)]
    pub unsafe extern "C" fn fc4p_006f6f80() {
        core::arch::naked_asm!("mov al, byte ptr [esp + 8]", "mov ecx, dword ptr [esp + 4]", "mov byte ptr [ecx], al", "ret")
    }
}

port! {
    /// Borrow the proximity map for wealth 1..=3; other wealth values return null.
    /// The unused fastcall register parameter carries the unchanged EDX back to the caller.
    #[port(address = 0x006F_6F90, owner = "residential", name = "cSC4ResidentialSimulator::GetProximityMap")]
    pub unsafe extern "fastcall" fn fc4p_006f6f90(this: *mut u8, edx: u32, wealth: u32) -> u64 {
        let sim = unsafe { ResidentialSim::from_ptr(this) };
        let map = match wealth { 1 => sim.proximity_low(), 2 => sim.proximity_medium(), 3 => sim.proximity_high(), _ => core::ptr::null_mut() };
        (u64::from(edx) << 32) | u64::from(map as u32)
    }
}

/// Store three grid pointers before retaining any of them. Each output is reread before its AddRef.
unsafe fn retain_grids(this: *mut u8, offset: usize, a: *mut *mut u8, b: *mut *mut u8, c: *mut *mut u8) -> u64 {
    unsafe {
        a.write_unaligned(this.add(offset).cast::<*mut u8>().read_unaligned());
        b.write_unaligned(this.add(offset + 4).cast::<*mut u8>().read_unaligned());
        c.write_unaligned(this.add(offset + 8).cast::<*mut u8>().read_unaligned());
        vcall!(a.read_unaligned(), 4 => fn() -> u32);
        let eax = vcall!(b.read_unaligned(), 4 => fn() -> u32);
        let object = c.read_unaligned();
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(object, 4),
            object as u32,
            &[],
        );
        call.eax = eax;
        call.edx = object.cast::<u32>().read_unaligned();
        crate::abi::call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

port! {
    /// Return and retain the three EQ grids in wealth order.
    #[port(address = 0x006F_72C0, owner = "residential", name = "cSC4ResidentialSimulator::GetEQGrids")]
    pub unsafe extern "thiscall" fn fc4p_006f72c0(this: *mut u8, a: *mut *mut u8, b: *mut *mut u8, c: *mut *mut u8) -> u64 {
        unsafe { retain_grids(this, 0x7C, a, b, c) }
    }
}

port! {
    /// Return and retain the three population grids in wealth order.
    #[port(address = 0x006F_7340, owner = "residential", name = "cSC4ResidentialSimulator::GetPopulationGrids")]
    pub unsafe extern "thiscall" fn fc4p_006f7340(this: *mut u8, a: *mut *mut u8, b: *mut *mut u8, c: *mut *mut u8) -> u64 {
        unsafe { retain_grids(this, 0x48, a, b, c) }
    }
}

port! {
    /// A signed count above one is decremented. Otherwise shut down, then invoke the deleting destructor.
    #[port(address = 0x006F_7DD0, owner = "residential", name = "cSC4ResidentialSimulator::Release")]
    pub unsafe extern "thiscall" fn fc4p_006f7dd0(this: *mut u8) -> u32 {
        unsafe {
            let sim = ResidentialSim::from_ptr(this);
            let count = sim.references();
            if count > 1 {
                let next = count.wrapping_sub(1);
                sim.set_references(next);
                return next as u32;
            }
            vcall!(this, 0x10 => fn() -> u32);
            vcall!(this, 0xF0 => fn(u32) -> u32, 1);
            0
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006f6e50::PORT,
    fc4p_006f6e60::PORT,
    fc4p_006f6ee0::PORT,
    fc4p_006f6f10::PORT,
    fc4p_006f6f20::PORT,
    fc4p_006f6f30::PORT,
    fc4p_006f6f70::PORT,
    fc4p_006f6f80::PORT,
    fc4p_006f6f90::PORT,
    fc4p_006f72c0::PORT,
    fc4p_006f7340::PORT,
    fc4p_006f7dd0::PORT,
];

#[cfg(test)]
mod tests;
