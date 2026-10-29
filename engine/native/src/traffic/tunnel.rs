// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tunnel portal changes, network selection and per-travel-type costs.

use super::late_containers::call;
use super::late_passes::virtual_call;
use super::*;
use crate::abi::Conv;

layout! {
    /// City service used to convert portal positions to cell coordinates.
    pub struct TunnelSim {
        0x14 => city / set_city: *mut u8,
    }
}

game_fn! {#[allow(dead_code)] fn paired_portal(this:*mut u8)->*mut u8="thiscall" 0x0064_7570;}
game_fn! {#[allow(dead_code)] fn erase(this:*mut u8,key:*const u32)="thiscall" 0x0071_0910;}
game_fn! {#[allow(dead_code)] fn entry(this:*mut u8,key:*const u32)->*mut u8="thiscall" 0x0071_3D60;}
game_data! {static network_masks:[u32;13]=0x00AB_3174;}
game_data! {static inverse_speeds:[[f32;13];9]=0x00B4_B110;}
game_data! {static cost_scale:f32=0x00A8_E174;}

port! {
    /// The 44-byte record includes two retained padding bytes immediately below the entry stack
    /// pointer. Reserve the original local area so these bytes are copied from the same locations.
    #[port(address=0x0071_40E0,owner="traffic",name="cSC4TrafficSimulator::DoTunnelChanged",naked)]
    pub unsafe extern "C" fn fc4p_007140e0(){
        core::arch::naked_asm!(
            "sub esp, 0x54","push ebx","push esi","push edi",
            "sub esp, 12","mov [esp], eax","mov [esp + 4], ecx","mov [esp + 8], edx","lea edx, [esp + 12]",
            "push dword ptr [esp + 116]","push dword ptr [esp + 116]","call {body}",
            "mov eax, [esp]","mov ecx, [esp + 4]","mov edx, [esp + 8]",
            "add esp, 12","pop edi","pop esi","pop ebx","add esp, 0x54","ret 8",body=sym changed);
    }
}

unsafe extern "fastcall" fn changed(this: *mut u8, frame: *mut u8, occupant: *mut u8, inserted: u32) {
    unsafe {
        let mut r = read_unaligned(frame.sub(12).cast::<Registers>());
        let position = frame.add(0x1C).cast::<f32>();
        let other_position = frame.add(0x28).cast::<f32>();
        let x = frame.add(12).cast::<i32>();
        let z = frame.add(16).cast::<i32>();
        let other_x = frame.add(20).cast::<i32>();
        let other_z = frame.add(24).cast::<i32>();
        r.eax = word(occupant, 4);
        r.edx = position as u32;
        virtual_call(occupant.add(4), 0x20, &[position as u32], &mut r);
        let city = TunnelSim::from_ptr(this).city();
        r.eax = word(city, 0);
        r.edx = x as u32;
        virtual_call(
            city,
            0x17C,
            &[
                position.cast::<u32>().read_unaligned(),
                position.cast::<u32>().add(2).read_unaligned(),
                x as u32,
                z as u32,
            ],
            &mut r,
        );
        let key = frame.add(100).cast::<u32>();
        key.write_unaligned(((x.read_unaligned() as u32 & 255) << 8) | z.read_unaligned() as u32);
        r.eax = (key.read_unaligned() & !255) | (inserted & 255);
        if inserted as u8 == 0 {
            r.ecx = this.add(0xC8) as u32;
            call(0x0071_0910, Conv::Thiscall, &[key as u32], &mut r);
            write_unaligned(frame.sub(12).cast::<Registers>(), r);
            return;
        }
        r.ecx = occupant as u32;
        call(0x0064_7570, Conv::Thiscall, &[], &mut r);
        let paired = r.eax as *mut u8;
        if paired.is_null() {
            write_unaligned(frame.sub(12).cast::<Registers>(), r);
            return;
        }
        r.edx = word(paired, 0);
        virtual_call(paired, 4, &[], &mut r);
        r.eax = word(paired, 0);
        virtual_call(paired, 0xC0, &[], &mut r);
        let other = r.eax as *mut u8;
        r.edx = word(other, 0);
        r.ecx = other_position as u32;
        virtual_call(other, 0x20, &[other_position as u32], &mut r);
        let city = TunnelSim::from_ptr(this).city();
        r.edx = word(city, 0);
        r.eax = other_x as u32;
        virtual_call(
            city,
            0x17C,
            &[
                other_position.cast::<u32>().read_unaligned(),
                other_position.cast::<u32>().add(2).read_unaligned(),
                other_x as u32,
                other_z as u32,
            ],
            &mut r,
        );
        r.edx = word(paired, 0);
        virtual_call(paired, 8, &[], &mut r);
        let record = frame.add(0x34);
        record.write(other_x.read_unaligned() as u8);
        record.add(1).write(other_z.read_unaligned() as u8);
        let mut network = None;
        for kind in [2u32, 6, 0, 1, 10, 12, 9] {
            if kind == 6 || kind == 1 || kind == 12 {
                r.eax = word(occupant, 0);
            } else {
                r.edx = word(occupant, 0);
            }
            virtual_call(occupant, 0x58, &[kind], &mut r);
            if r.eax as u8 != 0 {
                network = Some(kind);
                break;
            }
        }
        let Some(network) = network else {
            write_unaligned(frame.sub(12).cast::<Registers>(), r);
            return;
        };
        let mask = read_unaligned(network_masks().cast::<u32>().add(network as usize));
        write_unaligned(record.add(2).cast::<u16>(), 0x1FE);
        write_unaligned(record.add(40).cast::<u16>(), network as u16);
        let dx = subtract_float(
            F80::from_f32(position.read_unaligned()),
            other_position.read_unaligned(),
        );
        let dy = subtract_float(
            F80::from_f32(position.add(1).read_unaligned()),
            other_position.add(1).read_unaligned(),
        );
        let dz = subtract_float(
            F80::from_f32(position.add(2).read_unaligned()),
            other_position.add(2).read_unaligned(),
        );
        let distance = ((dz * dz + dy * dy) + dx * dx).sqrt();
        for travel in 0..9 {
            let speed = read_unaligned(inverse_speeds().cast::<f32>().add(travel * 13 + network as usize));
            let cost = multiply_float(multiply_float(distance, speed), read_unaligned(cost_scale())).to_f32();
            write_unaligned(record.add(4 + travel * 4).cast::<f32>(), cost);
        }
        r.eax = key as u32;
        r.ecx = this.add(0xC8) as u32;
        r.edx = mask | 0x10;
        call(0x0071_3D60, Conv::Thiscall, &[key as u32], &mut r);
        // A word copy retains the record's two padding bytes without interpreting them as Rust values.
        let destination = r.eax as *mut u8;
        let mut count = 11u32;
        core::arch::asm!("push esi","mov esi, eax","rep movsd","pop esi",
            in("eax") record,inout("edi") destination=>_,inout("ecx") count);
        r.ecx = count;
        write_unaligned(frame.sub(12).cast::<Registers>(), r);
    }
}

pub const PORTS: &[Port] = &[fc4p_007140e0::PORT];

#[cfg(test)]
mod tests;
