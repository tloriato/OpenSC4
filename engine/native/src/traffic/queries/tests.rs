// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

thread_local! {
    static CALLS: RefCell<Vec<(usize, u32)>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<*mut *mut u8> = const { RefCell::new(core::ptr::null_mut()) };
    static LANGUAGE: RefCell<(*mut u8, *mut u8, u32)> = const { RefCell::new((core::ptr::null_mut(), core::ptr::null_mut(), 1)) };
}
fn take() -> Vec<(usize, u32)> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}
extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    OUTPUT.with(|o| {
        let out = *o.borrow();
        if !out.is_null() {
            CALLS.with(|c| c.borrow_mut().push((unsafe { out.read() } as usize, 4)));
        }
    });
    CALLS.with(|c| c.borrow_mut().push((this as usize, 4)));
    0xABCD_1234_9876_5400
}
extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((this as usize, 0x10)));
    // Swap the table to verify the destructor is looked up after Shutdown.
    unsafe {
        store(this, 0, word(this, 0x20));
    }
    0x1234
}
extern "thiscall" fn destroy(this: *mut u8, flags: u32) -> u64 {
    CALLS.with(|c| c.borrow_mut().push((this as usize, 0xAC + flags)));
    0x1122_3344_FFFF_FFFF
}

#[test]
fn getters_and_class_id_preserve_edx_and_do_not_write() {
    let mut o = Object::new(0xD00);
    let p = o.ptr();
    unsafe {
        store(p, 0x48, 0x1234_5678);
        store(p, 0x30, 0x8765_4321);
    }
    let before = unsafe { bytes(p, 0xD00) };
    for (f, want) in [
        (fc4p_00698cf0 as *const () as usize, 0x1234_5678),
        (fc4p_006aac50 as *const () as usize, 0x8765_4321),
        (fc4p_0070d7d0 as *const () as usize, 0x6990_C1AA),
    ] {
        let r = unsafe { invoke(f, p, &[], 0xDEAD_BEEF, 0x1357_9BDF, false) };
        assert_eq!((r.eax, r.edx), (want, 0x1357_9BDF));
    }
    assert_eq!(unsafe { bytes(p, 0xD00) }, before);
}

#[test]
fn directions_cover_equal_cells_large_upper_bytes_and_wrapped_deltas() {
    for (x, z, want) in [
        (-1, 0, 0),
        (0, -1, 1),
        (1, 0, 2),
        (0, 1, 3),
        (0, 0, -1),
        (1, 1, -1),
        (65536, -1, 1),
        (65535, 0, 0),
        (65537, 0, 2),
    ] {
        assert_eq!(unsafe { fc4p_0070d780(0, 0, x, z) }, want);
    }
    assert_eq!(unsafe { fc4p_0070d780(i32::MAX, 0, i32::MIN, 0) }, 2);
    for (a, b, c, d, want) in [
        (0, 0, -257, 0, 0xFFFF_FE00),
        (0, 0, 257, 0, 0x102),
        (5, 600, 5, 257, 0x201),
        (5, 257, 5, 257, 0x203),
        (5, 0, 5, -257, 0xFFFF_FC01),
    ] {
        assert_eq!(unsafe { fc4p_006d6000(a, b, c, d) }, want);
    }
}

#[test]
fn query_interfaces_write_before_addref_and_rejection_keeps_canary() {
    let vt = table(&[(4, add_ref as *const () as usize)]);
    let mut o = object(0xD00, &vt);
    let p = o.ptr();
    for (iid, offset) in [(1, 0), (0x6802_7F37, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
        let mut out = 0xDEAD_BEEFusize as *mut u8;
        OUTPUT.with(|o| *o.borrow_mut() = &mut out);
        take();
        let result = unsafe { fc4p_0070d7e0(p, 0x1234, iid, &mut out) };
        assert_eq!(out, p.wrapping_add(offset));
        assert_eq!(result, 0xABCD_1234_9876_5401);
        assert_eq!(take(), [(out as usize, 4), (p as usize, 4)]);
    }
    OUTPUT.with(|o| *o.borrow_mut() = core::ptr::null_mut());
    for iid in [0u32, 2, 0x4522_94AB, 0x6802_7F36, 0x6802_7F38, 0xFFFF_FFFF] {
        let mut out = 0xDEAD_BEEFusize as *mut u8;
        let result = unsafe { fc4p_0070d7e0(p, 0x1234_5678, iid, &mut out) };
        let eax = if iid > 0x6802_7F37 {
            iid
        } else {
            iid.wrapping_sub(0x4522_94AA)
        } & !255;
        assert_eq!(result, (0x1234_5678u64 << 32) | u64::from(eax));
        assert_eq!(out as usize, 0xDEAD_BEEF);
        assert!(take().is_empty());
    }
}

#[test]
fn release_uses_signed_count_and_rereads_the_table() {
    let a = table(&[(0x10, shutdown as *const () as usize)]);
    let b = table(&[(0xAC, destroy as *const () as usize)]);
    let mut o = object(0xD00, &a);
    let p = o.ptr();
    let s = unsafe { TrafficSim::from_ptr(p) };
    for count in [2, 3, i32::MAX] {
        s.set_references(count);
        let before = unsafe { bytes(p, 0xD00) };
        assert_eq!(
            unsafe { fc4p_0070e2f0(p, 0xAABB_CCDD) },
            (0xAABB_CCDDu64 << 32) | (count - 1) as u64
        );
        let mut want = before;
        want[0x10..0x14].copy_from_slice(&(count - 1).to_le_bytes());
        assert_eq!(unsafe { bytes(p, 0xD00) }, want);
        assert!(take().is_empty());
    }
    for count in [1, 0, -1, i32::MIN] {
        unsafe {
            store(p, 0, a.as_ptr() as u32);
            store(p, 0x20, b.as_ptr() as u32);
        }
        s.set_references(count);
        assert_eq!(unsafe { fc4p_0070e2f0(p, 0) }, 0x1122_3344_0000_0000);
        assert_eq!(s.references(), count);
        assert_eq!(take(), [(p as usize, 0x10), (p as usize, 0xAD)]);
    }
}

extern "C" fn language_service_fake() -> *mut u8 {
    LANGUAGE.with(|l| l.borrow().0)
}
extern "thiscall" fn get_language(this: *mut u8, flags: u32) -> *mut u8 {
    CALLS.with(|c| c.borrow_mut().push((this as usize, 0x98 + flags)));
    LANGUAGE.with(|l| l.borrow().1)
}
extern "thiscall" fn drive(this: *mut u8) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((this as usize, 0x64)));
    LANGUAGE.with(|l| l.borrow().2)
}

#[test]
fn ts17_highway_return_tile_all_edges_driving_sides_and_byte_flags() {
    let _fake = fake(0x0091_3D7A, language_service_fake as *const () as usize);
    let manager_vt = table(&[(0x98, get_language as *const () as usize)]);
    let language_vt = table(&[(0x64, drive as *const () as usize)]);
    let mut manager = object(4, &manager_vt);
    let mut language = object(4, &language_vt);
    let mut o = Object::new(0xD00);
    let s = unsafe { TrafficSim::from_ptr(o.ptr()) };
    s.set_max_x(63);
    s.set_max_z(63);
    for (x0, z0, reverse, right, want) in [
        (-1, 40, 0, true, (-1, 41)),
        (-1, 40, 1, true, (-1, 39)),
        (-1, 40, 0, false, (-1, 39)),
        (-1, 40, 1, false, (-1, 41)),
        (64, 10, 0, true, (64, 9)),
        (10, -1, 0, true, (9, -1)),
        (10, 64, 0, true, (11, 64)),
        (-1, -1, 0, true, (-1, 0)),
        (5, 5, 0, true, (5, 5)),
        (-1, i32::MAX, 256, true, (-1, i32::MIN)),
    ] {
        LANGUAGE.with(|l| *l.borrow_mut() = (manager.ptr(), language.ptr(), if right { 0xFF01 } else { 2 }));
        take();
        let (mut x, mut z) = (x0, z0);
        unsafe {
            fc4p_0070dd30(o.ptr(), &mut x, &mut z, reverse);
        }
        assert_eq!((x, z), want);
        assert_eq!(
            take(),
            [(manager.ptr() as usize, 0x98), (language.ptr() as usize, 0x64)]
        );
    }
    LANGUAGE.with(|l| *l.borrow_mut() = (manager.ptr(), core::ptr::null_mut(), 0));
    let (mut x, mut z) = (-1, 0);
    unsafe {
        fc4p_0070dd30(o.ptr(), &mut x, &mut z, 0);
    }
    assert_eq!((x, z), (-1, 1));
}

#[test]
fn inner_cell_rejects_corners_clamps_edges_and_keeps_registers() {
    let mut o = Object::new(0xD00);
    let p = o.ptr();
    let s = unsafe { TrafficSim::from_ptr(p) };
    s.set_max_x(63);
    s.set_max_z(31);
    for (x, z, side, want) in [
        (-1, 8, 0u32, (0, 8)),
        (64, 8, 2, (63, 8)),
        (5, -1, 1, (5, 0)),
        (5, 32, 3, (5, 31)),
        (0, 0, u32::MAX, (123, 456)),
        (-1, -1, u32::MAX, (123, 456)),
        (64, 32, u32::MAX, (123, 456)),
    ] {
        let (mut ox, mut oz) = (123, 456);
        let args = [
            x as u32,
            z as u32,
            (&mut ox as *mut i32) as u32,
            (&mut oz as *mut i32) as u32,
        ];
        let r = unsafe { invoke(fc4p_0070ddd0 as *const () as usize, p, &args, 0, 0xCAFE_BABE, false) };
        assert_eq!(r.eax, side);
        assert_eq!((ox, oz), want);
        assert_eq!(r.ecx, if side == 3 { 31 } else { p as u32 });
        assert_eq!(r.edx, if side == u32::MAX { 0xCAFE_BABE } else { args[2] });
    }
    // Output aliasing preserves write order, including the final clamp.
    let mut both = 99;
    unsafe {
        invoke(
            fc4p_0070ddd0 as *const () as usize,
            p,
            &[
                u32::MAX,
                5,
                (&mut both as *mut i32) as u32,
                (&mut both as *mut i32) as u32,
            ],
            0,
            0,
            false,
        );
    }
    assert_eq!(both, 0);
}

#[test]
fn edge_connection_offsets_use_x_extent_and_do_not_dereference_records() {
    let mut o = Object::new(0xD00);
    let s = unsafe { TrafficSim::from_ptr(o.ptr()) };
    s.set_max_cell_x(63);
    s.set_west_connections(0x1000usize as *mut u8);
    s.set_east_connections(0x2000usize as *mut u8);
    s.set_north_connections(0x3000usize as *mut u8);
    s.set_south_connections(0x4000usize as *mut u8);
    for (x, z, want) in [
        (-1, 5, 0x1028),
        (64, 5, 0x2028),
        (7, -1, 0x3038),
        (7, 4, 0x4038),
        (-1, -1, 0xFF8),
    ] {
        assert_eq!(unsafe { fc4p_0070f970(o.ptr(), x, z) } as usize, want);
    }
}

#[test]
fn ferry_searches_return_first_match_and_preserve_empty_list_registers() {
    let mut terminal = Object::new(0x20);
    let mut header = Object::new(16);
    let mut a = Object::new(16);
    let mut b = Object::new(16);
    unsafe {
        store(terminal.ptr(), 0x10, header.ptr() as u32);
        store(terminal.ptr(), 0x14, header.ptr() as u32);
        store(header.ptr(), 0, a.ptr() as u32);
        store(a.ptr(), 0, b.ptr() as u32);
        store(b.ptr(), 0, header.ptr() as u32);
        store(a.ptr(), 8, 0x2222_1111);
        store(a.ptr(), 12, 0x4444_3333);
        store(b.ptr(), 8, 0x5555_3333);
        store(b.ptr(), 12, 0x7777_6666);
        let link = fc4p_0070f9c0 as *const () as usize;
        for (id, want, node) in [
            (0xABCD_1111, a.ptr().add(8), a.ptr()),
            (0x3333, b.ptr().add(8), b.ptr()),
            (0x9999, core::ptr::null_mut(), header.ptr()),
        ] {
            let r = invoke(link, core::ptr::null_mut(), &[terminal.ptr() as u32, id], 0, 0, false);
            assert_eq!(
                r,
                Registers {
                    eax: want as u32,
                    ecx: node as u32,
                    edx: header.ptr() as u32
                }
            );
        }
        let neighbor = fc4p_0070f9f0 as *const () as usize;
        for (x, z, want) in [
            (0x1111, 0x2222, a.ptr().add(8)),
            (0x3333, 0x4444, a.ptr().add(8)),
            (0x6666, 0x7777, b.ptr().add(8)),
            (0x1111, 0x5555, core::ptr::null_mut()),
        ] {
            let r = invoke(
                neighbor,
                core::ptr::null_mut(),
                &[terminal.ptr() as u32, x, z],
                0,
                0xABCD_9876,
                false,
            );
            assert_eq!((r.eax, r.edx), (want as u32, 0xABCD_0000 | x));
        }
        store(header.ptr(), 0, header.ptr() as u32);
        assert_eq!(
            invoke(link, core::ptr::null_mut(), &[terminal.ptr() as u32, 1], 0, 0, false),
            Registers {
                eax: 0,
                ecx: header.ptr() as u32,
                edx: header.ptr() as u32
            }
        );
        assert_eq!(
            invoke(
                neighbor,
                core::ptr::null_mut(),
                &[terminal.ptr() as u32, 1, 2],
                0,
                0xABCD_9876,
                false
            )
            .edx,
            0xABCD_9876
        );
    }
}
