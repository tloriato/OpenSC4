// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant height updates and their foundation refreshes.

use super::maintenance::OccupantNode;
use super::*;

game_data! { static move_tolerance: f32 = 0x00A8_C950; }
game_fn! { #[allow(dead_code)] fn clear_nodes(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3E60; }

port! {
    /// Repositions buildings, props and flora only when their height differs by more than the tolerance.
    /// Building foundations are removed and rebuilt after movement. Network interfaces are released
    /// without moving their occupants. All list payloads are released before the nodes are cleared.
    #[port(address = 0x006C_91F0, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateLotObjectAlitudes", naked)]
    pub unsafe extern "C" fn fc4p_006c91f0() {
        core::arch::naked_asm!(
            "sub esp, 8", "mov [esp], eax", "mov [esp + 4], edx", "mov edx, esp",
            "push dword ptr [esp + 12]", "call {}", "add esp, 8", "ret 4", sym update_altitudes,
        );
    }
}

unsafe extern "fastcall" fn update_altitudes(this: *mut u8, registers: *const [u32; 2], lot: *mut u8) -> u64 {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, 0, this as u32, &[]);
        let entry = registers.read_unaligned();
        call.eax = entry[0];
        call.edx = entry[1];
        let (allocator, returned) = network::allocator(call);
        call = virtual_call(allocator, 0x0C, &[12], table(allocator), returned.edx, false);
        let mut head = call.eax as *mut u8;
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        call = virtual_call(
            lot,
            0xC4,
            &[&mut head as *mut *mut u8 as u32],
            &mut head as *mut *mut u8 as u32,
            table(lot),
            false,
        );
        let developer = LotDeveloper::from_ptr(this);
        let mut current = OccupantNode::from_ptr(head).next();
        call.eax = head as u32;
        while current != head {
            let occupant = OccupantNode::from_ptr(current).occupant();
            call = virtual_call(occupant, 0x1C, &[], call.eax, table(occupant), false);
            let kind = call.eax;
            if matches!(kind, 0x2781_28A0 | 0xA823_821E | 0x7475_8926) {
                let mut position = [0u32; 3];
                call = virtual_call(
                    occupant,
                    0x20,
                    &[position.as_mut_ptr() as u32],
                    table(occupant),
                    call.edx,
                    false,
                );
                call = virtual_call(lot, 0x24, &[], call.eax, table(lot), false);
                let config = call.eax;
                call = virtual_call(lot, 0x2C, &[occupant as u32], table(lot), call.edx, false);
                let mut target = [0u32; 3];
                if call.eax != 0 {
                    let mut facing = current as u32;
                    call = virtual_call(
                        this,
                        0x9C,
                        &[
                            lot as u32,
                            config,
                            call.eax,
                            target.as_mut_ptr() as u32,
                            &mut facing as *mut u32 as u32,
                        ],
                        config,
                        table(this),
                        false,
                    );
                } else {
                    target[0] = position[0];
                    let terrain = developer.terrain();
                    call = virtual_call(
                        terrain,
                        0x34,
                        &[position[0], position[2]],
                        position[0],
                        table(terrain),
                        true,
                    );
                    target[1] = call.st0.to_f32().to_bits();
                    target[2] = position[2];
                }
                let difference =
                    terrain::subtract_float(F80::from_f32(f32::from_bits(position[1])), f32::from_bits(target[1]))
                        .abs();
                let comparison = difference.compare(F80::from_f32(move_tolerance().read()));
                if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                    position[1] = target[1];
                    call = virtual_call(
                        occupant,
                        0x24,
                        &[position.as_mut_ptr() as u32],
                        table(occupant),
                        position[1],
                        false,
                    );
                    let manager = developer.occupant_manager();
                    call = virtual_call(manager, 0x60, &[occupant as u32, 1], call.eax, table(manager), false);
                    if kind == 0x2781_28A0 {
                        call = virtual_call(this, 0xC0, &[occupant as u32], table(this), call.edx, false);
                        let foundation = call.eax as *mut u8;
                        let mut foundation_kind = 0;
                        if !foundation.is_null() {
                            call = virtual_call(foundation, 0x10, &[], call.eax, table(foundation), false);
                            foundation_kind = call.eax;
                            call = virtual_call(this, 0xF0, &[occupant as u32], table(this), call.edx, false);
                        }
                        call = virtual_call(this, 0xE0, &[occupant as u32], call.eax, table(this), false);
                        if call.eax as u8 != 0 {
                            let mut building = core::ptr::null_mut::<u8>();
                            call = virtual_call(
                                occupant,
                                0,
                                &[0x87DD_A3A9, &mut building as *mut *mut u8 as u32],
                                table(occupant),
                                call.edx,
                                false,
                            );
                            if call.eax as u8 != 0 {
                                let saved_table = table(this);
                                let (slot, args) = if foundation_kind != 0 {
                                    (0xEC, [building as u32, foundation_kind])
                                } else {
                                    (0xE8, [building as u32, config])
                                };
                                call = virtual_call(
                                    this,
                                    slot,
                                    &args,
                                    if foundation_kind != 0 { building as u32 } else { config },
                                    saved_table,
                                    false,
                                );
                            }
                            if !building.is_null() {
                                call = virtual_call(building, 8, &[], call.eax, table(building), false);
                            }
                        }
                    }
                }
            } else if kind == 0xC772_BF98 {
                let mut network = core::ptr::null_mut::<u8>();
                call = virtual_call(
                    occupant,
                    0,
                    &[0xA821_EF94, &mut network as *mut *mut u8 as u32],
                    table(occupant),
                    call.edx,
                    false,
                );
                if !network.is_null() {
                    call = virtual_call(network, 8, &[], call.eax, table(network), false);
                }
            }
            call = virtual_call(occupant, 8, &[], table(occupant), call.edx, false);
            current = OccupantNode::from_ptr(current).next();
            call.eax = current as u32;
        }
        call.target = crate::abi::target(0x0041_3E60);
        call.ecx = &mut head as *mut *mut u8 as u32;
        call.count = 0;
        call.take_st0 = 0;
        call_raw(&mut call);
        let sentinel = head;
        let (allocator, call) = network::allocator(call);
        let mut call = virtual_call(allocator, 0x10, &[sentinel as u32], table(allocator), call.edx, false);
        call.eax &= !255;
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006c91f0::PORT];
