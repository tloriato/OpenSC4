// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Grade service buildings and apply their effects to the three wealth grids.

use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Population, age and ordinance fields used by building grading.
    struct GradeSim {
        0x48 => population_low / set_population_low: *mut u8,
        0x54 => age / set_age: *mut u8,
        0x324 => slop / set_slop: f32,
    }
}
layout! {
    /// Circular links and tract coordinates.
    struct Node {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => x / set_x: u32,
        0x0C => z / set_z: u32,
    }
}
layout! {
    /// Owned allocation of a temporary curve.
    struct Curve {
        0x00 => storage / set_storage: *mut u8,
    }
}

game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(storage:*mut u8)->u32 = "C" 0x0090_CF63; }
game_fn! { fn curve_new(this:*mut u8)->u32 = "thiscall" 0x007D_0410; }
game_fn! { fn read_float(holder:*mut u8,id:u32,out:*mut f32)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn read_uint(holder:*mut u8,id:u32,out:*mut u32)->u32 = "C" 0x005F_D480; }
game_fn! { fn write_uint(holder:*mut u8,id:u32,value:u32)->u32 = "C" 0x005F_E3C0; }
game_fn! { fn write_float(holder:*mut u8,id:u32,value:f32)->u32 = "C" 0x005F_E4F0; }
game_fn! { fn efficiency(this:*mut u8,occupant:*mut u8,staff:*mut f32,vehicle:*mut f32,school:u32,ignore_strikes:u32) = "thiscall" 0x006F_8710; }
game_fn! { fn base_efficiency(this:*mut u8,out:*mut f32,occupant:*mut u8,school:u32)->*mut f32 = "thiscall" 0x006F_85F0; }
game_fn! { fn property_curves(this:*mut u8,holder:*mut u8,age:*mut u8,wealth:*mut u8,distance:*mut u8,school:u32) = "thiscall" 0x006F_C400; }
game_fn! { fn clear_effects(this:*mut *mut u8)->u32 = "thiscall" 0x006F_CB40; }
game_fn! { fn select_effects(this:*mut u8,occupant:*mut u8,kind:u32,source:*mut *mut u8,out:*mut *mut u8)->u32 = "thiscall" 0x006F_D070; }
game_fn! { fn modifiers(this:*mut u8,occupant:*mut u8,effects:*mut *mut u8,radius:*mut f32,capacity:*mut f32,strength:*mut f32)->u32 = "thiscall" 0x006F_9340; }
game_fn! { fn covered_tracts(this:*mut u8,occupant:*mut u8,radius:f32,scale:f32,x:u32,z:u32,list:*mut *mut u8)->u32 = "thiscall" 0x006F_7E00; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn population_served(this:*mut u8,occupant:*mut u8,radius:f32,capacity:u32,age:*mut u8,wealth:*mut u8,distance:*mut u8,effects:*mut *mut u8,coverage:*mut u8,chunk:u32,tracts:*mut *mut u8,unused:*mut u8)->u32 = "thiscall" 0x006F_8F60;
}
game_fn! { fn effect_curves(this:*mut u8,occupant:*mut u8,kind:u32,effects:*mut *mut u8,distance:*mut *mut u8,age:*mut *mut u8)->u32 = "thiscall" 0x006F_CD70; }
game_fn! { fn copy_curve(this:*mut u8,source:*mut u8)->u32 = "thiscall" 0x005F_4430; }
game_fn! { fn curve_output(this:*mut u8,input:f32)->St0 = "thiscall" 0x005F_4250; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn radius_effect(age:*mut u8,output:*mut u8,age_curves:*mut *mut u8,x:f32,z:f32,radius:f32,distance_curves:*mut *mut u8,intensity:f32,context:u32,units:u32,coverage:*mut u8,population:*mut u8,extra:u32)->u32 = "C" 0x005E_48F0;
}
game_fn! { fn clear_curves(this:*mut *mut u8)->u32 = "thiscall" 0x006F_CBD0; }

game_data! { static unsigned_bias:f32 = 0x00A8_0AA8; }
game_data! { static maximum:f32 = 0x00B0_D440; }
game_data! { static default_grade:f32 = 0x00B4_AF90; }
game_data! { static zero:f32 = 0x00A8_1054; }
game_data! { static one:f32 = 0x00A8_1228; }
game_data! { static percent:f32 = 0x00A8_23A4; }
game_data! { static lots:*mut u8 = 0x00B4_3D08; }
game_data! { static terrain:*mut u8 = 0x00B4_3CEC; }

macro_rules! stored_operation {
    ($name:ident,$instruction:literal) => {
        fn $name(value:F80,operand:f32)->F80 {
            let mut out=F80::ZERO;
            unsafe {
                core::arch::asm!("fld tbyte ptr [{value}]",$instruction,"fstp tbyte ptr [{out}]",
                    value=in(reg)&value,operand=in(reg)&operand,out=in(reg)&mut out,out("st(0)") _);
            }
            out
        }
    };
}
stored_operation!(multiply, "fmul dword ptr [{operand}]");
stored_operation!(add, "fadd dword ptr [{operand}]");

unsafe fn unsigned(value: u32) -> F80 {
    let result = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        result
    } else {
        unsafe { add(result, *unsigned_bias()) }
    }
}

fn greater(value: F80, limit: F80) -> bool {
    value.compare(limit).status_word() & 0x4100 == 0
}

unsafe fn ratio(capacity: u32, served: u32) -> f32 {
    unsafe {
        let value = if capacity == 0 {
            *default_grade()
        } else if served == 0 {
            *maximum()
        } else {
            (unsigned(capacity) / unsigned(served)).to_f32()
        };
        if greater(F80::from_f32(value), F80::from_f32(*maximum())) {
            *maximum()
        } else {
            value
        }
    }
}

unsafe fn head(size: u32) -> *mut u8 {
    unsafe {
        let storage = allocate(size);
        Node::from_ptr(storage).set_next(storage);
        Node::from_ptr(storage).set_previous(storage);
        storage
    }
}

unsafe fn clear_tracts(list: *mut *mut u8) {
    unsafe {
        let sentinel = list.read();
        let mut node = Node::from_ptr(sentinel).next();
        while node != list.read() {
            let previous = node;
            node = Node::from_ptr(node).next();
            free(previous);
        }
        Node::from_ptr(list.read()).set_next(list.read());
        Node::from_ptr(list.read()).set_previous(list.read());
    }
}

unsafe fn append_curve(list: *mut u8, source: *mut u8) {
    unsafe {
        let storage = allocate(0x18);
        let curve = storage.wrapping_add(8);
        if !curve.is_null() {
            copy_curve(curve, source);
        }
        let tail = Node::from_ptr(list).previous();
        Node::from_ptr(storage).set_next(list);
        Node::from_ptr(storage).set_previous(tail);
        Node::from_ptr(tail).set_next(storage);
        Node::from_ptr(list).set_previous(storage);
    }
}

port! {
    /// Grade the selected service, blend its tract grades and apply each wealth's radius effect.
    /// A missing capacity retains the radius property's ID as its value. A missing radius writes
    /// zero served and effective capacity, and the global default grade. Grade blends multiply the
    /// old value by coverage minus one; zero coverage keeps the resulting NaN unless it exceeds one.
    #[port(address=0x006F_D970,owner="residential",name="cSC4ResidentialSimulator::ComputeOccupantGrade")]
    pub unsafe extern "thiscall" fn fc4p_006fd970(this:*mut u8,occupant:*mut u8,coverage:*mut u8,chunk:u32,grade_grid:*mut u8,boost_grids:*mut *mut u8,context:u32,school:u32,extra:u32)->u32 {
        unsafe {
            let sim = GradeSim::from_ptr(this);
            let original_head = head(0x3C);
            let mut effects = original_head;
            let mut age_storage = [MaybeUninit::<u32>::uninit();4];
            let mut wealth_storage = [MaybeUninit::<u32>::uninit();4];
            let mut distance_storage = [MaybeUninit::<u32>::uninit();4];
            let age = age_storage.as_mut_ptr().cast::<u8>();
            let wealth = wealth_storage.as_mut_ptr().cast::<u8>();
            let distance = distance_storage.as_mut_ptr().cast::<u8>();
            curve_new(age);
            curve_new(wealth);
            curve_new(distance);
            let (kind, radius_id, capacity_id, boost_id, served_id, effective_id, grade_id, staff_id, source) = if school as u8 != 0 {
                (0xA92D_8E40,0x691B_42B3,0x8922_041B,0xA92A_E446,0x0922_0161,0xE924_4B2E,0x2924_53C5,0x2A97_6C20,this.add(0x198))
            } else {
                (0x292D_8F9B,0x091B_42A2,0x6922_0415,0xC929_9659,0xC922_015C,0x4924_4B21,0x0924_53CB,0xAA97_6BE7,this.add(0x194))
            };
            let holder = vcall!(occupant,0x18 => fn()->*mut u8);
            let radius_holder = vcall!(occupant,0x18 => fn()->*mut u8);
            let mut radius = MaybeUninit::<f32>::uninit();
            let mut effective_capacity = 0u32;
            let mut served = 0u32;
            let grade;
            if read_float(radius_holder,radius_id,radius.as_mut_ptr()) as u8 != 0 {
                let mut radius = radius.assume_init();
                let mut staff = 0.0f32;
                let mut vehicle = 0.0f32;
                efficiency(this,occupant,&mut staff,&mut vehicle,school,0);
                let mut base = MaybeUninit::<f32>::uninit();
                let base = base_efficiency(this,base.as_mut_ptr(),occupant,school).read();
                property_curves(this,holder,age,wealth,distance,school);
                clear_effects(&mut effects);
                select_effects(this,occupant,kind,source.cast(),&mut effects);
                let mut m1 = MaybeUninit::<f32>::uninit();
                let mut m2 = MaybeUninit::<f32>::uninit();
                let mut m3 = MaybeUninit::<f32>::uninit();
                modifiers(this,occupant,&mut effects,m1.as_mut_ptr(),m2.as_mut_ptr(),m3.as_mut_ptr());
                let (m1,m2,m3) = (m1.assume_init(),m2.assume_init(),m3.assume_init());
                radius = multiply(multiply(F80::from_f32(vehicle),m1),radius).to_f32();
                let mut tracts = head(0x10);
                clear_tracts(&mut tracts);
                let grid = sim.population_low();
                let z = vcall!(grid,0x34 => fn()->u32);
                let x = vcall!(grid,0x30 => fn()->u32);
                let scale = vcall_st0!(grid,0x38).to_f32();
                covered_tracts(this,occupant,radius,scale,x,z,&mut tracts);
                let mut capacity = radius_id;
                read_uint(holder,capacity_id,&mut capacity);
                served = population_served(this,occupant,radius,capacity,age,wealth,distance,&mut effects,coverage,chunk,&mut tracts,grade_grid);
                let staff_capacity = multiply(multiply(unsigned(capacity),base),m2).ftol() as u32;
                let staff_ratio = ratio(staff_capacity,served);
                write_float(holder,staff_id,staff_ratio);
                effective_capacity = multiply(multiply(unsigned(capacity),staff),m2).ftol() as u32;
                grade = ratio(effective_capacity,served);
                let units = capacity / chunk;
                let sentinel = tracts;
                let mut node = Node::from_ptr(sentinel).next();
                if node != sentinel {
                    let contribution = multiply(unsigned(units),grade).to_f32();
                    while node != sentinel {
                        let tract = Node::from_ptr(node);
                        let n = vcall!(coverage,0x58 => fn(u32,u32)->u32,tract.x(),tract.z()) as u8;
                        let old = if n > 1 {
                            let previous = vcall_st0!(grade_grid,0x58,tract.x(),tract.z());
                            previous * unsigned(u32::from(n - 1))
                        } else { F80::from_f32(*zero()) };
                        let value = old + (F80::from_f32(contribution) / unsigned(u32::from(n)));
                        let stored = value.to_f32();
                        let result = if greater(value,F80::from_f32(*one())) { 1.0 } else { stored };
                        vcall!(grade_grid,0x68 => fn(u32,u32,f32)->u32,tract.x(),tract.z(),result);
                        node = Node::from_ptr(node).next();
                    }
                }
                let mut boost_multiplier = 1.0f32;
                read_float(holder,boost_id,&mut boost_multiplier);
                let lot = vcall!(*lots(),0x38 => fn(*mut u8)->*mut u8,occupant);
                let mut position = [MaybeUninit::<f32>::uninit();3];
                if lot.is_null() { vcall!(occupant,0x20 => fn(*mut f32)->u32,position.as_mut_ptr().cast()); }
                else { vcall!(lot,0x3C => fn(*mut f32)->u32,position.as_mut_ptr().cast()); }
                let mut tx = MaybeUninit::<u32>::uninit();
                let mut tz = MaybeUninit::<u32>::uninit();
                let (px,pz) = (position[0].assume_init(),position[2].assume_init());
                vcall!(*terrain(),0x17C => fn(f32,f32,*mut u32,*mut u32)->u32,px,pz,tz.as_mut_ptr(),tx.as_mut_ptr());
                if greater(F80::from_f32(radius),F80::from_f32(*zero())) {
                    let mut distance_curves = head(0x18);
                    let mut age_curves = head(0x18);
                    effect_curves(this,occupant,0xA92D_8E40,&mut effects,&mut distance_curves,&mut age_curves);
                    append_curve(age_curves,age);
                    append_curve(distance_curves,distance);
                    for wealth_index in 0..3 {
                        let input = F80::from_i32(wealth_index as i32).to_f32();
                        let wealth_factor = multiply(curve_output(wealth,input),*percent());
                        let population = this.add(0x48+wealth_index*4).cast::<*mut u8>().read_unaligned();
                        let intensity = multiply(multiply(F80::from_f32(grade)*wealth_factor,boost_multiplier),m3).to_f32();
                        let radius = multiply(F80::from_f32(radius),sim.slop()).to_f32();
                        let output = boost_grids.add(wealth_index).read_unaligned();
                        radius_effect(sim.age(),output,&mut age_curves,px,pz,radius,&mut distance_curves,intensity,context,units,coverage,population,extra);
                    }
                    clear_curves(&mut age_curves);
                    if !age_curves.is_null() { free(age_curves); }
                    clear_curves(&mut distance_curves);
                    if !distance_curves.is_null() { free(distance_curves); }
                }
                clear_tracts(&mut tracts);
                free(tracts);
            } else { grade = *default_grade(); }
            write_uint(holder,served_id,served);
            write_uint(holder,effective_id,effective_capacity);
            write_float(holder,grade_id,grade);
            for curve in [distance,wealth,age] {
                let storage = Curve::from_ptr(curve).storage();
                if !storage.is_null() { free(storage); }
            }
            clear_effects(&mut effects);
            free(original_head)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006fd970::PORT];

#[cfg(test)]
mod tests;
