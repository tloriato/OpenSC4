// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Plopped-lot constraints, slope, occupied cells and outward network connections.

use super::{Port, ZoneDeveloper};
use crate::{
    abi::{call_raw, vslot, Conv, RawCall},
    x87::F80,
};
use core::ptr::read_unaligned;

game_data! { static network_flags: u32 = 0x00AB_3680; }
game_data! { static neighbour_offsets: i32 = 0x00AB_3780; }
game_fn! {
    fn constraints(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, facing: i32, cfg: *mut u8) -> u64
        = "thiscall" 0x0072_B480;
}
game_fn! { fn includes(this: *mut u8, x: i32, z: i32) -> u64 = "thiscall" 0x0046_6610; }
game_fn! { fn network(this: *mut u8, x: i32, z: i32, flags: u32) -> u64 = "thiscall" 0x0072_FCA0; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn placement(cfg: *mut u8, record: *mut u8, origin: *const i32, facing: i32,
        x: *mut i32, z: *mut i32, mask_a: *mut u32, mask_b: *mut u32, kind: *mut i32) -> u64
        = "C" 0x0072_AD00;
}
game_fn! { fn matches(mask: u32, edges: u32) -> u64 = "C" 0x0072_9EC0; }

unsafe fn integer(this: *mut u8, slot: usize, eax: u32, edx: &mut u32, args: &[u32]) -> u32 {
    unsafe {
        let result = super::virtual_registers(this, slot, eax, *edx, args);
        *edx = (result >> 32) as u32;
        result as u32
    }
}

unsafe fn float(this: *mut u8, slot: usize, eax: u32, edx: &mut u32, args: &[u32]) -> F80 {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(this, slot), this as u32, args);
        call.eax = eax;
        call.edx = *edx;
        call.take_st0 = 1;
        call_raw(&mut call);
        *edx = call.edx;
        call.st0
    }
}

/// Compare the unrounded maximum in ST0 against the rounded area slope in memory, then pop it.
fn too_steep(maximum: F80, slope: f32) -> bool {
    let status: u16;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{maximum}]", "fcomp dword ptr [{slope}]", "fnstsw ax",
            maximum = in(reg) &maximum, slope = in(reg) &slope, out("ax") status, out("st(0)") _);
    }
    status & 0x4500 == 0x0100
}

port! {
    /// Validate the by-value rectangle through the lot-manager interface. Retain the caller's
    /// scratch word above the water flag, whose low byte alone is replaced before the slope call.
    /// All placement logic and return-register handling live in the Rust body.
    #[port(address = 0x0073_06C0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::CanCreatePloppedLot", naked)]
    pub unsafe extern "C" fn fc4p_007306c0() {
        core::arch::naked_asm!("mov edx, [esp - 0x1c]", "jmp {body}", body = sym validate);
    }
}

unsafe extern "fastcall" fn validate(
    this: *mut u8,
    water_word: u32,
    x1: i32,
    z1: i32,
    x2: i32,
    z2: i32,
    facing: i32,
    cfg: *mut u8,
) -> u64 {
    // SAFETY: the game's lot-manager interface, configuration and rectangle.
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this.sub(4));
        let mut rect = [x1, z1, x2, z2];
        let mut edx = 0;
        let cfg_table = read_unaligned(cfg.cast::<u32>());
        let water = integer(cfg, 0x38, cfg_table, &mut edx, &[]) as u8;
        if water != 0 {
            let checked = constraints(dev.as_ptr(), rect[0], rect[1], rect[2], rect[3], facing, cfg);
            if checked as u32 != 8 {
                return checked;
            }
        }
        let lot_dev = dev.lot_configuration();
        edx = read_unaligned(lot_dev.cast::<u32>());
        let slope = float(
            lot_dev,
            0x74,
            rect.as_ptr() as u32,
            &mut edx,
            &[rect.as_mut_ptr() as u32, (water_word & !0xFF) | u32::from(water)],
        )
        .to_f32();
        edx = read_unaligned(cfg.cast::<u32>());
        let max = float(cfg, 0x20, 0, &mut edx, &[]);
        if too_steep(max, slope) {
            return (u64::from(edx) << 32) | 4;
        }

        let mut result = 8i32;
        let mut x = rect[0];
        while x <= rect[2] {
            let mut z = rect[1];
            while z <= rect[3] {
                let code;
                if x < 0
                    || x >= dev.cells_x()
                    || z < 0
                    || z >= dev.cells_z()
                    || (!dev.restriction().is_null() && {
                        let value = includes(dev.restriction(), x, z);
                        edx = (value >> 32) as u32;
                        value as u8 == 0
                    })
                {
                    code = 0;
                } else if water == 0
                    && integer(
                        dev.as_ptr(),
                        0x24,
                        read_unaligned(dev.as_ptr().cast()),
                        &mut edx,
                        &[x as u32, z as u32],
                    ) as u8
                        != 0
                {
                    code = 1;
                } else {
                    let found = network(dev.as_ptr(), x, z, 0x1F4F);
                    let occupant = found as u32 as *mut u8;
                    if occupant.is_null() {
                        let zones = dev.zone_manager();
                        edx = this as u32;
                        let zone = integer(
                            zones,
                            0x30,
                            read_unaligned(zones.cast()),
                            &mut edx,
                            &[x as u32, z as u32],
                        );
                        edx = 1u32.wrapping_shl(zone);
                        if edx & 0xFC00 != 0 {
                            edx = (!(edx >> 13) & 4) | 3;
                            code = edx as i32;
                        } else {
                            code = 8;
                        }
                    } else {
                        edx = read_unaligned(occupant.cast::<u32>());
                        if integer(occupant, 0x20, occupant as u32, &mut edx, &[0x20_0000]) as u8 != 0 {
                            code = 3;
                        } else {
                            let vector =
                                integer(cfg, 0x5C, read_unaligned(cfg.cast()), &mut edx, &[]) as *const *mut u8;
                            let mut record = read_unaligned(vector);
                            let end = read_unaligned(vector.add(1));
                            let mut current = 3;
                            while record != end {
                                if *record == 7 {
                                    let (mut px, mut pz, mut a, mut b, mut kind) = (0, 0, 0, 0, 0);
                                    let placed = placement(
                                        cfg,
                                        record,
                                        rect.as_ptr(),
                                        facing,
                                        &mut px,
                                        &mut pz,
                                        &mut a,
                                        &mut b,
                                        &mut kind,
                                    );
                                    edx = (placed >> 32) as u32;
                                    if px == x && pz == z {
                                        edx = read_unaligned(occupant.cast());
                                        let flags = integer(occupant, 0x30, px as u32, &mut edx, &[]) & 0x1FFF;
                                        edx = read_unaligned(network_flags().wrapping_offset(kind as isize));
                                        if flags != edx {
                                            break;
                                        }
                                        edx = read_unaligned(occupant.cast());
                                        let edges = integer(occupant, 0x98, kind as u32, &mut edx, &[kind as u32]);
                                        let first = matches(a, edges);
                                        edx = (first >> 32) as u32;
                                        if first as u8 != 0 {
                                            // The second comparison deliberately uses the piece as its mask.
                                            let second = matches(edges, b);
                                            edx = (second >> 32) as u32;
                                            if second as u8 != 0 {
                                                current = 8;
                                            }
                                        }
                                    }
                                }
                                record = record.wrapping_add(0x34);
                            }
                            code = current;
                        }
                    }
                }
                result = result.min(code);
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        if result == 8 {
            edx = read_unaligned(cfg.cast());
            let vector = integer(cfg, 0x5C, rect[2] as u32, &mut edx, &[]) as *const *mut u8;
            let mut record = read_unaligned(vector);
            let end = read_unaligned(vector.add(1));
            while record != end {
                if *record == 7 {
                    let (mut px, mut pz, mut a, mut b, mut kind) = (0, 0, 0, 0, 0);
                    placement(
                        cfg,
                        record,
                        rect.as_ptr(),
                        facing,
                        &mut px,
                        &mut pz,
                        &mut a,
                        &mut b,
                        &mut kind,
                    );
                    for edge in 0..4 {
                        let nx = px.wrapping_add(read_unaligned(neighbour_offsets().add(edge + 1)));
                        let nz = pz.wrapping_add(read_unaligned(neighbour_offsets().add(edge)));
                        edx = nz as u32;
                        if nx as u32 >= dev.cells_x() as u32 || nz as u32 >= dev.cells_z() as u32 {
                            continue;
                        }
                        if nx >= rect[0] && nz >= rect[1] && nx <= rect[2] && nz <= rect[3] {
                            continue;
                        }
                        if b & 0xFFu32.wrapping_shl(edge as u32 * 8) == 0 {
                            continue;
                        }
                        let mut flags = read_unaligned(network_flags().wrapping_offset(kind as isize));
                        if flags & 0x1F4F != 0 {
                            flags = 0x1F4F;
                        }
                        let found = network(dev.as_ptr(), nx, nz, flags);
                        edx = (found >> 32) as u32;
                        let occupant = found as u32 as *mut u8;
                        if !occupant.is_null() {
                            edx = read_unaligned(occupant.cast());
                            if integer(occupant, 0x24, occupant as u32, &mut edx, &[0x60_0000]) as u8 != 0 {
                                result = result.min(3);
                                break;
                            }
                        }
                    }
                }
                record = record.wrapping_add(0x34);
            }
        }
        (u64::from(edx) << 32) | u64::from(result as u32)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_007306c0::PORT];

#[cfg(test)]
mod tests;
