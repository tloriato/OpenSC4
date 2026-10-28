// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Re-create lots on the surviving runs of a removed lot.

use super::{Port, ZoneDeveloper};
use crate::abi::{Conv, RawCall};
use core::ptr::{read_unaligned, write_unaligned};

game_fn! {fn allocate(size:u32)->*mut u8 = "C" 0x0090_CF54;}
game_fn! {fn free(node:*mut u8) = "C" 0x0090_CF63;}
game_fn! {fn append(list:*mut *mut u8,value:*const i32) = "thiscall" 0x0042_7640;}
game_fn! {#[allow(dead_code)] fn destroy_list(list:*mut *mut u8)->u64 = "thiscall" 0x0056_50A0;}
game_fn! {#[allow(clippy::too_many_arguments)] fn face_road(this:*mut u8,x:i32,z:i32,w:i32,h:i32,list:*mut *mut u8,out:*mut *mut u8)->u32 = "thiscall" 0x0072_D200;}
game_fn! {fn includes(region:*mut u8,x:i32,z:i32)->u32 = "thiscall" 0x0046_6610;}
game_fn! {fn zone_matches(this:*mut u8,x:i32,z:i32)->u32 = "thiscall" 0x0072_A040;}
game_fn! {fn occupied(this:*mut u8,x:i32,z:i32)->u32 = "thiscall" 0x0072_C050;}
game_data! {static facing_zero:i32 = 0x00AB_36C4;}
game_data! {static facing_one:i32 = 0x00AB_36C8;}
game_data! {static facing_two:i32 = 0x00AB_36CC;}
game_data! {static facing_three:i32 = 0x00AB_36D0;}

unsafe fn empty_list() -> *mut u8 {
    unsafe {
        let header = allocate(12);
        write_unaligned(header.cast::<*mut u8>(), header);
        write_unaligned(header.add(4).cast::<*mut u8>(), header);
        header
    }
}

unsafe fn buildable(dev: ZoneDeveloper, x: i32, z: i32) -> bool {
    unsafe {
        if x < 0 || x >= dev.cells_x() || z < 0 || z >= dev.cells_z() {
            return false;
        }
        let restriction = dev.restriction();
        if !restriction.is_null() && includes(restriction, x, z) as u8 == 0 {
            return false;
        }
        if vcall!(dev.as_ptr(),0x24 => fn(i32,i32)->u32,x,z) as u8 != 0 {
            return false;
        }
        if dev.real_mode() != 0 && zone_matches(dev.as_ptr(), x, z) as u8 == 0 {
            return false;
        }
        if vcall!(dev.as_ptr(),0x20 => fn(i32,i32,u32)->u32,x,z,0x1F4F) as u8 != 0 {
            return false;
        }
        occupied(dev.as_ptr(), x, z) as u8 == 0
    }
}

unsafe fn create_strip(dev: ZoneDeveloper, rect: [i32; 4], list: *mut *mut u8, out: *mut *mut u8) -> u8 {
    unsafe {
        let result = face_road(dev.as_ptr(), rect[0], rect[1], rect[2], rect[3], list, out) as u8;
        let lot = read_unaligned(out);
        if !lot.is_null() {
            vcall!(lot,8 => fn());
            write_unaligned(out, core::ptr::null_mut());
        }
        result
    }
}

port! {
    /// Zones 7–9 place one lot per cell, z outer and x inner, using a fresh empty orientation list.
    /// Other zones scan away from the facing side and place each contiguous buildable run. Release each
    /// returned lot. AL is the last strip result (zero if none), or one for the per-cell path.
    /// ECX retains the final list destructor's result or the per-cell path's next row coordinate.
    #[port(address = 0x0072_D440, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CreateLotRemnants", naked)]
    pub unsafe extern "C" fn fc4p_0072d440() {
        core::arch::naked_asm!("sub esp, 4", "mov edx, esp",
            "push dword ptr [esp + 24]", "push dword ptr [esp + 24]", "push dword ptr [esp + 24]",
            "push dword ptr [esp + 24]", "push dword ptr [esp + 24]", "call {body}",
            "mov ecx, [esp]", "add esp, 4", "ret 20", body = sym create_lot_remnants);
    }
}

unsafe extern "fastcall" fn create_lot_remnants(
    this: *mut u8,
    out_ecx: *mut u32,
    x: i32,
    z: i32,
    width: i32,
    height: i32,
    facing: u32,
) -> u32 {
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this);
        let x_end = x.wrapping_add(width).wrapping_sub(1);
        let z_end = z.wrapping_add(height).wrapping_sub(1);
        if (7..=9).contains(&(dev.current_zone() as i32)) {
            let mut row = 0i32;
            while row < height {
                let mut column = 0i32;
                while column < width {
                    let header = empty_list();
                    let mut list = header;
                    let mut out = core::ptr::null_mut();
                    face_road(
                        this,
                        x.wrapping_add(column),
                        z.wrapping_add(row),
                        1,
                        1,
                        &mut list,
                        &mut out,
                    );
                    if !out.is_null() {
                        vcall!(out,8 => fn());
                    }
                    let mut node = read_unaligned(header.cast::<*mut u8>());
                    while node != header {
                        let next = read_unaligned(node.cast::<*mut u8>());
                        free(node);
                        node = next;
                    }
                    write_unaligned(header.cast::<*mut u8>(), header);
                    write_unaligned(header.add(4).cast::<*mut u8>(), header);
                    free(header);
                    column = column.wrapping_add(1);
                }
                row = row.wrapping_add(1);
            }
            write_unaligned(
                out_ecx,
                if height > 0 {
                    z.wrapping_add(height) as u32
                } else {
                    width as u32
                },
            );
            return 1;
        }
        let value = match facing {
            0 => facing_zero(),
            1 => facing_one(),
            2 => facing_two(),
            3 => facing_three(),
            _ => {
                write_unaligned(out_ecx, width as u32);
                return facing & !255;
            }
        };
        let mut list = empty_list();
        append(&mut list, value);
        let mut out = core::ptr::null_mut();
        let mut result = 0u8;
        let horizontal = facing == 1 || facing == 3;
        let descending = facing == 1 || facing == 2;
        let (outer_count, inner_count) = if horizontal { (height, width) } else { (width, height) };
        let start = if horizontal {
            if descending {
                x_end
            } else {
                x
            }
        } else if descending {
            z_end
        } else {
            z
        };
        let step = if descending { -1i32 } else { 1i32 };
        let mut row = 0i32;
        while row < outer_count {
            let fixed = if horizontal {
                z.wrapping_add(row)
            } else {
                x.wrapping_add(row)
            };
            let mut run_start = start;
            let mut run_length = 0i32;
            let mut index = 0i32;
            while index < inner_count {
                let position = start.wrapping_add(index.wrapping_mul(step));
                let (cx, cz) = if horizontal {
                    (position, fixed)
                } else {
                    (fixed, position)
                };
                if buildable(dev, cx, cz) {
                    run_length = run_length.wrapping_add(1);
                } else {
                    if run_length > 0 {
                        let low = if descending {
                            run_start.wrapping_sub(run_length).wrapping_add(1)
                        } else {
                            run_start
                        };
                        let rect = if horizontal {
                            [low, fixed, run_length, 1]
                        } else {
                            [fixed, low, 1, run_length]
                        };
                        result = create_strip(dev, rect, &mut list, &mut out);
                        run_length = 0;
                    }
                    run_start = position.wrapping_add(step);
                }
                index = index.wrapping_add(1);
            }
            if run_length > 0 {
                let low = if descending {
                    run_start.wrapping_sub(run_length).wrapping_add(1)
                } else {
                    run_start
                };
                let rect = if horizontal {
                    [low, fixed, run_length, 1]
                } else {
                    [fixed, low, 1, run_length]
                };
                result = create_strip(dev, rect, &mut list, &mut out);
            }
            row = row.wrapping_add(1);
        }
        let mut call = RawCall::new(
            Conv::Thiscall,
            crate::abi::target(0x0056_50A0),
            &mut list as *mut *mut u8 as u32,
            &[],
        );
        super::registers::call(&mut call);
        write_unaligned(out_ecx, call.ecx);
        (call.eax & !255) | u32::from(result)
    }
}

pub const PORTS: &[Port] = &[fc4p_0072d440::PORT];

#[cfg(test)]
mod tests;
