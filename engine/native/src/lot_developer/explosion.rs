// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Turning an exploded lot's building into a detached occupant and rubble decal.

use super::*;

layout! {
    /// The clone's occupant interface is embedded eight bytes after its primary interface.
    struct DetachedOccupant {
        0x08 => occupant_table / set_occupant_table: u32,
    }
}
layout! {
    /// The decal's occupant interface is embedded twelve bytes after its primary interface.
    struct RubbleDecal {
        0x0C => occupant_table / set_occupant_table: u32,
    }
}

game_fn! { #[allow(dead_code)] fn allocate_clone(bytes: u32) -> u64 = "C" 0x005E_DAE0; }
game_fn! { #[allow(dead_code)] fn clone_new(this: *mut u8, flags: u32) -> u64 = "thiscall" 0x005E_F810; }
game_fn! { #[allow(dead_code)] fn clone_from(this: *mut u8, occupant: *mut u8) -> u64 = "thiscall" 0x005E_EB70; }
game_fn! { #[allow(dead_code)] fn allocate_decal(bytes: u32) -> u64 = "C" 0x005E_D6E0; }
game_fn! { #[allow(dead_code)] fn decal_new(this: *mut u8) -> u64 = "thiscall" 0x005E_E360; }
game_fn! { #[allow(dead_code)] fn clear_lot(this: *mut u8, lot: *mut u8, state: u32, immediate: u32, reason: u32) -> u64 = "thiscall" 0x006C_3C50; }
game_fn! { #[allow(dead_code)] fn clear_textures(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_42D0; }
game_fn! { #[allow(dead_code)] fn remove_connections(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_9FB0; }
game_fn! { #[allow(dead_code)] fn rubble_props(this: *mut u8, lot: *mut u8, min: *const f32, max: *const f32, burnt: u32) -> u64 = "thiscall" 0x006C_4BD0; }
game_data! { static prop_manager: *mut u8 = 0x00B4_3D10; }

unsafe fn direct(address: u32, conv: Conv, this: u32, args: &[u32], eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

/// Retains the live x extent while storing its single-precision copy and comparing with stored z.
fn extent_less(x: F80, z: f32) -> bool {
    let status: u16;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{x}]", "fcomp dword ptr [{z}]", "fnstsw ax",
            x = in(reg) &x, z = in(reg) &z, out("ax") status, out("st(0)") _,);
    }
    (status >> 8) & 5 == 1
}

port! {
    /// Supplies the caller's argument words; the lot word also holds the clone-created byte and z extent.
    #[port(address = 0x006C_C250, owner = "lotdeveloper", name = "cSC4LotDeveloper::ExplodeLot", naked)]
    pub unsafe extern "C" fn fc4p_006cc250() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 8", sym explode);
    }
}

unsafe extern "fastcall" fn explode(this: *mut u8, entry_edx: u32, arguments: *mut u32) -> u64 {
    unsafe {
        let lot = arguments.read_unaligned() as *mut u8;
        let mut call = virtual_call(lot, 0x84, &[], table(lot), entry_edx, false);
        if call.eax == 5 {
            return result(call);
        }
        call = virtual_call(lot, 0x84, &[], call.eax, table(lot), false);
        if call.eax == 6 {
            return result(call);
        }
        arguments.cast::<u8>().write(0);
        call = virtual_call(this, 0x98, &[lot as u32], table(this), call.edx, false);
        let building = call.eax as *mut u8;
        let mut clone = core::ptr::null_mut::<u8>();
        let mut position = [0u32; 3];
        let mut min = [0u32; 3];
        let mut max = [0u32; 3];
        let mut scale = 0u32;
        if !building.is_null() {
            call = virtual_call(building, 0x0C, &[], call.eax, table(building), false);
            let occupant = call.eax as *mut u8;
            call = virtual_call(
                occupant,
                0x20,
                &[position.as_mut_ptr() as u32],
                call.eax,
                table(occupant),
                false,
            );
            call = virtual_call(building, 0x0C, &[], call.eax, table(building), false);
            let occupant = call.eax as *mut u8;
            call = virtual_call(
                occupant,
                0x28,
                &[min.as_mut_ptr() as u32, max.as_mut_ptr() as u32],
                call.eax,
                table(occupant),
                false,
            );
            let z = terrain::subtract_float(F80::from_f32(f32::from_bits(max[2])), f32::from_bits(min[2])).to_f32();
            arguments.write_unaligned(z.to_bits());
            let x = terrain::subtract_float(F80::from_f32(f32::from_bits(max[0])), f32::from_bits(min[0]));
            let stored_x = x.to_f32();
            let extent = if extent_less(x, z) { z } else { stored_x };
            let extent = F80::from_f32(extent);
            scale = (extent + extent).to_f32().to_bits();
            call = direct(
                0x005E_DAE0,
                Conv::Cdecl,
                occupant as u32,
                &[0xD8],
                arguments as u32,
                call.edx,
            );
            clone = call.eax as *mut u8;
            if !clone.is_null() {
                call = direct(0x005E_F810, Conv::Thiscall, clone as u32, &[1], call.eax, call.edx);
                clone = call.eax as *mut u8;
            }
            call = virtual_call(clone, 4, &[], call.eax, table(clone), false);
            call = virtual_call(building, 0x0C, &[], table(building), call.edx, false);
            call = direct(
                0x005E_EB70,
                Conv::Thiscall,
                clone as u32,
                &[call.eax],
                call.eax,
                call.edx,
            );
            let interface = clone.add(8);
            call = virtual_call(
                interface,
                0x74,
                &[0x40],
                call.eax,
                DetachedOccupant::from_ptr(clone).occupant_table(),
                false,
            );
            call = virtual_call(interface, 0x0C, &[], table(interface), call.edx, false);
            arguments.cast::<u8>().write(1);
        }
        call = direct(
            0x006C_3C50,
            Conv::Thiscall,
            this as u32,
            &[lot as u32, 6, 0, 4],
            call.eax,
            call.edx,
        );
        call = direct(
            0x006C_42D0,
            Conv::Thiscall,
            this as u32,
            &[lot as u32],
            call.eax,
            call.edx,
        );
        call = direct(
            0x006C_9FB0,
            Conv::Thiscall,
            this as u32,
            &[lot as u32],
            call.eax,
            call.edx,
        );
        call.eax = (call.eax & !255) | u32::from(!clone.is_null());
        if !clone.is_null() {
            let burnt = arguments.add(1).read_unaligned();
            let key = [
                0x7AB5_0E44u32,
                0x1ABE_787D,
                if burnt as u8 == 0 { 0x2A2E_F55D } else { 0x49C8_0EC7 },
            ];
            call = direct(0x005E_D6E0, Conv::Cdecl, this as u32, &[0xA0], call.eax, call.edx);
            let mut decal = call.eax as *mut u8;
            if !decal.is_null() {
                call = direct(0x005E_E360, Conv::Thiscall, decal as u32, &[], call.eax, call.edx);
                decal = call.eax as *mut u8;
            }
            if !decal.is_null() {
                call = virtual_call(decal, 4, &[], call.eax, table(decal), false);
            }
            let interface = decal.add(0x0C);
            virtual_call(
                interface,
                0x0C,
                &[],
                RubbleDecal::from_ptr(decal).occupant_table(),
                call.edx,
                false,
            );
            call = virtual_call(decal, 0x0C, &[key.as_ptr() as u32, scale], scale, table(decal), false);
            call = virtual_call(decal, 0x10, &[0x3F33_3333], call.eax, table(decal), false);
            call = virtual_call(
                interface,
                0x24,
                &[position.as_ptr() as u32],
                table(interface),
                call.edx,
                false,
            );
            call = virtual_call(interface, 0x74, &[0x40], call.eax, table(interface), false);
            let manager = prop_manager().read();
            virtual_call(
                manager,
                0x64,
                &[lot as u32, interface as u32],
                table(manager),
                call.edx,
                false,
            );
            call = direct(
                0x006C_4BD0,
                Conv::Thiscall,
                this as u32,
                &[
                    lot as u32,
                    min.as_ptr() as u32,
                    max.as_ptr() as u32,
                    arguments.add(1).read_unaligned(),
                ],
                min.as_ptr() as u32,
                max.as_ptr() as u32,
            );
            call = virtual_call(decal, 8, &[], call.eax, table(decal), false);
        }
        if !clone.is_null() {
            let manager = prop_manager().read();
            let interface = clone.add(8);
            call = virtual_call(
                manager,
                0x64,
                &[lot as u32, interface as u32],
                table(manager),
                interface as u32,
                false,
            );
            call = virtual_call(clone, 8, &[], table(clone), call.edx, false);
        }
        call.eax = (call.eax & !255) | (arguments.add(1).read_unaligned() & 255);
        if call.eax as u8 != 0 {
            call = virtual_call(lot, 0x90, &[6], call.eax, table(lot), false);
        }
        call = virtual_call(lot, 0xBC, &[0], table(lot), call.edx, false);
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006cc250::PORT];
