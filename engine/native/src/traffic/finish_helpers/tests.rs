// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Random(u32), Allocate(u32), Clone(u32, u32, u32), Copy(u32, u32, u32),
    Assign(u32, u32), Holder(u32), Property(u32, u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>, allocations: Vec<u32>, root: u32, random: f64, holder: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) { STATE.with(|s| s.borrow_mut().calls.push(c)); }
fn reset() { STATE.with(|s| *s.borrow_mut() = State::default()); }
fn calls() -> Vec<Call> { STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)) }

extern "thiscall" fn random_body(this: *mut u8) -> f64 {
    log(Call::Random(this as u32));
    STATE.with(|s| s.borrow().random)
}
#[unsafe(naked)]
unsafe extern "thiscall" fn random_fake(_this: *mut u8) -> f64 {
    core::arch::naked_asm!("call {body}", "mov edx, 0xaabbccdd", "ret", body = sym random_body);
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow_mut().allocations.remove(0) as *mut u8)
}
extern "thiscall" fn clone_fake(this: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 {
    log(Call::Clone(this as u32, source as u32, parent as u32));
    STATE.with(|s| s.borrow().root as *mut u8)
}
extern "C" fn copy_fake(destination: *mut u8, source: *mut u8, size: u32) -> *mut u8 {
    log(Call::Copy(destination as u32, source as u32, size));
    unsafe { core::ptr::copy(source, destination, size as usize); }
    destination
}
extern "thiscall" fn assign_body(this: *mut u8, source: *mut u8) -> *mut u8 {
    log(Call::Assign(this as u32, source as u32));
    unsafe { core::ptr::copy(source, this, 12); }
    this
}
#[unsafe(naked)]
unsafe extern "thiscall" fn assign_fake(_this: *mut u8, _source: *mut u8) -> *mut u8 {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}", "mov ecx, 0x1234abcd", "ret 4", body = sym assign_body);
}
extern "thiscall" fn holder_fake(this: *mut u8) -> *mut u8 {
    log(Call::Holder(this as u32));
    STATE.with(|s| s.borrow().holder as *mut u8)
}
extern "C" fn property_fake(holder: *mut u8, id: u32, output: *mut u8) -> u64 {
    log(Call::Property(holder as u32, id, output as u32));
    unsafe {
        assert_eq!(word(output, 0), word(output, 4));
        store(output, 4, word(output, 0).wrapping_add(8));
    }
    0x8765_4321_ABCD_EF01
}

#[test]
fn rail_accident_draws_only_for_damage_and_uses_strict_threshold() {
    reset();
    let _fpu = GameFpu::new();
    let _random = fake(0x0091_3A29, random_fake as *const () as usize);
    let mut sim = Object::new(0xD08);
    let mut buckets = [0u32; 3];
    let mut nodes = [[0u32; 2]; 3];
    nodes[0] = [nodes[1].as_ptr() as u32, 0x0304];
    nodes[1] = [nodes[2].as_ptr() as u32, 0x0303];
    nodes[2] = [0, 0x0303];
    buckets[0] = nodes[0].as_ptr() as u32;
    let view = unsafe { RailDamageSim::from_ptr(sim.ptr()) };
    view.set_buckets(buckets.as_mut_ptr().cast());
    view.set_buckets_end(unsafe { buckets.as_mut_ptr().add(3).cast() });
    unsafe { TrafficSim::from_ptr(sim.ptr()).set_random(0x10203040 as *mut u8); }
    for (threshold, draw, expected) in [
        (1.0, 0.3279835, 1), (0.5, 0.3279835, 1), (0.5, 0.434206, 1),
        (0.5, 0.551609, 0), (0.5, 0.5, 0), (f32::NAN, 0.5, 0),
        (1.0, f64::NAN, 0), (f32::INFINITY, 0.5, 1),
    ] {
        view.set_accident_factor(threshold);
        STATE.with(|s| s.borrow_mut().random = draw);
        let result = unsafe { fc4p_00711590(sim.ptr(), 0xAAAA0003, 0xBBBB0003) };
        assert_eq!(result, 0xAABB_CCDD_0000_0000 | expected);
        assert_eq!(calls(), [Call::Random(0x10203040)]);
    }
    // A missing key traverses its chain and returns the bucket-array pointer with AL cleared.
    let result = unsafe { fc4p_00711590(sim.ptr(), 4, 4) };
    assert_eq!(result, u64::from(buckets.as_ptr() as u32 & !255));
    assert!(calls().is_empty());
}

#[test]
fn subnetwork_output_is_cleared_before_the_property_call() {
    reset();
    let _property = fake(0x005F_F510, property_fake as *const () as usize);
    let vtable = table(&[(0x0C, holder_fake as *const () as usize)]);
    let mut lot = object(8, &vtable);
    let mut vector = [0x12340000u32, 0x12340040, 0x12340080];
    STATE.with(|s| s.borrow_mut().holder = 0x23456789);
    let result = unsafe { invoke(fc4p_00711b50 as *const () as usize, core::ptr::null_mut(),
        &[lot.ptr() as u32, vector.as_mut_ptr() as u32], 0, 0xA0B0C0D0, false) };
    assert_eq!((result.eax, result.edx), (0xABCD_EF01, 0x8765_4321));
    assert_eq!(vector, [0x12340000, 0x12340008, 0x12340080]);
    assert_eq!(calls(), [Call::Holder(lot.ptr() as u32),
        Call::Property(0x23456789, 0x2A4F_9C74, vector.as_mut_ptr() as u32)]);
}

#[test]
fn tree_copy_initializes_only_header_links_then_clones_nonempty_root() {
    reset();
    let _allocate = fake(0x0090_CF54, allocate_fake as *const () as usize);
    let _clone = fake(0x0071_0B80, clone_fake as *const () as usize);
    for nonempty in [false, true] {
        let mut destination = [0xCCCCCCCCu32; 3];
        let mut header = [0xA5A5A5A5u32; 6];
        let mut source_root = [0u32; 6];
        let mut left = [0u32; 6];
        let mut right = [0u32; 6];
        let mut cloned = [0, 0, left.as_mut_ptr() as u32, right.as_mut_ptr() as u32, 7, 9];
        let mut source_header = [0u32; 6];
        source_header[1] = if nonempty { source_root.as_mut_ptr() as u32 } else { 0 };
        let mut source = [source_header.as_mut_ptr() as u32, if nonempty { 3 } else { 0 }];
        STATE.with(|s| { let mut s = s.borrow_mut(); s.allocations.push(header.as_mut_ptr() as u32); s.root = cloned.as_mut_ptr() as u32; });
        let answer = unsafe { fc4p_00712280(destination.as_mut_ptr().cast(), source.as_mut_ptr().cast()) };
        assert_eq!(answer, destination.as_mut_ptr().cast());
        assert_eq!(destination, [header.as_mut_ptr() as u32, source[1], 0xCCCCCCCC]);
        assert_eq!(header[0], 0xA5A5A500);
        assert_eq!(&header[4..], &[0xA5A5A5A5; 2]);
        if nonempty {
            assert_eq!(&header[1..4], &[cloned.as_ptr() as u32, left.as_ptr() as u32, right.as_ptr() as u32]);
            assert_eq!(calls(), [Call::Allocate(24), Call::Clone(destination.as_ptr() as u32, source_root.as_ptr() as u32, header.as_ptr() as u32)]);
        } else {
            assert_eq!(&header[1..4], &[0, header.as_ptr() as u32, header.as_ptr() as u32]);
            assert_eq!(calls(), [Call::Allocate(24)]);
        }
    }
}

#[test]
fn vector_construction_copies_rows_and_retains_destination_padding() {
    reset();
    let _allocate = fake(0x0090_CF54, allocate_fake as *const () as usize);
    let _copy = fake(0x009E_EB70, copy_fake as *const () as usize);
    let mut words = [4u32, 7, 0xFFFFFFFF];
    let mut allocation = [0xAAAAAAAAu32; 3];
    let begin = words.as_mut_ptr() as u32;
    let mut source = [begin, begin + 12, begin + 20, 0, 0, 0];
    let mut destination = [0xA5A5A5A5u32; 7];
    STATE.with(|s| s.borrow_mut().allocations.push(allocation.as_mut_ptr() as u32));
    let result = unsafe { invoke(fc4p_00712330 as *const () as usize, core::ptr::null_mut(),
        &[source.as_mut_ptr() as u32, source.as_mut_ptr().add(6) as u32, destination.as_mut_ptr() as u32], 0, 0, true) };
    assert_eq!(result.eax, unsafe { destination.as_mut_ptr().add(6) as u32 });
    assert_eq!(result.ecx, 0);
    let a = allocation.as_mut_ptr() as u32;
    assert_eq!(destination, [a, a + 12, a + 12, 0, 0, 0, 0xA5A5A5A5]);
    assert_eq!(allocation, words);
    assert_eq!(calls(), [Call::Allocate(12), Call::Copy(a, begin, 12)]);
}

#[test]
fn vector_range_empty_null_and_negative_boundaries() {
    reset();
    let mut memory = Object::new(32);
    let before = unsafe { bytes(memory.ptr(), 32) };
    for export in [fc4p_00712330 as *const () as usize, fc4p_007123b0 as *const () as usize] {
        let r = unsafe { invoke(export, 0x87654321 as *mut u8, &[100, 100, memory.ptr() as u32], 0, 0, true) };
        assert_eq!(r.eax, memory.ptr() as u32);
        assert_eq!(r.ecx, if export == fc4p_00712330 as *const () as usize { 0x87654321 } else { 0 });
    }
    let r = unsafe { invoke(fc4p_007123b0 as *const () as usize, core::ptr::null_mut(), &[112, 100, memory.ptr() as u32], 0, 0, true) };
    assert_eq!((r.eax, r.ecx), (memory.ptr() as u32, (-12i32) as u32));
    let r = unsafe { invoke(fc4p_00712330 as *const () as usize, core::ptr::null_mut(), &[100, 112, 0], 0, 0, true) };
    assert_eq!(r.eax, 12);
    assert_eq!(unsafe { bytes(memory.ptr(), 32) }, before);
    assert!(calls().is_empty());
}

#[test]
fn vector_assignment_calls_each_row_in_order_and_keeps_last_ecx() {
    reset();
    let _assign = fake(0x0072_BD50, assign_fake as *const () as usize);
    let mut source = [1u32, 2, 3, 4, 5, 6];
    let mut destination = [0u32; 6];
    let s = source.as_mut_ptr() as u32;
    let d = destination.as_mut_ptr() as u32;
    let r = unsafe { invoke(fc4p_007123b0 as *const () as usize, core::ptr::null_mut(), &[s, s + 24, d], 0, 0, true) };
    assert_eq!((r.eax, r.ecx), (d + 24, 0x1234ABCD));
    assert_eq!(destination, source);
    assert_eq!(calls(), [Call::Assign(d, s), Call::Assign(d + 12, s + 12)]);
}
