// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Conversion between city cells and regional neighbor cells.

use super::{pair, pointer, virtual_call, word};
use crate::port::Port;

game_data! {
    static region_manager: *mut u8 = 0x00B4_3C94;
}
game_data! {
    static current_city: *mut u8 = 0x00B4_3CEC;
}

fn side(x: i32, z: i32, width: i32, height: i32) -> u32 {
    if x < 0 {
        3
    } else if z < 0 {
        0
    } else if x >= width {
        1
    } else if z >= height {
        2
    } else {
        u32::MAX
    }
}

unsafe fn add(destination: *mut i32, value: i32) -> i32 {
    unsafe {
        let result = destination.read_unaligned().wrapping_add(value);
        destination.write_unaligned(result);
        result
    }
}

port! {
    /// Starts at a regional city's position and advances across the selected edge. Division by 64
    /// truncates toward zero. The size getter uses the incoming regional-city argument as scratch.
    #[port(address = 0x006C_E820, owner = "neighbors", name = "cSC4NeighborsSimulator::ConvertConnectionCellToNeighboringRegionCell", naked)]
    pub unsafe extern "C" fn fc4p_006ce820() {
        core::arch::naked_asm!("lea ecx, [esp + 4]", "jmp {}", sym region_cell);
    }
}

unsafe extern "fastcall" fn region_cell(args: *mut u32, _entry_edx: u32) -> u64 {
    unsafe {
        let mut city = args.add(4).read_unaligned() as *mut u8;
        if city.is_null() {
            let manager = region_manager().read();
            city = virtual_call(manager, 0x90, &[], word(manager, 0), 0) as u32 as *mut u8;
        }
        let mut height = 0i32;
        virtual_call(
            city,
            0x1C,
            &[args.add(4) as u32, &mut height as *mut i32 as u32],
            0,
            word(city, 0),
        );
        let x = args.read_unaligned() as i32;
        let z = args.add(1).read_unaligned() as i32;
        let edge = side(
            x,
            z,
            (args.add(4).read_unaligned() as i32).wrapping_shl(6),
            height.wrapping_shl(6),
        );
        let rx = args.add(2).read_unaligned() as *mut i32;
        let rz = args.add(3).read_unaligned() as *mut i32;
        virtual_call(city, 0x14, &[rx as u32, rz as u32], 0, word(city, 0));
        let qx = (args.read_unaligned() as i32) / 64;
        let z = args.add(1).read_unaligned() as i32;
        let qz = z / 64;
        let edx = if z < 0 { 63 } else { 0 };
        match edge {
            3 => {
                add(rx, -1);
                add(rz, qz);
                pair(qz as u32, edx)
            }
            0 => {
                add(rx, qx);
                pair(add(rz, -1) as u32, edx)
            }
            1 => {
                let new_x = add(rx, args.add(4).read_unaligned() as i32);
                add(rz, qz);
                pair(qz as u32, new_x as u32)
            }
            2 => {
                add(rx, qx);
                pair(add(rz, height) as u32, height as u32)
            }
            _ => pair(qz as u32, edx),
        }
    }
}

port! {
    /// Clears all neighbor outputs, locates the regional city across an edge, and maps to its outside
    /// connection cell. Interior cells find our own city and keep the zero connection cell.
    /// The incoming x and city-output argument words hold the z remainder and neighbor position x.
    #[port(address = 0x006C_E920, owner = "neighbors", name = "cSC4NeighborsSimulator::ConvertConnectionCellToNeighborCell", naked)]
    pub unsafe extern "C" fn fc4p_006ce920() {
        core::arch::naked_asm!("lea ecx, [esp + 4]", "jmp {}", sym neighbor_cell);
    }
}

unsafe extern "fastcall" fn neighbor_cell(args: *mut u32, _entry_edx: u32) -> u64 {
    unsafe {
        let city_out = args.add(4).read_unaligned() as *mut *mut u8;
        let nx = args.add(5).read_unaligned() as *mut i32;
        let nz = args.add(6).read_unaligned() as *mut i32;
        city_out.write_unaligned(core::ptr::null_mut());
        nz.write_unaligned(0);
        nx.write_unaligned(0);
        let city = current_city().read();
        let width = virtual_call(city, 0x174, &[], word(city, 0), nx as u32) as u32 as i32;
        let height_result = virtual_call(city, 0x178, &[], width as u32, word(city, 0));
        let height = height_result as u32 as i32;
        let x = args.read_unaligned() as i32;
        let edge = side(x, args.add(1).read_unaligned() as i32, width, height);
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], word(manager, 0), (height_result >> 32) as u32);
        let ours = result as u32 as *mut u8;
        if ours.is_null() {
            return result & !0xFF;
        }
        let rx = args.add(2).read_unaligned() as *mut i32;
        let rz = args.add(3).read_unaligned() as *mut i32;
        virtual_call(ours, 0x14, &[rx as u32, rz as u32], ours as u32, word(ours, 0));
        let (mut sx, mut sz) = (0i32, 0i32);
        virtual_call(
            ours,
            0x1C,
            &[&mut sx as *mut i32 as u32, &mut sz as *mut i32 as u32],
            word(ours, 0),
            0,
        );
        let z = args.add(1).read_unaligned() as i32;
        let (qx, qz, mx, mz) = (x / 64, z / 64, x % 64, z % 64);
        args.write_unaligned(mz as u32);
        let mut edx = if z < 0 { 63 } else { 0 };
        match edge {
            3 => {
                add(rx, -1);
                add(rz, qz);
            }
            0 => {
                add(rx, qx);
                add(rz, -1);
            }
            1 => {
                edx = add(rx, sx) as u32;
                add(rz, qz);
            }
            2 => {
                add(rx, qx);
                add(rz, sz);
                edx = sz as u32;
            }
            _ => {}
        }
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x8C, &[], word(manager, 0), edx);
        let region = result as u32 as *mut u8;
        if region.is_null() {
            return result & !0xFF;
        }
        let result = virtual_call(
            region,
            0x2C,
            &[rx.read_unaligned() as u32, rz.read_unaligned() as u32],
            region as u32,
            word(region, 0),
        );
        let reference = result as u32 as *mut u8;
        if reference.is_null() {
            return result & !0xFF;
        }
        let neighbor = pointer(reference, 0);
        if neighbor.is_null() {
            return result & !0xFF;
        }
        let city_out = args.add(4).read_unaligned() as *mut *mut u8;
        city_out.write_unaligned(neighbor);
        let mut neighbor_z = 0i32;
        virtual_call(
            neighbor,
            0x14,
            &[args.add(4) as u32, &mut neighbor_z as *mut i32 as u32],
            0,
            word(neighbor, 0),
        );
        let neighbor = city_out.read_unaligned();
        let (mut neighbor_sx, mut neighbor_sz) = (0i32, 0i32);
        let result = virtual_call(
            neighbor,
            0x1C,
            &[&mut neighbor_sx as *mut i32 as u32, &mut neighbor_sz as *mut i32 as u32],
            0,
            word(neighbor, 0),
        );
        let dx = rx.read_unaligned().wrapping_sub(args.add(4).read_unaligned() as i32);
        let dz = rz.read_unaligned().wrapping_sub(neighbor_z);
        let nx = args.add(5).read_unaligned() as *mut i32;
        let nz = args.add(6).read_unaligned() as *mut i32;
        let (eax, edx) = match edge {
            1 => {
                let z = dz.wrapping_shl(6).wrapping_add(args.read_unaligned() as i32);
                nx.write_unaligned(-1);
                nz.write_unaligned(z);
                (nz as u32, nx as u32)
            }
            2 => {
                let x = dx.wrapping_shl(6).wrapping_add(mx);
                nx.write_unaligned(x);
                nz.write_unaligned(-1);
                (x as u32, nz as u32)
            }
            3 => {
                nx.write_unaligned(neighbor_sx.wrapping_shl(6));
                let z = dz.wrapping_shl(6).wrapping_add(args.read_unaligned() as i32);
                nz.write_unaligned(z);
                (nz as u32, nx as u32)
            }
            0 => {
                let x = dx.wrapping_shl(6).wrapping_add(mx);
                nx.write_unaligned(x);
                let height = neighbor_sz.wrapping_shl(6);
                nz.write_unaligned(height);
                (nz as u32, height as u32)
            }
            _ => (dx as u32, (result >> 32) as u32),
        };
        pair((eax & !0xFF) | 1, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006ce820::PORT, fc4p_006ce920::PORT];
