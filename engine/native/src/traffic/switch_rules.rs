// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Transit-switch property bytes, rotated rule records, and mode flags.

use super::*;

layout! {
    /// A switch owns an occupant reference, flags, ten-byte rule records and a destination mask.
    pub struct TransitSwitch {
        0x00 => occupant / set_occupant: *mut u8,
        0x10 => flags / set_flags: u8,
        0x1C => begin / set_begin: *mut u8,
        0x20 => end / set_end: *mut u8,
        0x24 => capacity / set_capacity: *mut u8,
        0x28 => mask / set_mask: u16,
    }
}

game_fn! { fn property_bytes(holder: *mut u8, id: u32, out: *mut u32) = "C" 0x005F_FB40; }
game_fn! { fn reserve_rules(this: *mut u8, count: u32) = "thiscall" 0x0071_1CD0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_data! { static lots: *mut u8 = 0x00B4_3D08; }
game_data! { static rotation: [u32; 16] = 0x00AB_3280; }

unsafe fn copy_rule(source: *const u8, dest: *mut u8) {
    // SAFETY: the game's rule storage consists of two words and a final halfword.
    unsafe {
        if !dest.is_null() {
            write_unaligned(dest.cast::<u32>(), read_unaligned(source.cast::<u32>()));
            write_unaligned(dest.add(4).cast::<u32>(), read_unaligned(source.add(4).cast::<u32>()));
            write_unaligned(dest.add(8).cast::<u16>(), read_unaligned(source.add(8).cast::<u16>()));
        }
    }
}

unsafe fn append(switch: TransitSwitch, record: *const u8) {
    // SAFETY: the switch owns this vector; all allocation and release go through its game allocator.
    unsafe {
        let end = switch.end();
        if end != switch.capacity() {
            copy_rule(record, end);
            switch.set_end(switch.end().wrapping_add(10));
            return;
        }
        let size = (end as u32).wrapping_sub(switch.begin() as u32) as i32 / 10;
        let growth = if (size as u32) < 1 { 1 } else { size };
        let capacity = size.wrapping_add(growth) as u32;
        let memory = if capacity != 0 {
            allocate(capacity.wrapping_mul(10))
        } else {
            core::ptr::null_mut()
        };
        let mut src = switch.begin();
        let mut dest = memory;
        while src != end {
            copy_rule(src, dest);
            src = src.wrapping_add(10);
            dest = dest.wrapping_add(10);
        }
        copy_rule(record, dest);
        let new_end = dest.wrapping_add(10);
        let old = switch.begin();
        if !old.is_null() {
            free(old);
        }
        switch.set_begin(memory);
        switch.set_end(new_end);
        switch.set_capacity(memory.wrapping_add(capacity.wrapping_mul(10) as usize));
    }
}

port! {
    /// Appends rotated rules from property E90E25A1. Resets the destination mask before looking up the
    /// lot; an absent lot retains the flags and rules. A present lot recomputes flags from the first
    /// floor(byte_count / 4) records. The rule's padding byte retains its incoming stack value.
    #[port(address = 0x0071_5CB0, owner = "traffic", name = "cSC4TrafficSimulator::ReadTransitSwitchRules", naked)]
    pub unsafe extern "C" fn fc4p_00715cb0() {
        core::arch::naked_asm!("mov edx, esp", "movzx eax, byte ptr [esp - 0x17]", "push eax", "call {}", "ret 4", sym read_rules);
    }
}

unsafe extern "fastcall" fn read_rules(_this: *mut u8, stack: *mut u32, padding: u32) {
    // SAFETY: the entry supplies the live switch and its occupant, lot manager, and property services.
    unsafe {
        let switch = TransitSwitch::from_ptr(stack.add(1).read() as *mut u8);
        let mut raw = [0u32; 3];
        let holder = vcall!(switch.occupant(), 0x18 => fn() -> *mut u8);
        property_bytes(holder, 0xE90E_25A1, raw.as_mut_ptr());
        switch.set_mask(0);
        let mut position = core::mem::MaybeUninit::<[u32; 2]>::uninit();
        vcall!(switch.occupant(), 0x2C => fn(*mut u32), position.as_mut_ptr().cast());
        let position = position.assume_init();
        let lot = vcall!(lots().read(), 0x18 => fn(u32, u32, u32) -> *mut u8, position[0], position[1], 0);
        if !lot.is_null() {
            let count = raw[1].wrapping_sub(raw[0]) >> 2;
            reserve_rules(switch.as_ptr().add(0x1C), count);
            let facing = vcall!(lot, 0x5C => fn() -> u32);
            let mut offset = 0u32;
            if raw[1] != raw[0] {
                let rot = rotation().cast::<u32>().wrapping_add(facing.wrapping_mul(4) as usize);
                loop {
                    let data = (raw[0] as *const u8).wrapping_add(offset as usize);
                    let mut record = [0u8; 10];
                    record[0] = u8::from(data.read() == 0x82);
                    record[5] = padding as u8;
                    let sides = data.add(1).read();
                    for i in 0..4 {
                        record
                            .as_mut_ptr()
                            .wrapping_add(1 + rot.add(i).read() as usize)
                            .write(u8::from(sides & (0x80 >> i) != 0));
                    }
                    record[6] = data.add(2).read();
                    record[8] = data.add(3).read();
                    if record[0] == 0 {
                        switch.set_mask(switch.mask() | (1u32.wrapping_shl(u32::from(record[8])) as u16));
                    }
                    append(switch, record.as_ptr());
                    offset = offset.wrapping_add(4);
                    if offset >= raw[1].wrapping_sub(raw[0]) {
                        break;
                    }
                }
            }
            switch.set_flags(0);
            if count as i32 > 0 {
                let mut rule = switch.begin().add(8);
                for _ in 0..count {
                    let flag = match rule.read() {
                        2 => 1,
                        3 => 2,
                        6 => 4,
                        7 => 8,
                        8 => 16,
                        _ => 0,
                    };
                    if flag != 0 {
                        switch.set_flags(switch.flags() | flag);
                    }
                    let to = read_unaligned(rule.cast::<u16>());
                    let from = read_unaligned(rule.sub(2).cast::<u16>());
                    if (to == 7 && from == 6) || (from == 7 && to == 6) {
                        switch.set_flags(switch.flags() | 0x40);
                    }
                    rule = rule.add(10);
                }
            }
        }
        if raw[0] != 0 {
            free(raw[0] as *mut u8);
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_00715cb0::PORT];

#[cfg(test)]
mod tests;
