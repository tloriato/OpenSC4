// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! World positions and rotated footprints for individual configuration objects.

use super::terrain::{add_float, multiply_float, subtract_float};
use super::*;

game_data! { static inverse_cell: f32 = 0x00AB_1B04; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static cell_width: f32 = 0x00A8_D45C; }

layout! {
    /// Position, footprint and orientation fields in a 52-byte configuration record.
    pub struct PositionRecord {
        0x00 => kind / set_kind: u8,
        0x02 => orientation / set_orientation: u8,
        0x04 => x / set_x: f32,
        0x08 => y / set_y: f32,
        0x0C => z / set_z: f32,
        0x10 => x0 / set_x0: f32,
        0x14 => z0 / set_z0: f32,
        0x18 => x1 / set_x1: f32,
        0x1C => z1 / set_z1: f32,
    }
}

port! {
    /// Supplies the original argument words, reused for corner coordinates, snapped cells and water flags.
    #[port(address = 0x006C_4EF0, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetPositionForLotConfigurationObject", naked)]
    pub unsafe extern "C" fn fc4p_006c4ef0() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 20", sym object_position);
    }
}

unsafe extern "fastcall" fn object_position(this: *mut u8, _entry_edx: u32, args: *mut u32) -> u64 {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let lot = args.read_unaligned() as *mut u8;
        let mut rect = [0i32; 4];
        vcall!(lot, 0x38 => fn(*mut i32), rect.as_mut_ptr());
        let (mut x0, mut z0, mut x1) = (0.0f32, 0.0f32, 0.0f32);
        vcall!(developer.city(), 0x180 => fn(i32, i32, *mut f32, *mut f32), rect[0], rect[1], &mut x0, &mut z0);
        vcall!(developer.city(), 0x180 => fn(i32, i32, *mut f32, *mut f32),
            rect[2].wrapping_add(1), rect[3].wrapping_add(1), &mut x1, args.cast::<f32>());
        let facing = vcall!(lot, 0x5C => fn() -> i32);
        let record = PositionRecord::from_ptr(args.add(2).read_unaligned() as *mut u8);
        let mut oz = record.z();
        let mut ox = F80::from_f32(record.x());
        let oy = record.y();
        if record.kind() == 2 {
            let cell = multiply_float(ox, inverse_cell().read()).ftol() as i32;
            args.add(2).write_unaligned(cell as u32);
            ox = multiply_float(add_float(F80::from_i32(cell), half().read()), cell_width().read());
            let cell = multiply_float(F80::from_f32(oz), inverse_cell().read()).ftol() as i32;
            args.add(2).write_unaligned(cell as u32);
            oz = multiply_float(add_float(F80::from_i32(cell), half().read()), cell_width().read()).to_f32();
        }
        let position = args.add(3).read_unaligned() as *mut f32;
        let z1 = f32::from_bits(args.read_unaligned());
        let mut footprint = [0.0f32; 4];
        match facing {
            0 => {
                position.write_unaligned((F80::from_f32(x1) - ox).to_f32());
                position
                    .add(2)
                    .write_unaligned(subtract_float(F80::from_f32(z1), oz).to_f32());
                footprint[2] = subtract_float(F80::from_f32(x1), record.x0()).to_f32();
                footprint[3] = subtract_float(F80::from_f32(z1), record.z0()).to_f32();
                footprint[0] = subtract_float(F80::from_f32(x1), record.x1()).to_f32();
                footprint[1] = subtract_float(F80::from_f32(z1), record.z1()).to_f32();
            }
            1 => {
                position.write_unaligned(add_float(F80::from_f32(oz), x0).to_f32());
                position.add(2).write_unaligned((F80::from_f32(z1) - ox).to_f32());
                footprint[2] = add_float(F80::from_f32(x0), record.z1()).to_f32();
                footprint[3] = subtract_float(F80::from_f32(z1), record.x0()).to_f32();
                footprint[0] = add_float(F80::from_f32(x0), record.z0()).to_f32();
                footprint[1] = subtract_float(F80::from_f32(z1), record.x1()).to_f32();
            }
            2 => {
                position.write_unaligned(add_float(ox, x0).to_f32());
                position
                    .add(2)
                    .write_unaligned(add_float(F80::from_f32(oz), z0).to_f32());
                footprint[2] = add_float(F80::from_f32(x0), record.x1()).to_f32();
                footprint[3] = add_float(F80::from_f32(z0), record.z1()).to_f32();
                footprint[0] = add_float(F80::from_f32(x0), record.x0()).to_f32();
                footprint[1] = add_float(F80::from_f32(z0), record.z0()).to_f32();
            }
            3 => {
                position.write_unaligned(subtract_float(F80::from_f32(x1), oz).to_f32());
                position.add(2).write_unaligned(add_float(ox, z0).to_f32());
                footprint[2] = subtract_float(F80::from_f32(x1), record.z0()).to_f32();
                footprint[3] = add_float(F80::from_f32(z0), record.x1()).to_f32();
                footprint[0] = subtract_float(F80::from_f32(x1), record.z1()).to_f32();
                footprint[1] = add_float(F80::from_f32(z0), record.x0()).to_f32();
            }
            _ => {
                position.write_unaligned(0.0);
                position.add(2).write_unaligned(0.0);
                footprint = [0.0, 1.0, 0.0, 1.0];
            }
        }
        if vcall!(lot, 0x30 => fn() -> u32) as u8 & 1 != 0 {
            position.add(1).write_unaligned(vcall_st0!(lot, 0x40).to_f32());
        } else {
            let foundation = vcall!(this, 0xCC => fn(*mut u8) -> *mut u8, lot);
            if !foundation.is_null() {
                let occupant = vcall!(foundation, 0x0C => fn() -> *mut u8);
                vcall!(occupant, 0x20 => fn(*mut f32), footprint.as_mut_ptr());
                position.add(1).write_unaligned(footprint[1]);
            } else {
                let config = args.add(1).read_unaligned() as *mut u8;
                let water = if config.is_null() {
                    0
                } else {
                    vcall!(config, 0x38 => fn() -> u32) as u8
                };
                args.add(3).cast::<u8>().write(water);
                let kind = record.kind();
                let altitude = if kind == 1 || kind == 2 || kind == 4 {
                    vcall_st0!(
                        developer.terrain(),
                        0x34,
                        position.read_unaligned(),
                        position.add(2).read_unaligned()
                    )
                } else {
                    vcall_st0!(this, 0x40, footprint.as_mut_ptr(), 0u32, args.add(3).read_unaligned())
                };
                position.add(1).write_unaligned(altitude.to_f32());
            }
        }
        position
            .add(1)
            .write_unaligned(add_float(F80::from_f32(oy), position.add(1).read_unaligned()).to_f32());
        let output = args.add(4).read_unaligned() as *mut i32;
        output.write_unaligned(facing);
        let orientation = facing.wrapping_add(i32::from(record.orientation())) % 4;
        output.write_unaligned(orientation);
        (u64::from(orientation as u32) << 32) | u64::from(output as u32)
    }
}

pub const PORTS: &[Port] = &[fc4p_006c4ef0::PORT];
