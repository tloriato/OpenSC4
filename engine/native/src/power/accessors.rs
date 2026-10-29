// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Stored plant statistics, the powered grid, and secondary interface forwarding.

use super::*;

game_fn! { #[allow(dead_code)] fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 = "thiscall" 0x006E_ABC0; }
game_fn! { #[allow(dead_code)] fn release(this: *mut u8) -> u32 = "thiscall" 0x006E_B190; }

macro_rules! getter {
    ($export:ident, $address:literal, $name:literal, $load:literal) => {
        port! {
            /// Return the stored word without changing EDX or the object.
            #[port(address = $address, owner = "power", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!($load, "ret");
            }
        }
    };
}

getter!(
    fc4p_006ef9f0,
    0x006E_F9F0,
    "cSC4PowerSimulator::GetPowerPlantCount",
    "mov eax, [ecx + 0x2dc]"
);
getter!(
    fc4p_006efa10,
    0x006E_FA10,
    "cSC4PowerSimulator::GetPowerPoleCount",
    "mov eax, [ecx + 0x2b8]"
);
getter!(
    fc4p_006efa20,
    0x006E_FA20,
    "cSC4PowerSimulator::GetOverworkedPowerPlantCount",
    "mov eax, [ecx + 0x308]"
);
getter!(
    fc4p_006efa30,
    0x006E_FA30,
    "cSC4PowerSimulator::GetAgedPowerPlantCount",
    "mov eax, [ecx + 0x30c]"
);
getter!(
    fc4p_006efa40,
    0x006E_FA40,
    "cSC4PowerSimulator::GetDistressedPowerPlantCount",
    "mov eax, [ecx + 0x310]"
);
getter!(
    fc4p_006efa50,
    0x006E_FA50,
    "cSC4PowerSimulator::GetPoweredGrid",
    "mov eax, [ecx + 0x80]"
);

port! {
    /// Index the stored type words directly, with 32-bit address wrapping and no range check.
    #[port(address = 0x006E_FA00, owner = "power", name = "cSC4PowerSimulator::GetPowerPlantCount(type)", naked)]
    pub unsafe extern "C" fn fc4p_006efa00() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov eax, [ecx + eax*4 + 0x2e0]", "ret 4");
    }
}

macro_rules! forward {
    ($export:ident, $address:literal, $name:literal, $adjust:literal, $target:literal, $test_target:path) => {
        port! {
            /// Adjust the secondary interface pointer and forward all arguments and return registers.
            #[port(address = $address, owner = "power", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!($adjust, "jmp dword ptr [2f]", $target);
                #[cfg(test)]
                core::arch::naked_asm!($adjust, "jmp {}", sym $test_target);
            }
        }
    };
}

forward!(
    fc4p_006efa80,
    0x006E_FA80,
    "cSC4PowerSimulator::SerializableQueryInterface",
    "sub ecx, 8",
    "2: .long 0x006eabc0",
    super::com::fc4p_006eabc0
);
forward!(
    fc4p_006efa90,
    0x006E_FA90,
    "cSC4PowerSimulator::MessageTargetRelease",
    "sub ecx, 4",
    "2: .long 0x006eb190",
    super::com::fc4p_006eb190
);
forward!(
    fc4p_006efaa0,
    0x006E_FAA0,
    "cSC4PowerSimulator::SerializableRelease",
    "sub ecx, 8",
    "2: .long 0x006eb190",
    super::com::fc4p_006eb190
);

pub const PORTS: &[Port] = &[
    fc4p_006ef9f0::PORT,
    fc4p_006efa00::PORT,
    fc4p_006efa10::PORT,
    fc4p_006efa20::PORT,
    fc4p_006efa30::PORT,
    fc4p_006efa40::PORT,
    fc4p_006efa50::PORT,
    fc4p_006efa80::PORT,
    fc4p_006efa90::PORT,
    fc4p_006efaa0::PORT,
];

#[cfg(test)]
mod tests;
