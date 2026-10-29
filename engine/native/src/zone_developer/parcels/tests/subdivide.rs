// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Buildable(usize),
    Extract(i32, i32),
    Compress(usize),
    Allocate(u32),
    Free(usize),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    filtered: usize,
    labels: Vec<Vec<u32>>,
    nodes: Vec<Box<[u32]>>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn events() -> Vec<Event> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn buildable_fake(_this: *mut u8, source: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Buildable(source as usize));
        s.filtered as *mut u8
    })
}
extern "thiscall" fn extract_fake(_this: *mut u8, source: *mut u8, x: i32, z: i32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Extract(x, z));
        assert_eq!(source as usize, s.filtered);
        let label = s.labels[x as usize][z as usize];
        assert_ne!(label, 0);
        let region = unsafe { Region::from_ptr(source) };
        for (x, column) in s.labels.iter().enumerate() {
            for (z, &l) in column.iter().enumerate() {
                if l == label {
                    unsafe { region.set(x as u32, z as u32, false) };
                }
            }
        }
        (0x1000 + label * 0x100) as *mut u8
    })
}
extern "thiscall" fn compress_fake(_this: *mut u8, slot: *mut *mut u8) {
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::Compress(unsafe { slot.read() } as usize))
    });
    unsafe { slot.write(0x1234_5678usize as *mut u8) };
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Allocate(size));
        let mut node = vec![0xA5A5_A5A5u32; 3].into_boxed_slice();
        let p = node.as_mut_ptr().cast();
        s.nodes.push(node);
        p
    })
}
extern "C" fn free_fake(memory: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(unsafe { Region::from_ptr(s.filtered as *mut u8) }.vtable(), 0x00A8_6364);
        s.events.push(Event::Free(memory as usize));
    });
}
struct Fixture {
    dev: Object,
    restriction: Object,
    filtered: Object,
    rows: Vec<*mut u32>,
    _columns: Vec<Vec<u32>>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new(width: i32, height: i32, labels: Vec<Vec<u32>>) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut columns: Vec<Vec<u32>> = labels
            .iter()
            .map(|col| {
                let mut bits = vec![0u32; height.max(0) as usize / 32 + 1];
                for (z, &l) in col.iter().enumerate() {
                    if l != 0 {
                        bits[z / 32] |= 1u32 << (z % 32);
                    }
                }
                bits
            })
            .collect();
        let mut rows: Vec<*mut u32> = columns.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let mut filtered = Object::new(0x24);
        let r = unsafe { Region::from_ptr(filtered.ptr()) };
        r.set_vtable(0xAABB_CCDD);
        r.set_width(width);
        r.set_height(height);
        r.set_columns(if rows.is_empty() { null_mut() } else { rows.as_mut_ptr() });
        let mut restriction = Object::new(0x24);
        let r = unsafe { Region::from_ptr(restriction.ptr()) };
        r.set_width(width);
        r.set_height(height);
        let mut dev = Object::new(0xF0);
        unsafe { ZoneDeveloper::from_ptr(dev.ptr()) }.set_restriction(restriction.ptr());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.filtered = filtered.ptr() as usize;
            s.labels = labels;
        });
        let fakes = vec![
            fake(0x0072_CB90, buildable_fake as *const () as usize),
            fake(0x0072_CDA0, extract_fake as *const () as usize),
            fake(0x0072_AF30, compress_fake as *const () as usize),
            fake(0x0090_CF54, allocate_fake as *const () as usize),
            fake(0x005E_5620, free_fake as *const () as usize),
        ];
        Self {
            dev,
            restriction,
            filtered,
            rows,
            _columns: columns,
            _fakes: fakes,
        }
    }
}
fn values(header: *mut u8) -> Vec<u32> {
    let mut out = vec![];
    let mut node = unsafe { header.cast::<*mut u8>().read() };
    let mut previous = header;
    while node != header {
        assert_eq!(unsafe { node.add(4).cast::<*mut u8>().read() }, previous);
        out.push(unsafe { node.add(8).cast::<u32>().read() });
        previous = node;
        node = unsafe { node.cast::<*mut u8>().read() };
    }
    assert_eq!(unsafe { header.add(4).cast::<*mut u8>().read() }, previous);
    out
}

#[test]
fn subdivision_extracts_components_in_x_major_order_then_frees_the_filtered_source() {
    let mut f = Fixture::new(3, 4, vec![vec![1, 1, 0, 2], vec![1, 0, 0, 2], vec![0, 3, 0, 0]]);
    let mut header = [0u32, 0, 0xA5A5_A5A5];
    let p = header.as_mut_ptr().cast::<u8>();
    unsafe { p.cast::<u32>().write(p as u32) };
    unsafe { p.add(4).cast::<u32>().write(p as u32) };
    let mut list = p;
    unsafe { fc4p_0072ec60(f.dev.ptr(), f.restriction.ptr(), &mut list) };
    assert_eq!(values(p), vec![0x1234_5678; 3]);
    assert_eq!(header[2], 0xA5A5_A5A5);
    let mut expected = vec![
        Event::Buildable(f.restriction.ptr() as usize),
        Event::Extract(0, 0),
        Event::Compress(0x1100),
        Event::Allocate(12),
        Event::Extract(0, 3),
        Event::Compress(0x1200),
        Event::Allocate(12),
        Event::Extract(2, 1),
        Event::Compress(0x1300),
        Event::Allocate(12),
    ];
    expected.extend(f.rows.iter().map(|&row| Event::Free(row as usize)));
    expected.extend([
        Event::Free(f.rows.as_ptr() as usize),
        Event::Free(f.filtered.ptr() as usize),
    ]);
    assert_eq!(events(), expected);
}

#[test]
fn empty_output_receives_the_compressed_whole_source_even_with_empty_scan_dimensions() {
    for (w, h, labels) in [
        (2, 2, vec![vec![0, 0], vec![0, 0]]),
        (0, 3, vec![]),
        (-1, 3, vec![]),
        (2, 0, vec![vec![], vec![]]),
    ] {
        let mut f = Fixture::new(w, h, labels);
        let mut header = [0u32, 0, 0xA5A5_A5A5];
        let p = header.as_mut_ptr().cast::<u8>();
        unsafe { p.cast::<u32>().write(p as u32) };
        unsafe { p.add(4).cast::<u32>().write(p as u32) };
        let mut list = p;
        unsafe { fc4p_0072ec60(f.dev.ptr(), f.restriction.ptr(), &mut list) };
        assert_eq!(values(p), vec![0x1234_5678]);
        assert_eq!(
            events(),
            vec![
                Event::Buildable(f.restriction.ptr() as usize),
                Event::Compress(f.filtered.ptr() as usize),
                Event::Allocate(12)
            ]
        );
    }
}

#[test]
fn preexisting_output_prevents_the_empty_source_transfer_and_null_columns_skip_array_frees() {
    let mut f = Fixture::new(0, 0, vec![]);
    let mut header = [0u32; 3];
    let mut node = [0u32; 3];
    let p = header.as_mut_ptr().cast::<u8>();
    let n = node.as_mut_ptr().cast::<u8>();
    unsafe { p.cast::<u32>().write(n as u32) };
    unsafe { p.add(4).cast::<u32>().write(n as u32) };
    unsafe { n.cast::<u32>().write(p as u32) };
    unsafe { n.add(4).cast::<u32>().write(p as u32) };
    unsafe { n.add(8).cast::<u32>().write(99) };
    let mut list = p;
    unsafe { fc4p_0072ec60(f.dev.ptr(), f.restriction.ptr(), &mut list) };
    assert_eq!(values(p), vec![99]);
    assert_eq!(
        events(),
        vec![
            Event::Buildable(f.restriction.ptr() as usize),
            Event::Free(f.filtered.ptr() as usize)
        ]
    );
}
