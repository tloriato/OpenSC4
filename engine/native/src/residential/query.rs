// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! School and hospital query records and system totals.

use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// The building query's string header and scalar fields.
    pub struct Query {
        0x04 => string / set_string: *mut u8,
        0x14 => capacity / set_capacity: u32,
        0x18 => effective / set_effective: u32,
        0x1C => capacity_staff / set_capacity_staff: u32,
        0x20 => effective_staff / set_effective_staff: u32,
        0x24 => served / set_served: u32,
        0x28 => grade / set_grade: f32,
        0x2C => staff_funding / set_staff_funding: u32,
        0x30 => staff_cost / set_staff_cost: u32,
        0x34 => vehicle_funding / set_vehicle_funding: u32,
        0x38 => vehicle_cost / set_vehicle_cost: u32,
    }
}
layout! {
    /// Query dependencies in the residential simulator.
    pub struct QuerySim {
        0xD0 => budget / set_budget: *mut u8,
        0x2F4 => school_divisor / set_school_divisor: u32,
        0x2F8 => hospital_divisor / set_hospital_divisor: u32,
    }
}
layout! {
    /// A vector's first and past-last pointers.
    pub struct Vector {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
    }
}
game_fn! { fn read_uint(properties:*mut u8,id:u32,out:*mut u32)->u64 = "C" 0x005F_D480; }
game_fn! { fn read_float(properties:*mut u8,id:u32,out:*mut f32)->u64 = "C" 0x005F_D4F0; }
game_fn! { fn allocate_string(bytes:u32)->*mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free_string(buffer:*mut u8)->u64 = "C" 0x0090_CF63; }
game_fn! { fn system_totals(this:*mut u8,lists:*mut u8,out:*mut u8,school:u32,groups:*mut u8)->u64 = "thiscall" 0x006F_A070; }
game_data! { static one:u32 = 0x00B0_D440; }
game_data! { static zero:u32 = 0x00B4_AF90; }

/// Use the unsigned divide instruction, including its fault on a zero divisor.
fn divide(numerator: u32, divisor: u32) -> u32 {
    let result: u32;
    unsafe {
        core::arch::asm!("div {divisor:e}",divisor=in(reg) divisor,inlateout("eax") numerator=>result,inlateout("edx") 0u32=>_,options(nomem,nostack));
    }
    result
}
fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

/// Both services read the same record shape, but a hospital requires the staff budget item.
unsafe fn building_query(this: *mut u8, occupant: *mut u8, out: *mut u8, school: bool) -> u64 {
    unsafe {
        let sim = QuerySim::from_ptr(this);
        let properties = vcall!(occupant,0x18 => fn()->*mut u8);
        let (capacity, effective, served, grade, staff, vehicle) = if school {
            (
                0x8922_041B,
                0xE924_4B2E,
                0x0922_0161,
                0x2924_53C5,
                0xEA56_54B6,
                0x4A56_54BA,
            )
        } else {
            (
                0x6922_0415,
                0x4924_4B21,
                0xC922_015C,
                0x0924_53CB,
                0xCA56_5486,
                0xEA56_549E,
            )
        };
        let mut scratch = MaybeUninit::<u32>::uninit();
        let result = read_uint(properties, capacity, scratch.as_mut_ptr());
        if result as u8 == 0 {
            return result & !0xFF;
        }
        let mut item = [MaybeUninit::<u32>::uninit(); 8];
        item[7].write(0);
        for (id, offset) in [(capacity, 0x14), (effective, 0x18), (served, 0x24)] {
            let result = read_uint(properties, id, out.add(offset).cast());
            if result as u8 == 0 {
                return result & !0xFF;
            }
        }
        let mut grade_value = MaybeUninit::<f32>::uninit();
        let result = read_float(properties, grade, grade_value.as_mut_ptr());
        if result as u8 == 0 {
            return result & !0xFF;
        }
        let q = Query::from_ptr(out);
        if !school {
            let result = vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut MaybeUninit<u32>)->u64,properties,staff,item.as_mut_ptr());
            if result as u8 == 0 {
                return result & !0xFF;
            }
        }
        out.add(0x28).cast::<u32>().write(grade_value.assume_init().to_bits());
        if q.served() == u32::MAX {
            q.set_served(0);
        }
        let divisor = if school {
            sim.school_divisor()
        } else {
            sim.hospital_divisor()
        };
        q.set_capacity_staff(divide(q.capacity(), divisor));
        let effective = q.effective();
        let divisor = if school {
            sim.school_divisor()
        } else {
            sim.hospital_divisor()
        };
        let numerator = if school {
            divisor.wrapping_add(effective).wrapping_sub(1)
        } else {
            effective
        };
        let effective_staff = divide(numerator, divisor);
        if school {
            q.set_effective_staff(effective_staff);
            let result = vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut MaybeUninit<u32>)->u64,properties,staff,item.as_mut_ptr());
            if result as u8 != 0 {
                q.set_staff_cost(item[4].assume_init());
                q.set_staff_funding(item[7].assume_init());
            } else {
                q.set_staff_cost(0);
                q.set_staff_funding(*one());
            }
        } else {
            q.set_staff_funding(item[7].assume_init());
            q.set_staff_cost(item[4].assume_init());
            q.set_effective_staff(effective_staff);
        }
        let result = vcall!(sim.budget(),0xCC => fn(*mut u8,u32,*mut MaybeUninit<u32>)->u64,properties,vehicle,item.as_mut_ptr());
        if result as u8 != 0 {
            let cost = item[4].assume_init();
            let funding = item[7].assume_init();
            if school {
                q.set_vehicle_funding(funding);
                q.set_vehicle_cost(cost);
                registers((funding & !0xFF) | 1, cost)
            } else {
                q.set_vehicle_cost(cost);
                q.set_vehicle_funding(funding);
                registers((cost & !0xFF) | 1, (result >> 32) as u32)
            }
        } else {
            q.set_vehicle_cost(0);
            let funding = *one();
            q.set_vehicle_funding(funding);
            registers(
                (result as u32 & !0xFF) | 1,
                if school { (result >> 32) as u32 } else { funding },
            )
        }
    }
}
port! {
    /// Fill hospital query scalars, stopping at the first missing required property or staff item.
    #[port(address=0x006F_7E80,owner="residential",name="cSC4ResidentialSimulator::GetHospitalQueryData")]
    pub unsafe extern "thiscall" fn fc4p_006f7e80(this:*mut u8,occupant:*mut u8,out:*mut u8)->u64 {unsafe{building_query(this,occupant,out,false)}}
}
port! {
    /// Fill school query scalars; optional budget items use default funding and zero cost.
    #[port(address=0x006F_91C0,owner="residential",name="cSC4ResidentialSimulator::GetSchoolQueryData")]
    pub unsafe extern "thiscall" fn fc4p_006f91c0(this:*mut u8,occupant:*mut u8,out:*mut u8)->u64 {unsafe{building_query(this,occupant,out,true)}}
}

fn vector_count(v: Vector) -> u32 {
    ((v.end() as u32).wrapping_sub(v.begin() as u32) as i32 >> 2) as u32
}
fn add_stored(value: F80, operand: f32) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]","fadd dword ptr [{operand}]","fstp tbyte ptr [{result}]",value=in(reg) &value,operand=in(reg) &operand,result=in(reg) &mut result,out("st(0)") _);
    }
    result
}
port! {
    /// Sum matching buildings in traversal order. Query failures still count and retain the previous
    /// query's unwritten fields. Only the six total fields are reset and updated.
    #[port(address=0x006F_A070,owner="residential",name="cSC4ResidentialSimulator::GetSystemTotals")]
    pub unsafe extern "thiscall" fn fc4p_006fa070(this:*mut u8,lists:*mut u8,out:*mut u8,school:u32,groups:*mut u8)->u64 {
        unsafe {
            let mut memory=[MaybeUninit::<u32>::uninit();15];
            let query=memory.as_mut_ptr().cast::<u8>();
            let storage=allocate_string(8);
            *storage=0;
            query.cast::<u32>().write(0x00A8_0810);
            query.add(4).cast::<*mut u8>().write(storage);
            query.add(8).cast::<*mut u8>().write(storage);
            query.add(12).cast::<*mut u8>().write(storage.wrapping_add(8));
            for offset in [0x10,0x28,0x2C,0x34]{query.add(offset).cast::<u32>().write(0);}
            let total=Query::from_ptr(out);
            total.set_capacity(0);total.set_effective(0);total.set_capacity_staff(0);total.set_effective_staff(0);total.set_served(0);
            out.add(0x28).cast::<u32>().write(*zero());
            let lists=Vector::from_ptr(lists);let groups=Vector::from_ptr(groups);
            let mut index=0u32;let mut count=0u32;
            while index<vector_count(lists) {
                let head= *lists.begin().wrapping_add(index as usize) as *mut u8;
                let mut node= *head.cast::<*mut u8>();
                while node!=head {
                    let building= *node.add(8).cast::<*mut u8>();
                    let mut group=0u32;let mut matched=false;
                    while group<vector_count(groups) {
                        if vcall!(building,0x58 => fn(u32)->u32,*groups.begin().wrapping_add(group as usize)) as u8!=0 {matched=true;break;}
                        group=group.wrapping_add(1);
                    }
                    if matched {
                        if school as u8!=0 {vcall!(this,0x68 => fn(*mut u8,*mut u8)->u32,building,query);}
                        else{vcall!(this,0x6C => fn(*mut u8,*mut u8)->u32,building,query);}
                        let q=Query::from_ptr(query);
                        let grade=F80::from_f32(q.grade());
                        let capacity=total.capacity().wrapping_add(q.capacity());
                        let effective=total.effective().wrapping_add(q.effective());
                        let cap_staff=total.capacity_staff().wrapping_add(q.capacity_staff());
                        let eff_staff=total.effective_staff().wrapping_add(q.effective_staff());
                        let served=total.served().wrapping_add(q.served());
                        total.set_capacity(capacity);total.set_effective_staff(eff_staff);total.set_effective(effective);total.set_capacity_staff(cap_staff);total.set_served(served);
                        count=count.wrapping_add(1);
                        total.set_grade(add_stored(grade,total.grade()).to_f32());
                    }
                    node= *node.cast::<*mut u8>();
                }
                index=index.wrapping_add(1);
            }
            query.cast::<u32>().write(0x00A8_0810);
            let buffer=Query::from_ptr(query).string();
            let edx=if buffer.is_null(){lists.as_ptr() as u32}else{(free_string(buffer)>>32) as u32};
            registers(count,edx)
        }
    }
}
port! {
    /// Forward school lists, output record and requested groups to the shared totals function.
    #[port(address=0x006F_A240,owner="residential",name="cSC4ResidentialSimulator::GetSchoolSystemTotals")]
    pub unsafe extern "thiscall" fn fc4p_006fa240(this:*mut u8,out:*mut u8,groups:*mut u8)->u64 {unsafe{system_totals(this,this.add(0x1A8),out,1,groups)}}
}
port! {
    /// Forward hospital lists, output record and requested groups to the shared totals function.
    #[port(address=0x006F_A260,owner="residential",name="cSC4ResidentialSimulator::GetHospitalSystemTotals")]
    pub unsafe extern "thiscall" fn fc4p_006fa260(this:*mut u8,out:*mut u8,groups:*mut u8)->u64 {unsafe{system_totals(this,this.add(0x1B4),out,0,groups)}}
}

pub const PORTS: &[Port] = &[
    fc4p_006f7e80::PORT,
    fc4p_006f91c0::PORT,
    fc4p_006fa070::PORT,
    fc4p_006fa240::PORT,
    fc4p_006fa260::PORT,
];

#[cfg(test)]
mod tests;
