// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population, quotient and age changes for one residential tract.

use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Fields used by the per-tract pass, relative to the primary interface.
    pub struct TractSim {
        0x28 => low_developer / set_low_developer: *mut u8,
        0x2C => medium_developer / set_medium_developer: *mut u8,
        0x30 => high_developer / set_high_developer: *mut u8,
        0x48 => low_population / set_low_population: *mut u8,
        0x4C => medium_population / set_medium_population: *mut u8,
        0x50 => high_population / set_high_population: *mut u8,
        0x54 => age_grid / set_age_grid: *mut u8,
        0x7C => eq_grids / set_eq_grids: [*mut u8;3],
        0x88 => hq_grids / set_hq_grids: [*mut u8;3],
        0xA8 => running_population / set_running_population: i32,
        0x1C4 => age_counts / set_age_counts: [i32;20],
        0x214 => age_eq_sums / set_age_eq_sums: [i32;20],
        0x348 => life_low / set_life_low: u32,
        0x34C => life_high / set_life_high: u32,
    }
}

layout! {
    /// Decays, newcomer data, reset factors and effect settings read directly from the supplied tuning.
    struct Tuning {
        0x00 => eq_decay / set_eq_decay: f32,
        0x04 => hq_decay / set_hq_decay: f32,
        0x08 => newcomer_ages / set_newcomer_ages: [i16;3],
        0x0E => reset_age / set_reset_age: u8,
        0x10 => eq_reset / set_eq_reset: f32,
        0x14 => hq_reset / set_hq_reset: f32,
        0x18 => newcomer_eq / set_newcomer_eq: [f32;3],
        0x24 => newcomer_hq / set_newcomer_hq: [f32;3],
        0x30 => door / set_door: f32,
        0x34 => effect_height / set_effect_height: f32,
        0x38 => effect_delta / set_effect_delta: f32,
        0x3C => effect_chance / set_effect_chance: f32,
    }
}

#[repr(C)]
struct EffectName {
    table: u32,
    begin: *mut u8,
    end: *mut u8,
    capacity: *mut u8,
    extra: u32,
}

game_fn! { fn string_range(this:*mut u8,begin:*const u8,end:*const u8,tag:*const u8) = "thiscall" 0x0040_7FA0; }
game_fn! { fn string_chars(this:*mut EffectName)->*const u8 = "thiscall" 0x009D_7E63; }
game_fn! { fn free(pointer:*mut u8) = "C" 0x0090_CF63; }
game_fn! { fn pollution(this:*mut u8,kind:u32,x:i32,z:i32,size:i32)->u32 = "thiscall" 0x006F_7FF0; }
game_fn! { fn curve(this:*mut u8,input:f32)->St0 = "thiscall" 0x005F_4250; }
game_fn! { fn random(this:*mut u8,low_lo:u32,low_hi:u32,high_lo:u32,high_hi:u32)->St0 = "thiscall" 0x0091_3761; }

game_data! { static effect_name:[u8;8] = 0x00AB_2BD8; }
game_data! { static effects:*mut u8 = 0x00B4_3D1C; }
game_data! { static terrain:*mut u8 = 0x00B4_3CF4; }
game_data! { static simulator:*mut u8 = 0x00B4_3D5C; }
game_data! { static zero:f32 = 0x00A8_1054; }
game_data! { static one:f32 = 0x00A8_1228; }
game_data! { static two:f32 = 0x00A8_825C; }
game_data! { static hundred:f64 = 0x00A8_0AC0; }
game_data! { static random_low:f64 = 0x00A8_0990; }
game_data! { static half:f32 = 0x00A8_4D2C; }
game_data! { static maximum:f32 = 0x00A9_44D8; }
game_data! { static transform_default:u32 = 0x00B4_AF94; }

macro_rules! stored {
    ($name:ident,$instruction:literal,$ty:ty) => {
        fn $name(value:F80,operand:$ty)->F80 {
            let mut result=F80::ZERO;
            unsafe {
                core::arch::asm!("fld tbyte ptr [{value}]",$instruction,"fstp tbyte ptr [{result}]",
                    value=in(reg)&value,operand=in(reg)&operand,result=in(reg)&mut result,out("st(0)") _);
            }
            result
        }
    };
}
stored!(add, "fadd dword ptr [{operand}]", f32);
stored!(subtract, "fsub dword ptr [{operand}]", f32);
stored!(multiply, "fmul dword ptr [{operand}]", f32);
stored!(divide, "fdiv dword ptr [{operand}]", f32);
stored!(multiply_integer, "fimul dword ptr [{operand}]", i32);
stored!(add_double, "fadd qword ptr [{operand}]", f64);

unsafe fn pointer(this: *mut u8, offset: usize) -> *mut u8 {
    unsafe { this.add(offset).cast::<*mut u8>().read_unaligned() }
}

fn greater(a: F80, b: F80) -> bool {
    let comparison = a.compare(b);
    comparison.status_word() & 0x4100 == 0
}

unsafe fn range(this: *mut u8, low: f64, high: f64) -> F80 {
    let low = low.to_bits();
    let high = high.to_bits();
    unsafe { random(this, low as u32, (low >> 32) as u32, high as u32, (high >> 32) as u32) }
}

/// The two flag bytes precede an identity matrix and its translation. Padding retains its entry bytes.
unsafe fn place_effect(effect: *mut u8, padding: u32, x: f32, y: F80, z: f32) {
    unsafe {
        let mut transform = MaybeUninit::<[u8; 56]>::uninit();
        let transform = transform.as_mut_ptr().cast::<u8>();
        transform.add(2).cast::<u16>().write_unaligned((padding >> 16) as u16);
        for index in 0..9 {
            transform
                .add(4 + index * 4)
                .cast::<u32>()
                .write_unaligned(*transform_default());
        }
        for (offset, value) in [
            (4, 1.0f32),
            (8, 0.0),
            (12, 0.0),
            (16, 0.0),
            (20, 1.0),
            (24, 0.0),
            (28, 0.0),
            (32, 0.0),
            (36, 1.0),
            (52, 1.0),
        ] {
            transform.add(offset).cast::<f32>().write_unaligned(value);
        }
        transform.add(40).cast::<f32>().write_unaligned(x);
        transform.add(44).cast::<f32>().write_unaligned(y.to_f32());
        transform.add(48).cast::<f32>().write_unaligned(z);
        transform.add(1).write(1);
        transform.write(1);
        vcall!(effect,0x20 => fn(*mut u8),transform);
    }
}

port! {
    /// Decay EQ and HQ, optionally create one sickness effect, then copy each developer population.
    /// Blend newcomers using signed 16-bit deltas, age the tract, apply the generational reset,
    /// and accumulate the life expectancy and age histograms with wrapping integer arithmetic.
    #[port(address=0x006F_A450,owner="residential",name="cSC4ResidentialSimulator::DoPopulationChange",naked)]
    pub unsafe extern "C" fn fc4p_006fa450() {
        core::arch::naked_asm!("mov edx, dword ptr [esp - 0x38]", "jmp {body}",body=sym population_change);
    }
}

// The transform's padding comes from the original frame before any Rust frame can reuse those bytes.
unsafe extern "fastcall" fn population_change(this: *mut u8, padding: u32, tuning: *mut u8, x: i32, z: i32) {
    unsafe {
        let sim = TractSim::from_ptr(this);
        let tuning = Tuning::from_ptr(tuning);
        let mut name = EffectName {
            table: 0,
            begin: core::ptr::null_mut(),
            end: core::ptr::null_mut(),
            capacity: core::ptr::null_mut(),
            extra: 0,
        };
        let tag = MaybeUninit::<u8>::uninit();
        string_range(
            (&mut name.begin as *mut *mut u8).cast(),
            effect_name().cast(),
            effect_name().cast::<u8>().add(8),
            tag.as_ptr(),
        );
        name.table = 0x00A8_0810;
        name.extra = 0;
        let maps = [
            vcall!(sim.low_developer(),0x3C => fn()->*mut u8),
            vcall!(sim.medium_developer(),0x3C => fn()->*mut u8),
            vcall!(sim.high_developer(),0x3C => fn()->*mut u8),
        ];
        let size = vcall!(sim.low_population(),0x28 => fn()->i32);
        let mut hq_loss = tuning.hq_decay();
        let cell_x = size.wrapping_mul(x);
        let cell_z = size.wrapping_mul(z);
        for kind in 0..3 {
            let sample = pollution(this, kind, cell_x, cell_z, size) as u8;
            let loss = curve(
                tuning.as_ptr().add(0x40 + kind as usize * 16),
                F80::from_i32(i32::from(sample)).to_f32(),
            );
            hq_loss = add(loss, hq_loss).to_f32();
        }

        let mut effect_created = false;
        for wealth in 0..3 {
            let offset = 0x7C + wealth * 4;
            let value = vcall_st0!(pointer(this, offset), 0x58, x, z);
            let decayed = subtract(value, tuning.eq_decay());
            let stored = decayed.to_f32();
            let stored = if decayed.compare(F80::from_f32(*zero())).less() {
                0.0
            } else {
                stored
            };
            vcall!(pointer(this,offset),0x68 => fn(i32,i32,f32),x,z,stored);
            let value = vcall_st0!(pointer(this, offset + 12), 0x58, x, z);
            let decayed = subtract(value, hq_loss);
            let stored = decayed.to_f32();
            let stored = if decayed.compare(F80::from_f32(*zero())).less() {
                0.0
            } else {
                stored
            };
            vcall!(pointer(this,offset+12),0x68 => fn(i32,i32,f32),x,z,stored);
            if F80::from_f32(stored).compare(F80::from_f32(tuning.door())).less()
                && !effect_created
                && vcall!(pointer(this,offset-0x34),0x58 => fn(i32,i32)->u32,x,z) as u16 != 0
            {
                let draw = range(this.add(0xB8), *random_low(), *hundred());
                if greater(F80::from_f32(tuning.effect_chance()), draw) {
                    let manager = *effects();
                    let mut effect = core::ptr::null_mut::<u8>();
                    let chars = string_chars(&mut name);
                    vcall!(manager,0x1C => fn(*const u8,*mut *mut u8),chars,&mut effect);
                    if !effect.is_null() {
                        let grid = pointer(this, offset);
                        let start = multiply_integer(vcall_st0!(grid, 0x38), x).to_f32();
                        let px = add(multiply(vcall_st0!(grid, 0x38), *half()), start).to_f32();
                        let grid = pointer(this, offset);
                        let start = multiply_integer(vcall_st0!(grid, 0x3C), z).to_f32();
                        let pz = add(multiply(vcall_st0!(grid, 0x3C), *half()), start).to_f32();
                        let base = add(vcall_st0!(*terrain(), 0x34, px, pz), tuning.effect_height()).to_f64();
                        let delta = F80::from_f32(tuning.effect_delta());
                        let high = delta.to_f64();
                        let low = (-F80::from_f32(tuning.effect_delta())).to_f64();
                        let height = add_double(range(this.add(0xB8), low, high), base);
                        place_effect(effect, padding, px, height, pz);
                        vcall!(effect,0x0C => fn(u32),0);
                        effect_created = true;
                        if !effect.is_null() {
                            vcall!(effect,8 => fn());
                        }
                    }
                }
            }
        }

        let population = [
            vcall!(maps[0],0x58 => fn(i32,i32)->u32,x,z) as u16,
            vcall!(maps[1],0x58 => fn(i32,i32)->u32,x,z) as u16,
            vcall!(maps[2],0x58 => fn(i32,i32)->u32,x,z) as u16,
        ];
        let mut old_population = [0u16; 3];
        let mut newcomers = [0i16; 3];
        let mut arrived = false;
        let mut total = 0i32;
        for wealth in 0..3 {
            let new = population[wealth];
            total = total.wrapping_add(i32::from(new));
            let offset = 0x7C + wealth * 4;
            let old = vcall!(pointer(this,offset-0x34),0x58 => fn(i32,i32)->u32,x,z) as u16;
            old_population[wealth] = old;
            vcall!(pointer(this,offset-0x34),0x68 => fn(i32,i32,u32),x,z,u32::from(new));
            sim.set_running_population(sim.running_population().wrapping_add(i32::from(new)));
            if old < new {
                let difference = i32::from(new).wrapping_sub(i32::from(old));
                newcomers[wealth] = difference as i16;
                let old_eq = vcall_st0!(pointer(this, offset), 0x58, x, z);
                let divisor = if greater(old_eq, F80::from_f32(*zero())) {
                    *two()
                } else {
                    *one()
                };
                let delta_float = F80::from_i32(i32::from(difference as i16)).to_f32();
                let old_float = F80::from_i32(i32::from(old)).to_f32();
                let new_float = F80::from_i32(i32::from(new)).to_f32();
                let bonus = tuning.as_ptr().add(0x18 + wealth * 4).cast::<f32>().read_unaligned();
                let incoming = multiply(add(old_eq, bonus) / F80::from_f32(divisor), delta_float);
                let combined = divide(incoming + F80::from_f32(old_float) * old_eq, new_float);
                let stored = combined.to_f32();
                let stored = if greater(combined, F80::from_f32(*maximum())) {
                    200.0
                } else {
                    stored
                };
                vcall!(pointer(this,offset),0x68 => fn(i32,i32,f32),x,z,stored);
                let old_hq = vcall_st0!(pointer(this, offset + 12), 0x58, x, z);
                let divisor = if greater(old_hq, F80::from_f32(*zero())) {
                    *two()
                } else {
                    *one()
                };
                let bonus = tuning.as_ptr().add(0x24 + wealth * 4).cast::<f32>().read_unaligned();
                let incoming = multiply(add(old_hq, bonus) / F80::from_f32(divisor), delta_float);
                let combined = divide(incoming + F80::from_f32(old_float) * old_hq, new_float);
                let stored = combined.to_f32();
                let stored = if greater(combined, F80::from_f32(*maximum())) {
                    200.0
                } else {
                    stored
                };
                vcall!(pointer(this,offset+12),0x68 => fn(i32,i32,f32),x,z,stored);
                arrived = true;
            }
            if new == 0 {
                vcall!(pointer(this,offset),0x68 => fn(i32,i32,u32),x,z,0);
                vcall!(pointer(this,offset+12),0x68 => fn(i32,i32,u32),x,z,0);
            }
        }
        let mut age = vcall!(sim.age_grid(),0x58 => fn(i32,i32)->u32,x,z) as u8;
        if total != 0 {
            let mut month = MaybeUninit::<u32>::uninit();
            vcall!(*simulator(),0x18 => fn(u32,*mut u32,u32,u32,u32),0,month.as_mut_ptr(),0,0,0);
            if month.assume_init() == 12 {
                age = age.wrapping_add(1);
            }
        } else {
            age = 0;
        }
        let mut age = i32::from(age);
        if arrived {
            let sum = population.iter().map(|&n| i32::from(n)).sum::<i32>();
            if sum == 0 {
                age = 0;
            } else {
                let old_sum = old_population.iter().map(|&n| i32::from(n)).sum::<i32>();
                let mut numerator = old_sum.wrapping_mul(age);
                for wealth in (0..3).rev() {
                    let newcomer_age = tuning.as_ptr().add(8 + wealth * 2).cast::<i16>().read_unaligned();
                    numerator =
                        numerator.wrapping_add(i32::from(newcomer_age).wrapping_mul(i32::from(newcomers[wealth])));
                }
                age = numerator / sum;
            }
        }
        let grid = sim.low_population();
        let table = this.cast::<*const usize>().read_unaligned();
        let cell_z = vcall!(grid,0x28 => fn()->i32).wrapping_mul(z);
        let cell_x = vcall!(grid,0x28 => fn()->i32).wrapping_mul(x);
        let life_function = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, i32, i32) -> u32>(
            table.add(0x74 / 4).read(),
        );
        let life = i32::from(life_function(this, cell_x, cell_z) as u8);
        if age > life {
            age = i32::from(tuning.reset_age());
            for wealth in 0..3 {
                let eq_factor = tuning.eq_reset();
                let eq = vcall_st0!(pointer(this, 0x7C + wealth * 4), 0x58, x, z);
                let stored = multiply(eq, eq_factor).to_f32();
                vcall!(pointer(this,0x7C+wealth*4),0x68 => fn(i32,i32,f32),x,z,stored);
                let hq_factor = tuning.hq_reset();
                let hq = vcall_st0!(pointer(this, 0x88 + wealth * 4), 0x58, x, z);
                let stored = multiply(hq, hq_factor).to_f32();
                vcall!(pointer(this,0x88+wealth*4),0x68 => fn(i32,i32,f32),x,z,stored);
            }
        }
        vcall!(sim.age_grid(),0x68 => fn(i32,i32,i32),x,z,age);
        let product = life.wrapping_mul(total);
        let sum = (u64::from(sim.life_high()) << 32) | u64::from(sim.life_low());
        let sum = sum.wrapping_add(i64::from(product) as u64);
        sim.set_life_low(sum as u32);
        sim.set_life_high((sum >> 32) as u32);
        let bucket = (age.wrapping_sub(20) / 3) as u32;
        let bucket = if bucket < 20 { bucket } else { 19 };
        for wealth in 0..3 {
            let count = vcall!(pointer(this,0x48+wealth*4),0x58 => fn(i32,i32)->u32,x,z) as u16;
            let eq = vcall_st0!(pointer(this, 0x7C + wealth * 4), 0x58, x, z).ftol() as i32;
            let count_ptr = this.add(0x1C4 + bucket as usize * 4).cast::<i32>();
            let eq_ptr = this.add(0x214 + bucket as usize * 4).cast::<i32>();
            count_ptr.write_unaligned(count_ptr.read_unaligned().wrapping_add(i32::from(count)));
            eq_ptr.write_unaligned(eq_ptr.read_unaligned().wrapping_add(eq.wrapping_mul(i32::from(count))));
        }
        name.table = 0x00A8_0810;
        if !name.begin.is_null() {
            free(name.begin);
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006fa450::PORT];

#[cfg(test)]
mod tests;
