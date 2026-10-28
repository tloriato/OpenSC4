// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Scalar statistics, worst-tract outputs and occupant-list counts.

use super::*;

game_data! { static landfill_tile_capacity: u32 = 0x00B4_9BA4; }

port! {
    /// GetTotalGarbageUncollected.
    #[port(address = 0x006e_3000, owner = "pollution", name = "cSC4PollutionSimulator::GetTotalGarbageUncollected", naked)]
    pub unsafe extern "C" fn fc4p_006e3000() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x13c]", "ret");
    }
}

port! {
    /// GetAverageWaterValue.
    #[port(address = 0x006e_3010, owner = "pollution", name = "cSC4PollutionSimulator::GetAverageWaterValue", naked)]
    pub unsafe extern "C" fn fc4p_006e3010() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x104]", "ret");
    }
}

port! {
    /// GetAverageGarbageValue.
    #[port(address = 0x006e_3020, owner = "pollution", name = "cSC4PollutionSimulator::GetAverageGarbageValue", naked)]
    pub unsafe extern "C" fn fc4p_006e3020() {
        core::arch::naked_asm!("movzx eax, word ptr [ecx + 0x128]", "ret");
    }
}

port! {
    /// GetRecyclingCenterEffect.
    #[port(address = 0x006e_3150, owner = "pollution", name = "cSC4PollutionSimulator::GetRecyclingCenterEffect", naked)]
    pub unsafe extern "C" fn fc4p_006e3150() {
        core::arch::naked_asm!("lea eax, [ecx + 0x134]", "ret");
    }
}

port! {
    /// GetGarbageCapacity.
    #[port(address = 0x006e_5040, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageCapacity", naked)]
    pub unsafe extern "C" fn fc4p_006e5040() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x230]", "ret");
    }
}

port! {
    /// GetGarbageProducedLastMonth.
    #[port(address = 0x006e_4f30, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageProducedLastMonth", naked)]
    pub unsafe extern "C" fn fc4p_006e4f30() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x178]", "mov edx, dword ptr [ecx + 0x17c]", "ret");
    }
}

port! {
    /// GetGarbageConvertedToEnergyLastMonth.
    #[port(address = 0x006e_4f80, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageConvertedToEnergyLastMonth", naked)]
    pub unsafe extern "C" fn fc4p_006e4f80() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x1a0]", "mov edx, dword ptr [ecx + 0x1a4]", "ret");
    }
}

port! {
    /// GetGarbageLandfilledLastMonth.
    #[port(address = 0x006e_4f90, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageLandfilledLastMonth", naked)]
    pub unsafe extern "C" fn fc4p_006e4f90() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x1a8]", "mov edx, dword ptr [ecx + 0x1ac]", "ret");
    }
}

port! {
    /// GetGarbageAbductedPrevMonth.
    #[port(address = 0x006e_4f20, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageAbductedPrevMonth", naked)]
    pub unsafe extern "C" fn fc4p_006e4f20() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x1b8]", "xor edx, edx", "ret");
    }
}

port! {
    /// GetGarbageRecycledLastMonth.
    #[port(address = 0x006e_4f60, owner = "pollution", name = "cSC4PollutionSimulator::GetGarbageRecycledLastMonth")]
    pub unsafe extern "thiscall" fn fc4p_006e4f60(this: *mut u8) -> i64 {
        // SAFETY: the simulator contains the saved double counter.
        F80::from_f64(unsafe { PollutionSim::from_ptr(this) }.recycled_previous()).ftol()
    }
}

port! {
    /// Reads the process-wide landfill tile capacity, preserving EDX.
    #[port(address = 0x006E_4F10, owner = "pollution", name = "cSC4PollutionSimulator::GetLandfillTileCapacity")]
    pub unsafe extern "fastcall" fn fc4p_006e4f10(_this: *mut u8, edx: u32) -> u64 {
        // SAFETY: the game's tuning global.
        registers(unsafe { *landfill_tile_capacity() }, edx)
    }
}

port! {
    /// SetGarbageCapacity.
    #[port(address = 0x006e_5050, owner = "pollution", name = "cSC4PollutionSimulator::SetGarbageCapacity", naked)]
    pub unsafe extern "C" fn fc4p_006e5050() {
        core::arch::naked_asm!("mov eax, dword ptr [esp + 4]", "mov dword ptr [ecx + 0x230], eax", "ret 4");
    }
}

port! {
    /// GetWorstAirTract.
    #[port(address = 0x006e_4fa0, owner = "pollution", name = "cSC4PollutionSimulator::GetWorstAirTract")]
    pub unsafe extern "thiscall" fn fc4p_006e4fa0(this: *mut u8, x: *mut u32, z: *mut u32, value: *mut u32) -> u64 {
        // SAFETY: original output destinations may alias each other or simulator fields.
        unsafe {
            write_unaligned(x, u32::from(read_unaligned(this.add(0x210).cast::<u16>())));
            write_unaligned(z, u32::from(read_unaligned(this.add(0x212).cast::<u16>())));
            let result = read_unaligned(this.add(0x214).cast::<u32>());
            write_unaligned(value, result);
            registers(result, z as u32)
        }
    }
}

port! {
    /// GetWorstWaterTract.
    #[port(address = 0x006e_4fd0, owner = "pollution", name = "cSC4PollutionSimulator::GetWorstWaterTract")]
    pub unsafe extern "thiscall" fn fc4p_006e4fd0(this: *mut u8, x: *mut u32, z: *mut u32, value: *mut u32) -> u64 {
        // SAFETY: original output destinations may alias each other or simulator fields.
        unsafe {
            write_unaligned(x, u32::from(read_unaligned(this.add(0x218).cast::<u16>())));
            write_unaligned(z, u32::from(read_unaligned(this.add(0x21A).cast::<u16>())));
            let result = read_unaligned(this.add(0x21C).cast::<u32>());
            write_unaligned(value, result);
            registers(result, z as u32)
        }
    }
}

port! {
    /// GetWorstGarbageTract.
    #[port(address = 0x006e_5000, owner = "pollution", name = "cSC4PollutionSimulator::GetWorstGarbageTract")]
    pub unsafe extern "thiscall" fn fc4p_006e5000(this: *mut u8, x: *mut u32, z: *mut u32, value: *mut u32) -> u64 {
        // SAFETY: original output destinations may alias each other or simulator fields.
        unsafe {
            write_unaligned(x, u32::from(read_unaligned(this.add(0x220).cast::<u16>())));
            write_unaligned(z, u32::from(read_unaligned(this.add(0x222).cast::<u16>())));
            let result = read_unaligned(this.add(0x224).cast::<u32>());
            write_unaligned(value, result);
            registers(result, z as u32)
        }
    }
}

port! {
    /// GetIncineratorCount.
    #[port(address = 0x006e_50d0, owner = "pollution", name = "cSC4PollutionSimulator::GetIncineratorCount")]
    pub unsafe extern "thiscall" fn fc4p_006e50d0(this: *mut u8) -> u64 {
        // SAFETY: the simulator owns a live circular list with a sentinel.
        unsafe {
            let sentinel = PollutionSim::from_ptr(this).incinerator_list();
            let mut node = ListNode::from_ptr(sentinel).next();
            let mut count = 0u32;
            while node != sentinel {
                node = ListNode::from_ptr(node).next();
                count = count.wrapping_add(1);
            }
            registers(count, sentinel as u32)
        }
    }
}

port! {
    /// GetWasteToEnergyBuildingCount.
    #[port(address = 0x006e_50f0, owner = "pollution", name = "cSC4PollutionSimulator::GetWasteToEnergyBuildingCount")]
    pub unsafe extern "thiscall" fn fc4p_006e50f0(this: *mut u8) -> u64 {
        // SAFETY: the simulator owns a live circular list with a sentinel.
        unsafe {
            let sentinel = PollutionSim::from_ptr(this).energy_list();
            let mut node = ListNode::from_ptr(sentinel).next();
            let mut count = 0u32;
            while node != sentinel {
                node = ListNode::from_ptr(node).next();
                count = count.wrapping_add(1);
            }
            registers(count, sentinel as u32)
        }
    }
}

port! {
    /// GetRecyclingCenterCount.
    #[port(address = 0x006e_5110, owner = "pollution", name = "cSC4PollutionSimulator::GetRecyclingCenterCount")]
    pub unsafe extern "thiscall" fn fc4p_006e5110(this: *mut u8) -> u64 {
        // SAFETY: the simulator owns a live circular list with a sentinel.
        unsafe {
            let sentinel = PollutionSim::from_ptr(this).recycling_list();
            let mut node = ListNode::from_ptr(sentinel).next();
            let mut count = 0u32;
            while node != sentinel {
                node = ListNode::from_ptr(node).next();
                count = count.wrapping_add(1);
            }
            registers(count, sentinel as u32)
        }
    }
}

port! {
    /// GetWaterTreatmentPlantCount.
    #[port(address = 0x006e_5130, owner = "pollution", name = "cSC4PollutionSimulator::GetWaterTreatmentPlantCount")]
    pub unsafe extern "thiscall" fn fc4p_006e5130(this: *mut u8) -> u64 {
        // SAFETY: the simulator owns a live circular list with a sentinel.
        unsafe {
            let sentinel = PollutionSim::from_ptr(this).treatment_list();
            let mut node = ListNode::from_ptr(sentinel).next();
            let mut count = 0u32;
            while node != sentinel {
                node = ListNode::from_ptr(node).next();
                count = count.wrapping_add(1);
            }
            registers(count, sentinel as u32)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006e3000::PORT,
    fc4p_006e3010::PORT,
    fc4p_006e3020::PORT,
    fc4p_006e3150::PORT,
    fc4p_006e5040::PORT,
    fc4p_006e4f30::PORT,
    fc4p_006e4f80::PORT,
    fc4p_006e4f90::PORT,
    fc4p_006e4f20::PORT,
    fc4p_006e4f60::PORT,
    fc4p_006e4f10::PORT,
    fc4p_006e5050::PORT,
    fc4p_006e4fa0::PORT,
    fc4p_006e4fd0::PORT,
    fc4p_006e5000::PORT,
    fc4p_006e50d0::PORT,
    fc4p_006e50f0::PORT,
    fc4p_006e5110::PORT,
    fc4p_006e5130::PORT,
];
