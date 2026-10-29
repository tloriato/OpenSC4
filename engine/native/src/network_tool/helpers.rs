// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Plane coefficients, pool allocation, neighbor offsets, and packed directions.

use super::*;
use crate::x87::F80;

layout! {
    /// Pool state behind a global allocator handle.
    struct Pool {
        0x08 => free_head / set_free_head: *mut u8,
        0x0C => live_count / set_live_count: u32,
    }
}
layout! {
    /// Five construction parameters retained by a network occupant.
    struct OccupantParameters {
        0x19C => first / set_first: u32,
        0x1A0 => second / set_second: u32,
        0x1A4 => third / set_third: u32,
        0x1A8 => fourth / set_fourth: u32,
        0x1AC => fifth / set_fifth: u32,
    }
}

game_data! { static occupant_pool: *mut u8 = 0x00B0_B2D4; }
game_data! { static occupant_pool_handle: u32 = 0x00B0_B2D0; }
game_data! { static alternate_pool: *mut u8 = 0x00B0_B3AC; }
game_data! { static alternate_pool_handle: u32 = 0x00B0_B3A8; }
game_data! { static neighbor_x: [i8; 256] = 0x00B0_B3D8; }
game_data! { static neighbor_z: [i8; 256] = 0x00B0_B3F4; }
game_fn! { fn grow_pool(handle: *mut u32) -> *mut u8 = "thiscall" 0x0090_CF9A; }
game_fn! { fn construct_base(this: *mut u8, initialize: u32) = "thiscall" 0x0061_E1A0; }
game_fn! { fn construct_member(this: *mut u8) = "thiscall" 0x005E_8670; }

// Arithmetic keeps each stored operand in its original role, including signaling NaN payload selection.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_op!(sub_float, "fsub dword ptr [{operand}]");
stored_op!(mul_float, "fmul dword ptr [{operand}]");

port! {
    /// Write the four unnormalized plane coefficients through three points. Inputs are read again after
    /// each output store, so overlapping output and input storage follows the same sequence.
    #[port(address = 0x005D_67E0, owner = "networktool", name = "PlaneThroughPoints")]
    pub unsafe extern "C" fn fc4p_005d67e0(a: *const f32, b: *const f32, c: *const f32, out: *mut f32) -> *const f32 {
        unsafe {
            let x = mul_float(sub_float(F80::from_f32(*c.add(2)), *a.add(2)), *b.add(1));
            let y = mul_float(sub_float(F80::from_f32(*b.add(2)), *c.add(2)), *a.add(1));
            let sum = x + y;
            let z = mul_float(sub_float(F80::from_f32(*a.add(2)), *b.add(2)), *c.add(1));
            *out = (sum + z).to_f32();
            let x = mul_float(sub_float(F80::from_f32(*b), *c), *a.add(2));
            let y = mul_float(sub_float(F80::from_f32(*c), *a), *b.add(2));
            let sum = x + y;
            let z = mul_float(sub_float(F80::from_f32(*a), *b), *c.add(2));
            *out.add(1) = (sum + z).to_f32();
            let x = mul_float(sub_float(F80::from_f32(*a.add(1)), *b.add(1)), *c);
            let y = mul_float(sub_float(F80::from_f32(*c.add(1)), *a.add(1)), *b);
            let sum = x + y;
            let z = mul_float(sub_float(F80::from_f32(*b.add(1)), *c.add(1)), *a);
            *out.add(2) = (sum + z).to_f32();
            let x = mul_float(F80::from_f32(*b.add(1)), *c.add(2));
            let y = mul_float(F80::from_f32(*b.add(2)), *c.add(1));
            let first = -mul_float(x - y, *a);
            let x = mul_float(F80::from_f32(*a.add(2)), *c.add(1));
            let y = mul_float(F80::from_f32(*a.add(1)), *c.add(2));
            let second = mul_float(x - y, *b);
            let sum = first - second;
            let x = mul_float(F80::from_f32(*a.add(1)), *b.add(2));
            let y = mul_float(F80::from_f32(*a.add(2)), *b.add(1));
            let third = mul_float(x - y, *c);
            *out.add(3) = (sum - third).to_f32();
        }
        c
    }
}

unsafe fn allocate(pool_location: *mut *mut u8, handle: *mut u32, site: u32) -> u64 {
    unsafe {
        let pool = Pool::from_ptr(*pool_location);
        let mut block = pool.free_head();
        if block.is_null() {
            block = grow_pool(handle);
        }
        let next: *mut u8 = read(block, 0);
        crate::abi::pool_take(site);
        let pool = Pool::from_ptr(*pool_location);
        let edx = pool.as_ptr() as u32;
        pool.set_free_head(next);
        let pool = Pool::from_ptr(*pool_location);
        pool.set_live_count(pool.live_count().wrapping_add(1));
        registers(block as u32, edx)
    }
}
port! {
    /// Pop one occupant block; grow the pool only when its free list is empty. The size word is ignored.
    #[port(address = 0x0060_5040, owner = "networktool", name = "AllocateOccupantBlock")]
    pub unsafe extern "C" fn fc4p_00605040(_size: u32) -> u64 {
        unsafe { allocate(occupant_pool(), occupant_pool_handle(), 0x0060_5058) }
    }
}
port! {
    /// Initialize the derived occupant interfaces and member, retaining the base constructor's fields.
    #[port(address = 0x0060_5160, owner = "networktool", name = "ConstructNetworkOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00605160(this: *mut u8, initialize: u32) -> *mut u8 {
        unsafe {
            if initialize != 0 {
                write(this, 0x3C, 0x00AA_7448u32);
                write(this, 0x1EC, 0x00AA_1F38u32);
            }
            construct_base(this, 0);
            let base: *mut u8 = read(this, 0x3C);
            for (offset, table) in [(0, 0x00AA_7300u32), (4, 0x00AA_7278), (8, 0x00AA_725C),
                (0xC, 0x00AA_71F8), (0x20, 0x00AA_71E0), (0x38, 0x00AA_71DC)] {
                write(this, offset, table);
            }
            let offset: u32 = read(base, 4);
            write(this.wrapping_add(offset as usize), 0x3C, 0x00AA_7190u32);
            let base: *mut u8 = read(this, 0x3C);
            let offset: u32 = read(base, 4);
            write(this.wrapping_add(offset as usize), 0x38, offset.wrapping_sub(0x1B0));
            construct_member(this.add(0x1D0));
            let flags: u32 = read(this, 0x13C);
            write(this, 0x13C, flags | 0x00C0_0000);
        }
        this
    }
}
port! {
    /// Pop one block from the alternate occupant pool, ignoring the caller's size word.
    #[port(address = 0x0061_D200, owner = "networktool", name = "AllocateAlternateOccupantBlock")]
    pub unsafe extern "C" fn fc4p_0061d200(_size: u32) -> u64 {
        unsafe { allocate(alternate_pool(), alternate_pool_handle(), 0x0061_D218) }
    }
}
port! {
    /// Retain the five raw construction words; EDX and EAX retain the fourth and fifth words.
    #[port(address = 0x0061_D340, owner = "networktool", name = "SetOccupantConstructionParameters")]
    pub unsafe extern "thiscall" fn fc4p_0061d340(this: *mut u8, a: u32, b: u32, c: u32, d: u32, e: u32) -> u64 {
        let p = unsafe { OccupantParameters::from_ptr(this) };
        p.set_first(a); p.set_second(b); p.set_third(c); p.set_fourth(d); p.set_fifth(e);
        registers(e, d)
    }
}

port! {
    /// Add the signed neighbor offsets selected by the record's byte at +4. Read both offsets before
    /// writing either output, then write x before z.
    #[port(address = 0x0062_0DA0, owner = "networktool", name = "GetNeighborCell")]
    pub unsafe extern "thiscall" fn fc4p_00620da0(this: *mut u8, x: u32, z: u32, out_x: *mut u32, out_z: *mut u32) -> u32 {
        unsafe {
            let index: u8 = read(this, 4);
            let dz = (*neighbor_z())[usize::from(index)];
            let dx = (*neighbor_x())[usize::from(index)];
            out_x.write_unaligned(x.wrapping_add(i32::from(dx) as u32));
            let result = z.wrapping_add(i32::from(dz) as u32);
            out_z.write_unaligned(result);
            result
        }
    }
}
register_port!(
    0x0062_0DE0,
    fc4p_00620de0,
    "GetPackedDirection",
    packed_direction,
    "ret 4"
);
unsafe extern "fastcall" fn packed_direction(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let shift = frame.add(3).read().wrapping_mul(8);
        registers(read::<u32>(this, 0).wrapping_shr(shift), frame.read())
    }
}
register_port!(
    0x0062_0DF0,
    fc4p_00620df0,
    "CollectConnectionDirections",
    collect_directions,
    "ret 16"
);
unsafe extern "fastcall" fn collect_directions(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let two = frame.add(3).read() as *mut u32;
        let one = frame.add(4).read() as *mut u32;
        let n_two = frame.add(5).read() as *mut u32;
        let n_one = frame.add(6).read() as *mut u32;
        n_two.write_unaligned(0);
        n_one.write_unaligned(0);
        let mut eax = frame.add(1).read();
        for i in 0..3u32 {
            if i == 1 {
                eax = read::<u32>(this, 0) >> 8;
            } else {
                eax = (eax & !0xFF) | u32::from(read::<u8>(this, i as usize));
            }
            eax = (eax & !0xFF) | (eax & 7);
            match eax as u8 {
                2 => {
                    eax = n_two.read_unaligned();
                    two.wrapping_add(eax as usize).write_unaligned(i);
                    n_two.write_unaligned(n_two.read_unaligned().wrapping_add(1));
                }
                1 | 3 => {
                    if i == 0 {
                        one.write_unaligned(0);
                    } else {
                        eax = n_one.read_unaligned();
                        one.wrapping_add(eax as usize).write_unaligned(i);
                    }
                    n_one.write_unaligned(n_one.read_unaligned().wrapping_add(1));
                }
                _ => {}
            }
        }
        eax = (eax & !0xFF) | (u32::from(read::<u8>(this, 3)) & 7);
        let mut edx = n_two as u32;
        match eax as u8 {
            2 => {
                let n = n_two.read_unaligned();
                two.wrapping_add(n as usize).write_unaligned(3);
                eax = n_two.read_unaligned().wrapping_add(1);
                n_two.write_unaligned(eax);
            }
            1 | 3 => {
                edx = n_one.read_unaligned();
                one.wrapping_add(edx as usize).write_unaligned(3);
                n_one.write_unaligned(n_one.read_unaligned().wrapping_add(1));
            }
            _ => {}
        }
        registers(eax, edx)
    }
}
port! {
    /// Replace one direction's upper five bits, retaining its low three bits. Direction indices wrap
    /// modulo four, and the supplied value is truncated to five bits.
    #[port(address = 0x0062_1070, owner = "networktool", name = "SetPackedDirectionValue")]
    pub unsafe extern "thiscall" fn fc4p_00621070(this: *mut u8, direction: u32, value: u32) -> u32 {
        unsafe {
            let shift = direction.wrapping_mul(8);
            let previous: u32 = read(this, 0);
            let byte = (previous.wrapping_shr(shift) & 7) | ((value & 31) << 3);
            let eax = byte.wrapping_shl(shift);
            write(this, 0, (previous & !0xFFu32.wrapping_shl(shift)) | eax);
            eax
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_005d67e0::PORT,
    fc4p_00605040::PORT,
    fc4p_00605160::PORT,
    fc4p_0061d200::PORT,
    fc4p_0061d340::PORT,
    fc4p_00620da0::PORT,
    fc4p_00620de0::PORT,
    fc4p_00620df0::PORT,
    fc4p_00621070::PORT,
];

#[cfg(test)]
mod tests;
