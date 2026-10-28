// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Identity, zoning, construction suitability, and developer categories.

use super::*;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! {
    fn building_fits(building_zone: i32, cell_zone: i32) -> u32 = "C" 0x0069_5390;
}
game_fn! {
    fn demolition_property(holder: *mut u8, property: u32, out: *mut u64) -> u32 = "C" 0x005F_D4B0;
}
game_data! {
    static world_to_cell: f32 = 0x00B0_D1DC;
}

layout! {
    /// The construction candidate supplied by a growth developer.
    pub struct Candidate {
        0x00 => zone / set_zone: i32,
        0x08 => land_value / set_land_value: u32,
        0x10 => min_land_value / set_min_land_value: u8,
        0x11 => max_land_value / set_max_land_value: u8,
        0x1C => x / set_x: i32,
        0x20 => z / set_z: i32,
    }
}

/// Calls a virtual integer method with the register inputs retained by the original caller.
unsafe fn integer_call(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
    // SAFETY: the enclosing port supplies the live interface and its exact stack arguments.
    unsafe {
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(object, slot),
            object as u32,
            args,
        );
        call.eax = eax;
        call.edx = edx;
        crate::abi::call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

port! {
    /// Keeps the three argument words available as the coordinate outputs and occupant output.
    #[port(address = 0x0069_5590, owner = "building", name = "cSC4BuildingDevelopmentSimulator::LocationIsOKForNewBuilding", naked)]
    pub unsafe extern "C" fn fc4p_00695590() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push dword ptr [esp + 12]",
            "push dword ptr [esp + 12]", "push dword ptr [esp + 12]", "push eax",
            "call {body}", "ret 12", body = sym location_ok,
        );
    }
}

/// # Safety
/// The entry supplies a live simulator and the three writable original argument words.
unsafe extern "fastcall" fn location_ok(
    this: *mut u8,
    entry_edx: u32,
    args: *mut u32,
    candidate: u32,
    x: u32,
    z: u32,
) -> u64 {
    // SAFETY: all interfaces and stack outputs are the ones supplied to the original method.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let check_args = [candidate];
        let mut check = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(this, 0x3C),
            this as u32,
            &check_args,
        );
        check.eax = read_unaligned(this.cast());
        check.edx = entry_edx;
        crate::abi::call_raw(&mut check);
        if check.eax as u8 == 0 {
            return (u64::from(check.edx) << 32) | u64::from(check.eax);
        }
        let city = sim.city();
        write_unaligned(args, 0);
        vcall!(city, 0x17C => fn(u32, u32, *mut u32, *mut u32), x, z, args.add(1), args.add(2));
        let occupants = sim.occupants();
        let table = read_unaligned(occupants.cast::<u32>());
        let words = [
            args as u32,
            read_unaligned(args.add(1)),
            read_unaligned(args.add(2)),
            0x2781_28A0,
        ];
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(occupants, 0x84),
            occupants as u32,
            &words,
        );
        call.eax = args as u32;
        call.edx = table;
        crate::abi::call_raw(&mut call);
        let occupant = read_unaligned(args) as *mut u8;
        let present = !occupant.is_null();
        if present {
            let mut release = crate::abi::RawCall::new(
                crate::abi::Conv::Thiscall,
                crate::abi::vslot(occupant, 8),
                occupant as u32,
                &[],
            );
            release.eax = call.eax;
            release.edx = read_unaligned(occupant.cast());
            crate::abi::call_raw(&mut release);
            call.edx = release.edx;
        }
        (u64::from(call.edx) << 32) | u64::from(!present)
    }
}

port! {
    /// Reads the signed 64-bit demolition property through the occupant's two property interfaces.
    /// The second stack argument is ignored.
    #[port(address = 0x0069_5700, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetCostOfOccupantDemolition")]
    pub unsafe extern "thiscall" fn fc4p_00695700(_this: *mut u8, occupant: *mut u8, _flags: u32) -> u64 {
        // SAFETY: the game supplies the occupant and both zero-argument interface getters.
        unsafe {
            let mut result = 0u64;
            let first = vcall!(occupant, 0x0C => fn() -> *mut u8);
            let holder = vcall!(first, 0x18 => fn() -> *mut u8);
            demolition_property(holder, 0x099A_FACD, &mut result);
            result
        }
    }
}

port! {
    /// Preserves incoming EDX on the absent-manager path.
    #[port(address = 0x0069_5740, owner = "building", name = "InsertOccupantResource", naked)]
    pub unsafe extern "C" fn fc4p_00695740() {
        core::arch::naked_asm!("jmp {body}", body = sym insert_occupant_resource);
    }
}

/// # Safety
/// The game supplies a live simulator, incoming EDX, and an occupant resource interface.
unsafe extern "fastcall" fn insert_occupant_resource(this: *mut u8, entry_edx: u32, occupant: *mut u8) -> u64 {
    // SAFETY: the simulator's occupant manager and the occupant's getter have these signatures.
    unsafe {
        let sim = BuildingSim::from_ptr(this);
        let manager = sim.occupants();
        if manager.is_null() {
            return u64::from(entry_edx) << 32;
        }
        let table = read_unaligned(manager.cast::<*const usize>());
        let mut get = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(occupant, 0x0C),
            occupant as u32,
            &[],
        );
        get.eax = manager as u32;
        get.edx = read_unaligned(occupant.cast());
        crate::abi::call_raw(&mut get);
        let manager = sim.occupants();
        let args = [get.eax, 0];
        let mut insert = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            read_unaligned(table.add(0x50 / 4)),
            manager as u32,
            &args,
        );
        insert.eax = get.eax;
        insert.edx = get.edx;
        crate::abi::call_raw(&mut insert);
        (u64::from(insert.edx) << 32) | u64::from(insert.eax)
    }
}

port! {
    /// The serializable interface's class identity. EDX is preserved.
    #[port(address = 0x0069_5280, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_00695280() {
        core::arch::naked_asm!("mov eax, 0x8990be7a", "ret");
    }
}

port! {
    /// Known interfaces retain the primary object. Unknown IDs leave the output untouched.
    /// EAX retains the upper bytes of AddRef's result, or the adjusted unknown ID.
    #[port(address = 0x0069_5290, owner = "building", name = "cSC4BuildingDevelopmentSimulator::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_00695290(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        let offset = match iid {
            1 | 0x0781_3EAE => 0,
            0x4522_94AA => 8,
            0xE98B_2F57 => 4,
            0xE4FD_A3D4 => 12,
            _ => {
                let eax = if iid < 0x4522_94AA { iid.wrapping_sub(0x0781_3EAE) } else { iid };
                return eax & !255;
            }
        };
        // SAFETY: the caller supplies its output location and a live primary interface.
        unsafe {
            write_unaligned(out, if this.is_null() { this } else { this.wrapping_add(offset) });
            let result = vcall!(this, 4 => fn() -> u32);
            (result & !255) | 1
        }
    }
}

port! {
    /// Low and medium density use signed stage limits; high density accepts any stage.
    #[port(address = 0x0069_5340, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GrowthStageFitsInZone")]
    pub unsafe extern "C" fn fc4p_00695340(stage: i32, zone: i32) -> u32 {
        match zone {
            1 | 4 => u32::from(stage <= 3),
            2 | 5 => u32::from(stage <= 6),
            3 | 6..=9 => 1,
            _ => (zone as u32).wrapping_sub(1) & !255,
        }
    }
}

port! {
    /// A denser cell accepts lower density in the same family. The one-byte branches
    /// preserve the building zone's upper bytes in EAX.
    #[port(address = 0x0069_5390, owner = "building", name = "cSC4BuildingDevelopmentSimulator::BuildingFitsInZone")]
    pub unsafe extern "C" fn fc4p_00695390(building: i32, cell: i32) -> u32 {
        let upper = building as u32 & !255;
        if building == cell {
            return upper | 1;
        }
        match cell {
            2 => upper | u32::from(building == 1),
            3 => u32::from(building == 1 || building == 2),
            5 => upper | u32::from(building == 4),
            6 => u32::from(building == 4 || building == 5),
            9 => upper | u32::from(building == 8),
            11 | 12 | 15 => upper | u32::from(building == 0),
            _ => upper,
        }
    }
}

port! {
    /// Checks zoning, lot presence, power, land value, and lot state in that order.
    /// Failure writes its reason; success leaves the reason word untouched.
    #[port(address = 0x0069_5430, owner = "building", name = "cSC4BuildingDevelopmentSimulator::IsSuitableForConstruction")]
    pub unsafe extern "thiscall" fn fc4p_00695430(this: *mut u8, candidate: *mut u8, reason: *mut u32) -> u64 {
        // SAFETY: all pointers and virtual interfaces belong to the game's live candidate and simulator.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let c = Candidate::from_ptr(candidate);
            let zone = vcall!(sim.zones(), 0x30 => fn(i32, i32) -> u32, c.x(), c.z());
            let fit = building_fits(c.zone(), i32::from(zone as i8));
            if fit as u8 == 0 {
                write_unaligned(reason, 0);
                return registers(fit, reason as u32);
            }
            let lots = sim.lots();
            let lot_result = integer_call(lots, 0x18, &[c.x() as u32, c.z() as u32, 1], read_unaligned(lots.cast()), c.z() as u32);
            let lot = lot_result as u32 as *mut u8;
            let mut edx = (lot_result >> 32) as u32;
            if lot.is_null() {
                write_unaligned(reason, 3);
                return registers(reason as u32 & !255, edx);
            }
            if sim.ignore_power() == 0 {
                let service = sim.power();
                let power = integer_call(service, 0x4C, &[c.x() as u32, c.z() as u32], c.x() as u32, read_unaligned(service.cast()));
                if power as u8 == 0 {
                    write_unaligned(reason, 5);
                    return power;
                }
            }
            if c.min_land_value() == 0 && c.max_land_value() == 0 {
                let service = sim.land_value();
                let value = integer_call(service, 0x14, &[c.x() as u32, c.z() as u32], c.x() as u32, read_unaligned(service.cast()));
                edx = (value >> 32) as u32;
                let value = value as u32;
                if !matches!(c.zone(), 7..=9) && value != c.land_value() {
                    write_unaligned(reason, 2);
                    return registers(value & !255, edx);
                }
            } else {
                let service = sim.land_value();
                let value = integer_call(service, 0x18, &[c.x() as u32, c.z() as u32], c.x() as u32, read_unaligned(service.cast())) as u8;
                if value < c.min_land_value() || value > c.max_land_value() {
                    write_unaligned(reason, 1);
                    return registers(0, reason as u32);
                }
                edx = u32::from(c.max_land_value());
            }
            let state = integer_call(lot, 0x84, &[], read_unaligned(lot.cast()), edx);
            if state as u32 != 1 {
                write_unaligned(reason, 4);
                return state & !255;
            }
            (state & 0xFFFF_FFFF_0000_0000) | 1
        }
    }
}

port! {
    /// Converts a world rectangle to cells, then clamps only the lower starts and upper ends.
    /// The last dimension comparison determines EAX.
    #[port(address = 0x0069_5610, owner = "building", name = "WorldRectangleToCells")]
    pub unsafe extern "thiscall" fn fc4p_00695610(this: *mut u8, source: *const f32, out: *mut i32) -> i32 {
        // SAFETY: four input/output elements and the simulator's dimensions are supplied by the game.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let mut scale = F80::from_f32(read_unaligned(world_to_cell()));
            for i in 0..4 {
                let result = (scale * F80::from_f32(read_unaligned(source.add(i)))).ftol() as i32;
                if i != 3 { scale = F80::from_f32(read_unaligned(world_to_cell())); }
                write_unaligned(out.add(i), result);
            }
            if read_unaligned(out) < 0 { write_unaligned(out, 0); }
            if read_unaligned(out.add(1)) < 0 { write_unaligned(out.add(1), 0); }
            let width = sim.cells_x();
            if read_unaligned(out.add(2)) >= width { write_unaligned(out.add(2), width.wrapping_sub(1)); }
            let height = sim.cells_z();
            if read_unaligned(out.add(3)) >= height {
                let last = height.wrapping_sub(1);
                write_unaligned(out.add(3), last);
                last
            } else { height }
        }
    }
}

port! {
    /// Maps purpose and wealth to a developer type. Industrial purposes ignore wealth.
    #[port(address = 0x0069_5790, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetDeveloperType_Static")]
    pub unsafe extern "C" fn fc4p_00695790(purpose: u32, wealth: u32) -> u32 {
        match (purpose, wealth) {
            (1, 1) => 0x1010, (1, 2) => 0x1020, (1, 3) => 0x1030,
            (2, 1) => 0x3110, (2, 2) => 0x3120, (2, 3) => 0x3130,
            (3, 2) => 0x3320, (3, 3) => 0x3330,
            (5, _) => 0x4100, (6, _) => 0x4200, (7, _) => 0x4300, (8, _) => 0x4400,
            _ => 0,
        }
    }
}

port! {
    /// Returns the wealth associated with a developer type, or zero for an unknown type.
    #[port(address = 0x0069_5840, owner = "building", name = "DeveloperWealth")]
    pub unsafe extern "C" fn fc4p_00695840(developer: i32) -> u32 {
        match developer {
            0x1010 | 0x3110 | 0x4100 => 1,
            0x1020 | 0x3120 | 0x3320 | 0x4200 | 0x4300 => 2,
            0x1030 | 0x3130 | 0x3330 | 0x4400 => 3,
            _ => 0,
        }
    }
}

port! {
    /// Uses the same primary vtable for the type conversion and developer lookup.
    #[port(address = 0x0069_58B0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::GetDeveloper(PurposeType,WealthType)")]
    pub unsafe extern "thiscall" fn fc4p_006958b0(this: *mut u8, purpose: u32, wealth: u32) -> u64 {
        // SAFETY: the primary interface has both methods with two and one stack arguments respectively.
        unsafe {
            let table = BuildingSim::from_ptr(this).vtable();
            let category: unsafe extern "thiscall" fn(*mut u8, u32, u32) -> u32 = core::mem::transmute(read_unaligned(table.add(0x78 / 4)));
            let kind = category(this, purpose, wealth);
            let lookup: unsafe extern "thiscall" fn(*mut u8, u32) -> u64 = core::mem::transmute(read_unaligned(table.add(0x80 / 4)));
            lookup(this, kind)
        }
    }
}

port! {
    /// Decrements counts above one. Otherwise shutdown precedes deleting destruction.
    #[port(address = 0x0069_59B0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::Release")]
    pub unsafe extern "thiscall" fn fc4p_006959b0(this: *mut u8) -> u32 {
        // SAFETY: the game supplies a retained simulator and its lifetime methods.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let count = sim.references();
            if count > 1 {
                let remaining = count.wrapping_sub(1);
                sim.set_references(remaining);
                remaining as u32
            } else {
                vcall!(this, 0x10 => fn());
                vcall!(this, 0xEC => fn(u32), 1);
                0
            }
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00695280::PORT,
    fc4p_00695290::PORT,
    fc4p_00695340::PORT,
    fc4p_00695390::PORT,
    fc4p_00695430::PORT,
    fc4p_00695590::PORT,
    fc4p_00695610::PORT,
    fc4p_00695700::PORT,
    fc4p_00695740::PORT,
    fc4p_00695790::PORT,
    fc4p_00695840::PORT,
    fc4p_006958b0::PORT,
    fc4p_006959b0::PORT,
];

#[cfg(test)]
mod tests;
