// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::test_support::{bytes, invoke, object, table};
use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Free(u32),
    Clone(u32, u32),
    Clear(u32),
    Resize(u32, u32, u32),
    Funding(u32),
    Road(u32, u32),
    Transit(u32, u32),
    Strikes(u32),
    Accident(u32),
    History(u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Object>,
    sim: usize,
    replacement: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
extern "C" fn allocate(n: u32) -> *mut u8 {
    log(Call::Allocate(n));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut o = Object::new(n as usize);
        unsafe {
            core::ptr::write_bytes(o.ptr(), 0xCD, n as usize);
        }
        let p = o.ptr();
        s.allocations.push(o);
        p
    })
}
extern "C" fn free(p: *mut u8) -> u64 {
    log(Call::Free(p as u32));
    0xABCD_5678_1234_9000
}
extern "thiscall" fn clone_routes(this: *mut u8, source: *mut u8) {
    log(Call::Clone(this as u32, source as u32));
    unsafe {
        assert_eq!(word(this.sub(8), 0), 0);
        assert_eq!(word(this.sub(8), 4), word(source.sub(4), 0));
        core::ptr::copy_nonoverlapping(source, this, 0x34);
    }
}
extern "thiscall" fn clone_bytes(this: *mut u8, source: *mut u8) -> u64 {
    log(Call::Clone(this as u32, source as u32));
    unsafe {
        core::ptr::copy_nonoverlapping(source, this, 12);
    }
    0x1122_3344_5566_7788
}
extern "thiscall" fn clear(this: *mut u8) -> u64 {
    log(Call::Clear(this as u32));
    unsafe {
        let header = word(this, 0) as *mut u8;
        store(header, 0, header as u32);
        store(header, 4, header as u32);
    }
    0xAABB_CCDD_7788_9900
}
extern "thiscall" fn resize(this: *mut u8, n: u32, value: *const u32) {
    let zero = unsafe { value.read() };
    log(Call::Resize(this as u32, n, zero));
    assert_eq!((n, zero), (3, 0));
    STATE.with(|s| {
        let s = s.borrow();
        if s.replacement != 0 {
            unsafe {
                store(this, 0, s.replacement as u32);
            }
        }
    });
}
extern "thiscall" fn funding(this: *mut u8) -> *mut u8 {
    log(Call::Funding(this as u32));
    unsafe { word(this, 4) as *mut u8 }
}
extern "thiscall" fn road(this: *mut u8, f: *mut u8) {
    log(Call::Road(this as u32, f as u32));
    STATE.with(|s| {
        let s = s.borrow();
        if s.replacement != 0 {
            unsafe { TrafficSim::from_ptr(this) }.set_roads(s.replacement as *mut u8);
        }
    });
}
extern "thiscall" fn transit(this: *mut u8, f: *mut u8) {
    log(Call::Transit(this as u32, f as u32));
}
extern "thiscall" fn strikes(this: *mut u8) {
    log(Call::Strikes(this as u32));
}
extern "thiscall" fn accidents(this: *mut u8) {
    log(Call::Accident(this as u32));
}
extern "thiscall" fn history(_this: *mut u8, id: u32, value: u32) -> u64 {
    log(Call::History(id, value));
    STATE.with(|s| {
        let s = s.borrow();
        if id == 0xEA5B_A01D && s.sim != 0 {
            unsafe { TrafficSim::from_ptr(s.sim as *mut u8) }.set_average_commute(2.5);
        }
    });
    0xCDEF_1234_89AB_CDEF
}
fn fakes() -> Vec<Fake> {
    STATE.with(|s| *s.borrow_mut() = State::default());
    [
        (0x0090_CF54, allocate as *const () as usize),
        (0x0090_CF63, free as *const () as usize),
        (0x006D_6C30, clone_routes as *const () as usize),
        (0x0045_9E80, clone_bytes as *const () as usize),
        (0x006F_CBD0, clear as *const () as usize),
        (0x0071_69C0, resize as *const () as usize),
        (0x0071_7D30, road as *const () as usize),
        (0x0071_7E00, transit as *const () as usize),
        (0x0070_DBA0, strikes as *const () as usize),
        (0x0071_84E0, accidents as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect()
}

#[test]
fn route_hash_inserts_at_bucket_head_and_does_not_copy_duplicates() {
    let _fakes = fakes();
    let mut hash = Object::new(0x14);
    let h = unsafe { RouteHash::from_ptr(hash.ptr()) };
    let mut buckets = [0u32; 3];
    h.set_buckets(buckets.as_mut_ptr());
    h.set_end(buckets.as_mut_ptr().wrapping_add(3));
    h.set_count(u32::MAX);
    let mut source = [0xA5u8; 0x38];
    unsafe {
        store(source.as_mut_ptr(), 0, 0xFFFF_FFFE);
    }
    let mut result = [0x5Au8; 12];
    let hash_ptr = hash.ptr();
    let invoke_insert = |result: &mut [u8; 12], source: &mut [u8; 0x38]| unsafe {
        invoke(
            fc4p_00718800 as *const () as usize,
            hash_ptr,
            &[result.as_mut_ptr() as u32, source.as_mut_ptr() as u32],
            0xDEAD_BEEF,
            0,
            false,
        )
    };
    let r = invoke_insert(&mut result, &mut source);
    let node = unsafe { word(result.as_mut_ptr(), 0) } as *mut u8;
    assert_eq!(r.eax, result.as_ptr() as u32);
    assert_eq!(h.count(), 0);
    assert_eq!(buckets, [0, 0, node as u32]);
    assert_eq!(unsafe { word(node, 0) }, 0);
    assert_eq!(unsafe { bytes(node.add(4), 0x38) }, source);
    assert_eq!(&result[8..], &[1, 0x5A, 0x5A, 0x5A]);
    assert_eq!(
        calls(),
        [
            Call::Allocate(0x3C),
            Call::Clone(node.wrapping_add(8) as u32, source.as_ptr().wrapping_add(4) as u32)
        ]
    );
    invoke_insert(&mut result, &mut source);
    assert_eq!(unsafe { word(result.as_mut_ptr(), 0) }, node as u32);
    assert_eq!(&result[8..], &[0, 0x5A, 0x5A, 0x5A]);
    assert_eq!(h.count(), 0);
    assert!(calls().is_empty());
    // A colliding distinct key goes ahead of the existing node.
    unsafe {
        store(source.as_mut_ptr(), 0, 2);
    }
    invoke_insert(&mut result, &mut source);
    let second = unsafe { word(result.as_mut_ptr(), 0) } as *mut u8;
    assert_eq!(unsafe { word(second, 0) }, node as u32);
    assert_eq!(buckets[2], second as u32);
    assert_eq!(h.count(), 1);
    calls();
    // Finding a key below the bucket head retains both links and the count.
    unsafe {
        store(source.as_mut_ptr(), 0, 0xFFFF_FFFE);
    }
    invoke_insert(&mut result, &mut source);
    assert_eq!(unsafe { word(result.as_mut_ptr(), 0) }, node as u32);
    assert_eq!(h.count(), 1);
    assert!(calls().is_empty());
}

#[test]
fn route_hash_erasure_frees_payload_before_node_and_relinks_head_or_interior() {
    let _fakes = fakes();
    let mut hash = Object::new(0x14);
    let h = unsafe { RouteHash::from_ptr(hash.ptr()) };
    let mut nodes = [Object::new(0x3C), Object::new(0x3C), Object::new(0x3C)];
    let ptrs = nodes.each_mut().map(Object::ptr);
    let mut buckets = [ptrs[0] as u32];
    h.set_buckets(buckets.as_mut_ptr());
    h.set_end(buckets.as_mut_ptr().wrapping_add(1));
    h.set_count(3);
    unsafe {
        store(ptrs[0], 0, ptrs[1] as u32);
        store(ptrs[1], 0, ptrs[2] as u32);
        store(ptrs[1], 0x24, 0x1234_5000);
    }
    let mut iterator = [ptrs[1] as u32];
    let hash_ptr = hash.ptr();
    let erase = |iterator: &mut [u32; 1]| unsafe {
        invoke(
            fc4p_007188a0 as *const () as usize,
            hash_ptr,
            &[iterator.as_mut_ptr() as u32],
            7,
            9,
            false,
        )
    };
    let r = erase(&mut iterator);
    assert_eq!((r.eax, r.edx), (2, 0xABCD_5678));
    assert_eq!(unsafe { word(ptrs[0], 0) }, ptrs[2] as u32);
    assert_eq!(calls(), [Call::Free(0x1234_5000), Call::Free(ptrs[1] as u32)]);
    iterator[0] = ptrs[0] as u32;
    erase(&mut iterator);
    assert_eq!(buckets[0], ptrs[2] as u32);
    assert_eq!(h.count(), 1);
    assert_eq!(calls(), [Call::Free(ptrs[0] as u32)]);
    iterator[0] = ptrs[1] as u32;
    let before = unsafe { bytes(hash.ptr(), 0x14) };
    erase(&mut iterator);
    assert_eq!(unsafe { bytes(hash.ptr(), 0x14) }, before);
    assert!(calls().is_empty());
    iterator[0] = 0;
    let r = erase(&mut iterator);
    assert_eq!((r.eax, r.edx), (iterator.as_ptr() as u32, 9));
    assert!(calls().is_empty());
}

fn strategy_call(this: *mut u8, kind: u32, vector: *mut u8) -> (u32, u32, u32) {
    let mut frame = [
        fc4p_00718bb0 as *const () as u32,
        this as u32,
        kind,
        vector as u32,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 12]", "push dword ptr [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
            "mov [edi + 16], eax", "mov [edi + 20], edx", "mov eax, [esp - 4]", "mov [edi + 24], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"));
    }
    (frame[4], frame[5], frame[6])
}

#[test]
fn strategy_percentages_keep_byte_values_x87_bits_and_the_overwritten_argument() {
    let _fpu = GameFpu::new();
    let _fakes = fakes();
    let mut scale = 0.01f32;
    let _scale = fake_data(0x00A8_23A4, (&mut scale as *mut f32).cast());
    let mut sim = object(0xD60, &[]);
    let mut vector = Object::new(12);
    let mut destination = [0u32; 5];
    unsafe {
        store(vector.ptr(), 0, destination.as_mut_ptr().wrapping_add(1) as u32);
    }
    let before = unsafe { bytes(sim.ptr(), 0xD60) };
    for (kind, input, expected) in [
        (1, [80, 20, 0], [0x3F4C_CCCC, 0x3E4C_CCCC, 0]),
        (2, [0, 50, 50], [0, 0x3F00_0000, 0x3F00_0000]),
        (0, [255, 100, 1], [0x4023_3333, 0x3F80_0000, 0x3C23_D70A]),
    ] {
        unsafe {
            core::ptr::copy_nonoverlapping(input.as_ptr(), sim.ptr().add(0xC5C + kind * 3), 3);
        }
        let result = strategy_call(sim.ptr(), kind as u32, vector.ptr());
        assert_eq!(destination[1..4], expected);
        assert_eq!((destination[0], destination[4]), (0, 0));
        assert_eq!(
            result,
            (
                u32::from(input[2]),
                destination.as_ptr().wrapping_add(1) as u32,
                u32::from(input[2])
            )
        );
        assert_eq!(calls(), [Call::Resize(vector.ptr() as u32, 3, 0)]);
    }
    let after = unsafe { bytes(sim.ptr(), 0xD60) };
    assert_eq!(&before[..0xC5C], &after[..0xC5C]);
    assert_eq!(&before[0xC65..], &after[0xC65..]);
    // Resize may replace the vector's storage. An index of -1 addresses the preceding three bytes.
    let mut replacement = [0u32; 3];
    STATE.with(|s| s.borrow_mut().replacement = replacement.as_mut_ptr() as usize);
    unsafe {
        core::ptr::copy_nonoverlapping([10u8, 20, 30].as_ptr(), sim.ptr().add(0xC59), 3);
    }
    strategy_call(sim.ptr(), u32::MAX, vector.ptr());
    assert_eq!(replacement, [0x3DCC_CCCC, 0x3E4C_CCCC, 0x3E99_9999]);
    calls();
}

#[test]
fn monthly_calls_maintenance_before_snapshot_and_history_and_reloads_services() {
    let _fpu = GameFpu::new();
    let _fakes = fakes();
    let mut scales = [100.0f32, 25.0];
    let _s1 = fake_data(0x00A8_0AA0, (&mut scales[0] as *mut f32).cast());
    let _s2 = fake_data(0x00AB_32C0, (&mut scales[1] as *mut f32).cast());
    let vt = table(&[(0x28, funding as *const () as usize)]);
    let hv = table(&[(0x30, history as *const () as usize)]);
    let mut recorder = object(4, &hv);
    let mut recorder_ptr = recorder.ptr();
    let _history = fake_data(0x00B4_3DC0, (&mut recorder_ptr as *mut *mut u8).cast());
    let mut road = object(8, &vt);
    let mut transit = object(8, &vt);
    let mut next = object(8, &vt);
    let mut funding_values = [0.5f32.to_bits(), 0.75f32.to_bits(), 1.25f32.to_bits()];
    unsafe {
        store(road.ptr(), 4, funding_values.as_mut_ptr() as u32);
        store(transit.ptr(), 4, funding_values.as_mut_ptr().add(1) as u32);
        store(next.ptr(), 4, funding_values.as_mut_ptr().add(2) as u32);
    }
    let mut sim = object(0xD60, &[]);
    let ts = unsafe { TrafficSim::from_ptr(sim.ptr()) };
    let ms = unsafe { MonthlySim::from_ptr(sim.ptr()) };
    ts.set_roads(road.ptr());
    ts.set_transit(transit.ptr());
    ts.set_average_commute(99.0);
    ms.set_congestion(0.123_456);
    ms.set_ferry_count(123);
    let mut counts = [0, 1, u32::MAX, 0x8000_0000, 4, 5, 6, 7, 8];
    ms.set_type_counts(counts.as_mut_ptr());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.sim = sim.ptr() as usize;
        s.replacement = next.ptr() as usize;
    });
    let mut expected_memory = unsafe { bytes(sim.ptr(), 0xD60) };
    for (offset, value) in [
        (0x724, next.ptr() as u32),
        (0x40C, funding_values[2]),
        (0x410, funding_values[1]),
        (0x420, 2.5f32.to_bits()),
    ] {
        expected_memory[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let r = unsafe { invoke(fc4p_00718ac0 as *const () as usize, sim.ptr(), &[], 0, 0, false) };
    assert_eq!((r.eax, r.edx), (0x89AB_CDEF, 0xCDEF_1234));
    assert_eq!(unsafe { bytes(sim.ptr(), 0xD60) }, expected_memory);
    let mut expected = vec![
        Call::Funding(road.ptr() as u32),
        Call::Road(sim.ptr() as u32, funding_values.as_ptr() as u32),
        Call::Funding(transit.ptr() as u32),
        Call::Transit(sim.ptr() as u32, funding_values.as_ptr().wrapping_add(1) as u32),
        Call::Strikes(sim.ptr() as u32),
        Call::Accident(sim.ptr() as u32),
        Call::Funding(next.ptr() as u32),
        Call::Funding(transit.ptr() as u32),
        Call::History(0xEA5B_A01D, 12),
        Call::History(0x8A5D_4116, 62),
    ];
    for (i, value) in counts.into_iter().enumerate() {
        expected.push(Call::History(0xABB8_FAAC + i as u32, value));
    }
    expected.push(Call::History(0xEC0B_DBF8, 123));
    assert_eq!(calls(), expected);
    // The helper's low word wraps rather than saturating, and unordered values use integer indefinite.
    for (value, expected) in [
        (f32::NAN, 0),
        (f32::INFINITY, 0),
        (-0.125, (-12i32) as u32),
        (42_949_672.0, 0),
    ] {
        ms.set_congestion(value);
        unsafe {
            invoke(fc4p_00718ac0 as *const () as usize, sim.ptr(), &[], 0, 0, false);
        }
        assert!(calls().contains(&Call::History(0xEA5B_A01D, expected)));
    }
}

unsafe fn ring(nodes: &mut [Object]) -> Vec<*mut u8> {
    let p: Vec<_> = nodes.iter_mut().map(Object::ptr).collect();
    for i in 0..p.len() {
        unsafe {
            store(p[i], 0, p[(i + 1) % p.len()] as u32);
            store(p[i], 4, p[(i + p.len() - 1) % p.len()] as u32);
        }
    }
    p
}

#[test]
fn ferry_routes_clear_then_append_both_lists_in_terminal_order() {
    let _fakes = fakes();
    let mut sim = Object::new(0xD60);
    let mut terminals = [Object::new(0x20), Object::new(0x20), Object::new(0x20)];
    let tp = unsafe { ring(&mut terminals) };
    let mut lists = [
        vec![Object::new(0x20), Object::new(0x20), Object::new(0x20)],
        vec![Object::new(0x20), Object::new(0x20)],
        vec![Object::new(0x20)],
        vec![Object::new(0x20), Object::new(0x20)],
    ];
    let mut sources = Vec::new();
    for (i, list) in lists.iter_mut().enumerate() {
        let p = unsafe { ring(list) };
        unsafe {
            store(tp[1 + i / 2], 0x18 + (i % 2) * 4, p[0] as u32);
        }
        for source in &p[1..] {
            unsafe {
                store(*source, 0x14, sources.len() as u32 + 100);
            }
            sources.push(source.wrapping_add(0x14));
        }
    }
    unsafe { MonthlySim::from_ptr(sim.ptr()) }.set_ferries(tp[0]);
    let mut header = Object::new(8);
    let mut destination = [header.ptr() as u32, 0xA5A5_A5A5];
    unsafe {
        store(header.ptr(), 0, 0xDEAD_BEEF);
    }
    let r = unsafe {
        invoke(
            fc4p_00718c40 as *const () as usize,
            sim.ptr(),
            &[destination.as_mut_ptr() as u32],
            0,
            0,
            false,
        )
    };
    assert_eq!((r.eax, r.edx), (tp[0] as u32, 0x1122_3344));
    assert_eq!(destination[1], 0xA5A5_A5A5);
    let mut expected = vec![Call::Clear(destination.as_ptr() as u32)];
    let mut previous = header.ptr();
    let mut node = unsafe { word(header.ptr(), 0) } as *mut u8;
    for (i, source) in sources.iter().enumerate() {
        assert_eq!(unsafe { word(node, 4) }, previous as u32);
        assert_eq!(unsafe { word(node, 8) }, i as u32 + 100);
        expected.push(Call::Allocate(0x14));
        expected.push(Call::Clone(node.wrapping_add(8) as u32, *source as u32));
        previous = node;
        node = unsafe { word(node, 0) } as *mut u8;
    }
    assert_eq!(node, header.ptr());
    assert_eq!(unsafe { word(header.ptr(), 4) }, previous as u32);
    assert_eq!(calls(), expected);
    // An empty terminal list still clears the output and returns its own sentinel.
    unsafe {
        store(tp[0], 0, tp[0] as u32);
    }
    let r = unsafe {
        invoke(
            fc4p_00718c40 as *const () as usize,
            sim.ptr(),
            &[destination.as_mut_ptr() as u32],
            0,
            0,
            false,
        )
    };
    assert_eq!((r.eax, r.edx), (tp[0] as u32, 0xAABB_CCDD));
    assert_eq!(calls(), [Call::Clear(destination.as_ptr() as u32)]);
}
