// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lot dimensions chosen from developer demand, building capacities and configuration order.

use super::{region::Region, Port};
use crate::{
    abi::{call_raw, Conv, RawCall},
    x87::F80,
};
use core::{
    mem::MaybeUninit,
    ptr::{null_mut, read_unaligned, write_unaligned},
};

layout! {
    /// Services, the current zone and the five dimensions written by lot-size selection.
    struct Settings {
        0x20 => growth / set_growth: *mut u8,
        0x84 => width / set_width: u32,
        0x88 => depth / set_depth: u32,
        0x8C => minimum / set_minimum: u32,
        0x90 => maximum / set_maximum: u32,
        0x94 => block / set_block: u32,
        0xA0 => zone / set_zone: u32,
    }
}

layout! {
    /// Hash buckets are pointer words; chained nodes contain a next pointer and a value.
    struct Table {
        0x04 => buckets / set_buckets: *mut *mut u8,
        0x08 => end / set_end: *mut *mut u8,
        0x0C => limit / set_limit: *mut *mut u8,
        0x10 => count / set_count: u32,
    }
}

game_fn! { fn density(zone: u32) -> u32 = "C" 0x0045_8960; }
game_fn! { fn initialize_table(this: *mut u8, size: u32) = "thiscall" 0x0082_9BC0; }
game_fn! { fn initialize_filter(this: *mut u8) = "thiscall" 0x0048_9A50; }
game_fn! { fn destroy_filter(this: *mut u8) = "thiscall" 0x0048_9660; }
game_fn! { fn begin_table(this: *mut u8, iterator: *mut u32) = "thiscall" 0x004B_0390; }
game_fn! { fn clear_table(this: *mut u8) -> u64 = "thiscall" 0x008E_BA60; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_data! { static configurations: *mut u8 = 0x00B4_3D18; }
game_data! { static unsigned_float: f32 = 0x00A8_0AA8; }
game_data! { static unsigned_double: f64 = 0x00A8_0AB8; }
game_data! { static medium_multiplier: f64 = 0x00A9_56F8; }

fn midpoint(a: i32, b: i32) -> i32 {
    a.wrapping_add(b) / 2
}

// Keep the unsigned remainder operation, including its zero-divisor failure.
#[allow(clippy::manual_is_multiple_of)]
fn divides(width: u32, divisor: u32) -> bool {
    width % divisor == 0
}

fn adjust_range(settings: Settings) {
    if settings.maximum().wrapping_sub(settings.minimum()) < 2 {
        settings.set_minimum(if settings.minimum() > 2 {
            settings.maximum().wrapping_sub(2)
        } else {
            1
        });
        settings.set_maximum(settings.minimum().wrapping_add(2));
    }
}

unsafe fn free_with_registers(memory: *mut u8, eax: u32, edx: u32) -> u64 {
    let args = [memory as u32];
    let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

unsafe fn new_table(memory: *mut u8) -> Table {
    unsafe {
        let table = Table::from_ptr(memory);
        table.set_buckets(null_mut());
        table.set_end(null_mut());
        table.set_limit(null_mut());
        table.set_count(0);
        initialize_table(memory, 100);
        table
    }
}

unsafe fn next_node(table: Table, node: *mut u8, pointer_keys: bool) -> *mut u8 {
    unsafe {
        let next = read_unaligned(node.cast::<*mut u8>());
        if !next.is_null() {
            return next;
        }
        let bucket_count = ((table.end() as u32).wrapping_sub(table.buckets() as u32) as i32 >> 2) as u32;
        let value = read_unaligned(node.add(4).cast::<u32>());
        let mut bucket = (if pointer_keys { value >> 2 } else { value }) % bucket_count;
        let mut result = null_mut();
        loop {
            bucket = bucket.wrapping_add(1);
            let count = ((table.end() as u32).wrapping_sub(table.buckets() as u32) as i32 >> 2) as u32;
            if bucket >= count {
                break;
            }
            result = read_unaligned(table.buckets().wrapping_add(bucket as usize));
            if !result.is_null() {
                break;
            }
        }
        result
    }
}

unsafe fn first_node(table: Table) -> *mut u8 {
    unsafe {
        let count = ((table.end() as u32).wrapping_sub(table.buckets() as u32) as i32 >> 2) as u32;
        let mut bucket = 0u32;
        while bucket < count {
            let node = read_unaligned(table.buckets().wrapping_add(bucket as usize));
            if !node.is_null() {
                return node;
            }
            bucket = bucket.wrapping_add(1);
        }
        null_mut()
    }
}

/// Delete nodes through the original helper, then free its separately owned bucket storage.
unsafe fn delete_table(table: Table) -> u64 {
    unsafe {
        let result = clear_table(table.as_ptr());
        let buckets = table.buckets();
        if buckets.is_null() {
            result & 0xFFFF_FFFF_0000_0000
        } else {
            free_with_registers(buckets.cast(), buckets as u32, (result >> 32) as u32)
        }
    }
}

unsafe fn estimate(settings: Settings, id: u32, x: i32, z: i32) -> (*mut u8, i32, i32) {
    unsafe {
        let developer = vcall!(settings.growth(), 0x80 => fn(u32) -> *mut u8, id);
        let desirability = vcall!(developer, 0x74 => fn(i32, i32) -> u32, x, z) as i16 as i32;
        let source = vcall!(developer, 0x48 => fn() -> *mut u8);
        let demand = vcall_st0!(source, 0x30).ftol() as i32;
        (developer, desirability, demand)
    }
}

unsafe fn stage(developer: *mut u8, desirability: i32) -> u32 {
    unsafe {
        let mut stage = 1;
        if vcall!(developer, 0x28 => fn() -> u32) as u8 == 0 {
            return stage;
        }
        let vector = vcall!(developer, 0x2C => fn() -> *mut u8);
        let mut item = read_unaligned(vector.cast::<*mut u8>());
        while item != read_unaligned(vector.add(4).cast::<*mut u8>()) {
            let grid = vcall!(developer, 0x34 => fn() -> *mut u8);
            let a = read_unaligned(item.cast::<u32>());
            let b = read_unaligned(item.add(4).cast::<u32>());
            let value = vcall!(grid, 0x58 => fn(u32, u32) -> u32, a, b) as i16 as i32;
            if desirability >= value {
                break;
            }
            stage = read_unaligned(item.add(8).cast());
            item = item.wrapping_add(12);
        }
        stage
    }
}

/// The area is an unsigned product, converted by a signed load and a conditional 2^32 addition.
unsafe fn capacity(tract: i32, area: u32, average: i32) -> i32 {
    unsafe {
        let numerator = F80::from_i32(tract);
        let mut denominator = F80::from_i32(area as i32);
        if (area as i32) < 0 {
            denominator = denominator + F80::from_f32(*unsigned_float());
        }
        ((numerator / denominator) * F80::from_i32(average)).ftol() as i32
    }
}

port! {
    /// Choose the highest-desirability developer, breaking ties by truncated source demand. Use the
    /// configuration manager's pointer-keyed table order to select the nearest capacity; strict score
    /// comparisons keep the first match. Release every configuration while deriving the depth range.
    /// All integer arithmetic wraps, thresholds use their original signedness, and capacities use x87.
    #[port(address = 0x0073_2BF0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::DetermineLotSize")]
    pub unsafe extern "thiscall" fn fc4p_00732bf0(this: *mut u8, input: *mut u8) -> u64 {
        unsafe {
            let settings = Settings::from_ptr(this);
            let density = density(settings.zone());
            let region = Region::from_ptr(input);
            let (x, z) = (midpoint(region.x1(), region.x2()), midpoint(region.z1(), region.z2()));
            let zone = settings.zone();
            let mut best: *mut u8 = null_mut();
            let mut desirability = 0;
            let mut demand = 0;
            let shallow = zone == 5 || zone == 6;
            let candidates: &[u32] = match zone {
                1..=3 => &[0x1010, 0x1020, 0x1030],
                4..=6 => &[0x3110, 0x3120, 0x3130, 0x3320, 0x3330],
                _ => &[],
            };
            for &id in candidates {
                let (developer, value, source_demand) = estimate(settings, id, x, z);
                if best.is_null() || value > desirability || (value == desirability && source_demand > demand) {
                    best = developer; desirability = value; demand = source_demand;
                }
            }
            let stage = if best.is_null() { 1 } else { stage(best, desirability) };
            let manager = read_unaligned(configurations());
            let mut map_memory = MaybeUninit::<[u32; 5]>::uninit();
            let map = new_table(map_memory.as_mut_ptr().cast());
            if !best.is_null() {
                let wealth = vcall!(best, 0x20 => fn() -> u32);
                let purpose = vcall!(best, 0x1C => fn() -> u32);
                let mut filter = MaybeUninit::<[u32; 5]>::uninit();
                let filter = filter.as_mut_ptr().cast::<u8>();
                initialize_filter(filter);
                write_unaligned(filter.cast::<u32>(), 0x00AB_36FC);
                write_unaligned(filter.add(8).cast(), purpose);
                write_unaligned(filter.add(12).cast(), wealth);
                write_unaligned(filter.add(16).cast(), stage);
                vcall!(manager, 0x24 => fn(*mut u8, *mut u8), map.as_ptr(), filter);
                destroy_filter(filter);
            }
            let mut chosen = null_mut();
            let mut best_score = i32::MAX;
            let mut iterator = MaybeUninit::<[u32; 2]>::uninit();
            begin_table(map.as_ptr(), iterator.as_mut_ptr().cast());
            let mut node = read_unaligned(iterator.as_ptr().cast::<*mut u8>());
            while !node.is_null() {
                let configuration = read_unaligned(node.add(4).cast::<*mut u8>());
                let property = vcall!(configuration, 0x60 => fn() -> *mut u8);
                if !property.is_null() {
                    let (mut width, mut depth) = (MaybeUninit::<u32>::uninit(), MaybeUninit::<u32>::uninit());
                    vcall!(configuration, 0x1C => fn(*mut u32, *mut u32), width.as_mut_ptr(), depth.as_mut_ptr());
                    let (width, depth) = (width.assume_init(), depth.assume_init());
                    if (2..=3).contains(&depth) && width <= 3 && (!shallow || depth <= 2) {
                        let mut id = read_unaligned(property.add(0x28).cast::<*mut u32>());
                        let mut minimum = i32::MAX;
                        let mut maximum = i32::MIN;
                        while id != read_unaligned(property.add(0x2C).cast::<*mut u32>()) {
                            let family = read_unaligned(id);
                            let mut family_memory = MaybeUninit::<[u32; 5]>::uninit();
                            let family_table = new_table(family_memory.as_mut_ptr().cast());
                            vcall!(settings.growth(), 0x4C => fn(*mut u8, u32), family_table.as_ptr(), family);
                            let mut iterator = MaybeUninit::<[u32; 2]>::uninit();
                            begin_table(family_table.as_ptr(), iterator.as_mut_ptr().cast());
                            let mut building = read_unaligned(iterator.as_ptr().cast::<*mut u8>());
                            while !building.is_null() {
                                let id = read_unaligned(building.add(4).cast::<u32>());
                                let profile = vcall!(settings.growth(), 0x9C => fn(u32) -> *mut u8, id);
                                let purpose = vcall!(best, 0x1C => fn() -> u32);
                                if read_unaligned(profile.cast::<u32>()) == purpose {
                                    let wealth = vcall!(best, 0x20 => fn() -> u32);
                                    if read_unaligned(profile.add(4).cast::<u32>()) == wealth {
                                        let value = i32::from(read_unaligned(profile.add(8).cast::<u16>()));
                                        if value < minimum { minimum = value; }
                                        if value > maximum { maximum = value; }
                                    }
                                }
                                building = next_node(family_table, building, false);
                            }
                            delete_table(family_table);
                            id = id.wrapping_add(1);
                        }
                        if minimum != i32::MAX {
                            let grid = vcall!(best, 0x34 => fn() -> *mut u8);
                            let shift = vcall!(grid, 0x2C => fn() -> u32);
                            let tract = 1i32.wrapping_shl(shift).wrapping_shl(shift);
                            let value = capacity(tract, width.wrapping_mul(depth), midpoint(minimum, maximum));
                            let score = value.wrapping_sub(demand).wrapping_abs();
                            if score < best_score { best_score = score; chosen = configuration; }
                        }
                    }
                }
                node = next_node(map, node, true);
            }
            if chosen.is_null() {
                settings.set_width(2); settings.set_depth(2);
            } else {
                vcall!(chosen, 0x1C => fn(*mut u32, *mut u32), this.add(0x84).cast(), this.add(0x88).cast());
            }
            settings.set_minimum(settings.depth()); settings.set_maximum(settings.depth());
            let mut node = first_node(map);
            while !node.is_null() {
                let configuration = read_unaligned(node.add(4).cast::<*mut u8>());
                let (mut width, mut depth) = (MaybeUninit::<u32>::uninit(), MaybeUninit::<u32>::uninit());
                vcall!(configuration, 0x1C => fn(*mut u32, *mut u32), width.as_mut_ptr(), depth.as_mut_ptr());
                let (width, depth) = (width.assume_init(), depth.assume_init());
                if divides(settings.width(), width) {
                    if depth < settings.minimum() { settings.set_minimum(depth); }
                    if depth > settings.maximum() { settings.set_maximum(depth); }
                }
                vcall!(configuration, 8 => fn());
                node = next_node(map, node, true);
            }
            adjust_range(settings);
            let residential = !best.is_null() && vcall!(best, 0x1C => fn() -> u32) == 1;
            match density {
                1 => settings.set_block(if residential { settings.depth().wrapping_mul(3) } else { 7 }),
                2 if residential => {
                    let depth = settings.depth();
                    let mut value = F80::from_i32(depth as i32);
                    if (depth as i32) < 0 { value = value + F80::from_f64(*unsigned_double()); }
                    settings.set_block((value * F80::from_f64(*medium_multiplier())).ftol() as u32);
                }
                2 => settings.set_block(6),
                3 => settings.set_block(if residential { settings.depth().wrapping_mul(2) } else { 5 }),
                _ => {},
            }
            // The original exposes EDX from the final table helper/free, including the null-bucket path.
            let mut clear = RawCall::new(Conv::Thiscall, crate::abi::target(0x008E_BA60), map.as_ptr() as u32, &[]);
            call_raw(&mut clear);
            let buckets = map.buckets();
            if buckets.is_null() { u64::from(clear.edx) << 32 } else { free_with_registers(buckets.cast(), buckets as u32, clear.edx) }
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00732bf0::PORT];

#[cfg(test)]
mod tests;
