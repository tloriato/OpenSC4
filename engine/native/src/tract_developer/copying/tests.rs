// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::tract_developer::testing::{run, snapshot, EDX};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Dispose(u32, u32),
    Copy(u32, u32, u32),
    Set(u32, u32),
    Vector(u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static COPY_ROOT: RefCell<u32> = const { RefCell::new(0) };
    static SOURCE: RefCell<u32> = const { RefCell::new(0) };
}
fn log(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn dispose(this: u32, root: u32) -> u64 {
    log(Call::Dispose(this, root));
    0x1234_5678_AABB_CCDD
}
extern "thiscall" fn copy(this: u32, source: u32, parent: u32) -> u64 {
    log(Call::Copy(this, source, parent));
    // The source count is intentionally changed by the callee, before the assignment reads it.
    SOURCE.with(|s| unsafe {
        store(*s.borrow(), 4, 99);
    });
    (0x1122_3344u64 << 32) | COPY_ROOT.with(|p| u64::from(*p.borrow()))
}

#[test]
fn set_assignment_handles_self_empty_and_nonempty_source_and_destination() {
    let _dispose = fake(0x00704990, dispose as *const () as usize);
    let _copy = fake(0x00705380, copy as *const () as usize);
    for source_nonempty in [false, true] {
        for destination_nonempty in [false, true] {
            let mut objects: Vec<_> = (0..8).map(|_| Object::new(0x20)).collect();
            let p: Vec<_> = objects.iter_mut().map(|o| o.ptr() as u32).collect();
            let [destination, source, header, source_header, old_root, new_root, left, right] = p[..] else {
                unreachable!()
            };
            unsafe {
                store(destination, 0, header);
                store(destination, 4, u32::from(destination_nonempty));
                store(header, 4, old_root);
                store(header, 8, old_root);
                store(header, 0xC, old_root);
                store(source, 0, source_header);
                store(source, 4, 17);
                store(source_header, 4, if source_nonempty { 0x1234 } else { 0 });
                store(new_root, 8, left);
                store(new_root, 0xC, right);
            }
            COPY_ROOT.with(|p| *p.borrow_mut() = new_root);
            SOURCE.with(|p| *p.borrow_mut() = source);
            let dest_before = snapshot(destination as *mut u8, 0x20);
            let header_before = snapshot(header as *mut u8, 0x20);
            assert_eq!(
                run(0x00705950, Conv::Thiscall, destination, &[destination]),
                (destination, destination, EDX)
            );
            assert_eq!(snapshot(destination as *mut u8, 0x20), dest_before);
            assert_eq!(snapshot(header as *mut u8, 0x20), header_before);
            assert!(take().is_empty());
            let result = run(0x00705950, Conv::Thiscall, destination, &[source]);
            assert_eq!(result.0, destination);
            let mut calls = Vec::new();
            if destination_nonempty {
                calls.push(Call::Dispose(destination, old_root));
            }
            if source_nonempty {
                calls.push(Call::Copy(destination, 0x1234, header));
            }
            assert_eq!(take(), calls);
            let mut want_dest = dest_before;
            want_dest[4..8].copy_from_slice(&(if source_nonempty { 99u32 } else { 0 }).to_le_bytes());
            assert_eq!(snapshot(destination as *mut u8, 0x20), want_dest);
            let mut want_header = header_before;
            for (offset, value) in [
                (4usize, if source_nonempty { new_root } else { 0 }),
                (8, if source_nonempty { left } else { header }),
                (0xC, if source_nonempty { right } else { header }),
            ] {
                want_header[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(snapshot(header as *mut u8, 0x20), want_header);
            if source_nonempty {
                assert_eq!((result.1, result.2), (right, 99));
            } else {
                assert_eq!(result.1, header);
                assert_eq!(result.2, if destination_nonempty { header } else { EDX });
            }
        }
    }
}

extern "thiscall" fn assign_set(destination: u32, source: u32) -> u64 {
    log(Call::Set(destination, source));
    unsafe {
        let a = word(source, 0);
        let b = word(source, 4);
        store(destination, 0, a);
        store(destination, 4, b);
        store(source.wrapping_sub(0x18), 0x24, 0x0123_4567);
        store(source.wrapping_sub(0x18), 0x34, 0x7F80_0001);
    }
    0x1122_3344_AABB_CCDD
}
extern "thiscall" fn assign_vector(destination: u32, source: u32) -> u64 {
    log(Call::Vector(destination, source));
    unsafe {
        for offset in [0, 4, 8] {
            let value = word(source, offset);
            store(destination, offset, value);
        }
        // Change the last byte during the final callback, so the enclosing assignment must reread it.
        if CALLS.with(|calls| calls.borrow().len()) == 5 {
            *(source.wrapping_add(0xC) as *mut u8) = 0xE7;
        }
    }
    0x5566_7788_AABB_CCDD
}

#[test]
fn candidate_assignment_keeps_padding_and_defers_reads_until_after_helpers() {
    let _set = fake(0x00705950, assign_set as *const () as usize);
    let _vector = fake(0x0050DB90, assign_vector as *const () as usize);
    for self_assignment in [false, true] {
        let mut a = Object::new(0xA4);
        let mut b = Object::new(0xA4);
        let destination = a.ptr() as u32;
        let source = if self_assignment { destination } else { b.ptr() as u32 };
        unsafe {
            for i in 0..0xA4 {
                *(destination as *mut u8).add(i) = 0xA5;
                *(source as *mut u8).add(i) = i as u8;
            }
        }
        let before = snapshot(destination as *mut u8, 0xA4);
        let result = run(0x00707240, Conv::Thiscall, destination, &[source]);
        assert_eq!(result.0, destination);
        assert_eq!(result.2, 0x5566_7788);
        assert_eq!(result.1 as u8, 0xE7);
        assert_eq!(
            take(),
            [
                Call::Set(destination + 0x18, source + 0x18),
                Call::Vector(destination + 0x70, source + 0x70),
                Call::Vector(destination + 0x7C, source + 0x7C),
                Call::Vector(destination + 0x88, source + 0x88),
                Call::Vector(destination + 0x94, source + 0x94)
            ]
        );
        let mut expected = snapshot(source as *mut u8, 0xA4);
        for range in [0x16..0x18, 0x20..0x24, 0x6D..0x70, 0xA1..0xA4] {
            expected[range.clone()].copy_from_slice(&before[range]);
        }
        assert_eq!(snapshot(destination as *mut u8, 0xA4), expected);
        assert_eq!(unsafe { word(destination, 0x34) }, 0x7F80_0001);
    }
}
