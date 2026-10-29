// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Version 41 traffic archive, including working containers and both statistics snapshots.

use super::monthly_queries::{frame_entry, unsigned_division};
use super::*;

layout! {
    /// Offsets from the serializable interface, eight bytes after the primary interface.
    pub struct TrafficArchive {
        0x44 => byte_rows / set_byte_rows: *mut u8,
        0x48 => byte_rows_end / set_byte_rows_end: *mut u8,
        0xB0 => pools / set_pools: *mut u8,
        0xFC => ferries / set_ferries: *mut u8,
        0x15C => references / set_references: *mut u32,
        0x160 => references_end / set_references_end: *mut u32,
    }
}
game_fn! { fn write_reference(stream: *mut u8, object: u32) -> u32 = "C" 0x005D_99A0; }
game_fn! { fn write_bool(stream: *mut u8, value: u32) -> u32 = "C" 0x005D_9FD0; }
game_fn! { fn write_pointer_range(stream: *mut u8, range: u32) -> u32 = "C" 0x005D_A480; }
game_fn! { fn write_word_range(stream: *mut u8, range: u32) -> u32 = "C" 0x005D_A390; }
game_fn! { fn write_bytes(stream: *mut u8, vector: *mut u8) -> u32 = "C" 0x005D_ADA0; }

struct Writer {
    stream: *mut u8,
    eax: u32,
}
impl Writer {
    unsafe fn value(&mut self, slot: usize, value: u32) {
        unsafe {
            self.eax = vcall!(self.stream, slot => fn(u32) -> u32, value);
        }
    }
    unsafe fn field(&mut self, p: *mut u8, offset: usize, slot: usize) {
        unsafe {
            self.value(slot, word(p, offset));
        }
    }
    unsafe fn byte(&mut self, p: *mut u8, offset: usize) {
        unsafe {
            self.value(0x14, u32::from(p.wrapping_add(offset).read()));
        }
    }
    unsafe fn short(&mut self, p: *mut u8, offset: usize, slot: usize) {
        unsafe {
            self.value(slot, u32::from(p.wrapping_add(offset).cast::<u16>().read_unaligned()));
        }
    }
    unsafe fn boolean(&mut self, p: *mut u8, offset: usize) {
        unsafe {
            self.eax = write_bool(self.stream, u32::from(p.wrapping_add(offset).read()));
        }
    }
    unsafe fn reference(&mut self, p: *mut u8, offset: usize) {
        unsafe {
            self.eax = write_reference(self.stream, word(p, offset));
        }
    }
    unsafe fn fields(&mut self, p: *mut u8, fields: &[(usize, usize)]) {
        for &(offset, slot) in fields {
            unsafe {
                self.field(p, offset, slot);
            }
        }
    }
}

unsafe fn first_bucket(this: *mut u8, begin: usize, end: usize) -> *mut u8 {
    unsafe {
        let buckets = word(this, begin) as *mut u32;
        let count = (word(this, end).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
        let mut i = 0u32;
        while i < count {
            let node = buckets.wrapping_add(i as usize).read_unaligned() as *mut u8;
            if !node.is_null() {
                return node;
            }
            i = i.wrapping_add(1);
        }
        core::ptr::null_mut()
    }
}

unsafe fn next_bucket(this: *mut u8, begin: usize, end: usize, node: *mut u8, short: bool) -> *mut u8 {
    unsafe {
        let next = word(node, 0) as *mut u8;
        if !next.is_null() {
            return next;
        }
        let buckets = word(this, begin) as *mut u32;
        let count = (word(this, end).wrapping_sub(buckets as u32) as i32 >> 2) as u32;
        let key = if short {
            u32::from(node.add(4).cast::<u16>().read_unaligned())
        } else {
            word(node, 4)
        };
        let mut i = unsigned_division(key, count).1;
        loop {
            i = i.wrapping_add(1);
            if i >= count {
                return core::ptr::null_mut();
            }
            let node = (word(this, begin) as *mut u32)
                .wrapping_add(i as usize)
                .read_unaligned() as *mut u8;
            if !node.is_null() {
                return node;
            }
        }
    }
}

unsafe fn short_set(w: &mut Writer, this: *mut u8, begin: usize, end: usize) {
    unsafe {
        let mut node = first_bucket(this, begin, end);
        while !node.is_null() {
            w.short(node, 4, 0x1C);
            node = next_bucket(this, begin, end, node, true);
        }
    }
}

unsafe fn successor(mut node: *mut u8) -> *mut u8 {
    unsafe {
        let mut parent = word(node, 12) as *mut u8;
        if !parent.is_null() {
            node = parent;
            parent = word(node, 8) as *mut u8;
            while !parent.is_null() {
                node = parent;
                parent = word(node, 8) as *mut u8;
            }
        } else {
            parent = word(node, 4) as *mut u8;
            while node == word(parent, 12) as *mut u8 {
                node = parent;
                parent = word(parent, 4) as *mut u8;
            }
            if word(node, 12) as *mut u8 != parent {
                node = parent;
            }
        }
        node
    }
}

unsafe fn list_len(head: *mut u8) -> u32 {
    unsafe {
        let mut node = word(head, 0) as *mut u8;
        let mut count = 0u32;
        while node != head {
            node = word(node, 0) as *mut u8;
            count = count.wrapping_add(1);
        }
        count
    }
}

unsafe fn statistics(w: &mut Writer, p: *mut u8, arg: *mut u32) {
    unsafe {
        for i in 0..13 {
            let record = p.add(i * 20);
            w.fields(record, &[(0, 0x30), (4, 0x24), (8, 0x20), (12, 0x20), (16, 0x30)]);
        }
        w.fields(
            p,
            &[
                (0x104, 0x30),
                (0x108, 0x24),
                (0x10C, 0x20),
                (0x110, 0x20),
                (0x114, 0x30),
            ],
        );
        for i in 0..15 {
            w.short(p, 0x118 + i * 2, 0x1C);
        }
        w.value(0x20, 12);
        for i in 0..12 {
            let record = p.add(0x138 + i * 20);
            w.short(record, 0, 0x1C);
            w.fields(record, &[(4, 0x30), (8, 0x20), (12, 0x20), (16, 0x30)]);
        }
        w.fields(p, &[(0x228, 0x30), (0x22C, 0x30)]);
        w.boolean(p, 0x230);
        w.field(p, 0x234, 0x30);
        w.byte(p, 0x238);
        arg.write(arg.read().wrapping_add(1));
    }
}

unsafe fn ferry(w: &mut Writer, terminal: *mut u8) {
    unsafe {
        for offset in [8, 9, 10, 11, 12, 13, 16] {
            w.byte(terminal, offset);
        }
        for offset in [0x14, 0x20, 0x22] {
            w.short(terminal, offset, 0x1C);
        }
        w.byte(terminal, 0x24);
        w.byte(terminal, 0x25);
        w.value(0x24, list_len(word(terminal, 0x18) as *mut u8));
        let head = word(terminal, 0x18) as *mut u8;
        let mut link = word(head, 0) as *mut u8;
        while link != head {
            w.field(link, 0x10, 0x30);
            w.short(link, 0x0A, 0x18);
            w.short(link, 0x0C, 0x18);
            w.short(link, 8, 0x1C);
            w.eax = write_bytes(w.stream, link.add(0x14));
            for offset in [0x20, 0x24, 0x22, 0x26] {
                w.short(link, offset, 0x1C);
            }
            link = word(link, 0) as *mut u8;
        }
        w.value(0x24, list_len(word(terminal, 0x1C) as *mut u8));
        let head = word(terminal, 0x1C) as *mut u8;
        let mut link = word(head, 0) as *mut u8;
        while link != head {
            w.field(link, 0x10, 0x30);
            for offset in [8, 10, 12, 14] {
                w.short(link, offset, 0x18);
            }
            w.eax = write_bytes(w.stream, link.add(0x14));
            w.eax = write_bytes(w.stream, link.add(0x20));
            for offset in [0x2C, 0x30, 0x2E, 0x32] {
                w.short(link, offset, 0x1C);
            }
            link = word(link, 0) as *mut u8;
        }
    }
}

frame_entry!(
    0x0071_2400,
    fc4p_00712400,
    "cSC4TrafficSimulator::Write",
    write_archive,
    4
);
unsafe extern "fastcall" fn write_archive(this: *mut u8, out: *mut Registers, arg: *mut u32) {
    unsafe {
        let mut w = Writer {
            stream: arg.read() as *mut u8,
            eax: out.read().eax,
        };
        let sim = TrafficArchive::from_ptr(this);
        w.value(0x1C, 41);
        for value in [13, 1, 13, 9] {
            w.value(0x20, value);
        }
        for offset in [0x28, 0x2C, 0x30, 0x34, 0x3C, 0x40] {
            w.reference(this, offset);
        }
        let rows = (sim.byte_rows_end() as u32).wrapping_sub(sim.byte_rows() as u32) as i32 / 12;
        let columns = word(sim.byte_rows(), 4).wrapping_sub(word(sim.byte_rows(), 0));
        w.value(0x24, rows as u32);
        w.value(0x24, columns);
        if rows != 0 {
            arg.write(0);
            let mut remaining = rows as u32;
            loop {
                let mut z = 0u32;
                while z < columns {
                    let row = word(sim.byte_rows(), arg.read() as usize) as *mut u8;
                    w.byte(row, z as usize);
                    z = z.wrapping_add(1);
                }
                arg.write(arg.read().wrapping_add(12));
                remaining = remaining.wrapping_sub(1);
                if remaining == 0 {
                    break;
                }
            }
        }
        let count = (sim.references_end() as u32).wrapping_sub(sim.references() as u32) as i32 >> 2;
        w.value(0x24, count as u32);
        for i in 0..count as u32 {
            w.eax = write_reference(w.stream, sim.references().wrapping_add(i as usize).read_unaligned());
        }
        w.reference(this, 0x168);
        w.reference(this, 0x16C);
        w.field(this, 0x180, 0x24);
        let mut node = first_bucket(this, 0x174, 0x178);
        while !node.is_null() {
            w.field(node, 4, 0x24);
            let mut p = node.add(8);
            arg.write(4);
            loop {
                for _ in 0..4 {
                    w.short(p, 0, 0x1C);
                    p = p.add(2);
                }
                arg.write(arg.read().wrapping_sub(1));
                if arg.read() == 0 {
                    break;
                }
            }
            w.short(node, 0x28, 0x1C);
            let mut p = node.add(0x2A);
            arg.write(4);
            loop {
                for _ in 0..4 {
                    w.short(p, 0, 0x1C);
                    p = p.add(2);
                }
                arg.write(arg.read().wrapping_sub(1));
                if arg.read() == 0 {
                    break;
                }
            }
            node = next_bucket(this, 0x174, 0x178, node, false);
        }
        for i in 0..3 {
            w.field(this, 0x1B8 + i * 4, 0x24);
            w.field(this, 0x1C4 + i * 4, 0x24);
            w.field(this, 0x1D0 + i * 4, 0x24);
        }
        w.reference(this, 0x6F8);
        w.reference(this, 0x6FC);
        w.field(this, 0x700, 0x24);
        w.field(this, 0x714, 0x24);
        let mut node = first_bucket(this, 0x708, 0x70C);
        while !node.is_null() {
            w.field(node, 4, 0x24);
            w.reference(node, 8);
            w.fields(node, &[(12, 0x30), (16, 0x30), (20, 0x30)]);
            w.boolean(node, 0x19);
            w.byte(node, 0x1A);
            w.byte(node, 0x1B);
            w.fields(node, &[(0x1C, 0x30), (0x20, 0x30)]);
            let count = word(node, 0x28).wrapping_sub(word(node, 0x24)) as i32 / 10;
            w.value(0x24, count as u32);
            let mut offset = 0u32;
            for _ in 0..count as u32 {
                let record = (word(node, 0x24) as *mut u8).wrapping_add(offset as usize);
                w.boolean(record, 0);
                arg.write(0);
                loop {
                    w.boolean(record, arg.read().wrapping_add(1) as usize);
                    arg.write(arg.read().wrapping_add(1));
                    if arg.read() as i32 >= 4 {
                        break;
                    }
                }
                w.short(record, 6, 0x1C);
                w.short(record, 8, 0x1C);
                offset = offset.wrapping_add(10);
            }
            w.field(node, 0x34, 0x30);
            w.boolean(node, 0x38);
            node = next_bucket(this, 0x708, 0x70C, node, false);
        }
        w.reference(this, 0x71C);
        w.reference(this, 0x720);
        w.eax = vcall!(w.stream, 0x28 => fn(u32, u32) -> u32, word(this, 0x770), word(this, 0x774));
        w.fields(this, &[(0x778, 0x30), (0x77C, 0x30), (0x780, 0x30)]);
        for offset in [0x788, 0x790] {
            let value = F80::from_f64(this.add(offset).cast::<f64>().read_unaligned()).to_f64();
            w.eax = vcall!(w.stream, 0x34 => fn(f64) -> u32, value);
        }
        w.field(this, 0x798, 0x24);
        w.byte(this, 0x79C);
        w.fields(
            this,
            &[
                (0x7A0, 0x20),
                (0x7A4, 0x24),
                (0x7A8, 0x24),
                (0x7AC, 0x30),
                (0x7C0, 0x24),
            ],
        );
        short_set(&mut w, this, 0x7B4, 0x7B8);
        w.field(this, 0x7D4, 0x24);
        short_set(&mut w, this, 0x7C8, 0x7CC);
        w.fields(this, &[(0x7D8, 0x24), (0x7DC, 0x24), (0x7E0, 0x30), (0x7F4, 0x24)]);
        short_set(&mut w, this, 0x7E8, 0x7EC);
        w.field(this, 0x808, 0x24);
        short_set(&mut w, this, 0x7FC, 0x800);
        w.byte(this, 0x80C);
        w.eax = write_pointer_range(w.stream, word(this, 0x810));
        w.boolean(this, 0x814);
        w.fields(this, &[(0x818, 0x20), (0x81C, 0x20), (0x820, 0x20), (0x824, 0x20)]);
        w.boolean(this, 0x918);
        w.fields(this, &[(0x91C, 0x24), (0x920, 0x24)]);
        w.short(this, 0x904, 0x18);
        w.short(this, 0x906, 0x18);
        w.eax = write_word_range(w.stream, word(this, 0x908));
        w.boolean(this, 0x878);
        w.boolean(this, 0x861);
        w.fields(this, &[(0x85C, 0x30), (0x834, 0x20), (0x838, 0x20), (0x874, 0x24)]);
        for i in 0..4 {
            w.field(this, 0x84C + i * 4, 0x24);
        }
        w.byte(this, 0x860);
        w.field(this, 0x848, 0x24);
        for offset in [0x864, 0x868, 0x86C, 0x870] {
            w.field(this, offset, 0x20);
        }
        for i in 0..27 {
            w.field(this, 0x88C + i * 4, 0x24);
        }
        w.byte(this, 0x8F8);
        w.byte(this, 0x8F9);
        for offset in [0x90C, 0x910, 0x914] {
            w.field(this, offset, 0x20);
        }
        for offset in [0x924, 0x928, 0x92C, 0x930] {
            w.field(this, offset, 0x24);
        }
        for offset in [0x934, 0x938, 0x93C] {
            w.field(this, offset, 0x20);
        }
        for offset in (0x940..=0x964).step_by(4) {
            w.field(this, offset, 0x24);
        }
        arg.write(0);
        loop {
            let base = if arg.read() == 0 { 0x1DC } else { 0x438 };
            statistics(&mut w, this.add(base), arg);
            if arg.read() as i32 >= 2 {
                break;
            }
        }
        w.field(this, 0xD0, 0x24);
        let mut node = first_bucket(this, 0xC4, 0xC8);
        while !node.is_null() {
            w.short(node, 4, 0x1C);
            w.byte(node, 8);
            w.byte(node, 9);
            w.short(node, 10, 0x1C);
            w.short(node, 0x30, 0x1C);
            let mut offset = 12;
            arg.write(9);
            loop {
                w.field(node, offset, 0x30);
                offset += 4;
                arg.write(arg.read().wrapping_sub(1));
                if arg.read() == 0 {
                    break;
                }
            }
            node = next_bucket(this, 0xC4, 0xC8, node, true);
        }
        w.fields(
            this,
            &[
                (0x418, 0x30),
                (0x6E4, 0x30),
                (0x6E8, 0x24),
                (0x6EC, 0x30),
                (0x6F0, 0x24),
            ],
        );
        for i in 0..13 {
            w.field(this, 0x738 + i * 4, 0x24);
        }
        w.value(0x24, 4);
        for i in 0..4 {
            w.field(this, 0x1A8 + i * 4, 0x24);
        }
        w.field(this, 0xB4, 0x24);
        let head = sim.pools();
        let mut node = word(head, 8) as *mut u8;
        while node != head {
            w.short(node, 0x10, 0x1C);
            let pool = word(node, 0x14) as *mut u8;
            w.eax = vcall!(pool, 0x5C => fn(*mut u8) -> u32, w.stream);
            node = successor(node);
        }
        for i in 0..9 {
            w.field(word(this, 0x41C) as *mut u8, i * 4, 0x24);
            w.field(word(this, 0x428) as *mut u8, i * 4, 0x24);
        }
        w.field(this, 0x434, 0x24);
        w.value(0x24, list_len(sim.ferries()));
        let head = sim.ferries();
        let mut terminal = word(head, 0) as *mut u8;
        arg.write(head as u32);
        while terminal != arg.read() as *mut u8 {
            ferry(&mut w, terminal);
            terminal = word(terminal, 0) as *mut u8;
        }
        let result = virtual_registers(w.stream, 0x48, &[], w.eax, word(w.stream, 0));
        out.write(Registers {
            eax: u32::from(result.eax as u8 == 0),
            ..result
        });
    }
}

pub const PORTS: &[Port] = &[fc4p_00712400::PORT];

#[cfg(test)]
mod tests;
