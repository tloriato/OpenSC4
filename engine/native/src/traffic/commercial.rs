// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Commercial traffic's per-cell maximum of wrapping network totals.

use super::late_containers::{call, register_entry};
use super::late_passes::remainder;
use super::*;
use crate::abi::Conv;

game_fn! { #[allow(dead_code)] fn fill_words(this: *mut u8, position: u32, count: u32, initial: *const u16) = "thiscall" 0x0046_5760; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_data! { static travel_networks: [i32; 9] = 0x00AB_31A8; }

register_entry!(
    0x0071_4940,
    fc4p_00714940,
    "CommercialTrafficCellValue",
    commercial_value,
    12,
    [16, 12, 8]
);
unsafe extern "fastcall" fn commercial_value(this: *mut u8, out: *mut Registers, x: u32, z: u32, evening: u32) {
    unsafe {
        let mut r = out.read();
        let initial = 0u32;
        let mut vector = [0u32; 3];
        r.eax = &initial as *const u32 as u32;
        r.ecx = vector.as_mut_ptr() as u32;
        call(0x0046_5760, Conv::Thiscall, &[0, 6, r.eax], &mut r);
        for travel in 0..9 {
            let network = read_unaligned(travel_networks().cast::<i32>().add(travel));
            if network == 1 || (network > 3 && network <= 5) {
                let key = ((evening & 255) << 24) | ((travel as u32) << 16) | ((x & 255) << 8) | (z & 255);
                let buckets = word(this, 0x17C) as *mut u8;
                let count = (word(this, 0x180).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
                let mut node = word(buckets, remainder(key, count).wrapping_mul(4) as usize) as *mut u8;
                while !node.is_null() && word(node, 4) != key {
                    node = word(node, 0) as *mut u8;
                }
                if !node.is_null() {
                    let slot = (vector[0] as *mut u16).wrapping_offset(network as isize);
                    slot.write_unaligned(
                        slot.read_unaligned()
                            .wrapping_add(read_unaligned(node.add(40).cast::<u16>())),
                    );
                }
            }
        }
        let values = vector[0] as *mut u16;
        let mut scratch = 0u32;
        let mut selected = if values.read_unaligned() > 0 {
            values
        } else {
            (&mut scratch as *mut u32).cast::<u16>()
        };
        for i in 1..6 {
            r.edx = u32::from(selected.read_unaligned());
            scratch = r.edx;
            selected = if (r.edx as u16) < values.add(i).read_unaligned() {
                values.add(i)
            } else {
                (&mut scratch as *mut u32).cast::<u16>()
            };
        }
        let maximum = selected.read_unaligned();
        r.ecx = selected as u32;
        // Only AX is replaced after the allocator's free; EAX's high half is observable.
        r.eax = vector[0];
        call(0x0090_CF63, Conv::Cdecl, &[vector[0]], &mut r);
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(maximum);
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[fc4p_00714940::PORT];

#[cfg(test)]
mod tests;
