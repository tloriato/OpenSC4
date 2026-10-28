// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::queue::QueueIterator;
use crate::testing::{fake, Object};

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Version(u32),
    Vertices(u32),
    FloatBits(u32),
    Count(u32),
    Reference(u32),
    Byte(u32),
    Boolean(u32),
    Error,
}

#[repr(C)]
struct Stream {
    vtable: *const usize,
    bytes: Vec<u8>,
    calls: Vec<Call>,
    developer: *mut u8,
    mutate: bool,
    error: u32,
}

const RETURNED: u64 = 0x1234_5678_89AB_CDEF;

unsafe extern "thiscall" fn version(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Version(value));
    stream.bytes.extend_from_slice(&(value as u16).to_le_bytes());
    RETURNED
}
unsafe extern "thiscall" fn vertices(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Vertices(value));
    stream.bytes.extend_from_slice(&value.to_le_bytes());
    if stream.mutate {
        unsafe { LotDeveloper::from_ptr(stream.developer) }.set_extent_x_bits(0xFFC1_2345);
    }
    RETURNED
}
unsafe extern "thiscall" fn float_bits(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::FloatBits(value));
    stream.bytes.extend_from_slice(&value.to_le_bytes());
    RETURNED
}
unsafe extern "thiscall" fn count(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Count(value));
    stream.bytes.extend_from_slice(&value.to_le_bytes());
    RETURNED
}
unsafe extern "thiscall" fn byte(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Byte(value));
    stream.bytes.push(value as u8);
    RETURNED
}
unsafe extern "C" fn boolean(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Boolean(value));
    stream.bytes.push(u8::from(value != 0));
    RETURNED
}
unsafe extern "C" fn reference(this: *mut Stream, value: u32) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Reference(value));
    stream.bytes.extend_from_slice(&value.to_le_bytes());
    stream.bytes.extend_from_slice(&0x87DD_A3A9u32.to_le_bytes());
    if stream.mutate && stream.calls.iter().filter(|c| matches!(c, Call::Reference(_))).count() == 1 {
        let developer = unsafe { LotDeveloper::from_ptr(stream.developer) };
        let mut begin = developer.night_begin();
        unsafe { begin.advance() };
        developer.set_night_begin(begin);
        developer.set_night_end(begin);
    }
    RETURNED
}
unsafe extern "thiscall" fn error(this: *mut Stream) -> u64 {
    let stream = unsafe { &mut *this };
    stream.calls.push(Call::Error);
    (u64::from(0xDEAD_BEEFu32) << 32) | u64::from(stream.error)
}

fn table() -> Vec<usize> {
    let mut table = vec![0; 0x4C / 4];
    for (offset, function) in [
        (0x1C, version as *const () as usize),
        (0x20, vertices as *const () as usize),
        (0x30, float_bits as *const () as usize),
        (0x24, count as *const () as usize),
        (0x14, byte as *const () as usize),
        (0x48, error as *const () as usize),
    ] {
        table[offset / 4] = function;
    }
    table
}

fn iterator(block: &mut [u32; 32], map: &mut [*mut u32; 2], index: usize) -> QueueIterator {
    let first = block.as_mut_ptr();
    QueueIterator {
        current: first.wrapping_add(index),
        first,
        last: first.wrapping_add(32),
        node: map.as_mut_ptr(),
    }
}

#[test]
fn empty_save_matches_core_version_one_record_and_uses_only_final_error_byte() {
    let _boolean = fake(0x005D_9FD0, boolean as *const () as usize);
    let table = table();
    for error in [0, 0x1234_5600, 1, 0xFFFF_FFFF] {
        let mut object = Object::new(0x108);
        let this = object.ptr();
        let developer = unsafe { LotDeveloper::from_ptr(this) };
        developer.set_vertices_x(65);
        developer.set_extent_x_bits(1024f32.to_bits());
        developer.set_extent_z_bits(1024f32.to_bits());
        let mut block = [0u32; 32];
        let mut map = [block.as_mut_ptr(); 2];
        let empty = iterator(&mut block, &mut map, 3);
        developer.set_night_begin(empty);
        developer.set_night_end(empty);
        developer.set_night_cursor(empty);
        let mut stream = Stream {
            vtable: table.as_ptr(),
            bytes: vec![],
            calls: vec![],
            developer: this,
            mutate: false,
            error,
        };
        let returned = unsafe { fc4p_006c66f0(this.add(8), 0x5566_7788, (&mut stream as *mut Stream).cast()) };
        assert_eq!(returned, 0xDEAD_BEEF_0000_0000 | u64::from(error as u8 == 0));
        assert_eq!(
            stream.bytes,
            [1, 0, 65, 0, 0, 0, 0, 0, 128, 68, 0, 0, 128, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            stream.calls,
            [
                Call::Version(1),
                Call::Vertices(65),
                Call::FloatBits(1024f32.to_bits()),
                Call::FloatBits(1024f32.to_bits()),
                Call::Count(0),
                Call::Count(0),
                Call::Byte(0),
                Call::Boolean(0),
                Call::Error
            ]
        );
    }
}

#[test]
fn save_crosses_blocks_snapshots_reference_range_and_rereads_cursor_geometry() {
    let _boolean = fake(0x005D_9FD0, boolean as *const () as usize);
    let _reference = fake(0x005D_99A0, reference as *const () as usize);
    let table = table();
    let mut object = Object::new(0x108);
    let this = object.ptr();
    let developer = unsafe { LotDeveloper::from_ptr(this) };
    developer.set_vertices_x(-7);
    developer.set_extent_x_bits(0x7F80_0001);
    developer.set_extent_z_bits(0x8000_0000);
    developer.set_night_mode(0xFE);
    developer.set_lit(0x80);
    let mut first_block = [0u32; 32];
    let mut last_block = [0u32; 32];
    first_block[31] = 0x1234;
    last_block[0] = 0x5678;
    let mut map = [first_block.as_mut_ptr(), last_block.as_mut_ptr()];
    let begin = iterator(&mut first_block, &mut map, 31);
    let mut end = iterator(&mut last_block, &mut map, 1);
    end.node = map.as_mut_ptr().wrapping_add(1);
    developer.set_night_begin(begin);
    developer.set_night_end(end);
    developer.set_night_cursor(end);
    let mut stream = Stream {
        vtable: table.as_ptr(),
        bytes: vec![],
        calls: vec![],
        developer: this,
        mutate: true,
        error: 0,
    };
    unsafe { fc4p_006c66f0(this.add(8), 0, (&mut stream as *mut Stream).cast()) };
    assert_eq!(
        stream.calls,
        [
            Call::Version(1),
            Call::Vertices((-7i32) as u32),
            Call::FloatBits(0xFFC1_2345),
            Call::FloatBits(0x8000_0000),
            Call::Count(2),
            Call::Reference(0x1234),
            Call::Reference(0x5678),
            Call::Count(1),
            Call::Byte(0xFE),
            Call::Boolean(0x80),
            Call::Error
        ]
    );
    assert_eq!(stream.bytes.len(), 40);
    assert_eq!(
        &stream.bytes[14..34],
        &[2, 0, 0, 0, 0x34, 0x12, 0, 0, 0xA9, 0xA3, 0xDD, 0x87, 0x78, 0x56, 0, 0, 0xA9, 0xA3, 0xDD, 0x87]
    );
    assert_eq!(&stream.bytes[34..], &[1, 0, 0, 0, 0xFE, 1]);
}
