// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Travel strategy updates and the shared archive and vector helpers they use.

use super::{store, word};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// A grid archive's dimensions and row storage.
    struct GridArchive {
        0x1C => rows / set_rows: i32,
        0x20 => columns / set_columns: i32,
        0x34 => row_storage / set_row_storage: *mut u8,
    }
}

layout! {
    /// A vector of eight-byte records.
    struct PairVector {
        0x00 => begin / set_begin: *mut u8,
        0x04 => end / set_end: *mut u8,
        0x08 => limit / set_limit: *mut u8,
    }
}

game_fn! { fn allocate_array(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free_array(pointer: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn read_archive_header(stream: *mut u8, destination: *mut u8) -> u32 = "C" 0x005D_9FA0; }
game_fn! { fn new_row_vector(this: *mut u8, allocator: *mut u32) -> *mut u8 = "thiscall" 0x0045_BB10; }
game_fn! { fn resize_rows(this: *mut u8, count: u32, value: *mut u8) -> u32 = "thiscall" 0x0046_2B20; }
game_fn! { fn move_words(first: *mut u8, last: *mut u8, destination: *mut u8) -> *mut u8 = "C" 0x0040_7A50; }
game_fn! { fn insert_words(this: *mut u8, position: *mut u8, count: u32, value: *const u32) -> u32 = "thiscall" 0x0042_3320; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn free(pointer: *mut u8) -> u32 = "C" 0x005E_5620; }
game_fn! { fn byte_vector_property(holder: *mut u8, id: u32, output: *mut u32) -> u32 = "C" 0x005F_FB40; }
game_fn! { fn int_vector_property(holder: *mut u8, id: u32, output: *mut u32) -> u32 = "C" 0x005F_FC00; }

game_data! { static resources: *mut u8 = 0x00B4_3CA8; }
game_data! { static ordinances: *mut u8 = 0x00B4_3D90; }

layout! {
    /// Four three-byte strategy groups; active ordinances adjust the last three groups.
    struct TravelTuning {
        0xC5C => strategies / set_strategies: [u8; 12],
    }
}

port! {
    /// Loads travel percentages, then applies each active ordinance's clamped adjustments.
    #[port(address = 0x0071_1810, owner = "traffic", name = "cSC4TrafficSimulator::UpdateTravelStrategies")]
    pub unsafe extern "thiscall" fn fc4p_00711810(this: *mut u8) -> u32 {
        unsafe {
            let key = [0x6534_284Au32, 0xE7E2_C2DB, 0xC913_3286];
            let mut resource = core::mem::MaybeUninit::<*mut u8>::uninit();
            let mut result = vcall!(*resources(), 0x0C => fn(*const u32, u32, *mut *mut u8, u32, u32) -> u32,
                key.as_ptr(), 0xA521_60F5, resource.as_mut_ptr(), 0, 0);
            if result as u8 != 0 {
                let resource = resource.assume_init();
                let holder = vcall!(resource, 0x1C => fn() -> *mut u8);
                for group in 0..4 {
                    let mut values = [0u32; 3];
                    let present = byte_vector_property(holder, 0x4953_E8A3 + group, values.as_mut_ptr());
                    if present as u8 != 0 {
                        for index in 0..3 {
                            this.wrapping_add(0xC5C + group as usize * 3 + index)
                                .write((values[0] as *const u8).wrapping_add(index).read());
                        }
                    }
                    if values[0] != 0 { free_array(values[0] as *mut u8); }
                }
                result = vcall!(resource, 8 => fn() -> u32);
            }
            let manager = *ordinances();
            if manager.is_null() { return result; }
            let count = vcall!(manager, 0x38 => fn() -> u32);
            if count == 0 { return count; }
            let count = vcall!(manager, 0x38 => fn() -> u32);
            let ids = allocate(count.wrapping_mul(4));
            let mut length = core::mem::MaybeUninit::<u32>::uninit();
            vcall!(manager, 0x3C => fn(*mut u8, *mut u32) -> u32, ids, length.as_mut_ptr());
            let length = length.assume_init();
            for index in 0..length {
                let id = read_unaligned(ids.wrapping_add(index.wrapping_mul(4) as usize).cast::<u32>());
                let ordinance = vcall!(manager, 0x50 => fn(u32) -> *mut u8, id);
                let mut values = [0u32; 3];
                if !ordinance.is_null() {
                    let holder = vcall!(ordinance, 0x40 => fn() -> *mut u8);
                    if !holder.is_null() {
                        let holder = vcall!(ordinance, 0x40 => fn() -> *mut u8);
                        if int_vector_property(holder, 0x8A61_2FEE, values.as_mut_ptr()) as u8 != 0 {
                            for index in 0..9 {
                                let destination = this.wrapping_add(0xC5F + index);
                                let adjustment = read_unaligned((values[0] as *const i32).wrapping_add(index));
                                let value = i32::from(destination.read()).wrapping_add(adjustment);
                                destination.write(value.clamp(0, 100) as u8);
                            }
                        }
                    }
                    if values[0] != 0 { free_array(values[0] as *mut u8); }
                }
            }
            free(ids)
        }
    }
}

port! {
    /// Reads two bytes and seven words of an archive record, returning the stream.
    #[port(address = 0x005D_A320, owner = "traffic", name = "ReadArchiveRecord")]
    pub unsafe extern "C" fn fc4p_005da320(stream: *mut u8, record: *mut u8) -> *mut u8 {
        unsafe {
            vcall!(stream, 0x10 => fn(*mut u8) -> u32, record);
            vcall!(stream, 0x10 => fn(*mut u8) -> u32, record.wrapping_add(1));
            for offset in (4..=0x1C).step_by(4) {
                vcall!(stream, 0x20 => fn(*mut u8) -> u32, record.wrapping_add(offset));
            }
        }
        stream
    }
}

port! {
    /// Constructs a vector by repeating a pair, retaining the unused third argument.
    #[port(address = 0x0071_0AD0, owner = "traffic", name = "FillPairVector")]
    pub unsafe extern "thiscall" fn fc4p_00710ad0(this: *mut u8, count: u32, value: *const u32, _allocator: u32) -> *mut u8 {
        unsafe {
            let vector = PairVector::from_ptr(this);
            vector.set_begin(core::ptr::null_mut());
            vector.set_end(core::ptr::null_mut());
            vector.set_limit(core::ptr::null_mut());
            let pointer = if count != 0 { allocate_array(count.wrapping_mul(8)) } else { core::ptr::null_mut() };
            vector.set_begin(pointer);
            vector.set_end(pointer);
            vector.set_limit(pointer.wrapping_add(count.wrapping_mul(8) as usize));
            let mut cursor = pointer;
            for _ in 0..count {
                if !cursor.is_null() {
                    write_unaligned(cursor.cast::<u32>(), read_unaligned(value));
                    write_unaligned(cursor.wrapping_add(4).cast::<u32>(), read_unaligned(value.wrapping_add(1)));
                }
                cursor = cursor.wrapping_add(8);
            }
            vector.set_end(cursor);
        }
        this
    }
}

port! {
    /// Reads a grid and retains the row cursor in the caller's argument word.
    #[port(address = 0x0046_5EE0, owner = "traffic", name = "ReadGridArchive", naked)]
    pub unsafe extern "C" fn fc4p_00465ee0() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 4", sym read_grid);
    }
}

unsafe extern "fastcall" fn read_grid(this: *mut u8, frame: *mut u32) -> u32 {
    unsafe {
        let stream = read_unaligned(frame.add(1)) as *mut u8;
        let grid = GridArchive::from_ptr(this);
        let mut version = core::mem::MaybeUninit::<u16>::uninit();
        let result = vcall!(stream, 0x1C => fn(*mut u16) -> u32, version.as_mut_ptr());
        if version.assume_init() != 1 {
            return result & !255;
        }
        read_archive_header(stream, this.wrapping_add(4));
        for (offset, slot) in [
            (0xC, 0x24),
            (0x10, 0x24),
            (0x14, 0x20),
            (0x18, 0x20),
            (0x1C, 0x20),
            (0x20, 0x20),
            (0x24, 0x30),
            (0x28, 0x30),
            (0x2C, 0x30),
            (0x30, 0x30),
        ] {
            vcall!(stream, slot => fn(*mut u8) -> u32, this.wrapping_add(offset));
        }
        let rows = grid.rows();
        let mut temporary = core::mem::MaybeUninit::<[u32; 3]>::uninit();
        let temporary_pointer = temporary.as_mut_ptr().cast::<u8>();
        new_row_vector(temporary_pointer, frame.add(1));
        resize_rows(this.wrapping_add(0x34), rows as u32, temporary_pointer);
        let allocation = word(temporary_pointer, 0) as *mut u8;
        if !allocation.is_null() {
            free_array(allocation);
        }
        let mut row = 0i32;
        let mut offset = 0u32;
        if grid.rows() > 0 {
            write_unaligned(frame.add(1), offset);
            loop {
                let vector = grid.row_storage().wrapping_add(offset as usize);
                let columns = grid.columns() as u32;
                let begin = word(vector, 0);
                let end = word(vector, 4);
                let length = (end.wrapping_sub(begin) as i32 >> 2) as u32;
                let zero = 0u32;
                if columns < length {
                    let new_end = move_words(
                        end as *mut u8,
                        end as *mut u8,
                        begin.wrapping_add(columns.wrapping_mul(4)) as *mut u8,
                    );
                    store(vector, 4, new_end as u32);
                } else {
                    insert_words(vector, end as *mut u8, columns.wrapping_sub(length), &zero);
                }
                let mut column = 0i32;
                while column < grid.columns() {
                    let vector = grid.row_storage().wrapping_add(offset as usize);
                    let cell = word(vector, 0).wrapping_add((column as u32).wrapping_mul(4)) as *mut u8;
                    vcall!(stream, 0x24 => fn(*mut u8) -> u32, cell);
                    column = column.wrapping_add(1);
                }
                row = row.wrapping_add(1);
                offset = offset.wrapping_add(12);
                write_unaligned(frame.add(1), offset);
                if row >= grid.rows() {
                    break;
                }
            }
        }
        (row as u32 & !255) | 1
    }
}

pub const PORTS: &[Port] = &[
    fc4p_005da320::PORT,
    fc4p_00710ad0::PORT,
    fc4p_00711810::PORT,
];
