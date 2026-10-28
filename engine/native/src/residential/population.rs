// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population averaging and age-group history publication.

use super::civic::multiply;
use super::ResidentialSim;
use crate::port::Port;
use crate::x87::F80;

game_data! { static population_simulator:*mut u8 = 0x00B4_3D9C; }
game_data! { static unsigned_bias:f32 = 0x00A8_0AA8; }
game_data! { static percent:f32 = 0x00A8_23A4; }
game_fn! { fn signed_divide(numerator:i64,denominator:i64)->i64 = "stdcall" 0x009F_0710; }
game_fn! { fn record_history(this:*mut u8,id:u32,value:u32)->u32 = "thiscall" 0x006F_7D00; }
game_fn! { fn curve_output(this:*mut u8,x:f32)->St0 = "thiscall" 0x005F_4250; }
game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this:*mut u8)->*mut u8 = "thiscall" 0x0091_34D6; }

pub(super) fn load_unsigned(value: u32) -> F80 {
    let result = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        return result;
    }
    let mut out = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]","fadd dword ptr [{bias}]","fstp tbyte ptr [{out}]",value=in(reg) &result,bias=in(reg) unsigned_bias(),out=in(reg) &mut out,out("st(0)") _);
    }
    out
}

unsafe extern "stdcall" fn average_population(wealth: u32, x: u32, z: u32, width: u32) -> u64 {
    unsafe {
        let simulator = *population_simulator();
        let mut sum = 0u32;
        let mut cx = x;
        while cx < x.wrapping_add(width) {
            let mut cz = z;
            while cz < z.wrapping_add(width) {
                let value = vcall!(simulator,0x18 => fn(u32,u32,u32)->u32,wealth,cx,cz);
                sum = sum.wrapping_add(value & 0xFFFF);
                cz = cz.wrapping_add(1);
            }
            cx = cx.wrapping_add(1);
        }
        let divisor = width.wrapping_mul(width);
        let (quotient, remainder): (u32, u32);
        core::arch::asm!("div {divisor:e}",divisor=in(reg) divisor,inlateout("eax") sum=>quotient,inlateout("edx") 0u32=>remainder,options(nomem,nostack));
        (u64::from(remainder) << 32) | u64::from(quotient)
    }
}
port! {
    /// Average the low words of population samples over an unsigned square. Preserve entry ECX.
    #[port(address=0x006F_7FF0,owner="residential",name="AverageResidentialPopulationSquare",naked)]
    pub unsafe extern "C" fn fc4p_006f7ff0(){
        core::arch::naked_asm!("push ecx","push dword ptr [esp+20]","push dword ptr [esp+20]","push dword ptr [esp+20]","push dword ptr [esp+20]","call {body}","pop ecx","ret 16",body=sym average_population);
    }
}

/// Inline history publication used for the nine age groups.
unsafe fn post(sim: ResidentialSim, id: u32, value: i32) {
    unsafe {
        if sim.message_server().is_null() {
            return;
        }
        let storage = allocate(0x2C);
        let message = if storage.is_null() {
            core::ptr::null_mut()
        } else {
            construct_message(storage)
        };
        vcall!(message,4 => fn()->u32);
        vcall!(message,0x14 => fn(u32)->u32,0x89EF_A536);
        vcall!(message,0x2C => fn(u32)->u32,id);
        vcall!(message,0x40 => fn(i32)->u32,value);
        vcall!(sim.message_server(),0x0C => fn(*mut u8)->u32,message);
        vcall!(message,8 => fn()->u32);
    }
}
port! {
    /// Publish the running population, average life expectancy, and nine age-group populations and EQs.
    /// Bucket EQs are snapshotted before the response curves; bucket counts are read after each curve call.
    #[port(address=0x006F_8070,owner="residential",name="cSC4ResidentialSimulator::PostPopulationChange")]
    pub unsafe extern "thiscall" fn fc4p_006f8070(this:*mut u8) {
        unsafe {
            let sim=ResidentialSim::from_ptr(this);
            let population=sim.running_population();sim.set_population(population);
            let life=if population==0{0}else{
                let sum=(u64::from(sim.life_sum_high())<<32)|u64::from(sim.life_sum_low());
                signed_divide(sum as i64,i64::from(population)) as u32
            };
            record_history(this,0x2A5E_7009,life);
            let mut eq=[0f32;20];
            for (i,value) in eq.iter_mut().enumerate(){
                let count=*this.add(0x1C4+i*4).cast::<u32>();
                *value=if count==0{0.0}else{(load_unsigned(*this.add(0x214+i*4).cast::<u32>())/load_unsigned(count)).to_f32()};
            }
            for group in 0..9 {
                let mut count=0i32;let mut weighted=0i32;
                for (i,eq) in eq.iter().enumerate() {
                    if *this.add(0x1C4+i*4).cast::<u32>()!=0 {
                        let output=curve_output(this.add(0x264+group*16),F80::from_i32(21+i as i32*3).to_f32());
                        let count_value=load_unsigned(*this.add(0x1C4+i*4).cast::<u32>());
                        let n=multiply(output*count_value,*percent()).ftol() as i32;
                        count=count.wrapping_add(n);
                        let product=multiply(F80::from_i32(n),*eq);
                        weighted=(product+F80::from_i32(weighted)).ftol() as i32;
                    }
                }
                let id=0xCA54_5CCAu32.wrapping_add(group as u32);
                post(sim,id.wrapping_sub(31),count);
                let average=if count>0{weighted/count}else{0};
                post(sim,id,average);
            }
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_006f7ff0::PORT, fc4p_006f8070::PORT];

#[cfg(test)]
mod tests;
