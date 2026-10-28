// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cases for the core population-transfer and boundary rules, plus container and callback edges.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

const RETURN: u64 = 0x8765_4321_AABB_CCDD;

type Effect = Box<dyn FnOnce(&[u32])>;

struct Step {
    object: u32,
    method: &'static str,
    args: Vec<u32>,
    returned: u64,
    effect: Option<Effect>,
}

thread_local! {
    static STEPS: RefCell<VecDeque<Step>> = const { RefCell::new(VecDeque::new()) };
}

fn expect(object: *mut u8, method: &'static str, args: &[u32], returned: u64) {
    STEPS.with(|s| {
        s.borrow_mut().push_back(Step {
            object: object as u32,
            method,
            args: args.to_vec(),
            returned,
            effect: None,
        })
    });
}

fn effect(f: impl FnOnce(&[u32]) + 'static) {
    STEPS.with(|s| s.borrow_mut().back_mut().unwrap().effect = Some(Box::new(f)));
}

fn run(object: *mut u8, method: &'static str, args: &[u32]) -> u64 {
    let step = STEPS.with(|s| s.borrow_mut().pop_front()).expect("unexpected call");
    assert_eq!((object as u32, method), (step.object, step.method));
    if !step.args.is_empty() {
        assert_eq!(args, step.args);
    }
    if let Some(f) = step.effect {
        f(args);
    }
    step.returned
}

fn finished() {
    STEPS.with(|s| assert!(s.borrow().is_empty(), "uncalled methods remain"));
}

macro_rules! slot {
    ($name:ident $(, $arg:ident)*) => {
        unsafe extern "thiscall" fn $name(this: *mut u8 $(, $arg: u32)*) -> u64 {
            run(this, stringify!($name), &[$($arg),*])
        }
    };
}
slot!(inserted, flag);
slot!(shutdown);
slot!(release);
slot!(add_ref);
slot!(remove, occupant, immediate, flags);
slot!(members);
slot!(reset);
slot!(designate);
slot!(accumulate, source);
slot!(business_unit);
slot!(remove_member, lot);

fn object(table: &[(usize, usize)], size: usize) -> (Vec<usize>, Object) {
    let mut slots = vec![0; 0x108 / 4];
    for &(offset, address) in table {
        slots[offset / 4] = address;
    }
    let mut obj = Object::new(size);
    unsafe {
        obj.ptr().cast::<*const usize>().write(slots.as_ptr());
    }
    (slots, obj)
}

fn node(occupant: *mut u8) -> Object {
    let mut storage = Object::new(12);
    let node = unsafe { OccupantNode::from_ptr(storage.ptr()) };
    node.set_next(storage.ptr());
    node.set_previous(storage.ptr());
    node.set_occupant(occupant);
    storage
}

fn link(nodes: &mut [&mut Object]) {
    for i in 0..nodes.len() {
        let next = nodes[(i + 1) % nodes.len()].ptr();
        let prev = nodes[(i + nodes.len() - 1) % nodes.len()].ptr();
        let n = unsafe { OccupantNode::from_ptr(nodes[i].ptr()) };
        n.set_next(next);
        n.set_previous(prev);
    }
}

unsafe extern "C" fn allocate(size: u32) -> *mut u8 {
    run(core::ptr::null_mut(), "allocate", &[size]) as u32 as *mut u8
}
unsafe extern "C" fn free(storage: *mut u8) -> u64 {
    run(core::ptr::null_mut(), "free", &[storage as u32])
}
unsafe extern "thiscall" fn lookup(this: *mut u8, output: *mut *mut u8, key: *const i32) -> u64 {
    run(this, "lookup", &[output as u32, key as u32])
}
unsafe extern "fastcall" fn release_primary(this: *mut u8, edx: u32) -> u64 {
    run(this, "release_primary", &[edx])
}

#[test]
fn occupant_removal_uses_low_flag_byte_and_rereads_next_and_manager() {
    let (_, mut developer_mem) = object(&[], 0x108);
    let developer = unsafe { LotDeveloper::from_ptr(developer_mem.ptr()) };
    let (_mt, mut manager) = object(&[(0x54, remove as *const () as usize)], 4);
    let (_mt2, mut replacement) = object(&[(0x54, remove as *const () as usize)], 4);
    developer.set_occupant_manager(manager.ptr());
    let (_ot, mut first) = object(
        &[
            (0x80, inserted as *const () as usize),
            (0x10, shutdown as *const () as usize),
            (8, release as *const () as usize),
        ],
        4,
    );
    let (_ot2, mut second) = object(
        &[
            (0x80, inserted as *const () as usize),
            (0x10, shutdown as *const () as usize),
            (8, release as *const () as usize),
        ],
        4,
    );
    let mut sentinel = node(core::ptr::null_mut());
    let mut a = node(first.ptr());
    let mut b = node(second.ptr());
    link(&mut [&mut sentinel, &mut a, &mut b]);
    let mut head = sentinel.ptr();
    expect(first.ptr(), "inserted", &[1], 0x100);
    expect(first.ptr(), "shutdown", &[], RETURN);
    expect(first.ptr(), "release", &[], RETURN);
    let developer_ptr = developer_mem.ptr() as usize;
    let manager_ptr = replacement.ptr() as usize;
    effect(move |_| unsafe {
        LotDeveloper::from_ptr(developer_ptr as *mut u8).set_occupant_manager(manager_ptr as *mut u8)
    });
    expect(second.ptr(), "inserted", &[1], 0x101);
    expect(replacement.ptr(), "remove", &[second.ptr() as u32, 0, 0], RETURN);
    expect(second.ptr(), "shutdown", &[], RETURN);
    expect(second.ptr(), "release", &[], RETURN);
    unsafe {
        assert_eq!(fc4p_006c62f0(developer_mem.ptr(), &mut head), RETURN as u32);
    }
    finished();
    // An empty range neither touches its payload nor calls a method.
    link(&mut [&mut sentinel]);
    unsafe {
        fc4p_006c62f0(developer_mem.ptr(), &mut head);
    }
    finished();
}

#[test]
fn population_transfer_resets_all_members_then_uses_self_or_designate() {
    let (_ut, mut unit) = object(&[(0x14, members as *const () as usize)], 4);
    let slots = [
        (0xE4, reset as *const () as usize),
        (0xEC, designate as *const () as usize),
        (0xE8, accumulate as *const () as usize),
    ];
    let (_at, mut a) = object(&slots, 4);
    let (_bt, mut b) = object(&slots, 4);
    let (_ct, mut receiver) = object(&slots, 4);
    let mut sentinel = node(core::ptr::null_mut());
    let mut an = node(a.ptr());
    let mut bn = node(b.ptr());
    link(&mut [&mut sentinel, &mut an, &mut bn]);
    let mut head = sentinel.ptr();
    expect(unit.ptr(), "members", &[], &mut head as *mut *mut u8 as u64);
    expect(a.ptr(), "reset", &[], RETURN);
    expect(b.ptr(), "reset", &[], RETURN);
    expect(a.ptr(), "designate", &[], 0);
    expect(a.ptr(), "accumulate", &[a.ptr() as u32], RETURN);
    expect(b.ptr(), "designate", &[], receiver.ptr() as u64);
    expect(receiver.ptr(), "accumulate", &[b.ptr() as u32], RETURN);
    unsafe {
        assert_eq!(fc4p_006c6340(core::ptr::null_mut(), 0x1234, unit.ptr()), RETURN);
    }
    finished();
    link(&mut [&mut sentinel]);
    expect(
        unit.ptr(),
        "members",
        &[],
        (RETURN & !0xFFFF_FFFF) | (&mut head as *mut *mut u8 as u32 as u64),
    );
    unsafe {
        assert_eq!(
            fc4p_006c6340(core::ptr::null_mut(), 0, unit.ptr()),
            (RETURN & !0xFFFF_FFFF) | sentinel.ptr() as u32 as u64
        );
    }
    finished();
}

#[test]
fn population_passes_reread_replaced_sentinel_after_callbacks() {
    let (_ut, mut unit) = object(&[(0x14, members as *const () as usize)], 4);
    let (_at, mut a) = object(&[(0xE4, reset as *const () as usize)], 4);
    let mut old = node(core::ptr::null_mut());
    let mut replacement = node(core::ptr::null_mut());
    let mut an = node(a.ptr());
    link(&mut [&mut old, &mut an]);
    let mut head = old.ptr();
    expect(unit.ptr(), "members", &[], &mut head as *mut *mut u8 as u64);
    expect(a.ptr(), "reset", &[], RETURN);
    let hp = &mut head as *mut *mut u8 as usize;
    let np = replacement.ptr() as usize;
    let ap = an.ptr() as usize;
    effect(move |_| unsafe {
        (hp as *mut *mut u8).write(np as *mut u8);
        OccupantNode::from_ptr(ap as *mut u8).set_next(np as *mut u8);
    });
    unsafe {
        assert_eq!(
            fc4p_006c6340(core::ptr::null_mut(), 0, unit.ptr()),
            (RETURN & !0xFFFF_FFFF) | replacement.ptr() as u32 as u64
        );
    }
    finished();
}

#[test]
fn list_copy_addrefs_before_linking_and_keeps_null_payloads() {
    let _alloc = fake(0x0090_CF54, allocate as *const () as usize);
    let (_ot, mut occupant) = object(&[(4, add_ref as *const () as usize)], 4);
    let mut source = node(core::ptr::null_mut());
    let mut a = node(occupant.ptr());
    let mut b = node(core::ptr::null_mut());
    link(&mut [&mut source, &mut a, &mut b]);
    let mut destination = node(core::ptr::null_mut());
    let mut allocated_a = Object::new(12);
    let mut allocated_b = Object::new(12);
    expect(core::ptr::null_mut(), "allocate", &[12], allocated_a.ptr() as u64);
    expect(occupant.ptr(), "add_ref", &[], RETURN);
    let dest = destination.ptr() as usize;
    let payload = allocated_a.ptr() as usize;
    let value = occupant.ptr() as usize;
    effect(move |_| unsafe {
        assert_eq!(OccupantNode::from_ptr(dest as *mut u8).next() as usize, dest);
        assert_eq!(OccupantNode::from_ptr(payload as *mut u8).occupant() as usize, value);
    });
    expect(core::ptr::null_mut(), "allocate", &[12], allocated_b.ptr() as u64);
    unsafe {
        fc4p_006c6ca0(destination.ptr(), a.ptr(), source.ptr(), core::ptr::null_mut());
    }
    finished();
    for (obj, next, previous, occupant) in [
        (
            destination.ptr(),
            allocated_a.ptr(),
            allocated_b.ptr(),
            core::ptr::null_mut(),
        ),
        (allocated_a.ptr(), allocated_b.ptr(), destination.ptr(), occupant.ptr()),
        (
            allocated_b.ptr(),
            destination.ptr(),
            allocated_a.ptr(),
            core::ptr::null_mut(),
        ),
    ] {
        let n = unsafe { OccupantNode::from_ptr(obj) };
        assert_eq!((n.next(), n.previous(), n.occupant()), (next, previous, occupant));
    }
    unsafe {
        fc4p_006c6ca0(destination.ptr(), source.ptr(), source.ptr(), core::ptr::null_mut());
    }
    finished();
}

#[test]
fn boundary_lookup_wraps_unmasked_side_codes_and_reads_fallback_even_on_miss() {
    let _lookup = fake(0x006C_0030, lookup as *const () as usize);
    for (sides, fallback) in [([1i32, 0, 0, 2], false), ([i32::MIN, -2, 17, -3], true), ([0; 4], true)] {
        let mut developer = Object::new(0x108);
        let mut sentinel = Object::new(0x20);
        let mut found = Object::new(0x20);
        let map = unsafe { developer.ptr().add(0xFC) };
        unsafe {
            map.cast::<*mut u8>().write(sentinel.ptr());
        }
        let selected = if fallback { sentinel.ptr() } else { found.ptr() };
        unsafe {
            selected.add(0x14).cast::<u32>().write_unaligned(0x1234_5678);
            selected.add(0x18).write(0xAB);
            selected.add(0x19).write(0xFD);
        }
        let key = sides.into_iter().fold(0i32, |a, b| a.wrapping_shl(4).wrapping_add(b));
        let first = if fallback { sentinel.ptr() } else { found.ptr() } as usize;
        expect(map, "lookup", &[], RETURN);
        effect(move |args| unsafe {
            assert_eq!(*(args[1] as *const i32), key);
            (args[0] as *mut *mut u8).write(first as *mut u8);
        });
        if fallback {
            expect(map, "lookup", &[], RETURN);
            let sp = sentinel.ptr() as usize;
            effect(move |args| unsafe {
                assert_eq!(*(args[1] as *const i32), 0);
                (args[0] as *mut *mut u8).write(sp as *mut u8);
            });
        }
        let (mut texture, mut rotation, mut flip) = (0u32, 0u8, 0u8);
        let returned =
            unsafe { fc4p_006c6cf0(developer.ptr(), sides.as_ptr(), &mut texture, &mut rotation, &mut flip) };
        assert_eq!((texture, rotation, flip), (0x1234_5678, 0xAB, 0xFD));
        assert_eq!(
            returned,
            ((&mut rotation as *mut u8 as u64) << 32) | u64::from((selected as u32 & !255) | 0xFD)
        );
        finished();
    }
}

#[test]
fn pointer_vector_releases_nonnull_elements_then_rereads_storage_and_leaves_words() {
    let _free = fake(0x0090_CF63, free as *const () as usize);
    let (_ot, mut occupant) = object(&[(8, release as *const () as usize)], 4);
    let mut values = [core::ptr::null_mut(), occupant.ptr(), core::ptr::null_mut()];
    let mut vector = [
        values.as_mut_ptr() as u32,
        unsafe { values.as_mut_ptr().add(3) as u32 },
        0x1234_5678,
    ];
    let mut replacement = Object::new(16);
    expect(occupant.ptr(), "release", &[], RETURN);
    let vp = vector.as_mut_ptr() as usize;
    let rp = replacement.ptr() as usize;
    effect(move |_| unsafe {
        (vp as *mut u32).write(rp as u32);
    });
    expect(core::ptr::null_mut(), "free", &[replacement.ptr() as u32], RETURN);
    unsafe {
        assert_eq!(
            call_thiscall_with_eax(fc4p_006c6e90 as *const () as usize, vector.as_mut_ptr().cast(), &[], 0).0,
            RETURN as u32
        );
    }
    assert_eq!(
        vector,
        [
            replacement.ptr() as u32,
            unsafe { values.as_mut_ptr().add(3) as u32 },
            0x1234_5678
        ]
    );
    finished();
    // Both registers survive a wholly empty vector with no storage.
    let mut empty = [0u32; 3];
    let mut c = RawCall::new(
        Conv::Thiscall,
        fc4p_006c6e90 as *const () as usize,
        empty.as_mut_ptr() as u32,
        &[],
    );
    c.eax = 0x1234_5678;
    c.edx = 0xFEDC_BA98;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!((c.eax, c.edx), (0x1234_5678, 0xFEDC_BA98));
    finished();
}

#[test]
fn business_unit_removal_sets_only_dirty_byte_after_removal() {
    let mut memory = Object::new(0x108);
    unsafe {
        memory.ptr().write_bytes(0xA5, 0x108);
    }
    let (_lt, mut lot) = object(&[(0x64, business_unit as *const () as usize)], 4);
    let (_ut, mut unit) = object(&[(0x10, remove_member as *const () as usize)], 4);
    expect(lot.ptr(), "business_unit", &[], unit.ptr() as u64);
    expect(unit.ptr(), "remove_member", &[lot.ptr() as u32], RETURN);
    let dp = memory.ptr() as usize;
    effect(move |_| unsafe {
        assert_eq!(*(dp as *mut u8).add(0x5E), 0xA5);
    });
    unsafe {
        assert_eq!(fc4p_006c7320(memory.ptr(), 0, lot.ptr()), RETURN);
    }
    let mut expected = vec![0xA5; 0x108];
    expected[0x5E] = 1;
    assert_eq!(unsafe { std::slice::from_raw_parts(memory.ptr(), 0x108) }, expected);
    finished();
}

#[test]
fn release_thunks_adjust_this_and_forward_edx_and_return_words() {
    let _release = fake(0x006C_49D0, release_primary as *const () as usize);
    let mut memory = Object::new(0x108);
    for (entry, offset) in [
        (fc4p_006c7360 as *const () as usize, 4),
        (fc4p_006c7370 as *const () as usize, 8),
    ] {
        expect(memory.ptr(), "release_primary", &[0x1234_5678], RETURN);
        let mut c = RawCall::new(Conv::Thiscall, entry, unsafe { memory.ptr().add(offset) as u32 }, &[]);
        c.edx = 0x1234_5678;
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (RETURN as u32, (RETURN >> 32) as u32));
        finished();
    }
}
