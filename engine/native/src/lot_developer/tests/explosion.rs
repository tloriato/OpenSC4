// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::explosion::*;

slot!(explosion_building, lot);
slot!(occupant_position, position);
slot!(occupant_bounds, min, max);
slot!(detach_occupant);
slot!(occupant_flags, flags);
slot!(initialize_decal);
slot!(decal_key, key, scale);
slot!(decal_opacity, opacity);
slot!(decal_position, position);
slot!(attach_rubble, lot, occupant);

unsafe extern "thiscall" fn new_clone(this: *mut u8, flags: u32) -> u64 {
    run(this, "new_clone", &[flags])
}
unsafe extern "thiscall" fn copy_clone(this: *mut u8, source: u32) -> u64 {
    run(this, "copy_clone", &[source])
}
unsafe extern "thiscall" fn new_decal(this: *mut u8) -> u64 {
    run(this, "new_decal", &[])
}
unsafe extern "thiscall" fn explode_clear(this: *mut u8, lot: u32, state: u32, immediate: u32, reason: u32) -> u64 {
    run(this, "explode_clear", &[lot, state, immediate, reason])
}
unsafe extern "thiscall" fn explode_textures(this: *mut u8, lot: u32) -> u64 {
    run(this, "explode_textures", &[lot])
}
unsafe extern "thiscall" fn explode_connections(this: *mut u8, lot: u32) -> u64 {
    run(this, "explode_connections", &[lot])
}
unsafe extern "thiscall" fn explode_props(this: *mut u8, lot: u32, min: u32, max: u32, burnt: u32) -> u64 {
    run(this, "explode_props", &[lot, min, max, burnt])
}

unsafe fn invoke(this: *mut u8, lot: *mut u8, burnt: u32) -> [u32; 4] {
    let mut frame = [lot as u32, burnt, 0, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 4]", "push dword ptr [edi]", "call {function}",
            "mov [edi + 8], eax", "mov [edi + 12], edx",
            "mov eax, [esp - 8]", "mov [edi + 16], eax",
            "mov eax, [esp - 4]", "mov [edi + 20], eax",
            function = sym fc4p_006cc250, in("edi") frame.as_mut_ptr(), in("ecx") this,
            in("edx") 0x5566_7788u32, clobber_abi("C"),
        );
    }
    frame[2..].try_into().unwrap()
}

#[test]
fn explosion_rechecks_state_and_leaves_demolishing_or_destroyed_lots_alone() {
    let mut developer = Object::new(0x108);
    let mut lot_object = mock!(0x84 => habitation_state);
    let (this, lot) = (developer.ptr(), lot_object.ptr());
    for first in [5, 3] {
        expect(lot, "habitation_state", vec![], (RETURN & !0xFFFF_FFFF) | first);
        if first != 5 {
            expect(lot, "habitation_state", vec![], (RETURN & !0xFFFF_FFFF) | 6);
        }
        assert_eq!(
            unsafe { invoke(this, lot, 0xAABB_CC80) },
            [
                if first == 5 { 5 } else { 6 },
                (RETURN >> 32) as u32,
                lot as u32,
                0xAABB_CC80
            ]
        );
        finished();
    }
}

#[test]
fn explosion_without_building_still_clears_lot_textures_connections_and_building_type() {
    let _clear = fake(0x006C_3C50, explode_clear as *const () as usize);
    let _textures = fake(0x006C_42D0, explode_textures as *const () as usize);
    let _connections = fake(0x006C_9FB0, explode_connections as *const () as usize);
    let mut developer = mock!(0x98 => explosion_building);
    let mut lot_object = mock!(0x84 => habitation_state, 0x90 => set_condition, 0xBC => set_growth_type);
    let (this, lot) = (developer.ptr(), lot_object.ptr());
    for burnt in [0u32, 1, 0x100, 0xAABB_CC80] {
        expect(lot, "habitation_state", vec![], 3);
        expect(lot, "habitation_state", vec![], 3);
        expect(this, "explosion_building", vec![p(lot)], 0);
        expect(this, "explode_clear", vec![p(lot), w(6), w(0), w(4)], RETURN);
        expect(this, "explode_textures", vec![p(lot)], RETURN);
        expect(this, "explode_connections", vec![p(lot)], RETURN);
        if burnt as u8 != 0 {
            expect(lot, "set_condition", vec![w(6)], RETURN);
        }
        expect(lot, "set_growth_type", vec![w(0)], RETURN);
        assert_eq!(
            unsafe { invoke(this, lot, burnt) },
            [RETURN as u32, (RETURN >> 32) as u32, lot as u32 & !255, burnt]
        );
        finished();
    }
}

#[test]
fn explosion_clones_before_clearing_then_attaches_decal_props_and_clone_in_order() {
    let _fpu = GameFpu::new();
    let _clone_alloc = fake(0x005E_DAE0, operation_allocate as *const () as usize);
    let _decal_alloc = fake(0x005E_D6E0, operation_allocate as *const () as usize);
    let _clone_new = fake(0x005E_F810, new_clone as *const () as usize);
    let _clone_copy = fake(0x005E_EB70, copy_clone as *const () as usize);
    let _decal_new = fake(0x005E_E360, new_decal as *const () as usize);
    let _clear = fake(0x006C_3C50, explode_clear as *const () as usize);
    let _textures = fake(0x006C_42D0, explode_textures as *const () as usize);
    let _connections = fake(0x006C_9FB0, explode_connections as *const () as usize);
    let _props = fake(0x006C_4BD0, explode_props as *const () as usize);
    unsafe {
        for (extent_x, extent_z, size, burnt) in [
            (10.0, 20.0, 40.0, 0),
            (20.0, 10.0, 40.0, 1),
            (10.0, 10.0, 20.0, 0x100),
            (20.0, 10.0, 40.0, 0xAABB_CC80),
            (f32::NAN, 10.0, f32::NAN, 1),
            (10.0, f32::NAN, 20.0, 1),
            (-1.0, -2.0, -2.0, 1),
        ] {
            let mut developer = mock!(0x98 => explosion_building);
            let mut lot_object = mock!(0x84 => habitation_state, 0x90 => set_condition, 0xBC => set_growth_type);
            let mut building_object = mock!(0x0C => get_occupant);
            let mut occupant_object = mock!(0x20 => occupant_position, 0x28 => occupant_bounds);
            let mut clone_object = mock!(4 => add_ref, 8 => release);
            let clone_interface_table = mock!(0x74 => occupant_flags, 0x0C => detach_occupant);
            let mut decal_object = mock!(4 => add_ref, 8 => release, 0x0C => decal_key, 0x10 => decal_opacity);
            let decal_interface_table = mock!(0x0C => initialize_decal, 0x24 => decal_position, 0x74 => occupant_flags);
            let mut first_manager = mock!(0x64 => attach_rubble);
            let mut second_manager = mock!(0x64 => attach_rubble);
            let (this, lot, building, occupant, clone, decal, manager1, manager2) = (
                developer.ptr(),
                lot_object.ptr(),
                building_object.ptr(),
                occupant_object.ptr(),
                clone_object.ptr(),
                decal_object.ptr(),
                first_manager.ptr(),
                second_manager.ptr(),
            );
            clone
                .add(8)
                .cast::<usize>()
                .write(clone_interface_table._vtable.as_ptr() as usize);
            decal
                .add(12)
                .cast::<usize>()
                .write(decal_interface_table._vtable.as_ptr() as usize);
            let mut manager = manager1;
            let _manager = fake_data(0x00B4_3D10, (&mut manager as *mut *mut u8).cast());
            let min = [0.0f32.to_bits(), 1.0f32.to_bits(), 0.0f32.to_bits()];
            let max = [f32::to_bits(extent_x), 3.0f32.to_bits(), f32::to_bits(extent_z)];
            let position = [2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits()];
            expect(lot, "habitation_state", vec![], 3);
            expect(lot, "habitation_state", vec![], 3);
            expect(this, "explosion_building", vec![p(lot)], building as u64);
            expect(building, "get_occupant", vec![], occupant as u64);
            expect(occupant, "occupant_position", vec![out(&position)], RETURN);
            expect(building, "get_occupant", vec![], occupant as u64);
            expect(occupant, "occupant_bounds", vec![out(&min), out(&max)], RETURN);
            expect(core::ptr::null_mut(), "operation_allocate", vec![w(0xD8)], clone as u64);
            expect(clone, "new_clone", vec![w(1)], clone as u64);
            expect(clone, "add_ref", vec![], RETURN);
            expect(building, "get_occupant", vec![], occupant as u64);
            expect(clone, "copy_clone", vec![p(occupant)], RETURN);
            expect(clone.add(8), "occupant_flags", vec![w(0x40)], RETURN);
            expect(clone.add(8), "detach_occupant", vec![], RETURN);
            expect(this, "explode_clear", vec![p(lot), w(6), w(0), w(4)], RETURN);
            expect(this, "explode_textures", vec![p(lot)], RETURN);
            expect(this, "explode_connections", vec![p(lot)], RETURN);
            expect(core::ptr::null_mut(), "operation_allocate", vec![w(0xA0)], decal as u64);
            expect(decal, "new_decal", vec![], decal as u64);
            expect(decal, "add_ref", vec![], RETURN);
            expect(decal.add(12), "initialize_decal", vec![], RETURN);
            expect(
                decal,
                "decal_key",
                vec![
                    Arg::In(vec![
                        0x7AB5_0E44,
                        0x1ABE_787D,
                        if burnt as u8 == 0 { 0x2A2E_F55D } else { 0x49C8_0EC7 },
                    ]),
                    w(f32::to_bits(size)),
                ],
                RETURN,
            );
            expect(decal, "decal_opacity", vec![w(0x3F33_3333)], RETURN);
            expect(
                decal.add(12),
                "decal_position",
                vec![Arg::In(position.to_vec())],
                RETURN,
            );
            expect(decal.add(12), "occupant_flags", vec![w(0x40)], RETURN);
            expect(manager1, "attach_rubble", vec![p(lot), p(decal.add(12))], RETURN);
            expect(
                this,
                "explode_props",
                vec![p(lot), Arg::In(min.to_vec()), Arg::In(max.to_vec()), w(burnt)],
                RETURN,
            );
            edit_last((&mut manager as *mut *mut u8).cast(), manager2 as u32);
            expect(decal, "release", vec![], RETURN);
            expect(manager2, "attach_rubble", vec![p(lot), p(clone.add(8))], RETURN);
            expect(clone, "release", vec![], RETURN);
            if burnt as u8 != 0 {
                expect(lot, "set_condition", vec![w(6)], RETURN);
            }
            expect(lot, "set_growth_type", vec![w(0)], RETURN);
            assert_eq!(
                invoke(this, lot, burnt),
                [
                    RETURN as u32,
                    (RETURN >> 32) as u32,
                    (extent_z.to_bits() & !255) | 1,
                    burnt
                ]
            );
            finished();
        }
    }
}
