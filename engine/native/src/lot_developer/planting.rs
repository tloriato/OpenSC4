// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Default building selection and flora placement from lot configuration records.

use super::*;

game_fn! { fn make_set(this: *mut u32, capacity: u32) = "thiscall" 0x0082_9BC0; }
game_fn! { fn insert_set(this: *mut u32, output: *mut u32, value: *const u32) = "thiscall" 0x0045_D650; }
game_fn! { fn begin_set(this: *mut u32, output: *mut u32) = "thiscall" 0x004B_0390; }
game_fn! { fn next_bucket(this: *mut u32) -> u32 = "thiscall" 0x0053_DCF0; }
game_fn! { fn destroy_set(this: *mut u32) = "thiscall" 0x008E_BA60; }
game_fn! { fn free_storage(storage: u32) = "C" 0x0090_CF63; }
game_fn! { fn random_index(this: *mut u8, count: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { fn pick_flora(this: *mut u8, ids: *mut u32, position: *mut u32) -> u32 = "thiscall" 0x006C_D010; }
game_data! { static random_state: u8 = 0x00B4_9AF0; }
game_data! { static position_initial: u32 = 0x00B4_9AEC; }

layout! {
    /// A configuration's default building list and object list are three-word vectors.
    struct BuildingChoices {
        0x28 => begin / set_begin: *mut u32,
        0x2C => end / set_end: *mut u32,
    }
}

unsafe fn dispose_set(set: &mut [u32; 5]) {
    unsafe {
        destroy_set(set.as_mut_ptr());
        if set[1] != 0 {
            free_storage(set[1]);
        }
    }
}

port! {
    /// Exposes the building-type argument word, which receives the selected default before construction.
    #[port(address = 0x006C_CEF0, owner = "lotdeveloper", name = "cSC4LotDeveloper::ConstructDefaultLot", naked)]
    pub unsafe extern "C" fn fc4p_006ccef0() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 16", sym construct_default);
    }
}

unsafe extern "fastcall" fn construct_default(this: *mut u8, _entry_edx: u32, args: *mut u32) -> u64 {
    unsafe {
        let config = args.add(1).read_unaligned() as *mut u8;
        if !config.is_null() && args.add(2).read_unaligned() == 0 {
            let choices = vcall!(config, 0x60 => fn() -> *mut u8);
            if !choices.is_null() {
                let choices = BuildingChoices::from_ptr(choices);
                if choices.begin() != choices.end() {
                    let mut set = [0u32; 5];
                    make_set(set.as_mut_ptr(), 100);
                    let count = (choices.end() as u32).wrapping_sub(choices.begin() as u32) as i32 >> 2;
                    let mut index = 0i32;
                    while index < count {
                        let value = choices.begin().wrapping_add(index as usize).read_unaligned();
                        let manager = LotDeveloper::from_ptr(this).building_development();
                        if vcall!(manager, 0x3C => fn(u32) -> u32, value) as u8 != 0 {
                            let mut inserted = [0u32; 3];
                            insert_set(set.as_mut_ptr(), inserted.as_mut_ptr(), &value);
                        } else {
                            let manager = LotDeveloper::from_ptr(this).building_development();
                            vcall!(manager, 0x4C => fn(*mut u32, u32), set.as_mut_ptr(), value);
                        }
                        index = index.wrapping_add(1);
                    }
                    if set[4] != 0 {
                        let mut iterator = [0u32; 2];
                        begin_set(set.as_mut_ptr(), iterator.as_mut_ptr());
                        let selected = (iterator[0] as *const u32).add(1).read_unaligned();
                        args.add(2).write_unaligned(selected);
                    }
                    dispose_set(&mut set);
                }
            }
        }
        result(virtual_call(
            this,
            0x18,
            &[
                args.read_unaligned(),
                config as u32,
                args.add(2).read_unaligned(),
                args.add(3).read_unaligned(),
            ],
            table(this),
            args.read_unaligned(),
            false,
        ))
    }
}

port! {
    /// Chooses an explicit flora type or a uniformly selected entry in its family set. Position is unused.
    #[port(address = 0x006C_D010, owner = "lotdeveloper", name = "cSC4LotDeveloper::PickFloraTypeForLot")]
    pub unsafe extern "thiscall" fn fc4p_006cd010(this: *mut u8, ids: *mut u32, _position: *mut u32) -> u32 {
        unsafe {
            let begin = ids.read_unaligned();
            let end = ids.add(1).read_unaligned();
            if begin == end { return 0; }
            let count = (end.wrapping_sub(begin) as i32 >> 2) as u32;
            let index = random_index(random_state(), count);
            let selected = (ids.read_unaligned() as *const u32).wrapping_add(index as usize).read_unaligned();
            let manager = updates::UpdateState::from_ptr(this).flora();
            if vcall!(manager, 0x2C => fn(u32) -> u32, selected) as u8 != 0 { return selected; }
            let mut set = [0u32; 5];
            make_set(set.as_mut_ptr(), 100);
            let manager = updates::UpdateState::from_ptr(this).flora();
            let found = vcall!(manager, 0x38 => fn(*mut u32, u32) -> u32, set.as_mut_ptr(), selected);
            let mut selected = 0;
            if found != 0 && set[4] != 0 {
                let mut index = random_index(random_state(), set[4]);
                let mut iterator = [0u32; 2];
                begin_set(set.as_mut_ptr(), iterator.as_mut_ptr());
                let mut node = iterator[0];
                while index != 0 {
                    node = (node as *const u32).read_unaligned();
                    if node == 0 { node = next_bucket(iterator.as_mut_ptr()); }
                    index = index.wrapping_sub(1);
                    iterator[0] = node;
                }
                selected = (node as *const u32).add(1).read_unaligned();
            }
            dispose_set(&mut set);
            selected
        }
    }
}

port! {
    /// Visits type-four records in configuration order and applies the record's tag to each created flora.
    #[port(address = 0x006C_D2B0, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddLotFlora")]
    pub unsafe extern "thiscall" fn fc4p_006cd2b0(this: *mut u8, lot: *mut u8, config: *mut u8, _flags: u32) {
        unsafe {
            let objects = vcall!(config, 0x5C => fn() -> *mut u32);
            if objects.is_null() { return; }
            let count = (objects.add(1).read_unaligned().wrapping_sub(objects.read_unaligned()) as i32) / 52;
            if count <= 0 { return; }
            let mut offset = 0u32;
            let mut remaining = count;
            while remaining != 0 {
                let record = (objects.read_unaligned() as *mut u8).wrapping_add(offset as usize);
                if record.read() == 4 {
                    let mut position = [position_initial().read(); 3];
                    let mut facing = 0u32;
                    vcall!(this, 0x9C => fn(*mut u8, *mut u8, *mut u8, *mut u32, *mut u32),
                        lot, config, record, position.as_mut_ptr(), &mut facing);
                    let selected = pick_flora(this, record.add(0x28).cast(), position.as_mut_ptr());
                    if selected != 0 {
                        let manager = updates::UpdateState::from_ptr(this).flora();
                        let occupant = vcall!(manager, 0x44 => fn(u32, *mut u32, u32) -> *mut u8,
                            selected, position.as_mut_ptr(), 0);
                        if !occupant.is_null() {
                            vcall!(occupant, 0x70 => fn(u32), record.add(0x24).cast::<u32>().read_unaligned());
                        }
                    }
                }
                offset = offset.wrapping_add(52);
                remaining -= 1;
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006ccef0::PORT, fc4p_006cd010::PORT, fc4p_006cd2b0::PORT];
