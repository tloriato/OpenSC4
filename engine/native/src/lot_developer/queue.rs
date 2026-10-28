// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Search and copying for the building queue's 32-word blocks.

use super::*;

/// A queue iterator is passed as four stack words. Its node points into the block map.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueIterator {
    pub current: *mut u32,
    pub first: *mut u32,
    pub last: *mut u32,
    pub node: *mut *mut u32,
}

impl QueueIterator {
    pub(super) unsafe fn advance(&mut self) {
        unsafe {
            self.current = self.current.wrapping_add(1);
            if self.current == self.last {
                self.first = self.node.wrapping_add(1).read_unaligned();
                self.node = self.node.wrapping_add(1);
                self.last = self.first.wrapping_add(32);
                self.current = self.first;
            }
        }
    }

    unsafe fn retreat(&mut self) {
        unsafe {
            if self.current == self.first {
                self.first = self.node.wrapping_sub(1).read_unaligned();
                self.node = self.node.wrapping_sub(1);
                self.last = self.first.wrapping_add(32);
                self.current = self.last;
            }
            self.current = self.current.wrapping_sub(1);
        }
    }
}

fn words_between(a: usize, b: usize) -> i32 {
    (a as i32).wrapping_sub(b as i32) >> 2
}

pub(super) fn distance(first: QueueIterator, last: QueueIterator) -> i32 {
    words_between(last.node as usize, first.node as usize)
        .wrapping_shl(5)
        .wrapping_add(words_between(last.current as usize, last.first as usize))
        .wrapping_add(words_between(first.last as usize, first.current as usize))
        .wrapping_sub(32)
}

unsafe fn write_iterator(output: *mut QueueIterator, value: QueueIterator) -> u64 {
    unsafe {
        // Keep the four stores distinct, including when output overlaps the queue map.
        let words = output.cast::<u32>();
        words.write_unaligned(value.current as u32);
        words.add(1).write_unaligned(value.first as u32);
        words.add(2).write_unaligned(value.last as u32);
        words.add(3).write_unaligned(value.node as u32);
    }
    (u64::from(value.node as u32) << 32) | u64::from(output as u32)
}

game_fn! { fn find_words(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, value: *const u32, tag: *const u32) -> u64 = "C" 0x006C_63C0; }
game_fn! { fn copy_backward(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: *const u32, unused: u32) -> u64 = "C" 0x006C_6570; }
game_fn! { fn copy_forward(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: *const u32, unused: u32) -> u64 = "C" 0x006C_6630; }
game_fn! { fn copy_backward_dispatch(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: *const u32) -> u64 = "C" 0x006C_6BA0; }
game_fn! { fn copy_forward_dispatch(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: *const u32) -> u64 = "C" 0x006C_6C20; }
game_fn! { fn queue_at(this: *const QueueIterator, output: *mut QueueIterator, offset: i32) -> u64 = "thiscall" 0x006C_6860; }
game_fn! { fn advance_queue(this: *mut QueueIterator, offset: i32) -> u64 = "thiscall" 0x006C_6010; }
game_fn! { fn crt_random() -> i32 = "C" 0x009E_FB9F; }
game_fn! { fn copy_range_backward(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator) -> u64 = "C" 0x006C_6ED0; }
game_fn! { fn copy_range_forward(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator) -> u64 = "C" 0x006C_6F50; }
game_fn! { fn free_block(block: *mut u32) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn find_queue(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, value: *const u32) -> u64 = "C" 0x006C_69D0; }
game_fn! { fn erase_queue(this: *mut u8, output: *mut QueueIterator, position: QueueIterator) -> u64 = "thiscall" 0x006C_7C50; }

layout! {
    /// The queue's two live iterators; block-map ownership follows them in the containing object.
    pub struct QueueRange {
        0x00 => begin / set_begin: QueueIterator,
        0x10 => end / set_end: QueueIterator,
    }
}

port! {
    /// Finds a word in a queue range, testing four words per group and then the remaining zero to three.
    #[port(address = 0x006C_63C0, owner = "lotdeveloper", name = "FindBuildingQueueWord")]
    pub unsafe extern "C" fn fc4p_006c63c0(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, value: *const u32, _tag: *const u32) -> u64 {
        let mut first = first;
        unsafe {
            let groups = distance(first, last) >> 2;
            if groups > 0 {
                let key = value.read_unaligned();
                for _ in 0..groups {
                    for _ in 0..4 {
                        if first.current.read_unaligned() == key {
                            return write_iterator(output, first);
                        }
                        first.advance();
                    }
                }
            }
            let tail = distance(first, last);
            if (1..=3).contains(&tail) {
                for i in 0..tail {
                    if first.current.read_unaligned() == value.read_unaligned() {
                        return write_iterator(output, first);
                    }
                    if i != tail - 1 { first.advance(); }
                }
            }
            write_iterator(output, last)
        }
    }
}

port! {
    /// Copies a queue range from its back, allowing the destination to overlap to the right.
    #[port(address = 0x006C_6570, owner = "lotdeveloper", name = "CopyBuildingQueueBackward")]
    pub unsafe extern "C" fn fc4p_006c6570(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, _tag: *const u32, _unused: u32) -> u64 {
        let mut last = last;
        let mut destination = destination;
        unsafe {
            let count = distance(first, last);
            for _ in 0..count {
                last.retreat();
                destination.retreat();
                destination.current.write_unaligned(last.current.read_unaligned());
            }
            // EDX retains the destination block's beginning, rather than its map node.
            let returned = write_iterator(output, destination);
            (u64::from(destination.first as u32) << 32) | (returned & 0xFFFF_FFFF)
        }
    }
}

port! {
    /// Copies a queue range from its front, allowing the destination to overlap to the left.
    #[port(address = 0x006C_6630, owner = "lotdeveloper", name = "CopyBuildingQueueForward")]
    pub unsafe extern "C" fn fc4p_006c6630(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, _tag: *const u32, _unused: u32) -> u64 {
        let mut first = first;
        let mut destination = destination;
        unsafe {
            let count = distance(first, last);
            for _ in 0..count {
                destination.current.write_unaligned(first.current.read_unaligned());
                first.advance();
                destination.advance();
            }
            let returned = write_iterator(output, destination);
            (u64::from(destination.first as u32) << 32) | (returned & 0xFFFF_FFFF)
        }
    }
}

port! {
    /// Dispatches queue search through the game's range-search function.
    #[port(address = 0x006C_69D0, owner = "lotdeveloper", name = "FindBuildingQueue")]
    pub unsafe extern "C" fn fc4p_006c69d0(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, value: *const u32) -> u64 {
        unsafe {
            let tag = value as u32;
            let returned = find_words(output, first, last, value, &tag);
            (returned & 0xFFFF_FFFF_0000_0000) | u64::from(output as u32)
        }
    }
}

macro_rules! copy_dispatch {
    ($address:literal, $export:ident, $name:literal, $callee:ident) => {
        port! {
            /// Dispatches copying with the queue's iterator representation and a zero category tag.
            #[port(address = $address, owner = "lotdeveloper", name = $name)]
            pub unsafe extern "C" fn $export(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, _tag: *const u32) -> u64 {
                unsafe {
                    let tag = output as u32;
                    let returned = $callee(output, first, last, destination, &tag, 0);
                    (returned & 0xFFFF_FFFF_0000_0000) | u64::from(output as u32)
                }
            }
        }
    };
}
copy_dispatch!(
    0x006C_6BA0,
    fc4p_006c6ba0,
    "DispatchBuildingQueueCopyBackward",
    copy_backward
);
copy_dispatch!(
    0x006C_6C20,
    fc4p_006c6c20,
    "DispatchBuildingQueueCopyForward",
    copy_forward
);

macro_rules! copy_wrapper {
    ($address:literal, $export:ident, $name:literal, $callee:ident) => {
        port! {
            /// Calls the game's copy dispatcher with an iterator category pointer.
            #[port(address = $address, owner = "lotdeveloper", name = $name)]
            pub unsafe extern "C" fn $export(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator) -> u64 {
                unsafe {
                    let tag = output as u32;
                    let returned = $callee(output, first, last, destination, &tag);
                    (returned & 0xFFFF_FFFF_0000_0000) | u64::from(output as u32)
                }
            }
        }
    };
}
copy_wrapper!(
    0x006C_6ED0,
    fc4p_006c6ed0,
    "CopyBuildingQueueRangeBackward",
    copy_backward_dispatch
);
copy_wrapper!(
    0x006C_6F50,
    fc4p_006c6f50,
    "CopyBuildingQueueRangeForward",
    copy_forward_dispatch
);

port! {
    /// Shuffles a queue range using one C random draw per element after the first. Each draw chooses
    /// an element in the inclusive prefix, and the queue's own iterator helper locates it.
    #[port(address = 0x006C_68C0, owner = "lotdeveloper", name = "ShuffleBuildingQueue", naked)]
    pub unsafe extern "C" fn fc4p_006c68c0() {
        core::arch::naked_asm!(
            "push edx", "mov edx, eax", "lea ecx, [esp + 4]", "call {body}", "ret",
            body = sym shuffle,
        )
    }
}

unsafe extern "fastcall" fn shuffle(stack: *mut u32, entry_eax: u32, entry_edx: u32) -> u64 {
    unsafe {
        let first_ptr = stack.add(1).cast::<QueueIterator>();
        let last_ptr = stack.add(5).cast::<QueueIterator>();
        let first = first_ptr.read_unaligned();
        if first.current == last_ptr.read_unaligned().current {
            return (u64::from(entry_edx) << 32) | u64::from(entry_eax);
        }
        let mut current = first;
        let returned = queue_at(first_ptr, &mut current, 1);
        if current.current == last_ptr.read_unaligned().current {
            return returned;
        }
        let remaining = words_between(first.last as usize, first.current as usize);
        let mut returned;
        loop {
            let random = crt_random();
            let prefix = words_between(current.node as usize, first_ptr.read_unaligned().node as usize)
                .wrapping_shl(5)
                .wrapping_add(remaining)
                .wrapping_add(words_between(current.current as usize, current.first as usize))
                .wrapping_sub(31);
            // Signed division retains the CPU's behavior for unusual random-generator results.
            let index: i32;
            core::arch::asm!("cdq", "idiv {divisor}", divisor = in(reg) prefix,
                inout("eax") random => _, out("edx") index);
            let mut selected = first_ptr.read_unaligned();
            advance_queue(&mut selected, index);
            let selected_value = selected.current.read_unaligned();
            let current_value = current.current.read_unaligned();
            current.current.write_unaligned(selected_value);
            selected.current.write_unaligned(current_value);
            current.advance();
            returned = (u64::from(current_value) << 32) | u64::from(current.last as u32);
            if current.current == last_ptr.read_unaligned().current {
                break;
            }
        }
        returned
    }
}

port! {
    /// Erases one element by shifting the shorter side, then advances the front or retreats the back.
    /// An emptied edge block is freed before its replacement block pointer is read from the map.
    #[port(address = 0x006C_7C50, owner = "lotdeveloper", name = "EraseBuildingQueueElement")]
    pub unsafe extern "thiscall" fn fc4p_006c7c50(this: *mut u8, output: *mut QueueIterator, position: QueueIterator) -> u64 {
        unsafe {
            let mut after = position;
            after.advance();
            let queue = QueueRange::from_ptr(this);
            let begin = queue.begin();
            let index = distance(begin, position);
            let half = (distance(begin, queue.end()) as u32) >> 1;
            let mut temporary = position;
            if (index as u32) < half {
                copy_range_backward(&mut temporary, queue.begin(), position, after);
                let begin = queue.begin();
                if begin.current != begin.last.wrapping_sub(1) {
                    this.cast::<*mut u32>().write_unaligned(begin.current.wrapping_add(1));
                } else {
                    if !begin.first.is_null() { free_block(begin.first); }
                    let node = queue.begin().node.wrapping_add(1);
                    this.add(12).cast::<*mut *mut u32>().write_unaligned(node);
                    let first = node.read_unaligned();
                    this.add(4).cast::<*mut u32>().write_unaligned(first);
                    let first = queue.begin().first;
                    this.add(8).cast::<*mut u32>().write_unaligned(first.wrapping_add(32));
                    this.cast::<*mut u32>().write_unaligned(first);
                }
            } else {
                copy_range_forward(&mut temporary, after, queue.end(), position);
                let end = queue.end();
                if end.current != end.first {
                    this.add(16).cast::<*mut u32>().write_unaligned(end.current.wrapping_sub(1));
                } else {
                    if !end.first.is_null() { free_block(end.first); }
                    let node = queue.end().node.wrapping_sub(1);
                    this.add(28).cast::<*mut *mut u32>().write_unaligned(node);
                    let first = node.read_unaligned();
                    this.add(20).cast::<*mut u32>().write_unaligned(first);
                    this.add(24).cast::<*mut u32>().write_unaligned(first.wrapping_add(32));
                    let last = queue.end().last;
                    this.add(16).cast::<*mut u32>().write_unaligned(last.wrapping_sub(1));
                }
            }
            let returned = queue_at(this.cast(), output, index);
            (returned & 0xFFFF_FFFF_0000_0000) | u64::from(output as u32)
        }
    }
}

unsafe fn store_cursor(this: *mut u8, source: *const u32) -> u32 {
    let mut word = 0;
    unsafe {
        for i in 0..4 {
            word = source.add(i).read_unaligned();
            this.add(0x8C + i * 4).cast::<u32>().write_unaligned(word);
        }
    }
    word
}

port! {
    /// Removes an occupant's building interface from the night queue. A cursor on the erased element
    /// takes erase's successor; other cursors retain their logical element. Found entries release the
    /// queried interface twice, while an absent entry or a failed query releases it at most once.
    #[port(address = 0x006C_8830, owner = "lotdeveloper", name = "RemoveNightLightingBuilding")]
    pub unsafe extern "thiscall" fn fc4p_006c8830(this: *mut u8, occupant: *mut u8) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut building = core::ptr::null_mut::<u8>();
            let output = &mut building as *mut *mut u8 as u32;
            let mut call = virtual_call(occupant, 0, &[0x87DD_A3A9, output], table(occupant), output, false);
            if call.eax as u8 != 0 {
                let mut found = developer.night_begin();
                find_queue(&mut found, developer.night_begin(), developer.night_end(), (&building as *const *mut u8).cast());
                let end = developer.night_end();
                call.eax = end.current as u32;
                call.edx = end.last as u32;
                if found.current != end.current {
                    let cursor = developer.night_cursor();
                    let mut successor = found;
                    let returned = if cursor.current == found.current {
                        erase_queue(this.add(0x64), &mut successor, found)
                    } else {
                        let begin = developer.night_begin();
                        let index = distance(begin, cursor);
                        erase_queue(this.add(0x64), &mut successor, found);
                        let removed_index = distance(begin, found);
                        let index = if index > removed_index { index.wrapping_sub(1) } else { index };
                        let begin = developer.night_begin();
                        queue_at(&begin, &mut successor, index)
                    };
                    let node = store_cursor(this, returned as u32 as *const u32);
                    call = virtual_call(building, 8, &[], table(building), node, false);
                }
            }
            if !building.is_null() {
                call = virtual_call(building, 8, &[], call.eax, table(building), false);
            }
            result(call)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c63c0::PORT,
    fc4p_006c6570::PORT,
    fc4p_006c6630::PORT,
    fc4p_006c69d0::PORT,
    fc4p_006c6ba0::PORT,
    fc4p_006c6c20::PORT,
    fc4p_006c6ed0::PORT,
    fc4p_006c6f50::PORT,
    fc4p_006c68c0::PORT,
    fc4p_006c7c50::PORT,
    fc4p_006c8830::PORT,
];

#[cfg(test)]
mod tests;
