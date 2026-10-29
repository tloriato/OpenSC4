// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Incremental volume, ratio and commercial maps, and publication of traffic statistics.

use super::finish_passes::{advance, scope, PassScan};
use super::late_containers::call;
use super::late_passes::virtual_call;
use super::*;
use crate::abi::Conv;

layout! {
    /// Services, grids and tuning read by the map creation pass.
    pub struct CreateMapsSim {
        0x30 => pollution / set_pollution: *mut u8,
        0x34 => peak / set_peak: *mut u8,
        0x3C => commercial / set_commercial: *mut u8,
        0x7C => network_rows / set_network_rows: *mut u8,
        0x164 => volumes / set_volumes: *mut u8,
        0x174 => ratio / set_ratio: *mut u8,
        0x704 => switches / set_switches: *mut u8,
        0x818 => cursor / set_cursor: *mut u8,
        0x97C => reduction / set_reduction: f32,
        0xD0C => commercial_scale / set_commercial_scale: f32,
    }
}
layout! {
    /// The map pass interprets the first eight bytes separately from the network query interface.
    pub struct MapNetworkInfo {
        0x00 => inverse_capacity / set_inverse_capacity: f32,
        0x04 => connections / set_connections: u8,
        0x05 => statistics_slot / set_statistics_slot: u8,
        0x06 => network / set_network: u8,
        0x07 => flags / set_flags: u8,
        0x14 => next / set_next: *mut u8,
    }
}
game_fn! { #[allow(dead_code)] fn budget_expired(this: *mut u32) -> u32 = "thiscall" 0x0068_BF80; }
game_fn! { #[allow(dead_code)] fn commercial_window(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0071_4AA0; }
game_fn! { fn curve(this: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { #[allow(dead_code)] fn add_accident(this: *mut u8, x: i32, z: i32, probability: f32) = "thiscall" 0x0071_6950; }
game_fn! { #[allow(dead_code)] fn transit_switch(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x006D_90F0; }
game_fn! { #[allow(dead_code)] fn roads_budget(this: *mut u8) = "thiscall" 0x0070_D8A0; }
game_fn! { #[allow(dead_code)] fn transit_budget(this: *mut u8) = "thiscall" 0x0070_DA40; }
game_fn! { #[allow(dead_code)] fn assign_accidents(this: *mut u8, source: *mut u8) = "thiscall" 0x007A_D7A0; }
game_data! { static pacing: [u32; 2] = 0x00B4_B348; }
game_data! { static initialized: u32 = 0x00B4_B350; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static rounding_bias: f32 = 0x00AB_32D8; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }

pub(super) unsafe fn cell<T>(vector: *mut u8, x: i32, z: i32) -> *mut T {
    unsafe {
        let rows = word(vector, 0) as *mut u8;
        let row = word(rows, x.wrapping_mul(12) as usize) as *mut T;
        row.wrapping_offset(z as isize)
    }
}

/// Compares the register value directly against a stored float, including signaling NaNs.
pub(super) fn compare_float(value: F80, operand: f32) -> u16 {
    let sw: u16;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand, out("ax") sw, out("st(0)") _);
    }
    sw
}
fn greater(value: F80, operand: f32) -> bool {
    compare_float(value, operand) & 0x4100 == 0
}
fn less(value: F80, operand: f32) -> bool {
    compare_float(value, operand) & 0x4500 == 0x0100
}
unsafe fn float(p: *mut u8, offset: usize) -> f32 {
    unsafe { f32::from_bits(word(p, offset)) }
}
unsafe fn put_float(p: *mut u8, offset: usize, value: f32) {
    unsafe { store(p, offset, value.to_bits()) }
}
unsafe fn increment_short(p: *mut u8) {
    unsafe {
        write_unaligned(p.cast::<u16>(), read_unaligned(p.cast::<u16>()).wrapping_add(1));
    }
}
unsafe fn unsigned(value: u32) -> F80 {
    let n = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        n
    } else {
        add_float(n, unsafe { read_unaligned(unsigned_bias()) })
    }
}

/// Copies the five statistic words in order, retaining all surrounding bytes.
unsafe fn publish(this: *mut u8, offset: usize) {
    unsafe {
        for i in 0..5 {
            store(this, offset - 0x25C + i * 4, word(this, offset + i * 4));
        }
    }
}

unsafe fn finish(this: *mut u8, argument: *mut u32, r: &mut Registers) {
    unsafe {
        for i in 0..13 {
            let row = 0x440 + i * 20;
            let count = word(this, 0x6B4 + i * 4);
            if count != 0 {
                argument.write(count);
                put_float(
                    this,
                    row,
                    reverse_divide_float(unsigned(count), float(this, row)).to_f32(),
                );
            }
            publish(this, row);
        }
        let count = word(this, 0x6E8);
        if count != 0 {
            argument.write(count);
            put_float(
                this,
                0x544,
                reverse_divide_float(unsigned(count), float(this, 0x544)).to_f32(),
            );
        }
        publish(this, 0x544);
        for i in 0..12 {
            let row = 0x578 + i * 20;
            let count = u32::from(read_unaligned(this.add(row).cast::<u16>()));
            if count != 0 {
                argument.write(count);
                put_float(
                    this,
                    row + 4,
                    reverse_divide_float(F80::from_i32(count as i32), float(this, row + 4)).to_f32(),
                );
            }
            publish(this, row);
        }
        for i in 0..13 {
            let count = read_unaligned(this.add(0x558 + i * 2).cast::<u16>());
            write_unaligned(this.add(0x2FC + i * 2).cast::<u16>(), count);
            store(this, 0x740 + i * 4, u32::from(count));
        }
        r.ecx = this as u32;
        call(0x0070_D8A0, Conv::Thiscall, &[], r);
        r.ecx = this as u32;
        call(0x0070_DA40, Conv::Thiscall, &[], r);
        r.ecx = this.add(0x13C) as u32;
        call(0x007A_D7A0, Conv::Thiscall, &[this.add(0x130) as u32], r);
        store(this, 0x134, word(this, 0x130));
        r.edx = 0;
    }
}

/// Inline adaptive pacing has distinct EAX behavior on completion and expiration.
unsafe fn paced(work: u32, completed: bool, r: &mut Registers) {
    unsafe {
        let count = work >> 10;
        if completed {
            r.eax = count;
        }
        r.ecx = 0;
        if count != 0 {
            if count as i32 > 20 {
                r.eax = word(pacing().cast(), 4).wrapping_add(1);
                store(pacing().cast(), 4, r.eax);
                if r.eax as i32 >= 16 {
                    r.eax = word(pacing().cast(), 0);
                    store(pacing().cast(), 4, 0);
                    if r.eax > 1 {
                        r.eax >>= 1;
                        store(pacing().cast(), 0, r.eax);
                    }
                }
            } else if (count as i32) < 5 {
                r.eax = word(pacing().cast(), 4).wrapping_sub(1);
                store(pacing().cast(), 4, r.eax);
                if r.eax as i32 <= -4 {
                    r.eax = word(pacing().cast(), 0);
                    store(pacing().cast(), 4, 0);
                    if r.eax < 1024 {
                        if completed {
                            r.eax = r.eax.wrapping_mul(2);
                            store(pacing().cast(), 0, r.eax);
                        } else {
                            r.ecx = r.eax.wrapping_mul(2);
                            store(pacing().cast(), 0, r.ecx);
                        }
                    }
                }
            }
        }
        r.eax = (r.eax & !255) | u32::from(completed);
    }
}

port! {
    /// Rebuilds maps one whole cell at a time, then publishes statistics and rolls the accident lists.
    /// The deadline scope and writable caller argument retain their original stack locations.
    #[port(address = 0x0071_A0C0, owner = "traffic", name = "cSC4TrafficSimulator::WorkOnCreateMaps", naked)]
    pub unsafe extern "C" fn fc4p_0071a0c0() {
        core::arch::naked_asm!(
            "sub esp, 0x60", "push ebp", "mov ebp, esp", "sub esp, 12", "lea edx, [esp]",
            "lea eax, [ebp + 52]", "push eax", "lea eax, [ebp + 104]", "push eax", "call {body}",
            "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
            "mov esp, ebp", "pop ebp", "add esp, 0x60", "ret 4", body = sym create_maps,
        );
    }
}

unsafe extern "fastcall" fn create_maps(
    this: *mut u8,
    out: *mut Registers,
    argument: *mut u32,
    scope_pointer: *mut [u32; 6],
) {
    unsafe {
        scope_pointer.write(scope(argument.read() as *mut u32, pacing(), initialized()));
        let budget = &mut *scope_pointer;
        let sim = CreateMapsSim::from_ptr(this);
        vcall!(sim.pollution(), 0x60 => fn() -> *mut u8);
        let peaks = vcall!(sim.peak(), 0x60 => fn() -> *mut u8);
        let commercial = vcall!(sim.commercial(), 0x60 => fn() -> *mut u8);
        let ratios = vcall!(sim.ratio(), 0x60 => fn() -> *mut u8);
        let mut volumes = [core::ptr::null_mut(); 6];
        for (i, volume) in volumes.iter_mut().enumerate() {
            *volume = vcall!(word(sim.volumes(), i * 4) as *mut u8, 0x60 => fn() -> *mut u8);
        }
        let switches = vcall!(sim.switches(), 0x60 => fn() -> *mut u8);
        let mut r = Registers::default();
        virtual_call(sim.commercial(), 0x30, &[], &mut r);
        let completed = loop {
            let work = budget[2].wrapping_add(budget[3]);
            budget[2] = work;
            if work & 1023 == 0 {
                r.ecx = budget.as_mut_ptr() as u32;
                call(0x0068_BF80, Conv::Thiscall, &[], &mut r);
                if r.eax as u8 != 0 {
                    break false;
                }
            }
            let scan = PassScan::from_ptr(sim.cursor());
            if !advance(scan, &mut r) {
                break true;
            }
            let (x, z) = (scan.x(), scan.z());
            let b = cell::<f32>(peaks, x, z);
            let d = cell::<f32>(ratios, x, z);
            write_unaligned(b, 0.0);
            write_unaligned(d, 0.0);
            if x & 3 == 0 && z & 3 == 0 {
                r.ecx = this as u32;
                call(0x0071_4AA0, Conv::Thiscall, &[(x >> 2) as u32, (z >> 2) as u32], &mut r);
                argument.write(r.eax & 0xFFFF);
                let v = multiply_float(F80::from_i32(argument.read() as i32), sim.commercial_scale());
                let n = add_float(v, read_unaligned(rounding_bias()))
                    .to_f32()
                    .to_bits()
                    .wrapping_add(0xB4C0_0000);
                argument.write(n);
                cell::<u8>(commercial, x >> 2, z >> 2).write(if n as i32 > 255 { 255 } else { n as u8 });
            }
            let mut found = false;
            for (network, &volume) in volumes.iter().enumerate() {
                let row = word(sim.network_rows(), x.wrapping_mul(12) as usize) as *mut u8;
                let mut info = word(row, z.wrapping_mul(4) as usize) as *mut u8;
                while !info.is_null() && MapNetworkInfo::from_ptr(info).network() as usize != network {
                    info = MapNetworkInfo::from_ptr(info).next();
                }
                if info.is_null() {
                    continue;
                }
                let entry = MapNetworkInfo::from_ptr(info);
                let vol = cell::<f32>(volume, x, z);
                if network != 2 && network != 3 && greater(F80::from_f32(read_unaligned(vol)), read_unaligned(b)) {
                    write_unaligned(b, read_unaligned(vol));
                }
                let ratio = multiply_float(F80::from_f32(read_unaligned(vol)), entry.inverse_capacity());
                let saved_ratio = ratio.to_f32();
                argument.write(saved_ratio.to_bits());
                if greater(ratio, read_unaligned(d)) {
                    write_unaligned(d, saved_ratio);
                }
                if (network == 1 || network == 2) && entry.connections() > 3 {
                    let p = curve(this.add(0xD30), saved_ratio);
                    let saved_p = p.to_f32();
                    if greater(p, read_unaligned(zero())) {
                        let capacity =
                            divide_float(F80::from_f32(read_unaligned(one())), entry.inverse_capacity()).to_f32();
                        let p = multiply_float(curve(this.add(0xD40), capacity), saved_p);
                        let saved_p = p.to_f32();
                        if greater(p, read_unaligned(zero())) {
                            r.ecx = this as u32;
                            call(
                                0x0071_6950,
                                Conv::Thiscall,
                                &[x as u32, z as u32, saved_p.to_bits()],
                                &mut r,
                            );
                        }
                    }
                }
                let (row, count, maximum, records) = if entry.flags() & 1 == 0 {
                    let slot = usize::from(entry.statistics_slot());
                    (0x440 + slot * 20, 0x6B4 + slot * 4, 0x67C + slot * 4, 0x558 + slot * 2)
                } else {
                    (0x544, 0x6E8, 0x6B0, 0x572)
                };
                let sum = add_float(F80::from_f32(saved_ratio), float(this, row));
                store(this, count, word(this, count).wrapping_add(1));
                put_float(this, row, sum.to_f32());
                if less(F80::from_f32(float(this, maximum)), saved_ratio) {
                    put_float(this, maximum, saved_ratio);
                    store(this, row + 12, z as u32);
                    store(this, row + 8, x as u32);
                    put_float(this, row + 16, saved_ratio);
                }
                if greater(F80::from_f32(saved_ratio), read_unaligned(one())) {
                    store(this, row + 4, word(this, row + 4).wrapping_add(1));
                }
                increment_short(this.add(records));
                write_unaligned(
                    vol,
                    multiply_float(F80::from_f32(sim.reduction()), read_unaligned(vol)).to_f32(),
                );
                if network == 1 {
                    let background = vcall_st0!(this, 0x2C, x, z);
                    write_unaligned(vol, add_float(background, read_unaligned(vol)).to_f32());
                }
                found = true;
            }
            let id = read_unaligned(cell::<u32>(switches, x, z));
            argument.write(id);
            if id != 0 {
                r.ecx = this.add(0x70C) as u32;
                call(0x006D_90F0, Conv::Thiscall, &[argument as u32], &mut r);
                let sw = r.eax as *mut u8;
                store(sw, 0x14, word(sw, 4));
                let ratio = divide_float(F80::from_f32(float(sw, 4)), float(sw, 12));
                let saved_ratio = ratio.to_f32();
                argument.write(saved_ratio.to_bits());
                if greater(ratio, read_unaligned(d)) {
                    write_unaligned(d, saved_ratio);
                }
                let capped = if less(F80::from_f32(read_unaligned(one())), saved_ratio) {
                    1.0
                } else {
                    saved_ratio
                };
                put_float(sw, 0x18, capped);
                argument.write(capped.to_bits());
                for i in 0..12 {
                    let row = 0x588 + i * 20;
                    if u32::from(sw.add(0x10).read()) & (1u32 << i) == 0 {
                        continue;
                    }
                    put_float(
                        this,
                        row - 12,
                        add_float(F80::from_f32(capped), float(this, row - 12)).to_f32(),
                    );
                    increment_short(this.add(row - 16));
                    if less(F80::from_f32(capped), float(this, row)) {
                        put_float(this, row, capped);
                        store(this, row - 8, x as u32);
                        store(this, row - 4, z as u32);
                    }
                }
            }
            if (id != 0 || found) && less(F80::from_f32(read_unaligned(b)), read_unaligned(one())) {
                write_unaligned(b, 1.0);
            }
        };
        if completed {
            finish(this, argument, &mut r);
        }
        paced(budget[2], completed, &mut r);
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[fc4p_0071a0c0::PORT];

#[cfg(test)]
mod tests;
