// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Power-record field order and stream side effects on objects at their original offsets.

use super::*;
use crate::testing::{fake, Object};

#[derive(Debug, PartialEq, Eq)]
struct Call(usize, Vec<u32>);
#[repr(C)]
struct Stream {
    vt: *const usize,
    calls: Vec<Call>,
    bytes: Vec<u8>,
    sim: *mut u8,
    mutate: bool,
}
impl Stream {
    fn word(&mut self, slot: usize, value: u32) {
        self.calls.push(Call(slot, vec![value]));
        match slot {
            0x14 => self.bytes.push(value as u8),
            0x18 | 0x1C => self.bytes.extend_from_slice(&(value as u16).to_le_bytes()),
            _ => self.bytes.extend_from_slice(&value.to_le_bytes()),
        }
        if self.mutate {
            // Exercise repeated reads of the container pointer, dimensions, counts and scalar fields.
            unsafe {
                if slot == 0x24 && value == 2 && self.calls.len() < 25 {
                    let rows = self.sim.add(0x7C).cast::<*mut u8>().read_unaligned();
                    let second = rows.add(12).cast::<*mut u8>().read_unaligned();
                    rows.cast::<*mut u8>().write_unaligned(second);
                }
                if slot == 0x24 && self.calls.len() > 150 {
                    self.sim.add(0x2B0).cast::<u32>().write_unaligned(0);
                    self.sim.add(0x2C4).cast::<u32>().write_unaligned(0);
                }
                if slot == 0x14 {
                    self.sim.add(0x344).cast::<u32>().write_unaligned(0x1234_5678);
                }
            }
        }
    }
}
macro_rules! writer {
    ($name:ident, $slot:literal) => {
        extern "thiscall" fn $name(this: *mut Stream, value: u32) -> u64 {
            unsafe {
                (*this).word($slot, value);
            }
            0x1122_3344_AABB_CCDD
        }
    };
}
writer!(byte, 0x14);
writer!(short, 0x18);
writer!(version, 0x1C);
writer!(signed, 0x20);
writer!(unsigned, 0x24);
writer!(float, 0x30);
extern "thiscall" fn wide(this: *mut Stream, value: u64) {
    unsafe {
        (*this).calls.push(Call(0x28, vec![value as u32, (value >> 32) as u32]));
        (*this).bytes.extend_from_slice(&value.to_le_bytes());
    }
}
extern "C" fn reference(this: *mut Stream, object: *mut u8) -> u32 {
    unsafe {
        (*this).calls.push(Call(0x005D_99A0, vec![object as u32]));
        (*this).bytes.extend_from_slice(&(object as u32).to_le_bytes());
        if (*this).mutate {
            (*this).sim.add(0x30).cast::<u32>().write_unaligned(0x9876_5432);
        }
    }
    0
}
#[unsafe(naked)]
extern "thiscall" fn partial_unsigned(_this: *mut Stream, _value: u32) -> u64 {
    core::arch::naked_asm!("mov al, 0x77", "ret 4");
}

struct World {
    object: Object,
    rows: Box<[[u32; 3]; 2]>,
    _cells: Box<[[u16; 3]; 2]>,
    nodes: Box<[[u32; 4]; 2]>,
    poles: Box<[u32; 2]>,
    bitmaps: Box<[[u32; 2]; 4]>,
    vt: Box<[usize; 13]>,
    stream: Box<Stream>,
    _fake: crate::testing::Fake,
}
impl World {
    fn new() -> Self {
        let mut vt = Box::new([0; 13]);
        for (slot, f) in [
            (0x14, byte as *const ()),
            (0x18, short as *const ()),
            (0x1C, version as *const ()),
            (0x20, signed as *const ()),
            (0x24, unsigned as *const ()),
            (0x28, wide as *const ()),
            (0x30, float as *const ()),
        ] {
            vt[slot / 4] = f as usize;
        }
        let mut w = Self {
            object: Object::new(0x400),
            rows: Box::new([[0; 3]; 2]),
            _cells: Box::new([[0, 0x8000, 0xFFFF], [7, 8, 9]]),
            nodes: Box::new([[0; 4]; 2]),
            poles: Box::new([0x1234, 0x5678]),
            bitmaps: Box::new([[1, 2], [3, 4], [5, 6], [7, 8]]),
            stream: Box::new(Stream {
                vt: vt.as_ptr(),
                calls: vec![],
                bytes: vec![],
                sim: core::ptr::null_mut(),
                mutate: false,
            }),
            vt,
            _fake: fake(0x005D_99A0, reference as *const () as usize),
        };
        let p = w.serial();
        w.stream.sim = p;
        unsafe {
            for i in 0..0x350 / 4 {
                p.add(i * 4).cast::<u32>().write_unaligned(0xAB00_0000 + i as u32);
            }
            let save = SaveView::from_ptr(p);
            save.set_department(0x1111 as *mut u8);
            save.set_grid(0x2222 as *mut u8);
            save.set_cost_base(0x8765_4321_FEDC_BA98);
            for i in 0..2 {
                let begin = w._cells[i].as_mut_ptr() as u32;
                w.rows[i] = [begin, begin + 6, begin + 6];
            }
            save.set_rows(w.rows.as_mut_ptr().cast());
            save.set_rows_end(w.rows.as_mut_ptr().add(2).cast());
            let h = w.nodes[0].as_mut_ptr() as u32;
            let n = w.nodes[1].as_mut_ptr() as u32;
            w.nodes[0][0] = n;
            w.nodes[1] = [h, h, 0x10, 0x3333];
            save.set_plant_header(h as *mut u8);
            save.set_pole_count(2);
            save.set_broken_count(1);
            save.set_poles(w.poles.as_mut_ptr().cast());
            save.set_bitmap_words(2);
            for (i, offset) in [0x2BC, 0x2C0, 0x2CC, 0x2C8].into_iter().enumerate() {
                p.add(offset)
                    .cast::<*mut u32>()
                    .write_unaligned(w.bitmaps[i].as_mut_ptr());
            }
            for (offset, value) in [(0x2D0, 0x80), (0x328, 0xFF), (0x330, 0xA5), (0x331, 0x7F)] {
                *p.add(offset) = value;
            }
        }
        w
    }
    fn serial(&mut self) -> *mut u8 {
        unsafe { self.object.ptr().add(8) }
    }
    fn run(&mut self) -> u64 {
        unsafe { fc4p_006eb6a0(self.serial(), (&mut *self.stream as *mut Stream).cast()) }
    }
    fn expected(&mut self, populated: bool) -> Vec<Call> {
        let mut c = vec![Call(0x1C, vec![6]), Call(0x005D_99A0, vec![0x1111])];
        let p = self.serial();
        let scalar = |offset| unsafe { read_word(p, offset) };
        for o in [0x30, 0x34, 0x38] {
            c.push(Call(0x20, vec![scalar(o)]));
        }
        c.push(Call(0x30, vec![scalar(0x5C)]));
        for o in [0x60, 0x64, 0x68, 0x6C] {
            c.push(Call(0x20, vec![scalar(o)]));
        }
        c.push(Call(0x28, vec![0xFEDC_BA98, 0x8765_4321]));
        for o in [0x48, 0x4C, 0x50, 0x54, 0x58] {
            c.push(Call(0x20, vec![scalar(o)]));
        }
        c.extend([
            Call(0x005D_99A0, vec![0x2222]),
            Call(0x24, vec![if populated { 2 } else { 0 }]),
            Call(0x24, vec![3]),
        ]);
        if populated {
            for v in [0, 0x8000, 0xFFFF, 7, 8, 9] {
                c.push(Call(0x18, vec![v]));
            }
        }
        for i in 0..120 {
            c.push(Call(0x20, vec![0xAB00_0022 + i]));
        }
        for i in 0..10 {
            c.push(Call(0x20, vec![0xAB00_009A + i]));
        }
        c.push(Call(0x20, vec![scalar(0x2D4)]));
        for i in 0..10 {
            c.push(Call(0x20, vec![0xAB00_00B6 + i]));
        }
        if populated {
            c.extend([Call(0x24, vec![0x10]), Call(0x005D_99A0, vec![0x3333])]);
        }
        c.extend([Call(0x24, vec![if populated { 2 } else { 0 }]), Call(0x24, vec![1])]);
        if populated {
            c.extend([Call(0x005D_99A0, vec![0x1234]), Call(0x005D_99A0, vec![0x5678])]);
        }
        c.push(Call(0x24, vec![if populated { 2 } else { 0 }]));
        if populated {
            for i in 1..=8 {
                c.push(Call(0x24, vec![i]));
            }
        }
        c.extend([
            Call(0x14, vec![0x80]),
            Call(0x14, vec![0xFF]),
            Call(0x20, vec![scalar(0x32C)]),
            Call(0x14, vec![0xA5]),
            Call(0x14, vec![0x7F]),
            Call(0x24, vec![scalar(0x344)]),
        ]);
        c
    }
}

#[test]
fn populated_version_six_record_preserves_word_order_and_signed_cell_bits() {
    let mut w = World::new();
    let expected = w.expected(true);
    let before = unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x400).to_vec() };
    assert_eq!(w.run(), 0x1122_3344_AABB_CC01);
    assert_eq!(w.stream.calls, expected);
    let bytes: Vec<_> = expected
        .iter()
        .flat_map(|Call(slot, args)| {
            args.iter()
                .flat_map(|v| {
                    let width = match slot {
                        0x14 => 1,
                        0x18 | 0x1C => 2,
                        _ => 4,
                    };
                    v.to_le_bytes()[..width].to_vec()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(w.stream.bytes, bytes);
    assert_eq!(unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x400) }, before);
}

#[test]
fn empty_containers_still_write_all_fixed_fields_and_both_dimensions() {
    let mut w = World::new();
    unsafe {
        let save = SaveView::from_ptr(w.serial());
        save.set_rows_end(save.rows());
        save.set_pole_count(0);
        save.set_bitmap_words(0);
        let h = save.plant_header();
        h.cast::<*mut u8>().write_unaligned(h);
    }
    let expected = w.expected(false);
    assert_eq!(w.run() as u32 & 255, 1);
    assert_eq!(w.stream.calls, expected);
}

#[test]
fn stream_side_effects_are_visible_to_later_reads() {
    let mut w = World::new();
    w.stream.mutate = true;
    w.run();
    assert_eq!(w.stream.calls[2], Call(0x20, vec![0x9876_5432]));
    let cells: Vec<_> = w
        .stream
        .calls
        .iter()
        .filter(|Call(s, _)| *s == 0x18)
        .map(|Call(_, a)| a[0])
        .collect();
    assert_eq!(cells, [7, 8, 9, 7, 8, 9]);
    assert!(!w.stream.calls.contains(&Call(0x005D_99A0, vec![0x1234])));
    assert_eq!(w.stream.calls.last(), Some(&Call(0x24, vec![0x1234_5678])));
}

#[test]
fn final_writer_may_leave_upper_eax_and_edx_untouched() {
    let mut w = World::new();
    w.vt[0x24 / 4] = partial_unsigned as *const () as usize;
    unsafe {
        w.serial().add(0x344).cast::<u32>().write_unaligned(0xDEAD_BEEF);
    }
    let result = w.run();
    assert_eq!(result as u32, 0xDEAD_BE01);
    assert_eq!((result >> 32) as u32, w.vt.as_ptr() as u32);
}
