// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Queue resizing and restored business-unit state.

use super::queue::{QueueIterator, QueueRange};
use super::*;
use core::mem::MaybeUninit;

game_fn! { fn reserve_front(this: *mut u8, count: u32) -> u64 = "thiscall" 0x006C_6FD0; }
game_fn! { fn reserve_back(this: *mut u8, count: u32) -> u64 = "thiscall" 0x006C_7020; }
game_fn! { fn advance(this: *mut QueueIterator, count: i32) -> u64 = "thiscall" 0x006C_6010; }
game_fn! { fn fill(first: QueueIterator, last: QueueIterator, value: *const u32) -> u64 = "C" 0x006C_6A90; }
game_fn! { fn insert(this: *mut u8, position: QueueIterator, count: u32, value: *const u32) -> u64 = "thiscall" 0x006C_8080; }
game_fn! { fn insert_dispatch(this: *mut u8, position: QueueIterator, count: u32, value: *const u32) -> u64 = "thiscall" 0x006C_8C50; }
game_fn! { fn at(this: *mut u8, output: *mut QueueIterator, count: u32) -> u64 = "thiscall" 0x006C_6860; }
game_fn! { fn erase(this: *mut u8, output: *mut QueueIterator, first: QueueIterator, last: QueueIterator) -> u64 = "thiscall" 0x006C_7E50; }
game_fn! { fn resize(this: *mut u8, count: u32, value: *const u32) -> u64 = "thiscall" 0x006C_CE30; }
game_fn! { fn read_occupant(stream: *mut u8, class: u32, output: *mut u32, flag: u32) -> u64 = "C" 0x005D_9730; }
game_fn! { fn read_reference(stream: *mut u8, output: *mut u8) -> u64 = "C" 0x005D_9FA0; }
game_fn! { fn rule_value(this: *mut u8, key: *const u32) -> u64 = "thiscall" 0x0072_F850; }

layout! {
    /// Fields restored by the business-unit reader; its queue has 32-word blocks.
    pub struct BusinessUnit {
        0x38 => kind / set_kind: u32,
        0x3C => capacity / set_capacity: u32,
        0x40 => population / set_population: u32,
        0x5C => begin / set_begin: QueueIterator,
        0x6C => end / set_end: QueueIterator,
        0x84 => cursor / set_cursor: QueueIterator,
        0x94 => dirty / set_dirty: u32,
        0x9C => pending / set_pending: u32,
    }
}

unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { p.add(offset).cast::<u32>().read_unaligned() }
}

unsafe fn put(p: *mut u8, offset: usize, value: u32) {
    unsafe { p.add(offset).cast::<u32>().write_unaligned(value) }
}

unsafe fn copy_iterator(output: *mut QueueIterator, value: QueueIterator) {
    unsafe {
        let p = output.cast::<u32>();
        p.write_unaligned(value.current as u32);
        p.add(1).write_unaligned(value.first as u32);
        p.add(2).write_unaligned(value.last as u32);
        p.add(3).write_unaligned(value.node as u32);
    }
}

port! {
    /// Insert queue words, retaining the caller's iterator slots as scratch on the end branches.
    #[port(address = 0x006C_8C50, owner = "lotdeveloper", name = "InsertBuildingQueueWords", naked)]
    pub unsafe extern "C" fn fc4p_006c8c50() {
        core::arch::naked_asm!(
            "push ebx", "push ebp", "push esi", "push edi", "mov edx, esp", "call {body}",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "ret 24", body = sym insert_words,
        );
    }
}

unsafe extern "fastcall" fn insert_words(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let queue = QueueRange::from_ptr(this);
        let position = frame.add(0x14).cast::<QueueIterator>();
        let count = word(frame, 0x24);
        let value = word(frame, 0x28) as *const u32;
        let current = word(frame, 0x14);
        if current == queue.begin().current as u32 {
            let begin = queue.begin();
            let available = (begin.current as u32).wrapping_sub(begin.first as u32) as i32 >> 2;
            if count > available as u32 {
                reserve_front(this, count.wrapping_sub(available as u32));
            }
            copy_iterator(position, queue.begin());
            advance(position, count.wrapping_neg() as i32);
            let begin = position.read_unaligned();
            let returned = fill(begin, queue.begin(), value);
            put(this, 0, begin.current as u32);
            put(this, 4, begin.first as u32);
            put(this, 8, begin.last as u32);
            let node = word(frame, 0x20);
            put(this, 12, node);
            (returned & 0xFFFF_FFFF_0000_0000) | u64::from(node)
        } else if current == queue.end().current as u32 {
            let end = queue.end();
            let available = ((end.last as u32).wrapping_sub(end.current as u32) as i32 >> 2).wrapping_sub(1);
            if count > available as u32 {
                reserve_back(this, count.wrapping_sub(available as u32));
            }
            copy_iterator(position, queue.end());
            advance(position, count as i32);
            let end = position.read_unaligned();
            let returned = fill(queue.end(), end, value);
            put(this, 0x10, end.current as u32);
            put(this, 0x14, end.first as u32);
            put(this, 0x18, end.last as u32);
            let node = word(frame, 0x20);
            put(this, 0x1C, node);
            (returned & 0xFFFF_FFFF_0000_0000) | u64::from(node)
        } else {
            insert(this, position.read_unaligned(), count, value)
        }
    }
}

port! {
    /// Resize the queue by erasing its tail or filling new words at its end.
    #[port(address = 0x006C_CE30, owner = "lotdeveloper", name = "ResizeBuildingQueue")]
    pub unsafe extern "thiscall" fn fc4p_006cce30(this: *mut u8, count: u32, value: *const u32) -> u64 {
        unsafe {
            let queue = QueueRange::from_ptr(this);
            let length = queue::distance(queue.begin(), queue.end()) as u32;
            if count < length {
                let mut first = MaybeUninit::<QueueIterator>::uninit();
                let last = queue.end();
                at(this, first.as_mut_ptr(), count);
                let mut output = MaybeUninit::<QueueIterator>::uninit();
                erase(this, output.as_mut_ptr(), first.assume_init(), last)
            } else {
                insert_dispatch(this, queue.end(), count.wrapping_sub(length), value)
            }
        }
    }
}

port! {
    /// Read version-one business-unit state. The stream argument slot also holds counts and the dirty byte.
    #[port(address = 0x006C_D120, owner = "lotdeveloper", name = "ReadBusinessUnit", naked)]
    pub unsafe extern "C" fn fc4p_006cd120() {
        core::arch::naked_asm!(
            "sub esp, 0x28", "push ebx", "push esi", "push ebp", "push edi",
            "mov edx, esp", "call {body}", "pop edi", "pop ebp", "pop esi", "pop ebx",
            "add esp, 0x28", "ret 4", body = sym read_business_unit,
        );
    }
}

unsafe extern "fastcall" fn read_business_unit(this: *mut u8, frame: *mut u8) -> u32 {
    unsafe {
        let stream = word(frame, 0x3C) as *mut u8;
        let version = frame.add(0x10).cast::<u16>();
        let eax = vcall!(stream, 0x1C => fn(*mut u16) -> u32, version);
        if version.read_unaligned() != 1 {
            return eax & !0xFF;
        }
        vcall!(stream, 0x20 => fn(*mut u8) -> u32, this.add(0x38));
        vcall!(stream, 0x30 => fn(*mut u8) -> u32, this.add(0x3C));
        vcall!(stream, 0x30 => fn(*mut u8) -> u32, this.add(0x40));
        vcall!(stream, 0x24 => fn(*mut u8) -> u32, frame.add(0x14));
        put(frame, 0x3C, 0);
        resize(this.add(0x5C), word(frame, 0x14), frame.add(0x3C).cast());
        let unit = BusinessUnit::from_ptr(this);
        let mut current = unit.begin();
        let last = unit.end().current;
        while current.current != last {
            read_occupant(stream, 0x87DD_A3A9, current.current, 0);
            current.advance();
        }
        vcall!(stream, 0x24 => fn(*mut u8) -> u32, frame.add(0x3C));
        let cursor = frame.add(0x18).cast::<QueueIterator>();
        copy_iterator(cursor, unit.begin());
        advance(cursor, word(frame, 0x3C) as i32);
        unit.set_cursor(cursor.read_unaligned());
        vcall!(stream, 0x14 => fn(*mut u8) -> u32, frame.add(0x3C));
        unit.set_dirty(u32::from(frame.add(0x3C).read()));
        read_reference(stream, this.add(0xA0));
        unit.set_pending(0);
        let error = vcall!(stream, 0x48 => fn() -> u32);
        u32::from(error as u8 == 0)
    }
}

// Keys encode the four neighboring edges. Each rule specifies a texture, rotation and reflection.
const BOUNDARY_RULES: &[(u32, u32, u8, u8)] = &[
    (0x0000, 0x3000, 0, 0),
    (0x1000, 0x2000, 0, 0),
    (0x0100, 0x2000, 1, 0),
    (0x0010, 0x2000, 2, 0),
    (0x0001, 0x2000, 3, 0),
    (0x2000, 0x2600, 0, 0),
    (0x0200, 0x2600, 1, 0),
    (0x0020, 0x2600, 2, 0),
    (0x0002, 0x2600, 3, 0),
    (0x1100, 0x2100, 1, 0),
    (0x0110, 0x2100, 2, 0),
    (0x0011, 0x2100, 3, 0),
    (0x1001, 0x2100, 0, 0),
    (0x1200, 0x2400, 1, 0),
    (0x0120, 0x2400, 2, 0),
    (0x0012, 0x2400, 3, 0),
    (0x2001, 0x2400, 0, 0),
    (0x2100, 0x2400, 0, 1),
    (0x0210, 0x2400, 1, 1),
    (0x0021, 0x2400, 2, 1),
    (0x1002, 0x2400, 3, 1),
    (0x1010, 0x2500, 0, 0),
    (0x0101, 0x2500, 1, 0),
    (0x1020, 0x2900, 2, 0),
    (0x0102, 0x2900, 3, 0),
    (0x2010, 0x2900, 0, 0),
    (0x0201, 0x2900, 1, 0),
    (0x1110, 0x2200, 1, 0),
    (0x0111, 0x2200, 2, 0),
    (0x1011, 0x2200, 3, 0),
    (0x1101, 0x2200, 0, 0),
    (0x1120, 0x2700, 2, 0),
    (0x0112, 0x2700, 3, 0),
    (0x2011, 0x2700, 0, 0),
    (0x1201, 0x2700, 1, 0),
    (0x1210, 0x2800, 1, 0),
    (0x0121, 0x2800, 2, 0),
    (0x1012, 0x2800, 3, 0),
    (0x2101, 0x2800, 0, 0),
    (0x2110, 0x2700, 0, 1),
    (0x0211, 0x2700, 1, 1),
    (0x1021, 0x2700, 2, 1),
    (0x1102, 0x2700, 3, 1),
    (0x1112, 0x2300, 3, 0),
    (0x2111, 0x2300, 0, 0),
    (0x1211, 0x2300, 1, 0),
    (0x1121, 0x2300, 2, 0),
];

port! {
    /// Install the boundary texture rules, preserving both padding bytes in each rule value.
    #[port(address = 0x006C_A160, owner = "lotdeveloper", name = "cSC4LotDeveloper::InitBoundaryRules", naked)]
    pub unsafe extern "C" fn fc4p_006ca160() {
        core::arch::naked_asm!(
            "sub esp, 12", "push ebx", "push esi", "push edi", "mov edx, esp", "call {body}",
            "pop edi", "pop esi", "pop ebx", "add esp, 12", "ret", body = sym boundary_rules,
        );
    }
}

unsafe extern "fastcall" fn boundary_rules(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let mut returned = 0;
        for &(key, texture, rotation, reflection) in BOUNDARY_RULES {
            put(frame, 0x0C, key);
            frame.add(0x14).write(rotation);
            frame.add(0x15).write(reflection);
            returned = rule_value(this.add(0xFC), frame.add(0x0C).cast());
            let output = returned as u32 as *mut u8;
            put(output, 0, texture);
            put(output, 4, word(frame, 0x14));
        }
        returned
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c8c50::PORT,
    fc4p_006ca160::PORT,
    fc4p_006cce30::PORT,
];
