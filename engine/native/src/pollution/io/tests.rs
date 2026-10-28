// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistence vectors from the core pollution record, using the original interfaces and field widths.

use super::*;
use crate::testing::{fake, GameFpu, Object};

#[derive(Debug, PartialEq)]
enum Call {
    Scalar(usize, Vec<u32>),
    Reference(u32),
    List(usize),
    Iterator(usize),
}

#[repr(C)]
struct Stream {
    vtable: *const usize,
    calls: Vec<Call>,
    serializable: *mut u8,
    mutate: bool,
}

unsafe fn scalar(this: *mut Stream, slot: usize, words: &[u32]) -> u64 {
    // SAFETY: the stream is owned by the test.
    unsafe { (*this).calls.push(Call::Scalar(slot, words.to_vec())) };
    0xAABB_CCDD_1234_5600
}

macro_rules! scalar_fake {
    ($name:ident, $slot:literal) => {
        extern "thiscall" fn $name(this: *mut Stream, value: u32) -> u64 {
            unsafe { scalar(this, $slot, &[value]) }
        }
    };
}
scalar_fake!(byte, 0x14);
scalar_fake!(short, 0x1C);
scalar_fake!(signed, 0x20);
scalar_fake!(unsigned, 0x24);
scalar_fake!(float, 0x30);

extern "thiscall" fn integer64(this: *mut Stream, low: u32, high: u32) -> u64 {
    unsafe { scalar(this, 0x28, &[low, high]) }
}
extern "thiscall" fn float64(this: *mut Stream, low: u32, high: u32) -> u64 {
    unsafe { scalar(this, 0x34, &[low, high]) }
}
extern "C" fn reference(this: *mut Stream, object: u32) -> u64 {
    unsafe {
        let stream = &mut *this;
        stream.calls.push(Call::Reference(object));
        if stream.mutate && stream.calls.len() == 2 {
            // The next reference must read the field after the preceding call.
            write_unaligned(stream.serializable.add(0x38).cast::<u32>(), 0x7654_3210);
        }
    }
    0xBEEF_1234_ABCD_EF00
}
extern "C" fn list(this: *mut Stream, value: *mut u8) {
    unsafe { (*this).calls.push(Call::List(value as usize)) };
}
extern "C" fn iterator(this: *mut Stream, value: *mut u8) {
    unsafe { (*this).calls.push(Call::Iterator(value as usize)) };
}

fn table() -> [usize; 14] {
    let mut table = [0; 14];
    for (slot, function) in [
        (0x14, byte as *const () as usize),
        (0x1C, short as *const () as usize),
        (0x20, signed as *const () as usize),
        (0x24, unsigned as *const () as usize),
        (0x28, integer64 as *const () as usize),
        (0x30, float as *const () as usize),
        (0x34, float64 as *const () as usize),
    ] {
        table[slot / 4] = function;
    }
    table
}

#[test]
fn version_four_record_order_widths_and_unconditional_success() {
    let _fpu = GameFpu::new();
    let _reference = fake(0x005D_99A0, reference as *const () as usize);
    let _list = fake(0x005D_AFF0, list as *const () as usize);
    let _iterator = fake(0x005D_AB70, iterator as *const () as usize);
    let table = table();
    let mut object = Object::new(0x240);
    let base = object.ptr();
    let this = unsafe { base.add(12) };
    let mut stream = Stream {
        vtable: table.as_ptr(),
        calls: vec![],
        serializable: this,
        mutate: false,
    };
    unsafe {
        for offset in (0..0x228).step_by(4) {
            write_unaligned(this.add(offset).cast::<u32>(), 0x1020_0000 + offset as u32);
        }
        SaveView::from_ptr(this).set_average_garbage(0xFFFF);
        SaveView::from_ptr(this).set_uncollected_flag(0xFE);
        SaveView::from_ptr(this).set_dirty([0, 2, 0x80, 0xFF]);
        write_unaligned(this.add(0x184).cast::<f64>(), -0.0);
        write_unaligned(this.add(0x14C).cast::<f64>(), f64::from_bits(0x7FF0_0000_0000_1234));
        let before = std::slice::from_raw_parts(base, 0x240).to_vec();
        assert_eq!(
            fc4p_006e3380(this, (&mut stream as *mut Stream).cast()),
            0xBEEF_1234_ABCD_EF01
        );
        assert_eq!(std::slice::from_raw_parts(base, 0x240), before);
    }
    let mut expected = vec![Call::Scalar(0x1C, vec![4])];
    for offset in [0x34, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C, 0x50] {
        expected.push(Call::Reference(unsafe { word(this, offset) }));
    }
    let fields: &[(usize, usize, usize)] = &[
        (0x24, 0x54, 1),
        (0x24, 0x58, 1),
        (0, 0x5C, 0),
        (0, 0x60, 0),
        (0, 0x64, 0),
        (0, 0x68, 0),
        (0x24, 0xF0, 1),
        (0x24, 0xF4, 1),
        (0x24, 0xF8, 1),
        (0x20, 0xFC, 1),
        (0x20, 0x100, 1),
        (0x20, 0x104, 1),
        (0x20, 0x108, 1),
        (0x20, 0x10C, 1),
        (0x20, 0x110, 1),
        (0x24, 0x114, 1),
        (0x1C, 0x11C, 1),
        (0x24, 0x120, 1),
        (0x24, 0x124, 1),
        (0x30, 0x128, 1),
        (0x14, 0x12C, 1),
        (0x24, 0x130, 1),
        (0x28, 0x1B4, 2),
        (0x28, 0x1BC, 2),
        (0x24, 0x1AC, 1),
        (0x28, 0x16C, 2),
        (0x28, 0x174, 2),
        (0x28, 0x17C, 2),
        (0x34, 0x184, 2),
        (0x28, 0x18C, 2),
        (0x28, 0x194, 2),
        (0x28, 0x19C, 2),
        (0x28, 0x134, 2),
        (0x28, 0x13C, 2),
        (0x28, 0x144, 2),
        (0x34, 0x14C, 2),
        (0x28, 0x154, 2),
        (0x28, 0x15C, 2),
        (0x28, 0x164, 2),
        (0x24, 0x1A8, 1),
        (0x20, 0x1A4, 1),
        (0x24, 0x1B0, 1),
        (0x24, 0x1C8, 1),
        (1, 0x1CC, 0),
        (0x14, 0x1FC, 1),
        (0x14, 0x1FD, 1),
        (0x14, 0x1FE, 1),
        (0x14, 0x1FF, 1),
        (0x30, 0x21C, 1),
        (0x20, 0x220, 1),
        (0x20, 0x224, 1),
    ];
    for &(slot, offset, count) in fields {
        if count == 0 {
            expected.push(if slot == 0 {
                Call::List(this as usize + offset)
            } else {
                Call::Iterator(this as usize + offset)
            });
        } else {
            let mut words: Vec<u32> = (0..count).map(|i| unsafe { word(this, offset + i * 4) }).collect();
            if slot == 0x14 {
                words[0] &= 0xFF;
            }
            if slot == 0x1C {
                words[0] &= 0xFFFF;
            }
            if offset == 0x14C {
                words = vec![0x1234, 0x7FF8_0000];
            }
            expected.push(Call::Scalar(slot, words));
        }
    }
    expected.push(Call::Reference(unsafe { word(this, 0xCC) }));
    assert_eq!(stream.calls, expected);
}

#[test]
fn reads_later_fields_after_stream_callbacks() {
    let _reference = fake(0x005D_99A0, reference as *const () as usize);
    let _list = fake(0x005D_AFF0, list as *const () as usize);
    let _iterator = fake(0x005D_AB70, iterator as *const () as usize);
    let mut object = Object::new(0x240);
    let this = unsafe { object.ptr().add(12) };
    let table = table();
    let mut stream = Stream {
        vtable: table.as_ptr(),
        calls: vec![],
        serializable: this,
        mutate: true,
    };
    unsafe { fc4p_006e3380(this, (&mut stream as *mut Stream).cast()) };
    assert_eq!(stream.calls[2], Call::Reference(0x7654_3210));
}
