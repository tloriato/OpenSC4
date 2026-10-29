// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Normal lot base textures assembled from configuration records.

use super::*;

layout! {
    /// Unsigned cell limits used after positions have been narrowed to bytes.
    pub struct TextureBounds {
        0x4C => cells_z / set_cells_z: u32,
        0x50 => cells_x / set_cells_x: u32,
    }
}

layout! {
    /// Texture configuration fields in a 52-byte object record.
    pub struct TextureObject {
        0x00 => kind / set_kind: u8,
        0x03 => flip / set_flip: u8,
        0x28 => candidates / set_candidates: *mut u32,
        0x2C => candidates_end / set_candidates_end: *mut u32,
    }
}

game_data! { static position_initial: u32 = 0x00B4_9AEC; }
game_data! { static cell_scale: f32 = 0x00AB_1B04; }
game_fn! { #[allow(dead_code)] fn pick_texture(this: *mut u8, lot: *mut u8, candidates: *mut u8) -> u64 = "thiscall" 0x006C_60C0; }
game_fn! { #[allow(dead_code)] fn initialize_cell(this: *mut u8) -> u64 = "thiscall" 0x005C_A770; }
game_fn! { #[allow(dead_code)] fn append_cell(this: *mut u32, cell: *mut u8) -> u64 = "thiscall" 0x004B_2400; }
game_fn! { #[allow(dead_code)] fn free_cells(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }

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
    /// Supplies both entry result words and the caller's mutable lot/configuration argument slots.
    /// The third argument is unused, but remains part of the twelve-byte callee pop.
    #[port(address = 0x006C_A8A0, owner = "lotdeveloper", name = "cSC4LotDeveloper::SetNormalLotBaseTexture", naked)]
    pub unsafe extern "C" fn fc4p_006ca8a0() {
        core::arch::naked_asm!(
            "sub esp, 96", "mov [esp], eax", "mov [esp + 4], edx", "lea eax, [esp + 96]",
            "mov [esp + 8], eax", "mov edx, esp", "call {}", "mov eax, [esp]", "mov edx, [esp + 4]",
            "add esp, 96", "ret 12", sym normal_textures,
        );
    }
}

unsafe extern "fastcall" fn normal_textures(this: *mut u8, registers: *mut [u32; 3]) {
    unsafe {
        let input = registers.read_unaligned();
        let stack = input[2] as *mut u8;
        let returned = set_normal_textures(this, stack.add(4).cast(), stack, input[0], input[1]);
        registers.write_unaligned([returned.eax, returned.edx, input[2]]);
    }
}

unsafe fn set_normal_textures(
    this: *mut u8,
    arguments: *mut u32,
    stack: *mut u8,
    entry_eax: u32,
    entry_edx: u32,
) -> RawCall {
    unsafe {
        let lot = arguments.read_unaligned() as *mut u8;
        let config = arguments.add(1).read_unaligned() as *mut u8;
        let mut call = RawCall::new(Conv::Thiscall, 0, this as u32, &[]);
        call.eax = entry_eax;
        call.edx = entry_edx;
        if lot.is_null() || config.is_null() {
            return call;
        }
        let developer = LotDeveloper::from_ptr(this);
        let mode = developer.force_empty();
        call.eax = (call.eax & !255) | u32::from(mode);
        if mode != 0 {
            call = virtual_call(lot, 0x7C, &[], table(lot), call.edx, false);
            if !matches!(call.eax, 0 | 11 | 12 | 15) {
                return virtual_call(this, 0xAC, &[lot as u32], call.eax, table(this), false);
            }
        }
        let cells = stack.sub(0x38).cast::<u32>();
        for i in 0..3 {
            cells.add(i).write_unaligned(0);
        }
        let rectangle = stack.sub(0x10).cast::<u32>();
        call = virtual_call(lot, 0x38, &[rectangle as u32], table(lot), call.edx, false);
        call = virtual_call(config, 0x5C, &[], call.eax, table(config), false);
        let objects = call.eax as *mut u32;
        if objects.is_null() {
            return call;
        }
        let difference = objects.add(1).read_unaligned().wrapping_sub(objects.read_unaligned()) as i32;
        let product = i64::from(difference) * 0x4EC4_EC4F;
        let high = (product >> 32) as i32 >> 4;
        let count = high.wrapping_add((high as u32 >> 31) as i32);
        call.edx = high as u32;
        call.eax = count as u32;
        if count <= 0 {
            return call;
        }
        arguments.write_unaligned(0);
        let mut remaining = count;
        let mut end = 0u32;
        loop {
            let offset = arguments.read_unaligned();
            let record = objects.read_unaligned().wrapping_add(offset) as *mut u8;
            let object = TextureObject::from_ptr(record);
            if object.kind() == 2 {
                let position = stack.sub(0x2C).cast::<u32>();
                let initial = position_initial().read();
                position.add(1).write_unaligned(initial);
                position.add(2).write_unaligned(initial);
                position.write_unaligned(initial);
                let facing = stack.sub(0x3C).cast::<u32>();
                call = virtual_call(
                    this,
                    0x9C,
                    &[
                        lot as u32,
                        arguments.add(1).read_unaligned(),
                        record as u32,
                        position as u32,
                        facing as u32,
                    ],
                    table(this),
                    position as u32,
                    false,
                );
                let candidates = object.candidates();
                if candidates != object.candidates_end() {
                    let mut texture = 0;
                    let zero_candidate = candidates.read_unaligned() == 0;
                    call.eax = record.add(0x28) as u32;
                    call.edx = candidates as u32;
                    if !zero_candidate {
                        call = direct(
                            0x006C_60C0,
                            Conv::Thiscall,
                            this as u32,
                            &[lot as u32, record.add(0x28) as u32],
                            record.add(0x28) as u32,
                            candidates as u32,
                        );
                        texture = call.eax;
                        call.eax = (call.eax & !255) | u32::from(texture != 0);
                    }
                    if zero_candidate || texture != 0 {
                        // Keep the two padding bytes from the caller's scratch space.
                        let cell = stack.sub(0x20).cast::<u32>();
                        direct(0x005C_A770, Conv::Thiscall, cell as u32, &[], call.eax, call.edx);
                        cell.write_unaligned(texture);
                        let x = terrain::multiply_float(
                            F80::from_f32(f32::from_bits(position.read_unaligned())),
                            cell_scale().read(),
                        )
                        .ftol();
                        let z = terrain::multiply_float(
                            F80::from_f32(f32::from_bits(position.add(2).read_unaligned())),
                            cell_scale().read(),
                        )
                        .ftol();
                        let bytes = cell.cast::<u8>();
                        bytes.add(4).write(x as u8);
                        bytes.add(5).write(z as u8);
                        bytes.add(6).write(facing.read_unaligned() as u8);
                        bytes.add(7).write(object.flip());
                        call.eax = z as u32;
                        call.edx = u32::from(x as u8);
                        let bounds = TextureBounds::from_ptr(this);
                        if u32::from(x as u8) < bounds.cells_x() && u32::from(z as u8) < bounds.cells_z() {
                            direct(
                                0x004B_2400,
                                Conv::Thiscall,
                                cells as u32,
                                &[cell as u32],
                                u32::from(z as u8),
                                u32::from(x as u8),
                            );
                        }
                    }
                }
                end = cells.add(1).read_unaligned();
            }
            let offset = arguments.read_unaligned().wrapping_add(52);
            remaining = remaining.wrapping_sub(1);
            arguments.write_unaligned(offset);
            call.eax = remaining as u32;
            call.edx = offset;
            if remaining == 0 {
                break;
            }
        }
        let mut begin = cells.read_unaligned();
        if begin != end {
            call = virtual_call(
                this,
                0xA4,
                &[lot as u32, arguments.add(1) as u32, 1],
                arguments.add(1) as u32,
                table(this),
                false,
            );
            if call.eax as u8 != 0 {
                let texture = arguments.add(1).read_unaligned() as *mut u8;
                let count = (end as i32).wrapping_sub(begin as i32) >> 4;
                call = virtual_call(
                    texture,
                    0x20,
                    &[begin, count as u32, 0],
                    call.eax,
                    table(texture),
                    false,
                );
                let texture = arguments.add(1).read_unaligned() as *mut u8;
                call = virtual_call(texture, 0x0C, &[], table(texture), call.edx, false);
                let occupant = call.eax as *mut u8;
                call = virtual_call(occupant, 0x80, &[1], call.eax, table(occupant), false);
                let manager = developer.occupant_manager();
                if call.eax as u8 != 0 {
                    call = virtual_call(manager, 0x60, &[occupant as u32, 0], table(manager), call.edx, false);
                    call = virtual_call(occupant, 0x3C, &[0x99EF_1145, 0], call.eax, table(occupant), false);
                } else {
                    call = virtual_call(manager, 0x50, &[occupant as u32, 0], table(manager), call.edx, false);
                }
                let texture = arguments.add(1).read_unaligned() as *mut u8;
                call = virtual_call(texture, 8, &[], call.eax, table(texture), false);
                begin = cells.read_unaligned();
            }
        }
        if begin != 0 {
            call = direct(0x0090_CF63, Conv::Cdecl, 0, &[begin], call.eax, call.edx);
        }
        call
    }
}

pub const PORTS: &[Port] = &[fc4p_006ca8a0::PORT];
