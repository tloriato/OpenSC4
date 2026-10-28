// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Version-seven residential save writer, relative to the serialization interface.

use crate::port::Port;

layout! {
    /// Saved fields relative to the serialization interface, eight bytes after the primary one.
    pub struct SaveView {
        0x2C => proximity_size / set_proximity_size: u32,
        0x30 => proximity / set_proximity: [u32;3],
        0x3C => population_size / set_population_size: u32,
        0x40 => populations / set_populations: [u32;3],
        0x4C => age / set_age: u32,
        0x50 => school_maps / set_school_maps: *mut u32,
        0x54 => school_maps_end / set_school_maps_end: *mut u32,
        0x5C => health_maps / set_health_maps: *mut u32,
        0x60 => health_maps_end / set_health_maps_end: *mut u32,
        0x68 => grades / set_grades: [u32;2],
        0x70 => quotient_size / set_quotient_size: u32,
        0x74 => eq_maps / set_eq_maps: [u32;3],
        0x80 => hq_maps / set_hq_maps: [u32;3],
        0x8C => state / set_state: u32,
        0x90 => unused / set_unused: [u32;2],
        0x98 => dirty / set_dirty: u8,
        0x9C => population / set_population: u32,
        0xA0 => running_population / set_running_population: u32,
        0x1A0 => schools / set_schools: *mut u32,
        0x1A4 => schools_end / set_schools_end: *mut u32,
        0x1AC => hospitals / set_hospitals: *mut u32,
        0x1B0 => hospitals_end / set_hospitals_end: *mut u32,
        0x340 => life_sum / set_life_sum: u64,
        0x3C0 => school_auto / set_school_auto: u8,
        0x3C1 => health_auto / set_health_auto: u8,
    }
}
game_fn! { fn write_reference(stream:*mut u8,object:u32)->u32 = "C" 0x005D_99A0; }
game_fn! { fn write_list(stream:*mut u8,list:*mut u8)->u32 = "C" 0x005D_AFF0; }
game_fn! { fn write_iterator(stream:*mut u8,iterator:*mut u8)->u32 = "C" 0x005D_A480; }
game_fn! { fn write_boolean(stream:*mut u8,value:u32)->u64 = "C" 0x005D_9FD0; }
game_fn! { fn write_department(this:*mut u8,stream:*mut u8,department:*mut u8)->u32 = "thiscall" 0x006F_7B00; }

unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { p.add(offset).cast::<u32>().read_unaligned() }
}
unsafe fn count(p: *mut u8, offset: usize) -> u32 {
    unsafe { (word(p, offset + 4).wrapping_sub(word(p, offset)) as i32 >> 2) as u32 }
}
unsafe fn reference_vector(this: *mut u8, stream: *mut u8, offset: usize) {
    unsafe {
        vcall!(stream,0x24 => fn(u32)->u32,count(this,offset));
        let mut i = 0u32;
        while i < count(this, offset) {
            write_reference(stream, *(word(this, offset) as *const u32).wrapping_add(i as usize));
            i = i.wrapping_add(1);
        }
    }
}
unsafe fn list_vector(this: *mut u8, stream: *mut u8, offset: usize) {
    unsafe {
        vcall!(stream,0x24 => fn(u32)->u32,count(this,offset));
        let mut i = 0u32;
        while i < count(this, offset) {
            write_list(stream, (word(this, offset) as *mut u8).wrapping_add(i as usize * 4));
            i = i.wrapping_add(1);
        }
    }
}
port! {
    /// Write every saved field in version-seven order. Calls proceed despite individual failures;
    /// only AL of the last helper's EDX:EAX result is replaced by one.
    #[port(address=0x006F_8C40,owner="residential",name="cSC4ResidentialSimulator::Write")]
    pub unsafe extern "thiscall" fn fc4p_006f8c40(this:*mut u8,stream:*mut u8)->u64 {
        unsafe{
            let view=SaveView::from_ptr(this);
            vcall!(stream,0x1C => fn(u32)->u32,7);
            vcall!(stream,0x20 => fn(u32)->u32,view.proximity_size());
            for offset in [0x30,0x34,0x38]{write_reference(stream,word(this,offset));}
            vcall!(stream,0x20 => fn(u32)->u32,view.population_size());
            for offset in [0x40,0x44,0x48,0x4C]{write_reference(stream,word(this,offset));}
            reference_vector(this,stream,0x50);reference_vector(this,stream,0x5C);
            for offset in [0x68,0x6C]{write_reference(stream,word(this,offset));}
            vcall!(stream,0x20 => fn(u32)->u32,view.quotient_size());
            for offset in [0x74,0x78,0x7C,0x80,0x84,0x88]{write_reference(stream,word(this,offset));}
            vcall!(stream,0x30 => fn(u32)->u32,word(this,0x90));
            vcall!(stream,0x30 => fn(u32)->u32,word(this,0x94));
            vcall!(stream,0x24 => fn(u32)->u32,view.state());
            let changed=view.dirty()!=0||view.state()!=10;
            let dirty_word=(stream as u32 & !0xFF)|u32::from(changed);
            vcall!(stream,0x14 => fn(u32)->u32,dirty_word);
            vcall!(stream,0x20 => fn(u32)->u32,view.population());
            write_department(this.sub(8),stream,this.add(0xCC));
            write_department(this.sub(8),stream,this.add(0x12C));
            for offset in [0x194,0x198,0x19C]{write_list(stream,this.add(offset));}
            list_vector(this,stream,0x1A0);list_vector(this,stream,0x1AC);
            write_list(stream,this.add(0x1B8));
            write_iterator(stream,this.add(0x320));
            vcall!(stream,0x2C => fn(u64)->u32,view.life_sum());
            vcall!(stream,0x20 => fn(u32)->u32,view.running_population());
            write_boolean(stream,u32::from(view.school_auto()));
            (write_boolean(stream,u32::from(view.health_auto())) & !0xFF)|1
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_006f8c40::PORT];

#[cfg(test)]
mod tests;
