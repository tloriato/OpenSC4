// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Periodic visible-cell accident checks, iterator lifetime and queued notification.

use super::*;

layout! {
    /// Accident data and its independent elapsed-time deadline.
    pub struct AccidentSim {
        0x1C => networks / set_networks: *mut u8,
        0x13C => entries / set_entries: *mut u8,
        0x140 => entries_end / set_entries_end: *mut u8,
        0x160 => deadline / set_deadline: u32,
        0xD50 => duration / set_duration: f32,
        0xD54 => period / set_period: f32,
    }
}

game_fn! { fn elapsed(this: *mut u8) -> u32 = "thiscall" 0x0089_0198; }
game_fn! { fn reset(this: *mut u8, randomize: u32) = "thiscall" 0x005C_F610; }
game_fn! { fn next(this: *mut u8, index: *mut u32) -> u32 = "thiscall" 0x005C_BD90; }
game_fn! { fn random(this: *mut u8, min_low: u32, min_high: u32, max_low: u32, max_high: u32) -> St0 = "thiscall" 0x0091_3A46; }
game_fn! { fn create(this: *mut u8, controller: *mut u8) = "thiscall" 0x0055_4400; }
game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_data! { static view: *mut u8 = 0x00B4_3DD8; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static messages: *mut u8 = 0x00B4_3CCC; }
game_data! { static cell_scale: f32 = 0x00A8_E174; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static bias: f32 = 0x00AB_32D8; }
game_data! { static milliseconds: f32 = 0x00A8_FAF0; }
game_data! { static minimum: f64 = 0x00A8_0990; }
game_data! { static maximum: f64 = 0x00A8_0AB0; }

// Preserve the register roles of the half-cell adjustment, then make the quiet comparison used to
// correct an integral boundary. The integer candidate is an exact FILD, with no float-to-int shortcut.
fn bound(world: f32, upper: bool, scale: f32, half: f32, bias: f32) -> i32 {
    let scaled = multiply_float(F80::from_f32(world), scale);
    let mut adjusted = F80::ZERO;
    unsafe {
        if upper {
            core::arch::asm!("fld tbyte ptr [{scaled}]", "fld dword ptr [{half}]", "fadd st(0), st(1)",
                "fstp tbyte ptr [{out}]", "fstp st(0)", scaled = in(reg) &scaled,
                half = in(reg) &half, out = in(reg) &mut adjusted, out("st(0)") _, out("st(1)") _);
        } else {
            core::arch::asm!("fld tbyte ptr [{scaled}]", "fld dword ptr [{half}]", "fsubr st(0), st(1)",
                "fstp tbyte ptr [{out}]", "fstp st(0)", scaled = in(reg) &scaled,
                half = in(reg) &half, out = in(reg) &mut adjusted, out("st(0)") _, out("st(1)") _);
        }
    }
    let rounded = add_float(adjusted, bias).to_f32().to_bits().wrapping_add(0xB4C0_0000) as i32;
    let candidate = rounded.wrapping_add(if upper { -1 } else { 1 });
    let sw: u16;
    unsafe {
        core::arch::asm!("fild dword ptr [{candidate}]", "fld tbyte ptr [{scaled}]", "fucompp", "fnstsw ax",
            candidate = in(reg) &candidate, scaled = in(reg) &scaled, out("ax") sw,
            out("st(0)") _, out("st(1)") _);
    }
    if sw & 0x4400 == 0x4000 {
        candidate
    } else {
        rounded
    }
}

port! {
    /// Always reads elapsed time. A strictly later unsigned time advances the deadline, converts the
    /// view bounds, and visits the randomized index iterator. Every visit draws before rejecting the
    /// probability or visibility. The first live occupant creates an accident and ends the scan.
    #[port(address = 0x0071_84E0, owner = "traffic", name = "cSC4TrafficSimulator::CheckForAccident")]
    pub unsafe extern "thiscall" fn fc4p_007184e0(this: *mut u8) {
        unsafe {
            let sim = AccidentSim::from_ptr(this);
            let now = elapsed(this.add(0x148));
            if now <= sim.deadline() { return; }
            sim.set_deadline((F80::from_f32(sim.period()).ftol() as u32).wrapping_add(now));
            let rect = (*view()).add(0x3EC).cast::<[f32; 4]>().read_unaligned();
            let x0 = bound(rect[0], false, *cell_scale(), *half(), *bias());
            let x1 = bound(rect[2], true, *cell_scale(), *half(), *bias());
            let z0 = bound(rect[1], false, *cell_scale(), *half(), *bias());
            let z1 = bound(rect[3], true, *cell_scale(), *half(), *bias());
            let count = (sim.entries_end() as u32).wrapping_sub(sim.entries() as u32) as i32 / 12;
            let mut iterator = [0u32; 8];
            iterator[0] = 0x0001_0101;
            iterator[4] = count as u32;
            iterator[7] = 1;
            let it = iterator.as_mut_ptr().cast();
            reset(it, 1);
            let mut index = 0u32;
            while next(it, &mut index) as u8 != 0 {
                let hi = maximum().read().to_bits();
                let lo = minimum().read().to_bits();
                let random = random(TrafficSim::from_ptr(this).random(), lo as u32, (lo >> 32) as u32, hi as u32, (hi >> 32) as u32);
                let entry = sim.entries().wrapping_add(index.wrapping_mul(12) as usize);
                let probability = f32::from_bits(word(entry, 8));
                if !random.compare(F80::from_f32(probability)).less() { continue; }
                let x = word(entry, 0) as i32;
                let z = word(entry, 4) as i32;
                if x < x0 || z < z0 || x > x1 || z > z1 { continue; }
                let manager = vcall!(*city(), 0xAC => fn() -> *mut u8);
                let z = word(entry, 4) as i32;
                let x = word(entry, 0) as i32;
                let holder = vcall!(sim.networks(), 0x38 => fn(i32, i32) -> *mut u8, x, z);
                if holder.is_null() { continue; }
                let occupant = word(holder, 0) as *mut u8;
                if occupant.is_null() { continue; }
                let duration = multiply_float(F80::from_f32(sim.duration()), *milliseconds()).ftol() as u32;
                let controller = vcall!(occupant, 0xC0 => fn(u32, u32) -> *mut u8, 2, duration);
                create(manager, controller);
                let server = *messages();
                if !server.is_null() {
                    let memory = allocate_message(0x2C);
                    let msg = if memory.is_null() { memory } else { construct_message(memory) };
                    if !msg.is_null() { vcall!(msg, 4 => fn()); }
                    vcall!(msg, 0x14 => fn(u32), 0x0C05_E4C3);
                    vcall!(msg, 0x2C => fn(u32), word(entry, 0));
                    vcall!(msg, 0x40 => fn(u32), word(entry, 4));
                    vcall!(server, 0x10 => fn(*mut u8, u32), msg, 0);
                    vcall!(msg, 8 => fn());
                }
                break;
            }
            let allocation = iterator[1] as *mut u8;
            if !allocation.is_null() { free(allocation); }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_007184e0::PORT];

#[cfg(test)]
mod tests;
