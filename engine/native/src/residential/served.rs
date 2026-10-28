// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population shares served by civic buildings.

use super::civic::multiply;
use super::population::load_unsigned;
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// The grids needed when counting a building's served population.
    pub struct ServedSim {
        0x48 => population_low / set_population_low: *mut u8,
        0x4C => population_medium / set_population_medium: *mut u8,
        0x50 => population_high / set_population_high: *mut u8,
        0x54 => age / set_age: *mut u8,
    }
}
layout! {
    /// A tract coordinate in a circular list.
    pub struct Tract {
        0x00 => next / set_next: *mut u8,
        0x08 => x / set_x: u32,
        0x0C => z / set_z: u32,
    }
}
game_fn! { fn curve_output(this:*mut u8,x:f32)->St0 = "thiscall" 0x005F_4250; }
game_data! { static zero:f32 = 0x00A8_1054; }
game_data! { static hundred:f32 = 0x00A8_0AA0; }
game_data! { static percent:f32 = 0x00A8_23A4; }
game_data! { static rounding_bias:f32 = 0x00AB_2BB0; }

pub(super) fn divide(numerator: u32, divisor: u32) -> u32 {
    let result: u32;
    unsafe {
        core::arch::asm!("div {divisor:e}",divisor=in(reg) divisor,inlateout("eax") numerator=>result,inlateout("edx") 0u32=>_,options(nomem,nostack));
    }
    result
}
fn divide_stored(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]","fdiv dword ptr [{operand}]","fstp tbyte ptr [{result}]",value=in(reg) &value,operand=in(reg) &operand,result=in(reg) &mut result,out("st(0)") _);
    }
    result
}
fn add_stored(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]","fadd dword ptr [{operand}]","fstp tbyte ptr [{result}]",value=in(reg) &value,operand=in(reg) &operand,result=in(reg) &mut result,out("st(0)") _);
    }
    result
}
port! {
    /// Accumulate each wealth's share in tract order, then round by the single-precision bias method.
    /// X differences are unsigned; squared Z differences wrap to a word before their unsigned load.
    #[port(address=0x006F_8F60,owner="residential",name="cSC4ResidentialSimulator::GetPopulationServed")]
    pub unsafe extern "thiscall" fn fc4p_006f8f60(this:*mut u8,occupant:*mut u8,radius:f32,capacity:u32,age_curve:*mut u8,wealth_curve:*mut u8,distance_curve:*mut u8,effects:*mut *mut u8,coverage:*mut u8,chunk:u32,tracts:*mut *mut u8,_unused:u32)->u32 {
        unsafe {
            let comparison=F80::from_f32(radius).compare(F80::from_f32(*zero()));
            let units=divide(capacity,chunk);
            if comparison.less()||comparison.equal(){return 0;}
            let sim=ServedSim::from_ptr(this);
            let head=*tracts;let mut node=Tract::from_ptr(head).next();
            let mut position=MaybeUninit::<[f32;3]>::uninit();
            vcall!(occupant,0x20 => fn(*mut f32)->u32,position.as_mut_ptr().cast());
            let position=position.assume_init();let (mut tx,mut tz)=(MaybeUninit::<u32>::uninit(),MaybeUninit::<u32>::uninit());
            vcall!(sim.population_low(),0x4C => fn(f32,f32,*mut u32,*mut u32)->u32,position[0],position[2],tx.as_mut_ptr(),tz.as_mut_ptr());
            let (tx,tz)=(tx.assume_init(),tz.assume_init());
            let mut total=0f32;
            while node!=head {
                let tract=Tract::from_ptr(node);let (x,z)=(tract.x(),tract.z());
                if vcall!(coverage,0x58 => fn(u32,u32)->u32,x,z) as u8!=0 {
                    let dx=load_unsigned(tx.wrapping_sub(x));let dz=tz.wrapping_sub(z);
                    let distance=(load_unsigned(dz.wrapping_mul(dz))+dx*dx).sqrt();
                    let input=multiply(divide_stored(distance,radius),*hundred()).to_f32();
                    let mut distance_factor=multiply(curve_output(distance_curve,input),*percent()).to_f32();
                    let age=vcall!(sim.age(),0x58 => fn(u32,u32)->u32,x,z) as u8;
                    let age=F80::from_i32(i32::from(age)).to_f32();
                    let mut age_factor=multiply(curve_output(age_curve,age),*percent()).to_f32();
                    let effect_head=*effects;let mut effect=Tract::from_ptr(effect_head).next();
                    while effect!=effect_head {
                        distance_factor=multiply(multiply(curve_output(effect.add(0x0C),input),distance_factor),*percent()).to_f32();
                        let output=curve_output(effect.add(0x24),age);
                        let product=multiply(output,age_factor);
                        effect=Tract::from_ptr(effect).next();
                        age_factor=multiply(product,*percent()).to_f32();
                    }
                    let units=load_unsigned(units).to_f32();
                    for wealth in 0..3 {
                        let grid=*this.add(0x48+wealth*4).cast::<*mut u8>();
                        let population=vcall!(grid,0x58 => fn(u32,u32)->u32,x,z) as u16;
                        let population=F80::from_i32(i32::from(population)).to_f32();
                        let n=vcall!(coverage,0x58 => fn(u32,u32)->u32,x,z) as u8;
                        let share=multiply(F80::from_f32(units)/F80::from_i32(i32::from(n)),population).to_f32();
                        let input=F80::from_i32(wealth as i32).to_f32();
                        let result=multiply(multiply(multiply(multiply(curve_output(wealth_curve,input),*percent()),share),age_factor),distance_factor);
                        total=add_stored(result,total).to_f32();
                    }
                }
                node=Tract::from_ptr(node).next();
            }
            add_stored(F80::from_f32(total),*rounding_bias()).to_f32().to_bits().wrapping_add(0xB4C0_0000)
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_006f8f60::PORT];

#[cfg(test)]
mod tests;
