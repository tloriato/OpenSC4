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
game_fn! { fn read_uint(properties:*mut u8,id:u32,out:*mut u32)->u32 = "C" 0x005F_D480; }
game_fn! { fn write_uint(properties:*mut u8,id:u32,value:u32)->u32 = "C" 0x005F_E3C0; }
game_fn! { fn random_range(this:*mut u8,min_low:u32,min_high:u32,max_low:u32,max_high:u32)->St0 = "thiscall" 0x0091_3761; }
game_fn! { fn string_new(this:*mut u8,text:*const u8)->*mut u8 = "thiscall" 0x0040_8480; }
game_fn! { fn free_string(buffer:*mut u8)->u32 = "C" 0x0090_CF63; }
game_data! { static random_min:f64 = 0x00A8_0990; }
game_data! { static random_max:f64 = 0x00A8_0AC0; }
game_data! { static simulator:*mut u8 = 0x00B4_3CEC; }
game_data! { static health_controllers:*const u8 = 0x00B0_D438; }
game_data! { static school_controllers:*const u8 = 0x00B0_D43C; }

layout! {
    /// City services used for strike transitions.
    pub struct TransitionSim {
        0x18 => city / set_city: *mut u8,
    }
}
port! {
    /// Start a building strike on one random draw, or end it after its duration or staff threshold.
    /// A changed day property is written before requesting the optional automata controller.
    #[port(address=0x006F_A280,owner="residential",name="cSC4ResidentialSimulator::UpdateStrikingState")]
    pub unsafe extern "thiscall" fn fc4p_006fa280(this:*mut u8,building:*mut u8,department:*mut u8,staff_property:u32,grade_property:u32)->u32 {
        unsafe {
            let city=TransitionSim::from_ptr(this).city();
            let clock=vcall!(city,0xC8 => fn()->*mut u8);
            let clock=vcall!(clock,0x1C => fn()->*mut u8);
            let today=vcall!(clock,0x68 => fn()->u32);
            let mut old=0u32;
            let properties=vcall!(building,0x18 => fn()->*mut u8);
            read_uint(properties,0xEAA6_9F83,&mut old);
            let mut new=old;
            let d=Department::from_ptr(department);
            if old!=0 {
                let fallback=f32::from_bits(*one());
                let properties=vcall!(building,0x18 => fn()->*mut u8);
                let mut value=core::mem::MaybeUninit::<f32>::uninit();
                let ratio=if read_float(properties,staff_property,value.as_mut_ptr()) as u8!=0{value.assume_init()}else{fallback};
                let ratio=F80::from_f32(ratio);
                if today.wrapping_sub(old)>d.count(){new=0;}
                else{
                    let comparison=ratio.compare(F80::from_f32(f32::from_bits(d.fourth())));
                    if !comparison.less()&&!comparison.unordered(){new=0;}
                }
            }else{
                let fallback=f32::from_bits(*zero());
                let properties=vcall!(building,0x18 => fn()->*mut u8);
                let mut value=core::mem::MaybeUninit::<f32>::uninit();
                let grade=if read_float(properties,grade_property,value.as_mut_ptr()) as u8!=0{value.assume_init()}else{fallback};
                let input=multiply(F80::from_f32(grade),*hundred()).to_f32();
                let chance=curve_output(department.add(0x24),input).to_f32();
                let max=F80::from_f64(*random_max()).to_f64();let min=F80::from_f64(*random_min()).to_f64();
                let draw=random_range(this.add(0xB8),min.to_bits() as u32,(min.to_bits()>>32) as u32,max.to_bits() as u32,(max.to_bits()>>32) as u32);
                if draw.compare(F80::from_f32(chance)).less(){new=today;}
            }
            if new==old{return 0;}
            let properties=vcall!(building,0x18 => fn()->*mut u8);
            write_uint(properties,0xEAA6_9F83,new);
            let controller=vcall!(*simulator(),0xBC => fn()->*mut u8);
            if !controller.is_null(){
                let name=if d.final_word()==0x077A_CCD2{*health_controllers()}else{*school_controllers()};
                let mut storage=[core::mem::MaybeUninit::<u32>::uninit();5];let string=storage.as_mut_ptr().cast::<u8>();
                string_new(string,name);
                if new!=0{vcall!(controller,0x28 => fn(*mut u8,*mut u8,u32)->u32,string,building,0);}
                else{vcall!(controller,0x30 => fn(*mut u8,*mut u8)->u32,string,building);}
                let buffer=*string.add(4).cast::<*mut u8>();string.cast::<u32>().write(0x00A8_0810);
                if !buffer.is_null(){free_string(buffer);}
            }
            1
        }
    }
}

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
pub const PORTS: &[Port] = &[
    fc4p_006f9390::PORT,
    fc4p_006f9670::PORT,
    fc4p_006f9730::PORT,
    fc4p_006fa280::PORT,
];

#[cfg(test)]
mod tests;
