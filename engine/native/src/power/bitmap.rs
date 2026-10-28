// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Shared inclusive bitmap rectangle operations. Argument words changed by these helpers stay visible to callers.

use super::*;
use core::ptr::{read_unaligned, write_unaligned};

unsafe fn load(bits: *mut u32, index: i32) -> u32 {
    unsafe { read_unaligned(bits.wrapping_offset(index as isize)) }
}
unsafe fn store(bits: *mut u32, index: i32, value: u32) {
    unsafe { write_unaligned(bits.wrapping_offset(index as isize), value) }
}
unsafe fn put(bits: *mut u32, index: i32, mask: u32, on: bool) {
    unsafe {
        let word = load(bits, index);
        store(bits, index, if on { word | mask } else { word & !mask });
    }
}

/// The entry wrappers pass the actual caller argument words, since width and rectangle words are scratch storage.
unsafe extern "C" fn rect_put(args: *mut u32, on: bool) {
    unsafe {
        let bits = args.read() as *mut u32;
        let width = args.add(1).read() as i32;
        let rect = args.add(2).read() as *mut Rect;
        let r = rect.read_unaligned();
        let base = (r.z0 as i32).wrapping_mul(width);
        let mut first = base.wrapping_add(r.x0 as i32) >> 5;
        let mut last = base.wrapping_add(r.x1 as i32) >> 5;
        let left = u32::MAX.wrapping_shl(r.x0);
        let right = 2u32.wrapping_shl(r.x1).wrapping_sub(1);
        let stride = width >> 5;
        if !on {
            args.add(1).write(stride as u32);
        }
        let mut row = r.z0;
        if first == last {
            let mask = left & right;
            while row <= read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                put(bits, first, mask, on);
                first = first.wrapping_add(stride);
                row = row.wrapping_add(1);
            }
        } else {
            if on {
                args.add(1).write(row);
            }
            if row > read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                return;
            }
            let mut inner = first.wrapping_add(1);
            args.add(2).write(inner as u32);
            loop {
                put(bits, first, left, on);
                put(bits, last, right, on);
                if (inner as u32) < (last as u32) {
                    let mut i = inner;
                    while i != last {
                        store(bits, i, if on { u32::MAX } else { 0 });
                        i = i.wrapping_add(1);
                    }
                }
                inner = inner.wrapping_add(stride);
                first = first.wrapping_add(stride);
                last = last.wrapping_add(stride);
                row = row.wrapping_add(1);
                args.add(2).write(inner as u32);
                if on {
                    args.add(1).write(row);
                }
                if row > read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                    break;
                }
            }
        }
    }
}

unsafe extern "C" fn set_body(args: *mut u32) {
    unsafe { rect_put(args, true) }
}
unsafe extern "C" fn clear_body(args: *mut u32) {
    unsafe { rect_put(args, false) }
}

port! {
    /// Set inclusive rectangle bits. Multiword rows also overwrite the width and rectangle argument words.
    #[port(address = 0x0060_4180, owner = "power", name = "Bit1_RectSet", naked)]
    pub unsafe extern "C" fn fc4p_00604180() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push eax", "call {body}", "add esp, 4", "ret", body = sym set_body,
        );
    }
}
port! {
    /// Clear inclusive bits, replacing the width argument with its signed word stride.
    #[port(address = 0x0060_4260, owner = "power", name = "Bit1_RectClear", naked)]
    pub unsafe extern "C" fn fc4p_00604260() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push eax", "call {body}", "add esp, 4", "ret", body = sym clear_body,
        );
    }
}

unsafe extern "C" fn any_body(args: *mut u32) -> u32 {
    unsafe {
        let bits = args.read() as *mut u32;
        let width = args.add(1).read() as i32;
        let rect = args.add(2).read() as *const Rect;
        let r = rect.read_unaligned();
        let base = (r.z0 as i32).wrapping_mul(width);
        let mut first = base.wrapping_add(r.x0 as i32) >> 5;
        let mut last = base.wrapping_add(r.x1 as i32) >> 5;
        let left = u32::MAX.wrapping_shl(r.x0);
        let right = 2u32.wrapping_shl(r.x1).wrapping_sub(1);
        let stride = width >> 5;
        let mut row = r.z0;
        if first == last {
            while row <= r.z1 {
                if load(bits, first) & left & right != 0 {
                    return (row & !255) | 1;
                }
                first = first.wrapping_add(stride);
                row = row.wrapping_add(1);
            }
            row & !255
        } else {
            args.add(1).write(r.z1);
            if row > r.z1 {
                return last as u32 & !255;
            }
            args.add(2).write(bits.wrapping_offset(first as isize) as u32);
            loop {
                if load(bits, first) & left != 0 || load(bits, last) & right != 0 {
                    return (last as u32 & !255) | 1;
                }
                let mut inner = first.wrapping_add(1);
                while (inner as u32) < (last as u32) {
                    if load(bits, inner) != 0 {
                        return (last as u32 & !255) | 1;
                    }
                    inner = inner.wrapping_add(1);
                }
                first = first.wrapping_add(stride);
                last = last.wrapping_add(stride);
                row = row.wrapping_add(1);
                args.add(2).write(bits.wrapping_offset(first as isize) as u32);
                if row > r.z1 {
                    return last as u32 & !255;
                }
            }
        }
    }
}
port! {
    /// Test any inclusive rectangle bit. Preserve entry ECX and the original upper EAX loop result.
    #[port(address = 0x0060_4350, owner = "power", name = "Bit1_RectTestAny", naked)]
    pub unsafe extern "C" fn fc4p_00604350() {
        core::arch::naked_asm!(
            "push ecx", "lea eax, [esp + 8]", "push eax", "call {body}", "add esp, 4", "pop ecx", "ret",
            body = sym any_body,
        );
    }
}

unsafe extern "C" fn or_body(args: *mut u32) {
    unsafe {
        let dst = args.read() as *mut u32;
        let src = args.add(1).read() as *mut u32;
        let width = args.add(2).read() as i32;
        let rect = args.add(3).read() as *const Rect;
        let r = rect.read_unaligned();
        let base = (r.z0 as i32).wrapping_mul(width);
        let mut first = base.wrapping_add(r.x0 as i32) >> 5;
        let mut last = base.wrapping_add(r.x1 as i32) >> 5;
        let left = u32::MAX.wrapping_shl(r.x0);
        let right = 2u32.wrapping_shl(r.x1).wrapping_sub(1);
        let stride = width >> 5;
        let mut row = r.z0;
        if first == last {
            while row <= read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                let value = load(src, first) & left & right;
                put(dst, first, value, true);
                first = first.wrapping_add(stride);
                row = row.wrapping_add(1);
            }
        } else {
            if row > read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                return;
            }
            let mut inner = first.wrapping_add(1);
            args.add(2).write(inner as u32);
            args.add(3).write((src as u32).wrapping_sub(dst as u32));
            loop {
                put(dst, first, load(src, first) & left, true);
                put(dst, last, load(src, last) & right, true);
                // The interior loop repeatedly ORs the first word and never advances its pointer.
                let count = last.wrapping_sub(inner) as u32;
                if (inner as u32) < (last as u32) {
                    for _ in 0..count {
                        put(dst, first, load(src, first), true);
                    }
                }
                inner = inner.wrapping_add(stride);
                args.add(2).write(inner as u32);
                first = first.wrapping_add(stride);
                last = last.wrapping_add(stride);
                row = row.wrapping_add(1);
                if row > read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                    break;
                }
            }
        }
    }
}
port! {
    /// OR source bits at the rectangle edges; the interior loop repeats the first word without advancing.
    #[port(address = 0x0060_4440, owner = "power", name = "Bit1_RectOr", naked)]
    pub unsafe extern "C" fn fc4p_00604440() {
        core::arch::naked_asm!(
            "lea eax, [esp + 4]", "push eax", "call {body}", "add esp, 4", "ret", body = sym or_body,
        );
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00604180::PORT,
    fc4p_00604260::PORT,
    fc4p_00604350::PORT,
    fc4p_00604440::PORT,
];

#[cfg(test)]
mod tests;
