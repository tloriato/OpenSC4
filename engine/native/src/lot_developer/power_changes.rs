// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Updating lot power flags, condition and power zots for a cell rectangle.

use super::maintenance::OccupantNode;
use super::*;

layout! {
    /// The power simulator interface retained by the lot developer.
    struct PowerServices {
        0x34 => power / set_power: *mut u8,
    }
}

game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn add_zot(this: *mut u8, lot: *mut u8, kind: u32, zot: u32) -> u64 = "thiscall" 0x006C_98C0; }
game_fn! { #[allow(dead_code)] fn remove_zot(this: *mut u8, lot: *mut u8, zot: u32) -> u64 = "thiscall" 0x006C_9780; }
game_fn! {
    /// Receives the lot in ECX, x in EDX and z in EAX, with no stack arguments.
    #[allow(dead_code)] fn empty_lot_zot() -> u64 = "C" 0x006C_3B10;
}

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
    /// The caller's rectangle word becomes the powered-byte scratch, retaining its upper bytes.
    #[port(address = 0x006C_BF40, owner = "lotdeveloper", name = "cSC4LotDeveloper::DoPowerChanged", naked)]
    pub unsafe extern "C" fn fc4p_006cbf40() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 4", sym power_changed);
    }
}

unsafe extern "fastcall" fn power_changed(this: *mut u8, entry_edx: u32, argument: *mut u32) -> u64 {
    unsafe {
        let mut call = direct(0x0090_CF54, Conv::Cdecl, this as u32, &[12], 0, entry_edx);
        let allocation = call.eax as *mut u8;
        OccupantNode::from_ptr(allocation).set_next(allocation);
        OccupantNode::from_ptr(allocation).set_previous(allocation);
        let mut head = allocation;
        let rect = argument.read_unaligned() as *const u32;
        let developer = LotDeveloper::from_ptr(this);
        let manager = developer.lot_manager();
        call = virtual_call(
            manager,
            0x1C,
            &[
                &mut head as *mut *mut u8 as u32,
                rect.read_unaligned(),
                rect.add(1).read_unaligned(),
                rect.add(2).read_unaligned(),
                rect.add(3).read_unaligned(),
                1,
            ],
            &mut head as *mut *mut u8 as u32,
            table(manager),
            false,
        );
        let query = call.eax != 0;
        call.eax = head as u32;
        if query {
            let mut current = OccupantNode::from_ptr(head).next();
            while current != head {
                let lot = OccupantNode::from_ptr(current).occupant();
                call = virtual_call(lot, 0xC0, &[], call.eax, table(lot), false);
                let building = call.eax as *mut u8;
                let mut rectangle = [0u32; 4];
                argument.cast::<u8>().write(0);
                call = virtual_call(lot, 0x38, &[rectangle.as_mut_ptr() as u32], table(lot), call.edx, false);
                let (mut x, mut z) = (0u32, 0u32);
                if !building.is_null() {
                    call = virtual_call(building, 0x0C, &[], call.eax, table(building), false);
                    let occupant = call.eax as *mut u8;
                    let mut min = [1.0f32; 3];
                    let mut max = [0.0f32; 3];
                    call = virtual_call(
                        occupant,
                        0x28,
                        &[min.as_mut_ptr() as u32, max.as_mut_ptr() as u32],
                        call.eax,
                        table(occupant),
                        false,
                    );
                    let sum = terrain::add_float(F80::from_f32(max[0]), min[0]).to_f32();
                    let city = developer.city();
                    call = virtual_call(city, 0x16C, &[], call.eax, table(city), true);
                    let width = call.st0 + call.st0;
                    let converted = terrain::divide_from_float(width, sum).ftol();
                    x = converted as u32;
                    call.edx = (converted as u64 >> 32) as u32;
                    let sum = terrain::add_float(F80::from_f32(max[2]), min[2]).to_f32();
                    let city = developer.city();
                    call = virtual_call(city, 0x170, &[], table(city), call.edx, true);
                    let width = call.st0 + call.st0;
                    let converted = terrain::divide_from_float(width, sum).ftol();
                    z = converted as u32;
                    call.eax = z;
                    call.edx = (converted as u64 >> 32) as u32;
                } else {
                    call = virtual_call(
                        lot,
                        0x54,
                        &[&mut x as *mut u32 as u32, &mut z as *mut u32 as u32],
                        &mut z as *mut u32 as u32,
                        table(lot),
                        false,
                    );
                }
                let service = developer.building_development();
                call = virtual_call(service, 0xDC, &[], call.eax, table(service), false);
                let mut powered = call.eax as u8 != 0;
                if !powered {
                    let service = PowerServices::from_ptr(this).power();
                    call = virtual_call(service, 0x94, &[x, z], table(service), x, false);
                    powered = call.eax as u8 != 0;
                }
                if powered {
                    argument.cast::<u8>().write(1);
                }
                call = virtual_call(lot, 0x108, &[], table(lot), call.edx, false);
                if call.eax as u8 != u8::from(powered) {
                    let flags = argument.read_unaligned();
                    call = virtual_call(lot, 0x10C, &[flags], flags, table(lot), false);
                    call = virtual_call(lot, 0x8C, &[], call.eax, table(lot), false);
                    if (!powered && call.eax == 0) || (powered && call.eax == 1) {
                        call = virtual_call(lot, 0x90, &[u32::from(!powered)], table(lot), call.edx, false);
                    }
                }
                let needs = if building.is_null() {
                    true
                } else {
                    call = virtual_call(lot, 0x104, &[], call.eax, table(lot), false);
                    call.eax as u8 != 0
                };
                if !powered && needs {
                    let mut suitable = !building.is_null();
                    if !suitable {
                        call = direct(0x006C_3B10, Conv::Thiscall, lot as u32, &[], z, x);
                        suitable = call.eax as u8 != 0;
                    }
                    if suitable {
                        call = direct(
                            0x006C_98C0,
                            Conv::Thiscall,
                            this as u32,
                            &[lot as u32, 0x0FD1_0000, 4],
                            call.eax,
                            call.edx,
                        );
                    }
                } else {
                    call = virtual_call(lot, 0x0C, &[], table(lot), call.edx, false);
                    let holder = call.eax as *mut u8;
                    call = virtual_call(holder, 0x24, &[0xC999_C45E], call.eax, table(holder), false);
                    let property = call.eax as *mut u8;
                    if !property.is_null() {
                        call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                        let variant = call.eax as *mut u8;
                        call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
                        if (call.eax as *const u32).read_unaligned() == 4 {
                            call = direct(
                                0x006C_9780,
                                Conv::Thiscall,
                                this as u32,
                                &[lot as u32, 4],
                                call.eax,
                                call.edx,
                            );
                            call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
                        }
                    }
                }
                current = OccupantNode::from_ptr(current).next();
                call.eax = head as u32;
            }
        }
        let mut current = OccupantNode::from_ptr(head).next();
        while current != head {
            let node = OccupantNode::from_ptr(current);
            let next = node.next();
            let lot = node.occupant();
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
            call.eax = head as u32;
        }
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        result(direct(
            0x0090_CF63,
            Conv::Cdecl,
            head as u32,
            &[head as u32],
            head as u32,
            call.edx,
        ))
    }
}

pub const PORTS: &[Port] = &[fc4p_006cbf40::PORT];
