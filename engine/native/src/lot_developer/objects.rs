// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant searches, prop removal and the end of lot demolition.

use super::maintenance::OccupantNode;
use super::*;

game_fn! { #[allow(dead_code)] fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_service(service: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn clear_nodes(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3E60; }
game_fn! { #[allow(dead_code)] fn destroy_list(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3FE0; }
game_fn! { #[allow(dead_code)] fn clear_lot(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_8A20; }
game_fn! { #[allow(dead_code)] fn remove_prop(this: *mut u8, lot: *mut u8, kind: u32) -> u64 = "thiscall" 0x006C_9780; }
game_fn! { fn post_message(id: u32, data1: u32, data2: u32, data3: u32, data4: u32, object: *mut u8) -> u64 = "C" 0x005F_98C0; }

game_data! { static node_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static occupant_manager: *mut u8 = 0x00B4_3D0C; }
game_data! { static position_inset: f32 = 0x00A8_80D8; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static blocking_margin: f32 = 0x00A9_5690; }
game_data! { static view_utilities: *mut u8 = 0x00B4_3DD8; }
game_data! { static zot_height: f32 = 0x00A8_5074; }

layout! {
    /// The current view rotation used for zot props.
    struct ViewUtilities {
        0x10 => rotation / set_rotation: u32,
    }
}

layout! {
    /// The filter used when querying occupants that obstruct a lot.
    struct BlockingQuery {
        0x58 => filter / set_filter: *mut u8,
    }
}

/// Some helpers retain EDX on an empty list. Supply the caller's register inputs explicitly.
unsafe fn direct(address: u32, conv: Conv, this: u32, args: &[u32], eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

/// Lazily obtains the game's shared list allocator. Releasing the temporary interface happens after
/// publishing it, so a release callback can replace the global before the following allocation.
unsafe fn allocator(mut edx: u32) -> (*mut u8, u32) {
    unsafe {
        if node_allocator().read().is_null() {
            let mut service = core::ptr::null_mut();
            let returned = direct(0x0087_93EC, Conv::Cdecl, 0, &[], 0, edx);
            let root = returned.eax as *mut u8;
            if !root.is_null() {
                vcall!(root, 0x14 => fn(u32, u32, *mut *mut u8) -> u64,
                    0x3AE4_BEA3, 0x3AE4_BEAB, &mut service);
            }
            node_allocator().write(service);
            edx = direct(
                0x0041_1870,
                Conv::Thiscall,
                &mut service as *mut *mut u8 as u32,
                &[],
                returned.eax,
                service as u32,
            )
            .edx;
        }
        (node_allocator().read(), edx)
    }
}

pub(super) unsafe fn new_list(edx: u32) -> (*mut u8, u32) {
    unsafe {
        let (alloc, edx) = allocator(edx);
        let call = virtual_call(alloc, 0x0C, &[12], table(alloc), edx, false);
        let head = call.eax as *mut u8;
        let node = OccupantNode::from_ptr(head);
        node.set_next(head);
        node.set_previous(head);
        (head, call.edx)
    }
}

/// The node clearer does not release payloads; callers release them during their own traversal.
unsafe fn free_list(list: *mut *mut u8, eax: u32, edx: u32) -> RawCall {
    unsafe {
        let returned = direct(0x0041_3E60, Conv::Thiscall, list as u32, &[], eax, edx);
        let head = list.read_unaligned();
        let (alloc, edx) = allocator(returned.edx);
        virtual_call(alloc, 0x10, &[head as u32], table(alloc), edx, false)
    }
}

port! {
    /// Finishes an initialized lot's demolition, clears it, and deletes it for zones 0, 11, 12 or 15.
    /// Other zones retain the lot. Integer return registers are those of the last service call.
    #[port(address = 0x006C_90A0, owner = "lotdeveloper", name = "cSC4LotDeveloper::EndDemolishLot")]
    pub unsafe extern "fastcall" fn fc4p_006c90a0(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        unsafe {
            let mut call = virtual_call(lot, 0x18, &[], table(lot), entry_edx, false);
            if call.eax as u8 != 0 {
                call = virtual_call(lot, 0x88, &[6, 0, 0], call.eax, table(lot), false);
                call = direct(0x006C_8A20, Conv::Thiscall, this as u32, &[lot as u32], call.eax, call.edx);
                call = virtual_call(lot, 0x7C, &[], table(lot), call.edx, false);
                if matches!(call.eax, 0 | 11 | 12 | 15) {
                    let manager = LotDeveloper::from_ptr(this).lot_manager();
                    call = virtual_call(manager, 0x60, &[lot as u32], call.eax, table(manager), false);
                }
            }
            result(call)
        }
    }
}

port! {
    /// Queries one building in the lot's cell rectangle. A successful interface query is released before
    /// the occupant list; the borrowed interface pointer remains the result, including on query failure.
    #[port(address = 0x006C_9100, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetFirstBuilding", naked)]
    pub unsafe extern "fastcall" fn fc4p_006c9100(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        stack_entry!(first_building, 4);
    }
}

unsafe extern "fastcall" fn first_building(this: *mut u8, stack: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let lot = stack.add(1).read() as *mut u8;
        let mut rect = [0u32; 4];
        let mut building = core::ptr::null_mut();
        let _ = entry_edx;
        let call = virtual_call(lot, 0x38, &[rect.as_mut_ptr() as u32], table(lot), rect.as_ptr() as u32, false);
        let head_slot = stack.add(1).cast::<*mut u8>();
        head_slot.write(core::ptr::null_mut());
        let (head, _) = new_list(call.edx);
        head_slot.write(head);
        let manager = LotDeveloper::from_ptr(this).occupant_manager();
        let mut call = virtual_call(manager, 0xA4,
            &[head_slot as u32, rect.as_ptr() as u32, rect.as_ptr().add(2) as u32, 0x2781_28A0, 1],
            head_slot as u32, table(manager), false);
        if call.eax as u8 != 0 {
            let head = head_slot.read();
            let first = OccupantNode::from_ptr(head).next();
            call.eax = first as u32;
            if first != head {
                let occupant = OccupantNode::from_ptr(first).occupant();
                call = virtual_call(occupant, 0, &[0x87DD_A3A9, &mut building as *mut *mut u8 as u32],
                    &mut building as *mut *mut u8 as u32, table(occupant), false);
                if call.eax as u8 != 0 {
                    virtual_call(building, 8, &[], call.eax, table(building), false);
                }
                let manager = LotDeveloper::from_ptr(this).occupant_manager();
                call = virtual_call(manager, 0xC0, &[head_slot as u32],
                    table(manager), head_slot as u32, false);
            }
        }
        let saved = building;
        let returned = direct(0x0041_3FE0, Conv::Thiscall, head_slot as u32,
            &[], call.eax, call.edx);
        (u64::from(returned.edx) << 32) | u64::from(saved as u32)
    }
}

/// Finds the first matching foundation, but continues releasing every occupant in the list.
unsafe fn find_foundation(head: *mut u8, kind: u32, clear_interface: bool, mut call: RawCall, interface_slot: *mut *mut u8) -> (*mut u8, RawCall) {
    unsafe {
        let mut found = core::ptr::null_mut::<u8>();
        let mut current = OccupantNode::from_ptr(head).next();
        call.eax = head as u32;
        while current != head {
            let node = OccupantNode::from_ptr(current);
            let occupant = node.occupant();
            if clear_interface {
                interface_slot.write(core::ptr::null_mut());
            }
            if found.is_null() {
                call = virtual_call(
                    occupant,
                    0,
                    &[0xC8FC_E101, interface_slot as u32],
                    if clear_interface {
                        table(occupant)
                    } else {
                        interface_slot as u32
                    },
                    if clear_interface {
                        interface_slot as u32
                    } else {
                        table(occupant)
                    },
                    false,
                );
                if call.eax as u8 != 0 {
                    let interface = interface_slot.read();
                    call = virtual_call(interface, 0x24, &[], call.eax, table(interface), false);
                    if call.eax == kind {
                        found = interface;
                    }
                    call = virtual_call(interface, 8, &[], table(interface), call.edx, false);
                }
            }
            call = virtual_call(occupant, 8, &[], call.eax, table(occupant), false);
            current = node.next();
        }
        (found, call)
    }
}

port! {
    /// Returns the first foundation of the requested kind on one cell, releasing interfaces and all
    /// queried occupants. A false low query byte skips payload releases but still frees the list nodes.
    #[port(address = 0x006C_9480, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetFoundationByStandardCityCell", naked)]
    pub unsafe extern "fastcall" fn fc4p_006c9480(this: *mut u8, entry_edx: u32, kind: u32, x: u32, z: u32) -> u64 {
        stack_entry!(foundation_cell, 12);
    }
}

// The x argument becomes the list head, and the z argument becomes the queried interface.
unsafe extern "fastcall" fn foundation_cell(this: *mut u8, stack: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let kind = stack.add(1).read();
        let x = stack.add(2).read();
        let z = stack.add(3).read();
        let rect = [x, z, x, z];
        let head_slot = stack.add(2).cast::<*mut u8>();
        head_slot.write(core::ptr::null_mut());
        stack.add(3).write(0);
        let (head, _) = new_list(entry_edx);
        head_slot.write(head);
        let manager = LotDeveloper::from_ptr(this).occupant_manager();
        let queried = virtual_call(manager, 0xA4,
            &[head_slot as u32, rect.as_ptr().add(2) as u32, rect.as_ptr() as u32, 0x68FD_0C69, u32::MAX],
            head_slot as u32, table(manager), false);
        let (found, call) = if queried.eax as u8 != 0 { find_foundation(head_slot.read(), kind, false, queried, stack.add(3).cast()) } else { (core::ptr::null_mut(), queried) };
        let mut call = free_list(&mut *head_slot, call.eax, call.edx);
        call.eax = found as u32;
        result(call)
    }
}

port! {
    /// Probes a vertical line at (x - inset, z + inset), with the same horizontal position for both
    /// endpoints. A missing local occupant manager is retained from the global before the query.
    #[port(address = 0x006C_95E0, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetFoundationByPosition", naked)]
    pub unsafe extern "fastcall" fn fc4p_006c95e0(this: *mut u8, entry_edx: u32, kind: u32, x: f32, z: f32) -> u64 {
        stack_entry!(foundation_position, 12);
    }
}

unsafe extern "fastcall" fn foundation_position(this: *mut u8, stack: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let kind = stack.add(1).read();
        let x = f32::from_bits(stack.add(2).read());
        let z = f32::from_bits(stack.add(3).read());
        let (mut head, _) = new_list(entry_edx);
        let x = terrain::subtract_float(F80::from_f32(x), *position_inset()).to_f32();
        let developer = LotDeveloper::from_ptr(this);
        let missing_manager = developer.occupant_manager().is_null();
        let z = terrain::add_float(F80::from_f32(z), *position_inset()).to_f32();
        let min = [x, -f32::MAX, z];
        let max = [x, f32::MAX, z];
        if missing_manager {
            developer.set_occupant_manager(occupant_manager().read());
            vcall!(developer.occupant_manager(), 4 => fn() -> u32);
        }
        let manager = developer.occupant_manager();
        let queried = virtual_call(manager, 0x94,
            &[&mut head as *mut *mut u8 as u32, min.as_ptr() as u32, max.as_ptr() as u32, 0x68FD_0C69, u32::MAX],
            table(manager), &mut head as *mut *mut u8 as u32, false);
        let (found, call) = if queried.eax as u8 != 0 { find_foundation(head, kind, true, queried, stack.add(2).cast()) } else { (core::ptr::null_mut(), queried) };
        let mut call = free_list(&mut head, call.eax, call.edx);
        call.eax = found as u32;
        result(call)
    }
}

port! {
    /// Removes every non-null lot prop carrying the requested zot number. Finding a matching prop sets
    /// AL even when the manager's removal call returns false. All non-null queried props are released.
    #[port(address = 0x006C_9780, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveLotProp")]
    pub unsafe extern "fastcall" fn fc4p_006c9780(this: *mut u8, entry_edx: u32, lot: *mut u8, kind: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let (mut head, _) = new_list(entry_edx);
            let manager = developer.prop_manager();
            let mut call = virtual_call(manager, 0x60, &[&mut head as *mut *mut u8 as u32, lot as u32],
                &mut head as *mut *mut u8 as u32, table(manager), false);
            let mut removed = false;
            let mut current = OccupantNode::from_ptr(head).next();
            call.eax = head as u32;
            while current != head {
                let node = OccupantNode::from_ptr(current);
                let prop = node.occupant();
                if !prop.is_null() {
                    call = virtual_call(prop, 0x18, &[], call.eax, table(prop), false);
                    let holder = call.eax as *mut u8;
                    call = virtual_call(holder, 0x24, &[0x48E9_5539], call.eax, table(holder), false);
                    let property = call.eax as *mut u8;
                    if !property.is_null() {
                        call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                        let variant = call.eax as *mut u8;
                        call = virtual_call(variant, 0x120, &[], call.eax, table(variant), false);
                        if call.eax == kind {
                            let manager = developer.prop_manager();
                            call = virtual_call(manager, 0x74, &[prop as u32], table(manager), call.edx, false);
                            removed = true;
                        }
                    }
                    call = virtual_call(prop, 8, &[], call.eax, table(prop), false);
                    call.eax = head as u32;
                }
                current = node.next();
            }
            let mut call = free_list(&mut head, call.eax, call.edx);
            call.eax = (call.eax & !255) | u32::from(removed);
            result(call)
        }
    }
}

port! {
    /// Adds the requested zot when no zot exists or its number has higher priority. Road notifications
    /// precede the priority test. The prop uses the building roof when all configuration inputs exist,
    /// otherwise the lot center, and faces the current view rotation.
    #[port(address = 0x006C_98C0, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddBuildingZotProp")]
    pub unsafe extern "fastcall" fn fc4p_006c98c0(this: *mut u8, entry_edx: u32, lot: *mut u8, prop_type: u32, kind: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut call = virtual_call(lot, 0xC0, &[], table(lot), entry_edx, false);
            let building = call.eax as *mut u8;
            call = virtual_call(lot, 0x24, &[], call.eax, table(lot), false);
            let config = call.eax as *mut u8;
            let primary = if config.is_null() { 0 } else {
                call = virtual_call(config, 0x60, &[], table(config), call.edx, false);
                call.eax
            };
            if kind == 6 {
                let returned = post_message(0x4C04_DE69, 1, 0, 0, 0, lot);
                call.eax = returned as u32;
            }
            call = virtual_call(lot, 0x0C, &[], call.eax, table(lot), false);
            let holder = call.eax as *mut u8;
            call = virtual_call(holder, 0x24, &[0xC999_C45E], table(holder), call.edx, false);
            if call.eax == 0 {
                call = virtual_call(holder, 0x34, &[0xC999_C45E, 0, 0], 0, table(holder), false);
                call = virtual_call(holder, 0x24, &[0xC999_C45E], table(holder), call.edx, false);
            }
            let property = call.eax as *mut u8;
            call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
            let variant = call.eax as *mut u8;
            call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
            let current = (call.eax as *const u32).read_unaligned();
            call.eax = current;
            if current != 0 {
                if kind >= current { return result(call); }
                direct(0x006C_9780, Conv::Thiscall, this as u32, &[lot as u32, current], call.eax, call.edx);
            }
            virtual_call(variant, 0x68, &[kind], kind, table(variant), false);
            let mut position = [0u32; 3];
            let y = if !building.is_null() && !config.is_null() && primary != 0 {
                let mut orientation = primary;
                virtual_call(this, 0xA0, &[lot as u32, position.as_mut_ptr() as u32, &mut orientation as *mut u32 as u32],
                    &mut orientation as *mut u32 as u32, table(this), false);
                let building = vcall!(lot, 0xC0 => fn() -> *mut u8);
                let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
                let mut min = [0u32; 3];
                let mut max = [0u32; 3];
                vcall!(occupant, 0x28 => fn(*mut u32, *mut u32) -> u32, min.as_mut_ptr(), max.as_mut_ptr());
                f32::from_bits(max[1])
            } else {
                vcall!(lot, 0x3C => fn(*mut u32) -> u32, position.as_mut_ptr());
                f32::from_bits(position[1])
            };
            let height = terrain::add_float(F80::from_f32(y), *zot_height());
            let view = view_utilities().read();
            let manager = developer.prop_manager();
            position[1] = height.to_f32().to_bits();
            let rotation = ViewUtilities::from_ptr(view).rotation();
            call = virtual_call(manager, 0x68, &[prop_type, position.as_mut_ptr() as u32, rotation],
                prop_type, table(manager), false);
            let prop = call.eax as *mut u8;
            call = virtual_call(prop, 0x30, &[3], call.eax, table(prop), false);
            call = virtual_call(prop, 0x18, &[], table(prop), call.edx, false);
            let holder = call.eax as *mut u8;
            result(virtual_call(holder, 0x34, &[0x48E9_5539, kind, 0], call.eax, table(holder), false))
        }
    }
}

port! {
    /// Clears the current zot for a matching request or request zero. A road request posts its message
    /// only after the zot property has been found and read, even if the current zot does not match.
    #[port(address = 0x006C_9A40, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveBuildingZotProp")]
    pub unsafe extern "fastcall" fn fc4p_006c9a40(this: *mut u8, entry_edx: u32, lot: *mut u8, kind: u32) -> u64 {
        unsafe {
            let mut call = virtual_call(lot, 0x0C, &[], table(lot), entry_edx, false);
            let holder = call.eax as *mut u8;
            call = virtual_call(holder, 0x24, &[0xC999_C45E], call.eax, table(holder), false);
            let property = call.eax as *mut u8;
            if !property.is_null() {
                call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                let variant = call.eax as *mut u8;
                call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
                let current = (call.eax as *const u32).read_unaligned();
                if kind == 6 {
                    let returned = post_message(0x4C04_DE69, 2, 0, 0, 0, lot);
                    call.eax = returned as u32;
                    call.edx = (returned >> 32) as u32;
                }
                if kind == 0 || current == kind {
                    if current != 0 {
                        call = direct(0x006C_9780, Conv::Thiscall, this as u32, &[lot as u32, current], call.eax, call.edx);
                    }
                    call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
                }
            }
            result(call)
        }
    }
}

port! {
    /// Demolishes every blocking occupant in the lot's inset world box. The three trailing arguments
    /// are unused. Coordinates shift with 32-bit wrapping before conversion to x87 integers.
    #[port(address = 0x006C_9AD0, owner = "lotdeveloper", name = "cSC4LotDeveloper::ClearLotBlockingObjects")]
    pub unsafe extern "fastcall" fn fc4p_006c9ad0(this: *mut u8, entry_edx: u32, lot: *mut u8, _a: u32, _b: u32, _c: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut rect = [0i32; 4];
            let _ = entry_edx;
            virtual_call(lot, 0x38, &[rect.as_mut_ptr() as u32], table(lot), rect.as_ptr() as u32, false);
            let call = virtual_call(this, 0x44, &[rect.as_ptr() as u32, 0, 1], table(this), rect.as_ptr() as u32, true);
            let top = terrain::add_float(call.st0, *blocking_margin());
            let min = [
                terrain::add_float(F80::from_i32(rect[0].wrapping_shl(4)), *half()).to_f32(),
                0.0,
                terrain::add_float(F80::from_i32(rect[1].wrapping_shl(4)), *half()).to_f32(),
            ];
            let max_x = terrain::subtract_float(F80::from_i32(rect[2].wrapping_add(1).wrapping_shl(4)), *half()).to_f32();
            let top = top.to_f32();
            let max_z = terrain::subtract_float(F80::from_i32(rect[3].wrapping_add(1).wrapping_shl(4)), *half()).to_f32();
            let max = [max_x, top, max_z];
            let (mut head, _) = new_list(rect[3].wrapping_add(1).wrapping_shl(4) as u32);
            let manager = developer.occupant_manager();
            let mut call = virtual_call(manager, 0x90,
                &[&mut head as *mut *mut u8 as u32, min.as_ptr() as u32, max.as_ptr() as u32,
                    BlockingQuery::from_ptr(this).filter() as u32, u32::MAX],
                &mut head as *mut *mut u8 as u32, table(manager), false);
            let sentinel = head;
            let mut current = OccupantNode::from_ptr(sentinel).next();
            call.eax = head as u32;
            while current != sentinel {
                let node = OccupantNode::from_ptr(current);
                let occupant = node.occupant();
                let demolition = developer.demolition();
                call = virtual_call(demolition, 0x20, &[1, occupant as u32, 0, 1, 0, 0, 0],
                    call.eax, table(demolition), false);
                call = virtual_call(occupant, 8, &[], table(occupant), call.edx, false);
                current = node.next();
            }
            result(free_list(&mut head, call.eax, call.edx))
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c9100::PORT,
    fc4p_006c9480::PORT,
    fc4p_006c95e0::PORT,
    fc4p_006c9780::PORT,
    fc4p_006c98c0::PORT,
    fc4p_006c9a40::PORT,
    fc4p_006c9ad0::PORT,
];
