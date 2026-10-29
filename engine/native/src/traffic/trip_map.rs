// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Propagating trip lengths to lots without their own road access.

use super::*;
use super::finish_passes::{check_budget, paced_return, pass_port, scope};
use super::network::registers;
use crate::abi::{Conv, RawCall};

layout! {
    /// Trip-map cursor, grid and saved location.
    pub struct TripMapSim {
        0x44 => length_grid / set_length_grid: *mut u8,
        0x904 => iterator / set_iterator: *mut u8,
        0x90C => phase_x / set_phase_x: i16,
        0x90E => phase_z / set_phase_z: i16,
    }
}
game_fn! { #[allow(dead_code)] fn budget_expired(this: *mut u32) -> u32 = "thiscall" 0x0068_BF80; }
game_data! { static pacing: [u32; 2] = 0x00B4_B324; }
game_data! { static initialized: u32 = 0x00B4_B32C; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }
game_data! { static direction_x: [i32; 4] = 0x00AB_325C; }
game_data! { static direction_z: [i32; 4] = 0x00AB_326C; }

unsafe fn location(lot: *mut u8) -> (i32, i32) {
    let (mut x, mut z) = (0i32, 0i32);
    unsafe { vcall!(lot, 0x54 => fn(*mut i32, *mut i32), &mut x, &mut z); }
    (x, z)
}
unsafe fn length(vector: *mut u8, x: i32, z: i32) -> u8 {
    unsafe {
        let row = word(word(vector, 0) as *mut u8, x.wrapping_mul(12) as usize) as *mut u8;
        row.wrapping_offset(z as isize).read()
    }
}

/// Search one radius at a time, continuing only the directions that still contain the same zone.
unsafe fn donor(sim: TrafficSim, manager: *mut u8, vector: *mut u8, lot: *mut u8, zone: u32) -> u8 {
    unsafe {
        let link = vcall!(lot, 0xEC => fn() -> *mut u8);
        if !link.is_null() {
            let (x, z) = location(link);
            return length(vector, x, z);
        }
        let (x, z) = location(lot);
        let mut alive = 15u8;
        let mut distance = 0u8;
        let mut direction = 3u8;
        while alive != 0 {
            loop {
                direction = direction.wrapping_add(1);
                if direction == 4 { direction = 0; distance = distance.wrapping_add(1); }
                if alive & 1u8.wrapping_shl(u32::from(direction)) != 0 { break; }
            }
            let dx = read_unaligned(direction_x().cast::<i32>().add(usize::from(direction)));
            let dz = read_unaligned(direction_z().cast::<i32>().add(usize::from(direction)));
            let nx = x.wrapping_add(dx.wrapping_mul(i32::from(distance)));
            let nz = z.wrapping_add(dz.wrapping_mul(i32::from(distance)));
            if nx < sim.min_cell_x() || nz < sim.min_cell_z() || nx > sim.max_cell_x() || nz > sim.max_cell_z() {
                alive &= !1u8.wrapping_shl(u32::from(direction));
                continue;
            }
            let other = vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, nx, nz, 0);
            if other.is_null() || vcall!(other, 0x7C => fn() -> u32) != zone {
                alive &= !1u8.wrapping_shl(u32::from(direction));
            } else if other != lot && vcall!(other, 0x70 => fn() -> u32) as u8 != 0 {
                return length(vector, nx, nz);
            }
        }
        0
    }
}

pass_port!(0x0070_FEF0, fc4p_0070fef0, "cSC4TrafficSimulator::WorkOnFinishTripMap", finish_trip_map);
unsafe extern "fastcall" fn finish_trip_map(this: *mut u8, out: *mut Registers, budget: *mut u32, scope_pointer: *mut [u32; 6]) {
    unsafe {
        scope_pointer.write(scope(budget, pacing(), initialized()));
        let scope = &mut *scope_pointer;
        let sim = TripMapSim::from_ptr(this);
        let vector = vcall!(sim.length_grid(), 0x60 => fn() -> *mut u8);
        let manager = read_unaligned(lot_manager());
        let mut work = 0u32;
        let mut result = Registers::default();
        let completed = loop {
            work = work.wrapping_add(scope[3]);
            scope[2] = work;
            if work & 1023 == 0 {
                result = check_budget(scope, result);
                if result.eax as u8 != 0 { result.ecx = 0; break false; }
            }
            let iterator = sim.iterator();
            result.ecx = iterator as u32;
            let list = word(iterator, 4) as *mut u8;
            result.edx = list as u32;
            if list.is_null() { break true; }
            let node = word(iterator, 0) as *mut u8;
            result.eax = node as u32;
            if node as u32 == word(list, 0) { break true; }
            store(iterator, 0, word(node, 0));
            let lot = word(node, 8) as *mut u8;
            let table = word(lot, 0) as *mut u8;
            let mut call = RawCall::new(Conv::Thiscall, word(table, 0x30) as usize, lot as u32, &[]);
            call.eax = node as u32;
            call.edx = word(node, 0);
            result = registers(call);
            let args = [result.eax & !0x80];
            let mut call = RawCall::new(Conv::Thiscall, word(table, 0x34) as usize, lot as u32, &args);
            call.eax = args[0];
            call.edx = result.edx;
            result = registers(call);
            let (x, z) = location(lot);
            sim.set_phase_x(x as i16);
            sim.set_phase_z(z as i16);
            let zone = vcall!(lot, 0x7C => fn() -> u32);
            if zone != 8 && zone != 9 { continue; }
            if vcall!(lot, 0x70 => fn() -> u32) as u8 != 0 { continue; }
            let value = donor(TrafficSim::from_ptr(this), manager, vector, lot, zone);
            let mut rect = [0i32; 4];
            vcall!(lot, 0x38 => fn(*mut i32), rect.as_mut_ptr());
            let mut x = rect[0];
            while x <= rect[2] {
                let mut z = rect[1];
                while z <= rect[3] {
                    let row = word(word(vector, 0) as *mut u8, x.wrapping_mul(12) as usize) as *mut u8;
                    row.wrapping_offset(z as isize).write(value);
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            work = scope[2];
        };
        out.write(paced_return(pacing(), work, completed, false, !completed, result));
    }
}

pub const PORTS: &[Port] = &[fc4p_0070fef0::PORT];

#[cfg(test)]
mod tests;
