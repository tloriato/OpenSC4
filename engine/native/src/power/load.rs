// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Restoring power simulator records through the serializable interface.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn read_reference(stream: *mut u8, iid: u32, out: *mut u8, immediate: u32) -> u32 = "C" 0x005D_9730; }
game_fn! { fn rows_resize(this: *mut u8, count: u32, value: *const u32) -> u32 = "thiscall" 0x0046_23F0; }
game_fn! { fn row_insert(this: *mut u8, end: u32, count: u32, value: *const u16) -> u32 = "thiscall" 0x0046_2170; }
game_fn! { fn poles_insert(this: *mut u8, end: u32, count: u32, value: *const u32) -> u32 = "thiscall" 0x004F_BA20; }
game_fn! { fn container_allocate(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn allocate(bytes: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_data! { static percent_scale: f32 = 0x00A8_23A4; }

layout! {
    /// Saved containers and flags, relative to the serializable interface.
    struct LoadView {
        0x05 => ready / set_ready: u8,
        0x7C => rows / set_rows: *mut u8,
        0x290 => plant_header / set_plant_header: *mut u8,
        0x294 => poles / set_poles: *mut u8,
        0x2B0 => pole_count / set_pole_count: u32,
        0x2C4 => bitmap_words / set_bitmap_words: u32,
    }
}

unsafe fn word(this: *mut u8, offset: usize) -> u32 {
    // SAFETY: the caller supplies a live object with an accessible word at offset.
    unsafe { read_unaligned(this.add(offset).cast()) }
}
unsafe fn store(this: *mut u8, offset: usize, value: u32) {
    // SAFETY: the caller supplies a live object with an accessible word at offset.
    unsafe { write_unaligned(this.add(offset).cast(), value) }
}

unsafe fn transfer(stream: *mut u8, slot: usize, out: *mut u8) -> u32 {
    // SAFETY: the input stream supplies the matching reader, and out covers its destination.
    unsafe { vcall!(stream, slot => fn(*mut u8) -> u32, out) }
}

port! {
    /// Restore versions zero through six, including references, nested consumption rows, histories,
    /// plant and pole lists and four allocated bitmaps. The version reader reuses the caller's
    /// argument word. AL reports version acceptance; intermediate reader results are ignored.
    #[port(address = 0x006F_0880, owner = "power", name = "cSC4PowerSimulator::Read", naked)]
    pub unsafe extern "C" fn fc4p_006f0880() {
        // Reserve the original local slots before the Rust frame: deferred reference readers can
        // retain destinations in that area, and a failed read leaves the incoming slot intact.
        core::arch::naked_asm!(
            "mov eax, esp", "sub esp, 0x20", "push ebx", "push ebp", "push esi", "push edi",
            "push eax", "call {body}", "pop edi", "pop esi", "pop ebp", "pop ebx",
            "add esp, 0x20", "ret 4", body = sym read_fields,
        );
    }
}

unsafe extern "fastcall" fn read_fields(this: *mut u8, entry_edx: u32, stack: *mut u8) -> u64 {
    // SAFETY: the entry reserves the original frame and supplies the game's live stream and object.
    unsafe {
        let view = LoadView::from_ptr(this);
        let stream_slot = stack.add(4);
        let stream = word(stream_slot, 0) as *mut u8;
        view.set_ready(1);
        let args = [stream_slot as u32];
        let mut call = RawCall::new(Conv::Thiscall, vslot(stream, 0x1C), stream as u32, &args);
        call.eax = word(stream, 0);
        call.edx = entry_edx;
        call_raw(&mut call);
        if read_unaligned(stream_slot.cast::<u16>()) > 6 {
            return (u64::from(call.edx) << 32) | u64::from(call.eax & !255);
        }
        read_reference(stream, 0x223E_2B67, this.add(0x18), 0);
        for offset in [0x30, 0x34, 0x38] {
            transfer(stream, 0x20, this.add(offset));
        }
        let temporary = stack.sub(0x14);
        if read_unaligned(stream_slot.cast::<u16>()) >= 6 {
            transfer(stream, 0x30, temporary);
            store(this, 0x5C, word(temporary, 0));
        } else {
            transfer(stream, 0x20, temporary);
            let value = multiply_float(F80::from_i32(word(temporary, 0) as i32), percent_scale().read());
            store(this, 0x5C, value.to_f32().to_bits());
        }
        for offset in [0x60, 0x64, 0x68, 0x6C] {
            transfer(stream, 0x20, this.add(offset));
        }
        transfer(stream, 0x28, this.add(0x70));
        for offset in [0x48, 0x4C, 0x50, 0x54, 0x58] {
            transfer(stream, 0x20, this.add(offset));
        }
        read_reference(stream, 0xC9BD_939A, this.add(0x78), 0);
        let rows_slot = stack.sub(0x18);
        let columns_slot = stack.sub(0x20);
        transfer(stream, 0x24, rows_slot);
        transfer(stream, 0x24, columns_slot);
        for offset in [0, 4, 8] {
            store(temporary, offset, 0);
        }
        rows_resize(this.add(0x7C), word(rows_slot, 0), temporary.cast());
        store(temporary, 0, 0);
        let zero_slot = stack.sub(8);
        if word(rows_slot, 0) != 0 {
            store(zero_slot, 0, 0);
            let mut row_offset = 0u32;
            let mut row_index = 0u32;
            while row_index < word(rows_slot, 0) {
                let row = view.rows().wrapping_add(row_offset as usize);
                let begin = word(row, 0);
                let end = word(row, 4);
                let size = (end.wrapping_sub(begin) as i32 >> 1) as u32;
                let columns = word(columns_slot, 0);
                if columns < size {
                    store(row, 4, begin.wrapping_add(columns.wrapping_mul(2)));
                } else {
                    row_insert(row, end, columns.wrapping_sub(size), zero_slot.cast());
                }
                let mut z = 0u32;
                while z < word(columns_slot, 0) {
                    let row = view.rows().wrapping_add(row_offset as usize);
                    let destination = (word(row, 0) as *mut u8).wrapping_add(z.wrapping_mul(2) as usize);
                    transfer(stream, 0x18, destination);
                    z = z.wrapping_add(1);
                }
                row_index = row_index.wrapping_add(1);
                row_offset = row_offset.wrapping_add(12);
                store(temporary, 0, row_index);
                store(stack.sub(0x1C), 0, row_offset);
            }
        }
        for i in 0..120 {
            transfer(stream, 0x20, this.add(0x88 + i * 4));
        }
        for i in 0..10 {
            transfer(stream, 0x20, this.add(0x268 + i * 4));
        }
        transfer(stream, 0x20, this.add(0x2D4));
        for i in 0..10 {
            transfer(stream, 0x20, this.add(0x2D8 + i * 4));
        }
        let mut i = 0i32;
        while i < word(this, 0x2D4) as i32 {
            transfer(stream, 0x24, zero_slot);
            store(temporary, 0, word(zero_slot, 0));
            store(temporary, 4, 0);
            read_reference(stream, 0x26BB_767E, temporary.add(4), 0);
            let header = view.plant_header();
            let node = container_allocate(0x10);
            let pair = node.wrapping_add(8);
            if !pair.is_null() {
                store(pair, 0, word(temporary, 0));
                store(pair, 4, word(temporary, 4));
            }
            let previous = word(header, 4) as *mut u8;
            store(node, 0, header as u32);
            store(node, 4, previous as u32);
            store(previous, 0, node as u32);
            store(header, 4, node as u32);
            i = i.wrapping_add(1);
        }
        transfer(stream, 0x24, this.add(0x2B0));
        transfer(stream, 0x24, this.add(0x2B4));
        let count = view.pole_count();
        let begin = view.poles();
        let end = word(this, 0x298);
        let size = (end.wrapping_sub(begin as u32) as i32 >> 2) as u32;
        store(zero_slot, 0, 0);
        if count < size {
            store(this, 0x298, (begin as u32).wrapping_add(count.wrapping_mul(4)));
        } else {
            poles_insert(this.add(0x294), end, count.wrapping_sub(size), zero_slot.cast());
        }
        let mut i = 0u32;
        while i < view.pole_count() {
            read_reference(
                stream,
                0xA8C8_6142,
                view.poles().wrapping_add(i.wrapping_mul(4) as usize),
                0,
            );
            i = i.wrapping_add(1);
        }
        transfer(stream, 0x24, this.add(0x2C4));
        for offset in [0x2BC, 0x2C0, 0x2CC, 0x2C8] {
            let data = allocate(view.bitmap_words().wrapping_mul(4));
            store(this, offset, data as u32);
            let mut i = 0u32;
            while i < view.bitmap_words() {
                let data = word(this, offset) as *mut u8;
                transfer(stream, 0x24, data.wrapping_add(i.wrapping_mul(4) as usize));
                i = i.wrapping_add(1);
            }
        }
        transfer(stream, 0x14, this.add(0x2D0));
        transfer(stream, 0x14, this.add(0x328));
        transfer(stream, 0x20, this.add(0x32C));
        transfer(stream, 0x14, this.add(0x330));
        transfer(stream, 0x14, this.add(0x331));
        this.add(0x328).write(0);
        let mode_slot = stack.sub(4);
        let args = [mode_slot as u32];
        let mut call = RawCall::new(Conv::Thiscall, vslot(stream, 0x24), stream as u32, &args);
        call.eax = mode_slot as u32;
        call.edx = word(stream, 0);
        call_raw(&mut call);
        store(this, 0x344, word(mode_slot, 0));
        (u64::from(call.edx) << 32) | u64::from((call.eax & !255) | 1)
    }
}

pub const PORTS: &[Port] = &[fc4p_006f0880::PORT];
