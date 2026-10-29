// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Foundation lookup, removal, refresh and slope threshold.

use super::*;

game_fn! {
    fn remove_lot_foundation(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_3F30;
}
game_fn! { fn read_uint(holder: *mut u8, id: u32, output: *mut u32) -> u64 = "C" 0x005F_D480; }
game_fn! { fn random_uniform(random: *mut u8, count: u32) -> u64 = "thiscall" 0x0091_36B1; }
game_fn! { fn bottom_altitude(this: *mut u8, lot: *mut u8) -> St0 = "thiscall" 0x006C_7630; }
game_fn! { fn corner_altitudes(this: *mut u8, rect: *const i32, output: *mut f32, water: u32) -> u64 = "thiscall" 0x006C_4360; }
game_fn! { fn pick_altitude(this: *mut u8, lot: *mut u8, bottom: f32, highest: f32, corners: *const f32) -> St0 = "thiscall" 0x006C_5360; }
game_fn! { fn retaining_wall(lot: *mut u8, enabled: u32) -> u64 = "C" 0x005E_3070; }
game_fn! { fn surrounding_foundations(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_7720; }
game_fn! { fn surrounding_walls(lot: *mut u8) -> u64 = "C" 0x005E_3960; }
game_data! { static file_random: u8 = 0x00B4_9AF0; }
game_data! { static world_to_cell_scale: f32 = 0x00AB_1B04; }
game_data! { static foundation_half: f32 = 0x00A8_4D2C; }

layout! {
    /// Foundation configuration object's type and optional ID vector.
    struct FoundationObject {
        0x20 => kind / set_kind: u32,
        0x28 => ids / set_ids: *const u32,
        0x2C => ids_end / set_ids_end: *const u32,
    }
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
    #[port(address = 0x006C_3E70, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetLotFoundation", naked)]
    pub unsafe extern "thiscall" fn fc4p_006c3e70(this: *mut u8, lot: *mut u8) -> u64 {
        stack_entry!(lot_foundation, 4);
    }
}

// The lot argument becomes z; the local x word is restored into ECX on return.
unsafe extern "fastcall" fn lot_foundation(this: *mut u8, stack: *mut u32, _entry_edx: u32) -> u64 {
    unsafe {
        let lot = stack.add(1).read() as *mut u8;
        let x = stack.sub(1);
        let z = stack.add(1);
        vcall!(lot, 0x54 => fn(*mut u32, *mut u32) -> u32, x, z);
        let x = x.read();
        result(virtual_call(this, 0xC4, &[2, x, z.read()], table(this), x, false))
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
    #[port(address = 0x006C_3F30, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveLotFoundation(lot)", naked)]
    pub unsafe extern "thiscall" fn fc4p_006c3f30(this: *mut u8, lot: *mut u8) -> u64 {
        stack_entry!(remove_foundation_lot, 4);
    }
}

// The lot argument becomes z; the local x word is restored into ECX on return.
unsafe extern "fastcall" fn remove_foundation_lot(this: *mut u8, stack: *mut u32, _entry_edx: u32) -> u64 {
    unsafe {
        let lot = stack.add(1).read() as *mut u8;
        let x = stack.sub(1);
        let z = stack.add(1);
        vcall!(lot, 0x54 => fn(*mut u32, *mut u32) -> u32, x, z);
        let removed = vcall!(this, 0xD4 => fn(u32, u32) -> u32, x.read(), z.read());
        let flags = vcall!(lot, 0x30 => fn() -> u32) & !3;
        let mut call = virtual_call(lot, 0x34, &[flags], flags, table(lot), false);
        call.eax = (call.eax & !255) | (removed & 255);
        result(call)
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
    /// Uses the explicit type, then the exemplar property, then the developer's default. Width and depth
    /// are the occupant's stored bounds subtracted on x87, depth first. The bottom altitude is -1.
    #[port(address = 0x006C_4020, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddBuildingFoundation(type)")]
    pub unsafe extern "thiscall" fn fc4p_006c4020(this: *mut u8, building: *mut u8, kind: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
            let mut position = core::mem::MaybeUninit::<[u32; 3]>::uninit();
            vcall!(occupant, 0x20 => fn(*mut u32) -> u32, position.as_mut_ptr().cast());
            let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
            let mut min = core::mem::MaybeUninit::<[f32; 3]>::uninit();
            let mut max = core::mem::MaybeUninit::<[f32; 3]>::uninit();
            vcall!(occupant, 0x28 => fn(*mut f32, *mut f32) -> u32, min.as_mut_ptr().cast(), max.as_mut_ptr().cast());
            let facing = vcall!(building, 0x38 => fn() -> u32);
            let mut kind = kind;
            if kind == 0 {
                let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                read_uint(holder, 0x88FC_D877, &mut kind);
                if kind == 0 { kind = developer.building_foundation_type(); }
            }
            let min = min.assume_init();
            let max = max.assume_init();
            let depth = terrain::subtract_float(F80::from_f32(max[2]), min[2]);
            let bds = developer.building_development();
            let t = table(bds);
            let depth = depth.to_f32();
            let width = terrain::subtract_float(F80::from_f32(max[0]), min[0]).to_f32();
            result(virtual_call(bds, 0x5C, &[kind, 1, position.as_mut_ptr() as u32, width.to_bits(),
                depth.to_bits(), facing, (-1.0f32).to_bits()], kind, t, false))
        }
    }
}

port! {
    /// Uses the configuration's primary building foundation type, otherwise the exemplar property with
    /// the default as its initial value. A zero result skips creation.
    #[port(address = 0x006C_40E0, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddBuildingFoundation(configuration)")]
    pub unsafe extern "thiscall" fn fc4p_006c40e0(this: *mut u8, building: *mut u8, config: *mut u8) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut kind = developer.building_foundation_type();
            let primary = vcall!(config, 0x60 => fn() -> *mut u8);
            let specified = if primary.is_null() { 0 } else { FoundationObject::from_ptr(primary).kind() };
            let mut property_result = 0;
            if specified != 0 { kind = specified; }
            else {
                let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                property_result = read_uint(holder, 0x88FC_D877, &mut kind);
            }
            if kind == 0 { return property_result & !0xFFFF_FFFF; }
            result(virtual_call(this, 0xEC, &[building as u32, kind], kind, table(this), false))
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

port! {
    /// Inserts a hidden entry-EAX word before the original arguments; the Rust body handles all selection.
    #[port(address = 0x006C_5460, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddLotFoundation(configuration)", naked)]
    pub unsafe extern "C" fn fc4p_006c5460() {
        core::arch::naked_asm!("xchg eax, dword ptr [esp]", "push eax", "jmp {body}", body = sym add_lot_foundation)
    }
}

#[allow(clippy::too_many_arguments)]
unsafe extern "fastcall" fn add_lot_foundation(
    this: *mut u8,
    entry_edx: u32,
    entry_eax: u32,
    lot: *mut u8,
    config: *mut u8,
    use_altitude: u32,
    level: u32,
    walls: u32,
) -> u64 {
    unsafe {
        if lot.is_null() || config.is_null() {
            return (u64::from(entry_edx) << 32) | u64::from(entry_eax & !255);
        }
        let mut kind = LotDeveloper::from_ptr(this).lot_foundation_type();
        let mut call = virtual_call(config, 0x68, &[], table(config), entry_edx, false);
        let primary = call.eax as *mut u8;
        if !primary.is_null() {
            let object = FoundationObject::from_ptr(primary);
            let count = (object.ids_end() as u32).wrapping_sub(object.ids() as u32) as i32 >> 2;
            call.eax = count as u32;
            if count != 0 {
                let draw = random_uniform(file_random(), count as u32);
                call.eax = draw as u32;
                call.edx = (draw >> 32) as u32;
                kind = object.ids().wrapping_add(call.eax as usize).read_unaligned();
            } else {
                kind = object.kind();
            }
        }
        if kind == 0 {
            call.eax &= !255;
            return result(call);
        }
        result(virtual_call(
            this,
            0xDC,
            &[lot as u32, kind, use_altitude, level, walls],
            use_altitude,
            table(this),
            false,
        ))
    }
}

port! {
    /// Removes the foundation at a world position. For lot foundations, converts z then x with the game's
    /// integer helper and clears both foundation flags on the lot found at that cell.
    #[port(address = 0x006C_54E0, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveFoundation")]
    pub unsafe extern "thiscall" fn fc4p_006c54e0(this: *mut u8, kind: u32, x: u32, z: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut call = virtual_call(this, 0xC8, &[kind, x, z], table(this), x, false);
            let foundation = call.eax as *mut u8;
            let found = !foundation.is_null();
            if found {
                call = virtual_call(foundation, 0x0C, &[], call.eax, table(foundation), false);
                let occupant = call.eax as *mut u8;
                if !occupant.is_null() {
                    call = virtual_call(occupant, 4, &[], table(occupant), call.edx, false);
                }
                let manager = developer.occupant_manager();
                call = virtual_call(manager, 0x54, &[occupant as u32, 1, 0], call.eax, table(manager), false);
                if !occupant.is_null() {
                    call = virtual_call(occupant, 8, &[], table(occupant), call.edx, false);
                }
            }
            if kind == 2 {
                let manager = developer.lot_manager();
                let t = table(manager);
                let cz = terrain::multiply_float(F80::from_f32(f32::from_bits(z)), *world_to_cell_scale()).ftol();
                let cx = terrain::multiply_float(F80::from_f32(f32::from_bits(x)), *world_to_cell_scale()).ftol();
                let args = [cx as u32, cz as u32, 0];
                let target = (t as *const u8).add(0x18).cast::<usize>().read_unaligned();
                call = RawCall::new(Conv::Thiscall, target, manager as u32, &args);
                call.eax = cx as u32;
                call.edx = ((cx as u64) >> 32) as u32;
                call_raw(&mut call);
                let lot = call.eax as *mut u8;
                if !lot.is_null() {
                    let flags = vcall!(lot, 0x30 => fn() -> u32) & !3;
                    call = virtual_call(lot, 0x34, &[flags], flags, table(lot), false);
                }
            }
            call.eax = (call.eax & !255) | u32::from(found);
            result(call)
        }
    }
}

port! {
    /// Creates a lot foundation at the lot's altitude or a selected terrain altitude. Terrain levelling
    /// precedes creation; altitude and flags are updated even when creation fails. Optional wall refresh
    /// uses the post-creation altitude and the highest vertex sampled before any levelling.
    #[port(address = 0x006C_8AB0, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddLotFoundation(type)")]
    pub unsafe extern "fastcall" fn fc4p_006c8ab0(this: *mut u8, entry_edx: u32, lot: *mut u8,
        kind: u32, use_lot_altitude: u32, level: u32, walls: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut center = [0.0f32; 3];
            let mut rect = [0i32; 4];
            virtual_call(lot, 0x3C, &[center.as_mut_ptr() as u32], table(lot), entry_edx, false);
            let mut call = virtual_call(lot, 0x38, &[rect.as_mut_ptr() as u32], rect.as_ptr() as u32, table(lot), false);
            call = virtual_call(lot, 0x5C, &[], call.eax, table(lot), false);
            let facing = call.eax;
            let width = F80::from_i32(rect[2].wrapping_sub(rect[0]).wrapping_add(1).wrapping_shl(4)).to_f32();
            let depth = F80::from_i32(rect[3].wrapping_sub(rect[1]).wrapping_add(1).wrapping_shl(4)).to_f32();
            let bottom = bottom_altitude(this, lot).to_f32();
            call = virtual_call(this, 0x44, &[rect.as_ptr() as u32, 0, 1], rect.as_ptr() as u32, table(this), true);
            let highest = call.st0.to_f32();
            call.eax = (call.eax & !255) | (use_lot_altitude & 255);
            let top = if use_lot_altitude as u8 != 0 {
                virtual_call(lot, 0x40, &[], call.eax, table(lot), true).st0
            } else {
                let mut corners = [0.0f32; 4];
                corner_altitudes(this, rect.as_ptr(), corners.as_mut_ptr(), 1);
                pick_altitude(this, lot, bottom, highest, corners.as_ptr())
            };
            center[1] = top.to_f32();
            if level as u8 != 0 {
                if rect[2].wrapping_sub(rect[0]) > 1 && rect[3].wrapping_sub(rect[1]) > 1 {
                    let target = terrain::subtract_float(F80::from_f32(center[1]), *foundation_half()).to_f32();
                    vcall!(this, 0x4C => fn(*const i32, f32, u32, u32, *mut u8) -> u32,
                        rect.as_ptr(), target, 1, 1, lot);
                }
                vcall!(this, 0x50 => fn(*const i32, f32, u32, *mut u8) -> u32,
                    rect.as_ptr(), center[1], 1, lot);
            }
            let manager = developer.building_development();
            let created = virtual_call(manager, 0x5C,
                &[kind, 2, center.as_mut_ptr() as u32, width.to_bits(), depth.to_bits(), facing, bottom.to_bits()],
                kind, table(manager), false).eax as u8;
            call = virtual_call(lot, 0x44, &[center[1].to_bits()], center[1].to_bits(), table(lot), false);
            call = virtual_call(lot, 0x30, &[], call.eax, table(lot), false);
            let flags = (call.eax & !4) | 3;
            call = virtual_call(lot, 0x34, &[flags], flags, table(lot), false);
            call.eax = (call.eax & !255) | (walls & 255);
            if walls as u8 != 0 {
                let compared = F80::from_f32(center[1]).compare(F80::from_f32(highest));
                call.eax = (call.eax & 0xFFFF_0000) | u32::from(compared.status_word());
                if compared.less() {
                    retaining_wall(lot, 1);
                    surrounding_foundations(this, lot);
                    let returned = surrounding_walls(lot);
                    call.eax = returned as u32;
                    call.edx = (returned >> 32) as u32;
                }
            }
            call.eax = (call.eax & !255) | u32::from(created);
            result(call)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c3e10::PORT,
    fc4p_006c3e70::PORT,
    fc4p_006c3eb0::PORT,
    fc4p_006c3f80::PORT,
    fc4p_006c3ff0::PORT,
    fc4p_006c4020::PORT,
    fc4p_006c40e0::PORT,
    fc4p_006c4150::PORT,
    fc4p_006c5460::PORT,
    fc4p_006c54e0::PORT,
    fc4p_006c8ab0::PORT,
];
