// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Month boundaries, pole decay, and repair.

use super::*;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn curve_output(curve: *mut u8, x: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { fn copy_poles(out: *mut u32, source: *mut u8) -> u32 = "thiscall" 0x004B_03E0; }
game_fn! { fn shuffle_poles(begin: *mut *mut u8, end: *mut *mut u8) = "C" 0x006D_B010; }
game_fn! { fn free_poles(begin: *mut *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn apply_pole_funding(this: *mut u8, funding: *const f32) -> u32 = "thiscall" 0x006E_C3F0; }
game_data! { static ordinances: *mut u8 = 0x00B4_3D90; }
game_data! { static zero_percent: u32 = 0x00B4_A5B0; }

stored_op!(subtract_from_float, "fsubr dword ptr [{operand}]");

port! {
    /// Subtract the funding curve from the broken fraction, clamp ordered values to [0, 1], and
    /// truncate the unsigned pole count times that stored fraction. NaN remains NaN and yields target 0.
    /// If the target differs, copy and shuffle the poles through the game, visit until it is reached,
    /// and free the copy. Coordinates are queried even when a pole needs no change.
    /// The funding argument word is reused as the pole coordinate's second output. EAX is the target
    /// on the early exit, otherwise the last count or the storage release result.
    #[port(address = 0x006E_C3F0, owner = "power", name = "cSC4PowerSimulator::ApplyPoleFunding", naked)]
    pub unsafe extern "C" fn fc4p_006ec3f0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {body}", body = sym funding_body);
    }
}

unsafe extern "fastcall" fn funding_body(this: *mut u8, argument: *mut i32, funding: *const f32) -> u32 {
    // SAFETY: the simulator, funding pointer, pole interfaces and game vector storage are live.
    unsafe {
        let sim = PowerSim::from_ptr(this);
        let x = multiply_float(F80::from_f32(funding.read_unaligned()), 100.0).to_f32();
        let delta = curve_output(this.add(0x3B8), x);
        let fraction = subtract_from_float(delta, sim.broken_fraction());
        let upper = fraction.compare(F80::from_f64(1.0));
        sim.set_broken_fraction(fraction.to_f32());
        if !upper.less() && !upper.equal() && !upper.unordered() {
            sim.set_broken_fraction(1.0);
        } else if fraction.compare(F80::from_f64(0.0)).less() {
            sim.set_broken_fraction(0.0);
        }
        let count = sim.pole_count();
        let mut total = F80::from_i32(count as i32);
        if (count as i32) < 0 {
            total = add_float(total, 4_294_967_296.0);
        }
        let target = multiply_float(total, sim.broken_fraction()).ftol() as u32;
        if target == sim.broken_count() {
            return target;
        }
        let mut copy = MaybeUninit::<[u32; 3]>::uninit();
        let vector = copy.as_mut_ptr().cast::<u32>();
        copy_poles(vector, this.add(0x29C));
        let begin = vector.read() as *mut *mut u8;
        let end = vector.add(1).read() as *mut *mut u8;
        shuffle_poles(begin, end);
        let mut eax = vector.add(1).read();
        let mut cursor = begin;
        let mut release_begin = begin;
        while cursor != vector.add(1).read() as *mut *mut u8 {
            if target == sim.broken_count() {
                break;
            }
            let pole = cursor.read_unaligned();
            let mut x = MaybeUninit::<i32>::uninit();
            cursor = cursor.wrapping_add(1);
            vcall!(pole, 0x20 => fn(*mut i32, *mut i32), x.as_mut_ptr(), argument);
            let distressed = vcall!(pole, 0x2C => fn() -> u32) as u8 != 0;
            eax = sim.broken_count();
            if distressed && eax > target {
                vcall!(pole, 0x30 => fn(u32), 0);
                eax = sim.broken_count().wrapping_sub(1);
                sim.set_broken_count(eax);
            } else if !distressed && eax < target {
                vcall!(pole, 0x30 => fn(u32), 1);
                eax = sim.broken_count().wrapping_add(1);
                sim.set_broken_count(eax);
            }
            release_begin = vector.read() as *mut *mut u8;
            if cursor == vector.add(1).read() as *mut *mut u8 {
                break;
            }
        }
        if !release_begin.is_null() {
            eax = free_poles(release_begin);
        }
        eax
    }
}
/// The ring row calculation wraps like the game's word-sized address arithmetic.
fn ring_cell(this: *mut u8, index: i32, kind: usize) -> *mut i32 {
    this.wrapping_add(0x90)
        .wrapping_offset(index.wrapping_mul(40) as isize)
        .wrapping_add(kind * 4)
        .cast()
}

port! {
    /// Refresh the ordinance factor, advance the twelve-month ring, subtract and clear its ten entries,
    /// store the integer deficit as a float, and add back the current row. Mark both pending flags
    /// before querying department funding through the simulator and applying pole funding by address.
    /// Negative ring indices retain their signed increment; only values >= 12 reset to zero.
    #[port(address = 0x006E_C570, owner = "power", name = "cSC4PowerSimulator::DoMonthlySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006ec570(this: *mut u8) -> u32 {
        // SAFETY: the game supplies the simulator and storage reached through its signed ring index.
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let factor = vcall_st0!(*ordinances(), 0x64, 0x0911_E117u32, 0u32);
            sim.set_ordinance_factor(factor.to_f32());
            let index = sim.ring_index().wrapping_add(1);
            sim.set_ring_index(index);
            if index >= 12 { sim.set_ring_index(0); }
            for kind in 0..10 {
                let annual = this.add(0x270 + kind * 4).cast::<i32>();
                let value = read_unaligned(ring_cell(this, sim.ring_index(), kind));
                write_unaligned(annual, read_unaligned(annual).wrapping_sub(value));
                write_unaligned(ring_cell(this, sim.ring_index(), kind), 0);
            }
            let (consumption, production) = (sim.consumption(), sim.production());
            if consumption > production && production > 0 {
                let quotient = production.wrapping_sub(consumption) / production;
                sim.set_deficit(F80::from_i32(quotient).to_f32().to_bits());
            } else {
                sim.set_deficit(*zero_percent());
            }
            for kind in 0..10 {
                let annual = this.add(0x270 + kind * 4).cast::<i32>();
                let value = read_unaligned(ring_cell(this, sim.ring_index(), kind));
                write_unaligned(annual, read_unaligned(annual).wrapping_add(value));
            }
            sim.set_recompute(1);
            sim.set_age_plants(1);
            let funding = vcall!(this, 0xC0 => fn() -> *const f32);
            apply_pole_funding(this, funding)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006ec3f0::PORT, fc4p_006ec570::PORT];

#[cfg(test)]
mod tests;
