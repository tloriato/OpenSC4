// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Interface lifetime, cell predicates and shared small helpers.

use super::{Port, ZoneDeveloper};
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

game_data! { static pool_slot: *mut u8 = 0x00B0_D3B0; }
game_data! { static pool: u8 = 0x00B0_D3AC; }
game_data! { static broad_network_match: u8 = 0x00B4_B391; }
game_data! { static cell_scale: f32 = 0x00AB_36C0; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static rounding_bias: f32 = 0x00AB_36D4; }
game_fn! { fn refill_pool(this: *mut u8) -> *mut u8 = "thiscall" 0x0090_CF9A; }

port! {
    /// Return the density for residential, commercial, agricultural and industrial zone types.
    #[port(address = 0x0045_8960, owner = "zonedeveloper", name = "ZoneDensity")]
    pub unsafe extern "C" fn fc4p_00458960(zone: u32) -> u32 {
        match zone {
            1 | 4 => 1,
            2 | 5 => 2,
            3 | 6..=9 => 3,
            _ => 0,
        }
    }
}

port! {
    /// Take a free node, refilling an empty pool, then unlink it and increment the allocation count.
    #[port(address = 0x006B_F430, owner = "zonedeveloper", name = "AllocatePooledNode")]
    pub unsafe extern "C" fn fc4p_006bf430() -> *mut u8 {
        unsafe {
            let mut node = read_unaligned((*pool_slot()).add(8).cast::<*mut u8>());
            if node.is_null() {
                node = refill_pool(pool());
            }
            let next = read_unaligned(node.cast::<*mut u8>());
            write_unaligned((*pool_slot()).add(8).cast::<*mut u8>(), next);
            let count = (*pool_slot()).add(12).cast::<u32>();
            write_unaligned(count, read_unaligned(count).wrapping_add(1));
            node
        }
    }
}

port! {
    /// Return the zone developer class identifier.
    #[port(address = 0x0072_9EB0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_00729eb0() {
        core::arch::naked_asm!("mov eax, 0x498f9b01", "ret");
    }
}

port! {
    /// Compare the selected low nibbles, using the mask's wrapping byte-selection arithmetic.
    #[port(address = 0x0072_9EC0, owner = "zonedeveloper", name = "PlopMaskMatches")]
    pub unsafe extern "C" fn fc4p_00729ec0(mask: u32, value: u32) -> u32 {
        let selected = ((mask.wrapping_add(0x0F0F_0F0F) >> 4) & 0x0101_0101).wrapping_mul(15);
        u32::from((mask ^ value) & selected == 0)
    }
}

port! {
    /// Store a recognised interface and AddRef through the primary vtable. Unknown IDs leave the output alone.
    /// AL reports success; the other bytes retain the AddRef result or the ID arithmetic on failure.
    #[port(address = 0x0072_9EF0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_00729ef0(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        unsafe {
            let offset = match iid {
                1 | 0xE818_ED2B => 0,
                0x6819_05BD => 4,
                0x4522_94AA => 8,
                0xE4FD_A3D4 => 12,
                _ => {
                    let eax = if iid <= 0x6819_05BD { iid.wrapping_sub(0x4522_94AA) } else { iid };
                    return eax & !0xFF;
                }
            };
            write_unaligned(out, this.wrapping_add(if this.is_null() { 0 } else { offset }));
            let eax = vcall!(this, 4 => fn() -> u32);
            (eax & !0xFF) | 1
        }
    }
}

port! {
    /// Adapt the network object's ESI argument to the Rust body.
    #[port(address = 0x0072_9F90, owner = "zonedeveloper", name = "NetworkMatchesPlacement", naked)]
    pub unsafe extern "C" fn fc4p_00729f90() {
        core::arch::naked_asm!("mov ecx, esi", "jmp {body}", body = sym network_matches_placement);
    }
}

unsafe extern "thiscall" fn network_matches_placement(network: *mut u8) -> u32 {
    unsafe {
        let broad = *broad_network_match() != 0;
        let mut matched = vcall!(network, 0x58 => fn(u32) -> u32, 3) as u8 != 0;
        if !matched {
            matched = vcall!(network, 0x58 => fn(u32) -> u32, 11) as u8 != 0;
        }
        if broad && matched {
            return 1;
        }
        if !broad && !matched {
            return 0;
        }
        let mut other = vcall!(network, 0x58 => fn(u32) -> u32, 10) as u8 != 0;
        if !other {
            other = vcall!(network, 0x58 => fn(u32) -> u32, 0) as u8 != 0;
        }
        u32::from(if broad { other } else { !other })
    }
}

port! {
    /// Query the developer for any supported network at the cell, passing the complete return value through.
    #[port(address = 0x0072_A010, owner = "zonedeveloper", name = "CellHasNetwork")]
    pub unsafe extern "thiscall" fn fc4p_0072a010(this: *mut u8, x: i32, z: i32) -> u32 {
        unsafe { vcall!(this, 0x20 => fn(i32, i32, u32) -> u32, x, z, 0x1F4F) }
    }
}

port! {
    /// Forward directly to the retained terrain interface, keeping its argument and register conventions.
    #[port(address = 0x0072_A030, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::IsCellWater", naked)]
    pub unsafe extern "C" fn fc4p_0072a030() {
        core::arch::naked_asm!("mov ecx, [ecx + 0x34]", "mov eax, [ecx]", "jmp dword ptr [eax + 0x88]");
    }
}

port! {
    /// Read the cell's zone, then compare the selected zone or ask the manager whether it is an RCI zone.
    #[port(address = 0x0072_A040, owner = "zonedeveloper", name = "CellMatchesZone")]
    pub unsafe extern "thiscall" fn fc4p_0072a040(this: *mut u8, x: i32, z: i32) -> u32 {
        unsafe {
            let developer = ZoneDeveloper::from_ptr(this);
            let zone = vcall!(developer.zone_manager(), 0x30 => fn(i32, i32) -> u32, x, z);
            let selected = developer.current_zone();
            if selected == 0 {
                vcall!(developer.zone_manager(), 0x34 => fn(u32) -> u32, zone)
            } else {
                (zone & !0xFF) | u32::from(zone == selected)
            }
        }
    }
}

// Quiet comparison matches the coordinate correction without raising invalid for a quiet NaN.
fn equal_quiet(a: F80, b: F80) -> bool {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{b}]", "fld tbyte ptr [{a}]", "fucompp", "fnstsw ax",
            a = in(reg) &a, b = in(reg) &b, out("ax") status,
            out("st(0)") _, out("st(1)") _,
        );
    }
    status & 0x4400 == 0x4000
}

unsafe fn position_cell(position: f32) -> i32 {
    unsafe {
        let scaled = F80::from_f32(position) * F80::from_f32(*cell_scale());
        let rounded = ((scaled - F80::from_f32(*half())) + F80::from_f32(*rounding_bias())).to_f32();
        let cell = rounded.to_bits().wrapping_add(0xB4C0_0000) as i32;
        let next = cell.wrapping_add(1);
        if equal_quiet(scaled, F80::from_i32(next)) {
            next
        } else {
            cell
        }
    }
}

port! {
    /// Read an occupant position, convert X and Z to cells and ask the lot manager for that cell's lot.
    #[port(address = 0x0072_A110, owner = "zonedeveloper", name = "GetOccupantLot")]
    pub unsafe extern "thiscall" fn fc4p_0072a110(this: *mut u8, occupant: *mut u8) -> u64 {
        unsafe {
            let mut position = [0.0f32; 3];
            vcall!(occupant, 0x20 => fn(*mut f32) -> u32, position.as_mut_ptr());
            let x = position_cell(position[0]);
            let z = position_cell(position[2]);
            super::virtual_registers(this, 0x18, read_unaligned(this.cast::<u32>()), z as u32,
                &[x as u32, z as u32, 0])
        }
    }
}

port! {
    /// Decrement a shared reference, or call Shutdown and the deleting destructor on the final reference.
    #[port(address = 0x0072_A270, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::Release")]
    pub unsafe extern "thiscall" fn fc4p_0072a270(this: *mut u8) -> u32 {
        unsafe {
            let developer = ZoneDeveloper::from_ptr(this);
            let references = developer.references();
            if references > 1 {
                let remaining = references.wrapping_sub(1);
                developer.set_references(remaining);
                remaining as u32
            } else {
                vcall!(this, 0x10 => fn() -> u32);
                vcall!(this, 0x28 => fn(u32) -> u32, 1);
                0
            }
        }
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_00458960::PORT,
    fc4p_006bf430::PORT,
    fc4p_00729eb0::PORT,
    fc4p_00729ec0::PORT,
    fc4p_00729ef0::PORT,
    fc4p_00729f90::PORT,
    fc4p_0072a010::PORT,
    fc4p_0072a030::PORT,
    fc4p_0072a040::PORT,
    fc4p_0072a110::PORT,
    fc4p_0072a270::PORT,
];

#[cfg(test)]
mod tests;
