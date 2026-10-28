// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Network solution cost, including the x87 biased conversion of terrain work.

use super::*;
use crate::x87::F80;

game_fn! { fn lookup_cell(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_data! { static network_flags: [u32; 13] = 0x00AA_835C; }
game_data! { static build_prices: [u8; 13*0x114] = 0x00B4_53A4; }
game_data! { static bridge_arrays: [u8; 13*0x114] = 0x00B4_52F8; }
game_data! { static support_buckets_begin: u32 = 0x00B4_52B4; }
game_data! { static support_buckets_end: u32 = 0x00B4_52B8; }
game_data! { static integer_bias: f32 = 0x00AA_84F8; }

macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut out = F80::ZERO;
            unsafe { core::arch::asm!("fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{out}]",
                value = in(reg) &value, operand = in(reg) &operand, out = in(reg) &mut out, out("st(0)") _); }
            out
        }
    };
}
stored_op!(add_float, "fadd dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");

port! {
    /// Sum new-cell prices, terrain displacement, bridge spans, tunnel lengths and support costs.
    /// Each cell pays the greatest unsigned price among its network flags. Terrain displacement is
    /// accumulated as stored floats, multiplied by the tool rate, then converted with the game's
    /// positive float bias and integer-bit subtraction; both integer result registers are retained.
    #[port(address = 0x0062_5640, owner = "networktool", name = "ComputeNetworkSolutionCost")]
    pub unsafe extern "thiscall" fn fc4p_00625640(this: *mut u8) -> u64 {
        unsafe {
            let mut total = 0u32; let mut displacement = 0.0f32;
            let mut p: *mut u8 = read(this, 0x98); let end: *mut u8 = read(this, 0x9C);
            while p != end {
                let cell = lookup_cell(this, read(p, 8));
                if !cell.is_null() {
                    if read::<u8>(cell, 0x10) != 0 && read::<u8>(cell, 0x52) == 0 {
                        let flags: u32 = read(cell, 8); let mut price = 0u32;
                        for i in 0..13 {
                            if flags & (*network_flags())[i] != 0 { price = price.max(read(build_prices().cast(), i*0x114)); }
                        }
                        total = total.wrapping_add(price);
                    }
                    for i in 0..4 {
                        let terrain = NetworkTool::from_ptr(this).terrain();
                        let altitude = vcall_st0!(terrain, 0x30, read::<u32>(cell, 0xA8+i*4));
                        let height = F80::from_f32(read(cell, 0x58+i*4));
                        let cmp = altitude.compare(height);
                        let difference = if !cmp.less() && !cmp.equal() && !cmp.unordered() { altitude-height } else { height-altitude };
                        displacement = add_float(difference, displacement).to_f32();
                    }
                }
                p = p.wrapping_add(12);
            }
            let mut p: *mut u8 = read(this, 0xD8); let end: *mut u8 = read(this, 0xDC);
            if p != end {
                let extra: u32 = read(this, 0x320);
                while p != end {
                    let kind: u32 = read(p, 0); let index: u32 = read(p, 0xC);
                    let array: *mut u8 = read(bridge_arrays().cast(), kind.wrapping_mul(0x114) as usize);
                    let record = array.wrapping_add(index.wrapping_mul(0x178) as usize);
                    let length = read::<u32>(p, 8).wrapping_sub(read(p, 4));
                    let unit = read::<u32>(record, 0x154).wrapping_add(extra);
                    total = total.wrapping_add(length.wrapping_mul(unit).wrapping_add(read(record, 0x150)));
                    p = p.wrapping_add(0x28);
                }
            }
            let mut p: *mut u8 = read(this, 0xE4); let end: *mut u8 = read(this, 0xE8);
            if p != end {
                let unit: u32 = read(this, 0x324);
                while p != end {
                    total = total.wrapping_add(read::<u32>(p, 0xC).wrapping_sub(read(p, 8)).wrapping_mul(unit));
                    p = p.wrapping_add(0x10);
                }
            }
            let mut p: *mut u8 = read(this, 0xCC); let end: *mut u8 = read(this, 0xD0);
            if p != end {
                let begin = *support_buckets_begin();
                while p != end {
                    let count = ((*support_buckets_end()).wrapping_sub(begin) as i32 >> 2) as u32;
                    let key: u32 = read(p, 0); let remainder: u32;
                    core::arch::asm!("div {count:e}", count = in(reg) count, inout("eax") key => _, inout("edx") 0u32 => remainder);
                    let mut node: *mut u8 = read(begin as *mut u8, remainder.wrapping_mul(4) as usize);
                    while !node.is_null() && read::<u32>(node, 4) != key { node = read(node, 0); }
                    let payload = if node.is_null() { core::ptr::null_mut() } else { node.add(8) };
                    total = total.wrapping_add(read(payload, 0xB4));
                    p = p.wrapping_add(0x28);
                }
            }
            let terrain_cost = multiply_float(F80::from_f32(displacement), read(this, 0x328));
            let biased = add_float(terrain_cost, *integer_bias()).to_f32().to_bits();
            let edx = read::<u32>(this, 0x220).wrapping_add(biased);
            registers(total.wrapping_add(edx).wrapping_add(0xB4C0_0000), edx)
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_00625640::PORT];
#[cfg(test)]
mod tests;
