// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant construction and placement of one network piece by instance id.

use super::occupants::virtual_call;
use super::*;
use crate::x87::F80;

game_data! { static unsigned_correction: f32 = 0x00A8_0AA8; }
game_data! { static network_flags: [u32; 13] = 0x00AA_835C; }
game_fn! { fn allocate_plain(size: u32) -> *mut u8 = "C" 0x0061_9E90; }
game_fn! { fn allocate_model(size: u32) -> *mut u8 = "C" 0x0061_D200; }
game_fn! { fn allocate_extended(size: u32) -> *mut u8 = "C" 0x0060_5040; }
game_fn! { fn allocate_tunnel(size: u32) -> *mut u8 = "C" 0x0064_7230; }
game_fn! { fn construct_plain(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0061_BDD0; }
game_fn! { fn construct_model(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0061_E1A0; }
game_fn! { fn construct_extended(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0060_5160; }
game_fn! { fn construct_tunnel(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0064_76F0; }
game_fn! { fn reset_model(this: *mut u8) = "thiscall" 0x0061_D230; }
game_fn! { fn construct_cell(this: *mut u8) = "thiscall" 0x0064_7850; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(this: *mut u8) -> u64 = "C" 0x0090_CF63; }

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
stored_op!(subtract_float, "fsub dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");
unsafe fn unsigned(value: u32) -> F80 {
    let value = value as i32;
    let mut out = F80::from_i32(value);
    if value < 0 {
        out = add_float(out, unsafe { *unsigned_correction() });
    }
    out
}
port! {
    /// Build the four world-space vertices, construct the requested occupant class (or use an existing
    /// object), and configure flags, model, connections and support. Retain the cell's occupant only
    /// in real mode; release the temporary model interface after the final support call.
    #[port(address = 0x0062_4210, owner = "networktool", name = "cSC4NetworkTool::CreateNetworkOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00624210(this: *mut u8, cell: *mut u8, output: *mut *mut u8, points: *mut u8, class: u32) -> u64 {
        unsafe {
            let scale = vcall_st0!(NetworkTool::from_ptr(this).terrain(), 0x18).to_f32();
            write(points, 0, multiply_float(unsigned(read(cell, 0)), scale).to_f32());
            let z = multiply_float(unsigned(read(cell, 4)), scale);
            let x_bits: u32 = read(points, 0);
            write(points, 8, z.to_f32());
            write(points, 4, read::<u32>(cell, 0x58));
            let high_z = add_float(F80::from_f32(scale), read(points, 8));
            write(points, 0xC, x_bits);
            let z_bits: u32 = read(points, 8);
            write(points, 0x14, high_z.to_f32());
            let height: u32 = read(cell, 0x5C);
            write(points, 0x10, height);
            let high_x = add_float(F80::from_f32(scale), read(points, 0));
            write(points, 0x18, high_x.to_f32());
            write(points, 0x20, high_z.to_f32());
            write(points, 0x1C, read::<u32>(cell, 0x60));
            write(points, 0x24, high_x.to_f32());
            write(points, 0x2C, z_bits);
            write(points, 0x28, read::<u32>(cell, 0x64));
            let occupant = match class {
                0 => { let p = allocate_plain(0x180); if p.is_null() { p } else { construct_plain(p, 1) } }
                0x49C1_A034 => { let p = allocate_model(0x1D8); if p.is_null() { p } else { construct_model(p, 1) } }
                0x49CC_1BCD => { let p = allocate_extended(0x1F0); if p.is_null() { p } else { construct_extended(p, 1) } }
                0x8A4B_D52B => { let p = allocate_tunnel(0x1E0); if p.is_null() { p } else { construct_tunnel(p, 1) } }
                p => p as *mut u8,
            };
            output.write_unaligned(occupant);
            vcall!(occupant, 4 => fn() -> u32);
            let real: u8 = read(this, 0x50);
            let flags = if real != 0 {
                let old: *mut u8 = read(cell, 0xC); let new = output.read_unaligned();
                if new != old {
                    if !new.is_null() { vcall!(new, 4 => fn() -> u32); }
                    write(cell, 0xC, new);
                    if !old.is_null() { vcall!(old, 8 => fn() -> u32); }
                }
                read::<u32>(cell, 8)
            } else { read::<u32>(cell, 8) & 0x000C_0000 };
            vcall!(occupant, 0x28 => fn(u32) -> u32, flags);
            vcall!(occupant, 0xC => fn(*mut u8) -> u32, points);
            if read::<u8>(this, 0x50) != 0 { vcall!(occupant, 0x2C => fn(u32) -> u32, 0x8000_0000); }
            else { vcall!(occupant, 0x28 => fn(u32) -> u32, 0x8000_0000); }
            let mut interface = core::ptr::null_mut();
            if vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0x49C1_A034, &mut interface) as u8 != 0 { reset_model(interface); }
            if class == 0x8A4B_D52B {
                vcall!(occupant, 0x28 => fn(u32) -> u32, 0x1000_0000);
                vcall!(occupant, 0x2C => fn(u32) -> u32, 0x4000);
            } else { vcall!(occupant, 0x28 => fn(u32) -> u32, 0x4000); }
            if read::<u8>(this, 0x50) != 0 {
                for i in 0..13 {
                    if read::<u32>(cell, 8) & (*network_flags())[i] != 0 {
                        vcall!(occupant, 0x94 => fn(u32, u32) -> u32, i as u32, read::<u32>(cell, 0x18+i*4));
                    }
                }
                vcall!(occupant, 0x118 => fn(u32) -> u32, u32::from(read::<u8>(cell, 0x50)));
            }
            let model: u32 = read(this, 0x32C);
            let mut result = virtual_call(occupant, 0x100, &[model, 0], model, read(occupant, 0));
            if !interface.is_null() { result = virtual_call(interface, 8, &[], result.eax, read(interface, 0)); }
            registers(result.eax, result.edx)
        }
    }
}
port! {
    /// Place one piece at one cell. Query and transform its four corner heights, force real mode only
    /// for insertion and occupant finalization, then restore it before freeing the output list and
    /// releasing the cell's retained occupant. Usability examines only the supplied low byte.
    #[port(address = 0x0062_4FD0, owner = "networktool", name = "cSC4NetworkTool::PlaceNetworkOccupantById")]
    pub unsafe extern "thiscall" fn fc4p_00624fd0(this: *mut u8, x: u32, z: u32, piece: u32, usable: u32, kind: u32) -> u64 {
        unsafe {
            let mut cell = [0u8; 0xB8]; let c = cell.as_mut_ptr();
            construct_cell(c);
            write(c, 8, read::<u32>(network_flags().cast(), kind.wrapping_mul(4) as usize));
            for i in 0..4 { write(c, 0x58+i*4, 0u32); }
            let stride: u32 = read(this, 0x248); let index = stride.wrapping_mul(z).wrapping_add(x);
            let far = index.wrapping_add(stride);
            let vertices = [index, far, far.wrapping_add(1), index.wrapping_add(1)];
            for (i, vertex) in vertices.into_iter().enumerate() { write(c, 0xA8+i*4, vertex); }
            write(c, 0, x); write(c, 4, z); write(c, 0x10, 0u8);
            for i in 0..4 {
                let terrain = NetworkTool::from_ptr(this).terrain();
                let transform = crate::abi::vslot(this, 0x128);
                let altitude = vcall_st0!(terrain, 0x30, read::<u32>(c, 0xA8+i*4)).to_f32();
                let height = crate::abi::call_st0(crate::abi::Conv::Thiscall, transform, this as u32, &[altitude.to_bits()]);
                write(c, 0x58+i*4, height.to_f32());
                write(c, 0x68+i*8, subtract_float(height, read(this, 0x2C4)).to_f32());
                let upper = add_float(F80::from_f32(read(this, 0x2C4)), read(c, 0x58+i*4));
                write(c, 0x6C+i*8, upper.to_f32());
            }
            let saved_real: u8 = read(this, 0x50);
            let descriptor = [piece, 2u32, 0];
            write(this, 0x50, 1u8);
            let sentinel = allocate(12);
            write(sentinel, 0, sentinel); write(sentinel, 4, sentinel);
            let mut head = sentinel;
            vcall!(this, 0x120 => fn(*mut u8, *const u32, *mut *mut u8) -> u32, c, descriptor.as_ptr(), &mut head);
            let end = head; let mut node: *mut u8 = read(head, 0);
            while node != end {
                let occupant: *mut u8 = read(node, 8);
                if usable as u8 != 0 {
                    vcall!(occupant, 0x50 => fn(u32) -> u32, 1);
                    vcall!(occupant, 0x2C => fn(u32) -> u32, 0x4000);
                }
                vcall!(occupant, 8 => fn() -> u32);
                node = read(node, 0);
            }
            write(this, 0x50, saved_real);
            let mut node: *mut u8 = read(head, 0);
            while node != head {
                let old = node; node = read(node, 0);
                free(old);
            }
            write(head, 0, head); write(head, 4, head);
            let mut release = crate::abi::RawCall::new(crate::abi::Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &[]);
            let arguments = [head as u32]; release.args = arguments.as_ptr(); release.count = 1;
            release.eax = head as u32; release.edx = head as u32;
            crate::abi::call_raw(&mut release);
            let retained: *mut u8 = read(c, 0xC);
            if !retained.is_null() {
                let result = virtual_call(retained, 8, &[], read(retained, 0), release.edx);
                registers(result.eax, result.edx)
            } else { registers(release.eax, release.edx) }
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_00624210::PORT, fc4p_00624fd0::PORT];
#[cfg(test)]
mod tests;
