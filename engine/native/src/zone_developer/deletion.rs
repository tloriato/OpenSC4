// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Real lot removal and the highlight-mode record of virtually deleted lot cells.

use super::{network::virtual_call, Port};
use core::mem::MaybeUninit;
use core::ptr::{null_mut, read_unaligned, write_unaligned};

layout! {
    /// Services and cell rows relative to the lot-manager interface.
    struct DeletionManager {
        0x1C => city / set_city: *mut u8,
        0x2C => occupants / set_occupants: *mut u8,
        0x4C => configuration / set_configuration: *mut u8,
        0x50 => rows / set_rows: *mut u8,
        0x95 => real_mode / set_real_mode: u8,
    }
}

game_fn! { fn deleted_cell(this: *mut u8, cell: *const i32) -> *mut u8 = "thiscall" 0x0072_F7D0; }
game_fn! { fn remove_list_entry(this: *mut u8, lot: *const *mut u8) = "thiscall" 0x0072_B990; }

port! {
    /// Highlight mode records the lot's original anchor and facing at every cell of its bounds.
    /// Real mode removes configuration and building references, clears the cell map, removes its list
    /// entry and any occupant attachment, then shuts down and releases the lot. Bounds normalize each
    /// upper coordinate to its lower coordinate. Retain EDX and the full final EAX result.
    #[port(address = 0x0072_F8E0, owner = "zonedeveloper", name = "DeleteLot")]
    pub unsafe extern "thiscall" fn fc4p_0072f8e0(this: *mut u8, lot: *mut u8) -> u64 {
        unsafe {
            let manager = DeletionManager::from_ptr(this);
            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
            vcall!(lot, 0x38 => fn(*mut i32), bounds.as_mut_ptr().cast());
            let [x1, z1, x2, z2] = bounds.assume_init();
            let (x2, z2) = (x2.max(x1), z2.max(z1));
            if manager.real_mode() == 0 {
                let facing = vcall!(lot, 0x5C => fn() -> u32);
                let mut z = z1;
                loop {
                    let mut x = x1;
                    loop {
                        let cell = [x, z];
                        let record = deleted_cell(this.add(0xE0), cell.as_ptr());
                        write_unaligned(record.cast::<i32>(), x1);
                        write_unaligned(record.add(4).cast::<i32>(), z1);
                        write_unaligned(record.add(8).cast::<u32>(), facing);
                        x = x.wrapping_add(1);
                        if x > x2 { break; }
                    }
                    z = z.wrapping_add(1);
                    if z > z2 { break; }
                }
                return (u64::from(z1 as u32) << 32) | u64::from(z2 as u32);
            }
            vcall!(manager.configuration(), 0x2C => fn(*mut u8), lot);
            let occupants = vcall!(manager.city(), 0x90 => fn() -> *mut u8);
            let mut building = null_mut();
            let result = vcall!(occupants, 0xA4 => fn(*mut u8, *mut *mut u8, u32) -> u32, lot, &mut building, 0);
            if result as u8 != 0 {
                // The destination method is selected before the building call, while its this is reloaded after.
                let destination = manager.occupants();
                let function = crate::abi::vslot(destination, 0x54);
                let occupant = vcall!(building, 0x0C => fn(u32, u32) -> *mut u8, 1, 0);
                let function: unsafe extern "thiscall" fn(*mut u8, *mut u8) = core::mem::transmute(function);
                function(manager.occupants(), occupant);
                vcall!(building, 8 => fn());
            }
            let mut z = z1;
            loop {
                let mut x = x1;
                loop {
                    let row = read_unaligned(manager.rows().wrapping_offset(x.wrapping_mul(12) as isize).cast::<*mut u8>());
                    write_unaligned(row.wrapping_offset(z.wrapping_mul(4) as isize).cast::<u32>(), 0);
                    x = x.wrapping_add(1);
                    if x > x2 { break; }
                }
                z = z.wrapping_add(1);
                if z > z2 { break; }
            }
            remove_list_entry(this.add(0x5C), &lot);
            if vcall!(lot, 0x64 => fn() -> u32) != 0 {
                let occupants = vcall!(manager.city(), 0x90 => fn() -> *mut u8);
                vcall!(occupants, 0x84 => fn(*mut u8), lot);
            }
            let result = vcall!(lot, 0x1C => fn() -> u32);
            let call = virtual_call(lot, 8, result, read_unaligned(lot.cast::<u32>()), &[]);
            (u64::from(call.edx) << 32) | u64::from(call.eax)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072f8e0::PORT];

#[cfg(test)]
mod tests;
