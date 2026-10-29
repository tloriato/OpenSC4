// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Arrival accounting on lots and neighbor connections.

use super::late_containers::{call, register_entry};
use super::late_passes::virtual_call;
use super::*;
use crate::abi::Conv;

game_data! {static lot_manager: *mut u8 = 0x00B4_3D08;}
game_fn! {#[allow(dead_code)] fn pool(this:*mut u8,travel:u32,x:i32,z:i32)->*mut u8="thiscall" 0x0071_14E0;}
game_fn! {#[allow(dead_code)] fn connection(this:*mut u8,travel:u32,x:i32,z:i32)->*mut u8="thiscall" 0x0071_04A0;}
game_fn! {#[allow(dead_code)] fn get_vector(holder:*mut u8,id:u32,vector:*mut u32)->u32="C" 0x005F_F510;}
game_fn! {#[allow(dead_code)] fn set_vector(holder:*mut u8,id:u32,vector:*mut u32)->u32="C" 0x005F_E6B0;}
game_fn! {#[allow(dead_code)] fn resize(this:*mut u8,count:u32,initial:*const u32)="thiscall" 0x006B_DF10;}
game_fn! {#[allow(dead_code)] fn free(memory:*mut u8)="C" 0x0090_CF63;}
game_fn! {#[allow(dead_code)] fn available(this:*mut u8,holder:*mut u8,kind:u32)->u32="thiscall" 0x0070_DFF0;}
game_fn! {#[allow(dead_code)] fn set_capacity(index:u32)="C" 0x0070_DE70;}

/// The capacity writer receives the holder in EDI and value in EBX, and the caller pops its index.
unsafe fn write_capacity(holder: u32, value: u32, index: u32, r: &mut Registers) {
    let mut frame = [
        crate::abi::target(0x0070_DE70) as u32,
        r.eax,
        r.ecx,
        r.edx,
        holder,
        value,
        index,
    ];
    unsafe {
        core::arch::asm!(
        "push esi","push edi","push ebx","mov esi, ecx","mov edi, [esi + 16]","mov ebx, [esi + 20]",
        "push dword ptr [esi + 24]","mov eax, [esi + 4]","mov ecx, [esi + 8]","mov edx, [esi + 12]",
        "call dword ptr [esi]","add esp, 4","mov [esi + 4], eax","mov [esi + 8], ecx","mov [esi + 12], edx",
        "pop ebx","pop edi","pop esi",in("ecx") frame.as_mut_ptr(),clobber_abi("C"));
    }
    *r = Registers {
        eax: frame[1],
        ecx: frame[2],
        edx: frame[3],
    };
}

register_entry!(
    0x0071_4740,
    fc4p_00714740,
    "cSC4TrafficSimulator::ApplyTrafficToDestination",
    apply,
    20,
    [24, 20, 16, 12, 8]
);
unsafe extern "fastcall" fn apply(
    this: *mut u8,
    out: *mut Registers,
    count: u32,
    x_word: u32,
    z_word: u32,
    last_word: u32,
    kind_word: u32,
) {
    unsafe {
        let mut r = out.read();
        let sim = TrafficSim::from_ptr(this);
        let x = i32::from(x_word as i16);
        let z = i32::from(z_word as i16);
        let last = u32::from(last_word as u8);
        let kind = u32::from(kind_word as u8);
        let mut connection_holder = 0u32;
        let holder;
        let mut rect = core::mem::MaybeUninit::<[i32; 4]>::uninit();
        r.eax = z as u32;
        if x >= sim.min_cell_x() && z >= sim.min_cell_z() && x <= sim.max_cell_x() && z <= sim.max_cell_z() {
            let manager = read_unaligned(lot_manager());
            r.edx = word(manager, 0);
            virtual_call(manager, 0x18, &[x as u32, z as u32, 0], &mut r);
            let lot = r.eax as *mut u8;
            if lot.is_null() {
                out.write(r);
                return;
            }
            r.eax = word(lot, 0);
            virtual_call(lot, 0x0C, &[], &mut r);
            holder = r.eax;
            r.edx = word(lot, 0);
            r.eax = rect.as_mut_ptr() as u32;
            virtual_call(lot, 0x38, &[r.eax], &mut r);
        } else {
            r.ecx = this as u32;
            call(0x0071_14E0, Conv::Thiscall, &[last, x as u32, z as u32], &mut r);
            holder = r.eax;
            r.eax = z as u32;
            r.ecx = this as u32;
            call(0x0071_04A0, Conv::Thiscall, &[last, x as u32, z as u32], &mut r);
            if r.eax != 0 {
                let connection = r.eax as *mut u8;
                r.edx = word(connection, 0);
                virtual_call(connection, 0x0C, &[], &mut r);
                connection_holder = r.eax;
                r.edx = (r.edx & !255) | kind;
                if kind < 3 {
                    let mut vector = [0u32; 3];
                    let property = 0xAA9E_A81Au32.wrapping_add(kind);
                    r.ecx = property;
                    r.edx = vector.as_mut_ptr() as u32;
                    call(0x005F_F510, Conv::Cdecl, &[r.eax, property, r.edx], &mut r);
                    if r.eax as u8 == 0 {
                        let zero = 0u32;
                        r.eax = &zero as *const u32 as u32;
                        r.ecx = vector.as_mut_ptr() as u32;
                        call(0x006B_DF10, Conv::Thiscall, &[9, r.eax], &mut r);
                    }
                    r.eax = vector[0];
                    r.edx = word(r.eax as *mut u8, last.wrapping_mul(4) as usize).wrapping_add(count);
                    r.ecx = connection_holder;
                    store(r.eax as *mut u8, last.wrapping_mul(4) as usize, r.edx);
                    r.eax = property;
                    r.edx = vector.as_mut_ptr() as u32;
                    call(0x005F_E6B0, Conv::Cdecl, &[r.ecx, property, r.edx], &mut r);
                    r.eax = vector[0];
                    if r.eax != 0 {
                        call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
                    }
                }
            }
            r.eax = z as u32;
            rect.write([x, z, x, z]);
        }
        if holder != 0 {
            r.edx = word(this, 0);
            virtual_call(this, 0x50, &[holder, kind], &mut r);
            let maximum = r.eax;
            r.ecx = this as u32;
            call(0x0070_DFF0, Conv::Thiscall, &[holder, kind], &mut r);
            let arrived = r.eax.wrapping_add(count);
            r.eax = count;
            let index = kind.wrapping_add(4);
            write_capacity(holder, arrived, index, &mut r);
            if connection_holder != 0 {
                write_capacity(connection_holder, arrived, index, &mut r);
            }
            if arrived >= maximum {
                let [x0, z0, x1, z1] = rect.assume_init();
                let bit = 1u32.wrapping_shl(kind);
                let mut cx = x0;
                while cx <= x1 {
                    let mut cz = z0;
                    while cz <= z1 {
                        let rows = word(this, 0x4C) as *mut u8;
                        let row = word(rows, cx.wrapping_mul(12).wrapping_add(12) as usize) as *mut u8;
                        let dest = row.wrapping_offset(cz.wrapping_add(1) as isize);
                        dest.write(dest.read() & !(bit as u8));
                        cz = cz.wrapping_add(1);
                    }
                    cx = cx.wrapping_add(1);
                }
                let offset = 0x1B0usize.wrapping_add(kind.wrapping_mul(4) as usize);
                store(this, offset, word(this, offset).wrapping_sub(1));
            }
        }
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[fc4p_00714740::PORT];

#[cfg(test)]
mod tests;
