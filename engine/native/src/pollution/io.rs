// SPDX-License-Identifier: GPL-3.0-or-later
//! Version-four pollution serialization.

use super::*;

game_fn! { fn write_reference(stream: *mut u8, object: *mut u8) -> u64 = "C" 0x005D_99A0; }
game_fn! { fn write_list(stream: *mut u8, list: *mut u8) = "C" 0x005D_AFF0; }
game_fn! { fn write_iterator(stream: *mut u8, iterator: *mut u8) = "C" 0x005D_AB70; }

layout! {
    /// Persisted fields relative to the serializable interface, twelve bytes into the simulator.
    struct SaveView {
        0x34 => sources / set_sources: [u32; 4],
        0x44 => values / set_values: [u32; 4],
        0x54 => cell / set_cell: [u32; 2],
        0x5C => lists / set_lists: [u32; 4],
        0xCC => department / set_department: *mut u8,
        0xF0 => averages / set_averages: [u32; 3],
        0xFC => sums / set_sums: [u32; 6],
        0x114 => developed / set_developed: u32,
        0x11C => average_garbage / set_average_garbage: u16,
        0x120 => capacities / set_capacities: [u32; 2],
        0x128 => recycling / set_recycling: u32,
        0x12C => uncollected_flag / set_uncollected_flag: u8,
        0x130 => uncollected / set_uncollected: u32,
        0x134 => month / set_month: [u32; 14],
        0x16C => previous / set_previous: [u32; 14],
        0x1A4 => export_capacity / set_export_capacity: u32,
        0x1A8 => decomposed / set_decomposed: u32,
        0x1AC => previous_decomposed / set_previous_decomposed: u32,
        0x1B0 => active_cells / set_active_cells: u32,
        0x1B4 => unused_and_cost / set_unused_and_cost: [u32; 4],
        0x1C8 => tracts / set_tracts: u32,
        0x1CC => iterator / set_iterator: [u8; 48],
        0x1FC => dirty / set_dirty: [u8; 4],
        0x21C => efficiency / set_efficiency: u32,
        0x220 => supply / set_supply: u32,
        0x224 => capacity / set_capacity: u32,
    }
}

unsafe fn word(this: *mut u8, offset: usize) -> u32 {
    // SAFETY: each offset names a readable persisted word in the original object.
    unsafe { read_unaligned(this.add(offset).cast()) }
}

unsafe fn put_word(this: *mut u8, stream: *mut u8, offset: usize, slot: usize) {
    // SAFETY: the original field and stream method accepting its raw word.
    unsafe { vcall!(stream, slot => fn(u32), word(this, offset)) };
}

unsafe fn put_pair(this: *mut u8, stream: *mut u8, offset: usize, floating: bool) {
    // SAFETY: each pair is a readable integer or floating counter, and the stream accepts two words.
    unsafe {
        let bits = if floating {
            F80::from_f64(read_unaligned(this.add(offset).cast()))
                .to_f64()
                .to_bits()
        } else {
            u64::from(word(this, offset)) | (u64::from(word(this, offset + 4)) << 32)
        };
        vcall!(stream, if floating { 0x34 } else { 0x28 } => fn(u32, u32), bits as u32, (bits >> 32) as u32);
    }
}

port! {
    /// Saves version four and all persisted fields in stream order. Intermediate failures never stop the
    /// writes. Success replaces AL only; upper EAX and EDX come from the final department-reference call.
    #[port(address = 0x006E_3380, owner = "pollution", name = "cSC4PollutionSimulator::Write")]
    pub unsafe extern "thiscall" fn fc4p_006e3380(this: *mut u8, stream: *mut u8) -> u64 {
        // SAFETY: the live serializable interface and output stream supplied by the game.
        unsafe {
            let view = SaveView::from_ptr(this);
            vcall!(stream, 0x1C => fn(u32), 4);
            for offset in (0x34..0x54).step_by(4) {
                write_reference(stream, word(this, offset) as *mut u8);
            }
            for offset in [0x54, 0x58] { put_word(this, stream, offset, 0x24); }
            for offset in [0x5C, 0x60, 0x64, 0x68] { write_list(stream, this.add(offset)); }
            for offset in [0xF0, 0xF4, 0xF8] { put_word(this, stream, offset, 0x24); }
            for offset in (0xFC..0x114).step_by(4) { put_word(this, stream, offset, 0x20); }
            put_word(this, stream, 0x114, 0x24);
            vcall!(stream, 0x1C => fn(u32), u32::from(view.average_garbage()));
            for offset in [0x120, 0x124] { put_word(this, stream, offset, 0x24); }
            put_word(this, stream, 0x128, 0x30);
            vcall!(stream, 0x14 => fn(u32), u32::from(view.uncollected_flag()));
            put_word(this, stream, 0x130, 0x24);
            for offset in [0x1B4, 0x1BC] { put_pair(this, stream, offset, false); }
            put_word(this, stream, 0x1AC, 0x24);
            for offset in (0x16C..0x1A4).step_by(8) { put_pair(this, stream, offset, offset == 0x184); }
            for offset in (0x134..0x16C).step_by(8) { put_pair(this, stream, offset, offset == 0x14C); }
            put_word(this, stream, 0x1A8, 0x24);
            put_word(this, stream, 0x1A4, 0x20);
            for offset in [0x1B0, 0x1C8] { put_word(this, stream, offset, 0x24); }
            write_iterator(stream, this.add(0x1CC));
            for offset in 0x1FC..0x200 { vcall!(stream, 0x14 => fn(u32), u32::from(*this.add(offset))); }
            put_word(this, stream, 0x21C, 0x30);
            for offset in [0x220, 0x224] { put_word(this, stream, offset, 0x20); }
            (write_reference(stream, view.department()) & !0xFF) | 1
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e3380::PORT];

#[cfg(test)]
mod tests;
