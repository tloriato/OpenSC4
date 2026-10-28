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
        args.sub(2).write(left);
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
    /// Test any inclusive rectangle bit. Return the left-edge mask in ECX and retain the original upper EAX loop result.
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

game_fn! {
    fn horizontal(bits:*mut u32, mask:*mut u32, width:i32, lo:*mut i32, hi:*mut i32) -> u32 = "C" 0x0060_3E50;
}
game_fn! {
    fn vertical(bits:*mut u32, mask:*mut u32, width:i32, lo:*mut i32, hi:*mut i32) -> u32 = "C" 0x0060_3FD0;
}

#[allow(clippy::too_many_arguments)]
unsafe fn change(
    bits: *mut u32,
    mask: *mut u32,
    index: i32,
    old: u32,
    expanded: u32,
    row: i32,
    lo: *mut i32,
    hi: *mut i32,
) -> bool {
    unsafe {
        let difference = (expanded & load(mask, index)) ^ old;
        if difference == 0 {
            return false;
        }
        if row < lo.read_unaligned() {
            lo.write_unaligned(row);
        }
        if row >= hi.read_unaligned() {
            hi.write_unaligned(row.wrapping_add(1));
        }
        store(bits, index, old | difference);
        true
    }
}

unsafe extern "C" fn horizontal_body(args: *mut u32) -> u32 {
    unsafe {
        let bits = args.read() as *mut u32;
        let mask = args.add(1).read() as *mut u32;
        let width = args.add(2).read() as i32;
        let lo = args.add(3).read() as *mut i32;
        let hi = args.add(4).read() as *mut i32;
        let mut row = lo.read_unaligned();
        let end = hi.read_unaligned();
        let mut index = (width >> 5).wrapping_mul(row);
        lo.write_unaligned(width);
        hi.write_unaligned(0);
        args.add(2).write(row as u32);
        let mut changed = false;
        while row < end {
            let mut old = load(bits, index);
            let mut next = load(bits, index.wrapping_add(1));
            let expanded = ((next << 30) | old) << 1 | (old >> 1) | old;
            changed |= change(bits, mask, index, old, expanded, row, lo, hi);
            index = index.wrapping_add(1);
            if width.wrapping_sub(96) >= 0 {
                let count = (width.wrapping_sub(64) as u32) >> 5;
                for _ in 0..count {
                    let previous = old;
                    old = next;
                    next = load(bits, index.wrapping_add(1));
                    let expanded = (((previous >> 30) | old) >> 1) | (old << 1) | old | (next << 31);
                    changed |= change(bits, mask, index, old, expanded, row, lo, hi);
                    index = index.wrapping_add(1);
                }
            }
            let expanded = (((old >> 30) | next) >> 1) | (next << 1) | next;
            changed |= change(bits, mask, index, next, expanded, row, lo, hi);
            index = index.wrapping_add(1);
            row = row.wrapping_add(1);
            args.add(2).write(row as u32);
        }
        u32::from(changed)
    }
}
port! {
    /// One horizontal growth pass, retaining pre-pass neighboring words and reporting a half-open changed band.
    /// Reset the band to width/zero first and use the width argument word as the current row.
    #[port(address = 0x0060_3E50, owner = "power", name = "Bit1_GrowHorizontal", naked)]
    pub unsafe extern "C" fn fc4p_00603e50() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {body}", "add esp, 4", "ret", body=sym horizontal_body);
    }
}
port! {
    /// One vertical pass, processing whole word columns and keeping old row words across updates.
    /// The supplied row interval is half open; an interval shorter than two rows still visits two.
    #[port(address = 0x0060_3FD0, owner = "power", name = "Bit1_GrowVertical")]
    pub unsafe extern "C" fn fc4p_00603fd0(bits:*mut u32, mask:*mut u32, width:i32, lo:*mut i32, hi:*mut i32) -> u32 {
        unsafe {
            let end=hi.read_unaligned().wrapping_sub(1);
            let start=lo.read_unaligned();
            lo.write_unaligned(width);
            hi.write_unaligned(0);
            if width <= 0 { return 0; }
            let stride=width>>5;
            let base=stride.wrapping_mul(start);
            let mut x=0i32;
            let mut changed=false;
            loop {
                let mut index=base.wrapping_add(x/32);
                let mut old=load(bits,index);
                let mut next=load(bits,index.wrapping_add(stride));
                changed |= change(bits,mask,index,old,old|next,start,lo,hi);
                index=index.wrapping_add(stride);
                let mut row=start.wrapping_add(1);
                while row < end {
                    let previous=old;
                    old=next;
                    next=load(bits,index.wrapping_add(stride));
                    changed |= change(bits,mask,index,old,previous|old|next,row,lo,hi);
                    index=index.wrapping_add(stride);
                    row=row.wrapping_add(1);
                }
                changed |= change(bits,mask,index,next,next|old,row,lo,hi);
                x=x.wrapping_add(32);
                if x >= width { break; }
            }
            u32::from(changed)
        }
    }
}

unsafe extern "C" fn grow_body(args: *mut u32) -> i32 {
    unsafe {
        let bits = args.read() as *mut u32;
        let mask = args.add(1).read() as *mut u32;
        let width = args.add(2).read() as i32;
        let budget = args.add(3).read() as i32;
        let lo = args.add(4).read() as *mut i32;
        let hi = args.add(5).read() as *mut i32;
        let mut hmin = lo.read_unaligned();
        let mut hmax = hi.read_unaligned();
        let mut vmin = if hmin > 0 { hmin.wrapping_sub(1) } else { hmin };
        let mut vmax = if hmax < width.wrapping_sub(1) {
            hmax.wrapping_add(1)
        } else {
            hmax
        };
        let mut iterations = 0i32;
        let mut changed = true;
        args.add(2).write(0);
        while iterations < budget && changed {
            if hmin < lo.read_unaligned() {
                lo.write_unaligned(hmin);
            }
            if hmax > hi.read_unaligned() {
                hi.write_unaligned(hmax);
            }
            let h = horizontal(bits, mask, width, &mut hmin, &mut hmax);
            let v = vertical(bits, mask, width, &mut vmin, &mut vmax);
            hmin = hmin.min(vmin);
            hmax = hmax.max(vmax);
            vmin = if hmin > 0 { hmin.wrapping_sub(1) } else { hmin };
            vmax = if hmax < width { hmax.wrapping_add(1) } else { hmax };
            changed = h as u8 != 0 || v as u8 != 0;
            iterations = iterations.wrapping_add(1);
            args.add(2).write(iterations as u32);
        }
        iterations
    }
}
port! {
    /// Alternate horizontal and vertical passes, merging their bands before widening the next vertical band.
    /// Publish changed bounds at the start of each iteration. The width argument becomes the iteration count.
    #[port(address = 0x0060_4570, owner = "power", name = "Bit1_SelectionGrow", naked)]
    pub unsafe extern "C" fn fc4p_00604570() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {body}", "add esp, 4", "ret", body=sym grow_body);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00603e50::PORT,
    fc4p_00603fd0::PORT,
    fc4p_00604570::PORT,
    fc4p_00604180::PORT,
    fc4p_00604260::PORT,
    fc4p_00604350::PORT,
    fc4p_00604440::PORT,
];

#[cfg(test)]
mod tests;
