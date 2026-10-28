// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Terrain altitude overridden by a lot with locked vertices.

use super::{manager::LotManager, Port};
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

game_data! { static cell_scale: f32 = 0x00B0_D4AC; }
game_data! { static cell_offset: f32 = 0x00A8_F7BC; }
game_data! { static rounding_bias: f32 = 0x00AB_36D4; }

// Use memory operands so that signaling NaNs keep their operand roles when a payload is chosen.
macro_rules! float_memory {
    ($name:ident, $instruction:literal) => {
        fn $name(input: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input=in(reg) &input, operand=in(reg) &operand, output=in(reg) &mut output,
                    out("st(0)") _,);
            }
            output
        }
    };
}
float_memory!(multiply, "fmul dword ptr [{operand}]");
float_memory!(subtract, "fsub dword ptr [{operand}]");
float_memory!(add, "fadd dword ptr [{operand}]");

unsafe fn cell(position: f32) -> i32 {
    unsafe {
        let scaled = multiply(F80::from_f32(*cell_scale()), position);
        let offset = subtract(scaled, *cell_offset());
        add(offset, *rounding_bias())
            .to_f32()
            .to_bits()
            .wrapping_add(0xB4C0_0000) as i32
    }
}

fn clamp_cell(mut cell: i32, count: u32) -> i32 {
    if cell as u32 >= count {
        if cell < 0 {
            cell = 0;
        }
        if cell >= count as i32 {
            cell = count.wrapping_sub(1) as i32;
        }
    }
    cell
}

port! {
    /// Marshal an unrounded ST0 return and the callee's EDX:EAX through a Rust body.
    #[port(address = 0x0072_A9F0, owner = "zonedeveloper", name = "GetLotSavvyTerrainAltitude", naked)]
    pub unsafe extern "C" fn fc4p_0072a9f0() {
        core::arch::naked_asm!(
            "sub esp, 12", "mov edx, esp",
            "push dword ptr [esp + 24]", "push dword ptr [esp + 24]", "push dword ptr [esp + 24]",
            "call {body}", "fld tbyte ptr [esp]", "add esp, 12", "ret 12",
            body=sym lot_savvy_altitude,
        );
    }
}

// The output pointer is supplied by the entry stub; all placement decisions are made here.
unsafe extern "fastcall" fn lot_savvy_altitude(this: *mut u8, output: *mut F80, x: f32, z: f32, ok: *mut u8) -> u64 {
    unsafe {
        let manager = LotManager::from_ptr(this);
        let count_x = manager.cells_x();
        let cx = cell(x);
        let cz = cell(z);
        let cx = clamp_cell(cx, count_x);
        let cz = clamp_cell(cz, manager.cells_z());
        let row = read_unaligned(
            manager
                .rows()
                .wrapping_add((cx as u32).wrapping_mul(12) as usize)
                .cast::<*mut u8>(),
        );
        let lot = read_unaligned(row.wrapping_add((cz as u32).wrapping_mul(4) as usize).cast::<*mut u8>());
        let mut flags_edx = 0;
        let locked = if lot.is_null() {
            false
        } else {
            let mut call = RawCall::new(Conv::Thiscall, vslot(lot, 0x30), lot as u32, &[]);
            call.eax = row as u32;
            call.edx = read_unaligned(lot.cast::<u32>());
            call_raw(&mut call);
            flags_edx = call.edx;
            call.eax & 1 != 0
        };
        let args = [x.to_bits(), z.to_bits()];
        let mut call = if locked {
            if !ok.is_null() {
                *ok = 1;
            }
            let mut call = RawCall::new(Conv::Thiscall, vslot(lot, 0x40), lot as u32, &[]);
            call.eax = read_unaligned(lot.cast::<u32>());
            call.edx = flags_edx;
            call
        } else {
            if !ok.is_null() {
                *ok = 0;
            }
            let terrain = read_unaligned(this.add(0x30).cast::<*mut u8>());
            let mut call = RawCall::new(Conv::Thiscall, vslot(terrain, 0x34), terrain as u32, &args);
            call.eax = x.to_bits();
            call.edx = read_unaligned(terrain.cast::<u32>());
            call
        };
        call.take_st0 = 1;
        call_raw(&mut call);
        write_unaligned(output, call.st0);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

port! {
    /// Collect distinct lots adjoining a vertex, X outer and Z inner. Start both adjustment limits at FLT_MAX;
    /// set both to 32 whenever a lot's signed state is at least two, visiting every distinct lot in order.
    /// EDX:EAX retains the final state call, or the scan's scratch values when there are no lots.
    #[port(address = 0x0072_AE20, owner = "zonedeveloper", name = "GetLotVertexAltitudeAdjustmentRange")]
    pub unsafe extern "thiscall" fn fc4p_0072ae20(
        this: *mut u8, vx: i32, vz: i32, first: *mut u32, second: *mut u32
    ) -> u64 {
        unsafe {
            let manager = LotManager::from_ptr(this);
            let start_x = vx.wrapping_sub(1).max(0);
            let start_z = vz.wrapping_sub(1).max(0);
            let end_x = if vx >= manager.cells_x() as i32 { manager.cells_x().wrapping_sub(1) as i32 } else { vx };
            let end_z = if vz >= manager.cells_z() as i32 { manager.cells_z().wrapping_sub(1) as i32 } else { vz };
            let mut lots = [core::ptr::null_mut::<u8>(); 4];
            let mut count = 0usize;
            let mut eax = start_x as u32;
            let mut edx = end_x as u32;
            if start_x <= end_x {
                let mut remaining_x = end_x.wrapping_sub(start_x).wrapping_add(1) as u32;
                edx = remaining_x;
                let mut row_offset = (start_x as u32).wrapping_mul(12);
                loop {
                    if start_z <= end_z {
                        let row = read_unaligned(manager.rows().wrapping_add(row_offset as usize).cast::<*mut u8>());
                        let mut cell = row.wrapping_add((start_z as u32).wrapping_mul(4) as usize);
                        let mut remaining_z = end_z.wrapping_sub(start_z).wrapping_add(1) as u32;
                        loop {
                            let lot = read_unaligned(cell.cast::<*mut u8>());
                            edx = lot as u32;
                            if !lot.is_null() {
                                let mut i = 0;
                                while i < count && read_unaligned(lots.as_ptr().add(i)) != lot { i += 1; }
                                if i == count {
                                    write_unaligned(lots.as_mut_ptr().add(count), lot);
                                    count += 1;
                                }
                            }
                            cell = cell.wrapping_add(4);
                            remaining_z = remaining_z.wrapping_sub(1);
                            if remaining_z == 0 { break; }
                        }
                    }
                    row_offset = row_offset.wrapping_add(12);
                    remaining_x = remaining_x.wrapping_sub(1);
                    if remaining_x == 0 { break; }
                }
                eax = 0;
            }
            write_unaligned(first, 0x7F7F_FFFF);
            write_unaligned(second, 0x7F7F_FFFF);
            for i in 0..count {
                let lot = read_unaligned(lots.as_ptr().add(i));
                let result = super::virtual_registers(lot, 0x84, eax, read_unaligned(lot.cast::<u32>()), &[]);
                eax = result as u32;
                edx = (result >> 32) as u32;
                if eax as i32 >= 2 {
                    write_unaligned(first, 0x4200_0000);
                    write_unaligned(second, 0x4200_0000);
                }
            }
            (u64::from(edx) << 32) | u64::from(eax)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072a9f0::PORT, fc4p_0072ae20::PORT];

#[cfg(test)]
mod tests;
