// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Terrain classification, padded cells, draw flags, and support lookup.

use super::*;
use crate::x87::F80;

game_data! { static network_flags: [u32; 13] = 0x00AA_835C; }
game_data! { static height_epsilon: f32 = 0x00AA_8424; }
game_fn! { fn collect_connections(this: *mut u8, two: *mut u32, one: *mut u32, n_two: *mut u32, n_one: *mut u32) -> u64 = "thiscall" 0x0062_0DF0; }
game_fn! { fn find_network_type(flags: u32) -> u32 = "C" 0x0062_3E60; }

fn subtract_from_float(value: F80, operand: f32) -> F80 {
    let mut out = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fsubr dword ptr [{operand}]", "fstp tbyte ptr [{out}]",
            value = in(reg) &value, operand = in(reg) &operand, out = in(reg) &mut out, out("st(0)") _);
    }
    out
}
port! {
    /// Classify four corner heights: zero when no positive clearance exceeds epsilon, one below the
    /// tool's height threshold, two at or above it. Always query all four terrain vertices in order.
    #[port(address = 0x0062_3BB0, owner = "networktool", name = "ClassifyNetworkElevation")]
    pub unsafe extern "thiscall" fn fc4p_00623bb0(this: *mut u8, cell: *mut u8) -> u32 {
        unsafe {
            let tool = NetworkTool::from_ptr(this);
            let mut result = 0;
            for i in 0..4 {
                let vertex: u32 = read(cell, 0xA8+i*4);
                let terrain = vcall_st0!(tool.terrain(), 0x30, vertex);
                let difference = subtract_from_float(terrain, read(cell, 0x58+i*4));
                let cmp = difference.compare(F80::from_f32(*height_epsilon()));
                if !cmp.less() && !cmp.equal() && !cmp.unordered() {
                    let threshold: f32 = read(this, 0x2C8);
                    let level = if difference.compare(F80::from_f32(threshold)).less() { 1 } else { 2 };
                    result = result.max(level);
                }
            }
            result
        }
    }
}
port! {
    /// Inclusive unsigned cell rectangle. Low bounds clamp to zero; high bounds clamp to dimension
    /// minus one after wrapping the padding addition. A zero dimension therefore produces UINT_MAX.
    #[port(address = 0x0062_3C20, owner = "networktool", name = "cSC4NetworkTool::GetPaddedRectAroundCell")]
    pub unsafe extern "thiscall" fn fc4p_00623c20(this: *mut u8, x: u32, z: u32, output: *mut u8, padding: u32) -> *mut u8 {
        unsafe {
            let tool = NetworkTool::from_ptr(this);
            write(output, 0, x.saturating_sub(padding));
            let high = x.wrapping_add(padding);
            write(output, 8, high);
            let width = tool.width();
            if high >= width { write(output, 8, width.wrapping_sub(1)); }
            write(output, 4, z.saturating_sub(padding));
            let high = z.wrapping_add(padding);
            write(output, 12, high);
            let height = tool.height();
            if high >= height { write(output, 12, height.wrapping_sub(1)); }
        }
        output
    }
}
register_port!(
    0x0062_3C90,
    fc4p_00623c90,
    "HasOneOrTwoSimpleConnections",
    simple_connections,
    "ret"
);
unsafe extern "fastcall" fn simple_connections(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let record = frame.add(3).read() as *mut u8;
        let index = frame.add(4).read();
        let mut two = [0u32; 4];
        let mut one = [0u32; 4];
        // Both count outputs intentionally overwrite this cdecl caller's argument words.
        let result = collect_connections(
            record.wrapping_add(index.wrapping_mul(4).wrapping_add(0x18) as usize),
            two.as_mut_ptr(),
            one.as_mut_ptr(),
            frame.add(4),
            frame.add(3),
        );
        let yes = frame.add(4).read() == 0 && matches!(frame.add(3).read(), 1 | 2);
        registers(u32::from(yes), (result >> 32) as u32)
    }
}
register_port!(
    0x0062_3CE0,
    fc4p_00623ce0,
    "cSC4NetworkTool::SetQuadTrianglesToDraw",
    quad_triangles,
    "ret 8"
);
// Keep the vtable register loads as well as both return registers of each flag operation.
unsafe fn flag_call(occupant: *mut u8, slot: usize, flag: u32, state: u64, table_in_eax: bool) -> u64 {
    unsafe {
        let table: u32 = read(occupant, 0);
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(occupant, slot),
            occupant as u32,
            &[flag],
        );
        // The argument word must live across call_raw.
        let arguments = [flag];
        call.args = arguments.as_ptr();
        call.eax = if table_in_eax { table } else { state as u32 };
        call.edx = if table_in_eax { (state >> 32) as u32 } else { table };
        crate::abi::call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}
unsafe extern "fastcall" fn quad_triangles(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let occupant = frame.add(3).read() as *mut u8;
        let pattern = frame.add(4).read() as *mut u32;
        let mut state = flag_call(
            occupant,
            0x20,
            0x4000_0000,
            registers(frame.add(1).read(), frame.read()),
            true,
        );
        if state as u8 == 0 {
            state = flag_call(occupant, 0x28, 0x0200_0000, state, false);
            return flag_call(occupant, 0x28, 0x0400_0000, state, true);
        }
        let pattern = pattern.read_unaligned();
        match pattern {
            0x9 | 0x301 | 0x309 | 0xB00 | 0xB01 => {
                let eax = if pattern == 9 { 0 } else { pattern };
                state = flag_call(occupant, 0x28, 0x0100_0000, registers(eax, (state >> 32) as u32), false);
                state = flag_call(occupant, 0x28, 0x0200_0000, state, true);
                flag_call(occupant, 0x2C, 0x0400_0000, state, false)
            }
            0xB | 0x0100_0003 | 0x0100_000B | 0x0900_0000 | 0x0900_0003 => {
                state = flag_call(occupant, 0x28, 0x0200_0000, state, true);
                flag_call(occupant, 0x2C, 0x0400_0000, state, false)
            }
            0x900 | 0x30100 | 0x30900 | 0xB0000 | 0xB0100 => {
                state = flag_call(occupant, 0x28, 0x0400_0000, state, true);
                flag_call(occupant, 0x2C, 0x0200_0000, state, false)
            }
            0x90000 | 0x0301_0000 | 0x0309_0000 | 0x0B00_0000 | 0x0B01_0000 => {
                state = flag_call(occupant, 0x28, 0x0100_0000, state, true);
                state = flag_call(occupant, 0x28, 0x0400_0000, state, false);
                flag_call(occupant, 0x2C, 0x0200_0000, state, true)
            }
            _ => {
                state = flag_call(occupant, 0x28, 0x0200_0000, state, true);
                flag_call(occupant, 0x28, 0x0400_0000, state, false)
            }
        }
    }
}
register_port!(
    0x0062_3E60,
    fc4p_00623e60,
    "FindNetworkTypeForMask",
    type_for_mask,
    "ret"
);
unsafe extern "fastcall" fn type_for_mask(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let flags = frame.add(3).read();
        for i in 0..12 {
            if (*network_flags())[i] & flags != 0 {
                return registers(i as u32, frame.read());
            }
        }
        registers(12, frame.read())
    }
}
register_port!(
    0x0062_3EC0,
    fc4p_00623ec0,
    "cSC4NetworkTool::GetFirstNetworkTypeFromFlags",
    first_type,
    "ret"
);
unsafe extern "fastcall" fn first_type(_this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let flags = frame.add(3).read();
        let first = flags & flags.wrapping_neg();
        frame.add(3).write(first);
        registers(find_network_type(first), frame.read())
    }
}
port! {
    /// Find the support-structure payload in the type's chained hash buckets; keep unsigned key and
    /// bucket arithmetic, including the processor's divide exception for a zero bucket count.
    #[port(address = 0x0062_5860, owner = "networktool", name = "cSC4NetworkTool::GetSupportStructureInfo")]
    pub unsafe extern "thiscall" fn fc4p_00625860(this: *mut u8, key: u32) -> *mut u8 {
        unsafe {
            let info = NetworkTool::from_ptr(this).type_info();
            let begin: u32 = read(info, 0x40); let end: u32 = read(info, 0x44);
            let count = ((end.wrapping_sub(begin) as i32) >> 2) as u32;
            let remainder: u32;
            core::arch::asm!("div {count:e}", count = in(reg) count, inout("eax") key => _, inout("edx") 0u32 => remainder);
            let mut node: *mut u8 = read(begin as *mut u8, remainder.wrapping_mul(4) as usize);
            while !node.is_null() {
                if read::<u32>(node, 4) == key { return node.add(8); }
                node = read(node, 0);
            }
            core::ptr::null_mut()
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00623bb0::PORT,
    fc4p_00623c20::PORT,
    fc4p_00623c90::PORT,
    fc4p_00623ce0::PORT,
    fc4p_00623e60::PORT,
    fc4p_00623ec0::PORT,
    fc4p_00625860::PORT,
];
#[cfg(test)]
mod tests;
