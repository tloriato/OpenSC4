// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Version-six power records written from the serializable interface.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use core::ptr::read_unaligned;

layout! {
    /// Saved fields, relative to the serializable interface, eight bytes past the primary interface.
    struct SaveView {
        0x18 => department / set_department: *mut u8,
        0x70 => cost_base / set_cost_base: u64,
        0x78 => grid / set_grid: *mut u8,
        0x7C => rows / set_rows: *mut u8,
        0x80 => rows_end / set_rows_end: *mut u8,
        0x290 => plant_header / set_plant_header: *mut u8,
        0x294 => poles / set_poles: *mut *mut u8,
        0x2B0 => pole_count / set_pole_count: u32,
        0x2B4 => broken_count / set_broken_count: u32,
        0x2C4 => bitmap_words / set_bitmap_words: u32,
    }
}
game_fn! { fn write_reference(stream: *mut u8, object: *mut u8) -> u32 = "C" 0x005D_99A0; }

unsafe fn put_word(stream: *mut u8, slot: usize, word: u32) {
    // SAFETY: the caller supplies the stream's word writer.
    unsafe {
        vcall!(stream, slot => fn(u32), word);
    }
}
unsafe fn read_word(this: *mut u8, offset: usize) -> u32 {
    // SAFETY: the caller supplies a readable field at this offset.
    unsafe { read_unaligned(this.add(offset).cast()) }
}

port! {
    /// Write the version, references, totals, signed consumption cells, twelve usage rows, annual totals,
    /// plants, poles, four bitmaps and pipeline flags in file order. Results of individual writes are
    /// ignored. AL is always one; upper EAX and EDX come from the final state-word writer.
    /// Vector lengths use the stored pointer differences, and fields are read again after stream calls.
    #[port(address = 0x006E_B6A0, owner = "power", name = "cSC4PowerSimulator::Write")]
    pub unsafe extern "thiscall" fn fc4p_006eb6a0(this: *mut u8, stream: *mut u8) -> u64 {
        // SAFETY: the game supplies the serializable interface, its containers and the output stream.
        unsafe {
            let sim = SaveView::from_ptr(this);
            put_word(stream, 0x1C, 6);
            write_reference(stream, sim.department());
            for offset in [0x30, 0x34, 0x38] { put_word(stream, 0x20, read_word(this, offset)); }
            put_word(stream, 0x30, read_word(this, 0x5C));
            for offset in [0x60, 0x64, 0x68, 0x6C] { put_word(stream, 0x20, read_word(this, offset)); }
            vcall!(stream, 0x28 => fn(u64), sim.cost_base());
            for offset in [0x48, 0x4C, 0x50, 0x54, 0x58] { put_word(stream, 0x20, read_word(this, offset)); }
            write_reference(stream, sim.grid());
            let rows = ((sim.rows_end() as u32).wrapping_sub(sim.rows() as u32) as i32 / 12) as u32;
            // Even an empty outer vector has its first row inspected for the second dimension.
            let first = sim.rows();
            let columns = (read_word(first, 4).wrapping_sub(read_word(first, 0)) as i32 >> 1) as u32;
            put_word(stream, 0x24, rows);
            put_word(stream, 0x24, columns);
            for x in 0..rows {
                for z in 0..columns {
                    let row = read_unaligned(sim.rows().wrapping_add(x.wrapping_mul(12) as usize).cast::<*const u16>());
                    let cell = read_unaligned(row.wrapping_add(z as usize));
                    put_word(stream, 0x18, u32::from(cell));
                }
            }
            for i in 0..120 { put_word(stream, 0x20, read_word(this, 0x88 + i * 4)); }
            for i in 0..10 { put_word(stream, 0x20, read_word(this, 0x268 + i * 4)); }
            put_word(stream, 0x20, read_word(this, 0x2D4));
            for i in 0..10 { put_word(stream, 0x20, read_word(this, 0x2D8 + i * 4)); }
            let header = sim.plant_header();
            let mut node = read_unaligned(header.cast::<*mut u8>());
            while node != header {
                put_word(stream, 0x24, read_word(node, 8));
                write_reference(stream, read_unaligned(node.add(12).cast()));
                node = read_unaligned(node.cast());
            }
            put_word(stream, 0x24, sim.pole_count());
            put_word(stream, 0x24, sim.broken_count());
            let mut i = 0u32;
            while i < sim.pole_count() {
                write_reference(stream, read_unaligned(sim.poles().wrapping_add(i as usize)));
                i = i.wrapping_add(1);
            }
            put_word(stream, 0x24, sim.bitmap_words());
            for offset in [0x2BC, 0x2C0, 0x2CC, 0x2C8] {
                let mut i = 0u32;
                while i < sim.bitmap_words() {
                    let bitmap = read_word(this, offset) as *const u32;
                    put_word(stream, 0x24, read_unaligned(bitmap.wrapping_add(i as usize)));
                    i = i.wrapping_add(1);
                }
            }
            for offset in [0x2D0, 0x328] { put_word(stream, 0x14, u32::from(*this.add(offset))); }
            put_word(stream, 0x20, read_word(this, 0x32C));
            for offset in [0x330, 0x331] { put_word(stream, 0x14, u32::from(*this.add(offset))); }
            let args = [read_word(this, 0x344)];
            let mut final_write = RawCall::new(Conv::Thiscall, vslot(stream, 0x24), stream as u32, &args);
            final_write.eax = args[0];
            final_write.edx = read_unaligned(stream.cast());
            call_raw(&mut final_write);
            (u64::from(final_write.edx) << 32) | u64::from((final_write.eax & !255) | 1)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006eb6a0::PORT];

#[cfg(test)]
mod tests;
