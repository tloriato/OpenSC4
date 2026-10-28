// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tunnel span collection and endpoint height propagation.

use super::*;

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn copy_records(first: *mut u8, last: *mut u8, output: *mut u8, scratch: *mut u32) -> *mut u8 = "C" 0x0063_7E20; }
game_fn! { fn free(pointer: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn find_cell(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }

layout! {
    /// Heights and step indices retained by each tunnel span.
    struct TunnelSpan {
        0x00 => first_height / set_first_height: u32,
        0x04 => last_height / set_last_height: u32,
        0x08 => first_step / set_first_step: u32,
        0x0C => last_step / set_last_step: i32,
    }
}

unsafe fn append(output: *mut u8, record: [u32; 4]) {
    unsafe {
        let end: *mut u8 = read(output, 4);
        let capacity: *mut u8 = read(output, 8);
        if end != capacity {
            if !end.is_null() {
                for (i, word) in record.into_iter().enumerate() {
                    write(end, i * 4, word);
                }
            }
            write(output, 4, read::<*mut u8>(output, 4).wrapping_add(16));
        } else {
            let begin: *mut u8 = read(output, 0);
            let count = (end as u32).wrapping_sub(begin as u32) as i32 >> 4;
            let capacity = (count as u32).wrapping_add((count as u32).max(1));
            let allocation = if capacity == 0 {
                core::ptr::null_mut()
            } else {
                allocate(capacity.wrapping_shl(4))
            };
            let mut scratch = output as u32;
            let position = copy_records(read(output, 0), end, allocation, &mut scratch);
            if !position.is_null() {
                for (i, word) in record.into_iter().enumerate() {
                    write(position, i * 4, word);
                }
            }
            let begin: *mut u8 = read(output, 0);
            if !begin.is_null() {
                free(begin);
            }
            write(output, 0, allocation);
            write(output, 4, position.wrapping_add(16));
            write(output, 8, allocation.wrapping_add(capacity.wrapping_shl(4) as usize));
        }
    }
}

unsafe fn apply_height(this: *mut u8, span: TunnelSpan, endpoint: usize) {
    unsafe {
        let step: u32 = read(span.as_ptr(), endpoint + 8);
        let steps: *mut u8 = read(this, 0x54);
        let range = steps.wrapping_add(step.wrapping_mul(12) as usize);
        let mut index: u32 = read(range, 0);
        let end: u32 = read(range, 4);
        if index > end {
            return;
        }
        loop {
            let points: *mut u8 = read(this, 0x60);
            let point = points.wrapping_add(index.wrapping_mul(8) as usize);
            let key = read::<u32>(point, 4).wrapping_shl(16).wrapping_add(read(point, 0));
            let height: u32 = read(span.as_ptr(), endpoint);
            let cell = find_cell(this, key);
            for corner in 0..4 {
                write(cell, 0x6C + corner * 8, height);
                write(cell, 0x68 + corner * 8, height);
                write(cell, 0x58 + corner * 4, height);
            }
            let step: u32 = read(span.as_ptr(), endpoint + 8);
            let steps: *mut u8 = read(this, 0x54);
            let end: u32 = read(steps.wrapping_add(step.wrapping_mul(12) as usize), 4);
            index = index.wrapping_add(1);
            if index > end {
                break;
            }
        }
    }
}

port! {
    /// Append tunnel spans while their signed end step is below the valid step count; stop at the first
    /// later span. For each accepted span, set all endpoint-cell heights over both inclusive ranges.
    /// Height words are copied unchanged, including NaN payloads and signed zero.
    #[port(address = 0x0062_74B0, owner = "networktool", name = "CollectTunnelSpans")]
    pub unsafe extern "thiscall" fn fc4p_006274b0(this: *mut u8, output: *mut u8) -> u32 {
        unsafe {
            let tool = NetworkTool::from_ptr(this);
            let mut current = tool.tunnel_begin();
            let end = tool.tunnel_end();
            while current != end {
                let span = TunnelSpan::from_ptr(current);
                let last = span.last_step();
                if last >= read::<i32>(this, 0x78) { break; }
                let first_height = span.first_height();
                let first_step = span.first_step();
                let last_height = span.last_height();
                append(output, [first_step, last as u32, first_height, last_height]);
                apply_height(this, span, 0);
                apply_height(this, span, 4);
                current = current.wrapping_add(16);
            }
            end as u32
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006274b0::PORT];

#[cfg(test)]
mod tests;
