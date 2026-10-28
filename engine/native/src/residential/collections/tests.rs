// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Group(usize, u32),
    Retain(usize),
    Allocate(u32),
    Framework,
    Acquire(u32, u32),
    ReleaseTemporary,
    Copy(usize, usize),
    Insert(usize, usize, usize, usize),
    Free(usize),
    Collect(usize, usize, usize, usize),
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static ALLOCATIONS:RefCell<Vec<Object>>=const{RefCell::new(Vec::new())};
    static ALLOCATOR:RefCell<usize>=const{RefCell::new(0)};
    static GROW:RefCell<Option<(usize,usize,usize)>>=const{RefCell::new(None)};
    static FILTER_CHANGE:RefCell<Option<(usize,usize,usize)>>=const{RefCell::new(None)};
}
fn record(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}
fn take() -> Vec<Call> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    record(Call::Allocate(size));
    ALLOCATIONS.with(|a| {
        let mut a = a.borrow_mut();
        a.push(Object::new(size as usize));
        a.last_mut().unwrap().ptr()
    })
}
extern "thiscall" fn allocator_alloc(_this: *mut u8, size: u32) -> u64 {
    registers(alloc(size) as u32, 0xCAFE_BABE)
}
extern "C" fn release(p: *mut u8) -> u32 {
    record(Call::Free(p as usize));
    0x7654_3200
}
extern "thiscall" fn group(this: *mut u8, id: u32) -> u64 {
    record(Call::Group(this as usize, id));
    FILTER_CHANGE.with(|change| {
        if let Some((filter, begin, end)) = change.borrow_mut().take() {
            unsafe {
                let filter = Groups::from_ptr(filter as *mut u8);
                filter.set_begin(begin as *mut u32);
                filter.set_end(end as *mut u32);
            }
        }
    });
    let yes = unsafe { read_unaligned(this.add(4).cast::<u32>()) } == id;
    registers(u32::from(yes), 0xFACE_1234)
}
extern "thiscall" fn retain(this: *mut u8) -> u32 {
    record(Call::Retain(this as usize));
    2
}
extern "C" fn get_framework() -> *mut u8 {
    record(Call::Framework);
    ALLOCATOR.with(|a| *a.borrow() as *mut u8)
}
extern "thiscall" fn acquire(_this: *mut u8, a: u32, b: u32, out: *mut *mut u8) -> u32 {
    record(Call::Acquire(a, b));
    unsafe { *out = ALLOCATOR.with(|a| *a.borrow() as *mut u8) };
    1
}
extern "thiscall" fn temporary(_this: *mut *mut u8) -> u32 {
    record(Call::ReleaseTemporary);
    0
}
extern "thiscall" fn copy(this: *mut u8, source: *mut u8) -> u32 {
    record(Call::Copy(this as usize, source as usize));
    unsafe { core::ptr::copy_nonoverlapping(source, this, 16) };
    this as u32
}
extern "thiscall" fn insert(this: *mut u8, position: *mut u8, first: *mut u8, last: *mut u8, _tag: *const u8) -> u32 {
    record(Call::Insert(
        this as usize,
        position as usize,
        first as usize,
        last as usize,
    ));
    0
}
extern "fastcall" fn collect(this: *mut u8, _edx: u32, list: *mut *mut u8, out: *mut *mut u8, groups: *mut u8) -> u64 {
    record(Call::Collect(
        this as usize,
        list as usize,
        out as usize,
        groups as usize,
    ));
    GROW.with(|g| {
        if let Some((sim, begin, end)) = g.borrow_mut().take() {
            unsafe {
                let sim = CollectionsSim::from_ptr(sim as *mut u8);
                sim.set_schools(begin as *mut *mut u8);
                sim.set_schools_end(end as *mut *mut u8);
            }
        }
    });
    registers(0x1234_5678, 0x9ABC_DEF0)
}

struct List {
    head: Object,
    nodes: Vec<Object>,
}
impl List {
    fn new(values: &[*mut u8], size: usize) -> Self {
        let mut list = Self {
            head: Object::new(size),
            nodes: values.iter().map(|_| Object::new(size)).collect(),
        };
        let head = list.head.ptr();
        let mut previous = head;
        for (object, &value) in list.nodes.iter_mut().zip(values) {
            unsafe {
                let p = object.ptr();
                let n = Node::from_ptr(p);
                n.set_value(value);
                n.set_previous(previous);
                Node::from_ptr(previous).set_next(p);
                previous = p;
            }
        }
        unsafe {
            Node::from_ptr(previous).set_next(head);
            Node::from_ptr(head).set_previous(previous)
        };
        list
    }
    fn head(&mut self) -> *mut u8 {
        self.head.ptr()
    }
    fn values(&mut self) -> Vec<*mut u8> {
        unsafe {
            let head = self.head();
            let mut p = Node::from_ptr(head).next();
            let mut out = Vec::new();
            while p != head {
                out.push(Node::from_ptr(p).value());
                p = Node::from_ptr(p).next();
            }
            out
        }
    }
}

#[test]
fn building_filters_retain_before_appending_and_preserve_sentinels() {
    let mut table = [0usize; 23];
    table[1] = retain as *const () as usize;
    table[22] = group as *const () as usize;
    let mut a = [table.as_ptr() as usize, 0x1510];
    let mut b = [table.as_ptr() as usize, 0x151B];
    let a = a.as_mut_ptr().cast();
    let b = b.as_mut_ptr().cast();
    let mut input = List::new(&[a, b], 12);
    let mut output = List::new(&[b], 12);
    let mut allocator_table = [0usize; 4];
    allocator_table[3] = allocator_alloc as *const () as usize;
    let mut allocator = [allocator_table.as_ptr() as usize];
    let mut ap = allocator.as_mut_ptr().cast();
    let _data = fake_data(0x00B2_07E4, (&mut ap as *mut *mut u8).cast());
    for ids in [
        None,
        Some(vec![]),
        Some(vec![0x9999, 0x1510, 0x151B]),
        Some(vec![0x1510, 0x1510]),
    ] {
        let mut ih = input.head();
        let mut oh = output.head();
        let mut ids = ids;
        let mut filter = ids
            .as_mut()
            .map(|v| [v.as_mut_ptr(), v.as_mut_ptr().wrapping_add(v.len())]);
        let gp = filter.as_mut().map_or(null_mut(), |f| f.as_mut_ptr().cast());
        let result = unsafe { fc4p_006face0(null_mut(), 0xDEAD_BEEF, &mut ih, &mut oh, gp) };
        assert_eq!(result as u32, ih as u32);
        assert_eq!(ih, input.head());
        assert_eq!(oh, output.head());
        let expected = match ids.as_deref() {
            None => vec![
                Call::Retain(a as usize),
                Call::Allocate(12),
                Call::Retain(b as usize),
                Call::Allocate(12),
            ],
            Some([]) => vec![],
            Some([0x1510, 0x1510]) => vec![
                Call::Group(a as usize, 0x1510),
                Call::Retain(a as usize),
                Call::Allocate(12),
                Call::Group(b as usize, 0x1510),
                Call::Group(b as usize, 0x1510),
            ],
            _ => vec![
                Call::Group(a as usize, 0x9999),
                Call::Group(a as usize, 0x1510),
                Call::Retain(a as usize),
                Call::Allocate(12),
                Call::Group(b as usize, 0x9999),
                Call::Group(b as usize, 0x1510),
                Call::Group(b as usize, 0x151B),
                Call::Retain(b as usize),
                Call::Allocate(12),
            ],
        };
        assert_eq!(take(), expected);
    }
    assert_eq!(output.values(), vec![b, a, b, a, b, a]);
}

#[test]
fn lazy_allocator_acquisition_and_empty_list_registers() {
    let mut table = [0usize; 6];
    table[1] = retain as *const () as usize;
    table[3] = allocator_alloc as *const () as usize;
    table[5] = acquire as *const () as usize;
    let mut allocator = [table.as_ptr() as usize];
    let pointer = allocator.as_mut_ptr().cast::<u8>();
    ALLOCATOR.with(|a| *a.borrow_mut() = pointer as usize);
    let mut ap = null_mut();
    let _data = fake_data(0x00B2_07E4, (&mut ap as *mut *mut u8).cast());
    let _fw = fake(0x0087_93EC, get_framework as *const () as usize);
    let _temp = fake(0x0041_1870, temporary as *const () as usize);
    let mut input = List::new(&[], 12);
    let mut output = List::new(&[], 12);
    let mut ih = input.head();
    let mut oh = output.head();
    assert_eq!(
        unsafe { fc4p_006face0(null_mut(), 0xAABB_CCDD, &mut ih, &mut oh, null_mut()) },
        registers(ih as u32, 0xAABB_CCDD)
    );
    assert!(take().is_empty());
    input = List::new(&[pointer, pointer], 12);
    ih = input.head();
    unsafe { fc4p_006face0(null_mut(), 0, &mut ih, &mut oh, null_mut()) };
    assert_eq!(
        take(),
        vec![
            Call::Retain(pointer as usize),
            Call::Framework,
            Call::Acquire(0x3AE4_BEA3, 0x3AE4_BEAB),
            Call::ReleaseTemporary,
            Call::Allocate(12),
            Call::Retain(pointer as usize),
            Call::Allocate(12)
        ]
    );
    assert_eq!(ap, pointer);
    assert_eq!(output.values(), vec![pointer, pointer]);
}

#[test]
fn service_queries_reload_arrays_then_visit_the_pending_list() {
    let _fake = fake(0x006F_ACE0, collect as *const () as usize);
    let mut object = Object::new(0x1C0);
    let p = object.ptr();
    let sim = unsafe { CollectionsSim::from_ptr(p) };
    let mut first = [null_mut()];
    let mut grown = [null_mut::<u8>(); 3];
    let mut hospitals = [null_mut(); 2];
    sim.set_schools(first.as_mut_ptr());
    sim.set_schools_end(first.as_mut_ptr().wrapping_add(1));
    sim.set_hospitals(hospitals.as_mut_ptr());
    sim.set_hospitals_end(hospitals.as_mut_ptr().wrapping_add(2));
    GROW.with(|g| {
        *g.borrow_mut() = Some((
            p as usize,
            grown.as_mut_ptr() as usize,
            grown.as_mut_ptr().wrapping_add(3) as usize,
        ))
    });
    let mut out = null_mut();
    let outp = &mut out as *mut *mut u8;
    let groups = 0x1234usize as *mut u8;
    assert_eq!(
        unsafe { fc4p_006fade0(p, 0, outp, groups) },
        registers(0x1234_5678, 0x9ABC_DEF0)
    );
    let expected = [
        first.as_mut_ptr(),
        grown.as_mut_ptr().wrapping_add(1),
        grown.as_mut_ptr().wrapping_add(2),
        unsafe { p.add(0x19C).cast() },
    ];
    assert_eq!(
        take(),
        expected.map(|l| Call::Collect(p as usize, l as usize, outp as usize, groups as usize))
    );
    unsafe { fc4p_006fae50(p, 0, outp, groups) };
    let expected = [hospitals.as_mut_ptr(), hospitals.as_mut_ptr().wrapping_add(1), unsafe {
        p.add(0x1A0).cast()
    }];
    assert_eq!(
        take(),
        expected.map(|l| Call::Collect(p as usize, l as usize, outp as usize, groups as usize))
    );
    sim.set_hospitals_end(hospitals.as_mut_ptr());
    unsafe { fc4p_006fae50(p, 0, outp, groups) };
    assert_eq!(
        take(),
        [Call::Collect(
            p as usize,
            unsafe { p.add(0x1A0) } as usize,
            outp as usize,
            groups as usize
        )]
    );
}

#[test]
fn record_copy_calls_constructors_and_range_in_order() {
    let _copy = fake(0x005F_4430, copy as *const () as usize);
    let _alloc = fake(0x0090_CF54, alloc as *const () as usize);
    let _insert = fake(0x006F_1570, insert as *const () as usize);
    let mut source = Object::new(0x40);
    let mut destination = Object::new(0x40);
    let s = source.ptr();
    let d = destination.ptr();
    unsafe {
        core::ptr::write_bytes(s, 0xAA, 0x40);
        core::ptr::write_bytes(d, 0xCC, 0x40)
    };
    let mut input = List::new(&[0x1234usize as *mut u8], 12);
    let original = unsafe { Record::from_ptr(s) };
    original.set_list(input.head());
    original.set_first(1);
    original.set_second(2);
    original.set_third(3);
    original.set_last(0x9876_5432);
    assert_eq!(unsafe { fc4p_006fb9f0(d, s) }, registers(d as u32, 0x9876_5432));
    let record = unsafe { Record::from_ptr(d) };
    let head = record.list();
    assert_eq!(
        take(),
        vec![
            Call::Copy(d as usize + 4, s as usize + 4),
            Call::Copy(d as usize + 0x1C, s as usize + 0x1C),
            Call::Allocate(12),
            Call::Insert(
                d as usize + 0x2C,
                head as usize,
                unsafe { Node::from_ptr(input.head()).next() } as usize,
                input.head() as usize
            )
        ]
    );
    assert_eq!(
        (record.first(), record.second(), record.third(), record.last()),
        (1, 2, 3, 0x9876_5432)
    );
    unsafe {
        assert_eq!(Node::from_ptr(head).next(), head);
        assert_eq!(Node::from_ptr(head).previous(), head);
        assert_eq!(*d.add(0x34), 0xCC)
    };
}

#[test]
fn owned_pointer_clear_frees_nonnull_payloads_before_nodes() {
    let _free = fake(0x0090_CF63, release as *const () as usize);
    let mut list = List::new(&[0x1234usize as *mut u8, null_mut(), 0x5678usize as *mut u8], 12);
    let mut head = list.head();
    let nodes: Vec<_> = list.nodes.iter_mut().map(|o| o.ptr() as usize).collect();
    assert_eq!(unsafe { fc4p_006fcbd0(&mut head) }, head as u32);
    assert_eq!(
        take(),
        [
            Call::Free(0x1234),
            Call::Free(nodes[0]),
            Call::Free(nodes[1]),
            Call::Free(0x5678),
            Call::Free(nodes[2])
        ]
    );
    assert!(list.values().is_empty());
    assert_eq!(unsafe { Node::from_ptr(head).previous() }, head);
    unsafe { fc4p_006fcbd0(&mut head) };
    assert!(take().is_empty());
}

#[test]
fn record_clear_deletes_nested_nodes_and_curves_in_order() {
    let _free = fake(0x0090_CF63, release as *const () as usize);
    let mut list = List::new(&[null_mut(), null_mut()], 0x40);
    let mut children = List::new(&[null_mut(), null_mut()], 12);
    let mut empty = List::new(&[], 12);
    let n0 = list.nodes[0].ptr();
    let n1 = list.nodes[1].ptr();
    let h0 = children.head();
    let h1 = empty.head();
    unsafe {
        let r = OwnedRecordNode::from_ptr(n0);
        r.set_nested_head(h0);
        r.set_first_curve(0x1000usize as *mut u8);
        r.set_second_curve(0x2000usize as *mut u8);
        OwnedRecordNode::from_ptr(n1).set_nested_head(h1)
    };
    let c0 = children.nodes[0].ptr();
    let c1 = children.nodes[1].ptr();
    let mut head = list.head();
    assert_eq!(unsafe { fc4p_006fcb40(&mut head) }, head as u32);
    assert_eq!(
        take(),
        [c0, c1, h0, 0x2000usize as *mut u8, 0x1000usize as *mut u8, n0, h1, n1].map(|p| Call::Free(p as usize))
    );
    assert!(list.values().is_empty());
    assert!(children.values().is_empty());
    unsafe { fc4p_006fcb40(&mut head) };
    assert!(take().is_empty());
}

#[test]
fn group_callback_reloads_changed_filter_storage_and_bounds() {
    let mut table = [0usize; 23];
    table[1] = retain as *const () as usize;
    table[22] = group as *const () as usize;
    let mut building = [table.as_ptr() as usize, 0x1510];
    let building = building.as_mut_ptr().cast();
    let mut input = List::new(&[building], 12);
    let mut output = List::new(&[], 12);
    let mut ih = input.head();
    let mut oh = output.head();
    let mut allocator_table = [0usize; 4];
    allocator_table[3] = allocator_alloc as *const () as usize;
    let mut allocator = [allocator_table.as_ptr() as usize];
    let mut ap = allocator.as_mut_ptr().cast();
    let _data = fake_data(0x00B2_07E4, (&mut ap as *mut *mut u8).cast());
    let mut first = [0x9999u32];
    let mut extended = [0x9999u32, 0x1510];
    let mut filter = [first.as_mut_ptr(), first.as_mut_ptr().wrapping_add(1)];
    FILTER_CHANGE.with(|c| {
        *c.borrow_mut() = Some((
            filter.as_mut_ptr() as usize,
            extended.as_mut_ptr() as usize,
            extended.as_mut_ptr().wrapping_add(2) as usize,
        ))
    });
    unsafe { fc4p_006face0(null_mut(), 0, &mut ih, &mut oh, filter.as_mut_ptr().cast()) };
    assert_eq!(
        take(),
        [
            Call::Group(building as usize, 0x9999),
            Call::Group(building as usize, 0x1510),
            Call::Retain(building as usize),
            Call::Allocate(12)
        ]
    );
    assert_eq!(output.values(), [building]);
}

#[test]
fn school_and_hospital_queries_append_group_order_then_filter_pending_buildings() {
    let mut table = [0usize; 23];
    table[1] = retain as *const () as usize;
    table[22] = group as *const () as usize;
    let mut a = [table.as_ptr() as usize, 0x1510];
    let mut b = [table.as_ptr() as usize, 0x151B];
    let mut c = [table.as_ptr() as usize, 0x1510];
    let (a, b, c) = (a.as_mut_ptr().cast(), b.as_mut_ptr().cast(), c.as_mut_ptr().cast());
    let mut first = List::new(&[b], 12);
    let mut second = List::new(&[a], 12);
    let mut pending = List::new(&[c], 12);
    let mut lists = [first.head(), second.head()];
    let mut sim_object = Object::new(0x1C0);
    let sim = unsafe { CollectionsSim::from_ptr(sim_object.ptr()) };
    sim.set_schools(lists.as_mut_ptr());
    sim.set_schools_end(lists.as_mut_ptr().wrapping_add(2));
    sim.set_pending_school(pending.head());
    sim.set_hospitals(lists.as_mut_ptr());
    sim.set_hospitals_end(lists.as_mut_ptr().wrapping_add(2));
    sim.set_pending_hospital(pending.head());
    let mut allocator_table = [0usize; 4];
    allocator_table[3] = allocator_alloc as *const () as usize;
    let mut allocator = [allocator_table.as_ptr() as usize];
    let mut ap = allocator.as_mut_ptr().cast();
    let _data = fake_data(0x00B2_07E4, (&mut ap as *mut *mut u8).cast());
    for school in [false, true] {
        for (ids, expected) in [
            (None, vec![b, a, c]),
            (Some(vec![0x1510]), vec![a, c]),
            (Some(vec![]), vec![]),
        ] {
            let mut ids = ids;
            let mut filter = ids
                .as_mut()
                .map(|v| [v.as_mut_ptr(), v.as_mut_ptr().wrapping_add(v.len())]);
            let groups = filter.as_mut().map_or(null_mut(), |g| g.as_mut_ptr().cast());
            let mut output = List::new(&[], 12);
            let mut out = output.head();
            unsafe {
                if school {
                    fc4p_006fade0(sim.as_ptr(), 0, &mut out, groups)
                } else {
                    fc4p_006fae50(sim.as_ptr(), 0, &mut out, groups)
                }
            };
            assert_eq!(output.values(), expected);
            let retains: Vec<_> = take()
                .into_iter()
                .filter_map(|c| if let Call::Retain(p) = c { Some(p) } else { None })
                .collect();
            assert_eq!(retains, expected.into_iter().map(|p| p as usize).collect::<Vec<_>>());
        }
    }
}
