// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Allocate(u32),
    Balance(u32, u32, u32, u32, u32),
    Copy(u32, u32),
    Previous(u32),
    Insert(u32, u32, u32, u32, u32),
    Unlink(u32, u32, u32, u32),
    Free(u32),
    Delete(u32, u32),
    Element(u32, u32),
    Range(u32, u32, u32, u8),
    Destroy(u32, u32, u8),
}
#[derive(Default)]
struct State {
    allocations: Vec<Object>,
    events: Vec<Event>,
    map: u32,
    out: u32,
    inserted: u32,
    previous: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "C" fn alloc_fake(size: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut o = Object::new(size as usize);
        let p = o.ptr();
        unsafe { core::ptr::write_bytes(p, 0xA5, size as usize) };
        s.events.push(Event::Allocate(size));
        s.allocations.push(o);
        (0xBAD0_CAFEu64 << 32) | u64::from(p as u32)
    })
}
extern "C" fn balance_fake(p: *mut u8, root: *mut u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe {
            let n = node(p);
            let event = Event::Balance(
                p as u32,
                root as u32,
                n.parent() as u32,
                tree(s.map as *mut u8).count(),
                read(s.out as *const u32),
            );
            s.events.push(event);
            assert!(n.left().is_null() && n.right().is_null());
            n.set_color(1);
        }
    });
    0x1234_5678_AABB_CCDD
}
extern "thiscall" fn copy_fake(this: *mut u8, source: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().events.push(Event::Copy(this as u32, source as u32)));
    unsafe {
        write(this.cast::<u32>(), 0x1234);
        write(this.add(4).cast::<u32>(), 0x5678)
    };
    (0xDEAD_BEEFu64 << 32) | u64::from(this as u32)
}
extern "thiscall" fn previous_fake(slot: *mut u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.events.push(Event::Previous(unsafe { slot.read() }));
        unsafe { slot.write(s.previous) };
    });
    0x0123_4567_89AB_CDEF
}
extern "thiscall" fn insert_fake(
    this: *mut u8,
    out: *mut u32,
    child: u32,
    parent: u32,
    source: u32,
    force: u32,
) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(unsafe { out.read() }, s.previous);
        s.events.push(Event::Insert(this as u32, child, parent, source, force));
        unsafe { out.write(s.inserted) };
    });
    (0xFEED_FACEu64 << 32) | u64::from(out as u32)
}

// Port addresses resolve through the test table; direct entries are invoked with the public raw helper.
fn entry(f: usize, this: *mut u8, args: &[u32]) -> (u32, u32) {
    unsafe { crate::testing::call_thiscall_with_eax(f, this, args, 0xAABB_CCDD) }
}

struct World {
    map: Object,
    header: Object,
    parent: Object,
    out: [u32; 2],
    source: [u32; 3],
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            map: Object::new(8),
            header: Object::new(24),
            parent: Object::new(24),
            out: [0xCCDD_EEFF; 2],
            source: [10, 20, 30],
        };
        unsafe {
            let h = w.header.ptr();
            tree(w.map.ptr()).set_header(h);
            node(h).set_left(h);
            node(h).set_right(h);
        }
        w
    }
    fn setup_state(&mut self) {
        let map = self.map.ptr() as u32;
        let out = self.out.as_mut_ptr() as u32;
        STATE.with(|s| {
            s.borrow_mut().map = map;
            s.borrow_mut().out = out;
        });
    }
}

#[test]
fn inserts_initialize_only_payload_links_and_balancer_color() {
    let _a = fake(0x0090_CF54, alloc_fake as *const () as usize);
    let _b = fake(0x0040_D5E0, balance_fake as *const () as usize);
    let _c = fake(0x0069_8A80, copy_fake as *const () as usize);
    for (f, nested) in [
        (fc4p_00696060 as *const () as usize, false),
        (fc4p_00696880 as *const () as usize, false),
        (fc4p_0069a610 as *const () as usize, true),
    ] {
        let mut w = World::new();
        w.setup_state();
        let h = w.header.ptr();
        unsafe { tree(w.map.ptr()).set_count(u32::MAX) };
        let args = [w.out.as_mut_ptr() as u32, 0, h as u32, w.source.as_mut_ptr() as u32, 7];
        assert_eq!(entry(f, w.map.ptr(), &args), (args[0], 0x1234_5678));
        let p = w.out[0] as *mut u8;
        unsafe {
            assert_eq!(tree(w.map.ptr()).count(), 0);
            assert_eq!((node(h).parent(), node(h).left(), node(h).right()), (p, p, p));
            assert_eq!(node(p).key(), 10);
            assert_eq!(std::slice::from_raw_parts(p.add(1), 3), &[0xA5; 3]);
            if nested {
                assert_eq!(
                    (read(p.add(20).cast::<u32>()), read(p.add(24).cast::<u32>())),
                    (0x1234, 0x5678)
                );
            } else {
                assert_eq!(node(p).value(), 20);
            }
        }
        let events = STATE.with(|s| std::mem::take(&mut s.borrow_mut().events));
        let mut want = vec![Event::Allocate(if nested { 32 } else { 24 })];
        if nested {
            want.push(Event::Copy(p as u32 + 20, args[3] + 4));
        }
        want.push(Event::Balance(p as u32, h as u32 + 4, h as u32, u32::MAX, 0xCCDD_EEFF));
        assert_eq!(events, want);
        assert_eq!(w.out[1], 0xCCDD_EEFF);
    }
}

#[test]
fn insertion_side_obeys_signed_pairs_unsigned_keys_and_hints() {
    let _a = fake(0x0090_CF54, alloc_fake as *const () as usize);
    let _b = fake(0x0040_D5E0, balance_fake as *const () as usize);
    let _c = fake(0x0069_8A80, copy_fake as *const () as usize);
    for (pair, nested, child, force, a, b, left) in [
        (true, false, 0, 0, 0xFFFFFFFF, 20, true),
        (false, false, 0, 0, 0xFFFFFFFF, 20, false),
        (true, false, 0, 0, 10, 19, true),
        (true, false, 0, 0, 10, 20, false),
        (false, false, 1, 0, 99, 0, true),
        (false, false, 1, 1, 0, 0, false),
        (false, true, 0, 0, 9, 0, true),
        (false, true, 0, 0, 11, 0, false),
    ] {
        let mut w = World::new();
        w.setup_state();
        let (h, p) = (w.header.ptr(), w.parent.ptr());
        unsafe {
            node(h).set_parent(p);
            node(h).set_left(p);
            node(h).set_right(p);
            node(p).set_key(10);
            node(p).set_value(20);
        }
        w.source[0] = a;
        w.source[1] = b;
        let f = if pair {
            fc4p_00696060 as *const () as usize
        } else if nested {
            fc4p_0069a610 as *const () as usize
        } else {
            fc4p_00696880 as *const () as usize
        };
        entry(
            f,
            w.map.ptr(),
            &[
                w.out.as_mut_ptr() as u32,
                child,
                p as u32,
                w.source.as_mut_ptr() as u32,
                force,
            ],
        );
        let n = w.out[0] as *mut u8;
        unsafe {
            assert_eq!(if left { node(p).left() } else { node(p).right() }, n);
            assert_eq!(if left { node(h).left() } else { node(h).right() }, n);
        }
    }
}

#[test]
fn unique_insert_handles_empty_minimum_predecessor_and_duplicate() {
    let _p = fake(0x0068_09E0, previous_fake as *const () as usize);
    let guards = [0x0069_6060, 0x006C_03E0, 0x0069_A610].map(|a| fake(a, insert_fake as *const () as usize));
    for f in [
        fc4p_00696300 as *const () as usize,
        fc4p_006967c0 as *const () as usize,
        fc4p_0069a6e0 as *const () as usize,
    ] {
        for mode in 0..4 {
            let mut w = World::new();
            let (h, p) = (w.header.ptr(), w.parent.ptr());
            let mut predecessor = Object::new(24);
            let prev = predecessor.ptr();
            unsafe {
                node(p).set_key(10);
                node(p).set_value(20);
                node(prev).set_key(5);
                if mode != 0 {
                    node(h).set_parent(p);
                    node(h).set_left(if mode == 2 { prev } else { p });
                }
            }
            w.source[0] = match mode {
                0 => 1,
                1 | 2 => 9,
                _ => 10,
            };
            w.source[1] = 20;
            let candidate = if mode == 0 {
                h
            } else if mode == 2 {
                prev
            } else {
                p
            };
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.previous = candidate as u32;
                s.inserted = 0x1234_5678;
            });
            let out = w.out.as_mut_ptr() as u32;
            let source = w.source.as_mut_ptr() as u32;
            let result = entry(f, w.map.ptr(), &[out, source]);
            assert_eq!(result.0, out);
            assert_eq!(w.out[0], if mode == 3 { p as u32 } else { 0x1234_5678 });
            assert_eq!(w.out[1], if mode == 3 { 0xCCDD_EE00 } else { 0xCCDD_EE01 });
            let mut want = vec![];
            if mode == 2 {
                want.push(Event::Previous(p as u32));
            }
            if mode != 3 {
                want.push(Event::Insert(
                    w.map.ptr() as u32,
                    0,
                    if mode == 0 { h as u32 } else { p as u32 },
                    source,
                    0,
                ));
            }
            assert_eq!(STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)), want);
            if mode == 3 && f != fc4p_00696300 as *const () as usize {
                assert_eq!(result.1, p as u32);
            }
        }
    }
    drop(guards);
}

/// Records the argument slot immediately after the callee pops it, before any compiler stack reuse.
fn unique_with_slot(f: usize, this: *mut u8, out: *mut u32, source: *mut u32) -> [u32; 4] {
    let mut words = [f as u32, this as u32, out as u32, source as u32, 0, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 12]", "push dword ptr [edi + 8]", "mov ecx, [edi + 4]",
            "call dword ptr [edi]", "mov [edi + 16], eax", "mov [edi + 20], edx",
            "mov [edi + 24], ecx", "mov eax, [esp - 4]", "mov [edi + 28], eax",
            in("edi") words.as_mut_ptr(), clobber_abi("C"),
        );
    }
    words[4..].try_into().unwrap()
}

#[test]
fn unique_insert_writes_the_original_argument_slot_and_preserves_duplicate_result_registers() {
    for f in [
        fc4p_00696300 as *const () as usize,
        fc4p_006967c0 as *const () as usize,
        fc4p_0069a6e0 as *const () as usize,
    ] {
        let mut w = World::new();
        let (h, p) = (w.header.ptr(), w.parent.ptr());
        unsafe {
            node(h).set_parent(p);
            node(h).set_left(p);
            node(p).set_key(10);
            node(p).set_value(20);
        }
        let r = unique_with_slot(f, w.map.ptr(), w.out.as_mut_ptr(), w.source.as_mut_ptr());
        assert_eq!(r[0], w.out.as_mut_ptr() as u32);
        assert_eq!(r[3], p as u32);
        if f == fc4p_00696300 as *const () as usize {
            assert_eq!((r[1], r[2]), (10, p as u32));
        } else {
            assert_eq!((r[1], r[2]), (p as u32, h as u32));
        }
    }
}

#[test]
fn subtree_copy_visits_right_subtrees_before_the_left_spine_and_keeps_padding() {
    let _a = fake(0x0090_CF54, alloc_fake as *const () as usize);
    let mut w = World::new();
    let mut nodes: Vec<Object> = (0..5).map(|_| Object::new(24)).collect();
    let p: Vec<_> = nodes.iter_mut().map(Object::ptr).collect();
    unsafe {
        for (i, &p) in p.iter().enumerate() {
            node(p).set_color((i + 1) as u8);
            node(p).set_key(i as u32);
            node(p).set_value(100 + i as u32);
        }
        node(p[0]).set_right(p[1]);
        node(p[0]).set_left(p[2]);
        node(p[2]).set_right(p[3]);
        node(p[2]).set_left(p[4]);
    }
    let r = entry(
        fc4p_00696f10 as *const () as usize,
        w.map.ptr(),
        &[p[0] as u32, w.header.ptr() as u32],
    );
    assert_eq!(r.1, 104);
    let allocated: Vec<_> = STATE.with(|s| s.borrow_mut().allocations.iter_mut().map(Object::ptr).collect());
    assert_eq!(allocated.len(), 5);
    assert_eq!(r.0, allocated[0] as u32);
    for (i, &d) in allocated.iter().enumerate() {
        unsafe {
            assert_eq!(
                (node(d).key(), node(d).value(), node(d).color()),
                (i as u32, 100 + i as u32, (i + 1) as u8)
            );
            assert_eq!(std::slice::from_raw_parts(d.add(1), 3), &[0xA5; 3]);
            assert_eq!(
                node(d).parent(),
                if i == 0 {
                    w.header.ptr()
                } else {
                    allocated[if i <= 2 { 0 } else { 2 }]
                }
            );
        }
    }
    unsafe {
        assert_eq!(
            (node(allocated[0]).right(), node(allocated[0]).left()),
            (allocated[1], allocated[2])
        );
        assert_eq!(
            (node(allocated[2]).right(), node(allocated[2]).left()),
            (allocated[3], allocated[4])
        );
    }
    assert_eq!(
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)),
        vec![Event::Allocate(24); 5]
    );
}

#[test]
fn tree_copies_build_headers_and_copy_count_without_rebalancing() {
    let _a = fake(0x0090_CF54, alloc_fake as *const () as usize);
    for f in [fc4p_006989d0 as *const () as usize, fc4p_00698a80 as *const () as usize] {
        for populated in [false, true] {
            let mut w = World::new();
            let mut dest = Object::new(8);
            let mut left = Object::new(24);
            let mut right = Object::new(24);
            let (h, p, l, r) = (w.header.ptr(), w.parent.ptr(), left.ptr(), right.ptr());
            unsafe {
                if populated {
                    node(h).set_parent(p);
                    node(p).set_left(l);
                    node(p).set_right(r);
                    node(p).set_key(10);
                    node(l).set_key(5);
                    node(r).set_key(15);
                }
                tree(w.map.ptr()).set_count(if populated { 0xFFFF_FFFE } else { 0xBAD0 });
            }
            let result = entry(f, dest.ptr(), &[w.map.ptr() as u32]);
            assert_eq!(
                result,
                (dest.ptr() as u32, if populated { 0xFFFF_FFFE } else { 0xBAD0 })
            );
            unsafe {
                let d = tree(dest.ptr());
                let dh = d.header();
                assert_eq!(node(dh).color(), 0);
                assert_eq!(std::slice::from_raw_parts(dh.add(1), 3), &[0xA5; 3]);
                assert_eq!(d.count(), tree(w.map.ptr()).count());
                if populated {
                    assert_eq!(node(node(dh).parent()).parent(), dh);
                    assert_eq!((node(node(dh).left()).key(), node(node(dh).right()).key()), (5, 15));
                } else {
                    assert!(node(dh).parent().is_null());
                    assert_eq!((node(dh).left(), node(dh).right()), (dh, dh));
                }
            }
            assert_eq!(
                STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)),
                vec![Event::Allocate(24); if populated { 4 } else { 1 }]
            );
        }
    }
}

extern "C" fn unlink_fake(p: *mut u8, root: u32, first: u32, last: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().events.push(Event::Unlink(p as u32, root, first, last)));
    (0xCAFE_BABEu64 << 32) | u64::from(p as u32)
}
extern "C" fn free_fake(p: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().events.push(Event::Free(p as u32)));
    0x1234_5678_8765_4321
}
extern "thiscall" fn delete_fake(this: *mut u8, root: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().events.push(Event::Delete(this as u32, root as u32)));
    0x1111_2222_3333_4444
}

#[test]
fn erasure_uses_one_subtree_call_for_full_ranges_and_unlinks_partial_ranges_in_order() {
    let _u = fake(0x0040_DAF0, unlink_fake as *const () as usize);
    let _f = fake(0x0090_CF63, free_fake as *const () as usize);
    let _d = fake(0x0069_5CD0, delete_fake as *const () as usize);
    for full in [false, true] {
        let mut w = World::new();
        let mut left = Object::new(24);
        let (h, p, l) = (w.header.ptr(), w.parent.ptr(), left.ptr());
        unsafe {
            node(h).set_parent(p);
            node(h).set_left(l);
            node(h).set_right(p);
            node(p).set_parent(h);
            node(p).set_left(l);
            node(l).set_parent(p);
            tree(w.map.ptr()).set_count(2);
        }
        entry(
            fc4p_00696240 as *const () as usize,
            w.map.ptr(),
            &[l as u32, if full { h as u32 } else { p as u32 }],
        );
        let events = STATE.with(|s| std::mem::take(&mut s.borrow_mut().events));
        if full {
            assert_eq!(events, [Event::Delete(w.map.ptr() as u32, p as u32)]);
            unsafe {
                assert!(node(h).parent().is_null());
                assert_eq!((node(h).left(), node(h).right(), tree(w.map.ptr()).count()), (h, h, 0));
            }
        } else {
            assert_eq!(
                events,
                [
                    Event::Unlink(l as u32, h as u32 + 4, h as u32 + 8, h as u32 + 12),
                    Event::Free(l as u32)
                ]
            );
            assert_eq!(unsafe { tree(w.map.ptr()).count() }, 1);
        }
    }
    let mut w = World::new();
    let h = w.header.ptr();
    entry(fc4p_00696240 as *const () as usize, w.map.ptr(), &[h as u32, h as u32]);
    assert!(STATE.with(|s| s.borrow().events.is_empty()));
}

extern "thiscall" fn element_fake(dest: *mut u8, source: *mut u8) {
    STATE.with(|s| s.borrow_mut().events.push(Event::Element(dest as u32, source as u32)));
    unsafe { core::ptr::copy_nonoverlapping(source, dest, 12) };
}
extern "C" fn range_fake(mut source: *mut u8, end: *mut u8, mut dest: *mut u8, cookie: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::Range(source as u32, end as u32, dest as u32, unsafe { *cookie }))
    });
    unsafe {
        while source != end {
            core::ptr::copy_nonoverlapping(source, dest, 12);
            source = source.add(12);
            dest = dest.add(12);
        }
        *cookie = 0x91;
    }
    dest
}
extern "C" fn destroy_fake(source: *mut u8, end: *mut u8, cookie: *mut u8) {
    STATE.with(|s| {
        s.borrow_mut()
            .events
            .push(Event::Destroy(source as u32, end as u32, unsafe { *cookie }))
    });
}

#[test]
fn object_vector_growth_copies_prefix_repeats_value_optionally_copies_tail_then_destroys_and_frees() {
    let _a = fake(0x0090_CF54, alloc_fake as *const () as usize);
    let _c = fake(0x004C_9470, element_fake as *const () as usize);
    let _r = fake(0x0069_9150, range_fake as *const () as usize);
    let _d = fake(0x0045_9170, destroy_fake as *const () as usize);
    let _f = fake(0x0090_CF63, free_fake as *const () as usize);
    for (requested, skip) in [(0, 0), (1, 0), (2, 0), (4, 0), (2, 0x80)] {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut object = Object::new(12);
        let mut old = Object::new(36);
        let (v, base) = (object.ptr(), old.ptr());
        let vector = unsafe { super::super::shared::ObjectVector::from_ptr(v) };
        vector.set_begin(base);
        vector.set_end(base.wrapping_add(36));
        vector.set_limit(base.wrapping_add(36));
        unsafe {
            for i in 0..9 {
                write(base.add(i * 4).cast::<u32>(), i as u32);
            }
        }
        let mut value = [0x1111u32, 0x2222, 0x3333];
        let source = value.as_mut_ptr().cast::<u8>();
        let args = [
            base as u32 + 12,
            source as u32,
            0xCAFE_BABE,
            requested,
            0xDEAD_BE00 | skip,
        ];
        let result = entry(fc4p_0069a7c0 as *const () as usize, v, &args);
        let allocation = vector.begin();
        let count = 3 + requested.max(3);
        assert_eq!(result, (allocation as u32, allocation as u32 + count * 12));
        assert_eq!(vector.limit(), allocation.wrapping_add(count as usize * 12));
        assert_eq!(
            vector.end(),
            allocation.wrapping_add((1 + requested + if skip == 0 { 2 } else { 0 }) as usize * 12)
        );
        let mut want = vec![
            Event::Allocate(count * 12),
            Event::Element(allocation as u32, base as u32),
        ];
        for i in 0..requested {
            want.push(Event::Element(allocation as u32 + 12 * (i + 1), source as u32));
        }
        if skip == 0 {
            want.push(Event::Range(
                base as u32 + 12,
                base as u32 + 36,
                allocation as u32 + 12 * (requested + 1),
                0,
            ));
        }
        want.push(Event::Destroy(
            base as u32,
            base as u32 + 36,
            if skip == 0 { 0x91 } else { skip as u8 },
        ));
        want.push(Event::Free(base as u32));
        assert_eq!(STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)), want);
        unsafe {
            assert_eq!(read(allocation.cast::<[u32; 3]>()), [0, 1, 2]);
            for i in 0..requested {
                assert_eq!(read(allocation.add((i as usize + 1) * 12).cast::<[u32; 3]>()), value);
            }
        }
    }
    STATE.with(|s| *s.borrow_mut() = State::default());
    let mut object = Object::new(12);
    let v = object.ptr();
    assert_eq!(entry(fc4p_0069a7c0 as *const () as usize, v, &[0, 0, 0, 0, 0]), (0, 0));
    assert_eq!(
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().events)),
        [Event::Range(0, 0, 0, 0), Event::Destroy(0, 0, 0x91)]
    );
}
