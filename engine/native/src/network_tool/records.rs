// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Copying network parameter records and constructing a cell rule context.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};

game_fn! { #[allow(dead_code)] fn assign_vector(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0062_4EA0; }
game_fn! { #[allow(dead_code)] fn assign_words(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0050_DB90; }
game_fn! { fn find_cell(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_data! { static type_masks: [u32; 13] = 0x00AA_835C; }

layout! {
    /// Cached rule values followed by the cell and tool that supply them.
    struct CellContext {
        0x00 => vtable / set_vtable: u32,
        0x04 => flags / set_flags: u32,
        0x08 => begin / set_begin: u32,
        0x0C => end / set_end: u32,
        0x10 => capacity / set_capacity: u32,
        0x14 => x / set_x: u32,
        0x18 => z / set_z: u32,
        0x1C => tool / set_tool: *mut u8,
        0x20 => network_type / set_network_type: u32,
        0x24 => other_types / set_other_types: u32,
    }
}

port! {
    /// Assign the fixed parameters in word order, retaining the padding after both byte flags, then
    /// assign the trailing vector through its own helper. Source and destination may overlap.
    #[port(address = 0x0062_67D0, owner = "networktool", name = "AssignNetworkParameters")]
    pub unsafe extern "thiscall" fn fc4p_006267d0(this: *mut u8, source: *mut u8) -> u64 {
        unsafe {
            for offset in [0, 4, 8, 12] { write(this, offset, read::<u32>(source, offset)); }
            write(this, 0x10, read::<u8>(source, 0x10));
            let mut edx = 0;
            for offset in (0x14..0x40).step_by(4) {
                let value = read::<u32>(source, offset);
                write(this, offset, value);
                if offset == 0x3C { edx = value; }
            }
            write(this, 0x40, read::<u8>(source, 0x40));
            let args = [source.add(0x44) as u32];
            let mut c = RawCall::new(Conv::Thiscall, crate::abi::target(0x0062_4EA0), this.add(0x44) as u32, &args);
            c.eax = args[0]; c.edx = edx;
            call_raw(&mut c);
            registers(this as u32, c.edx)
        }
    }
}

port! {
    /// Initialize a cell rule context, using only the low byte of `exclude_other_types`.
    /// An existing cached cell supplies this network type's flags; an absent cell leaves zero flags.
    #[port(address = 0x0062_7080, owner = "networktool", name = "ConstructCellRuleContext")]
    pub unsafe extern "thiscall" fn fc4p_00627080(
        this: *mut u8, x: u32, z: u32, tool: *mut u8, network_type: u32, exclude_other_types: u32,
    ) -> *mut u8 {
        unsafe {
            let c = CellContext::from_ptr(this);
            c.set_flags(0); c.set_begin(0); c.set_end(0); c.set_capacity(0);
            c.set_tool(tool);
            c.set_vtable(0x00AA_8700);
            c.set_x(x); c.set_z(z); c.set_network_type(network_type);
            let mask = if exclude_other_types as u8 != 0 { 0 } else {
                type_masks().cast::<u32>().wrapping_add(network_type as usize).read_unaligned() ^ 0x1FFF
            };
            c.set_other_types(mask);
            c.set_flags(0);
            let cell = find_cell(c.tool(), z.wrapping_shl(16).wrapping_add(x));
            if !cell.is_null() {
                c.set_flags(read(cell, 0x18usize.wrapping_add(c.network_type().wrapping_mul(4) as usize)));
            }
        }
        this
    }
}

port! {
    /// Assign whole 40-byte records, truncating the signed byte distance towards zero. Copy seven words
    /// in order, then call the trailing vector's assignment helper. Empty and reversed ranges do nothing.
    #[port(address = 0x0062_7220, owner = "networktool", name = "AssignNetworkRecordRange")]
    pub unsafe extern "C" fn fc4p_00627220(first: *mut u8, last: *mut u8, output: *mut u8) -> u64 {
        let (mut first, mut output) = (first, output);
        let distance = (last as u32).wrapping_sub(first as u32) as i32;
        let product = i64::from(distance) * 0x6666_6667;
        let high = ((product >> 32) as i32) >> 4;
        let count = high.wrapping_add(((high as u32) >> 31) as i32);
        let mut edx = high as u32;
        unsafe {
            for _ in 0..count {
                let mut eax = 0;
                for offset in (0..0x1C).step_by(4) {
                    let value = read::<u32>(first, offset);
                    write(output, offset, value);
                    if offset == 0x14 { edx = value; }
                    if offset == 0x18 { eax = value; }
                }
                let args = [first.add(0x1C) as u32];
                let mut c = RawCall::new(Conv::Thiscall, crate::abi::target(0x0050_DB90), output.add(0x1C) as u32, &args);
                c.eax = eax; c.edx = edx;
                call_raw(&mut c);
                edx = c.edx;
                first = first.wrapping_add(0x28);
                output = output.wrapping_add(0x28);
            }
        }
        registers(output as u32, edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006267d0::PORT, fc4p_00627080::PORT, fc4p_00627220::PORT];

#[cfg(test)]
mod tests;
