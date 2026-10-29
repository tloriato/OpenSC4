// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Constructor defaults from the core state tests, plus native member lifetimes and call boundaries.

use super::*;
use crate::testing::{fake, fake_data, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, u32, Vec<u32>);
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<u32>,
    tool: u32,
    next_header: u32,
    change_highway: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
fn log(name: &'static str, this: *mut u8, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(name, this as u32, args.to_vec())));
}
fn take_calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn cache_new(this: *mut u8) {
    log("cache_new", this, &[]);
    unsafe {
        assert_eq!(read::<u32>(this.sub(0x1C), 0), 0x00AA_8748);
        assert_eq!(read::<u32>(this.sub(0x1C), 4), 0x00AA_8738);
        assert_eq!(read::<u8>(this.sub(0x1C), 0x18), 0);
        assert_eq!(read::<u32>(this.sub(0x1C), 0x44), 0xA5A5_A5A5);
        write(this, 0, 0x1010_2020u32);
    }
}
extern "thiscall" fn set_new(this: *mut u8, count: u32) {
    log("set_new", this, &[count]);
    unsafe {
        assert_eq!(read::<u32>(this, 0), 0xA5A5_A5A5);
        for offset in [4, 8, 12, 16] {
            assert_eq!(read::<u32>(this, offset), 0);
        }
        write(this, 0, 0x3030_4040u32);
    }
}
extern "thiscall" fn constraints_new(this: *mut u8) {
    log("constraints_new", this, &[]);
    unsafe { write(this, 0, 0x5050_6060u32) };
}
extern "C" fn allocate_header(size: u32) -> *mut u8 {
    log("allocate", core::ptr::null_mut(), &[size]);
    STATE.with(|s| s.borrow_mut().allocations.remove(0) as *mut u8)
}

#[test]
fn state_defaults_preserve_padding_and_later_initialization_fields_for_every_network_type() {
    let _a = fake(0x0064_8000, cache_new as *const () as usize);
    let _b = fake(0x0082_9BC0, set_new as *const () as usize);
    let _c = fake(0x0066_0810, constraints_new as *const () as usize);
    let _d = fake(0x0090_CF54, allocate_header as *const () as usize);
    let mut masks = [1u32, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
    let _m = fake_data(0x00AA_835C, masks.as_mut_ptr().cast());
    for network_type in 0..13u32 {
        reset();
        let mut bytes = [0xA5u8; 0x380];
        let p = bytes.as_mut_ptr();
        let mut headers = [[0xA5u8; 20]; 2];
        let h0 = headers[0].as_mut_ptr();
        let h1 = headers[1].as_mut_ptr();
        STATE.with(|s| s.borrow_mut().allocations = vec![h0 as u32, h1 as u32]);
        assert_eq!(unsafe { fc4p_0062c5f0(p, network_type) }, p);
        let mut expected = [0xA5u8; 0x380];
        let ep = expected.as_mut_ptr();
        unsafe {
            write(ep, 0, 0x00AA_8748u32);
            write(ep, 4, 0x00AA_8738u32);
            for offset in [
                8, 12, 16, 20, 0x44, 0x48, 0x4C, 0x54, 0x58, 0x5C, 0x60, 0x64, 0x68, 0x88, 0x8C, 0x90, 0x94, 0x98,
                0x9C, 0xA0, 0xA8, 0xAC, 0xB0, 0xB4, 0xBC, 0xC0, 0xC4, 0xC8, 0xCC, 0xD0, 0xD4, 0xD8, 0xDC, 0xE0, 0xE4,
                0xE8, 0xEC, 0xF0, 0xF4, 0xF8, 0xFC, 0x100, 0x104, 0x108, 0x10C, 0x110, 0x114, 0x118, 0x11C, 0x120,
                0x1F0, 0x1F4, 0x1F8, 0x1FC, 0x210, 0x218, 0x220, 0x224, 0x22C, 0x240, 0x244, 0x2B8, 0x2BC, 0x2C0,
                0x2E4, 0x2E8, 0x2EC, 0x2F0, 0x2F4, 0x2F8, 0x2FC, 0x300, 0x304, 0x330, 0x334, 0x338, 0x33C, 0x340,
                0x344, 0x348, 0x34C, 0x350, 0x358,
            ] {
                write(ep, offset, 0u32);
            }
            for offset in [0x18, 0x50, 0x53, 0x354] {
                write(ep, offset, 0u8);
            }
            for offset in [0x52, 0x234] {
                write(ep, offset, 1u8);
            }
            write(ep, 0x1C, 0x1010_2020u32);
            for offset in [0x84, 0xA4, 0xB8] {
                write(ep, offset, 0x3030_4040u32);
            }
            for offset in [0x124, 0x188] {
                write(ep, offset, 0x5050_6060u32);
            }
            write(ep, 0x1EC, !masks[network_type as usize] & 0x1F4Fu32);
            write(ep, 0x200, u32::MAX);
            write(ep, 0x204, 0xBF80_0000u32);
            write(ep, 0x208, 1u32);
            write(ep, 0x214, h0);
            write(ep, 0x228, h1);
            write(ep, 0x35C, network_type);
        }
        assert_eq!(bytes, expected);
        let tool = unsafe { NetworkTool::from_ptr(p) };
        assert_eq!(
            (tool.real(), tool.insertion_data(), tool.network_type()),
            (0, 1, network_type)
        );
        assert_eq!(
            take_calls(),
            [
                Call("cache_new", p as u32 + 0x1C, vec![]),
                Call("set_new", p as u32 + 0x84, vec![100]),
                Call("set_new", p as u32 + 0xA4, vec![100]),
                Call("set_new", p as u32 + 0xB8, vec![100]),
                Call("constraints_new", p as u32 + 0x124, vec![]),
                Call("constraints_new", p as u32 + 0x188, vec![]),
                Call("allocate", 0, vec![20]),
                Call("allocate", 0, vec![20]),
            ]
        );
        for (header, address) in headers.iter().zip([h0, h1]) {
            let mut expected = [0xA5u8; 20];
            expected[0] = 0;
            expected[4..8].fill(0);
            expected[8..12].copy_from_slice(&(address as u32).to_le_bytes());
            expected[12..16].copy_from_slice(&(address as u32).to_le_bytes());
            assert_eq!(header, &expected);
        }
    }
}

extern "thiscall" fn shutdown_fake(this: *mut u8) {
    log("shutdown", this, &[]);
    unsafe {
        assert_eq!(read::<u32>(this, 0), 0x00AA_8748);
        assert_eq!(read::<u32>(this, 4), 0x00AA_8738);
    }
}
extern "C" fn free_fake(p: *mut u8) -> u64 {
    log("free", core::ptr::null_mut(), &[p as u32]);
    registers(0x1234_ABCD, 0x5678_EFAB)
}
extern "thiscall" fn parameters_drop(this: *mut u8) {
    log("parameters_drop", this, &[]);
}
extern "thiscall" fn constraints_drop(this: *mut u8) {
    log("constraints_drop", this, &[]);
}
extern "thiscall" fn erase(this: *mut u8, root: *mut u8) {
    log("erase", this, &[root as u32]);
    STATE.with(|s| {
        let s = s.borrow();
        if s.next_header != 0 {
            unsafe { write(this, 0, s.next_header) };
        }
    });
}
extern "thiscall" fn preview_release(this: *mut u8) {
    log("release", this, &[]);
}
extern "C" fn nested_drop(first: *mut u8, last: *mut u8, _scratch: *mut u8) {
    log("nested_drop", core::ptr::null_mut(), &[first as u32, last as u32]);
}
extern "thiscall" fn intersections_drop(this: *mut u8) {
    log("intersections_drop", this, &[]);
}
extern "thiscall" fn clear(this: *mut u8) {
    log("clear", this, &[]);
}
extern "thiscall" fn cache_drop(this: *mut u8) -> u64 {
    log("cache_drop", this, &[]);
    registers(0xA1A2_A3A4, 0xB1B2_B3B4)
}
fn destructor_fakes() -> Vec<Fake> {
    [
        (0x0062_6D30, shutdown_fake as *const ()),
        (0x0090_CF63, free_fake as *const ()),
        (0x0062_4F70, parameters_drop as *const ()),
        (0x0046_76A0, erase as *const ()),
        (0x0066_02C0, constraints_drop as *const ()),
        (0x0062_C590, nested_drop as *const ()),
        (0x0062_BE40, intersections_drop as *const ()),
        (0x008E_BA60, clear as *const ()),
        (0x0064_7AE0, cache_drop as *const ()),
    ]
    .iter()
    .map(|&(address, entry)| fake(address, entry as usize))
    .collect()
}

#[test]
fn destruction_calls_every_member_in_reverse_order_and_keeps_freed_pointers_stored() {
    let _f = destructor_fakes();
    for populated in [false, true] {
        reset();
        let mut bytes = [0xA5u8; 0x380];
        let p = bytes.as_mut_ptr();
        let mut headers = [[0xAAu32, 0xBEEF, 0x1234, 0x5678, 0xCCDD]; 2];
        let h0 = headers[0].as_mut_ptr();
        let h1 = headers[1].as_mut_ptr();
        let mut replacement = [0xBBu32, 0xCAFE, 0x3456, 0x789A, 0xEEFF];
        let rp = replacement.as_mut_ptr();
        let vtable = [0usize, 0, preview_release as *const () as usize];
        let mut preview = [vtable.as_ptr() as usize];
        let pp = preview.as_mut_ptr().cast::<u8>();
        let offsets = [
            0x348, 0x33C, 0x1F4, 0x118, 0x10C, 0x100, 0xF0, 0xE4, 0xD8, 0xBC, 0xA8, 0x98, 0x88, 0x60, 0x54, 8,
        ];
        unsafe {
            for offset in offsets {
                write(p, offset, 0x1000u32 + offset as u32);
            }
            write(p, 0x228, h0);
            write(p, 0x214, h1);
            write(p, 0x22C, u32::from(populated));
            write(p, 0x218, u32::from(populated));
            write(p, 0x1F0, pp);
            write(p, 0xDC, 0x9999u32);
        }
        STATE.with(|s| s.borrow_mut().next_header = rp as u32);
        let mut expected = bytes;
        unsafe {
            write(expected.as_mut_ptr(), 0, 0x00AA_8748u32);
            write(expected.as_mut_ptr(), 4, 0x00AA_8738u32);
            if populated {
                for offset in [0x214, 0x228] {
                    write(expected.as_mut_ptr(), offset, rp);
                    write(expected.as_mut_ptr(), offset + 4, 0u32);
                }
            }
        }
        let mut c = RawCall::new(Conv::Thiscall, fc4p_0062caf0 as *const () as usize, p as u32, &[]);
        unsafe { call_registers(&mut c) };
        assert_eq!((c.eax, c.edx, c.ecx), (0x1234_ABCD, 0x5678_EFAB, p as u32));
        assert_eq!(bytes, expected);
        let mut calls = vec![
            Call("shutdown", p as u32, vec![]),
            Call("free", 0, vec![0x1348]),
            Call("free", 0, vec![0x133C]),
            Call("parameters_drop", p as u32 + 0x290, vec![]),
        ];
        for (offset, header) in [(0x228, h0), (0x214, h1)] {
            if populated {
                calls.push(Call("erase", p as u32 + offset, vec![0xBEEF]));
            }
            calls.push(Call("free", 0, vec![if populated { rp as u32 } else { header as u32 }]));
        }
        calls.extend([
            Call("free", 0, vec![0x11F4]),
            Call("release", pp as u32, vec![]),
            Call("constraints_drop", p as u32 + 0x188, vec![]),
            Call("constraints_drop", p as u32 + 0x124, vec![]),
        ]);
        for offset in [0x118, 0x10C, 0x100, 0xF0, 0xE4] {
            calls.push(Call("free", 0, vec![0x1000 + offset]));
        }
        calls.extend([
            Call("nested_drop", 0, vec![0x10D8, 0x9999]),
            Call("free", 0, vec![0x10D8]),
            Call("intersections_drop", p as u32 + 0xCC, vec![]),
        ]);
        for offset in [0xB8, 0xA4] {
            calls.push(Call("clear", p as u32 + offset, vec![]));
            calls.push(Call("free", 0, vec![0x1000 + offset + 4]));
        }
        calls.extend([
            Call("free", 0, vec![0x1098]),
            Call("clear", p as u32 + 0x84, vec![]),
            Call("free", 0, vec![0x1088]),
            Call("free", 0, vec![0x1060]),
            Call("free", 0, vec![0x1054]),
            Call("cache_drop", p as u32 + 0x1C, vec![]),
            Call("free", 0, vec![0x1008]),
        ]);
        assert_eq!(take_calls(), calls);
        assert_eq!(headers, [[0xAA, 0xBEEF, 0x1234, 0x5678, 0xCCDD]; 2]);
        assert_eq!(
            replacement,
            if populated {
                [0xBB, 0, rp as u32, rp as u32, 0xEEFF]
            } else {
                [0xBB, 0xCAFE, 0x3456, 0x789A, 0xEEFF]
            }
        );
    }
}

#[test]
fn empty_destruction_skips_optional_releases_but_always_destroys_members() {
    reset();
    let _f = destructor_fakes();
    let mut bytes = [0u8; 0x380];
    let p = bytes.as_mut_ptr();
    let mut c = RawCall::new(Conv::Thiscall, fc4p_0062caf0 as *const () as usize, p as u32, &[]);
    unsafe { call_registers(&mut c) };
    assert_eq!((c.eax, c.edx), (0xA1A2_A3A4, 0xB1B2_B3B4));
    assert_eq!(
        take_calls(),
        [
            Call("shutdown", p as u32, vec![]),
            Call("parameters_drop", p as u32 + 0x290, vec![]),
            Call("constraints_drop", p as u32 + 0x188, vec![]),
            Call("constraints_drop", p as u32 + 0x124, vec![]),
            Call("nested_drop", 0, vec![0, 0]),
            Call("intersections_drop", p as u32 + 0xCC, vec![]),
            Call("clear", p as u32 + 0xB8, vec![]),
            Call("clear", p as u32 + 0xA4, vec![]),
            Call("clear", p as u32 + 0x84, vec![]),
            Call("cache_drop", p as u32 + 0x1C, vec![]),
        ]
    );
    assert_eq!(&bytes[8..], &[0; 0x378]);
}

#[unsafe(naked)]
unsafe extern "C" fn parameters_copy() {
    core::arch::naked_asm!("ret 4")
}
extern "thiscall" fn parameters_copy_log(this: *mut u8, source: *mut u8) -> u64 {
    log("copy_parameters", this, &[source as u32]);
    registers(0xAA55_FF00, 0x1357_2468)
}
#[test]
fn placement_getter_calls_the_assignment_at_the_supplied_destination_and_keeps_both_return_registers() {
    reset();
    let _f = fake(0x0062_6860, parameters_copy as *const () as usize);
    let mut bytes = [0xA5u8; 0x380];
    let p = bytes.as_mut_ptr();
    let args = [0x1234_5678u32];
    let mut c = RawCall::new(Conv::Thiscall, fc4p_0062cad0 as *const () as usize, p as u32, &args);
    c.eax = 0x89AB_CDEF;
    c.edx = 0x9876_5432;
    unsafe { call_registers(&mut c) };
    assert_eq!((c.eax, c.edx), (0x89AB_CDEF, 0x9876_5432));
    let _f = fake(0x0062_6860, parameters_copy_log as *const () as usize);
    c.ecx = p as u32;
    unsafe { call_registers(&mut c) };
    assert_eq!((c.eax, c.edx), (0xAA55_FF00, 0x1357_2468));
    assert_eq!(take_calls(), [Call("copy_parameters", args[0], vec![p as u32 + 0x290])]);
    assert_eq!(bytes, [0xA5; 0x380]);
}

extern "thiscall" fn insert(this: *mut u8, x: u32, z: u32, id: u32, list: *mut u8, real: u32, flags: u32) -> u64 {
    log("insert", this, &[x, z, id, list as u32, real, flags]);
    STATE.with(|s| {
        let s = s.borrow();
        if s.change_highway {
            let tool = unsafe { NetworkTool::from_ptr(s.tool as *mut u8) };
            tool.set_real(0xE7);
            tool.set_valid_steps(2);
            tool.set_support_end(tool.support_begin());
        }
    });
    registers(0x2468_1357, 0xABCD_EF01)
}

#[test]
fn highway_insertion_stops_at_first_signed_step_limit_and_reloads_real_and_limit_after_calls() {
    let _f = fake(0x0062_C3A0, insert as *const () as usize);
    for (limit, empty, change, count, result_eax, result_edx) in [
        (5i32, false, false, 2u32, 7u32, 0xABCD_EF01u32),
        (0, true, false, 0, 0x1122_3344, 0x5566_7788),
        (-2, false, false, 1, 1, 0xABCD_EF01),
        (i32::MIN, false, false, 0, (-3i32) as u32, 0x5566_7788),
        (8, false, false, 3, 0x2468_1357, 0xABCD_EF01),
        (i32::MAX, false, true, 2, 7, 0xABCD_EF01),
    ] {
        reset();
        let mut bytes = [0xA5u8; 0x380];
        let p = bytes.as_mut_ptr();
        let mut records = [0u32; 30];
        let rp = records.as_mut_ptr().cast::<u8>();
        for i in 0..3 {
            records[i * 10] = 10 + i as u32;
            records[i * 10 + 1] = 20 + i as u32;
            records[i * 10 + 2] = 30 + i as u32;
            records[i * 10 + 5] = [-3i32, 1, 7][i] as u32;
        }
        let tool = unsafe { NetworkTool::from_ptr(p) };
        tool.set_support_begin(rp);
        tool.set_support_end(if empty { rp } else { unsafe { rp.add(120) } });
        tool.set_real(0x82);
        tool.set_valid_steps(limit);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tool = p as u32;
            s.change_highway = change;
        });
        let before = bytes;
        let args = [0x3141_5926];
        let mut c = RawCall::new(Conv::Thiscall, fc4p_0062c4b0 as *const () as usize, p as u32, &args);
        c.eax = 0x1122_3344;
        c.edx = 0x5566_7788;
        unsafe { call_registers(&mut c) };
        let calls = (0..count)
            .map(|i| {
                Call(
                    "insert",
                    p as u32,
                    vec![
                        20 + i,
                        30 + i,
                        10 + i,
                        args[0],
                        if i == 1 && change { 0xE7 } else { 0x82 },
                        0,
                    ],
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(take_calls(), calls);
        assert_eq!((c.eax, c.edx), (result_eax, result_edx));
        if !change {
            assert_eq!(bytes, before);
        }
    }
}
