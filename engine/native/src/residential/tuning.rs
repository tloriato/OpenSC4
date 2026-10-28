// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population tuning and ordinance decay products.

use super::collections::Node;
use super::curves::{default_points, free, lookup, multiply_percent, set_points, set_resource, Points, KEY};
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Ordinance records are visited before any output defaults are stored.
    pub struct TuningSim {
        0x194 => health / set_health: *mut u8,
        0x198 => education / set_education: *mut u8,
        0x3C0 => changed / set_changed: u8,
    }
}
layout! {
    /// One ordinance's decay multiplier, following its two curves and list.
    struct Ordinance {
        0x38 => multiplier / set_multiplier: f32,
    }
}
layout! {
    /// Population parameters; byte 0x0F, HQ bonuses and existing curve storage have no code default.
    pub struct PopulationTuning {
        0x00 => eq_decay / set_eq_decay: f32,
        0x04 => hq_decay / set_hq_decay: f32,
        0x08 => low_age / set_low_age: u16,
        0x0A => medium_age / set_medium_age: u16,
        0x0C => high_age / set_high_age: u16,
        0x0E => reset_age / set_reset_age: u8,
        0x10 => eq_reset / set_eq_reset: f32,
        0x14 => hq_reset / set_hq_reset: f32,
        0x18 => low_eq / set_low_eq: f32,
        0x1C => medium_eq / set_medium_eq: f32,
        0x20 => high_eq / set_high_eq: f32,
        0x24 => low_hq / set_low_hq: f32,
        0x28 => medium_hq / set_medium_hq: f32,
        0x2C => high_hq / set_high_hq: f32,
        0x30 => door_hq / set_door_hq: f32,
        0x34 => effect_chance / set_effect_chance: f32,
        0x38 => effect_height / set_effect_height: f32,
        0x3C => effect_delta / set_effect_delta: f32,
    }
}
game_fn! { fn read_float(holder:*mut u8,id:u32,out:*mut u8)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn read_u16(holder:*mut u8,id:u32,out:*mut u8)->u32 = "C" 0x005F_D3F0; }
game_fn! { fn read_u8(holder:*mut u8,id:u32,out:*mut u8)->u32 = "C" 0x005F_D3C0; }
game_fn! { fn read_percent(holder:*mut u8,id:u32,out:*mut u8)->u32 = "C" 0x005F_DAC0; }
game_data! { static one:f32 = 0x00A8_1228; }

unsafe fn product(head: *mut u8) -> f32 {
    unsafe {
        let mut value = F80::from_f32(*one());
        let mut node = Node::from_ptr(head).next();
        while node != head {
            value = multiply_percent(value, Ordinance::from_ptr(node).multiplier());
            node = Node::from_ptr(node).next();
        }
        value.to_f32()
    }
}

port! {
    /// Set population defaults, read available properties and rebuild the pollution curves when the
    /// resource exists. Keep padding, missing HQ bonuses and curves when the resource is absent.
    /// Apply the two ordinance products after loading and mark the population tuning changed.
    #[port(address=0x006F_C0C0,owner="residential",name="cSC4ResidentialSimulator::ReadTunablePopulationValues")]
    pub unsafe extern "thiscall" fn fc4p_006fc0c0(this:*mut u8,out:*mut u8) {
        unsafe {
            let sim=TuningSim::from_ptr(this);
            let education=product(sim.education());let health=product(sim.health());
            let tuning=PopulationTuning::from_ptr(out);
            tuning.set_eq_decay(1.0);tuning.set_hq_decay(1.0);
            tuning.set_low_age(25);tuning.set_medium_age(30);tuning.set_high_age(35);tuning.set_reset_age(40);
            tuning.set_eq_reset(f32::from_bits(0x3F33_3333));tuning.set_hq_reset(f32::from_bits(0x3F33_3333));
            tuning.set_low_eq(4.0);tuning.set_medium_eq(6.0);tuning.set_high_eq(8.0);
            tuning.set_door_hq(0.0);tuning.set_effect_chance(0.0);tuning.set_effect_height(0.0);tuning.set_effect_delta(0.0);
            let key=KEY;let mut resource=MaybeUninit::<*mut u8>::uninit();
            let result=lookup(key.as_ptr(),resource.as_mut_ptr());
            if result as u8!=0 {
                let resource=resource.assume_init();let holder=vcall!(resource,0x1C => fn()->*mut u8);
                read_float(holder,0xC929_C433,out);read_float(holder,0x0929_C439,out.add(4));
                read_u16(holder,0xC929_C652,out.add(8));read_u16(holder,0x0929_CC30,out.add(0x0A));read_u16(holder,0x0929_CC36,out.add(0x0C));
                read_u8(holder,0xCA02_7307,out.add(0x0E));
                read_percent(holder,0xCA02_7325,out.add(0x10));read_percent(holder,0xCA02_7328,out.add(0x14));
                for (id,offset) in [
                    (0xC929_CC58,0x18),(0x0929_CC65,0x1C),(0x2929_CC6B,0x20),
                    (0xC929_CC8D,0x24),(0x6929_CC92,0x28),(0x4929_CC97,0x2C),
                    (0x6AB7_0DA7,0x30),(0x6AB7_1654,0x34),(0x4AB7_1659,0x38),(0x4AB7_0DAC,0x3C),
                ] {read_float(holder,id,out.add(offset));}
                let mut storage=[0u32;3];let points=Points::from_ptr(storage.as_mut_ptr().cast());let mut point=[0u32;2];
                default_points(points,&mut point,[0,0],[32000f32.to_bits(),100f32.to_bits()],(&resource as *const *mut u8).cast());
                for offset in [0x40,0x50,0x60] {set_points(out.add(offset),points.as_ptr());}
                for (id,offset) in [(0x6941_B022,0x40),(0xA941_B028,0x50),(0xC941_B02E,0x60)] {set_resource(out.add(offset),key.as_ptr(),id);}
                vcall!(resource,8 => fn()->u32);
                if !points.begin().is_null(){free(points.begin().cast());}
            }
            tuning.set_eq_decay(multiply_percent(F80::from_f32(education),tuning.eq_decay()).to_f32());
            tuning.set_hq_decay(multiply_percent(F80::from_f32(health),tuning.hq_decay()).to_f32());
            sim.set_changed(1);
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_006fc0c0::PORT];

#[cfg(test)]
mod tests;
