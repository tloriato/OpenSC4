// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Building removal, placement, habitation and prop maintenance.

use super::*;

game_fn! { fn refresh_retaining_walls(lot: *mut u8) -> u64 = "C" 0x005E_37F0; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0090_DDF1; }
game_fn! { fn allocate_texture(size: u32) -> *mut u8 = "C" 0x005D_ECF0; }
game_fn! { fn construct_texture(this: *mut u8) -> *mut u8 = "thiscall" 0x005E_05B0; }

port! {
    /// Finds the base texture at the lot origin and queries its texture interface. On request, creates a
    /// missing texture through the framework, falling back to the game's allocator and constructor.
    /// Query failure retains the found occupant's reference. Success is the final output pointer's presence.
    #[port(address = 0x006C_41A0, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetLotBaseTextureOccupant", naked)]
    pub unsafe extern "thiscall" fn fc4p_006c41a0(this: *mut u8, lot: *mut u8, output: *mut *mut u8, create: u32) -> u64 {
        stack_entry!(base_texture, 12);
    }
}

// Coordinates reuse the lot argument; the occupant output then reuses the output argument.
unsafe extern "fastcall" fn base_texture(this: *mut u8, stack: *mut u32, _entry_edx: u32) -> u64 {
    unsafe {
        let lot = stack.add(1).read() as *mut u8;
        let output = stack.add(2).read() as *mut *mut u8;
        let create = stack.add(3).read();
        output.write_unaligned(core::ptr::null_mut());
        let developer = LotDeveloper::from_ptr(this);
        let x = stack.sub(1);
        let z = stack.add(1);
        vcall!(lot, 0x54 => fn(*mut u32, *mut u32) -> u32, x, z);
        let manager = developer.occupant_manager();
        let occupant = stack.add(2).cast::<*mut u8>();
        let mut call = virtual_call(manager, 0x84,
            &[occupant as u32, x.read(), z.read(), 0x497F_6D9D],
            table(manager), occupant as u32, false);
        if call.eax as u8 != 0 {
            let occupant = occupant.read();
            call = virtual_call(occupant, 0, &[0x097F_6D98, output as u32], table(occupant), call.edx, false);
            if call.eax as u8 != 0 {
                let texture = output.read_unaligned();
                call = virtual_call(texture, 0x14, &[2], call.eax, table(texture), false);
                call = virtual_call(occupant, 8, &[], table(occupant), call.edx, false);
            }
        } else if create as u8 != 0 {
            let services = framework();
            call = virtual_call(services, 0x0C, &[0xC97F_987C, 0x097F_6D98, output as u32],
                services as u32, table(services), false);
            if call.eax as u8 == 0 {
                let allocation = allocate_texture(0x60);
                let texture = if allocation.is_null() { allocation } else { construct_texture(allocation) };
                output.write_unaligned(texture);
                call = virtual_call(texture, 4, &[], texture as u32, table(texture), false);
            }
            let texture = output.read_unaligned();
            call = virtual_call(texture, 0x0C, &[], table(texture), call.edx, false);
            let occupant = call.eax as *mut u8;
            call = virtual_call(occupant, 0x0C, &[], occupant as u32, table(occupant), false);
            let texture = output.read_unaligned();
            call = virtual_call(texture, 0x14, &[2], table(texture), call.edx, false);
        }
        call.eax = u32::from(!output.read_unaligned().is_null());
        result(call)
    }
}

port! {
    /// Changes the lot state, repeatedly removes its first building, then removes its props.
    #[port(address = 0x006C_3C50, owner = "lotdeveloper", name = "cSC4LotDeveloper::ClearLotBuildings")]
    pub unsafe extern "thiscall" fn fc4p_006c3c50(
        this: *mut u8, lot: *mut u8, state: u32, remove_foundation: u32, flags: u32,
    ) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut building = vcall!(this, 0x98 => fn(*mut u8) -> *mut u8, lot);
            vcall!(lot, 0x88 => fn(u32, u32, u32) -> u32, state, 0, 0);
            while !building.is_null() {
                vcall!(building, 4 => fn() -> u32);
                let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
                vcall!(developer.prop_manager(), 0x98 => fn(*mut u8) -> u32, occupant);
                if remove_foundation as u8 != 0 {
                    vcall!(this, 0xF0 => fn(*mut u8) -> u32, occupant);
                }
                vcall!(developer.occupant_manager(), 0x54 => fn(*mut u8, u32, u32) -> u32, occupant, 1, flags);
                vcall!(building, 8 => fn() -> u32);
                building = vcall!(this, 0x98 => fn(*mut u8) -> *mut u8, lot);
            }
            result(virtual_call(this, 0x2C, &[lot as u32], 0, table(this), false))
        }
    }
}

port! {
    /// Delegates placement to the lot's configuration and its primary building object, when both exist.
    #[port(address = 0x006C_3CF0, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetPositionForBuilding")]
    pub unsafe extern "fastcall" fn fc4p_006c3cf0(
        this: *mut u8, entry_edx: u32, lot: *mut u8, position: *mut u8, facing: *mut u8,
    ) -> u64 {
        unsafe {
            let mut call = virtual_call(lot, 0x24, &[], table(lot), entry_edx, false);
            let config = call.eax as *mut u8;
            if config.is_null() { return result(call); }
            call = virtual_call(config, 0x60, &[], call.eax, table(config), false);
            if call.eax == 0 { return result(call); }
            result(virtual_call(this, 0x9C,
                &[lot as u32, config as u32, call.eax, position as u32, facing as u32],
                call.eax, table(this), false))
        }
    }
}

port! {
    /// Sets habitation to abandoned, passing the full vacate reason word.
    #[port(address = 0x006C_3D30, owner = "lotdeveloper", name = "cSC4LotDeveloper::AbandonBuilding")]
    pub unsafe extern "thiscall" fn fc4p_006c3d30(_this: *mut u8, lot: *mut u8, reason: u32) -> u64 {
        unsafe { result(virtual_call(lot, 0x88, &[4, 0, reason], table(lot), reason, false)) }
    }
}

port! {
    /// Recomputes condition from wealth loss and the no-power property, occupies the lot and transfers
    /// the requested developer type's population. Service pointers are read again after each callback.
    #[port(address = 0x006C_3D50, owner = "lotdeveloper", name = "cSC4LotDeveloper::ReoccupyBuilding")]
    pub unsafe extern "thiscall" fn fc4p_006c3d50(this: *mut u8, lot: *mut u8, developer_type: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let building_type = vcall!(lot, 0xB8 => fn(u32) -> u32, 0);
            let profile = vcall!(developer.building_development(), 0x9C => fn(u32) -> *mut u8, building_type);
            let wealth = vcall!(developer.building_development(), 0x7C => fn(u32) -> i32, developer_type);
            let loss = profile.add(4).cast::<i32>().read_unaligned().wrapping_sub(wealth);
            let holder = vcall!(lot, 0x0C => fn() -> *mut u8);
            let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, 0xE999_B11F);
            let mut condition = 0;
            if !property.is_null() {
                let variant = vcall!(property, 0x14 => fn() -> *mut u8);
                let data = vcall!(variant, 0x124 => fn() -> *const u32);
                condition = u32::from(data.read_unaligned() != 0);
            }
            if loss == 1 { condition = 2; }
            else if loss == 2 { condition = 3; }
            vcall!(lot, 0x88 => fn(u32, i32, u32) -> u32, 3, wealth, 0);
            vcall!(lot, 0x90 => fn(u32) -> u32, condition);
            let population = vcall!(lot, 0xD0 => fn(u32, u32) -> u32, developer_type, 0);
            result(virtual_call(lot, 0xDC, &[developer_type, population], population, table(lot), false))
        }
    }
}

port! {
    /// Refreshes the retaining walls through the shared lot helper.
    #[port(address = 0x006C_4190, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateLotRetainingWalls")]
    pub unsafe extern "thiscall" fn fc4p_006c4190(_this: *mut u8, lot: *mut u8) -> u64 {
        unsafe { refresh_retaining_walls(lot) }
    }
}

port! {
    /// Finds an existing base texture occupant, updates it, then releases the interface.
    #[port(address = 0x006C_4290, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateLotBaseTextureOccupant")]
    pub unsafe extern "thiscall" fn fc4p_006c4290(this: *mut u8, lot: *mut u8, foundation: u32) -> u64 {
        unsafe {
            let mut texture = core::ptr::null_mut::<u8>();
            let mut call = virtual_call(this, 0xA4,
                &[lot as u32, &mut texture as *mut *mut u8 as u32, 0], table(this), lot as u32, false);
            if call.eax as u8 != 0 {
                call = virtual_call(texture, 0x24, &[1, foundation, 0], table(texture), foundation, false);
                call = virtual_call(texture, 8, &[], table(texture), call.edx, false);
            }
            result(call)
        }
    }
}

port! {
    /// Removes all props and clears the existing zot property without creating a missing one.
    #[port(address = 0x006C_42D0, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveLotProps")]
    pub unsafe extern "thiscall" fn fc4p_006c42d0(this: *mut u8, lot: *mut u8) -> u32 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            vcall!(developer.prop_manager(), 0x70 => fn(*mut u8) -> u32, lot);
            let holder = vcall!(lot, 0x0C => fn() -> *mut u8);
            let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, 0xC999_C45E);
            if property.is_null() { return 0; }
            let variant = vcall!(property, 0x14 => fn() -> *mut u8);
            vcall!(variant, 0x68 => fn(u32) -> u32, 0)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c3c50::PORT,
    fc4p_006c3cf0::PORT,
    fc4p_006c3d30::PORT,
    fc4p_006c3d50::PORT,
    fc4p_006c4190::PORT,
    fc4p_006c41a0::PORT,
    fc4p_006c4290::PORT,
    fc4p_006c42d0::PORT,
];
