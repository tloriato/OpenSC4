// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::test_support::{bytes, object};
use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Construct(u32),
    Reserve(u32),
    Insert(u32),
    Clear,
    Allocate(u32),
    Copy(u32),
    Free(u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Object>,
    empty_buckets: bool,
    bucket_ptr: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn memory(size: usize) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut o = Object::new(size);
        let p = o.ptr();
        s.allocations.push(o);
        p
    })
}
extern "thiscall" fn construct(this: *mut u32, hint: u32) {
    log(Call::Construct(hint));
    assert_eq!(unsafe { this.add(1).cast::<[u32; 4]>().read() }, [0; 4]);
    let empty = STATE.with(|s| s.borrow().empty_buckets);
    let buckets = if empty { core::ptr::null_mut() } else { memory(12) };
    STATE.with(|s| s.borrow_mut().bucket_ptr = buckets as u32);
    unsafe {
        this.add(1).write(buckets as u32);
        this.add(2).write(if empty { 0 } else { buckets as u32 + 12 });
    }
}
extern "thiscall" fn reserve(_this: *mut u32, count: u32) {
    log(Call::Reserve(count));
}
extern "thiscall" fn insert(this: *mut u32, result: *mut u32, key: *const u32) {
    let key = unsafe { key.read() };
    log(Call::Insert(key));
    unsafe {
        let buckets = this.add(1).read() as *mut u32;
        let slot = buckets.add((key % 3) as usize);
        let head = slot.read() as *mut u8;
        let mut node = head;
        while !node.is_null() && word(node, 4) != key {
            node = word(node, 0) as *mut u8;
        }
        let inserted = node.is_null();
        if inserted {
            node = memory(8);
            store(node, 0, head as u32);
            store(node, 4, key);
            slot.write(node as u32);
            this.add(4).write(this.add(4).read() + 1);
        }
        result.write(node as u32);
        result.add(1).write(this as u32);
        result.add(2).cast::<u8>().write(u8::from(inserted));
    }
}
extern "thiscall" fn clear(_this: *mut u32) -> u64 {
    log(Call::Clear);
    0x1234_5678_9ABC_DEF0
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    memory(size as usize)
}
extern "C" fn copy(destination: *mut u8, source: *mut u8, size: u32) -> *mut u8 {
    log(Call::Copy(size));
    unsafe {
        core::ptr::copy(source, destination, size as usize);
    }
    destination
}
extern "C" fn free(p: *mut u8) -> u64 {
    log(Call::Free(p as u32));
    0x8765_4321_FEDC_BA98
}
fn fakes() -> Vec<Fake> {
    STATE.with(|s| *s.borrow_mut() = State::default());
    [
        (0x0082_9BC0, construct as *const () as usize),
        (0x004F_0940, reserve as *const () as usize),
        (0x0062_4DD0, insert as *const () as usize),
        (0x008E_BA60, clear as *const () as usize),
        (0x0090_CF54, allocate as *const () as usize),
        (0x0090_CF63, free as *const () as usize),
        (0x009E_EB70, copy as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect()
}

fn invoke_query(this: *mut u8, region: *mut u8, output: *mut u8) -> (u32, u32, [u32; 2]) {
    let mut frame = [
        fc4p_00718f90 as *const () as u32,
        this as u32,
        region as u32,
        output as u32,
        0,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 12]", "push dword ptr [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
            "mov [edi + 16], eax", "mov [edi + 20], edx", "mov eax, [esp - 8]", "mov [edi + 24], eax",
            "mov eax, [esp - 4]", "mov [edi + 28], eax", in("edi") frame.as_mut_ptr(), clobber_abi("C"));
    }
    (frame[4], frame[5], [frame[6], frame[7]])
}

struct World {
    sim: Object,
    region: Object,
    vector: Object,
    rows: Vec<[u32; 3]>,
    cells: Vec<Vec<u32>>,
    mask_rows: Vec<u32>,
    masks: Vec<Vec<u32>>,
    infos: Vec<Object>,
}
impl World {
    fn new(width: usize, height: usize) -> Self {
        let mut w = Self {
            sim: object(0xD60, &[]),
            region: Object::new(0x24),
            vector: Object::new(12),
            rows: vec![[0; 3]; width],
            cells: vec![vec![0; height]; width],
            mask_rows: vec![0; width],
            masks: vec![vec![0; height.div_ceil(32)]; width],
            infos: Vec::new(),
        };
        for i in 0..width {
            w.rows[i][0] = w.cells[i].as_mut_ptr() as u32;
            w.mask_rows[i] = w.masks[i].as_mut_ptr() as u32;
        }
        let sim = unsafe { RegionSim::from_ptr(w.sim.ptr()) };
        sim.set_rows(w.rows.as_mut_ptr().cast());
        sim.set_min_x(0);
        sim.set_min_z(0);
        sim.set_max_x(width as i32 - 1);
        sim.set_max_z(height as i32 - 1);
        let region = unsafe { Region::from_ptr(w.region.ptr()) };
        region.set_min_x(0);
        region.set_min_z(0);
        region.set_max_x(width as i32 - 1);
        region.set_max_z(height as i32 - 1);
        region.set_width(width as u32);
        region.set_height(height as u32);
        region.set_rows(w.mask_rows.as_mut_ptr());
        w
    }
    fn info(&mut self, x: usize, z: usize, kind: u8, labels: [u16; 4]) {
        let mut info = Object::new(0x18);
        unsafe {
            info.ptr().add(6).write(kind);
            info.ptr().add(12).cast::<[u16; 4]>().write_unaligned(labels);
            store(info.ptr(), 20, self.cells[x][z]);
        }
        self.cells[x][z] = info.ptr() as u32;
        self.infos.push(info);
    }
    fn select(&mut self, x: usize, z: usize) {
        self.masks[x][z / 32] |= 1 << (z % 32);
    }
    fn run(&mut self) -> (u32, u32, [u32; 2]) {
        invoke_query(self.sim.ptr(), self.region.ptr(), self.vector.ptr())
    }
    fn output(&mut self) -> Vec<u32> {
        unsafe {
            let start = word(self.vector.ptr(), 0) as *const u32;
            let count = word(self.vector.ptr(), 4).wrapping_sub(start as u32) / 4;
            if count == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(start, count as usize).to_vec()
            }
        }
    }
}

#[test]
fn region_query_matches_masked_core_case_and_ignores_unselected_cells() {
    let _fakes = fakes();
    let mut w = World::new(4, 4);
    w.info(2, 1, 1, [1, 1, 0, 0]);
    w.select(0, 0);
    w.select(3, 3);
    assert_eq!(w.run().2, [4, w.vector.ptr() as u32]);
    assert!(w.output().is_empty());
    let buckets = STATE.with(|s| s.borrow().bucket_ptr);
    assert_eq!(calls(), [Call::Construct(100), Call::Clear, Call::Free(buckets)]);
    w.select(2, 1);
    let r = w.run();
    assert_eq!(w.output(), [1]);
    assert_eq!((r.0, r.1), (0xFEDC_BA98, 0x8765_4321));
    assert_eq!(r.2, [1, 0]);
    let buckets = STATE.with(|s| s.borrow().bucket_ptr);
    assert_eq!(
        calls(),
        [
            Call::Construct(100),
            Call::Reserve(1),
            Call::Insert(1),
            Call::Reserve(2),
            Call::Insert(1),
            Call::Allocate(4),
            Call::Clear,
            Call::Free(buckets)
        ]
    );
}

#[test]
fn region_query_deduplicates_in_game_hash_order_and_grows_vector_with_copy_then_free() {
    let _fakes = fakes();
    let mut w = World::new(2, 2);
    w.info(0, 0, 1, [1, 4, 2, 7]);
    w.info(0, 1, 1, [2, 0, 8, 3]);
    // The first non-car record is skipped, and the first car record ends the chain search.
    w.info(0, 0, 2, [99; 4]);
    w.select(0, 0);
    w.select(0, 1);
    let before = unsafe { bytes(w.sim.ptr(), 0xD60) };
    let r = w.run();
    assert_eq!(w.output(), [3, 7, 4, 1, 8, 2]);
    assert_eq!(r.2, [8, 4]);
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD60) }, before);
    let log = calls();
    assert_eq!(
        &log[..15],
        &[
            Call::Construct(100),
            Call::Reserve(1),
            Call::Insert(1),
            Call::Reserve(2),
            Call::Insert(4),
            Call::Reserve(3),
            Call::Insert(2),
            Call::Reserve(4),
            Call::Insert(7),
            Call::Reserve(5),
            Call::Insert(2),
            Call::Reserve(5),
            Call::Insert(8),
            Call::Reserve(6),
            Call::Insert(3)
        ]
    );
    let allocations: Vec<_> = log
        .iter()
        .filter_map(|c| match c {
            Call::Allocate(n) => Some(*n),
            _ => None,
        })
        .collect();
    let copies: Vec<_> = log
        .iter()
        .filter_map(|c| match c {
            Call::Copy(n) => Some(*n),
            _ => None,
        })
        .collect();
    assert_eq!(allocations, [4, 8, 16, 32]);
    assert_eq!(copies, [4, 8, 16]);
    // Growth calls occur in allocation/copy/free order; the final clear precedes bucket release.
    let suffix: Vec<_> = log[15..]
        .iter()
        .map(|c| match c {
            Call::Allocate(n) => (0, *n),
            Call::Copy(n) => (1, *n),
            Call::Free(_) => (2, 0),
            Call::Clear => (3, 0),
            _ => panic!("unexpected query call"),
        })
        .collect();
    assert_eq!(
        suffix,
        [
            (0, 4),
            (0, 8),
            (1, 4),
            (2, 0),
            (0, 16),
            (1, 8),
            (2, 0),
            (0, 32),
            (1, 16),
            (2, 0),
            (3, 0),
            (2, 0)
        ]
    );
}

#[test]
fn region_query_reuses_output_capacity_and_reads_high_mask_words_and_all_u16_labels() {
    let _fakes = fakes();
    let mut w = World::new(1, 35);
    w.info(0, 33, 1, [65535, 32768, 0, 65535]);
    w.select(0, 33);
    let mut destination = [0xA5A5_A5A5u32; 8];
    let start = destination.as_mut_ptr() as u32;
    unsafe {
        store(w.vector.ptr(), 0, start);
        store(w.vector.ptr(), 4, start + 24);
        store(w.vector.ptr(), 8, start + 32);
    }
    let output = w.vector.ptr() as u32;
    let r = w.run();
    assert_eq!(w.output(), [65535, 32768]);
    assert_eq!(r.2, [1, output]);
    assert_eq!(destination[2..], [0xA5A5_A5A5; 6]);
    let log = calls();
    assert!(!log.iter().any(|c| matches!(c, Call::Allocate(_) | Call::Copy(_))));
}

#[test]
fn region_query_checks_logical_dimensions_city_bounds_and_empty_rectangles() {
    let _fakes = fakes();
    let mut w = World::new(3, 3);
    for x in 0..3 {
        for z in 0..3 {
            w.info(x, z, 1, [1; 4]);
            w.select(x, z);
        }
    }
    let sim = unsafe { RegionSim::from_ptr(w.sim.ptr()) };
    sim.set_min_x(1);
    sim.set_max_x(1);
    sim.set_min_z(1);
    sim.set_max_z(1);
    let region = unsafe { Region::from_ptr(w.region.ptr()) };
    region.set_width(1);
    region.set_height(1);
    w.run();
    assert!(w.output().is_empty());
    assert_eq!(calls().len(), 3);
    region.set_width(3);
    region.set_height(3);
    w.run();
    assert_eq!(w.output(), [1]);
    let log = calls();
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Insert(1))).count(), 4);
    region.set_min_x(2);
    region.set_max_x(1);
    let r = w.run();
    assert!(w.output().is_empty());
    assert_eq!(r.2[0], 2);
    assert_eq!(calls().len(), 3);
    region.set_min_x(-1);
    region.set_min_z(2);
    region.set_max_z(1);
    w.run();
    assert!(w.output().is_empty());
    assert_eq!(calls().len(), 3);
    // A set with no bucket allocation leaves the destructor's EDX and returns zero in EAX.
    STATE.with(|s| s.borrow_mut().empty_buckets = true);
    let r = w.run();
    assert_eq!((r.0, r.1), (0, 0x1234_5678));
    assert_eq!(calls(), [Call::Construct(100), Call::Clear]);
}
