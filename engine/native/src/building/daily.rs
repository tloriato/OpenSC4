// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Daily growth dispatch and execution of the due decline-plan entries.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_data! { static simulator: *mut u8 = 0x00B4_3D5C; }
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(node: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn destroy_list(this: *mut *mut u8) -> u64 = "thiscall" 0x0056_50A0; }
game_fn! { fn timer_new(this: *mut u8, category: u32) -> u64 = "thiscall" 0x0088_FEDF; }
game_fn! { fn timer_start(this: *mut u8) -> u64 = "thiscall" 0x0089_05C4; }
game_fn! { #[allow(dead_code)] fn timer_end(this: *mut u8) -> u64 = "thiscall" 0x00A6_D837; }
game_fn! { #[allow(dead_code)] fn execute_growth(this: *mut u8) -> u64 = "thiscall" 0x0069_9270; }
game_fn! { #[allow(dead_code)] fn grow(this: *mut u8) -> u64 = "thiscall" 0x0069_B0B0; }
game_fn! { #[allow(dead_code)] fn execute_decline(this: *mut u8) -> u64 = "thiscall" 0x0069_6A50; }

layout! {
    /// A circular decline-plan node, with next/previous links and its scheduled growth profile.
    pub struct DeclineNode {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => day / set_day: u32,
        0x0C => x / set_x: u32,
        0x10 => z / set_z: u32,
        0x14 => developer / set_developer: *mut u8,
        0x18 => stage / set_stage: u8,
    }
}

unsafe fn direct(address: u32, conv: Conv, ecx: u32, args: &[u32], registers: u64) -> u64 {
    let mut call = RawCall::new(conv, crate::abi::target(address), ecx, args);
    call.eax = registers as u32;
    call.edx = (registers >> 32) as u32;
    // SAFETY: each caller declares the matching game function and supplies its arguments.
    unsafe { call_raw(&mut call) };
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

unsafe fn virtual_call(this: *mut u8, offset: usize, args: &[u32], registers: u64) -> u64 {
    // SAFETY: each caller supplies the original interface and slot signature.
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(this, offset), this as u32, args);
        call.eax = registers as u32;
        call.edx = (registers >> 32) as u32;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

unsafe fn unlink(node: DeclineNode) -> (*mut u8, *mut u8) {
    let next = node.next();
    let previous = node.previous();
    // SAFETY: both neighbours belong to this circular game list.
    unsafe {
        DeclineNode::from_ptr(previous).set_next(next);
        DeclineNode::from_ptr(next).set_previous(previous);
    }
    (next, previous)
}

port! {
    /// Removes recovered or unsuccessful due entries and stops after the first successful decline.
    /// Future days compare as unsigned words; demand is truncated through the game's helper.
    #[port(address = 0x0069_6A50, owner = "building", name = "cSC4BuildingDevelopmentSimulator::ExecuteDeclinePlan")]
    pub unsafe extern "fastcall" fn fc4p_00696a50(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the simulator, developers, demand nodes and list links are game-owned objects.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let clock = read(simulator());
            let clock_table = read(clock.cast::<u32>());
            let mut r = virtual_call(clock, 0x1C, &[], (u64::from(entry_edx) << 32) | u64::from(clock_table));
            let date = r as u32 as *mut u8;
            let date_table = read(date.cast::<u32>());
            r = virtual_call(date, 0x68, &[], (u64::from(date_table) << 32) | u64::from(date as u32));
            let today = r as u32;
            let header = sim.decline_plan();
            let mut current = DeclineNode::from_ptr(header).next();
            r = (r & !0xFFFF_FFFF) | u64::from(header as u32);
            while current != sim.decline_plan() {
                let node = DeclineNode::from_ptr(current);
                r = (r & !0xFFFF_FFFF) | u64::from(today);
                if today < node.day() {
                    current = node.next();
                    continue;
                }
                let developer = node.developer();
                r = virtual_call(developer, 0x48, &[],
                    (u64::from(read(developer.cast::<u32>())) << 32) | u64::from(today));
                let demand = r as u32 as *mut u8;
                let mut call = RawCall::new(Conv::Thiscall, vslot(demand, 0x30), demand as u32, &[]);
                call.eax = demand as u32;
                call.edx = read(demand.cast());
                call.take_st0 = 1;
                call_raw(&mut call);
                r = call.st0.ftol() as u64;
                if r as u32 as i32 > -20 {
                    let (next, previous) = unlink(node);
                    r = direct(0x0090_CF63, Conv::Cdecl, 0, &[current as u32],
                        (r & !0xFFFF_FFFF) | u64::from(previous as u32));
                    current = next;
                    continue;
                }
                let mut profile = MaybeUninit::<[u8; 16]>::uninit();
                let profile_ptr = profile.as_mut_ptr().cast::<u8>();
                write(profile_ptr.cast::<u32>(), node.developer() as u32);
                write(profile_ptr.add(4).cast::<u32>(), node.x());
                write(profile_ptr.add(8).cast::<u32>(), node.z());
                *profile_ptr.add(12) = node.stage();
                r = direct(0x0090_CF54, Conv::Cdecl, node.x(), &[12],
                    (u64::from(node.z()) << 32) | u64::from((node.developer() as u32 & !255) | u32::from(node.stage())));
                let mut list = r as u32 as *mut u8;
                write(list.cast::<*mut u8>(), list);
                write(list.add(4).cast::<*mut u8>(), list);
                let tract = sim.tract_developer();
                r = virtual_call(tract, 0x18, &[profile_ptr as u32],
                    (u64::from(read(tract.cast::<u32>())) << 32) | u64::from(profile_ptr as u32));
                let succeeded = r as u32 as i32 > 0;
                let (next, previous) = unlink(node);
                let free_edx = r >> 32;
                r = direct(0x0090_CF63, Conv::Cdecl, if succeeded { previous as u32 } else { 0 },
                    &[current as u32], (free_edx << 32) | u64::from(if succeeded { next as u32 } else { previous as u32 }));
                if succeeded { return direct(0x0056_50A0, Conv::Thiscall, &mut list as *mut _ as u32, &[], r); }
                current = next;
                let mut item = read(list.cast::<*mut u8>());
                while item != list {
                    let old = item;
                    item = read(item.cast::<*mut u8>());
                    r = direct(0x0090_CF63, Conv::Cdecl, 0, &[old as u32],
                        (r & !0xFFFF_FFFF) | u64::from(old as u32));
                }
                write(list.cast::<*mut u8>(), list);
                write(list.add(4).cast::<*mut u8>(), list);
                r = direct(0x0090_CF63, Conv::Cdecl, 0, &[list as u32], r);
            }
            r
        }
    }
}

port! {
    /// Profiles a daily growth attempt (planned or immediate), then executes due decline entries.
    #[port(address = 0x0069_B6D0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::DoDailySimulation")]
    pub unsafe extern "thiscall" fn fc4p_0069b6d0(this: *mut u8, _unused: u32) -> u64 {
        // SAFETY: the timer owns 24 local bytes and all calls retain the original simulator interfaces.
        unsafe {
            let sim = BuildingSim::from_ptr(this);
            let mut timer = MaybeUninit::<[u8; 24]>::uninit();
            let timer = timer.as_mut_ptr().cast::<u8>();
            timer_new(timer, 4);
            let mut r = timer_start(timer);
            r = (r & !255) | u64::from(sim.use_growth_plan());
            r = direct(if sim.use_growth_plan() == 0 { 0x0069_B0B0 } else { 0x0069_9270 },
                Conv::Thiscall, this as u32, &[], r);
            r = direct(0x0069_6A50, Conv::Thiscall, this as u32, &[], r);
            direct(0x00A6_D837, Conv::Thiscall, timer as u32, &[], r)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00696a50::PORT, fc4p_0069b6d0::PORT];

#[cfg(test)]
mod tests;
