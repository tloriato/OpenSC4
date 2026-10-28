// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Copy(u32, usize, usize, usize, usize),
    Preview(usize),
    Remove(usize, usize, u32, u32),
    Release(usize),
    Erase(usize, usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    buffer: usize,
    destination: usize,
    source_swap: Option<(usize, usize, usize)>,
    manager_swap: Option<(usize, usize)>,
    tree_swap: Option<(usize, usize)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        let destination = s.destination as *mut u8;
        assert_eq!(
            unsafe { [read::<u32>(destination, 0), read(destination, 4), read(destination, 8)] },
            [0; 3]
        );
        if let Some((p, begin, end)) = s.source_swap.take() {
            unsafe {
                write(p as *mut u8, 0, begin as u32);
                write(p as *mut u8, 4, end as u32);
            }
        }
        s.buffer as *mut u8
    })
}
fn copy(shift: u32, begin: *mut u8, end: *mut u8, buffer: *mut u8, scratch: *mut *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Copy(
            shift,
            begin as usize,
            end as usize,
            buffer as usize,
            unsafe { *scratch } as usize,
        ));
        let d = s.destination as *mut u8;
        assert_eq!(unsafe { read::<usize>(d, 0) }, buffer as usize);
        assert_eq!(unsafe { read::<usize>(d, 4) }, buffer as usize);
    });
    let count = (end as usize).wrapping_sub(begin as usize);
    if count != 0 {
        unsafe {
            core::ptr::copy_nonoverlapping(begin, buffer, count);
        }
    }
    unsafe {
        *scratch = core::ptr::null_mut();
    }
    buffer.wrapping_add(count)
}
extern "C" fn copy_large(begin: *mut u8, end: *mut u8, buffer: *mut u8, scratch: *mut *mut u8) -> *mut u8 {
    copy(5, begin, end, buffer, scratch)
}
extern "C" fn copy_small(begin: *mut u8, end: *mut u8, buffer: *mut u8, scratch: *mut *mut u8) -> *mut u8 {
    copy(4, begin, end, buffer, scratch)
}
extern "thiscall" fn preview(this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Preview(this as usize)));
    0x1234_5678
}
extern "thiscall" fn remove(this: *mut u8, occupant: *mut u8, a: u32, b: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Remove(this as usize, occupant as usize, a, b));
        if let Some((location, replacement)) = s.manager_swap.take() {
            unsafe {
                (location as *mut usize).write(replacement);
            }
        }
    });
}
extern "thiscall" fn release(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this as usize)));
}
extern "thiscall" fn erase(this: *mut u8, root: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Erase(this as usize, root as usize));
        if let Some((location, replacement)) = s.tree_swap.take() {
            unsafe {
                (location as *mut usize).write(replacement);
            }
        }
    });
}
#[test]
fn record_vectors_copy_payload_and_preserve_source_and_destination_guards() {
    let _a = fake(0x0090_CF54, allocate as *const () as usize);
    let _b = fake(0x0062_E970, copy_large as *const () as usize);
    let _c = fake(0x0062_4E60, copy_small as *const () as usize);
    for (shift, function) in [
        (
            5u32,
            fc4p_00624ea0 as unsafe extern "thiscall" fn(*mut u8, *mut u8) -> *mut u8,
        ),
        (4, fc4p_00625190),
    ] {
        for count in [0usize, 1, 3] {
            let mut source_bytes = [0u8; 128];
            for (i, b) in source_bytes.iter_mut().enumerate() {
                *b = i as u8;
            }
            let mut buffer = [0xA5u8; 160];
            let mut source = [
                source_bytes.as_mut_ptr() as u32,
                unsafe { source_bytes.as_mut_ptr().add(count << shift) } as u32,
                0xDEAD_BEEF,
            ];
            let source_before = source;
            let mut destination = [0xC3C3_C3C3u32; 5];
            let d = unsafe { destination.as_mut_ptr().add(1).cast::<u8>() };
            let b = unsafe { buffer.as_mut_ptr().add(16) };
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    destination: d as usize,
                    buffer: b as usize,
                    ..State::default()
                }
            });
            assert_eq!(unsafe { function(d, source.as_mut_ptr().cast()) }, d);
            let pointer = if count == 0 { 0 } else { b as u32 };
            assert_eq!(
                destination,
                [
                    0xC3C3_C3C3,
                    pointer,
                    pointer + (count << shift) as u32,
                    pointer + (count << shift) as u32,
                    0xC3C3_C3C3
                ]
            );
            assert_eq!(source, source_before);
            assert_eq!(&buffer[16..16 + (count << shift)], &source_bytes[..count << shift]);
            assert!(buffer[..16]
                .iter()
                .chain(buffer[16 + (count << shift)..].iter())
                .all(|&b| b == 0xA5));
            let mut expected = vec![];
            if count != 0 {
                expected.push(Call::Allocate((count as u32) << shift));
            }
            expected.push(Call::Copy(
                shift,
                source[0] as usize,
                source[1] as usize,
                pointer as usize,
                source.as_ptr() as usize,
            ));
            assert_eq!(calls(), expected);
        }
    }
}
#[test]
fn vector_copy_reloads_source_range_after_allocation_and_keeps_previous_capacity() {
    let _a = fake(0x0090_CF54, allocate as *const () as usize);
    let _b = fake(0x0062_4E60, copy_small as *const () as usize);
    let mut bytes = [17u8; 64];
    let mut buffer = [0u8; 64];
    let mut destination = [0u32; 3];
    let d = destination.as_mut_ptr().cast::<u8>();
    let mut source = [
        bytes.as_mut_ptr() as u32,
        unsafe { bytes.as_mut_ptr().add(48) } as u32,
        0,
    ];
    let begin = unsafe { bytes.as_mut_ptr().add(16) };
    let end = unsafe { bytes.as_mut_ptr().add(32) };
    let b = buffer.as_mut_ptr();
    STATE.with(|s| {
        *s.borrow_mut() = State {
            destination: d as usize,
            buffer: b as usize,
            source_swap: Some((source.as_mut_ptr() as usize, begin as usize, end as usize)),
            ..State::default()
        }
    });
    unsafe {
        fc4p_00625190(d, source.as_mut_ptr().cast());
    }
    assert_eq!(destination, [b as u32, b as u32 + 16, b as u32 + 48]);
    assert_eq!(&buffer[..16], &[17; 16]);
    assert_eq!(&buffer[16..], &[0; 48]);
    assert_eq!(
        calls(),
        [
            Call::Allocate(48),
            Call::Copy(4, begin as usize, end as usize, b as usize, source.as_ptr() as usize)
        ]
    );
}
#[test]
fn retained_occupants_clear_preview_remove_release_in_order_then_reset_live_header() {
    let _a = fake(0x0062_08D0, preview as *const () as usize);
    let _b = fake(0x0046_76A0, erase as *const () as usize);
    let mut table = [0usize; 22];
    table[2] = release as *const () as usize;
    table[21] = remove as *const () as usize;
    let mut manager = [table.as_ptr() as usize];
    let m = manager.as_mut_ptr().cast::<u8>();
    let mut global = m;
    let _g = fake_data(0x00B4_3D0C, (&mut global as *mut *mut u8).cast());
    for count in [0usize, 1, 6] {
        for recorded_count in [0u32, 42] {
            let mut tool = Object::new(0x360);
            let p = tool.ptr();
            let mut head = [0u32; 5];
            let h = head.as_mut_ptr() as u32;
            let mut nodes = vec![[0u32; 5]; count];
            let mut objects = vec![[table.as_ptr() as usize]; count];
            for i in 0..count {
                nodes[i][1] = if i == 0 { h } else { nodes[i - 1].as_ptr() as u32 };
                nodes[i][3] = if i + 1 == count {
                    0
                } else {
                    nodes[i + 1].as_ptr() as u32
                };
                nodes[i][4] = objects[i].as_mut_ptr() as u32;
            }
            head[1] = if count == 0 { 0 } else { nodes[0].as_ptr() as u32 };
            head[2] = if count == 0 { h } else { nodes[0].as_ptr() as u32 };
            head[3] = if count == 0 {
                h
            } else {
                nodes[count - 1].as_ptr() as u32
            };
            let root = head[1];
            let mut replacement = [0xA5A5_A5A5u32; 5];
            let r = replacement.as_mut_ptr() as u32;
            unsafe {
                write(p, 0x1F0, 0x1234_5678u32);
                write(p, 0x228, h);
                write(p, 0x22C, recorded_count);
            }
            global = m;
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    manager_swap: Some((&mut global as *mut *mut u8 as usize, 0xDEAD_BEEF)),
                    tree_swap: Some((p as usize + 0x228, r as usize)),
                    ..State::default()
                }
            });
            assert_eq!(unsafe { fc4p_00625580(p) }, if recorded_count == 0 { 0 } else { r });
            let mut expected = vec![Call::Preview(0x1234_5678)];
            for obj in &objects {
                expected.push(Call::Remove(m as usize, obj.as_ptr() as usize, 1, 0));
                expected.push(Call::Release(obj.as_ptr() as usize));
            }
            if recorded_count != 0 {
                expected.push(Call::Erase(p as usize + 0x228, root as usize));
                assert_eq!(replacement, [0xA5A5_A5A5, 0, r, r, 0xA5A5_A5A5]);
            }
            assert_eq!(calls(), expected);
            assert_eq!(unsafe { read::<u32>(p, 0x22C) }, 0);
        }
    }
}
