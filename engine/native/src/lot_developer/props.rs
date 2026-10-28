// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Prop creation from explicit instances or randomly selected configuration families.

use super::*;

game_data! { static position_initial: u32 = 0x00B4_9AEC; }
game_data! { static prop_resources: *mut u8 = 0x00B4_3D18; }
game_data! { static random_state: u8 = 0x00B4_9AF0; }
game_fn! { fn random_index(this: *mut u8, count: u32) -> u64 = "thiscall" 0x0091_36B1; }
game_fn! { fn free_instances(storage: *mut u8) -> u64 = "C" 0x0090_CF63; }
// EDX is an additional register input; the two ordinary arguments still occupy eight callee-popped bytes.
game_fn! { fn create_family_prop(this: *mut u8, edx: u32, lot: *mut u8, index: i32) -> u64 = "fastcall" 0x006C_6D70; }

layout! {
    /// Fields used from one 52-byte configuration object.
    pub struct ConfigurationObject {
        0x00 => kind / set_kind: u8,
        0x01 => flags / set_flags: u8,
        0x24 => appearance / set_appearance: u32,
    }
}

port! {
    /// Positions the indexed configuration object, creates the requested nonzero instance and applies
    /// its appearance word. The position starts with three copies of the game's initial-position word;
    /// facing starts with the lot pointer's bits and is passed to the positioning callback by reference.
    #[port(address = 0x006C_6230, owner = "lotdeveloper", name = "cSC4LotDeveloper::CreateLotProp")]
    pub unsafe extern "fastcall" fn fc4p_006c6230(this: *mut u8, entry_edx: u32, lot: *mut u8, index: i32, instance: u32) -> u64 {
        unsafe {
            if lot.is_null() { return u64::from(entry_edx) << 32; }
            let mut call = virtual_call(lot, 0x24, &[], table(lot), entry_edx, false);
            let config = call.eax as *mut u8;
            if config.is_null() { return result(call); }
            call = virtual_call(config, 0x5C, &[], call.eax, table(config), false);
            let begin = (call.eax as *const *mut u8).read_unaligned();
            let record = begin.wrapping_add(index.wrapping_mul(52) as usize);
            let initial = position_initial().read();
            let mut position = [initial; 3];
            let mut facing = lot as u32;
            call = virtual_call(this, 0x9C,
                &[lot as u32, config as u32, record as u32, position.as_mut_ptr() as u32, &mut facing as *mut u32 as u32],
                table(this), position.as_mut_ptr() as u32, false);
            if instance != 0 {
                let manager = LotDeveloper::from_ptr(this).prop_manager();
                call = virtual_call(manager, 0x68, &[instance, position.as_mut_ptr() as u32, facing], instance, table(manager), false);
                let prop = call.eax as *mut u8;
                if !prop.is_null() {
                    let appearance = ConfigurationObject::from_ptr(record).appearance();
                    call = virtual_call(prop, 0x70, &[appearance], appearance, table(prop), false);
                    call.eax = prop as u32;
                    return result(call);
                }
            }
            call.eax = 0;
            result(call)
        }
    }
}

port! {
    /// Uses the prop resource service to enumerate instances for the indexed object, chooses one through
    /// the game's random generator, positions and creates it, and applies the object's appearance.
    /// The temporary instance array is freed on both success and failure without changing its elements.
    #[port(address = 0x006C_6D70, owner = "lotdeveloper", name = "CreateFamilyLotProp")]
    pub unsafe extern "fastcall" fn fc4p_006c6d70(this: *mut u8, entry_edx: u32, lot: *mut u8, index: i32) -> u64 {
        unsafe {
            if lot.is_null() { return u64::from(entry_edx) << 32; }
            let mut call = virtual_call(lot, 0x24, &[], table(lot), entry_edx, false);
            let config = call.eax as *mut u8;
            if config.is_null() { return result(call); }
            call = virtual_call(config, 0x5C, &[], call.eax, table(config), false);
            let begin = (call.eax as *const *mut u8).read_unaligned();
            let record = begin.wrapping_add(index.wrapping_mul(52) as usize);
            let initial = position_initial().read();
            let mut position = [initial; 3];
            let mut facing = lot as u32;
            virtual_call(this, 0x9C,
                &[lot as u32, config as u32, record as u32, position.as_mut_ptr() as u32, &mut facing as *mut u32 as u32],
                table(this), position.as_mut_ptr() as u32, false);
            let mut instances = [0u32; 3];
            let resources = prop_resources().read();
            call = virtual_call(resources, 0x6C, &[record as u32, instances.as_mut_ptr() as u32],
                table(resources), instances.as_mut_ptr() as u32, false);
            let mut prop = core::ptr::null_mut();
            if instances[0] != instances[1] {
                let count = (instances[1].wrapping_sub(instances[0]) as i32 >> 2) as u32;
                let returned = random_index(random_state(), count);
                call.edx = (returned >> 32) as u32;
                let instance = (instances[0] as *const u32).wrapping_add(returned as u32 as usize).read_unaligned();
                if instance != 0 {
                    let manager = LotDeveloper::from_ptr(this).prop_manager();
                    call = virtual_call(manager, 0x68, &[instance, position.as_mut_ptr() as u32, facing], instance, table(manager), false);
                    prop = call.eax as *mut u8;
                    if !prop.is_null() {
                        let appearance = ConfigurationObject::from_ptr(record).appearance();
                        call = virtual_call(prop, 0x70, &[appearance], appearance, table(prop), false);
                    }
                }
            }
            if instances[0] != 0 { call.edx = (free_instances(instances[0] as *mut u8) >> 32) as u32; }
            call.eax = prop as u32;
            result(call)
        }
    }
}

port! {
    /// Walks the configuration's object array with its initial count, creating family props for kind one
    /// when flag bit zero is clear. The array's begin pointer is reread after every creation callback.
    /// The third stack argument is accepted and ignored.
    #[port(address = 0x006C_7860, owner = "lotdeveloper", name = "CreateUnflaggedFamilyProps")]
    pub unsafe extern "fastcall" fn fc4p_006c7860(this: *mut u8, entry_edx: u32, lot: *mut u8, config: *mut u8, _flags: u32) -> u64 {
        unsafe {
            let mut call = virtual_call(config, 0x5C, &[], table(config), entry_edx, false);
            let vector = call.eax as *const u32;
            if vector.is_null() { return result(call); }
            let difference = vector.add(1).read_unaligned().wrapping_sub(vector.read_unaligned()) as i32;
            let product = i64::from(difference) * 0x4EC4_EC4F;
            let high = (product >> 32) as i32 >> 4;
            let count = high.wrapping_add((high as u32 >> 31) as i32);
            call.eax = product as u32;
            call.edx = high as u32;
            let mut index = 0i32;
            let mut offset = 0u32;
            while index < count {
                let record = (vector.read_unaligned() as *mut u8).wrapping_add(offset as usize);
                call.eax = record as u32;
                let object = ConfigurationObject::from_ptr(record);
                if object.kind() == 1 && object.flags() & 1 == 0 {
                    let returned = create_family_prop(this, call.edx, lot, index);
                    call.eax = returned as u32;
                    call.edx = (returned >> 32) as u32;
                }
                index = index.wrapping_add(1);
                offset = offset.wrapping_add(52);
            }
            result(call)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c6230::PORT, fc4p_006c6d70::PORT, fc4p_006c7860::PORT];
