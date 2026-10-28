// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

type Event = (u32, usize, Vec<u32>);
thread_local! {
    static LOG: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static STATE: RefCell<(usize, usize, usize)> = const { RefCell::new((0, 0, 0)) };
}
fn log(tag: u32, this: *mut u8, args: &[u32]) {
    LOG.with(|s| s.borrow_mut().push((tag, this as usize, args.to_vec())));
}
fn start(tool: *mut u8, other: *mut u8, result: *mut u8) {
    LOG.with(|s| s.borrow_mut().clear());
    STATE.with(|s| *s.borrow_mut() = (tool as usize, other as usize, result as usize));
}
extern "C" fn erase(last: *mut u8, end: *mut u8, first: *mut u8, _scratch: *mut u8, count: u32) -> u64 {
    assert_eq!(last, end);
    assert_eq!(count, 0);
    log(1, first, &[end as u32]);
    registers(STATE.with(|s| s.borrow().2 as u32), 0x1234_5678)
}
extern "thiscall" fn clear_world(this: *mut u8) {
    log(2, this, &[]);
}
extern "thiscall" fn cleanup(this: *mut u8) -> u64 {
    log(3, this, &[]);
    registers(0x1234_789A, 0xABC0_0123)
}
extern "thiscall" fn cleanup_other(this: *mut u8) -> u64 {
    log(4, this, &[]);
    registers(0xFFFF_2222, 0x9999_7777)
}
extern "thiscall" fn failure(this: *mut u8) {
    log(5, this, &[]);
    unsafe {
        assert_eq!(read::<u32>(this, 0x200), u32::MAX);
        assert_eq!(read::<u32>(this, 0x204), 0xBF80_0000);
        assert_eq!(read::<u32>(this, 0x11C), read::<u32>(this, 0x118));
        assert_eq!(read::<u32>(this, 0x64), read::<u32>(this, 0x60));
        assert_eq!(read::<u32>(this, 0x1F8), read::<u32>(this, 0x1F4));
        write(this, 0x20C, 0u32);
    }
}
extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    log(6, this, &[key]);
    STATE.with(|s| s.borrow().1 as *mut u8)
}
extern "thiscall" fn update(this: *mut u8) {
    log(7, this, &[]);
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    log(8, this, &[]);
    registers(0x1234_56FF, 0xCAFE_BABE)
}
extern "thiscall" fn deregister(this: *mut u8, target: *mut u8, id: u32) -> u64 {
    log(9, this, &[target as u32, id]);
    registers(0x8888_7777, 0x1111_2222)
}
extern "thiscall" fn remove_preview(this: *mut u8, preview: *mut u8) -> u64 {
    log(10, this, &[preview as u32]);
    registers(0x5555_6666, 0x3333_4444)
}
extern "thiscall" fn stop_preview(this: *mut u8) -> u64 {
    log(11, this, &[]);
    registers(0xBEEF_2222, 0xFACE_3333)
}

#[test]
fn cell_record_clear_calls_range_helper_even_when_empty_and_retains_its_result() {
    let _f = fake(0x0061_43E0, erase as *const () as usize);
    for end in [0x1000u32, 0x1030] {
        let mut vector = [0x1000u32, end, 0x2000, 0xAA55_AA55];
        start(core::ptr::null_mut(), core::ptr::null_mut(), 0x1004 as *mut u8);
        let result = unsafe { fc4p_00626130(vector.as_mut_ptr().cast()) };
        assert_eq!(result, registers(0x1004, 0x1234_5678));
        assert_eq!(vector, [0x1000, 0x1004, 0x2000, 0xAA55_AA55]);
        LOG.with(|s| assert_eq!(*s.borrow(), [(1, 0x1000, vec![end])]));
    }
}

#[test]
fn neighbor_connections_promote_each_lane_only_for_an_existing_two_and_clear_at_end() {
    let _f = fake(0x0063_3220, lookup as *const () as usize);
    let _g = fake(0x0064_7830, update as *const () as usize);
    let _h = fake(0x0062_40D0, erase as *const () as usize);
    for existing in [0, 0x0303_0303, 2, 0x200, 0x20000, 0x2000000] {
        let mut tool = Object::new(0x368);
        let t = tool.ptr();
        let mut cell = [0xA5A5_A5A5u32; 30];
        let cp = cell.as_mut_ptr().cast::<u8>();
        cell[8] = existing;
        let mut records = [
            0x1000u32,
            0x0103_0101,
            91,
            92,
            93,
            94,
            0x2000,
            0x0000_0001,
            95,
            96,
            97,
            98,
        ];
        let begin = records.as_mut_ptr().cast::<u8>();
        unsafe {
            write(t, 0x35C, 2u32);
            write(t, 0x100, begin);
            write(t, 0x104, begin.add(48));
        }
        start(t, cp, begin);
        assert_eq!(unsafe { fc4p_00626eb0(t) }, registers(begin as u32, 0x1234_5678));
        let promoted = existing != 0 && existing != 0x0303_0303;
        let expected = if promoted { 0x0202_0202 } else { 0x0103_0101 };
        assert_eq!(records[1], expected);
        assert_eq!(records[7], if promoted { 2 } else { 1 });
        assert_eq!(cell[8], existing | expected | records[7]);
        assert_eq!(&cell[..8], &[0xA5A5_A5A5; 8]);
        assert_eq!(&cell[9..], &[0xA5A5_A5A5; 21]);
        assert_eq!([records[2], records[3], records[4], records[5]], [91, 92, 93, 94]);
        LOG.with(|s| {
            assert_eq!(
                *s.borrow(),
                [
                    (6, t as usize, vec![0x1000]),
                    (7, cp as usize, vec![]),
                    (6, t as usize, vec![0x2000]),
                    (7, cp as usize, vec![]),
                    (1, begin as usize, vec![begin as u32 + 48]),
                ]
            )
        });
        assert_eq!(unsafe { read::<*mut u8>(t, 0x104) }, begin);
        unsafe {
            write(t, 0x104, begin);
        }
        start(t, cp, begin);
        unsafe {
            fc4p_00626eb0(t);
        }
        LOG.with(|s| assert_eq!(*s.borrow(), [(1, begin as usize, vec![begin as u32])]));
    }
}

#[test]
fn reset_clears_draft_state_in_order_without_touching_vector_storage_or_other_fields() {
    let _f = fake(0x0064_7B20, clear_world as *const () as usize);
    let _g = fake(0x0062_4070, erase as *const () as usize);
    let mut table = [0usize; 18];
    table[0x44 / 4] = cleanup as *const () as usize;
    table[0x40 / 4] = cleanup_other as *const () as usize;
    table[0x34 / 4] = failure as *const () as usize;
    let mut tool = [0xA5A5_A5A5u32; 0x368 / 4];
    tool[0] = table.as_ptr() as u32;
    for offset in [0x60, 0x118, 0x1F4, 0xF0] {
        tool[offset / 4] = offset as u32 + 0x4000;
    }
    let mut expected = tool;
    for (dest, source) in [(0x64, 0x60), (0x11C, 0x118), (0x1F8, 0x1F4)] {
        expected[dest / 4] = expected[source / 4];
    }
    expected[0x200 / 4] = u32::MAX;
    expected[0x204 / 4] = 0xBF80_0000;
    expected[0x20C / 4] = 0;
    expected[0xF4 / 4] = 0x4321;
    let t = tool.as_mut_ptr().cast::<u8>();
    start(t, core::ptr::null_mut(), 0x4321 as *mut u8);
    assert_eq!(unsafe { fc4p_00626f90(t) }, registers(0x4321, 0x1234_5678));
    assert_eq!(tool, expected);
    LOG.with(|s| {
        assert_eq!(
            *s.borrow(),
            [
                (2, t as usize + 0x1C, vec![]),
                (3, t as usize, vec![]),
                (4, t as usize, vec![]),
                (5, t as usize, vec![]),
                (1, 0x40F0, vec![0xA5A5_A5A5]),
            ]
        )
    });
}

#[test]
fn shutdown_inactive_keeps_entry_registers_and_all_bytes() {
    let mut tool = [0xA5A5_A5A5u32; 0x368 / 4];
    tool[0x18 / 4] &= !0xFF;
    let expected = tool;
    let mut c = RawCall::new(
        Conv::Thiscall,
        fc4p_00626d30 as *const () as usize,
        tool.as_mut_ptr() as u32,
        &[],
    );
    c.eax = 0x1234_5678;
    c.edx = 0x89AB_CDEF;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!((c.eax, c.edx), (0x1234_5601, 0x89AB_CDEF));
    assert_eq!(tool, expected);
}

#[test]
fn shutdown_full_and_sparse_release_services_preview_and_preserve_upper_result_bytes() {
    let _f = fake(0x0061_FBC0, stop_preview as *const () as usize);
    for full in [false, true] {
        let mut table = [0usize; 18];
        table[0x44 / 4] = cleanup as *const () as usize;
        table[0x40 / 4] = cleanup_other as *const () as usize;
        let mut service_table = [0usize; 34];
        service_table[2] = release as *const () as usize;
        service_table[6] = deregister as *const () as usize;
        service_table[0x84 / 4] = remove_preview as *const () as usize;
        let mut service = [service_table.as_ptr() as usize];
        let s = service.as_mut_ptr().cast::<u8>();
        let mut server = if full { s } else { core::ptr::null_mut() };
        let mut manager = s;
        let _g = fake_data(0x00B4_3CCC, (&mut server as *mut *mut u8).cast());
        let _h = fake_data(0x00B4_3DD0, (&mut manager as *mut *mut u8).cast());
        let mut tool = Object::new(0x368);
        let t = tool.ptr();
        unsafe {
            write(t, 0, table.as_ptr());
            write(t, 0x18, 1u8);
            for offset in [0x4C, 0x48, 0x44, 0x364, 0x1F0] {
                write(t, offset, if full { s } else { core::ptr::null_mut() });
            }
            for offset in [0x1F4, 0x118, 0x60] {
                write(t, offset, offset as u32 + 1);
            }
            start(t, s, core::ptr::null_mut());
            let mut c = RawCall::new(Conv::Thiscall, fc4p_00626d30 as *const () as usize, t as u32, &[]);
            call_raw(&mut c);
            assert_eq!(
                (c.eax, c.edx),
                if full {
                    (0x1234_5601, 0xCAFE_BABE)
                } else {
                    (1, 0x9999_7777)
                }
            );
            for offset in [0x4C, 0x48, 0x44, 0x364, 0x1F0] {
                assert_eq!(read::<u32>(t, offset), 0);
            }
            for (end, begin) in [(0x1F8, 0x1F4), (0x11C, 0x118), (0x64, 0x60)] {
                assert_eq!(read::<u32>(t, end), read::<u32>(t, begin));
            }
            assert_eq!(read::<u8>(t, 0x18), 0);
        }
        let mut expected = vec![];
        if full {
            expected.push((9, s as usize, vec![t as u32 + 4, 0xA6B7_9602]));
        }
        expected.extend([(3, t as usize, vec![]), (4, t as usize, vec![])]);
        if full {
            expected.extend((0..4).map(|_| (8, s as usize, vec![])));
            expected.extend([
                (10, s as usize, vec![s as u32]),
                (11, s as usize, vec![]),
                (8, s as usize, vec![]),
            ]);
        }
        LOG.with(|s| assert_eq!(*s.borrow(), expected));
    }
}
