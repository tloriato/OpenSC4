// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tunnel portal placement and terrain changes around its raised edge.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_fn! { fn set_exemplar(this: *mut u8, occupant: *mut u8, id: u32, position: *const u32, orientation: u32, flags: u32) = "thiscall" 0x0062_4860; }
game_fn! { fn clear_cell(this: *mut u8, x: u32, z: u32, network: u32) = "thiscall" 0x0062_72F0; }

layout! {
    /// Tunnel resource vectors and the two vertex dimensions used by portal insertion.
    struct TunnelTool {
        0x248 => x_stride / set_x_stride: u32,
        0x24C => z_stride / set_z_stride: u32,
        0x2E4 => exemplars / set_exemplars: *mut u8,
        0x2E8 => exemplars_end / set_exemplars_end: *mut u8,
        0x2F0 => rotations / set_rotations: *mut u8,
        0x2FC => raises / set_raises: *mut u8,
        0x364 => clear_network / set_clear_network: u32,
    }
}

layout! {
    /// Cell fields touched by inserting a tunnel portal.
    struct PortalCell {
        0x00 => x / set_x: u32,
        0x04 => z / set_z: u32,
        0x54 => inserted / set_inserted: u8,
    }
}

// Retain the stored operand and the duplicated register operand when raising both corners. This
// preserves operand order, NaN payloads, rounding and the order of the two stores.
unsafe fn raise_edge(raise: F80, first: *mut u8, second: *mut u8) {
    let bytes = raise.to_bytes();
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{raise}]", "fld st(0)",
            "fadd dword ptr [{first}]", "fstp dword ptr [{first}]",
            "fadd dword ptr [{second}]", "fstp dword ptr [{second}]",
            raise = in(reg) bytes.as_ptr(), first = in(reg) first, second = in(reg) second,
            out("st(0)") _, out("st(1)") _,
        );
    }
}

unsafe fn below(value: F80, height: *const u8) -> bool {
    let bytes = value.to_bytes();
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fcomp dword ptr [{height}]", "fnstsw ax",
            value = in(reg) bytes.as_ptr(), height = in(reg) height,
            out("ax") status, out("st(0)") _,
        );
    }
    // TEST AH,5 has odd parity only for an ordered less-than result.
    (status >> 8) & 5 == 1
}

fn minimum(a: u32, b: u32) -> u32 {
    (a as i32).min(b as i32) as u32
}
fn maximum(a: u32, b: u32) -> u32 {
    (a as i32).max(b as i32) as u32
}

// Unsigned division uses a zero high word and the CPU's divide exception for a zero divisor.
// Keeping this primitive on the CPU also avoids introducing a Rust panic path into the DLL.
fn divide(numerator: u32, divisor: u32) -> (u32, u32) {
    let quotient: u32;
    let remainder: u32;
    unsafe {
        core::arch::asm!(
            "xor edx, edx", "div {divisor:e}",
            divisor = in(reg) divisor,
            inout("eax") numerator => quotient, out("edx") remainder,
            options(nomem, nostack),
        );
    }
    (quotient, remainder)
}

port! {
    /// Insert a portal, raise its edge, and extend the terrain update rectangle for low neighbouring
    /// vertices. The caller's three argument words are reused for the rectangle's x bounds and low z.
    #[port(address = 0x0062_8390, owner = "networktool", name = "cSC4NetworkTool::InsertTunnelPiece", naked)]
    pub unsafe extern "C" fn fc4p_00628390() {
        core::arch::naked_asm!(
            "lea edx, [esp + 4]", "call {body}", "ret 12",
            body = sym insert,
        );
    }
}

// The entry supplies the original argument words; fastcall has no stack arguments of its own.
unsafe extern "fastcall" fn insert(this: *mut u8, args: *mut u32) -> *mut u8 {
    unsafe {
        let tunnel = TunnelTool::from_ptr(this);
        if tunnel.exemplars() == tunnel.exemplars_end() {
            return core::ptr::null_mut();
        }
        let tool = NetworkTool::from_ptr(this);
        let cell = args.add(2).read() as *mut u8;
        let portal = PortalCell::from_ptr(cell);
        let mut occupant = core::ptr::null_mut::<u8>();
        let mut creation_points = MaybeUninit::<[u32; 12]>::uninit();
        vcall!(this, 0xD8 => fn(*mut u8, *mut *mut u8, *mut u32, u32),
            cell, &mut occupant, creation_points.as_mut_ptr().cast(), 0x8A4B_D52B);
        vcall!(occupant, 0x2C => fn(u32), 0x0200_0000);
        vcall!(occupant, 0x2C => fn(u32), 0x0400_0000);
        vcall!(occupant, 0x2C => fn(u32), 0x4000);
        vcall!(occupant, 0x28 => fn(u32), 0x1000_0000);
        let index = u32::from(args.add(1).read() as u8).wrapping_mul(4) as usize;
        let position = [0u32; 3];
        let rotation = read::<u8>(tunnel.rotations(), index).wrapping_add(args.read() as u8) & 3;
        let exemplar = read(tunnel.exemplars(), index);
        set_exemplar(this, occupant, exemplar, position.as_ptr(), u32::from(rotation), 0);
        clear_cell(this, portal.x(), portal.z(), tunnel.clear_network());
        if !tool.occupant_manager().is_null() {
            // The insertion callback may change the manager pointer; its old vtable is retained.
            let table: *mut u8 = read(tool.occupant_manager(), 0);
            let data = vcall!(occupant, 0xC0 => fn(u32) -> u32, tool.insertion_data());
            let slot: usize = read(table, 0x50);
            let f = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32) -> u32>(slot);
            portal.set_inserted(f(tool.occupant_manager(), data) as u8);
        }
        let raise = F80::from_f32(read(tunnel.raises(), index));
        let direction = args.read();
        let first = (!direction) & 3;
        let second = first.wrapping_add(1) & 3;
        raise_edge(
            raise,
            cell.add(0x58 + first as usize * 4),
            cell.add(0x58 + second as usize * 4),
        );

        let v_second: u32 = read(cell, 0xA8 + second as usize * 4);
        let stride = tunnel.x_stride();
        let x_second = divide(v_second, stride).1;
        let v_first: u32 = read(cell, 0xA8 + first as usize * 4);
        args.write(x_second);
        args.add(1).write(divide(v_first, stride).1);
        args.write(minimum(args.read(), args.add(1).read()));
        let stride = tunnel.x_stride();
        let x_second = divide(v_second, stride).1;
        let v_first: u32 = read(cell, 0xA8 + first as usize * 4);
        args.add(1).write(x_second);
        args.add(2).write(divide(v_first, stride).1);
        args.add(2).write(maximum(args.add(1).read(), args.add(2).read()));
        let stride = tunnel.z_stride();
        args.add(1).write(divide(v_second, stride).0);
        let v_first: u32 = read(cell, 0xA8 + first as usize * 4);
        args.add(1)
            .write(minimum(args.add(1).read(), divide(v_first, stride).0));
        let stride = tunnel.z_stride();
        let z_second = divide(v_second, stride).0;
        let v_first: u32 = read(cell, 0xA8 + first as usize * 4);
        let mut high_z = maximum(z_second, divide(v_first, stride).0);

        for corner in 0..4 {
            let height: u32 = read(cell, 0x58 + corner * 4);
            vcall!(tool.terrain(), 0xC8 => fn(u32, u32), read::<u32>(cell, 0xA8 + corner * 4), height);
        }
        let v_first: u32 = read(cell, 0xA8 + first as usize * 4);
        let preceding: u32 = read(cell, 0xA8 + (first.wrapping_sub(1) & 3) as usize * 4);
        let outer_first = v_first.wrapping_mul(2).wrapping_sub(preceding);
        let v_second: u32 = read(cell, 0xA8 + second as usize * 4);
        let following: u32 = read(cell, 0xA8 + ((second + 1) & 3) as usize * 4);
        let outer_second = v_second.wrapping_mul(2).wrapping_sub(following);

        for (vertex, corner) in [(outer_first, first), (outer_second, second)] {
            let value = vcall_st0!(tool.terrain(), 0x30, vertex);
            let height = cell.add(0x58 + corner as usize * 4);
            if below(value, height) {
                vcall!(tool.terrain(), 0xC8 => fn(u32, u32), vertex, read::<u32>(height, 0));
                let x = divide(vertex, tunnel.x_stride()).1;
                args.write(minimum(x, args.read()));
                args.add(2).write(maximum(args.add(2).read(), x));
                let z = divide(vertex, tunnel.z_stride()).0;
                args.add(1).write(minimum(z, args.add(1).read()));
                high_z = maximum(high_z, z);
            }
        }
        let rect = [args.read(), args.add(1).read(), args.add(2).read(), high_z];
        vcall!(tool.terrain(), 0x15C => fn());
        vcall!(tool.terrain(), 0x110 => fn(u32, u32, *const u32, u32), 1, 1, rect.as_ptr(), 6);
        vcall!(tool.terrain(), 0x160 => fn());
        let result = occupant;
        if !result.is_null() {
            vcall!(result, 8 => fn());
        }
        result
    }
}

pub const PORTS: &[Port] = &[fc4p_00628390::PORT];

#[cfg(test)]
mod tests;
