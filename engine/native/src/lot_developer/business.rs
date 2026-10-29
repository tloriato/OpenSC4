// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Selecting neighboring lots for business units.

use super::maintenance::OccupantNode;
use super::*;

layout! {
    /// The purpose word at the beginning of a building profile.
    struct BuildingProfile {
        0x00 => purpose / set_purpose: i32,
    }
}

game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn reference_new(output: *mut *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x004F_20C0; }
game_fn! { #[allow(dead_code)] fn vector_push(vector: *mut u32, reference: *mut *mut u8) -> u64 = "thiscall" 0x0044_2E40; }
game_fn! { #[allow(dead_code)] fn vector_destroy(vector: *mut u32) -> u64 = "thiscall" 0x006C_6E90; }
game_fn! { #[allow(dead_code)] fn list_clear(list: *mut *mut u8) -> u64 = "thiscall" 0x0047_3EC0; }
game_fn! { #[allow(dead_code)] fn random(this: *mut u8, bound: u32) -> u64 = "thiscall" 0x0091_36B1; }
game_fn! { #[allow(dead_code)] fn allocate_unit(bytes: u32) -> u64 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn unit_new(this: *mut u8) -> u64 = "thiscall" 0x006C_0800; }
game_fn! { #[allow(dead_code)] fn initial_anchor(first: u32, last: u32, output: *mut u8, allocator: *mut u32) -> u64 = "C" 0x0042_E4C0; }
game_fn! { #[allow(dead_code)] fn copy_nodes(this: *mut u32, head: *mut u8, first: *mut u8, last: *mut u8, allocator: *mut u32) -> u64 = "thiscall" 0x006C_6CA0; }
game_fn! { #[allow(dead_code)] fn pick_unit_to_join(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_B260; }
game_data! { static random_state: u8 = 0x00B4_9AF0; }

unsafe fn direct(address: u32, conv: Conv, this: u32, args: &[u32], eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

port! {
    /// Supplies the caller's lot word, which becomes the temporary neighbor-list head.
    #[port(address = 0x006C_B260, owner = "lotdeveloper", name = "cSC4LotDeveloper::PickUnitToJoin", naked)]
    pub unsafe extern "C" fn fc4p_006cb260() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 4", sym pick_unit);
    }
}

unsafe extern "fastcall" fn pick_unit(this: *mut u8, _entry_edx: u32, list: *mut *mut u8) -> u64 {
    unsafe {
        let lot = list.read_unaligned();
        let mut x = 0u32;
        let mut z = 0u32;
        let mut call = virtual_call(
            lot,
            0x54,
            &[&mut x as *mut u32 as u32, &mut z as *mut u32 as u32],
            table(lot),
            &mut x as *mut u32 as u32,
            false,
        );
        call = virtual_call(lot, 0x64, &[], table(lot), call.edx, false);
        if call.eax != 0 {
            return result(call);
        }
        call = virtual_call(lot, 0xB8, &[1], call.eax, table(lot), false);
        if call.eax == 0 {
            return result(call);
        }
        let developer = LotDeveloper::from_ptr(this);
        let service = developer.building_development();
        call = virtual_call(service, 0x9C, &[call.eax], call.eax, table(service), false);
        let profile = BuildingProfile::from_ptr(call.eax as *mut u8);
        call.eax = profile.purpose() as u32;
        if !(5..=8).contains(&(call.eax as i32)) {
            return result(call);
        }
        list.write_unaligned(core::ptr::null_mut());
        call = direct(0x0090_CF54, Conv::Cdecl, service as u32, &[12], call.eax, call.edx);
        let head = call.eax as *mut u8;
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        list.write_unaligned(head);
        for flags in [1, 2, 4, 8] {
            let manager = developer.lot_manager();
            call = virtual_call(
                manager,
                0x24,
                &[lot as u32, list as u32, flags],
                table(manager),
                list as u32,
                false,
            );
        }
        let mut vector = [0u32; 3];
        let mut current = OccupantNode::from_ptr(list.read_unaligned()).next();
        call.eax = current as u32;
        call.edx = list.read_unaligned() as u32;
        while current != list.read_unaligned() {
            let neighbor = OccupantNode::from_ptr(current).occupant();
            call = virtual_call(neighbor, 0xB8, &[1], table(neighbor), call.edx, false);
            let kind = call.eax;
            let mut eligible = false;
            if kind != 0 {
                call = virtual_call(neighbor, 0x84, &[], call.eax, table(neighbor), false);
                let active = if call.eax as i32 > 1 {
                    true
                } else {
                    call = virtual_call(neighbor, 0x64, &[], table(neighbor), call.edx, false);
                    call.eax != 0
                };
                if active {
                    let service = developer.building_development();
                    call = virtual_call(service, 0x9C, &[kind], call.eax, table(service), false);
                    let purpose = profile.purpose();
                    if BuildingProfile::from_ptr(call.eax as *mut u8).purpose() == purpose {
                        if purpose == 5 {
                            call = virtual_call(neighbor, 0xC0, &[], call.eax, table(neighbor), false);
                            eligible = call.eax != 0;
                        } else {
                            call = virtual_call(neighbor, 0x70, &[], table(neighbor), call.edx, false);
                            eligible = call.eax as u8 != 0;
                            if !eligible {
                                call = virtual_call(neighbor, 0xEC, &[], call.eax, table(neighbor), false);
                                let designate = call.eax as *mut u8;
                                if !designate.is_null() {
                                    call = virtual_call(designate, 0x70, &[], call.eax, table(designate), false);
                                    eligible = call.eax as u8 != 0;
                                }
                            }
                        }
                    }
                }
            }
            if eligible {
                let mut reference = core::ptr::null_mut::<u8>();
                call = direct(
                    0x004F_20C0,
                    Conv::Thiscall,
                    &mut reference as *mut *mut u8 as u32,
                    &[neighbor as u32],
                    call.eax,
                    call.edx,
                );
                call = direct(
                    0x0044_2E40,
                    Conv::Thiscall,
                    vector.as_mut_ptr() as u32,
                    &[&mut reference as *mut *mut u8 as u32],
                    &mut reference as *mut *mut u8 as u32,
                    call.edx,
                );
                if !reference.is_null() {
                    call = virtual_call(reference, 8, &[], call.eax, table(reference), false);
                }
            }
            current = OccupantNode::from_ptr(current).next();
            call.eax = current as u32;
        }
        let count = vector[1].wrapping_sub(vector[0]) as i32 >> 2;
        if count > 0 {
            call = direct(
                0x0091_36B1,
                Conv::Thiscall,
                random_state() as u32,
                &[count as u32],
                call.eax,
                call.edx,
            );
            let neighbor = (vector[0].wrapping_add(call.eax.wrapping_mul(4)) as *const *mut u8).read_unaligned();
            call = virtual_call(neighbor, 0x64, &[], call.eax, table(neighbor), false);
            let mut unit = call.eax as *mut u8;
            if unit.is_null() {
                call = direct(0x005E_55E0, Conv::Cdecl, neighbor as u32, &[32], call.eax, call.edx);
                unit = call.eax as *mut u8;
                if !unit.is_null() {
                    call = direct(0x006C_0800, Conv::Thiscall, unit as u32, &[], call.eax, call.edx);
                    unit = call.eax as *mut u8;
                }
                call = virtual_call(unit, 0x0C, &[neighbor as u32], table(unit), call.edx, false);
            }
            call = virtual_call(unit, 0x0C, &[lot as u32], call.eax, table(unit), false);
            if profile.purpose() == 5 {
                call = virtual_call(unit, 0x14, &[], profile.as_ptr() as u32, table(unit), false);
                let members = (call.eax as *const *mut u8).read_unaligned();
                let first = OccupantNode::from_ptr(members).next();
                let designate = OccupantNode::from_ptr(first).occupant();
                call = virtual_call(lot, 0xF0, &[designate as u32], designate as u32, table(lot), false);
            } else {
                developer.set_business_units_dirty(1);
                call.eax = profile.as_ptr() as u32;
            }
        }
        call = direct(
            0x006C_6E90,
            Conv::Thiscall,
            vector.as_mut_ptr() as u32,
            &[],
            call.eax,
            call.edx,
        );
        call = direct(0x0047_3EC0, Conv::Thiscall, list as u32, &[], call.eax, call.edx);
        call.eax = list.read_unaligned() as u32;
        if call.eax != 0 {
            call = direct(0x0090_CF63, Conv::Cdecl, list as u32, &[call.eax], call.eax, call.edx);
        }
        result(call)
    }
}

port! {
    /// Supplies the caller's reference argument word, reused for the membership-test byte.
    #[port(address = 0x006C_B4D0, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateTravelDesignates", naked)]
    pub unsafe extern "C" fn fc4p_006cb4d0() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 4", sym update_designates);
    }
}

unsafe extern "fastcall" fn update_designates(this: *mut u8, entry_edx: u32, arguments: *mut u32) -> u64 {
    unsafe {
        let unit_reference = arguments.read_unaligned() as *mut *mut u8;
        let unit = unit_reference.read_unaligned();
        let mut call = virtual_call(unit, 0x14, &[], table(unit), entry_edx, false);
        let members = call.eax as *mut *mut u8;
        let head = members.read_unaligned();
        let mut current = OccupantNode::from_ptr(head).next();
        let mut vector_begin = 0u32;
        let mut vector_end = 0u32;
        while current != members.read_unaligned() {
            let lot = OccupantNode::from_ptr(current).occupant();
            call = virtual_call(lot, 0x70, &[], call.eax, table(lot), false);
            if call.eax as u8 != 0 {
                call = virtual_call(lot, 0xC0, &[], table(lot), call.edx, false);
                if call.eax != 0 {
                    call = virtual_call(lot, 4, &[], call.eax, table(lot), false);
                    call = direct(0x0090_CF54, Conv::Cdecl, lot as u32, &[4], call.eax, call.edx);
                    vector_begin = call.eax;
                    call = direct(
                        0x0042_E4C0,
                        Conv::Cdecl,
                        arguments as u32,
                        &[0, 0, vector_begin, arguments as u32],
                        call.eax,
                        call.edx,
                    );
                    let insertion = call.eax;
                    if insertion != 0 {
                        (insertion as *mut *mut u8).write_unaligned(lot);
                        virtual_call(lot, 4, &[], call.eax, table(lot), false);
                    }
                    vector_end = insertion.wrapping_add(4);
                    call = virtual_call(lot, 8, &[], vector_begin, table(lot), false);
                    break;
                }
            }
            current = OccupantNode::from_ptr(current).next();
        }
        call.eax = (vector_end.wrapping_sub(vector_begin) as i32 >> 2) as u32;
        if call.eax as i32 > 0 {
            let sentinel = members.read_unaligned();
            current = OccupantNode::from_ptr(sentinel).next();
            while current != members.read_unaligned() {
                let lot = OccupantNode::from_ptr(current).occupant();
                call = virtual_call(lot, 0xEC, &[], call.eax, table(lot), false);
                let designate = call.eax as *mut u8;
                let mut absent = false;
                if !designate.is_null() {
                    let sentinel = members.read_unaligned();
                    call = virtual_call(designate, 4, &[], table(designate), call.edx, false);
                    let head = members.read_unaligned();
                    let mut search = OccupantNode::from_ptr(head).next();
                    while search != head && OccupantNode::from_ptr(search).occupant() != designate {
                        search = OccupantNode::from_ptr(search).next();
                    }
                    absent = search == sentinel;
                    arguments.cast::<u8>().write(u8::from(absent));
                    call = virtual_call(designate, 8, &[], call.eax, table(designate), false);
                } else {
                    arguments.cast::<u8>().write(0);
                }
                call.eax = (call.eax & !255) | u32::from(absent);
                let mut keep = false;
                if !absent && !designate.is_null() {
                    call = virtual_call(designate, 0x70, &[], call.eax, table(designate), false);
                    keep = call.eax as u8 != 0;
                }
                if !keep {
                    if absent || !designate.is_null() {
                        call = virtual_call(lot, 0xF0, &[0], table(lot), call.edx, false);
                    }
                    let anchor = (vector_begin as *const u32).read_unaligned();
                    call.eax = anchor;
                    if lot as u32 != anchor {
                        call = virtual_call(lot, 0xF0, &[anchor], anchor, table(lot), false);
                    }
                }
                current = OccupantNode::from_ptr(current).next();
                call.eax = current as u32;
            }
        } else {
            let unit = unit_reference.read_unaligned();
            call = virtual_call(unit, 0x14, &[], call.eax, table(unit), false);
            let source = call.eax as *mut *mut u8;
            call = direct(0x0090_CF54, Conv::Cdecl, unit as u32, &[12], call.eax, call.edx);
            let head = call.eax as *mut u8;
            let sentinel = OccupantNode::from_ptr(head);
            sentinel.set_next(head);
            sentinel.set_previous(head);
            let last = source.read_unaligned();
            let first = OccupantNode::from_ptr(last).next();
            call = direct(
                0x006C_6CA0,
                Conv::Thiscall,
                arguments as u32,
                &[head as u32, first as u32, last as u32, arguments as u32],
                call.eax,
                head as u32,
            );
            current = sentinel.next();
            let mut joined = core::ptr::null_mut::<u8>();
            while current != head {
                let lot = OccupantNode::from_ptr(current).occupant();
                let unit = unit_reference.read_unaligned();
                call = virtual_call(unit, 0x10, &[lot as u32], table(unit), call.edx, false);
                call = direct(
                    0x006C_B260,
                    Conv::Thiscall,
                    this as u32,
                    &[lot as u32],
                    call.eax,
                    call.edx,
                );
                call = virtual_call(lot, 0x64, &[], call.eax, table(lot), false);
                joined = call.eax as *mut u8;
                if !joined.is_null() {
                    break;
                }
                current = OccupantNode::from_ptr(current).next();
            }
            if !joined.is_null() {
                current = sentinel.next();
                while current != head {
                    let lot = OccupantNode::from_ptr(current).occupant();
                    call = virtual_call(lot, 0x64, &[], table(lot), call.edx, false);
                    let old_unit = call.eax as *mut u8;
                    if !old_unit.is_null() {
                        call = virtual_call(old_unit, 0x10, &[lot as u32], call.eax, table(old_unit), false);
                    }
                    call = virtual_call(joined, 0x0C, &[lot as u32], table(joined), call.edx, false);
                    current = OccupantNode::from_ptr(current).next();
                }
            }
            unit_reference.write_unaligned(joined);
            current = sentinel.next();
            while current != head {
                let node = OccupantNode::from_ptr(current);
                let lot = node.occupant();
                let next = node.next();
                if !lot.is_null() {
                    call = virtual_call(lot, 8, &[], call.eax, table(lot), false);
                }
                call = direct(
                    0x0090_CF63,
                    Conv::Cdecl,
                    lot as u32,
                    &[current as u32],
                    call.eax,
                    call.edx,
                );
                current = next;
            }
            sentinel.set_next(head);
            sentinel.set_previous(head);
            call = direct(
                0x0090_CF63,
                Conv::Cdecl,
                head as u32,
                &[head as u32],
                call.eax,
                call.edx,
            );
        }
        let mut cursor = vector_begin;
        while cursor != vector_end {
            let lot = (cursor as *const *mut u8).read_unaligned();
            if !lot.is_null() {
                call = virtual_call(lot, 8, &[], table(lot), call.edx, false);
            }
            cursor = cursor.wrapping_add(4);
        }
        if vector_begin != 0 {
            call = direct(
                0x0090_CF63,
                Conv::Cdecl,
                vector_begin,
                &[vector_begin],
                call.eax,
                call.edx,
            );
        }
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006cb260::PORT, fc4p_006cb4d0::PORT];
