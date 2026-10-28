// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Cases for the core population-transfer and boundary rules, plus container and callback edges.

use super::*;
use crate::lot_developer::props::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Object};
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
slot!(get_config);
slot!(configuration_objects);
slot!(position_object, lot, config, record, position, facing);
slot!(create_prop, instance, position, facing);
slot!(set_appearance, appearance);
slot!(enumerate_instances, record, output);
slot!(safe_iterator);
slot!(set_empty, lot);
slot!(normal_textures, lot, config);
slot!(replacement_normal, lot, config);
slot!(get_config_create, create);

unsafe extern "thiscall" fn unregister(this: *mut u8, iterator: *mut *mut u8) -> u64 {
    run(this, "unregister", &[iterator as u32])
}

unsafe extern "C" fn delete_iterator(iterator: *mut u8) -> u64 {
    run(core::ptr::null_mut(), "free_iterator", &[iterator as u32])
}

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

unsafe extern "thiscall" fn choose_instance(this: *mut u8, count: u32) -> u64 {
    run(this, "choose_instance", &[count])
}

unsafe extern "thiscall" fn family_prop(this: *mut u8, lot: *mut u8, index: i32) -> u64 {
    run(this, "family_prop", &[lot as u32, index as u32])
}

#[test]
fn prop_creation_rejects_null_lot_and_null_configuration_before_positioning() {
    for entry in [fc4p_006c6230 as *const () as usize, fc4p_006c6d70 as *const () as usize] {
        let (_lt, mut lot) = object(&[(0x24, get_config as *const () as usize)], 4);
        let arg_count = if entry == fc4p_006c6230 as *const () as usize {
            3
        } else {
            2
        };
        let mut args = [0, 0x1122_3344, 0x5566_7788];
        let mut c = RawCall::new(Conv::Thiscall, entry, 0, &args[..arg_count]);
        c.edx = 0x1234_5678;
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (0, 0x1234_5678));
        finished();
        args[0] = lot.ptr() as u32;
        expect(lot.ptr(), "get_config", &[], RETURN & !0xFFFF_FFFF);
        let mut c = RawCall::new(Conv::Thiscall, entry, 0, &args[..arg_count]);
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (0, (RETURN >> 32) as u32));
        finished();
    }
}

#[test]
fn explicit_prop_positions_before_checking_instance_and_applies_fresh_appearance() {
    for scenario in 0..3 {
        let mut initial = 0x7FC0_0123u32;
        let _initial = fake_data(0x00B4_9AEC, (&mut initial as *mut u32).cast());
        let (_dt, mut developer) = object(&[(0x9C, position_object as *const () as usize)], 0x108);
        let (_lt, mut lot) = object(&[(0x24, get_config as *const () as usize)], 4);
        let (_ct, mut config) = object(&[(0x5C, configuration_objects as *const () as usize)], 4);
        let (_mt, mut manager) = object(&[(0x68, create_prop as *const () as usize)], 4);
        let (_pt, mut prop) = object(&[(0x70, set_appearance as *const () as usize)], 4);
        let mut records = Object::new(104);
        let mut vector = [
            records.ptr() as u32,
            records.ptr() as u32 + 104,
            records.ptr() as u32 + 104,
        ];
        let record = unsafe { records.ptr().add(52) };
        expect(lot.ptr(), "get_config", &[], config.ptr() as u64);
        expect(config.ptr(), "configuration_objects", &[], vector.as_mut_ptr() as u64);
        expect(developer.ptr(), "position_object", &[], RETURN);
        let lot_pointer = lot.ptr() as u32;
        let config_pointer = config.ptr() as u32;
        let record_pointer = record as u32;
        let developer_pointer = developer.ptr() as usize;
        let manager_pointer = manager.ptr() as usize;
        effect(move |args| unsafe {
            assert_eq!(&args[..3], &[lot_pointer, config_pointer, record_pointer]);
            assert_eq!(std::slice::from_raw_parts(args[3] as *const u32, 3), [0x7FC0_0123; 3]);
            assert_eq!(*(args[4] as *const u32), lot_pointer);
            core::ptr::copy_nonoverlapping([10u32, 20, 30].as_ptr(), args[3] as *mut u32, 3);
            (args[4] as *mut u32).write(0x1234_ABCD);
            ConfigurationObject::from_ptr(record_pointer as *mut u8).set_appearance(0x9876_5432);
            LotDeveloper::from_ptr(developer_pointer as *mut u8).set_prop_manager(manager_pointer as *mut u8);
        });
        if scenario != 0 {
            expect(
                manager.ptr(),
                "create_prop",
                &[],
                if scenario == 2 {
                    prop.ptr() as u64
                } else {
                    RETURN & !0xFFFF_FFFF
                },
            );
            effect(|args| unsafe {
                assert_eq!((args[0], args[2]), (0xBEEF, 0x1234_ABCD));
                assert_eq!(std::slice::from_raw_parts(args[1] as *const u32, 3), [10, 20, 30]);
            });
            if scenario == 2 {
                expect(prop.ptr(), "set_appearance", &[0x9876_5432], RETURN);
            }
        }
        let returned =
            unsafe { fc4p_006c6230(developer.ptr(), 0, lot.ptr(), 1, if scenario == 0 { 0 } else { 0xBEEF }) };
        assert_eq!(returned as u32, if scenario == 2 { prop.ptr() as u32 } else { 0 });
        assert_eq!(returned >> 32, RETURN >> 32);
        finished();
    }
}

#[test]
fn family_prop_draws_only_for_nonempty_arrays_and_frees_on_every_exit() {
    let _random = fake(0x0091_36B1, choose_instance as *const () as usize);
    let _free = fake(0x0090_CF63, free as *const () as usize);
    for scenario in 0..5 {
        let mut initial = 0u32;
        let _initial = fake_data(0x00B4_9AEC, (&mut initial as *mut u32).cast());
        let mut random_state = Object::new(16);
        let _state = fake_data(0x00B4_9AF0, random_state.ptr());
        let (_dt, mut developer) = object(&[(0x9C, position_object as *const () as usize)], 0x108);
        let (_lt, mut lot) = object(&[(0x24, get_config as *const () as usize)], 4);
        let (_ct, mut config) = object(&[(0x5C, configuration_objects as *const () as usize)], 4);
        let (_rt, mut resources) = object(&[(0x6C, enumerate_instances as *const () as usize)], 4);
        let (_mt, mut manager) = object(&[(0x68, create_prop as *const () as usize)], 4);
        let (_pt, mut prop) = object(&[(0x70, set_appearance as *const () as usize)], 4);
        unsafe {
            LotDeveloper::from_ptr(developer.ptr()).set_prop_manager(manager.ptr());
        }
        let mut resource_pointer = resources.ptr();
        let _resources = fake_data(0x00B4_3D18, (&mut resource_pointer as *mut *mut u8).cast());
        let mut records = Object::new(104);
        let record = unsafe { records.ptr().add(52) };
        unsafe {
            ConfigurationObject::from_ptr(record).set_appearance(0x8765);
        }
        let mut vector = [records.ptr() as u32, 0, 0];
        let mut instances = [0x1234u32, if scenario == 2 { 0 } else { 0x5678 }];
        let original_instances = instances;
        expect(lot.ptr(), "get_config", &[], config.ptr() as u64);
        expect(config.ptr(), "configuration_objects", &[], vector.as_mut_ptr() as u64);
        expect(developer.ptr(), "position_object", &[], RETURN);
        let rp = record as u32;
        effect(move |args| unsafe {
            assert_eq!(args[2], rp);
            (args[4] as *mut u32).write(3);
        });
        expect(resources.ptr(), "enumerate_instances", &[], RETURN);
        let begin = if scenario == 0 {
            0
        } else {
            instances.as_mut_ptr() as u32
        };
        let end = if scenario <= 1 { begin } else { begin.wrapping_add(8) };
        effect(move |args| unsafe {
            assert_eq!(args[0], rp);
            assert_eq!(std::slice::from_raw_parts(args[1] as *const u32, 3), [0; 3]);
            core::ptr::copy_nonoverlapping([begin, end, end].as_ptr(), args[1] as *mut u32, 3);
        });
        if scenario >= 2 {
            expect(random_state.ptr(), "choose_instance", &[2], 1);
        }
        if scenario >= 3 {
            expect(
                manager.ptr(),
                "create_prop",
                &[],
                if scenario == 4 {
                    prop.ptr() as u64
                } else {
                    RETURN & !0xFFFF_FFFF
                },
            );
            effect(|args| unsafe {
                assert_eq!((args[0], args[2]), (0x5678, 3));
                assert_eq!(std::slice::from_raw_parts(args[1] as *const u32, 3), [0; 3]);
            });
        }
        if scenario == 4 {
            expect(prop.ptr(), "set_appearance", &[0x8765], RETURN);
        }
        if scenario != 0 {
            expect(core::ptr::null_mut(), "free", &[begin], RETURN);
        }
        let returned = unsafe { fc4p_006c6d70(developer.ptr(), 0, lot.ptr(), 1) };
        assert_eq!(returned as u32, if scenario == 4 { prop.ptr() as u32 } else { 0 });
        assert_eq!(returned >> 32, RETURN >> 32);
        assert_eq!(instances, original_instances);
        finished();
    }
}

#[test]
fn family_prop_walk_filters_type_and_flags_and_reloads_array_after_callback() {
    let _family = fake(0x006C_6D70, family_prop as *const () as usize);
    let (_ct, mut config) = object(&[(0x5C, configuration_objects as *const () as usize)], 4);
    let mut developer = Object::new(0x108);
    let mut lot = Object::new(4);
    let mut old_records = Object::new(4 * 52);
    let mut new_records = Object::new(4 * 52);
    for (index, kind, flags) in [(0, 2, 0), (1, 1, 1), (2, 1, 2)] {
        let record = unsafe { ConfigurationObject::from_ptr(old_records.ptr().add(index * 52)) };
        record.set_kind(kind);
        record.set_flags(flags);
    }
    unsafe {
        ConfigurationObject::from_ptr(new_records.ptr().add(3 * 52)).set_kind(1);
    }
    let mut vector = [old_records.ptr() as u32, old_records.ptr() as u32 + 4 * 52, 0];
    expect(config.ptr(), "configuration_objects", &[], vector.as_mut_ptr() as u64);
    expect(developer.ptr(), "family_prop", &[lot.ptr() as u32, 2], RETURN);
    let vp = vector.as_mut_ptr() as usize;
    let next_begin = new_records.ptr() as u32;
    effect(move |_| unsafe {
        (vp as *mut u32).write(next_begin);
    });
    expect(developer.ptr(), "family_prop", &[lot.ptr() as u32, 3], RETURN);
    unsafe {
        assert_eq!(
            fc4p_006c7860(developer.ptr(), 0, lot.ptr(), config.ptr(), 0xFFFF_FFFF),
            RETURN
        );
    }
    finished();
    for difference in [0i32, -1, -52, i32::MIN] {
        vector = [0x8000_0000, 0x8000_0000u32.wrapping_add(difference as u32), 0];
        expect(config.ptr(), "configuration_objects", &[], vector.as_mut_ptr() as u64);
        let product = i64::from(difference) * 0x4EC4_EC4F;
        let returned = unsafe { fc4p_006c7860(developer.ptr(), 0, lot.ptr(), config.ptr(), 0) };
        assert_eq!(returned as u32, product as u32);
        assert_eq!(returned >> 32, ((product >> 32) as i32 >> 4) as u32 as u64);
        finished();
    }
    expect(config.ptr(), "configuration_objects", &[], RETURN & !0xFFFF_FFFF);
    unsafe {
        assert_eq!(
            fc4p_006c7860(developer.ptr(), 0, lot.ptr(), config.ptr(), 0),
            RETURN & !0xFFFF_FFFF
        );
    }
    finished();
}

#[test]
fn unchanged_texture_mode_keeps_upper_eax_and_edx_and_compares_only_the_low_byte() {
    let mut developer = Object::new(0x108);
    for mode in [0u8, 1, 0xFD] {
        unsafe {
            LotDeveloper::from_ptr(developer.ptr()).set_force_empty(mode);
        }
        let args = [0x1234_5600 | u32::from(mode)];
        let mut c = RawCall::new(
            Conv::Thiscall,
            fc4p_006c77d0 as *const () as usize,
            developer.ptr() as u32,
            &args,
        );
        c.eax = 0xAABB_CCDD;
        c.edx = 0x1234_ABCD;
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (0xAABB_CC00 | u32::from(mode), 0x1234_ABCD));
        finished();
    }
}

#[test]
fn texture_mode_advances_iterator_before_callback_rereads_mode_and_keeps_saved_method() {
    let _unregister = fake(0x0080_9580, unregister as *const () as usize);
    let _delete = fake(0x005E_5620, delete_iterator as *const () as usize);
    let (_mt, mut manager) = object(&[(0x3C, safe_iterator as *const () as usize)], 4);
    let mut manager_pointer = manager.ptr();
    let _manager = fake_data(0x00B4_3D08, (&mut manager_pointer as *mut *mut u8).cast());
    let (_dt, mut developer) = object(
        &[
            (0xAC, set_empty as *const () as usize),
            (0xB0, normal_textures as *const () as usize),
        ],
        0x108,
    );
    let (replacement_table, _replacement) = object(&[(0xB0, replacement_normal as *const () as usize)], 4);
    let mut first_lot = Object::new(4);
    let (_lt, mut second_lot) = object(&[(0x24, get_config_create as *const () as usize)], 4);
    let mut sentinel = node(core::ptr::null_mut());
    let mut first = node(first_lot.ptr());
    let mut second = node(second_lot.ptr());
    link(&mut [&mut sentinel, &mut first, &mut second]);
    let mut owner = [sentinel.ptr() as u32, 0, 0];
    let mut iterator = [first.ptr() as u32, owner.as_mut_ptr() as u32];
    expect(manager.ptr(), "safe_iterator", &[], iterator.as_mut_ptr() as u64);
    let dp = developer.ptr() as usize;
    let ip = iterator.as_mut_ptr() as usize;
    let next = second.ptr() as u32;
    effect(move |_| unsafe {
        assert_eq!(LotDeveloper::from_ptr(dp as *mut u8).force_empty(), 0xFD);
    });
    expect(developer.ptr(), "set_empty", &[first_lot.ptr() as u32], RETURN);
    effect(move |_| unsafe {
        assert_eq!(*(ip as *const u32), next);
        LotDeveloper::from_ptr(dp as *mut u8).set_force_empty(0);
    });
    expect(second_lot.ptr(), "get_config_create", &[1], 0xABCD);
    let rt = replacement_table.as_ptr() as usize;
    effect(move |_| unsafe {
        (dp as *mut u32).write(rt as u32);
    });
    expect(
        developer.ptr(),
        "normal_textures",
        &[second_lot.ptr() as u32, 0xABCD],
        RETURN,
    );
    expect(unsafe { owner.as_mut_ptr().add(1).cast() }, "unregister", &[], RETURN);
    let original_iterator = iterator.as_mut_ptr() as u32;
    effect(move |args| unsafe {
        assert_eq!(*(args[0] as *const u32), original_iterator);
    });
    expect(core::ptr::null_mut(), "free_iterator", &[original_iterator], RETURN);
    let returned =
        unsafe { call_thiscall_with_eax(fc4p_006c77d0 as *const () as usize, developer.ptr(), &[0x1234_56FD], 0) };
    assert_eq!(returned, (RETURN as u32, (RETURN >> 32) as u32));
    assert_eq!(iterator[0], sentinel.ptr() as u32);
    finished();
}

#[test]
fn texture_mode_handles_detached_and_empty_iterators_and_callback_detachment() {
    let _unregister = fake(0x0080_9580, unregister as *const () as usize);
    let _delete = fake(0x005E_5620, delete_iterator as *const () as usize);
    for scenario in 0..3 {
        let (_mt, mut manager) = object(&[(0x3C, safe_iterator as *const () as usize)], 4);
        let mut manager_pointer = manager.ptr();
        let _manager = fake_data(0x00B4_3D08, (&mut manager_pointer as *mut *mut u8).cast());
        let (_dt, mut developer) = object(&[(0xAC, set_empty as *const () as usize)], 0x108);
        let mut lot = Object::new(4);
        let mut sentinel = node(core::ptr::null_mut());
        let mut member = node(lot.ptr());
        link(&mut [&mut sentinel, &mut member]);
        let mut owner = [sentinel.ptr() as u32, 0, 0];
        let mut iterator = [
            if scenario == 2 {
                member.ptr() as u32
            } else {
                sentinel.ptr() as u32
            },
            if scenario == 0 { 0 } else { owner.as_mut_ptr() as u32 },
        ];
        expect(manager.ptr(), "safe_iterator", &[], iterator.as_mut_ptr() as u64);
        if scenario == 2 {
            expect(developer.ptr(), "set_empty", &[lot.ptr() as u32], RETURN);
            let ip = iterator.as_mut_ptr() as usize;
            effect(move |_| unsafe {
                (ip as *mut u32).add(1).write(0);
            });
        }
        if scenario == 1 {
            expect(unsafe { owner.as_mut_ptr().add(1).cast() }, "unregister", &[], RETURN);
        }
        expect(
            core::ptr::null_mut(),
            "free_iterator",
            &[iterator.as_mut_ptr() as u32],
            RETURN,
        );
        unsafe {
            call_thiscall_with_eax(fc4p_006c77d0 as *const () as usize, developer.ptr(), &[1], 0);
        }
        finished();
    }
}

#[test]
fn inflated_rectangle_calls_original_helper_and_preserves_source_and_aliasing() {
    let _inflate = fake(0x006C_6010, inflate_copy as *const () as usize);
    for alias in [false, true] {
        let mut source = [1u32, 2, 3, 4];
        let mut destination = [0xA5A5_A5A5u32; 4];
        let out = if alias {
            source.as_mut_ptr()
        } else {
            destination.as_mut_ptr()
        };
        // The temporary rectangle's address is private, so check it in the callback.
        expect(core::ptr::null_mut(), "inflate", &[i32::MIN as u32], RETURN);
        let returned = unsafe {
            call_thiscall_with_eax(
                fc4p_006c6860 as *const () as usize,
                source.as_mut_ptr().cast(),
                &[out as u32, i32::MIN as u32],
                0,
            )
        };
        assert_eq!(returned, (out as u32, 0x8765_4321));
        assert_eq!(unsafe { std::slice::from_raw_parts(out, 4) }, [5, 6, 7, 0x8765_4321]);
        if !alias {
            assert_eq!(source, [1, 2, 3, 4]);
        }
        finished();
    }
}

unsafe extern "thiscall" fn inflate_copy(this: *mut u8, amount: i32) -> u64 {
    unsafe {
        assert_eq!(std::slice::from_raw_parts(this.cast::<u32>(), 4), [1, 2, 3, 4]);
        core::ptr::copy_nonoverlapping([5u32, 6, 7, 0x8765_4321].as_ptr(), this.cast::<u32>(), 4);
    }
    run(core::ptr::null_mut(), "inflate", &[amount as u32])
}
