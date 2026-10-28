// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Terrain-change handling: update lot elements, dezone, or queue demolition and charge its cost.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn construct_list(this: *mut *mut u8, cookie: *mut u8) -> u64 = "thiscall" 0x004F_5A00; }
game_fn! { fn clear_list(this: *mut *mut u8) -> u64 = "thiscall" 0x0047_3EC0; }
game_fn! { fn construct_region(this: *mut u8, rect: *mut i32, convert_cells: u32) -> u64 = "thiscall" 0x0044_2640; }
game_fn! { fn destroy_region_base(this: *mut u8) -> u64 = "thiscall" 0x0044_1A60; }
game_data! { static demolition: *mut u8 = 0x00B4_3D00; }
game_data! { static budget: *mut u8 = 0x00B4_3D64; }
game_data! { static lower_delta: f32 = 0x00A8_FE78; }
game_data! { static upper_delta: f32 = 0x00A8_825C; }
game_data! { static one: f32 = 0x00A8_1228; }
game_data! { static max_slope_change: f32 = 0x00AB_0ABC; }

unsafe fn dispose_list(list: *mut *mut u8) -> u64 {
    // SAFETY: the list owns its header; the game clear helper owns all element releases and frees.
    unsafe {
        let r = clear_list(list);
        let header = read(list);
        if header.is_null() {
            return r & !0xFFFF_FFFF;
        }
        let args = [header as u32];
        let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
        call.eax = header as u32;
        call.edx = (r >> 32) as u32;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

unsafe fn new_list() -> *mut u8 {
    // SAFETY: the game's allocator supplies a twelve-byte header.
    unsafe {
        let p = allocate(12);
        write(p.cast(), p);
        write(p.add(4).cast(), p);
        p
    }
}

unsafe fn get_lots(sim: BuildingSim, list: *mut *mut u8, rect: *mut i32) {
    // SAFETY: the rectangle and circular list are original game objects.
    unsafe {
        vcall!(sim.lots(), 0x1C => fn(*mut *mut u8, i32, i32, i32, i32, u32) -> u32,
        list, read(rect), read(rect.add(1)), read(rect.add(2)), read(rect.add(3)), 0);
    }
}

unsafe fn destroy_region(region: *mut u8) -> u64 {
    // SAFETY: the constructor initialized this region's embedded base at +16.
    unsafe {
        write(region.add(16).cast::<u32>(), 0x00A8_6364);
        destroy_region_base(region.add(16))
    }
}

port! {
    /// Updates lots for ordinary changes; checks altitude and slope for terraforming modes 1, 2 and 3.
    /// The entry retains the upper bytes of the original local flag word passed to PlaceZone.
    #[port(address = 0x0069_8610, owner = "building", name = "cSC4BuildingDevelopmentSimulator::DoTerrainChanged", naked)]
    pub unsafe extern "C" fn fc4p_00698610() {
        core::arch::naked_asm!(
            "push dword ptr [esp - 136]", "push dword ptr [esp + 12]", "push dword ptr [esp + 12]",
            "call {body}", "ret 8", body = sym terrain_changed,
        );
    }
}

/// # Safety
/// The entry supplies a live simulator and rectangle, and the incoming local flag word's retained bytes.
unsafe extern "fastcall" fn terrain_changed(
    this: *mut u8,
    entry_edx: u32,
    rect: *mut i32,
    kind: u32,
    flag_word: u32,
) -> u64 {
    // SAFETY: all interfaces, allocated list nodes, output rectangles and temporary regions follow game layouts.
    unsafe {
        let z0 = read(rect.add(1));
        if z0 > read(rect.add(3)) || read(rect) > read(rect.add(2)) {
            return (u64::from(entry_edx) << 32) | u64::from(z0 as u32);
        }
        let sim = BuildingSim::from_ptr(this);
        if !(1..=3).contains(&kind) {
            let mut list = MaybeUninit::<*mut u8>::uninit();
            let mut cookie = MaybeUninit::<u8>::uninit();
            construct_list(list.as_mut_ptr(), cookie.as_mut_ptr());
            get_lots(sim, list.as_mut_ptr(), rect);
            let mut current = read(list.assume_init().cast::<*mut u8>());
            while current != list.assume_init() {
                vcall!(sim.lot_developer(), 0x94 => fn(*mut u8, u32) -> u32, read(current.add(8).cast()), kind);
                current = read(current.cast());
            }
            return dispose_list(list.as_mut_ptr());
        }
        let charge = kind == 1 || kind == 2;
        let flag_word = (flag_word & !255) | u32::from(charge);
        let mut queued = new_list();
        let mut lots = new_list();
        get_lots(sim, &mut lots, rect);
        let mut region = MaybeUninit::<[u8; 36]>::uninit();
        construct_region(region.as_mut_ptr().cast(), rect, 0);
        let mut current = read(lots.cast::<*mut u8>());
        while current != lots {
            let lot: *mut u8 = read(current.add(8).cast());
            let mut center = MaybeUninit::<[f32; 3]>::uninit();
            vcall!(lot, 0x3C => fn(*mut f32), center.as_mut_ptr().cast());
            let center = center.assume_init();
            let delta = (vcall_st0!(sim.terrain(), 0x34, center[0], center[2]) - F80::from_f32(center[1])).to_f32();
            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
            vcall!(lot, 0x38 => fn(*mut i32), bounds.as_mut_ptr().cast());
            let low = F80::from_f32(delta).compare(F80::from_f32(read(lower_delta())));
            let mut too_large = low.less();
            if !too_large {
                let high = F80::from_f32(delta).compare(F80::from_f32(read(upper_delta())));
                too_large = !high.less() && !high.equal() && !high.unordered();
            }
            if !too_large {
                let old_slope = vcall_st0!(lot, 0x4C).to_f32();
                let new_slope = vcall_st0!(sim.lot_developer(), 0x78, lot, 0u32).to_f32();
                let width = vcall_st0!(sim.city(), 0x16C);
                let scale = F80::from_f32(read(one())) / width;
                let angle_new = (F80::from_f32(new_slope) * scale).atan2(F80::from_i32(1));
                let angle_old = (scale * F80::from_f32(old_slope)).atan2(F80::from_i32(1));
                let difference = (angle_new - angle_old).compare(F80::from_f32(read(max_slope_change())));
                too_large = !difference.less() && !difference.equal() && !difference.unordered();
            }
            if !too_large {
                vcall!(sim.lot_developer(), 0x94 => fn(*mut u8, u32) -> u32, lot, kind);
            } else if vcall!(lot, 0x9C => fn() -> u32) as u8 != 0 {
                vcall!(lot, 4 => fn() -> u32);
                let node = allocate(12);
                if !node.wrapping_add(8).is_null() {
                    write(node.add(8).cast::<*mut u8>(), lot);
                    vcall!(lot, 4 => fn() -> u32);
                }
                let previous: *mut u8 = read(queued.add(4).cast());
                write(node.cast::<*mut u8>(), queued);
                write(node.add(4).cast::<*mut u8>(), previous);
                write(previous.cast::<*mut u8>(), node);
                write(queued.add(4).cast::<*mut u8>(), node);
                vcall!(lot, 8 => fn() -> u32);
            } else {
                let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
                vcall!(lot, 0x38 => fn(*mut i32), bounds.as_mut_ptr().cast());
                let mut local_region = MaybeUninit::<[u8; 36]>::uninit();
                construct_region(local_region.as_mut_ptr().cast(), bounds.as_mut_ptr().cast(), 1);
                vcall!(sim.zones(), 0x2C => fn(*mut u8, u32, u32, u32, u32, u32, u32, u32, u32, u32) -> u32,
                    local_region.as_mut_ptr().cast(), 0, 1, flag_word, 0, 1, 1, 0, 0, 0);
                destroy_region(local_region.as_mut_ptr().cast());
            }
            current = read(current.cast());
        }
        let mut cost = 0i64;
        vcall!(read(demolition()), 0x1C => fn(u32, *mut *mut u8, u32, u32, u32, u32, *mut i64, u32) -> u32,
            1, &mut queued, 0, 1, 1, 0, &mut cost, 0);
        if charge && cost > 0 {
            let treasury = read(budget());
            let funds = vcall!(treasury, 0x18 => fn() -> u64);
            let remaining = funds.wrapping_sub(cost as u64);
            vcall!(treasury, 0x14 => fn(u32, u32) -> u32, remaining as u32, (remaining >> 32) as u32);
        }
        destroy_region(region.as_mut_ptr().cast());
        dispose_list(&mut lots);
        dispose_list(&mut queued)
    }
}

pub const PORTS: &[Port] = &[fc4p_00698610::PORT];

#[cfg(test)]
mod tests;
