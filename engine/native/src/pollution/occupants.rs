// SPDX-License-Identifier: GPL-3.0-or-later
//! Occupant insertion/removal, source grids, facility lists and budget registration.

use super::*;

game_fn! { fn property_bool(holder: *mut u8, id: u32, output: *mut u8) -> u32 = "C" 0x005F_D390; }
game_fn! { fn property_u32(holder: *mut u8, id: u32, output: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn set_i32(holder: *mut u8, id: u32, value: i32) = "C" 0x005F_E330; }
game_fn! { fn set_u32(holder: *mut u8, id: u32, value: u32) = "C" 0x005F_E3C0; }
game_fn! { fn set_i64(holder: *mut u8, id: u32, value: i64) -> u32 = "C" 0x005F_E450; }
game_fn! { fn change_list(this: *mut u8, list: *mut *mut u8, occupant: *mut u8, insert: u32) = "thiscall" 0x006E_45F0; }
game_fn! { #[allow(clippy::too_many_arguments)] fn add_radius(data: *mut u8, width: i32, height: i32, x: i32, z: i32, shape: f32, amplitude: f32, radius: f32) = "C" 0x005E_4500; }
game_data! { static default_radius: [i32; 4] = 0x00B4_A598; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static budget: *mut u8 = 0x00B4_3D64; }

layout! {
    /// Occupant-change fields in the primary simulator.
    struct ChangedView {
        0x18 => city / set_city: *mut u8,
        0x40 => sources / set_sources: [*mut u8; 4],
        0x48 => garbage / set_garbage: *mut u8,
        0x74 => recycling / set_recycling: *mut u8,
        0xFC => recycling_count / set_recycling_count: u32,
        0x130 => energy_capacity / set_energy_capacity: i32,
        0x20B => dirty_garbage / set_dirty_garbage: u8,
    }
}

unsafe fn property(holder: *mut u8, id: u32) -> *mut u8 {
    // SAFETY: the live occupant property holder.
    unsafe { vcall!(holder, 0x24 => fn(u32) -> *mut u8,id) }
}
unsafe fn array(property: *mut u8, slot: usize) -> *mut u8 {
    // SAFETY: a present property supplies the original variant and array interface.
    unsafe {
        let variant = vcall!(property, 0x14 => fn() -> *mut u8);
        vcall!(variant, slot => fn() -> *mut u8)
    }
}
unsafe fn source(this: *mut u8, index: usize) -> *mut u8 {
    // SAFETY: index is one of the four original source-grid fields.
    unsafe { read_unaligned(this.add(0x40 + index * 4).cast()) }
}
unsafe fn ensure(holder: *mut u8, id: u32) {
    // SAFETY: the holder's property-creation method takes identifier, null value, count one, and flag zero.
    unsafe { vcall!(holder,0x28 => fn(u32,u32,u32,u32),id,0,1,0) };
}
fn clamp(value: i32, end: i32) -> i32 {
    if value < 0 {
        0
    } else if value > end {
        end
    } else {
        value
    }
}

port! {
    /// Adds or removes source pollution and membership for each facility property. The excluded flag skips
    /// all work. Garbage covers the clamped footprint; the four source cones use signed radii and magnitudes.
    /// New plants receive usage/history properties, and sanitation budget entries are registered or removed.
    #[port(address = 0x006E_4680, owner = "pollution", name = "cSC4PollutionSimulator::DoOccupantChanged")]
    pub unsafe extern "thiscall" fn fc4p_006e4680(this: *mut u8, occupant: *mut u8, insert: u32) -> u32 {
        // SAFETY: the original simulator, occupant, property arrays, grids, list headers and budget.
        unsafe {
            let sim = ChangedView::from_ptr(this);
            let holder = vcall!(occupant,0x18 => fn() -> *mut u8);
            let mut excluded = 0;
            let result = property_bool(holder,0x0292_44DB,&mut excluded);
            if result as u8 != 0 && excluded != 0 { return (result & !0xFF) | u32::from(excluded); }
            let pollution = property(holder,0x2781_2851);
            if !pollution.is_null() {
                let emissions = array(pollution,0x1A4).cast::<i32>();
                let radius_property = property(holder,0xAA58_32F3);
                let radii = if radius_property.is_null() { default_radius().cast::<i32>() } else { array(radius_property,0x1A4).cast::<i32>() };
                let mut minimum = [0.0f32;3];
                let mut maximum = [0.0f32;3];
                vcall!(occupant,0x28 => fn(*mut f32,*mut f32),minimum.as_mut_ptr(),maximum.as_mut_ptr());
                let (mut x0,mut z0,mut x1,mut z1) = (0i32,0i32,0i32,0i32);
                vcall!(sim.city(),0x17C => fn(f32,f32,*mut i32,*mut i32),minimum[0],minimum[2],&mut x0,&mut z0);
                vcall!(sim.city(),0x17C => fn(f32,f32,*mut i32,*mut i32),maximum[0],maximum[2],&mut x1,&mut z1);
                let max_x = vcall!(*city(),0x174 => fn() -> i32).wrapping_sub(1);
                let max_z = vcall!(*city(),0x178 => fn() -> i32).wrapping_sub(1);
                x0=clamp(x0,max_x);x1=clamp(x1,max_x);z0=clamp(z0,max_z);z1=clamp(z1,max_z);
                let garbage = read_unaligned(emissions.add(2));
                let garbage = if insert as u8 != 0 {garbage} else {garbage.wrapping_neg()};
                let shift = vcall!(sim.garbage(),0x2C => fn() -> u32);
                let mut x=x0;
                while x<=x1 {
                    let tract_x=x.wrapping_shr(shift);
                    let mut z=z0;
                    while z<=z1 {
                        let value=vcall!(sim.garbage(),0x1C => fn(i32,i32) -> u32,x,z) as i16 as i32;
                        vcall!(sim.garbage(),0x68 => fn(i32,i32,i32),tract_x,z.wrapping_shr(shift),value.wrapping_add(garbage));
                        z=z.wrapping_add(1);
                    }
                    x=x.wrapping_add(1);
                }
                sim.set_dirty_garbage(1);
                let mut position=[0.0f32;3];
                vcall!(occupant,0x20 => fn(*mut f32),position.as_mut_ptr());
                let shape_property=property(holder,0x68EE_9764);
                let defaults=[2.5f32,1.5,0.5,4.0];
                let shapes=if shape_property.is_null() {defaults.as_ptr()} else {array(shape_property,0x1BC).cast::<f32>()};
                for i in 0..4 {
                    let shape=F80::from_f32(read_unaligned(shapes.add(i))).compare(F80::ZERO);
                    if shape.less() || shape.equal() || shape.unordered() {continue;}
                    let width=vcall!(source(this,i),0x30 => fn() -> i32);
                    let height=vcall!(source(this,i),0x34 => fn() -> i32);
                    let(mut x,mut z)=(0i32,0i32);
                    vcall!(source(this,i),0x4C => fn(f32,f32,*mut i32,*mut i32),position[0],position[2],&mut x,&mut z);
                    vcall!(source(this,i),0x28 => fn() -> u32);
                    let radius=read_unaligned(radii.add(i));
                    let amount=read_unaligned(emissions.add(i));
                    let radius=F80::from_i32(if insert as u8 != 0 {radius} else {radius.wrapping_neg()}).to_f32();
                    let amount=F80::from_i32(if insert as u8 != 0 {amount} else {amount.wrapping_neg()}).to_f32();
                    let shape=read_unaligned(shapes.add(i));
                    let data=vcall!(source(this,i),0x60 => fn() -> *mut u8);
                    add_radius(data,width,height,x,z,shape,amount,radius);
                }
            }
            for (capacity,list,used,previous) in [
                (0xC8EB_D131,0x68,0x28EC_F510,0x08FC_DB69),
                (0xA8EB_CF0C,0x6C,0xE8EC_F641,0xA8FC_DBA8),
            ] {
                if !property(holder,capacity).is_null() {
                    change_list(this,this.add(list).cast(),occupant,insert);
                    if insert as u8 != 0 {
                        ensure(holder,used);set_i32(holder,used,0);
                        ensure(holder,previous);set_u32(holder,previous,0);
                    }
                }
            }
            let energy=property(holder,0x48EB_D0B1);
            if !energy.is_null() {
                let capacity=read_unaligned(array(energy,0x124).cast::<i32>());
                change_list(this,this.add(0x70).cast(),occupant,insert);
                if insert as u8 != 0 {
                    ensure(holder,0x88EC_F76C);set_i32(holder,0x88EC_F76C,0);
                    ensure(holder,0x28FC_DBB0);set_u32(holder,0x28FC_DBB0,0);
                    let mut output=0;
                    if property_u32(holder,0x2781_2852,&mut output) as u8 != 0 {
                        ensure(holder,0x08FD_1613);set_u32(holder,0x08FD_1613,output);
                        vcall!(holder,0x34 => fn(u32,u32,u32),0x2781_2852,0,0);
                    }
                    sim.set_energy_capacity(sim.energy_capacity().wrapping_add(capacity));
                } else {
                    sim.set_energy_capacity(sim.energy_capacity().wrapping_sub(capacity));
                }
                if sim.energy_capacity()<0 {sim.set_energy_capacity(0);}
            }
            if !property(holder,0x88ED_1059).is_null() {
                change_list(this,this.add(0x74).cast(),occupant,insert);
                let end=sim.recycling();
                let mut node=ListNode::from_ptr(end).next();
                let mut count=0u32;
                while node!=end {node=ListNode::from_ptr(node).next();count=count.wrapping_add(1);}
                sim.set_recycling_count(count);
            }
            let budget=*budget();
            let mut key=[0u32;8];
            let result=vcall!(budget,0xCC => fn(*mut u8,u32,*mut u32) -> u32,holder,0xCA58_EDC7,key.as_mut_ptr());
            if result as u8 == 0 || key[1]!=10 {return result;}
            vcall!(budget,if insert as u8 != 0 {0xDC}else{0xE0} => fn(*mut u8,u32),holder,key[6]);
            let cost=(u64::from(key[4]) | (u64::from(key[5])<<32)) as i64;
            set_i64(holder,0x2A58_D350,cost)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e4680::PORT];

#[cfg(test)]
mod tests;
