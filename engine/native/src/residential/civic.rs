// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Civic building funding, ordinance effects, and department strikes.

use super::service::Department;
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Civic services accessed from the residential simulator's primary interface.
    pub struct CivicSim {
        0x18 => city / set_city: *mut u8,
        0xD0 => budget / set_budget: *mut u8,
        0xDC => health_strike_efficiency / set_health_strike_efficiency: f32,
        0x13C => school_strike_efficiency / set_school_strike_efficiency: f32,
    }
}
layout! {
    /// A circular list node containing an ordinance effect.
    pub struct Effect {
        0x00 => next / set_next: *mut u8,
        0x08 => radius / set_radius: f32,
        0x1C => third / set_third: f32,
        0x20 => capacity / set_capacity: f32,
        0x2C => groups / set_groups: *mut u8,
    }
}

game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this:*mut u8)->*mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { fn curve_output(this:*mut u8,x:f32)->St0 = "thiscall" 0x005F_4250; }
game_fn! { fn building_on_strike(this:*mut u8,occupant:*mut u8)->u32 = "thiscall" 0x006F_6F30; }
game_data! { static message_server:*mut u8 = 0x00B4_3CCC; }
game_data! { static default_funding:u32 = 0x00B0_D440; }
game_data! { static hundred:f32 = 0x00A8_0AA0; }
game_data! { static percent:f32 = 0x00A8_23A4; }

/// Stack record filled by the budget simulator. The funding word is the only initialized member.
#[repr(C)]
struct BudgetItem {
    words: [MaybeUninit<u32>; 8],
}
impl BudgetItem {
    fn new() -> Self {
        let mut item = Self {
            words: [MaybeUninit::uninit(); 8],
        };
        item.words[7].write(0);
        item
    }
    unsafe fn department(&self) -> u32 {
        unsafe { self.words[0].assume_init() }
    }
    unsafe fn funding(&self) -> u32 {
        unsafe { self.words[7].assume_init() }
    }
}

port! {
    /// Resolve staff funding first. A missing staff item supplies defaults; a missing department
    /// leaves the corresponding caller value alone. Vehicle funding is queried only after staff succeeds.
    #[port(address=0x006F_8360,owner="residential",name="cSC4ResidentialSimulator::GetRawOccupantFundingPercentages")]
    pub unsafe extern "thiscall" fn fc4p_006f8360(this:*mut u8,occupant:*mut u8,staff:*mut u32,vehicle:*mut u32,school:u32)->u32 {
        unsafe {
            let sim=CivicSim::from_ptr(this);
            let properties=vcall!(occupant,0x18 => fn()->*mut u8);
            let (staff_purpose,vehicle_purpose)=if school as u8!=0 {(0xEA56_54B6,0x4A56_54BA)}else{(0xCA56_5486,0xEA56_549E)};
            let mut item=BudgetItem::new();
            if vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut BudgetItem)->u32,properties,staff_purpose,&mut item) as u8==0 {
                *staff= *default_funding();
                *vehicle= *default_funding();
                return vehicle as u32;
            }
            let department=vcall!(sim.budget(),0x70 => fn(u32)->*mut u8,item.department());
            if department.is_null(){return 0;}
            if vcall!(department,0x70 => fn(*mut u8,u32)->u32,properties,staff_purpose) as u8!=0 {
                *staff=item.funding();
            }else{
                *staff= *vcall!(department,0x28 => fn()->*const u32);
            }
            if vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut BudgetItem)->u32,properties,vehicle_purpose,&mut item) as u8==0 {
                let value= *default_funding();
                *vehicle=value;
                return value;
            }
            let department=vcall!(sim.budget(),0x70 => fn(u32)->*mut u8,item.department());
            if department.is_null(){return 0;}
            if vcall!(department,0x70 => fn(*mut u8,u32)->u32,properties,vehicle_purpose) as u8!=0 {
                *vehicle=item.funding();
                vehicle as u32
            }else{
                let value= *vcall!(department,0x28 => fn()->*const u32);
                *vehicle=value;
                value
            }
        }
    }
}

/// The item lookup caches the budget's method before asking the occupant for its properties.
unsafe fn staff_item(sim: CivicSim, occupant: *mut u8, purpose: u32) -> u32 {
    unsafe {
        let method = crate::abi::vslot(sim.budget(), 0xCC);
        let properties = vcall!(occupant,0x18 => fn()->*mut u8);
        let lookup = core::mem::transmute::<
            usize,
            unsafe extern "thiscall" fn(*mut u8, *mut u8, u32, *mut BudgetItem) -> u32,
        >(method);
        let mut item = BudgetItem::new();
        lookup(sim.budget(), properties, purpose, &mut item)
    }
}
port! {
    /// Use the funding efficiency curve when a staff item exists; otherwise return raw staff funding.
    #[port(address=0x006F_85F0,owner="residential",name="cSC4ResidentialSimulator::GetBaseStaffEfficiency")]
    pub unsafe extern "thiscall" fn fc4p_006f85f0(this:*mut u8,out:*mut f32,occupant:*mut u8,school:u32)->u32 {
        unsafe {
            let sim=CivicSim::from_ptr(this);
            let (mut staff,mut vehicle)=(0f32,0f32);
            vcall!(this,0x50 => fn(*mut u8,*mut f32,*mut f32,u32)->u32,occupant,&mut staff,&mut vehicle,school);
            let (purpose,curve)=if school as u8!=0 {(0xEA56_54B6,0x148)}else{(0xCA56_5486,0xE8)};
            let original=staff.to_bits();
            if staff_item(sim,occupant,purpose) as u8!=0 {
                let input=multiply(F80::from_f32(staff),*hundred()).to_f32();
                *out=multiply(curve_output(this.add(curve),input),*percent()).to_f32();
            }else{
                out.cast::<u32>().write(original);
            }
            out as u32
        }
    }
}
port! {
    /// Compute staffing and coverage efficiencies, applying the strike multiplier unless ignored.
    #[port(address=0x006F_8710,owner="residential",name="cSC4ResidentialSimulator::GetOccupantEfficiencyRatings")]
    pub unsafe extern "thiscall" fn fc4p_006f8710(this:*mut u8,occupant:*mut u8,efficiency:*mut f32,coverage:*mut f32,school:u32,ignore_strike:u32) {
        unsafe {
            *efficiency.cast::<u32>()= *default_funding();
            *coverage.cast::<u32>()= *default_funding();
            let (mut staff,mut vehicle)=(0f32,0f32);
            vcall!(this,0x50 => fn(*mut u8,*mut f32,*mut f32,u32)->u32,occupant,&mut staff,&mut vehicle,school);
            let sim=CivicSim::from_ptr(this);
            let (purpose,curve,coverage_curve)=if school as u8!=0 {(0xEA56_54B6,0x148,0x168)}else{(0xCA56_5486,0xE8,0x108)};
            if staff_item(sim,occupant,purpose) as u8==0{return;}
            let result=if ignore_strike as u8==0 && building_on_strike(this,occupant) as u8!=0 {
                let factor=if school as u8!=0 {sim.school_strike_efficiency()}else{sim.health_strike_efficiency()};
                multiply(F80::from_f32(staff),factor)
            }else{
                let input=multiply(F80::from_f32(staff),*hundred()).to_f32();
                multiply(curve_output(this.add(curve),input),*percent())
            };
            *efficiency=result.to_f32();
            let input=multiply(F80::from_f32(vehicle),*hundred()).to_f32();
            *coverage=multiply(curve_output(this.add(coverage_curve),input),*percent()).to_f32();
        }
    }
}

/// Multiply an x87 value by a stored single, keeping the stored operand's NaN behavior.
fn multiply(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value=in(reg) &value,operand=in(reg) &operand,result=in(reg) &mut result,out("st(0)") _);
    }
    result
}

port! {
    /// Set the department strike flag and date, then post its start message with the occupant payload.
    #[port(address=0x006F_84C0,owner="residential",name="cSC4ResidentialSimulator::BeginStrike",entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006f84c0(this:*mut u8,[entry:u32],block:*mut u8,occupant:*mut u8)->u32 {
        unsafe {
            let d=Department::from_ptr(block);
            if d.strike()!=0 {return entry & !0xFF;}
            d.set_strike(1);
            let city=CivicSim::from_ptr(this).city();
            let clock=vcall!(city,0xC8 => fn()->*mut u8);
            let clock=vcall!(clock,0x1C => fn()->*mut u8);
            d.set_strike_day(vcall!(clock,0x68 => fn()->u32));
            let storage=allocate(0x2C);
            let message=if storage.is_null(){core::ptr::null_mut()}else{construct_message(storage)};
            vcall!(message,4 => fn()->u32);
            vcall!(message,0x14 => fn(u32)->u32,d.final_word());
            vcall!(message,0x30 => fn(*mut u8)->u32,occupant);
            vcall!(message,0x84 => fn(*mut u8)->u32,occupant);
            let server= *message_server();
            vcall!(message,0x40 => fn(u32)->u32,1);
            vcall!(server,0x10 => fn(*mut u8,u32)->u32,message,0);
            (vcall!(message,8 => fn()->u32)&!0xFF)|1
        }
    }
}
port! {
    /// Clear the strike and date, and post its end message with the supplied elapsed days.
    #[port(address=0x006F_8570,owner="residential",name="cSC4ResidentialSimulator::EndStrike",entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006f8570(_this:*mut u8,[entry:u32],block:*mut u8,days:u32)->u32 {
        unsafe {
            let d=Department::from_ptr(block);
            if d.strike()==0 {return entry & !0xFF;}
            d.set_strike(0);
            d.set_strike_day(0);
            let storage=allocate(0x2C);
            let message=if storage.is_null(){core::ptr::null_mut()}else{construct_message(storage)};
            vcall!(message,4 => fn()->u32);
            vcall!(message,0x14 => fn(u32)->u32,d.final_word());
            let server= *message_server();
            vcall!(message,0x40 => fn(u32)->u32,0);
            vcall!(message,0x54 => fn(u32)->u32,days);
            vcall!(server,0x10 => fn(*mut u8,u32)->u32,message,0);
            (vcall!(message,8 => fn()->u32)&!0xFF)|1
        }
    }
}
port! {
    /// Multiply the three modifiers in list order, storing each product before reading the next one.
    #[port(address=0x006F_9340,owner="residential",name="cSC4ResidentialSimulator::GetOrdinanceModifiers")]
    pub unsafe extern "stdcall" fn fc4p_006f9340(_occupant:*mut u8,list:*mut *mut u8,radius:*mut f32,capacity:*mut f32,third:*mut f32)->u32 {
        unsafe {
            let head= *list;
            let mut node=Effect::from_ptr(head).next();
            *radius=1.0;
            *third=1.0;
            *capacity=1.0;
            while node!=head {
                let e=Effect::from_ptr(node);
                *radius=multiply(F80::from_f32(*radius),e.radius()).to_f32();
                *capacity=multiply(F80::from_f32(e.capacity()),*capacity).to_f32();
                *third=multiply(F80::from_f32(e.third()),*third).to_f32();
                node=e.next();
            }
            node as u32
        }
    }
}
port! {
    /// Test the requested property groups against the ordinance's allowed group list.
    #[port(address=0x006F_94F0,owner="residential",name="cSC4ResidentialSimulator::DoesOrdinanceAffectOccupant")]
    pub unsafe extern "stdcall" fn fc4p_006f94f0(occupant:*mut u8,effect:*mut u8,property:u32)->u32 {
        unsafe {
            let properties=vcall!(occupant,0x18 => fn()->*mut u8);
            let e=Effect::from_ptr(effect);
            let head=e.groups();
            if Effect::from_ptr(head).next()==head {return (properties as u32 & !0xFF)|1;}
            let property=vcall!(properties,0x24 => fn(u32)->*mut u8,property);
            if property.is_null(){return 0;}
            let values=vcall!(property,0x14 => fn()->*mut u8);
            let data=vcall!(values,0x1A4 => fn()->*const u32);
            let mut count=vcall!(values,0x10 => fn()->u32);
            let mut i=0u32;
            while i<count {
                let head=e.groups();
                let mut node=Effect::from_ptr(head).next();
                while node!=head {
                    if *node.add(8).cast::<u32>()== *data.add(i as usize){break;}
                    node=Effect::from_ptr(node).next();
                }
                if node!=e.groups(){return (node as u32 & !0xFF)|1;}
                i=i.wrapping_add(1);
                count=vcall!(values,0x10 => fn()->u32);
            }
            count & !0xFF
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006f8360::PORT,
    fc4p_006f84c0::PORT,
    fc4p_006f8570::PORT,
    fc4p_006f85f0::PORT,
    fc4p_006f8710::PORT,
    fc4p_006f9340::PORT,
    fc4p_006f94f0::PORT,
];

#[cfg(test)]
mod tests;
