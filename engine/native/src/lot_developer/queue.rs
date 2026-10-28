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
    pub unsafe extern "C" fn fc4p_006c69d0(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, value: *const u32, tag: u32) -> u64 {
        unsafe {
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
            pub unsafe extern "C" fn $export(output: *mut QueueIterator, first: QueueIterator, last: QueueIterator, destination: QueueIterator, tag: u32) -> u64 {
                unsafe {
                    let returned = $callee(output, first, last, destination, &tag, 0);
                    (returned & 0xFFFF_FFFF_0000_0000) | u64::from(output as u32)
                }
            }
        }
    };
}
copy_dispatch!(0x006C_6BA0, fc4p_006c6ba0, "DispatchBuildingQueueCopyBackward", copy_backward);
copy_dispatch!(0x006C_6C20, fc4p_006c6c20, "DispatchBuildingQueueCopyForward", copy_forward);

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
copy_wrapper!(0x006C_6ED0, fc4p_006c6ed0, "CopyBuildingQueueRangeBackward", copy_backward_dispatch);
copy_wrapper!(0x006C_6F50, fc4p_006c6f50, "CopyBuildingQueueRangeForward", copy_forward_dispatch);

pub const PORTS: &[Port] = &[
    fc4p_006c63c0::PORT, fc4p_006c6570::PORT, fc4p_006c6630::PORT, fc4p_006c69d0::PORT,
    fc4p_006c6ba0::PORT, fc4p_006c6c20::PORT, fc4p_006c6ed0::PORT, fc4p_006c6f50::PORT,
];

#[cfg(test)]
mod tests;
