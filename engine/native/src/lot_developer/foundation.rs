// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Foundation lookup, removal, refresh and slope threshold.

use super::*;

game_fn! {
    fn remove_lot_foundation(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_3F30;
}

port! {
    /// Selects building, prop or other foundation kind from the occupant type and looks up its position.
    #[port(address = 0x006C_3E10, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetFoundationByOccupant")]
    pub unsafe extern "thiscall" fn fc4p_006c3e10(this: *mut u8, occupant: *mut u8) -> u64 {
        unsafe {
            let mut position = core::mem::MaybeUninit::<[u32; 3]>::uninit();
            vcall!(occupant, 0x20 => fn(*mut u32) -> u32, position.as_mut_ptr().cast());
            let occupant_type = vcall!(occupant, 0x1C => fn() -> u32);
            let kind = match occupant_type { 0x2781_28A0 => 1, 0xA823_821E => 3, _ => 4 };
            let p = position.assume_init();
            result(virtual_call(this, 0xC8, &[kind, p[0], p[2]], kind, table(this), false))
        }
    }
}

port! {
    /// Looks up the lot foundation by its cell location.
    #[port(address = 0x006C_3E70, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetLotFoundation")]
    pub unsafe extern "thiscall" fn fc4p_006c3e70(this: *mut u8, lot: *mut u8) -> u64 {
        unsafe {
            let (mut x, mut z) = (core::mem::MaybeUninit::<u32>::uninit(), core::mem::MaybeUninit::<u32>::uninit());
            vcall!(lot, 0x54 => fn(*mut u32, *mut u32) -> u32, x.as_mut_ptr(), z.as_mut_ptr());
            let x = x.assume_init();
            result(virtual_call(this, 0xC4, &[2, x, z.assume_init()], table(this), x, false))
        }
    }
}

port! {
    /// Removes the foundation occupant, then clears both foundation flag bits on the lot found at the cell.
    /// AL reports whether an occupant existed; the other return bytes come from the last virtual call.
    #[port(address = 0x006C_3EB0, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveLotFoundation(cell)")]
    pub unsafe extern "thiscall" fn fc4p_006c3eb0(this: *mut u8, x: u32, z: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let foundation = vcall!(this, 0xC4 => fn(u32, u32, u32) -> *mut u8, 2, x, z);
            let found = !foundation.is_null();
            if found {
                let occupant = vcall!(foundation, 0x0C => fn() -> *mut u8);
                vcall!(occupant, 4 => fn() -> u32);
                vcall!(developer.occupant_manager(), 0x54 => fn(*mut u8, u32, u32) -> u32, occupant, 1, 0);
                vcall!(occupant, 8 => fn() -> u32);
            }
            let manager = developer.lot_manager();
            let mut call = virtual_call(manager, 0x18, &[x, z, 0], x, table(manager), false);
            let lot = call.eax as *mut u8;
            if !lot.is_null() {
                let flags = vcall!(lot, 0x30 => fn() -> u32) & !3;
                call = virtual_call(lot, 0x34, &[flags], flags, table(lot), false);
            }
            call.eax = (call.eax & !255) | u32::from(found);
            result(call)
        }
    }
}

port! {
    /// Removes the cell's foundation, then clears the supplied lot's flags again.
    #[port(address = 0x006C_3F30, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveLotFoundation(lot)")]
    pub unsafe extern "thiscall" fn fc4p_006c3f30(this: *mut u8, lot: *mut u8) -> u64 {
        unsafe {
            let (mut x, mut z) = (core::mem::MaybeUninit::<u32>::uninit(), core::mem::MaybeUninit::<u32>::uninit());
            vcall!(lot, 0x54 => fn(*mut u32, *mut u32) -> u32, x.as_mut_ptr(), z.as_mut_ptr());
            let removed = vcall!(this, 0xD4 => fn(u32, u32) -> u32, x.assume_init(), z.assume_init());
            let flags = vcall!(lot, 0x30 => fn() -> u32) & !3;
            let mut call = virtual_call(lot, 0x34, &[flags], flags, table(lot), false);
            call.eax = (call.eax & !255) | (removed & 255);
            result(call)
        }
    }
}

port! {
    /// Rebuilds a flagged lot's existing foundation with its current type, or adds one from configuration.
    #[port(address = 0x006C_3F80, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateLotFoundation")]
    pub unsafe extern "fastcall" fn fc4p_006c3f80(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        unsafe {
            let call = virtual_call(lot, 0x30, &[], table(lot), entry_edx, false);
            if call.eax & 3 == 0 { return result(call); }
            let foundation = vcall!(this, 0xCC => fn(*mut u8) -> *mut u8, lot);
            if !foundation.is_null() {
                let kind = vcall!(foundation, 0x10 => fn() -> u32);
                let removed = remove_lot_foundation(this, lot);
                result(virtual_call(this, 0xDC, &[lot as u32, kind, 1, 0, 0], table(this), (removed >> 32) as u32, false))
            } else {
                let lot_table = table(lot);
                let developer_table = table(this);
                let call = virtual_call(lot, 0x24, &[], 0, lot_table, false);
                let args = [lot as u32, call.eax, 1, 0, 0];
                let target = (developer_table as *const u8).add(0xD8).cast::<usize>().read_unaligned();
                let mut last = RawCall::new(Conv::Thiscall, target, this as u32, &args);
                last.eax = call.eax;
                last.edx = call.edx;
                call_raw(&mut last);
                result(last)
            }
        }
    }
}

port! {
    /// Requires a foundation only when the occupant slope is strictly above the stored threshold.
    #[port(address = 0x006C_3FF0, owner = "lotdeveloper", name = "cSC4LotDeveloper::IsLotObjectFoundationRequired")]
    pub unsafe extern "fastcall" fn fc4p_006c3ff0(this: *mut u8, entry_edx: u32, occupant: *mut u8) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut call = virtual_call(this, 0x7C, &[occupant as u32, 1], table(this), entry_edx, true);
            let cmp = call.st0.compare(F80::from_f32(developer.foundation_slope()));
            call.eax = u32::from(!cmp.less() && !cmp.equal() && !cmp.unordered());
            result(call)
        }
    }
}

port! {
    /// Removes the building foundation at the occupant's x and z position.
    #[port(address = 0x006C_4150, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveBuildingFoundation")]
    pub unsafe extern "thiscall" fn fc4p_006c4150(this: *mut u8, occupant: *mut u8) -> u64 {
        unsafe {
            let mut position = core::mem::MaybeUninit::<[u32; 3]>::uninit();
            vcall!(occupant, 0x20 => fn(*mut u32) -> u32, position.as_mut_ptr().cast());
            let p = position.assume_init();
            result(virtual_call(this, 0xD0, &[1, p[0], p[2]], table(this), p[0], false))
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c3e10::PORT,
    fc4p_006c3e70::PORT,
    fc4p_006c3eb0::PORT,
    fc4p_006c3f30::PORT,
    fc4p_006c3f80::PORT,
    fc4p_006c3ff0::PORT,
    fc4p_006c4150::PORT,
];
