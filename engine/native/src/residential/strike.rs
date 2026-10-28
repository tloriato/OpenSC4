// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Department strike probabilities and their combination across building groups.

use super::civic::multiply;
use super::service::Department;
use crate::port::Port;
use crate::x87::F80;

layout! {
    /// The simulator's arrays of building lists.
    pub struct StrikeSim {
        0x1A8 => schools / set_schools: *mut *mut u8,
        0x1AC => schools_end / set_schools_end: *mut *mut u8,
        0x1B4 => hospitals / set_hospitals: *mut *mut u8,
        0x1B8 => hospitals_end / set_hospitals_end: *mut *mut u8,
    }
}
layout! {
    /// One circular list of building pointers.
    pub struct BuildingNode {
        0x00 => next / set_next: *mut u8,
        0x08 => building / set_building: *mut u8,
    }
}
game_fn! { fn read_float(properties:*mut u8,id:u32,out:*mut f32)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn curve_output(this:*mut u8,x:f32)->St0 = "thiscall" 0x005F_4250; }
game_fn! { fn list_chance(this:*mut u8,out:*mut f32,department:*mut u8,list:*mut *mut u8,grade:u32,group:u32)->u32 = "thiscall" 0x006F_9390; }
game_data! { static zero:u32 = 0x00B4_AF90; }
game_data! { static one:u32 = 0x00B0_D440; }
game_data! { static hundred:f32 = 0x00A8_0AA0; }
game_data! { static percent:f32 = 0x00A8_23A4; }

fn add_stored(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fadd dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value=in(reg) &value,operand=in(reg) &operand,result=in(reg) &mut result,out("st(0)") _);
    }
    result
}
/// Each group is stored back to a single before the next group is considered.
fn combine(a: f32, b: f32) -> f32 {
    let sum = add_stored(F80::from_f32(a), b);
    let product = multiply(F80::from_f32(a), b);
    (sum - product).to_f32()
}

port! {
    /// Buildings outside the strike group reuse the previous chance; a missing grade reuses the
    /// previous grade. An active department strike supplies the default funding word directly.
    #[port(address=0x006F_9390,owner="residential",name="cSC4ResidentialSimulator::GetChanceOfStrike")]
    pub unsafe extern "stdcall" fn fc4p_006f9390(out:*mut f32,department:*mut u8,list:*mut *mut u8,grade_property:u32,strike_group:u32)->u32 {
        unsafe {
            let mut total=f32::from_bits(*zero());
            if Department::from_ptr(department).strike()!=0 {
                *out.cast::<u32>()= *one();
                return out as u32;
            }
            let mut chance=total;
            let mut grade=total;
            let head=*list;
            let mut node=BuildingNode::from_ptr(head).next();
            while node!=head {
                let building=BuildingNode::from_ptr(node).building();
                if vcall!(building,0x58 => fn(u32)->u32,strike_group) as u8!=0 {
                    let properties=vcall!(building,0x18 => fn()->*mut u8);
                    let mut value=core::mem::MaybeUninit::<f32>::uninit();
                    if read_float(properties,grade_property,value.as_mut_ptr()) as u8!=0 {grade=value.assume_init();}
                    let input=multiply(F80::from_f32(grade),*hundred()).to_f32();
                    chance=multiply(curve_output(department.add(0x24),input),*percent()).to_f32();
                }
                total=combine(total,chance);
                node=BuildingNode::from_ptr(node).next();
            }
            *out=total;
            out as u32
        }
    }
}

/// Combine the individual group lists, reloading the array and its end after every callback.
unsafe fn department_chance(this: *mut u8, out: *mut f32, school: bool) -> u64 {
    unsafe {
        let sim = StrikeSim::from_ptr(this);
        let initial = *zero();
        let status = if school {
            vcall!(this,0x18 => fn()->u64)
        } else {
            vcall!(this,0x1C => fn()->u64)
        };
        let mut edx = (status >> 32) as u32;
        if status as u8 != 0 {
            *out.cast::<u32>() = *one();
            return (u64::from(edx) << 32) | u64::from(out as u32);
        }
        let mut total = f32::from_bits(initial);
        let mut i = 0u32;
        let (mut begin, mut end) = if school {
            (sim.schools(), sim.schools_end())
        } else {
            (sim.hospitals(), sim.hospitals_end())
        };
        let mut count = ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32;
        while i < count {
            let mut chance = core::mem::MaybeUninit::<f32>::uninit();
            let (block, grade, group) = if school {
                (0x134, 0x2924_53C5, 0x1A03)
            } else {
                (0xD4, 0x0924_53CB, 0x1A02)
            };
            list_chance(
                this,
                chance.as_mut_ptr(),
                this.add(block),
                begin.wrapping_add(i as usize),
                grade,
                group,
            );
            total = combine(chance.assume_init(), total);
            (begin, end) = if school {
                (sim.schools(), sim.schools_end())
            } else {
                (sim.hospitals(), sim.hospitals_end())
            };
            count = ((end as u32).wrapping_sub(begin as u32) as i32 >> 2) as u32;
            edx = count;
            i = i.wrapping_add(1);
        }
        *out = total;
        (u64::from(edx) << 32) | u64::from(out as u32)
    }
}
port! {
    /// Return the combined school strike probability, or the active-strike default.
    #[port(address=0x006F_9670,owner="residential",name="cSC4ResidentialSimulator::ChanceOfSchoolStrike")]
    pub unsafe extern "thiscall" fn fc4p_006f9670(this:*mut u8,out:*mut f32)->u64 {unsafe{department_chance(this,out,true)}}
}
port! {
    /// Return the combined health strike probability, or the active-strike default.
    #[port(address=0x006F_9730,owner="residential",name="cSC4ResidentialSimulator::ChanceOfHealthStrike")]
    pub unsafe extern "thiscall" fn fc4p_006f9730(this:*mut u8,out:*mut f32)->u64 {unsafe{department_chance(this,out,false)}}
}
pub const PORTS: &[Port] = &[fc4p_006f9390::PORT, fc4p_006f9670::PORT, fc4p_006f9730::PORT];

#[cfg(test)]
mod tests;
