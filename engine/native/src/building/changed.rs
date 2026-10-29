// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Building-map updates and the highest-growth-stage advisor event.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { fn profile(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x0069_B720; }
game_fn! { fn developer_type(purpose: u32, wealth: u32) -> u32 = "C" 0x0069_5790; }
game_fn! { fn highest_stage(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x006C_0F30; }
game_fn! { fn gzcom() -> *mut u8 = "C" 0x0090_DDF1; }
game_data! { static profiles: u8 = 0x00B4_8584; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }

layout! {
    /// A building's catalogue profile, returned by reference.
    pub struct Profile {
        0x00 => purpose / set_purpose: u32,
        0x04 => wealth / set_wealth: u32,
    }
}

port! {
    /// Writes the raw building-map byte across the lot rectangle and records any new highest growth stage.
    /// The filled advice subject is released after the event is pushed with a null subject argument.
    #[port(address = 0x0069_CA80, owner = "building", name = "cSC4BuildingDevelopmentSimulator::DoBuildingChanged")]
    pub unsafe extern "thiscall" fn fc4p_0069ca80(this: *mut u8, lot: *mut u8, wealth: u32, built: u32) -> u64 {
        // SAFETY: the simulator, lot, raw grid rows, catalogue entries and service interfaces are game-owned.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let mut kind = vcall!(lot, 0xB8 => fn(u32) -> u32, 0);
            let p = Profile::from_ptr(profile(profiles(), &mut kind));
            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
            vcall!(lot, 0x38 => fn(*mut i32), bounds.as_mut_ptr().cast());
            let built_byte = built as u8;
            let key = if built_byte == 0 { 0 } else {
                (p.purpose() as u8).wrapping_mul(4).wrapping_add(wealth as u8).wrapping_add(1)
            };
            let data = vcall!(sim.building_map(), 0x60 => fn() -> *mut u8);
            let [x0, z0, x1, z1] = bounds.assume_init();
            let mut x = x0;
            while x <= x1 {
                let mut z = z0;
                while z <= z1 {
                    let rows: *mut u8 = read(data.cast());
                    let row: *mut u8 = read(rows.wrapping_offset(x.wrapping_mul(12) as isize).cast());
                    write(row.wrapping_offset(z as isize), key);
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            let early = (data as u32 & !255) | u32::from(built_byte);
            if built_byte == 0 || p.purpose() == 0 { return u64::from(early); }
            if p.wealth() != wealth { return u64::from(wealth); }
            let configuration = vcall!(lot, 0x24 => fn() -> *mut u8);
            let building = vcall!(lot, 0xC0 => fn() -> *mut u8);
            if configuration.is_null() || building.is_null() { return u64::from(building as u32); }
            let mut dev_type = developer_type(p.purpose(), p.wealth());
            let stage = vcall!(configuration, 0x2C => fn() -> u32) as u8;
            let stage_map = this.add(0x50);
            let old_stage = highest_stage(stage_map, &mut dev_type);
            if stage <= read(old_stage) { return u64::from(old_stage as u32); }
            write(highest_stage(stage_map, &mut dev_type), stage);
            let occupant = vcall!(building, 0x0C => fn() -> *mut u8);
            let mut position = MaybeUninit::<[f32; 3]>::uninit();
            vcall!(occupant, 0x20 => fn(*mut f32), position.as_mut_ptr().cast());
            let system = vcall!(read(city()), 0x144 => fn() -> *mut u8);
            let advisor = vcall!(system, 0x1C => fn(u32) -> *mut u8, 0x17);
            let city = read(city());
            let mut subject = core::ptr::null_mut::<u8>();
            let factory = gzcom();
            vcall!(factory, 0x0C => fn(u32, u32, *mut *mut u8) -> u32, 0x6A26_D049, 0x6A26_D050, &mut subject);
            let cached_subject = subject;
            let cached_table: *mut u8 = read(cached_subject.cast());
            let z_width = vcall_st0!(city, 0x170);
            let position_ptr = position.as_mut_ptr().cast::<f32>();
            let z = (F80::from_f32(read(position_ptr.add(2))) / z_width).to_f32();
            let x_width = vcall_st0!(city, 0x16C);
            let x = (F80::from_f32(read(position_ptr)) / x_width).to_f32();
            let words = [x.to_bits(), z.to_bits()];
            let cached_location = read(cached_table.add(0x0C).cast::<usize>());
            let mut location = RawCall::new(Conv::Thiscall, cached_location, cached_subject as u32, &words);
            call_raw(&mut location);
            vcall!(subject, 0x1C => fn(*mut u8) -> u32, lot);
            let data_word = (u32::from(stage) << 24) | dev_type;
            vcall!(subject, 0x28 => fn(u32) -> u32, data_word);
            let args = [0x4A64_A9CC, 0];
            let mut push = RawCall::new(Conv::Thiscall, vslot(advisor, 0x48), advisor as u32, &args);
            push.eax = data_word; push.edx = read(advisor.cast()); call_raw(&mut push);
            if subject.is_null() { return (u64::from(push.edx) << 32) | u64::from(push.eax); }
            let mut release = RawCall::new(Conv::Thiscall, vslot(subject, 8), subject as u32, &[]);
            release.eax = read(subject.cast()); release.edx = push.edx; call_raw(&mut release);
            (u64::from(release.edx) << 32) | u64::from(release.eax)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0069ca80::PORT];

#[cfg(test)]
mod tests;
