// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Base-texture selection and lot clearing with their original service calls.

use super::*;
use super::maintenance::OccupantNode;

game_fn! { fn random_index(this: *mut u8, count: u32) -> u32 = "thiscall" 0x0091_36B1; }
game_fn! { fn allocate_node(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free_node(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn clear_lot_contents(lot: *mut u8, flags: u32) -> u64 = "C" 0x005E_3070; }
game_fn! { fn update_surrounding(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_7720; }
game_fn! { fn clear_lot_traffic(lot: *mut u8) -> u64 = "C" 0x005E_3960; }
game_data! { static random_state: u8 = 0x00B4_9AF0; }
game_data! { static resource_manager: *mut u8 = 0x00B4_3CA8; }

layout! {
    /// The texture object's candidate instance vector.
    pub struct TextureCandidates {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
    }
}

port! {
    /// Draws a candidate, tries wealth/condition, wealth-only and condition-only variants, then uses the
    /// candidate unchanged. An empty vector makes no random or service call.
    #[port(address = 0x006C_60C0, owner = "lotdeveloper", name = "cSC4LotDeveloper::PickLotBaseTextureForLot")]
    pub unsafe extern "thiscall" fn fc4p_006c60c0(this: *mut u8, lot: *mut u8, candidates: *mut u8) -> u32 {
        unsafe {
            let vector = TextureCandidates::from_ptr(candidates);
            let end = vector.end();
            let begin = vector.begin();
            if begin == end { return 0; }
            let count = (end as i32).wrapping_sub(begin as i32) >> 2;
            let index = random_index(random_state(), count as u32);
            let selected = vector.begin().wrapping_add(index as usize).read_unaligned();
            let mut wealth = vcall!(lot, 0xA4 => fn() -> u32);
            let condition = match vcall!(lot, 0x8C => fn() -> u32) {
                2 | 3 => 1,
                4..=6 => 2,
                _ => 0,
            };
            if wealth == 0 {
                let kind = vcall!(lot, 0xB8 => fn(u32) -> u32, 0);
                wealth = if kind == 0 { 1 } else {
                    let profile = vcall!(LotDeveloper::from_ptr(this).building_development(), 0x9C => fn(u32) -> *mut u32, kind);
                    profile.add(1).read_unaligned()
                };
            }
            let base = selected & 0xFFFF_0000;
            let resource = resource_manager().read_unaligned();
            let mut key = [0x7AB5_0E44, 0x0986_135E, 0];
            for candidate in [
                ((wealth.wrapping_shl(4) | condition).wrapping_shl(8)) | base,
                wealth.wrapping_shl(12) | base,
                condition.wrapping_shl(8) | base,
            ] {
                key[2] = candidate;
                if vcall!(resource, 0x38 => fn(*mut u32) -> u32, key.as_mut_ptr()) as u8 != 0 {
                    return candidate;
                }
            }
            selected
        }
    }
}

port! {
    /// Refreshes foundations of surrounding lots with either foundation flag and releases every list
    /// member before freeing the temporary list. ECX retains the entry developer pointer.
    #[port(address = 0x006C_7720, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateSurroundingLotFoundations", naked)]
    pub unsafe extern "C" fn fc4p_006c7720() {
        core::arch::naked_asm!(
            "push ecx", "push dword ptr [esp + 8]", "call {body}", "pop ecx", "ret 4",
            body = sym update_surrounding_body,
        )
    }
}

unsafe extern "thiscall" fn update_surrounding_body(this: *mut u8, lot: *mut u8) -> u64 {
    unsafe {
        let allocation = allocate_node(12);
        let sentinel = OccupantNode::from_ptr(allocation);
        sentinel.set_next(allocation);
        sentinel.set_previous(allocation);
        let mut head = allocation;
        let queried = vcall!(LotDeveloper::from_ptr(this).lot_manager(), 0x24 => fn(*mut u8, *mut *mut u8, u32) -> u32,
            lot, &mut head, 15);
        if queried != 0 {
            let mut current = OccupantNode::from_ptr(head).next();
            while current != head {
                let member = OccupantNode::from_ptr(current).occupant();
                if vcall!(member, 0x30 => fn() -> u32) as u8 & 3 != 0 {
                    vcall!(this, 0xF4 => fn(*mut u8) -> u32, member);
                }
                current = OccupantNode::from_ptr(current).next();
            }
        }
        let mut current = OccupantNode::from_ptr(head).next();
        while current != head {
            let node = OccupantNode::from_ptr(current);
            let member = node.occupant();
            let next = node.next();
            if !member.is_null() { vcall!(member, 8 => fn() -> u32); }
            free_node(current);
            current = next;
        }
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        free_node(head)
    }
}

port! {
    /// Clears lot contents, the foundation at its saved location, neighboring foundations and traffic,
    /// then resets the lot's unit, habitation, growth type and configuration before setting empty texture.
    #[port(address = 0x006C_8A20, owner = "lotdeveloper", name = "cSC4LotDeveloper::ClearLot")]
    pub unsafe extern "thiscall" fn fc4p_006c8a20(this: *mut u8, lot: *mut u8) -> u64 {
        unsafe {
            let mut x = 0u32;
            let mut z = 0u32;
            vcall!(lot, 0x54 => fn(*mut u32, *mut u32) -> u32, &mut x, &mut z);
            clear_lot_contents(lot, 0);
            vcall!(this, 0xD4 => fn(u32, u32) -> u32, x, z);
            update_surrounding(this, lot);
            clear_lot_traffic(lot);
            vcall!(lot, 0x114 => fn(u32) -> u32, 0);
            vcall!(lot, 0x88 => fn(u32, u32, u32) -> u32, 1, 0, 0);
            vcall!(lot, 0xBC => fn(u32) -> u32, 0);
            vcall!(lot, 0x28 => fn(u32) -> u32, 0);
            vcall!(this, 0xAC => fn(*mut u8) -> u64, lot)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c60c0::PORT, fc4p_006c7720::PORT, fc4p_006c8a20::PORT];
